//! Password handling: secrets, policy, hashing and generation.
//!
//! Ocinye Core owns username-and-password authentication (ADR-0103, which
//! supersedes ADR-0102). This module is the whole of the credential surface
//! that decision created, deliberately kept in one place so it can be reviewed
//! as one thing.
//!
//! # The four pieces
//!
//! - [`secret`] — a value that cannot be logged by accident.
//! - [`policy`] — what makes a password acceptable, and the one normalisation.
//! - [`hashing`] — Argon2id verifiers in PHC form, with transparent rehashing.
//! - [`generate`] — CSPRNG temporary credentials.
//! - [`blocklist`] — refusal of known-bad passwords.
//!
//! # What is never here
//!
//! Storage. A plaintext password exists in this process for the length of one
//! request and is never written anywhere but a hash (briefing §95).

pub mod blocklist;
pub mod generate;
pub mod hashing;
pub mod policy;
pub mod secret;

pub use blocklist::BlockReason;
pub use generate::temporary_credential;
pub use hashing::{Hasher, HashingParams};
pub use policy::{assess, normalise, validate, Rejection, Strength, MAX_LENGTH, MIN_LENGTH};
pub use secret::Secret;
