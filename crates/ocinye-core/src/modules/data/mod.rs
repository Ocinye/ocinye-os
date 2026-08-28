//! Data: datasets, versions and files.
//!
//! # What belongs here
//!
//! The catalogue of datasets the institution holds or uses, their immutable
//! versions, and the files that make up each version.
//!
//! # The versioning invariant
//!
//! A published version is never silently overwritten (briefing §31). Publishing
//! creates a new immutable row with its own checksums and provenance; earlier
//! versions stay readable and citable, because a result that cited version 1
//! must still be reproducible after version 2 exists.

mod model;
mod repository;
mod service;

pub use model::{Dataset, DatasetFile, DatasetOrigin, DatasetState, DatasetVersion, VersionStatus};
pub use service::{
    add_version_file, create_dataset, create_version, get_dataset, get_dataset_version,
    list_datasets, list_versions, publish_version, NewDataset, NewVersion,
};
