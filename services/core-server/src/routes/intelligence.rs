//! Intelligence Plane routes.
//!
//! These endpoints exist even though no Ocinye AI node does. They report the
//! true state — unavailable — rather than hiding the section or, worse,
//! reaching for an external provider to make it look populated (ADR-0300).

use axum::extract::{Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use ocinye_contracts::{
    AiCapability, Classification, IntelligenceStatus, Permission, SystemCapability,
};
use ocinye_core::modules::intelligence::{self, AgentScope, NewAgent};
use ocinye_core::modules::platform;
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
/// # Why this endpoint exists with no AI node
///
/// Because the alternative is worse. Without it the Workspace form posts to a
/// route that does not exist and the member gets a bare 405 — the exact "botão
/// que não faz nada" this audit exists to remove (briefing §8).
///
/// With no capability available it returns
/// [`CoreError::CapabilityUnavailable`], which carries a 503 and an
/// institutional message. The Workspace renders that as a native state, not an
/// alert.
async fn submit_prompt(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
    Json(request): Json<PromptRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // Permission first, then availability. Conflating them would tell someone
    // who may not use AI that the hardware is missing, and someone who may that
    // they lack permission (briefing §57).
    require(&principal, Permission::AiUse, &ids)?;

    if request.prompt.trim().is_empty() {
        return Err(ApiError::new(
            CoreError::Validation("Escreva um pedido antes de enviar.".to_owned()),
            &ids,
        ));
    }

    let capability = parse_capability(request.capability.as_deref(), &ids)?;

    let capabilities = platform::system_capabilities(
        &state.pool,
        &state.config,
        state.store.is_some(),
        state.mail_registry.reachability().await,
    )
    .await
    .map_err(|error| ApiError::new(error, &ids))?;

    let system = match capability {
        AiCapability::General => SystemCapability::AiGeneral,
        AiCapability::Coding => SystemCapability::AiCoding,
        AiCapability::Reasoning => SystemCapability::AiReasoning,
        AiCapability::Embedding => SystemCapability::AiEmbedding,
    };

    if !capabilities.is_usable(system) {
        let reason = capabilities.get(system).map_or_else(
            || "Nenhuma capacidade de IA compatível está disponível.".to_owned(),
            |report| report.reason.clone(),
        );
        // Recorded so an operator can see demand for a capability that does not
        // yet exist — which is exactly the evidence that justifies a node.
        // Best-effort: failing to record demand must not change the answer.
        if let Ok(mut tx) = state.pool.begin().await {
            let recorded = intelligence::record_rejected_job(
                &mut tx,
                &principal,
                capability,
                ocinye_contracts::RagScope::Institutional,
                None,
                &reason,
            )
            .await;
            if recorded.is_ok() {
                let _ = tx.commit().await;
            }
        }
        return Err(ApiError::new(
            CoreError::CapabilityUnavailable(reason),
            &ids,
        ));
    }

    // Reached only when a capability is genuinely available, which cannot
    // happen in this installation today. The inference path itself is
    // `PLANNED`: see docs/ai/ and the Feature Status document.
    Err(ApiError::new(
        CoreError::CapabilityUnavailable(
            "A execução de pedidos ainda não está activada nesta instalação do \
             Ocinye OS, mesmo com uma capacidade registada."
                .to_owned(),
        ),
        &ids,
    ))
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
