//! Institutional resource governance vocabulary.
//!
//! # Why this is a separate axis from access
//!
//! RBAC ([`crate::access::Permission`]) answers *what* an actor may access or
//! do. Resource governance answers *how much* institutional capacity an actor
//! may consume. They are different systems and must never collapse into one: a
//! resource allocation grants no access, and an institutional position grants
//! no capacity — a Founder is not automatically unlimited.
//!
//! # Four concepts this vocabulary keeps apart
//!
//! - **Capacity** — what the institution physically or logically has.
//! - **Entitlement** — what a scope is allowed to consume (an allocation).
//! - **Reservation** — capacity temporarily committed to an operation.
//! - **Usage** — what was actually consumed.
//!
//! These are never one mutable counter.

use serde::{Deserialize, Serialize};

/// A typed institutional resource.
///
/// Every quantity in the governance domain names one of these, so that a bare
/// number is never ambiguous between gigabytes, GPU-hours and requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceType {
    /// Bytes at rest a scope may hold.
    PersistentStorage,
    /// Virtual CPUs.
    Cpu,
    /// Working memory.
    Ram,
    /// Whole GPUs (dedicated compute).
    Gpu,
    /// GPU memory.
    Vram,
    /// GPU wall-clock budget over a period.
    GpuTime,
    /// General compute wall-clock budget over a period.
    ComputeTime,
    /// How many operations may run at once.
    Concurrency,
    /// Which model classes an actor may reach.
    ModelAccess,
    /// The largest context an inference request may carry.
    ModelContext,
    /// The longest a single job may run.
    JobDuration,
    /// How many requests may be made over a period.
    RequestRate,
}

impl ResourceType {
    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PersistentStorage => "persistent_storage",
            Self::Cpu => "cpu",
            Self::Ram => "ram",
            Self::Gpu => "gpu",
            Self::Vram => "vram",
            Self::GpuTime => "gpu_time",
            Self::ComputeTime => "compute_time",
            Self::Concurrency => "concurrency",
            Self::ModelAccess => "model_access",
            Self::ModelContext => "model_context",
            Self::JobDuration => "job_duration",
            Self::RequestRate => "request_rate",
        }
    }

    /// Parse from the stable representation.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "persistent_storage" => Self::PersistentStorage,
            "cpu" => Self::Cpu,
            "ram" => Self::Ram,
            "gpu" => Self::Gpu,
            "vram" => Self::Vram,
            "gpu_time" => Self::GpuTime,
            "compute_time" => Self::ComputeTime,
            "concurrency" => Self::Concurrency,
            "model_access" => Self::ModelAccess,
            "model_context" => Self::ModelContext,
            "job_duration" => Self::JobDuration,
            "request_rate" => Self::RequestRate,
            _ => return None,
        })
    }

    /// Every resource type. The array length is the count, so `parse` and this
    /// cannot drift.
    #[must_use]
    pub const fn all() -> [Self; 12] {
        [
            Self::PersistentStorage,
            Self::Cpu,
            Self::Ram,
            Self::Gpu,
            Self::Vram,
            Self::GpuTime,
            Self::ComputeTime,
            Self::Concurrency,
            Self::ModelAccess,
            Self::ModelContext,
            Self::JobDuration,
            Self::RequestRate,
        ]
    }

    /// The unit a quantity of this resource is always measured in.
    ///
    /// The domain fixes one canonical unit per resource so that stored numbers
    /// are directly comparable; presentation may scale (bytes → GiB) but the
    /// stored quantity does not.
    #[must_use]
    pub const fn canonical_unit(self) -> ResourceUnit {
        match self {
            Self::PersistentStorage => ResourceUnit::Bytes,
            Self::Cpu => ResourceUnit::Vcpu,
            Self::Ram => ResourceUnit::Bytes,
            Self::Gpu => ResourceUnit::GpuCount,
            Self::Vram => ResourceUnit::Bytes,
            Self::GpuTime => ResourceUnit::GpuSeconds,
            Self::ComputeTime => ResourceUnit::CpuSeconds,
            Self::Concurrency => ResourceUnit::Count,
            Self::ModelAccess => ResourceUnit::Count,
            Self::ModelContext => ResourceUnit::ContextTokens,
            Self::JobDuration => ResourceUnit::Seconds,
            Self::RequestRate => ResourceUnit::Requests,
        }
    }

    /// Whether this resource is a persistent stock (storage), measured as a
    /// standing amount, rather than a flow consumed over a period.
    ///
    /// The distinction decides accounting: a stock is measured instantaneously
    /// (used vs limit now), a flow accrues against a period budget.
    #[must_use]
    pub const fn is_standing_stock(self) -> bool {
        matches!(self, Self::PersistentStorage)
    }
}

/// The unit a resource quantity is expressed in.
///
/// Explicit, never implied: `value = 10` alone is a defect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceUnit {
    /// Raw bytes (the canonical storage/VRAM/RAM unit; presentation scales it).
    Bytes,
    /// Virtual CPUs.
    Vcpu,
    /// Whole GPUs.
    GpuCount,
    /// GPU-seconds of wall-clock.
    GpuSeconds,
    /// CPU-seconds of wall-clock.
    CpuSeconds,
    /// A count of requests.
    Requests,
    /// A count of tokens.
    Tokens,
    /// A count of context-window tokens.
    ContextTokens,
    /// Seconds of duration.
    Seconds,
    /// A dimensionless count.
    Count,
}

impl ResourceUnit {
    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Bytes => "bytes",
            Self::Vcpu => "vcpu",
            Self::GpuCount => "gpu_count",
            Self::GpuSeconds => "gpu_seconds",
            Self::CpuSeconds => "cpu_seconds",
            Self::Requests => "requests",
            Self::Tokens => "tokens",
            Self::ContextTokens => "context_tokens",
            Self::Seconds => "seconds",
            Self::Count => "count",
        }
    }

    /// Parse from the stable representation.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "bytes" => Self::Bytes,
            "vcpu" => Self::Vcpu,
            "gpu_count" => Self::GpuCount,
            "gpu_seconds" => Self::GpuSeconds,
            "cpu_seconds" => Self::CpuSeconds,
            "requests" => Self::Requests,
            "tokens" => Self::Tokens,
            "context_tokens" => Self::ContextTokens,
            "seconds" => Self::Seconds,
            "count" => Self::Count,
            _ => return None,
        })
    }
}

/// The kind of scope an allocation targets or a usage event charges.
///
/// Designed so a further scope can be added without reshaping the schema: the
/// scope is a `(type, id)` pair, not a column per kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceScopeType {
    /// The whole institution. The scope id is the organisation.
    Organization,
    /// One member. The scope id is the person.
    Member,
    /// One unit.
    Unit,
    /// One research workspace.
    ResearchWorkspace,
    /// One project.
    Project,
}

impl ResourceScopeType {
    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Organization => "organization",
            Self::Member => "member",
            Self::Unit => "unit",
            Self::ResearchWorkspace => "research_workspace",
            Self::Project => "project",
        }
    }

    /// Parse from the stable representation.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "organization" => Self::Organization,
            "member" => Self::Member,
            "unit" => Self::Unit,
            "research_workspace" => Self::ResearchWorkspace,
            "project" => Self::Project,
            _ => return None,
        })
    }

    /// Every scope type.
    #[must_use]
    pub const fn all() -> [Self; 5] {
        [
            Self::Organization,
            Self::Member,
            Self::Unit,
            Self::ResearchWorkspace,
            Self::Project,
        ]
    }
}

/// Where an allocation's authority comes from.
///
/// Effective entitlement is derived from a profile plus explicit overrides; the
/// source records which, so an administrator can be shown *why* a scope has a
/// given quota rather than an opaque number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AllocationSource {
    /// Derived from the scope's assigned allocation profile.
    Profile,
    /// An explicit institutional override of the profile value.
    Override,
    /// A time-bounded grant on top of the standing entitlement.
    Temporary,
}

impl AllocationSource {
    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Profile => "profile",
            Self::Override => "override",
            Self::Temporary => "temporary",
        }
    }

    /// Parse from the stable representation.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "profile" => Self::Profile,
            "override" => Self::Override,
            "temporary" => Self::Temporary,
            _ => return None,
        })
    }
}

/// Whether an allocation profile may currently be assigned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileStatus {
    /// Assignable and resolving.
    Active,
    /// Kept for existing references, not offered for new assignment.
    Inactive,
}

impl ProfileStatus {
    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Inactive => "inactive",
        }
    }

    /// Parse from the stable representation.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "active" => Self::Active,
            "inactive" => Self::Inactive,
            _ => return None,
        })
    }
}

/// Scheduling priority. Affects ordering; it never grants access or capacity.
///
/// There is deliberately no `AdminUnlimited`: administrative privilege is not
/// infinite compute, and one member must not be able to starve the institution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SchedulingPriority {
    /// Yields to everything else.
    Low,
    /// The institutional default.
    Normal,
    /// Ahead of normal work.
    High,
    /// Reserved for system operations, not a member tier.
    CriticalSystem,
}

impl SchedulingPriority {
    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
            Self::CriticalSystem => "critical_system",
        }
    }

    /// Parse from the stable representation.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "low" => Self::Low,
            "normal" => Self::Normal,
            "high" => Self::High,
            "critical_system" => Self::CriticalSystem,
            _ => return None,
        })
    }
}

/// A member's standing-stock state against a limit (storage today).
///
/// Thresholds are policy, not stored: derived from used vs limit. `OverQuota`
/// blocks new writes; it never deletes data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageState {
    /// Below the warning threshold.
    Normal,
    /// At or above the warning threshold.
    Warning,
    /// At or above the strong-warning threshold, still below the limit.
    Critical,
    /// At or above the limit. New writes are refused; existing data is intact.
    OverQuota,
}

impl StorageState {
    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Warning => "warning",
            Self::Critical => "critical",
            Self::OverQuota => "over_quota",
        }
    }

    /// Derive the state from used and limit bytes.
    ///
    /// A limit of zero means "no limit resolved yet" and reads as `Normal` — the
    /// absence of a resolved quota is not an over-quota condition.
    #[must_use]
    pub fn from_usage(used: i64, limit: i64) -> Self {
        if limit <= 0 {
            return Self::Normal;
        }
        let ratio = used as f64 / limit as f64;
        if used >= limit {
            Self::OverQuota
        } else if ratio >= 0.9 {
            Self::Critical
        } else if ratio >= 0.8 {
            Self::Warning
        } else {
            Self::Normal
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_estados_de_armazenamento_seguem_os_limiares() {
        assert_eq!(StorageState::from_usage(0, 100), StorageState::Normal);
        assert_eq!(StorageState::from_usage(79, 100), StorageState::Normal);
        assert_eq!(StorageState::from_usage(80, 100), StorageState::Warning);
        assert_eq!(StorageState::from_usage(90, 100), StorageState::Critical);
        assert_eq!(StorageState::from_usage(100, 100), StorageState::OverQuota);
        assert_eq!(StorageState::from_usage(140, 100), StorageState::OverQuota);
        // Sem limite resolvido não é excesso.
        assert_eq!(StorageState::from_usage(5, 0), StorageState::Normal);
    }

    #[test]
    fn cada_tipo_de_recurso_tem_unidade_e_ida_e_volta() {
        for tipo in ResourceType::all() {
            assert_eq!(ResourceType::parse(tipo.as_str()), Some(tipo));
            // A unidade canónica existe e faz ida-e-volta.
            let unidade = tipo.canonical_unit();
            assert_eq!(ResourceUnit::parse(unidade.as_str()), Some(unidade));
        }
        for escopo in ResourceScopeType::all() {
            assert_eq!(ResourceScopeType::parse(escopo.as_str()), Some(escopo));
        }
    }
}
