//! Hardware discovered on the target (D011_HARDWARE_CAPABILITY_MATRIX).
//!
//! Read-only, after preflight. Everything here is a **detected fact**:
//! [`Evidence::Detected`] is the strongest evidence D011 may produce —
//! `Verified` and `Attested` exist only so D012 does not have to change the
//! wire format, and nothing in D011 sets them.
//!
//! GPU absence is a normal state, never a blocker. A discovery error is not
//! «no GPU»: it is recorded as an error, with a code.

use serde::{Deserialize, Serialize};

use crate::manifest::Arch;

/// CPU features of interest (closed set).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CpuFeature {
    /// AVX2.
    Avx2,
    /// AVX-512 (foundation).
    Avx512,
    /// AMD SEV-SNP.
    SevSnp,
    /// Intel TDX.
    Tdx,
    /// Arm SVE.
    Sve,
}

/// The processor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CpuCapabilities {
    /// Model name, when the platform says one.
    pub model: Option<String>,
    /// `uname -m`, mapped; `None` for an architecture Ocinye does not run on.
    pub arch: Option<Arch>,
    /// Physical cores, when topology is readable.
    pub cores: Option<u32>,
    /// Online logical CPUs.
    pub threads: u32,
    /// Features of interest.
    pub features: Vec<CpuFeature>,
}

/// Memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryCapabilities {
    /// `MemTotal`.
    pub total_bytes: u64,
    /// `MemAvailable`.
    pub available_bytes: u64,
    /// `SwapTotal` — reported, never relied on.
    pub swap_bytes: u64,
}

/// Storage where Ocinye will live (`/srv`, or `/` if `/srv` does not exist).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorageCapabilities {
    /// The path measured.
    pub path: String,
    /// Free bytes for unprivileged use.
    pub free_bytes: u64,
    /// Total bytes.
    pub total_bytes: u64,
    /// Filesystem type from `/proc/mounts`.
    pub filesystem: Option<String>,
}

/// Network capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkCapabilities {
    /// Interfaces that are up (loopback excluded).
    pub interfaces_up: u8,
    /// Fastest link reported, in Mb/s.
    pub max_link_mbps: Option<u32>,
    /// A global IPv6 address exists.
    pub ipv6: bool,
}

/// GPU vendor, from the PCI vendor id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "vendor", content = "pci_id", rename_all = "snake_case")]
pub enum GpuVendor {
    /// 0x10de.
    Nvidia,
    /// 0x1002.
    Amd,
    /// 0x8086.
    Intel,
    /// Any other vendor id.
    Other(u16),
}

impl GpuVendor {
    /// From the PCI vendor id.
    #[must_use]
    pub const fn from_pci(id: u16) -> Self {
        match id {
            0x10de => Self::Nvidia,
            0x1002 => Self::Amd,
            0x8086 => Self::Intel,
            other => Self::Other(other),
        }
    }
}

/// The kernel driver bound to a device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DriverInfo {
    /// Module name (`nvidia`, `amdgpu`, `i915`, …).
    pub module: String,
    /// Module version, when the module reports one.
    pub version: Option<String>,
}

/// Accelerator runtime kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeKind {
    /// NVIDIA Container Toolkit (a `nvidia` Docker runtime).
    NvidiaContainerToolkit,
    /// ROCm (`/dev/kfd`).
    Rocm,
    /// Intel oneAPI Level Zero.
    IntelOneApi,
}

/// Runtime status for one device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeStatus {
    /// Present.
    Detected,
    /// The device is visible but its runtime is not.
    Unavailable,
}

/// One accelerator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GpuCapabilities {
    /// 0-based index in discovery order.
    pub index: u8,
    /// PCI address (`0000:01:00.0`).
    pub pci_address: String,
    /// Vendor.
    pub vendor: GpuVendor,
    /// PCI device id.
    pub device_id: u16,
    /// Model, when the driver names it.
    pub model: Option<String>,
    /// VRAM, when the driver reports it.
    pub vram_bytes: Option<u64>,
    /// Bound driver.
    pub driver: Option<DriverInfo>,
    /// CUDA compute capability or AMD gfx target.
    pub compute_capability: Option<String>,
    /// Runtime for this vendor.
    pub runtime: Option<(RuntimeKind, RuntimeStatus)>,
}

/// The GPU picture. None of these blocks the base installation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GpuDiscovery {
    /// No accelerator (a BMC / basic display adapter may be named).
    NoGpu {
        /// A basic display adapter that was seen and is not an accelerator.
        basic_display: Option<String>,
    },
    /// One or more accelerators with their runtime.
    GpuDetected {
        /// The devices.
        gpus: Vec<GpuCapabilities>,
    },
    /// Accelerators visible, runtime missing for at least one.
    GpuRuntimeUnavailable {
        /// The devices.
        gpus: Vec<GpuCapabilities>,
    },
    /// Discovery could not read what it needed. **Not** «no GPU».
    GpuDiscoveryError {
        /// Stable code (`EACCES`, `SYSFS_UNREADABLE`, …).
        code: String,
    },
}

/// Evidence level. D011 produces `NotDetected`, `Detected` or `Unknown` only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Evidence {
    /// Looked, not there.
    NotDetected,
    /// Announced by the platform; not verified, not attested.
    Detected,
    /// Reserved for D012.
    Verified,
    /// Reserved for D012.
    Attested,
    /// Could not look.
    Unknown,
}

/// Confidential-computing technology.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfidentialTech {
    /// AMD SEV-SNP.
    AmdSevSnp,
    /// Intel TDX.
    IntelTdx,
}

/// Security-relevant capability (detected only).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComputeSecurityCapability {
    /// Confidential-computing technologies announced.
    pub confidential: Vec<(ConfidentialTech, Evidence)>,
    /// TPM 2.0 resource manager present.
    pub tpm: Evidence,
}

/// Everything discovered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HardwareCapabilities {
    /// Processor.
    pub cpu: CpuCapabilities,
    /// Memory.
    pub memory: MemoryCapabilities,
    /// Storage.
    pub storage: StorageCapabilities,
    /// Network.
    pub network: NetworkCapabilities,
    /// Accelerators.
    pub gpu: GpuDiscovery,
    /// Security capability.
    pub security: ComputeSecurityCapability,
}

/// Informational only. There is deliberately no `Registered`, `Connected`,
/// `Available` or `Earning`: D011 enrols nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ComputeReadiness {
    /// Discovery did not complete.
    NotAssessed,
    /// CPU only.
    CpuOnlyFutureCandidate,
    /// Accelerators with their runtime.
    GpuFutureCandidate,
    /// Accelerators without their runtime.
    GpuRuntimeMissing,
}

impl ComputeReadiness {
    /// Derived from discovery.
    #[must_use]
    pub const fn of(gpu: &GpuDiscovery) -> Self {
        match gpu {
            GpuDiscovery::NoGpu { .. } => Self::CpuOnlyFutureCandidate,
            GpuDiscovery::GpuDetected { .. } => Self::GpuFutureCandidate,
            GpuDiscovery::GpuRuntimeUnavailable { .. } => Self::GpuRuntimeMissing,
            GpuDiscovery::GpuDiscoveryError { .. } => Self::NotAssessed,
        }
    }
}

/// Provider Mode. D011: the only value. D012 adds others behind explicit,
/// reversible consent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProviderMode {
    /// Off.
    Off,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_prontidao_nunca_diz_mais_do_que_foi_detectado() {
        assert_eq!(
            ComputeReadiness::of(&GpuDiscovery::NoGpu {
                basic_display: None
            }),
            ComputeReadiness::CpuOnlyFutureCandidate
        );
        assert_eq!(
            ComputeReadiness::of(&GpuDiscovery::GpuDiscoveryError {
                code: "EACCES".into()
            }),
            ComputeReadiness::NotAssessed,
            "um erro de descoberta não é «sem GPU»"
        );
        // O contrato não tem como dizer «registado» ou «a ganhar».
        for s in ["REGISTERED", "CONNECTED", "AVAILABLE", "EARNING"] {
            assert!(serde_json::from_str::<ComputeReadiness>(&format!("\"{s}\"")).is_err());
        }
        assert!(serde_json::from_str::<ProviderMode>("\"ON\"").is_err());
    }

    #[test]
    fn o_fornecedor_vem_do_identificador_pci() {
        assert_eq!(GpuVendor::from_pci(0x10de), GpuVendor::Nvidia);
        assert_eq!(GpuVendor::from_pci(0x1a03), GpuVendor::Other(0x1a03));
    }
}
