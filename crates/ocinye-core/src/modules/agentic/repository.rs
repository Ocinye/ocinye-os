//! Persistence for plans, approvals and executions.
//!
//! # What is kept, and for how long
//!
//! The plan, its steps, who confirmed it and how each step ended. **Not** the
//! prompt, not the model's reasoning, not the retrieved context. Those carry a
//! member's own words and other people's material, and keeping them would build
//! a second copy of the institution inside a table nobody audits
//! (briefing §48, §177).

use chrono::{DateTime, Duration, Utc};
use ocinye_contracts::agentic::{ActionPlan, PlanState};
use sqlx::{PgExecutor, Row};
use uuid::Uuid;

use crate::error::CoreResult;

/// How long a confirmation stays good.
///
/// # Why an approval expires at all
///
/// A confirmation is consent given with a situation in mind. An hour later the
/// mailbox has other mail in it, the project has moved on, and the person who
/// said yes is not watching. Fifteen minutes is long enough to read a plan and
/// short enough that nothing acts on stale consent (briefing §99).
pub const APPROVAL_LIFETIME_MINUTES: i64 = 15;

/// A plan as stored.
#[derive(Debug, Clone)]
pub struct StoredPlan {
    /// Identifier.
    pub id: Uuid,
    /// Who asked for it.
    pub requested_by: Uuid,
    /// The agent that built it.
    pub agent_id: Option<Uuid>,
    /// What the member wanted.
    pub intent: String,
    /// The steps, as JSON.
    pub steps: serde_json::Value,
    /// Where it stands.
    pub state: PlanState,
    /// The digest an approval is bound to.
    pub digest: String,
    /// When it was built.
    pub created_at: DateTime<Utc>,
}

/// What a confirmation recorded.
#[derive(Debug, Clone)]
pub struct ApprovalRecord {
    /// Which plan.
    pub plan_id: Uuid,
    /// Who confirmed. Compared against the actor at execution.
    pub approved_by: Uuid,
    /// The digest at the moment of confirmation.
    pub digest: String,
    /// When it stops counting.
    pub expires_at: DateTime<Utc>,
}

impl ApprovalRecord {
    /// Whether this confirmation still counts, for this person, for this plan.
    ///
    /// All three have to hold. A confirmation is not a token somebody else can
    /// spend, and it is not good forever (briefing §158).
    #[must_use]
    pub fn still_valid(&self, actor: Uuid, plan: &ActionPlan, now: DateTime<Utc>) -> bool {
        self.approved_by == actor && self.digest == plan.digest && now < self.expires_at
    }
}

/// Write a plan.
///
/// # Errors
///
/// Returns an error when the statement fails.
pub async fn create_plan<'e>(
    executor: impl PgExecutor<'e>,
    plan: &ActionPlan,
    requested_by: Uuid,
    organisation_id: Uuid,
    agent_id: Option<Uuid>,
) -> CoreResult<()> {
    sqlx::query(
        "INSERT INTO action_plans
             (id, organisation_id, requested_by, agent_id, intent, steps, state, digest)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(plan.id)
    .bind(organisation_id)
    .bind(requested_by)
    .bind(agent_id)
    .bind(&plan.intent)
    .bind(serde_json::to_value(&plan.steps).unwrap_or(serde_json::Value::Null))
    .bind(plan.state.as_str())
    .bind(&plan.digest)
    .execute(executor)
    .await?;

    Ok(())
}

/// One plan, if it belongs to the caller.
///
/// The ownership check is in the query. A plan somebody else built is not
/// reachable by knowing its identifier.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn own_plan<'e>(
    executor: impl PgExecutor<'e>,
    person_id: Uuid,
    plan_id: Uuid,
) -> CoreResult<Option<StoredPlan>> {
    let row = sqlx::query(&format!(
        "SELECT {PLAN_COLUMNS} FROM action_plans WHERE id = $1 AND requested_by = $2"
    ))
    .bind(plan_id)
    .bind(person_id)
    .fetch_optional(executor)
    .await?;

    row.as_ref().map(read_plan).transpose()
}

/// The columns a stored plan is read from, in the order `read_plan` expects.
const PLAN_COLUMNS: &str = "id, requested_by, agent_id, intent, steps, state, digest, created_at";

/// Shape one row into a [`StoredPlan`].
fn read_plan(row: &sqlx::postgres::PgRow) -> CoreResult<StoredPlan> {
    let state: String = row.try_get("state")?;
    Ok(StoredPlan {
        id: row.try_get("id")?,
        requested_by: row.try_get("requested_by")?,
        agent_id: row.try_get("agent_id")?,
        intent: row.try_get("intent")?,
        steps: row.try_get("steps")?,
        // An unreadable state reads as `Failed`, which is terminal: a row this
        // build cannot interpret must not become executable.
        state: PlanState::parse(&state).unwrap_or(PlanState::Failed),
        digest: row.try_get("digest")?,
        created_at: row.try_get("created_at")?,
    })
}

/// Move a plan from one of `from` to `to`, atomically, and return it.
///
/// # Why every lifecycle transition goes through one conditional `UPDATE`
///
/// Reading the state, deciding, and then writing is three steps with gaps
/// between them. Two requests that both read `approved` both decide they may
/// proceed, and both write — which is how the same plan gets executed twice, or
/// approved and rejected at once. Nothing in the application can close that
/// gap, because the gap is between the application and the database.
///
/// A single `UPDATE … WHERE state = ANY($3)` closes it: PostgreSQL takes a row
/// lock for the duration, and the second statement sees the state the first one
/// committed. Exactly one of two racing callers gets a row back; the other gets
/// `None` and must read what actually happened.
///
/// Ownership is in the same `WHERE`. A plan somebody else asked for is not
/// transitionable by knowing its identifier, and the answer is indistinguishable
/// from a plan that is not in one of the `from` states — neither confirms that
/// the plan exists (`CLAUDE.md` §60).
///
/// # Errors
///
/// Returns an error when the statement fails.
pub async fn transition<'e>(
    executor: impl PgExecutor<'e>,
    plan_id: Uuid,
    person_id: Uuid,
    from: &[PlanState],
    to: PlanState,
) -> CoreResult<Option<StoredPlan>> {
    let from: Vec<String> = from.iter().map(|state| state.as_str().to_owned()).collect();

    let row = sqlx::query(&format!(
        "UPDATE action_plans
            SET state = $4,
                -- The schema requires a settling moment for a terminal state
                -- and forbids one otherwise, so the two move together.
                settled_at = CASE WHEN $5 THEN now() ELSE settled_at END
          WHERE id = $1
            AND requested_by = $2
            AND state = ANY($3)
      RETURNING {PLAN_COLUMNS}"
    ))
    .bind(plan_id)
    .bind(person_id)
    .bind(&from)
    .bind(to.as_str())
    .bind(to.is_terminal())
    .fetch_optional(executor)
    .await?;

    row.as_ref().map(read_plan).transpose()
}

/// Attach the outcome of each step, and settle the plan.
///
/// # Why the steps are written only here
///
/// The material content of a plan — capability, input, resources, order — is
/// what a confirmation is bound to, and it never changes. What this writes is
/// the *result* of each step, which [`super::planner::digest_of`] deliberately
/// does not hash: attaching an outcome must not invalidate the approval that
/// authorised producing it.
///
/// # Errors
///
/// Returns an error when the statement fails.
pub async fn settle<'e>(
    executor: impl PgExecutor<'e>,
    plan_id: Uuid,
    state: PlanState,
    steps: &serde_json::Value,
) -> CoreResult<()> {
    sqlx::query(
        "UPDATE action_plans
            SET state = $2,
                steps = $3,
                settled_at = CASE WHEN $4 THEN now() ELSE settled_at END
          WHERE id = $1",
    )
    .bind(plan_id)
    .bind(state.as_str())
    .bind(steps)
    .bind(state.is_terminal())
    .execute(executor)
    .await?;

    Ok(())
}

/// Move a plan to a new state, recording its outcome.
///
/// # Errors
///
/// Returns an error when the statement fails.
pub async fn set_plan_state<'e>(
    executor: impl PgExecutor<'e>,
    plan_id: Uuid,
    state: PlanState,
    steps: Option<&serde_json::Value>,
) -> CoreResult<()> {
    sqlx::query(
        "UPDATE action_plans
            SET state = $2,
                steps = COALESCE($3, steps),
                settled_at = CASE WHEN $4 THEN now() ELSE settled_at END
          WHERE id = $1",
    )
    .bind(plan_id)
    .bind(state.as_str())
    .bind(steps)
    .bind(state.is_terminal())
    .execute(executor)
    .await?;

    Ok(())
}

/// Record a confirmation.
///
/// # Errors
///
/// Returns an error when the statement fails.
pub async fn approve<'e>(
    executor: impl PgExecutor<'e>,
    plan_id: Uuid,
    approved_by: Uuid,
    digest: &str,
) -> CoreResult<ApprovalRecord> {
    let expires_at = Utc::now() + Duration::minutes(APPROVAL_LIFETIME_MINUTES);

    sqlx::query(
        "INSERT INTO action_approvals (plan_id, approved_by, digest, expires_at)
              VALUES ($1, $2, $3, $4)
         ON CONFLICT (plan_id) DO UPDATE
                SET approved_by = EXCLUDED.approved_by,
                    digest = EXCLUDED.digest,
                    expires_at = EXCLUDED.expires_at,
                    approved_at = now()",
    )
    .bind(plan_id)
    .bind(approved_by)
    .bind(digest)
    .bind(expires_at)
    .execute(executor)
    .await?;

    Ok(ApprovalRecord {
        plan_id,
        approved_by,
        digest: digest.to_owned(),
        expires_at,
    })
}

/// The confirmation for a plan, if there is one.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn approval_for<'e>(
    executor: impl PgExecutor<'e>,
    plan_id: Uuid,
) -> CoreResult<Option<ApprovalRecord>> {
    let row = sqlx::query(
        "SELECT plan_id, approved_by, digest, expires_at
           FROM action_approvals
          WHERE plan_id = $1",
    )
    .bind(plan_id)
    .fetch_optional(executor)
    .await?;

    let Some(row) = row else { return Ok(None) };

    Ok(Some(ApprovalRecord {
        plan_id: row.try_get("plan_id")?,
        approved_by: row.try_get("approved_by")?,
        digest: row.try_get("digest")?,
        expires_at: row.try_get("expires_at")?,
    }))
}

/// The plans this person asked for, most recent first.
///
/// Answers «o que é que o Ocinye fez por mim?» (briefing §135).
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn own_plans<'e>(
    executor: impl PgExecutor<'e>,
    person_id: Uuid,
    limit: i64,
    offset: i64,
) -> CoreResult<Vec<StoredPlan>> {
    // `created_at` alone is not a total order — two plans built in the same
    // millisecond would page unpredictably, showing one twice and the other
    // never. The identifier breaks the tie.
    let rows = sqlx::query(&format!(
        "SELECT {PLAN_COLUMNS}
           FROM action_plans
          WHERE requested_by = $1
          ORDER BY created_at DESC, id DESC
          LIMIT $2 OFFSET $3"
    ))
    .bind(person_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(executor)
    .await?;

    rows.iter().map(read_plan).collect()
}

/// How many plans this person has asked for.
///
/// Counted with the same predicate the listing uses, so a total can never
/// describe rows the listing would not return.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn own_plan_count<'e>(executor: impl PgExecutor<'e>, person_id: Uuid) -> CoreResult<i64> {
    let total =
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM action_plans WHERE requested_by = $1")
            .bind(person_id)
            .fetch_one(executor)
            .await?;
    Ok(total)
}

#[cfg(test)]
mod tests {
    use ocinye_contracts::agentic::ActionPlan;

    use super::*;

    fn plan(digest: &str) -> ActionPlan {
        ActionPlan {
            id: Uuid::from_u128(1),
            intent: "x".to_owned(),
            steps: Vec::new(),
            state: PlanState::Approved,
            digest: digest.to_owned(),
        }
    }

    fn approval(actor: Uuid, digest: &str, expires_at: DateTime<Utc>) -> ApprovalRecord {
        ApprovalRecord {
            plan_id: Uuid::from_u128(1),
            approved_by: actor,
            digest: digest.to_owned(),
            expires_at,
        }
    }

    #[test]
    fn a_confirmation_is_bound_to_the_person_who_gave_it() {
        let ana = Uuid::from_u128(10);
        let carlos = Uuid::from_u128(11);
        let now = Utc::now();
        let record = approval(ana, "abc", now + Duration::minutes(5));

        assert!(record.still_valid(ana, &plan("abc"), now));
        assert!(
            !record.still_valid(carlos, &plan("abc"), now),
            "a confirmação de uma pessoa serviu a outra"
        );
    }

    #[test]
    fn a_confirmation_is_bound_to_the_plan_it_confirmed() {
        let ana = Uuid::from_u128(10);
        let now = Utc::now();
        let record = approval(ana, "abc", now + Duration::minutes(5));

        assert!(!record.still_valid(ana, &plan("outro-digest"), now));
    }

    #[test]
    fn a_confirmation_expires() {
        let ana = Uuid::from_u128(10);
        let now = Utc::now();
        let record = approval(ana, "abc", now - Duration::seconds(1));

        assert!(
            !record.still_valid(ana, &plan("abc"), now),
            "uma confirmação caducada continuou a valer"
        );
    }

    #[test]
    fn all_three_conditions_have_to_hold() {
        let ana = Uuid::from_u128(10);
        let carlos = Uuid::from_u128(11);
        let now = Utc::now();

        // Actor certo, digest errado, dentro do prazo.
        assert!(!approval(ana, "a", now + Duration::minutes(5)).still_valid(ana, &plan("b"), now));
        // Actor errado, digest certo, dentro do prazo.
        assert!(!approval(ana, "a", now + Duration::minutes(5)).still_valid(
            carlos,
            &plan("a"),
            now
        ));
        // Tudo certo excepto o prazo.
        assert!(!approval(ana, "a", now - Duration::minutes(1)).still_valid(ana, &plan("a"), now));
        // Os três.
        assert!(approval(ana, "a", now + Duration::minutes(5)).still_valid(ana, &plan("a"), now));
    }
}
