//! Explicit access grants.
//!
//! The escape hatch for `RESTRICTED` material, deliberately built so that using
//! it is more work than membership: someone must name the permission, name the
//! scope, write down why, and be recorded as having done so (briefing §63).

use chrono::{DateTime, Utc};
use ocinye_contracts::{Permission, Scope};
use ocinye_observability::CorrelationIds;
use serde::Serialize;
use sqlx::{PgExecutor, PgPool, Row};
use uuid::Uuid;

use crate::audit::{self, action, AuditEntry};
use crate::error::{CoreError, CoreResult};
use ocinye_domain::Principal;

/// A grant as shown in the administration interface.
#[derive(Debug, Clone, Serialize)]
pub struct GrantView {
    /// Identifier.
    pub id: Uuid,
    /// Who holds it.
    pub subject_id: Uuid,
    /// Their name, for display.
    pub subject_name: String,
    /// What is granted.
    pub permission: String,
    /// Where it applies.
    pub scope: String,
    /// Which unit, workspace or resource.
    pub scope_id: Option<Uuid>,
    /// Why it was granted.
    pub reason: String,
    /// Who granted it.
    pub granted_by_name: String,
    /// When.
    pub granted_at: DateTime<Utc>,
    /// When it lapses, if it does.
    pub expires_at: Option<DateTime<Utc>>,
    /// When it was revoked, if it was.
    pub revoked_at: Option<DateTime<Utc>>,
}

/// What is needed to create a grant.
#[derive(Debug, Clone)]
pub struct NewGrant {
    /// Who receives it.
    pub subject_id: Uuid,
    /// What they receive.
    pub permission: Permission,
    /// Where it applies.
    pub scope: Scope,
    /// Which unit, workspace or resource.
    pub scope_id: Option<Uuid>,
    /// Why. Recorded verbatim and required.
    pub reason: String,
    /// When it lapses.
    pub expires_at: Option<DateTime<Utc>>,
}

/// Create a grant.
///
/// # Errors
///
/// Returns [`CoreError::Validation`] when the scope and its identifier
/// disagree or the reason is too thin, and [`CoreError::Conflict`] when an
/// identical live grant already exists.
pub async fn create(
    pool: &PgPool,
    actor: &Principal,
    new: &NewGrant,
    ids: &CorrelationIds,
) -> CoreResult<Uuid> {
    let reason = new.reason.trim();
    if reason.chars().count() < 8 {
        return Err(CoreError::Validation(
            "Indique a razão do acesso — será revista mais tarde.".to_owned(),
        ));
    }

    match (new.scope, new.scope_id) {
        (Scope::Institution, Some(_)) => {
            return Err(CoreError::Validation(
                "Um grant institucional não nomeia um alvo.".to_owned(),
            ))
        }
        (Scope::Institution, None) => {}
        (_, None) => {
            return Err(CoreError::Validation(
                "Um grant com âmbito tem de nomear o seu alvo.".to_owned(),
            ))
        }
        (_, Some(_)) => {}
    }

    if let Some(expires_at) = new.expires_at {
        if expires_at <= Utc::now() {
            return Err(CoreError::Validation(
                "A validade do grant já passou.".to_owned(),
            ));
        }
    }

    let mut tx = pool.begin().await?;

    let id: Option<Uuid> = sqlx::query_scalar(
        "INSERT INTO explicit_access_grants
             (organisation_id, subject_id, permission, scope, scope_id, reason,
              granted_by_id, expires_at)
         SELECT $1, $2, $3, $4, $5, $6, $7, $8
          WHERE EXISTS (SELECT 1 FROM people WHERE id = $2 AND organisation_id = $1)
         ON CONFLICT DO NOTHING
         RETURNING id",
    )
    .bind(actor.organisation_id)
    .bind(new.subject_id)
    .bind(new.permission.as_str())
    .bind(new.scope.as_str())
    .bind(new.scope_id)
    .bind(reason)
    .bind(actor.person_id)
    .bind(new.expires_at)
    .fetch_optional(&mut *tx)
    .await?;

    let id = id.ok_or_else(|| {
        CoreError::Conflict("Este acesso já foi concedido, ou o membro não existe.".to_owned())
    })?;

    audit::record(
        &mut tx,
        Some(actor),
        ids,
        AuditEntry::new(action::GRANT_CREATED, "access_grant")
            .resource(id)
            .detail("subject_id", new.subject_id.to_string())
            .detail("permission", new.permission.as_str())
            .detail("scope", new.scope.as_str())
            .detail("scope_id", new.scope_id.map(|id| id.to_string()))
            .detail("reason", reason)
            .detail("expires_at", new.expires_at.map(|at| at.to_rfc3339())),
    )
    .await?;

    tx.commit().await?;
    Ok(id)
}

/// Revoke a grant.
///
/// Revocation is a timestamp, never a delete: that an access existed is part of
/// the institutional record (`CLAUDE.md` §58).
///
/// # Errors
///
/// Returns [`CoreError::NotFound`] when no live grant with that identifier
/// belongs to the actor's organisation.
pub async fn revoke(
    pool: &PgPool,
    actor: &Principal,
    grant_id: Uuid,
    reason: &str,
    ids: &CorrelationIds,
) -> CoreResult<()> {
    let mut tx = pool.begin().await?;

    let affected = sqlx::query(
        "UPDATE explicit_access_grants
            SET revoked_at = now(), revoked_by_id = $3, revoked_reason = $4
          WHERE id = $1 AND organisation_id = $2 AND revoked_at IS NULL",
    )
    .bind(grant_id)
    .bind(actor.organisation_id)
    .bind(actor.person_id)
    .bind(reason.trim())
    .execute(&mut *tx)
    .await?
    .rows_affected();

    if affected == 0 {
        return Err(CoreError::NotFound("Grant não encontrado.".to_owned()));
    }

    audit::record(
        &mut tx,
        Some(actor),
        ids,
        AuditEntry::new(action::GRANT_REVOKED, "access_grant")
            .resource(grant_id)
            .detail("reason", reason.trim()),
    )
    .await?;

    tx.commit().await?;
    Ok(())
}

/// Grants held by one person, live and historical.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn for_subject<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    subject_id: Uuid,
) -> CoreResult<Vec<GrantView>> {
    let rows = sqlx::query(
        "SELECT g.id, g.subject_id, s.full_name AS subject_name, g.permission, g.scope,
                g.scope_id, g.reason, b.full_name AS granted_by_name, g.granted_at,
                g.expires_at, g.revoked_at
           FROM explicit_access_grants g
           JOIN people s ON s.id = g.subject_id
           JOIN people b ON b.id = g.granted_by_id
          WHERE g.organisation_id = $1 AND g.subject_id = $2
          ORDER BY g.granted_at DESC",
    )
    .bind(organisation_id)
    .bind(subject_id)
    .fetch_all(executor)
    .await?;

    rows.into_iter().map(view_from_row).collect()
}

fn view_from_row(row: sqlx::postgres::PgRow) -> CoreResult<GrantView> {
    Ok(GrantView {
        id: row.try_get("id")?,
        subject_id: row.try_get("subject_id")?,
        subject_name: row.try_get("subject_name")?,
        permission: row.try_get("permission")?,
        scope: row.try_get("scope")?,
        scope_id: row.try_get("scope_id")?,
        reason: row.try_get("reason")?,
        granted_by_name: row.try_get("granted_by_name")?,
        granted_at: row.try_get("granted_at")?,
        expires_at: row.try_get("expires_at")?,
        revoked_at: row.try_get("revoked_at")?,
    })
}
