//! Governance application layer.

use chrono::{DateTime, Utc};
use ocinye_contracts::PageRequest;
use ocinye_domain::policy::{authorize, Action, ResourceContext, ResourceKind};
use ocinye_domain::Principal;
use sqlx::PgPool;
use uuid::Uuid;

use super::model::AuditRecord;
use super::repository as repo;
use crate::error::{CoreError, CoreResult};

/// Filters for an audit query.
#[derive(Debug, Clone, Default)]
pub struct AuditQuery {
    /// Restrict to a kind of resource.
    pub resource_type: Option<String>,
    /// Restrict to one resource.
    pub resource_id: Option<Uuid>,
    /// Restrict to one actor.
    pub actor_person_id: Option<Uuid>,
    /// Restrict to records from this moment onwards.
    pub since: Option<DateTime<Utc>>,
}

/// Read the audit trail.
///
/// Requires the `auditor` role or an administrative role — and grants nothing
/// else: an auditor can see *that* an action happened without gaining access to
/// the institutional content it acted on (ADR-0100).
///
/// # Errors
///
/// Returns [`CoreError::NotFound`] when the caller may not read the trail. The
/// denial hides existence, so probing it reveals nothing.
pub async fn list_audit(
    pool: &PgPool,
    principal: &Principal,
    query: AuditQuery,
    page: PageRequest,
) -> CoreResult<(Vec<AuditRecord>, i64)> {
    let ctx = ResourceContext::organisation(ResourceKind::AuditEvent, principal.organisation_id);
    authorize(principal, Action::ReadAudit, &ctx)
        .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    let records = repo::list(
        pool,
        principal.organisation_id,
        query.resource_type.as_deref(),
        query.resource_id,
        query.actor_person_id,
        query.since,
        page.limit(),
        page.offset(),
    )
    .await?;

    let total = repo::count(
        pool,
        principal.organisation_id,
        query.resource_type.as_deref(),
        query.resource_id,
        query.actor_person_id,
        query.since,
    )
    .await?;

    Ok((records, total))
}
