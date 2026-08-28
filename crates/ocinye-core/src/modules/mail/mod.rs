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
pub mod repository;
pub mod sanitize;
pub mod service;

pub use policy::{SendDecision, SendPolicy};
pub use provider::{MailProvider, ProviderError, ProviderHealth, ProviderResult};
pub use repository::{AccessibleMailbox, IndexedMessage, MailDraft, MailPreferences};
pub use sanitize::{sanitize_html, text_to_html, SanitizedBody};
pub use service::{
    assist, evaluate_send, mailbox, mailboxes, read_message, safe_filename, send, sender_identity,
    set_flag, sync, AssistRequest, AssistResult, ReadableMessage, SyncOutcome,
};
