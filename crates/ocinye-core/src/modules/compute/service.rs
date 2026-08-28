//! Compute application layer.

use chrono::{Duration, Utc};
use ocinye_contracts::{ComputeNodeStatus, ComputeStatus, NodeKind};
use ocinye_domain::identifiers::validate_node_identifier;
use ocinye_domain::policy::{authorize, Action, ResourceContext, ResourceKind};
use ocinye_domain::Principal;
use ocinye_observability::CorrelationIds;
use rand::rngs::SysRng;
use rand::TryRng;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::PgPool;

use super::model::{ComputeNode, NodeHeartbeat};
use super::repository as repo;
use crate::audit::{self, action, AuditEntry};
use crate::config::ComputeConfig;
use crate::error::{CoreError, CoreResult};
use crate::outbox::{self, event};
use crate::Tx;

const TOKEN_BYTES: usize = 32;

fn new_token() -> (String, String) {
    let mut bytes = [0_u8; TOKEN_BYTES];
    // Entropia do sistema: estes tokens são credenciais de máquina. A falha
    // é ruidosa de propósito — ver `identity::service::new_token`.
    SysRng
        .try_fill_bytes(&mut bytes)
        .expect("o sistema não deu entropia para um token de nó");
    let token = hex::encode(bytes);
    (token.clone(), digest(&token))
}

fn digest(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

/// Details of a node being registered.
#[derive(Debug, Clone)]
pub struct NewNode {
    /// Institutional identifier, for example `CAM-01`. Supplied, never assumed.
    pub identifier: String,
    /// Display name.
    pub display_name: String,
    /// Kind of node.
    pub kind: NodeKind,
    /// Human label of where it is.
    pub location_label: Option<String>,
}

/// A registered node together with its one-time enrollment token.
pub struct EnrolledNode {
    /// The node.
    pub node: ComputeNode,
    /// The enrollment token. Shown once, never stored, never logged.
    pub enrollment_token: String,
}

/// Register a node and issue a single-use enrollment token.
///
/// The node does not exist as a participant yet: it is `pending_enrollment`
/// until an agent presents this token.
///
/// # Errors
///
/// Returns an error when the caller may not administer the platform, or the
/// identifier is malformed.
pub async fn register_node(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    config: &ComputeConfig,
    request: NewNode,
) -> CoreResult<EnrolledNode> {
    let ctx = ResourceContext::organisation(ResourceKind::Platform, principal.organisation_id);
    authorize(principal, Action::Administer, &ctx)
        .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    let identifier = validate_node_identifier(&request.identifier)?;
    let display_name = request.display_name.trim();
    if display_name.is_empty() {
        return Err(CoreError::Validation(
            "A node needs a display name.".to_owned(),
        ));
    }

    let node = repo::insert_node(
        &mut **tx,
        principal.organisation_id,
        &identifier,
        display_name,
        request.kind.as_str(),
        request.location_label.as_deref(),
        principal.person_id,
    )
    .await?;

    let (token, token_digest) = new_token();
    let expires_at = Utc::now()
        + Duration::from_std(config.enrollment_token_ttl)
            .unwrap_or_else(|_| Duration::seconds(3600));

    repo::insert_credential(
        &mut **tx,
        node.id,
        "enrollment",
        &token_digest,
        Some(expires_at),
        Some(principal.person_id),
    )
    .await?;

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::ADMIN_OPERATION, "compute_node")
            .resource(node.id)
            .detail("event", "registered")
            .detail("identifier", identifier.as_str()),
    )
    .await?;

    Ok(EnrolledNode {
        node,
        enrollment_token: token,
    })
}

/// Exchange a single-use enrollment token for a long-lived agent credential.
///
/// Unauthenticated by human standards: the enrollment token is the proof. The
/// agent receives its own identity and never reuses a person's credentials
/// (ADR-0500).
///
/// # Errors
///
/// Returns [`CoreError::Unauthenticated`] when the token is not valid. Expired,
/// consumed and unknown tokens are indistinguishable.
pub async fn enroll_node(
    tx: &mut Tx<'_>,
    ids: &CorrelationIds,
    enrollment_token: &str,
) -> CoreResult<String> {
    let node_id = repo::consume_enrollment(&mut **tx, &digest(enrollment_token))
        .await?
        .ok_or_else(|| {
            CoreError::Unauthenticated("This enrollment token is not valid.".to_owned())
        })?;

    let (agent_token, agent_digest) = new_token();
    repo::insert_credential(&mut **tx, node_id, "agent", &agent_digest, None, None).await?;
    repo::set_status(&mut **tx, node_id, ComputeNodeStatus::Offline).await?;

    outbox::emit(
        tx,
        event::COMPUTE_NODE_ENROLLED,
        "compute_node",
        node_id,
        &ids.correlation_id,
        json!({}),
    )
    .await?;

    audit::record(
        tx,
        None,
        ids,
        AuditEntry::new(action::NODE_ENROLLMENT, "compute_node")
            .resource(node_id)
            .detail("event", "enrolled"),
    )
    .await?;

    Ok(agent_token)
}

/// Record a heartbeat from an agent.
///
/// Everything in the payload is untrusted input from a machine that may be
/// compromised. It is recorded for operators and used for liveness; it never
/// influences an authorization decision.
///
/// # Errors
///
/// Returns [`CoreError::Unauthenticated`] when the agent token is not valid.
pub async fn heartbeat(
    tx: &mut Tx<'_>,
    ids: &CorrelationIds,
    agent_token: &str,
    report: NodeHeartbeat,
) -> CoreResult<()> {
    let node_id = repo::node_for_agent_token(&mut **tx, &digest(agent_token))
        .await?
        .ok_or_else(|| {
            CoreError::Unauthenticated("This agent credential is not valid.".to_owned())
        })?;

    // Reported numbers are clamped into the column domain rather than trusted:
    // a hostile agent must not be able to overflow a column.
    let cpu_cores = i32::try_from(report.resources.cpu_cores).unwrap_or(i32::MAX);
    let memory_bytes = i64::try_from(report.resources.memory_bytes).unwrap_or(i64::MAX);
    let storage_bytes = i64::try_from(report.resources.storage_bytes).unwrap_or(i64::MAX);

    let gpus = serde_json::to_value(&report.resources.gpus).unwrap_or_else(|_| json!([]));
    let capabilities = serde_json::to_value(&report.capabilities).unwrap_or_else(|_| json!([]));

    repo::record_heartbeat(
        &mut **tx,
        node_id,
        &report.agent_version,
        cpu_cores,
        memory_bytes,
        storage_bytes,
        &gpus,
        &capabilities,
        &report.health,
    )
    .await?;

    let models: Vec<(String, String, Value, Option<i32>)> = report
        .models
        .iter()
        .map(|model| {
            (
                model.name.clone(),
                model
                    .version
                    .clone()
                    .unwrap_or_else(|| "unknown".to_owned()),
                serde_json::to_value(&model.capabilities).unwrap_or_else(|_| json!([])),
                model.context_limit,
            )
        })
        .collect();

    let identifier =
        sqlx::query_scalar::<_, String>("SELECT identifier FROM compute_nodes WHERE id = $1")
            .bind(node_id)
            .fetch_one(&mut **tx)
            .await?;

    repo::replace_reported_models(tx, node_id, &identifier, &models).await?;

    outbox::emit(
        tx,
        event::COMPUTE_NODE_ONLINE,
        "compute_node",
        node_id,
        &ids.correlation_id,
        json!({ "models": models.len() }),
    )
    .await?;

    Ok(())
}

/// List the nodes of the organisation.
///
/// # Errors
///
/// Returns an error when the caller may not read, or the query fails.
pub async fn list_nodes(
    pool: &PgPool,
    principal: &Principal,
    config: &ComputeConfig,
) -> CoreResult<Vec<(ComputeNode, ComputeNodeStatus)>> {
    let ctx = ResourceContext::organisation(ResourceKind::ComputeNode, principal.organisation_id);
    authorize(principal, Action::Read, &ctx)
        .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    let offline_after =
        Duration::from_std(config.node_offline_after).unwrap_or_else(|_| Duration::seconds(120));

    Ok(repo::list_nodes(pool, principal.organisation_id)
        .await?
        .into_iter()
        .map(|node| {
            let status = node.effective_status(offline_after);
            (node, status)
        })
        .collect())
}

/// Report the state of the Compute Plane.
///
/// With no node enrolled this reports zero, and says why. It is the truth, and
/// it is what the interface shows (`CLAUDE.md` §69).
///
/// # Errors
///
/// Returns an error when the caller may not read, or the query fails.
pub async fn compute_status(
    pool: &PgPool,
    principal: &Principal,
    config: &ComputeConfig,
) -> CoreResult<ComputeStatus> {
    let nodes = list_nodes(pool, principal, config).await?;

    let registered = u32::try_from(nodes.len()).unwrap_or(u32::MAX);
    let online = u32::try_from(
        nodes
            .iter()
            .filter(|(_, status)| *status == ComputeNodeStatus::Online)
            .count(),
    )
    .unwrap_or(u32::MAX);

    let message = if registered == 0 {
        "No Ocinye compute node is registered. The platform operates fully without one.".to_owned()
    } else if online == 0 {
        "No registered compute node is currently reporting in.".to_owned()
    } else {
        format!("{online} of {registered} registered nodes are online.")
    };

    Ok(ComputeStatus {
        registered_nodes: registered,
        online_nodes: online,
        message,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_tokens_are_unpredictable_and_stored_only_as_digests() {
        let (a, digest_a) = new_token();
        let (b, _) = new_token();
        assert_ne!(a, b);
        assert_eq!(a.len(), TOKEN_BYTES * 2);
        assert_eq!(digest_a.len(), 64);
        assert_ne!(digest_a, a);
        assert_eq!(digest(&a), digest_a);
    }
}
