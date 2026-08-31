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
use ocinye_core::error::CoreError;
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

/// Uma pessoa criada pelo bootstrap ganha acesso **pelo produto**, sem duplicar.
///
/// > **O servidor arranca o primeiro administrador. O administrador arranca a
/// > instituição pelo Ocinye OS.**
///
/// É o caso do `Fidel Monteiro`: existe porque a identidade privilegiada precisa
/// de dono, não tem credencial porque o servidor não provisiona a instituição, e
/// tem de poder receber login sem que apareça um segundo registo com o mesmo
/// nome. Dois `Fidel Monteiro` repartiriam autoria, pertenças e histórico por
/// dois sítios que ninguém volta a juntar.
#[tokio::test]
async fn a_pessoa_do_bootstrap_ganha_acesso_sem_duplicar() {
    let Some(pool) = pool().await else { return };
    let org = organizacao(&pool).await;
    let (humano, admin) = enderecos();

    let (privilegiada, _) = identity::bootstrap_privileged_identity(
        &pool, &autenticador(), org,
        HumanOwner { full_name: "Fidel Monteiro".to_owned(), email: humano.clone() },
        "Fidel Admin", &admin, &CorrelationIds::generate(),
    ).await.expect("bootstrap");

    let dono = privilegiada.belongs_to_person_id.expect("dono");
    let antes: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM people WHERE organisation_id = $1 AND full_name = 'Fidel Monteiro'",
    ).bind(org).fetch_one(&pool).await.expect("contagem");
    assert_eq!(antes, 1, "havia mais do que um Fidel Monteiro antes de provisionar");

    // Quem administra é a identidade privilegiada.
    let pessoa_admin = identity::person_by_id(&pool, privilegiada.id).await.expect("l").expect("e");
    let actor = identity::principal_for_person(&pool, &pessoa_admin).await.expect("principal");

    let (provisionada, credencial) = identity::provision_existing_person(
        &pool, &autenticador(), &actor, dono, &CorrelationIds::generate(),
    ).await.expect("provisionar");

    assert_eq!(provisionada.id, dono, "provisionou outra pessoa");
    assert_eq!(credencial.email, humano);

    let depois: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM people WHERE organisation_id = $1 AND full_name = 'Fidel Monteiro'",
    ).bind(org).fetch_one(&pool).await.expect("contagem");
    assert_eq!(depois, 1, "provisionar criou um segundo Fidel Monteiro");

    // E continua sem autoridade administrativa: dar login não é dar poder.
    let agora = identity::person_by_id(&pool, dono).await.expect("l").expect("e");
    let principal = identity::principal_for_person(&pool, &agora).await.expect("principal");
    assert!(
        !principal.roles.contains(&TechnicalRole::PlatformAdmin),
        "provisionar deu autoridade administrativa a quem só precisava de entrar"
    );
    assert!(!principal.identity_kind.is_privileged(), "a pessoa tornou-se privilegiada");
}

/// Provisionar duas vezes é recusado, e pela razão certa.
///
/// A segunda tentativa não é uma criação nem um engano inofensivo: seria repor
/// a palavra-passe pelas costas de quem já a usa. Isso é `reset_password`, que
/// existe e fica registado como reposição.
#[tokio::test]
async fn provisionar_quem_ja_tem_acesso_e_recusado() {
    let Some(pool) = pool().await else { return };
    let org = organizacao(&pool).await;
    let (humano, admin) = enderecos();

    let (privilegiada, _) = identity::bootstrap_privileged_identity(
        &pool, &autenticador(), org,
        HumanOwner { full_name: "Fidel Monteiro".to_owned(), email: humano },
        "Fidel Admin", &admin, &CorrelationIds::generate(),
    ).await.expect("bootstrap");
    let dono = privilegiada.belongs_to_person_id.expect("dono");
    let pessoa_admin = identity::person_by_id(&pool, privilegiada.id).await.expect("l").expect("e");
    let actor = identity::principal_for_person(&pool, &pessoa_admin).await.expect("principal");

    identity::provision_existing_person(&pool, &autenticador(), &actor, dono, &CorrelationIds::generate())
        .await
        .expect("a primeira vez funciona");

    let segunda = identity::provision_existing_person(
        &pool, &autenticador(), &actor, dono, &CorrelationIds::generate(),
    ).await;

    // Exigir a recusa **certa**, e não um erro qualquer. A base tem um índice
    // único sobre credenciais vivas, por isso uma segunda inserção rebenta na
    // mesma — e um teste que aceitasse `is_err()` ficava verde com a guarda
    // desligada, a apoiar-se numa rede que não sabe dizer porquê a quem
    // administra.
    let erro = segunda.expect_err("provisionar duas vezes emitiu uma segunda credencial");
    assert!(
        matches!(erro, CoreError::Conflict(ref m) if m.contains("reposição")),
        "a recusa não veio da guarda de acesso existente, mas de: {erro}"
    );
}

/// Uma identidade privilegiada não se provisiona por aqui.
#[tokio::test]
async fn uma_identidade_privilegiada_nao_se_provisiona() {
    let Some(pool) = pool().await else { return };
    let org = organizacao(&pool).await;
    let (humano, admin) = enderecos();

    let (privilegiada, _) = identity::bootstrap_privileged_identity(
        &pool, &autenticador(), org,
        HumanOwner { full_name: "Fidel Monteiro".to_owned(), email: humano },
        "Fidel Admin", &admin, &CorrelationIds::generate(),
    ).await.expect("bootstrap");
    let pessoa_admin = identity::person_by_id(&pool, privilegiada.id).await.expect("l").expect("e");
    let actor = identity::principal_for_person(&pool, &pessoa_admin).await.expect("principal");

    // Tirar-lhe a credencial primeiro. Sem isto, a guarda do acesso existente
    // responde antes e a de `identity_kind` nunca chega a ser exercida: o teste
    // ficaria verde a provar a guarda errada.
    sqlx::query("UPDATE credentials SET state = 'revoked', revoked_at = now() WHERE person_id = $1")
        .bind(privilegiada.id)
        .execute(&pool)
        .await
        .expect("revogar");

    let erro = identity::provision_existing_person(
        &pool, &autenticador(), &actor, privilegiada.id, &CorrelationIds::generate(),
    ).await
    .expect_err("uma identidade privilegiada foi provisionada pela administração");
    assert!(
        matches!(erro, CoreError::Conflict(ref m) if m.contains("bootstrap institucional")),
        "a recusa não veio da guarda de identidade privilegiada, mas de: {erro}"
    );
}

/// A auditoria diz quem está por trás da identidade privilegiada.
///
/// Uma linha que dissesse apenas «Fidel Admin» perderia quem responde. E uma que
/// dissesse apenas «Super Admin» seria pior: é um rótulo, não um actor.
#[tokio::test]
async fn a_auditoria_resolve_a_pessoa_por_tras_da_identidade() {
    let Some(pool) = pool().await else { return };
    let org = organizacao(&pool).await;
    let (humano, admin) = enderecos();

    let (privilegiada, _) = identity::bootstrap_privileged_identity(
        &pool, &autenticador(), org,
        HumanOwner { full_name: "Fidel Monteiro".to_owned(), email: humano.clone() },
        "Fidel Admin", &admin, &CorrelationIds::generate(),
    ).await.expect("bootstrap");
    let dono = privilegiada.belongs_to_person_id.expect("dono");
    let pessoa_admin = identity::person_by_id(&pool, privilegiada.id).await.expect("l").expect("e");
    let actor = identity::principal_for_person(&pool, &pessoa_admin).await.expect("principal");

    identity::provision_existing_person(&pool, &autenticador(), &actor, dono, &CorrelationIds::generate())
        .await
        .expect("provisionar");

    // O actor registado é a identidade privilegiada — que é o que executou.
    let (actor_id, accao): (Uuid, String) = sqlx::query_as(
        "SELECT actor_person_id, action FROM audit_events
          WHERE organisation_id = $1 AND action = 'account_provisioned'
          ORDER BY occurred_at DESC LIMIT 1",
    ).bind(org).fetch_one(&pool).await.expect("registo");
    assert_eq!(actor_id, privilegiada.id, "o registo não diz qual identidade executou");
    assert_eq!(accao, "account_provisioned", "a acção não distingue provisionar de criar");

    // E a pessoa por trás resolve-se pela ligação, sem duplicar a informação.
    let (nome_do_actor, pessoa_por_tras): (String, Option<String>) = sqlx::query_as(
        "SELECT a.full_name, h.full_name
           FROM people a LEFT JOIN people h ON h.id = a.belongs_to_person_id
          WHERE a.id = $1",
    ).bind(actor_id).fetch_one(&pool).await.expect("resolução");
    assert_eq!(nome_do_actor, "Fidel Admin");
    assert_eq!(
        pessoa_por_tras.as_deref(),
        Some("Fidel Monteiro"),
        "a auditoria não consegue dizer quem está por trás da identidade privilegiada"
    );
}

/// Administrar uma instituição não é administrar as outras.
///
/// A resposta é «não encontrada», e não «não tem autoridade sobre ela»: a
/// segunda confirmaria a quem tenta que aquela pessoa existe, e o identificador
/// de uma pessoa noutra organização passaria a ser um oráculo de existência.
#[tokio::test]
async fn nao_se_provisiona_alguem_de_outra_organizacao() {
    let Some(pool) = pool().await else { return };
    let casa = organizacao(&pool).await;
    let alheia = organizacao(&pool).await;
    let (humano, admin) = enderecos();
    let (outro_humano, outro_admin) = enderecos();

    let (privilegiada, _) = identity::bootstrap_privileged_identity(
        &pool, &autenticador(), casa,
        HumanOwner { full_name: "Fidel Monteiro".to_owned(), email: humano },
        "Fidel Admin", &admin, &CorrelationIds::generate(),
    ).await.expect("bootstrap");
    let pessoa_admin = identity::person_by_id(&pool, privilegiada.id).await.expect("l").expect("e");
    let actor = identity::principal_for_person(&pool, &pessoa_admin).await.expect("principal");

    // Uma pessoa da outra instituição, por provisionar — o alvo mais apetecível.
    let (estrangeira, _) = identity::bootstrap_privileged_identity(
        &pool, &autenticador(), alheia,
        HumanOwner { full_name: "Alguém de Outra Casa".to_owned(), email: outro_humano },
        "Outra Admin", &outro_admin, &CorrelationIds::generate(),
    ).await.expect("bootstrap alheio");
    let alvo = estrangeira.belongs_to_person_id.expect("dono");

    let erro = identity::provision_existing_person(
        &pool, &autenticador(), &actor, alvo, &CorrelationIds::generate(),
    ).await
    .expect_err("provisionou uma pessoa de outra organização");
    assert!(
        matches!(erro, CoreError::NotFound(_)),
        "a recusa revelou que a pessoa existe noutra instituição: {erro}"
    );

    // E a pessoa alheia continua sem credencial nenhuma.
    let credenciais: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM credentials WHERE person_id = $1",
    ).bind(alvo).fetch_one(&pool).await.expect("contagem");
    assert_eq!(credenciais, 0, "ficou-lhe uma credencial emitida de fora da instituição");
}
