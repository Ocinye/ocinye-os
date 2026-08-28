//! Institutional domains of the Ocinye Core.
//!
//! Every module has the same shape (ADR-0006):
//!
//! - `model` — persistence rows, private to the module;
//! - `repository` — explicit SQL;
//! - `service` — the application layer: authorization, invariants, domain
//!   events and auditing.
//!
//! **Every state change goes through a service.** A repository is never called
//! from outside its own module, and HTTP handlers never call one directly:
//! that is what keeps authorization from being something a route can forget.
//!
//! Modules import each other only through these `pub` re-exports, never through
//! another module's `model` or `repository`.

pub mod agentic;
pub mod calendar;
pub mod collaboration;
pub mod compute;
pub mod data;
pub mod governance;
pub mod identity;
pub mod intelligence;
pub mod knowledge;
pub mod mail;
pub mod messaging;
pub mod organisation;
pub mod platform;
pub mod research;
pub mod science;
pub mod search;
