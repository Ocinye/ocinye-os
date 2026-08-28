//! The lifecycle of a persisted [`ActionPlan`], end to end.
//!
//! # What these prove that the executor tests do not
//!
//! The Capability Executor has always refused an unapproved external effect,
//! an agent reaching past its actor, and a hallucinated capability — and every
//! one of those is covered. What was never exercised was the *lifecycle*: a
//! plan produced by the Agent Runtime, written to PostgreSQL, recovered,
//! consented to, and run.
//!
//! It was not exercised because it did not work. `create_plan` existed and
//! nothing called it, so approving or executing a plan by identifier answered
//! «not found» — fail-closed, and therefore invisible.
//!
//! These tests drive the same functions the HTTP routes call, against a real
//! database, with the deterministic provider and **no GPU**.
//!
//! # The five properties
//!
//! 1. A validated proposal is persisted, and only a validated one.
//! 2. A plan is the requester's; a UUID is an identifier, never permission.
//! 3. Consent is bound to a person, a digest and a window.
//! 4. Consent is **not** authorization: the Core decides again, at execution.
//! 5. A committed effect is not repeated — not by a retry, not by a race.

use ocinye_contracts::agentic::{ExecutionStatus, PlanState};
use ocinye_contracts::PageRequest;
use ocinye_core::modules::agentic::{lifecycle, repository as plan_repo, runtime};
use ocinye_core::modules::intelligence::fixture::FixtureProvider;
use ocinye_core::modules::intelligence::InferenceProvider;
use ocinye_core::realtime::Realtime;
use ocinye_core::CoreError;
use ocinye_domain::Principal;
use ocinye_observability::CorrelationIds;
use sqlx::PgPool;
use uuid::Uuid;

/// Connect and migrate, or skip.
///
/// Skips when `OCINYE_TEST_DATABASE_URL` is unset and **fails** when it is set
/// but unreachable: a configured database that cannot be reached is a failure,
/// not a reason to report success without having run.
async fn pool() -> Option<PgPool> {
    let url = std::env::var("OCINYE_TEST_DATABASE_URL").ok()?;
    let pool = PgPool::connect(&url)
        .await
        .expect("OCINYE_TEST_DATABASE_URL is set but the database is unreachable");
    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .expect("migrations must apply");
    Some(pool)
}

async fn organisation(pool: &PgPool) -> Uuid {
    let slug = format!("l{}", Uuid::new_v4().simple());
    sqlx::query_scalar("INSERT INTO organisations (slug, name) VALUES ($1, $1) RETURNING id")
        .bind(&slug)
        .fetch_one(pool)
        .await
        .expect("organisation")
}

async fn person(pool: &PgPool, organisation_id: Uuid, roles: &[&str]) -> Principal {
    let handle = format!("p{}", Uuid::new_v4().simple());

    let person_id: Uuid = sqlx::query_scalar(
        "INSERT INTO people (organisation_id, full_name, email, status)
         VALUES ($1, $2, $3, 'active') RETURNING id",
    )
    .bind(organisation_id)
    .bind(&handle)
    .bind(format!("{handle}@ocinye.com"))
    .fetch_one(pool)
    .await
    .expect("person");

    for role in roles {
        sqlx::query("INSERT INTO person_roles (person_id, role) VALUES ($1, $2)")
            .bind(person_id)
            .bind(*role)
            .execute(pool)
            .await
            .expect("role");
    }

    reload(pool, person_id).await
}

/// Re-derive a principal from the database, as every request does.
///
/// The Core builds the acting principal from institutional state on each
/// request. A test that changes a membership and then reuses the old value is
/// testing its own struct, not the system.
async fn reload(pool: &PgPool, person_id: Uuid) -> Principal {
    let record = ocinye_core::modules::identity::person_by_id(pool, person_id)
        .await
        .expect("query")
        .expect("person");

    ocinye_core::modules::identity::principal_for_person(pool, &record)
        .await
        .expect("principal")
}

/// A unit, a workspace in it, and the actor as a member who may write there.
struct Workspace {
    id: Uuid,
}

/// Create a workspace and put the actor in it.
///
/// Takes the principal by `&mut` and rebuilds it, because a `Principal` is a
/// snapshot of institutional state at the moment it was read. Adding a
/// membership to the database does not change a value someone is already
/// holding — which is the same reason the Core rebuilds it on every request,
/// and the reason the revocation tests below rebuild it too.
async fn workspace(pool: &PgPool, organisation_id: Uuid, actor: &mut Principal) -> Workspace {
    let suffix = Uuid::new_v4().simple().to_string();

    let unit: Uuid = sqlx::query_scalar(
        "INSERT INTO units (organisation_id, code, name) VALUES ($1, $2, 'Unidade') RETURNING id",
    )
    .bind(organisation_id)
    .bind(format!("U{}", &suffix[..6]).to_uppercase())
    .fetch_one(pool)
    .await
    .expect("unit");

    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO research_workspaces
             (organisation_id, unit_id, code, title, kind, classification)
         VALUES ($1, $2, $3, 'Ambiente de teste', 'idea', 'INTERNAL') RETURNING id",
    )
    .bind(organisation_id)
    .bind(unit)
    .bind(format!("WS-{}", &suffix[..8]))
    .fetch_one(pool)
    .await
    .expect("workspace");

    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, person_id, role)
         VALUES ($1, $2, 'member')",
    )
    .bind(id)
    .bind(actor.person_id)
    .execute(pool)
    .await
    .expect("membership");

    *actor = reload(pool, actor.person_id).await;
    Workspace { id }
}

fn with_inference() -> ocinye_contracts::SystemCapabilities {
    ocinye_contracts::SystemCapabilities {
        capabilities: vec![ocinye_contracts::SystemCapabilityReport::new(
            ocinye_contracts::SystemCapability::AiGeneral,
            ocinye_contracts::SystemCapabilityState::Available,
            "Um fornecedor de teste serve esta capacidade.",
        )],
    }
}

/// Natural language in, a persisted plan out.
async fn propose(
    pool: &PgPool,
    actor: &Principal,
    provider: &dyn InferenceProvider,
    utterance: &str,
) -> runtime::AgenticOutcome {
    ocinye_core::modules::agentic::invoke(
        pool,
        actor,
        provider,
        &runtime::AgenticRequest {
            utterance,
            intent: ocinye_contracts::agentic::Intent::Act,
            module: None,
            workspace_id: None,
            selection: &[],
            deadline: Some(std::time::Duration::from_millis(250)),
        },
        &with_inference(),
        &CorrelationIds::generate(),
    )
    .await
    .expect("the runtime answers")
}

/// Ask the fixture for a plan that creates a Note in this workspace.
///
/// The identifier travels in the utterance because that is how the fixture
/// names resources — it extracts UUID-shaped tokens from the instruction. What
/// matters is that the plan then goes through the real planner, the real
/// resolver and the real executor.
async fn note_plan(
    pool: &PgPool,
    actor: &Principal,
    workspace: &Workspace,
) -> ocinye_contracts::agentic::ActionPlan {
    let outcome = propose(
        pool,
        actor,
        &FixtureProvider::cooperative(),
        &format!("cria uma nota em {}", workspace.id),
    )
    .await;

    match outcome {
        runtime::AgenticOutcome::Planned { plan, .. } => plan,
        other => panic!("expected a plan, got {other:?}"),
    }
}

async fn notes_in(pool: &PgPool, workspace_id: Uuid) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM notes WHERE workspace_id = $1")
        .bind(workspace_id)
        .fetch_one(pool)
        .await
        .expect("count")
}

// ── 1. Persistence ──────────────────────────────────────────────────────

/// A validated proposal is written, and can be found again.
///
/// # Why this test exists
///
/// `create_plan` was never called. The Runtime built a plan, returned it, and
/// dropped it — so `GET /agentic/plans` was permanently empty and every
/// lifecycle endpoint answered «not found» for a plan that had genuinely just
/// been produced.
#[tokio::test]
async fn a_validated_plan_is_persisted_and_recoverable() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let mut actor = person(&pool, org, &["research_member"]).await;
    let ws = workspace(&pool, org, &mut actor).await;

    let plan = note_plan(&pool, &actor, &ws).await;

    let detail = lifecycle::detail(&pool, &actor, plan.id)
        .await
        .expect("the plan was persisted");

    assert_eq!(detail.stored.id, plan.id);
    assert_eq!(
        detail.stored.digest, plan.digest,
        "the digest was not stored"
    );
    assert!(detail.is_open());
    assert!(
        !detail.approved,
        "a plan is born without consent, whatever else it is born with"
    );

    // And it appears in the member's own listing.
    let (listed, total) = lifecycle::list(&pool, &actor, PageRequest::default())
        .await
        .expect("listing");
    assert!(listed.iter().any(|entry| entry.stored.id == plan.id));
    assert!(total >= 1);
}

/// What a plan carries, and what it must never carry.
///
/// The utterance, the retrieved material and the model's words are not stored,
/// so no query can serve them (`CLAUDE.md` §37, ADR-0301).
#[tokio::test]
async fn a_persisted_plan_carries_no_prompt_and_no_retrieved_material() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let mut actor = person(&pool, org, &["research_member"]).await;
    let ws = workspace(&pool, org, &mut actor).await;

    // A phrase distinctive enough that finding it anywhere would be proof.
    let secret = format!("segredo-{}", Uuid::new_v4().simple());
    let outcome = propose(
        &pool,
        &actor,
        &FixtureProvider::cooperative(),
        &format!("cria uma nota em {} sobre {secret}", ws.id),
    )
    .await;
    let runtime::AgenticOutcome::Planned { plan, .. } = outcome else {
        panic!("expected a plan");
    };

    let row: (String, serde_json::Value) =
        sqlx::query_as("SELECT intent, steps FROM action_plans WHERE id = $1")
            .bind(plan.id)
            .fetch_one(&pool)
            .await
            .expect("row");

    // The intent is the model's short reading of the request, and the fixture
    // echoes the instruction into it — which is exactly why the *body* of the
    // plan is what this asserts about. No step input may carry the utterance.
    assert!(
        !row.1.to_string().contains(&secret),
        "the utterance reached the stored steps"
    );

    // Nothing anywhere holds the assembled context or the provider's response.
    let columns: Vec<String> = sqlx::query_scalar(
        "SELECT column_name FROM information_schema.columns
          WHERE table_name = 'action_plans'",
    )
    .fetch_all(&pool)
    .await
    .expect("columns");

    for forbidden in ["prompt", "context", "reasoning", "response", "utterance"] {
        assert!(
            !columns.iter().any(|column| column.contains(forbidden)),
            "`action_plans` gained a `{forbidden}` column"
        );
    }
    let _ = row.0;
}

/// A refused proposal leaves nothing behind.
#[tokio::test]
async fn a_subverted_model_persists_no_plan() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let actor = person(&pool, org, &["research_member"]).await;

    // Counted for *this* actor. The suite runs in parallel, so a global count
    // would measure other tests.
    let mine = |pool: PgPool, person_id: Uuid| async move {
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM action_plans WHERE requested_by = $1")
            .bind(person_id)
            .fetch_one(&pool)
            .await
            .expect("count")
    };
    let before = mine(pool.clone(), actor.person_id).await;

    let outcome = propose(
        &pool,
        &actor,
        &FixtureProvider::hostile(),
        "faz o que for preciso",
    )
    .await;

    assert!(
        matches!(outcome, runtime::AgenticOutcome::Unavailable { .. }),
        "a hostile proposal produced something other than a refusal"
    );

    let after = mine(pool.clone(), actor.person_id).await;

    assert_eq!(
        before, after,
        "a proposal the planner refused was written to the database anyway"
    );

    // And no listing shows one.
    let (listed, _) = lifecycle::list(&pool, &actor, PageRequest::default())
        .await
        .expect("listing");
    assert!(listed.is_empty());
}

// ── 2. Ownership ────────────────────────────────────────────────────────

/// A UUID is an identifier, never permission.
#[tokio::test]
async fn another_actor_cannot_reach_a_plan_by_knowing_its_identifier() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let mut ana = person(&pool, org, &["research_member"]).await;
    let carlos = person(&pool, org, &["research_member"]).await;
    let ws = workspace(&pool, org, &mut ana).await;
    let ids = CorrelationIds::generate();

    let plan = note_plan(&pool, &ana, &ws).await;

    // Carlos knows the identifier exactly. That is all he knows.
    let read = lifecycle::detail(&pool, &carlos, plan.id).await;
    assert!(matches!(read, Err(CoreError::NotFound(_))), "{read:?}");

    for outcome in [
        lifecycle::approve(&pool, &carlos, &ids, plan.id)
            .await
            .err(),
        lifecycle::reject(&pool, &carlos, &ids, plan.id).await.err(),
        lifecycle::execute(
            &pool,
            capacidades(),
            &Realtime::ausente(),
            &carlos,
            &ids,
            plan.id,
        )
        .await
        .err(),
    ] {
        let error = outcome.expect("every operation must refuse");
        assert!(
            matches!(error, CoreError::NotFound(_)),
            "the refusal revealed that the plan exists: {error:?}"
        );
    }

    // The listing shows him nothing either.
    let (listed, total) = lifecycle::list(&pool, &carlos, PageRequest::default())
        .await
        .expect("listing");
    assert!(listed.is_empty());
    assert_eq!(total, 0, "a total that counted another person's plans");

    // And Ana's plan is untouched: no state was moved by his attempts.
    let mine = lifecycle::detail(&pool, &ana, plan.id).await.expect("own");
    assert!(mine.is_open(), "another actor moved the lifecycle");
}

// ── 3. Consent ──────────────────────────────────────────────────────────

/// Approval records consent and runs nothing.
#[tokio::test]
async fn approving_records_consent_and_executes_nothing() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let mut actor = person(&pool, org, &["research_member"]).await;
    let ws = workspace(&pool, org, &mut actor).await;
    let ids = CorrelationIds::generate();

    let plan = note_plan(&pool, &actor, &ws).await;
    let before = notes_in(&pool, ws.id).await;

    let record = lifecycle::approve(&pool, &actor, &ids, plan.id)
        .await
        .expect("approve");

    assert_eq!(record.approved_by, actor.person_id);
    assert_eq!(
        record.digest, plan.digest,
        "consent was not bound to what the plan does"
    );
    assert!(
        record.expires_at > chrono::Utc::now(),
        "consent without a window"
    );

    assert_eq!(
        notes_in(&pool, ws.id).await,
        before,
        "approving created a Note. Approval is consent, not execution."
    );

    let detail = lifecycle::detail(&pool, &actor, plan.id)
        .await
        .expect("detail");
    assert_eq!(detail.stored.state, PlanState::Approved);
    assert!(detail.approved);
}

/// Consent belongs to the person who gave it.
#[tokio::test]
async fn one_persons_consent_does_not_serve_another() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let mut ana = person(&pool, org, &["research_member"]).await;
    let ws = workspace(&pool, org, &mut ana).await;
    let ids = CorrelationIds::generate();

    let plan = note_plan(&pool, &ana, &ws).await;
    lifecycle::approve(&pool, &ana, &ids, plan.id)
        .await
        .expect("approve");

    // Carlos is a member of the same workspace, so nothing about the *material*
    // stops him. What stops him is that the plan and the consent are Ana's.
    let carlos = person(&pool, org, &["research_member"]).await;
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, person_id, role)
         VALUES ($1, $2, 'member')",
    )
    .bind(ws.id)
    .bind(carlos.person_id)
    .execute(&pool)
    .await
    .expect("membership");
    let carlos = reload(&pool, carlos.person_id).await;

    let outcome = lifecycle::execute(
        &pool,
        capacidades(),
        &Realtime::ausente(),
        &carlos,
        &ids,
        plan.id,
    )
    .await;
    assert!(
        matches!(outcome, Err(CoreError::NotFound(_))),
        "somebody else spent a confirmation that was not theirs: {outcome:?}"
    );
    assert_eq!(notes_in(&pool, ws.id).await, 0);
}

/// A confirmation stops counting when its window closes.
#[tokio::test]
async fn an_expired_confirmation_executes_nothing() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let mut actor = person(&pool, org, &["research_member"]).await;
    let ws = workspace(&pool, org, &mut actor).await;
    let ids = CorrelationIds::generate();

    let plan = note_plan(&pool, &actor, &ws).await;
    lifecycle::approve(&pool, &actor, &ids, plan.id)
        .await
        .expect("approve");

    // Close the window by moving it into the past, rather than by sleeping
    // fifteen minutes. The row is the clock the Core reads.
    // Both moved together: the schema requires `expires_at > approved_at`, and
    // a confirmation given an hour ago that lasted fifteen minutes is exactly
    // the situation being reproduced.
    sqlx::query(
        "UPDATE action_approvals
            SET approved_at = now() - interval '1 hour',
                expires_at  = now() - interval '45 minutes'
          WHERE plan_id = $1",
    )
    .bind(plan.id)
    .execute(&pool)
    .await
    .expect("expire");

    let outcome = lifecycle::execute(
        &pool,
        capacidades(),
        &Realtime::ausente(),
        &actor,
        &ids,
        plan.id,
    )
    .await;
    assert!(
        matches!(outcome, Err(CoreError::Validation(_))),
        "an expired confirmation still ran the plan: {outcome:?}"
    );
    assert_eq!(
        notes_in(&pool, ws.id).await,
        0,
        "an expired confirmation produced an effect"
    );

    // And it is back where a person can act on it, not stuck in `executing`.
    let detail = lifecycle::detail(&pool, &actor, plan.id)
        .await
        .expect("detail");
    assert_eq!(detail.stored.state, PlanState::AwaitingApproval);
    assert!(detail.is_open());
}

/// A refusal is terminal, and cannot be undone by approving afterwards.
#[tokio::test]
async fn a_rejected_plan_can_never_run() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let mut actor = person(&pool, org, &["research_member"]).await;
    let ws = workspace(&pool, org, &mut actor).await;
    let ids = CorrelationIds::generate();

    let plan = note_plan(&pool, &actor, &ws).await;
    lifecycle::reject(&pool, &actor, &ids, plan.id)
        .await
        .expect("reject");

    // reject → approve → execute must not be a way back in.
    let approved = lifecycle::approve(&pool, &actor, &ids, plan.id).await;
    assert!(
        matches!(approved, Err(CoreError::Conflict(_))),
        "{approved:?}"
    );

    let executed = lifecycle::execute(
        &pool,
        capacidades(),
        &Realtime::ausente(),
        &actor,
        &ids,
        plan.id,
    )
    .await;
    assert!(
        matches!(executed, Err(CoreError::Conflict(_))),
        "{executed:?}"
    );

    assert_eq!(notes_in(&pool, ws.id).await, 0);

    // The record survives. That somebody said no is institutional history.
    let detail = lifecycle::detail(&pool, &actor, plan.id)
        .await
        .expect("still there");
    assert_eq!(detail.stored.state, PlanState::Rejected);
    assert!(!detail.is_open());
}

// ── 4. Consent is not authorization ─────────────────────────────────────

/// The one that matters most: consent cannot freeze a stale authorization.
///
/// A member proposes, confirms, and then loses the access the plan depends on.
/// The confirmation is still there, still theirs, still unexpired — and the
/// Core refuses anyway.
///
/// This test reloads the principal before executing, which is what an HTTP
/// request does. It therefore measures the **policy**: a current principal
/// without the membership is refused.
///
/// It does not measure whether the executor resists a stale principal. That is
/// `a_plan_cannot_run_on_authority_captured_before_it_was_revoked`, which hands
/// one in deliberately.
#[tokio::test]
async fn revoking_access_after_approval_stops_the_execution() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let mut actor = person(&pool, org, &["research_member"]).await;
    let ws = workspace(&pool, org, &mut actor).await;
    let ids = CorrelationIds::generate();

    let plan = note_plan(&pool, &actor, &ws).await;
    let record = lifecycle::approve(&pool, &actor, &ids, plan.id)
        .await
        .expect("approve");

    // The membership that made the workspace writable goes away.
    // Revoked, not deleted: memberships are withdrawn, and the policy reads
    // `revoked_at`. Deleting the row would test something the Core never does.
    sqlx::query(
        "UPDATE workspace_memberships SET revoked_at = now()
          WHERE workspace_id = $1 AND person_id = $2",
    )
    .bind(ws.id)
    .bind(actor.person_id)
    .execute(&pool)
    .await
    .expect("revoke");

    // The principal is rebuilt, exactly as the next request would build it.
    let actor = reload(&pool, actor.person_id).await;

    let executed = lifecycle::execute(
        &pool,
        capacidades(),
        &Realtime::ausente(),
        &actor,
        &ids,
        plan.id,
    )
    .await
    .expect("the lifecycle answers rather than failing");

    assert_ne!(
        executed.state,
        PlanState::Completed,
        "a plan ran on authorization the actor no longer had"
    );
    assert_eq!(
        notes_in(&pool, ws.id).await,
        0,
        "consent given earlier produced an effect the actor may no longer cause"
    );

    let refused = executed.plan.steps[0]
        .result
        .as_ref()
        .expect("every step reports");
    assert!(
        matches!(
            refused.status,
            ExecutionStatus::PermissionDenied | ExecutionStatus::ResourceNotFound
        ),
        "the step did not read as refused: {refused:?}"
    );

    // The confirmation was never invalidated — it simply was not authority.
    let still_there = plan_repo::approval_for(&pool, plan.id)
        .await
        .expect("query")
        .expect("the consent record survives");
    assert_eq!(still_there.approved_by, record.approved_by);
}

/// The same, for classification: material that moves out of reach.
#[tokio::test]
async fn raising_classification_after_approval_stops_the_execution() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let mut actor = person(&pool, org, &["research_member"]).await;
    let ws = workspace(&pool, org, &mut actor).await;
    let ids = CorrelationIds::generate();

    let plan = note_plan(&pool, &actor, &ws).await;
    lifecycle::approve(&pool, &actor, &ids, plan.id)
        .await
        .expect("approve");

    // The workspace becomes RESTRICTED, and the actor is only a `member` — the
    // membership still admits RESTRICTED reads, so this alone would not refuse.
    // Removing the membership *and* raising the classification is what puts the
    // material out of reach.
    sqlx::query("UPDATE research_workspaces SET classification = 'RESTRICTED' WHERE id = $1")
        .bind(ws.id)
        .execute(&pool)
        .await
        .expect("reclassify");
    // Revoked, not deleted: memberships are withdrawn, and the policy reads
    // `revoked_at`. Deleting the row would test something the Core never does.
    sqlx::query(
        "UPDATE workspace_memberships SET revoked_at = now()
          WHERE workspace_id = $1 AND person_id = $2",
    )
    .bind(ws.id)
    .bind(actor.person_id)
    .execute(&pool)
    .await
    .expect("revoke");

    let actor = reload(&pool, actor.person_id).await;
    let executed = lifecycle::execute(
        &pool,
        capacidades(),
        &Realtime::ausente(),
        &actor,
        &ids,
        plan.id,
    )
    .await
    .expect("answers");

    assert_ne!(executed.state, PlanState::Completed);
    assert_eq!(notes_in(&pool, ws.id).await, 0);
}

// ── 5. One effect, once ─────────────────────────────────────────────────

/// The whole path, and a real change to institutional state.
///
/// Natural language → the deterministic provider → a validated plan →
/// PostgreSQL → recovery → consent → execution-time authorization → the
/// Capability Executor → a domain service → a row that exists.
///
/// Nothing is mocked below the Runtime, and nothing is inserted by hand to
/// simulate an effect.
#[tokio::test]
async fn the_whole_lifecycle_changes_institutional_state_exactly_once() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let mut actor = person(&pool, org, &["research_member"]).await;
    let ws = workspace(&pool, org, &mut actor).await;
    let ids = CorrelationIds::generate();

    assert_eq!(notes_in(&pool, ws.id).await, 0);

    // 1. Natural language becomes a persisted proposal.
    let plan = note_plan(&pool, &actor, &ws).await;
    let detail = lifecycle::detail(&pool, &actor, plan.id)
        .await
        .expect("persisted");
    assert!(
        detail.requires_approval,
        "a mutation was offered without asking"
    );

    // 2. Consent.
    lifecycle::approve(&pool, &actor, &ids, plan.id)
        .await
        .expect("approve");

    // 3. Execution.
    let executed = lifecycle::execute(
        &pool,
        capacidades(),
        &Realtime::ausente(),
        &actor,
        &ids,
        plan.id,
    )
    .await
    .expect("execute");

    assert_eq!(
        executed.state,
        PlanState::Completed,
        "the plan did not complete: {} — {:?}",
        executed.summary,
        executed.plan.steps[0].result
    );
    assert_eq!(
        notes_in(&pool, ws.id).await,
        1,
        "the domain did not change: the effect is what the plan was for"
    );

    // 4. The result is the Core's, and the record says so.
    let settled = lifecycle::detail(&pool, &actor, plan.id)
        .await
        .expect("settled");
    assert_eq!(settled.stored.state, PlanState::Completed);
    assert!(!settled.is_open());

    // 5. A retry does not do it again.
    let again = lifecycle::execute(
        &pool,
        capacidades(),
        &Realtime::ausente(),
        &actor,
        &ids,
        plan.id,
    )
    .await;
    assert!(
        matches!(again, Err(CoreError::Conflict(_))),
        "a completed plan accepted a second execution: {again:?}"
    );
    assert_eq!(notes_in(&pool, ws.id).await, 1, "the effect happened twice");

    // 6. And it was audited, without the words that produced it.
    let audited: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_events
          WHERE resource_id = $1 AND action IN ('plan_created', 'plan_approved', 'plan_executed')",
    )
    .bind(plan.id)
    .fetch_one(&pool)
    .await
    .expect("count");
    assert_eq!(audited, 3, "the lifecycle was not fully audited");
}

/// Two executions at the same instant. At most one effect.
///
/// # Why this needs the database
///
/// Both requests read the plan as approved. Nothing in this process can stop
/// the second, because the gap they race through is between the application and
/// PostgreSQL — and a second instance of the Core would not share a lock held in
/// memory. The claim is one conditional `UPDATE`, and the row lock is what makes
/// «at most one» true.
#[tokio::test]
async fn two_concurrent_executions_produce_one_effect() {
    let Some(pool) = pool().await else { return };

    for round in 0..3 {
        let org = organisation(&pool).await;
        let mut actor = person(&pool, org, &["research_member"]).await;
        let ws = workspace(&pool, org, &mut actor).await;
        let ids = CorrelationIds::generate();

        let plan = note_plan(&pool, &actor, &ws).await;
        lifecycle::approve(&pool, &actor, &ids, plan.id)
            .await
            .expect("approve");

        let run = || {
            let pool = pool.clone();
            let actor = actor.clone();
            let ids = ids.clone();
            async move {
                lifecycle::execute(
                    &pool,
                    capacidades(),
                    &Realtime::ausente(),
                    &actor,
                    &ids,
                    plan.id,
                )
                .await
            }
        };

        let (first, second) = tokio::join!(run(), run());

        let winners = usize::from(first.is_ok()) + usize::from(second.is_ok());
        assert_eq!(
            winners, 1,
            "round {round}: {winners} executions were admitted, and only one may be"
        );

        assert_eq!(
            notes_in(&pool, ws.id).await,
            1,
            "round {round}: the effect did not happen exactly once"
        );
    }
}

/// Approve and reject at the same instant. One of them wins, and only one.
#[tokio::test]
async fn approve_and_reject_cannot_both_win() {
    let Some(pool) = pool().await else { return };

    for round in 0..3 {
        let org = organisation(&pool).await;
        let mut actor = person(&pool, org, &["research_member"]).await;
        let ws = workspace(&pool, org, &mut actor).await;
        let ids = CorrelationIds::generate();

        let plan = note_plan(&pool, &actor, &ws).await;

        let approving = {
            let (pool, actor, ids) = (pool.clone(), actor.clone(), ids.clone());
            async move {
                lifecycle::approve(&pool, &actor, &ids, plan.id)
                    .await
                    .is_ok()
            }
        };
        let rejecting = {
            let (pool, actor, ids) = (pool.clone(), actor.clone(), ids.clone());
            async move {
                lifecycle::reject(&pool, &actor, &ids, plan.id)
                    .await
                    .is_ok()
            }
        };

        let (approved, rejected) = tokio::join!(approving, rejecting);
        assert!(
            approved || rejected,
            "round {round}: neither operation was admitted"
        );

        // A plan has one state. Rejecting a plan one had just approved is a
        // legitimate change of mind, so «both calls returned Ok» is not the
        // failure — two states at once would be, and a row cannot hold two.
        let detail = lifecycle::detail(&pool, &actor, plan.id)
            .await
            .expect("detail");
        assert!(
            matches!(
                detail.stored.state,
                PlanState::Approved | PlanState::Rejected
            ),
            "round {round}: the plan settled in {:?}",
            detail.stored.state
        );

        // And the property that actually protects anything: if the refusal won,
        // nothing can run afterwards.
        if detail.stored.state == PlanState::Rejected {
            let executed = lifecycle::execute(
                &pool,
                capacidades(),
                &Realtime::ausente(),
                &actor,
                &ids,
                plan.id,
            )
            .await;
            assert!(
                matches!(executed, Err(CoreError::Conflict(_))),
                "round {round}: a rejected plan was executable: {executed:?}"
            );
            assert_eq!(notes_in(&pool, ws.id).await, 0);
        }
    }
}

/// An external effect never runs on a plan nobody confirmed.
///
/// Mail is the only capability in the registry with an effect outside the
/// institution, and it carries `ApprovalRequirement::Always`. No autonomy
/// level and no lifecycle state reaches it without a person.
#[tokio::test]
async fn an_external_effect_never_runs_unconfirmed_through_the_lifecycle() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let actor = person(&pool, org, &["research_member"]).await;
    let ids = CorrelationIds::generate();

    let outcome = propose(
        &pool,
        &actor,
        &FixtureProvider::cooperative(),
        "envia a mensagem ao Carlos",
    )
    .await;
    let runtime::AgenticOutcome::Planned {
        plan,
        requires_approval,
    } = outcome
    else {
        panic!("expected a plan");
    };

    assert!(requires_approval, "a send was offered without asking");

    let detail = lifecycle::detail(&pool, &actor, plan.id)
        .await
        .expect("persisted");
    assert_eq!(
        detail.stored.state,
        PlanState::AwaitingApproval,
        "a plan needing consent was not born waiting for it"
    );
    assert!(detail.requires_approval);

    // Straight to execution, with no confirmation.
    let executed = lifecycle::execute(
        &pool,
        capacidades(),
        &Realtime::ausente(),
        &actor,
        &ids,
        plan.id,
    )
    .await;
    assert!(
        matches!(executed, Err(CoreError::Validation(_))),
        "an unconfirmed external effect was admitted: {executed:?}"
    );
}

/// A plan that cannot be read settles as failed rather than staying claimed.
///
/// # Why this matters
///
/// Claiming a plan moves it to `executing`, and nothing transitions out of
/// `executing` — that is what makes the claim exclusive. So a failure between
/// the claim and the result has to settle the plan, or the row sits forever:
/// not runnable, not rejectable, and with nothing written down about why.
///
/// A material content that no longer matches its digest is the case that
/// produces this. It is refused — that is the immutability check doing its
/// job — and the plan lands in a terminal state that says so.
#[tokio::test]
async fn a_plan_that_cannot_be_read_settles_instead_of_staying_claimed() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let actor = person(&pool, org, &["research_member"]).await;
    let ids = CorrelationIds::generate();

    // Written straight to the table with a digest that does not describe it —
    // the shape a plan would have if something outside the Core had edited it.
    let plan_id: Uuid = sqlx::query_scalar(
        "INSERT INTO action_plans (organisation_id, requested_by, intent, steps, state, digest)
         VALUES ($1, $2, 'plano adulterado', $3, 'approved', 'nao-corresponde')
         RETURNING id",
    )
    .bind(org)
    .bind(actor.person_id)
    .bind(serde_json::json!([{
        "ordinal": 1,
        "summary": "x",
        "request": {
            "capability": "knowledge.search",
            "input": {"query": "x"},
            "resources": [],
            "dry_run": false
        },
        "risk": "read_only",
        "result": null
    }]))
    .fetch_one(&pool)
    .await
    .expect("insert");

    let executed = lifecycle::execute(
        &pool,
        capacidades(),
        &Realtime::ausente(),
        &actor,
        &ids,
        plan_id,
    )
    .await;
    assert!(
        matches!(executed, Err(CoreError::Internal(_))),
        "a plan that does not match its digest was run: {executed:?}"
    );

    let state: String = sqlx::query_scalar("SELECT state FROM action_plans WHERE id = $1")
        .bind(plan_id)
        .fetch_one(&pool)
        .await
        .expect("state");

    assert_eq!(
        state, "failed",
        "the plan was left claimed, so nothing could ever act on it again"
    );

    let settled: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT settled_at FROM action_plans WHERE id = $1")
            .bind(plan_id)
            .fetch_one(&pool)
            .await
            .expect("settled_at");
    assert!(
        settled.is_some(),
        "a terminal plan without a settling moment"
    );
}

/// Ter um `Principal` antigo em mão não faz o plano correr.
///
/// # Porque este teste existe além do anterior
///
/// `revoking_access_after_approval_stops_the_execution` recarrega o principal
/// antes de executar, tal como um pedido HTTP faria. Mede a política, e passava
/// mesmo quando o executor não tinha defesa nenhuma — porque era o teste, e não
/// o executor, a fazer o trabalho.
///
/// Este entrega ao executor um retrato tirado **antes** da revogação e não
/// recarrega coisa nenhuma. Antes da fronteira central, o plano corria até ao
/// fim e a nota era escrita. É a prova de que a autoridade se estabelece dentro
/// do executor, e não na disciplina de quem o chama (ADR-0411).
#[tokio::test]
async fn a_plan_cannot_run_on_authority_captured_before_it_was_revoked() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let mut actor = person(&pool, org, &["research_member"]).await;
    let ws = workspace(&pool, org, &mut actor).await;
    let ids = CorrelationIds::generate();

    let plan = note_plan(&pool, &actor, &ws).await;
    lifecycle::approve(&pool, &actor, &ids, plan.id)
        .await
        .expect("approve");

    // The snapshot, taken while the authority is still real.
    let stale = actor.clone();

    sqlx::query(
        "UPDATE workspace_memberships SET revoked_at = now()
          WHERE workspace_id = $1 AND person_id = $2",
    )
    .bind(ws.id)
    .bind(actor.person_id)
    .execute(&pool)
    .await
    .expect("revoke");

    // The snapshot really is stale: rebuilt from the database, the same person
    // no longer reaches the workspace. Without this the test could pass because
    // the snapshot never had the authority in the first place.
    let fresh = reload(&pool, actor.person_id).await;
    assert!(
        stale.workspace_ids().contains(&ws.id),
        "the snapshot never held the membership: nothing is being measured"
    );
    assert!(
        !fresh.workspace_ids().contains(&ws.id),
        "the membership was not actually revoked"
    );

    // The stale snapshot goes in. Nothing reloads it.
    let executed = lifecycle::execute(
        &pool,
        capacidades(),
        &Realtime::ausente(),
        &stale,
        &ids,
        plan.id,
    )
    .await
    .expect("the lifecycle answers");

    assert_ne!(
        executed.state,
        PlanState::Completed,
        "a plan ran on a Principal captured before the authority was revoked"
    );
    assert_eq!(
        notes_in(&pool, ws.id).await,
        0,
        "possession of a stale Principal produced an institutional effect"
    );
}

/// Uma conta suspensa não executa, mesmo com o plano confirmado.
///
/// A pertença continua lá; o que mudou foi a conta. Se a fronteira só olhasse
/// para pertenças, isto passaria — e uma pessoa suspensa continuaria a causar
/// efeitos institucionais enquanto tivesse planos por correr.
#[tokio::test]
async fn a_suspended_account_executes_nothing() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let mut actor = person(&pool, org, &["research_member"]).await;
    let ws = workspace(&pool, org, &mut actor).await;
    let ids = CorrelationIds::generate();

    let plan = note_plan(&pool, &actor, &ws).await;
    lifecycle::approve(&pool, &actor, &ids, plan.id)
        .await
        .expect("approve");

    let stale = actor.clone();
    assert!(stale.is_active, "o retrato não dizia activo: nada é medido");

    sqlx::query("UPDATE people SET status = 'suspended' WHERE id = $1")
        .bind(actor.person_id)
        .execute(&pool)
        .await
        .expect("suspend");

    let executed = lifecycle::execute(
        &pool,
        capacidades(),
        &Realtime::ausente(),
        &stale,
        &ids,
        plan.id,
    )
    .await;

    assert!(
        executed.is_err(),
        "um plano correu com a conta suspensa: {executed:?}"
    );
    assert_eq!(
        notes_in(&pool, ws.id).await,
        0,
        "uma conta suspensa produziu um efeito institucional"
    );
}

/// Retirado o papel, o plano deixa de correr.
///
/// Nem a conta nem a pertença mudaram: o que mudou foi a autoridade efectiva.
/// É o caso que distingue «resolver factos» de «verificar pertenças».
#[tokio::test]
async fn a_revoked_role_stops_the_execution() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let mut actor = person(&pool, org, &["research_member"]).await;
    let ws = workspace(&pool, org, &mut actor).await;
    let ids = CorrelationIds::generate();

    let plan = note_plan(&pool, &actor, &ws).await;
    lifecycle::approve(&pool, &actor, &ids, plan.id)
        .await
        .expect("approve");

    let stale = actor.clone();

    sqlx::query("DELETE FROM person_roles WHERE person_id = $1")
        .bind(actor.person_id)
        .execute(&pool)
        .await
        .expect("revoke role");
    sqlx::query("UPDATE workspace_memberships SET revoked_at = now() WHERE person_id = $1")
        .bind(actor.person_id)
        .execute(&pool)
        .await
        .expect("revoke membership");

    let fresh = reload(&pool, actor.person_id).await;
    assert!(
        stale.has_role(&[ocinye_contracts::TechnicalRole::ResearchMember]),
        "o retrato não tinha o papel: nada é medido"
    );
    assert!(
        !fresh.has_role(&[ocinye_contracts::TechnicalRole::ResearchMember]),
        "a autoridade não mudou de facto: o teste não mede nada"
    );

    let executed = lifecycle::execute(
        &pool,
        capacidades(),
        &Realtime::ausente(),
        &stale,
        &ids,
        plan.id,
    )
    .await
    .expect("the lifecycle answers");

    assert_ne!(executed.state, PlanState::Completed);
    assert_eq!(notes_in(&pool, ws.id).await, 0);
}

/// Quem não se consegue resolver não executa.
///
/// # Porque este teste chama o resolvedor directamente
///
/// Porque «a pessoa deixou de existir» não se consegue montar através do ciclo
/// de vida: `action_plans.requested_by` referencia `people`, e as chaves
/// estrangeiras impedem tanto apagar a pessoa como apontar o plano a alguém que
/// não existe. Tentei-o, e o `DELETE` foi recusado — o teste passava a medir
/// nada.
///
/// O que é alcançável, e é o que importa, é a fronteira em si: perante uma
/// identidade que não resolve, ela recusa em vez de deixar passar com o que
/// tinha em mão.
#[tokio::test]
async fn authority_that_cannot_be_resolved_fails_closed() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let actor = person(&pool, org, &["research_member"]).await;

    // Controlo positivo: quem existe resolve-se.
    let corrente =
        ocinye_core::authority::resolve(&pool, ocinye_core::authority::ActorRef::of(&actor)).await;
    assert!(
        corrente.is_ok(),
        "um actor real não se resolveu: {corrente:?}"
    );

    // Ninguém.
    let ninguem = ocinye_core::authority::resolve(
        &pool,
        ocinye_core::authority::ActorRef {
            person_id: Uuid::new_v4(),
            organisation_id: org,
        },
    )
    .await;
    assert!(
        ninguem.is_err(),
        "uma identidade inexistente produziu autoridade"
    );

    // Conta suspensa: existe, e não age.
    sqlx::query("UPDATE people SET status = 'suspended' WHERE id = $1")
        .bind(actor.person_id)
        .execute(&pool)
        .await
        .expect("suspend");
    let suspensa =
        ocinye_core::authority::resolve(&pool, ocinye_core::authority::ActorRef::of(&actor)).await;
    assert!(suspensa.is_err(), "uma conta suspensa produziu autoridade");

    // Outra organização: a identidade existe, mas não aqui.
    sqlx::query("UPDATE people SET status = 'active' WHERE id = $1")
        .bind(actor.person_id)
        .execute(&pool)
        .await
        .expect("reactivate");
    let outra = ocinye_core::authority::resolve(
        &pool,
        ocinye_core::authority::ActorRef {
            person_id: actor.person_id,
            organisation_id: Uuid::new_v4(),
        },
    )
    .await;
    assert!(
        outra.is_err(),
        "uma identidade de outra organização produziu autoridade nesta"
    );
}

/// O Capability Runtime, com os componentes desta árvore.
///
/// Uma vez por processo: ler e compilar um módulo custa, e estas suites chamam
/// o executor muitas vezes.
fn capacidades() -> &'static ocinye_core::capabilities::Capabilities {
    use std::sync::OnceLock;
    static UM: OnceLock<ocinye_core::capabilities::Capabilities> = OnceLock::new();
    UM.get_or_init(|| {
        ocinye_core::capabilities::Capabilities::load(&format!(
            "{}/../../target/wasm32-wasip1/release",
            env!("CARGO_MANIFEST_DIR")
        ))
        .expect("motor de capacidades")
    })
}
