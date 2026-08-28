//! Compute rows and the node protocol payloads.

use chrono::{DateTime, Duration, Utc};
use ocinye_contracts::{ComputeNodeStatus, NodeKind};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use uuid::Uuid;

/// A registered compute node.
#[derive(Debug, Clone, FromRow)]
pub struct ComputeNode {
    /// Identifier.
    pub id: Uuid,
    /// Institutional identifier supplied at registration, for example `CAM-01`.
    pub identifier: String,
    /// Display name.
    pub display_name: String,
    /// Kind of node.
    pub kind: String,
    /// Human label of where it is.
    pub location_label: Option<String>,
    /// Stored status.
    pub status: String,
    /// Reported CPU cores.
    pub cpu_cores: Option<i32>,
    /// Reported memory.
    pub memory_bytes: Option<i64>,
    /// Reported storage.
    pub storage_bytes: Option<i64>,
    /// Reported GPUs.
    pub gpus: Value,
    /// Reported capabilities.
    pub capabilities: Value,
    /// Agent version.
    pub agent_version: Option<String>,
    /// Last heartbeat.
    pub last_seen_at: Option<DateTime<Utc>>,
    /// Registration time.
    pub created_at: DateTime<Utc>,
}

impl ComputeNode {
    /// Parsed kind.
    #[must_use]
    pub fn kind(&self) -> NodeKind {
        NodeKind::parse(&self.kind).unwrap_or(NodeKind::Cpu)
    }

    /// Effective status, derived from the heartbeat rather than the stored flag.
    ///
    /// A node is online only if it has spoken recently. Nothing can set a node
    /// "online" without it actually reporting in (ADR-0500).
    #[must_use]
    pub fn effective_status(&self, offline_after: Duration) -> ComputeNodeStatus {
        let stored =
            ComputeNodeStatus::parse(&self.status).unwrap_or(ComputeNodeStatus::PendingEnrollment);

        match stored {
            // Terminal or administrative states are not overridden by liveness.
            ComputeNodeStatus::Retired
            | ComputeNodeStatus::Draining
            | ComputeNodeStatus::PendingEnrollment => stored,
            ComputeNodeStatus::Online | ComputeNodeStatus::Offline => match self.last_seen_at {
                Some(seen) if Utc::now() - seen < offline_after => ComputeNodeStatus::Online,
                _ => ComputeNodeStatus::Offline,
            },
        }
    }
}

/// A GPU as reported by an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuReport {
    /// Model name as reported.
    pub model: String,
    /// Memory in bytes.
    pub memory_bytes: u64,
    /// Driver or runtime version.
    #[serde(default)]
    pub driver_version: Option<String>,
}

/// Resources a node reports about itself.
///
/// Untrusted input: recorded for operators to see, never used to decide
/// authorization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeResources {
    /// CPU cores.
    pub cpu_cores: u32,
    /// Memory in bytes.
    pub memory_bytes: u64,
    /// Storage in bytes.
    pub storage_bytes: u64,
    /// GPUs present.
    #[serde(default)]
    pub gpus: Vec<GpuReport>,
}

/// A heartbeat from a node agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeHeartbeat {
    /// Version of the agent.
    pub agent_version: String,
    /// Resources currently available.
    pub resources: NodeResources,
    /// Capabilities the node offers, for example `GENERAL` or `EMBEDDING`.
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// Models the node has loaded, if any.
    #[serde(default)]
    pub models: Vec<ReportedModel>,
    /// Free-form health detail for operators.
    #[serde(default)]
    pub health: Value,
}

/// A model a node reports as loaded.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportedModel {
    /// Model name, for example `qwen2.5`.
    pub name: String,
    /// Version.
    #[serde(default)]
    pub version: Option<String>,
    /// Capabilities it serves.
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// Context window, when known.
    #[serde(default)]
    pub context_limit: Option<i32>,
}
