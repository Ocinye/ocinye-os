//! HTTP surface of the Ocinye Core.
//!
//! # Why this is a library and not only a binary
//!
//! The Workspace does not call [`ocinye_core`] directly. A member who submits a
//! form reaches the institution through an HTTP route in this crate, and only
//! then through the Core operation behind it. ADR-0307 claims that this entry
//! and the agentic entry converge on that same operation — and a claim about
//! the HTTP entry can only be verified by driving the HTTP entry.
//!
//! Exposing the router as a library lets the parity tests mount the real
//! [`routes::router`], with the real middleware and the real extractors, rather
//! than a reconstruction of it that would be free to drift.

pub mod bootstrap;
pub mod error;
pub mod extract;
pub mod mail_check;
pub mod middleware;
pub mod provision;
pub mod routes;
pub mod state;
