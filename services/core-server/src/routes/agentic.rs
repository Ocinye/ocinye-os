//! Agentic Control Plane routes.
//!
//! # What this surface deliberately does not expose
//!
//! No route runs a capability by identifier on request. Execution happens
//! through a **plan**, which was validated, which the person owns, and which —
//! when it changes anything material — they confirmed. A `POST /capability/run`
//! taking an identifier and a JSON blob would be the shell this architecture
//! exists to avoid (briefing §7).

use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use ocinye_contracts::agentic::{Intent, PlanState, ResourceKind as AgenticKind, ResourceRef};
use ocinye_contracts::{Page, PageRequest};
use ocinye_core::modules::agentic::{self, lifecycle, runtime};
use ocinye_core::modules::platform;
use ocinye_core::CoreError;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ApiError;
use crate::extract::{CurrentPrincipal, Ids};
use crate::state::AppState;

/// Agentic routes.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/agentic/invoke", post(invoke))
        .route("/agentic/capabilities", get(capabilities))
        .route("/agentic/plans", get(own_plans))
        .route("/agentic/plans/{plan_id}", get(plan_detail))
        .route("/agentic/plans/{plan_id}/approve", post(approve))
        .route("/agentic/plans/{plan_id}/reject", post(reject))
        .route("/agentic/plans/{plan_id}/execute", post(execute))
}

// ── Invoke ──────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct InvokeRequest {
    /// What the member typed.
    utterance: String,
    /// What they want. Chosen by the surface, not inferred by a model.
    #[serde(default)]
    intent: Option<String>,
    /// Where they were.
    #[serde(default)]
    module: Option<String>,
    #[serde(default)]
    workspace_id: Option<Uuid>,
    /// The one resource they were looking at, when there is one.
    #[serde(default)]
    resource_kind: Option<String>,
    #[serde(default)]
    resource_id: Option<Uuid>,
    /// Everything they had selected, when the surface supports selection.
    ///
    /// Bounded on purpose: a request naming a hundred resources is not a
    /// selection, and resolving each one costs a query.
    #[serde(default)]
    selection: Vec<SelectedResource>,
}

/// One entry in a member's selection, as the interface sends it.
#[derive(Debug, Deserialize)]
struct SelectedResource {
    kind: String,
    id: Uuid,
}

/// How many resources one request may point at.
const MAX_SELECTION: usize = 12;

/// `POST /agentic/invoke`
///
/// The one entry point of the command surface. Answers `Search` without a
/// model, and says why it cannot answer `Ask` or `Act` when nothing can serve
/// them.
async fn invoke(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
    Json(request): Json<InvokeRequest>,
) -> Result<Json<runtime::AgenticOutcome>, ApiError> {
    let utterance = request.utterance.trim();
    if utterance.is_empty() {
        return Err(ApiError::new(
            CoreError::Validation("Escreva o que procura ou o que pretende.".to_owned()),
            &ids,
        ));
    }

    // An explicit choice wins. Absent one, the surface reads the sentence —
    // deterministically, and always downward when ambiguous, so a phrase read
    // wrongly shows results rather than performing something
    // (`Intent::detect`).
    let intent = request
        .intent
        .as_deref()
        .and_then(Intent::parse)
        .unwrap_or_else(|| Intent::detect(utterance));

    // The single resource the member is looking at, and anything they
    // selected, become one list. The Core does not care which field a reference
    // arrived in; it resolves every one of them the same way.
    let mut selection: Vec<ResourceRef> = Vec::new();
    if let (Some(kind), Some(id)) = (request.resource_kind.as_deref(), request.resource_id) {
        if let Some(kind) = AgenticKind::parse(kind) {
            selection.push(ResourceRef {
                kind,
                id,
                // Never from the client. The Core supplies the title on
                // resolution, so a label nobody checked cannot reach a plan.
                label: None,
            });
        }
    }
    for entry in &request.selection {
        if let Some(kind) = AgenticKind::parse(&entry.kind) {
            let reference = ResourceRef {
                kind,
                id: entry.id,
                label: None,
            };
            if !selection
                .iter()
                .any(|existing| existing.id == reference.id && existing.kind == reference.kind)
            {
                selection.push(reference);
            }
        }
    }

    if selection.len() > MAX_SELECTION {
        return Err(ApiError::new(
            CoreError::Validation(format!(
                "Seleccione no máximo {MAX_SELECTION} recursos de cada vez."
            )),
            &ids,
        ));
    }

    let capabilities = platform::system_capabilities(
        &state.pool,
        &state.config,
        state.store.is_some(),
        state.mail_registry.reachability().await,
    )
    .await
    .map_err(|error| ApiError::new(error, &ids))?;

    let outcome = agentic::invoke(
        &state.pool,
        &principal,
        state.inference.as_ref(),
        &runtime::AgenticRequest {
            utterance,
            intent,
            module: request.module.as_deref(),
            workspace_id: request.workspace_id,
            selection: &selection,
            // The member is watching a command bar. The standard deadline.
            deadline: None,
        },
        &capabilities,
        &ids,
    )
    .await
    .map_err(|error| ApiError::new(error, &ids))?;

    Ok(Json(outcome))
}

// ── Capabilities ────────────────────────────────────────────────────────

#[derive(Serialize)]
struct CapabilityView {
    id: String,
    domain: String,
    summary: String,
    risk: &'static str,
    risk_label: &'static str,
    requires_approval: bool,
    reversibility: &'static str,
    supports_dry_run: bool,
    /// Whether the acting person could use this at all.
    available_to_you: bool,
}

/// `GET /agentic/capabilities`
///
/// What the Ocinye OS publishes to its agents. Everything, with a flag for what
/// this person could use — an administration view rather than the filtered set
/// a model receives.
async fn capabilities(
    State(_state): State<AppState>,
    Ids(_ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
) -> Json<Vec<CapabilityView>> {
    let mine: Vec<String> = agentic::registry::registry()
        .available_to(&principal, None)
        .into_iter()
        .map(|descriptor| descriptor.id.as_str().to_owned())
        .collect();

    Json(
        agentic::registry::registry()
            .all()
            .into_iter()
            .map(|descriptor| CapabilityView {
                available_to_you: mine.iter().any(|id| id == descriptor.id.as_str()),
                id: descriptor.id.as_str().to_owned(),
                domain: descriptor.domain.clone(),
                summary: descriptor.summary.clone(),
                risk: descriptor.risk.as_str(),
                risk_label: descriptor.risk.label(),
                requires_approval: descriptor.requires_approval(),
                reversibility: descriptor.reversibility.as_str(),
                supports_dry_run: descriptor.supports_dry_run,
            })
            .collect(),
    )
}

// ── Plans ───────────────────────────────────────────────────────────────

/// One plan, as a caller sees it.
///
/// # What is here, and what is deliberately not
///
/// Enough to render the lifecycle and decide on it: what it proposes to do, how
/// risky the Core considers that **now**, where it stands, and whether it can
/// still be acted on.
///
/// Absent: the utterance that produced it, the retrieved material, the model's
/// words, the provider's response. None of those are stored, so none of them
/// can be served (ADR-0301).
#[derive(Serialize)]
struct PlanView {
    id: Uuid,
    intent: String,
    state: &'static str,
    state_label: &'static str,
    created_at: chrono::DateTime<chrono::Utc>,
    steps: serde_json::Value,
    /// The peak risk of the plan **as the registry classifies it today**, not
    /// the value stored when the plan was built.
    risk: &'static str,
    risk_label: &'static str,
    /// Whether a person still has to confirm before anything runs.
    requires_approval: bool,
    /// Whether a live confirmation exists for this exact plan, right now.
    approved: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    approval_expires_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Whether the lifecycle still admits an approval, rejection or execution.
    ///
    /// Derived from the state, so an interface never offers a control the Core
    /// would refuse (briefing §66).
    open: bool,
}

impl From<lifecycle::PlanDetail> for PlanView {
    fn from(detail: lifecycle::PlanDetail) -> Self {
        Self {
            open: detail.is_open(),
            id: detail.stored.id,
            intent: detail.stored.intent,
            state: detail.stored.state.as_str(),
            state_label: detail.stored.state.label(),
            created_at: detail.stored.created_at,
            steps: detail.stored.steps,
            risk: detail.risk.as_str(),
            risk_label: detail.risk.label(),
            requires_approval: detail.requires_approval,
            approved: detail.approved,
            approval_expires_at: detail.approval_expires_at,
        }
    }
}

#[derive(Deserialize)]
struct PlanPageQuery {
    #[serde(default)]
    page: Option<u32>,
    #[serde(default)]
    page_size: Option<u32>,
}

/// `GET /agentic/plans`
///
/// Answers «o que é que o Ocinye fez por mim?». One's own, always: the query
/// filters on the requester, so knowing another plan's identifier reaches
/// nothing (briefing §135).
async fn own_plans(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
    Query(query): Query<PlanPageQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let page = PageRequest {
        page: query.page.unwrap_or(1),
        page_size: query
            .page_size
            .unwrap_or(ocinye_contracts::page::DEFAULT_PAGE_SIZE),
    };

    let (plans, total) = lifecycle::list(&state.pool, &principal, page)
        .await
        .map_err(|error| ApiError::new(error, &ids))?;

    let items: Vec<PlanView> = plans.into_iter().map(PlanView::from).collect();
    Ok(Json(
        serde_json::to_value(Page::new(items, page, total)).unwrap_or(serde_json::Value::Null),
    ))
}

/// `GET /agentic/plans/{id}`
///
/// One plan, if it is the caller's. A plan somebody else asked for reads as
/// absent: whether it exists is not the caller's business (`CLAUDE.md` §60).
async fn plan_detail(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(plan_id): Path<Uuid>,
) -> Result<Json<PlanView>, ApiError> {
    let detail = lifecycle::detail(&state.pool, &principal, plan_id)
        .await
        .map_err(|error| ApiError::new(error, &ids))?;

    Ok(Json(PlanView::from(detail)))
}

/// `POST /agentic/plans/{id}/approve`
///
/// Records consent. **Runs nothing.** Approval and execution stay separate acts
/// because they answer separate questions: «do you want this to happen» and
/// «may it happen now».
async fn approve(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(plan_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let record = lifecycle::approve(&state.pool, &principal, &ids, plan_id)
        .await
        .map_err(|error| ApiError::new(error, &ids))?;

    Ok(Json(serde_json::json!({
        "approved": true,
        "state": PlanState::Approved.as_str(),
        "expires_at": record.expires_at,
        // Said plainly, because it is the whole distinction this endpoint
        // exists to preserve.
        "note": "A confirmação foi registada. Nada foi executado: o Ocinye Core \
                 volta a autorizar cada passo no momento de o executar.",
    })))
}

/// `POST /agentic/plans/{id}/reject`
///
/// Terminal. A rejected plan is never executable again.
async fn reject(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(plan_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    lifecycle::reject(&state.pool, &principal, &ids, plan_id)
        .await
        .map_err(|error| ApiError::new(error, &ids))?;

    Ok(Json(serde_json::json!({
        "rejected": true,
        "state": PlanState::Rejected.as_str(),
    })))
}

/// `POST /agentic/plans/{id}/execute`
///
/// The plan must be the caller's and still open; the claim on it is atomic; the
/// confirmation must be theirs, for this digest, and unexpired; and every step
/// is authorised **again** by the Capability Executor.
async fn execute(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(plan_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let executed = lifecycle::execute(
        &state.pool,
        &state.capabilities,
        &state.realtime,
        &principal,
        &ids,
        plan_id,
    )
    .await
    .map_err(|error| ApiError::new(error, &ids))?;

    Ok(Json(serde_json::json!({
        "state": executed.state.as_str(),
        "summary": executed.summary,
        "undoable": executed.undoable,
        "steps": serde_json::to_value(&executed.plan.steps).unwrap_or(serde_json::Value::Null),
    })))
}
