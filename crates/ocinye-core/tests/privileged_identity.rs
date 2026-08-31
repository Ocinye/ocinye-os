//! Identidade privilegiada ligada: administrar não é a identidade normal de
//! alguém.
//!
//! # A propriedade
//!
//! > **Uma identidade privilegiada ligada estabelece responsabilidade, e não
//! > herança de autoridade.**
//!
//! E a que governa a criação:
//!
//! > **Uma identidade privilegiada com credencial utilizável e sem dono válido
//! > é impossível — não improvável.**

use ocinye_contracts::TechnicalRole;
use ocinye_core::modules::identity::{self, HumanOwner};
use ocinye_core::password::{Hasher, HashingParams};
use ocinye_observability::CorrelationIds;
use sqlx::PgPool;
use uuid::Uuid;

async fn pool() -> Option<PgPool> {
    let Ok(url) = std::env::var("OCINYE_TEST_DATABASE_URL") else {
        assert!(
            std::env::var("CI").is_err(),
            "OCINYE_TEST_DATABASE_URL em falta em CI: a identidade privilegiada \
             ficaria por verificar e a suite reportaria verde"
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

fn autenticador() -> identity::Authenticator {
    identity::Authenticator::new(
        Hasher::new(HashingParams {
            memory_kib: 19 * 1024,
            iterations: 2,
            parallelism: 1,
        }),
        identity::Throttle::default(),
        24,
    )
}

async fn organizacao(pool: &PgPool) -> Uuid {
    let slug = format!("p{}", Uuid::new_v4().simple());
    sqlx::query_scalar("INSERT INTO organisations (slug, name) VALUES ($1,$1) RETURNING id")
        .bind(&slug)
        .fetch_one(pool)
        .await
        .expect("organização")
}

fn enderecos() -> (String, String) {
    let s = Uuid::new_v4().simple().to_string();
    (format!("h{s}@ocinye.com"), format!("a{s}@ocinye.com"))
}

/// As duas nascem juntas, e só a privilegiada recebe autoridade.
#[tokio::test]
async fn a_pessoa_e_a_identidade_privilegiada_nascem_juntas() {
    let Some(pool) = pool().await else { return };
    let org = organizacao(&pool).await;
    let (humano, admin) = enderecos();

    let (privilegiada, credencial) = identity::bootstrap_privileged_identity(
        &pool,
        &autenticador(),
        org,
        HumanOwner {
            full_name: "Fidel Monteiro".to_owned(),
            email: humano.clone(),
        },
        "Fidel Admin",
        &admin,
        &CorrelationIds::generate(),
    )
    .await
    .expect("bootstrap");

    assert_eq!(privilegiada.full_name, "Fidel Admin");
    assert_eq!(privilegiada.identity_kind, "privileged");
    assert_eq!(credencial.email, admin);

    // A pessoa existe, e é humana.
    let dono = privilegiada
        .belongs_to_person_id
        .expect("a identidade privilegiada tem de pertencer a alguém");
    let (nome, tipo, email_dono): (String, String, String) =
        sqlx::query_as("SELECT full_name, identity_kind, email FROM people WHERE id = $1")
            .bind(dono)
            .fetch_one(&pool)
            .await
            .expect("a pessoa");
    assert_eq!(
        nome, "Fidel Monteiro",
        "o nome institucional não é o da conta administrativa"
    );
    assert_eq!(tipo, "human");
    assert_eq!(email_dono, humano);

    // ── Nada atravessa a ligação ────────────────────────────────────────
    let papeis_do_humano: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM person_roles WHERE person_id = $1 AND revoked_at IS NULL",
    )
    .bind(dono)
    .fetch_one(&pool)
    .await
    .expect("papéis");
    assert_eq!(
        papeis_do_humano, 0,
        "a pessoa herdou autoridade da identidade que lhe pertence"
    );

    let credenciais_do_humano: i64 =
        sqlx::query_scalar("SELECT count(*) FROM credentials WHERE person_id = $1")
            .bind(dono)
            .fetch_one(&pool)
            .await
            .expect("credenciais");
    assert_eq!(
        credenciais_do_humano, 0,
        "a pessoa ficou com login; o servidor provisionou a instituição em vez do administrador"
    );

    // E a privilegiada tem mesmo PlatformAdmin.
    let pessoa = identity::person_by_id(&pool, privilegiada.id)
        .await
        .expect("leitura")
        .expect("existe");
    let principal = identity::principal_for_person(&pool, &pessoa)
        .await
        .expect("principal");
    assert!(
        principal.roles.contains(&TechnicalRole::PlatformAdmin),
        "a identidade privilegiada não recebeu PlatformAdmin"
    );
}

/// **A prova central:** credencial utilizável sem dono válido é impossível.
///
/// Não «improvável». Se a criação da pessoa falhar, a transacção leva consigo a
/// identidade privilegiada, a credencial e o papel — e o que fica é nada.
#[tokio::test]
async fn uma_credencial_utilizavel_sem_dono_valido_e_impossivel() {
    let Some(pool) = pool().await else { return };
    let org = organizacao(&pool).await;
    let (humano, admin) = enderecos();

    // A pessoa não pode ser criada: o endereço já está tomado. É o modo de
    // falha mais próximo de uma criação parcial que se consegue provocar sem
    // mutilar o código.
    sqlx::query(
        "INSERT INTO people (organisation_id, email, full_name, status) VALUES ($1,$2,$3,'active')",
    )
    .bind(org)
    .bind(&humano)
    .bind("Alguém que já lá estava")
    .execute(&pool)
    .await
    .expect("colisão preparada");

    let erro = identity::bootstrap_privileged_identity(
        &pool,
        &autenticador(),
        org,
        HumanOwner {
            full_name: "Fidel Monteiro".to_owned(),
            email: humano.clone(),
        },
        "Fidel Admin",
        &admin,
        &CorrelationIds::generate(),
    )
    .await;
    assert!(
        erro.is_err(),
        "o bootstrap aceitou uma pessoa que não pôde ser criada"
    );

    // E nada ficou: nem identidade, nem credencial, nem papel.
    let privilegiadas: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM people WHERE organisation_id = $1 AND identity_kind = 'privileged'",
    )
    .bind(org)
    .fetch_one(&pool)
    .await
    .expect("contagem");
    assert_eq!(
        privilegiadas, 0,
        "ficou uma identidade privilegiada de um bootstrap que falhou"
    );

    let orfas: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM credentials c JOIN people p ON p.id = c.person_id
          WHERE p.organisation_id = $1 AND p.email = $2",
    )
    .bind(org)
    .bind(&admin)
    .fetch_one(&pool)
    .await
    .expect("contagem");
    assert_eq!(orfas, 0, "ficou uma credencial utilizável sem dono");
}

/// A pessoa e a identidade privilegiada não partilham endereço.
///
/// Partilhá-lo fá-las a mesma credencial, e a separação deixaria de existir
/// enquanto continuava a parecer que existia.
#[tokio::test]
async fn a_pessoa_e_a_identidade_nao_partilham_endereco() {
    let Some(pool) = pool().await else { return };
    let org = organizacao(&pool).await;
    let (_, admin) = enderecos();

    let erro = identity::bootstrap_privileged_identity(
        &pool,
        &autenticador(),
        org,
        HumanOwner {
            full_name: "Fidel Monteiro".to_owned(),
            email: admin.clone(),
        },
        "Fidel Admin",
        &admin,
        &CorrelationIds::generate(),
    )
    .await;
    assert!(
        erro.is_err(),
        "a pessoa e a identidade privilegiada partilharam o endereço"
    );
}

/// Uma pessoa projecta-se como sessão normal.
#[tokio::test]
async fn uma_pessoa_projecta_se_como_normal() {
    let Some(pool) = pool().await else { return };
    let org = organizacao(&pool).await;
    let email = format!("h{}@ocinye.com", Uuid::new_v4().simple());
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO people (organisation_id,email,full_name,status) VALUES ($1,$2,'Alguém','active') RETURNING id",
    )
    .bind(org).bind(&email).fetch_one(&pool).await.expect("pessoa");

    let pessoa = identity::person_by_id(&pool, id)
        .await
        .expect("leitura")
        .expect("existe");
    let principal = identity::principal_for_person(&pool, &pessoa)
        .await
        .expect("principal");
    assert!(!principal.identity_kind.is_privileged());
}

/// Uma identidade privilegiada projecta-se como privilegiada.
#[tokio::test]
async fn uma_identidade_privilegiada_projecta_se_como_privilegiada() {
    let Some(pool) = pool().await else { return };
    let org = organizacao(&pool).await;
    let (humano, admin) = enderecos();

    let (privilegiada, _) = identity::bootstrap_privileged_identity(
        &pool,
        &autenticador(),
        org,
        HumanOwner {
            full_name: "Fidel Monteiro".to_owned(),
            email: humano,
        },
        "Fidel Admin",
        &admin,
        &CorrelationIds::generate(),
    )
    .await
    .expect("bootstrap");

    let pessoa = identity::person_by_id(&pool, privilegiada.id)
        .await
        .expect("leitura")
        .expect("existe");
    let principal = identity::principal_for_person(&pool, &pessoa)
        .await
        .expect("principal");
    assert!(principal.identity_kind.is_privileged());
    assert!(principal.roles.contains(&TechnicalRole::PlatformAdmin));
}

/// **As duas verdades são independentes, e esta é a direcção esquecida.**
///
/// Revogado o `PlatformAdmin`, a identidade continua a ser o que é: a sessão não
/// deixa de ser privilegiada porque a autoridade acabou. Colapsar as duas faria a
/// Experience tratar como sessão normal uma sessão que não o é — e é justamente
/// a sessão de quem administrava há um minuto.
#[tokio::test]
async fn revogar_a_autoridade_nao_torna_a_identidade_normal() {
    let Some(pool) = pool().await else { return };
    let org = organizacao(&pool).await;
    let (humano, admin) = enderecos();

    let (privilegiada, _) = identity::bootstrap_privileged_identity(
        &pool,
        &autenticador(),
        org,
        HumanOwner {
            full_name: "Fidel Monteiro".to_owned(),
            email: humano,
        },
        "Fidel Admin",
        &admin,
        &CorrelationIds::generate(),
    )
    .await
    .expect("bootstrap");

    sqlx::query("UPDATE person_roles SET revoked_at = now() WHERE person_id = $1")
        .bind(privilegiada.id)
        .execute(&pool)
        .await
        .expect("revogar");

    let pessoa = identity::person_by_id(&pool, privilegiada.id)
        .await
        .expect("leitura")
        .expect("existe");
    let principal = identity::principal_for_person(&pool, &pessoa)
        .await
        .expect("principal");

    assert!(
        !principal.roles.contains(&TechnicalRole::PlatformAdmin),
        "a autoridade não foi revogada; o resto do teste não mede nada"
    );
    assert!(
        principal.identity_kind.is_privileged(),
        "a identidade deixou de ser privilegiada por lhe terem tirado a autoridade: \
         as duas verdades colapsaram numa só"
    );
}

/// **E a direcção inversa, que protege a separação dos conceitos.**
///
/// Dar `PlatformAdmin` a uma pessoa não a transforma numa identidade
/// privilegiada. Continua a ser uma pessoa — com autoridade a mais, que é outro
/// problema, mas não este.
#[tokio::test]
async fn dar_autoridade_a_uma_pessoa_nao_a_torna_privilegiada() {
    let Some(pool) = pool().await else { return };
    let org = organizacao(&pool).await;
    let email = format!("h{}@ocinye.com", Uuid::new_v4().simple());
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO people (organisation_id,email,full_name,status) VALUES ($1,$2,'Alguém','active') RETURNING id",
    ).bind(org).bind(&email).fetch_one(&pool).await.expect("pessoa");
    sqlx::query("INSERT INTO person_roles (person_id, role) VALUES ($1,'platform_admin')")
        .bind(id)
        .execute(&pool)
        .await
        .expect("papel");

    let pessoa = identity::person_by_id(&pool, id)
        .await
        .expect("leitura")
        .expect("existe");
    let principal = identity::principal_for_person(&pool, &pessoa)
        .await
        .expect("principal");

    assert!(
        principal.roles.contains(&TechnicalRole::PlatformAdmin),
        "o papel não foi dado; o resto do teste não mede nada"
    );
    assert!(
        !principal.identity_kind.is_privileged(),
        "uma pessoa com PlatformAdmin foi projectada como identidade privilegiada: \
         a projecção está a inferir o tipo a partir da autoridade"
    );
}
