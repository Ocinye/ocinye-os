//! MFA obrigatório, contra a base de dados (ADR-0107).
//!
//! A correcção do TOTP em si — que o código é o que um autenticador espera —
//! está fixada pelos vectores da RFC 6238 nos testes unitários do módulo. Aqui
//! provam-se as propriedades que só existem com estado guardado: enrolar,
//! confirmar, desafiar, **não** aceitar um passo já gasto, gastar um código de
//! recuperação uma só vez, e revogar a sessão de um membro sem tocar na de
//! outro.

use hmac::{Hmac, Mac};
use sha1::Sha1;
use sqlx::PgPool;
use uuid::Uuid;

use ocinye_core::modules::identity::{self, Person};
use ocinye_core::password::sealed::SealingKey;
use ocinye_observability::CorrelationIds;

async fn pool() -> Option<PgPool> {
    let Ok(url) = std::env::var("OCINYE_TEST_DATABASE_URL") else {
        assert!(
            std::env::var("CI").is_err(),
            "OCINYE_TEST_DATABASE_URL em falta em CI: o MFA ficaria por verificar"
        );
        eprintln!("SALTADO: OCINYE_TEST_DATABASE_URL não está definida.");
        return None;
    };
    let pool = PgPool::connect(&url).await.expect("base");
    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .expect("migrations");
    ocinye_core::fixtures::refuse_canonical_organisation(&pool).await;
    Some(pool)
}

async fn organizacao(pool: &PgPool) -> Uuid {
    let slug = format!("m{}", Uuid::new_v4().simple());
    sqlx::query_scalar("INSERT INTO organisations (slug, name) VALUES ($1,$1) RETURNING id")
        .bind(&slug)
        .fetch_one(pool)
        .await
        .expect("organização")
}

async fn pessoa(pool: &PgPool, org: Uuid) -> Person {
    let email = format!("u{}@ocinye.com", Uuid::new_v4().simple());
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO people (organisation_id, email, full_name, status)
         VALUES ($1, $2, 'Alguém', 'active') RETURNING id",
    )
    .bind(org)
    .bind(email)
    .fetch_one(pool)
    .await
    .expect("pessoa");
    identity::person_by_id(pool, id)
        .await
        .expect("leitura")
        .expect("existe")
}

async fn sessao_activa(pool: &PgPool, person_id: Uuid) -> Uuid {
    let digest = format!("{:064x}", u128::from_le_bytes(*Uuid::new_v4().as_bytes()));
    sqlx::query_scalar(
        "INSERT INTO sessions (person_id, token_digest, state, expires_at)
         VALUES ($1, $2, 'active', now() + interval '1 hour') RETURNING id",
    )
    .bind(person_id)
    .bind(digest)
    .fetch_one(pool)
    .await
    .expect("sessão")
}

fn chave() -> SealingKey {
    SealingKey::from_base64(&SealingKey::generate()).expect("chave")
}

/// Gera o código TOTP de um seed num instante, como um autenticador faria.
fn codigo(seed_base32: &str, unix: i64) -> String {
    let seed = data_encoding::BASE32_NOPAD
        .decode(seed_base32.as_bytes())
        .expect("base32");
    let counter = (unix as u64) / 30;
    let mut mac = <Hmac<Sha1> as Mac>::new_from_slice(&seed).expect("hmac");
    mac.update(&counter.to_be_bytes());
    let d = mac.finalize().into_bytes();
    let off = (d[d.len() - 1] & 0x0f) as usize;
    let bin = (u32::from(d[off] & 0x7f) << 24)
        | (u32::from(d[off + 1]) << 16)
        | (u32::from(d[off + 2]) << 8)
        | u32::from(d[off + 3]);
    format!("{:06}", bin % 1_000_000)
}

/// Enrolar, confirmar, desafiar — e um passo já gasto não volta a valer.
#[tokio::test]
async fn enrolar_confirmar_desafiar_e_recusar_replay() {
    let Some(pool) = pool().await else {
        return;
    };
    let org = organizacao(&pool).await;
    let person = pessoa(&pool, org).await;
    let raiz = chave();
    let actor = identity::principal_for_person(&pool, &person)
        .await
        .expect("principal");
    let ids = CorrelationIds::generate();

    assert!(!identity::has_confirmed_totp(&pool, person.id)
        .await
        .unwrap());

    let enr = identity::begin_enrollment(&pool, Some(&raiz), &person, "Ocinye")
        .await
        .expect("enrolar");

    // Reaproveitamento: pedir outra vez devolve o mesmo seed (QR == chave
    // manual), e não gera outro.
    let outra = identity::begin_enrollment(&pool, Some(&raiz), &person, "Ocinye")
        .await
        .expect("enrolar de novo");
    assert_eq!(enr.secret_base32, outra.secret_base32);

    let agora = chrono::Utc::now().timestamp();
    let recuperacao = identity::confirm_enrollment(
        &pool,
        Some(&raiz),
        &hasher(),
        &actor,
        &person,
        &codigo(&enr.secret_base32, agora),
        &ids,
    )
    .await
    .expect("confirmar");
    assert_eq!(recuperacao.len(), 10, "esperava dez códigos de recuperação");
    assert!(identity::has_confirmed_totp(&pool, person.id).await.unwrap());

    // Simula que o passo do desafio é posterior ao da confirmação: sem isto o
    // relógio real teria de avançar 30 s. O que se prova é o mecanismo de passo
    // monótono, não o relógio.
    sqlx::query("UPDATE mfa_totp_secrets SET last_accepted_step = NULL WHERE person_id = $1")
        .bind(person.id)
        .execute(&pool)
        .await
        .unwrap();

    let atual = codigo(&enr.secret_base32, chrono::Utc::now().timestamp());
    assert!(
        identity::verify_challenge(&pool, Some(&raiz), person.id, &atual)
            .await
            .unwrap(),
        "o desafio com o código actual devia passar"
    );
    // O mesmo código, o mesmo passo — recusado (replay).
    assert!(
        !identity::verify_challenge(&pool, Some(&raiz), person.id, &atual)
            .await
            .unwrap(),
        "um passo já aceite foi aceite outra vez"
    );

    // Sem raiz de selagem, o factor não se verifica — e não se assume satisfeito.
    assert!(
        !identity::verify_challenge(&pool, None, person.id, &atual)
            .await
            .unwrap(),
        "sem chave, o desafio devia recusar (fail closed)"
    );
}

/// Um código de recuperação vale exactamente uma vez.
#[tokio::test]
async fn codigo_de_recuperacao_e_de_uso_unico() {
    let Some(pool) = pool().await else {
        return;
    };
    let org = organizacao(&pool).await;
    let person = pessoa(&pool, org).await;
    let raiz = chave();
    let actor = identity::principal_for_person(&pool, &person)
        .await
        .expect("principal");
    let ids = CorrelationIds::generate();

    let enr = identity::begin_enrollment(&pool, Some(&raiz), &person, "Ocinye")
        .await
        .expect("enrolar");
    let recuperacao = identity::confirm_enrollment(
        &pool,
        Some(&raiz),
        &hasher(),
        &actor,
        &person,
        &codigo(&enr.secret_base32, chrono::Utc::now().timestamp()),
        &ids,
    )
    .await
    .expect("confirmar");

    let um = &recuperacao[0];
    assert!(
        identity::consume_recovery_code(&pool, &hasher(), &actor, person.id, um, &ids)
            .await
            .unwrap(),
        "o primeiro uso do código devia servir"
    );
    assert!(
        !identity::consume_recovery_code(&pool, &hasher(), &actor, person.id, um, &ids)
            .await
            .unwrap(),
        "o segundo uso do mesmo código não pode servir"
    );
    // Um código que nunca existiu também não serve.
    assert!(
        !identity::consume_recovery_code(&pool, &hasher(), &actor, person.id, "XXXXX-XXXXX-XXXXX", &ids)
            .await
            .unwrap()
    );
}

/// Revogar a sessão de um membro exige que a sessão seja dele — senão é como se
/// não existisse (anti-IDOR).
#[tokio::test]
async fn revogar_sessao_valida_a_posse() {
    let Some(pool) = pool().await else {
        return;
    };
    let org = organizacao(&pool).await;
    let ana = pessoa(&pool, org).await;
    let bruno = pessoa(&pool, org).await;
    let actor = identity::principal_for_person(&pool, &ana)
        .await
        .expect("principal");
    let ids = CorrelationIds::generate();

    let sessao_da_ana = sessao_activa(&pool, ana.id).await;
    let sessao_do_bruno = sessao_activa(&pool, bruno.id).await;

    // A sessão do Bruno pedida como sendo da Ana: NotFound, não revoga.
    let erro = identity::revoke_member_session(&pool, &actor, &ana, sessao_do_bruno, &ids)
        .await
        .expect_err("revogou a sessão de outra pessoa");
    assert!(matches!(erro, ocinye_core::CoreError::NotFound(_)));

    // A do Bruno continua viva.
    let estado_bruno: String = sqlx::query_scalar("SELECT state FROM sessions WHERE id = $1")
        .bind(sessao_do_bruno)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(estado_bruno, "active", "a sessão do Bruno foi tocada");

    // A da Ana, essa, revoga-se.
    identity::revoke_member_session(&pool, &actor, &ana, sessao_da_ana, &ids)
        .await
        .expect("revogar a própria");
    let estado_ana: String = sqlx::query_scalar("SELECT state FROM sessions WHERE id = $1")
        .bind(sessao_da_ana)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(estado_ana, "revoked");
}

fn hasher() -> ocinye_core::password::Hasher {
    ocinye_core::password::Hasher::new(ocinye_core::password::HashingParams {
        memory_kib: 19 * 1024,
        iterations: 2,
        parallelism: 1,
    })
}
