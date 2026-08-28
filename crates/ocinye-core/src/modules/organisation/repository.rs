//! Organisation persistence.

use ocinye_contracts::UnitRole;
use sqlx::PgExecutor;
use uuid::Uuid;

use super::model::{Organisation, Unit, UnitMember};
use crate::error::CoreResult;

const UNIT_COLUMNS: &str = "id, organisation_id, code, name, description, research_areas,
                            status, archived_at, created_at";

/// Find the organisation by slug.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn find_organisation_by_slug<'e>(
    executor: impl PgExecutor<'e>,
    slug: &str,
) -> CoreResult<Option<Organisation>> {
    let organisation = sqlx::query_as::<_, Organisation>(
        "SELECT id, slug, name, legal_name, country, description, created_at
           FROM organisations WHERE slug = $1",
    )
    .bind(slug)
    .fetch_optional(executor)
    .await?;
    Ok(organisation)
}

/// Insert the organisation.
///
/// # Errors
///
/// Returns an error when the insert fails.
pub async fn insert_organisation<'e>(
    executor: impl PgExecutor<'e>,
    slug: &str,
    name: &str,
    country: Option<&str>,
) -> CoreResult<Organisation> {
    let organisation = sqlx::query_as::<_, Organisation>(
        "INSERT INTO organisations (slug, name, country) VALUES ($1, $2, $3)
         RETURNING id, slug, name, legal_name, country, description, created_at",
    )
    .bind(slug)
    .bind(name)
    .bind(country)
    .fetch_one(executor)
    .await?;
    Ok(organisation)
}

/// Load a unit within an organisation.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn find_unit<'e>(
    executor: impl PgExecutor<'e>,
    id: Uuid,
    organisation_id: Uuid,
) -> CoreResult<Option<Unit>> {
    let unit = sqlx::query_as::<_, Unit>(&format!(
        "SELECT {UNIT_COLUMNS} FROM units WHERE id = $1 AND organisation_id = $2"
    ))
    .bind(id)
    .bind(organisation_id)
    .fetch_optional(executor)
    .await?;
    Ok(unit)
}

/// Whether a unit code is already taken.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn code_taken<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    code: &str,
) -> CoreResult<bool> {
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM units WHERE organisation_id = $1 AND code = $2)",
    )
    .bind(organisation_id)
    .bind(code)
    .fetch_one(executor)
    .await?;
    Ok(exists)
}

/// List units.
///
/// Units carry no classification of their own: their existence is `INTERNAL`,
/// so every active member may see the shape of the institution.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn list_units<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    include_archived: bool,
) -> CoreResult<Vec<Unit>> {
    let units = sqlx::query_as::<_, Unit>(&format!(
        "SELECT {UNIT_COLUMNS} FROM units
          WHERE organisation_id = $1 AND ($2 OR status = 'active')
          ORDER BY code"
    ))
    .bind(organisation_id)
    .bind(include_archived)
    .fetch_all(executor)
    .await?;
    Ok(units)
}

/// Insert a unit.
///
/// # Errors
///
/// Returns an error when the insert fails.
pub async fn insert_unit<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    code: &str,
    name: &str,
    description: Option<&str>,
    research_areas: &[String],
    created_by: Uuid,
) -> CoreResult<Unit> {
    let unit = sqlx::query_as::<_, Unit>(&format!(
        "INSERT INTO units (organisation_id, code, name, description, research_areas, created_by_id)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING {UNIT_COLUMNS}"
    ))
    .bind(organisation_id)
    .bind(code)
    .bind(name)
    .bind(description)
    .bind(research_areas)
    .bind(created_by)
    .fetch_one(executor)
    .await?;
    Ok(unit)
}

/// Archive a unit. Never deletes it.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn archive_unit<'e>(
    executor: impl PgExecutor<'e>,
    unit_id: Uuid,
    updated_by: Uuid,
) -> CoreResult<()> {
    sqlx::query(
        "UPDATE units SET status = 'archived', archived_at = now(),
                          updated_by_id = $2, updated_at = now()
          WHERE id = $1",
    )
    .bind(unit_id)
    .bind(updated_by)
    .execute(executor)
    .await?;
    Ok(())
}

/// List live members of a unit.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn list_members<'e>(
    executor: impl PgExecutor<'e>,
    unit_id: Uuid,
) -> CoreResult<Vec<UnitMember>> {
    let members = sqlx::query_as::<_, UnitMember>(
        "SELECT m.id, m.unit_id, m.person_id, p.full_name, m.role, m.created_at
           FROM unit_memberships m
           JOIN people p ON p.id = m.person_id
          WHERE m.unit_id = $1 AND m.revoked_at IS NULL
          ORDER BY p.full_name",
    )
    .bind(unit_id)
    .fetch_all(executor)
    .await?;
    Ok(members)
}

/// Grant or restore a unit membership.
///
/// # Errors
///
/// Returns an error when the upsert fails.
pub async fn upsert_member<'e>(
    executor: impl PgExecutor<'e>,
    unit_id: Uuid,
    person_id: Uuid,
    role: UnitRole,
    actor: Uuid,
) -> CoreResult<Uuid> {
    let id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO unit_memberships (unit_id, person_id, role, created_by_id)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (unit_id, person_id) DO UPDATE
            SET role = EXCLUDED.role, revoked_at = NULL,
                updated_by_id = EXCLUDED.created_by_id, updated_at = now()
         RETURNING id",
    )
    .bind(unit_id)
    .bind(person_id)
    .bind(role.as_str())
    .bind(actor)
    .fetch_one(executor)
    .await?;
    Ok(id)
}

/// Revoke a unit membership. The row is kept: that a person belonged to a unit
/// is institutional memory.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn revoke_member<'e>(
    executor: impl PgExecutor<'e>,
    unit_id: Uuid,
    person_id: Uuid,
    actor: Uuid,
) -> CoreResult<bool> {
    let result = sqlx::query(
        "UPDATE unit_memberships
            SET revoked_at = now(), updated_by_id = $3, updated_at = now()
          WHERE unit_id = $1 AND person_id = $2 AND revoked_at IS NULL",
    )
    .bind(unit_id)
    .bind(person_id)
    .bind(actor)
    .execute(executor)
    .await?;
    Ok(result.rows_affected() > 0)
}
