//! The AI Fabric's provider registry (ADR-0310).
//!
//! Register, list, enable, disable and remove providers, and register the
//! models they serve. A provider carries a **reference** to its credential in
//! the Secrets Authority; no route here accepts or returns a credential value.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use ocinye_contracts::AiCapability;
use ocinye_contracts::Classification;
use ocinye_core::modules::intelligence::providers::{
    self, AiProvider, NewProvider, NewProviderModel,
};
use ocinye_core::modules::intelligence::routing::{self, AiPolicy};
use ocinye_core::CoreError;
use serde::Deserialize;
use uuid::Uuid;

use crate::error::ApiError;
use crate::extract::{CurrentPrincipal, Ids};
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/ai/providers", get(list).post(create))
        .route("/ai/providers/{provider_id}", axum::routing::delete(remove))
        .route("/ai/providers/{provider_id}/enabled", put(set_enabled))
        .route("/ai/providers/{provider_id}/models", post(register_model))
        .route("/ai/policy", get(get_policy).put(set_policy))
        .route("/ai/routing/{capability}", put(set_routing))
}

#[derive(Deserialize)]
struct CreateBody {
    kind: String,
    label: String,
    endpoint_url: String,
    residency: String,
    #[serde(default)]
    secret_id: Option<Uuid>,
}

#[derive(Deserialize)]
struct EnabledBody {
    enabled: bool,
}

#[derive(Deserialize)]
struct ModelBody {
    model_name: String,
    #[serde(default)]
    version: Option<String>,
    capabilities: Vec<String>,
    #[serde(default)]
    context_limit: Option<i32>,
    #[serde(default)]
    max_classification: Option<String>,
}

async fn list(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
) -> Result<Json<Vec<AiProvider>>, ApiError> {
    providers::list_providers(&state.pool, &principal)
        .await
        .map(Json)
        .map_err(|error| ApiError::new(error, &ids))
}

async fn create(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
    Json(body): Json<CreateBody>,
) -> Result<Json<AiProvider>, ApiError> {
    providers::create_provider(
        &state.pool,
        &principal,
        NewProvider {
            kind: body.kind,
            label: body.label,
            endpoint_url: body.endpoint_url,
            residency: body.residency,
            secret_id: body.secret_id,
        },
        &ids,
    )
    .await
    .map(Json)
    .map_err(|error| ApiError::new(error, &ids))
}

async fn set_enabled(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(provider_id): Path<Uuid>,
    Json(body): Json<EnabledBody>,
) -> Result<Json<AiProvider>, ApiError> {
    providers::set_provider_enabled(&state.pool, &principal, provider_id, body.enabled, &ids)
        .await
        .map(Json)
        .map_err(|error| ApiError::new(error, &ids))
}

async fn remove(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(provider_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    providers::delete_provider(&state.pool, &principal, provider_id, &ids)
        .await
        .map(|()| StatusCode::NO_CONTENT)
        .map_err(|error| ApiError::new(error, &ids))
}

async fn register_model(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(provider_id): Path<Uuid>,
    Json(body): Json<ModelBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let capabilities = body
        .capabilities
        .iter()
        .map(|value| AiCapability::parse(value))
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| {
            ApiError::new(
                CoreError::Validation(
                    "Capacidade desconhecida: GENERAL, CODING, REASONING ou EMBEDDING.".to_owned(),
                ),
                &ids,
            )
        })?;
    let id = providers::register_provider_model(
        &state.pool,
        &principal,
        provider_id,
        NewProviderModel {
            model_name: body.model_name,
            version: body.version,
            capabilities,
            context_limit: body.context_limit,
            max_classification: body.max_classification,
        },
        &ids,
    )
    .await
    .map_err(|error| ApiError::new(error, &ids))?;
    Ok(Json(serde_json::json!({ "id": id })))
}

#[derive(Deserialize)]
struct PolicyBody {
    /// `NONE`, `PUBLIC`, `INTERNAL`, `CONFIDENTIAL` or `RESTRICTED`.
    external_max_classification: String,
}

#[derive(Deserialize)]
struct RoutingBody {
    #[serde(default)]
    preferred_provider_id: Option<Uuid>,
    #[serde(default = "sim")]
    allow_fallback: bool,
}

const fn sim() -> bool {
    true
}

async fn get_policy(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
) -> Result<Json<AiPolicy>, ApiError> {
    routing::get_policy(&state.pool, &principal, &state.config.ai)
        .await
        .map(Json)
        .map_err(|error| ApiError::new(error, &ids))
}

async fn set_policy(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
    Json(body): Json<PolicyBody>,
) -> Result<Json<AiPolicy>, ApiError> {
    let ceiling = match body.external_max_classification.as_str() {
        "NONE" => None,
        value => Some(Classification::parse(value).ok_or_else(|| {
            ApiError::new(
                CoreError::Validation(
                    "Tecto desconhecido: NONE, PUBLIC, INTERNAL, CONFIDENTIAL ou RESTRICTED."
                        .to_owned(),
                ),
                &ids,
            )
        })?),
    };
    routing::set_external_ceiling(&state.pool, &principal, ceiling, &ids)
        .await
        .map_err(|error| ApiError::new(error, &ids))?;
    get_policy(State(state), Ids(ids), CurrentPrincipal(principal)).await
}

async fn set_routing(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(capability): Path<String>,
    Json(body): Json<RoutingBody>,
) -> Result<Json<AiPolicy>, ApiError> {
    let capability = AiCapability::parse(&capability.to_uppercase()).ok_or_else(|| {
        ApiError::new(
            CoreError::Validation("Capacidade de IA desconhecida.".to_owned()),
            &ids,
        )
    })?;
    routing::set_preference(
        &state.pool,
        &principal,
        capability,
        body.preferred_provider_id,
        body.allow_fallback,
        &ids,
    )
    .await
    .map_err(|error| ApiError::new(error, &ids))?;
    get_policy(State(state), Ids(ids), CurrentPrincipal(principal)).await
}
