//! Nós e capacidade, pelo HTTP (Parte 5 da generalização, ADR-0013).
//!
//! Uma Instância não é o servidor onde corre: os nós contribuem-lhe recursos, e
//! entram e saem sem que o Core deixe de responder. As oito etapas A–H do
//! programa, de ponta a ponta: registo, heartbeat, descoberta de recursos, nó
//! que cai, Core saudável, nó que volta, capacidade que regressa sozinha, e nó
//! falso recusado — mais a capacidade física, reservada, alocável, alocada e
//! consumida.

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
        .bind(format!("nod-{}", Uuid::new_v4().simple()))
        .bind("Instituição dos nós")
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
             VALUES ($1, $2, $3, now() + interval '1 hour', 'node-fabric-http-test', true)",
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

fn batimento(modelo: &str) -> Value {
    json!({
        "agent_version": "0.0.0-teste",
        "resources": {
            "cpu_cores": 16,
            "memory_bytes": 64_u64 * 1024 * 1024 * 1024,
            "storage_bytes": 0,
            "gpus": [{ "model": "GPU de teste", "memory_bytes": 48_u64 * 1024 * 1024 * 1024 }],
            "memory_used_bytes": 8_u64 * 1024 * 1024 * 1024
        },
        "capabilities": ["GENERAL"],
        "models": [{ "name": modelo, "version": "1", "capabilities": ["GENERAL"], "context_limit": 8192 }],
        "health": {}
    })
}

async fn capacidade_geral(state: &AppState, admin: &Secret) -> bool {
    let (status, corpo) = pedido(state, Some(admin), None, "GET", "/api/v1/ai/status", None).await;
    assert_eq!(status, StatusCode::OK);
    corpo["capabilities"]
        .as_array()
        .and_then(|c| c.iter().find(|c| c["capability"] == "GENERAL"))
        .and_then(|c| c["available"].as_bool())
        .unwrap_or(false)
}

#[tokio::test]
async fn um_no_entra_reporta_cai_e_volta_sem_o_core_deixar_de_responder() {
    let pool = pool!();
    let org = organisation(&pool).await;
    let state = nucleo(pool.clone(), org);
    let (_, admin) = membro(&pool, org, TechnicalRole::PlatformAdmin).await;
    let identificador = format!("N{}", &Uuid::new_v4().simple().to_string()[..10]).to_uppercase();

    // A — registo: o administrador regista o nó e recebe um token de enrolamento.
    let (status, registo) = pedido(
        &state,
        Some(&admin),
        None,
        "POST",
        "/api/v1/compute/nodes",
        Some(json!({ "identifier": identificador, "display_name": "Nó de teste", "kind": "gpu" })),
    )
    .await;
    assert!(status.is_success(), "registo: {status} {registo}");
    let enrolamento = registo["enrollment_token"]
        .as_str()
        .expect("token")
        .to_owned();
    let node_id = registo["node_id"].as_str().expect("id").to_owned();

    let (status, enrolado) = pedido(
        &state,
        None,
        None,
        "POST",
        "/api/v1/compute/enroll",
        Some(json!({ "enrollment_token": enrolamento })),
    )
    .await;
    assert!(status.is_success(), "enrolamento: {status}");
    let agente = enrolado["agent_token"].as_str().expect("agente").to_owned();

    // H (parte) — o token de enrolamento é de uso único.
    let (reuso, _) = pedido(
        &state,
        None,
        None,
        "POST",
        "/api/v1/compute/enroll",
        Some(json!({ "enrollment_token": enrolamento })),
    )
    .await;
    assert_eq!(
        reuso,
        StatusCode::UNAUTHORIZED,
        "um token de enrolamento não se reutiliza"
    );

    // B, C — heartbeat e descoberta: recursos, GPU, consumo.
    assert!(
        !capacidade_geral(&state, &admin).await,
        "antes do nó, nada serve GENERAL"
    );
    let modelo = format!("modelo-{identificador}");
    let (status, _) = pedido(
        &state,
        None,
        Some(&agente),
        "POST",
        "/api/v1/compute/heartbeat",
        Some(batimento(&modelo)),
    )
    .await;
    assert!(status.is_success(), "heartbeat: {status}");
    let (_, nos) = pedido(
        &state,
        Some(&admin),
        None,
        "GET",
        "/api/v1/compute/nodes",
        None,
    )
    .await;
    let no = nos
        .as_array()
        .and_then(|n| n.iter().find(|n| n["id"] == node_id.as_str()))
        .expect("o nó aparece");
    assert_eq!(no["status"], "online");
    assert_eq!(no["capacity"]["cpu_cores"]["physical"], 16);
    assert_eq!(no["capacity"]["gpus"], 1);
    assert_eq!(
        no["capacity"]["memory_bytes"]["consumed"],
        8_i64 * 1024 * 1024 * 1024
    );
    assert_eq!(no["capacity"]["cpu_cores"]["allocated"], 0);
    assert!(
        capacidade_geral(&state, &admin).await,
        "G: o modelo do nó serve GENERAL"
    );

    // A reserva do operador tira capacidade alocável, e nunca a física.
    let (status, _) = pedido(
        &state,
        Some(&admin),
        None,
        "PUT",
        &format!("/api/v1/compute/nodes/{node_id}/reservation"),
        Some(json!({ "cpu_cores": 2, "memory_bytes": 4_i64 * 1024 * 1024 * 1024 })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, total) = pedido(
        &state,
        Some(&admin),
        None,
        "GET",
        "/api/v1/compute/capacity",
        None,
    )
    .await;
    assert_eq!(total["online_nodes"], 1);
    assert_eq!(total["cpu_cores"]["physical"], 16);
    assert_eq!(total["cpu_cores"]["reserved"], 2);
    assert_eq!(total["cpu_cores"]["allocatable"], 14);

    // D, E — o nó cala-se: fica offline, a capacidade sai, e o Core continua.
    sqlx::query(
        "UPDATE compute_nodes SET last_seen_at = now() - interval '1 day' WHERE id = $1::uuid",
    )
    .bind(&node_id)
    .execute(&pool)
    .await
    .expect("silenciar o nó");
    ocinye_core::modules::compute::internal::mark_stale_models_unavailable(&pool, 120)
        .await
        .expect("o worker marca os modelos");
    let (_, nos) = pedido(
        &state,
        Some(&admin),
        None,
        "GET",
        "/api/v1/compute/nodes",
        None,
    )
    .await;
    assert_eq!(
        nos.as_array()
            .and_then(|n| n.iter().find(|n| n["id"] == node_id.as_str()))
            .expect("nó")["status"],
        "offline"
    );
    assert!(
        !capacidade_geral(&state, &admin).await,
        "D: offline, o modelo deixa de servir"
    );
    let (pronto, _) = pedido(&state, None, None, "GET", "/ready", None).await;
    assert_eq!(pronto, StatusCode::OK, "E: o Core continua pronto");
    let (me, _) = pedido(&state, Some(&admin), None, "GET", "/api/v1/me", None).await;
    assert_eq!(me, StatusCode::OK);
    let (_, total) = pedido(
        &state,
        Some(&admin),
        None,
        "GET",
        "/api/v1/compute/capacity",
        None,
    )
    .await;
    assert_eq!(total["online_nodes"], 0);

    // F, G — o nó volta, e a capacidade regressa sozinha: sem reinício, sem toggle.
    let (status, _) = pedido(
        &state,
        None,
        Some(&agente),
        "POST",
        "/api/v1/compute/heartbeat",
        Some(batimento(&modelo)),
    )
    .await;
    assert!(status.is_success());
    assert!(
        capacidade_geral(&state, &admin).await,
        "G: a capacidade volta com o nó"
    );

    // H — um nó falso é recusado.
    let (falso, _) = pedido(
        &state,
        None,
        Some("token-que-nao-existe"),
        "POST",
        "/api/v1/compute/heartbeat",
        Some(batimento("modelo-falso")),
    )
    .await;
    assert_eq!(
        falso,
        StatusCode::UNAUTHORIZED,
        "um token que não existe não é um nó"
    );
    let (sem, _) = pedido(
        &state,
        None,
        None,
        "POST",
        "/api/v1/compute/heartbeat",
        Some(batimento("x")),
    )
    .await;
    assert_eq!(sem, StatusCode::UNAUTHORIZED, "sem token não há nó");
    let (inventado, _) = pedido(
        &state,
        None,
        None,
        "POST",
        "/api/v1/compute/enroll",
        Some(json!({ "enrollment_token": "inventado" })),
    )
    .await;
    assert_eq!(inventado, StatusCode::UNAUTHORIZED);
}

/// Só quem administra a plataforma reserva capacidade, e uma reserva negativa
/// é recusada.
#[tokio::test]
async fn a_reserva_e_de_quem_administra_e_nunca_negativa() {
    let pool = pool!();
    let org = organisation(&pool).await;
    let state = nucleo(pool.clone(), org);
    let (_, admin) = membro(&pool, org, TechnicalRole::PlatformAdmin).await;
    let (_, pessoa) = membro(&pool, org, TechnicalRole::ResearchMember).await;
    let identificador = format!("R{}", &Uuid::new_v4().simple().to_string()[..10]).to_uppercase();
    let (_, registo) = pedido(
        &state,
        Some(&admin),
        None,
        "POST",
        "/api/v1/compute/nodes",
        Some(json!({ "identifier": identificador, "display_name": "Nó" })),
    )
    .await;
    let caminho = format!(
        "/api/v1/compute/nodes/{}/reservation",
        registo["node_id"].as_str().expect("id")
    );
    let (membro_status, _) = pedido(
        &state,
        Some(&pessoa),
        None,
        "PUT",
        &caminho,
        Some(json!({ "cpu_cores": 1 })),
    )
    .await;
    assert!(
        membro_status == StatusCode::FORBIDDEN || membro_status == StatusCode::NOT_FOUND,
        "um membro não reserva: {membro_status}"
    );
    let (negativa, _) = pedido(
        &state,
        Some(&admin),
        None,
        "PUT",
        &caminho,
        Some(json!({ "cpu_cores": -1 })),
    )
    .await;
    assert_eq!(negativa, StatusCode::UNPROCESSABLE_ENTITY);
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
