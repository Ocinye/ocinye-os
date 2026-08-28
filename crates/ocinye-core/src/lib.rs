//! Ocinye Core — the institutional kernel of the Ocinye OS.
//!
//! This crate owns institutional state: it persists it, enforces the policy
//! from [`ocinye_domain`] over it, records what happened, and emits domain
//! events. It is not the backend of a website; it is the kernel that other
//! runtimes — Workspace, Worker, Node Agent, future CLIs and notebooks — sit on
//! top of.
//!
//! # Shape of a module
//!
//! Every institutional domain lives under [`modules`] with the same shape
//! (ADR-0006):
//!
//! - `model` — persistence rows, private to the module;
//! - `repository` — explicit SQL;
//! - `service` — the application layer: authorization, invariants, events,
//!   auditing. **Every state change goes through here.**
//!
//! # The transaction rule
//!
//! A state change, its domain event and its audit record commit together or not
//! at all. Services therefore take a transaction rather than a pool, and the
//! caller decides the boundary.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod audit;
pub mod authn;
pub mod authority;
pub mod avatar;
pub mod capabilities;
pub mod config;
pub mod db;
pub mod error;
pub mod modules;
pub mod operations;
pub mod outbox;
pub mod password;
pub mod readiness;
pub mod storage;
pub mod visibility;

pub use config::CoreConfig;
pub use error::{CoreError, CoreResult};

/// Transaction type used throughout the application layer.
pub type Tx<'t> = sqlx::Transaction<'t, sqlx::Postgres>;
