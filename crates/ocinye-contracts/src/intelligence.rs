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

/// Who authored an interaction turn.
///
/// The distinction is load-bearing: a deterministic answer written by the
/// platform because no inference could run is `SYSTEM`, and must never be
/// mistaken for `MODEL` — a real answer from Qwen, DeepSeek or any model. The
/// caller reads `origin` to know what it is looking at (briefing §8, M5 §8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InteractionOrigin {
    /// The Ocinye OS itself, deterministically. Never a model.
    System,
    /// A model, through the AI Gateway.
    Model,
    /// A typed capability executed by the Core.
    Tool,
    /// An agent orchestrating capabilities.
    Agent,
}

impl InteractionOrigin {
    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::System => "SYSTEM",
            Self::Model => "MODEL",
            Self::Tool => "TOOL",
            Self::Agent => "AGENT",
        }
    }
}

/// How an interaction turn concluded.
///
/// `DEGRADED` is a **successful** conclusion: the request was received,
/// authorized and processed, and the platform answered deterministically
/// because no inference capacity could execute it. It is not an error — nothing
/// is broken (M5 §6). Errors are reserved for real failures and travel as
/// [`crate::ErrorCode`], not as a status here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub enum InteractionStatus {
    /// A model produced the answer.
    Completed,
    /// Processed correctly, answered deterministically without inference.
    Degraded,
}

impl InteractionStatus {
    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "COMPLETED",
            Self::Degraded => "DEGRADED",
        }
    }
}

/// A machine-readable reason a request could not be served by inference.
///
/// The vocabulary is established now (M5 §7); only a subset is reachable in
/// M5.1 — today's real state is `AI_NO_PROVIDER_AVAILABLE`. The rest name the
/// distinct future conditions (an unhealthy provider, a loading model, a quota
/// exhausted) so later execution paths classify without inventing strings. The
/// wire form is `SCREAMING_SNAKE_CASE` — the variant `AiNoProviderAvailable`
/// serializes as `AI_NO_PROVIDER_AVAILABLE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub enum AiReasonCode {
    /// No inference provider is registered to serve the capability.
    AiNoProviderAvailable,
    /// A provider exists, but no model serves the requested capability.
    AiNoCompatibleModel,
    /// The capability itself is not yet built on this installation.
    AiCapacityUnavailable,
    /// A candidate model's hardware requirements are not met by any node.
    AiModelHardwareNotSatisfied,
    /// A mapped provider is registered but reporting unhealthy.
    AiProviderUnhealthy,
    /// A model is registered and loading, not yet ready to serve.
    AiModelLoading,
    /// The caller lacks permission to use AI.
    AiPermissionDenied,
    /// The caller is not entitled to the resolved model.
    AiModelNotEntitled,
    /// A resource-governance quota for the request is exhausted.
    AiResourceQuotaExceeded,
    /// AI is administratively disabled by policy.
    AiDisabledByPolicy,
}

impl AiReasonCode {
    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AiNoProviderAvailable => "AI_NO_PROVIDER_AVAILABLE",
            Self::AiNoCompatibleModel => "AI_NO_COMPATIBLE_MODEL",
            Self::AiCapacityUnavailable => "AI_CAPACITY_UNAVAILABLE",
            Self::AiModelHardwareNotSatisfied => "AI_MODEL_HARDWARE_NOT_SATISFIED",
            Self::AiProviderUnhealthy => "AI_PROVIDER_UNHEALTHY",
            Self::AiModelLoading => "AI_MODEL_LOADING",
            Self::AiPermissionDenied => "AI_PERMISSION_DENIED",
            Self::AiModelNotEntitled => "AI_MODEL_NOT_ENTITLED",
            Self::AiResourceQuotaExceeded => "AI_RESOURCE_QUOTA_EXCEEDED",
            Self::AiDisabledByPolicy => "AI_DISABLED_BY_POLICY",
        }
    }

    /// Parse from the stable representation.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "AI_NO_PROVIDER_AVAILABLE" => Self::AiNoProviderAvailable,
            "AI_NO_COMPATIBLE_MODEL" => Self::AiNoCompatibleModel,
            "AI_CAPACITY_UNAVAILABLE" => Self::AiCapacityUnavailable,
            "AI_MODEL_HARDWARE_NOT_SATISFIED" => Self::AiModelHardwareNotSatisfied,
            "AI_PROVIDER_UNHEALTHY" => Self::AiProviderUnhealthy,
            "AI_MODEL_LOADING" => Self::AiModelLoading,
            "AI_PERMISSION_DENIED" => Self::AiPermissionDenied,
            "AI_MODEL_NOT_ENTITLED" => Self::AiModelNotEntitled,
            "AI_RESOURCE_QUOTA_EXCEEDED" => Self::AiResourceQuotaExceeded,
            "AI_DISABLED_BY_POLICY" => Self::AiDisabledByPolicy,
            _ => return None,
        })
    }
}

/// The typed result of a Prompt Ocinye interaction.
///
/// One shape for every conclusion. A model answer is
/// `origin = MODEL, status = COMPLETED, reason_code = None`, naming its model,
/// provider and node. Today's real conclusion is
/// `origin = SYSTEM, status = DEGRADED, reason_code = AI_NO_PROVIDER_AVAILABLE`,
/// with `model`, `provider` and `compute_node` explicitly `null` — the request
/// was processed, and the platform, not a model, wrote `content`.
///
/// This is returned with HTTP success: a processed request that no inference
/// could execute did not fail (M5 §5, §6). The `model`/`provider`/`compute_node`
/// fields serialize as `null` when absent — deliberately present, so a reader
/// can prove no model was involved rather than infer it from omission.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiInteractionResponse {
    /// Who authored this turn.
    pub origin: InteractionOrigin,
    /// How it concluded.
    pub status: InteractionStatus,
    /// The machine reason, when the conclusion was degraded.
    #[serde(default)]
    pub reason_code: Option<AiReasonCode>,
    /// The model that answered, when one did.
    pub model: Option<String>,
    /// The provider that served the model, when one did.
    pub provider: Option<String>,
    /// The compute node that ran the model, when one did.
    pub compute_node: Option<String>,
    /// The answer shown to the member, in the platform's own words.
    pub content: String,
}

impl AiInteractionResponse {
    /// A deterministic system answer for a request that no inference could run.
    ///
    /// Every provenance field is `None`: no model, no provider, no node was
    /// involved. This is the only way the platform answers without a model, and
    /// it is unmistakably `SYSTEM`.
    #[must_use]
    pub fn degraded(reason_code: AiReasonCode, content: String) -> Self {
        Self {
            origin: InteractionOrigin::System,
            status: InteractionStatus::Degraded,
            reason_code: Some(reason_code),
            model: None,
            provider: None,
            compute_node: None,
            content,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn os_codigos_maquina_sao_screaming_snake_com_prefixo_ai() {
        // O prefixo `AI_` faz parte do código, e a derivação tem de o produzir.
        assert_eq!(
            serde_json::to_value(AiReasonCode::AiNoProviderAvailable).unwrap(),
            json!("AI_NO_PROVIDER_AVAILABLE")
        );
        assert_eq!(
            AiReasonCode::AiNoProviderAvailable.as_str(),
            "AI_NO_PROVIDER_AVAILABLE"
        );
        // O par serde/`as_str` não pode divergir para nenhuma variante.
        for code in [
            AiReasonCode::AiNoProviderAvailable,
            AiReasonCode::AiNoCompatibleModel,
            AiReasonCode::AiCapacityUnavailable,
            AiReasonCode::AiModelHardwareNotSatisfied,
            AiReasonCode::AiProviderUnhealthy,
            AiReasonCode::AiModelLoading,
            AiReasonCode::AiPermissionDenied,
            AiReasonCode::AiModelNotEntitled,
            AiReasonCode::AiResourceQuotaExceeded,
            AiReasonCode::AiDisabledByPolicy,
        ] {
            assert_eq!(
                serde_json::to_value(code).unwrap(),
                json!(code.as_str()),
                "serde e as_str divergem para {code:?}"
            );
            assert_eq!(AiReasonCode::parse(code.as_str()), Some(code));
        }
    }

    #[test]
    fn o_envelope_degradado_e_do_sistema_com_provenance_nula_explicita() {
        let response = AiInteractionResponse::degraded(
            AiReasonCode::AiNoProviderAvailable,
            "Sem inferência.".to_owned(),
        );
        let wire = serde_json::to_value(&response).unwrap();

        assert_eq!(wire["origin"], "SYSTEM");
        assert_eq!(wire["status"], "DEGRADED");
        assert_eq!(wire["reason_code"], "AI_NO_PROVIDER_AVAILABLE");
        // Presentes e nulos, para se provar a ausência de modelo.
        for field in ["model", "provider", "compute_node"] {
            assert!(wire.get(field).is_some(), "{field} devia estar presente");
            assert!(wire[field].is_null(), "{field} devia ser nulo");
        }
    }

    #[test]
    fn origem_e_estado_serializam_em_maiusculas() {
        assert_eq!(
            serde_json::to_value(InteractionOrigin::System).unwrap(),
            json!("SYSTEM")
        );
        assert_eq!(
            serde_json::to_value(InteractionOrigin::Model).unwrap(),
            json!("MODEL")
        );
        assert_eq!(
            serde_json::to_value(InteractionStatus::Degraded).unwrap(),
            json!("DEGRADED")
        );
        assert_eq!(
            serde_json::to_value(InteractionStatus::Completed).unwrap(),
            json!("COMPLETED")
        );
    }
}
