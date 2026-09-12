//! Personal storage accounting and enforcement.
//!
//! # The invariant
//!
//! A member's personal storage usage is measured directly from the bytes they
//! own — `SUM(size_bytes)` over `storage_objects` keyed on `owner_id` — not from
//! a counter that could drift (briefing §61). Their limit is the effective
//! entitlement (ADR-0108). New personal bytes are admitted server-side against
//! `used + incoming <= limit`, and the admission serialises per member so
//! parallel uploads cannot both slip past a full quota (briefing §10).
//!
//! Reducing a quota below current usage never deletes data: the member goes
//! over quota, existing bytes stay, and new writes are refused until usage
//! falls or the quota rises (briefing §7, §8).

use ocinye_contracts::{Permission, ResourceScopeType, ResourceType, StorageState};
use ocinye_domain::{can, Principal, ResourceContext, ResourceKind};
use sqlx::{PgExecutor, PgPool};
use uuid::Uuid;

use crate::error::{CoreError, CoreResult};
use crate::Tx;

/// A member's personal storage picture, for the member and for administration.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PersonalStorageStatus {
    /// Bytes currently occupied by the member's own objects.
    pub used_bytes: i64,
    /// The effective limit in bytes, from the entitlement. Zero means no limit
    /// has resolved yet, which reads as unlimited here.
    pub limit_bytes: i64,
    /// Bytes still writable before the limit. Zero once at or over the limit.
    pub available_bytes: i64,
    /// The state derived from usage against the limit.
    pub state: StorageState,
}

/// The bytes a member's own objects occupy.
///
/// Personal objects are exactly those keyed on the owner (`owner_id`): personal
/// files, note images, mail attachments. Institutional (workspace) objects carry
/// no `owner_id` and are not a member's personal usage, even when that member
/// uploaded them. Avatars are personal but unmarked (`owner_id IS NULL`) and are
/// deliberately not counted — they are small, fixed, and replace in place.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn personal_usage_bytes<'e>(
    executor: impl PgExecutor<'e>,
    person_id: Uuid,
) -> CoreResult<i64> {
    let used: i64 = sqlx::query_scalar(
        // SUM over a BIGINT column is NUMERIC; cast back to bigint.
        "SELECT COALESCE(SUM(size_bytes), 0)::bigint
           FROM storage_objects
          WHERE owner_id = $1 AND status = 'stored'",
    )
    .bind(person_id)
    .fetch_one(executor)
    .await?;
    Ok(used)
}

/// The effective personal storage limit of a member, in bytes.
///
/// This resolves the same entitlement as [`super::resolve_entitlement`], but in
/// one query so it can run inside a write transaction: the active override or
/// profile allocation (override wins), falling back to the member's assigned
/// profile rule and then the organisation default profile rule, plus every live
/// temporary allocation. `0` means no limit resolved — treated as unlimited by
/// the admission. A test proves this equals the Rust resolver.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn personal_storage_limit_bytes<'e>(
    executor: impl PgExecutor<'e>,
    person_id: Uuid,
) -> CoreResult<i64> {
    let limit: i64 = sqlx::query_scalar(
        "WITH base AS (
             SELECT quantity
               FROM resource_allocations
              WHERE scope_type = 'member' AND scope_id = $1
                AND resource_type = 'persistent_storage'
                AND status = 'active' AND starts_at <= now()
                AND (expires_at IS NULL OR expires_at > now())
                AND source IN ('override', 'profile')
              ORDER BY (source = 'override') DESC, starts_at DESC
              LIMIT 1
         ),
         profile_base AS (
             SELECT r.quantity
               FROM people p
               LEFT JOIN resource_profiles ap ON ap.id = p.resource_profile_id
               LEFT JOIN resource_profiles dp
                      ON dp.organisation_id = p.organisation_id AND dp.is_default
               JOIN resource_profile_rules r
                      ON r.profile_id = COALESCE(ap.id, dp.id)
                     AND r.resource_type = 'persistent_storage'
              WHERE p.id = $1
              LIMIT 1
         ),
         temporaries AS (
             SELECT COALESCE(SUM(quantity), 0)::bigint AS q
               FROM resource_allocations
              WHERE scope_type = 'member' AND scope_id = $1
                AND resource_type = 'persistent_storage'
                AND status = 'active' AND starts_at <= now()
                AND (expires_at IS NULL OR expires_at > now())
                AND source = 'temporary'
         )
         SELECT (COALESCE((SELECT quantity FROM base),
                          (SELECT quantity FROM profile_base),
                          0)
              + (SELECT q FROM temporaries))::bigint",
    )
    .bind(person_id)
    .fetch_one(executor)
    .await?;
    Ok(limit)
}

/// Admit `additional_bytes` of new personal storage for a member, or refuse.
///
/// Takes a per-member transaction advisory lock first, so concurrent admissions
/// serialise and cannot both pass a nearly-full quota (briefing §10). Then it
/// measures current usage and the limit inside the transaction and checks
/// `used + additional <= limit`. The lock is held until the caller's transaction
/// ends, so the store that follows is covered by the same admission.
///
/// A limit of zero (none resolved) admits everything.
///
/// # Errors
///
/// [`CoreError::Validation`] when the write would exceed the member's quota.
pub async fn admit_personal_bytes(
    tx: &mut Tx<'_>,
    person_id: Uuid,
    additional_bytes: i64,
) -> CoreResult<()> {
    // Serialise per member. `hashtext` maps the id onto the advisory-lock key
    // space; the lock is transaction-scoped and released on commit or rollback.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
        .bind(person_id.to_string())
        .execute(&mut **tx)
        .await?;

    let used = personal_usage_bytes(&mut **tx, person_id).await?;
    let limit = personal_storage_limit_bytes(&mut **tx, person_id).await?;

    if limit > 0 && used + additional_bytes > limit {
        return Err(CoreError::Validation(
            "Não há espaço de armazenamento pessoal suficiente. Liberte espaço \
             ou peça mais recursos."
                .to_owned(),
        ));
    }
    Ok(())
}

/// A member's personal storage status, for display.
///
/// A member always sees their own; seeing another member's takes
/// `resources.view`. The authority is checked here.
///
/// # Errors
///
/// [`CoreError::PermissionDenied`] when reading another member's status without
/// `resources.view`.
pub async fn personal_storage_status(
    pool: &PgPool,
    principal: &Principal,
    person_id: Uuid,
) -> CoreResult<PersonalStorageStatus> {
    if person_id != principal.person_id {
        let ctx = ResourceContext::organisation(ResourceKind::Person, principal.organisation_id);
        if !can(principal, Permission::ResourcesView, &ctx, None).allowed {
            return Err(CoreError::PermissionDenied(
                "Não possui acesso aos recursos deste membro.".to_owned(),
            ));
        }
    }

    let used = personal_usage_bytes(pool, person_id).await?;
    // The limit resolves through the shared entitlement resolver, so display and
    // enforcement cannot disagree about the number.
    let entitlement = super::resolve_entitlement(
        pool,
        principal.organisation_id,
        ResourceScopeType::Member,
        person_id,
        ResourceType::PersistentStorage,
    )
    .await?;
    let limit = entitlement.quantity;
    let available = if limit > 0 { (limit - used).max(0) } else { 0 };

    Ok(PersonalStorageStatus {
        used_bytes: used,
        limit_bytes: limit,
        available_bytes: available,
        state: StorageState::from_usage(used, limit),
    })
}
