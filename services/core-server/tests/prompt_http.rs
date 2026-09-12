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
fn state(pool: PgPool, organisation_id: Uuid) -> AppState {
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
        inference: Arc::new(ocinye_core::modules::intelligence::NoProvider),
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

/// Teste de aceitação principal de M5.1.
///
/// Zero fornecedores, zero modelos, zero nós, Core saudável. Um pedido de código
/// é aceite, processado, e concluído como resposta de sistema degradada — com o
/// código-máquina certo e sem provenance de modelo nenhum.
#[tokio::test]
async fn sem_no_um_pedido_conclui_como_resposta_de_sistema() {
    let pool = pool!();
    let organisation_id = organisation(&pool).await;
    let state = state(pool.clone(), organisation_id);
    let (person_id, token) = membro(&pool, organisation_id).await;

    let (status, corpo) = submit(
        &state,
        &token,
        json!({
            "prompt": "Cria uma função Rust que some dois números.",
            "capability": "CODING",
        }),
    )
    .await;

    // Um pedido processado não é uma falha: HTTP 200, nunca 503.
    assert_eq!(
        status,
        StatusCode::OK,
        "um pedido processado sem inferência devolveu erro em vez de estado: {corpo}"
    );

    // A origem é o SISTEMA — nunca um modelo.
    assert_eq!(corpo["origin"], "SYSTEM", "{corpo}");
    assert_eq!(corpo["status"], "DEGRADED", "{corpo}");
    assert_eq!(corpo["reason_code"], "AI_NO_PROVIDER_AVAILABLE", "{corpo}");

    // Nenhuma proveniência de modelo: os três campos estão presentes e nulos,
    // para que se prove a ausência em vez de a inferir de uma omissão.
    assert!(
        corpo.get("model").is_some() && corpo["model"].is_null(),
        "{corpo}"
    );
    assert!(
        corpo.get("provider").is_some() && corpo["provider"].is_null(),
        "{corpo}"
    );
    assert!(
        corpo.get("compute_node").is_some() && corpo["compute_node"].is_null(),
        "{corpo}"
    );

    // A resposta é do sistema, em português, e afirma-o.
    let content = corpo["content"].as_str().unwrap_or_default();
    assert!(
        content.contains("Prompt Ocinye continua operacional"),
        "a resposta de sistema não afirma que o Prompt está operacional: «{content}»"
    );

    // A procura ficou registada no ledger, como recusa, com o código-máquina —
    // a evidência de demanda que justifica um nó. Nenhum token, nenhuma GPU.
    let (registados, razao): (i64, Option<String>) = sqlx::query_as(
        "SELECT count(*), max(rejection_reason)
             FROM ai_jobs
             WHERE requested_by_id = $1 AND status = 'rejected' AND capability = 'CODING'",
    )
    .bind(person_id)
    .fetch_one(&pool)
    .await
    .expect("ledger");
    assert_eq!(registados, 1, "a procura não ficou registada uma vez");
    assert_eq!(
        razao.as_deref(),
        Some("AI_NO_PROVIDER_AVAILABLE"),
        "o ledger não guardou o código-máquina da razão"
    );
}

/// O input mantém-se operacional: um segundo pedido conclui do mesmo modo.
///
/// Prova que a primeira conclusão degradada não deixou a superfície num estado
/// que recuse o próximo pedido — o Prompt não «gasta» a sua disponibilidade.
#[tokio::test]
async fn o_prompt_continua_operacional_apos_uma_resposta_degradada() {
    let pool = pool!();
    let organisation_id = organisation(&pool).await;
    let state = state(pool.clone(), organisation_id);
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
    let state = state(pool.clone(), organisation_id);
    let (_person_id, token) = membro(&pool, organisation_id).await;

    let (status, _corpo) = submit(&state, &token, json!({ "prompt": "   " })).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
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
