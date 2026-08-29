//! End-to-end tests of the identity and credential lifecycle.
//!
//! These run against a real PostgreSQL. They skip when
//! `OCINYE_TEST_DATABASE_URL` is unset and **fail** when it is set but the
//! database is unreachable — a suite that silently reports success without
//! having run is worse than no suite (see `docs/testing/`).
//!
//! What is proved here is the founding invariant of ADR-0103:
//!
//! > Nobody enters the Ocinye Workspace with the credential an administrator
//! > created for them.

use chrono::{Duration, Utc};
use ocinye_contracts::{AccountStatus, CredentialKind, SessionState, TechnicalRole};
use ocinye_core::modules::identity::{self, AttemptContext, Authenticator, NewMember, Throttle};
use ocinye_core::password::{Hasher, HashingParams, Secret};
use ocinye_core::CoreError;
use ocinye_observability::CorrelationIds;
use sqlx::PgPool;
use uuid::Uuid;

/// A password that clears the policy, used wherever the value is irrelevant.
const GOOD_PASSWORD: &str = "a chuva em Camama cai devagar";
const OTHER_PASSWORD: &str = "o vento sopra do mar ao anoitecer";

/// Connect and migrate, or skip.
async fn pool() -> Option<PgPool> {
    let url = std::env::var("OCINYE_TEST_DATABASE_URL").ok()?;
    let pool = PgPool::connect(&url)
        .await
        .expect("OCINYE_TEST_DATABASE_URL is set but the database is unreachable");
    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .expect("migrations must apply to the test database");

    // Antes da primeira escrita, e não depois: falhar depois de escrever
    // não é uma guarda, é um relatório de estragos.
    ocinye_core::fixtures::refuse_canonical_organisation(&pool).await;
    Some(pool)
}

/// Cheap hashing so the suite stays fast. Never used outside tests.
fn authenticator() -> Authenticator {
    Authenticator::new(
        Hasher::new(HashingParams {
            memory_kib: 8 * 1024,
            iterations: 2,
            parallelism: 1,
        }),
        Throttle {
            per_ip: 1_000,
            per_email: 1_000,
            window_minutes: 15,
        },
        24,
    )
}

/// A throttle tight enough to trip inside a test.
fn strict_authenticator() -> Authenticator {
    Authenticator::new(
        Hasher::new(HashingParams {
            memory_kib: 8 * 1024,
            iterations: 2,
            parallelism: 1,
        }),
        Throttle {
            per_ip: 1_000,
            per_email: 3,
            window_minutes: 15,
        },
        24,
    )
}

/// A fresh organisation, so tests never collide.
async fn organisation(pool: &PgPool) -> Uuid {
    let slug = format!("t{}", Uuid::new_v4().simple());
    sqlx::query_scalar("INSERT INTO organisations (slug, name) VALUES ($1, $1) RETURNING id")
        .bind(&slug)
        .fetch_one(pool)
        .await
        .expect("organisation")
}

/// Um endereço único por corrida.
///
/// Desde o ADR-0106 o endereço **é** a credencial: não há um nome de entrada e
/// um endereço, e por isso não há dois valores a compor aqui.
fn endereco() -> String {
    format!("u{}@ocinye.com", Uuid::new_v4().simple())
}

/// The bootstrap administrator of an organisation, already past first login.
async fn admin(pool: &PgPool, organisation_id: Uuid) -> ocinye_domain::Principal {
    let auth = authenticator();
    let ids = CorrelationIds::generate();
    let name = endereco();

    let (person, credential) = identity::bootstrap_platform_admin(
        pool,
        &auth,
        organisation_id,
        "Administrador",
        &name,
        &ids,
    )
    .await
    .expect("bootstrap");

    let context = AttemptContext::default();
    let issued = auth
        .sign_in(pool, &name, &credential.secret, &context, &ids)
        .await
        .expect("sign in with the bootstrap credential");
    assert_eq!(issued.state, SessionState::PasswordChangeRequired);

    identity::set_permanent_password(
        pool,
        &auth,
        &person,
        &Secret::new(GOOD_PASSWORD),
        &context,
        &ids,
    )
    .await
    .expect("set permanent password");

    let person = identity::person_by_id(pool, person.id)
        .await
        .expect("query")
        .expect("person");
    identity::principal_for_person(pool, &person)
        .await
        .expect("principal")
}

/// Create an ordinary member and return them with their temporary credential.
async fn member(
    pool: &PgPool,
    admin: &ocinye_domain::Principal,
    role: TechnicalRole,
) -> (ocinye_core::modules::identity::Person, Secret, String) {
    let auth = authenticator();
    let ids = CorrelationIds::generate();
    let name = endereco();

    let (person, credential) = identity::create_member(
        pool,
        &auth,
        admin,
        &NewMember {
            full_name: "Investigadora".into(),
            email: name.clone(),
            position: None,
            role,
            unit_id: None,
        },
        &ids,
    )
    .await
    .expect("create member");

    (person, credential.secret, name)
}

macro_rules! skip_without_database {
    () => {
        match pool().await {
            Some(pool) => pool,
            None => {
                eprintln!("skipping: OCINYE_TEST_DATABASE_URL is not set");
                return;
            }
        }
    };
}

// ── §101 Login ──────────────────────────────────────────────────────────

#[tokio::test]
async fn a_temporary_credential_yields_only_a_restricted_session() {
    let pool = skip_without_database!();
    let org = organisation(&pool).await;
    let admin = admin(&pool, org).await;
    let (person, secret, name) = member(&pool, &admin, TechnicalRole::ResearchMember).await;

    let issued = authenticator()
        .sign_in(
            &pool,
            &name,
            &secret,
            &AttemptContext::default(),
            &CorrelationIds::generate(),
        )
        .await
        .expect("sign in");

    assert_eq!(
        issued.state,
        SessionState::PasswordChangeRequired,
        "a temporary credential must never open an ordinary session"
    );
    assert_eq!(
        person.account_status(),
        AccountStatus::Invited,
        "the account stays invited until its holder sets a password"
    );
}

#[tokio::test]
async fn a_wrong_password_is_refused_with_the_same_message_as_an_unknown_account() {
    let pool = skip_without_database!();
    let org = organisation(&pool).await;
    let admin = admin(&pool, org).await;
    let (_, _, name) = member(&pool, &admin, TechnicalRole::ResearchMember).await;

    let auth = authenticator();
    let context = AttemptContext::default();
    let ids = CorrelationIds::generate();

    let wrong = auth
        .sign_in(
            &pool,
            &name,
            &Secret::new("not the credential"),
            &context,
            &ids,
        )
        .await
        .expect_err("must refuse");
    let unknown = auth
        .sign_in(
            &pool,
            "nobody-at-all",
            &Secret::new("not the credential"),
            &context,
            &ids,
        )
        .await
        .expect_err("must refuse");

    assert_eq!(
        wrong.public_message(),
        unknown.public_message(),
        "the two failures must be indistinguishable to the caller"
    );
    assert!(!wrong.public_message().to_lowercase().contains("existe"));
}

#[tokio::test]
async fn an_expired_temporary_credential_never_authenticates() {
    let pool = skip_without_database!();
    let org = organisation(&pool).await;
    let admin = admin(&pool, org).await;
    let (person, secret, name) = member(&pool, &admin, TechnicalRole::ResearchMember).await;

    // Move the expiry into the past, leaving the row's state at `active` — the
    // state a real credential sits in until a sweep runs.
    sqlx::query("UPDATE credentials SET expires_at = $2 WHERE person_id = $1")
        .bind(person.id)
        .bind(Utc::now() - Duration::hours(1))
        .execute(&pool)
        .await
        .expect("expire the credential");

    let result = authenticator()
        .sign_in(
            &pool,
            &name,
            &secret,
            &AttemptContext::default(),
            &CorrelationIds::generate(),
        )
        .await;

    assert!(
        matches!(result, Err(CoreError::Unauthenticated(_))),
        "an expired credential authenticated"
    );
}

#[tokio::test]
async fn a_suspended_account_cannot_sign_in_and_loses_its_sessions() {
    let pool = skip_without_database!();
    let org = organisation(&pool).await;
    let admin = admin(&pool, org).await;
    let (person, secret, name) = member(&pool, &admin, TechnicalRole::ResearchMember).await;
    let auth = authenticator();
    let context = AttemptContext::default();
    let ids = CorrelationIds::generate();

    // Complete first login so the account is active with a live session.
    auth.sign_in(&pool, &name, &secret, &context, &ids)
        .await
        .expect("first sign in");
    identity::set_permanent_password(
        &pool,
        &auth,
        &person,
        &Secret::new(GOOD_PASSWORD),
        &context,
        &ids,
    )
    .await
    .expect("set password");

    let person = identity::person_by_id(&pool, person.id)
        .await
        .unwrap()
        .unwrap();
    assert!(!identity::list_sessions(&pool, person.id)
        .await
        .unwrap()
        .is_empty());

    identity::set_account_status(
        &pool,
        &admin,
        &person,
        AccountStatus::Suspended,
        "test",
        &ids,
    )
    .await
    .expect("suspend");

    assert!(
        identity::list_sessions(&pool, person.id)
            .await
            .unwrap()
            .is_empty(),
        "suspension must revoke live sessions immediately"
    );

    let result = auth
        .sign_in(&pool, &name, &Secret::new(GOOD_PASSWORD), &context, &ids)
        .await;
    assert!(
        matches!(result, Err(CoreError::Unauthenticated(_))),
        "a suspended account signed in"
    );
}

#[tokio::test]
async fn repeated_failures_against_one_account_are_throttled() {
    let pool = skip_without_database!();
    let org = organisation(&pool).await;
    let admin = admin(&pool, org).await;
    let (_, _, name) = member(&pool, &admin, TechnicalRole::ResearchMember).await;

    let auth = strict_authenticator();
    let context = AttemptContext::default();
    let ids = CorrelationIds::generate();

    for _ in 0..3 {
        let _ = auth
            .sign_in(
                &pool,
                &name,
                &Secret::new("wrong wrong wrong"),
                &context,
                &ids,
            )
            .await;
    }

    let result = auth
        .sign_in(
            &pool,
            &name,
            &Secret::new("wrong wrong wrong"),
            &context,
            &ids,
        )
        .await;
    assert!(
        matches!(result, Err(CoreError::RateLimited(_))),
        "the fourth attempt should have been throttled, got {result:?}"
    );
}

// ── §102 Password change ────────────────────────────────────────────────

#[tokio::test]
async fn setting_a_password_activates_the_account_and_rotates_the_session() {
    let pool = skip_without_database!();
    let org = organisation(&pool).await;
    let admin = admin(&pool, org).await;
    let (person, secret, name) = member(&pool, &admin, TechnicalRole::ResearchMember).await;
    let auth = authenticator();
    let context = AttemptContext::default();
    let ids = CorrelationIds::generate();

    let bootstrap = auth
        .sign_in(&pool, &name, &secret, &context, &ids)
        .await
        .expect("sign in");

    let issued = identity::set_permanent_password(
        &pool,
        &auth,
        &person,
        &Secret::new(GOOD_PASSWORD),
        &context,
        &ids,
    )
    .await
    .expect("set password");

    assert_eq!(issued.state, SessionState::Active);
    assert_ne!(
        issued.token.expose(),
        bootstrap.token.expose(),
        "the bootstrap session identifier was reused"
    );

    // The bootstrap session must be gone, not merely upgraded.
    assert!(
        identity::find_session(&pool, &bootstrap.token)
            .await
            .unwrap()
            .is_none(),
        "the bootstrap session survived the password change"
    );

    let person = identity::person_by_id(&pool, person.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(person.account_status(), AccountStatus::Active);
}

#[tokio::test]
async fn the_password_policy_is_enforced_by_the_core() {
    let pool = skip_without_database!();
    let org = organisation(&pool).await;
    let admin = admin(&pool, org).await;
    let (person, secret, _name) = member(&pool, &admin, TechnicalRole::ResearchMember).await;
    let auth = authenticator();
    let context = AttemptContext::default();
    let ids = CorrelationIds::generate();

    // Too short.
    assert!(matches!(
        identity::set_permanent_password(
            &pool,
            &auth,
            &person,
            &Secret::new("curta demais"),
            &context,
            &ids
        )
        .await,
        Err(CoreError::Validation(_))
    ));

    // Blocklisted, despite being long enough.
    assert!(matches!(
        identity::set_permanent_password(
            &pool,
            &auth,
            &person,
            &Secret::new("password12345678"),
            &context,
            &ids
        )
        .await,
        Err(CoreError::Validation(_))
    ));

    // The temporary credential cannot become the permanent password.
    assert!(matches!(
        identity::set_permanent_password(&pool, &auth, &person, &secret, &context, &ids).await,
        Err(CoreError::Validation(_))
    ));

    // And a good one is accepted.
    assert!(identity::set_permanent_password(
        &pool,
        &auth,
        &person,
        &Secret::new(GOOD_PASSWORD),
        &context,
        &ids
    )
    .await
    .is_ok());
}

#[tokio::test]
async fn the_temporary_credential_stops_working_once_it_has_been_used() {
    let pool = skip_without_database!();
    let org = organisation(&pool).await;
    let admin = admin(&pool, org).await;
    let (person, secret, name) = member(&pool, &admin, TechnicalRole::ResearchMember).await;
    let auth = authenticator();
    let context = AttemptContext::default();
    let ids = CorrelationIds::generate();

    identity::set_permanent_password(
        &pool,
        &auth,
        &person,
        &Secret::new(GOOD_PASSWORD),
        &context,
        &ids,
    )
    .await
    .expect("set password");

    let result = auth.sign_in(&pool, &name, &secret, &context, &ids).await;
    assert!(
        matches!(result, Err(CoreError::Unauthenticated(_))),
        "a consumed temporary credential still authenticates"
    );

    // The new password does work, and opens an ordinary session directly.
    let issued = auth
        .sign_in(&pool, &name, &Secret::new(GOOD_PASSWORD), &context, &ids)
        .await
        .expect("sign in with the new password");
    assert_eq!(issued.state, SessionState::Active);
}

// ── §103 Administrative reset ───────────────────────────────────────────

#[tokio::test]
async fn a_reset_revokes_sessions_and_invalidates_the_old_password() {
    let pool = skip_without_database!();
    let org = organisation(&pool).await;
    let admin = admin(&pool, org).await;
    let (person, secret, name) = member(&pool, &admin, TechnicalRole::ResearchMember).await;
    let auth = authenticator();
    let context = AttemptContext::default();
    let ids = CorrelationIds::generate();

    auth.sign_in(&pool, &name, &secret, &context, &ids)
        .await
        .expect("first sign in");
    identity::set_permanent_password(
        &pool,
        &auth,
        &person,
        &Secret::new(GOOD_PASSWORD),
        &context,
        &ids,
    )
    .await
    .expect("set password");

    let person = identity::person_by_id(&pool, person.id)
        .await
        .unwrap()
        .unwrap();

    let fresh = identity::reset_password(&pool, &auth, &admin, &person, &ids)
        .await
        .expect("reset");

    assert!(
        identity::list_sessions(&pool, person.id)
            .await
            .unwrap()
            .is_empty(),
        "a reset must revoke every session"
    );

    // The old permanent password is dead.
    let old = auth
        .sign_in(&pool, &name, &Secret::new(GOOD_PASSWORD), &context, &ids)
        .await;
    assert!(
        matches!(old, Err(CoreError::Unauthenticated(_))),
        "the password that was reset still works"
    );

    // The new temporary credential works, restricted.
    let issued = auth
        .sign_in(&pool, &name, &fresh.secret, &context, &ids)
        .await
        .expect("sign in with the reset credential");
    assert_eq!(issued.state, SessionState::PasswordChangeRequired);
}

#[tokio::test]
async fn an_administrator_cannot_lock_themselves_out() {
    let pool = skip_without_database!();
    let org = organisation(&pool).await;
    let admin = admin(&pool, org).await;
    let person = identity::person_by_id(&pool, admin.person_id)
        .await
        .unwrap()
        .unwrap();

    let result = identity::set_account_status(
        &pool,
        &admin,
        &person,
        AccountStatus::Disabled,
        "oops",
        &CorrelationIds::generate(),
    )
    .await;

    assert!(
        matches!(result, Err(CoreError::Validation(_))),
        "an administrator disabled their own account"
    );
}

// ── Storage invariants ──────────────────────────────────────────────────

#[tokio::test]
async fn no_password_is_ever_stored_in_the_clear() {
    let pool = skip_without_database!();
    let org = organisation(&pool).await;
    let admin = admin(&pool, org).await;
    let (person, secret, _name) = member(&pool, &admin, TechnicalRole::ResearchMember).await;

    identity::set_permanent_password(
        &pool,
        &authenticator(),
        &person,
        &Secret::new(OTHER_PASSWORD),
        &AttemptContext::default(),
        &CorrelationIds::generate(),
    )
    .await
    .expect("set password");

    for needle in [secret.expose(), OTHER_PASSWORD] {
        let in_credentials: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM credentials WHERE verifier LIKE '%' || $1 || '%'",
        )
        .bind(needle)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(in_credentials, 0, "a password appears in credentials");

        let in_audit: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM audit_events WHERE metadata::text LIKE '%' || $1 || '%'",
        )
        .bind(needle)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(in_audit, 0, "a password appears in the audit trail");

        let in_attempts: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM authentication_attempts
              WHERE coalesce(email, '') LIKE '%' || $1 || '%'",
        )
        .bind(needle)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(in_attempts, 0, "a password appears in the attempt log");
    }
}

#[tokio::test]
async fn every_stored_verifier_is_argon2id() {
    let pool = skip_without_database!();
    let org = organisation(&pool).await;
    let admin = admin(&pool, org).await;
    let (person, _, _) = member(&pool, &admin, TechnicalRole::ResearchMember).await;

    identity::set_permanent_password(
        &pool,
        &authenticator(),
        &person,
        &Secret::new(GOOD_PASSWORD),
        &AttemptContext::default(),
        &CorrelationIds::generate(),
    )
    .await
    .expect("set password");

    let bad: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM credentials WHERE verifier NOT LIKE '$argon2id$%'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(bad, 0);
}

#[tokio::test]
async fn a_second_bootstrap_is_refused() {
    let pool = skip_without_database!();
    let org = organisation(&pool).await;
    let _admin = admin(&pool, org).await;

    let name = endereco();
    let result = identity::bootstrap_platform_admin(
        &pool,
        &authenticator(),
        org,
        "Segundo",
        &name,
        &CorrelationIds::generate(),
    )
    .await;

    assert!(
        matches!(result, Err(CoreError::Conflict(_))),
        "bootstrap ran twice"
    );
}

#[tokio::test]
async fn os_enderecos_sao_unicos_ignorando_maiusculas() {
    let pool = skip_without_database!();
    let org = organisation(&pool).await;
    let admin = admin(&pool, org).await;
    let (_, _, name) = member(&pool, &admin, TechnicalRole::ResearchMember).await;

    let result = identity::create_member(
        &pool,
        &authenticator(),
        &admin,
        &NewMember {
            full_name: "Homónimo".into(),
            // O mesmo endereço em maiúsculas é o mesmo endereço.
            email: name.clone().to_uppercase(),
            position: None,
            role: TechnicalRole::ResearchMember,
            unit_id: None,
        },
        &CorrelationIds::generate(),
    )
    .await;

    assert!(
        matches!(result, Err(CoreError::Conflict(_))),
        "two accounts differing only in case were allowed"
    );
}

#[tokio::test]
async fn a_created_member_holds_exactly_one_temporary_credential_and_no_permanent_one() {
    let pool = skip_without_database!();
    let org = organisation(&pool).await;
    let admin = admin(&pool, org).await;
    let (person, _, _) = member(&pool, &admin, TechnicalRole::ResearchMember).await;

    let rows: Vec<(String, String)> =
        sqlx::query_as("SELECT kind, state FROM credentials WHERE person_id = $1")
            .bind(person.id)
            .fetch_all(&pool)
            .await
            .unwrap();

    assert_eq!(
        rows.len(),
        1,
        "expected exactly one credential, got {rows:?}"
    );
    assert_eq!(rows[0].0, CredentialKind::Temporary.as_str());
    assert_eq!(rows[0].1, "active");
}

/// Dois `bootstrap-admin` em simultâneo produzem **um** administrador.
///
/// # Porque este teste existe
///
/// A garantia de execução única era verificada duas vezes — antes da transacção
/// e dentro dela — e a segunda verificação não valia nada: um `SELECT` em
/// `READ COMMITTED` não bloqueia ninguém. Duas execuções concorrentes liam
/// ambas «não há administrador», inseriam pessoas com nomes de utilizador
/// diferentes, e ambas commitavam. Nada no esquema proíbe um segundo
/// `platform_admin`.
///
/// A correcção é um `pg_advisory_xact_lock` dentro da transacção. Este teste
/// corre o cenário várias vezes, porque uma corrida que passa uma vez por
/// acaso não prova nada.
#[tokio::test]
async fn two_concurrent_bootstraps_produce_one_administrator() {
    let Some(pool) = pool().await else { return };

    for round in 0..5 {
        let organisation_id = organisation(&pool).await;

        let attempt = |suffix: &str| {
            let pool = pool.clone();
            let email = format!("u{suffix}-{}", endereco());

            async move {
                identity::bootstrap_platform_admin(
                    &pool,
                    &authenticator(),
                    organisation_id,
                    "Primeiro Administrador",
                    &email,
                    &CorrelationIds::generate(),
                )
                .await
            }
        };

        let (first, second) = tokio::join!(attempt("a"), attempt("b"));

        let succeeded = usize::from(first.is_ok()) + usize::from(second.is_ok());
        assert_eq!(
            succeeded, 1,
            "ronda {round}: {succeeded} bootstraps foram aceites, e só um pode ser"
        );

        // E a base de dados concorda: um administrador, não dois.
        let admins: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM person_roles r
               JOIN people p ON p.id = r.person_id
              WHERE p.organisation_id = $1
                AND r.role = 'platform_admin'
                AND r.revoked_at IS NULL",
        )
        .bind(organisation_id)
        .fetch_one(&pool)
        .await
        .expect("count");

        assert_eq!(
            admins, 1,
            "ronda {round}: a instalação ficou com {admins} administradores de plataforma"
        );
    }
}

/// Um identificador de sessão escrito pelo cliente não confere autoridade.
///
/// # Porque este teste existe antes da rota
///
/// A primitiva `revoke_session` recebe um UUID e revoga. É correcta enquanto o
/// identificador vier de dentro — a rota de terminar sessão passa o da sessão
/// autenticada, que nunca atravessou a rede.
///
/// `Definições` muda isso: o membro escolhe qual sessão terminar, e o
/// identificador passa a vir do cliente. É o mesmo padrão que produziu o
/// `SB1-FU-02` nos ambientes de investigação, agora na autenticação — e a
/// fronteira foi construída **antes** de a superfície existir, não depois.
///
/// A recusa é indistinguível de «não existe»: dizer «existe mas não é sua»
/// confirmaria sessões alheias a quem tentasse adivinhar.
#[tokio::test]
async fn um_identificador_de_sessao_escrito_pelo_cliente_nao_confere_autoridade() {
    let Some(pool) = pool().await else { return };
    let organisation_id = organisation(&pool).await;
    let gestor = admin(&pool, organisation_id).await;

    let (pessoa_a, _, _) = member(&pool, &gestor, TechnicalRole::ResearchMember).await;
    let (pessoa_b, _, _) = member(&pool, &gestor, TechnicalRole::ResearchMember).await;

    let a = identity::principal_for_person(&pool, &pessoa_a)
        .await
        .expect("principal A");
    let b = identity::principal_for_person(&pool, &pessoa_b)
        .await
        .expect("principal B");

    // A sessão é infraestrutura de fixture: o que está sob teste é a fronteira
    // de posse em `revoke_own_session`, não a emissão de sessões.
    let sessao_de_b: Uuid = sqlx::query_scalar(
        "INSERT INTO sessions (person_id, token_digest, state, expires_at)
         VALUES ($1, $2, 'active', now() + interval '8 hours') RETURNING id",
    )
    .bind(pessoa_b.id)
    .bind(Uuid::new_v4().simple().to_string())
    .fetch_one(&pool)
    .await
    .expect("sessão de B");

    // A conhece o identificador de B e tenta terminá-la.
    let recusa = identity::revoke_own_session(&pool, &a, sessao_de_b, "tentativa").await;
    assert!(
        recusa.is_err(),
        "um identificador de sessão escrito pelo cliente conferiu autoridade sobre a sessão de outra pessoa"
    );

    let estado: String = sqlx::query_scalar("SELECT state FROM sessions WHERE id = $1")
        .bind(sessao_de_b)
        .fetch_one(&pool)
        .await
        .expect("ler estado");
    assert_eq!(
        estado, "active",
        "a sessão de B foi revogada por outra pessoa"
    );

    // A listagem de A não inclui sessões de B, mesmo conhecendo o identificador.
    let minhas = identity::list_own_sessions(&pool, &a)
        .await
        .expect("listar as minhas");
    assert!(
        !minhas.iter().any(|s| s.id == sessao_de_b),
        "a listagem própria incluiu uma sessão alheia"
    );

    // B termina a sua própria sessão, e isso funciona.
    identity::revoke_own_session(&pool, &b, sessao_de_b, "signed_out")
        .await
        .expect("B deve poder terminar a sua própria sessão");
}

/// A mudança de palavra-passe só está concluída quando o mundo acompanha.
///
/// # A propriedade
///
/// > Não basta a credencial ter sido persistida. A mudança está concluída
/// > quando **todas as sessões que a política invalida deixaram de funcionar** e
/// > o cliente recebeu uma sessão de substituição válida.
///
/// O cenário usa **duas** sessões antes da mudança, e executa-a por uma delas.
/// Com uma só, um bug que revogasse apenas a sessão chamadora passaria — e é
/// precisamente esse o bug fácil de escrever.
#[tokio::test]
async fn mudar_a_palavra_passe_invalida_todas_as_sessoes_e_devolve_uma_nova() {
    let pool = skip_without_database!();
    let org = organisation(&pool).await;
    let gestor = admin(&pool, org).await;
    let (pessoa, temporaria, nome) = member(&pool, &gestor, TechnicalRole::ResearchMember).await;

    let auth = authenticator();
    let ids = CorrelationIds::generate();
    let contexto = AttemptContext::default();

    // A conta começa com credencial temporária; o primeiro acesso define a
    // permanente, que é o ponto de partida deste teste.
    let antiga = Secret::from("uma frase longa e antiga para entrar".to_owned());
    auth.sign_in(&pool, &nome, &temporaria, &contexto, &ids)
        .await
        .expect("primeiro início de sessão");
    identity::set_permanent_password(&pool, &auth, &pessoa, &antiga, &contexto, &ids)
        .await
        .expect("definir a permanente");

    // Duas sessões reais, antes da mudança.
    let s1 = auth
        .sign_in(&pool, &nome, &antiga, &contexto, &ids)
        .await
        .expect("sessão 1");
    let s2 = auth
        .sign_in(&pool, &nome, &antiga, &contexto, &ids)
        .await
        .expect("sessão 2");
    assert_ne!(s1.token.expose(), s2.token.expose());

    // A mudança, com a palavra-passe actual confirmada.
    let nova = Secret::from("outra frase bem diferente da anterior".to_owned());
    let s3 = identity::change_own_password(&pool, &auth, &pessoa, &antiga, &nova, &contexto, &ids)
        .await
        .expect("mudar a palavra-passe");

    // As duas anteriores deixaram de servir.
    for (rotulo, sessao) in [("s1", &s1), ("s2", &s2)] {
        let viva = identity::find_session(&pool, &sessao.token)
            .await
            .expect("consultar")
            .is_some_and(|s| s.state == SessionState::Active);
        assert!(
            !viva,
            "{rotulo} continuou utilizável depois de a palavra-passe mudar"
        );
    }

    // A substituição é válida e serve para trabalhar.
    let substituta = identity::find_session(&pool, &s3.token)
        .await
        .expect("consultar")
        .expect("a sessão de substituição existe");
    assert_eq!(
        substituta.state,
        SessionState::Active,
        "a sessão devolvida não permite trabalho normal"
    );

    // A antiga já não autentica; a nova autentica.
    assert!(
        auth.sign_in(&pool, &nome, &antiga, &contexto, &ids)
            .await
            .is_err(),
        "a palavra-passe antiga continuou a autenticar"
    );
    auth.sign_in(&pool, &nome, &nova, &contexto, &ids)
        .await
        .expect("a nova palavra-passe deve autenticar");
}

/// A palavra-passe actual errada não muda nada, e não roda sessão nenhuma.
#[tokio::test]
async fn uma_palavra_passe_actual_errada_nao_altera_nem_roda() {
    let pool = skip_without_database!();
    let org = organisation(&pool).await;
    let gestor = admin(&pool, org).await;
    let (pessoa, temporaria, nome) = member(&pool, &gestor, TechnicalRole::ResearchMember).await;

    let auth = authenticator();
    let ids = CorrelationIds::generate();
    let contexto = AttemptContext::default();

    let antiga = Secret::from("uma frase longa e antiga para entrar".to_owned());
    auth.sign_in(&pool, &nome, &temporaria, &contexto, &ids)
        .await
        .expect("primeiro acesso");
    identity::set_permanent_password(&pool, &auth, &pessoa, &antiga, &contexto, &ids)
        .await
        .expect("permanente");

    let sessao = auth
        .sign_in(&pool, &nome, &antiga, &contexto, &ids)
        .await
        .expect("sessão");

    let errada = Secret::from("isto nao e a palavra passe actual".to_owned());
    let nova = Secret::from("outra frase bem diferente da anterior".to_owned());
    let recusa =
        identity::change_own_password(&pool, &auth, &pessoa, &errada, &nova, &contexto, &ids).await;
    assert!(
        recusa.is_err(),
        "uma palavra-passe actual errada foi aceite"
    );

    // Nada mudou: a antiga continua a servir, a nova não, e a sessão sobrevive.
    auth.sign_in(&pool, &nome, &antiga, &contexto, &ids)
        .await
        .expect("a antiga deve continuar válida");
    assert!(
        auth.sign_in(&pool, &nome, &nova, &contexto, &ids)
            .await
            .is_err(),
        "a nova palavra-passe passou a autenticar sem a mudança ter sido aceite"
    );
    let viva = identity::find_session(&pool, &sessao.token)
        .await
        .expect("consultar")
        .is_some_and(|s| s.state == SessionState::Active);
    assert!(viva, "uma tentativa recusada revogou a sessão");
}

// ── Identidade própria ──────────────────────────────────────────────────

/// Um membro lê o seu próprio registo, mesmo sem poder ler o directório.
///
/// `get_person` pergunta à política se o autor pode ler *pessoas* — uma
/// permissão de âmbito institucional. Encaminhada por lá, uma colaboradora
/// externa seria recusada ao pedir o seu próprio nome, e o ecrã da conta
/// dir-lhe-ia que não tem autorização para se ver a si mesma.
///
/// Identidade não é permissão. Estar autenticada já significa que o Core a
/// resolveu; devolver o que resolveu não revela nada que a sessão não tenha
/// estabelecido.
#[tokio::test]
async fn um_membro_le_o_seu_proprio_registo_sem_permissao_de_directorio() {
    let pool = skip_without_database!();
    let organisation_id = organisation(&pool).await;
    let admin = admin(&pool, organisation_id).await;

    // Um papel sem leitura do directório: é este o caso que quebrava.
    let (person, _, name) = member(&pool, &admin, TechnicalRole::ExternalCollaborator).await;
    let principal = identity::principal_for_person(&pool, &person)
        .await
        .expect("principal");

    // O caminho institucional recusa-lhe o directório. A recusa chega como
    // `NotFound` e não como `PermissionDenied` de propósito: dizer «não tem
    // autorização» confirmaria que a pessoa existe, e quem não pode ler o
    // directório também não deve poder sondá-lo. O que importa aqui é que
    // recusa — a forma da recusa é assunto do módulo de identidade.
    let directorio = identity::get_person(&pool, &principal, person.id).await;
    assert!(
        directorio.is_err(),
        "um colaborador externo não devia poder ler o directório: {directorio:?}"
    );

    // ...e ainda assim ele vê-se a si próprio.
    let proprio = identity::get_own_person(&pool, &principal)
        .await
        .expect("o membro tem de conseguir ler o seu próprio registo");

    assert_eq!(proprio.id, person.id);
    assert_eq!(proprio.email, name.clone());
    // `invited`, e não `active`: o membro acabou de ser criado e ainda deve ao
    // Core uma palavra-passe permanente. É o estado real, e é isso que o ecrã
    // da conta passa a mostrar em vez de um traço.
    assert_eq!(proprio.status, AccountStatus::Invited.as_str().to_owned());
}

/// A leitura da própria identidade não tem para onde apontar.
///
/// Não recebe `person_id`: fixa-se em `principal.person_id` e na organização do
/// principal. Duas pessoas distintas obtêm registos distintos, e nenhuma delas
/// tem parâmetro por onde alcançar a outra.
#[tokio::test]
async fn a_leitura_da_propria_identidade_nao_alcanca_terceiros() {
    let pool = skip_without_database!();
    let organisation_id = organisation(&pool).await;
    let admin = admin(&pool, organisation_id).await;

    let (uma, _, nome_uma) = member(&pool, &admin, TechnicalRole::ResearchMember).await;
    let (outra, _, nome_outra) = member(&pool, &admin, TechnicalRole::ResearchMember).await;

    let principal_uma = identity::principal_for_person(&pool, &uma)
        .await
        .expect("principal");
    let principal_outra = identity::principal_for_person(&pool, &outra)
        .await
        .expect("principal");

    let vista_uma = identity::get_own_person(&pool, &principal_uma)
        .await
        .expect("própria");
    let vista_outra = identity::get_own_person(&pool, &principal_outra)
        .await
        .expect("própria");

    assert_eq!(vista_uma.email, nome_uma.clone());
    assert_eq!(vista_outra.email, nome_outra.clone());
    assert_ne!(vista_uma.id, vista_outra.id);
}

// ── Avatar ──────────────────────────────────────────────────────────────

/// Um membro novo é representado pelas iniciais.
///
/// É o estado de origem, e não uma ausência a corrigir: ninguém tem de carregar
/// uma fotografia para deixar de ser um caso por tratar.
#[tokio::test]
async fn um_membro_novo_e_representado_pelas_iniciais() {
    let pool = skip_without_database!();
    let organisation_id = organisation(&pool).await;
    let admin = admin(&pool, organisation_id).await;
    let (person, _, _) = member(&pool, &admin, TechnicalRole::ResearchMember).await;
    let principal = identity::principal_for_person(&pool, &person)
        .await
        .expect("principal");

    assert_eq!(
        identity::own_avatar(&pool, &principal)
            .await
            .expect("avatar"),
        ocinye_contracts::AvatarChoice::Initials
    );
}

/// Escolher um avatar do produto guarda a escolha, e não copia ficheiro nenhum.
///
/// Um preset não é um upload: não cria `storage_objects`, não toca no bucket, e
/// não custa nada trocar. Doze presets vezes os membros de uma instituição
/// seriam milhares de cópias do mesmo ficheiro para representar uma escolha
/// entre doze.
#[tokio::test]
async fn escolher_um_preset_nao_cria_objectos_em_storage() {
    let pool = skip_without_database!();
    let organisation_id = organisation(&pool).await;
    let admin = admin(&pool, organisation_id).await;
    let (person, _, _) = member(&pool, &admin, TechnicalRole::ResearchMember).await;
    let principal = identity::principal_for_person(&pool, &person)
        .await
        .expect("principal");

    let antes: i64 = sqlx::query_scalar("SELECT count(*) FROM storage_objects")
        .fetch_one(&pool)
        .await
        .expect("contagem");

    let escolha = identity::choose_preset(&pool, &principal, None, "science-02")
        .await
        .expect("escolher preset");
    assert_eq!(
        escolha,
        ocinye_contracts::AvatarChoice::Preset {
            preset: "science-02".to_owned()
        }
    );

    // E persiste: uma nova leitura devolve o mesmo.
    assert_eq!(
        identity::own_avatar(&pool, &principal).await.expect("ler"),
        escolha
    );

    // Trocar de preset continua a não criar nada.
    identity::choose_preset(&pool, &principal, None, "energy-03")
        .await
        .expect("trocar preset");
    assert_eq!(
        identity::own_avatar(&pool, &principal).await.expect("ler"),
        ocinye_contracts::AvatarChoice::Preset {
            preset: "energy-03".to_owned()
        }
    );

    let depois: i64 = sqlx::query_scalar("SELECT count(*) FROM storage_objects")
        .fetch_one(&pool)
        .await
        .expect("contagem");
    assert_eq!(
        antes, depois,
        "escolher um preset criou objectos em storage"
    );
}

/// Um identificador fora do catálogo é recusado.
///
/// O identificador vem do cliente, e um identificador que viesse a ser usado
/// como caminho seria um caminho escolhido por quem o envia.
#[tokio::test]
async fn um_preset_fora_do_catalogo_e_recusado() {
    let pool = skip_without_database!();
    let organisation_id = organisation(&pool).await;
    let admin = admin(&pool, organisation_id).await;
    let (person, _, _) = member(&pool, &admin, TechnicalRole::ResearchMember).await;
    let principal = identity::principal_for_person(&pool, &person)
        .await
        .expect("principal");

    for tentativa in [
        "compute-99",
        "../../../etc/passwd",
        "/static/avatars/compute-01.svg",
        "https://exemplo.org/foto.png",
        "compute-01.svg",
        "",
    ] {
        let resultado = identity::choose_preset(&pool, &principal, None, tentativa).await;
        assert!(
            matches!(resultado, Err(CoreError::Validation(_))),
            "«{tentativa}» foi aceite como avatar Ocinye: {resultado:?}"
        );
    }

    // E nada mudou: uma tentativa recusada não deixa estado a meio.
    assert_eq!(
        identity::own_avatar(&pool, &principal).await.expect("ler"),
        ocinye_contracts::AvatarChoice::Initials
    );
}

/// Voltar às iniciais é uma escolha, e não o que sobra.
///
/// Quem tem um preset e prefere o nome carrega em «Usar iniciais», e é isso que
/// fica guardado.
#[tokio::test]
async fn voltar_as_iniciais_e_uma_escolha_explicita() {
    let pool = skip_without_database!();
    let organisation_id = organisation(&pool).await;
    let admin = admin(&pool, organisation_id).await;
    let (person, _, _) = member(&pool, &admin, TechnicalRole::ResearchMember).await;
    let principal = identity::principal_for_person(&pool, &person)
        .await
        .expect("principal");

    identity::choose_preset(&pool, &principal, None, "compute-03")
        .await
        .expect("preset");
    identity::use_initials(&pool, &principal, None)
        .await
        .expect("iniciais");

    assert_eq!(
        identity::own_avatar(&pool, &principal).await.expect("ler"),
        ocinye_contracts::AvatarChoice::Initials
    );
}

/// A escolha de um membro não alcança a de outro.
///
/// Nenhuma destas funções recebe `person_id`. Não é que o rejeitem: não existe
/// parâmetro por onde ele possa entrar, e uma verificação que não existe não
/// pode ser esquecida numa função nova.
#[tokio::test]
async fn a_escolha_de_um_membro_nao_alcanca_a_de_outro() {
    let pool = skip_without_database!();
    let organisation_id = organisation(&pool).await;
    let admin = admin(&pool, organisation_id).await;

    let (uma, _, _) = member(&pool, &admin, TechnicalRole::ResearchMember).await;
    let (outra, _, _) = member(&pool, &admin, TechnicalRole::ResearchMember).await;
    let principal_uma = identity::principal_for_person(&pool, &uma)
        .await
        .expect("principal");
    let principal_outra = identity::principal_for_person(&pool, &outra)
        .await
        .expect("principal");

    identity::choose_preset(&pool, &principal_uma, None, "compute-01")
        .await
        .expect("preset");

    // A outra continua nas iniciais: a escolha de uma não escorreu para a outra.
    assert_eq!(
        identity::own_avatar(&pool, &principal_outra)
            .await
            .expect("ler"),
        ocinye_contracts::AvatarChoice::Initials
    );

    identity::choose_preset(&pool, &principal_outra, None, "energy-01")
        .await
        .expect("preset");

    assert_eq!(
        identity::own_avatar(&pool, &principal_uma)
            .await
            .expect("ler"),
        ocinye_contracts::AvatarChoice::Preset {
            preset: "compute-01".to_owned()
        },
        "escolher para uma pessoa alterou a outra"
    );
}

/// Sem fotografia não há leitura, seja qual for a versão pedida.
///
/// A pergunta que a leitura faz não é «existe um avatar com esta versão», é «a
/// versão pedida é a do avatar deste principal». Conhecer o identificador não
/// concede acesso.
#[tokio::test]
async fn conhecer_uma_versao_nao_da_acesso_a_ela() {
    let pool = skip_without_database!();
    let organisation_id = organisation(&pool).await;
    let admin = admin(&pool, organisation_id).await;
    let (person, _, _) = member(&pool, &admin, TechnicalRole::ResearchMember).await;
    let principal = identity::principal_for_person(&pool, &person)
        .await
        .expect("principal");

    // Um membro sem fotografia não abre nada, nem com uma versão bem formada.
    let inventada = "0".repeat(64);
    assert!(matches!(
        identity::own_photograph_key(&pool, &principal, &inventada).await,
        Err(CoreError::NotFound(_))
    ));

    // E com um preset escolhido continua a não haver fotografia para ler.
    identity::choose_preset(&pool, &principal, None, "science-01")
        .await
        .expect("preset");
    assert!(matches!(
        identity::own_photograph_key(&pool, &principal, &inventada).await,
        Err(CoreError::NotFound(_))
    ));
}

// ── Avatar: fotografia contra storage real ─────────────────────────────

/// O `ObjectStore` de teste, quando existe um serviço S3-compatível.
///
/// # Porque é opt-in
///
/// A suite normal não pode depender de um serviço que pode não estar a correr:
/// um teste que passa por não ter sido executado é pior do que um teste que não
/// existe, porque conta como cobertura. Sem `OCINYE_TEST_STORAGE_ENDPOINT` este
/// caminho é saltado **e o relatório tem de o dizer** — não «testado», mas
/// «não exercido, porque não havia serviço».
fn test_store() -> Option<ocinye_core::storage::ObjectStore> {
    let endpoint = std::env::var("OCINYE_TEST_STORAGE_ENDPOINT").ok()?;
    ocinye_core::storage::ObjectStore::new(ocinye_core::config::StorageConfig {
        endpoint_url: endpoint,
        region: std::env::var("OCINYE_TEST_STORAGE_REGION")
            .unwrap_or_else(|_| "us-east-1".to_owned()),
        access_key: std::env::var("OCINYE_TEST_STORAGE_ACCESS_KEY").ok()?,
        secret_key: std::env::var("OCINYE_TEST_STORAGE_SECRET_KEY").ok()?,
        bucket: std::env::var("OCINYE_TEST_STORAGE_BUCKET")
            .unwrap_or_else(|_| "ocinye-test-artifacts".to_owned()),
        backend_code: "test".to_owned(),
        location_label: "test".to_owned(),
        residency: ocinye_contracts::storage::Residency::Undeclared,
        max_upload_bytes: 32 * 1024 * 1024,
    })
}

/// Regista o backend por omissão que uma instalação com armazenamento tem.
///
/// Sem ele, `INSERT … SELECT FROM storage_backends WHERE is_default` não
/// encontra linha nenhuma. Não é um detalhe do teste: é a diferença entre uma
/// instalação com armazenamento configurado e uma sem — e foi assim que o teste
/// contra MinIO real destapou que o Core não distinguia as duas.
async fn registar_backend(pool: &PgPool, bucket: &str) {
    sqlx::query(
        "INSERT INTO storage_backends
             (code, kind, display_name, location_label, bucket, is_default, is_active)
         VALUES ('ocinye-test-default', 's3_compatible', 'Test', 'test', $1, TRUE, TRUE)
         ON CONFLICT (code) DO UPDATE
             SET bucket = EXCLUDED.bucket, is_default = TRUE, is_active = TRUE,
                 updated_at = now()",
    )
    .bind(bucket)
    .execute(pool)
    .await
    .expect("registar backend de teste");
}

/// Exclusão entre testes que mexem no registo de armazenamento.
///
/// `storage_backends` é global: `is_default` não tem organização, e um teste que
/// o desliga desliga-o para todos os outros que estiverem a correr ao mesmo
/// tempo — inclusive noutro binário de teste, que o `cargo` corre em paralelo.
///
/// Um `Mutex` de Rust não chega, porque não atravessa processos. Um advisory
/// lock do PostgreSQL chega, e liberta-se sozinho quando a ligação fecha.
async fn com_registo_exclusivo<F, Fut, T>(pool: &PgPool, corpo: F) -> T
where
    F: FnOnce(PgPool) -> Fut,
    Fut: std::future::Future<Output = T>,
{
    const CHAVE: i64 = 0x0000_C109_E570_9A6E;

    let mut ligacao = pool.acquire().await.expect("ligação");
    sqlx::query("SELECT pg_advisory_lock($1)")
        .bind(CHAVE)
        .execute(&mut *ligacao)
        .await
        .expect("advisory lock");

    let resultado = corpo(pool.clone()).await;

    sqlx::query("SELECT pg_advisory_unlock($1)")
        .bind(CHAVE)
        .execute(&mut *ligacao)
        .await
        .expect("advisory unlock");
    resultado
}

macro_rules! skip_without_storage {
    () => {
        match test_store() {
            Some(store) => store,
            None => {
                // Sem armazenamento em máquina de alguém, salta-se. Sem
                // armazenamento na CI, falha: `cargo test` engole este
                // `eprintln!` num teste que passa, pelo que a guarda que
                // procura «skipping» na saída nunca o viu.
                assert!(
                    std::env::var("CI").is_err(),
                    "não há armazenamento, e isto é a CI: estas provas exigem \
                     um object store. Defina OCINYE_TEST_STORAGE_ENDPOINT."
                );
                eprintln!("skipping: OCINYE_TEST_STORAGE_ENDPOINT is not set");
                return;
            }
        }
    };
}

/// Uma fotografia sólida, do tamanho pedido.
fn fotografia(largura: u32, altura: u32) -> Vec<u8> {
    let buffer = image::ImageBuffer::from_fn(largura, altura, |x, y| {
        image::Rgb([(x % 256) as u8, (y % 256) as u8, 96u8])
    });
    let mut saida = Vec::new();
    image::DynamicImage::ImageRgb8(buffer)
        .write_to(
            &mut std::io::Cursor::new(&mut saida),
            image::ImageFormat::Jpeg,
        )
        .expect("codificar");
    saida
}

/// A cadeia completa: carregar, ler, substituir, remover.
///
/// Não basta o `put_object` devolver sucesso. O objecto é lido outra vez do
/// storage e verificado: é uma imagem descodificável, no formato canónico, nas
/// dimensões canónicas, e não é o ficheiro que entrou.
#[tokio::test]
async fn uma_fotografia_atravessa_a_cadeia_toda() {
    let pool = skip_without_database!();
    let store = skip_without_storage!();
    com_registo_exclusivo(&pool, |pool| async move {
        registar_backend(&pool, store.bucket()).await;
        let organisation_id = organisation(&pool).await;
        let admin = admin(&pool, organisation_id).await;
        let (person, _, _) = member(&pool, &admin, TechnicalRole::ResearchMember).await;
        let principal = identity::principal_for_person(&pool, &person)
            .await
            .expect("principal");

        let entrada = fotografia(900, 500);
        let escolha = identity::set_photograph(&pool, &principal, &store, "ocinye-test", &entrada)
            .await
            .expect("carregar fotografia");

        let ocinye_contracts::AvatarChoice::Custom { version } = escolha.clone() else {
            panic!("carregar uma fotografia devia dar um avatar personalizado: {escolha:?}");
        };

        // `/me` anuncia a mesma versão.
        assert_eq!(
            identity::own_avatar(&pool, &principal).await.expect("ler"),
            escolha
        );

        // A leitura autoriza e devolve os bytes normalizados.
        let key = identity::own_photograph_key(&pool, &principal, &version)
            .await
            .expect("a versão actual devia abrir");
        let guardado = store.get(&key).await.expect("ler do storage");

        let imagem = image::load_from_memory(&guardado).expect("o que está guardado é uma imagem");
        assert_eq!(imagem.width(), ocinye_core::avatar::AVATAR_SIDE);
        assert_eq!(imagem.height(), ocinye_core::avatar::AVATAR_SIDE);
        assert_eq!(
            image::guess_format(&guardado).expect("formato"),
            image::ImageFormat::WebP,
            "o objecto guardado não está no formato canónico"
        );
        assert_ne!(guardado, entrada, "guardou-se o ficheiro de origem");
        assert_eq!(
            ocinye_core::storage::sha256_hex(&guardado),
            version,
            "a versão anunciada não é o checksum do que está guardado"
        );

        // Substituir cria uma versão nova e larga a anterior.
        let nova = identity::set_photograph(
            &pool,
            &principal,
            &store,
            "ocinye-test",
            &fotografia(400, 400),
        )
        .await
        .expect("substituir");
        let ocinye_contracts::AvatarChoice::Custom {
            version: nova_versao,
        } = nova
        else {
            panic!("substituir devia dar outra fotografia");
        };
        assert_ne!(nova_versao, version, "substituir não mudou a versão");

        // A versão antiga deixa de abrir: o endereço muda com o conteúdo.
        assert!(
            identity::own_photograph_key(&pool, &principal, &version)
                .await
                .is_err(),
            "a versão anterior continua a abrir"
        );
        // E o objecto antigo saiu do storage e da base de dados.
        assert!(
            store.get(&key).await.is_err(),
            "a fotografia anterior ficou no armazenamento"
        );
        let orfaos: i64 =
            sqlx::query_scalar("SELECT count(*) FROM storage_objects WHERE object_key = $1")
                .bind(&key)
                .fetch_one(&pool)
                .await
                .expect("contagem");
        assert_eq!(orfaos, 0, "ficou uma linha órfã em storage_objects");

        // Remover volta às iniciais e não deixa nada para trás.
        let nova_key = identity::own_photograph_key(&pool, &principal, &nova_versao)
            .await
            .expect("a versão actual devia abrir");
        identity::use_initials(&pool, &principal, Some(&store))
            .await
            .expect("iniciais");
        assert_eq!(
            identity::own_avatar(&pool, &principal).await.expect("ler"),
            ocinye_contracts::AvatarChoice::Initials
        );
        assert!(
            store.get(&nova_key).await.is_err(),
            "remover a fotografia deixou o objecto no armazenamento"
        );
    })
    .await;
}

/// Um upload recusado não deixa objectos no armazenamento.
///
/// A normalização acontece antes de o storage ser tocado, e é por isso que
/// recusar não tem nada para limpar. Este teste conta os objectos antes e
/// depois para provar que continua assim.
#[tokio::test]
async fn um_upload_recusado_nao_deixa_objectos() {
    let pool = skip_without_database!();
    let store = skip_without_storage!();
    com_registo_exclusivo(&pool, |pool| async move {
        registar_backend(&pool, store.bucket()).await;
        let organisation_id = organisation(&pool).await;
        let admin = admin(&pool, organisation_id).await;
        let (person, _, _) = member(&pool, &admin, TechnicalRole::ResearchMember).await;
        let principal = identity::principal_for_person(&pool, &person)
            .await
            .expect("principal");

        let antes: i64 = sqlx::query_scalar("SELECT count(*) FROM storage_objects")
            .fetch_one(&pool)
            .await
            .expect("contagem");

        let recusados: Vec<(&str, Vec<u8>)> = vec![
            (
                "svg",
                br#"<svg xmlns="http://www.w3.org/2000/svg"><script/></svg>"#.to_vec(),
            ),
            ("executável", b"\x7fELF\x02\x01\x01\x00".to_vec()),
            ("texto", b"nao sou uma imagem".to_vec()),
            ("vazio", Vec::new()),
            (
                "grande de mais",
                vec![0u8; ocinye_core::avatar::MAX_AVATAR_BYTES + 1],
            ),
            ("truncado", fotografia(300, 300)[..40].to_vec()),
        ];

        for (nome, bytes) in recusados {
            let resultado =
                identity::set_photograph(&pool, &principal, &store, "ocinye-test", &bytes).await;
            assert!(
                resultado.is_err(),
                "«{nome}» foi aceite como fotografia de perfil"
            );
        }

        let depois: i64 = sqlx::query_scalar("SELECT count(*) FROM storage_objects")
            .fetch_one(&pool)
            .await
            .expect("contagem");
        assert_eq!(antes, depois, "um upload recusado deixou objectos");

        assert_eq!(
            identity::own_avatar(&pool, &principal).await.expect("ler"),
            ocinye_contracts::AvatarChoice::Initials,
            "um upload recusado alterou a representação do membro"
        );
    })
    .await;
}

/// A fotografia de um membro não abre para outro.
///
/// A versão é um identificador de conteúdo, e não uma chave: quem a conheça não
/// ganha nada com isso.
#[tokio::test]
async fn a_versao_de_um_membro_nao_abre_para_outro() {
    let pool = skip_without_database!();
    let store = skip_without_storage!();
    com_registo_exclusivo(&pool, |pool| async move {
        registar_backend(&pool, store.bucket()).await;
        let organisation_id = organisation(&pool).await;
        let admin = admin(&pool, organisation_id).await;

        let (dona, _, _) = member(&pool, &admin, TechnicalRole::ResearchMember).await;
        let (outra, _, _) = member(&pool, &admin, TechnicalRole::ResearchMember).await;
        let principal_dona = identity::principal_for_person(&pool, &dona)
            .await
            .expect("principal");
        let principal_outra = identity::principal_for_person(&pool, &outra)
            .await
            .expect("principal");

        let escolha = identity::set_photograph(
            &pool,
            &principal_dona,
            &store,
            "ocinye-test",
            &fotografia(300, 300),
        )
        .await
        .expect("carregar");
        let ocinye_contracts::AvatarChoice::Custom { version } = escolha else {
            panic!("devia ser uma fotografia");
        };

        // A dona abre.
        let key = identity::own_photograph_key(&pool, &principal_dona, &version)
            .await
            .expect("a dona devia abrir a sua fotografia");

        // Outro membro, com a versão exacta na mão, não abre — e recebe a mesma
        // resposta que receberia se a versão nunca tivesse existido.
        let alheia = identity::own_photograph_key(&pool, &principal_outra, &version).await;
        assert!(
            matches!(alheia, Err(CoreError::NotFound(_))),
            "conhecer a versão de outra pessoa deu acesso: {alheia:?}"
        );

        // E carregar a sua própria fotografia não lhe dá a da outra.
        identity::set_photograph(
            &pool,
            &principal_outra,
            &store,
            "ocinye-test",
            &fotografia(200, 200),
        )
        .await
        .expect("carregar");
        let ainda = identity::own_photograph_key(&pool, &principal_dona, &version)
            .await
            .expect("a dona continua a abrir a sua");
        assert_eq!(ainda, key);

        // Limpeza determinística: nada fica no bucket depois do teste.
        identity::use_initials(&pool, &principal_dona, Some(&store))
            .await
            .expect("limpar");
        identity::use_initials(&pool, &principal_outra, Some(&store))
            .await
            .expect("limpar");
    })
    .await;
}

/// Trocar de fotografia para preset não deixa a fotografia no armazenamento.
///
/// Era o caminho esquecido. Substituir uma fotografia por outra apagava a
/// anterior; trocá-la por um avatar do produto — ou por iniciais — largava a
/// associação e deixava o objecto no bucket. A interface mostrava o preset, e
/// estava certa: ninguém dava por isso a olhar.
#[tokio::test]
async fn trocar_de_fotografia_para_preset_limpa_o_armazenamento() {
    let pool = skip_without_database!();
    let store = skip_without_storage!();
    com_registo_exclusivo(&pool, |pool| async move {
        registar_backend(&pool, store.bucket()).await;
        let organisation_id = organisation(&pool).await;
        let admin = admin(&pool, organisation_id).await;
        let (person, _, _) = member(&pool, &admin, TechnicalRole::ResearchMember).await;
        let principal = identity::principal_for_person(&pool, &person)
            .await
            .expect("principal");

        let escolha = identity::set_photograph(
            &pool,
            &principal,
            &store,
            "ocinye-test",
            &fotografia(250, 250),
        )
        .await
        .expect("carregar");
        let ocinye_contracts::AvatarChoice::Custom { version } = escolha else {
            panic!("devia ser uma fotografia");
        };
        let key = identity::own_photograph_key(&pool, &principal, &version)
            .await
            .expect("a fotografia devia abrir");
        assert!(
            store.get(&key).await.is_ok(),
            "a fotografia não foi guardada"
        );

        identity::choose_preset(&pool, &principal, Some(&store), "engineering-02")
            .await
            .expect("preset");

        assert_eq!(
            identity::own_avatar(&pool, &principal).await.expect("ler"),
            ocinye_contracts::AvatarChoice::Preset {
                preset: "engineering-02".to_owned()
            }
        );
        assert!(
            store.get(&key).await.is_err(),
            "trocar para um preset deixou a fotografia no armazenamento"
        );
        let linhas: i64 =
            sqlx::query_scalar("SELECT count(*) FROM storage_objects WHERE object_key = $1")
                .bind(&key)
                .fetch_one(&pool)
                .await
                .expect("contagem");
        assert_eq!(linhas, 0, "ficou uma linha órfã em storage_objects");
    })
    .await;
}

/// Sem armazenamento registado, carregar uma fotografia diz porquê.
///
/// `INSERT … SELECT` insere tantas linhas quantas o `SELECT` devolver, e sem
/// backend por omissão devolve zero — sem erro. O objecto ficava no bucket, a
/// linha não existia, e a operação rebentava numa violação de chave estrangeira
/// sobre um UUID que ninguém reconhecia. A causa era outra, e agora é dita.
#[tokio::test]
async fn sem_backend_registado_a_recusa_diz_a_causa() {
    let pool = skip_without_database!();
    let store = skip_without_storage!();
    let organisation_id = organisation(&pool).await;
    let admin = admin(&pool, organisation_id).await;
    let (person, _, _) = member(&pool, &admin, TechnicalRole::ResearchMember).await;
    let principal = identity::principal_for_person(&pool, &person)
        .await
        .expect("principal");

    // O registo é global: enquanto este teste o desliga, mais nenhum pode estar
    // a guardar objectos.
    com_registo_exclusivo(&pool, |pool| async move {
        sqlx::query("UPDATE storage_backends SET is_default = FALSE")
            .execute(&pool)
            .await
            .expect("desregistar");

        let erro = identity::set_photograph(
            &pool,
            &principal,
            &store,
            "ocinye-test",
            &fotografia(200, 200),
        )
        .await
        .expect_err("devia recusar sem backend registado");
        assert!(
            matches!(erro, CoreError::StorageUnavailable(ref m) if m.contains("storage backend")),
            "a recusa não explica a causa: {erro:?}"
        );

        // E o membro continua nas iniciais: uma recusa não deixa estado a meio.
        assert_eq!(
            identity::own_avatar(&pool, &principal).await.expect("ler"),
            ocinye_contracts::AvatarChoice::Initials
        );

        registar_backend(&pool, store.bucket()).await;
    })
    .await;
}

// ── O endereço é a credencial ───────────────────────────────────────────

/// Não há segundo identificador aceite em silêncio.
///
/// # Porque isto é um teste e não uma leitura do código
///
/// Porque um sistema que aceitasse dois teria duas superfícies de
/// autenticação — e a segunda seria a que ninguém revê. O ADR-0106 diz que há
/// uma; este teste é o que o torna verdade.
#[tokio::test]
async fn so_o_endereco_autentica() {
    let Some(pool) = pool().await else { return };
    let email = endereco();
    let (_, credencial) = identity::bootstrap_platform_admin(
        &pool,
        &authenticator(),
        organisation(&pool).await,
        "Quem Entra",
        &email,
        &CorrelationIds::generate(),
    )
    .await
    .expect("bootstrap");

    let auth = authenticator();
    let contexto = AttemptContext::default();
    let ids = CorrelationIds::generate();

    // Com o endereço, entra.
    auth.sign_in(&pool, &email, &credencial.secret, &contexto, &ids)
        .await
        .expect("o endereço tem de autenticar");

    // Com a parte antes da arroba — o que teria sido um username —, não.
    let parte = email.split('@').next().expect("parte local").to_owned();
    let resultado = auth
        .sign_in(&pool, &parte, &credencial.secret, &contexto, &ids)
        .await;
    assert!(
        resultado.is_err(),
        "a parte local do endereço autenticou: haveria dois identificadores"
    );
}

/// O endereço não distingue maiúsculas para entrar.
///
/// Quem escreve `Fidel@Ocinye.com` está a escrever o mesmo endereço, e um
/// sistema que o recusasse mandava uma pessoa adivinhar como se escreveu a si
/// própria no dia em que a conta foi criada.
#[tokio::test]
async fn o_endereco_nao_distingue_maiusculas_para_entrar() {
    let Some(pool) = pool().await else { return };
    let email = endereco();

    let (_, credencial) = identity::bootstrap_platform_admin(
        &pool,
        &authenticator(),
        organisation(&pool).await,
        "Quem Entra",
        &email,
        &CorrelationIds::generate(),
    )
    .await
    .expect("bootstrap");

    authenticator()
        .sign_in(
            &pool,
            &email.to_uppercase(),
            &credencial.secret,
            &AttemptContext::default(),
            &CorrelationIds::generate(),
        )
        .await
        .expect("o mesmo endereço em maiúsculas é o mesmo endereço");
}

/// A limitação de tentativas conta por endereço.
///
/// Contava por username. Se tivesse ficado a contar por uma coluna que já não
/// existe, deixaria de contar de todo — e a protecção desaparecia em silêncio.
#[tokio::test]
async fn as_tentativas_falhadas_contam_por_endereco() {
    let Some(pool) = pool().await else { return };
    let email = endereco();

    let (_, _) = identity::bootstrap_platform_admin(
        &pool,
        &authenticator(),
        organisation(&pool).await,
        "Quem Entra",
        &email,
        &CorrelationIds::generate(),
    )
    .await
    .expect("bootstrap");

    let auth = authenticator();
    let contexto = AttemptContext::default();
    let errada = Secret::new("isto-nao-e-a-palavra-passe");

    for _ in 0..3 {
        let _ = auth
            .sign_in(
                &pool,
                &email,
                &errada,
                &contexto,
                &CorrelationIds::generate(),
            )
            .await;
    }

    let contadas: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM authentication_attempts WHERE lower(email) = lower($1)",
    )
    .bind(&email)
    .fetch_one(&pool)
    .await
    .expect("contagem");

    assert!(
        contadas >= 3,
        "as tentativas contra este endereço não ficaram registadas: {contadas}"
    );
}
