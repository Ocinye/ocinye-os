//! The lifecycle of a persisted [`ActionPlan`].
//!
//! # Why this is a service and not four HTTP handlers
//!
//! Everything here decides something: whether a plan may still be approved,
//! whether a confirmation still counts, whether this request is the one that
//! gets to run the plan. A decision that lives in a route handler is a decision
//! that only an HTTP client can reach, and therefore only an HTTP client can
//! test — which is how the approval gate came to exist, be correct, and never
//! once be exercised through the surface that uses it.
//!
//! The Ocinye Core owns decisions; transport renders them (ADR-0006). So the
//! routes below this module are four thin calls, and the suite exercises the
//! same code they do.
//!
//! # The order, and why it is this order
//!
//! ```text
//! claim        →  one conditional UPDATE decides who runs it
//! consent      →  is a confirmation needed *by today's policy*, and is there one
//! authorise    →  the Capability Executor, per step, against each resource
//! execute      →  a domain service, which owns the invariant
//! settle       →  the state comes from what the steps actually did
//! audit        →  what happened, never what was said
//! ```
//!
//! **A persisted plan is a proposal, not authority.** Nothing in this file
//! authorises anything: it decides whether to *ask* the executor, and the
//! executor decides everything else, again, immediately before each effect.

use chrono::{DateTime, Utc};
use ocinye_contracts::agentic::{ActionPlan, ExecutionStatus, PlanState, Reversibility, RiskLevel};
use ocinye_contracts::PageRequest;
use ocinye_domain::{Principal, ResourceContext, ResourceKind};
use ocinye_observability::CorrelationIds;
use sqlx::PgPool;
use uuid::Uuid;

use super::repository::{self as repo, ApprovalRecord, StoredPlan};
use super::{executor, planner, runtime};
use crate::audit::{self, action, AuditEntry, Outcome};
use crate::error::{CoreError, CoreResult};

/// The states from which a plan may still be approved, rejected or run.
///
/// Everything else is terminal. A plan that has finished — completed, failed,
/// rejected, expired, cancelled — is a historical record, and a record does not
/// change its mind.
const OPEN_STATES: &[PlanState] = &[
    PlanState::Proposed,
    PlanState::AwaitingApproval,
    PlanState::Approved,
];

/// A plan, with everything a caller needs to decide about it.
#[derive(Debug, Clone)]
pub struct PlanDetail {
    /// The stored row.
    pub stored: StoredPlan,
    /// The plan, rebuilt and digest-checked.
    pub plan: ActionPlan,
    /// The peak risk **the registry gives today**, not the stored snapshot.
    pub risk: RiskLevel,
    /// Whether a person still has to confirm, by today's policy.
    pub requires_approval: bool,
    /// Whether a live confirmation exists for this exact plan, right now.
    pub approved: bool,
    /// When that confirmation stops counting.
    pub approval_expires_at: Option<DateTime<Utc>>,
}

impl PlanDetail {
    /// Whether the lifecycle still admits an approval, rejection or execution.
    #[must_use]
    pub fn is_open(&self) -> bool {
        !self.stored.state.is_terminal()
    }
}

/// What running a plan produced.
#[derive(Debug, Clone)]
pub struct ExecutedPlan {
    /// The plan, with each step's real outcome attached.
    pub plan: ActionPlan,
    /// Where it settled.
    pub state: PlanState,
    /// A factual sentence about what happened.
    pub summary: String,
    /// Whether an Undo may be offered.
    pub undoable: bool,
}

/// Rebuild the in-memory plan from its stored form, and check it is intact.
///
/// # Why the digest is re-derived rather than trusted
///
/// The material content of a plan — capability, input, resources, order — is
/// what a confirmation is bound to, and nothing in the Core writes it after
/// creation. Only step *results* are attached later, and
/// [`planner::digest_of`] deliberately does not hash those.
///
/// So a stored plan whose recomputed digest differs from its stored one has had
/// its material content changed by something outside this path. That is not a
/// state to reason about; it is a plan to refuse. This is where the
/// immutability of a persisted plan stops being a convention and becomes a
/// check.
fn rehydrate(stored: &StoredPlan) -> CoreResult<ActionPlan> {
    let steps = serde_json::from_value(stored.steps.clone())
        .map_err(|_| CoreError::Internal("stored plan could not be read".to_owned()))?;

    let plan = ActionPlan {
        id: stored.id,
        intent: stored.intent.clone(),
        steps,
        state: stored.state,
        digest: stored.digest.clone(),
    };

    if planner::digest_of(&plan) != stored.digest {
        tracing::error!(plan_id = %stored.id, "a stored plan no longer matches its digest");
        return Err(CoreError::Internal(
            "stored plan does not match its digest".to_owned(),
        ));
    }

    Ok(plan)
}

/// Assemble the full detail of a stored plan.
async fn detail_of(
    pool: &PgPool,
    principal: &Principal,
    stored: StoredPlan,
) -> CoreResult<PlanDetail> {
    let plan = rehydrate(&stored)?;
    let approval = repo::approval_for(pool, stored.id).await?;
    let approved = approval
        .as_ref()
        .is_some_and(|record| record.still_valid(principal.person_id, &plan, Utc::now()));

    Ok(PlanDetail {
        risk: planner::current_peak_risk(&plan),
        requires_approval: planner::approval_required_now(&plan),
        approved,
        approval_expires_at: approval.map(|record| record.expires_at),
        stored,
        plan,
    })
}

/// The plans this person asked for, most recent first.
///
/// Filtered on the requester in SQL, paginated, and totally ordered so a page
/// boundary cannot show one plan twice and another never.
///
/// # Errors
///
/// Returns an error when a query fails.
pub async fn list(
    pool: &PgPool,
    principal: &Principal,
    page: PageRequest,
) -> CoreResult<(Vec<PlanDetail>, i64)> {
    let stored = repo::own_plans(pool, principal.person_id, page.limit(), page.offset()).await?;

    let mut details = Vec::with_capacity(stored.len());
    for row in stored {
        details.push(detail_of(pool, principal, row).await?);
    }

    let total = repo::own_plan_count(pool, principal.person_id).await?;
    Ok((details, total))
}

/// One plan, if it belongs to the caller.
///
/// # Errors
///
/// Returns [`CoreError::NotFound`] when the plan does not exist **or** belongs
/// to somebody else. The two are deliberately indistinguishable: whether a
/// colleague asked the Ocinye to do something is not learned by guessing
/// identifiers (`CLAUDE.md` §60).
pub async fn detail(pool: &PgPool, principal: &Principal, plan_id: Uuid) -> CoreResult<PlanDetail> {
    let stored = repo::own_plan(pool, principal.person_id, plan_id)
        .await?
        .ok_or_else(|| CoreError::NotFound("Plano não encontrado.".to_owned()))?;

    detail_of(pool, principal, stored).await
}

/// Say why a conditional transition returned nothing.
///
/// Three things produce an empty result: the plan is not the caller's, it does
/// not exist, or it is no longer open. The first two answer as absence; the
/// third answers plainly, because the caller owns the plan and already knows it
/// exists.
async fn refusal(pool: &PgPool, principal: &Principal, plan_id: Uuid) -> CoreError {
    match repo::own_plan(pool, principal.person_id, plan_id).await {
        Ok(Some(stored)) => CoreError::Conflict(format!(
            "Este plano já não aceita esta operação: está {}.",
            stored.state.label().to_lowercase()
        )),
        Ok(None) => CoreError::NotFound("Plano não encontrado.".to_owned()),
        Err(error) => error,
    }
}

/// Record a person's consent to a plan. **Runs nothing.**
///
/// # Consent is not authorization
///
/// This writes that somebody said yes, bound to them, to this plan's digest and
/// to a window. It grants nothing: [`execute`] asks the Capability Executor to
/// decide the whole question again, against the actor as they are at that
/// moment (ADR-0303).
///
/// # Errors
///
/// Returns [`CoreError::NotFound`] when the plan is not the caller's, and
/// [`CoreError::Conflict`] when it is no longer open.
pub async fn approve(
    pool: &PgPool,
    principal: &Principal,
    ids: &CorrelationIds,
    plan_id: Uuid,
) -> CoreResult<ApprovalRecord> {
    let mut tx = pool.begin().await?;

    // The state moves first, conditionally. Two requests racing to approve and
    // to reject cannot both win: whichever `UPDATE` commits first takes the row
    // out of the open set, and the other comes back empty.
    let Some(stored) = repo::transition(
        &mut *tx,
        plan_id,
        principal.person_id,
        OPEN_STATES,
        PlanState::Approved,
    )
    .await?
    else {
        tx.rollback().await.ok();
        return Err(refusal(pool, principal, plan_id).await);
    };

    let plan = rehydrate(&stored)?;

    // Bound to the digest the Core computed, never to one a caller sent. The
    // client says «approve plan X»; what X *does* is the Core's own record.
    let record = repo::approve(&mut *tx, plan_id, principal.person_id, &plan.digest).await?;

    audit::record(
        &mut tx,
        Some(principal),
        ids,
        AuditEntry::new(action::PLAN_APPROVED, "action_plan")
            .resource(plan_id)
            .detail("risk", planner::current_peak_risk(&plan).as_str())
            .detail("expires_at", record.expires_at.to_rfc3339()),
    )
    .await?;

    tx.commit().await?;
    Ok(record)
}

/// Refuse a plan. Terminal.
///
/// A rejected plan is never executable again — not by approving it afterwards,
/// not by racing the rejection. Trying the same thing means producing a new
/// plan, which is a new proposal and a new decision.
///
/// The record is not destroyed: that somebody was asked and said no is
/// institutional history, worth as much as a yes (`CLAUDE.md` §37).
///
/// # Errors
///
/// Returns [`CoreError::NotFound`] when the plan is not the caller's, and
/// [`CoreError::Conflict`] when it is no longer open.
pub async fn reject(
    pool: &PgPool,
    principal: &Principal,
    ids: &CorrelationIds,
    plan_id: Uuid,
) -> CoreResult<()> {
    let mut tx = pool.begin().await?;

    if repo::transition(
        &mut *tx,
        plan_id,
        principal.person_id,
        OPEN_STATES,
        PlanState::Rejected,
    )
    .await?
    .is_none()
    {
        tx.rollback().await.ok();
        return Err(refusal(pool, principal, plan_id).await);
    }

    audit::record(
        &mut tx,
        Some(principal),
        ids,
        AuditEntry::new(action::PLAN_REJECTED, "action_plan").resource(plan_id),
    )
    .await?;

    tx.commit().await?;
    Ok(())
}

/// Run a plan, once.
///
/// # What makes «once» true
///
/// The first thing this does is move the plan to `executing` with a conditional
/// `UPDATE`. Whoever wins that statement owns the right to run it; anyone
/// else — a second click, a retried request, a concurrent call — finds the plan
/// no longer open and is told what happened rather than repeating the effect.
///
/// The guarantee lives in PostgreSQL rather than in this process, because a
/// second instance of the Core would not share a lock held in memory
/// (briefing §74).
///
/// # What is deliberately not promised
///
/// Exactly-once against systems outside the institution. `Core → SMTP` is not
/// an ACID transaction and no amount of local locking makes it one. What is
/// prevented here is the Ocinye repeating a committed plan; what a mail server
/// did with a message it already accepted is not this function's to claim.
///
/// # Errors
///
/// Returns [`CoreError::NotFound`] when the plan is not the caller's,
/// [`CoreError::Conflict`] when it is no longer open, and
/// [`CoreError::Validation`] when a needed confirmation is absent or expired.
pub async fn execute(
    pool: &PgPool,
    capabilities: &crate::capabilities::Capabilities,
    realtime: &crate::realtime::Realtime,
    principal: &Principal,
    ids: &CorrelationIds,
    plan_id: Uuid,
) -> CoreResult<ExecutedPlan> {
    // ── 1. Claim ────────────────────────────────────────────────────────
    let Some(stored) = repo::transition(
        pool,
        plan_id,
        principal.person_id,
        OPEN_STATES,
        PlanState::Executing,
    )
    .await?
    else {
        return Err(refusal(pool, principal, plan_id).await);
    };

    // From here the plan is claimed, and `executing` is not a state anything
    // can move it out of. So every failure below has to settle it, or a plan
    // that could not be read would sit claimed forever: not runnable, not
    // rejectable, and with nothing written down about why.
    //
    // `Failed` is the honest landing place. It is terminal, it is visible, and
    // it does not claim anything happened.
    let outcome = run_claimed(pool, capabilities, realtime, principal, ids, &stored).await;

    match outcome {
        Ok(executed) => Ok(executed),
        Err(error) => {
            // The approval gate returns its own error *after* putting the plan
            // back where a person can act on it; that is a refusal, not a
            // failure, and it must not be overwritten here.
            if !matches!(error, CoreError::Validation(_)) {
                let _ = repo::settle(pool, plan_id, PlanState::Failed, &stored.steps).await;
            }
            Err(error)
        }
    }
}

/// Everything that happens once a plan has been claimed for execution.
async fn run_claimed(
    pool: &PgPool,
    capabilities: &crate::capabilities::Capabilities,
    realtime: &crate::realtime::Realtime,
    principal: &Principal,
    ids: &CorrelationIds,
    stored: &StoredPlan,
) -> CoreResult<ExecutedPlan> {
    let plan_id = stored.id;
    let mut plan = rehydrate(stored)?;

    // ── 2. Consent ──────────────────────────────────────────────────────
    //
    // Whether one is needed is asked of the **registry**, not of the risk
    // stored with the plan. A capability reclassified upward since must not run
    // under yesterday's gentler answer; risk comes from the registry, and
    // «always» includes later (briefing §49).
    let needs_approval = planner::approval_required_now(&plan);
    let approval = repo::approval_for(pool, plan_id).await?;
    let approved = approval
        .as_ref()
        .is_some_and(|record| record.still_valid(principal.person_id, &plan, Utc::now()));

    if needs_approval && !approved {
        // Back where a person can still act on it. An expired confirmation
        // reads exactly like an absent one, because it is one.
        repo::set_plan_state(pool, plan_id, PlanState::AwaitingApproval, None).await?;

        audit::record_standalone(
            pool,
            ids,
            principal.person_id,
            principal.organisation_id,
            action::SECURITY_DENIAL,
            "action_plan",
            plan_id,
        )
        .await;

        return Err(CoreError::Validation(
            "Este plano precisa de confirmação, ou a confirmação expirou.".to_owned(),
        ));
    }

    // ── 3. Run ──────────────────────────────────────────────────────────
    //
    // Every step through the Capability Executor, which resolves the resources
    // again, authorises against each one's own context, validates, gates on
    // approval and only then reaches a domain service. Nothing here touches a
    // repository, a provider, a socket or SQL.
    let agent = runtime::main_agent_boundary();
    let institution =
        ResourceContext::organisation(ResourceKind::Person, principal.organisation_id);

    let mut halted = false;
    for step in &mut plan.steps {
        if halted {
            step.result = Some(ocinye_contracts::agentic::CapabilityResult {
                capability: step.request.capability.clone(),
                status: ExecutionStatus::NotAttempted,
                resources: Vec::new(),
                detail: "Não executada, porque uma acção anterior falhou.".to_owned(),
                reversibility: Reversibility::NothingToUndo,
                output: None,
            });
            continue;
        }

        let result = executor::execute(
            pool,
            capabilities,
            realtime,
            principal,
            &agent,
            None,
            &step.request,
            &institution,
            approved,
            ids,
        )
        .await?;

        if !result.status.changed_something() && result.status != ExecutionStatus::DryRun {
            halted = true;
        }
        step.result = Some(result);
    }

    // ── 4. Settle ───────────────────────────────────────────────────────
    //
    // The state comes from what the steps actually did. A plan that half worked
    // says so, and never «tudo feito» (briefing §56).
    let settled = runtime::settled_state(&plan);
    let steps = serde_json::to_value(&plan.steps).unwrap_or(serde_json::Value::Null);

    let mut tx = pool.begin().await?;
    repo::settle(&mut *tx, plan_id, settled, &steps).await?;

    let succeeded = plan
        .steps
        .iter()
        .filter(|step| {
            step.result
                .as_ref()
                .is_some_and(|result| result.status.changed_something())
        })
        .count();

    audit::record(
        &mut tx,
        Some(principal),
        ids,
        AuditEntry::new(action::PLAN_EXECUTED, "action_plan")
            .resource(plan_id)
            .detail("state", settled.as_str())
            .detail("steps", i64::try_from(plan.steps.len()).unwrap_or(0))
            .detail("steps_succeeded", i64::try_from(succeeded).unwrap_or(0))
            .outcome(if settled == PlanState::Completed {
                Outcome::Success
            } else {
                Outcome::Failure
            }),
    )
    .await?;
    tx.commit().await?;

    Ok(ExecutedPlan {
        summary: runtime::summarise(&plan),
        undoable: runtime::undoable(&plan),
        state: settled,
        plan,
    })
}
