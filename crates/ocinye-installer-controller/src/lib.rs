//! The local controller of the Ocinye OS Installer (D011, ADR-0022).
//!
//! Runs on the operator's computer. It verifies the release locally, talks to
//! the server over SSH with a pinned host key, uploads and verifies the
//! temporary executor (`ocinye-bootstrap`), builds and seals the installation
//! plan, transfers the release, follows the typed events, verifies the product
//! from the outside, and writes the non-secret receipt.
//!
//! It never opens a shell for anyone, never sends a command line it did not
//! build from a constant template, and never writes a secret to disk, a log or
//! the receipt.

#![forbid(unsafe_code)]

pub mod app;
pub mod bundle;
pub mod hostkeys;
pub mod installer;
pub mod opverify;
pub mod planner;
pub mod ssh;
pub mod tls;
pub mod ui;
