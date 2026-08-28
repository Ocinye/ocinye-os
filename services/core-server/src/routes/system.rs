//! System capability reporting.
//!
//! One endpoint, one answer: what this installation can currently do. The
//! Workspace renders availability from this rather than inferring it from three
//! different status endpoints and a guess (briefing §55).

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use ocinye_contracts::SystemCapabilities;
use ocinye_core::modules::platform;

use crate::error::ApiError;
use crate::extract::{CurrentPrincipal, Ids};
use crate::state::AppState;

/// System routes.
pub fn routes() -> Router<AppState> {
    Router::new().route("/system/capabilities", get(capabilities))
}

/// `GET /system/capabilities`
///
/// # Authentication, not authorization
///
/// Requires a session, because the shape of an installation is not public. It
/// requires no *permission*, because availability is the same fact for
/// everyone: whether a compute node exists does not depend on who is asking.
/// Gating it by permission would force the Workspace to guess availability for
/// members who cannot read the compute registry, and guessing is what this
/// endpoint exists to stop.
///
/// # What it does not expose
///
/// No hostnames, no endpoints, no model weights, no error detail. Each entry is
/// a state and a sentence safe to show a member (briefing §55).
async fn capabilities(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(_principal): CurrentPrincipal,
) -> Result<Json<SystemCapabilities>, ApiError> {
    let report = platform::system_capabilities(
        &state.pool,
        &state.config,
        state.store.is_some(),
        state.mail_registry.reachability().await,
    )
    .await
    .map_err(|error| ApiError::new(error, &ids))?;

    Ok(Json(report))
}
