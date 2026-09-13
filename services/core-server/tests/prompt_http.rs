//! O que `POST /api/v1/ai/prompt` responde com zero inferência.
//!
//! # O contrato de M5.1
//!
//! O Prompt Ocinye é uma superfície de comando, não um widget de modelo. Com
//! zero fornecedores, zero modelos e zero nós — o estado verdadeiro desta
//! instalação — um pedido autorizado **não falha**: é recebido, processado, e
//! concluído deterministicamente como `DEGRADED`, com a origem `SYSTEM` e um
//! código-máquina de razão. Nada está avariado; simplesmente não há inferência,
//! e nenhum fornecedor externo é usado em substituição.
//!
//! Este é o teste de aceitação principal de M5.1: prova que a resposta chega
//! com HTTP 200 e o envelope tipado — nunca um 503 opaco — e que a procura fica
//! registada no ledger sem consumir tokens, reservar GPU ou tocar na rede.

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
        .bind(format!("pmt-{}", Uuid::new_v4().simple()))
        .bind("Instituição do Prompt")
        .fetch_one(pool)
        .await
        .expect("organização")
}

/// Uma pessoa que pode usar IA (`ResearchMember` concede `AiUse`), e a sua
/// sessão activa.
async fn membro(pool: &PgPool, organisation_id: Uuid) -> (Uuid, Secret) {
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
        .bind(TechnicalRole::ResearchMember.as_str())
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
             VALUES ($1, $2, $3, now() + interval '1 hour', 'prompt-http-test')",
    )
    .bind(person_id)
    .bind(identity::session_digest(&token))
    .bind(SessionState::Active.as_str())
    .execute(pool)
    .await
    .expect("sessão");

    (person_id, token)
}

async fn submit(state: &AppState, token: &Secret, body: Value) -> (StatusCode, Value) {
    let response = routes::router(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/ai/prompt")
                .header(header::AUTHORIZATION, format!("Bearer {}", token.expose()))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&body).expect("json")))
                .expect("pedido"),
        )
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

/// Regista um modelo de nó que serve as capacidades dadas, `available`.
///
/// Um modelo `ocinye_node` precisa de um nó; o estado do nó é irrelevante para o
/// router, que decide por `serves()` do modelo.
async fn seed_serving_model(pool: &PgPool, organisation_id: Uuid, capabilities: &[&str]) {
    let identifier: String = Uuid::new_v4()
        .simple()
        .to_string()
        .chars()
        .take(24)
        .collect();
    let node_id: Uuid = sqlx::query_scalar(
        "INSERT INTO compute_nodes (organisation_id, identifier, display_name, status)
             VALUES ($1, $2, 'Nó de teste', 'online') RETURNING id",
    )
    .bind(organisation_id)
    .bind(identifier)
    .fetch_one(pool)
    .await
    .expect("nó");

    let caps = serde_json::Value::Array(
        capabilities
            .iter()
            .map(|c| serde_json::Value::String((*c).to_owned()))
            .collect(),
    );
    sqlx::query(
        "INSERT INTO ai_models
             (provider_kind, provider_name, node_id, model_name, version,
              capabilities, status, enabled)
         VALUES ('ocinye_node', $1, $2, $3, 'v1', $4, 'available', TRUE)",
    )
    .bind(format!("prov-{}", Uuid::new_v4().simple()))
    .bind(node_id)
    .bind(format!("modelo-{}", Uuid::new_v4().simple()))
    .bind(caps)
    .execute(pool)
    .await
    .expect("modelo");
}

/// O input mantém-se operacional: um segundo pedido conclui do mesmo modo.
///
/// Prova que a primeira conclusão degradada não deixou a superfície num estado
/// que recuse o próximo pedido — o Prompt não «gasta» a sua disponibilidade.
#[tokio::test]
async fn o_prompt_continua_operacional_apos_uma_resposta_degradada() {
    let pool = pool!();
    let organisation_id = organisation(&pool).await;
    let state = nucleo(pool.clone(), organisation_id);
    let (_person_id, token) = membro(&pool, organisation_id).await;

    for pedido in ["Primeiro pedido.", "Segundo pedido."] {
        let (status, corpo) = submit(&state, &token, json!({ "prompt": pedido })).await;
        assert_eq!(status, StatusCode::OK, "{corpo}");
        assert_eq!(corpo["status"], "DEGRADED", "{corpo}");
        assert_eq!(corpo["origin"], "SYSTEM", "{corpo}");
    }
}

/// Um pedido vazio é uma falha real — 422 —, e não uma conclusão degradada.
///
/// Reserva os erros para falhas verdadeiras: o `DEGRADED` é para um pedido que
/// foi processado, e um pedido sem texto não o foi.
#[tokio::test]
async fn um_pedido_vazio_e_uma_falha_de_validacao() {
    let pool = pool!();
    let organisation_id = organisation(&pool).await;
    let state = nucleo(pool.clone(), organisation_id);
    let (_person_id, token) = membro(&pool, organisation_id).await;

    let (status, _corpo) = submit(&state, &token, json!({ "prompt": "   " })).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

/// Limpa o inventário global de modelos e nós.
///
/// `ai_models` não tem âmbito de organização — `list_models` devolve todos —,
/// por isso os cenários de router têm de partir de um inventário conhecido. Este
/// teste corre-os em sequência, num só processo, para não competir com o
/// inventário de testes paralelos.
async fn clear_inventory(pool: &PgPool) {
    sqlx::query("DELETE FROM ai_models")
        .execute(pool)
        .await
        .expect("limpar modelos");
    sqlx::query("DELETE FROM compute_nodes")
        .execute(pool)
        .await
        .expect("limpar nós");
}

/// O router classifica o pedido pelo estado do inventário, e a execução roteia
/// quando um fornecedor serve (M5.2, resultado-chave §20; base do hot-plug §21).
///
/// Sequencial e isolado de propósito: `ai_models` é global.
#[tokio::test]
async fn o_router_classifica_o_inventario_e_a_execucao_roteia() {
    let pool = pool!();
    let organisation_id = organisation(&pool).await;
    let (person_id, token) = membro(&pool, organisation_id).await;

    // ── Cenário 0 (aceitação principal, M5.1 §16): inventário vazio, zero
    // fornecedores. Um pedido de código é processado e conclui como resposta de
    // sistema degradada, sem provenance de modelo, registada no ledger.
    clear_inventory(&pool).await;
    let state = nucleo(pool.clone(), organisation_id); // NoProvider
    let (status, corpo) = submit(
        &state,
        &token,
        json!({
            "prompt": "Cria uma função Rust que some dois números.",
            "capability": "CODING",
        }),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "um pedido processado não é falha: {corpo}"
    );
    assert_eq!(corpo["origin"], "SYSTEM", "{corpo}");
    assert_eq!(corpo["status"], "DEGRADED", "{corpo}");
    assert_eq!(corpo["reason_code"], "AI_NO_PROVIDER_AVAILABLE", "{corpo}");
    // Proveniência presente e nula — prova-se a ausência de modelo.
    for campo in ["model", "provider", "compute_node"] {
        assert!(
            corpo.get(campo).is_some() && corpo[campo].is_null(),
            "{campo}: {corpo}"
        );
    }
    assert!(
        corpo["content"]
            .as_str()
            .unwrap_or_default()
            .contains("Prompt Ocinye continua operacional"),
        "{corpo}"
    );
    let (rejeitados, razao): (i64, Option<String>) = sqlx::query_as(
        "SELECT count(*), max(rejection_reason) FROM ai_jobs
             WHERE requested_by_id = $1 AND status = 'rejected' AND capability = 'CODING'",
    )
    .bind(person_id)
    .fetch_one(&pool)
    .await
    .expect("ledger");
    assert_eq!(rejeitados, 1, "a procura não ficou registada uma vez");
    assert_eq!(
        razao.as_deref(),
        Some("AI_NO_PROVIDER_AVAILABLE"),
        "{corpo}"
    );

    // ── Cenário 1: modelos existem, nenhum serve CÓDIGO → NO_COMPATIBLE_MODEL.
    clear_inventory(&pool).await;
    seed_serving_model(&pool, organisation_id, &["GENERAL"]).await;
    let state = nucleo(pool.clone(), organisation_id); // NoProvider
    let (status, corpo) = submit(
        &state,
        &token,
        json!({ "prompt": "Escreve código.", "capability": "CODING" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{corpo}");
    assert_eq!(corpo["origin"], "SYSTEM", "{corpo}");
    assert_eq!(
        corpo["reason_code"], "AI_NO_COMPATIBLE_MODEL",
        "modelos existem mas nenhum serve CÓDIGO: {corpo}"
    );

    // ── Cenário 2: um modelo serve CÓDIGO, mas o fornecedor não o executa.
    clear_inventory(&pool).await;
    seed_serving_model(&pool, organisation_id, &["CODING"]).await;
    let state = nucleo(pool.clone(), organisation_id); // NoProvider
    let (status, corpo) = submit(
        &state,
        &token,
        json!({ "prompt": "Escreve código.", "capability": "CODING" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{corpo}");
    assert_eq!(corpo["origin"], "SYSTEM", "{corpo}");
    assert_eq!(
        corpo["reason_code"], "AI_NO_PROVIDER_AVAILABLE",
        "modelo resolvido mas sem fornecedor: {corpo}"
    );
    assert!(corpo["model"].is_null(), "{corpo}");

    // ── Cenário 3: modelo E fornecedor que serve → COMPLETED (origin=MODEL).
    // O mesmo inventário do cenário 2; só muda o fornecedor injectado — a prova
    // de que um fornecedor a aparecer roteia sem redeploy nem toggle (§21).
    let state = nucleo_com(
        pool.clone(),
        organisation_id,
        Arc::new(ocinye_core::modules::intelligence::fixture::FixtureProvider::cooperative()),
    );
    let (status, corpo) = submit(
        &state,
        &token,
        json!({ "prompt": "Soma dois números.", "capability": "CODING" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{corpo}");
    assert_eq!(corpo["origin"], "MODEL", "{corpo}");
    assert_eq!(corpo["status"], "COMPLETED", "{corpo}");
    assert!(corpo["reason_code"].is_null(), "{corpo}");
    assert!(corpo["model"].is_string(), "{corpo}");
    assert!(corpo["provider"].is_string(), "{corpo}");
    assert!(
        corpo["content"]
            .as_str()
            .unwrap_or_default()
            .contains("Resposta de teste"),
        "{corpo}"
    );

    // ── Cenário 4: o fornecedor desaparece de novo → SYSTEM/DEGRADED.
    let state = nucleo(pool.clone(), organisation_id); // volta a NoProvider
    let (status, corpo) = submit(
        &state,
        &token,
        json!({ "prompt": "Soma dois números.", "capability": "CODING" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{corpo}");
    assert_eq!(corpo["origin"], "SYSTEM", "{corpo}");
    assert_eq!(corpo["reason_code"], "AI_NO_PROVIDER_AVAILABLE", "{corpo}");

    // ── Hot-plug SEM reinício (M5.3 §21) ────────────────────────────────
    //
    // A prova fiel: uma **única** `AppState`, com o adaptador de inferência
    // fixo — como o adaptador de rede de um nó, que sabe falar com nós e não
    // muda. O que muda dinamicamente é o inventário `ai_models`, reportado pelo
    // nó: ligá-lo, e desligá-lo. O router lê-o a cada pedido, por isso o
    // roteamento segue o estado sem redeploy do Workspace, sem reinício do Core,
    // e sem nenhum interruptor manual `AI_ENABLED`.
    let nucleo_fixo = nucleo_com(
        pool.clone(),
        organisation_id,
        Arc::new(ocinye_core::modules::intelligence::fixture::FixtureProvider::cooperative()),
    );

    // 5. Nó ausente (inventário vazio) → SYSTEM/DEGRADED, mesmo com adaptador.
    clear_inventory(&pool).await;
    let (status, corpo) = submit(
        &nucleo_fixo,
        &token,
        json!({ "prompt": "Soma dois números.", "capability": "CODING" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{corpo}");
    assert_eq!(
        corpo["origin"], "SYSTEM",
        "hot-plug: sem nó, é do sistema: {corpo}"
    );
    assert_eq!(corpo["reason_code"], "AI_NO_PROVIDER_AVAILABLE", "{corpo}");

    // 6. O nó liga-se (reporta um modelo) → o MESMO núcleo passa a COMPLETED.
    seed_serving_model(&pool, organisation_id, &["CODING"]).await;
    let (status, corpo) = submit(
        &nucleo_fixo,
        &token,
        json!({ "prompt": "Soma dois números.", "capability": "CODING" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{corpo}");
    assert_eq!(
        corpo["origin"], "MODEL",
        "hot-plug: nó ligado, o pedido devia rotear sem reinício: {corpo}"
    );
    assert_eq!(corpo["status"], "COMPLETED", "{corpo}");

    // 7. O nó desliga-se (inventário limpo) → o MESMO núcleo volta a DEGRADED.
    clear_inventory(&pool).await;
    let (status, corpo) = submit(
        &nucleo_fixo,
        &token,
        json!({ "prompt": "Soma dois números.", "capability": "CODING" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{corpo}");
    assert_eq!(
        corpo["origin"], "SYSTEM",
        "hot-plug: nó desligado, volta a ser do sistema: {corpo}"
    );
    assert_eq!(corpo["reason_code"], "AI_NO_PROVIDER_AVAILABLE", "{corpo}");

    // O ledger regista dois trabalhos concluídos — o cenário 3 e o passo 6 do
    // hot-plug. O `model_id` de ambos ficou nulo quando o respectivo modelo foi
    // removido (FK `ON DELETE SET NULL`): o trabalho sobrevive ao seu modelo, o
    // que é o comportamento correcto. Nunca se guarda o prompt nem a resposta.
    let (concluidos,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM ai_jobs
             WHERE requested_by_id = $1 AND status = 'succeeded'",
    )
    .bind(person_id)
    .fetch_one(&pool)
    .await
    .expect("ledger");
    assert_eq!(concluidos, 2, "o cenário 3 e o hot-plug passo 6 concluíram");
}

/// A sonda do harness: aceita, porque não há servidor de correio para
/// perguntar.
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
