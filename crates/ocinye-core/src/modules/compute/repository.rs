//! Compute persistence.

use chrono::{DateTime, Utc};
use ocinye_contracts::ComputeNodeStatus;
use serde_json::Value;
use sqlx::PgExecutor;
use uuid::Uuid;

use super::model::ComputeNode;
use crate::error::CoreResult;

const NODE_COLUMNS: &str = "id, identifier, display_name, kind, location_label, status,
                            cpu_cores, memory_bytes, storage_bytes, gpus, capabilities,
                            agent_version, last_seen_at, created_at";

/// Insert a node in `pending_enrollment`.
///
/// # Errors
///
/// Returns an error when the insert fails.
pub async fn insert_node<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    identifier: &str,
    display_name: &str,
    kind: &str,
    location_label: Option<&str>,
    created_by: Uuid,
) -> CoreResult<ComputeNode> {
    let node = sqlx::query_as::<_, ComputeNode>(&format!(
        "INSERT INTO compute_nodes
             (organisation_id, identifier, display_name, kind, location_label,
              status, created_by_id)
         VALUES ($1, $2, $3, $4, $5, 'pending_enrollment', $6)
         RETURNING {NODE_COLUMNS}"
    ))
    .bind(organisation_id)
    .bind(identifier)
    .bind(display_name)
    .bind(kind)
    .bind(location_label)
    .bind(created_by)
    .fetch_one(executor)
    .await?;
    Ok(node)
}

/// List the nodes of an organisation.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn list_nodes<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
) -> CoreResult<Vec<ComputeNode>> {
    let nodes = sqlx::query_as::<_, ComputeNode>(&format!(
        "SELECT {NODE_COLUMNS} FROM compute_nodes
          WHERE organisation_id = $1 AND status <> 'retired'
          ORDER BY identifier"
    ))
    .bind(organisation_id)
    .fetch_all(executor)
    .await?;
    Ok(nodes)
}

/// Store an enrollment or agent credential digest.
///
/// # Errors
///
/// Returns an error when the insert fails.
pub async fn insert_credential<'e>(
    executor: impl PgExecutor<'e>,
    node_id: Uuid,
    purpose: &str,
    token_digest: &str,
    expires_at: Option<DateTime<Utc>>,
    created_by: Option<Uuid>,
) -> CoreResult<()> {
    sqlx::query(
        "INSERT INTO node_credentials (node_id, purpose, token_digest, expires_at, created_by_id)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(node_id)
    .bind(purpose)
    .bind(token_digest)
    .bind(expires_at)
    .bind(created_by)
    .execute(executor)
    .await?;
    Ok(())
}

/// Consume a single-use enrollment credential, returning its node.
///
/// The update is the check: `consumed_at IS NULL` in the `WHERE` clause makes
/// a concurrent second use fail rather than race.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn consume_enrollment<'e>(
    executor: impl PgExecutor<'e>,
    token_digest: &str,
) -> CoreResult<Option<Uuid>> {
    let node_id = sqlx::query_scalar::<_, Uuid>(
        "UPDATE node_credentials
            SET consumed_at = now()
          WHERE token_digest = $1
            AND purpose = 'enrollment'
            AND consumed_at IS NULL
            AND revoked_at IS NULL
            AND (expires_at IS NULL OR expires_at > now())
          RETURNING node_id",
    )
    .bind(token_digest)
    .fetch_optional(executor)
    .await?;
    Ok(node_id)
}

/// Resolve a live agent credential to its node.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn node_for_agent_token<'e>(
    executor: impl PgExecutor<'e>,
    token_digest: &str,
) -> CoreResult<Option<Uuid>> {
    let node_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT node_id FROM node_credentials
          WHERE token_digest = $1
            AND purpose = 'agent'
            AND revoked_at IS NULL
            AND (expires_at IS NULL OR expires_at > now())",
    )
    .bind(token_digest)
    .fetch_optional(executor)
    .await?;
    Ok(node_id)
}

/// Record a heartbeat and the resources reported with it.
///
/// # Errors
///
/// Returns an error when the update fails.
#[expect(
    clippy::too_many_arguments,
    reason = "um parâmetro por coluna: a alternativa é uma struct que só existe para atravessar esta chamada"
)]
pub async fn record_heartbeat<'e>(
    executor: impl PgExecutor<'e>,
    node_id: Uuid,
    agent_version: &str,
    cpu_cores: i32,
    memory_bytes: i64,
    storage_bytes: i64,
    gpus: &Value,
    capabilities: &Value,
    health: &Value,
) -> CoreResult<()> {
    sqlx::query(
        "UPDATE compute_nodes
            SET status = 'online',
                agent_version = $2,
                cpu_cores = $3,
                memory_bytes = $4,
                storage_bytes = $5,
                gpus = $6,
                capabilities = $7,
                last_health = $8,
                last_seen_at = now(),
                updated_at = now()
          WHERE id = $1 AND status <> 'retired'",
    )
    .bind(node_id)
    .bind(agent_version)
    .bind(cpu_cores)
    .bind(memory_bytes)
    .bind(storage_bytes)
    .bind(gpus)
    .bind(capabilities)
    .bind(health)
    .execute(executor)
    .await?;
    Ok(())
}

/// Set a node's stored status.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn set_status<'e>(
    executor: impl PgExecutor<'e>,
    node_id: Uuid,
    status: ComputeNodeStatus,
) -> CoreResult<()> {
    sqlx::query("UPDATE compute_nodes SET status = $2, updated_at = now() WHERE id = $1")
        .bind(node_id)
        .bind(status.as_str())
        .execute(executor)
        .await?;
    Ok(())
}

/// Replace the models a node reports.
///
/// # Errors
///
/// Returns an error when the statements fail.
pub async fn replace_reported_models(
    tx: &mut crate::Tx<'_>,
    node_id: Uuid,
    node_identifier: &str,
    models: &[(String, String, Value, Option<i32>)],
) -> CoreResult<()> {
    sqlx::query("DELETE FROM ai_models WHERE node_id = $1")
        .bind(node_id)
        .execute(&mut **tx)
        .await?;

    for (name, version, capabilities, context_limit) in models {
        sqlx::query(
            "INSERT INTO ai_models
                 (provider_kind, provider_name, node_id, model_name, version,
                  capabilities, context_limit, status, reported_at)
             VALUES ('ocinye_node', $1, $2, $3, $4, $5, $6, 'available', now())
             ON CONFLICT (provider_name, model_name, version) DO UPDATE
                SET capabilities = EXCLUDED.capabilities,
                    context_limit = EXCLUDED.context_limit,
                    status = 'available',
                    reported_at = now(),
                    updated_at = now()",
        )
        .bind(node_identifier)
        .bind(node_id)
        .bind(name)
        .bind(version)
        .bind(capabilities)
        .bind(context_limit)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

/// Mark the models of nodes that have gone silent as unavailable.
///
/// Availability follows the node's liveness rather than lingering as a stale
/// `available`.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn mark_stale_models_unavailable<'e>(
    executor: impl PgExecutor<'e>,
    offline_after_seconds: i64,
) -> CoreResult<u64> {
    let result = sqlx::query(
        "UPDATE ai_models m
            SET status = 'unavailable', updated_at = now()
           FROM compute_nodes n
          WHERE m.node_id = n.id
            AND m.status = 'available'
            AND (n.last_seen_at IS NULL
                 OR n.last_seen_at < now() - make_interval(secs => $1::double precision))",
    )
    .bind(offline_after_seconds as f64)
    .execute(executor)
    .await?;
    Ok(result.rows_affected())
}
