//! Compute Plane contracts.
//!
//! The registry represents 0..N nodes. Zero is the current, correct state; no
//! node identifier is hardcoded anywhere (briefing §54, §55).

use serde::{Deserialize, Serialize};

/// Kind of compute node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    /// GPU-bearing node.
    Gpu,
    /// CPU-only node.
    Cpu,
    /// High-performance computing cluster front-end.
    Hpc,
    /// Storage node.
    Storage,
}

impl NodeKind {
    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Gpu => "gpu",
            Self::Cpu => "cpu",
            Self::Hpc => "hpc",
            Self::Storage => "storage",
        }
    }

    /// Parse from the stable representation.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "gpu" => Self::Gpu,
            "cpu" => Self::Cpu,
            "hpc" => Self::Hpc,
            "storage" => Self::Storage,
            _ => return None,
        })
    }
}

/// Who controls the software and data running on a node.
///
/// A distinct axis from **where the hardware physically is** ([`Residency`]).
/// Renting a machine in a third-party datacentre does not cede control: an
/// enrolled Ocinye node runs the Ocinye Node Agent under the institution's own
/// credential, so it is `Ocinye`-controlled even when its `physical_residency`
/// is `THIRD_PARTY_CLOUD`. `External` is reserved for a provider the institution
/// does **not** control (an external inference API), which is represented at the
/// provider level, not as an enrolled node (ADR-0503).
///
/// [`Residency`]: crate::Residency
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InstitutionalControl {
    /// The institution controls the software and data. The default for a node,
    /// because enrolment means it runs the Ocinye Node Agent under our credential.
    #[default]
    Ocinye,
    /// Controlled by a third party. Not an enrolled node.
    External,
}

impl InstitutionalControl {
    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ocinye => "OCINYE",
            Self::External => "EXTERNAL",
        }
    }

    /// Parse from the stable representation.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "OCINYE" => Self::Ocinye,
            "EXTERNAL" => Self::External,
            _ => return None,
        })
    }
}

/// Lifecycle status of a compute node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComputeNodeStatus {
    /// Registered and issued an enrollment token; never yet seen.
    PendingEnrollment,
    /// Heartbeating within the liveness window.
    Online,
    /// Enrolled but not heartbeating.
    Offline,
    /// Finishing current work, accepting no new jobs.
    Draining,
    /// Permanently withdrawn.
    Retired,
}

impl ComputeNodeStatus {
    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PendingEnrollment => "pending_enrollment",
            Self::Online => "online",
            Self::Offline => "offline",
            Self::Draining => "draining",
            Self::Retired => "retired",
        }
    }

    /// Parse from the stable representation.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "pending_enrollment" => Self::PendingEnrollment,
            "online" => Self::Online,
            "offline" => Self::Offline,
            "draining" => Self::Draining,
            "retired" => Self::Retired,
            _ => return None,
        })
    }

    /// Whether the node may currently receive work.
    #[must_use]
    pub const fn accepts_jobs(self) -> bool {
        matches!(self, Self::Online)
    }
}

/// Status of a compute or AI job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    /// Accepted and waiting.
    Queued,
    /// Executing.
    Running,
    /// Finished successfully.
    Succeeded,
    /// Finished with an error.
    Failed,
    /// Refused before execution — for example, no node provides the capability.
    Rejected,
    /// Withdrawn before completion.
    Cancelled,
}

impl JobStatus {
    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Rejected => "rejected",
            Self::Cancelled => "cancelled",
        }
    }

    /// Parse from the stable representation.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "queued" => Self::Queued,
            "running" => Self::Running,
            "succeeded" => Self::Succeeded,
            "failed" => Self::Failed,
            "rejected" => Self::Rejected,
            "cancelled" => Self::Cancelled,
            _ => return None,
        })
    }
}

/// Reported state of the Compute Plane.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeStatus {
    /// Total registered nodes, including offline and pending ones.
    pub registered_nodes: u32,
    /// Nodes currently within the liveness window.
    pub online_nodes: u32,
    /// Explanation shown to members when the plane is empty.
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_online_nodes_accept_jobs() {
        assert!(ComputeNodeStatus::Online.accepts_jobs());
        for status in [
            ComputeNodeStatus::PendingEnrollment,
            ComputeNodeStatus::Offline,
            ComputeNodeStatus::Draining,
            ComputeNodeStatus::Retired,
        ] {
            assert!(!status.accepts_jobs());
        }
    }

    #[test]
    fn control_and_residency_are_independent_axes() {
        // The L40S case: Ocinye-controlled software on rented third-party
        // hardware. Control and physical residency are set independently, so one
        // never implies the other (ADR-0503).
        assert_eq!(
            InstitutionalControl::default(),
            InstitutionalControl::Ocinye
        );
        let control = InstitutionalControl::Ocinye;
        let residency = crate::Residency::ThirdPartyCloud;
        assert!(!residency.is_ocinye_owned());
        assert_eq!(control.as_str(), "OCINYE");
        assert_eq!(residency.as_str(), "THIRD_PARTY_CLOUD");
        // Round-trips through the stable representation.
        assert_eq!(InstitutionalControl::parse(control.as_str()), Some(control));
        assert_eq!(
            InstitutionalControl::parse("EXTERNAL"),
            Some(InstitutionalControl::External)
        );
        assert_eq!(InstitutionalControl::parse("nonsense"), None);
    }
}
