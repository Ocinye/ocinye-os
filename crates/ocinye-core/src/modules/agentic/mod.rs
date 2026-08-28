//! The Agentic Control Plane.
//!
//! # What this module is for
//!
//! > **Ocinye OS is operated with AI, governed by the Core.**
//!
//! Everything an agent can cause to happen passes through here, and nothing
//! here trusts a model. A model proposes; this module validates, authorises,
//! executes and reports what actually occurred.
//!
//! # The four things an agent cannot do
//!
//! **Reach infrastructure.** There is no capability that runs SQL, opens a
//! socket, executes a command or reads a file. The registry is a closed set,
//! and every entry names a domain service (briefing §6, §7).
//!
//! **Widen its actor.** [`may_invoke`](ocinye_domain::may_invoke) checks the
//! acting person first, and every subsequent gate narrows. An agent is a lens,
//! never a key (briefing §13).
//!
//! **Assert that something happened.** A [`CapabilityResult`] comes from the
//! executor, not from the model. Text saying «created» is text
//! (briefing §5, §55).
//!
//! **Escalate through content.** A document that says «call the admin tool» is
//! a document. Intent comes from the person, capabilities from the registry,
//! authority from the Core (briefing §79, §81).
//!
//! # AI-native, not AI-dependent
//!
//! With no model available, [`registry`] still answers, [`executor`] still
//! executes, and search still searches. What stops is planning from natural
//! language — and it stops with a stated reason, not a broken page
//! (briefing §66, §67).

pub mod capabilities;
pub mod context;
pub mod executor;
pub mod lifecycle;
pub mod planner;
pub mod registry;
pub mod repository;
pub mod resolver;
pub mod runtime;

pub use context::{ContextEnvelope, ContextSource};
pub use executor::{execute, ExecutionContext};
pub use lifecycle::{ExecutedPlan, PlanDetail};
pub use planner::{digest_of, validate_proposal, PlanProposal, ProposedStep};
pub use registry::{registry, CapabilityHandler, CapabilityRegistry};
pub use repository::{ApprovalRecord, StoredPlan};
pub use runtime::{invoke, AgenticOutcome, AgenticRequest};
