//! Institutional search.
//!
//! # What belongs here
//!
//! Maintenance of the search index and permission-aware querying of it.
//!
//! # The invariant
//!
//! Search is not a way around permissions. The authorization predicate is part
//! of every query, so `LIMIT`, `OFFSET`, `COUNT`, facets and suggestions all
//! operate on the authorised set only. Nothing here may reveal the existence of
//! an artefact the caller cannot read (ADR-0202).

mod model;
mod repository;
mod service;

pub use model::{BodyHit, SearchHit, SemanticAvailability};
pub use service::{
    index_entity, remove_entity, search, search_bodies, semantic_availability, IndexRequest,
};
