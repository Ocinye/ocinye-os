//! The Ocinye System Service: the one privileged authority of an installed
//! Ocinye OS.
//!
//! *Rust controls the system.* Every change to the machine that an interface,
//! the headless client or a Core proposal can cause passes through
//! [`service::Service::call`]: standing, authorisation, fresh proof,
//! validation, then a typed adapter. There is no path from a client to a
//! command line.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod activation;
pub mod audit;
pub mod authority;
pub mod linux;
pub mod server;
pub mod service;
pub mod tool;

#[cfg(test)]
mod tests;
