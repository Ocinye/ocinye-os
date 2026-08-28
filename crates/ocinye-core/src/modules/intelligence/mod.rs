//! Intelligence Plane: the Ocinye AI Gateway.
//!
//! # What belongs here
//!
//! The single point through which the platform asks for AI *capabilities*, the
//! registry of models that could serve them, and the honest reporting of what
//! is actually available.
//!
//! # Capabilities, never model names
//!
//! Callers ask for `GENERAL`, `CODING`, `REASONING` or `EMBEDDING`. Mapping a
//! capability to a model is configuration (ADR-0300), so enrolling a node with
//! different models changes behaviour without a code change. No model name
//! appears anywhere in this module.
//!
//! # Unavailable is a correct answer
//!
//! With no Ocinye node enrolled, every capability is unavailable and the
//! gateway says so. It never reaches for an external provider to hide the
//! absence of local infrastructure, and it never breaks the platform: features
//! that want AI degrade explicitly.
//!
//! # Retrieval is permission-aware by construction
//!
//! Context assembly applies the caller's own read policy *before* retrieval.
//! Filtering a generated answer afterwards does not correct a context that was
//! wrongly assembled (ADR-0300).

pub mod agents;
#[cfg(feature = "test-fixtures")]
pub mod conformance;
#[cfg(feature = "test-fixtures")]
pub mod fixture;
mod model;
pub mod provider;
mod repository;
mod service;

pub use agents::{Agent, AgentScope, AgentState, NewAgent};
pub use model::{RegisteredModel, RetrievedRef};
pub use provider::{
    infer_within_deadline, ContractVersion, DataBlock, InferenceError, InferenceProvider,
    InferenceRequest, InferenceResponse, InferenceResult, ModelIdentity, NoProvider, TokenUsage,
};
pub use service::{
    assemble_context, intelligence_status, list_models, record_rejected_job, refresh_availability,
    resolve_capability,
};
