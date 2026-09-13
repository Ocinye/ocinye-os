//! Admission and usage accounting for AI requests.
//!
//! # The invariant
//!
//! An AI request is admitted server-side against the member's effective
//! `ModelAccess` entitlement before any inference runs, and the admission
//! serialises per member so parallel requests cannot both slip past a full
//! quota — the same discipline personal storage already applies (ADR-0108,
//! [`super::admit_personal_bytes`]).
//!
//! Usage is a **live measure** of the append-only ledger
//! (`resource_usage_events`), not a counter that could drift: a request is
//! charged only when a model actually answered, and a request that no provider
//! could run charges nothing. The ledger row is written in the same transaction
//! as the admission, so the reservation the admission takes is either committed
//! (the model answered) or released (it did not) with the transaction — there is
//! no window in which a charge outlives a failure or a failure keeps a charge.
//!
//! A limit of zero (none resolved) admits everything, exactly as storage does:
//! an installation that has not chosen to meter AI does not accidentally block
//! it. Metering is a data decision — a `model_access` entitlement on a profile
//! or an allocation — not a code change.
//!
//! There is deliberately **no reservation table** here: for a synchronous
//! request the transaction *is* the reservation. A persisted reservation, for
//! asynchronous compute jobs, is a later slice (migration 0040 says so).

use sqlx::{PgExecutor, PgPool};
use uuid::Uuid;

use crate::error::CoreResult;
use crate::Tx;

/// A member's recorded AI access usage, summed from the append-only ledger.
///
/// The live measure: `SUM(quantity)` over `resource_usage_events` for this
/// member's `model_access` charges. Nothing writes there but
/// [`record_ai_access`], and it can never be updated or deleted (the ledger's
/// triggers refuse it), so the sum is authoritative.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn ai_access_used<'e>(executor: impl PgExecutor<'e>, person_id: Uuid) -> CoreResult<i64> {
    let used: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(quantity), 0)::bigint
           FROM resource_usage_events
          WHERE charge_scope_type = 'member' AND charge_scope_id = $1
            AND resource_type = 'model_access'",
    )
    .bind(person_id)
    .fetch_one(executor)
    .await?;
    Ok(used)
}

/// A member's effective `ModelAccess` limit, resolved in one query.
///
/// The same resolution as [`super::resolve_entitlement`] for `model_access`, in
/// one query so it runs inside the admission transaction: the active override or
/// profile allocation (override wins), falling back to the member's assigned
/// profile rule and then the organisation default profile rule, plus every live
/// temporary allocation. `0` means no limit resolved — treated as unlimited.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn ai_access_limit<'e>(
    executor: impl PgExecutor<'e>,
    person_id: Uuid,
) -> CoreResult<i64> {
    let limit: i64 = sqlx::query_scalar(
        "WITH base AS (
             SELECT quantity
               FROM resource_allocations
              WHERE scope_type = 'member' AND scope_id = $1
                AND resource_type = 'model_access'
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
                     AND r.resource_type = 'model_access'
              WHERE p.id = $1
              LIMIT 1
         ),
         temporaries AS (
             SELECT COALESCE(SUM(quantity), 0)::bigint AS q
               FROM resource_allocations
              WHERE scope_type = 'member' AND scope_id = $1
                AND resource_type = 'model_access'
                AND status = 'active' AND starts_at <= now()
                AND (expires_at IS NULL OR expires_at > now())
                AND source = 'temporary'
         )
         SELECT COALESCE(
                    (SELECT quantity FROM base),
                    (SELECT quantity FROM profile_base),
                    0
                )::bigint + (SELECT q FROM temporaries)",
    )
    .bind(person_id)
    .fetch_one(executor)
    .await?;
    Ok(limit)
}

/// Admit `amount` AI accesses for a member, or refuse — fail-closed.
///
/// Takes a per-member transaction advisory lock first, so concurrent requests
/// serialise and cannot both pass a nearly-full quota. Then it measures current
/// usage and the limit inside the transaction and checks `used + amount <=
/// limit`. The lock is held until the caller's transaction ends, so the usage
/// [`record_ai_access`] writes is covered by the same admission.
///
/// Returns `Ok(true)` when admitted, `Ok(false)` when the quota is exhausted.
/// An `Err` is a real fault (the database), never a refusal.
///
/// A limit of zero (none resolved) admits everything.
///
/// # Errors
///
/// Returns an error when a query fails.
pub async fn admit_ai_access(tx: &mut Tx<'_>, person_id: Uuid, amount: i64) -> CoreResult<bool> {
    // Serialise per member. `hashtext` maps the id onto the advisory-lock key
    // space; the lock is transaction-scoped and released on commit or rollback.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
        .bind(person_id.to_string())
        .execute(&mut **tx)
        .await?;

    let used = ai_access_used(&mut **tx, person_id).await?;
    let limit = ai_access_limit(&mut **tx, person_id).await?;

    Ok(limit <= 0 || used + amount <= limit)
}

/// Record AI access usage in the append-only ledger — the commit of a request
/// that a model answered.
///
/// Written in the admission transaction: the charge lands only if the caller
/// commits, and a failed request that never records leaves no charge. The
/// ledger keeps *that* the access happened, with the model and the job — never
/// the prompt or the completion.
///
/// # Errors
///
/// Returns an error when the insert fails.
#[expect(
    clippy::too_many_arguments,
    reason = "um parâmetro por coluna de contexto; a alternativa é uma struct que só atravessa esta chamada"
)]
pub async fn record_ai_access<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    person_id: Uuid,
    model_id: Option<Uuid>,
    compute_node_id: Option<Uuid>,
    job_id: Option<Uuid>,
    correlation_id: &str,
    amount: i64,
) -> CoreResult<Uuid> {
    let id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO resource_usage_events
             (organisation_id, resource_type, quantity, unit,
              actor_person_id, charge_scope_type, charge_scope_id,
              model_id, compute_node_id, job_id, correlation_id, source, reason)
         VALUES ($1, 'model_access', $2, 'count',
                 $3, 'member', $3,
                 $4, $5, $6, $7, 'measured', 'Prompt Ocinye')
         RETURNING id",
    )
    .bind(organisation_id)
    .bind(amount)
    .bind(person_id)
    .bind(model_id)
    .bind(compute_node_id)
    .bind(job_id)
    .bind(correlation_id)
    .fetch_one(executor)
    .await?;
    Ok(id)
}

/// A member's AI access standing, for display and for the status surface.
#[derive(Debug, Clone, Copy)]
pub struct AiAccessStatus {
    /// Accesses recorded against the member.
    pub used: i64,
    /// The effective limit; `0` means none resolved (unlimited).
    pub limit: i64,
}

/// Resolve a member's AI access standing.
///
/// # Errors
///
/// Returns an error when a query fails.
pub async fn ai_access_status(pool: &PgPool, person_id: Uuid) -> CoreResult<AiAccessStatus> {
    let used = ai_access_used(pool, person_id).await?;
    let limit = ai_access_limit(pool, person_id).await?;
    Ok(AiAccessStatus { used, limit })
}
