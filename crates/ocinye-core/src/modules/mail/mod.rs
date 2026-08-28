//! Ocinye Mail: institutional email inside the Ocinye OS.
//!
//! # What belongs here
//!
//! The mail domain: mailbox ownership, the provider abstraction, message
//! indexing, drafts, the send path, and the policy that decides whether
//! classified material may leave the institution by email.
//!
//! # Three boundaries this module holds
//!
//! **Privacy.** A personal mailbox is a boundary of its own. No administrative
//! role reaches inside one — not `OrganisationAdmin`, not `PlatformAdmin`. The
//! technical administration of the mail service and the reading of somebody's
//! correspondence are different powers, and the second is not granted by the
//! first (briefing §26).
//!
//! **Untrusted content.** Everything that arrives — HTML, attachments, display
//! names, filenames — is written by whoever sent it. It is sanitised, never
//! rendered as received ([`sanitize`]).
//!
//! **Data leaving the institution.** Sending is an export. Classification is
//! consulted before a message goes out, and `RESTRICTED` material does not
//! leave to an external recipient by default (briefing §35, §36).
//!
//! # Mail does not depend on AI
//!
//! Reading, writing, replying, sending and searching all work with zero AI
//! nodes registered. Only the assistance is unavailable, and it says so
//! (briefing §6).

pub mod imap_smtp;
pub mod policy;
pub mod provider;
pub mod registry;
pub mod repository;
pub mod sanitize;
pub mod service;

pub use policy::{SendDecision, SendPolicy};
pub use provider::{MailProvider, ProviderError, ProviderHealth, ProviderResult};
pub use registry::ProviderRegistry;
pub use repository::{AccessibleMailbox, IndexedMessage, MailDraft, MailPreferences};
pub use sanitize::{sanitize_html, text_to_html, SanitizedBody};
pub use service::{
    assist, connect_mailbox, disconnect_mailbox, evaluate_send, mailbox, mailboxes,
    provision_personal_mailbox, read_message, safe_filename, send, sender_identity, set_flag, sync,
    AssistRequest, AssistResult, MailboxConnection, ReadableMessage, SyncOutcome,
};

/// Constrói o adaptador de correio a partir da configuração desta instalação.
///
/// # Porque vive aqui e não em cada serviço
///
/// Porque o Core e o worker precisam do mesmo adaptador, e dois construtores
/// seriam dois sítios a decidir o que conta como «correio configurado» — que é
/// a maneira de um deles passar a ligar-se em texto simples sem ninguém dar por
/// isso.
///
/// Uma instalação sem correio devolve o fornecedor que recusa tudo da mesma
/// maneira, e di-lo. Nunca `None`: uma ausência silenciosa acabaria testada com
/// um `unwrap_or_default` algures.
#[must_use]
pub fn from_config(config: &crate::config::CoreConfig) -> std::sync::Arc<dyn MailProvider> {
    if !config.mail.is_configured() {
        // Duas ausências diferentes, e dizer-lhes o mesmo manda quem lê
        // procurar no sítio errado. Sem transporte não há serviço nenhum; com
        // transporte e sem conta de serviço há correio, e é de cada pessoa.
        if config.mail.transport_configured() {
            tracing::info!(
                imap = %config.mail.imap_host,
                smtp = %config.mail.smtp_host,
                "Ocinye Mail transport is configured with no institutional service \
                 account; each member connects their own mailbox (ADR-0409)"
            );
        } else {
            tracing::info!("Ocinye Mail is not configured on this deployment");
        }
        return std::sync::Arc::new(crate::modules::mail::provider::UnconfiguredProvider);
    }

    let settings = crate::modules::mail::imap_smtp::ImapSmtpConfig {
        imap_host: config.mail.imap_host.clone(),
        imap_port: config.mail.imap_port,
        imap_security: config.mail.imap_security,
        smtp_host: config.mail.smtp_host.clone(),
        smtp_port: config.mail.smtp_port,
        smtp_security: config.mail.smtp_security,
        username: config.mail.username.clone(),
        password: crate::password::Secret::new(config.mail.password.clone()),
    };

    match crate::modules::mail::imap_smtp::ImapSmtpProvider::new(settings) {
        Ok(provider) => {
            // Hosts and ports only. The username is an address and the password
            // is never anywhere near a log line (briefing §57).
            tracing::info!(
                imap = %config.mail.imap_host,
                imap_port = config.mail.imap_port,
                imap_security = config.mail.imap_security.as_str(),
                smtp = %config.mail.smtp_host,
                smtp_port = config.mail.smtp_port,
                smtp_security = config.mail.smtp_security.as_str(),
                "Ocinye Mail adapter ready"
            );
            std::sync::Arc::new(provider)
        }
        Err(error) => {
            tracing::error!(
                cause = %error,
                "Ocinye Mail adapter could not be built; mail will report as unavailable"
            );
            std::sync::Arc::new(crate::modules::mail::provider::UnconfiguredProvider)
        }
    }
}
