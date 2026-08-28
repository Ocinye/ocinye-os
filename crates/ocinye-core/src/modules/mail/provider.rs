//! The mail provider abstraction.
//!
//! # Why an abstraction
//!
//! The Ocinye domain must not know what IMAP is. `MailFolder::Archive` is an
//! Ocinye concept; whether the provider calls it `Archive`, `[Gmail]/All Mail`
//! or `Archives` is the adapter's problem (ADR-0400).
//!
//! Without this line, changing provider would mean changing the domain, the
//! API and the interface. With it, it means writing one more implementation of
//! this trait.
//!
//! # What an adapter is responsible for
//!
//! - mapping folder names in both directions;
//! - parsing MIME into [`FetchedMessage`], using a mature parser;
//! - deriving a thread key from `References`/`In-Reply-To`, never from subject;
//! - turning transport failures into [`ProviderError`], never leaking a raw
//!   protocol error to a caller.
//!
//! # What an adapter never does
//!
//! Authorization. By the time a call reaches an adapter the Core has already
//! decided that this actor may touch this mailbox. An adapter that also checked
//! permissions would be a second place for that rule to live, and to drift.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use ocinye_contracts::MailFolder;

/// Why a provider call did not succeed.
///
/// Deliberately coarse. A caller needs to know whether to retry, to tell the
/// member the service is down, or to stop — not which IMAP response code came
/// back (briefing §103).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProviderError {
    /// No provider is configured for this deployment.
    #[error("mail is not configured")]
    NotConfigured,

    /// The provider could not be reached, or refused the connection.
    ///
    /// Transient. The member is told the service is unavailable and nothing is
    /// reported as sent.
    #[error("the mail service is unavailable")]
    Unavailable,

    /// The provider refused the credentials.
    ///
    /// Distinct from [`ProviderError::Unavailable`]: retrying will not help,
    /// and an operator needs to know which of the two it is.
    #[error("the mail service refused the stored credentials")]
    AuthenticationFailed,

    /// The message or folder does not exist at the provider.
    #[error("the message no longer exists in the mailbox")]
    NotFound,

    /// The provider rejected the message.
    ///
    /// Carries a message already written for a member: an adapter translates
    /// the protocol's complaint before it gets here.
    #[error("{0}")]
    Rejected(String),

    /// The message exceeds what the service accepts.
    #[error("the message exceeds the size the mail service accepts")]
    TooLarge,
}

impl ProviderError {
    /// Whether trying the same call again could succeed.
    #[must_use]
    pub const fn is_transient(&self) -> bool {
        matches!(self, Self::Unavailable)
    }
}

/// The result of a provider call.
pub type ProviderResult<T> = Result<T, ProviderError>;

/// One address as the provider reported it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderAddress {
    /// The address.
    pub address: String,
    /// The display name, if the message carried one. Never trusted.
    pub display_name: Option<String>,
}

/// A message header, enough to list it without fetching the body.
///
/// Listing a mailbox must not cost one body fetch per row (briefing §10).
#[derive(Debug, Clone)]
pub struct MessageHeader {
    /// The provider's identifier. Opaque to the domain.
    pub provider_id: String,
    /// RFC 5322 `Message-ID`, when present.
    pub message_id: Option<String>,
    /// Conversation identity, derived from `References`/`In-Reply-To`.
    pub thread_key: Option<String>,
    /// Which folder it sits in.
    pub folder: MailFolder,
    /// Who sent it.
    pub from: ProviderAddress,
    /// Who it was addressed to.
    pub to: Vec<ProviderAddress>,
    /// Who was copied.
    pub cc: Vec<ProviderAddress>,
    /// Subject, if any.
    pub subject: Option<String>,
    /// A short excerpt for the list.
    pub snippet: Option<String>,
    /// When it was sent.
    pub sent_at: DateTime<Utc>,
    /// Whether it has been read.
    pub is_read: bool,
    /// Whether its owner flagged it.
    pub is_starred: bool,
    /// Whether it carries attachments.
    pub has_attachments: bool,
    /// Size in bytes, when the provider reports it.
    pub size_bytes: Option<i64>,
}

/// An attachment, described without its bytes.
#[derive(Debug, Clone)]
pub struct AttachmentInfo {
    /// The provider's identifier for this part.
    pub part_id: String,
    /// The filename the sender chose. **Untrusted** — sanitised before use.
    pub filename: String,
    /// The declared content type. **Untrusted** — a declaration, not a fact.
    pub content_type: String,
    /// Size in bytes.
    pub size_bytes: i64,
    /// Whether it is referenced from inside the body.
    pub is_inline: bool,
}

/// A message with its body, as fetched.
///
/// `html_body` is **raw, untrusted email HTML**. It is sanitised by
/// [`super::sanitize`] before it reaches any interface, and this type carries
/// the raw form precisely so that the sanitisation step cannot be skipped by
/// accident: nothing renders a `FetchedMessage` directly.
#[derive(Debug, Clone)]
pub struct FetchedMessage {
    /// Its header.
    pub header: MessageHeader,
    /// The plain-text alternative, when the message has one.
    pub text_body: Option<String>,
    /// The HTML alternative. **Raw and untrusted.**
    pub html_body: Option<String>,
    /// Attachments, described.
    pub attachments: Vec<AttachmentInfo>,
    /// Blind recipients, visible only on a message the caller sent.
    pub bcc: Vec<ProviderAddress>,
}

impl FetchedMessage {
    /// The addresses this was sent to.
    #[must_use]
    pub fn to_addresses(&self) -> Vec<String> {
        self.header.to.iter().map(|a| a.address.clone()).collect()
    }

    /// The addresses copied.
    #[must_use]
    pub fn cc_addresses(&self) -> Vec<String> {
        self.header.cc.iter().map(|a| a.address.clone()).collect()
    }
}

/// A message about to be handed to the provider.
#[derive(Debug, Clone)]
pub struct OutgoingMessage {
    /// The identity it is sent as. Already checked against what the sender may
    /// use: an adapter never decides this.
    pub from: ProviderAddress,
    /// Recipients.
    pub to: Vec<ProviderAddress>,
    /// Copied recipients.
    pub cc: Vec<ProviderAddress>,
    /// Blind recipients.
    pub bcc: Vec<ProviderAddress>,
    /// Subject.
    pub subject: String,
    /// The body, as plain text.
    ///
    /// Ocinye Mail composes plain text. A composer that produced HTML would
    /// make the institution a source of the very content the reader has to
    /// distrust, and buys nothing an institutional message needs.
    pub body: String,
    /// The message this replies to, for correct threading.
    pub in_reply_to: Option<String>,
    /// The `References` chain to continue.
    pub references: Vec<String>,
    /// Attachments, with their bytes.
    pub attachments: Vec<OutgoingAttachment>,
}

/// An attachment with its content.
#[derive(Debug, Clone)]
pub struct OutgoingAttachment {
    /// Filename. Already sanitised.
    pub filename: String,
    /// Content type.
    pub content_type: String,
    /// The bytes.
    pub content: Vec<u8>,
}

/// One page of a mailbox listing.
#[derive(Debug, Clone)]
pub struct MessagePage {
    /// The headers on this page.
    pub messages: Vec<MessageHeader>,
    /// An opaque cursor to continue from, when more remain.
    ///
    /// Opaque on purpose: an adapter stores whatever its protocol needs —
    /// a UID, a sequence number, a change token — and the domain never reads it.
    pub next_cursor: Option<String>,
}

/// What a provider can currently do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderHealth {
    /// Where the adapter connects, for the administration screen.
    ///
    /// Hosts and ports only. **Never a credential** — the administration
    /// screen shows connection state and nothing that could be used to
    /// connect (briefing §59).
    pub endpoints: Vec<String>,
    /// Whether mail can be read.
    pub can_read: bool,
    /// Whether mail can be sent.
    ///
    /// Separate from reading: IMAP and SMTP are different services, and one can
    /// be reachable while the other is not (briefing §105).
    pub can_send: bool,
    /// A sentence safe to show a member.
    pub detail: String,
    /// Whether the service refused the credential this adapter carries.
    ///
    /// # Porque é um campo e não uma leitura de `detail`
    ///
    /// A primeira escrita deduzia-o de `detail` conter «recusou as
    /// credenciais». Uma frase não é um guarda: reescrevê-la — traduzi-la,
    /// suavizá-la, acrescentar-lhe um ponto — mudaria o estado da caixa de
    /// alguém, em silêncio e sem nada a falhar.
    ///
    /// Distingue «esta senha não entra» de «o serviço não responde», e a
    /// distinção decide o que a pessoa faz a seguir: voltar a ligar a caixa,
    /// ou esperar.
    pub rejected_credential: bool,
}

/// A mail provider.
///
/// Implemented once per protocol family. The Core holds one behind a trait
/// object and never branches on which it is.
#[async_trait]
pub trait MailProvider: Send + Sync {
    /// A short name for logs and the administration screen.
    fn adapter_name(&self) -> &'static str;

    /// Whether the provider answers, and to what.
    async fn health(&self) -> ProviderHealth;

    /// List one page of a folder.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError`] when the provider cannot be reached or refuses.
    async fn list_messages(
        &self,
        mailbox_address: &str,
        folder: MailFolder,
        cursor: Option<&str>,
        limit: u32,
    ) -> ProviderResult<MessagePage>;

    /// Fetch one message, with its body.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::NotFound`] when the message has since been
    /// deleted at the provider.
    async fn fetch_message(
        &self,
        mailbox_address: &str,
        folder: MailFolder,
        provider_id: &str,
    ) -> ProviderResult<FetchedMessage>;

    /// Fetch the bytes of one attachment.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError`] when the part cannot be retrieved.
    async fn fetch_attachment(
        &self,
        mailbox_address: &str,
        folder: MailFolder,
        provider_id: &str,
        part_id: &str,
    ) -> ProviderResult<Vec<u8>>;

    /// Send a message.
    ///
    /// Returns the `Message-ID` the provider assigned, when it reports one.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::Rejected`] when the provider refuses, and
    /// [`ProviderError::Unavailable`] when it cannot be reached. **Neither is
    /// ever reported to a member as sent.**
    async fn send_message(
        &self,
        mailbox_address: &str,
        message: &OutgoingMessage,
    ) -> ProviderResult<Option<String>>;

    /// Move a message to another folder.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError`] when the move fails.
    async fn move_message(
        &self,
        mailbox_address: &str,
        folder: MailFolder,
        provider_id: &str,
        destination: MailFolder,
    ) -> ProviderResult<()>;

    /// Set the read flag.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError`] when the flag cannot be set.
    async fn set_read(
        &self,
        mailbox_address: &str,
        folder: MailFolder,
        provider_id: &str,
        read: bool,
    ) -> ProviderResult<()>;

    /// Set the flagged/starred flag.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError`] when the flag cannot be set.
    async fn set_starred(
        &self,
        mailbox_address: &str,
        folder: MailFolder,
        provider_id: &str,
        starred: bool,
    ) -> ProviderResult<()>;
}

/// The provider used when none is configured.
///
/// # Why this exists rather than an `Option`
///
/// Every call site would otherwise need to handle "no provider", and one of
/// them would eventually forget. This answers every call with
/// [`ProviderError::NotConfigured`], which the API turns into a 503 and the
/// interface into «O correio institucional ainda não foi configurado».
///
/// A fresh Ocinye installation has no mail service, and that is a normal
/// operational state, not a fault (briefing §62).
pub struct UnconfiguredProvider;

#[async_trait]
impl MailProvider for UnconfiguredProvider {
    fn adapter_name(&self) -> &'static str {
        "unconfigured"
    }

    async fn health(&self) -> ProviderHealth {
        ProviderHealth {
            endpoints: Vec::new(),
            can_read: false,
            can_send: false,
            detail: "O correio institucional ainda não foi configurado nesta instalação \
                     do Ocinye OS."
                .to_owned(),
            // Sem serviço não houve credencial nenhuma para recusar.
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
        Err(ProviderError::NotConfigured)
    }

    async fn fetch_message(
        &self,
        _mailbox_address: &str,
        _folder: MailFolder,
        _provider_id: &str,
    ) -> ProviderResult<FetchedMessage> {
        Err(ProviderError::NotConfigured)
    }

    async fn fetch_attachment(
        &self,
        _mailbox_address: &str,
        _folder: MailFolder,
        _provider_id: &str,
        _part_id: &str,
    ) -> ProviderResult<Vec<u8>> {
        Err(ProviderError::NotConfigured)
    }

    async fn send_message(
        &self,
        _mailbox_address: &str,
        _message: &OutgoingMessage,
    ) -> ProviderResult<Option<String>> {
        Err(ProviderError::NotConfigured)
    }

    async fn move_message(
        &self,
        _mailbox_address: &str,
        _folder: MailFolder,
        _provider_id: &str,
        _destination: MailFolder,
    ) -> ProviderResult<()> {
        Err(ProviderError::NotConfigured)
    }

    async fn set_read(
        &self,
        _mailbox_address: &str,
        _folder: MailFolder,
        _provider_id: &str,
        _read: bool,
    ) -> ProviderResult<()> {
        Err(ProviderError::NotConfigured)
    }

    async fn set_starred(
        &self,
        _mailbox_address: &str,
        _folder: MailFolder,
        _provider_id: &str,
        _starred: bool,
    ) -> ProviderResult<()> {
        Err(ProviderError::NotConfigured)
    }
}

/// Quem sabe dizer se uma credencial de caixa abre sessão.
///
/// # Porque é um trait e não uma chamada directa
///
/// Porque verificar uma credencial é falar com um servidor de correio, e o que
/// os testes precisam de exercitar é o que o Core faz com a resposta — guardar
/// quando abre, não guardar quando não abre. Uma chamada directa tornaria essa
/// decisão inobservável sem um servidor a sério (ADR-0409 §8).
#[async_trait]
pub trait CredentialProbe: Send + Sync {
    /// Tenta abrir sessão com esta credencial.
    ///
    /// # Errors
    ///
    /// Devolve erro quando a credencial não abre, ou quando esta instalação não
    /// tem transporte configurado para a experimentar.
    async fn verify(&self, endereco: &str, username: &str, senha: &str) -> ProviderResult<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_unreachability_is_worth_retrying() {
        assert!(ProviderError::Unavailable.is_transient());
        for error in [
            ProviderError::NotConfigured,
            ProviderError::AuthenticationFailed,
            ProviderError::NotFound,
            ProviderError::TooLarge,
            ProviderError::Rejected("x".into()),
        ] {
            assert!(!error.is_transient(), "{error:?} should not be retried");
        }
    }

    #[test]
    fn provider_errors_carry_no_protocol_detail() {
        // A member must never see an IMAP response code or an SMTP reply line.
        for error in [
            ProviderError::NotConfigured,
            ProviderError::Unavailable,
            ProviderError::AuthenticationFailed,
            ProviderError::NotFound,
            ProviderError::TooLarge,
        ] {
            let rendered = error.to_string();
            for leak in ["550", "BAD", "NO ", "EOF", "tls", "socket"] {
                assert!(
                    !rendered.contains(leak),
                    "{error:?} leaks «{leak}»: {rendered}"
                );
            }
        }
    }

    #[tokio::test]
    async fn the_unconfigured_provider_refuses_everything_the_same_way() {
        let provider = UnconfiguredProvider;

        let health = provider.health().await;
        assert!(!health.can_read);
        assert!(!health.can_send);
        assert!(health.detail.contains("ainda não foi configurado"));
        assert!(
            health.endpoints.is_empty(),
            "um provider por configurar não tem endpoints a mostrar"
        );

        assert_eq!(
            provider
                .list_messages("a@ocinye.com", MailFolder::Inbox, None, 25)
                .await
                .unwrap_err(),
            ProviderError::NotConfigured
        );
        assert_eq!(
            provider
                .fetch_message("a@ocinye.com", MailFolder::Inbox, "1")
                .await
                .unwrap_err(),
            ProviderError::NotConfigured
        );
        assert_eq!(
            provider
                .set_read("a@ocinye.com", MailFolder::Inbox, "1", true)
                .await
                .unwrap_err(),
            ProviderError::NotConfigured
        );
    }

    #[tokio::test]
    async fn an_unconfigured_provider_never_reports_a_send_as_succeeded() {
        // The failure mode that matters most: a member must not be told their
        // message went out when no provider exists to carry it.
        let provider = UnconfiguredProvider;
        let message = OutgoingMessage {
            from: ProviderAddress {
                address: "ana@ocinye.com".into(),
                display_name: None,
            },
            to: vec![ProviderAddress {
                address: "outro@exemplo.com".into(),
                display_name: None,
            }],
            cc: Vec::new(),
            bcc: Vec::new(),
            subject: "Teste".into(),
            body: "Corpo".into(),
            in_reply_to: None,
            references: Vec::new(),
            attachments: Vec::new(),
        };

        assert!(provider
            .send_message("ana@ocinye.com", &message)
            .await
            .is_err());
    }
}
