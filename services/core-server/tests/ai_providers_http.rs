//! O tecido de IA, pelo HTTP (ADR-0310, Parte 7 da generalização).
//!
//! Um fornecedor compatível com OpenAI numa porta efémera faz de Ollama ou de
//! vLLM. A Instância regista-o com uma credencial da Autoridade de Segredos,
//! regista um modelo seu, e o Prompt passa a responder por ele — com a
//! credencial no cabeçalho, e em mais lado nenhum. Desactivá-lo, revogar-lhe o
//! segredo ou declará-lo externo numa Instância que não os admite devolve o
//! Prompt ao estado degradado, sem reinício e sem chamada.

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
        .bind(format!("aif-{}", Uuid::new_v4().simple()))
        .bind("Instituição do tecido de IA")
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
             VALUES ($1, $2, $3, now() + interval '1 hour', 'ai-fabric-http-test', true)",
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

/// Um fornecedor compatível com OpenAI: responde sempre o mesmo, e guarda o
/// cabeçalho de autorização de cada chamada.
async fn fornecedor_simulado() -> (String, Arc<std::sync::Mutex<Vec<String>>>) {
    use axum::routing::post;
    let vistos: Arc<std::sync::Mutex<Vec<String>>> = Arc::default();
    let registo = vistos.clone();
    let app = axum::Router::new().route(
        "/v1/chat/completions",
        post(move |headers: axum::http::HeaderMap| {
            let registo = registo.clone();
            async move {
                let autorizacao = headers
                    .get(header::AUTHORIZATION)
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or_default()
                    .to_owned();
                registo.lock().expect("lock").push(autorizacao);
                axum::Json(json!({
                    "model": "modelo-local",
                    "choices": [{ "message": { "role": "assistant",
                                               "content": "resposta do fornecedor simulado" } }],
                    "usage": { "prompt_tokens": 12, "completion_tokens": 5 }
                }))
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("porta");
    let endereco = listener.local_addr().expect("endereço");
    tokio::spawn(async move { axum::serve(listener, app).await });
    (format!("http://{endereco}/v1"), vistos)
}

async fn perguntar(state: &AppState, token: &Secret, capacidade: &str) -> Value {
    let (status, corpo) = pedido(
        state,
        Some(token),
        None,
        "POST",
        "/api/v1/ai/prompt",
        Some(json!({ "prompt": "Resume o estado do laboratório.", "capability": capacidade })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{corpo}");
    corpo
}

fn origem(corpo: &Value) -> String {
    corpo["origin"].as_str().unwrap_or_default().to_uppercase()
}

#[tokio::test]
async fn um_fornecedor_registado_responde_ao_prompt_e_sai_dele_sem_reinicio() {
    let pool = pool!();
    let org = organisation(&pool).await;
    let state = nucleo(pool.clone(), org);
    let (_, admin) = membro(&pool, org, TechnicalRole::PlatformAdmin).await;
    let (_, pessoa) = membro(&pool, org, TechnicalRole::ResearchMember).await;
    let (endpoint, vistos) = fornecedor_simulado().await;

    // A credencial entra pela Autoridade de Segredos, no âmbito do Gateway.
    let chave = format!("sk-local-{}", Uuid::new_v4().simple());
    let (status, segredo) = pedido(
        &state,
        Some(&admin),
        None,
        "POST",
        "/api/v1/instance/secrets",
        Some(
            json!({ "kind": "ai_provider", "label": "Ollama do laboratório",
                     "scope": "ai_gateway", "value": chave }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{segredo}");
    let secret_id = segredo["id"].as_str().expect("id do segredo").to_owned();

    // Quem não administra a infraestrutura de IA não regista fornecedores.
    let novo = json!({ "kind": "openai_compatible", "label": "Ollama do laboratório",
                       "endpoint_url": endpoint, "residency": "local", "secret_id": secret_id });
    let (status, _) = pedido(
        &state,
        Some(&pessoa),
        None,
        "POST",
        "/api/v1/ai/providers",
        Some(novo.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, fornecedor) = pedido(
        &state,
        Some(&admin),
        None,
        "POST",
        "/api/v1/ai/providers",
        Some(novo),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{fornecedor}");
    assert!(
        !fornecedor.to_string().contains(&chave),
        "o registo devolveu a credencial"
    );
    let provider_id = fornecedor["id"].as_str().expect("id").to_owned();

    // Sem modelo registado, o fornecedor não serve nada: o Prompt degrada.
    assert_eq!(
        origem(&perguntar(&state, &pessoa, "GENERAL").await),
        "SYSTEM"
    );
    assert!(vistos.lock().expect("lock").is_empty());

    let (status, modelo) = pedido(
        &state,
        Some(&admin),
        None,
        "POST",
        &format!("/api/v1/ai/providers/{provider_id}/models"),
        Some(json!({ "model_name": "modelo-local", "version": "7b", "capabilities": ["GENERAL"] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{modelo}");

    // O Prompt responde pelo fornecedor, com a credencial no cabeçalho.
    let resposta = perguntar(&state, &pessoa, "GENERAL").await;
    assert_eq!(origem(&resposta), "MODEL", "{resposta}");
    assert_eq!(resposta["content"], "resposta do fornecedor simulado");
    assert_eq!(resposta["model"], "modelo-local");
    assert_eq!(resposta["provider"], "Ollama do laboratório");
    assert_eq!(
        vistos.lock().expect("lock").as_slice(),
        [format!("Bearer {chave}")]
    );
    let (_, lista) = pedido(
        &state,
        Some(&admin),
        None,
        "GET",
        "/api/v1/ai/providers",
        None,
    )
    .await;
    assert_eq!(lista[0]["health"], "healthy", "{lista}");

    // Desactivado, sai do roteamento no pedido seguinte — sem reinício.
    let (status, _) = pedido(
        &state,
        Some(&admin),
        None,
        "PUT",
        &format!("/api/v1/ai/providers/{provider_id}/enabled"),
        Some(json!({ "enabled": false })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        origem(&perguntar(&state, &pessoa, "GENERAL").await),
        "SYSTEM"
    );
    assert_eq!(
        vistos.lock().expect("lock").len(),
        1,
        "um fornecedor desactivado foi chamado"
    );

    // Reactivado, volta.
    let (status, _) = pedido(
        &state,
        Some(&admin),
        None,
        "PUT",
        &format!("/api/v1/ai/providers/{provider_id}/enabled"),
        Some(json!({ "enabled": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        origem(&perguntar(&state, &pessoa, "GENERAL").await),
        "MODEL"
    );
    assert_eq!(vistos.lock().expect("lock").len(), 2);

    // Revogado o segredo, o Gateway não tem com que chamar — e não chama.
    let (status, _) = pedido(
        &state,
        Some(&admin),
        None,
        "POST",
        &format!("/api/v1/instance/secrets/{secret_id}/revoke"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let degradada = perguntar(&state, &pessoa, "GENERAL").await;
    assert_eq!(origem(&degradada), "SYSTEM");
    assert_eq!(
        degradada["reason_code"], "AI_PROVIDER_UNHEALTHY",
        "{degradada}"
    );
    assert_eq!(
        vistos.lock().expect("lock").len(),
        2,
        "chamou sem credencial"
    );

    // A credencial nunca chegou à base em claro.
    assert_eq!(
        ocorrencias_na_base(&pool, &chave).await,
        0,
        "o valor em claro chegou à base"
    );
}

#[tokio::test]
async fn um_fornecedor_externo_nunca_e_chamado_numa_instancia_que_nao_os_admite() {
    let pool = pool!();
    let org = organisation(&pool).await;
    let state = nucleo(pool.clone(), org);
    assert!(
        !state.config.ai.allow_external_providers,
        "a prova pressupõe o valor por omissão"
    );
    let (_, admin) = membro(&pool, org, TechnicalRole::PlatformAdmin).await;
    let (_, pessoa) = membro(&pool, org, TechnicalRole::ResearchMember).await;

    // Um fornecedor externo só por https.
    let (status, _) = pedido(
        &state,
        Some(&admin),
        None,
        "POST",
        "/api/v1/ai/providers",
        Some(
            json!({ "kind": "openai", "label": "Nuvem", "residency": "external",
                     "endpoint_url": "http://api.exemplo.invalid/v1" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let (status, fornecedor) = pedido(
        &state,
        Some(&admin),
        None,
        "POST",
        "/api/v1/ai/providers",
        Some(
            json!({ "kind": "anthropic", "label": "Nuvem", "residency": "external",
                     "endpoint_url": "https://127.0.0.1:9" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{fornecedor}");
    let provider_id = fornecedor["id"].as_str().expect("id").to_owned();
    let (status, _) = pedido(
        &state,
        Some(&admin),
        None,
        "POST",
        &format!("/api/v1/ai/providers/{provider_id}/models"),
        Some(json!({ "model_name": "modelo-nuvem", "capabilities": ["CODING"] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let resposta = perguntar(&state, &pessoa, "CODING").await;
    assert_eq!(origem(&resposta), "SYSTEM", "{resposta}");
    // Nem uma tentativa: a saúde de um fornecedor chamado deixaria de ser `unknown`.
    let (_, lista) = pedido(
        &state,
        Some(&admin),
        None,
        "GET",
        "/api/v1/ai/providers",
        None,
    )
    .await;
    assert_eq!(lista[0]["health"], "unknown", "{lista}");

    // O modelo externo entra com o tecto PUBLIC por omissão.
    let tecto: String =
        sqlx::query_scalar("SELECT max_classification FROM ai_models WHERE provider_id = $1::uuid")
            .bind(&provider_id)
            .fetch_one(&pool)
            .await
            .expect("modelo");
    assert_eq!(tecto, "PUBLIC");
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
