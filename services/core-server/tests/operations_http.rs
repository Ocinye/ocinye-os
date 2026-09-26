//! A vista do operador (Parte 13 da generalização): cada avaria aparece como
//! é, e nenhuma derruba o que não depende dela.
//!
//! Cinco avarias ao mesmo tempo — um membro acima da quota, um nó calado há um
//! dia, um fornecedor de IA que não responde, uma aplicação desactivada — e o
//! operador tem de as ver todas em `GET /system/operations`, em contagens e
//! estados, sem nomes nem conteúdo; enquanto o Core, a persistência, a
//! identidade e a sessão do membro continuam a servir.

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
        .bind(format!("ops-{}", Uuid::new_v4().simple()))
        .bind("Instituição do operador")
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
             VALUES ($1, $2, $3, now() + interval '1 hour', 'operations-http-test', true)",
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

#[tokio::test]
async fn o_operador_ve_cada_avaria_e_nada_do_resto_cai() {
    let pool = pool!();
    let org = organisation(&pool).await;
    let state = nucleo(pool.clone(), org);
    let (_, admin) = membro(&pool, org, TechnicalRole::PlatformAdmin).await;
    let (pessoa_id, pessoa) = membro(&pool, org, TechnicalRole::ResearchMember).await;

    // O perfil de recursos por omissão, como o `bootstrap-admin` o cria numa
    // Instância real: sem ele, o limite não resolve e lê-se como «sem limite».
    ocinye_core::modules::resource::ensure_default_profile(&pool, org)
        .await
        .expect("perfil por omissão");

    // ── Um membro acima da quota: 11 GiB contra os 10 GiB por omissão ─────
    sqlx::query(
        "INSERT INTO storage_backends
             (code, kind, display_name, location_label, bucket, is_default, is_active)
         VALUES ('ocinye-test-default', 's3_compatible', 'Test', 'test', 'prova', TRUE, TRUE)
         ON CONFLICT DO NOTHING",
    )
    .execute(&pool)
    .await
    .expect("backend");
    sqlx::query(
        "INSERT INTO storage_objects
             (id, backend_id, organisation_id, owner_id, object_key,
              original_filename, content_type, size_bytes, checksum_sha256,
              classification, status, created_by_id)
         SELECT $1, b.id, $2, $3, $4, 'grande.bin', 'application/octet-stream', $5,
                repeat('0', 64), 'INTERNAL', 'stored', $3
           FROM storage_backends b WHERE b.is_default AND b.is_active",
    )
    .bind(Uuid::new_v4())
    .bind(org)
    .bind(pessoa_id)
    .bind(format!("prova/{}", Uuid::new_v4()))
    .bind(11_i64 * 1024 * 1024 * 1024)
    .execute(&pool)
    .await
    .expect("objecto");

    // ── Um nó que deixou de responder há um dia ──────────────────────────
    sqlx::query(
        "INSERT INTO compute_nodes (organisation_id, identifier, display_name, status, last_seen_at)
         VALUES ($1, 'CAM-TESTE', 'Nó calado', 'online', now() - interval '1 day')",
    )
    .bind(org)
    .execute(&pool)
    .await
    .expect("nó");

    // ── Um fornecedor que não responde ───────────────────────────────────
    let (status, fornecedor) = pedido(
        &state,
        Some(&admin),
        None,
        "POST",
        "/api/v1/ai/providers",
        Some(
            json!({ "kind": "openai_compatible", "label": "Calado", "residency": "local",
                     "endpoint_url": "http://127.0.0.1:9/v1" }),
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
        Some(json!({ "model_name": "calado", "capabilities": ["GENERAL"] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, resposta) = pedido(
        &state,
        Some(&pessoa),
        None,
        "POST",
        "/api/v1/ai/prompt",
        Some(json!({ "prompt": "Olá" })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "o Prompt caiu com o fornecedor: {resposta}"
    );
    assert_eq!(
        resposta["reason_code"], "AI_PROVIDER_UNHEALTHY",
        "{resposta}"
    );

    // ── Uma aplicação desactivada ────────────────────────────────────────
    let (status, _) = pedido(
        &state,
        Some(&admin),
        None,
        "PUT",
        "/api/v1/instance/applications/notes",
        Some(json!({ "active": false })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // ── O operador vê cada uma, e só o que lhe cabe ──────────────────────
    let (status, vista) = pedido(
        &state,
        Some(&admin),
        None,
        "GET",
        "/api/v1/system/operations",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{vista}");
    let no = vista["nodes"]
        .as_array()
        .expect("nós")
        .iter()
        .find(|n| n["identifier"] == "CAM-TESTE")
        .expect("o nó");
    assert_eq!(no["status"], "offline", "{no}");
    let prov = vista["ai_providers"]
        .as_array()
        .expect("fornecedores")
        .iter()
        .find(|p| p["id"] == provider_id.as_str())
        .expect("o fornecedor");
    assert_eq!(prov["health"], "unreachable", "{prov}");
    let notas = vista["applications"]
        .as_array()
        .expect("aplicações")
        .iter()
        .find(|a| a["id"] == "notes")
        .expect("notas");
    assert_eq!(notas["active"], false);
    let ficheiros = vista["applications"]
        .as_array()
        .expect("aplicações")
        .iter()
        .find(|a| a["id"] == "files")
        .expect("ficheiros");
    assert_eq!(
        ficheiros["active"], true,
        "desactivar Notas não pode tocar em Ficheiros"
    );
    assert_eq!(vista["storage"]["over_quota"], 1, "{}", vista["storage"]);
    // Contagens, nunca nomes nem conteúdo.
    let texto = vista.to_string();
    for proibido in ["grande.bin", "prova/", "CAM-TESTE-nome-inexistente"] {
        assert!(
            !texto.contains(proibido),
            "a vista do operador expôs «{proibido}»"
        );
    }

    // ── E nada do resto caiu ─────────────────────────────────────────────
    let (status, pronto) = pedido(&state, None, None, "GET", "/ready", None).await;
    assert!(status.is_success(), "{pronto}");
    let componente = |nome: &str| {
        pronto["components"]
            .as_array()
            .expect("componentes")
            .iter()
            .find(|c| c["component"] == nome)
            .expect("componente")["state"]
            .clone()
    };
    for critico in ["core", "persistence", "identity"] {
        assert_eq!(
            componente(critico),
            "available",
            "{critico} caiu com as avarias opcionais"
        );
    }
    let (status, _) = pedido(&state, Some(&pessoa), None, "GET", "/api/v1/me", None).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "o membro deixou de ter sessão utilizável"
    );
    let (status, recursos) = pedido(
        &state,
        Some(&pessoa),
        None,
        "GET",
        "/api/v1/resources/me",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        recursos.to_string().contains("over_quota"),
        "o membro não vê que está acima da quota: {recursos}"
    );

    // ── A vista é do operador ────────────────────────────────────────────
    let (status, _) = pedido(
        &state,
        Some(&pessoa),
        None,
        "GET",
        "/api/v1/system/operations",
        None,
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
