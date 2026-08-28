//! Compute capabilities.

use async_trait::async_trait;
use ocinye_contracts::agentic::{
    ApprovalRequirement, AutonomyLevel, CapabilityDescriptor, CapabilityId, CapabilityResult,
    ExecutionStatus, OperationId, ResourceKind as AgenticKind, ResourceRef, Reversibility,
    RiskLevel,
};
use ocinye_contracts::{Permission, Scope};

use crate::error::CoreResult;
use crate::modules::agentic::executor::ExecutionContext;
use crate::modules::agentic::registry::CapabilityHandler;
use crate::modules::compute;

/// List the compute nodes.
///
/// # The capability that tells the truth about nothing
///
/// This installation has zero nodes. Asked «que nós existem?», the honest
/// answer is «nenhum», and it comes from a real count rather than from a model
/// that might helpfully invent one (`CLAUDE.md` §29, §69).
pub struct ListNodes;

#[async_trait]
impl CapabilityHandler for ListNodes {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new("compute.node.list"),
            operation: OperationId::new("compute::list_nodes"),
            domain: "compute".to_owned(),
            summary: "Listar os nós de computação registados.".to_owned(),
            permission: Permission::ComputeView,
            scope: Scope::Institution,
            risk: RiskLevel::ReadOnly,
            approval: ApprovalRequirement::Never,
            max_autonomy: AutonomyLevel::Workflow,
            reversibility: Reversibility::NothingToUndo,
            supports_dry_run: false,
            classification_ceiling: None,
            input_schema: serde_json::json!({"type": "object", "properties": {}}),
        }
    }

    async fn execute(&self, ctx: &ExecutionContext<'_>) -> CoreResult<CapabilityResult> {
        // The configuration decides what counts as offline, so the answer is
        // the same one the Compute screen gives.
        let config = crate::config::ComputeConfig {
            enrollment_token_ttl: std::time::Duration::from_secs(3600),
            node_offline_after: std::time::Duration::from_secs(120),
        };

        let nodes = compute::list_nodes(ctx.pool, ctx.principal, &config).await?;

        let online = nodes
            .iter()
            .filter(|(_, status)| *status == ocinye_contracts::ComputeNodeStatus::Online)
            .count();

        Ok(CapabilityResult {
            capability: self.descriptor().id,
            status: ExecutionStatus::Succeeded,
            detail: if nodes.is_empty() {
                "Nenhum nó de computação está registado no Ocinye OS.".to_owned()
            } else {
                format!("{} nó(s) registado(s), {online} activo(s).", nodes.len())
            },
            resources: nodes
                .iter()
                .map(|(node, _)| ResourceRef {
                    kind: AgenticKind::ComputeNode,
                    id: node.id,
                    label: Some(node.identifier.clone()),
                })
                .collect(),
            reversibility: Reversibility::NothingToUndo,
            output: Some(serde_json::json!({
                "registered": nodes.len(),
                "online": online,
            })),
        })
    }
}
