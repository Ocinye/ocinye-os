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

pub mod document;
mod model;
mod repository;
mod service;

pub use document::{Block, ChecklistItem, Inline, Mark, NoteDocument, SCHEMA_VERSION};
pub use model::{ContentRight, Document, DocumentKind, Note, Source, SourceType};
pub use repository::{NoteRevisionContent, NoteRevisionMeta, NoteShareRow};
pub use service::{
    attach_full_text, create_document, create_note, create_personal_note, create_source,
    delete_personal_note, deleted_personal_notes, get_document, get_note, get_personal_note,
    get_source, issue_download, link_objects, list_accessible_documents, list_accessible_sources,
    list_documents, list_links, list_notes, list_personal_note_shares, list_personal_notes,
    list_sources, member_can_view_note_file, move_personal_note, note_activity,
    note_notify_recipients, notes_shared_with_me, personal_note_revision_content,
    personal_note_revisions, purge_personal_note, record_operation_provenance,
    restore_personal_note, restore_personal_note_revision, review_bibliography,
    revoke_personal_note_share, share_personal_note, update_note, update_personal_note,
    NewDocument, NewNote, NewSource, NoteAccess, PersonalNoteEdit, UploadedFile,
};
