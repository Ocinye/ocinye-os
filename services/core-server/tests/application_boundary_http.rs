//! A fronteira entre o Core e as aplicações, pelo HTTP (ADR-0014, Parte 3).
//!
//! Uma aplicação inactiva numa Instância é recusada **pelo Core**, venha o
//! pedido de onde vier — e o resto do Core continua a responder: identidade,
//! prontidão, configuração da Instância, e as outras aplicações.

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
    let config = config();
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
        .bind(format!("bnd-{}", Uuid::new_v4().simple()))
        .bind("Instituição da fronteira")
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

    sqlx::query(
        "INSERT INTO sessions (person_id, token_digest, state, expires_at, user_agent)
             VALUES ($1, $2, $3, now() + interval '1 hour', 'boundary-http-test')",
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

#[tokio::test]
async fn uma_aplicacao_inactiva_e_recusada_pelo_core_e_o_resto_continua() {
    let pool = pool!();
    let org = organisation(&pool).await;
    let state = nucleo(pool.clone(), org);
    let (_, admin) = membro(&pool, org, TechnicalRole::OrganisationAdmin).await;
    let (_, pessoa) = membro(&pool, org, TechnicalRole::ResearchMember).await;

    // Antes: o Correio responde (não configurado, mas responde).
    let (antes, _) = pedido(&state, Some(&pessoa), "GET", "/api/v1/mail/status", None).await;
    assert_eq!(antes, StatusCode::OK);

    let (status, _) = pedido(
        &state,
        Some(&admin),
        "PUT",
        "/api/v1/instance/applications/mail",
        Some(json!({ "active": false })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "o administrador desactiva o Correio");

    // A API do Correio recusa, com o código tipado.
    let (recusa, corpo) = pedido(&state, Some(&pessoa), "GET", "/api/v1/mail/status", None).await;
    assert_eq!(recusa, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(corpo["code"], "application_inactive", "{corpo}");

    // O resto do Core continua: identidade, prontidão, configuração, outras
    // aplicações — e o `me` diz o que está inactivo.
    let (me, corpo_me) = pedido(&state, Some(&pessoa), "GET", "/api/v1/me", None).await;
    assert_eq!(me, StatusCode::OK);
    assert!(corpo_me["inactive_applications"]
        .as_array()
        .is_some_and(|a| a.iter().any(|id| id == "mail")));
    let (pronto, _) = pedido(&state, None, "GET", "/ready", None).await;
    assert_eq!(pronto, StatusCode::OK, "o Core continua pronto");
    let (config, _) =
        pedido(&state, Some(&admin), "GET", "/api/v1/instance/applications", None).await;
    assert_eq!(config, StatusCode::OK);
    let (calendario, _) =
        pedido(&state, Some(&pessoa), "GET", "/api/v1/calendar/agenda", None).await;
    assert_ne!(calendario, StatusCode::SERVICE_UNAVAILABLE, "o Calendário não é afectado");

    // Reactivar (voltar ao perfil) devolve-a.
    let (status, _) = pedido(
        &state,
        Some(&admin),
        "PUT",
        "/api/v1/instance/applications/mail",
        Some(json!({ "active": null })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (depois, _) = pedido(&state, Some(&pessoa), "GET", "/api/v1/mail/status", None).await;
    assert_eq!(depois, StatusCode::OK);
}

#[tokio::test]
async fn uma_aplicacao_essencial_nao_se_desactiva_pelo_http() {
    let pool = pool!();
    let org = organisation(&pool).await;
    let state = nucleo(pool.clone(), org);
    let (_, admin) = membro(&pool, org, TechnicalRole::OrganisationAdmin).await;
    let (status, _) = pedido(
        &state,
        Some(&admin),
        "PUT",
        "/api/v1/instance/applications/files",
        Some(json!({ "active": false })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (desconhecida, _) = pedido(
        &state,
        Some(&admin),
        "PUT",
        "/api/v1/instance/applications/nao-existe",
        Some(json!({ "active": false })),
    )
    .await;
    assert_eq!(desconhecida, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn um_membro_nao_configura_a_instancia() {
    let pool = pool!();
    let org = organisation(&pool).await;
    let state = nucleo(pool.clone(), org);
    let (_, pessoa) = membro(&pool, org, TechnicalRole::ResearchMember).await;
    for (caminho, corpo) in [
        ("/api/v1/instance/applications/notes", json!({ "active": false })),
        ("/api/v1/instance/profile", json!({ "profile": "personal" })),
    ] {
        let (status, _) = pedido(&state, Some(&pessoa), "PUT", caminho, Some(corpo)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{caminho}");
    }
    let (status, _) = pedido(&state, Some(&pessoa), "GET", "/api/v1/mail/status", None).await;
    assert_eq!(status, StatusCode::OK, "nada ficou desactivado");
}

/// A decisão de uma Instância não recusa pedidos noutra.
#[tokio::test]
async fn a_recusa_e_da_instancia_que_a_decidiu() {
    let pool = pool!();
    let a = organisation(&pool).await;
    let b = organisation(&pool).await;
    let estado_a = nucleo(pool.clone(), a);
    let estado_b = nucleo(pool.clone(), b);
    let (_, admin_a) = membro(&pool, a, TechnicalRole::OrganisationAdmin).await;
    let (_, pessoa_b) = membro(&pool, b, TechnicalRole::ResearchMember).await;
    let (status, _) = pedido(
        &estado_a,
        Some(&admin_a),
        "PUT",
        "/api/v1/instance/applications/mail",
        Some(json!({ "active": false })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (em_b, _) = pedido(&estado_b, Some(&pessoa_b), "GET", "/api/v1/mail/status", None).await;
    assert_eq!(em_b, StatusCode::OK);
}

/// Uma aplicação que falha não derruba o Core: o `panic` de um handler é um
/// `500` com o envelope de sempre, a causa não sai para o cliente, e a rota ao
/// lado continua a responder.
#[tokio::test]
async fn um_panic_numa_aplicacao_e_um_500_e_o_core_continua() {
    use axum::routing::get;
    let router = axum::Router::new()
        .route("/avaria", get(|| async { panic!("detalhe interno que não pode sair") }))
        .route("/vizinha", get(|| async { "ok" }))
        .layer(routes::panic_boundary());

    let avaria = router
        .clone()
        .oneshot(Request::builder().uri("/avaria").body(Body::empty()).expect("pedido"))
        .await
        .expect("resposta");
    assert_eq!(avaria.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let bytes = axum::body::to_bytes(avaria.into_body(), 64 * 1024)
        .await
        .expect("corpo");
    let corpo: Value = serde_json::from_slice(&bytes).expect("envelope");
    assert_eq!(corpo["code"], "internal_error");
    assert!(!String::from_utf8_lossy(&bytes).contains("detalhe interno"));

    let vizinha = router
        .oneshot(Request::builder().uri("/vizinha").body(Body::empty()).expect("pedido"))
        .await
        .expect("resposta");
    assert_eq!(vizinha.status(), StatusCode::OK);
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
