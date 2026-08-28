//! A revisão de bibliografia pelo transporte, com o Runtime a sério.
//!
//! # O que estes testes provam, e o que não repetem
//!
//! A suite do Core já prova a operação. Isto prova o **transporte**: que o
//! caminho fala de bibliografia e não de execução, que o limite é o mesmo dos
//! dois lados, que uma recusa continua a ser uma recusa depois de passar por
//! HTTP, e — sobretudo — que nada do interior do motor sai pela resposta.
//!
//! Salta quando `OCINYE_TEST_DATABASE_URL` não está definida; **falha** quando
//! está e a base não responde, e **falha** quando o componente não está
//! construído.

use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use ocinye_contracts::bibliography::MAX_BIBTEX_BYTES;
use ocinye_contracts::TechnicalRole;
use ocinye_core::capabilities::{Capabilities, Component};
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

const BIBTEX: &str =
    "@ARTICLE{mucai2024, TITLE = {Vento no Mucai}, author = {Ana Mucai}, year = {2024}}";

macro_rules! base {
    () => {{
        let Ok(url) = std::env::var("OCINYE_TEST_DATABASE_URL") else {
            eprintln!("skipping: OCINYE_TEST_DATABASE_URL is not set");
            return;
        };
        let pool = PgPool::connect(&url)
            .await
            .expect("OCINYE_TEST_DATABASE_URL is set but the database is unreachable");
        sqlx::migrate!("../../migrations")
            .run(&pool)
            .await
            .expect("migrations must apply");
        pool
    }};
}

fn config() -> CoreConfig {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let url = std::env::var("OCINYE_TEST_DATABASE_URL").unwrap_or_default();
        // SAFETY: uma escrita, antes de qualquer teste começar trabalho.
        unsafe {
            std::env::set_var("OCINYE_DATABASE_URL", url);
        }
    });
    CoreConfig::from_env().expect("configuração de teste")
}

/// O directório onde o componente foi construído.
fn componentes() -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/wasm32-wasip1/release")
        .to_string_lossy()
        .into_owned()
}

fn state(pool: PgPool, organisation_id: Uuid, com_componente: bool) -> AppState {
    let config = config();
    let verifier =
        ocinye_core::authn::TokenVerifier::new(config.oidc.clone()).expect("verificador");
    let authenticator = Arc::new(Authenticator::new(
        Hasher::new(HashingParams {
            memory_kib: config.auth.argon2_memory_kib,
            iterations: config.auth.argon2_iterations,
            parallelism: config.auth.argon2_parallelism,
        }),
        Throttle {
            per_ip: config.auth.throttle_per_ip,
            per_username: config.auth.throttle_per_username,
            window_minutes: config.auth.throttle_window_minutes,
        },
        config.auth.temporary_credential_hours,
    ));

    let capabilities = if com_componente {
        let dir = componentes();
        let carregadas = Capabilities::load(&dir).expect("o motor constrói-se");
        assert!(
            carregadas.has(Component::BibtexImport),
            "o componente WebAssembly não está construído em «{dir}».\n\
             Corra: ./scripts/build-capabilities.sh"
        );
        carregadas
    } else {
        Capabilities::load("/um/directorio/sem/componentes").expect("o motor constrói-se")
    };

    AppState {
        pool,
        config: Arc::new(config),
        verifier,
        authenticator,
        store: None,
        inference: Arc::new(ocinye_core::modules::intelligence::NoProvider),
        mail_provider: Arc::new(UnconfiguredProvider),
        capabilities: Arc::new(capabilities),
        organisation_id,
    }
}

/// Uma sessão viva, como o Core as emite.
async fn sessao(pool: &PgPool, person_id: Uuid) -> Secret {
    let token = Secret::new(format!("t{}", Uuid::new_v4().simple()));
    sqlx::query(
        "INSERT INTO sessions (person_id, token_digest, state, expires_at)
             VALUES ($1, $2, 'active', now() + interval '1 hour')",
    )
    .bind(person_id)
    .bind(identity::session_digest(&token))
    .execute(pool)
    .await
    .expect("sessão");
    token
}

/// Uma instituição com um ambiente, e alguém que lhe pertence.
async fn mundo(pool: &PgPool) -> (Uuid, Uuid, Secret, Secret) {
    let tag = Uuid::new_v4().simple().to_string();
    let organisation_id: Uuid =
        sqlx::query_scalar("INSERT INTO organisations (slug, name) VALUES ($1, $1) RETURNING id")
            .bind(format!("h{tag}"))
            .fetch_one(pool)
            .await
            .expect("organização");

    let unidade = |sufixo: &str| {
        let codigo = format!("H{}{}", sufixo, &tag[..6]).to_uppercase();
        let pool = pool.clone();
        async move {
            sqlx::query_scalar::<_, Uuid>(
                "INSERT INTO units (organisation_id, code, name) VALUES ($1, $2, 'Unidade')
                 RETURNING id",
            )
            .bind(organisation_id)
            .bind(codigo)
            .fetch_one(&pool)
            .await
            .expect("unidade")
        }
    };
    let unidade_a = unidade("A").await;
    let unidade_b = unidade("B").await;

    let workspace: Uuid = sqlx::query_scalar(
        "INSERT INTO research_workspaces
             (organisation_id, unit_id, code, title, kind, classification)
         VALUES ($1, $2, $3, 'Ambiente', 'idea', 'INTERNAL') RETURNING id",
    )
    .bind(organisation_id)
    .bind(unidade_a)
    .bind(format!("WS-H-{}", &tag[..6]).to_uppercase())
    .fetch_one(pool)
    .await
    .expect("workspace");

    let membro = |unit_id: Uuid| {
        let pool = pool.clone();
        async move {
            let handle = format!("m{}", Uuid::new_v4().simple());
            let person_id: Uuid = sqlx::query_scalar(
                "INSERT INTO people (organisation_id, full_name, email, username, status)
                     VALUES ($1, $2, $3, $2, 'active') RETURNING id",
            )
            .bind(organisation_id)
            .bind(&handle)
            .bind(format!("{handle}@ocinye.com"))
            .fetch_one(&pool)
            .await
            .expect("pessoa");

            sqlx::query("INSERT INTO person_roles (person_id, role) VALUES ($1, $2)")
                .bind(person_id)
                .bind(TechnicalRole::ResearchMember.as_str())
                .execute(&pool)
                .await
                .expect("papel");

            sqlx::query(
                "INSERT INTO unit_memberships (unit_id, person_id, role) VALUES ($1, $2, 'member')",
            )
            .bind(unit_id)
            .bind(person_id)
            .execute(&pool)
            .await
            .expect("pertença");

            (sessao(&pool, person_id).await, person_id)
        }
    };

    let (dentro, dentro_id) = membro(unidade_a).await;
    let (fora, _) = membro(unidade_b).await;

    // Pertença ao ambiente: criar num ambiente de investigação depende dela. A
    // unidade dá para ler o que é interno, e é por isso que quem está de fora
    // recebe uma recusa em vez de uma ausência.
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, person_id, role)
             VALUES ($1, $2, 'member')",
    )
    .bind(workspace)
    .bind(dentro_id)
    .execute(pool)
    .await
    .expect("pertença ao ambiente");

    (organisation_id, workspace, dentro, fora)
}

/// Um pedido ao router real.
async fn pedir(
    state: &AppState,
    token: &Secret,
    caminho: &str,
    corpo: Value,
) -> (StatusCode, Value) {
    let pedido = Request::builder()
        .method("POST")
        .uri(caminho)
        .header(header::AUTHORIZATION, format!("Bearer {}", token.expose()))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(corpo.to_string()))
        .expect("pedido");

    let resposta = routes::router(state.clone())
        .oneshot(pedido)
        .await
        .expect("resposta");
    let estado = resposta.status();
    let bytes = axum::body::to_bytes(resposta.into_body(), 8 * 1024 * 1024)
        .await
        .expect("corpo");
    (
        estado,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn caminho(workspace: Uuid) -> String {
    format!("/api/v1/workspaces/{workspace}/bibliography/review")
}

// ── O caminho ───────────────────────────────────────────────────────────

/// Uma bibliografia atravessa HTTP, o Core, o Runtime e volta.
#[tokio::test]
async fn uma_bibliografia_atravessa_o_transporte_e_volta_lida() {
    let pool = base!();
    let (org, workspace, dentro, _) = mundo(&pool).await;
    let estado = state(pool, org, true);

    let (codigo, corpo) = pedir(
        &estado,
        &dentro,
        &caminho(workspace),
        json!({ "bibtex": BIBTEX }),
    )
    .await;

    assert_eq!(codigo, StatusCode::OK, "{corpo}");
    assert_eq!(corpo["entries"].as_array().expect("entradas").len(), 1);
    assert_eq!(corpo["entries"][0]["citation_key"], "mucai2024");
    assert_eq!(corpo["entries"][0]["entry_type"], "article");
    assert!(corpo["unreadable"]
        .as_array()
        .expect("ilegíveis")
        .is_empty());
    assert!(
        corpo["normalized"]
            .as_str()
            .expect("normalizado")
            .contains("@article{mucai2024,"),
        "{corpo}"
    );
}

// ── As recusas ──────────────────────────────────────────────────────────

/// Sem credencial não se revê nada.
#[tokio::test]
async fn sem_credencial_nao_se_revê_bibliografia() {
    let pool = base!();
    let (org, workspace, _, _) = mundo(&pool).await;
    let estado = state(pool, org, true);

    let pedido = Request::builder()
        .method("POST")
        .uri(caminho(workspace))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "bibtex": BIBTEX }).to_string()))
        .expect("pedido");

    let resposta = routes::router(estado)
        .oneshot(pedido)
        .await
        .expect("resposta");
    assert_eq!(resposta.status(), StatusCode::UNAUTHORIZED);
}

/// Quem não alcança o ambiente não o revê, e a recusa não o distingue de ausência.
#[tokio::test]
async fn quem_nao_alcanca_o_ambiente_e_recusado_pelo_transporte() {
    let pool = base!();
    let (org, workspace, _, fora) = mundo(&pool).await;
    let estado = state(pool, org, true);

    let (codigo, _) = pedir(
        &estado,
        &fora,
        &caminho(workspace),
        json!({ "bibtex": BIBTEX }),
    )
    .await;

    assert!(
        codigo == StatusCode::NOT_FOUND || codigo == StatusCode::FORBIDDEN,
        "esperava-se recusa, veio {codigo}"
    );
}

/// Um corpo malformado é um erro de pedido, e não do Runtime.
#[tokio::test]
async fn um_corpo_malformado_nao_chega_ao_runtime() {
    let pool = base!();
    let (org, workspace, dentro, _) = mundo(&pool).await;
    let estado = state(pool, org, true);

    let (codigo, _) = pedir(
        &estado,
        &dentro,
        &caminho(workspace),
        json!({ "isto_nao_e_o_campo": BIBTEX }),
    )
    .await;

    assert!(
        codigo.is_client_error(),
        "um corpo sem `bibtex` devia ser recusado pelo pedido, veio {codigo}"
    );
}

/// Uma bibliografia acima do limite é recusada.
#[tokio::test]
async fn uma_bibliografia_grande_de_mais_e_recusada_pelo_transporte() {
    let pool = base!();
    let (org, workspace, dentro, _) = mundo(&pool).await;
    let estado = state(pool, org, true);

    let enorme = "@misc{k, title = {t}}\n".repeat(MAX_BIBTEX_BYTES / 10);
    let (codigo, _) = pedir(
        &estado,
        &dentro,
        &caminho(workspace),
        json!({ "bibtex": enorme }),
    )
    .await;

    assert!(
        codigo.is_client_error(),
        "acima do limite devia ser recusado, veio {codigo}"
    );
}

// ── O que a resposta não leva ───────────────────────────────────────────

/// Sem componente, a resposta diz que a capacidade não está — e nada mais.
///
/// # O que se procura aqui
///
/// Não é o código de estado. É a mensagem: um caminho de ficheiro, o nome do
/// motor ou o do componente numa resposta pública são reconhecimento oferecido
/// a quem esteja a sondar a fronteira.
#[tokio::test]
async fn a_resposta_nao_leva_o_interior_do_motor() {
    let pool = base!();
    let (org, workspace, dentro, _) = mundo(&pool).await;
    let estado = state(pool, org, false);

    let (codigo, corpo) = pedir(
        &estado,
        &dentro,
        &caminho(workspace),
        json!({ "bibtex": BIBTEX }),
    )
    .await;

    assert!(codigo.is_server_error() || codigo.is_client_error());

    let texto = corpo.to_string().to_lowercase();
    for fragmento in [
        "wasm",
        "wasmtime",
        "wasi",
        "bibtex-import",
        "/users/",
        "/var/",
        "target/",
        "trap",
        "fuel",
        "manifest",
        "panic",
    ] {
        assert!(
            !texto.contains(fragmento),
            "«{fragmento}» chegou à resposta: {corpo}"
        );
    }
}

/// O sucesso também não leva o interior do motor.
///
/// A resposta bem-sucedida é a que mais gente vê, e é a que menos se inspecciona.
#[tokio::test]
async fn nem_a_resposta_bem_sucedida_leva_o_interior_do_motor() {
    let pool = base!();
    let (org, workspace, dentro, _) = mundo(&pool).await;
    let estado = state(pool, org, true);

    let (codigo, corpo) = pedir(
        &estado,
        &dentro,
        &caminho(workspace),
        json!({ "bibtex": BIBTEX }),
    )
    .await;
    assert_eq!(codigo, StatusCode::OK);

    let texto = corpo.to_string().to_lowercase();
    for fragmento in ["wasm", "wasmtime", "wasi", "fuel", "/users/", "target/"] {
        assert!(
            !texto.contains(fragmento),
            "«{fragmento}» chegou à resposta: {corpo}"
        );
    }
}
