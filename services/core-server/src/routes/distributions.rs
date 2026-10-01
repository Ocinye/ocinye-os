//! D010 · As Distribuições da Instância e a entrada do membro (ADR-0019).
//!
//! Entrada (o membro, para si): que Distribuições pode abrir, e entrar numa —
//! o Core decide e grava a escolha na sessão. Administração
//! (`organisation.manage`): activar, desactivar, e quem tem acesso a quê.
//! Nada disto é um papel: dentro da Distribuição, o RBAC continua a decidir.

use axum::extract::{Path, State};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use ocinye_contracts::Distribution;
use ocinye_core::modules::organisation::{contexts as ctx, distributions};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ApiError;
use crate::extract::{Authorised, CurrentPrincipal, CurrentSession, Ids, NeedsOrganisationManage};
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/me/distributions", get(mine))
        .route("/me/distribution", post(enter))
        .route("/me/contexts", get(contexts))
        .route("/me/context", post(choose_context))
        .route("/instance/distributions", get(list))
        .route(
            "/instance/distributions/{distribution}/enable",
            post(enable),
        )
        .route(
            "/instance/distributions/{distribution}/disable",
            post(disable),
        )
        .route("/instance/distribution-access", get(matrix))
        .route(
            "/instance/distribution-access/{person_id}/{distribution}",
            put(grant).delete(revoke),
        )
}

fn distribution(
    raw: &str,
    ids: &ocinye_observability::CorrelationIds,
) -> Result<Distribution, ApiError> {
    raw.parse()
        .map_err(|error: ocinye_contracts::UnknownDistribution| {
            ApiError::new(ocinye_core::CoreError::NotFound(error.to_string()), ids)
        })
}

/// As Distribuições do membro e a decisão de entrada (0 / 1 / várias).
async fn mine(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentSession(principal, scope): CurrentSession,
) -> Result<Json<distributions::MyDistributions>, ApiError> {
    distributions::mine(&state.pool, &principal, scope.active_distribution)
        .await
        .map(Json)
        .map_err(|error| ApiError::new(error, &ids))
}

#[derive(Deserialize)]
struct EnterBody {
    distribution: String,
}

#[derive(Serialize)]
struct Entered {
    active: Distribution,
}

/// Entrar numa Distribuição (ou mudar para outra) nesta sessão.
async fn enter(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentSession(principal, scope): CurrentSession,
    Json(body): Json<EnterBody>,
) -> Result<Json<Entered>, ApiError> {
    let d = distribution(&body.distribution, &ids)?;
    distributions::enter(&state.pool, &principal, scope.session_id, d, &ids)
        .await
        .map(|()| Json(Entered { active: d }))
        .map_err(|error| ApiError::new(error, &ids))
}

/// As quatro e o estado de cada uma (S26).
async fn list(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
) -> Result<Json<Vec<distributions::InstanceDistribution>>, ApiError> {
    distributions::instance_distributions(&state.pool, &principal)
        .await
        .map(Json)
        .map_err(|error| ApiError::new(error, &ids))
}

async fn enable(
    State(state): State<AppState>,
    Ids(ids): Ids,
    Authorised { principal, .. }: Authorised<NeedsOrganisationManage>,
    Path(raw): Path<String>,
) -> Result<Json<Vec<distributions::InstanceDistribution>>, ApiError> {
    let d = distribution(&raw, &ids)?;
    distributions::enable(&state.pool, &principal, d, &ids)
        .await
        .map_err(|error| ApiError::new(error, &ids))?;
    list(State(state), Ids(ids), CurrentPrincipal(principal)).await
}

async fn disable(
    State(state): State<AppState>,
    Ids(ids): Ids,
    Authorised { principal, .. }: Authorised<NeedsOrganisationManage>,
    Path(raw): Path<String>,
) -> Result<Json<Vec<distributions::InstanceDistribution>>, ApiError> {
    let d = distribution(&raw, &ids)?;
    distributions::disable(&state.pool, &principal, d, &ids)
        .await
        .map_err(|error| ApiError::new(error, &ids))?;
    list(State(state), Ids(ids), CurrentPrincipal(principal)).await
}

/// Quem pode abrir cada Distribuição activada (S37).
async fn matrix(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
) -> Result<Json<Vec<distributions::MemberAccess>>, ApiError> {
    distributions::access_matrix(&state.pool, &principal)
        .await
        .map(Json)
        .map_err(|error| ApiError::new(error, &ids))
}

async fn grant(
    State(state): State<AppState>,
    Ids(ids): Ids,
    Authorised { principal, .. }: Authorised<NeedsOrganisationManage>,
    Path((person_id, raw)): Path<(Uuid, String)>,
) -> Result<axum::http::StatusCode, ApiError> {
    let d = distribution(&raw, &ids)?;
    distributions::grant(&state.pool, &principal, person_id, d, &ids)
        .await
        .map(|()| axum::http::StatusCode::NO_CONTENT)
        .map_err(|error| ApiError::new(error, &ids))
}

async fn revoke(
    State(state): State<AppState>,
    Ids(ids): Ids,
    Authorised { principal, .. }: Authorised<NeedsOrganisationManage>,
    Path((person_id, raw)): Path<(Uuid, String)>,
) -> Result<axum::http::StatusCode, ApiError> {
    let d = distribution(&raw, &ids)?;
    distributions::revoke(&state.pool, &principal, person_id, d, &ids)
        .await
        .map(|()| axum::http::StatusCode::NO_CONTENT)
        .map_err(|error| ApiError::new(error, &ids))
}

/// Os contextos do membro e o activo desta sessão (S19–S21).
async fn contexts(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentSession(principal, scope): CurrentSession,
) -> Result<Json<ctx::MyContexts>, ApiError> {
    ctx::mine(
        &state.pool,
        &principal,
        scope.session_id,
        scope.active_context.as_ref(),
    )
    .await
    .map(Json)
    .map_err(|error| ApiError::new(error, &ids))
}

#[derive(Deserialize)]
struct ContextBody {
    /// `organisation` · `unit` · `project` · `personal`, ou `null` para repor.
    kind: Option<String>,
    id: Option<Uuid>,
}

/// Escolher (ou repor) o contexto activo. Não muda a Distribuição.
async fn choose_context(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentSession(principal, scope): CurrentSession,
    Json(body): Json<ContextBody>,
) -> Result<Json<Option<ctx::Context>>, ApiError> {
    ctx::choose(
        &state.pool,
        &principal,
        scope.session_id,
        scope.active_distribution,
        body.kind.as_deref(),
        body.id,
        &ids,
    )
    .await
    .map(Json)
    .map_err(|error| ApiError::new(error, &ids))
}
