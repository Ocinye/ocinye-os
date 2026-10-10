//! The installation plan (D011_CONTRACT_MATRIX › InstallationPlan, ADR-0023).
//!
//! Everything the operator configured, everything the server will be changed
//! by, and the release and server it applies to — sealed by
//! `plan_sha256 = SHA-256(canonical JSON without plan_sha256)`. After «Instalar
//! Ocinye OS» the bootstrap executes **that** plan and nothing else: `Execute`
//! carries the whole plan, the bootstrap recomputes the seal, and a plan whose
//! seal does not match is refused before any mutation. Changing anything
//! afterwards — even one letter of the Instance name — is a new plan, with a
//! new `plan_id`, reviewed and confirmed again.

use serde::{Deserialize, Serialize};

use ocinye_contracts::Distribution;

use crate::canonical;
use crate::hardware::{ComputeReadiness, HardwareCapabilities, ProviderMode};
use crate::ident::{
    EmailAddress, HostNameValue, InstallationId, InstanceName, PersonName, PlanId, SshUser,
    TargetHost,
};
use crate::manifest::{Arch, BuildKind, ReleaseManifest};
use crate::preflight::{CheckStatus, PreflightCheckId};

/// The 16 phases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum PhaseId {
    /// Prepare the server (bootstrap verified).
    P01,
    /// Transfer the release (remote hashes).
    P02,
    /// Confirm the preflight.
    P03,
    /// Install prerequisites (Docker; skipped when nothing is planned).
    P04,
    /// Install the release tree.
    P05,
    /// Configuration and secrets — point of no return.
    P06,
    /// Proxy and TLS.
    P07,
    /// Ocinye firewall rules (ufw; skipped when nothing is planned).
    P08,
    /// Load images.
    P09,
    /// Database and storage.
    P10,
    /// Instance, migrations and administrator.
    P11,
    /// Access endpoints.
    P12,
    /// Start services.
    P13,
    /// Start on boot.
    P14,
    /// Verify the product.
    P15,
    /// Remove the temporary executor.
    P16,
}

/// Safety class (D011_STATE_MACHINE): drives resume and cancel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SafetyClass {
    /// No server state.
    A,
    /// Prerequisite packages / ufw rules (no Ocinye state).
    BPrime,
    /// Files, no secrets.
    B,
    /// Secrets written.
    C,
    /// The Instance is being created: no resume inside.
    D,
    /// After the Instance.
    E,
}

/// When a stop request takes effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CancelPolicy {
    /// Right away.
    Immediate,
    /// When the current phase ends.
    EndOfPhase,
    /// When the transaction in progress ends.
    AfterTransaction,
}

impl PhaseId {
    /// All phases in order.
    pub const ALL: [Self; 16] = [
        Self::P01,
        Self::P02,
        Self::P03,
        Self::P04,
        Self::P05,
        Self::P06,
        Self::P07,
        Self::P08,
        Self::P09,
        Self::P10,
        Self::P11,
        Self::P12,
        Self::P13,
        Self::P14,
        Self::P15,
        Self::P16,
    ];

    /// `P01`…`P16`.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::P01 => "P01",
            Self::P02 => "P02",
            Self::P03 => "P03",
            Self::P04 => "P04",
            Self::P05 => "P05",
            Self::P06 => "P06",
            Self::P07 => "P07",
            Self::P08 => "P08",
            Self::P09 => "P09",
            Self::P10 => "P10",
            Self::P11 => "P11",
            Self::P12 => "P12",
            Self::P13 => "P13",
            Self::P14 => "P14",
            Self::P15 => "P15",
            Self::P16 => "P16",
        }
    }

    /// 1-based number.
    #[must_use]
    pub fn number(self) -> usize {
        Self::ALL.iter().position(|p| *p == self).unwrap_or(0) + 1
    }

    /// Safety class.
    #[must_use]
    pub const fn class(self) -> SafetyClass {
        match self {
            Self::P01 | Self::P02 | Self::P03 => SafetyClass::A,
            Self::P04 | Self::P08 => SafetyClass::BPrime,
            Self::P05 => SafetyClass::B,
            Self::P06 | Self::P07 | Self::P09 | Self::P10 => SafetyClass::C,
            Self::P11 => SafetyClass::D,
            Self::P12 | Self::P13 | Self::P14 | Self::P15 | Self::P16 => SafetyClass::E,
        }
    }

    /// Cancel policy.
    #[must_use]
    pub const fn cancel(self) -> CancelPolicy {
        match self.class() {
            SafetyClass::A | SafetyClass::B => CancelPolicy::Immediate,
            SafetyClass::D => CancelPolicy::AfterTransaction,
            _ => CancelPolicy::EndOfPhase,
        }
    }
}

/// The release the plan installs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseIdentity {
    /// Short id.
    pub id: String,
    /// Full commit.
    pub commit: String,
    /// Release or proof.
    pub build: BuildKind,
    /// Architecture.
    pub arch: Arch,
    /// SHA-256 of the manifest's canonical bytes.
    pub manifest_sha256: String,
}

impl ReleaseIdentity {
    /// From a validated manifest.
    #[must_use]
    pub fn of(manifest: &ReleaseManifest) -> Self {
        Self {
            id: manifest.release.id.clone(),
            commit: manifest.release.commit.clone(),
            build: manifest.release.build,
            arch: manifest.target.arch,
            manifest_sha256: manifest.sha256(),
        }
    }
}

/// The server the plan applies to — host key included, so a different
/// machine at the same address is a different plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetIdentity {
    /// Address.
    pub host: TargetHost,
    /// SSH port.
    pub port: u16,
    /// SSH user.
    pub user: SshUser,
    /// Host key algorithm (`ssh-ed25519`, …).
    pub host_key_algorithm: String,
    /// `SHA256:<base64>` of the host key.
    pub host_key_sha256: String,
}

/// Enabled Distributions: 1..=4, distinct, the first is the one the Instance
/// is born with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Vec<Distribution>", into = "Vec<Distribution>")]
pub struct Distributions(Vec<Distribution>);

/// Why a configuration was refused (local validation, before any plan).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ConfigurationError {
    /// No Distribution selected (I09).
    NoDistribution,
    /// A Distribution twice or more than four.
    DistributionsInvalid,
    /// A bound endpoint uses the canonical host (A001-M016).
    EndpointIsCanonical,
    /// Two endpoints with the same host.
    EndpointDuplicate,
    /// A bound endpoint names a Distribution that is not enabled.
    EndpointDistributionNotEnabled,
    /// The person and the privileged identity are not distinguishable (I12).
    AdminNotDistinct,
    /// The TLS certificate does not cover every endpoint.
    TlsDoesNotCover,
}

impl TryFrom<Vec<Distribution>> for Distributions {
    type Error = ConfigurationError;
    fn try_from(v: Vec<Distribution>) -> Result<Self, Self::Error> {
        Self::new(v)
    }
}

impl From<Distributions> for Vec<Distribution> {
    fn from(d: Distributions) -> Self {
        d.0
    }
}

impl std::fmt::Display for ConfigurationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl Distributions {
    /// # Errors
    ///
    /// [`ConfigurationError::NoDistribution`] or
    /// [`ConfigurationError::DistributionsInvalid`].
    pub fn new(ordered: Vec<Distribution>) -> Result<Self, ConfigurationError> {
        if ordered.is_empty() {
            return Err(ConfigurationError::NoDistribution);
        }
        let mut seen = Vec::new();
        for d in &ordered {
            if seen.contains(d) {
                return Err(ConfigurationError::DistributionsInvalid);
            }
            seen.push(*d);
        }
        if ordered.len() > 4 {
            return Err(ConfigurationError::DistributionsInvalid);
        }
        Ok(Self(ordered))
    }

    /// In order (first = birth).
    #[must_use]
    pub fn as_slice(&self) -> &[Distribution] {
        &self.0
    }

    /// Enabled?
    #[must_use]
    pub fn contains(&self, d: Distribution) -> bool {
        self.0.contains(&d)
    }
}

/// An endpoint bound to one Distribution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoundEndpoint {
    /// Host.
    pub host: HostNameValue,
    /// Distribution.
    pub distribution: Distribution,
}

/// The canonical (generic) endpoint and the bound ones.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccessEndpoints {
    /// Generic, never bound (A001-M016); `OCINYE_WORKSPACE_PUBLIC_URL`.
    pub canonical: HostNameValue,
    /// Bound endpoints, seeded by `endpoint-seed` in P12.
    pub bound: Vec<BoundEndpoint>,
}

impl AccessEndpoints {
    /// Every host, canonical first.
    #[must_use]
    pub fn hosts(&self) -> Vec<&HostNameValue> {
        std::iter::once(&self.canonical)
            .chain(self.bound.iter().map(|b| &b.host))
            .collect()
    }
}

/// TLS, as planned. The private key is **not** here: it travels once, by
/// file, and never into a plan, a journal or a receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TlsPlan {
    /// The operator's certificate («Certificado fornecido por si»).
    OperatorSupplied {
        /// SHA-256 of the leaf certificate (DER), lowercase hex.
        cert_sha256: String,
        /// `notAfter`, RFC 3339.
        not_after: String,
        /// The SAN host names it covers.
        covers: Vec<HostNameValue>,
        /// A chain file was supplied.
        chain: bool,
    },
    /// Test / development only — never OPERATIONAL.
    SelfSignedTest,
}

/// A person or identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    /// Full name.
    pub name: PersonName,
    /// E-mail.
    pub email: EmailAddress,
}

/// `bootstrap-admin`'s two identities: the institutional person and the
/// privileged identity linked to them. No password: the Core issues one
/// temporary credential.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FirstAdmin {
    /// The person who answers.
    pub person: Identity,
    /// What executes, with platform authority.
    pub privileged: Identity,
}

/// Everything the operator configured.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstallationConfiguration {
    /// I08.
    pub instance_name: InstanceName,
    /// I09.
    pub distributions: Distributions,
    /// I10.
    pub endpoints: AccessEndpoints,
    /// I11.
    pub tls: TlsPlan,
    /// I12.
    pub admin: FirstAdmin,
}

impl InstallationConfiguration {
    /// The cross-field rules.
    ///
    /// # Errors
    ///
    /// The first [`ConfigurationError`].
    pub fn validate(&self) -> Result<(), ConfigurationError> {
        let mut hosts: Vec<&str> = vec![self.endpoints.canonical.as_str()];
        for b in &self.endpoints.bound {
            if b.host == self.endpoints.canonical {
                return Err(ConfigurationError::EndpointIsCanonical);
            }
            if hosts.contains(&b.host.as_str()) {
                return Err(ConfigurationError::EndpointDuplicate);
            }
            hosts.push(b.host.as_str());
            if !self.distributions.contains(b.distribution) {
                return Err(ConfigurationError::EndpointDistributionNotEnabled);
            }
        }
        let a = &self.admin;
        if a.person.email.same_as(&a.privileged.email)
            || a.person
                .name
                .as_str()
                .eq_ignore_ascii_case(a.privileged.name.as_str())
        {
            return Err(ConfigurationError::AdminNotDistinct);
        }
        if let TlsPlan::OperatorSupplied { covers, .. } = &self.tls {
            if !hosts
                .iter()
                .all(|h| covers.iter().any(|c| c.as_str() == *h))
            {
                return Err(ConfigurationError::TlsDoesNotCover);
            }
        }
        Ok(())
    }
}

/// The firewall managers the Installer may change: ufw only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FirewallManager {
    /// Uncomplicated Firewall.
    Ufw,
}

/// Transport protocol of a firewall rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Proto {
    /// TCP.
    Tcp,
}

/// A change to the server that is not Ocinye itself — shown on I13, part of
/// the seal, journalled. The bootstrap maps each to a fixed argv.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum SystemChange {
    /// Ubuntu packages from the Ubuntu archive (`ca-certificates`, `curl`).
    EnsurePackages {
        /// Package names (closed set, see [`ENSURABLE_PACKAGES`]).
        packages: Vec<String>,
    },
    /// Docker Engine and the compose plugin from Docker's official apt
    /// repository, key pinned by fingerprint.
    InstallDockerFromOfficialRepo {
        /// `ubuntu-24.04`.
        target: String,
        /// dpkg architecture.
        arch: Arch,
        /// Exactly the manifest's packages.
        packages: Vec<String>,
        /// The manifest's fingerprint.
        repo_key_fingerprint: String,
    },
    /// An Ocinye-owned ufw allow rule.
    FirewallAllow {
        /// ufw.
        manager: FirewallManager,
        /// 80 or 443.
        port: u16,
        /// tcp.
        proto: Proto,
        /// Always `ocinye`.
        comment: String,
    },
}

/// The only Ubuntu packages the Installer may add besides Docker's.
pub const ENSURABLE_PACKAGES: [&str; 2] = ["ca-certificates", "curl"];

impl SystemChange {
    /// Is this change one the contract allows, with exactly the allowed
    /// values? The bootstrap re-checks every change before mapping it to argv.
    #[must_use]
    pub fn is_allowed(&self, manifest: &ReleaseManifest) -> bool {
        match self {
            Self::EnsurePackages { packages } => {
                !packages.is_empty()
                    && packages
                        .iter()
                        .all(|p| ENSURABLE_PACKAGES.contains(&p.as_str()))
            }
            Self::InstallDockerFromOfficialRepo {
                target,
                arch,
                packages,
                repo_key_fingerprint,
            } => {
                target == crate::SUPPORTED_TARGET
                    && *arch == manifest.target.arch
                    && *packages == manifest.prerequisites.docker_packages
                    && *repo_key_fingerprint == manifest.prerequisites.docker_repo_key_fingerprint
            }
            Self::FirewallAllow {
                manager: FirewallManager::Ufw,
                port,
                proto: Proto::Tcp,
                comment,
            } => matches!(port, 80 | 443) && comment == "ocinye",
        }
    }
}

/// A short, non-secret hardware summary (the receipt has the full facts).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HardwareSummary {
    /// Online vCPUs.
    pub threads: u32,
    /// MemTotal in MB.
    pub memory_mb: u64,
    /// Accelerators detected.
    pub gpus: u8,
    /// Informational readiness.
    pub compute_readiness: ComputeReadiness,
    /// Always `Off` in D011.
    pub provider_mode: ProviderMode,
}

impl HardwareSummary {
    /// From discovery.
    #[must_use]
    pub fn of(h: &HardwareCapabilities) -> Self {
        let gpus = match &h.gpu {
            crate::hardware::GpuDiscovery::GpuDetected { gpus }
            | crate::hardware::GpuDiscovery::GpuRuntimeUnavailable { gpus } => {
                u8::try_from(gpus.len()).unwrap_or(u8::MAX)
            }
            _ => 0,
        };
        Self {
            threads: h.cpu.threads,
            memory_mb: h.memory.total_bytes / (1024 * 1024),
            gpus,
            compute_readiness: ComputeReadiness::of(&h.gpu),
            provider_mode: ProviderMode::Off,
        }
    }
}

/// One phase of the plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlannedPhase {
    /// Which.
    pub phase: PhaseId,
    /// Nothing to do (P04, P08 only).
    pub skip: bool,
}

/// The plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstallationPlan {
    /// This plan.
    pub plan_id: PlanId,
    /// The installation it belongs to (stable across resume).
    pub installation_id: InstallationId,
    /// RFC 3339.
    pub created_at: String,
    /// The release.
    pub release: ReleaseIdentity,
    /// The server.
    pub target: TargetIdentity,
    /// The configuration.
    pub configuration: InstallationConfiguration,
    /// The changes to the server outside Ocinye.
    pub system_changes: Vec<SystemChange>,
    /// The preflight verdicts the plan was reviewed against (P03 compares).
    pub preflight: Vec<(PreflightCheckId, CheckStatus)>,
    /// Hardware summary.
    pub hardware: HardwareSummary,
    /// The 16 phases.
    pub phases: Vec<PlannedPhase>,
    /// The seal (empty before [`InstallationPlan::seal`]).
    pub plan_sha256: String,
}

/// Why a plan was refused by the executing side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PlanRefusal {
    /// The seal does not match the content: altered after confirmation.
    SealMismatch,
    /// Not the plan the operator confirmed (`plan_id` / hash differ).
    NotConfirmed,
    /// A change the contract does not allow.
    ChangeNotAllowed,
    /// The configuration breaks a rule.
    Configuration,
    /// The phases are not the 16, in order.
    Phases,
    /// For another release or another server.
    WrongTarget,
}

impl InstallationPlan {
    /// The phases implied by the system changes.
    #[must_use]
    pub fn phases_for(changes: &[SystemChange]) -> Vec<PlannedPhase> {
        let p04 = changes.iter().any(|c| {
            matches!(
                c,
                SystemChange::EnsurePackages { .. }
                    | SystemChange::InstallDockerFromOfficialRepo { .. }
            )
        });
        let p08 = changes
            .iter()
            .any(|c| matches!(c, SystemChange::FirewallAllow { .. }));
        PhaseId::ALL
            .iter()
            .map(|p| PlannedPhase {
                phase: *p,
                skip: (*p == PhaseId::P04 && !p04) || (*p == PhaseId::P08 && !p08),
            })
            .collect()
    }

    fn digest(&self) -> String {
        let mut unsealed = self.clone();
        unsealed.plan_sha256 = String::new();
        canonical::sha256_of(&unsealed).expect("a plan has integers only")
    }

    /// Seal: compute `plan_sha256`.
    #[must_use]
    pub fn seal(mut self) -> Self {
        self.plan_sha256 = self.digest();
        self
    }

    /// The seal matches the content.
    #[must_use]
    pub fn seal_holds(&self) -> bool {
        !self.plan_sha256.is_empty() && self.plan_sha256 == self.digest()
    }

    /// Everything the executing side checks before the first mutation.
    ///
    /// # Errors
    ///
    /// The first [`PlanRefusal`].
    pub fn check(&self, manifest: &ReleaseManifest) -> Result<(), PlanRefusal> {
        if !self.seal_holds() {
            return Err(PlanRefusal::SealMismatch);
        }
        if self.release != ReleaseIdentity::of(manifest) {
            return Err(PlanRefusal::WrongTarget);
        }
        if self.configuration.validate().is_err() {
            return Err(PlanRefusal::Configuration);
        }
        if !self.system_changes.iter().all(|c| c.is_allowed(manifest)) {
            return Err(PlanRefusal::ChangeNotAllowed);
        }
        if self.phases != Self::phases_for(&self.system_changes) {
            return Err(PlanRefusal::Phases);
        }
        Ok(())
    }

    /// Skipped?
    #[must_use]
    pub fn skips(&self, phase: PhaseId) -> bool {
        self.phases.iter().any(|p| p.phase == phase && p.skip)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::hardware::*;
    use crate::manifest::tests::sample;

    pub(crate) fn config() -> InstallationConfiguration {
        let h = |s: &str| HostNameValue::parse(s).unwrap();
        InstallationConfiguration {
            instance_name: InstanceName::parse("Empresa Exemplo").unwrap(),
            distributions: Distributions::new(vec![Distribution::Business, Distribution::Research])
                .unwrap(),
            endpoints: AccessEndpoints {
                canonical: h("os.empresa.test"),
                bound: vec![
                    BoundEndpoint {
                        host: h("business.empresa.test"),
                        distribution: Distribution::Business,
                    },
                    BoundEndpoint {
                        host: h("research.empresa.test"),
                        distribution: Distribution::Research,
                    },
                ],
            },
            tls: TlsPlan::SelfSignedTest,
            admin: FirstAdmin {
                person: Identity {
                    name: PersonName::parse("Ana Exemplo").unwrap(),
                    email: EmailAddress::parse("ana@empresa.test").unwrap(),
                },
                privileged: Identity {
                    name: PersonName::parse("Ana Exemplo (Admin)").unwrap(),
                    email: EmailAddress::parse("ana.admin@empresa.test").unwrap(),
                },
            },
        }
    }

    pub(crate) fn hardware() -> HardwareCapabilities {
        HardwareCapabilities {
            cpu: CpuCapabilities {
                model: None,
                arch: Some(Arch::Arm64),
                cores: None,
                threads: 2,
                features: vec![],
            },
            memory: MemoryCapabilities {
                total_bytes: 4 << 30,
                available_bytes: 3 << 30,
                swap_bytes: 0,
            },
            storage: StorageCapabilities {
                path: "/".into(),
                free_bytes: 30 << 30,
                total_bytes: 40 << 30,
                filesystem: Some("ext4".into()),
            },
            network: NetworkCapabilities {
                interfaces_up: 1,
                max_link_mbps: None,
                ipv6: false,
            },
            gpu: GpuDiscovery::NoGpu {
                basic_display: None,
            },
            security: ComputeSecurityCapability {
                confidential: vec![],
                tpm: Evidence::NotDetected,
            },
        }
    }

    pub(crate) fn plan() -> InstallationPlan {
        let m = sample();
        let changes = vec![
            SystemChange::InstallDockerFromOfficialRepo {
                target: "ubuntu-24.04".into(),
                arch: Arch::Arm64,
                packages: m.prerequisites.docker_packages.clone(),
                repo_key_fingerprint: m.prerequisites.docker_repo_key_fingerprint.clone(),
            },
            SystemChange::FirewallAllow {
                manager: FirewallManager::Ufw,
                port: 443,
                proto: Proto::Tcp,
                comment: "ocinye".into(),
            },
        ];
        InstallationPlan {
            plan_id: PlanId::parse("pl-5d0a910000000000").unwrap(),
            installation_id: InstallationId::parse("inst-7c41e20000000000").unwrap(),
            created_at: "2026-10-04T14:16:31Z".into(),
            release: ReleaseIdentity::of(&m),
            target: TargetIdentity {
                host: TargetHost::parse("192.0.2.10").unwrap(),
                port: 22,
                user: SshUser::parse("operador").unwrap(),
                host_key_algorithm: "ssh-ed25519".into(),
                host_key_sha256: "SHA256:EXEMPLO".into(),
            },
            configuration: config(),
            phases: InstallationPlan::phases_for(&changes),
            system_changes: changes,
            preflight: vec![(PreflightCheckId::PfOs, CheckStatus::Pass)],
            hardware: HardwareSummary::of(&hardware()),
            plan_sha256: String::new(),
        }
        .seal()
    }

    #[test]
    fn o_plano_selado_confere_e_qualquer_alteracao_o_quebra() {
        let m = sample();
        let p = plan();
        assert!(p.seal_holds());
        p.check(&m).unwrap();

        let mut tampered = p.clone();
        tampered.configuration.instance_name = InstanceName::parse("Empresa Exemplo.").unwrap();
        assert_eq!(tampered.check(&m), Err(PlanRefusal::SealMismatch));

        let mut more = p.clone();
        more.system_changes.push(SystemChange::FirewallAllow {
            manager: FirewallManager::Ufw,
            port: 22,
            proto: Proto::Tcp,
            comment: "ocinye".into(),
        });
        assert_eq!(more.check(&m), Err(PlanRefusal::SealMismatch));
        // Re-selado por quem o alterou: o selo confere, mas a mudança não é
        // permitida pelo contrato.
        let resealed = more.seal();
        assert_eq!(resealed.check(&m), Err(PlanRefusal::ChangeNotAllowed));
    }

    #[test]
    fn a_mesma_configuracao_da_o_mesmo_selo() {
        let a = plan();
        let json = serde_json::to_string_pretty(&a).unwrap();
        let b: InstallationPlan = serde_json::from_str(&json).unwrap();
        assert_eq!(a.plan_sha256, b.plan_sha256);
        assert!(b.seal_holds());
    }

    #[test]
    fn um_plano_para_outro_release_e_recusado() {
        let mut m = sample();
        m.release.build = BuildKind::Proof;
        assert_eq!(plan().check(&m), Err(PlanRefusal::WrongTarget));
    }

    #[test]
    fn so_as_mudancas_do_contrato_sao_permitidas() {
        let m = sample();
        assert!(!SystemChange::EnsurePackages {
            packages: vec!["openssh-server".into()]
        }
        .is_allowed(&m));
        assert!(!SystemChange::InstallDockerFromOfficialRepo {
            target: "debian-12".into(),
            arch: Arch::Arm64,
            packages: m.prerequisites.docker_packages.clone(),
            repo_key_fingerprint: m.prerequisites.docker_repo_key_fingerprint.clone(),
        }
        .is_allowed(&m));
        assert!(!SystemChange::InstallDockerFromOfficialRepo {
            target: "ubuntu-24.04".into(),
            arch: Arch::Arm64,
            packages: m.prerequisites.docker_packages.clone(),
            repo_key_fingerprint: "0".repeat(40),
        }
        .is_allowed(&m));
        assert!(!SystemChange::FirewallAllow {
            manager: FirewallManager::Ufw,
            port: 80,
            proto: Proto::Tcp,
            comment: "other".into(),
        }
        .is_allowed(&m));
    }

    #[test]
    fn as_fases_sem_nada_a_fazer_saltam_se() {
        let phases = InstallationPlan::phases_for(&[]);
        assert_eq!(phases.len(), 16);
        assert!(phases.iter().any(|p| p.phase == PhaseId::P04 && p.skip));
        assert!(phases.iter().any(|p| p.phase == PhaseId::P08 && p.skip));
        assert_eq!(phases.iter().filter(|p| p.skip).count(), 2);
        assert_eq!(PhaseId::P11.class(), SafetyClass::D);
        assert_eq!(PhaseId::P11.cancel(), CancelPolicy::AfterTransaction);
        assert_eq!(PhaseId::P16.number(), 16);
    }

    #[test]
    fn a_configuracao_cumpre_as_regras_da_d010() {
        let mut c = config();
        c.validate().unwrap();
        c.endpoints.bound[0].host = c.endpoints.canonical.clone();
        assert_eq!(c.validate(), Err(ConfigurationError::EndpointIsCanonical));
        let mut c = config();
        c.endpoints.bound[1].distribution = Distribution::Personal;
        assert_eq!(
            c.validate(),
            Err(ConfigurationError::EndpointDistributionNotEnabled)
        );
        let mut c = config();
        c.admin.privileged.email = EmailAddress::parse("ANA@empresa.test").unwrap();
        assert_eq!(c.validate(), Err(ConfigurationError::AdminNotDistinct));
        assert_eq!(
            Distributions::new(vec![]),
            Err(ConfigurationError::NoDistribution)
        );
        assert_eq!(
            Distributions::new(vec![Distribution::Research, Distribution::Research]),
            Err(ConfigurationError::DistributionsInvalid)
        );
        let mut c = config();
        c.tls = TlsPlan::OperatorSupplied {
            cert_sha256: "a".repeat(64),
            not_after: "2027-09-30T00:00:00Z".into(),
            covers: vec![HostNameValue::parse("os.empresa.test").unwrap()],
            chain: false,
        };
        assert_eq!(c.validate(), Err(ConfigurationError::TlsDoesNotCover));
    }
}
