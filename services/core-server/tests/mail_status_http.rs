//! O que `GET /api/v1/mail/status` responde, e sobre quem.
//!
//! # Duas perguntas viviam numa só
//!
//! «Esta instalação fala com o serviço de correio?» e «esta pessoa ligou a sua
//! caixa?» são factos diferentes, e a resposta era sempre a primeira.
//!
//! Desde o [ADR-0409] a primeira já não descreve ninguém. Uma instalação pode
//! ter transporte configurado e **nenhuma** conta de serviço institucional —
//! ninguém guarda uma senha que abre a caixa de toda a gente — e nesse caso o
//! adaptador institucional está ausente por decisão. Responder com ele diria
//! «indisponível» a quem tem a caixa a funcionar, e diria «disponível» a quem
//! ainda não a ligou.
//!
//! # Porque isto é um teste de HTTP e não do módulo
//!
//! Porque o defeito não estava no módulo: estava em **qual** adaptador o
//! handler escolhia. Um teste do serviço de correio não o veria — passaria com
//! o handler a perguntar à instituição, que foi exactamente o estado anterior.
//!
//! [ADR-0409]: ../../../docs/adrs/0409-mailbox-credentials-per-member.md

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
use serde_json::Value;
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

        // Antes da primeira escrita, e não depois: falhar depois de escrever
        // não é uma guarda, é um relatório de estragos.
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

fn state(pool: PgPool, organisation_id: Uuid, transporte: bool) -> AppState {
    estado_com(
        pool,
        organisation_id,
        transporte,
        Arc::new(UnconfiguredProvider),
    )
}

/// O mesmo, com um adaptador institucional à escolha.
///
/// Existe para poder haver um institucional **saudável**. Sem ele, o
/// institucional desta suite dizia sempre `can_read: false`, e uma reversão que
/// fizesse o handler responder por ele continuava verde: os dois davam falso
/// pela mesma razão, e o teste não distinguia qual tinha sido consultado.
fn estado_com(
    pool: PgPool,
    organisation_id: Uuid,
    transporte: bool,
    institucional: Arc<dyn ocinye_core::modules::mail::MailProvider>,
) -> AppState {
    let mut config = config();

    // O transporte é uma decisão de instalação, e o teste tem de poder
    // exprimir as duas. Sem isto, esta suite só saberia falar da instalação
    // que a máquina onde corre por acaso tem.
    if transporte {
        config.mail.imap_host = "mail.exemplo.test".to_owned();
        config.mail.smtp_host = "mail.exemplo.test".to_owned();
    } else {
        config.mail.imap_host = String::new();
        config.mail.smtp_host = String::new();
    }
    // Sem conta de serviço, nos dois casos: é a instalação que queremos
    // descrever, e a que expôs o defeito.
    config.mail.username = String::new();
    config.mail.password = String::new();

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
        institucional,
        config.mail.clone(),
        config.mail.sealing_key.clone(),
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
        .bind(format!("mst-{}", Uuid::new_v4().simple()))
        .bind("Instituição do estado de correio")
        .fetch_one(pool)
        .await
        .expect("organização")
}

/// Uma pessoa que pode usar o correio, e a sua sessão.
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
             VALUES ($1, $2, $3, now() + interval '1 hour', 'mail-status-test')",
    )
    .bind(person_id)
    .bind(identity::session_digest(&token))
    .bind(SessionState::Active.as_str())
    .execute(pool)
    .await
    .expect("sessão");

    (person_id, token)
}

/// Uma caixa pessoal, por ligar.
async fn caixa(pool: &PgPool, organisation_id: Uuid, person_id: Uuid) -> String {
    let endereco = format!("c{}@ocinye.com", Uuid::new_v4().simple());
    sqlx::query(
        "INSERT INTO mailboxes (organisation_id, owner_id, address, kind, display_name)
             VALUES ($1, $2, $3, 'personal', 'Caixa de teste')",
    )
    .bind(organisation_id)
    .bind(person_id)
    .bind(&endereco)
    .execute(pool)
    .await
    .expect("caixa");
    endereco
}

async fn estado(state: &AppState, token: &Secret) -> (StatusCode, Value) {
    let response = routes::router(state.clone())
        .oneshot(
            Request::builder()
                .uri("/api/v1/mail/status")
                .header(header::AUTHORIZATION, format!("Bearer {}", token.expose()))
                .body(Body::empty())
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

/// Uma caixa por ligar não é um serviço em baixo.
///
/// # O defeito que isto guarda
///
/// O handler respondia com a saúde do adaptador **institucional**. Numa
/// instalação sem conta de serviço esse adaptador está ausente por decisão, e
/// a resposta descrevia a ausência da conta em vez do estado desta pessoa.
///
/// Este teste falha se alguém voltar a responder pela instituição: sem conta
/// de serviço, `can_read` viria do `UnconfiguredProvider` — que também dá
/// `false` — mas `mailbox_linked` não existiria, e é ele que diz a quem lê o
/// que tem de fazer a seguir.
#[tokio::test]
async fn uma_caixa_por_ligar_declara_se_como_tal() {
    let pool = pool!();
    let organisation_id = organisation(&pool).await;
    let state = state(pool.clone(), organisation_id, true);
    let (person_id, token) = membro(&pool, organisation_id).await;
    caixa(&pool, organisation_id, person_id).await;

    let (status, corpo) = estado(&state, &token).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        corpo["transport_configured"], true,
        "a instalação sabe onde é o serviço e o estado não o diz: {corpo}"
    );
    assert_eq!(
        corpo["mailbox_linked"], false,
        "a caixa não tem credencial e o estado diz que está ligada: {corpo}"
    );
    assert_eq!(corpo["can_read"], false);
    assert_eq!(corpo["can_send"], false);

    let detalhe = corpo["detail"].as_str().unwrap_or_default();
    assert!(
        detalhe.contains("ainda não está ligada"),
        "a razão não diz a quem lê o que falta — diz outra coisa: «{detalhe}»"
    );
    assert!(
        !detalhe.contains("não está configurado"),
        "chamou configuração em falta a uma instalação configurada: «{detalhe}»"
    );
}

/// Sem transporte, a razão volta a ser de configuração.
///
/// A distinção só vale se as duas metades funcionarem: um estado que dissesse
/// sempre «ligue a sua caixa» mandaria alguém tentar ligá-la a um servidor que
/// esta instalação não conhece.
#[tokio::test]
async fn sem_transporte_a_razao_e_de_configuracao() {
    let pool = pool!();
    let organisation_id = organisation(&pool).await;
    let state = state(pool.clone(), organisation_id, false);
    let (person_id, token) = membro(&pool, organisation_id).await;
    caixa(&pool, organisation_id, person_id).await;

    let (status, corpo) = estado(&state, &token).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(corpo["transport_configured"], false);
    assert_eq!(corpo["mailbox_linked"], false);

    let detalhe = corpo["detail"].as_str().unwrap_or_default();
    assert!(
        !detalhe.contains("ainda não está ligada"),
        "ofereceu ligar uma caixa a um servidor que não existe: «{detalhe}»"
    );
}

/// A sonda do harness: aceita, porque não há servidor de correio para
/// perguntar. O que o harness assume fica escrito, em vez de a verificação
/// desaparecer do caminho que estes testes percorrem.
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

/// Um adaptador institucional que diz estar de pé.
///
/// Só `health` responde; tudo o resto recusa. É deliberado: este duplo existe
/// para uma pergunta — «o handler foi buscar a saúde a quem?» — e um duplo que
/// respondesse a mais do que isso deixaria de a fazer.
struct InstitucionalSaudavel;

#[async_trait::async_trait]
impl ocinye_core::modules::mail::MailProvider for InstitucionalSaudavel {
    fn adapter_name(&self) -> &'static str {
        "institucional-saudavel-de-teste"
    }

    async fn health(&self) -> ocinye_core::modules::mail::provider::ProviderHealth {
        ocinye_core::modules::mail::provider::ProviderHealth {
            endpoints: vec!["imap mail.exemplo.test:993".to_owned()],
            can_read: true,
            can_send: true,
            detail: "O serviço de correio está a responder.".to_owned(),
            rejected_credential: false,
        }
    }

    async fn list_messages(
        &self,
        _mailbox_address: &str,
        _folder: ocinye_contracts::MailFolder,
        _cursor: Option<&str>,
        _limit: u32,
    ) -> ocinye_core::modules::mail::provider::ProviderResult<
        ocinye_core::modules::mail::provider::MessagePage,
    > {
        Err(ocinye_core::modules::mail::ProviderError::NotConfigured)
    }

    async fn fetch_message(
        &self,
        _mailbox_address: &str,
        _folder: ocinye_contracts::MailFolder,
        _provider_id: &str,
    ) -> ocinye_core::modules::mail::provider::ProviderResult<
        ocinye_core::modules::mail::provider::FetchedMessage,
    > {
        Err(ocinye_core::modules::mail::ProviderError::NotConfigured)
    }

    async fn fetch_attachment(
        &self,
        _mailbox_address: &str,
        _folder: ocinye_contracts::MailFolder,
        _provider_id: &str,
        _part_id: &str,
    ) -> ocinye_core::modules::mail::provider::ProviderResult<Vec<u8>> {
        Err(ocinye_core::modules::mail::ProviderError::NotConfigured)
    }

    async fn send_message(
        &self,
        _mailbox_address: &str,
        _message: &ocinye_core::modules::mail::provider::OutgoingMessage,
    ) -> ocinye_core::modules::mail::provider::ProviderResult<Option<String>> {
        Err(ocinye_core::modules::mail::ProviderError::NotConfigured)
    }

    async fn move_message(
        &self,
        _mailbox_address: &str,
        _folder: ocinye_contracts::MailFolder,
        _provider_id: &str,
        _destination: ocinye_contracts::MailFolder,
    ) -> ocinye_core::modules::mail::provider::ProviderResult<()> {
        Err(ocinye_core::modules::mail::ProviderError::NotConfigured)
    }

    async fn set_read(
        &self,
        _mailbox_address: &str,
        _folder: ocinye_contracts::MailFolder,
        _provider_id: &str,
        _read: bool,
    ) -> ocinye_core::modules::mail::provider::ProviderResult<()> {
        Err(ocinye_core::modules::mail::ProviderError::NotConfigured)
    }

    async fn set_starred(
        &self,
        _mailbox_address: &str,
        _folder: ocinye_contracts::MailFolder,
        _provider_id: &str,
        _starred: bool,
    ) -> ocinye_core::modules::mail::provider::ProviderResult<()> {
        Err(ocinye_core::modules::mail::ProviderError::NotConfigured)
    }
}

/// A saúde da instituição não é a saúde de quem perguntou.
///
/// # O defeito que isto guarda, e que os outros dois não guardavam
///
/// O handler respondia com a saúde do adaptador institucional. Nas instalações
/// que os outros testes descrevem esse adaptador também dá `false`, e por isso
/// uma reversão que voltasse a perguntar-lhe continuava verde — os dois
/// caminhos davam a mesma resposta por razões diferentes.
///
/// Aqui a instituição diz **sim** e a pessoa não tem caixa ligada. Só uma das
/// duas respostas é verdade sobre ela.
#[tokio::test]
async fn a_saude_da_instituicao_nao_e_a_de_quem_perguntou() {
    let pool = pool!();
    let organisation_id = organisation(&pool).await;
    let state = estado_com(
        pool.clone(),
        organisation_id,
        true,
        Arc::new(InstitucionalSaudavel),
    );
    let (person_id, token) = membro(&pool, organisation_id).await;
    caixa(&pool, organisation_id, person_id).await;

    let (status, corpo) = estado(&state, &token).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        corpo["mailbox_linked"], false,
        "a caixa desta pessoa não tem credencial: {corpo}"
    );
    assert_eq!(
        corpo["can_read"], false,
        "respondeu com a saúde da instituição a uma pergunta sobre esta pessoa: {corpo}"
    );
    assert_eq!(
        corpo["can_send"], false,
        "respondeu com a saúde da instituição a uma pergunta sobre esta pessoa: {corpo}"
    );
}
