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
pub mod audio;
pub mod audit;
pub mod authority;
pub mod bluetooth;
pub mod dbus;
pub mod display;
pub mod files;
pub mod linux;
pub mod netplan;
pub mod server;
pub mod service;
pub mod tool;
pub mod update_link;
pub mod wifi;

#[cfg(test)]
mod tests;
