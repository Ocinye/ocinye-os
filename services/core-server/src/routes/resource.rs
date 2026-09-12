//! Resource governance routes — a member's own resource picture.
//!
//! This surface answers *how much* a member may consume and how much they have
//! consumed — never *what* they may access (ADR-0108). It reads the member's own
//! entitlement and measured usage; seeing another member's picture is a separate,
//! permission-gated concern handled in administration, not here.

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use ocinye_contracts::ResourceType;
use ocinye_core::modules::resource::{self, Entitlement, PersonalStorageStatus};
use serde::Serialize;

use crate::error::ApiError;
use crate::extract::CurrentPrincipal;
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new().route("/resources/me", get(my_resources))
}

/// A member's own resource picture, for the "Meus Recursos" screen.
///
/// Storage is the only resource enforced today. The entitlement carries the
/// parts that explain the number — the profile it came from, and any temporary
/// grants that add on top and when they expire — so the member sees *why* their
/// limit is what it is, not just the total.
#[derive(Serialize)]
struct MyResources {
    /// Measured usage, the effective limit, and the derived state.
    storage: PersonalStorageStatus,
    /// The resolved storage entitlement with its explanation parts.
    storage_entitlement: Entitlement,
}

/// Report the current member's own resources.
///
/// Reads the member's own picture by their session identity; no identifier from
/// the client selects whose resources are shown.
async fn my_resources(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
) -> Result<Json<MyResources>, ApiError> {
    let storage =
        resource::personal_storage_status(&state.pool, &principal, principal.person_id).await?;
    let storage_entitlement = resource::member_entitlement(
        &state.pool,
        &principal,
        principal.person_id,
        ResourceType::PersistentStorage,
    )
    .await?;
    Ok(Json(MyResources {
        storage,
        storage_entitlement,
    }))
}
