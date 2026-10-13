//! The Ocinye Update Engine (ADR-0032).
//!
//! The only authority over installed releases. It reads a channel's signed
//! metadata, downloads and verifies a release, checks the machine can take
//! it, stages it beside the running one, and installs it through an explicit
//! activation — the Instance by the release's own installer, the native layer
//! by an atomic pointer switch — behind a health gate, with rollback when the
//! gate fails. Its journal survives power cuts; at start it reconciles.
//!
//! It answers only the System Service, over a root-only socket; the System
//! Service has already decided who may ask.

pub mod base;
pub mod engine;
pub mod host;
pub mod native;
pub mod paths;
pub mod runner;
pub mod server;
pub mod source;
pub mod store;

pub use engine::{Engine, Fault, ImageFacts, Stop};

/// The System Service protocol this engine's machine speaks; a release that
/// speaks another is refused (`PROTOCOL_INCOMPATIBLE`). It is the System
/// Service's `CONTRACT_VERSION`.
pub const SYSTEM_PROTOCOL: u32 = 1;
