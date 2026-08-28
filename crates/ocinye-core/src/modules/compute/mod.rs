//! Compute Plane: the node registry and the node protocol.
//!
//! # What belongs here
//!
//! Registration, enrollment and heartbeat of compute nodes, and the derived
//! view of what the institution can currently compute on.
//!
//! # Zero is the current, correct state
//!
//! No Ocinye compute node exists. The registry reports `0` nodes and says so
//! plainly; it does not invent a `CAM-01` to make a screen look populated
//! (ADR-0500). No node identifier appears anywhere in this code: identifiers
//! are values supplied at registration.
//!
//! # A node is not trusted
//!
//! Everything a node reports about itself — cores, memory, GPUs, models — is
//! untrusted input. A compromised node may lie, so nothing it says is used to
//! make an authorization decision.

mod model;
mod repository;
mod service;

/// Cross-module operations on compute data.
///
/// The Intelligence Plane derives model availability from node liveness, which
/// is compute's fact to report. Exposing it here keeps that dependency explicit
/// instead of letting another module reach into this one's repository.
pub mod internal {
    pub use super::repository::mark_stale_models_unavailable;
}

pub use model::{ComputeNode, GpuReport, NodeHeartbeat, NodeResources};
pub use service::{
    compute_status, enroll_node, heartbeat, list_nodes, register_node, EnrolledNode, NewNode,
};
