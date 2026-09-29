//! O sistema real das viagens do Workspace (D001, D002): um Core e um
//! Workspace neste processo, cada um no seu porto, sobre uma base descartável.
//! Partilhado pelos ficheiros de viagens; cada um usa só uma parte.

#![allow(dead_code)]

use std::sync::{Arc, OnceLock};
use std::time::Duration;

use axum::http::StatusCode;
use ocinye_contracts::{CredentialKind, TechnicalRole};
use ocinye_core::authn::TokenVerifier;
use ocinye_core::config::CoreConfig;
use ocinye_core::modules::identity::{Authenticator, Throttle};
use ocinye_core::modules::mail::provider::UnconfiguredProvider;
use ocinye_core::password::{Hasher, HashingParams, Secret};
use ocinye_core_server::state::AppState;
use ocinye_workspace::config::WorkspaceConfig;
use ocinye_workspace::session::SessionStore;
use ocinye_workspace::{routes as workspace_routes, WorkspaceState};
use sqlx::PgPool;
use uuid::Uuid;

// ── O sistema, levantado de verdade ────────────────────────────────────────

pub fn sealing_key() -> &'static ocinye_core::password::sealed::SealingKey {
    static KEY: OnceLock<ocinye_core::password::sealed::SealingKey> = OnceLock::new();
    KEY.get_or_init(|| {
        ocinye_core::password::sealed::SealingKey::from_base64(
            &ocinye_core::password::sealed::SealingKey::generate(),
        )
        .expect("raiz de selagem de teste")
    })
}

pub fn core_state(pool: PgPool, organisation_id: Uuid) -> AppState {
    core_state_with(
        pool,
        organisation_id,
        Arc::new(ocinye_core::modules::intelligence::NoProvider),
    )
}

/// O Core com o fornecedor de inferência dado (nos testes, o
/// `FixtureProvider` do Core, que só existe com `test-fixtures`).
pub fn core_state_with(
    pool: PgPool,
    organisation_id: Uuid,
    inference: Arc<dyn ocinye_core::modules::intelligence::InferenceProvider>,
) -> AppState {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        // SAFETY: uma escrita, antes de qualquer viagem começar trabalho.
        unsafe {
            std::env::set_var("OCINYE_DATABASE_URL", "postgres://x/x");
        }
    });
    let mut config = CoreConfig::from_env().expect("configuração do Core");
    config.sealing_key = Some(sealing_key().clone());
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
        store: armazenamento(),
        embeddings: None,
        inference,
        mail_registry,
        realtime: Arc::new(ocinye_core::realtime::Realtime::ausente()),
        mail_probe: Arc::new(SemCorreio),
        capabilities: Arc::new(
            ocinye_core::capabilities::Capabilities::empty().expect("motor de capacidades"),
        ),
        organisation_id,
    }
}

/// O armazenamento de objectos de teste (Garage), quando o ambiente o dá.
/// Sem ele, o Core não tem onde guardar bytes, como numa instalação sem
/// armazenamento; as viagens que precisam dele dizem que saltaram.
///
/// Sem tecto fixo de envio: o do Core por omissão (5 TiB), para que a prova do
/// envio por partes não assente num limite que a aplicação não tem.
pub fn armazenamento() -> Option<Arc<ocinye_core::storage::ObjectStore>> {
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
        max_upload_bytes: 5 * 1024 * 1024 * 1024 * 1024,
    })
    .map(Arc::new)
}

/// Esta instalação não tem correio: a sonda nunca é chamada.
pub struct SemCorreio;

#[async_trait::async_trait]
impl ocinye_core::modules::mail::provider::CredentialProbe for SemCorreio {
    async fn verify(
        &self,
        _endereco: &str,
        _username: &str,
        _senha: &str,
    ) -> ocinye_core::modules::mail::provider::ProviderResult<()> {
        Ok(())
    }
}

pub fn workspace_state(core_url: &str, public_url: &str) -> WorkspaceState {
    WorkspaceState {
        config: Arc::new(WorkspaceConfig {
            core_transport: ocinye_workspace::config::CoreTransport::Roteavel,
            bind_address: "127.0.0.1:0".to_owned(),
            public_url: public_url.to_owned(),
            core_url: core_url.to_owned(),
            session_ttl: Duration::from_secs(3600),
            cookie_secure: false,
            log_level: "warn".to_owned(),
            log_format: "pretty".to_owned(),
            is_production: false,
            static_dir: format!("{}/static", env!("CARGO_MANIFEST_DIR")),
        }),
        sessions: SessionStore::new(),
        http: reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("cliente"),
    }
}

/// Um Core e um Workspace desta viagem, sobre uma organização só dela.
pub struct Sistema {
    pub pool: PgPool,
    pub organisation_id: Uuid,
    pub core_url: String,
    pub url: String,
    pub http: reqwest::Client,
}

pub async fn pool() -> Option<PgPool> {
    let Ok(url) = std::env::var("OCINYE_TEST_DATABASE_URL") else {
        assert!(
            std::env::var("CI").is_err(),
            "OCINYE_TEST_DATABASE_URL em falta em CI: as viagens D001 ficariam por correr"
        );
        eprintln!("SALTADA: sem OCINYE_TEST_DATABASE_URL");
        return None;
    };
    let pool = PgPool::connect(&url)
        .await
        .expect("OCINYE_TEST_DATABASE_URL definida mas a base não responde");
    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .expect("migrations");
    ocinye_core::fixtures::refuse_canonical_organisation(&pool).await;
    Some(pool)
}

impl Sistema {
    pub async fn levantar(profile: &str) -> Option<Self> {
        Self::levantar_com(
            profile,
            Arc::new(ocinye_core::modules::intelligence::NoProvider),
        )
        .await
    }

    /// Um sistema cujo Core usa o fornecedor de inferência dado.
    pub async fn levantar_com(
        profile: &str,
        inference: Arc<dyn ocinye_core::modules::intelligence::InferenceProvider>,
    ) -> Option<Self> {
        let pool = pool().await?;
        let organisation_id: Uuid = sqlx::query_scalar(
            "INSERT INTO organisations (slug, name, profile) VALUES ($1, $2, $3) RETURNING id",
        )
        .bind(format!("d001-{}", Uuid::new_v4().simple()))
        .bind("Instituição da viagem")
        .bind(profile)
        .fetch_one(&pool)
        .await
        .expect("organização");

        let core_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("porto do Core");
        let core_url = format!(
            "http://127.0.0.1:{}",
            core_listener.local_addr().expect("endereço").port()
        );
        let core = core_state_with(pool.clone(), organisation_id, inference);
        tokio::spawn(async move {
            let _ = axum::serve(core_listener, ocinye_core_server::routes::router(core)).await;
        });

        let ws_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("porto do Workspace");
        let url = format!(
            "http://127.0.0.1:{}",
            ws_listener.local_addr().expect("endereço").port()
        );
        let ws = workspace_state(&core_url, &url);
        tokio::spawn(async move {
            let _ = axum::serve(ws_listener, workspace_routes::router(ws)).await;
        });

        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(20))
            .build()
            .expect("cliente");
        // O `TcpListener` aceita antes de o servidor servir: espera-se por uma
        // resposta, não por uma resposta pronta.
        for _ in 0..100 {
            if http.get(format!("{core_url}/ready")).send().await.is_ok()
                && http.get(format!("{url}/health")).send().await.is_ok()
            {
                println!("VIAGEM LEVANTADA");
                return Some(Self {
                    pool,
                    organisation_id,
                    core_url,
                    url,
                    http,
                });
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        panic!("o Core ou o Workspace não responderam");
    }

    /// Uma pessoa activa com palavra-passe permanente e estes papéis.
    pub async fn pessoa(&self, roles: &[TechnicalRole]) -> (Uuid, String, String) {
        let handle = format!("d{}", Uuid::new_v4().simple());
        let email = format!("{handle}@ocinye.com");
        let person_id: Uuid = sqlx::query_scalar(
            "INSERT INTO people (organisation_id, full_name, email, status)
             VALUES ($1, $2, $3, 'active') RETURNING id",
        )
        .bind(self.organisation_id)
        .bind(format!("Ana {handle}"))
        .bind(&email)
        .fetch_one(&self.pool)
        .await
        .expect("pessoa");
        for role in roles {
            sqlx::query("INSERT INTO person_roles (person_id, role) VALUES ($1, $2)")
                .bind(person_id)
                .bind(role.as_str())
                .execute(&self.pool)
                .await
                .expect("papel");
        }
        let password = format!("Ocinye-{}-2026!", Uuid::new_v4().simple());
        let verifier = Hasher::new(HashingParams {
            memory_kib: 19 * 1024,
            iterations: 2,
            parallelism: 1,
        })
        .hash(&Secret::new(password.clone()))
        .expect("verificador");
        sqlx::query(
            "INSERT INTO credentials (person_id, kind, state, verifier, issued_reason)
             VALUES ($1, $2, 'active', $3, 'viagem D001')",
        )
        .bind(person_id)
        .bind(CredentialKind::Permanent.as_str())
        .bind(&verifier)
        .execute(&self.pool)
        .await
        .expect("credencial");
        (person_id, email, password)
    }

    pub async fn totp_confirmado(&self, person_id: Uuid) {
        let selado = ocinye_core::password::sealed::seal(
            sealing_key(),
            ocinye_core::password::sealed::SealingDomain::MfaTotp,
            SEMENTE_MFA,
        )
        .expect("selar o seed");
        sqlx::query(
            "INSERT INTO mfa_totp_secrets (person_id, nonce, ciphertext, confirmed_at)
             VALUES ($1, $2, $3, now())",
        )
        .bind(person_id)
        .bind(&selado.nonce)
        .bind(&selado.ciphertext)
        .execute(&self.pool)
        .await
        .expect("seed TOTP");
    }

    pub fn get(&self, path: &str, cookie: &str) -> reqwest::RequestBuilder {
        self.http
            .get(format!("{}{path}", self.url))
            .header("accept", "text/html")
            .header("cookie", format!("oc_boot=1; {cookie}"))
    }

    pub fn escrever(
        &self,
        method: reqwest::Method,
        path: &str,
        cookie: &str,
    ) -> reqwest::RequestBuilder {
        self.http
            .request(method, format!("{}{path}", self.url))
            .header("origin", &self.url)
            .header("cookie", format!("oc_boot=1; {cookie}"))
    }

    /// Entra pelo formulário real e devolve o cookie de sessão.
    pub async fn entrar(&self, email: &str, password: &str) -> (StatusCode, String, String) {
        let r = self
            .escrever(reqwest::Method::POST, "/login", "")
            .form(&[("email", email), ("password", password)])
            .send()
            .await
            .expect("POST /login");
        let status = StatusCode::from_u16(r.status().as_u16()).expect("estado");
        let destino = location(&r);
        let cookie = r
            .headers()
            .get_all("set-cookie")
            .iter()
            .filter_map(|v| v.to_str().ok())
            .find(|c| c.starts_with(&format!("{}=", ocinye_workspace::session::COOKIE_NAME)))
            .map(|c| c.split(';').next().unwrap_or_default().to_owned())
            .unwrap_or_default();
        (status, destino, cookie)
    }

    pub async fn membro_com_sessao(&self, roles: &[TechnicalRole]) -> (Uuid, String) {
        let (id, email, password) = self.pessoa(roles).await;
        let (status, destino, cookie) = self.entrar(&email, &password).await;
        assert_eq!((status, destino.as_str()), (StatusCode::SEE_OTHER, "/"));
        assert!(!cookie.is_empty(), "a entrada não deu sessão");
        (id, cookie)
    }

    pub async fn html(&self, path: &str, cookie: &str) -> (u16, String) {
        let r = self.get(path, cookie).send().await.expect("GET");
        let s = r.status().as_u16();
        (s, r.text().await.unwrap_or_default())
    }
}

pub fn location(r: &reqwest::Response) -> String {
    r.headers()
        .get("location")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_owned()
}

pub const SEMENTE_MFA: &str = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";

pub fn codigo_totp(seed_base32: &str) -> String {
    use data_encoding::BASE32_NOPAD;
    use hmac::{Hmac, Mac};
    use sha1::Sha1;
    let seed = BASE32_NOPAD
        .decode(seed_base32.trim().to_ascii_uppercase().as_bytes())
        .expect("base32");
    let counter = u64::try_from(chrono::Utc::now().timestamp().max(0)).unwrap_or(0) / 30;
    let mut mac = <Hmac<Sha1> as Mac>::new_from_slice(&seed).expect("hmac");
    mac.update(&counter.to_be_bytes());
    let d = mac.finalize().into_bytes();
    let off = usize::from(d[d.len() - 1] & 0x0f);
    let bin = (u32::from(d[off] & 0x7f) << 24)
        | (u32::from(d[off + 1]) << 16)
        | (u32::from(d[off + 2]) << 8)
        | u32::from(d[off + 3]);
    format!("{:06}", bin % 1_000_000)
}

/// A ordem dos widgets tal como a grelha os desenhou (só os da grelha: a
/// biblioteca também tem `data-kind`).
pub fn kinds(html: &str) -> Vec<String> {
    html.split(r#"data-part="desk-widget""#)
        .skip(1)
        .filter_map(|s| s.split(r#"data-kind=""#).nth(1)?.split('"').next())
        .map(str::to_owned)
        .collect()
}

/// O fundo da área de trabalho (o `data-wall` da casca, e não os da folha de
/// fundos, que os lista a todos).
pub fn wall(html: &str) -> &str {
    html.split(r#"data-wall=""#)
        .skip(1)
        .find(|resto| {
            resto
                .split('>')
                .next()
                .is_some_and(|tag| tag.contains(r#"class="oc-desk""#))
        })
        .and_then(|resto| resto.split('"').next())
        .unwrap_or_default()
}

/// O texto de um elemento, sem as marcas que o SSR põe entre nós de texto.
pub fn text_of(html: &str, class: &str) -> String {
    html.split(&format!(r#"class="{class}">"#))
        .nth(1)
        .and_then(|s| s.split("</").next())
        .unwrap_or_default()
        .replace("<!-- -->", "")
        .replace("<!>", "")
}
