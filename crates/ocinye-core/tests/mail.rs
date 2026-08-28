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
    ProviderHealth, ProviderResult,
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
        "INSERT INTO people (organisation_id, full_name, email, username, status)
              VALUES ($1, $2, $3, $2, 'active') RETURNING id",
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
    let provider = RecordingProvider::default();
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

    match mail::read_message(&pool, &provider, &administrator, message, false, &ids).await {
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

    let provider = RecordingProvider::default();
    let ids = CorrelationIds::generate();
    let recipients = [address("parceiro@exemplo.com")];

    for confirmed in [false, true] {
        let outcome = mail::send(
            &pool,
            &provider,
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

    let provider = RecordingProvider::default();
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
        &provider,
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

    let provider = RecordingProvider::default();
    let ids = CorrelationIds::generate();

    let mut message = outgoing(&mailbox.address, "colega@ocinye.com");
    message.subject = "SEGREDO-NO-ASSUNTO".to_owned();
    message.body = "SEGREDO-NO-CORPO".to_owned();

    mail::send(
        &pool,
        &provider,
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
    let provider = RecordingProvider::default();

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
