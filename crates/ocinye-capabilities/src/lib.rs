//! Ocinye SystemCapability Runtime.
//!
//! Institutional capabilities — importing BibTeX, extracting metadata,
//! validating a dataset, processing a result — run as WebAssembly components
//! under explicit, declared permissions.
//!
//! # Why WebAssembly here, and not everywhere
//!
//! Running these in the Core's own process would hand every one of them the
//! database, the filesystem, the network and the secrets, by default. That is
//! the problem this crate exists to solve. WASM earns its place here because it
//! gives isolation, portability, explicit permissions and resource limits at
//! once — not because WASM is available (ADR-0501, briefing §64).
//!
//! # Deny by default
//!
//! A capability receives only what its [`manifest::Manifest`] requests *and*
//! institutional policy approves. With no declaration it gets: no network, no
//! host filesystem, no environment, no clock beyond what WASI provides, and
//! bounded fuel, memory and wall time.
//!
//! # WebAssembly is not magic
//!
//! The sandbox is one layer. Input validation, authorization before invocation,
//! resource limits and provenance of the result all still apply.
//!
//! # Status
//!
//! Manifest, permission model, host runtime with limits, and one example
//! capability. This is a foundation, not a plugin marketplace.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod error;
pub mod manifest;
pub mod runtime;

pub use error::{CapabilityError, CapabilityResult};
pub use manifest::{FilesystemPolicy, Manifest, NetworkPolicy, ResourceLimits};
pub use runtime::{CapabilityRuntime, Invocation, InvocationOutcome};
