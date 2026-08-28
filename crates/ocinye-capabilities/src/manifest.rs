//! The capability manifest.
//!
//! A capability declares what it is and what it needs. The host grants the
//! intersection of what is declared and what policy allows — never more.

use serde::{Deserialize, Serialize};

use crate::error::{CapabilityError, CapabilityResult};

/// Largest fuel budget any capability may request.
///
/// Fuel bounds computation independently of wall time, so a capability cannot
/// spin forever on a loaded machine.
pub const MAX_FUEL: u64 = 5_000_000_000;
/// Largest memory any capability may request, in bytes.
pub const MAX_MEMORY_BYTES: u64 = 512 * 1024 * 1024;
/// Longest wall time any capability may request, in milliseconds.
pub const MAX_WALL_TIME_MS: u64 = 120_000;

/// What a capability may do with the network.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum NetworkPolicy {
    /// No network at all. The default.
    #[default]
    None,
    /// Outbound requests to named hosts only.
    ///
    /// Not implemented by the current host: declaring it is refused rather than
    /// silently granted, so a capability cannot believe it has network access
    /// it does not have.
    AllowHosts {
        /// Hosts the capability may reach.
        hosts: Vec<String>,
    },
}

/// What a capability may do with the filesystem.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum FilesystemPolicy {
    /// No host filesystem. Inputs are passed in by the host. The default.
    #[default]
    None,
    /// A scratch directory that exists only for the invocation.
    Scratch,
}

/// Bounds the host enforces on an invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Computation budget.
    pub fuel: u64,
    /// Memory ceiling in bytes.
    pub memory_bytes: u64,
    /// Wall-time ceiling in milliseconds.
    pub wall_time_ms: u64,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            fuel: 500_000_000,
            memory_bytes: 64 * 1024 * 1024,
            wall_time_ms: 10_000,
        }
    }
}

/// A capability's declaration of itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    /// Stable identifier, for example `ocinye.bibtex-import`.
    pub identifier: String,
    /// Human-readable name.
    pub name: String,
    /// Version.
    pub version: String,
    /// What it does.
    pub description: String,
    /// Media types it accepts.
    pub inputs: Vec<String>,
    /// Media types it produces.
    pub outputs: Vec<String>,
    /// Network policy requested.
    #[serde(default)]
    pub network: NetworkPolicy,
    /// Filesystem policy requested.
    #[serde(default)]
    pub filesystem: FilesystemPolicy,
    /// Resource limits requested.
    #[serde(default)]
    pub limits: ResourceLimits,
    /// Runtime the component targets.
    #[serde(default = "default_runtime")]
    pub runtime: String,
    /// SHA-256 of the component.
    ///
    /// **Not yet verified by the host.** Signature and checksum verification are
    /// `PLANNED`; declaring that plainly is better than implying the supply
    /// chain is already protected (ADR-0501).
    #[serde(default)]
    pub checksum_sha256: Option<String>,
}

fn default_runtime() -> String {
    "wasm32-wasip1".to_owned()
}

impl Manifest {
    /// Parse and validate a manifest.
    ///
    /// # Errors
    ///
    /// Returns [`CapabilityError::InvalidManifest`] when the declaration is
    /// malformed, and [`CapabilityError::PermissionDenied`] when it asks for
    /// more than the host will ever grant.
    pub fn parse(raw: &str) -> CapabilityResult<Self> {
        let manifest: Self = serde_json::from_str(raw)
            .map_err(|error| CapabilityError::InvalidManifest(error.to_string()))?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// Check the declaration against what the host will grant.
    ///
    /// # Errors
    ///
    /// Returns an error when the manifest is malformed or over-reaching.
    pub fn validate(&self) -> CapabilityResult<()> {
        if self.identifier.trim().is_empty() {
            return Err(CapabilityError::InvalidManifest(
                "identifier is empty".to_owned(),
            ));
        }
        if self.version.trim().is_empty() {
            return Err(CapabilityError::InvalidManifest(
                "version is empty".to_owned(),
            ));
        }
        if self.runtime != "wasm32-wasip1" {
            return Err(CapabilityError::InvalidManifest(format!(
                "unsupported runtime target `{}`",
                self.runtime
            )));
        }

        // Refusing an unimplemented permission is the point: a capability must
        // never be told it has network access that the host cannot police.
        if let NetworkPolicy::AllowHosts { .. } = self.network {
            return Err(CapabilityError::PermissionDenied(
                "network access is not implemented by this host and is therefore not granted"
                    .to_owned(),
            ));
        }

        if self.limits.fuel == 0 || self.limits.fuel > MAX_FUEL {
            return Err(CapabilityError::PermissionDenied(format!(
                "fuel must be between 1 and {MAX_FUEL}"
            )));
        }
        if self.limits.memory_bytes == 0 || self.limits.memory_bytes > MAX_MEMORY_BYTES {
            return Err(CapabilityError::PermissionDenied(format!(
                "memory must be between 1 and {MAX_MEMORY_BYTES} bytes"
            )));
        }
        if self.limits.wall_time_ms == 0 || self.limits.wall_time_ms > MAX_WALL_TIME_MS {
            return Err(CapabilityError::PermissionDenied(format!(
                "wall time must be between 1 and {MAX_WALL_TIME_MS} ms"
            )));
        }

        Ok(())
    }

    /// Whether the host will give this capability any filesystem at all.
    #[must_use]
    pub fn wants_scratch(&self) -> bool {
        self.filesystem == FilesystemPolicy::Scratch
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal() -> Manifest {
        Manifest {
            identifier: "ocinye.test".into(),
            name: "Test".into(),
            version: "0.1.0".into(),
            description: "A test capability".into(),
            inputs: vec!["text/plain".into()],
            outputs: vec!["application/json".into()],
            network: NetworkPolicy::None,
            filesystem: FilesystemPolicy::None,
            limits: ResourceLimits::default(),
            runtime: "wasm32-wasip1".into(),
            checksum_sha256: None,
        }
    }

    #[test]
    fn defaults_grant_nothing() {
        assert_eq!(NetworkPolicy::default(), NetworkPolicy::None);
        assert_eq!(FilesystemPolicy::default(), FilesystemPolicy::None);
        assert!(!minimal().wants_scratch());
    }

    #[test]
    fn requesting_network_is_refused_rather_than_silently_granted() {
        let mut manifest = minimal();
        manifest.network = NetworkPolicy::AllowHosts {
            hosts: vec!["example.org".into()],
        };
        assert!(matches!(
            manifest.validate(),
            Err(CapabilityError::PermissionDenied(_))
        ));
    }

    #[test]
    fn resource_requests_beyond_the_ceiling_are_refused() {
        for limits in [
            ResourceLimits {
                fuel: MAX_FUEL + 1,
                ..ResourceLimits::default()
            },
            ResourceLimits {
                memory_bytes: MAX_MEMORY_BYTES + 1,
                ..ResourceLimits::default()
            },
            ResourceLimits {
                wall_time_ms: MAX_WALL_TIME_MS + 1,
                ..ResourceLimits::default()
            },
            ResourceLimits {
                fuel: 0,
                ..ResourceLimits::default()
            },
        ] {
            let mut manifest = minimal();
            manifest.limits = limits;
            assert!(manifest.validate().is_err(), "should refuse {limits:?}");
        }
    }

    #[test]
    fn unknown_runtime_targets_are_refused() {
        let mut manifest = minimal();
        manifest.runtime = "wasm32-unknown-unknown".into();
        assert!(manifest.validate().is_err());
    }

    #[test]
    fn a_manifest_round_trips_through_json() {
        let manifest = minimal();
        let raw = serde_json::to_string(&manifest).unwrap();
        assert_eq!(Manifest::parse(&raw).unwrap(), manifest);
    }
}
