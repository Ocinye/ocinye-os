//! Política e roteamento de IA entre vários fornecedores, pelo HTTP (ADR-0311,
//! Parte 8 da generalização).
//!
//! Os cenários A–H do programa, contra dois fornecedores simulados em portas
//! efémeras e um externo num endereço onde nada responde — para que «o externo
//! nunca foi contactado» seja observável pela sua saúde, que fica `unknown`.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use axum::response::IntoResponse;
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

/// O mesmo Core, numa instalação que admite fornecedores externos: é a
/// política da Instância, e não o interruptor da instalação, que tem de os travar.
fn nucleo_com_externos(pool: PgPool, organisation_id: Uuid) -> AppState {
    let mut state = nucleo(pool, organisation_id);
    let mut config = (*state.config).clone();
    config.ai.allow_external_providers = true;
    state.config = Arc::new(config);
    state
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
        .bind(format!("air-{}", Uuid::new_v4().simple()))
        .bind("Instituição do roteamento")
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
             VALUES ($1, $2, $3, now() + interval '1 hour', 'ai-routing-http-test', true)",
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
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

/// Um fornecedor simulado. Fala *chat completions* e a Messages API, e
/// responde com o seu nome — ou `503`, quando o mandam cair.
struct Simulado {
    endpoint: String,
    chamadas: Arc<std::sync::atomic::AtomicUsize>,
    em_baixo: Arc<std::sync::atomic::AtomicBool>,
}

async fn simulado(nome: &'static str) -> Simulado {
    use axum::routing::post;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    let chamadas = Arc::new(AtomicUsize::new(0));
    let em_baixo = Arc::new(AtomicBool::new(false));
    let (c, b) = (chamadas.clone(), em_baixo.clone());
    let responder = move || {
        let (c, b) = (c.clone(), b.clone());
        async move {
            c.fetch_add(1, Ordering::SeqCst);
            if b.load(Ordering::SeqCst) {
                return (StatusCode::SERVICE_UNAVAILABLE, axum::Json(json!({}))).into_response();
            }
            axum::Json(json!({
                "choices": [{ "message": { "role": "assistant", "content": nome } }],
                "content": [{ "type": "text", "text": nome }],
                "usage": { "prompt_tokens": 1, "completion_tokens": 1,
                           "input_tokens": 1, "output_tokens": 1 }
            }))
            .into_response()
        }
    };
    let (r1, r2) = (responder.clone(), responder);
    let app = axum::Router::new()
        .route("/v1/chat/completions", post(r1))
        .route("/v1/messages", post(r2));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("porta");
    let endereco = listener.local_addr().expect("endereço");
    tokio::spawn(async move { axum::serve(listener, app).await });
    Simulado {
        endpoint: format!("http://{endereco}"),
        chamadas,
        em_baixo,
    }
}

impl Simulado {
    fn chamadas(&self) -> usize {
        self.chamadas.load(std::sync::atomic::Ordering::SeqCst)
    }
    fn cair(&self, caido: bool) {
        self.em_baixo
            .store(caido, std::sync::atomic::Ordering::SeqCst);
    }
}

async fn registar(
    state: &AppState,
    admin: &Secret,
    corpo: Value,
    modelo: &str,
    capacidades: Value,
    tecto: &str,
) -> String {
    let (status, fornecedor) = pedido(
        state,
        Some(admin),
        None,
        "POST",
        "/api/v1/ai/providers",
        Some(corpo),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{fornecedor}");
    let id = fornecedor["id"].as_str().expect("id").to_owned();
    let (status, m) = pedido(
        state,
        Some(admin),
        None,
        "POST",
        &format!("/api/v1/ai/providers/{id}/models"),
        Some(json!({ "model_name": modelo, "capabilities": capacidades, "max_classification": tecto })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{m}");
    id
}

async fn perguntar(
    state: &AppState,
    token: &Secret,
    capacidade: &str,
    classificacao: &str,
) -> Value {
    let (status, corpo) = pedido(
        state,
        Some(token),
        None,
        "POST",
        "/api/v1/ai/prompt",
        Some(
            json!({ "prompt": "Resume o relatório.", "capability": capacidade,
                     "classification": classificacao }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{corpo}");
    corpo
}

async fn activar(state: &AppState, admin: &Secret, id: &str, activo: bool) {
    let (status, _) = pedido(
        state,
        Some(admin),
        None,
        "PUT",
        &format!("/api/v1/ai/providers/{id}/enabled"),
        Some(json!({ "enabled": activo })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

async fn preferir(
    state: &AppState,
    admin: &Secret,
    capacidade: &str,
    id: Option<&str>,
    recurso: bool,
) {
    let (status, corpo) = pedido(
        state,
        Some(admin),
        None,
        "PUT",
        &format!("/api/v1/ai/routing/{capacidade}"),
        Some(json!({ "preferred_provider_id": id, "allow_fallback": recurso })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{corpo}");
}

#[tokio::test]
async fn o_router_escolhe_por_capacidade_preferencia_politica_e_disponibilidade() {
    let pool = pool!();
    let org = organisation(&pool).await;
    // A instalação admite externos: é a política da Instância que tem de os travar.
    let state = nucleo_com_externos(pool.clone(), org);
    let (_, admin) = membro(&pool, org, TechnicalRole::PlatformAdmin).await;
    let (_, pessoa) = membro(&pool, org, TechnicalRole::ResearchMember).await;
    let a = simulado("resposta de A").await;
    let b = simulado("resposta de B").await;

    // ── A. Só o fornecedor A ─────────────────────────────────────────────
    let id_a = registar(
        &state,
        &admin,
        json!({ "kind": "openai_compatible", "label": "A", "residency": "local",
                "endpoint_url": format!("{}/v1", a.endpoint) }),
        "modelo-a",
        json!(["GENERAL"]),
        "CONFIDENTIAL",
    )
    .await;
    let r = perguntar(&state, &pessoa, "GENERAL", "INTERNAL").await;
    assert_eq!(r["content"], "resposta de A", "{r}");

    // ── B. Só o fornecedor B ─────────────────────────────────────────────
    activar(&state, &admin, &id_a, false).await;
    let id_b = registar(
        &state,
        &admin,
        json!({ "kind": "anthropic", "label": "B", "residency": "local",
                "endpoint_url": b.endpoint }),
        "modelo-b",
        json!(["GENERAL", "CODING"]),
        "CONFIDENTIAL",
    )
    .await;
    let r = perguntar(&state, &pessoa, "GENERAL", "INTERNAL").await;
    assert_eq!(r["content"], "resposta de B", "{r}");

    // ── C. Os dois activos: a preferência decide ─────────────────────────
    activar(&state, &admin, &id_a, true).await;
    preferir(&state, &admin, "GENERAL", Some(&id_a), true).await;
    assert_eq!(
        perguntar(&state, &pessoa, "GENERAL", "INTERNAL").await["content"],
        "resposta de A"
    );
    preferir(&state, &admin, "GENERAL", Some(&id_b), true).await;
    assert_eq!(
        perguntar(&state, &pessoa, "GENERAL", "INTERNAL").await["content"],
        "resposta de B"
    );

    // ── D. Por capacidade: só B serve CODING, prefira-se quem se preferir ─
    preferir(&state, &admin, "GENERAL", Some(&id_a), true).await;
    assert_eq!(
        perguntar(&state, &pessoa, "CODING", "INTERNAL").await["content"],
        "resposta de B"
    );
    assert_eq!(
        perguntar(&state, &pessoa, "GENERAL", "INTERNAL").await["content"],
        "resposta de A"
    );

    // ── E. A em baixo: recurso permitido responde B; proibido, degrada ───
    a.cair(true);
    let antes = a.chamadas();
    assert_eq!(
        perguntar(&state, &pessoa, "GENERAL", "INTERNAL").await["content"],
        "resposta de B"
    );
    assert_eq!(a.chamadas(), antes + 1, "o preferido foi tentado primeiro");
    preferir(&state, &admin, "GENERAL", Some(&id_a), false).await;
    let r = perguntar(&state, &pessoa, "GENERAL", "INTERNAL").await;
    assert_eq!(
        r["origin"].as_str().map(str::to_uppercase).as_deref(),
        Some("SYSTEM"),
        "{r}"
    );
    assert_eq!(r["reason_code"], "AI_PROVIDER_UNHEALTHY");
    a.cair(false);

    // ── F. Confidencial nunca sai para um externo ────────────────────────
    // Um externo que aceitaria tudo — o tecto do modelo é RESTRICTED —, num
    // endereço onde nada responde: se o Router o escolhesse, a saúde dele
    // deixaria de ser `unknown`.
    let id_x = registar(
        &state,
        &admin,
        json!({ "kind": "openai", "label": "Nuvem", "residency": "external",
                "endpoint_url": "https://127.0.0.1:9/v1" }),
        "modelo-nuvem",
        json!(["REASONING", "GENERAL"]),
        "RESTRICTED",
    )
    .await;
    preferir(&state, &admin, "GENERAL", Some(&id_x), true).await;
    // Um local compatível responde, e o externo preferido nem é tentado.
    let r = perguntar(&state, &pessoa, "GENERAL", "CONFIDENTIAL").await;
    assert!(
        r["content"] == "resposta de A" || r["content"] == "resposta de B",
        "{r}"
    );
    // Sem local compatível, a resposta é o erro tipado de política.
    let r = perguntar(&state, &pessoa, "REASONING", "CONFIDENTIAL").await;
    assert_eq!(r["reason_code"], "AI_POLICY_BLOCKED", "{r}");
    let saude = |lista: &Value| {
        lista
            .as_array()
            .expect("lista")
            .iter()
            .find(|p| p["id"] == id_x.as_str())
            .expect("externo")["health"]
            .clone()
    };
    let (_, lista) = pedido(
        &state,
        Some(&admin),
        None,
        "GET",
        "/api/v1/ai/providers",
        None,
    )
    .await;
    assert_eq!(
        saude(&lista),
        "unknown",
        "o externo foi contactado com dados confidenciais"
    );
    // A política da Instância pode fechar a IA externa por completo.
    let (status, _) = pedido(
        &state,
        Some(&admin),
        None,
        "PUT",
        "/api/v1/ai/policy",
        Some(json!({ "external_max_classification": "NONE" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let r = perguntar(&state, &pessoa, "REASONING", "PUBLIC").await;
    assert_eq!(r["reason_code"], "AI_POLICY_BLOCKED", "{r}");
    let (_, lista) = pedido(
        &state,
        Some(&admin),
        None,
        "GET",
        "/api/v1/ai/providers",
        None,
    )
    .await;
    assert_eq!(saude(&lista), "unknown");
    // Controlo: com a política aberta e dados públicos, é a política — e não
    // outra coisa — que o travava: agora é tentado (e falha, porque nada
    // responde naquele endereço).
    let (status, _) = pedido(
        &state,
        Some(&admin),
        None,
        "PUT",
        "/api/v1/ai/policy",
        Some(json!({ "external_max_classification": "INTERNAL" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let _ = perguntar(&state, &pessoa, "REASONING", "PUBLIC").await;
    let (_, lista) = pedido(
        &state,
        Some(&admin),
        None,
        "GET",
        "/api/v1/ai/providers",
        None,
    )
    .await;
    assert_eq!(
        saude(&lista),
        "unreachable",
        "o controlo não chegou ao externo"
    );

    // ── G. Nada compatível: resposta degradada e determinística ──────────
    let r = perguntar(&state, &pessoa, "EMBEDDING", "PUBLIC").await;
    assert_eq!(r["reason_code"], "AI_NO_COMPATIBLE_MODEL", "{r}");

    // ── H. Desactivar muda o roteamento sem reinício ─────────────────────
    preferir(&state, &admin, "GENERAL", None, true).await;
    activar(&state, &admin, &id_a, false).await;
    activar(&state, &admin, &id_x, false).await;
    assert_eq!(
        perguntar(&state, &pessoa, "GENERAL", "INTERNAL").await["content"],
        "resposta de B"
    );
    activar(&state, &admin, &id_b, false).await;
    let r = perguntar(&state, &pessoa, "GENERAL", "INTERNAL").await;
    assert_eq!(
        r["origin"].as_str().map(str::to_uppercase).as_deref(),
        Some("SYSTEM"),
        "{r}"
    );

    // Quem não administra a IA não muda a política.
    let (status, _) = pedido(
        &state,
        Some(&pessoa),
        None,
        "PUT",
        "/api/v1/ai/policy",
        Some(json!({ "external_max_classification": "RESTRICTED" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
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
