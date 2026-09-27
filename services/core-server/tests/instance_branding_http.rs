//! A mesma release, duas Instâncias (Parte 14 da generalização, ADR-0017).
//!
//! Cada uma na sua base nova, como duas instalações. Nome, perfil, aplicações,
//! língua, fuso, fixações por omissão e logótipo diferentes — e a segurança
//! exactamente igual: sem sessão, recusa; um membro não muda a Instância; um fuso
//! inventado e um SVG são recusados nas duas.

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
             VALUES ($1, $2, $3, now() + interval '1 hour', 'branding-http-test', true)",
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

/// Uma base nova, só desta Instância — como uma instalação nova teria.
async fn base_nova(url: &str) -> (PgPool, String) {
    let admin = PgPool::connect(url).await.expect("base de teste");
    let nome = format!("ocinye_marca_{}", Uuid::new_v4().simple());
    sqlx::query(&format!("CREATE DATABASE {nome}"))
        .execute(&admin)
        .await
        .expect("criar a base");
    admin.close().await;
    let raiz = url.rsplit_once('/').map_or(url, |(raiz, _)| raiz);
    let pool = PgPool::connect(&format!("{raiz}/{nome}"))
        .await
        .expect("base nova");
    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .expect("migrations");
    (pool, nome)
}

async fn apagar_base(url: &str, nome: &str) {
    if let Ok(admin) = PgPool::connect(url).await {
        let _ = sqlx::query(&format!("DROP DATABASE IF EXISTS {nome} WITH (FORCE)"))
            .execute(&admin)
            .await;
    }
}

fn armazenamento() -> Option<Arc<ocinye_core::storage::ObjectStore>> {
    ocinye_core::storage::ObjectStore::new(ocinye_core::config::StorageConfig {
        endpoint_url: std::env::var("OCINYE_TEST_STORAGE_ENDPOINT").ok()?,
        region: "us-east-1".to_owned(),
        access_key: std::env::var("OCINYE_TEST_STORAGE_ACCESS_KEY").ok()?,
        secret_key: std::env::var("OCINYE_TEST_STORAGE_SECRET_KEY").ok()?,
        bucket: std::env::var("OCINYE_TEST_STORAGE_BUCKET")
            .unwrap_or_else(|_| "ocinye-test-artifacts".to_owned()),
        backend_code: "ocinye-test-default".to_owned(),
        location_label: "test".to_owned(),
        residency: ocinye_contracts::storage::Residency::Undeclared,
        max_upload_bytes: 512 * 1024 * 1024,
    })
    .map(Arc::new)
}

/// Um PNG de 1×1, feito aqui: o menor logótipo que é uma imagem de verdade.
const PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xf8, 0xcf, 0xc0, 0xf0,
    0x1f, 0x00, 0x05, 0x00, 0x01, 0xff, 0x89, 0x99, 0x3d, 0x1d, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
    0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

async fn pedido_bytes(state: &AppState, token: &Secret, path: &str, corpo: &[u8]) -> StatusCode {
    let request = Request::builder()
        .method("PUT")
        .uri(path)
        .header(header::AUTHORIZATION, format!("Bearer {}", token.expose()))
        .header(header::CONTENT_TYPE, "image/png")
        .body(Body::from(corpo.to_vec()))
        .expect("pedido");
    let resposta = routes::router(state.clone())
        .oneshot(request)
        .await
        .expect("resposta");
    let status = resposta.status();
    if !status.is_success() {
        let corpo = axum::body::to_bytes(resposta.into_body(), 64 * 1024)
            .await
            .unwrap_or_default();
        eprintln!("{path} → {status}: {}", String::from_utf8_lossy(&corpo));
    }
    status
}

#[tokio::test]
async fn duas_instancias_do_mesmo_release_diferem_na_configuracao_e_nao_na_seguranca() {
    let Ok(url) = std::env::var("OCINYE_TEST_DATABASE_URL") else {
        eprintln!("skipping: OCINYE_TEST_DATABASE_URL is not set");
        return;
    };
    let Some(store) = armazenamento() else {
        assert!(
            std::env::var("CI").is_err(),
            "sem armazenamento, o logótipo não se prova; defina OCINYE_TEST_STORAGE_ENDPOINT"
        );
        eprintln!("skipping: OCINYE_TEST_STORAGE_ENDPOINT is not set");
        return;
    };

    // (nome, perfil, língua, fuso, fixações, desactivar, com logótipo)
    let configuracoes = [
        (
            "Laboratório Kalandula",
            "research",
            "pt",
            "Africa/Luanda",
            vec!["notes", "files"],
            "calendar",
            true,
        ),
        (
            "Atelier Mwangole",
            "business",
            "fr",
            "Europe/Paris",
            vec!["files"],
            "notes",
            false,
        ),
    ];
    let mut vistas = Vec::new();
    for (nome, perfil, lingua, fuso, fixacoes, desactivar, com_logo) in configuracoes {
        let (pool, base) = base_nova(&url).await;
        let org: Uuid = sqlx::query_scalar(
            "INSERT INTO organisations (slug, name, profile) VALUES ($1, $2, $3) RETURNING id",
        )
        .bind(format!("m{}", Uuid::new_v4().simple()))
        .bind(nome)
        .bind(perfil)
        .fetch_one(&pool)
        .await
        .expect("organização");
        sqlx::query(
            "INSERT INTO storage_backends
                 (code, kind, display_name, location_label, bucket, is_default, is_active)
             VALUES ('ocinye-test-default', 's3_compatible', 'Test', 'test', 'prova', TRUE, TRUE)",
        )
        .execute(&pool)
        .await
        .expect("backend");
        let mut state = nucleo(pool.clone(), org);
        state.store = Some(store.clone());
        let (_, admin) = membro(&pool, org, TechnicalRole::PlatformAdmin).await;
        let (_, pessoa) = membro(&pool, org, TechnicalRole::ResearchMember).await;

        // ── A configuração desta Instância ──────────────────────────────
        let (status, corpo) = pedido(
            &state,
            Some(&admin),
            None,
            "PUT",
            "/api/v1/instance/settings",
            Some(json!({ "default_locale": lingua, "timezone": fuso, "default_pins": fixacoes })),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{corpo}");
        let (status, _) = pedido(
            &state,
            Some(&admin),
            None,
            "PUT",
            &format!("/api/v1/instance/applications/{desactivar}"),
            Some(json!({ "active": false })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        if com_logo {
            assert_eq!(
                pedido_bytes(&state, &admin, "/api/v1/instance/logo", PNG).await,
                StatusCode::OK
            );
        }

        // ── A segurança é a mesma nas duas ──────────────────────────────
        let (status, _) =
            pedido(&state, None, None, "GET", "/api/v1/instance/settings", None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{nome}: sem sessão");
        let (status, _) = pedido(
            &state,
            Some(&pessoa),
            None,
            "PUT",
            "/api/v1/instance/settings",
            Some(json!({ "default_locale": "en" })),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::FORBIDDEN,
            "{nome}: um membro mudou a Instância"
        );
        assert_eq!(
            pedido_bytes(&state, &pessoa, "/api/v1/instance/logo", PNG).await,
            StatusCode::FORBIDDEN,
            "{nome}: um membro mudou o logótipo"
        );
        let (status, _) = pedido(
            &state,
            Some(&admin),
            None,
            "PUT",
            "/api/v1/instance/settings",
            Some(json!({ "timezone": "Marte/Olympus" })),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{nome}: um fuso inventado"
        );
        assert_eq!(
            pedido_bytes(
                &state,
                &admin,
                "/api/v1/instance/logo",
                b"<svg onload='x'/>"
            )
            .await,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{nome}: um SVG é um documento com script, e não uma marca"
        );

        // ── O que cada uma mostra ───────────────────────────────────────
        let (_, marca) = pedido(&state, None, None, "GET", "/api/v1/instance/branding", None).await;
        let (_, fixadas) = pedido(
            &state,
            Some(&pessoa),
            None,
            "GET",
            "/api/v1/me/apps/pins",
            None,
        )
        .await;
        let (_, aplicacoes) = pedido(
            &state,
            Some(&admin),
            None,
            "GET",
            "/api/v1/instance/applications",
            None,
        )
        .await;
        let logo = routes::router(state.clone())
            .oneshot(
                Request::builder()
                    .uri("/api/v1/instance/logo")
                    .body(Body::empty())
                    .expect("pedido"),
            )
            .await
            .expect("resposta");
        let logo_status = logo.status();
        let logo_bytes = axum::body::to_bytes(logo.into_body(), 1024 * 1024)
            .await
            .expect("corpo");
        vistas.push((
            nome,
            perfil,
            lingua,
            fixacoes,
            desactivar,
            com_logo,
            marca,
            fixadas,
            aplicacoes,
            logo_status,
            logo_bytes,
        ));
        drop(state);
        pool.close().await;
        apagar_base(&url, &base).await;
    }

    for (
        nome,
        perfil,
        lingua,
        fixacoes,
        desactivar,
        com_logo,
        marca,
        fixadas,
        aplicacoes,
        logo_status,
        logo_bytes,
    ) in &vistas
    {
        assert_eq!(marca["name"], *nome);
        assert_eq!(marca["default_locale"], *lingua);
        assert_eq!(
            marca["product"], "Ocinye OS",
            "a marca não esconde a plataforma"
        );
        assert_eq!(marca["has_logo"], *com_logo);
        assert_eq!(fixadas["source"], "instance", "{nome}: {fixadas}");
        assert_eq!(fixadas["pinned"], json!(fixacoes));
        assert_eq!(aplicacoes["profile"], *perfil, "{aplicacoes}");
        let estado = aplicacoes["applications"]
            .as_array()
            .expect("aplicações")
            .iter()
            .find(|a| a["id"] == *desactivar)
            .expect("aplicação");
        assert_eq!(
            estado["active"], false,
            "{nome}: {desactivar} devia estar inactiva"
        );
        if *com_logo {
            assert_eq!(*logo_status, StatusCode::OK);
            assert_eq!(
                logo_bytes.as_ref(),
                PNG,
                "{nome}: o logótipo não voltou igual"
            );
        } else {
            assert_eq!(*logo_status, StatusCode::NOT_FOUND);
        }
    }
    assert_ne!(
        vistas[0].6, vistas[1].6,
        "as duas Instâncias mostram a mesma marca"
    );
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
