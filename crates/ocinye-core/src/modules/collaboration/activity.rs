//! The workspace activity feed.
//!
//! Written from several modules, so it lives on its own rather than inside the
//! task service. An entry inherits the classification of what it describes, and
//! it is read back through the same visibility filter as everything else — an
//! activity feed must not become a side channel.

use ocinye_contracts::Classification;
use ocinye_domain::Principal;
use serde_json::Value;
use uuid::Uuid;

use crate::error::CoreResult;
use crate::Tx;

/// The kind of thing that happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivityKind {
    /// Something was created.
    Created,
    /// Something was updated.
    Updated,
    /// Something moved through its workflow.
    StateChanged,
    /// Someone commented.
    Commented,
    /// Someone joined a workspace.
    MemberAdded,
    /// An artefact was attached.
    Attached,
    /// Something was published.
    Published,
}

impl ActivityKind {
    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Updated => "updated",
            Self::StateChanged => "state_changed",
            Self::Commented => "commented",
            Self::MemberAdded => "member_added",
            Self::Attached => "attached",
            Self::Published => "published",
        }
    }
}

/// Append an activity entry inside the caller's transaction.
///
/// The summary is truncated to the column width rather than rejected: an
/// over-long title must not fail the operation it describes.
///
/// # Errors
///
/// Returns an error when the insert fails.
#[expect(
    clippy::too_many_arguments,
    reason = "um parâmetro por coluna: a alternativa é uma struct que só existe para atravessar esta chamada"
)]
pub async fn record_activity(
    tx: &mut Tx<'_>,
    principal: &Principal,
    workspace_id: Uuid,
    unit_id: Uuid,
    kind: ActivityKind,
    subject_type: &str,
    subject_id: Option<Uuid>,
    summary: &str,
    classification: Classification,
) -> CoreResult<()> {
    let summary: String = summary.chars().take(512).collect();

    sqlx::query(
        "INSERT INTO activity_entries
             (organisation_id, unit_id, workspace_id, actor_person_id, kind,
              subject_type, subject_id, summary, classification, context)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
    )
    .bind(principal.organisation_id)
    .bind(unit_id)
    .bind(workspace_id)
    .bind(principal.person_id)
    .bind(kind.as_str())
    .bind(subject_type)
    .bind(subject_id)
    .bind(summary)
    .bind(classification.as_str())
    .bind(Value::Object(serde_json::Map::new()))
    .execute(&mut **tx)
    .await?;
    Ok(())
}
