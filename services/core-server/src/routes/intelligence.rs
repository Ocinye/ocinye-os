//! Intelligence Plane routes.
//!
//! These endpoints exist even though no Ocinye AI node does. They report the
//! true state — unavailable — rather than hiding the section or, worse,
//! reaching for an external provider to make it look populated (ADR-0300).

use axum::extract::{Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use ocinye_contracts::{
    AiCapability, AiInteractionResponse, AiReasonCode, Classification, IntelligenceStatus,
    InteractionOrigin, InteractionStatus, Permission,
};
use ocinye_core::modules::intelligence::{
    self, infer_within_deadline, AgentScope, InferenceRequest, ModelResolution, NewAgent,
};
use ocinye_core::modules::{platform, resource};
use ocinye_core::CoreError;
use ocinye_domain::{can, ResourceContext, ResourceKind};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ApiError;
use crate::extract::{CurrentPrincipal, Ids};
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/ai/status", get(status))
        .route("/ai/models", get(list_models))
        .route("/ai/context-preview", get(context_preview))
        .route("/ai/agents", get(list_agents).post(create_agent))
        .route("/ai/prompt", post(submit_prompt))
}

/// Report what the Intelligence Plane can currently do.
async fn status(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
) -> Result<Json<IntelligenceStatus>, ApiError> {
    Ok(Json(
        intelligence::intelligence_status(&state.pool, &principal, &state.config.ai).await?,
    ))
}

#[derive(Serialize)]
struct ModelView {
    id: Uuid,
    provider_kind: String,
    provider_name: String,
    model_name: String,
    version: String,
    capabilities: serde_json::Value,
    context_limit: Option<i32>,
    status: String,
    /// Ceiling on what may ever be sent to this model.
    max_classification: String,
    enabled: bool,
}

async fn list_models(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
) -> Result<Json<Vec<ModelView>>, ApiError> {
    let models = intelligence::list_models(&state.pool, &principal).await?;
    Ok(Json(
        models
            .into_iter()
            .map(|model| ModelView {
                id: model.id,
                provider_kind: model.provider_kind,
                provider_name: model.provider_name,
                model_name: model.model_name,
                version: model.version,
                capabilities: model.capabilities,
                context_limit: model.context_limit,
                status: model.status,
                max_classification: model.max_classification,
                enabled: model.enabled,
            })
            .collect(),
    ))
}

#[derive(Deserialize)]
struct ContextQuery {
    q: String,
    #[serde(default)]
    workspace_id: Option<Uuid>,
    /// Ceiling to simulate. Defaults to `INTERNAL`, the safe assumption for a
    /// model whose approval has not been stated.
    #[serde(default)]
    max_classification: Option<String>,
}

/// Show exactly which artefacts a retrieval would place in a model's context.
///
/// This endpoint exists so the retrieval boundary is inspectable *before* any
/// model exists to consume it: a member can see that context assembly returns
/// only what they themselves may read, and nothing above the model's ceiling
/// (ADR-0300).
async fn context_preview(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Query(query): Query<ContextQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let ceiling = query
        .max_classification
        .as_deref()
        .and_then(ocinye_contracts::Classification::parse)
        .unwrap_or(ocinye_contracts::Classification::Internal);

    let scope = if query.workspace_id.is_some() {
        ocinye_contracts::RagScope::ResearchWorkspace
    } else {
        ocinye_contracts::RagScope::Institutional
    };

    let refs = intelligence::assemble_context(
        &state.pool,
        &principal,
        &query.q,
        scope,
        query.workspace_id,
        ceiling,
    )
    .await?;

    Ok(Json(serde_json::json!({
        "scope": scope.as_str(),
        "model_max_classification": ceiling.as_str(),
        "artefacts": refs.len(),
        "references": refs,
        "note": "Retrieval applies the caller's own read policy before assembly, then the \
                 model's classification ceiling. Retrieved content is data, never instruction.",
    })))
}

// ── Agents ──────────────────────────────────────────────────────────────

/// `GET /ai/agents`
///
/// Returns only agents the caller may see, and each one's **derived** execution
/// state. With no AI node every agent reads `configured`, never `ready`: an
/// agent must not claim it can run when nothing can serve it (briefing §9).
async fn list_agents(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
) -> Result<Json<AgentList>, ApiError> {
    require(&principal, Permission::AgentsView, &ids)?;

    let capabilities = platform::system_capabilities(
        &state.pool,
        &state.config,
        state.store.is_some(),
        state.mail_registry.reachability().await,
    )
    .await
    .map_err(|error| ApiError::new(error, &ids))?;

    let agents = intelligence::agents::list(&state.pool, &principal, &capabilities)
        .await
        .map_err(|error| ApiError::new(error, &ids))?;

    Ok(Json(AgentList {
        total: agents.len(),
        // Stated so the Workspace can explain *why* every agent reads
        // `configured`, rather than leaving a member to deduce it.
        execution_available: capabilities.any_ai_usable(),
        items: agents,
    }))
}

#[derive(Serialize)]
struct AgentList {
    items: Vec<intelligence::Agent>,
    total: usize,
    execution_available: bool,
}

#[derive(Deserialize)]
struct CreateAgentRequest {
    name: String,
    #[serde(default)]
    purpose: Option<String>,
    #[serde(default)]
    instructions: Option<String>,
    #[serde(default)]
    capability: Option<String>,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    scope_id: Option<Uuid>,
    #[serde(default)]
    max_classification: Option<String>,
    #[serde(default)]
    uses_bibliography: bool,
    #[serde(default)]
    uses_documents: bool,
    #[serde(default)]
    uses_datasets: bool,
}

/// `POST /ai/agents`
///
/// Deliberately available with **no AI node registered**. An agent is a
/// definition; defining one needs no model. It simply cannot execute until a
/// capability can serve it, and its state says so (briefing §10).
async fn create_agent(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
    Json(request): Json<CreateAgentRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // Maiúsculas na fronteira: a representação canónica é `GENERAL`, e recusar
    // `general` por causa da caixa ajudaria ninguém.
    let capability = parse_capability(request.capability.as_deref(), &ids)?;

    let scope = request
        .scope
        .as_deref()
        .map_or(Some(AgentScope::Personal), AgentScope::parse)
        .ok_or_else(|| {
            ApiError::new(
                CoreError::Validation("Âmbito de agente desconhecido.".to_owned()),
                &ids,
            )
        })?;

    let max_classification = request
        .max_classification
        .as_deref()
        .map_or(Some(Classification::Internal), Classification::parse)
        .ok_or_else(|| {
            ApiError::new(
                CoreError::Validation("Classificação desconhecida.".to_owned()),
                &ids,
            )
        })?;

    let id = intelligence::agents::create(
        &state.pool,
        &principal,
        &NewAgent {
            name: request.name,
            purpose: request.purpose,
            instructions: request.instructions,
            capability,
            scope,
            scope_id: request.scope_id,
            max_classification,
            uses_bibliography: request.uses_bibliography,
            uses_documents: request.uses_documents,
            uses_datasets: request.uses_datasets,
        },
        &ids,
    )
    .await
    .map_err(|error| ApiError::new(error, &ids))?;

    Ok(Json(serde_json::json!({ "id": id })))
}

// ── Prompt ──────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct PromptRequest {
    #[serde(default)]
    prompt: String,
    #[serde(default)]
    capability: Option<String>,
}

/// `POST /ai/prompt`
///
/// # A processed request is not a failed one
///
/// The Prompt is a command surface, not a model widget: it stays operational
/// whenever the Core is healthy and the member may use AI, even with zero
/// providers, models or nodes. A request that is received, authorized and
/// processed but that no inference capacity can execute did **not** fail —
/// it concluded, deterministically, as `DEGRADED`. So this endpoint returns
/// HTTP success with a typed [`AiInteractionResponse`] whose `origin` is
/// `SYSTEM`, never a 503 (M5 §5, §6). Nothing is broken; there is simply no
/// model, and no external provider is reached in its place.
///
/// Real failures still fail: a caller without `AiUse` is denied (403), and an
/// empty prompt is a validation error (400). Those are genuine errors, kept
/// distinct from the degraded conclusion.
async fn submit_prompt(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
    Json(request): Json<PromptRequest>,
) -> Result<Json<AiInteractionResponse>, ApiError> {
    // Permission first, then availability. Conflating them would tell someone
    // who may not use AI that the hardware is missing, and someone who may that
    // they lack permission (briefing §57). Permission denial is a real failure
    // and stays an error, not a degraded envelope.
    require(&principal, Permission::AiUse, &ids)?;

    if request.prompt.trim().is_empty() {
        return Err(ApiError::new(
            CoreError::Validation("Escreva um pedido antes de enviar.".to_owned()),
            &ids,
        ));
    }

    let capability = parse_capability(request.capability.as_deref(), &ids)?;

    // The Model Router is the authority on availability for this path. It reads
    // the node-reported inventory fresh on every request, so a node that
    // connects or drops changes the answer with no restart — and «zero models»
    // is an ordinary typed result, not an error (M5 §20, §21).
    let resolution = intelligence::resolve_capability(&state.pool, &state.config.ai, capability)
        .await
        .map_err(|error| ApiError::new(error, &ids))?;

    let model = match resolution {
        ModelResolution::NoCandidate(reason_code) => {
            return Ok(Json(
                degraded(&state, &principal, capability, reason_code).await,
            ));
        }
        ModelResolution::Resolved(model) => model,
    };

    // A model serves the capability, but only a provider wired into this process
    // can run it. In production that is `NoProvider`, whose `serves` is always
    // false, so this degrades cleanly with no model involved.
    if !state.inference.serves(capability) {
        return Ok(Json(
            degraded(
                &state,
                &principal,
                capability,
                AiReasonCode::AiNoProviderAvailable,
            )
            .await,
        ));
    }

    // ── Resource admission (ADR-0108) ──────────────────────────────────
    //
    // A model serves and a provider can run it. Before inference, admit the
    // request against the member's AI entitlement, fail-closed. The admission
    // transaction holds a per-member advisory lock across the inference call —
    // it is the request's reservation — and either commits the usage (a model
    // answered) or releases it (it did not), atomically. With no `model_access`
    // entitlement configured the limit is zero and everything is admitted, so
    // an installation that has not chosen to meter AI is not blocked by it.
    let mut tx = match state.pool.begin().await {
        Ok(tx) => tx,
        Err(_) => {
            return Ok(Json(
                degraded(
                    &state,
                    &principal,
                    capability,
                    AiReasonCode::AiProviderUnhealthy,
                )
                .await,
            ));
        }
    };
    match resource::admit_ai_access(&mut tx, principal.person_id, 1).await {
        Ok(true) => {}
        Ok(false) => {
            drop(tx); // release the reservation
            return Ok(Json(
                degraded(
                    &state,
                    &principal,
                    capability,
                    AiReasonCode::AiResourceQuotaExceeded,
                )
                .await,
            ));
        }
        Err(_) => {
            drop(tx);
            return Ok(Json(
                degraded(
                    &state,
                    &principal,
                    capability,
                    AiReasonCode::AiProviderUnhealthy,
                )
                .await,
            ));
        }
    }

    // Execute, still holding the reservation. The prompt carries no retrieved
    // context here — permission-aware context assembly is a separate concern —
    // so no classified material leaves the Core (ADR-0300, ADR-0304). The Core
    // enforces its own contract around the provider via `infer_within_deadline`.
    let inference = InferenceRequest::new(capability, system_instruction(), request.prompt.clone());
    match infer_within_deadline(state.inference.as_ref(), &inference).await {
        Ok(response) => {
            // Commit the reservation: record the completed job and the usage in
            // the admission transaction, so the charge lands with the answer.
            let job_id = intelligence::record_completed_job(
                &mut tx,
                &principal,
                capability,
                ocinye_contracts::RagScope::Institutional,
                None,
                model.id,
            )
            .await
            .ok();
            let _ = resource::record_ai_access(
                &mut *tx,
                principal.organisation_id,
                principal.person_id,
                Some(model.id),
                model.node_id,
                job_id,
                &ids.correlation_id,
                1,
            )
            .await;
            let _ = tx.commit().await;

            Ok(Json(AiInteractionResponse {
                origin: InteractionOrigin::Model,
                status: InteractionStatus::Completed,
                reason_code: None,
                model: Some(response.model.model),
                provider: Some(response.model.provider),
                compute_node: model.node_id.map(|id| id.to_string()),
                content: response.text,
            }))
        }
        Err(error) => {
            drop(tx); // release: a failed request charges nothing
            Ok(Json(
                degraded(
                    &state,
                    &principal,
                    capability,
                    reason_code_for_inference_error(error),
                )
                .await,
            ))
        }
    }
}

/// Build the degraded system envelope and record the demand.
///
/// A degraded conclusion is a `SYSTEM` answer: the request was processed, and no
/// inference could run it. Recording the demand is best-effort — failing to
/// record must never change the answer.
async fn degraded(
    state: &AppState,
    principal: &ocinye_domain::Principal,
    capability: AiCapability,
    reason_code: AiReasonCode,
) -> AiInteractionResponse {
    if let Ok(mut tx) = state.pool.begin().await {
        let recorded = intelligence::record_rejected_job(
            &mut tx,
            principal,
            capability,
            ocinye_contracts::RagScope::Institutional,
            None,
            reason_code.as_str(),
        )
        .await;
        if recorded.is_ok() {
            let _ = tx.commit().await;
        }
    }
    AiInteractionResponse::degraded(reason_code, degraded_prompt_content())
}

/// The Ocinye OS's own instruction to a model.
///
/// Written by the Core, never by a member and never by retrieved content
/// (ADR-0304). Kept short and deterministic.
fn system_instruction() -> String {
    "És o Ocinye AI, ao serviço do sistema operacional institucional da Ocinye. Responde em \
     português europeu, com rigor, e nunca inventes factos."
        .to_owned()
}

/// Map an inference failure to a machine reason for the degraded envelope.
///
/// `NoProvider` is «nothing serves this»; every other failure means a provider
/// was present but could not produce a usable answer — an unhealthy provider.
fn reason_code_for_inference_error(error: intelligence::InferenceError) -> AiReasonCode {
    match error {
        intelligence::InferenceError::NoProvider => AiReasonCode::AiNoProviderAvailable,
        intelligence::InferenceError::ContextExceeded => AiReasonCode::AiModelHardwareNotSatisfied,
        _ => AiReasonCode::AiProviderUnhealthy,
    }
}

/// The deterministic pt-PT answer shown when no inference could run.
///
/// Written by the platform, not a model: it says what happened plainly, affirms
/// the Prompt is operational, and states that no external provider is used in
/// substitution (M5 §9).
fn degraded_prompt_content() -> String {
    "Nenhuma capacidade de inferência está actualmente disponível para executar este pedido. O \
     Prompt Ocinye continua operacional, e nenhum fornecedor externo é utilizado em substituição."
        .to_owned()
}

/// Read a capability from a request, defaulting to `GENERAL`.
fn parse_capability(
    value: Option<&str>,
    ids: &ocinye_observability::CorrelationIds,
) -> Result<AiCapability, ApiError> {
    match value.map(str::trim).filter(|v| !v.is_empty()) {
        None => Ok(AiCapability::General),
        Some(raw) => AiCapability::parse(&raw.to_uppercase()).ok_or_else(|| {
            ApiError::new(
                CoreError::Validation("Capacidade de IA desconhecida.".to_owned()),
                ids,
            )
        }),
    }
}

/// Authorise a permission at institution scope, or fail closed.
fn require(
    principal: &ocinye_domain::Principal,
    permission: Permission,
    ids: &ocinye_observability::CorrelationIds,
) -> Result<(), ApiError> {
    let ctx = ResourceContext::organisation(ResourceKind::AiCapability, principal.organisation_id);
    if can(principal, permission, &ctx, None).allowed {
        Ok(())
    } else {
        Err(ApiError::new(
            CoreError::PermissionDenied("Não possui acesso a esta operação.".to_owned()),
            ids,
        ))
    }
}
