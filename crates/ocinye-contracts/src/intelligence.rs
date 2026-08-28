//! Intelligence Plane contracts.
//!
//! The application asks for a *capability*, never for a model name. Mapping a
//! capability to a concrete model is configuration, so enrolling a node with
//! different models changes behaviour without a code change (briefing §49).

use serde::{Deserialize, Serialize};

/// A capability the platform can request from the AI Gateway.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AiCapability {
    /// General-purpose language work.
    General,
    /// Programming assistance.
    Coding,
    /// Extended reasoning.
    Reasoning,
    /// Vector embeddings for semantic search.
    Embedding,
}

impl AiCapability {
    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::General => "GENERAL",
            Self::Coding => "CODING",
            Self::Reasoning => "REASONING",
            Self::Embedding => "EMBEDDING",
        }
    }

    /// Parse from the stable representation.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "GENERAL" => Self::General,
            "CODING" => Self::Coding,
            "REASONING" => Self::Reasoning,
            "EMBEDDING" => Self::Embedding,
            _ => return None,
        })
    }

    /// Every capability.
    #[must_use]
    pub const fn all() -> [Self; 4] {
        [
            Self::General,
            Self::Coding,
            Self::Reasoning,
            Self::Embedding,
        ]
    }
}

/// Availability of a registered model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelStatus {
    /// Reported healthy by its node.
    Available,
    /// Registered but not currently reachable.
    Unavailable,
    /// Administratively disabled.
    Disabled,
}

impl ModelStatus {
    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::Unavailable => "unavailable",
            Self::Disabled => "disabled",
        }
    }

    /// Parse from the stable representation.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "available" => Self::Available,
            "unavailable" => Self::Unavailable,
            "disabled" => Self::Disabled,
            _ => return None,
        })
    }
}

/// Retrieval boundary for context assembly.
///
/// The scope narrows retrieval; it never widens authorization. Permissions are
/// applied before retrieval, not after generation (briefing §52).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RagScope {
    /// Everything the caller may read across the organisation.
    Institutional,
    /// One unit.
    Unit,
    /// One research workspace.
    ResearchWorkspace,
    /// One project.
    Project,
}

impl RagScope {
    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Institutional => "institutional",
            Self::Unit => "unit",
            Self::ResearchWorkspace => "research_workspace",
            Self::Project => "project",
        }
    }
}

/// Reported state of the Intelligence Plane.
///
/// With no Ocinye node enrolled this is `available == false` and
/// `providers == 0`. That is the true state and it is reported as such.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntelligenceStatus {
    /// Whether any capability can currently be served.
    pub available: bool,
    /// Number of registered providers reporting healthy.
    pub providers: u32,
    /// Per-capability availability.
    pub capabilities: Vec<CapabilityStatus>,
    /// Explanation shown to members when nothing is available.
    pub message: String,
}

/// Availability of a single capability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityStatus {
    /// The capability.
    pub capability: AiCapability,
    /// Whether a healthy model currently serves it.
    pub available: bool,
    /// Configured model name, when one is mapped.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configured_model: Option<String>,
}
