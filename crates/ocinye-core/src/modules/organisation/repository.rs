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

/// The organisation this installation's Instance is recorded as (ADR-0013).
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn recorded_instance<'e>(
    executor: impl PgExecutor<'e>,
) -> CoreResult<Option<Organisation>> {
    let organisation = sqlx::query_as::<_, Organisation>(
        "SELECT o.id, o.slug, o.name, o.legal_name, o.country, o.description, o.created_at
           FROM instance_identity i
           JOIN organisations o ON o.id = i.organisation_id",
    )
    .fetch_optional(executor)
    .await?;
    Ok(organisation)
}

/// Every organisation in the database, oldest first — capped, because only
/// «none», «exactly one» and «more than one» matter to the caller.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn first_organisations<'e>(
    executor: impl PgExecutor<'e>,
) -> CoreResult<Vec<Organisation>> {
    let organisations = sqlx::query_as::<_, Organisation>(
        "SELECT id, slug, name, legal_name, country, description, created_at
           FROM organisations ORDER BY created_at LIMIT 2",
    )
    .fetch_all(executor)
    .await?;
    Ok(organisations)
}

/// Record which organisation this installation's Instance is.
///
/// Idempotent for the same organisation. Recording a different one while a
/// record exists violates the singleton, and the database refuses it.
///
/// # Errors
///
/// Returns an error when the insert fails.
pub async fn record_instance<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
) -> CoreResult<Uuid> {
    let id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO instance_identity (organisation_id) VALUES ($1)
         ON CONFLICT (organisation_id) DO UPDATE SET organisation_id = EXCLUDED.organisation_id
         RETURNING id",
    )
    .bind(organisation_id)
    .fetch_one(executor)
    .await?;
    Ok(id)
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

/// The advisory-lock namespace under which unit-code numbers are allocated.
///
/// Any constant that is stable and unlikely to collide with another use of
/// `pg_advisory_xact_lock` in the same transaction would do; this one reads as
/// «UNIT» so a `pg_locks` row is legible to an operator.
const UNIT_CODE_LOCK_NAMESPACE: i32 = 0x554E_4954; // "UNIT"

/// Allocate the next free `<stem>-NNN` code for an organisation.
///
/// The number is per-stem and per-organisation: `UCS-001`, then `UCS-002`, while
/// `UDC-001` counts on its own. Allocation is serialised for the transaction by
/// an advisory lock keyed on the organisation, so two units created at the same
/// instant can never read the same maximum and race onto the same number — the
/// second waits, then sees the first.
///
/// Pure counting, not parsing intent: it reads the numbers already spent on this
/// stem and returns the lowest three-digit suffix above them all, starting at
/// `001`.
///
/// # Errors
///
/// Returns an error when the lock or the query fails.
pub async fn next_unit_code(
    conn: &mut sqlx::PgConnection,
    organisation_id: Uuid,
    stem: &str,
) -> CoreResult<String> {
    // Hold the organisation's unit-code namespace for the rest of the
    // transaction. Released on commit or rollback — never left dangling.
    sqlx::query("SELECT pg_advisory_xact_lock($1, hashtext($2))")
        .bind(UNIT_CODE_LOCK_NAMESPACE)
        .bind(organisation_id.to_string())
        .execute(&mut *conn)
        .await?;

    let prefix = format!("{stem}-");
    let spent = codes_with_prefix(&mut *conn, organisation_id, &prefix).await?;
    Ok(format!("{prefix}{:03}", next_number(&spent, &prefix)))
}

/// The code the next unit of this stem *would* get, without allocating it.
///
/// Read-only and lock-free: for a live preview in a form, where the exact number
/// is indicative and only confirmed when the unit is actually created (another
/// unit created in between simply shifts it). [`next_unit_code`] is the
/// authority; this only shows what it is about to do.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn peek_unit_code<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    stem: &str,
) -> CoreResult<String> {
    let prefix = format!("{stem}-");
    let spent = codes_with_prefix(executor, organisation_id, &prefix).await?;
    Ok(format!("{prefix}{:03}", next_number(&spent, &prefix)))
}

/// Codes in this organisation that begin with `prefix`.
async fn codes_with_prefix<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    prefix: &str,
) -> CoreResult<Vec<String>> {
    let codes = sqlx::query_scalar::<_, String>(
        "SELECT code FROM units WHERE organisation_id = $1 AND code LIKE $2",
    )
    .bind(organisation_id)
    .bind(format!("{prefix}%"))
    .fetch_all(executor)
    .await?;
    Ok(codes)
}

/// The lowest free number above every `<prefix>NNN` already spent, from `1`.
fn next_number(spent: &[String], prefix: &str) -> u32 {
    spent
        .iter()
        .filter_map(|code| code.strip_prefix(prefix))
        .filter_map(|suffix| suffix.parse::<u32>().ok())
        .max()
        .map_or(1, |highest| highest + 1)
}

/// Insert a unit only if its code is free, for idempotent seeding.
///
/// Returns the new unit, or `None` when a unit with this code already exists —
/// so re-running a seed adds nothing and clobbers nothing. `created_by_id` is
/// left null: a seeded unit has no human author, and saying so is honest.
///
/// # Errors
///
/// Returns an error when the insert fails.
pub async fn insert_unit_if_absent<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    code: &str,
    name: &str,
    description: Option<&str>,
    research_areas: &[String],
) -> CoreResult<Option<Unit>> {
    let unit = sqlx::query_as::<_, Unit>(&format!(
        "INSERT INTO units (organisation_id, code, name, description, research_areas)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (organisation_id, code) DO NOTHING
         RETURNING {UNIT_COLUMNS}"
    ))
    .bind(organisation_id)
    .bind(code)
    .bind(name)
    .bind(description)
    .bind(research_areas)
    .fetch_optional(executor)
    .await?;
    Ok(unit)
}

/// Update a unit's editable fields: name, description and research areas.
///
/// The **code is not editable**: it is institutional identity, appears in
/// citations, and stays stable for the life of the unit. Renaming a unit does
/// not renumber it.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn update_unit<'e>(
    executor: impl PgExecutor<'e>,
    unit_id: Uuid,
    name: &str,
    description: Option<&str>,
    research_areas: &[String],
    updated_by: Uuid,
) -> CoreResult<Unit> {
    let unit = sqlx::query_as::<_, Unit>(&format!(
        "UPDATE units
            SET name = $2, description = $3, research_areas = $4,
                updated_by_id = $5, updated_at = now()
          WHERE id = $1
         RETURNING {UNIT_COLUMNS}"
    ))
    .bind(unit_id)
    .bind(name)
    .bind(description)
    .bind(research_areas)
    .bind(updated_by)
    .fetch_one(executor)
    .await?;
    Ok(unit)
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

/// The live units of each person in a set, as `(person_id, code, name)`.
///
/// One query for the whole page — never one per row. Revoked memberships do not
/// travel: that a person once belonged to a unit is memory, not current
/// structure, and this feeds a «where does this member sit now?» column.
/// Ordered by code so the caller's grouping is stable.
pub async fn units_of_people<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    person_ids: &[Uuid],
) -> CoreResult<Vec<(Uuid, String, String)>> {
    let rows = sqlx::query_as::<_, (Uuid, String, String)>(
        "SELECT m.person_id, u.code, u.name
           FROM unit_memberships m
           JOIN units u ON u.id = m.unit_id
          WHERE m.person_id = ANY($1)
            AND m.revoked_at IS NULL
            AND u.organisation_id = $2
          ORDER BY u.code",
    )
    .bind(person_ids)
    .bind(organisation_id)
    .fetch_all(executor)
    .await?;
    Ok(rows)
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
