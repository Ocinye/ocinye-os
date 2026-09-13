//! Intelligence rows.

use chrono::{DateTime, Utc};
use ocinye_contracts::{AiCapability, AiReasonCode, Classification, ModelStatus};
use serde::Serialize;
use serde_json::Value;
use sqlx::FromRow;
use uuid::Uuid;

/// A model registered as able to serve capabilities.
#[derive(Debug, Clone, FromRow)]
pub struct RegisteredModel {
    /// Identifier.
    pub id: Uuid,
    /// Whether it runs on an Ocinye node or an approved external provider.
    pub provider_kind: String,
    /// Provider name — for a node-hosted model, the node identifier.
    pub provider_name: String,
    /// Node hosting it, when node-hosted.
    pub node_id: Option<Uuid>,
    /// Model name.
    pub model_name: String,
    /// Version.
    pub version: String,
    /// Capabilities it serves.
    pub capabilities: Value,
    /// Context window, when known.
    pub context_limit: Option<i32>,
    /// Availability.
    pub status: String,
    /// Ceiling on what may ever be sent to it.
    pub max_classification: String,
    /// Whether it is administratively enabled.
    pub enabled: bool,
    /// When the hosting node last reported it.
    pub reported_at: Option<DateTime<Utc>>,
}

impl RegisteredModel {
    /// Parsed status.
    #[must_use]
    pub fn status(&self) -> ModelStatus {
        ModelStatus::parse(&self.status).unwrap_or(ModelStatus::Unavailable)
    }

    /// Ceiling on what may be sent to this model.
    ///
    /// Falls back to the most restrictive value if unreadable: an unparseable
    /// ceiling must never widen what a model may see.
    #[must_use]
    pub fn max_classification(&self) -> Classification {
        Classification::parse(&self.max_classification).unwrap_or(Classification::Public)
    }

    /// Whether it can serve a capability right now.
    #[must_use]
    pub fn serves(&self, capability: AiCapability) -> bool {
        self.enabled
            && self.status() == ModelStatus::Available
            && self.capabilities.as_array().is_some_and(|values| {
                values
                    .iter()
                    .any(|value| value.as_str() == Some(capability.as_str()))
            })
    }
}

/// The outcome of routing a capability to a model.
///
/// Zero candidates is an **ordinary result**, not an exception: a healthy Core
/// with no model that serves a capability has resolved the question correctly —
/// the answer is «none», with a typed reason — rather than failed to answer it
/// (M5 §20). Only a real fault (a database error) is an `Err` around this.
#[derive(Debug, Clone)]
pub enum ModelResolution {
    /// A model serves the capability and was selected.
    ///
    /// Boxed: a resolved model is far larger than a reason code, and the two
    /// variants should not force every `NoCandidate` to carry that weight.
    Resolved(Box<RegisteredModel>),
    /// No model serves the capability, with the machine reason why.
    NoCandidate(AiReasonCode),
}

/// A reference to an artefact placed in an AI context.
///
/// Identifiers only. Contents never appear in a job record: the provenance of
/// an answer is which artefacts informed it, not a second copy of them.
#[derive(Debug, Clone, Serialize)]
pub struct RetrievedRef {
    /// Kind of artefact.
    pub entity_type: String,
    /// Identifier.
    pub entity_id: Uuid,
    /// Title, for citation.
    pub title: String,
    /// Classification of the artefact.
    pub classification: String,
}
