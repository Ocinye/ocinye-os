//! A configuração e a marca da Instância (ADR-0017).
//!
//! `GET/PUT /instance/settings` e `PUT /instance/logo` são da administração da
//! plataforma. `GET /instance/branding` e `GET /instance/logo` são públicos, e
//! são as únicas rotas públicas que dizem alguma coisa desta Instância: a página
//! de entrada precisa do nome e da marca **antes** de haver sessão. Dizem o que
//! está na porta de qualquer instituição — o nome, a língua, o logótipo — e nada
//! do que está lá dentro.

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use ocinye_core::modules::organisation::settings::{
    self, InstanceBranding, InstanceSettings, SettingsChange,
};

use crate::error::ApiError;
use crate::extract::{CurrentPrincipal, Ids};
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/instance/settings", get(get_settings).put(put_settings))
        .route("/instance/logo", get(get_logo).put(put_logo))
        .route("/instance/branding", get(get_branding))
}

async fn get_settings(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
) -> Result<Json<InstanceSettings>, ApiError> {
    settings::get_settings(&state.pool, &principal)
        .await
        .map(Json)
        .map_err(|error| ApiError::new(error, &ids))
}

async fn put_settings(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
    Json(change): Json<SettingsChange>,
) -> Result<Json<InstanceSettings>, ApiError> {
    settings::set_settings(&state.pool, &principal, change, &ids)
        .await
        .map(Json)
        .map_err(|error| ApiError::new(error, &ids))
}

/// `PUT /instance/logo` — o corpo são os bytes da imagem. O tipo decide-se pelos
/// bytes, e não pelo `Content-Type` que o cliente declara.
async fn put_logo(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
    corpo: Bytes,
) -> Result<Json<InstanceSettings>, ApiError> {
    let store = state.store().map_err(|error| ApiError::new(error, &ids))?;
    settings::set_logo(&state.pool, store, &principal, corpo.to_vec(), &ids)
        .await
        .map(Json)
        .map_err(|error| ApiError::new(error, &ids))
}

/// `GET /instance/branding` — público.
async fn get_branding(
    State(state): State<AppState>,
    Ids(ids): Ids,
) -> Result<Json<InstanceBranding>, ApiError> {
    settings::branding(&state.pool, state.organisation_id)
        .await
        .map(Json)
        .map_err(|error| ApiError::new(error, &ids))
}

/// `GET /instance/logo` — público; `404` quando não há.
async fn get_logo(State(state): State<AppState>, Ids(ids): Ids) -> Result<Response, ApiError> {
    let Ok(store) = state.store() else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    match settings::logo(&state.pool, store, state.organisation_id)
        .await
        .map_err(|error| ApiError::new(error, &ids))?
    {
        Some((tipo, bytes)) => Ok((
            [
                (
                    header::CONTENT_TYPE,
                    HeaderValue::from_str(&tipo)
                        .unwrap_or(HeaderValue::from_static("application/octet-stream")),
                ),
                (
                    header::X_CONTENT_TYPE_OPTIONS,
                    HeaderValue::from_static("nosniff"),
                ),
                (
                    header::CACHE_CONTROL,
                    HeaderValue::from_static("public, max-age=300"),
                ),
            ],
            bytes,
        )
            .into_response()),
        None => Ok(StatusCode::NOT_FOUND.into_response()),
    }
}
