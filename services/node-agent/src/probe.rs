//! What the node reports about itself.
//!
//! Everything here is a *claim* the node makes. The Core records it for
//! operators and uses it for liveness; it never uses it to decide authorization
//! (ADR-0500).

use serde::{Deserialize, Serialize};
use sysinfo::System;

/// A GPU as reported.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuReport {
    /// Model name.
    pub model: String,
    /// Memory in bytes.
    pub memory_bytes: u64,
    /// Driver or runtime version.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub driver_version: Option<String>,
}

/// Resources reported by the node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeResources {
    /// CPU cores.
    pub cpu_cores: u32,
    /// Total memory in bytes.
    pub memory_bytes: u64,
    /// Total storage in bytes.
    pub storage_bytes: u64,
    /// GPUs present.
    pub gpus: Vec<GpuReport>,
}

/// A model the node has loaded.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportedModel {
    /// Model name.
    pub name: String,
    /// Version.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Capabilities it serves.
    pub capabilities: Vec<String>,
    /// Context window.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_limit: Option<i32>,
}

/// A heartbeat payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Heartbeat {
    /// Agent version.
    pub agent_version: String,
    /// Reported resources.
    pub resources: NodeResources,
    /// Capabilities offered.
    pub capabilities: Vec<String>,
    /// Models loaded.
    pub models: Vec<ReportedModel>,
    /// Free-form health detail for operators.
    pub health: serde_json::Value,
}

/// Collect the current state of the machine.
///
/// GPU and model discovery are **not implemented**: they require vendor
/// tooling and a model runtime that do not exist on any Ocinye node yet. The
/// agent reports empty lists, which is the truth, rather than plausible
/// placeholders (`CLAUDE.md` §69).
#[must_use]
pub fn collect(agent_version: &str) -> Heartbeat {
    let mut system = System::new();
    system.refresh_memory();
    system.refresh_cpu_all();

    Heartbeat {
        agent_version: agent_version.to_owned(),
        resources: NodeResources {
            cpu_cores: u32::try_from(system.cpus().len()).unwrap_or(0),
            memory_bytes: system.total_memory(),
            // Storage discovery is deliberately left at zero rather than
            // guessed from an arbitrary mount point.
            storage_bytes: 0,
            gpus: Vec::new(),
        },
        capabilities: Vec::new(),
        models: Vec::new(),
        health: serde_json::json!({
            "uptime_seconds": System::uptime(),
            "gpu_discovery": "not_implemented",
            "model_discovery": "not_implemented",
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_node_with_no_models_reports_none() {
        let heartbeat = collect("0.1.0");
        assert!(heartbeat.models.is_empty());
        assert!(heartbeat.capabilities.is_empty());
        assert!(heartbeat.resources.gpus.is_empty());
    }

    #[test]
    fn unimplemented_discovery_is_declared_not_faked() {
        let heartbeat = collect("0.1.0");
        assert_eq!(heartbeat.health["gpu_discovery"], "not_implemented");
        assert_eq!(heartbeat.health["model_discovery"], "not_implemented");
    }

    #[test]
    fn cpu_and_memory_are_actually_probed() {
        let heartbeat = collect("0.1.0");
        assert!(heartbeat.resources.cpu_cores > 0);
        assert!(heartbeat.resources.memory_bytes > 0);
    }
}
