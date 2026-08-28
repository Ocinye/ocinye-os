//! Search routes.

use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use ocinye_contracts::{Page, PageRequest};
use ocinye_core::modules::search;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ApiError;
use crate::extract::CurrentPrincipal;
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/search", get(run_search))
        .route("/search/semantic-availability", get(semantic_availability))
}

#[derive(Deserialize)]
struct SearchQuery {
    q: String,
    #[serde(default)]
    entity_types: Option<String>,
    #[serde(default)]
    workspace_id: Option<Uuid>,
    #[serde(default)]
    page: Option<u32>,
    #[serde(default)]
    page_size: Option<u32>,
}

#[derive(Serialize)]
struct HitView {
    entity_type: String,
    entity_id: Uuid,
    title: String,
    excerpt: Option<String>,
    /// Shown alongside every result, so a member always knows what they are
    /// looking at (briefing §106).
    classification: String,
    workspace_id: Option<Uuid>,
}

/// Run a permission-aware search.
///
/// The authorization predicate is part of the query, so the total reported here
/// counts only what the caller may see.
async fn run_search(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Query(query): Query<SearchQuery>,
) -> Result<Json<Page<HitView>>, ApiError> {
    let page = PageRequest {
        page: query.page.unwrap_or(1),
        page_size: query
            .page_size
            .unwrap_or(ocinye_contracts::page::DEFAULT_PAGE_SIZE),
    };

    let entity_types = query.entity_types.map(|raw| {
        raw.split(',')
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .map(ToOwned::to_owned)
            .collect()
    });

    let (hits, total) = search::search(
        &state.pool,
        &principal,
        &query.q,
        entity_types,
        query.workspace_id,
        page,
    )
    .await?;

    Ok(Json(Page::new(
        hits.into_iter()
            .map(|hit| HitView {
                entity_type: hit.entity_type,
                entity_id: hit.entity_id,
                title: hit.title,
                excerpt: hit.excerpt,
                classification: hit.classification,
                workspace_id: hit.workspace_id,
            })
            .collect(),
        page,
        total,
    )))
}

/// Report whether semantic search can be offered.
///
/// With no Ocinye AI node there are no embeddings, and this says so rather than
/// quietly returning lexical results labelled as semantic.
async fn semantic_availability(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
) -> Result<Json<search::SemanticAvailability>, ApiError> {
    Ok(Json(
        search::semantic_availability(&state.pool, &principal).await?,
    ))
}
