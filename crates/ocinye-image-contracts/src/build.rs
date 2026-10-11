//! Image factory: build inputs, provenance, inventory, states and typed errors.
//!
//! Separate from the machine lifecycle (D013_STATE_MACHINE.md §1): an image is
//! built, verified, certified, signed, released and published; a machine
//! boots, is claimed and provisioned. One is never the other.

use serde::{Deserialize, Serialize};

use crate::manifest::{FileRef, ImageFormat, ImageProfile, OcinyeImageVersion, Sha256Hex};

/// Where a build ran.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BuildEnvironment {
    /// A developer machine (here: a disposable Lima builder VM).
    Local,
    /// Untrusted pull-request CI: no secrets.
    CiPr,
    /// Protected-branch CI: development key only.
    CiProtected,
    /// The release environment.
    Release,
}

/// Kinds of pinned input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildInputKind {
    /// The Ubuntu minimal cloud image.
    UbuntuBase,
    /// `snapshot.ubuntu.com` timestamp.
    AptSnapshot,
    /// Docker's apt repository.
    DockerRepo,
    /// The D011 release bundle.
    ReleaseBundle,
    /// An OCI image by digest.
    OciImage,
    /// The Rust toolchain.
    Toolchain,
    /// A build tool (QEMU, xorriso, syft, …).
    Tool,
}

/// One pinned input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildInput {
    /// Kind.
    pub kind: BuildInputKind,
    /// Serial, timestamp, version or reference.
    pub identity: String,
    /// Never empty for a release build.
    pub digest: Sha256Hex,
    /// `openpgp:<fingerprint>`, `apt-inrelease`, `manifest-key-fingerprint`,
    /// `oci-digest`, `lockfile`, `package-version`.
    pub trust: String,
}

/// One step result in the provenance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StepRecord {
    /// `B01`…`B17`.
    pub id: String,
    /// `completed`.
    pub status: String,
    /// Seconds (rounded).
    pub duration_s: u64,
}

/// Provenance: an Ocinye document; no SLSA or in-toto claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildProvenance {
    /// 1.
    pub schema: u32,
    /// Image.
    pub image: OcinyeImageVersion,
    /// Source commit (40 hex).
    pub source_commit: String,
    /// Whether the source tree was clean.
    pub source_tree_clean: bool,
    /// From the commit.
    pub source_date_epoch: i64,
    /// Every pinned input.
    pub inputs: Vec<BuildInput>,
    /// Builder version.
    pub builder_version: String,
    /// Builder commit.
    pub builder_commit: String,
    /// Where it ran.
    pub environment: BuildEnvironment,
    /// `local-<random>` or the CI run — never a hostname.
    pub execution_id: String,
    /// Step durations.
    pub steps: Vec<StepRecord>,
    /// Every output.
    pub outputs: Vec<FileRef>,
}

/// Inventory package class (BUILD_ONLY is not a class an image may contain).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ContentClass {
    /// Part of the supported Ubuntu system.
    BaseOs,
    /// The product needs it at runtime.
    OcinyeRuntimeRequired,
    /// Installation or first boot needs it.
    InstallationRequired,
    /// Local recovery needs it.
    RecoveryRequired,
    /// Justified, not required.
    Optional,
}

/// Package origin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PackageOrigin {
    /// Ubuntu archive (main/universe, release or updates pocket).
    UbuntuArchive,
    /// Ubuntu security pocket.
    UbuntuSecurity,
    /// Docker's repository (only the four D011 packages).
    DockerCe,
    /// Ocinye files.
    Ocinye,
}

/// One package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InventoryPackage {
    /// dpkg name.
    pub name: String,
    /// Exact version.
    pub version: String,
    /// dpkg architecture.
    pub architecture: String,
    /// Origin.
    pub origin: PackageOrigin,
    /// SHA-256 of the installed `.deb` (from the signed index).
    pub sha256: Sha256Hex,
    /// Class.
    pub class: ContentClass,
    /// Required for every class but BASE_OS.
    pub reason: Option<String>,
    /// `apt-mark hold`.
    pub held: bool,
}

/// Package inventory per profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageInventory {
    /// 1.
    pub schema: u32,
    /// Image.
    pub image: OcinyeImageVersion,
    /// Profile.
    pub profile: ImageProfile,
    /// Snapshot timestamp.
    pub apt_snapshot: String,
    /// Sorted by name.
    pub packages: Vec<InventoryPackage>,
}

/// The four packages D011 installs from Docker's repository [P, PD-04].
pub const DOCKER_PACKAGES: [&str; 4] = [
    "docker-ce",
    "docker-ce-cli",
    "containerd.io",
    "docker-compose-plugin",
];

/// Packages that are build tooling and must never be in an image.
pub const BUILD_ONLY_PACKAGES: [&str; 14] = [
    "build-essential",
    "gcc",
    "g++",
    "rustc",
    "cargo",
    "git",
    "syft",
    "libguestfs-tools",
    "xorriso",
    "qemu-system-x86",
    "qemu-utils",
    "squashfs-tools",
    "debootstrap",
    "minisign",
];

/// Packages removed before finalization when the base carries them.
pub const REMOVE_BEFORE_FINALIZATION: [&str; 5] = [
    "snapd",
    "linux-kvm",
    "linux-image-kvm",
    "popularity-contest",
    "apport",
];

/// Inventory rule violations (B12/B16).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "rule", rename_all = "snake_case")]
pub enum InventoryViolation {
    /// A build tool is in the image.
    BuildOnlyPackage {
        /// Package.
        name: String,
    },
    /// A non-BASE_OS package without a reason.
    MissingReason {
        /// Package.
        name: String,
    },
    /// A package that must be removed is present.
    MustBeRemoved {
        /// Package.
        name: String,
    },
    /// `docker-ce` origin for something that is not one of the four.
    UnexpectedDockerOrigin {
        /// Package.
        name: String,
    },
    /// A Docker package is not held.
    DockerNotHeld {
        /// Package.
        name: String,
    },
    /// Not sorted / duplicated.
    Order,
}

impl PackageInventory {
    /// The B12/B16 rules (D013_PACKAGE_INVENTORY_MODEL.md).
    #[must_use]
    pub fn violations(&self) -> Vec<InventoryViolation> {
        let mut v = Vec::new();
        if !self
            .packages
            .windows(2)
            .all(|w| (&w[0].name, &w[0].architecture) < (&w[1].name, &w[1].architecture))
        {
            v.push(InventoryViolation::Order);
        }
        for p in &self.packages {
            let name = p.name.clone();
            if BUILD_ONLY_PACKAGES.contains(&p.name.as_str()) {
                v.push(InventoryViolation::BuildOnlyPackage { name: name.clone() });
            }
            if REMOVE_BEFORE_FINALIZATION.contains(&p.name.as_str()) {
                v.push(InventoryViolation::MustBeRemoved { name: name.clone() });
            }
            if p.class != ContentClass::BaseOs && p.reason.as_deref().is_none_or(str::is_empty) {
                v.push(InventoryViolation::MissingReason { name: name.clone() });
            }
            let docker = DOCKER_PACKAGES.contains(&p.name.as_str());
            if p.origin == PackageOrigin::DockerCe && !docker {
                v.push(InventoryViolation::UnexpectedDockerOrigin { name: name.clone() });
            }
            if docker && !p.held {
                v.push(InventoryViolation::DockerNotHeld { name });
            }
        }
        v
    }
}

/// Publication channel (in the signed trusted comment, not in artifact bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    /// Development key only, always bannered.
    Development,
    /// Release key, before promotion.
    Candidate,
    /// Release key, promoted.
    Stable,
}

/// Image factory states.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ImageBuildState {
    /// B01/B02 pending.
    SourcePinned,
    /// Source and release verified.
    InputsVerified,
    /// Ubuntu base verified.
    BaseVerified,
    /// B04–B10 done.
    Assembled,
    /// B11 done.
    Sanitized,
    /// B12 done.
    Inspected,
    /// B13–B15 done.
    ArtifactsProduced,
    /// B16 done.
    MetadataGenerated,
    /// B17: artifacts match SHA256SUMS, inspection passed on each.
    ImageVerified,
    /// Every HARD certification gate passed.
    Certified,
    /// A signature over SHA256SUMS verifies.
    Signed {
        /// Channel.
        channel: Channel,
    },
    /// Release authorization recorded.
    ImageReleased,
    /// On the distribution site.
    Published,
    /// A newer revision exists.
    Superseded,
    /// Revoked.
    Revoked {
        /// Why.
        reason: String,
    },
    /// Terminal.
    BuildFailed {
        /// Why.
        error: ImageBuildError,
    },
    /// Terminal for this revision.
    CertificationFailed {
        /// Gate.
        gate: String,
    },
}

/// Typed build errors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "code", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ImageBuildError {
    /// B01: local changes.
    DirtySourceTree,
    /// B01: not a certified commit (release builds).
    SourceNotCertified,
    /// The requested target is not supported.
    UnsupportedArchitecture {
        /// Requested.
        arch: String,
    },
    /// B02.
    ReleaseManifestInvalid {
        /// Field.
        field: String,
    },
    /// B02.
    ReleaseArchMismatch,
    /// B03: `SHA256SUMS.gpg` did not verify against the pinned key.
    BaseSignatureInvalid,
    /// B03.
    BaseDigestMismatch {
        /// Expected.
        expected: String,
        /// Actual.
        actual: String,
    },
    /// B05–B08: a connection outside the allow-list.
    NetworkPolicyViolation {
        /// Host.
        host: String,
    },
    /// B06/B07.
    PackageInstallFailed {
        /// Package.
        package: String,
    },
    /// B07.
    RuntimeKeyMismatch,
    /// B08 / release builds: a tag without digest.
    FloatingImageReference {
        /// Reference.
        reference: String,
    },
    /// B08.
    OciDigestMismatch {
        /// Image.
        name: String,
    },
    /// B04–B10.
    ImageAssemblyFailed {
        /// Step.
        step: String,
    },
    /// B11.
    SanitizationFailed {
        /// Item.
        item: String,
    },
    /// B12.
    InspectionFailed {
        /// Check.
        check: String,
    },
    /// B13/B16.
    ManifestFailed {
        /// Field.
        field: String,
    },
    /// B16.
    SbomFailed {
        /// Reason.
        reason: String,
    },
    /// B15.
    ArtifactAssemblyFailed {
        /// Format.
        format: ImageFormat,
    },
    /// B17.
    ArtifactVerificationFailed {
        /// File.
        file: String,
    },
    /// B19.
    SigningFailed,
    /// A stable/public build was requested with a gate not satisfied.
    StableGateNotSatisfied {
        /// The gate.
        gate: StableGate,
    },
}

/// The gates a stable/public image needs (D013_SECURITY_MATRIX.md
/// §Publication). Each is checked by the builder before it agrees to build a
/// `release` image, and fails closed: none can be satisfied by a flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StableGate {
    /// D011 certified and merged (Phase B sync done).
    D011Certified,
    /// D013 Phase B synchronized with final D011 main.
    D011PhaseBSync,
    /// D013 final integration complete.
    D013FinalIntegration,
    /// Every runtime image identified by digest.
    ImmutableOciIdentities,
    /// A production release key is available in the release environment.
    ProductionSigning,
    /// Legal review recorded complete.
    LegalReview,
    /// The Redis runtime and redistribution decision is recorded.
    RedisDecision,
    /// Every HARD certification gate passed.
    Certification,
}

impl StableGate {
    /// Every gate, in order.
    pub const ALL: [Self; 8] = [
        Self::D011Certified,
        Self::D011PhaseBSync,
        Self::D013FinalIntegration,
        Self::ImmutableOciIdentities,
        Self::ProductionSigning,
        Self::LegalReview,
        Self::RedisDecision,
        Self::Certification,
    ];
}

/// The recorded state of each gate, read from the repository (not from flags).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StableGateRecord {
    /// D011_CERTIFIED.
    pub d011_certified: bool,
    /// Phase B done.
    pub d011_phase_b_sync: bool,
    /// D013 final integration.
    pub d013_final_integration: bool,
    /// All runtime images by digest.
    pub immutable_oci_identities: bool,
    /// Production key available.
    pub production_signing: bool,
    /// LEGAL_REVIEW_REQUIRED = FALSE recorded.
    pub legal_review: bool,
    /// Redis decision recorded.
    pub redis_decision: bool,
    /// Certification passed.
    pub certification: bool,
}

impl StableGateRecord {
    /// The first unsatisfied gate, if any.
    #[must_use]
    pub fn first_unsatisfied(&self) -> Option<StableGate> {
        StableGate::ALL.into_iter().find(|g| !match g {
            StableGate::D011Certified => self.d011_certified,
            StableGate::D011PhaseBSync => self.d011_phase_b_sync,
            StableGate::D013FinalIntegration => self.d013_final_integration,
            StableGate::ImmutableOciIdentities => self.immutable_oci_identities,
            StableGate::ProductionSigning => self.production_signing,
            StableGate::LegalReview => self.legal_review,
            StableGate::RedisDecision => self.redis_decision,
            StableGate::Certification => self.certification,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::tests::{h, version};

    fn pkg(
        name: &str,
        class: ContentClass,
        origin: PackageOrigin,
        held: bool,
        reason: Option<&str>,
    ) -> InventoryPackage {
        InventoryPackage {
            name: name.into(),
            version: "1".into(),
            architecture: "amd64".into(),
            origin,
            sha256: h('a'),
            class,
            reason: reason.map(Into::into),
            held,
        }
    }

    #[test]
    fn o_inventario_recusa_ferramentas_de_construcao_e_razoes_em_falta() {
        let mut inv = PackageInventory {
            schema: 1,
            image: version(),
            profile: ImageProfile::Virt,
            apt_snapshot: "20261001T000000Z".into(),
            packages: vec![
                pkg(
                    "containerd.io",
                    ContentClass::OcinyeRuntimeRequired,
                    PackageOrigin::DockerCe,
                    true,
                    Some("runtime"),
                ),
                pkg(
                    "systemd",
                    ContentClass::BaseOs,
                    PackageOrigin::UbuntuArchive,
                    false,
                    None,
                ),
            ],
        };
        assert!(inv.violations().is_empty());
        inv.packages.push(pkg(
            "xorriso",
            ContentClass::Optional,
            PackageOrigin::UbuntuArchive,
            false,
            None,
        ));
        inv.packages.push(pkg(
            "docker-extra",
            ContentClass::Optional,
            PackageOrigin::DockerCe,
            false,
            Some("x"),
        ));
        let v = inv.violations();
        assert!(v.contains(&InventoryViolation::BuildOnlyPackage {
            name: "xorriso".into()
        }));
        assert!(v.contains(&InventoryViolation::MissingReason {
            name: "xorriso".into()
        }));
        assert!(v.contains(&InventoryViolation::UnexpectedDockerOrigin {
            name: "docker-extra".into()
        }));
        assert!(v.contains(&InventoryViolation::Order));
    }

    #[test]
    fn os_portoes_estaveis_falham_fechados_hoje() {
        let today = StableGateRecord {
            d011_certified: false,
            d011_phase_b_sync: false,
            d013_final_integration: false,
            immutable_oci_identities: true,
            production_signing: false,
            legal_review: false,
            redis_decision: false,
            certification: false,
        };
        assert_eq!(today.first_unsatisfied(), Some(StableGate::D011Certified));
        let all = StableGateRecord {
            d011_certified: true,
            d011_phase_b_sync: true,
            d013_final_integration: true,
            immutable_oci_identities: true,
            production_signing: true,
            legal_review: true,
            redis_decision: false,
            certification: true,
        };
        assert_eq!(all.first_unsatisfied(), Some(StableGate::RedisDecision));
    }
}
