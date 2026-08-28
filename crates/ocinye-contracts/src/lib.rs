//! Canonical institutional types of the Ocinye OS.
//!
//! One definition per institutional concept, shared by the Core, the Workspace,
//! the Worker and the Node Agent. Duplicating an incompatible `Classification`
//! or `IdeaState` across runtimes is precisely the drift this crate exists to
//! prevent (briefing §16).
//!
//! # What belongs here
//!
//! Value types, enumerations, identifiers and wire DTOs. This crate is
//! dependency-light and free of I/O so it can also compile to `wasm32`.
//!
//! # What does not belong here
//!
//! Server-side policy decisions, persistence types and anything whose logic
//! must not be shipped to a browser. Sharing a *type* with the Workspace is
//! safe; sharing an *authorization decision* is not (briefing §17).

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod access;
pub mod agentic;
pub mod avatar;
pub mod bibliography;
pub mod calendar;
pub mod classification;
pub mod compute;
pub mod error;
pub mod ids;
pub mod intelligence;
pub mod mail;
pub mod page;
pub mod readiness;
pub mod research;
pub mod roles;
pub mod storage;
pub mod system_capability;
pub mod temporal;

pub use access::{AccountStatus, CredentialKind, CredentialState, Permission, Scope, SessionState};
pub use agentic::{AgenticExposure, OperationId, TrustBoundary};
pub use avatar::{AvatarChoice, AVATAR_PRESETS};
pub use classification::Classification;
pub use compute::{ComputeNodeStatus, ComputeStatus, JobStatus, NodeKind};
pub use error::{ErrorBody, ErrorCode};
pub use ids::ResourceIdentifier;
pub use intelligence::{AiCapability, CapabilityStatus, IntelligenceStatus, ModelStatus, RagScope};
pub use mail::{
    ComposeAction, DraftOrigin, MailAddress, MailFolder, MailboxKind, OutboxState, RecipientScope,
    RemoteContentPolicy, SharedMailboxRole,
};
pub use page::{Page, PageRequest};
pub use research::{IdeaState, ProjectState, TaskState, WorkspaceKind};
pub use roles::{InstitutionalPosition, TechnicalRole, UnitRole, WorkspaceRole};
pub use storage::{MigrationState, Residency};
pub use system_capability::{
    SystemCapabilities, SystemCapability, SystemCapabilityReport, SystemCapabilityState,
};

/// Version of the HTTP contract exposed by the Ocinye Core.
///
/// The API is explicitly versioned: a breaking change means a new version, an
/// ADR and a CHANGELOG entry — never a silent reshape of `v1`.
pub const API_VERSION: &str = "v1";
