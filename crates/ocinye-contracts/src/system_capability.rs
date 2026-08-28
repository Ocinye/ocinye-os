//! System capabilities: what this installation can actually do right now.
//!
//! # Why this exists
//!
//! Two questions look alike and are not:
//!
//! - *May this person do it?* — authorization, answered by [`crate::Permission`].
//! - *Can the system do it at all?* — availability, answered here.
//!
//! An action is executable only when **both** allow (briefing §56). Conflating
//! them produces the two worst messages a system can give: telling someone they
//! lack permission when the hardware simply is not installed, and telling them
//! a feature is unavailable when in fact they are not allowed to use it.
//!
//! # Why one place
//!
//! Without this, `if no_gpu` appears in twenty components and drifts. The Core
//! answers once, at `GET /api/v1/system/capabilities`, and every surface reads
//! the same answer.

use serde::{Deserialize, Serialize};

/// A capability the Ocinye OS may or may not be able to serve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SystemCapability {
    /// General-purpose inference.
    AiGeneral,
    /// Code-oriented inference.
    AiCoding,
    /// Reasoning-oriented inference.
    AiReasoning,
    /// Vector embeddings, on which semantic search depends.
    AiEmbedding,
    /// Storing and executing AI agent definitions.
    Agents,
    /// Registering compute nodes and running jobs on them.
    Compute,
    /// S3-compatible object storage for documents and datasets.
    ObjectStorage,
    /// Calendário institucional: compromissos, prazos e lembretes.
    Calendar,
    /// Lexical full-text search.
    LexicalSearch,
    /// Semantic search over embeddings.
    SemanticSearch,
    /// WebAssembly capability runtime.
    CapabilityRuntime,
    /// Reading institutional mail.
    Mail,
    /// Sending institutional mail.
    ///
    /// Separate from [`SystemCapability::Mail`]: IMAP and SMTP are different services
    /// and one can be reachable while the other is not.
    MailSend,
    /// Keeping the local index in step with the provider.
    MailSync,
    /// AI assistance while composing.
    ///
    /// Depends on an inference capability, never on the mail provider.
    MailAiAssist,
}

impl SystemCapability {
    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AiGeneral => "ai.general",
            Self::AiCoding => "ai.coding",
            Self::AiReasoning => "ai.reasoning",
            Self::AiEmbedding => "ai.embedding",
            Self::Agents => "agents",
            Self::Compute => "compute",
            Self::ObjectStorage => "object_storage",
            Self::Calendar => "calendar",
            Self::LexicalSearch => "search.lexical",
            Self::SemanticSearch => "search.semantic",
            Self::CapabilityRuntime => "capability_runtime",
            Self::Mail => "mail",
            Self::MailSend => "mail.send",
            Self::MailSync => "mail.sync",
            Self::MailAiAssist => "mail.ai_assist",
        }
    }

    /// Parse from the stable representation.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::all().into_iter().find(|c| c.as_str() == value)
    }

    /// Every capability.
    #[must_use]
    pub const fn all() -> [Self; 15] {
        [
            Self::AiGeneral,
            Self::AiCoding,
            Self::AiReasoning,
            Self::AiEmbedding,
            Self::Agents,
            Self::Compute,
            Self::ObjectStorage,
            Self::Calendar,
            Self::LexicalSearch,
            Self::SemanticSearch,
            Self::CapabilityRuntime,
            Self::Mail,
            Self::MailSend,
            Self::MailSync,
            Self::MailAiAssist,
        ]
    }

    /// Name shown to a member.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::AiGeneral => "IA — geral",
            Self::AiCoding => "IA — programação",
            Self::AiReasoning => "IA — raciocínio",
            Self::AiEmbedding => "IA — embeddings",
            Self::Agents => "Agentes",
            Self::Compute => "Computação",
            Self::ObjectStorage => "Armazenamento de objectos",
            Self::Calendar => "Calendário",
            Self::LexicalSearch => "Pesquisa textual",
            Self::SemanticSearch => "Pesquisa semântica",
            Self::CapabilityRuntime => "Capacidades WASM",
            Self::Mail => "Ocinye Mail",
            Self::MailSend => "Envio de correio",
            Self::MailSync => "Sincronização de correio",
            Self::MailAiAssist => "Assistência de IA no correio",
        }
    }
}

/// Why a capability is, or is not, usable.
///
/// Deliberately more than `online`/`offline` (briefing §4). "No node has been
/// registered" and "a node is registered but not answering" call for different
/// words and different next actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SystemCapabilityState {
    /// Works now.
    Available,
    /// Nothing has been registered or configured to provide it.
    ///
    /// The normal state of AI and Compute before the first node exists. **Not
    /// an error** (briefing §7).
    NoResource,
    /// A provider exists but this deployment has not been configured to use it.
    NotConfigured,
    /// Configured and registered, but not answering.
    Unavailable,
    /// Answering, but not fully.
    Degraded,
    /// Decided and designed, not built.
    Planned,
}

impl SystemCapabilityState {
    /// Whether an action depending on this capability can execute.
    #[must_use]
    pub const fn is_usable(self) -> bool {
        matches!(self, Self::Available | Self::Degraded)
    }

    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::NoResource => "no_resource",
            Self::NotConfigured => "not_configured",
            Self::Unavailable => "unavailable",
            Self::Degraded => "degraded",
            Self::Planned => "planned",
        }
    }

    /// Parse from the stable representation.
    ///
    /// An unrecognised value reads as [`SystemCapabilityState::Unavailable`]: a state
    /// this build cannot interpret must not be treated as usable.
    #[must_use]
    pub fn parse(value: &str) -> Self {
        match value {
            "available" => Self::Available,
            "no_resource" => Self::NoResource,
            "not_configured" => Self::NotConfigured,
            "degraded" => Self::Degraded,
            "planned" => Self::Planned,
            _ => Self::Unavailable,
        }
    }

    /// Short institutional label for the interface.
    ///
    /// Institutional register, never "Coming soon" or "Oops" (briefing §59).
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Available => "Disponível",
            Self::NoResource => "Sem recurso registado",
            Self::NotConfigured => "Não configurado",
            Self::Unavailable => "Indisponível",
            Self::Degraded => "Degradado",
            Self::Planned => "Ainda não activado",
        }
    }
}

/// The state of one capability, with the reason behind it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemCapabilityReport {
    /// Which capability.
    pub capability: SystemCapability,
    /// Its state.
    pub state: SystemCapabilityState,
    /// Why, in institutional language, safe to show a member.
    ///
    /// Never a stack trace, a hostname, or an internal error (briefing §47).
    pub reason: String,
    /// What this capability waits on, when it waits on something.
    ///
    /// Answers "porquê" without the member having to deduce it (briefing §77).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depends_on: Option<String>,
}

impl SystemCapabilityReport {
    /// Build a report.
    #[must_use]
    pub fn new(
        capability: SystemCapability,
        state: SystemCapabilityState,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            capability,
            state,
            reason: reason.into(),
            depends_on: None,
        }
    }

    /// Name the dependency this capability is waiting on.
    #[must_use]
    pub fn depending_on(mut self, dependency: impl Into<String>) -> Self {
        self.depends_on = Some(dependency.into());
        self
    }

    /// Whether an action depending on this capability can execute.
    #[must_use]
    pub const fn is_usable(&self) -> bool {
        self.state.is_usable()
    }
}

/// Everything the Workspace needs to render availability truthfully.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemCapabilities {
    /// One report per capability, in a stable order.
    pub capabilities: Vec<SystemCapabilityReport>,
}

impl SystemCapabilities {
    /// The report for one capability, if the Core returned it.
    #[must_use]
    pub fn get(&self, capability: SystemCapability) -> Option<&SystemCapabilityReport> {
        self.capabilities
            .iter()
            .find(|report| report.capability == capability)
    }

    /// Whether a capability can currently serve an action.
    ///
    /// A capability the Core did not report is **not** usable. Failing closed
    /// here matters: a Workspace talking to an older Core must not conclude
    /// that an unknown capability works.
    #[must_use]
    pub fn is_usable(&self, capability: SystemCapability) -> bool {
        self.get(capability)
            .is_some_and(SystemCapabilityReport::is_usable)
    }

    /// Whether any AI inference capability can serve an action.
    #[must_use]
    pub fn any_ai_usable(&self) -> bool {
        [
            SystemCapability::AiGeneral,
            SystemCapability::AiCoding,
            SystemCapability::AiReasoning,
        ]
        .into_iter()
        .any(|capability| self.is_usable(capability))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_capability_round_trips_and_is_unique() {
        let mut names: Vec<&str> = SystemCapability::all().iter().map(|c| c.as_str()).collect();
        let total = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), total, "two capabilities share a wire name");

        for capability in SystemCapability::all() {
            assert_eq!(
                SystemCapability::parse(capability.as_str()),
                Some(capability)
            );
            assert!(!capability.label().is_empty());
        }
    }

    #[test]
    fn only_available_and_degraded_can_serve_an_action() {
        assert!(SystemCapabilityState::Available.is_usable());
        assert!(SystemCapabilityState::Degraded.is_usable());
        for state in [
            SystemCapabilityState::NoResource,
            SystemCapabilityState::NotConfigured,
            SystemCapabilityState::Unavailable,
            SystemCapabilityState::Planned,
        ] {
            assert!(!state.is_usable(), "{state:?} should not be usable");
        }
    }

    #[test]
    fn an_unknown_state_reads_as_unavailable() {
        // A newer Core naming a state this build does not know must not be
        // interpreted as working.
        assert_eq!(
            SystemCapabilityState::parse("something_new"),
            SystemCapabilityState::Unavailable
        );
        assert!(!SystemCapabilityState::parse("something_new").is_usable());
    }

    #[test]
    fn state_labels_are_institutional() {
        for state in [
            SystemCapabilityState::Available,
            SystemCapabilityState::NoResource,
            SystemCapabilityState::NotConfigured,
            SystemCapabilityState::Unavailable,
            SystemCapabilityState::Degraded,
            SystemCapabilityState::Planned,
        ] {
            let label = state.label().to_lowercase();
            for banned in ["oops", "coming soon", "under construction", "!"] {
                assert!(!label.contains(banned), "{state:?} uses «{banned}»");
            }
        }
    }

    #[test]
    fn a_capability_the_core_did_not_report_is_not_usable() {
        let empty = SystemCapabilities {
            capabilities: Vec::new(),
        };
        for capability in SystemCapability::all() {
            assert!(
                !empty.is_usable(capability),
                "{capability:?} was usable without being reported"
            );
        }
        assert!(!empty.any_ai_usable());
    }

    #[test]
    fn any_ai_usable_ignores_non_inference_capabilities() {
        let report = |capability, state| SystemCapabilityReport::new(capability, state, "test");
        let storage_only = SystemCapabilities {
            capabilities: vec![
                report(
                    SystemCapability::ObjectStorage,
                    SystemCapabilityState::Available,
                ),
                report(
                    SystemCapability::AiEmbedding,
                    SystemCapabilityState::Available,
                ),
            ],
        };
        assert!(
            !storage_only.any_ai_usable(),
            "embeddings and storage are not inference"
        );

        let with_general = SystemCapabilities {
            capabilities: vec![report(
                SystemCapability::AiGeneral,
                SystemCapabilityState::Available,
            )],
        };
        assert!(with_general.any_ai_usable());
    }

    #[test]
    fn a_dependency_can_be_named() {
        let report = SystemCapabilityReport::new(
            SystemCapability::SemanticSearch,
            SystemCapabilityState::NoResource,
            "Nenhum modelo de embeddings está disponível.",
        )
        .depending_on("ai.embedding");

        assert_eq!(report.depends_on.as_deref(), Some("ai.embedding"));
        assert!(!report.is_usable());
    }
}
