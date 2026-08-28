//! Knowledge: bibliography, notes and documents.
//!
//! # What belongs here
//!
//! What the institution reads, writes and keeps: bibliographic sources, the
//! conceptual notes made about them, the documents attached to a workspace, and
//! the typed links between research objects.
//!
//! # The copyright position
//!
//! Ocinye does not indiscriminately store full articles and books. Full content
//! is retained only where an explicit legal basis has been recorded on the
//! source; otherwise the institution keeps metadata, citation, notes and an
//! authorised link (briefing §30). This module is the enforcement point, backed
//! by a database constraint so the rule cannot be bypassed by another path.

mod model;
mod repository;
mod service;

pub use model::{ContentRight, Document, DocumentKind, Note, Source, SourceType};
pub use service::{
    attach_full_text, create_document, create_note, create_source, get_document, get_note,
    get_source, issue_download, link_objects, list_accessible_documents, list_accessible_sources,
    list_documents, list_links, list_notes, list_sources, record_operation_provenance,
    review_bibliography, update_note, NewDocument, NewNote, NewSource, UploadedFile,
};
