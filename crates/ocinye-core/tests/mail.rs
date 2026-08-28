//! End-to-end tests of Ocinye Mail against real PostgreSQL.
//!
//! # Why a recording provider and never a real mailbox
//!
//! A suite that sends real email is a suite that cannot run in CI, cannot run
//! twice, and eventually sends something to somebody (briefing §92).
//! [`RecordingProvider`] answers like a mail service and remembers what it was
//! asked to do, which is exactly what these tests need to check.
//!
//! It lives here, in `tests/`, and not in the crate: a fixture that ships
//! inside the library is a fixture that can end up wired into a running
//! deployment.
//!
//! # What is proved here
//!
//! Three things that unit tests cannot reach, because each needs the database,
//! the policy and the provider at the same time:
//!
//! 1. **No administrative role reaches a personal mailbox** — the founding
//!    invariant of ADR-0404.
//! 2. **`RESTRICTED` does not leave the institution, and confirming does not
//!    change that** — ADR-0403.
//! 3. **Assistance never reaches the provider** — ADR-0406. The recording
//!    provider is what makes this observable rather than merely asserted.
//!
//! These skip when `OCINYE_TEST_DATABASE_URL` is unset and **fail** when it is
//! set but unreachable (see `docs/testing/`).

use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use ocinye_contracts::{
    Classification, ComposeAction, MailAddress, MailFolder, SystemCapabilities,
};
use ocinye_core::modules::mail::provider::{
    FetchedMessage, MailProvider, MessageHeader, MessagePage, OutgoingMessage, ProviderAddress,
    ProviderError, ProviderHealth, ProviderResult,
};
use ocinye_core::modules::mail::{self, SendDecision};
use ocinye_core::CoreError;
use ocinye_observability::CorrelationIds;
use sqlx::PgPool;
use uuid::Uuid;

/// The institutional domain used throughout this suite.
const DOMAIN: &str = "ocinye.com";

/// A mail provider that answers and remembers, and sends nothing anywhere.
#[derive(Default)]
struct RecordingProvider {
    /// How many messages it was asked to send.
    ///
    /// The number that matters is zero: several tests exist to prove a path
    /// leaves it there.
    sends: AtomicUsize,
}

impl RecordingProvider {
    fn sends(&self) -> usize {
        self.sends.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl MailProvider for RecordingProvider {
    fn adapter_name(&self) -> &'static str {
        "recording"
    }

    async fn health(&self) -> ProviderHealth {
        ProviderHealth {
            endpoints: vec!["recording:0".to_owned()],
            can_read: true,
            can_send: true,
            detail: "Fornecedor de teste.".to_owned(),
            rejected_credential: false,
        }
    }

    async fn list_messages(
        &self,
        _mailbox_address: &str,
        _folder: MailFolder,
        _cursor: Option<&str>,
        _limit: u32,
    ) -> ProviderResult<MessagePage> {
        Ok(MessagePage {
            messages: Vec::new(),
            next_cursor: None,
        })
    }

    async fn fetch_message(
        &self,
        _mailbox_address: &str,
        _folder: MailFolder,
        _provider_id: &str,
    ) -> ProviderResult<FetchedMessage> {
        Ok(FetchedMessage {
            header: MessageHeader {
                provider_id: "recorded".to_owned(),
                message_id: None,
                thread_key: None,
                folder: MailFolder::Inbox,
                from: ProviderAddress {
                    address: "externo@exemplo.com".to_owned(),
                    display_name: None,
                },
                to: Vec::new(),
                cc: Vec::new(),
                subject: None,
                snippet: None,
                sent_at: chrono::Utc::now(),
                is_read: false,
                is_starred: false,
                has_attachments: false,
                size_bytes: None,
            },
            text_body: None,
            html_body: None,
            attachments: Vec::new(),
            bcc: Vec::new(),
        })
    }

    async fn fetch_attachment(
        &self,
        _mailbox_address: &str,
        _folder: MailFolder,
        _provider_id: &str,
        _part_id: &str,
    ) -> ProviderResult<Vec<u8>> {
        Ok(Vec::new())
    }

    async fn send_message(
        &self,
        _from: &str,
        _message: &OutgoingMessage,
    ) -> ProviderResult<Option<String>> {
        self.sends.fetch_add(1, Ordering::SeqCst);
        Ok(Some("recorded".to_owned()))
    }

    async fn move_message(
        &self,
        _mailbox_address: &str,
        _folder: MailFolder,
        _provider_id: &str,
        _destination: MailFolder,
    ) -> ProviderResult<()> {
        Ok(())
    }

    async fn set_read(
        &self,
        _mailbox_address: &str,
        _folder: MailFolder,
        _provider_id: &str,
        _read: bool,
    ) -> ProviderResult<()> {
        Ok(())
    }

    async fn set_starred(
        &self,
        _mailbox_address: &str,
        _folder: MailFolder,
        _provider_id: &str,
        _starred: bool,
    ) -> ProviderResult<()> {
        Ok(())
    }
}

/// Um registo à volta de um adaptador de teste.
///
/// Sem chave de cifra configurada e sem credenciais guardadas, o registo devolve
/// sempre o adaptador da instituição — que é exactamente o comportamento que
/// estes testes exercitam, e o mesmo que o Ocinye tinha antes de as caixas se
/// poderem ligar (ADR-0409).
fn registo_de(provider: &std::sync::Arc<RecordingProvider>) -> mail::ProviderRegistry {
    registo_com(provider, None)
}

/// O mesmo registo, com uma chave de cifra — e portanto capaz de abrir sessões
/// de membro quando existem credenciais guardadas.
fn registo_com(
    provider: &std::sync::Arc<RecordingProvider>,
    chave: Option<ocinye_core::password::sealed::SealingKey>,
) -> mail::ProviderRegistry {
    mail::ProviderRegistry::new(
        std::sync::Arc::clone(provider) as std::sync::Arc<dyn MailProvider>,
        ocinye_core::config::MailConfig {
            institutional_domains: vec!["ocinye.com".to_owned()],
            // Um transporte que se descreve mas a que nada se liga: estes
            // testes medem qual credencial abre a sessão, e nunca chegam a
            // falar com um servidor.
            imap_host: "imap.exemplo.invalid".to_owned(),
            imap_port: 993,
            imap_security: ocinye_core::config::MailSecurity::ImplicitTls,
            smtp_host: "smtp.exemplo.invalid".to_owned(),
            smtp_port: 587,
            smtp_security: ocinye_core::config::MailSecurity::StartTls,
            username: String::new(),
            password: String::new(),
            max_message_bytes: 25 * 1024 * 1024,
            sealing_key: chave.clone(),
        },
        chave,
    )
}

/// Uma sonda que aceita, e conta quantas vezes foi consultada.
///
/// Declara o que este harness assume: que a credencial abre. Assumi-lo é
/// diferente de não verificar — a contagem prova que o Core perguntou.
#[derive(Default)]
struct SondaQueAceita {
    consultas: AtomicUsize,
}

#[async_trait]
impl ocinye_core::modules::mail::provider::CredentialProbe for SondaQueAceita {
    async fn verify(&self, _endereco: &str, _username: &str, _senha: &str) -> ProviderResult<()> {
        self.consultas.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

/// Uma sonda que recusa, como um servidor que rejeita a senha.
struct SondaQueRecusa;

#[async_trait]
impl ocinye_core::modules::mail::provider::CredentialProbe for SondaQueRecusa {
    async fn verify(&self, _endereco: &str, _username: &str, _senha: &str) -> ProviderResult<()> {
        Err(ProviderError::AuthenticationFailed)
    }
}

/// Connect and migrate, or skip.
async fn pool() -> Option<PgPool> {
    let url = std::env::var("OCINYE_TEST_DATABASE_URL").ok()?;
    let pool = PgPool::connect(&url)
        .await
        .expect("OCINYE_TEST_DATABASE_URL is set but the database is unreachable");
    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .expect("migrations must apply to the test database");
    Some(pool)
}

/// A fresh organisation, so tests never collide.
async fn organisation(pool: &PgPool) -> Uuid {
    let slug = format!("m{}", Uuid::new_v4().simple());
    sqlx::query_scalar("INSERT INTO organisations (slug, name) VALUES ($1, $1) RETURNING id")
        .bind(&slug)
        .fetch_one(pool)
        .await
        .expect("organisation")
}

/// A person with the given technical roles, and their principal.
async fn person(pool: &PgPool, organisation_id: Uuid, roles: &[&str]) -> ocinye_domain::Principal {
    let handle = format!("p{}", Uuid::new_v4().simple());

    let person_id: Uuid = sqlx::query_scalar(
        "INSERT INTO people (organisation_id, full_name, email, status)
              VALUES ($1, $2, $3, 'active') RETURNING id",
    )
    .bind(organisation_id)
    .bind(&handle)
    .bind(format!("{handle}@{DOMAIN}"))
    .fetch_one(pool)
    .await
    .expect("person");

    for role in roles {
        sqlx::query("INSERT INTO person_roles (person_id, role) VALUES ($1, $2)")
            .bind(person_id)
            .bind(*role)
            .execute(pool)
            .await
            .expect("role");
    }

    let record = ocinye_core::modules::identity::person_by_id(pool, person_id)
        .await
        .expect("query")
        .expect("person");

    ocinye_core::modules::identity::principal_for_person(pool, &record)
        .await
        .expect("principal")
}

/// A personal mailbox belonging to one person.
async fn personal_mailbox(pool: &PgPool, organisation_id: Uuid, owner_id: Uuid) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO mailboxes (organisation_id, address, kind, owner_id)
              VALUES ($1, $2, 'personal', $3) RETURNING id",
    )
    .bind(organisation_id)
    .bind(format!("mb{}@{DOMAIN}", Uuid::new_v4().simple()))
    .bind(owner_id)
    .fetch_one(pool)
    .await
    .expect("mailbox")
}

/// One indexed message sitting in a mailbox.
async fn message_in(pool: &PgPool, mailbox_id: Uuid) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO mail_messages
                (mailbox_id, provider_id, folder, from_address, subject, sent_at)
              VALUES ($1, $2, 'inbox', 'externo@exemplo.com', 'Assunto privado', now())
         RETURNING id",
    )
    .bind(mailbox_id)
    .bind(Uuid::new_v4().to_string())
    .fetch_one(pool)
    .await
    .expect("message")
}

fn address(value: &str) -> MailAddress {
    MailAddress::new(value, None, &[DOMAIN.to_owned()])
}

fn outgoing(from: &str, to: &str) -> OutgoingMessage {
    OutgoingMessage {
        from: mail::sender_identity(from, None),
        to: vec![mail::sender_identity(to, None)],
        cc: Vec::new(),
        bcc: Vec::new(),
        subject: "Assunto".to_owned(),
        body: "Corpo.".to_owned(),
        in_reply_to: None,
        references: Vec::new(),
        attachments: Vec::new(),
    }
}

// ── ADR-0404: privacy ───────────────────────────────────────────────────

/// The founding invariant of ADR-0404, exercised through the real service.
///
/// The administrator here holds **both** administrative roles. If any of them
/// were ever wired into a mail query, this fails.
#[tokio::test]
async fn no_administrative_role_reaches_a_personal_mailbox() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;

    let owner = person(&pool, org, &["research_member"]).await;
    let administrator = person(&pool, org, &["platform_admin", "organisation_admin"]).await;

    let mailbox = personal_mailbox(&pool, org, owner.person_id).await;
    let message = message_in(&pool, mailbox).await;
    let provider = std::sync::Arc::new(RecordingProvider::default());
    let registo = registo_de(&provider);
    let ids = CorrelationIds::generate();

    // The owner sees their own mailbox and their own message.
    let theirs = mail::mailboxes(&pool, &owner).await.expect("mailboxes");
    assert_eq!(theirs.len(), 1, "o dono não vê a sua própria caixa");
    assert!(mail::mailbox(&pool, &owner, mailbox).await.is_ok());

    // The administrator sees nothing.
    let seen = mail::mailboxes(&pool, &administrator)
        .await
        .expect("mailboxes");
    assert!(
        seen.is_empty(),
        "um papel administrativo alcançou uma caixa pessoal alheia"
    );

    // And a mailbox that is not theirs reads as *not found*, never as denied:
    // knowing that a closed mailbox exists is already information (ADR-0100).
    match mail::mailbox(&pool, &administrator, mailbox).await {
        Err(CoreError::NotFound(_)) => {}
        other => panic!("esperava NotFound, obtive {other:?}"),
    }

    match mail::read_message(&pool, &registo, &administrator, message, false, &ids).await {
        Err(CoreError::NotFound(_)) => {}
        other => panic!("esperava NotFound ao ler mensagem alheia, obtive {other:?}"),
    }
}

/// Knowing a message identifier is not authority to read the message.
#[tokio::test]
async fn a_message_identifier_is_not_a_key() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;

    let owner = person(&pool, org, &["research_member"]).await;
    let stranger = person(&pool, org, &["research_member"]).await;

    let mailbox = personal_mailbox(&pool, org, owner.person_id).await;
    let message = message_in(&pool, mailbox).await;

    let found = mail::repository::accessible_message(&pool, stranger.person_id, message)
        .await
        .expect("query");
    assert!(found.is_none(), "IDOR: um estranho alcançou a mensagem");

    let found = mail::repository::accessible_message(&pool, owner.person_id, message)
        .await
        .expect("query");
    assert!(found.is_some(), "o dono deixou de alcançar a sua mensagem");
}

// ── ADR-0403: classification on the way out ─────────────────────────────

/// `RESTRICTED` does not leave, and confirming does not change that.
///
/// The recording provider is what makes this a real assertion rather than a
/// hopeful one: after a refusal, nothing was handed to anything.
#[tokio::test]
async fn restricted_material_never_reaches_the_provider() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;

    let member = person(&pool, org, &["research_member"]).await;
    let mailbox_id = personal_mailbox(&pool, org, member.person_id).await;
    let mailbox = mail::mailbox(&pool, &member, mailbox_id)
        .await
        .expect("mailbox");

    let provider = std::sync::Arc::new(RecordingProvider::default());
    let registo = registo_de(&provider);
    let ids = CorrelationIds::generate();
    let recipients = [address("parceiro@exemplo.com")];

    for confirmed in [false, true] {
        let outcome = mail::send(
            &pool,
            &registo,
            &member,
            mailbox_id,
            outgoing(&mailbox.address, "parceiro@exemplo.com"),
            &recipients,
            &[Classification::Restricted],
            confirmed,
            &ids,
        )
        .await;

        assert!(
            outcome.is_err(),
            "material RESTRICTED saiu da instituição (confirmed = {confirmed})"
        );
    }

    assert_eq!(
        provider.sends(),
        0,
        "o fornecedor recebeu uma mensagem que a política recusou"
    );
}

/// Confirmation is consent to a permitted act, and it does work for `INTERNAL`.
#[tokio::test]
async fn confirmation_lets_internal_material_out_and_the_provider_sees_it() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;

    let member = person(&pool, org, &["research_member"]).await;
    let mailbox_id = personal_mailbox(&pool, org, member.person_id).await;
    let mailbox = mail::mailbox(&pool, &member, mailbox_id)
        .await
        .expect("mailbox");

    let provider = std::sync::Arc::new(RecordingProvider::default());
    let registo = registo_de(&provider);
    let ids = CorrelationIds::generate();
    let recipients = [address("parceiro@exemplo.com")];

    // Without confirmation the service refuses and says what to confirm.
    let decision = mail::evaluate_send(
        &pool,
        &member,
        &recipients,
        &[Classification::Internal],
        false,
    )
    .await
    .expect("evaluate");
    assert!(matches!(decision, SendDecision::NeedsConfirmation { .. }));

    let sent = mail::send(
        &pool,
        &registo,
        &member,
        mailbox_id,
        outgoing(&mailbox.address, "parceiro@exemplo.com"),
        &recipients,
        &[Classification::Internal],
        true,
        &ids,
    )
    .await;

    assert!(sent.is_ok(), "envio confirmado foi recusado: {sent:?}");
    assert_eq!(provider.sends(), 1);
}

/// A send is an auditable institutional operation, not a side effect.
#[tokio::test]
async fn sending_leaves_an_audit_trail_without_the_message_in_it() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;

    let member = person(&pool, org, &["research_member"]).await;
    let mailbox_id = personal_mailbox(&pool, org, member.person_id).await;
    let mailbox = mail::mailbox(&pool, &member, mailbox_id)
        .await
        .expect("mailbox");

    let provider = std::sync::Arc::new(RecordingProvider::default());
    let registo = registo_de(&provider);
    let ids = CorrelationIds::generate();

    let mut message = outgoing(&mailbox.address, "colega@ocinye.com");
    message.subject = "SEGREDO-NO-ASSUNTO".to_owned();
    message.body = "SEGREDO-NO-CORPO".to_owned();

    mail::send(
        &pool,
        &registo,
        &member,
        mailbox_id,
        message,
        &[address("colega@ocinye.com")],
        &[],
        false,
        &ids,
    )
    .await
    .expect("send");

    let recorded: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_events
          WHERE actor_person_id = $1 AND resource_type = 'mail_message'",
    )
    .bind(member.person_id)
    .fetch_one(&pool)
    .await
    .expect("audit");
    assert!(recorded > 0, "um envio não deixou rasto de auditoria");

    // The trail records *that* something was sent, never *what*. An audit log
    // holding message bodies is a second copy of the correspondence, under
    // backup (briefing §57, `CLAUDE.md` §37).
    let leaked: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_events
          WHERE actor_person_id = $1
            AND metadata::text LIKE '%SEGREDO-NO-%'",
    )
    .bind(member.person_id)
    .fetch_one(&pool)
    .await
    .expect("audit");
    assert_eq!(leaked, 0, "o conteúdo da mensagem entrou na auditoria");
}

// ── ADR-0406: generated is not sent ─────────────────────────────────────

/// Assistance never reaches the provider.
///
/// The recording provider is not even passed to `assist` — it takes no
/// provider, because it has nothing to say to one. This test exists so that a
/// future signature change that *adds* one is a change somebody has to justify.
#[tokio::test]
async fn assistance_cannot_reach_the_provider() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;

    let member = person(&pool, org, &["research_member"]).await;
    let provider = std::sync::Arc::new(RecordingProvider::default());

    // No AI node is registered, which is this deployment's true state. The
    // assistance says so; it does not fall back to anything.
    let outcome = mail::assist(
        &pool,
        &member,
        &mail::AssistRequest {
            action: ComposeAction::Generate,
            instruction: "Escreve um convite para uma reunião.".to_owned(),
            draft_body: None,
            source_message_id: None,
        },
        &SystemCapabilities {
            capabilities: Vec::new(),
        },
    )
    .await;

    assert!(
        matches!(outcome, Err(CoreError::CapabilityUnavailable(_))),
        "sem nó de IA, a assistência devia declarar-se indisponível: {outcome:?}"
    );

    assert_eq!(
        provider.sends(),
        0,
        "a assistência entregou algo a um fornecedor de correio"
    );
}

// ── Credenciais de caixa ────────────────────────────────────────────────

fn chave() -> ocinye_core::password::sealed::SealingKey {
    ocinye_core::password::sealed::SealingKey::from_base64(
        &ocinye_core::password::sealed::SealingKey::generate(),
    )
    .expect("chave")
}

/// Quem liga a sua caixa passa a poder usá-la, e a senha não fica legível.
#[tokio::test]
async fn ligar_uma_caixa_guarda_a_senha_cifrada() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let alice = person(&pool, org, &["research_member"]).await;
    let caixa = personal_mailbox(&pool, org, alice.person_id).await;

    let k = chave();
    ocinye_core::modules::mail::service::connect_mailbox(
        &pool,
        &alice,
        caixa,
        &mail::service::MailboxConnection {
            chave: Some(&k),
            sonda: &SondaQueAceita::default(),
            senha: "a-senha-do-imap",
        },
        &CorrelationIds::generate(),
    )
    .await
    .expect("ligar");

    // A senha não está em texto em lado nenhum da linha.
    let (nome, cifrado): (String, Vec<u8>) = sqlx::query_as(
        "SELECT username, ciphertext FROM mailbox_credentials WHERE mailbox_id = $1",
    )
    .bind(caixa)
    .fetch_one(&pool)
    .await
    .expect("credencial");

    // A conta guardada é o endereço **da caixa**, e não algo que veio de fora.
    //
    // Esta asserção era `assert_eq!(nome, "alice@ocinye.com")` — o valor que o
    // chamador tinha passado num campo `username`. Provava que o Core guardava
    // o que lhe davam, que é precisamente a propriedade que já não queremos:
    // o browser escolhia a conta com que o Ocinye se autentica no servidor de
    // correio, enquanto o ecrã mostrava outro endereço.
    let endereco: String = sqlx::query_scalar("SELECT address FROM mailboxes WHERE id = $1")
        .bind(caixa)
        .fetch_one(&pool)
        .await
        .expect("endereço");
    assert_eq!(
        nome, endereco,
        "a conta guardada não é o endereço da caixa que foi ligada"
    );

    assert!(
        !String::from_utf8_lossy(&cifrado).contains("a-senha-do-imap"),
        "a senha ficou legível na base de dados"
    );

    // E abre-se com a chave certa.
    let guardada = ocinye_core::modules::mail::repository::credential_of(&pool, caixa)
        .await
        .expect("ler")
        .expect("existe");
    assert_eq!(
        ocinye_core::password::sealed::open(&k, &guardada.sealed).expect("abrir"),
        "a-senha-do-imap"
    );
}

/// Sem chave de cifra, não se guarda nada.
///
/// Guardar em claro «porque ainda não há chave» seria trocar a propriedade que
/// esta funcionalidade existe para ter.
#[tokio::test]
async fn sem_chave_nao_se_liga() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let alice = person(&pool, org, &["research_member"]).await;
    let caixa = personal_mailbox(&pool, org, alice.person_id).await;

    let resultado = ocinye_core::modules::mail::service::connect_mailbox(
        &pool,
        &alice,
        caixa,
        &mail::service::MailboxConnection {
            chave: None,
            sonda: &SondaQueAceita::default(),
            senha: "a-senha",
        },
        &CorrelationIds::generate(),
    )
    .await;

    assert!(resultado.is_err(), "ligou uma caixa sem chave de cifra");

    let quantas: i64 =
        sqlx::query_scalar("SELECT count(*) FROM mailbox_credentials WHERE mailbox_id = $1")
            .bind(caixa)
            .fetch_one(&pool)
            .await
            .expect("contagem");
    assert_eq!(quantas, 0, "guardou uma credencial sem a poder cifrar");
}

/// Ninguém liga a caixa de outra pessoa.
#[tokio::test]
async fn nao_se_liga_a_caixa_de_outra_pessoa() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let dona = person(&pool, org, &["research_member"]).await;
    let outra = person(&pool, org, &["research_member"]).await;
    let caixa = personal_mailbox(&pool, org, dona.person_id).await;

    let resultado = ocinye_core::modules::mail::service::connect_mailbox(
        &pool,
        &outra,
        caixa,
        &mail::service::MailboxConnection {
            chave: Some(&chave()),
            sonda: &SondaQueAceita::default(),
            senha: "a-senha",
        },
        &CorrelationIds::generate(),
    )
    .await;

    assert!(
        resultado.is_err(),
        "alguém escreveu uma credencial na caixa de outra pessoa"
    );
}

/// Desligar esquece a senha.
#[tokio::test]
async fn desligar_esquece_a_credencial() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let alice = person(&pool, org, &["research_member"]).await;
    let caixa = personal_mailbox(&pool, org, alice.person_id).await;

    let ids = CorrelationIds::generate();
    ocinye_core::modules::mail::service::connect_mailbox(
        &pool,
        &alice,
        caixa,
        &mail::service::MailboxConnection {
            chave: Some(&chave()),
            sonda: &SondaQueAceita::default(),
            senha: "a-senha",
        },
        &ids,
    )
    .await
    .expect("ligar");

    ocinye_core::modules::mail::service::disconnect_mailbox(&pool, &alice, caixa, &ids)
        .await
        .expect("desligar");

    let quantas: i64 =
        sqlx::query_scalar("SELECT count(*) FROM mailbox_credentials WHERE mailbox_id = $1")
            .bind(caixa)
            .fetch_one(&pool)
            .await
            .expect("contagem");
    assert_eq!(quantas, 0, "desligar deixou a senha guardada");

    // E a caixa continua a existir para a instituição.
    //
    // `mailboxes.connected` governa outra coisa — se a caixa está anexada — e
    // desligar a credencial não a pode fazer desaparecer da lista, porque então
    // ninguém a voltaria a ligar.
    let anexada: bool = sqlx::query_scalar("SELECT connected FROM mailboxes WHERE id = $1")
        .bind(caixa)
        .fetch_one(&pool)
        .await
        .expect("estado");
    assert!(
        anexada,
        "desligar a credencial retirou a caixa da instituição"
    );

    // O que mudou é o que a listagem mostra sobre ela.
    let caixas =
        ocinye_core::modules::mail::repository::accessible_mailboxes(&pool, alice.person_id)
            .await
            .expect("listar");
    let esta = caixas
        .iter()
        .find(|c| c.id == caixa)
        .expect("a caixa devia continuar na lista");
    assert!(
        !esta.has_credential,
        "a caixa continua a dizer que tem credencial guardada"
    );
}

/// A escolha da credencial, medida.
///
/// # O que este teste guarda
///
/// Que o registo não devolve sempre a mesma coisa. Escolher a credencial errada
/// não dá erro nenhum: dá uma acção correcta atribuída a quem não a fez — a
/// instituição inteira a ler e a enviar sob uma conta só. A única maneira de
/// isso se ver é perguntar ao registo qual adaptador ele deu, antes de ligar a
/// caixa, depois de a ligar, e depois de a desligar.
#[tokio::test]
async fn o_registo_escolhe_a_credencial_de_quem_age() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let alice = person(&pool, org, &["research_member"]).await;
    let caixa = personal_mailbox(&pool, org, alice.person_id).await;

    let provider = std::sync::Arc::new(RecordingProvider::default());
    let k = chave();
    let registo = registo_com(&provider, Some(k.clone()));

    // Antes de ligar: a da instituição, que é o comportamento que o Ocinye
    // sempre teve.
    assert_eq!(
        registo
            .for_mailbox(&pool, caixa)
            .await
            .expect("escolher")
            .adapter_name(),
        "recording",
        "uma caixa por ligar devia usar a credencial da instituição"
    );

    ocinye_core::modules::mail::service::connect_mailbox(
        &pool,
        &alice,
        caixa,
        &mail::service::MailboxConnection {
            chave: Some(&k),
            sonda: &SondaQueAceita::default(),
            senha: "a-senha-do-imap",
        },
        &CorrelationIds::generate(),
    )
    .await
    .expect("ligar");

    // Depois de ligar: a dela.
    assert_eq!(
        registo
            .for_mailbox(&pool, caixa)
            .await
            .expect("escolher")
            .adapter_name(),
        "imap_smtp",
        "uma caixa ligada devia abrir a sessão com a credencial do membro"
    );

    ocinye_core::modules::mail::service::disconnect_mailbox(
        &pool,
        &alice,
        caixa,
        &CorrelationIds::generate(),
    )
    .await
    .expect("desligar");

    // E depois de desligar: outra vez a da instituição. Sem esta parte, uma
    // sessão em cache continuaria a servir os pedidos seguintes com uma senha
    // que a pessoa já mandou esquecer.
    assert_eq!(
        registo
            .for_mailbox(&pool, caixa)
            .await
            .expect("escolher")
            .adapter_name(),
        "recording",
        "desligar a caixa devia esquecer também a sessão aberta com ela"
    );
}

/// Trocar a senha troca a sessão.
///
/// Uma sessão em cache que sobrevivesse à troca continuaria a autenticar-se com
/// a senha antiga até ao próximo reinício — e o membro veria a caixa a recusá-lo
/// depois de a ter acabado de arranjar.
#[tokio::test]
async fn trocar_a_senha_descarta_a_sessao_anterior() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let alice = person(&pool, org, &["research_member"]).await;
    let caixa = personal_mailbox(&pool, org, alice.person_id).await;

    let provider = std::sync::Arc::new(RecordingProvider::default());
    let k = chave();
    let registo = registo_com(&provider, Some(k.clone()));

    let ligar = |senha: &'static str| {
        let pool = pool.clone();
        let alice = alice.clone();
        let k = k.clone();
        async move {
            ocinye_core::modules::mail::service::connect_mailbox(
                &pool,
                &alice,
                caixa,
                &mail::service::MailboxConnection {
                    chave: Some(&k),
                    sonda: &SondaQueAceita::default(),
                    senha,
                },
                &CorrelationIds::generate(),
            )
            .await
            .expect("ligar");
        }
    };

    ligar("a-primeira").await;
    let primeira = registo.for_mailbox(&pool, caixa).await.expect("escolher");

    ligar("a-segunda").await;
    let segunda = registo.for_mailbox(&pool, caixa).await.expect("escolher");

    assert!(
        !std::sync::Arc::ptr_eq(&primeira, &segunda),
        "a sessão sobreviveu à troca de senha, e continuaria a usar a antiga"
    );
}

/// Uma credencial que não abre sessão não se escreve.
///
/// # Porque isto importa mais do que parece
///
/// Guardar primeiro e falhar depois deixaria a caixa a dizer-se ligada com uma
/// senha que o servidor recusa. O membro descobri-lo-ia pela ausência de
/// correio — que é indistinguível de não ter recebido nada, e portanto não é
/// descoberta nenhuma (ADR-0409 §8).
#[tokio::test]
async fn uma_credencial_que_nao_abre_sessao_nao_e_guardada() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let alice = person(&pool, org, &["research_member"]).await;
    let caixa = personal_mailbox(&pool, org, alice.person_id).await;

    let resultado = ocinye_core::modules::mail::service::connect_mailbox(
        &pool,
        &alice,
        caixa,
        &mail::service::MailboxConnection {
            chave: Some(&chave()),
            sonda: &SondaQueRecusa,
            senha: "a-senha-errada",
        },
        &CorrelationIds::generate(),
    )
    .await;

    assert!(
        matches!(resultado, Err(CoreError::CapabilityUnavailable(_))),
        "uma senha recusada pelo servidor devia recusar a ligação: {resultado:?}"
    );

    let quantas: i64 =
        sqlx::query_scalar("SELECT count(*) FROM mailbox_credentials WHERE mailbox_id = $1")
            .bind(caixa)
            .fetch_one(&pool)
            .await
            .expect("contagem");
    assert_eq!(quantas, 0, "uma credencial recusada ficou guardada");
}

/// E a que abre é experimentada antes de ser escrita.
///
/// Sem esta contagem, uma implementação que nunca consultasse a sonda passaria
/// no teste acima por não haver nada que a obrigue a perguntar.
#[tokio::test]
async fn a_credencial_e_experimentada_antes_de_ser_escrita() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let alice = person(&pool, org, &["research_member"]).await;
    let caixa = personal_mailbox(&pool, org, alice.person_id).await;

    let sonda = SondaQueAceita::default();
    ocinye_core::modules::mail::service::connect_mailbox(
        &pool,
        &alice,
        caixa,
        &mail::service::MailboxConnection {
            chave: Some(&chave()),
            sonda: &sonda,
            senha: "a-senha-certa",
        },
        &CorrelationIds::generate(),
    )
    .await
    .expect("ligar");

    assert_eq!(
        sonda.consultas.load(Ordering::SeqCst),
        1,
        "o Core guardou a credencial sem a experimentar"
    );
}

// ── Provisionamento de caixas ───────────────────────────────────────────

/// Provisionar uma caixa cria-a, e recusa o que não deve criar.
///
/// # Porque as recusas estão no mesmo teste
///
/// Porque são a mesma propriedade vista de quatro lados: **uma caixa pessoal
/// pertence a uma pessoa da instituição, e a um endereço da instituição.**
/// Separá-las em quatro testes daria quatro nomes à mesma frase.
#[tokio::test]
async fn provisionar_cria_a_caixa_e_recusa_o_que_nao_e_da_instituicao() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let alice = person(&pool, org, &["research_member"]).await;

    let email: String = sqlx::query_scalar("SELECT email FROM people WHERE id = $1")
        .bind(alice.person_id)
        .fetch_one(&pool)
        .await
        .expect("endereço da pessoa");

    let dominios = vec!["ocinye.com".to_owned()];
    let ids = CorrelationIds::generate();
    let endereco = format!("pv{}@ocinye.com", &Uuid::new_v4().simple().to_string()[..8]);

    // Fora do domínio: não é uma caixa da Ocinye para provisionar.
    let fora =
        mail::provision_personal_mailbox(&pool, &dominios, &email, "alguem@gmail.com", None, &ids)
            .await;
    assert!(
        matches!(fora, Err(CoreError::Validation(_))),
        "um endereço fora do domínio devia ser recusado: {fora:?}"
    );

    // Sem pessoa: uma caixa pessoal sem dono não é pessoal.
    let sem_dono = mail::provision_personal_mailbox(
        &pool,
        &dominios,
        "ninguem-existe@ocinye.com",
        &endereco,
        None,
        &ids,
    )
    .await;
    assert!(
        matches!(sem_dono, Err(CoreError::NotFound(_))),
        "uma pessoa inexistente devia ser recusada: {sem_dono:?}"
    );

    // Nenhuma das recusas escreveu nada.
    let escritas: i64 = sqlx::query_scalar("SELECT count(*) FROM mailboxes WHERE address = $1")
        .bind(&endereco)
        .fetch_one(&pool)
        .await
        .expect("contagem");
    assert_eq!(escritas, 0, "uma recusa deixou uma caixa escrita");

    // E o caminho certo cria-a.
    let id =
        mail::provision_personal_mailbox(&pool, &dominios, &email, &endereco, Some("Alice"), &ids)
            .await
            .expect("provisionar");

    // Que a pessoa alcança, o que é a única prova que interessa: uma linha na
    // tabela que a listagem não devolvesse não seria uma caixa de correio.
    let caixas =
        ocinye_core::modules::mail::repository::accessible_mailboxes(&pool, alice.person_id)
            .await
            .expect("listar");
    assert!(
        caixas.iter().any(|c| c.id == id && c.address == endereco),
        "a caixa provisionada não apareceu para quem a detém"
    );

    // Uma segunda é recusada com uma razão, e não com uma violação de restrição.
    let segunda = mail::provision_personal_mailbox(
        &pool,
        &dominios,
        &email,
        &format!("outra{endereco}"),
        None,
        &ids,
    )
    .await;
    assert!(
        matches!(segunda, Err(CoreError::Conflict(_))),
        "uma segunda caixa pessoal devia ser recusada com uma razão: {segunda:?}"
    );
}
