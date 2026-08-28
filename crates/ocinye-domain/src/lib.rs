//! Institutional invariants of the Ocinye OS.
//!
//! This crate holds the rules that must hold regardless of transport,
//! persistence or presentation: who may do what, and which state transitions
//! are legitimate.
//!
//! # Why it is pure
//!
//! Nothing here performs I/O. A policy that cannot be exercised without a
//! database is a policy that will not be exhaustively tested, and authorization
//! is precisely where exhaustive testing pays for itself. Every decision is a
//! function of a [`Principal`] and a described resource.
//!
//! # What does not belong here
//!
//! SQL, HTTP, configuration, secrets. The Core renders
//! [`policy::VisibilityFilter`] into SQL; this crate defines what that filter
//! *means*.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod error;
pub mod identifiers;
pub mod policy;
pub mod principal;
pub mod workflow;

pub use error::DomainError;
pub use policy::{
    ai_processing_ceiling, approval_needed, can, effective_risk, explain, is_delegable_to_agents,
    may_invoke, may_process_with_ai, AccessSource, Action, AgentBoundary, AgenticRefusal, Decision,
    ExplicitGrant, ResourceContext, ResourceKind, VisibilityFilter,
};
pub use principal::Principal;
