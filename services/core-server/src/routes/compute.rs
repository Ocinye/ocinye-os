//! Compute Plane routes, including the node protocol.
//!
//! Two audiences share this module: members reading the state of the Compute
//! Plane, and node agents speaking the enrollment and heartbeat protocol. The
//! two authenticate differently — a person by OIDC, a node by its own machine
//! credential — and never share credentials (ADR-0500).

use axum::extract::State;
use axum::http::HeaderMap;
use axum::routing::{get, post};
use axum::{Json, Router};
use ocinye_contracts::{ComputeStatus, NodeKind};
use ocinye_core::modules::compute;
use ocinye_core::CoreError;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ApiError;
use crate::extract::{CurrentPrincipal, Ids};
use crate::state::AppState;

/// Header carrying a node agent's own credential.
///
/// Deliberately not `Authorization`: a machine credential is a different kind
/// of thing from a member's token, and conflating them invites reuse.
const NODE_TOKEN_HEADER: &str = "x-ocinye-node-token";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/compute/status", get(status))
        .route("/compute/nodes", get(list_nodes).post(register_node))
        .route("/compute/enroll", post(enroll))
        .route("/compute/heartbeat", post(heartbeat))
}

/// Report the state of the Compute Plane.
///
/// With no node registered this reports zero and says so. It does not invent a
/// node to make the screen look populated.
async fn status(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
) -> Result<Json<ComputeStatus>, ApiError> {
    Ok(Json(
        compute::compute_status(&state.pool, &principal, &state.config.compute).await?,
    ))
}

#[derive(Serialize)]
struct NodeView {
    id: Uuid,
    identifier: String,
    display_name: String,
    kind: String,
    location_label: Option<String>,
    /// Derived from the last heartbeat, never from a stored flag.
    status: String,
    cpu_cores: Option<i32>,
    memory_bytes: Option<i64>,
    gpus: serde_json::Value,
    capabilities: serde_json::Value,
    agent_version: Option<String>,
    last_seen_at: Option<chrono::DateTime<chrono::Utc>>,
}

async fn list_nodes(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
) -> Result<Json<Vec<NodeView>>, ApiError> {
    let nodes = compute::list_nodes(&state.pool, &principal, &state.config.compute).await?;
    Ok(Json(
        nodes
            .into_iter()
            .map(|(node, status)| NodeView {
                id: node.id,
                identifier: node.identifier,
                display_name: node.display_name,
                kind: node.kind,
                location_label: node.location_label,
                status: status.as_str().to_owned(),
                cpu_cores: node.cpu_cores,
                memory_bytes: node.memory_bytes,
                gpus: node.gpus,
                capabilities: node.capabilities,
                agent_version: node.agent_version,
                last_seen_at: node.last_seen_at,
            })
            .collect(),
    ))
}

#[derive(Deserialize)]
struct RegisterNodeRequest {
    /// Institutional identifier, supplied here and never hardcoded anywhere.
    identifier: String,
    display_name: String,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    location_label: Option<String>,
}

/// Register a node and issue a single-use enrollment token.
///
/// The token is returned once and only its digest is stored.
async fn register_node(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Json(request): Json<RegisterNodeRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let kind = request
        .kind
        .as_deref()
        .map(|raw| {
            NodeKind::parse(raw)
                .ok_or_else(|| CoreError::Validation("Unknown node kind.".to_owned()))
        })
        .transpose()?
        .unwrap_or(NodeKind::Gpu);

    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let enrolled = compute::register_node(
        &mut tx,
        &principal,
        &ids,
        &state.config.compute,
        compute::NewNode {
            identifier: request.identifier,
            display_name: request.display_name,
            kind,
            location_label: request.location_label,
        },
    )
    .await?;
    tx.commit().await.map_err(CoreError::from)?;

    Ok(Json(serde_json::json!({
        "node_id": enrolled.node.id,
        "identifier": enrolled.node.identifier,
        "enrollment_token": enrolled.enrollment_token,
    })))
}

#[derive(Deserialize)]
struct EnrollRequest {
    enrollment_token: String,
}

/// Exchange a single-use enrollment token for a long-lived agent credential.
///
/// Authenticated by the enrollment token itself: at this point the node has no
/// other identity, and it must never borrow a person's.
async fn enroll(
    State(state): State<AppState>,
    Ids(ids): Ids,
    Json(request): Json<EnrollRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let agent_token = compute::enroll_node(&mut tx, &ids, &request.enrollment_token).await?;
    tx.commit().await.map_err(CoreError::from)?;

    Ok(Json(serde_json::json!({ "agent_token": agent_token })))
}

/// Accept a heartbeat from a node agent.
///
/// Everything in the payload is untrusted input from a machine that may be
/// compromised: it is recorded and used for liveness, never for authorization.
async fn heartbeat(
    State(state): State<AppState>,
    Ids(ids): Ids,
    headers: HeaderMap,
    Json(report): Json<compute::NodeHeartbeat>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let token = headers
        .get(NODE_TOKEN_HEADER)
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| CoreError::Unauthenticated("A node credential is required.".to_owned()))?;

    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    compute::heartbeat(&mut tx, &ids, token.trim(), report).await?;
    tx.commit().await.map_err(CoreError::from)?;

    Ok(Json(serde_json::json!({ "accepted": true })))
}
