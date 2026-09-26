//! The Secrets Authority (ADR-0110, Part 6 of the generalization).
//!
//! # What belongs here
//!
//! The credentials an Instance configures for its providers and integrations:
//! an AI provider's API key, an OpenAI-compatible endpoint's token, a
//! connector's credential. The Core seals them, keeps their metadata, rotates
//! and revokes them, and opens one only for the Core service its scope names.
//!
//! # The flow
//!
//! ```text
//! administrator → Core (this module) → sealed at rest (instance-secrets domain)
//!               → a Core service in scope uses it → the application gets a result
//! ```
//!
//! **No route returns a plaintext**, and no application receives one. The only
//! way out of the seal is [`use_secret`], a Core function, and it demands the
//! scope the secret was created for.
//!
//! # What it is not
//!
//! The platform's own configuration secrets — the database URL, the object
//! store keys, the sealing root itself — stay in the operator's environment
//! (`/etc/ocinye/*.env`). They are the operator's, not the Instance
//! administrator's, and the root cannot seal itself.

mod repository;
mod service;

pub use service::{
    create_secret, list_secrets, revoke_secret, rotate_secret, use_secret, NewSecret, SecretScope,
    SecretSummary,
};
