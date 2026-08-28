//! Search rows and status.

use serde::Serialize;
use sqlx::FromRow;
use uuid::Uuid;

/// One search result.
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct SearchHit {
    /// Kind of artefact, for example `idea` or `dataset`.
    pub entity_type: String,
    /// Identifier of the artefact.
    pub entity_id: Uuid,
    /// Title.
    pub title: String,
    /// Bounded excerpt.
    pub excerpt: Option<String>,
    /// Classification, so the interface can show it alongside the result.
    pub classification: String,
    /// Owning workspace, when the artefact has one.
    pub workspace_id: Option<Uuid>,
    /// Lexical relevance.
    pub rank: f32,
}

/// Whether semantic search can be offered.
///
/// With no Ocinye AI node there are no embeddings. This reports that truthfully
/// rather than degrading silently to lexical results labelled as semantic
/// (`CLAUDE.md` §69).
#[derive(Debug, Clone, Serialize)]
pub struct SemanticAvailability {
    /// Whether semantic search is available.
    pub available: bool,
    /// Number of indexed documents carrying an embedding.
    pub embedded_documents: i64,
    /// Explanation for the interface.
    pub message: String,
}
