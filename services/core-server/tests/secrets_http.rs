//! A Autoridade de Segredos, pelo HTTP (ADR-0110, Parte 6 da generalização).
//!
//! Criar um segredo de fornecedor, nunca o conseguir ler de volta, deixar um
//! serviço do Core usá-lo no seu âmbito, recusar quem não pode, rodá-lo,
//! revogá-lo — e procurar o valor em claro em **toda** a base, como se procura
//! num despejo.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use ocinye_contracts::{SessionState, TechnicalRole};
use ocinye_core::authn::TokenVerifier;
use ocinye_core::config::CoreConfig;
use ocinye_core::modules::identity::{self, Authenticator, Throttle};
use ocinye_core::modules::mail::provider::UnconfiguredProvider;
use ocinye_core::password::{Hasher, HashingParams, Secret};
use ocinye_core_server::routes;
use ocinye_core_server::state::AppState;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

/// Salta quando não há base de dados; **falha** quando há e algo corre mal.
macro_rules! pool {
    () => {{
        let Ok(url) = std::env::var("OCINYE_TEST_DATABASE_URL") else {
            eprintln!("skipping: OCINYE_TEST_DATABASE_URL is not set");
            return;
        };
        let pool = PgPool::connect(&url).await.expect("base de dados");
        sqlx::migrate!("../../migrations")
            .run(&pool)
            .await
            .expect("migrations");
        ocinye_core::fixtures::refuse_canonical_organisation(&pool).await;
        pool
    }};
}

fn config() -> CoreConfig {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        // SAFETY: uma escrita, antes de qualquer teste começar trabalho.
        unsafe {
            std::env::set_var("OCINYE_DATABASE_URL", "postgres://x/x");
        }
    });
    CoreConfig::from_env().expect("configuração de teste")
}

/// O Core desta instalação: sem armazenamento, sem embeddings, e — o que
/// importa aqui — com o fornecedor de inferência por omissão, `NoProvider`. Zero
/// modelos, zero nós. É o estado que M5.1 tem de servir sem falhar.
fn nucleo(pool: PgPool, organisation_id: Uuid) -> AppState {
    nucleo_com(
        pool,
        organisation_id,
        Arc::new(ocinye_core::modules::intelligence::NoProvider),
    )
}

/// O mesmo, com um fornecedor de inferência à escolha — para provar que o
/// caminho de execução roteia quando um fornecedor serve (M5.2, e a base do
/// hot-plug de M5.3).
fn nucleo_com(
    pool: PgPool,
    organisation_id: Uuid,
    inference: Arc<dyn ocinye_core::modules::intelligence::InferenceProvider>,
) -> AppState {
    let mut config = config();
    // A raiz de selagem desta prova: gerada aqui, nunca uma chave real.
    config.sealing_key = Some(
        ocinye_core::password::sealed::SealingKey::from_base64(
            &ocinye_core::password::sealed::SealingKey::generate(),
        )
        .expect("raiz de teste"),
    );
    let verifier = TokenVerifier::new(config.oidc.clone()).expect("verificador");
    let authenticator = Arc::new(Authenticator::new(
        Hasher::new(HashingParams {
            memory_kib: config.auth.argon2_memory_kib,
            iterations: config.auth.argon2_iterations,
            parallelism: config.auth.argon2_parallelism,
        }),
        Throttle {
            per_ip: config.auth.throttle_per_ip,
            per_email: config.auth.throttle_per_email,
            window_minutes: config.auth.throttle_window_minutes,
        },
        config.auth.temporary_credential_hours,
    ));
    let mail_registry = Arc::new(ocinye_core::modules::mail::ProviderRegistry::new(
        Arc::new(UnconfiguredProvider),
        config.mail.clone(),
        config.sealing_key.clone(),
    ));

    AppState {
        pool,
        config: Arc::new(config),
        verifier,
        authenticator,
        store: None,
        embeddings: None,
        inference,
        mail_registry,
        realtime: Arc::new(ocinye_core::realtime::Realtime::ausente()),
        mail_probe: Arc::new(SondaDoHarness),
        capabilities: Arc::new(
            ocinye_core::capabilities::Capabilities::empty().expect("motor de capacidades"),
        ),
        organisation_id,
    }
}

async fn organisation(pool: &PgPool) -> Uuid {
    sqlx::query_scalar("INSERT INTO organisations (slug, name) VALUES ($1, $2) RETURNING id")
        .bind(format!("sec-{}", Uuid::new_v4().simple()))
        .bind("Instituição dos segredos")
        .fetch_one(pool)
        .await
        .expect("organização")
}

/// Uma pessoa que pode usar IA (`ResearchMember` concede `AiUse`), e a sua
/// sessão activa.
async fn membro(pool: &PgPool, organisation_id: Uuid, role: TechnicalRole) -> (Uuid, Secret) {
    let handle = format!("m{}", Uuid::new_v4().simple());

    let person_id: Uuid = sqlx::query_scalar(
        "INSERT INTO people (organisation_id, full_name, email, status)
             VALUES ($1, $2, $3, 'active') RETURNING id",
    )
    .bind(organisation_id)
    .bind(&handle)
    .bind(format!("{handle}@ocinye.com"))
    .fetch_one(pool)
    .await
    .expect("pessoa");

    sqlx::query("INSERT INTO person_roles (person_id, role) VALUES ($1, $2)")
        .bind(person_id)
        .bind(role.as_str())
        .execute(pool)
        .await
        .expect("papel");

    let mut bytes = [0_u8; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        let seed = Uuid::new_v4().as_u128();
        *byte = ((seed >> ((index % 16) * 8)) & 0xff) as u8;
    }
    let token = Secret::new(
        bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>(),
    );

    // O segundo factor satisfeito: um administrador da plataforma sem ele não
    // exerce autoridade (ADR-0107), e esta prova é sobre nós, não sobre MFA.
    sqlx::query(
        "INSERT INTO sessions (person_id, token_digest, state, expires_at, user_agent, mfa_satisfied)
             VALUES ($1, $2, $3, now() + interval '1 hour', 'secrets-http-test', true)",
    )
    .bind(person_id)
    .bind(identity::session_digest(&token))
    .bind(SessionState::Active.as_str())
    .execute(pool)
    .await
    .expect("sessão");

    (person_id, token)
}


async fn pedido(
    state: &AppState,
    token: Option<&Secret>,
    method: &str,
    path: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(path);
    if let Some(token) = token {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {}", token.expose()));
    }
    let request = match body {
        Some(body) => builder
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_vec(&body).expect("json"))),
        None => builder.body(Body::empty()),
    }
    .expect("pedido");
    let response = routes::router(state.clone())
        .oneshot(request)
        .await
        .expect("resposta");
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("corpo");
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}


async fn pedido(
    state: &AppState,
    token: Option<&Secret>,
    no: Option<&str>,
    method: &str,
    path: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(path);
    if let Some(token) = token {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {}", token.expose()));
    }
    if let Some(no) = no {
        builder = builder.header("x-ocinye-node-token", no);
    }
    let request = match body {
        Some(body) => builder
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_vec(&body).expect("json"))),
        None => builder.body(Body::empty()),
    }
    .expect("pedido");
    let response = routes::router(state.clone())
        .oneshot(request)
        .await
        .expect("resposta");
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("corpo");
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}


/// Quantas linhas, em todas as tabelas da base, contêm este texto — a
/// equivalência de procurar num `pg_dump`.
async fn ocorrencias_na_base(pool: &PgPool, agulha: &str) -> i64 {
    let tabelas: Vec<String> = sqlx::query_scalar(
        "SELECT table_name::text FROM information_schema.tables
          WHERE table_schema = 'public' AND table_type = 'BASE TABLE'",
    )
    .fetch_all(pool)
    .await
    .expect("tabelas");
    let mut total = 0_i64;
    for tabela in tabelas {
        let n: i64 = sqlx::query_scalar(&format!(
            "SELECT count(*) FROM \"{tabela}\" t WHERE t::text LIKE '%' || $1 || '%'"
        ))
        .bind(agulha)
        .fetch_one(pool)
        .await
        .unwrap_or(0);
        total += n;
    }
    total
}

#[tokio::test]
async fn um_segredo_de_fornecedor_guarda_se_usa_se_roda_e_revoga_sem_nunca_sair() {
    use ocinye_core::modules::secrets::{self, SecretScope};

    let pool = pool!();
    let org = organisation(&pool).await;
    let state = nucleo(pool.clone(), org);
    let (_, admin) = membro(&pool, org, TechnicalRole::PlatformAdmin).await;
    let (_, pessoa) = membro(&pool, org, TechnicalRole::ResearchMember).await;
    let ids = ocinye_observability::CorrelationIds::generate();

    let primeiro = format!("sk-test-{}", Uuid::new_v4().simple());
    let (status, criado) = pedido(
        &state,
        Some(&admin),
        None,
        "POST",
        "/api/v1/instance/secrets",
        Some(json!({
            "kind": "ai_provider",
            "label": "Fornecedor de teste",
            "scope": "ai_gateway",
            "value": primeiro
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{criado}");
    let id: Uuid = criado["id"].as_str().expect("id").parse().expect("uuid");
    assert_eq!(criado["hint"], &primeiro[primeiro.len() - 4..], "só os últimos quatro");
    assert!(!criado.to_string().contains(&primeiro), "a criação não devolve o valor");

    // Guardado selado: nenhuma linha da base contém o valor em claro.
    assert_eq!(ocorrencias_na_base(&pool, &primeiro).await, 0, "o valor em claro chegou à base");

    // A API não o devolve: a lista é só metadados, e não há rota de leitura.
    let (_, lista) = pedido(&state, Some(&admin), None, "GET", "/api/v1/instance/secrets", None).await;
    assert!(!lista.to_string().contains(&primeiro));
    for caminho in [format!("/api/v1/instance/secrets/{id}"), format!("/api/v1/instance/secrets/{id}/value")] {
        let (s, _) = pedido(&state, Some(&admin), None, "GET", &caminho, None).await;
        assert!(
            s == StatusCode::NOT_FOUND || s == StatusCode::METHOD_NOT_ALLOWED,
            "{caminho} não pode existir: {s}"
        );
    }

    // O serviço autorizado usa-o; outro âmbito não.
    let usado = secrets::use_secret(&pool, state.config.sealing_key.as_ref(), org, id, SecretScope::AiGateway, &ids)
        .await
        .expect("o AI Gateway usa o segredo");
    assert_eq!(usado.expose(), primeiro);
    assert!(
        secrets::use_secret(&pool, state.config.sealing_key.as_ref(), org, id, SecretScope::Mail, &ids)
            .await
            .is_err(),
        "fora do âmbito, o segredo não existe"
    );
    let outra_org = organisation(&pool).await;
    assert!(
        secrets::use_secret(&pool, state.config.sealing_key.as_ref(), outra_org, id, SecretScope::AiGateway, &ids)
            .await
            .is_err(),
        "outra instância não o abre"
    );

    // Quem não administra a plataforma não cria, não lista, não roda, não revoga.
    for (metodo, caminho, corpo) in [
        ("GET", "/api/v1/instance/secrets".to_owned(), None),
        (
            "POST",
            "/api/v1/instance/secrets".to_owned(),
            Some(json!({ "kind": "x", "label": "x", "scope": "ai_gateway", "value": "0123456789abcdef" })),
        ),
        ("POST", format!("/api/v1/instance/secrets/{id}/rotate"), Some(json!({ "value": "0123456789abcdef" }))),
        ("POST", format!("/api/v1/instance/secrets/{id}/revoke"), None),
    ] {
        let (s, _) = pedido(&state, Some(&pessoa), None, metodo, &caminho, corpo).await;
        assert_eq!(s, StatusCode::FORBIDDEN, "{metodo} {caminho}");
    }

    // Rodar: a versão sobe, o valor novo serve, o antigo deixa de existir.
    let segundo = format!("sk-test-{}", Uuid::new_v4().simple());
    let (status, rodado) = pedido(
        &state,
        Some(&admin),
        None,
        "POST",
        &format!("/api/v1/instance/secrets/{id}/rotate"),
        Some(json!({ "value": segundo })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(rodado["version"], 2);
    let usado = secrets::use_secret(&pool, state.config.sealing_key.as_ref(), org, id, SecretScope::AiGateway, &ids)
        .await
        .expect("usar depois de rodar");
    assert_eq!(usado.expose(), segundo, "a credencial antiga deixou de ser a que serve");
    assert_ne!(usado.expose(), primeiro);

    // Revogar: o criptograma apaga-se, e o uso falha — o fornecedor fica sem ela.
    let (status, revogado) = pedido(
        &state,
        Some(&admin),
        None,
        "POST",
        &format!("/api/v1/instance/secrets/{id}/revoke"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(revogado["status"], "revoked");
    assert!(
        secrets::use_secret(&pool, state.config.sealing_key.as_ref(), org, id, SecretScope::AiGateway, &ids)
            .await
            .is_err(),
        "um segredo revogado não se usa"
    );
    let criptograma: Option<Vec<u8>> =
        sqlx::query_scalar("SELECT ciphertext FROM instance_secrets WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .expect("linha");
    assert!(criptograma.is_none(), "revogar apaga o criptograma");

    // E, no fim, nenhum dos dois valores existe em claro em lado nenhum da base
    // — incluindo o registo de auditoria das cinco operações.
    assert_eq!(ocorrencias_na_base(&pool, &primeiro).await, 0);
    assert_eq!(ocorrencias_na_base(&pool, &segundo).await, 0);
}

struct SondaDoHarness;

#[async_trait::async_trait]
impl ocinye_core::modules::mail::provider::CredentialProbe for SondaDoHarness {
    async fn verify(
        &self,
        _endereco: &str,
        _username: &str,
        _senha: &str,
    ) -> ocinye_core::modules::mail::provider::ProviderResult<()> {
        Ok(())
    }
}
