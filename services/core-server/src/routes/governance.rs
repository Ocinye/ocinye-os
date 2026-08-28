//! Governance routes: reading the audit trail.

use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use ocinye_contracts::{Page, PageRequest};
use ocinye_core::modules::governance;
use serde::Deserialize;
use uuid::Uuid;

use crate::error::ApiError;
use crate::extract::CurrentPrincipal;
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new().route("/audit", get(list_audit))
}

#[derive(Deserialize)]
struct AuditQueryParams {
    #[serde(default)]
    resource_type: Option<String>,
    #[serde(default)]
    resource_id: Option<Uuid>,
    #[serde(default)]
    actor_person_id: Option<Uuid>,
    #[serde(default)]
    since: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    page: Option<u32>,
    #[serde(default)]
    page_size: Option<u32>,
}

/// Read the audit trail.
///
/// Requires the `auditor` role or an administrative role, and grants nothing
/// else: an auditor sees *that* something happened without gaining access to
/// the institutional content it happened to.
async fn list_audit(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Query(params): Query<AuditQueryParams>,
) -> Result<Json<Page<governance::AuditRecord>>, ApiError> {
    let page = PageRequest {
        page: params.page.unwrap_or(1),
        page_size: params
            .page_size
            .unwrap_or(ocinye_contracts::page::DEFAULT_PAGE_SIZE),
    };

    let (records, total) = governance::list_audit(
        &state.pool,
        &principal,
        governance::AuditQuery {
            resource_type: params.resource_type,
            resource_id: params.resource_id,
            actor_person_id: params.actor_person_id,
            since: params.since,
        },
        page,
    )
    .await?;

    Ok(Json(Page::new(records, page, total)))
}
