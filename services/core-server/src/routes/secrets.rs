//! The Secrets Authority's routes (ADR-0110).
//!
//! Create, list, rotate and revoke. **There is no route that returns a secret's
//! value**, and there must never be one: the only way a value leaves the seal
//! is a Core service using it (`secrets::use_secret`), in its scope.

use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use ocinye_core::modules::secrets::{self, NewSecret, SecretScope, SecretSummary};
use ocinye_core::password::Secret;
use ocinye_core::CoreError;
use serde::Deserialize;
use uuid::Uuid;

use crate::error::ApiError;
use crate::extract::{CurrentPrincipal, Ids};
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/instance/secrets", get(list).post(create))
        .route("/instance/secrets/{secret_id}/rotate", post(rotate))
        .route("/instance/secrets/{secret_id}/revoke", post(revoke))
}

#[derive(Deserialize)]
struct CreateBody {
    kind: String,
    label: String,
    scope: String,
    /// Wrapped at once; never logged, never echoed.
    value: String,
}

#[derive(Deserialize)]
struct RotateBody {
    value: String,
}

async fn list(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
) -> Result<Json<Vec<SecretSummary>>, ApiError> {
    secrets::list_secrets(&state.pool, &principal)
        .await
        .map(Json)
        .map_err(|error| ApiError::new(error, &ids))
}

async fn create(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
    Json(body): Json<CreateBody>,
) -> Result<Json<SecretSummary>, ApiError> {
    let value = Secret::new(body.value);
    let scope = SecretScope::parse(&body.scope).ok_or_else(|| {
        ApiError::new(
            CoreError::Validation("Âmbito desconhecido: ai_gateway, mail ou connector.".to_owned()),
            &ids,
        )
    })?;
    secrets::create_secret(
        &state.pool,
        state.config.sealing_key.as_ref(),
        &principal,
        NewSecret {
            kind: body.kind,
            label: body.label,
            scope,
            value,
        },
        &ids,
    )
    .await
    .map(Json)
    .map_err(|error| ApiError::new(error, &ids))
}

async fn rotate(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(secret_id): Path<Uuid>,
    Json(body): Json<RotateBody>,
) -> Result<Json<SecretSummary>, ApiError> {
    let value = Secret::new(body.value);
    secrets::rotate_secret(
        &state.pool,
        state.config.sealing_key.as_ref(),
        &principal,
        secret_id,
        value,
        &ids,
    )
    .await
    .map(Json)
    .map_err(|error| ApiError::new(error, &ids))
}

async fn revoke(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(secret_id): Path<Uuid>,
) -> Result<Json<SecretSummary>, ApiError> {
    secrets::revoke_secret(&state.pool, &principal, secret_id, &ids)
        .await
        .map(Json)
        .map_err(|error| ApiError::new(error, &ids))
}
