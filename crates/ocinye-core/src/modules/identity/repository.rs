//! Identity persistence.

use chrono::{DateTime, Utc};
use ocinye_contracts::{AccountStatus, TechnicalRole, UnitRole, WorkspaceRole};
use sqlx::{PgExecutor, Row};
use uuid::Uuid;

use super::model::{Invitation, Person};
use crate::error::CoreResult;

/// Find a person by their verified OIDC subject.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn find_by_subject<'e>(
    executor: impl PgExecutor<'e>,
    subject: &str,
) -> CoreResult<Option<Person>> {
    let person = sqlx::query_as::<_, Person>(
        "SELECT id, organisation_id, oidc_subject, email, full_name, display_name,
                institutional_position, orcid, biography, username, status,
                last_seen_at, deactivated_at, created_at
           FROM people
          WHERE oidc_subject = $1",
    )
    .bind(subject)
    .fetch_optional(executor)
    .await?;
    Ok(person)
}

/// Find an invited person by email who has not yet been bound to an identity.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn find_unbound_by_email<'e>(
    executor: impl PgExecutor<'e>,
    email: &str,
) -> CoreResult<Option<Person>> {
    let person = sqlx::query_as::<_, Person>(
        "SELECT id, organisation_id, oidc_subject, email, full_name, display_name,
                institutional_position, orcid, biography, username, status,
                last_seen_at, deactivated_at, created_at
           FROM people
          WHERE lower(email) = lower($1) AND oidc_subject IS NULL",
    )
    .bind(email)
    .fetch_optional(executor)
    .await?;
    Ok(person)
}

/// Load a person by identifier, scoped to an organisation.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn find_by_id<'e>(
    executor: impl PgExecutor<'e>,
    id: Uuid,
    organisation_id: Uuid,
) -> CoreResult<Option<Person>> {
    let person = sqlx::query_as::<_, Person>(
        "SELECT id, organisation_id, oidc_subject, email, full_name, display_name,
                institutional_position, orcid, biography, username, status,
                last_seen_at, deactivated_at, created_at
           FROM people
          WHERE id = $1 AND organisation_id = $2",
    )
    .bind(id)
    .bind(organisation_id)
    .fetch_optional(executor)
    .await?;
    Ok(person)
}

/// List the people of an organisation.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn list<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    limit: i64,
    offset: i64,
) -> CoreResult<Vec<Person>> {
    let people = sqlx::query_as::<_, Person>(
        "SELECT id, organisation_id, oidc_subject, email, full_name, display_name,
                institutional_position, orcid, biography, username, status,
                last_seen_at, deactivated_at, created_at
           FROM people
          WHERE organisation_id = $1
          ORDER BY full_name
          LIMIT $2 OFFSET $3",
    )
    .bind(organisation_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(executor)
    .await?;
    Ok(people)
}

/// Count the people of an organisation.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn count<'e>(executor: impl PgExecutor<'e>, organisation_id: Uuid) -> CoreResult<i64> {
    let total =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM people WHERE organisation_id = $1")
            .bind(organisation_id)
            .fetch_one(executor)
            .await?;
    Ok(total)
}

/// Bind a verified OIDC subject to a person and activate them.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn bind_subject<'e>(
    executor: impl PgExecutor<'e>,
    person_id: Uuid,
    subject: &str,
) -> CoreResult<()> {
    sqlx::query(
        "UPDATE people
            SET oidc_subject = $2,
                status = CASE WHEN status = 'invited' THEN 'active' ELSE status END,
                updated_at = now()
          WHERE id = $1 AND oidc_subject IS NULL",
    )
    .bind(person_id)
    .bind(subject)
    .execute(executor)
    .await?;
    Ok(())
}

/// Record that a person was seen.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn touch_last_seen<'e>(executor: impl PgExecutor<'e>, person_id: Uuid) -> CoreResult<()> {
    sqlx::query("UPDATE people SET last_seen_at = now() WHERE id = $1")
        .bind(person_id)
        .execute(executor)
        .await?;
    Ok(())
}

/// Live technical roles of a person.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn live_roles<'e>(
    executor: impl PgExecutor<'e>,
    person_id: Uuid,
) -> CoreResult<Vec<TechnicalRole>> {
    let rows =
        sqlx::query("SELECT role FROM person_roles WHERE person_id = $1 AND revoked_at IS NULL")
            .bind(person_id)
            .fetch_all(executor)
            .await?;

    Ok(rows
        .iter()
        .filter_map(|row| row.try_get::<String, _>("role").ok())
        .filter_map(|role| TechnicalRole::parse(&role))
        .collect())
}

/// Live unit memberships of a person.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn live_unit_roles<'e>(
    executor: impl PgExecutor<'e>,
    person_id: Uuid,
) -> CoreResult<Vec<(Uuid, UnitRole)>> {
    let rows = sqlx::query(
        "SELECT unit_id, role FROM unit_memberships
          WHERE person_id = $1 AND revoked_at IS NULL",
    )
    .bind(person_id)
    .fetch_all(executor)
    .await?;

    Ok(rows
        .iter()
        .filter_map(|row| {
            let unit_id: Uuid = row.try_get("unit_id").ok()?;
            let role: String = row.try_get("role").ok()?;
            Some((unit_id, UnitRole::parse(&role)?))
        })
        .collect())
}

/// Live workspace memberships of a person.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn live_workspace_roles<'e>(
    executor: impl PgExecutor<'e>,
    person_id: Uuid,
) -> CoreResult<Vec<(Uuid, WorkspaceRole)>> {
    let rows = sqlx::query(
        "SELECT workspace_id, role FROM workspace_memberships
          WHERE person_id = $1 AND revoked_at IS NULL",
    )
    .bind(person_id)
    .fetch_all(executor)
    .await?;

    Ok(rows
        .iter()
        .filter_map(|row| {
            let workspace_id: Uuid = row.try_get("workspace_id").ok()?;
            let role: String = row.try_get("role").ok()?;
            Some((workspace_id, WorkspaceRole::parse(&role)?))
        })
        .collect())
}

/// Grant a technical role, returning `true` when it was newly granted.
///
/// # Errors
///
/// Returns an error when the insert fails.
pub async fn grant_role<'e>(
    executor: impl PgExecutor<'e>,
    person_id: Uuid,
    role: TechnicalRole,
    reason: &str,
    // `None` only at bootstrap, where by definition nobody granted it.
    granted_by: Option<Uuid>,
) -> CoreResult<bool> {
    let result = sqlx::query(
        "INSERT INTO person_roles (person_id, role, granted_reason, granted_by_id)
         SELECT $1, $2, $3, $4
          WHERE NOT EXISTS (
              SELECT 1 FROM person_roles
               WHERE person_id = $1 AND role = $2 AND revoked_at IS NULL
          )",
    )
    .bind(person_id)
    .bind(role.as_str())
    .bind(reason)
    .bind(granted_by)
    .execute(executor)
    .await?;
    Ok(result.rows_affected() > 0)
}

/// Revoke a technical role, returning `true` when one was live.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn revoke_role<'e>(
    executor: impl PgExecutor<'e>,
    person_id: Uuid,
    role: TechnicalRole,
) -> CoreResult<bool> {
    let result = sqlx::query(
        "UPDATE person_roles SET revoked_at = now(), updated_at = now()
          WHERE person_id = $1 AND role = $2 AND revoked_at IS NULL",
    )
    .bind(person_id)
    .bind(role.as_str())
    .execute(executor)
    .await?;
    Ok(result.rows_affected() > 0)
}

/// Whether a person with this email already exists.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn email_taken<'e>(executor: impl PgExecutor<'e>, email: &str) -> CoreResult<bool> {
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM people WHERE lower(email) = lower($1))",
    )
    .bind(email)
    .fetch_one(executor)
    .await?;
    Ok(exists)
}

/// Insert an invitation.
///
/// # Errors
///
/// Returns an error when the insert fails.
#[expect(
    clippy::too_many_arguments,
    reason = "um parâmetro por coluna: a alternativa é uma struct que só existe para atravessar esta chamada"
)]
pub async fn insert_invitation<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    email: &str,
    full_name: &str,
    position: Option<&str>,
    token_digest: &str,
    expires_at: DateTime<Utc>,
    created_by: Uuid,
) -> CoreResult<Invitation> {
    let invitation = sqlx::query_as::<_, Invitation>(
        "INSERT INTO invitations
             (organisation_id, email, full_name, institutional_position,
              token_digest, expires_at, created_by_id)
         VALUES ($1, lower($2), $3, $4, $5, $6, $7)
         RETURNING id, organisation_id, email, full_name, institutional_position,
                   status, expires_at, accepted_at, accepted_person_id, created_at",
    )
    .bind(organisation_id)
    .bind(email)
    .bind(full_name)
    .bind(position)
    .bind(token_digest)
    .bind(expires_at)
    .bind(created_by)
    .fetch_one(executor)
    .await?;
    Ok(invitation)
}

/// Find a pending invitation by token digest.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn find_invitation_by_digest<'e>(
    executor: impl PgExecutor<'e>,
    digest: &str,
) -> CoreResult<Option<Invitation>> {
    let invitation = sqlx::query_as::<_, Invitation>(
        "SELECT id, organisation_id, email, full_name, institutional_position,
                status, expires_at, accepted_at, accepted_person_id, created_at
           FROM invitations
          WHERE token_digest = $1",
    )
    .bind(digest)
    .fetch_optional(executor)
    .await?;
    Ok(invitation)
}

/// Create the person shell for an accepted invitation.
///
/// The person is created **without** an OIDC subject: binding happens on first
/// verified sign-in, so an invitation alone never grants access.
///
/// # Errors
///
/// Returns an error when the insert fails.
pub async fn insert_person_from_invitation<'e>(
    executor: impl PgExecutor<'e>,
    invitation: &Invitation,
) -> CoreResult<Person> {
    let person = sqlx::query_as::<_, Person>(
        "INSERT INTO people
             (organisation_id, email, full_name, institutional_position, status)
         VALUES ($1, $2, $3, $4, 'invited')
         RETURNING id, organisation_id, oidc_subject, email, full_name, display_name,
                   institutional_position, orcid, biography, username, status,
                   last_seen_at, deactivated_at, created_at",
    )
    .bind(invitation.organisation_id)
    .bind(&invitation.email)
    .bind(&invitation.full_name)
    .bind(invitation.institutional_position.as_deref())
    .fetch_one(executor)
    .await?;
    Ok(person)
}

/// Mark an invitation accepted.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn mark_invitation_accepted<'e>(
    executor: impl PgExecutor<'e>,
    invitation_id: Uuid,
    person_id: Uuid,
) -> CoreResult<()> {
    sqlx::query(
        "UPDATE invitations
            SET status = 'accepted', accepted_at = now(), accepted_person_id = $2,
                updated_at = now()
          WHERE id = $1",
    )
    .bind(invitation_id)
    .bind(person_id)
    .execute(executor)
    .await?;
    Ok(())
}

/// Mark an invitation expired.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn mark_invitation_expired<'e>(
    executor: impl PgExecutor<'e>,
    invitation_id: Uuid,
) -> CoreResult<()> {
    sqlx::query("UPDATE invitations SET status = 'expired', updated_at = now() WHERE id = $1")
        .bind(invitation_id)
        .execute(executor)
        .await?;
    Ok(())
}

/// Find a person by their sign-in name.
///
/// Case-insensitive, matching the unique index. Deliberately does not filter on
/// status: the caller distinguishes "no such account" from "cannot sign in", and
/// must spend the same work on both (see `authentication`).
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn find_by_username<'e>(
    executor: impl PgExecutor<'e>,
    username: &str,
) -> CoreResult<Option<Person>> {
    let person = sqlx::query_as::<_, Person>(
        "SELECT id, organisation_id, oidc_subject, email, full_name, display_name,
                institutional_position, orcid, biography, username, status,
                last_seen_at, deactivated_at, created_at
           FROM people
          WHERE lower(username) = lower($1)",
    )
    .bind(username)
    .fetch_optional(executor)
    .await?;
    Ok(person)
}

/// Whether a username is already taken, case-insensitively.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn username_taken<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    username: &str,
) -> CoreResult<bool> {
    let taken: bool = sqlx::query_scalar(
        "SELECT EXISTS (
             SELECT 1 FROM people
              WHERE organisation_id = $1 AND lower(username) = lower($2)
         )",
    )
    .bind(organisation_id)
    .bind(username)
    .fetch_one(executor)
    .await?;
    Ok(taken)
}

/// Create a member account.
///
/// The account starts `invited`: it becomes `active` when its holder replaces
/// the temporary credential with one of their own.
///
/// # Errors
///
/// Returns an error when the insert fails, including on a duplicate username
/// or email.
pub async fn insert_person<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    username: &str,
    email: &str,
    full_name: &str,
    position: Option<&str>,
) -> CoreResult<Person> {
    let person = sqlx::query_as::<_, Person>(
        "INSERT INTO people
             (organisation_id, username, email, full_name, institutional_position, status)
         VALUES ($1, $2, $3, $4, $5, 'invited')
         RETURNING id, organisation_id, oidc_subject, email, full_name, display_name,
                   institutional_position, orcid, biography, username, status,
                   last_seen_at, deactivated_at, created_at",
    )
    .bind(organisation_id)
    .bind(username)
    .bind(email)
    .bind(full_name)
    .bind(position)
    .fetch_one(executor)
    .await?;
    Ok(person)
}

/// Change an account's status.
///
/// `deactivated_at` is set when leaving the usable states and cleared on
/// return, so the column always agrees with the status beside it.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn set_status<'e>(
    executor: impl PgExecutor<'e>,
    person_id: Uuid,
    status: AccountStatus,
) -> CoreResult<()> {
    sqlx::query(
        "UPDATE people
            SET status = $2,
                deactivated_at = CASE
                    WHEN $2 IN ('suspended', 'disabled') THEN coalesce(deactivated_at, now())
                    ELSE NULL
                END,
                updated_at = now()
          WHERE id = $1",
    )
    .bind(person_id)
    .bind(status.as_str())
    .execute(executor)
    .await?;
    Ok(())
}

/// Whether the organisation already has a live platform administrator.
///
/// The one-shot guard on bootstrap (briefing §12). Two choices are deliberate.
///
/// **Scoped to the organisation.** A deployment serves one institution, but the
/// scope is named rather than assumed, in keeping with the rest of the schema
/// (`CLAUDE.md` §25). Widening it to the whole table would make the guard
/// depend on a fact the caller never states.
///
/// **Counts only usable accounts.** A suspended or disabled former
/// administrator must not permanently block recovery — that would turn one
/// mistake into an unrecoverable installation.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn has_usable_platform_admin<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
) -> CoreResult<bool> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS (
             SELECT 1
               FROM person_roles r
               JOIN people p ON p.id = r.person_id
              WHERE r.role = 'platform_admin'
                AND r.revoked_at IS NULL
                AND p.organisation_id = $1
                AND p.status IN ('invited', 'active')
         )",
    )
    .bind(organisation_id)
    .fetch_one(executor)
    .await?;
    Ok(exists)
}

/// Place a person in a unit as an ordinary member.
///
/// Idempotent: re-adding someone who is already a member reinstates a revoked
/// membership rather than failing, because the unique constraint is on the pair
/// and membership is revoked rather than deleted.
///
/// # Errors
///
/// Returns an error when the statement fails.
pub async fn add_unit_membership<'e>(
    executor: impl PgExecutor<'e>,
    unit_id: Uuid,
    person_id: Uuid,
    created_by: Uuid,
) -> CoreResult<()> {
    sqlx::query(
        "INSERT INTO unit_memberships (unit_id, person_id, role, created_by_id)
         VALUES ($1, $2, 'member', $3)
         ON CONFLICT (unit_id, person_id)
         DO UPDATE SET revoked_at = NULL, updated_by_id = $3, updated_at = now()",
    )
    .bind(unit_id)
    .bind(person_id)
    .bind(created_by)
    .execute(executor)
    .await?;
    Ok(())
}

/// Find a person by identifier, without an organisation filter.
///
/// Deliberately unscoped: the session extractor resolves a person *before* it
/// knows their organisation. Every other lookup is scoped.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn find_by_id_unscoped<'e>(
    executor: impl PgExecutor<'e>,
    person_id: Uuid,
) -> CoreResult<Option<Person>> {
    let person = sqlx::query_as::<_, Person>(
        "SELECT id, organisation_id, oidc_subject, email, full_name, display_name,
                institutional_position, orcid, biography, username, status,
                last_seen_at, deactivated_at, created_at
           FROM people
          WHERE id = $1",
    )
    .bind(person_id)
    .fetch_optional(executor)
    .await?;
    Ok(person)
}
