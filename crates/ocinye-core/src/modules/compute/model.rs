//! Compute rows and the node protocol payloads.

use chrono::{DateTime, Duration, Utc};
use ocinye_contracts::{ComputeNodeStatus, InstitutionalControl, NodeKind, Residency};
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
    /// Who controls the software and data on the node (stored representation).
    pub institutional_control: String,
    /// Where the hardware physically resides (stored representation).
    pub physical_residency: String,
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
    /// CPU cores the operator holds back for the host.
    pub reserved_cpu_cores: i32,
    /// Memory the operator holds back for the host.
    pub reserved_memory_bytes: i64,
    /// Storage the operator holds back for the host.
    pub reserved_storage_bytes: i64,
    /// Memory the node last reported in use.
    pub memory_used_bytes: Option<i64>,
    /// Storage the node last reported in use.
    pub storage_used_bytes: Option<i64>,
}

impl ComputeNode {
    /// Parsed kind.
    #[must_use]
    pub fn kind(&self) -> NodeKind {
        NodeKind::parse(&self.kind).unwrap_or(NodeKind::Cpu)
    }

    /// Who controls the software and data on the node. Defaults to `Ocinye` —
    /// an enrolled node runs our agent under our credential (ADR-0503).
    #[must_use]
    pub fn institutional_control(&self) -> InstitutionalControl {
        InstitutionalControl::parse(&self.institutional_control).unwrap_or_default()
    }

    /// Where the hardware physically resides. Defaults to `Undeclared` — the
    /// system never claims a residency it was not told (ADR-0201, ADR-0503).
    #[must_use]
    pub fn physical_residency(&self) -> Residency {
        Residency::parse(&self.physical_residency).unwrap_or_default()
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
    /// Memory in use, when the agent reports it. Optional so an older agent
    /// that does not know the field keeps working.
    #[serde(default)]
    pub memory_used_bytes: Option<u64>,
    /// Storage in use, when the agent reports it.
    #[serde(default)]
    pub storage_used_bytes: Option<u64>,
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

/// One resource on one node, in the five quantities an Instance governs
/// (Part 5 of the generalization programme).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CapacityLine {
    /// What the node reports it has. `None` when it has not reported.
    pub physical: Option<i64>,
    /// What the operator holds back for the host.
    pub reserved: i64,
    /// `physical − reserved`, never below zero.
    pub allocatable: Option<i64>,
    /// What jobs hold. Zero by construction: there is no job dispatch yet, and
    /// the number says so rather than guessing one.
    pub allocated: i64,
    /// What the node reports in use. `None` when it does not report it.
    pub consumed: Option<i64>,
}

impl CapacityLine {
    /// The line from a physical figure, a holdback and a consumption report.
    #[must_use]
    pub fn new(physical: Option<i64>, reserved: i64, consumed: Option<i64>) -> Self {
        Self {
            physical,
            reserved,
            allocatable: physical.map(|p| p.saturating_sub(reserved).max(0)),
            allocated: 0,
            consumed,
        }
    }
}

/// The capacity of one node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct NodeCapacity {
    /// CPU cores.
    pub cpu_cores: CapacityLine,
    /// Memory, in bytes.
    pub memory_bytes: CapacityLine,
    /// Storage, in bytes.
    pub storage_bytes: CapacityLine,
    /// GPUs, counted.
    pub gpus: i64,
    /// GPU memory, summed, in bytes.
    pub gpu_memory_bytes: i64,
}

impl ComputeNode {
    /// The node's capacity, from what it reported and what the operator
    /// reserved.
    #[must_use]
    pub fn capacity(&self) -> NodeCapacity {
        let gpus: Vec<GpuReport> = serde_json::from_value(self.gpus.clone()).unwrap_or_default();
        NodeCapacity {
            cpu_cores: CapacityLine::new(
                self.cpu_cores.map(i64::from),
                i64::from(self.reserved_cpu_cores),
                None,
            ),
            memory_bytes: CapacityLine::new(
                self.memory_bytes,
                self.reserved_memory_bytes,
                self.memory_used_bytes,
            ),
            storage_bytes: CapacityLine::new(
                self.storage_bytes,
                self.reserved_storage_bytes,
                self.storage_used_bytes,
            ),
            gpus: i64::try_from(gpus.len()).unwrap_or(i64::MAX),
            gpu_memory_bytes: gpus
                .iter()
                .map(|gpu| i64::try_from(gpu.memory_bytes).unwrap_or(i64::MAX))
                .fold(0_i64, i64::saturating_add),
        }
    }
}

#[cfg(test)]
mod capacity_tests {
    use super::*;

    #[test]
    fn allocatable_e_fisico_menos_reservado_e_nunca_negativo() {
        let linha = CapacityLine::new(Some(16), 2, None);
        assert_eq!(linha.allocatable, Some(14));
        assert_eq!(linha.allocated, 0);
        assert_eq!(CapacityLine::new(Some(2), 8, None).allocatable, Some(0));
        assert_eq!(CapacityLine::new(None, 2, None).allocatable, None, "sem relatório não se inventa");
    }
}
