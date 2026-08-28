//! Audit persistence, read side only.

use chrono::{DateTime, Utc};
use sqlx::PgExecutor;
use uuid::Uuid;

use super::model::AuditRecord;
use crate::error::CoreResult;

const SELECT: &str = "SELECT a.id, a.occurred_at, a.actor_person_id, p.full_name AS actor_name,
                             a.action, a.resource_type, a.resource_id, a.unit_id, a.workspace_id,
                             a.classification, a.outcome, a.correlation_id, a.metadata
                        FROM audit_events a
                        LEFT JOIN people p ON p.id = a.actor_person_id";

/// List audit records matching the filters.
///
/// # Errors
///
/// Returns an error when the query fails.
#[expect(
    clippy::too_many_arguments,
    reason = "um parâmetro por coluna: a alternativa é uma struct que só existe para atravessar esta chamada"
)]
pub async fn list<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    resource_type: Option<&str>,
    resource_id: Option<Uuid>,
    actor_person_id: Option<Uuid>,
    since: Option<DateTime<Utc>>,
    limit: i64,
    offset: i64,
) -> CoreResult<Vec<AuditRecord>> {
    let records = sqlx::query_as::<_, AuditRecord>(&format!(
        "{SELECT}
          WHERE a.organisation_id = $1
            AND ($2::text IS NULL OR a.resource_type = $2)
            AND ($3::uuid IS NULL OR a.resource_id = $3)
            AND ($4::uuid IS NULL OR a.actor_person_id = $4)
            AND ($5::timestamptz IS NULL OR a.occurred_at >= $5)
          ORDER BY a.occurred_at DESC
          LIMIT $6 OFFSET $7"
    ))
    .bind(organisation_id)
    .bind(resource_type)
    .bind(resource_id)
    .bind(actor_person_id)
    .bind(since)
    .bind(limit)
    .bind(offset)
    .fetch_all(executor)
    .await?;
    Ok(records)
}

/// Count audit records matching the filters.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn count<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    resource_type: Option<&str>,
    resource_id: Option<Uuid>,
    actor_person_id: Option<Uuid>,
    since: Option<DateTime<Utc>>,
) -> CoreResult<i64> {
    let total = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM audit_events
          WHERE organisation_id = $1
            AND ($2::text IS NULL OR resource_type = $2)
            AND ($3::uuid IS NULL OR resource_id = $3)
            AND ($4::uuid IS NULL OR actor_person_id = $4)
            AND ($5::timestamptz IS NULL OR occurred_at >= $5)",
    )
    .bind(organisation_id)
    .bind(resource_type)
    .bind(resource_id)
    .bind(actor_person_id)
    .bind(since)
    .fetch_one(executor)
    .await?;
    Ok(total)
}
