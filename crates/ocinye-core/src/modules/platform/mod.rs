//! Platform: what this installation can actually do right now.
//!
//! # What belongs here
//!
//! The single answer to *can the system do X*, assembled from the real state of
//! each plane: registered AI models, registered compute nodes, configured object
//! storage, the search index.
//!
//! # What does not belong here
//!
//! Authorization. This module never asks *may this person*, only *can the
//! system*. The two are separate concerns and conflating them produces the two
//! worst messages a system can give (briefing §57):
//!
//! - "não tem permissão" when the hardware is simply not installed;
//! - "indisponível" when in fact the person is not allowed.
//!
//! # Why centralised
//!
//! Without it, `if no_gpu` appears in twenty components and drifts apart. Here
//! it is decided once and read everywhere (briefing §54).

mod service;

pub use service::system_capabilities;
