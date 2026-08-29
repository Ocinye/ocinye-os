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
        // O corpo dos ficheiros, num caminho próprio.
        //
        // Não é `/search` com um parâmetro: um resultado de corpo transporta
        // uma coisa que os outros não têm — onde no documento a frase está — e
        // fundi-los num único ranking obrigaria a interface a escolher qual das
        // duas afirmações mostrar.
        .route("/search/bodies", get(run_body_search))
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

#[derive(Serialize)]
struct BodyHitView {
    file_id: Uuid,
    file_version_id: Uuid,
    /// O número da versão, para se poder dizer «v2» a uma pessoa.
    sequence: i32,
    name: String,
    /// O trecho, com os termos realçados.
    excerpt: String,
    /// Onde está: `{"page": 4}` para PDF.
    locator: serde_json::Value,
    classification: String,
    workspace_id: Uuid,
}

/// Pesquisa o corpo dos ficheiros institucionais.
///
/// Lexical, e sem modelo nenhum. A visibilidade decide-se contra o estado
/// corrente do ficheiro e do ambiente — o índice descobre candidatos e não
/// autoriza coisa nenhuma.
async fn run_body_search(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Query(query): Query<SearchQuery>,
) -> Result<Json<Page<BodyHitView>>, ApiError> {
    let page = PageRequest {
        page: query.page.unwrap_or(1),
        page_size: query
            .page_size
            .unwrap_or(ocinye_contracts::page::DEFAULT_PAGE_SIZE),
    };

    // Híbrida quando há provider, lexical quando não há. É a mesma pesquisa:
    // sem embeddings devolve exactamente o que sempre devolveu.
    let (hits, total) = search::search_hybrid(
        &state.pool,
        &principal,
        &query.q,
        query.workspace_id,
        page,
        state.embeddings.as_deref(),
    )
    .await?;

    Ok(Json(Page::new(
        hits.into_iter()
            .map(|hit| BodyHitView {
                file_id: hit.file_id,
                file_version_id: hit.file_version_id,
                sequence: hit.sequence,
                name: hit.name,
                excerpt: hit.excerpt,
                locator: hit.locator,
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
        search::semantic_availability(&state.pool, &principal, state.embeddings.as_deref()).await?,
    ))
}
