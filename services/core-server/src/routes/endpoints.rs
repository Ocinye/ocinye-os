//! D010 · Pontos de acesso (ADR-0020).
//!
//! Administração (`organisation.view` para ler, `organisation.manage` para
//! mudar) e a resolução pública de um anfitrião, de que o Workspace precisa
//! antes de haver sessão. A resolução devolve só o destino e o nome da
//! Instância que a página de entrada mostra — nunca membros, nem as
//! Distribuições a que alguém tem acesso. Desconhecido → 404, igual para
//! qualquer nome (falha fechada).

use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use ocinye_contracts::Distribution;
use ocinye_core::modules::organisation::endpoints;
use ocinye_core::CoreError;
use serde::Deserialize;
use uuid::Uuid;

use crate::error::ApiError;
use crate::extract::{Authorised, CurrentPrincipal, Ids, NeedsOrganisationManage};
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/access/resolve", get(resolve))
        .route("/access-endpoints", get(list).post(create))
        .route("/access-endpoints/{id}/activate", post(activate))
        .route("/access-endpoints/{id}/binding", post(bind))
        .route("/access-endpoints/{id}/disable", post(disable))
        .route("/access-endpoints/{id}/observe", post(observe))
}

#[derive(Deserialize)]
struct HostQuery {
    host: String,
}

/// Pública: o anfitrião deste pedido é um ponto de acesso desta Instância?
async fn resolve(
    State(state): State<AppState>,
    Ids(ids): Ids,
    Query(query): Query<HostQuery>,
) -> Result<Json<endpoints::Resolved>, ApiError> {
    endpoints::resolve(&state.pool, state.organisation_id, &query.host)
        .await
        .map_err(|error| ApiError::new(error, &ids))?
        .map(Json)
        .ok_or_else(|| {
            ApiError::new(
                CoreError::NotFound("Endereço não configurado.".to_owned()),
                &ids,
            )
        })
}

async fn list(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
) -> Result<Json<Vec<endpoints::Endpoint>>, ApiError> {
    endpoints::list(&state.pool, &principal)
        .await
        .map(Json)
        .map_err(|error| ApiError::new(error, &ids))
}

fn binding(
    raw: Option<&str>,
    ids: &ocinye_observability::CorrelationIds,
) -> Result<Option<Distribution>, ApiError> {
    match raw
        .map(str::trim)
        .filter(|v| !v.is_empty() && *v != "generic")
    {
        None => Ok(None),
        Some(v) => v
            .parse()
            .map(Some)
            .map_err(|error: ocinye_contracts::UnknownDistribution| {
                ApiError::new(CoreError::Validation(error.to_string()), ids)
            }),
    }
}

#[derive(Deserialize)]
struct NewEndpoint {
    hostname: String,
    /// `null`/`"generic"` ou uma das quatro.
    binding: Option<String>,
}

async fn create(
    State(state): State<AppState>,
    Ids(ids): Ids,
    Authorised { principal, .. }: Authorised<NeedsOrganisationManage>,
    Json(body): Json<NewEndpoint>,
) -> Result<Json<endpoints::Endpoint>, ApiError> {
    let b = binding(body.binding.as_deref(), &ids)?;
    endpoints::create(&state.pool, &principal, &body.hostname, b, &ids)
        .await
        .map(Json)
        .map_err(|error| ApiError::new(error, &ids))
}

async fn activate(
    State(state): State<AppState>,
    Ids(ids): Ids,
    Authorised { principal, .. }: Authorised<NeedsOrganisationManage>,
    Path(id): Path<Uuid>,
) -> Result<Json<endpoints::Endpoint>, ApiError> {
    endpoints::activate(&state.pool, &principal, id, &ids)
        .await
        .map(Json)
        .map_err(|error| ApiError::new(error, &ids))
}

#[derive(Deserialize)]
struct Binding {
    binding: Option<String>,
}

async fn bind(
    State(state): State<AppState>,
    Ids(ids): Ids,
    Authorised { principal, .. }: Authorised<NeedsOrganisationManage>,
    Path(id): Path<Uuid>,
    Json(body): Json<Binding>,
) -> Result<Json<endpoints::Endpoint>, ApiError> {
    let b = binding(body.binding.as_deref(), &ids)?;
    endpoints::bind(&state.pool, &principal, id, b, &ids)
        .await
        .map(Json)
        .map_err(|error| ApiError::new(error, &ids))
}

async fn disable(
    State(state): State<AppState>,
    Ids(ids): Ids,
    Authorised { principal, .. }: Authorised<NeedsOrganisationManage>,
    Path(id): Path<Uuid>,
) -> Result<Json<endpoints::Endpoint>, ApiError> {
    endpoints::disable(&state.pool, &principal, id, &ids)
        .await
        .map(Json)
        .map_err(|error| ApiError::new(error, &ids))
}

async fn observe(
    State(state): State<AppState>,
    Ids(ids): Ids,
    Authorised { principal, .. }: Authorised<NeedsOrganisationManage>,
    Path(id): Path<Uuid>,
) -> Result<Json<endpoints::Endpoint>, ApiError> {
    endpoints::observe(&state.pool, &principal, id)
        .await
        .map(Json)
        .map_err(|error| ApiError::new(error, &ids))
}
