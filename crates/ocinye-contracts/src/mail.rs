//! Ocinye Mail: institutional email inside the Ocinye OS.
//!
//! # What lives here
//!
//! The vocabulary the Core, the Workspace and the provider adapters all speak.
//! Deliberately **not** a mirror of any provider's data model: an Ocinye
//! `MailFolder` is not a Gmail label and not an IMAP mailbox, and mapping
//! between them is the adapter's job (ADR-0400).
//!
//! # What does not live here
//!
//! Message bodies. A body is fetched on demand, sanitised server-side and never
//! travels as a shared type — the Workspace receives cleaned HTML, never raw
//! email content.

use serde::{Deserialize, Serialize};

/// A well-known mailbox folder.
///
/// Providers name these differently — `INBOX`, `[Gmail]/Sent Mail`, `Sent
/// Items`. The adapter maps its own names onto this closed set, so the domain
/// and the interface never branch on a provider's vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MailFolder {
    /// Incoming mail.
    Inbox,
    /// Flagged by its owner.
    Starred,
    /// Not yet sent.
    Drafts,
    /// Sent.
    Sent,
    /// Kept, out of the inbox.
    Archive,
    /// Marked as unsolicited by the provider.
    Spam,
    /// Deleted, not yet purged.
    Trash,
}

impl MailFolder {
    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Inbox => "inbox",
            Self::Starred => "starred",
            Self::Drafts => "drafts",
            Self::Sent => "sent",
            Self::Archive => "archive",
            Self::Spam => "spam",
            Self::Trash => "trash",
        }
    }

    /// Parse from the stable representation.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::all().into_iter().find(|f| f.as_str() == value)
    }

    /// Name shown to a member.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Inbox => "Caixa de entrada",
            Self::Starred => "Favoritos",
            Self::Drafts => "Rascunhos",
            Self::Sent => "Enviados",
            Self::Archive => "Arquivados",
            Self::Spam => "Spam",
            Self::Trash => "Lixo",
        }
    }

    /// Every folder, in the order the sidebar shows them.
    #[must_use]
    pub const fn all() -> [Self; 7] {
        [
            Self::Inbox,
            Self::Starred,
            Self::Drafts,
            Self::Sent,
            Self::Archive,
            Self::Spam,
            Self::Trash,
        ]
    }

    /// Whether messages here are still being written.
    #[must_use]
    pub const fn is_draft_folder(self) -> bool {
        matches!(self, Self::Drafts)
    }
}

/// Who a mailbox belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MailboxKind {
    /// One person's own mail.
    ///
    /// A boundary of its own. **No administrative role reaches inside one** —
    /// not `OrganisationAdmin`, not `PlatformAdmin` (briefing §26).
    Personal,
    /// An institutional address such as `info@` or `research@`.
    ///
    /// Reached only through explicit membership, never by belonging to a unit.
    Shared,
}

impl MailboxKind {
    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Personal => "personal",
            Self::Shared => "shared",
        }
    }

    /// Parse from the stable representation.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "personal" => Self::Personal,
            "shared" => Self::Shared,
            _ => return None,
        })
    }
}

/// What a member may do with a shared mailbox.
///
/// Ordered from narrowest to widest, so a comparison answers "at least".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SharedMailboxRole {
    /// Read messages.
    Reader,
    /// Read and reply from the shared address.
    Responder,
    /// Read, reply and send new mail as the shared address.
    Sender,
    /// All of the above, plus membership.
    Manager,
}

impl SharedMailboxRole {
    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Reader => "reader",
            Self::Responder => "responder",
            Self::Sender => "sender",
            Self::Manager => "manager",
        }
    }

    /// Parse from the stable representation.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "reader" => Self::Reader,
            "responder" => Self::Responder,
            "sender" => Self::Sender,
            "manager" => Self::Manager,
            _ => return None,
        })
    }

    /// Whether this role may send new mail as the shared address.
    #[must_use]
    pub const fn may_send(self) -> bool {
        matches!(self, Self::Sender | Self::Manager)
    }

    /// Whether this role may reply from the shared address.
    #[must_use]
    pub const fn may_reply(self) -> bool {
        !matches!(self, Self::Reader)
    }

    /// Whether this role may change membership.
    #[must_use]
    pub const fn may_manage(self) -> bool {
        matches!(self, Self::Manager)
    }

    /// Every role.
    #[must_use]
    pub const fn all() -> [Self; 4] {
        [Self::Reader, Self::Responder, Self::Sender, Self::Manager]
    }
}

/// Where an outgoing message stands.
///
/// Sending crosses a network to a system Ocinye does not own, so it is not
/// instantaneous and must not be reported as if it were (briefing §46).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutboxState {
    /// Accepted by the Core, not yet handed to the provider.
    Queued,
    /// Being handed to the provider.
    Sending,
    /// The provider accepted it.
    Sent,
    /// The provider refused it, or could not be reached.
    ///
    /// **The draft survives.** A failed send never loses what was written.
    Failed,
}

impl OutboxState {
    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Sending => "sending",
            Self::Sent => "sent",
            Self::Failed => "failed",
        }
    }

    /// Parse from the stable representation.
    ///
    /// An unrecognised value reads as [`OutboxState::Failed`]: a row this build
    /// cannot interpret must never be reported as delivered.
    #[must_use]
    pub fn parse(value: &str) -> Self {
        match value {
            "queued" => Self::Queued,
            "sending" => Self::Sending,
            "sent" => Self::Sent,
            _ => Self::Failed,
        }
    }

    /// Label shown to a member.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Queued => "Na fila",
            Self::Sending => "A enviar",
            Self::Sent => "Enviado",
            Self::Failed => "Não enviado",
        }
    }
}

/// How a recipient stands in relation to the institution.
///
/// The distinction that classification policy turns on: sending `CONFIDENTIAL`
/// material to a colleague is ordinary; sending it outside is data leaving the
/// institution (briefing §36).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecipientScope {
    /// A recipient at an institutional domain.
    Internal,
    /// A recipient outside every institutional domain.
    External,
}

impl RecipientScope {
    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Internal => "internal",
            Self::External => "external",
        }
    }
}

/// How the interface should treat remote content in a message body.
///
/// Remote images are the ordinary form of email tracking: fetching one tells
/// the sender the message was opened, from which address, and when. Blocking
/// them by default is the correct posture (briefing §12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteContentPolicy {
    /// Never load remote content. The default.
    Block,
    /// Load it for this message only, after the member asked.
    AllowOnce,
    /// Load it for messages from senders the member has allowed.
    AllowKnownSenders,
}

impl RemoteContentPolicy {
    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Block => "block",
            Self::AllowOnce => "allow_once",
            Self::AllowKnownSenders => "allow_known_senders",
        }
    }

    /// Parse from the stable representation.
    ///
    /// Anything unrecognised blocks. Failing closed here means a corrupted
    /// setting cannot turn tracking back on.
    #[must_use]
    pub fn parse(value: &str) -> Self {
        match value {
            "allow_once" => Self::AllowOnce,
            "allow_known_senders" => Self::AllowKnownSenders,
            _ => Self::Block,
        }
    }
}

/// How a draft came to be written.
///
/// Kept because "was this written by a person or by a model" is a question the
/// institution will want answered later. **Not** shown as a banner on every
/// message (briefing §71).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DraftOrigin {
    /// Typed by its author.
    Manual,
    /// Generated from a prompt, then editable.
    AiGenerated,
    /// Typed by its author, then transformed by a model.
    AiTransformed,
}

impl DraftOrigin {
    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::AiGenerated => "ai_generated",
            Self::AiTransformed => "ai_transformed",
        }
    }

    /// Parse from the stable representation.
    #[must_use]
    pub fn parse(value: &str) -> Self {
        match value {
            "ai_generated" => Self::AiGenerated,
            "ai_transformed" => Self::AiTransformed,
            _ => Self::Manual,
        }
    }
}

/// What the AI assistant is being asked to do to a draft.
///
/// A closed set, not free-form: the instruction that reaches the model is built
/// from this, so a member cannot steer the system prompt by typing into the
/// transformation field (briefing §38).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComposeAction {
    /// Write a draft from a description.
    Generate,
    /// Draft a reply to a message.
    Reply,
    /// Raise the register.
    MoreFormal,
    /// Shorten it.
    Shorter,
    /// Warm the tone.
    MoreCordial,
    /// Make it more direct.
    MoreDirect,
    /// Improve clarity without changing meaning.
    Clarify,
    /// Fix spelling and grammar.
    Proofread,
    /// Translate it.
    Translate,
    /// Summarise a message or a thread.
    Summarise,
}

impl ComposeAction {
    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Generate => "generate",
            Self::Reply => "reply",
            Self::MoreFormal => "more_formal",
            Self::Shorter => "shorter",
            Self::MoreCordial => "more_cordial",
            Self::MoreDirect => "more_direct",
            Self::Clarify => "clarify",
            Self::Proofread => "proofread",
            Self::Translate => "translate",
            Self::Summarise => "summarise",
        }
    }

    /// Parse from the stable representation.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::all().into_iter().find(|a| a.as_str() == value)
    }

    /// Label shown to a member.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Generate => "Gerar email",
            Self::Reply => "Preparar resposta",
            Self::MoreFormal => "Mais formal",
            Self::Shorter => "Mais curto",
            Self::MoreCordial => "Mais cordial",
            Self::MoreDirect => "Mais directo",
            Self::Clarify => "Melhorar clareza",
            Self::Proofread => "Corrigir a língua",
            Self::Translate => "Traduzir",
            Self::Summarise => "Resumir",
        }
    }

    /// Whether this action needs an existing draft to work on.
    #[must_use]
    pub const fn needs_draft(self) -> bool {
        !matches!(self, Self::Generate | Self::Reply | Self::Summarise)
    }

    /// Every action.
    #[must_use]
    pub const fn all() -> [Self; 10] {
        [
            Self::Generate,
            Self::Reply,
            Self::MoreFormal,
            Self::Shorter,
            Self::MoreCordial,
            Self::MoreDirect,
            Self::Clarify,
            Self::Proofread,
            Self::Translate,
            Self::Summarise,
        ]
    }

    /// The transformations offered beside a draft, in the order shown.
    #[must_use]
    pub const fn transformations() -> [Self; 6] {
        [
            Self::MoreFormal,
            Self::Shorter,
            Self::MoreCordial,
            Self::MoreDirect,
            Self::Clarify,
            Self::Proofread,
        ]
    }
}

/// One address on a message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MailAddress {
    /// The address itself, lower-cased.
    pub address: String,
    /// The display name the sender chose, when there was one.
    ///
    /// **Never trusted.** A display name is chosen by whoever sent the message
    /// and is the ordinary vehicle for impersonation, so the interface always
    /// shows the address beside it (briefing §70).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// Whether this address is inside the institution.
    pub scope: RecipientScope,
}

impl MailAddress {
    /// Build an address, deciding its scope from the institutional domains.
    ///
    /// Comparison is on the domain only, case-insensitively. A subdomain is not
    /// the same domain: `ocinye.com.attacker.net` must not read as internal.
    #[must_use]
    pub fn new(address: &str, display_name: Option<String>, institutional: &[String]) -> Self {
        let address = address.trim().to_lowercase();
        let domain = address.rsplit_once('@').map(|(_, domain)| domain);

        let scope = match domain {
            Some(domain)
                if institutional
                    .iter()
                    .any(|known| known.trim().to_lowercase() == domain) =>
            {
                RecipientScope::Internal
            }
            _ => RecipientScope::External,
        };

        Self {
            address,
            display_name: display_name.map(|name| name.trim().to_owned()),
            scope,
        }
    }

    /// Whether this address is outside the institution.
    #[must_use]
    pub const fn is_external(&self) -> bool {
        matches!(self.scope, RecipientScope::External)
    }

    /// The domain, when the address has one.
    #[must_use]
    pub fn domain(&self) -> Option<&str> {
        self.address.rsplit_once('@').map(|(_, domain)| domain)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn institutional() -> Vec<String> {
        vec!["ocinye.com".to_owned(), "Ocinye.AO".to_owned()]
    }

    #[test]
    fn folders_roles_and_actions_round_trip() {
        for folder in MailFolder::all() {
            assert_eq!(MailFolder::parse(folder.as_str()), Some(folder));
            assert!(!folder.label().is_empty());
        }
        for role in SharedMailboxRole::all() {
            assert_eq!(SharedMailboxRole::parse(role.as_str()), Some(role));
        }
        for action in ComposeAction::all() {
            assert_eq!(ComposeAction::parse(action.as_str()), Some(action));
            assert!(!action.label().is_empty());
        }
    }

    #[test]
    fn an_institutional_address_is_recognised_whatever_its_case() {
        let address = MailAddress::new("  Ana@OCINYE.com ", None, &institutional());
        assert_eq!(address.address, "ana@ocinye.com");
        assert!(!address.is_external());

        // The configured domain's own case must not matter either.
        assert!(!MailAddress::new("x@ocinye.ao", None, &institutional()).is_external());
    }

    #[test]
    fn a_lookalike_domain_is_external() {
        // The whole point of the check: `ocinye.com.attacker.net` ends with the
        // institutional domain and is not it.
        for address in [
            "ana@ocinye.com.attacker.net",
            "ana@notocinye.com",
            "ana@ocinye.co",
            "ana@sub.ocinye.com",
        ] {
            assert!(
                MailAddress::new(address, None, &institutional()).is_external(),
                "{address} was treated as internal"
            );
        }
    }

    #[test]
    fn an_address_without_a_domain_is_external() {
        // Failing closed: something that is not an address is not internal.
        assert!(MailAddress::new("nonsense", None, &institutional()).is_external());
        assert!(MailAddress::new("", None, &institutional()).is_external());
    }

    #[test]
    fn with_no_institutional_domains_everything_is_external() {
        // An unconfigured deployment must not conclude that mail stays inside.
        assert!(MailAddress::new("ana@ocinye.com", None, &[]).is_external());
    }

    #[test]
    fn shared_roles_are_ordered_and_gated() {
        assert!(SharedMailboxRole::Manager > SharedMailboxRole::Reader);

        assert!(!SharedMailboxRole::Reader.may_reply());
        assert!(SharedMailboxRole::Responder.may_reply());
        assert!(!SharedMailboxRole::Responder.may_send());
        assert!(SharedMailboxRole::Sender.may_send());
        assert!(!SharedMailboxRole::Sender.may_manage());
        assert!(SharedMailboxRole::Manager.may_manage());
    }

    #[test]
    fn an_unknown_outbox_state_never_reads_as_sent() {
        assert_eq!(OutboxState::parse("something_new"), OutboxState::Failed);
        assert_eq!(OutboxState::parse("sent"), OutboxState::Sent);
    }

    #[test]
    fn an_unknown_remote_content_policy_blocks() {
        assert_eq!(
            RemoteContentPolicy::parse("nonsense"),
            RemoteContentPolicy::Block
        );
        assert_eq!(RemoteContentPolicy::parse(""), RemoteContentPolicy::Block);
    }

    #[test]
    fn transformations_all_need_something_to_transform() {
        for action in ComposeAction::transformations() {
            assert!(action.needs_draft(), "{action:?} has nothing to work on");
        }
        assert!(!ComposeAction::Generate.needs_draft());
        assert!(!ComposeAction::Reply.needs_draft());
    }
}
