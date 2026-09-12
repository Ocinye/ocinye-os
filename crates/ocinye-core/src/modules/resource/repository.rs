//! Resource governance persistence — explicit SQL.

use chrono::{DateTime, Utc};
use ocinye_contracts::{
    AllocationSource, ProfileStatus, ResourceScopeType, ResourceType, ResourceUnit,
    SchedulingPriority,
};
use sqlx::{PgExecutor, Row};
use uuid::Uuid;

use super::model::{Allocation, ProfileRule, ResourceProfile};
use crate::error::CoreResult;

fn profile_from_row(row: &sqlx::postgres::PgRow) -> CoreResult<ResourceProfile> {
    let priority: String = row.try_get("priority")?;
    let status: String = row.try_get("status")?;
    Ok(ResourceProfile {
        id: row.try_get("id")?,
        code: row.try_get("code")?,
        name: row.try_get("name")?,
        description: row.try_get("description")?,
        is_default: row.try_get("is_default")?,
        priority: SchedulingPriority::parse(&priority).unwrap_or(SchedulingPriority::Normal),
        status: ProfileStatus::parse(&status).unwrap_or(ProfileStatus::Inactive),
    })
}

/// Every profile of an organisation, active ones first, then by name.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn list_profiles<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
) -> CoreResult<Vec<ResourceProfile>> {
    let rows = sqlx::query(
        "SELECT id, code, name, description, is_default, priority, status
           FROM resource_profiles
          WHERE organisation_id = $1
          ORDER BY (status = 'active') DESC, is_default DESC, lower(name)",
    )
    .bind(organisation_id)
    .fetch_all(executor)
    .await?;

    rows.iter().map(profile_from_row).collect()
}

/// The default profile of an organisation, if one is set.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn default_profile<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
) -> CoreResult<Option<ResourceProfile>> {
    let row = sqlx::query(
        "SELECT id, code, name, description, is_default, priority, status
           FROM resource_profiles
          WHERE organisation_id = $1 AND is_default
          LIMIT 1",
    )
    .bind(organisation_id)
    .fetch_optional(executor)
    .await?;

    row.as_ref().map(profile_from_row).transpose()
}

/// The typed rules of a profile.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn profile_rules<'e>(
    executor: impl PgExecutor<'e>,
    profile_id: Uuid,
) -> CoreResult<Vec<ProfileRule>> {
    let rows = sqlx::query(
        "SELECT resource_type, quantity, unit
           FROM resource_profile_rules
          WHERE profile_id = $1
          ORDER BY resource_type",
    )
    .bind(profile_id)
    .fetch_all(executor)
    .await?;

    Ok(rows
        .iter()
        .filter_map(|row| {
            let resource_type =
                ResourceType::parse(row.try_get::<String, _>("resource_type").ok()?.as_str())?;
            let unit = ResourceUnit::parse(row.try_get::<String, _>("unit").ok()?.as_str())?;
            Some(ProfileRule {
                resource_type,
                quantity: row.try_get("quantity").ok()?,
                unit,
            })
        })
        .collect())
}

/// One profile rule for a specific resource, if present.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn profile_rule<'e>(
    executor: impl PgExecutor<'e>,
    profile_id: Uuid,
    resource_type: ResourceType,
) -> CoreResult<Option<ProfileRule>> {
    let row = sqlx::query(
        "SELECT resource_type, quantity, unit
           FROM resource_profile_rules
          WHERE profile_id = $1 AND resource_type = $2",
    )
    .bind(profile_id)
    .bind(resource_type.as_str())
    .fetch_optional(executor)
    .await?;

    let Some(row) = row else { return Ok(None) };
    let unit: String = row.try_get("unit")?;
    Ok(Some(ProfileRule {
        resource_type,
        quantity: row.try_get("quantity")?,
        unit: ResourceUnit::parse(&unit).unwrap_or_else(|| resource_type.canonical_unit()),
    }))
}

/// The live allocations of a scope for one resource — active status, not
/// expired at `now`.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn active_allocations<'e>(
    executor: impl PgExecutor<'e>,
    scope_type: ResourceScopeType,
    scope_id: Uuid,
    resource_type: ResourceType,
    now: DateTime<Utc>,
) -> CoreResult<Vec<Allocation>> {
    let rows = sqlx::query(
        "SELECT id, resource_type, unit, scope_type, scope_id, quantity, source,
                starts_at, expires_at, reason
           FROM resource_allocations
          WHERE scope_type = $1 AND scope_id = $2 AND resource_type = $3
            AND status = 'active'
            AND starts_at <= $4
            AND (expires_at IS NULL OR expires_at > $4)
          ORDER BY created_at",
    )
    .bind(scope_type.as_str())
    .bind(scope_id)
    .bind(resource_type.as_str())
    .bind(now)
    .fetch_all(executor)
    .await?;

    Ok(rows
        .iter()
        .filter_map(|row| {
            let source =
                AllocationSource::parse(row.try_get::<String, _>("source").ok()?.as_str())?;
            let unit = ResourceUnit::parse(row.try_get::<String, _>("unit").ok()?.as_str())?;
            Some(Allocation {
                id: row.try_get("id").ok()?,
                resource_type,
                unit,
                scope_type,
                scope_id,
                quantity: row.try_get("quantity").ok()?,
                source,
                starts_at: row.try_get("starts_at").ok()?,
                expires_at: row.try_get("expires_at").ok()?,
                reason: row.try_get("reason").ok()?,
            })
        })
        .collect())
}
