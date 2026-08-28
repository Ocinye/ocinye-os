//! HTTP routes, grouped by institutional domain.
//!
//! Routes mirror the modules of [`ocinye_core`], so the API reads as the domain
//! reads rather than as a set of tables.

mod administration;
mod agentic;
mod auth;
mod calendar;
mod collaboration;
mod compute;
mod governance;
mod health;
mod identity;
mod intelligence;
mod knowledge;
mod mail;
mod messaging;
mod organisation;
pub mod realtime;
mod research;
mod science;
mod search;
mod system;

use axum::extract::DefaultBodyLimit;
use axum::http::{header, HeaderValue, Method};
use axum::Router;
use ocinye_contracts::API_VERSION;
use tower_http::cors::CorsLayer;

use crate::state::AppState;

/// Largest body accepted on an ordinary endpoint.
///
/// # Why this is small, and why it is the default
///
/// Every JSON extractor buffers the whole body before a handler sees it, and
/// the ones on `/auth/login` run before there is any session to refuse. A
/// single limit sized for uploads would therefore let an unauthenticated caller
/// make the Core hold hundreds of megabytes per connection, which is a
/// denial-of-service that costs the attacker nothing (`CLAUDE.md` §40, §62).
///
/// A megabyte is far above any request the API actually takes: the largest is a
/// plan step, bounded at sixteen kilobytes by the planner.
const BODY_LIMIT_BYTES: usize = 1024 * 1024;

/// Largest body accepted on the routes that carry a file.
///
/// Above [`crate::state::AppState::store`]'s own upload ceiling
/// (`OCINYE_STORAGE_MAX_UPLOAD_BYTES`, 512 MiB by default) so the service can
/// answer with a clear validation error rather than dropping the connection.
///
/// Applied **per route**, and only to the three that take a `multipart` body.
/// Adding a fourth is a deliberate act, which is the point.
pub(crate) const UPLOAD_BODY_LIMIT_BYTES: usize = 640 * 1024 * 1024;

/// Build the application router.
pub fn router(state: AppState) -> Router {
    let api = Router::new()
        .merge(administration::routes())
        .merge(agentic::routes())
        .merge(auth::routes())
        .merge(calendar::routes())
        .merge(identity::routes())
        .merge(organisation::routes())
        .merge(research::routes())
        .merge(knowledge::routes())
        .merge(science::routes())
        .merge(collaboration::routes())
        .merge(mail::routes())
        .merge(messaging::routes())
        .merge(realtime::routes())
        .merge(search::routes())
        .merge(intelligence::routes())
        .merge(compute::routes())
        .merge(governance::routes())
        .merge(system::routes());

    Router::new()
        .merge(health::routes())
        .nest(&format!("/api/{API_VERSION}"), api)
        // Sem isto, um caminho desconhecido devolvia um 404 de corpo vazio.
        // Um cliente da API merece o mesmo envelope de erro que qualquer outra
        // recusa, ou tem de adivinhar o que aconteceu (briefing §46).
        .fallback(unknown_route)
        .layer(DefaultBodyLimit::max(BODY_LIMIT_BYTES))
        .layer(cors(&state))
        // Depois de `apply`, para que a recusa já carregue os identificadores
        // de correlação — e antes de qualquer handler, porque uma escrita de
        // outra origem não deve chegar a ser encaminhada.
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            crate::middleware::same_origin_writes,
        ))
        .layer(axum::middleware::from_fn(crate::middleware::apply))
        .with_state(state)
}

/// CORS policy.
///
/// Only origins named in configuration are allowed, and only with credentials
/// they were granted. An empty configuration means no browser origin at all,
/// which is the correct default for a kernel that is normally reached by the
/// Workspace server rather than directly by a browser.
fn cors(state: &AppState) -> CorsLayer {
    let origins: Vec<HeaderValue> = state
        .config
        .cors_allowed_origins
        .iter()
        .filter_map(|origin| origin.parse().ok())
        .collect();

    if origins.is_empty() {
        return CorsLayer::new();
    }

    CorsLayer::new()
        .allow_origin(origins)
        .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE])
        .allow_credentials(true)
}

/// Caminho que o Core não serve.
///
/// Devolve o envelope de erro habitual, e não um corpo vazio: quem chama a API
/// distingue assim «este endpoint não existe» de «o serviço não respondeu».
async fn unknown_route(crate::extract::Ids(ids): crate::extract::Ids) -> crate::error::ApiError {
    crate::error::ApiError::new(
        ocinye_core::CoreError::NotFound("Este endpoint não existe.".to_owned()),
        &ids,
    )
}

// O corpo aceite por omissão é pequeno, e o grande é excepção por rota.
//
// # Porque isto é verificado, e porque em tempo de compilação
//
// O limite era único e valia 640 MiB em toda a API, incluindo em
// `POST /auth/login`, que corre **antes** de existir sessão para recusar. Um
// cliente não autenticado podia obrigar o Core a reter centenas de megabytes
// por ligação.
//
// Um teste apanharia o regresso; uma asserção constante impede-o de compilar,
// que é melhor sítio para uma relação entre dois números que ninguém deve
// alterar por distracção.
const _: () = assert!(
    BODY_LIMIT_BYTES <= 4 * 1024 * 1024,
    "o limite de corpo por omissão deixou de ser pequeno"
);
const _: () = assert!(
    UPLOAD_BODY_LIMIT_BYTES > BODY_LIMIT_BYTES,
    "as rotas de upload deixaram de ter um limite próprio"
);
