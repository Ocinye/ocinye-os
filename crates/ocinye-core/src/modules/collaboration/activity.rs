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
    /// Access was granted to someone.
    Shared,
    /// A grant was revoked.
    Revoked,
    /// Something was moved to the bin.
    Deleted,
    /// Something was brought back from the bin.
    Restored,
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
            Self::Shared => "shared",
            Self::Revoked => "revoked",
            Self::Deleted => "deleted",
            Self::Restored => "restored",
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

/// Append an owner-scoped activity entry inside the caller's transaction.
///
/// The counterpart to [`record_activity`] for a personal artefact — a note the
/// member owns, with no workspace. The entry belongs to the owner, is always
/// `INTERNAL`, and records the acting person, which for a shared note may be a
/// collaborator rather than the owner (ADR-0413).
///
/// # Errors
///
/// Returns an error when the insert fails.
pub async fn record_personal_activity(
    tx: &mut Tx<'_>,
    principal: &Principal,
    owner_id: Uuid,
    kind: ActivityKind,
    subject_type: &str,
    subject_id: Uuid,
    summary: &str,
) -> CoreResult<()> {
    let summary: String = summary.chars().take(512).collect();
    sqlx::query(
        "INSERT INTO activity_entries
             (organisation_id, owner_id, actor_person_id, kind,
              subject_type, subject_id, summary, classification, context)
         VALUES ($1, $2, $3, $4, $5, $6, $7, 'INTERNAL', $8)",
    )
    .bind(principal.organisation_id)
    .bind(owner_id)
    .bind(principal.person_id)
    .bind(kind.as_str())
    .bind(subject_type)
    .bind(subject_id)
    .bind(summary)
    .bind(Value::Object(serde_json::Map::new()))
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// One entry of an owner-scoped activity feed: who, what, and when.
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct PersonalActivity {
    /// What happened (`created`, `updated`, `shared`, `deleted`, …).
    pub kind: String,
    /// A short human summary.
    pub summary: String,
    /// The name of the person who did it, when still known.
    pub actor_name: Option<String>,
    /// When it happened.
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// The activity of one owner-scoped subject (for example a personal note),
/// newest first. Access is the **caller's** business to check first — this reads
/// what the owner's feed holds for that subject.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn list_personal_activity(
    pool: &sqlx::PgPool,
    owner_id: Uuid,
    subject_type: &str,
    subject_id: Uuid,
    limit: i64,
) -> CoreResult<Vec<PersonalActivity>> {
    let rows = sqlx::query_as::<_, PersonalActivity>(
        "SELECT a.kind, a.summary, p.full_name AS actor_name, a.created_at
           FROM activity_entries a
           LEFT JOIN people p ON p.id = a.actor_person_id
          WHERE a.owner_id = $1 AND a.subject_type = $2 AND a.subject_id = $3
          ORDER BY a.created_at DESC
          LIMIT $4",
    )
    .bind(owner_id)
    .bind(subject_type)
    .bind(subject_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
