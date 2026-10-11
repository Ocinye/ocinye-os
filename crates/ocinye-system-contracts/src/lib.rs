//! Typed contracts of the Ocinye System Service.
//!
//! *Rust controls the system; native system surfaces use TypeScript, HTML and
//! CSS to present the system. Applications may use their approved framework,
//! but never gain direct system authority.*
//!
//! This crate is the one definition of what can be asked of the installed
//! system: the capability catalogue, each request and its arguments, the state
//! that comes back, the reason codes, the events and the wire format. The
//! service, the native interface (through generated TypeScript bindings), the
//! headless client and the tests all read it. Nothing here executes anything.
//!
//! There is no capability that takes a command, and there never will be one:
//! [`capability`] has the test that measures it.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod capability;
pub mod event;
pub mod reason;
pub mod request;
pub mod session;
pub mod state;
pub mod wire;

/// The contract version. A client sends it first; the service refuses any
/// other, and nothing is invoked across a mismatch.
pub const CONTRACT_VERSION: u16 = 1;

/// Where the service listens.
pub const SOCKET_PATH: &str = "/run/ocinye/system.sock";
