//! O que `POST /api/v1/auth/machine-session` assina, e para quem.
//!
//! Uma máquina com o Ocinye OS abre uma sessão local quando o Core lhe entrega
//! uma política **assinada** para ela (ADR-0031). O Session Service da máquina
//! só aceita o que verifica com a chave pública da Instância — por isso o que
//! conta aqui não é o corpo parecer certo: é a assinatura verificar com a chave
//! que `GET /auth/machine-session/key` publica, sobre os bytes que o contrato
//! define, e deixar de verificar quando qualquer coisa muda.
//!
//! # Porque isto é um teste de HTTP
//!
//! Porque quem decide se uma sessão recebe política é o extractor da rota:
//! revogada dá `401`, restrita dá `403`. Um teste da função de assinatura não
//! veria nenhuma das duas recusas.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use ed25519_dalek::{Signature, VerifyingKey};
use ocinye_contracts::{SessionState, TechnicalRole};
use ocinye_core::authn::TokenVerifier;
use ocinye_core::config::CoreConfig;
use ocinye_core::modules::identity::{self, Authenticator, Throttle};
use ocinye_core::modules::mail::provider::UnconfiguredProvider;
use ocinye_core::password::sealed::SealingKey;
use ocinye_core::password::{Hasher, HashingParams, Secret};
use ocinye_core_server::routes;
use ocinye_core_server::state::AppState;
use ocinye_system_contracts::session::SignedSessionPolicy;
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

/// O estado, com ou sem raiz de selagem — que é o que decide se há chave.
fn state(pool: PgPool, organisation_id: Uuid, raiz: Option<&str>) -> AppState {
    let mut config = config();
    config.sealing_key = raiz.map(|valor| SealingKey::from_base64(valor).expect("raiz"));

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

async fn organisation(pool: &PgPool) -> Uuid {
    sqlx::query_scalar("INSERT INTO organisations (slug, name) VALUES ($1, $2) RETURNING id")
        .bind(format!("msn-{}", Uuid::new_v4().simple()))
        .bind("Instituição das sessões de máquina")
        .fetch_one(pool)
        .await
        .expect("organização")
}

/// Uma pessoa e uma sessão sua, no estado pedido.
async fn membro(pool: &PgPool, organisation_id: Uuid, estado: SessionState) -> (Uuid, Secret) {
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

    let token = Secret::new(format!(
        "{}{}",
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple()
    ));
    sqlx::query(
        "INSERT INTO sessions (person_id, token_digest, state, expires_at, user_agent)
             VALUES ($1, $2, $3, now() + interval '1 hour', 'machine-session-test')",
    )
    .bind(person_id)
    .bind(identity::session_digest(&token))
    .bind(estado.as_str())
    .execute(pool)
    .await
    .expect("sessão");

    (person_id, token)
}

async fn pedir(
    state: &AppState,
    metodo: Method,
    caminho: &str,
    token: Option<&Secret>,
    corpo: Option<Value>,
) -> (StatusCode, Value) {
    let mut pedido = Request::builder().method(metodo).uri(caminho);
    if let Some(token) = token {
        pedido = pedido.header(header::AUTHORIZATION, format!("Bearer {}", token.expose()));
    }
    let pedido = match corpo {
        Some(corpo) => pedido
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(corpo.to_string())),
        None => pedido.body(Body::empty()),
    }
    .expect("pedido");
    let response = routes::router(state.clone())
        .oneshot(pedido)
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

const POLITICA: &str = "/api/v1/auth/machine-session";
const CHAVE: &str = "/api/v1/auth/machine-session/key";

async fn chave_publica(state: &AppState) -> VerifyingKey {
    let (status, corpo) = pedir(state, Method::GET, CHAVE, None, None).await;
    assert_eq!(status, StatusCode::OK, "{corpo}");
    let bytes: [u8; 32] = hex::decode(corpo["public_key"].as_str().expect("chave"))
        .expect("hexadecimal")
        .try_into()
        .expect("32 bytes");
    VerifyingKey::from_bytes(&bytes).expect("chave Ed25519")
}

fn verifica(chave: &VerifyingKey, assinada: &SignedSessionPolicy) -> bool {
    let bruta: [u8; 64] = hex::decode(&assinada.signature)
        .expect("hexadecimal")
        .try_into()
        .expect("64 bytes");
    chave
        .verify_strict(
            &assinada.policy.signed_bytes(),
            &Signature::from_bytes(&bruta),
        )
        .is_ok()
}

/// A política vem assinada pela chave que a Instância publica, nomeia a
/// máquina que a pediu e a sessão que a pediu — e a assinatura cobre tudo isso.
#[tokio::test]
async fn a_politica_verifica_com_a_chave_publicada_e_nomeia_a_maquina() {
    let pool = pool!();
    let organisation_id = organisation(&pool).await;
    let raiz = SealingKey::generate();
    let state = state(pool.clone(), organisation_id, Some(&raiz));
    let (person_id, token) = membro(&pool, organisation_id, SessionState::Active).await;
    let chave = chave_publica(&state).await;

    let (status, corpo) = pedir(
        &state,
        Method::POST,
        POLITICA,
        Some(&token),
        Some(json!({ "machine_id": "0123456789abcdef0123456789abcdef" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{corpo}");
    let assinada: SignedSessionPolicy = serde_json::from_value(corpo).expect("o contrato");

    assert!(verifica(&chave, &assinada), "a assinatura não verifica");
    assert_eq!(assinada.policy.member_id, person_id.to_string());
    assert_eq!(
        assinada.policy.machine_id,
        "0123456789abcdef0123456789abcdef"
    );
    assert!(assinada.policy.max_unlock_attempts > 0);
    let sessao: Uuid =
        sqlx::query_scalar("SELECT id FROM sessions WHERE person_id = $1 AND state = 'active'")
            .bind(person_id)
            .fetch_one(&pool)
            .await
            .expect("sessão");
    assert_eq!(assinada.policy.session_id, sessao.to_string());

    // A assinatura cobre a máquina: a mesma política com outra máquina escrita
    // por cima deixa de verificar.
    let mut desviada = assinada.clone();
    desviada.policy.machine_id = "outra-maquina".to_owned();
    assert!(!verifica(&chave, &desviada));
    // E cobre as regras: quem alargar o prazo sem o Core fica sem assinatura.
    let mut alargada = assinada;
    alargada.policy.max_offline_secs = None;
    assert!(!verifica(&chave, &alargada));

    // Outra raiz, outra chave: uma Instância não verifica o que outra assinou.
    let outra = self::state(pool, organisation_id, Some(&SealingKey::generate()));
    assert_ne!(chave_publica(&outra).await, chave);
}

/// Sem raiz de selagem não há chave, e sem chave não se assina — nem se
/// publica uma chave inventada para a ocasião.
#[tokio::test]
async fn sem_raiz_de_selagem_nada_e_assinado() {
    let pool = pool!();
    let organisation_id = organisation(&pool).await;
    let state = state(pool.clone(), organisation_id, None);
    let (_, token) = membro(&pool, organisation_id, SessionState::Active).await;

    let (status, _) = pedir(&state, Method::GET, CHAVE, None, None).await;
    assert!(
        !status.is_success(),
        "publicou uma chave sem raiz: {status}"
    );
    let (status, corpo) = pedir(
        &state,
        Method::POST,
        POLITICA,
        Some(&token),
        Some(json!({ "machine_id": "maquina-a" })),
    )
    .await;
    assert!(!status.is_success(), "assinou sem raiz: {status}");
    assert!(corpo.get("signature").is_none());
}

/// Quem não tem sessão, quem a perdeu e quem ainda deve uma palavra-passe ou
/// um segundo factor não recebe política: é assim que a máquina sabe.
#[tokio::test]
async fn so_uma_sessao_de_trabalho_viva_recebe_politica() {
    let pool = pool!();
    let organisation_id = organisation(&pool).await;
    let state = state(pool.clone(), organisation_id, Some(&SealingKey::generate()));
    let corpo = || Some(json!({ "machine_id": "maquina-a" }));

    let (status, _) = pedir(&state, Method::POST, POLITICA, None, corpo()).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    for restrita in [
        SessionState::PasswordChangeRequired,
        SessionState::MfaRequired,
    ] {
        let (_, token) = membro(&pool, organisation_id, restrita).await;
        let (status, _) = pedir(&state, Method::POST, POLITICA, Some(&token), corpo()).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{restrita:?}");
    }

    let (person_id, token) = membro(&pool, organisation_id, SessionState::Active).await;
    let (status, _) = pedir(&state, Method::POST, POLITICA, Some(&token), corpo()).await;
    assert_eq!(status, StatusCode::OK);
    identity::revoke_all_sessions(&pool, person_id, "teste")
        .await
        .expect("revogação");
    let (status, _) = pedir(&state, Method::POST, POLITICA, Some(&token), corpo()).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "uma sessão revogada continuou a receber política"
    );
}

/// O identificador da máquina é texto de quem pede: só entra na política o que
/// tem a forma de um identificador.
#[tokio::test]
async fn um_identificador_de_maquina_sem_forma_e_recusado() {
    let pool = pool!();
    let organisation_id = organisation(&pool).await;
    let state = state(pool.clone(), organisation_id, Some(&SealingKey::generate()));
    let (_, token) = membro(&pool, organisation_id, SessionState::Active).await;

    for mau in ["", "   ", "a b", "maquina\n", &"x".repeat(65)] {
        let (status, corpo) = pedir(
            &state,
            Method::POST,
            POLITICA,
            Some(&token),
            Some(json!({ "machine_id": mau })),
        )
        .await;
        assert!(status.is_client_error(), "«{mau}» → {status} {corpo}");
        assert!(corpo.get("signature").is_none());
    }
    let (status, _) = pedir(
        &state,
        Method::POST,
        POLITICA,
        Some(&token),
        Some(json!({ "machine_id": "maquina-a", "offline_unlock": true })),
    )
    .await;
    assert!(
        status.is_client_error(),
        "aceitou um campo que ninguém definiu"
    );
}
