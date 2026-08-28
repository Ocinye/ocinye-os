//! Governance: reading the audit trail.
//!
//! # What belongs here
//!
//! Authorised access to the append-only audit trail written by
//! [`crate::audit`], and the explicit access grants that are the only way to
//! reach `RESTRICTED` material without membership ([`grants`]).
//!
//! # What does not belong here
//!
//! Writing audit records. Every module writes its own, inside the transaction
//! of the action being audited — an audit trail assembled afterwards by a
//! separate component is one that can silently miss things.

pub mod grants;
mod model;
mod repository;
mod service;

pub use grants::{GrantView, NewGrant};
pub use model::AuditRecord;
pub use service::{list_audit, AuditQuery};
