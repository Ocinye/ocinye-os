//! `OcinyeImageManifest` (published) and `ImageContentManifest` (embedded).
//!
//! Two manifests because an artifact cannot contain its own digest
//! (D013_IMAGE_MANIFEST.md). Both are hashed over the D011 canonical JSON
//! [P, PD-02]; signatures never cover pretty-printed JSON — they cover
//! `SHA256SUMS`, which lists the canonical files.

use ocinye_installer_contracts::canonical;
use serde::{Deserialize, Serialize};

use crate::is_lower_hex;

/// 64 lowercase hex.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Sha256Hex(pub String);

impl Sha256Hex {
    /// Whether it is 64 lowercase hex.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        is_lower_hex(&self.0, 64)
    }
}

/// `sha256:<64 hex>` — never a tag.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OciDigest(pub String);

impl OciDigest {
    /// Whether it is `sha256:` followed by 64 lowercase hex.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.0
            .strip_prefix("sha256:")
            .is_some_and(|h| is_lower_hex(h, 64))
    }
}

/// Release build or development build (D011 `build = proof` ⇒ development) [P, PD-01].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildKind {
    /// Built from a release bundle for publication.
    Release,
    /// Development / proof: never stable, always bannered.
    Development,
}

/// Architectures (D011 `Arch`) [P, PD-01].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Arch {
    /// x86-64: the intended production target (PROVISIONAL, PD-25).
    Amd64,
    /// aarch64: development only.
    Arm64,
}

impl Arch {
    /// The Debian/Ubuntu name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Amd64 => "amd64",
            Self::Arm64 => "arm64",
        }
    }
}

/// `ocinye-os-<release_id>-r<revision>[-dev]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OcinyeImageVersion {
    /// D011 release id, 12 hex [P, PD-01].
    pub release_id: String,
    /// ≥ 1; new bytes ⇒ new revision.
    pub revision: u32,
    /// Release or development.
    pub build_kind: BuildKind,
}

impl OcinyeImageVersion {
    /// The canonical name, `ocinye-os-<id>-r<N>[-dev]`.
    #[must_use]
    pub fn name(&self) -> String {
        format!(
            "{}-{}-r{}{}",
            crate::PRODUCT,
            self.release_id,
            self.revision,
            if self.build_kind == BuildKind::Development {
                "-dev"
            } else {
                ""
            }
        )
    }

    /// Artifact base name, `ocinye-os-<id>-r<N>[-dev]-<arch>`.
    #[must_use]
    pub fn artifact_stem(&self, arch: Arch) -> String {
        format!("{}-{}", self.name(), arch.as_str())
    }

    fn valid(&self) -> bool {
        is_lower_hex(&self.release_id, 12) && self.revision >= 1
    }
}

/// The pinned Ubuntu base.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UbuntuBaseIdentity {
    /// "24.04".
    pub release: String,
    /// "noble".
    pub codename: String,
    /// "minimal-cloudimg".
    pub variant: String,
    /// "YYYYMMDD" or "YYYYMMDD.n"; never "current"/"latest".
    pub serial: String,
    /// Architecture.
    pub arch: Arch,
    /// Path under cloud-images.ubuntu.com.
    pub url_path: String,
    /// SHA-256 of the image file.
    pub sha256: Sha256Hex,
    /// SHA-256 of the verified `SHA256SUMS` file.
    pub sums_sha256: Sha256Hex,
    /// OpenPGP fingerprint that signed `SHA256SUMS` (40 uppercase hex).
    pub sums_signing_key_fingerprint: String,
}

/// Whether an Ubuntu image serial is a pinned one (`YYYYMMDD[.n]`).
#[must_use]
pub fn is_pinned_serial(serial: &str) -> bool {
    let (date, point) = match serial.split_once('.') {
        Some((d, p)) => (d, Some(p)),
        None => (serial, None),
    };
    date.len() == 8
        && date.bytes().all(|b| b.is_ascii_digit())
        && point
            .is_none_or(|p| !p.is_empty() && p.len() <= 3 && p.bytes().all(|b| b.is_ascii_digit()))
}

impl UbuntuBaseIdentity {
    fn valid(&self) -> bool {
        self.release == "24.04"
            && self.codename == "noble"
            && self.variant == "minimal-cloudimg"
            && is_pinned_serial(&self.serial)
            && self.url_path.starts_with("minimal/releases/noble/release-")
            && self.url_path.contains(&format!("release-{}/", self.serial))
            && !self.url_path.contains("..")
            && self.sha256.is_valid()
            && self.sums_sha256.is_valid()
            && self.sums_signing_key_fingerprint.len() == 40
            && self
                .sums_signing_key_fingerprint
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'A'..=b'F').contains(&b))
    }
}

/// Artifact format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageFormat {
    /// Hybrid ISO: bare metal (USB, BMC virtual media), virtual CD.
    Iso,
    /// KVM/QEMU/Proxmox/OpenStack.
    Qcow2,
    /// Generic disk image (`.raw`, transported as `.raw.zst`).
    Raw,
}

/// Image profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageProfile {
    /// QCOW2/RAW: no `linux-firmware`.
    Virt,
    /// ISO payload: with `linux-firmware`.
    Metal,
}

/// Boot modes an artifact is built for (only tested ones may be claimed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BootMode {
    /// UEFI on x86-64.
    UefiX86_64,
    /// UEFI on aarch64 (development).
    UefiAarch64,
}

/// One published artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageArtifact {
    /// Format.
    pub format: ImageFormat,
    /// Profile (the ISO carries the metal payload).
    pub profile: ImageProfile,
    /// File name as published.
    pub file: String,
    /// SHA-256 of the published file.
    pub sha256: Sha256Hex,
    /// Bytes of the published file.
    pub bytes: u64,
    /// RAW only: digest of the uncompressed `.raw`.
    pub uncompressed_sha256: Option<Sha256Hex>,
    /// RAW only: size of the uncompressed `.raw`.
    pub uncompressed_bytes: Option<u64>,
    /// QCOW2/RAW: virtual disk size.
    pub virtual_bytes: Option<u64>,
    /// Boot modes built for.
    pub boot: Vec<BootMode>,
}

/// OCI image role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OciRole {
    /// One of the five release images (in the release bundle).
    Release,
    /// Proxy, cache, database, object store (by digest, D011 F-01).
    ThirdParty,
}

/// One preloaded OCI image.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OciImageRef {
    /// Compose name (repository, without tag).
    pub name: String,
    /// Role.
    pub role: OciRole,
    /// Reference as Compose pulls it — `repo:tag@sha256:…` for third-party
    /// images, `repo:<release id>` for release images.
    pub reference: String,
    /// Content identity: the manifest digest for third-party images, the
    /// image id for release images (they never pass through a registry).
    pub digest: OciDigest,
    /// Docker image id after load.
    pub image_id: OciDigest,
}

/// Exact runtime versions (apt) [P, PD-04].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimePins {
    /// Exact apt version of `docker-ce`.
    pub docker_ce: String,
    /// Exact apt version of `docker-ce-cli`.
    pub docker_ce_cli: String,
    /// Exact apt version of `containerd.io`.
    pub containerd_io: String,
    /// Exact apt version of `docker-compose-plugin`.
    pub docker_compose_plugin: String,
    /// Equals release manifest `prerequisites.docker_repo_key_fingerprint` [P].
    pub repo_key_fingerprint: String,
}

/// SBOM reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SbomReference {
    /// Always `spdx-2.3+json`.
    pub format: String,
    /// File.
    pub file: String,
    /// SHA-256.
    pub sha256: Sha256Hex,
}

/// The only SBOM format (D013_SBOM_MODEL.md).
pub const SBOM_FORMAT: &str = "spdx-2.3+json";

/// A file + digest reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileRef {
    /// File name relative to the output directory.
    pub file: String,
    /// SHA-256.
    pub sha256: Sha256Hex,
}

/// Per-profile facts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileFacts {
    /// Profile.
    pub name: ImageProfile,
    /// Digest of its `ImageContentManifest`.
    pub content_sha256: Sha256Hex,
    /// Digest of its `PackageInventory`.
    pub package_inventory_sha256: Sha256Hex,
    /// Installed root filesystem usage.
    pub installed_bytes: u64,
}

/// Source identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceRef {
    /// 40 hex.
    pub commit: String,
    /// The tree had no local changes. A release build requires `true`.
    pub tree_clean: bool,
    /// From the commit.
    pub source_date_epoch: i64,
}

/// Release reference [P, PD-01].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseRef {
    /// 12 hex.
    pub id: String,
    /// Canonical digest of the D011 `MANIFEST.json`.
    pub manifest_sha256: Sha256Hex,
    /// The static bootstrap.
    pub bootstrap_sha256: Sha256Hex,
}

/// Builder identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuilderRef {
    /// `ocinye-image-builder`.
    pub name: String,
    /// Builder version.
    pub version: String,
    /// Commit the builder was built from.
    pub commit: String,
    /// Where it ran.
    pub environment: crate::build::BuildEnvironment,
}

/// Compatibility.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageCompatibility {
    /// D011 bootstrap protocol [P, PD-05].
    pub bootstrap_protocol: u16,
    /// [`crate::CLAIM_PROTOCOL`].
    pub claim_protocol: u16,
    /// Oldest Installer that understands this image (E-01…E-06).
    pub installer_min: String,
    /// Receipt schema with `image_source` (E-05) [P, PD-08].
    pub d011_receipt_schema_min: u32,
}

/// The published manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OcinyeImageManifest {
    /// [`crate::IMAGE_MANIFEST_SCHEMA`].
    pub schema: u32,
    /// [`crate::PRODUCT`].
    pub product: String,
    /// Image identity.
    pub image: OcinyeImageVersion,
    /// Source.
    pub source: SourceRef,
    /// Release [P].
    pub release: ReleaseRef,
    /// Ubuntu base.
    pub ubuntu_base: UbuntuBaseIdentity,
    /// `YYYYMMDDTHHMMSSZ`.
    pub apt_snapshot: String,
    /// Target architecture.
    pub architecture: Arch,
    /// Per profile.
    pub profiles: Vec<ProfileFacts>,
    /// Every artifact.
    pub artifacts: Vec<ImageArtifact>,
    /// Every preloaded OCI image.
    pub oci_images: Vec<OciImageRef>,
    /// Exact runtime versions.
    pub runtime: RuntimePins,
    /// SBOM.
    pub sbom: SbomReference,
    /// Inventories, one per profile.
    pub package_inventory: Vec<FileRef>,
    /// Provenance.
    pub provenance: FileRef,
    /// Builder.
    pub builder: BuilderRef,
    /// Compatibility.
    pub compatibility: ImageCompatibility,
    /// RFC 3339 from `source_date_epoch` (never wall clock).
    pub created_at: String,
}

/// One embedded release file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentFile {
    /// Path relative to the release root (`/usr/lib/ocinye/release/<id>/`).
    pub path: String,
    /// SHA-256.
    pub sha256: Sha256Hex,
    /// Size.
    pub bytes: u64,
}

/// The embedded manifest (cannot contain the artifact digest).
///
/// Shape of the reference implementation in the Design package (which the
/// JSON schemas there follow); the prose in D013_IMAGE_MANIFEST.md nests
/// `release`/`firstboot` — recorded in CODE_FEEDBACK.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageContentManifest {
    /// [`crate::IMAGE_MANIFEST_SCHEMA`].
    pub schema: u32,
    /// Image identity.
    pub image: OcinyeImageVersion,
    /// Profile.
    pub profile: ImageProfile,
    /// Architecture.
    pub architecture: Arch,
    /// Release id [P].
    pub release_id: String,
    /// Canonical digest of the release `MANIFEST.json` [P].
    pub release_manifest_sha256: Sha256Hex,
    /// Files under the release root, sorted by path.
    pub release_files: Vec<ContentFile>,
    /// Preloaded OCI images, sorted by name.
    pub oci_images: Vec<OciImageRef>,
    /// Digest of the profile's package inventory.
    pub package_inventory_sha256: Sha256Hex,
    /// `ocinye-firstboot` version.
    pub firstboot_version: String,
    /// [`crate::CLAIM_PROTOCOL`].
    pub claim_protocol: u16,
}

/// Fail-closed validation result, naming the first bad field (as D011
/// `InvalidManifest`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvalidImageManifest {
    /// Dotted path of the offending field.
    pub field: String,
}

fn bad(field: &str) -> InvalidImageManifest {
    InvalidImageManifest {
        field: field.to_owned(),
    }
}

/// A safe file name in an output directory: no separators, no dot-dot.
#[must_use]
pub fn is_safe_file_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 200
        && !name.starts_with('.')
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}

/// A safe relative path (for content files): segments of safe names.
#[must_use]
pub fn is_safe_relative_path(path: &str) -> bool {
    !path.is_empty() && !path.starts_with('/') && path.split('/').all(is_safe_file_name)
}

fn valid_oci(o: &OciImageRef) -> bool {
    !o.name.is_empty()
        && o.name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._-/:".contains(&b))
        && o.digest.is_valid()
        && o.image_id.is_valid()
        && match o.role {
            // A registry image is pulled by its manifest digest.
            OciRole::ThirdParty => o.reference.ends_with(&format!("@{}", o.digest.0)),
            // A release image never passes through a registry: its tag is the
            // release id and its identity the image id.
            OciRole::Release => !o.reference.contains('@') && o.digest == o.image_id,
        }
}

impl OcinyeImageManifest {
    /// Parse and validate.
    ///
    /// # Errors
    ///
    /// The first offending field.
    pub fn parse(bytes: &[u8]) -> Result<Self, InvalidImageManifest> {
        let m: Self = serde_json::from_slice(bytes).map_err(|_| bad("$"))?;
        m.validate()?;
        Ok(m)
    }

    /// Fail-closed validation.
    ///
    /// # Errors
    ///
    /// The first offending field.
    pub fn validate(&self) -> Result<(), InvalidImageManifest> {
        if self.schema != crate::IMAGE_MANIFEST_SCHEMA {
            return Err(bad("schema"));
        }
        if self.product != crate::PRODUCT {
            return Err(bad("product"));
        }
        if !self.image.valid() {
            return Err(bad("image"));
        }
        if !is_lower_hex(&self.source.commit, 40) {
            return Err(bad("source.commit"));
        }
        // A release image is built only from a clean tree.
        if self.image.build_kind == BuildKind::Release && !self.source.tree_clean {
            return Err(bad("source.tree_clean"));
        }
        if self.release.id != self.image.release_id
            || !self.release.manifest_sha256.is_valid()
            || !self.release.bootstrap_sha256.is_valid()
            || !self.source.commit.starts_with(&self.release.id)
        {
            return Err(bad("release"));
        }
        if !self.ubuntu_base.valid() || self.ubuntu_base.arch != self.architecture {
            return Err(bad("ubuntu_base"));
        }
        if !is_apt_snapshot(&self.apt_snapshot) {
            return Err(bad("apt_snapshot"));
        }
        if self.profiles.is_empty()
            || self
                .profiles
                .iter()
                .any(|p| !p.content_sha256.is_valid() || !p.package_inventory_sha256.is_valid())
        {
            return Err(bad("profiles"));
        }
        let stem = self.image.artifact_stem(self.architecture);
        for a in &self.artifacts {
            let ext = match a.format {
                ImageFormat::Iso => ".iso",
                ImageFormat::Qcow2 => ".qcow2",
                ImageFormat::Raw => ".raw.zst",
            };
            if a.file != format!("{stem}{ext}")
                || !a.sha256.is_valid()
                || a.boot.is_empty()
                || !self.profiles.iter().any(|p| p.name == a.profile)
            {
                return Err(bad("artifacts"));
            }
            match a.format {
                ImageFormat::Raw => {
                    if !a
                        .uncompressed_sha256
                        .as_ref()
                        .is_some_and(Sha256Hex::is_valid)
                        || a.uncompressed_bytes.is_none()
                        || a.virtual_bytes.is_none()
                    {
                        return Err(bad("artifacts.raw"));
                    }
                }
                ImageFormat::Qcow2 => {
                    if a.virtual_bytes.is_none() || a.uncompressed_sha256.is_some() {
                        return Err(bad("artifacts.qcow2"));
                    }
                }
                ImageFormat::Iso => {
                    if a.virtual_bytes.is_some() || a.uncompressed_sha256.is_some() {
                        return Err(bad("artifacts.iso"));
                    }
                }
            }
        }
        if self.oci_images.iter().any(|o| !valid_oci(o)) {
            return Err(bad("oci_images"));
        }
        if self.runtime.repo_key_fingerprint.len() != 40 {
            return Err(bad("runtime.repo_key_fingerprint"));
        }
        if self.sbom.format != SBOM_FORMAT
            || !is_safe_relative_path(&self.sbom.file)
            || !self.sbom.sha256.is_valid()
        {
            return Err(bad("sbom"));
        }
        if self.package_inventory.is_empty()
            || self
                .package_inventory
                .iter()
                .chain(std::iter::once(&self.provenance))
                .any(|f| !is_safe_relative_path(&f.file) || !f.sha256.is_valid())
        {
            return Err(bad("package_inventory"));
        }
        if self.builder.name != "ocinye-image-builder" || !is_lower_hex(&self.builder.commit, 40) {
            return Err(bad("builder"));
        }
        if self.compatibility.claim_protocol != crate::CLAIM_PROTOCOL {
            return Err(bad("compatibility.claim_protocol"));
        }
        let expected = chrono::DateTime::from_timestamp(self.source.source_date_epoch, 0)
            .map(|t| t.to_rfc3339_opts(chrono::SecondsFormat::Secs, true));
        if expected.as_deref() != Some(self.created_at.as_str()) {
            return Err(bad("created_at"));
        }
        Ok(())
    }

    /// SHA-256 of the canonical bytes.
    ///
    /// # Panics
    ///
    /// Never: the type has no floats.
    #[must_use]
    pub fn sha256(&self) -> String {
        canonical::sha256_of(self).expect("integers only")
    }

    /// Canonical bytes.
    ///
    /// # Panics
    ///
    /// Never: the type has no floats.
    #[must_use]
    pub fn to_canonical(&self) -> String {
        canonical::to_canonical(self).expect("integers only")
    }
}

/// `YYYYMMDDTHHMMSSZ`.
#[must_use]
pub fn is_apt_snapshot(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 16
        && b[8] == b'T'
        && b[15] == b'Z'
        && b[..8].iter().chain(&b[9..15]).all(u8::is_ascii_digit)
}

impl ImageContentManifest {
    /// Parse and validate.
    ///
    /// # Errors
    ///
    /// The first offending field.
    pub fn parse(bytes: &[u8]) -> Result<Self, InvalidImageManifest> {
        let m: Self = serde_json::from_slice(bytes).map_err(|_| bad("$"))?;
        m.validate()?;
        Ok(m)
    }

    /// Fail-closed validation.
    ///
    /// # Errors
    ///
    /// The first offending field.
    pub fn validate(&self) -> Result<(), InvalidImageManifest> {
        if self.schema != crate::IMAGE_MANIFEST_SCHEMA {
            return Err(bad("schema"));
        }
        if !self.image.valid() {
            return Err(bad("image"));
        }
        if self.release_id != self.image.release_id || !self.release_manifest_sha256.is_valid() {
            return Err(bad("release_id"));
        }
        if self.release_files.is_empty()
            || self
                .release_files
                .iter()
                .any(|f| !is_safe_relative_path(&f.path) || !f.sha256.is_valid())
            || !self.release_files.windows(2).all(|w| w[0].path < w[1].path)
        {
            return Err(bad("release_files"));
        }
        if self.oci_images.iter().any(|o| !valid_oci(o))
            || !self.oci_images.windows(2).all(|w| w[0].name < w[1].name)
        {
            return Err(bad("oci_images"));
        }
        if !self.package_inventory_sha256.is_valid() {
            return Err(bad("package_inventory_sha256"));
        }
        if self.claim_protocol != crate::CLAIM_PROTOCOL {
            return Err(bad("claim_protocol"));
        }
        Ok(())
    }

    /// SHA-256 of the canonical bytes (`content_sha256`).
    ///
    /// # Panics
    ///
    /// Never: the type has no floats.
    #[must_use]
    pub fn sha256(&self) -> String {
        canonical::sha256_of(self).expect("integers only")
    }

    /// Canonical bytes.
    ///
    /// # Panics
    ///
    /// Never: the type has no floats.
    #[must_use]
    pub fn to_canonical(&self) -> String {
        canonical::to_canonical(self).expect("integers only")
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn h(c: char) -> Sha256Hex {
        Sha256Hex(c.to_string().repeat(64))
    }

    pub(crate) fn version() -> OcinyeImageVersion {
        OcinyeImageVersion {
            release_id: "3f9c2a7d1e04".into(),
            revision: 2,
            build_kind: BuildKind::Development,
        }
    }

    pub(crate) fn content() -> ImageContentManifest {
        ImageContentManifest {
            schema: 1,
            image: version(),
            profile: ImageProfile::Virt,
            architecture: Arch::Amd64,
            release_id: "3f9c2a7d1e04".into(),
            release_manifest_sha256: h('e'),
            release_files: vec![
                ContentFile {
                    path: "MANIFEST.json".into(),
                    sha256: h('1'),
                    bytes: 10,
                },
                ContentFile {
                    path: "install/ocinye".into(),
                    sha256: h('2'),
                    bytes: 20,
                },
            ],
            oci_images: vec![OciImageRef {
                name: "nginx".into(),
                role: OciRole::ThirdParty,
                reference: format!("nginx:1.30.5-alpine@sha256:{}", "a".repeat(64)),
                digest: OciDigest(format!("sha256:{}", "a".repeat(64))),
                image_id: OciDigest(format!("sha256:{}", "b".repeat(64))),
            }],
            package_inventory_sha256: h('8'),
            firstboot_version: "0.1.0".into(),
            claim_protocol: 1,
        }
    }

    pub(crate) fn manifest() -> OcinyeImageManifest {
        let v = version();
        let stem = v.artifact_stem(Arch::Amd64);
        OcinyeImageManifest {
            schema: 1,
            product: "ocinye-os".into(),
            image: v,
            source: SourceRef {
                commit: format!("3f9c2a7d1e04{}", "b".repeat(28)),
                tree_clean: true,
                source_date_epoch: 1_790_726_400,
            },
            release: ReleaseRef { id: "3f9c2a7d1e04".into(), manifest_sha256: h('e'), bootstrap_sha256: h('f') },
            ubuntu_base: UbuntuBaseIdentity {
                release: "24.04".into(),
                codename: "noble".into(),
                variant: "minimal-cloudimg".into(),
                serial: "20261001".into(),
                arch: Arch::Amd64,
                url_path: "minimal/releases/noble/release-20261001/ubuntu-24.04-minimal-cloudimg-amd64.img".into(),
                sha256: h('0'),
                sums_sha256: h('9'),
                sums_signing_key_fingerprint: "D2EB44626FDDC30B513D5BB71A5D6C4C7DB87C81".into(),
            },
            apt_snapshot: "20261001T000000Z".into(),
            architecture: Arch::Amd64,
            profiles: vec![ProfileFacts {
                name: ImageProfile::Virt,
                content_sha256: h('7'),
                package_inventory_sha256: h('8'),
                installed_bytes: 3_900_000_000,
            }],
            artifacts: vec![
                ImageArtifact {
                    format: ImageFormat::Qcow2,
                    profile: ImageProfile::Virt,
                    file: format!("{stem}.qcow2"),
                    sha256: h('3'),
                    bytes: 1,
                    uncompressed_sha256: None,
                    uncompressed_bytes: None,
                    virtual_bytes: Some(24 << 30),
                    boot: vec![BootMode::UefiX86_64],
                },
                ImageArtifact {
                    format: ImageFormat::Raw,
                    profile: ImageProfile::Virt,
                    file: format!("{stem}.raw.zst"),
                    sha256: h('4'),
                    bytes: 1,
                    uncompressed_sha256: Some(h('5')),
                    uncompressed_bytes: Some(24 << 30),
                    virtual_bytes: Some(24 << 30),
                    boot: vec![BootMode::UefiX86_64],
                },
            ],
            oci_images: content().oci_images,
            runtime: RuntimePins {
                docker_ce: "5:29.8.2-1~ubuntu.24.04~noble".into(),
                docker_ce_cli: "5:29.8.2-1~ubuntu.24.04~noble".into(),
                containerd_io: "2.3.6-1~ubuntu.24.04~noble".into(),
                docker_compose_plugin: "5.6.0-1~ubuntu.24.04~noble".into(),
                repo_key_fingerprint: "9DC858229FC7DD38854AE2D88D81803C0EBFCD88".into(),
            },
            sbom: SbomReference { format: SBOM_FORMAT.into(), file: "sbom/x.spdx.json".into(), sha256: h('a') },
            package_inventory: vec![FileRef { file: "inventory/packages-virt.json".into(), sha256: h('8') }],
            provenance: FileRef { file: "provenance/build-provenance.json".into(), sha256: h('c') },
            builder: BuilderRef {
                name: "ocinye-image-builder".into(),
                version: "0.1.0".into(),
                commit: "c".repeat(40),
                environment: crate::build::BuildEnvironment::Local,
            },
            compatibility: ImageCompatibility {
                bootstrap_protocol: 1,
                claim_protocol: 1,
                installer_min: "0.1.0".into(),
                d011_receipt_schema_min: 2,
            },
            created_at: "2026-09-30T00:00:00Z".into(),
        }
    }

    #[test]
    fn um_manifesto_valido_passa_e_e_estavel_na_forma_canonica() {
        let m = manifest();
        m.validate().unwrap();
        let bytes = m.to_canonical();
        let back = OcinyeImageManifest::parse(bytes.as_bytes()).unwrap();
        assert_eq!(back, m);
        assert_eq!(
            back.to_canonical(),
            bytes,
            "round-trip identical bytes (R1)"
        );
        // Field order in the input does not change the canonical form.
        let mut v: serde_json::Value = serde_json::from_str(&bytes).unwrap();
        let obj = v.as_object_mut().unwrap();
        let pairs: Vec<_> = obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        let rev: serde_json::Map<_, _> = pairs.into_iter().rev().collect();
        let reparsed: OcinyeImageManifest =
            serde_json::from_value(serde_json::Value::Object(rev)).unwrap();
        assert_eq!(reparsed.sha256(), m.sha256());
    }

    #[test]
    fn campos_desconhecidos_e_duplicados_sao_recusados() {
        let mut v: serde_json::Value = serde_json::from_str(&manifest().to_canonical()).unwrap();
        v["extra"] = serde_json::json!(1);
        assert_eq!(
            OcinyeImageManifest::parse(v.to_string().as_bytes())
                .unwrap_err()
                .field,
            "$"
        );
        // serde_json keeps the last of duplicated keys; the canonical bytes of
        // the parsed value then differ from the input, which is what the
        // signed SHA256SUMS over canonical files catches. A duplicate key in a
        // nested struct with deny_unknown_fields is refused outright.
        let dup = manifest().to_canonical().replacen(
            "{\"apt_snapshot\"",
            "{\"schema\":1,\"apt_snapshot\"",
            1,
        );
        assert!(OcinyeImageManifest::parse(dup.as_bytes()).is_err());
    }

    #[test]
    fn cada_regra_falha_fechada_com_o_seu_campo() {
        type Mutation = Box<dyn Fn(&mut OcinyeImageManifest)>;
        let cases: Vec<(&str, Mutation)> = vec![
            ("schema", Box::new(|m| m.schema = 2)),
            ("image", Box::new(|m| m.image.revision = 0)),
            (
                "source.tree_clean",
                Box::new(|m| {
                    m.image.build_kind = BuildKind::Release;
                    m.source.tree_clean = false;
                }),
            ),
            (
                "release",
                Box::new(|m| m.release.id = "000000000000".into()),
            ),
            (
                "ubuntu_base",
                Box::new(|m| m.ubuntu_base.serial = "current".into()),
            ),
            (
                "ubuntu_base",
                Box::new(|m| {
                    m.ubuntu_base.url_path = "minimal/releases/noble/release-latest/x.img".into()
                }),
            ),
            (
                "apt_snapshot",
                Box::new(|m| m.apt_snapshot = "latest".into()),
            ),
            (
                "artifacts",
                Box::new(|m| m.artifacts[0].file = "evil/../x.qcow2".into()),
            ),
            (
                "artifacts.raw",
                Box::new(|m| m.artifacts[1].uncompressed_sha256 = None),
            ),
            (
                "oci_images",
                Box::new(|m| m.oci_images[0].reference = "nginx:1.30.5-alpine".into()),
            ),
            (
                "oci_images",
                Box::new(|m| m.oci_images[0].digest = OciDigest("nginx:latest".into())),
            ),
            ("sbom", Box::new(|m| m.sbom.format = "cyclonedx".into())),
            (
                "package_inventory",
                Box::new(|m| m.provenance.file = "../etc/passwd".into()),
            ),
            (
                "created_at",
                Box::new(|m| m.created_at = "2026-10-05T12:00:00Z".into()),
            ),
        ];
        for (field, f) in cases {
            let mut m = manifest();
            f(&mut m);
            assert_eq!(m.validate().unwrap_err().field, field);
        }
    }

    #[test]
    fn o_nome_de_desenvolvimento_leva_dev() {
        let v = version();
        assert_eq!(v.name(), "ocinye-os-3f9c2a7d1e04-r2-dev");
        assert_eq!(
            v.artifact_stem(Arch::Amd64),
            "ocinye-os-3f9c2a7d1e04-r2-dev-amd64"
        );
        let r = OcinyeImageVersion {
            build_kind: BuildKind::Release,
            ..v
        };
        assert_eq!(r.name(), "ocinye-os-3f9c2a7d1e04-r2");
    }

    #[test]
    fn o_manifesto_de_conteudo_exige_listas_ordenadas() {
        let c = content();
        c.validate().unwrap();
        let mut d = c.clone();
        d.release_files.reverse();
        assert_eq!(d.validate().unwrap_err().field, "release_files");
        let mut e = c;
        e.release_files[1].path = "../../etc/shadow".into();
        assert_eq!(e.validate().unwrap_err().field, "release_files");
    }

    #[test]
    fn series_fixadas() {
        for ok in ["20261001", "20261001.1"] {
            assert!(is_pinned_serial(ok), "{ok}");
        }
        for no in ["current", "latest", "2026100", "20261001.", "20261001.x"] {
            assert!(!is_pinned_serial(no), "{no}");
        }
    }
}
