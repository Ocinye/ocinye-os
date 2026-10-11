//! The Ocinye Session Service: the Rust authority of the local interactive
//! session.
//!
//! The Core is the authority for who a member is. This service is the
//! authority for whether that member's session on this machine is active or
//! locked. Lock state lives here, never in an interface: a front end can ask
//! for the session to be unlocked, and only this service can unlock it.
//!
//! Unlocking does not need the Core. After a Core-backed sign-in the service
//! keeps a local verifier for that one session, and the policy the Core signed
//! decides whether, and for how long, the session may be unlocked while the
//! Core cannot be asked.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod engine;
pub mod policy;
pub mod server;
pub mod verifier;

#[cfg(test)]
mod tests;
