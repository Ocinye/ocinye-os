//! As viagens da integração Claude Design D001, pelo caminho real:
//!
//! ```text
//! pedido HTTP → rota do Workspace → controlador → HTTP → Core → PostgreSQL
//! ```
//!
//! O Core e o Workspace sobem neste processo, cada um no seu porto, sobre uma
//! base descartável. Nada aqui é simulado do lado do produto: a entrada passa
//! pelo `POST /login` verdadeiro, o Desktop lê e grava a disposição no Core, e
//! o que se afirma é o HTML que uma pessoa recebe.
//!
//! O que fica fora: o comportamento do JavaScript (arrastar, redimensionar,
//! gravar com espera). Esse mede-se no browser; aqui prova-se o contrato HTTP
//! que o JavaScript usa (`PUT /me/desktop`, `409`, `POST /me/desktop/restore`).
//!
//! Salta quando `OCINYE_TEST_DATABASE_URL` não está definida — e diz que saltou;
//! em CI, falha.

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

fn sealing_key() -> &'static ocinye_core::password::sealed::SealingKey {
    static KEY: OnceLock<ocinye_core::password::sealed::SealingKey> = OnceLock::new();
    KEY.get_or_init(|| {
        ocinye_core::password::sealed::SealingKey::from_base64(
            &ocinye_core::password::sealed::SealingKey::generate(),
        )
        .expect("raiz de selagem de teste")
    })
}

fn core_state(pool: PgPool, organisation_id: Uuid) -> AppState {
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
        store: None,
        embeddings: None,
        inference: Arc::new(ocinye_core::modules::intelligence::NoProvider),
        mail_registry,
        realtime: Arc::new(ocinye_core::realtime::Realtime::ausente()),
        mail_probe: Arc::new(SemCorreio),
        capabilities: Arc::new(
            ocinye_core::capabilities::Capabilities::empty().expect("motor de capacidades"),
        ),
        organisation_id,
    }
}

/// Esta instalação não tem correio: a sonda nunca é chamada.
struct SemCorreio;

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

fn workspace_state(core_url: &str, public_url: &str) -> WorkspaceState {
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
struct Sistema {
    pool: PgPool,
    organisation_id: Uuid,
    core_url: String,
    url: String,
    http: reqwest::Client,
}

async fn pool() -> Option<PgPool> {
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
    async fn levantar(profile: &str) -> Option<Self> {
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
        let core = core_state(pool.clone(), organisation_id);
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
    async fn pessoa(&self, roles: &[TechnicalRole]) -> (Uuid, String, String) {
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

    async fn totp_confirmado(&self, person_id: Uuid) {
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

    fn get(&self, path: &str, cookie: &str) -> reqwest::RequestBuilder {
        self.http
            .get(format!("{}{path}", self.url))
            .header("accept", "text/html")
            .header("cookie", format!("oc_boot=1; {cookie}"))
    }

    fn escrever(
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
    async fn entrar(&self, email: &str, password: &str) -> (StatusCode, String, String) {
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

    async fn membro_com_sessao(&self, roles: &[TechnicalRole]) -> (Uuid, String) {
        let (id, email, password) = self.pessoa(roles).await;
        let (status, destino, cookie) = self.entrar(&email, &password).await;
        assert_eq!((status, destino.as_str()), (StatusCode::SEE_OTHER, "/"));
        assert!(!cookie.is_empty(), "a entrada não deu sessão");
        (id, cookie)
    }

    async fn html(&self, path: &str, cookie: &str) -> (u16, String) {
        let r = self.get(path, cookie).send().await.expect("GET");
        let s = r.status().as_u16();
        (s, r.text().await.unwrap_or_default())
    }
}

fn location(r: &reqwest::Response) -> String {
    r.headers()
        .get("location")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_owned()
}

const SEMENTE_MFA: &str = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";

fn codigo_totp(seed_base32: &str) -> String {
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
fn kinds(html: &str) -> Vec<String> {
    html.split(r#"data-part="desk-widget""#)
        .skip(1)
        .filter_map(|s| s.split(r#"data-kind=""#).nth(1)?.split('"').next())
        .map(str::to_owned)
        .collect()
}

/// O fundo da área de trabalho (o `data-wall` da casca, e não os da folha de
/// fundos, que os lista a todos).
fn wall(html: &str) -> &str {
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
fn text_of(html: &str, class: &str) -> String {
    html.split(&format!(r#"class="{class}">"#))
        .nth(1)
        .and_then(|s| s.split("</").next())
        .unwrap_or_default()
        .replace("<!-- -->", "")
        .replace("<!>", "")
}

// ── Autenticação ──────────────────────────────────────────────────────────

#[tokio::test]
async fn a_porta_e_a_do_design_com_a_distribuicao_da_instancia() {
    let Some(s) = Sistema::levantar("business").await else {
        return;
    };
    let (status, html) = s.html("/login", "").await;
    assert_eq!(status, 200);
    assert!(
        html.contains(r#"data-part="login""#),
        "não é a porta do D001"
    );
    assert!(html.contains("INSTÂNCIA OCINYE OS · OPERACIONAL"));
    assert!(html.contains("Acesso seguro à sua Instância Ocinye OS"));
    // A distribuição da Instância, e não a de investigação por omissão.
    assert!(html.contains(r#"data-distribution="business""#));
    assert!(!html.contains(r#"data-distribution="research""#));
    assert!(!html.contains("Business Distribution") && !html.contains("Distribuição Business"));
    // Sem chave de acesso, sem SSO.
    assert!(!html.to_lowercase().contains("chave de acesso"));
    assert!(!html.to_lowercase().contains("passkey"));
}

#[tokio::test]
async fn a_porta_fala_pt_en_e_fr_sem_misturar() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (_, en) = s.html("/login", "oc_locale=en").await;
    assert!(en.contains("OCINYE OS INSTANCE · OPERATIONAL"));
    assert!(en.contains("Secure access to your Ocinye OS Instance"));
    assert!(!en.contains("Acesso seguro"));
    let (_, fr) = s.html("/login", "oc_locale=fr").await;
    assert!(fr.contains("INSTANCE OCINYE OS · OPÉRATIONNELLE"));
    assert!(fr.contains("Accès sécurisé à votre instance Ocinye OS"));
    assert!(!fr.contains("Acesso seguro"));
}

#[tokio::test]
async fn uma_entrada_recusada_mostra_a_porta_com_o_erro_e_o_endereco() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (_, email, _) = s.pessoa(&[TechnicalRole::ResearchMember]).await;
    let r = s
        .escrever(reqwest::Method::POST, "/login", "oc_locale=en")
        .form(&[("email", email.as_str()), ("password", "errada")])
        .send()
        .await
        .expect("POST /login");
    assert_eq!(r.status().as_u16(), 401);
    let html = r.text().await.unwrap_or_default();
    assert!(
        html.contains(r#"data-part="login""#),
        "a recusa não é a porta"
    );
    assert!(
        html.contains("Invalid address or password."),
        "a recusa não chegou, ou não em inglês"
    );
    assert!(html.contains(&email), "o endereço escrito perdeu-se");
    assert!(!html.contains("invalid_credentials"));
}

#[tokio::test]
async fn sem_sessao_vai_para_a_porta_e_uma_sessao_perdida_diz_que_expirou() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let r = s.get("/", "").send().await.expect("GET /");
    assert_eq!(
        (r.status().as_u16(), location(&r).as_str()),
        (303, "/login")
    );
    let r = s
        .get(
            "/",
            &format!("{}=desconhecida", ocinye_workspace::session::COOKIE_NAME),
        )
        .send()
        .await
        .expect("GET /");
    assert_eq!(location(&r), "/login?reason=expired");
    let (status, html) = s.html("/login?reason=expired", "").await;
    assert_eq!(status, 200);
    assert!(html.contains(r#"data-part="session-end""#));
}

#[tokio::test]
async fn uma_sessao_revogada_no_core_acaba_no_workspace() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (id, cookie) = s.membro_com_sessao(&[TechnicalRole::ResearchMember]).await;
    sqlx::query("UPDATE sessions SET state = 'revoked', revoked_at = now(), revoked_reason = 'viagem D001' WHERE person_id = $1")
        .bind(id)
        .execute(&s.pool)
        .await
        .expect("revogar");
    let r = s.get("/", &cookie).send().await.expect("GET /");
    assert_eq!(location(&r), "/login?reason=expired");
    // E a sessão local foi-se também.
    let r = s.get("/notes", &cookie).send().await.expect("GET /notes");
    assert_eq!(location(&r), "/login?reason=expired");
}

#[tokio::test]
async fn o_arranque_entrega_a_ligacao_profunda_a_quem_ja_tem_sessao() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (_, cookie) = s.membro_com_sessao(&[TechnicalRole::ResearchMember]).await;
    // Sem marcador de arranque, o portão leva ao `/boot` com o destino.
    let r = s
        .http
        .get(format!("{}/notes", s.url))
        .header("accept", "text/html")
        .header("cookie", &cookie)
        .send()
        .await
        .expect("GET /notes");
    assert_eq!(location(&r), "/boot?return_to=/notes");
    // Com sessão válida e a Instância pronta, segue para onde ia.
    let r = s
        .http
        .get(format!("{}/boot?return_to=/notes", s.url))
        .header("cookie", &cookie)
        .send()
        .await
        .expect("GET /boot");
    assert_eq!(location(&r), "/notes");
    // Sem sessão, o ecrã de arranque do Design, que continua para a porta.
    let (status, html) = s.html("/boot?return_to=/notes", "").await;
    assert_eq!(status, 200);
    assert!(html.contains("oc-boot") && html.contains(r#"href="/login""#));
}

#[tokio::test]
async fn o_segundo_factor_passa_pelo_desafio_do_design() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (id, email, password) = s.pessoa(&[TechnicalRole::PlatformAdmin]).await;
    s.totp_confirmado(id).await;
    let (status, destino, cookie) = s.entrar(&email, &password).await;
    assert_eq!((status, destino.as_str()), (StatusCode::SEE_OTHER, "/mfa"));
    // Antes do segundo factor, nenhuma superfície do Workspace.
    let r = s.get("/", &cookie).send().await.expect("GET /");
    assert_eq!(location(&r), "/mfa");
    let (status, html) = s.html("/mfa", &cookie).await;
    assert_eq!(status, 200);
    assert!(
        html.contains(r#"action="/mfa/challenge""#),
        "não é o desafio"
    );
    // Um código errado volta ao desafio com a recusa.
    let r = s
        .escrever(reqwest::Method::POST, "/mfa/challenge", &cookie)
        .form(&[("code", "000000")])
        .send()
        .await
        .expect("desafio errado");
    assert_eq!(r.status().as_u16(), 401);
    assert!(r
        .text()
        .await
        .unwrap_or_default()
        .contains("oc-auth__error"));
    // O código certo entra.
    let r = s
        .escrever(reqwest::Method::POST, "/mfa/challenge", &cookie)
        .form(&[("code", codigo_totp(SEMENTE_MFA).as_str())])
        .send()
        .await
        .expect("desafio");
    assert_eq!(location(&r), "/");
}

// ── Casca, Desktop e aplicações por entregar ────────────────────────────────

#[tokio::test]
async fn o_desktop_e_o_da_distribuicao_com_dados_reais() {
    let Some(s) = Sistema::levantar("business").await else {
        return;
    };
    let (_, cookie) = s.membro_com_sessao(&[TechnicalRole::ResearchMember]).await;
    let (status, html) = s.html("/", &cookie).await;
    assert_eq!(status, 200, "{html}");
    assert!(html.contains(r#"data-oc="desk""#) && html.contains(r#"data-version="0""#));
    // A predefinição do sistema para Business, pela ordem do registo do Design.
    assert_eq!(
        kinds(&html),
        ["kpis", "tasks", "notice", "mail", "continue", "calendar", "activity"]
    );
    // A casca: o distintivo da distribuição, o nome, o lançador e a paleta.
    assert!(html.contains(r##"href="#oc-launcher""##) && html.contains(r#"id="oc-palette""#));
    assert_eq!(text_of(&html, "oc-account__dist"), "OCINYE OS · BUSINESS");
    // Os avisos não existem no Core: o widget diz indisponível, não «nenhum».
    assert!(html.contains("Indisponível nesta Instância."));
}

#[tokio::test]
async fn o_desktop_grava_recusa_a_versao_obsoleta_e_repoe() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (_, cookie) = s.membro_com_sessao(&[TechnicalRole::ResearchMember]).await;
    let disposicao = |version: u32| {
        serde_json::json!({
            "version": version, "wallpaper": "mist", "fit": "fill", "dim": 35,
            "widgets": [
                {"id": "notice", "kind": "notice", "w": 2, "h": 1, "minimized": false},
                {"id": "notes", "kind": "notes", "w": 1, "h": 1, "minimized": true}
            ]
        })
    };
    let put = |body: serde_json::Value| {
        s.escrever(reqwest::Method::PUT, "/me/desktop", &cookie)
            .json(&body)
            .send()
    };
    let r = put(disposicao(0)).await.expect("PUT");
    assert_eq!(r.status().as_u16(), 200);
    let v: serde_json::Value = r.json().await.expect("json");
    assert_eq!(v["version"], 1);

    // Outra janela, que leu a versão 0, chega tarde: conflito, e não sobrepõe.
    let r = put(disposicao(0)).await.expect("PUT obsoleto");
    assert_eq!(r.status().as_u16(), 409);

    // O Desktop desenha o que ficou gravado: ordem, recolhido, fundo, versão.
    let (_, html) = s.html("/", &cookie).await;
    assert_eq!(kinds(&html), ["notice", "notes"]);
    assert!(html.contains(r#"data-version="1""#));
    assert_eq!(wall(&html), "mist");
    assert!(html.contains(r#"data-wall="mist" data-dim="35" class="oc-desk""#));
    // A casca de outra página leva o mesmo fundo.
    let (_, notas) = s.html("/notes", &cookie).await;
    assert_eq!(wall(&notas), "mist");

    // Uma disposição sem o obrigatório é recusada pelo Core.
    let mut sem_aviso = disposicao(1);
    sem_aviso["widgets"] = serde_json::json!([{"id": "notes", "kind": "notes", "w": 1, "h": 1}]);
    assert_eq!(put(sem_aviso).await.expect("PUT").status().as_u16(), 422);

    // Repor: volta à predefinição da distribuição.
    let r = s
        .escrever(reqwest::Method::POST, "/me/desktop/restore", &cookie)
        .header("accept", "application/json")
        .send()
        .await
        .expect("repor");
    assert_eq!(r.status().as_u16(), 200);
    let (_, html) = s.html("/", &cookie).await;
    assert_eq!(
        kinds(&html),
        ["kpis", "calendar", "notice", "continue", "tasks", "projects", "ideas", "storage"]
    );
    // Sem JS, repor é um formulário que volta ao Desktop.
    let r = s
        .escrever(reqwest::Method::POST, "/me/desktop/restore", &cookie)
        .send()
        .await
        .expect("repor sem JS");
    assert_eq!(location(&r), "/");
}

#[tokio::test]
async fn a_disposicao_de_um_membro_nao_e_a_de_outro() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (_, a) = s.membro_com_sessao(&[TechnicalRole::ResearchMember]).await;
    let (_, b) = s.membro_com_sessao(&[TechnicalRole::ResearchMember]).await;
    let r = s
        .escrever(reqwest::Method::PUT, "/me/desktop", &a)
        .json(
            &serde_json::json!({"version": 0, "wallpaper": "sand", "fit": "fill", "dim": 0,
            "widgets": [{"id": "notice", "kind": "notice", "w": 2, "h": 1}]}),
        )
        .send()
        .await
        .expect("PUT de A");
    assert_eq!(r.status().as_u16(), 200);
    let (_, html_b) = s.html("/", &b).await;
    assert_eq!(wall(&html_b), "ocinye", "o Desktop de A apareceu a B");
    assert!(html_b.contains(r#"data-version="0""#));
}

#[tokio::test]
async fn as_notas_do_membro_chegam_aos_widgets() {
    let Some(s) = Sistema::levantar("personal").await else {
        return;
    };
    let (_, email, password) = s.pessoa(&[TechnicalRole::ResearchMember]).await;
    let (_, _, cookie) = s.entrar(&email, &password).await;
    // A nota nasce pela API do Core, com a sessão da própria pessoa.
    let token = s
        .http
        .post(format!("{}/api/v1/auth/login", s.core_url))
        .json(&serde_json::json!({ "email": email, "password": password }))
        .send()
        .await
        .expect("login no Core")
        .json::<serde_json::Value>()
        .await
        .expect("sessão do Core")["session_token"]
        .as_str()
        .expect("token")
        .to_owned();
    let titulo = format!("Nota {}", Uuid::new_v4().simple());
    let r = s
        .http
        .post(format!("{}/api/v1/me/notes", s.core_url))
        .bearer_auth(&token)
        .json(&serde_json::json!({
            "title": titulo,
            "document": { "schema_version": 1, "blocks": [{ "type": "paragraph", "content": [] }] }
        }))
        .send()
        .await
        .expect("criar nota");
    assert!(r.status().is_success(), "{}", r.status());
    let (_, html) = s.html("/", &cookie).await;
    // Personal: Notas e Continuar trabalho estão na predefinição.
    assert!(kinds(&html).contains(&"notes".to_owned()));
    assert!(
        html.matches(&titulo).count() >= 2,
        "a nota não chegou às Notas e ao Continuar"
    );
    assert!(
        html.contains("NOTA"),
        "o tipo do item de Continuar não se compôs"
    );
}

#[tokio::test]
async fn as_aplicacoes_sem_ecra_sao_janelas_honestas_e_as_fechadas_nao_existem() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (_, cookie) = s.membro_com_sessao(&[TechnicalRole::ResearchMember]).await;
    for rota in [
        "/notes",
        "/files",
        "/my-work",
        "/calendar",
        "/notifications",
        "/help",
    ] {
        let (status, html) = s.html(rota, &cookie).await;
        assert_eq!(status, 200, "{rota}");
        assert!(html.contains("oc-app-pending"), "{rota} não é app_pending");
        assert!(html.contains("oc-window"), "{rota} sem janela");
    }
    // Um membro sem administração não vê a consola, nem por rota escrita à mão.
    for rota in ["/admin", "/admin/monitor", "/audit"] {
        let (status, _) = s.html(rota, &cookie).await;
        assert_eq!(status, 404, "{rota} existe para quem não a pode abrir");
    }
    let (_, admin_cookie) = {
        let (id, email, password) = s.pessoa(&[TechnicalRole::PlatformAdmin]).await;
        s.totp_confirmado(id).await;
        let (_, _, c) = s.entrar(&email, &password).await;
        let r = s
            .escrever(reqwest::Method::POST, "/mfa/challenge", &c)
            .form(&[("code", codigo_totp(SEMENTE_MFA).as_str())])
            .send()
            .await
            .expect("desafio");
        let novo = r
            .headers()
            .get_all("set-cookie")
            .iter()
            .filter_map(|v| v.to_str().ok())
            .find(|c| c.starts_with(&format!("{}=", ocinye_workspace::session::COOKIE_NAME)))
            .map(|c| c.split(';').next().unwrap_or_default().to_owned())
            .expect("sessão depois do MFA");
        (id, novo)
    };
    let (status, html) = s.html("/admin/monitor", &admin_cookie).await;
    assert_eq!(status, 200);
    assert!(html.contains("oc-app-pending"));
}

/// Um Core falso: `/ready` com o `overall` dado, e 500 em tudo o resto. Serve
/// para o que o Core verdadeiro não faz de propósito: não saber quem é a sessão.
async fn core_falso(overall: &'static str) -> String {
    let core = axum::Router::new()
        .route(
            "/ready",
            axum::routing::get(move || async move {
                axum::Json(serde_json::json!({
                    "overall": overall, "contract_version": 1, "components": []
                }))
            }),
        )
        .fallback(|| async { StatusCode::INTERNAL_SERVER_ERROR });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("porto");
    let url = format!(
        "http://127.0.0.1:{}",
        listener.local_addr().expect("endereço").port()
    );
    tokio::spawn(async move {
        let _ = axum::serve(listener, core).await;
    });
    url
}

/// O Workspace verdadeiro, com uma sessão local já aberta, à frente de `core_url`.
fn workspace_com_sessao(core_url: &str) -> (axum::Router, String) {
    let ws = workspace_state(core_url, "http://127.0.0.1");
    let id = ws.sessions.create(ocinye_workspace::session::Session {
        access_token: "t".to_owned(),
        display_name: "Ana".to_owned(),
        email: "a@ocinye.com".to_owned(),
        must_change_password: false,
        mfa_required: false,
        expires_at: std::time::Instant::now() + Duration::from_secs(60),
    });
    (
        workspace_routes::router(ws),
        format!("oc_boot=1; {}={id}", ocinye_workspace::session::COOKIE_NAME),
    )
}

async fn em_memoria(
    app: axum::Router,
    request: axum::http::Request<axum::body::Body>,
) -> (u16, String) {
    use tower::ServiceExt;
    let resposta = app.oneshot(request).await.expect("resposta");
    let estado = resposta.status().as_u16();
    let corpo = axum::body::to_bytes(resposta.into_body(), 1 << 20)
        .await
        .expect("corpo");
    (estado, String::from_utf8_lossy(&corpo).into_owned())
}

#[tokio::test]
async fn o_core_sem_resposta_a_identidade_falha_fechado() {
    let (app, cookie) = workspace_com_sessao(&core_falso("ready").await);
    let (estado, html) = em_memoria(
        app,
        axum::http::Request::builder()
            .uri("/notes")
            .header("cookie", cookie)
            .body(axum::body::Body::empty())
            .expect("pedido"),
    )
    .await;
    assert_eq!(estado, 503);
    assert!(
        !html.contains("oc-top")
            && !html.contains("oc-dock")
            && !html.contains(r#"data-oc="desk""#),
        "desenhou a casca"
    );
    assert!(
        html.contains(r#"data-part="identity""#),
        "não é a página de identidade do D001.1"
    );
    assert!(
        html.contains(r#"action="/logout""#),
        "sem «Terminar sessão»"
    );
    assert!(
        html.contains(r#"href="/notes""#),
        "«Tentar de novo» não repete a rota pedida"
    );
    assert!(html.contains("OC-"), "sem referência para o administrador");
}

#[tokio::test]
async fn uma_avaria_do_core_numa_accao_e_um_502_com_referencia() {
    let (app, cookie) = workspace_com_sessao(&core_falso("ready").await);
    let (estado, html) = em_memoria(
        app,
        axum::http::Request::builder()
            .method("POST")
            .uri(format!("/admin/members/{}/status", Uuid::new_v4()))
            .header("cookie", cookie)
            .header("origin", "http://127.0.0.1")
            .header("accept", "text/html")
            .header("content-type", "application/x-www-form-urlencoded")
            .body(axum::body::Body::from("status=suspended&reason=x"))
            .expect("pedido"),
    )
    .await;
    assert_eq!(estado, 502);
    assert!(html.contains("ERRO 502") && html.contains("OC-"), "{html}");
    // Sem identidade confirmada, a página é a da porta — nunca a casca.
    assert!(!html.contains("oc-top"));
    // Um formulário não se reenvia: sem «Tentar de novo».
    assert!(!html.contains("Tentar de novo"));
    assert!(!html.to_lowercase().contains("internal server error"));
}

#[tokio::test]
async fn a_porta_diz_operacional_com_o_ready_degradado() {
    let (app, _) = workspace_com_sessao(&core_falso("degraded").await);
    let (estado, html) = em_memoria(
        app,
        axum::http::Request::builder()
            .uri("/login")
            .header("cookie", "oc_boot=1")
            .body(axum::body::Body::empty())
            .expect("pedido"),
    )
    .await;
    assert_eq!(estado, 200);
    assert!(html.contains("INSTÂNCIA OCINYE OS · OPERACIONAL"));
    let (app, _) = workspace_com_sessao(&core_falso("blocked").await);
    let (_, html) = em_memoria(
        app,
        axum::http::Request::builder()
            .uri("/login")
            .header("cookie", "oc_boot=1")
            .body(axum::body::Body::empty())
            .expect("pedido"),
    )
    .await;
    assert!(
        !html.contains("OPERACIONAL"),
        "uma Instância bloqueada não é operacional"
    );
}

#[tokio::test]
async fn um_caminho_que_nao_existe_e_um_404_do_design_na_casca_e_a_porta() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (_, cookie) = s.membro_com_sessao(&[TechnicalRole::ResearchMember]).await;
    let (status, html) = s.html("/nao-existe", &cookie).await;
    assert_eq!(status, 404);
    assert!(
        html.contains("ERRO 404") && html.contains("oc-top"),
        "sem casca para quem tem sessão"
    );
    let (status, html) = s.html("/nao-existe", "").await;
    assert_eq!(status, 404);
    assert!(html.contains("ERRO 404") && html.contains(r#"data-part="error""#));
    assert!(!html.contains("oc-top"));
    // A aplicação que o membro não abre é igual a uma rota que não existe.
    let (status, html) = s.html("/admin", &cookie).await;
    assert_eq!(status, 404);
    assert!(html.contains("ERRO 404") && !html.contains("oc-app-pending"));
}

#[tokio::test]
async fn uma_recusa_do_core_numa_accao_e_um_403_do_design() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (id, cookie) = s.membro_com_sessao(&[TechnicalRole::ResearchMember]).await;
    let r = s
        .escrever(
            reqwest::Method::POST,
            &format!("/admin/members/{id}/status"),
            &cookie,
        )
        .header("accept", "text/html")
        .form(&[("status", "suspended"), ("reason", "viagem")])
        .send()
        .await
        .expect("POST");
    let status = r.status().as_u16();
    let html = r.text().await.unwrap_or_default();
    // O Core recusa a operação a quem não administra pessoas: 403 real.
    assert_eq!(status, 403);
    assert!(
        html.contains(&format!("ERRO {status}")) && html.contains("oc-top"),
        "{html}"
    );
}

#[tokio::test]
async fn o_estado_do_sistema_diz_copia_sem_registo_e_so_o_admin_abre_o_monitor() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (_, cookie) = s.membro_com_sessao(&[TechnicalRole::ResearchMember]).await;
    let r = s
        .escrever(reqwest::Method::PUT, "/me/desktop", &cookie)
        .json(
            &serde_json::json!({"version": 0, "wallpaper": "ocinye", "fit": "fill", "dim": 20,
            "widgets": [{"id": "notice", "kind": "notice", "w": 2, "h": 1},
                        {"id": "health", "kind": "health", "w": 2, "h": 1}]}),
        )
        .send()
        .await
        .expect("PUT");
    assert_eq!(r.status().as_u16(), 200);
    let (_, html) = s.html("/", &cookie).await;
    assert!(
        html.contains("cópia sem registo"),
        "Backup::Unknown não chegou ao widget"
    );
    assert!(!html.contains("cópia falhou") && !html.contains("sem cópia"));
    assert!(
        !html.contains(r#"href="/admin/monitor""#),
        "o Monitor aparece a quem não administra"
    );
}

#[tokio::test]
async fn a_predefinicao_do_sistema_nao_se_apresenta_como_publicada() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (_, cookie) = s.membro_com_sessao(&[TechnicalRole::ResearchMember]).await;
    let (_, html) = s.html("/", &cookie).await;
    assert!(html.contains(r#"data-source="system""#));
    assert!(html.contains("A administração da Instância ainda não publicou"));
    assert!(!html.contains("publicada a") && !html.contains("Desktop Default"));
    assert!(!html.contains("A administração publicou uma nova predefinição"));
}
