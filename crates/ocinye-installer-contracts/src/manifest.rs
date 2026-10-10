//! The release manifest (`MANIFEST.json` at the bundle root, D011 §5).
//!
//! Not to be confused with the Design package's own `MANIFEST.json`: this one
//! describes **an Ocinye OS release** — the commit it was built from, the
//! architecture, every artifact with its SHA-256 and size, the images, the
//! migrations, what the release is compatible with, and the fingerprint of the
//! Docker repository key the Installer may trust when it has to install the
//! runtime.
//!
//! The manifest is identified by the SHA-256 of its canonical bytes
//! ([`crate::canonical`]) and is itself listed in `SHA256SUMS` like every other
//! file. **It is not a signature**: sums prove a bundle arrived as it left, not
//! who made it (`RELEASE_SIGNING = NOT_IMPLEMENTED`).
//!
//! Validation fails closed, naming the field — the UI shows
//! `INVALID_MANIFEST{field}` (I22).

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::canonical;

/// The only schema this Installer reads.
pub const SCHEMA: u32 = 1;

/// The five images every release carries.
pub const SERVICES: [&str; 5] = [
    "core-server",
    "workspace",
    "worker",
    "conversion-runner",
    "converter",
];

/// The packages the Installer may install from Docker's repository, and only
/// these (D011_PREREQUISITE_MATRIX).
pub const DOCKER_PACKAGES: [&str; 4] = [
    "docker-ce",
    "docker-ce-cli",
    "containerd.io",
    "docker-compose-plugin",
];

/// Release or proof build.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildKind {
    /// A release build.
    Release,
    /// A proof build (no LTO): the UI says «DEVELOPMENT BUILD».
    Proof,
}

/// The server architectures a release may target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Arch {
    /// x86_64.
    Amd64,
    /// aarch64.
    Arm64,
}

impl Arch {
    /// From `uname -m`.
    #[must_use]
    pub fn from_uname(machine: &str) -> Option<Self> {
        match machine.trim() {
            "x86_64" | "amd64" => Some(Self::Amd64),
            "aarch64" | "arm64" => Some(Self::Arm64),
            _ => None,
        }
    }

    /// The release name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Amd64 => "amd64",
            Self::Arm64 => "arm64",
        }
    }

    /// Docker's apt architecture name.
    #[must_use]
    pub const fn dpkg(self) -> &'static str {
        self.as_str()
    }
}

/// `release`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseInfo {
    /// 12 hex: the short commit.
    pub id: String,
    /// 40 hex: the full commit.
    pub commit: String,
    /// Release or proof.
    pub build: BuildKind,
    /// RFC 3339 UTC.
    pub created_at: String,
    /// The commit time, in seconds (reproducible metadata).
    pub source_date_epoch: i64,
}

/// `target`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    /// Always `linux`.
    pub os: String,
    /// The one architecture of every image in the bundle.
    pub arch: Arch,
}

/// One file of the bundle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    /// Relative path inside the bundle.
    pub path: String,
    /// SHA-256, lowercase hex.
    pub sha256: String,
    /// Size in bytes.
    pub bytes: u64,
}

/// One release image.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Image {
    /// `ocinye/ocinye-<svc>`.
    pub name: String,
    /// The release id (never `latest`).
    pub tag: String,
    /// The archive path inside the bundle.
    pub archive: String,
    /// `sha256:<64 hex>`.
    pub image_id: String,
}

/// An image Compose pulls (not inside the bundle).
///
/// Its identity is the **digest**: a tag is a name someone can move, and a
/// release that says `redis:7-alpine` names whatever that tag points at on
/// the day of the install. The tag stays only so a person can read it. The
/// `reference` is exactly what Compose pulls, and it is
/// `repository[:tag]@sha256:<64 hex>` — never a tag alone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThirdPartyImage {
    /// The reference as Compose names it: `repository[:tag]@digest`.
    pub reference: String,
    /// The repository (`nginx`, `pgvector/pgvector`, `ghcr.io/…`).
    pub repository: String,
    /// The readable tag, when the reference has one (never `latest`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    /// `sha256:<64 hex>`: the identity.
    pub digest: String,
    /// Which compose file names it.
    pub from_compose: String,
}

impl ThirdPartyImage {
    /// Splits a Compose reference into its parts. Refuses a reference without
    /// an immutable digest, a `latest` tag, and anything that is not a plain
    /// `repository[:tag]@sha256:<64 hex>`.
    ///
    /// # Errors
    ///
    /// The reason, as a stable code.
    pub fn from_reference(reference: &str, from_compose: &str) -> Result<Self, &'static str> {
        if reference.is_empty()
            || reference
                .chars()
                .any(|c| c.is_whitespace() || c.is_control())
        {
            return Err("IMAGE_REFERENCE_INVALID");
        }
        let Some((name, digest)) = reference.split_once('@') else {
            return Err("IMAGE_NOT_PINNED_BY_DIGEST");
        };
        if !digest
            .strip_prefix("sha256:")
            .is_some_and(|h| is_hex(h, 64, false))
        {
            return Err("IMAGE_DIGEST_INVALID");
        }
        // A tag is the part after the last `:` that comes after the last `/`
        // (a registry port is before it: `host:5000/repo`).
        let slash = name.rfind('/').map_or(0, |i| i + 1);
        let (repository, tag) = match name[slash..].rfind(':') {
            Some(i) => (&name[..slash + i], Some(&name[slash + i + 1..])),
            None => (name, None),
        };
        let repo_ok = !repository.is_empty()
            && repository
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._-/:".contains(&b));
        if !repo_ok {
            return Err("IMAGE_REPOSITORY_INVALID");
        }
        if let Some(t) = tag {
            if t.is_empty()
                || t == "latest"
                || !t
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
            {
                return Err("IMAGE_TAG_INVALID");
            }
        }
        Ok(Self {
            reference: reference.to_owned(),
            repository: repository.to_owned(),
            tag: tag.map(str::to_owned),
            digest: digest.to_owned(),
            from_compose: from_compose.to_owned(),
        })
    }
}

/// `migrations`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Migrations {
    /// How many.
    pub count: u32,
    /// The latest, four digits.
    pub latest: String,
    /// SHA-256 of the sorted `name sha256` lines of `migrations/*.sql`.
    pub files_sha256: String,
}

/// `compatibility`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Compatibility {
    /// `GET /ready?contract=N`.
    pub readiness_contract: u32,
    /// [`crate::BOOTSTRAP_PROTOCOL`].
    pub bootstrap_protocol: u16,
    /// The oldest Installer that may install this release.
    pub installer_min: String,
    /// Docker Engine minimum.
    pub docker_engine_min: String,
    /// `v2`.
    pub compose: String,
    /// Where the Installer may install prerequisites. Ocinye OS v1:
    /// `["ubuntu-24.04"]` only.
    pub supported_prerequisite_targets: Vec<String>,
}

/// `prerequisites`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Prerequisites {
    /// Docker's published apt key fingerprint, 40 uppercase hex. The key the
    /// Installer downloads must have exactly this fingerprint, or it stops.
    pub docker_repo_key_fingerprint: String,
    /// Exactly [`DOCKER_PACKAGES`].
    pub docker_packages: Vec<String>,
}

/// `bootstrap`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BootstrapRef {
    /// `ocinye-bootstrap`.
    pub path: String,
    /// Its SHA-256 (also in `artifacts`).
    pub sha256: String,
    /// A static executable (no dynamic loader dependencies on the target).
    #[serde(rename = "static")]
    pub is_static: bool,
}

/// The release manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseManifest {
    /// [`SCHEMA`].
    pub schema: u32,
    /// `ocinye-os`.
    pub product: String,
    /// Identity of the build.
    pub release: ReleaseInfo,
    /// Where it runs.
    pub target: Target,
    /// Every file, except `MANIFEST.json` and `SHA256SUMS`.
    pub artifacts: Vec<Artifact>,
    /// The five release images.
    pub images: Vec<Image>,
    /// What Compose pulls.
    pub third_party_images: Vec<ThirdPartyImage>,
    /// The schema the release brings.
    pub migrations: Migrations,
    /// What it is compatible with.
    pub compatibility: Compatibility,
    /// Prerequisite trust data.
    pub prerequisites: Prerequisites,
    /// The remote executor.
    pub bootstrap: BootstrapRef,
}

/// Why a manifest was refused (I22): the first offending field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvalidManifest {
    /// Dotted path, e.g. `migrations.latest`.
    pub field: String,
}

impl std::fmt::Display for InvalidManifest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid manifest field: {}", self.field)
    }
}

impl std::error::Error for InvalidManifest {}

fn bad(field: &str) -> InvalidManifest {
    InvalidManifest {
        field: field.to_owned(),
    }
}

fn is_hex(s: &str, len: usize, upper: bool) -> bool {
    s.len() == len
        && s.bytes().all(|b| {
            b.is_ascii_digit()
                || if upper {
                    (b'A'..=b'F').contains(&b)
                } else {
                    (b'a'..=b'f').contains(&b)
                }
        })
}

/// A path inside the bundle: relative, no `..`, no empty or dot components, a
/// closed character set, at most three components.
#[must_use]
pub fn is_safe_bundle_path(path: &str) -> bool {
    let parts: Vec<&str> = path.split('/').collect();
    !path.is_empty()
        && path.len() <= 200
        && parts.len() <= 3
        && parts.iter().all(|p| {
            !p.is_empty()
                && *p != "."
                && *p != ".."
                && !p.starts_with('-')
                && p.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        })
}

impl ReleaseManifest {
    /// Parse bytes and validate. Unknown fields, unknown enum values and a
    /// floating-point anywhere are refused.
    ///
    /// # Errors
    ///
    /// [`InvalidManifest`] with the field.
    pub fn parse(bytes: &[u8]) -> Result<Self, InvalidManifest> {
        let value: serde_json::Value = serde_json::from_slice(bytes).map_err(|_| bad("$"))?;
        canonical::to_canonical(&value).map_err(|_| bad("$"))?;
        let manifest: Self = serde_json::from_value(value).map_err(|e| {
            // serde names the unknown or malformed field in its message; keep
            // only a field-shaped token, never the value.
            let msg = e.to_string();
            let field = msg
                .split('`')
                .nth(1)
                .filter(|f| f.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'))
                .unwrap_or("$");
            bad(field)
        })?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// The canonical bytes.
    ///
    /// # Panics
    ///
    /// Never: a manifest has no floating-point fields.
    #[must_use]
    pub fn canonical_bytes(&self) -> String {
        canonical::to_canonical(self).expect("a manifest has integers only")
    }

    /// `manifest_sha256`.
    #[must_use]
    pub fn sha256(&self) -> String {
        canonical::sha256_hex(self.canonical_bytes().as_bytes())
    }

    /// The artifact entry for a path.
    #[must_use]
    pub fn artifact(&self, path: &str) -> Option<&Artifact> {
        self.artifacts.iter().find(|a| a.path == path)
    }

    /// Every rule of the contract.
    ///
    /// # Errors
    ///
    /// [`InvalidManifest`] naming the first offending field.
    #[allow(clippy::too_many_lines)]
    pub fn validate(&self) -> Result<(), InvalidManifest> {
        if self.schema != SCHEMA {
            return Err(bad("schema"));
        }
        if self.product != "ocinye-os" {
            return Err(bad("product"));
        }
        let r = &self.release;
        if !is_hex(&r.id, 12, false) {
            return Err(bad("release.id"));
        }
        if !is_hex(&r.commit, 40, false) || !r.commit.starts_with(&r.id) {
            return Err(bad("release.commit"));
        }
        if chrono::DateTime::parse_from_rfc3339(&r.created_at).is_err()
            || !r.created_at.ends_with('Z')
        {
            return Err(bad("release.created_at"));
        }
        if r.source_date_epoch <= 0 {
            return Err(bad("release.source_date_epoch"));
        }
        if self.target.os != "linux" {
            return Err(bad("target.os"));
        }

        let mut seen = BTreeSet::new();
        for a in &self.artifacts {
            if !is_safe_bundle_path(&a.path)
                || matches!(a.path.as_str(), "MANIFEST.json" | "SHA256SUMS")
            {
                return Err(bad("artifacts.path"));
            }
            if !seen.insert(a.path.as_str()) {
                return Err(bad("artifacts.path"));
            }
            if !is_hex(&a.sha256, 64, false) {
                return Err(bad("artifacts.sha256"));
            }
            if a.bytes == 0 {
                return Err(bad("artifacts.bytes"));
            }
        }
        for required in ["source.tar.gz", "install/ocinye", "ocinye-bootstrap"] {
            if !seen.contains(required) {
                return Err(bad("artifacts"));
            }
        }

        if self.images.len() != SERVICES.len() {
            return Err(bad("images"));
        }
        let mut names = BTreeSet::new();
        for (image, svc) in self.images.iter().zip(SERVICES) {
            if image.name != format!("ocinye/ocinye-{svc}") || !names.insert(&image.name) {
                return Err(bad("images.name"));
            }
            if image.tag != r.id {
                return Err(bad("images.tag"));
            }
            if image.archive != format!("images/ocinye-{svc}.tar")
                || !seen.contains(image.archive.as_str())
            {
                return Err(bad("images.archive"));
            }
            if !image
                .image_id
                .strip_prefix("sha256:")
                .is_some_and(|h| is_hex(h, 64, false))
            {
                return Err(bad("images.image_id"));
            }
        }
        for t in &self.third_party_images {
            // The parts must be exactly what the reference says: a digest
            // field that disagrees with the reference Compose pulls would
            // certify bytes nobody installs.
            match ThirdPartyImage::from_reference(&t.reference, &t.from_compose) {
                Ok(parsed) if parsed == *t => {}
                _ => return Err(bad("third_party_images.reference")),
            }
            if !is_safe_bundle_path(&t.from_compose) {
                return Err(bad("third_party_images.from_compose"));
            }
        }

        let m = &self.migrations;
        if m.count == 0 {
            return Err(bad("migrations.count"));
        }
        if m.latest.len() != 4
            || !m.latest.bytes().all(|b| b.is_ascii_digit())
            || m.latest.parse::<u32>().ok() != Some(m.count)
        {
            return Err(bad("migrations.latest"));
        }
        if !is_hex(&m.files_sha256, 64, false) {
            return Err(bad("migrations.files_sha256"));
        }

        let c = &self.compatibility;
        if c.readiness_contract == 0 {
            return Err(bad("compatibility.readiness_contract"));
        }
        if c.bootstrap_protocol != crate::BOOTSTRAP_PROTOCOL {
            return Err(bad("compatibility.bootstrap_protocol"));
        }
        if c.compose != "v2" {
            return Err(bad("compatibility.compose"));
        }
        if c.docker_engine_min != "24.0" {
            return Err(bad("compatibility.docker_engine_min"));
        }
        if !semver_like(&c.installer_min) {
            return Err(bad("compatibility.installer_min"));
        }
        if c.supported_prerequisite_targets != [crate::SUPPORTED_TARGET] {
            return Err(bad("compatibility.supported_prerequisite_targets"));
        }

        let p = &self.prerequisites;
        if !is_hex(&p.docker_repo_key_fingerprint, 40, true) {
            return Err(bad("prerequisites.docker_repo_key_fingerprint"));
        }
        if p.docker_packages != DOCKER_PACKAGES {
            return Err(bad("prerequisites.docker_packages"));
        }

        let b = &self.bootstrap;
        if b.path != "ocinye-bootstrap" || !b.is_static {
            return Err(bad("bootstrap.path"));
        }
        if self.artifact(&b.path).map(|a| a.sha256.as_str()) != Some(b.sha256.as_str()) {
            return Err(bad("bootstrap.sha256"));
        }
        Ok(())
    }
}

fn semver_like(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
}

/// Compare `MAJOR.MINOR.PATCH` versions.
#[must_use]
pub fn version_at_least(have: &str, min: &str) -> bool {
    let parse = |s: &str| -> Vec<u32> {
        s.split(['.', '-', '+', '~'])
            .take(3)
            .map(|p| p.parse().unwrap_or(0))
            .collect()
    };
    parse(have) >= parse(min)
}

/// One line of `SHA256SUMS` (`<hex>  ./path` or `<hex>  path`).
///
/// # Errors
///
/// `None` for a malformed line.
#[must_use]
pub fn parse_sums_line(line: &str) -> Option<(String, String)> {
    let (hex, path) = line.split_once("  ")?;
    let path = path.trim_start_matches('*').trim_start_matches("./");
    (is_hex(hex, 64, false) && is_safe_bundle_path(path)).then(|| (path.to_owned(), hex.to_owned()))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn sample() -> ReleaseManifest {
        let h = |c: char| c.to_string().repeat(64);
        let mut artifacts = vec![
            Artifact {
                path: "source.tar.gz".into(),
                sha256: h('1'),
                bytes: 10,
            },
            Artifact {
                path: "install/ocinye".into(),
                sha256: h('2'),
                bytes: 20,
            },
            Artifact {
                path: "ocinye-bootstrap".into(),
                sha256: h('3'),
                bytes: 30,
            },
        ];
        let mut images = Vec::new();
        for (i, svc) in SERVICES.iter().enumerate() {
            artifacts.push(Artifact {
                path: format!("images/ocinye-{svc}.tar"),
                sha256: h(char::from(b'a' + u8::try_from(i).unwrap())),
                bytes: 100,
            });
            images.push(Image {
                name: format!("ocinye/ocinye-{svc}"),
                tag: "3f9c2a7d1e04".into(),
                archive: format!("images/ocinye-{svc}.tar"),
                image_id: format!("sha256:{}", h('9')),
            });
        }
        ReleaseManifest {
            schema: 1,
            product: "ocinye-os".into(),
            release: ReleaseInfo {
                id: "3f9c2a7d1e04".into(),
                commit: "3f9c2a7d1e04b7c19e6a5d20f81c4e7a9b03d6f2".into(),
                build: BuildKind::Release,
                created_at: "2026-10-04T12:00:00Z".into(),
                source_date_epoch: 1_790_000_000,
            },
            target: Target {
                os: "linux".into(),
                arch: Arch::Arm64,
            },
            artifacts,
            images,
            third_party_images: vec![ThirdPartyImage::from_reference(
                &format!("nginx:1.30.5-alpine@sha256:{}", h('0')),
                "infra/compose/docker-compose.production.yml",
            )
            .unwrap()],
            migrations: Migrations {
                count: 64,
                latest: "0064".into(),
                files_sha256: h('4'),
            },
            compatibility: Compatibility {
                readiness_contract: 1,
                bootstrap_protocol: 1,
                installer_min: "0.1.0".into(),
                docker_engine_min: "24.0".into(),
                compose: "v2".into(),
                supported_prerequisite_targets: vec!["ubuntu-24.04".into()],
            },
            prerequisites: Prerequisites {
                docker_repo_key_fingerprint: "9DC858229FC7DD38854AE2D88D81803C0EBFCD88".into(),
                docker_packages: DOCKER_PACKAGES.iter().map(|s| (*s).to_owned()).collect(),
            },
            bootstrap: BootstrapRef {
                path: "ocinye-bootstrap".into(),
                sha256: h('3'),
                is_static: true,
            },
        }
    }

    #[test]
    fn um_manifesto_valido_passa_e_volta_aos_mesmos_bytes() {
        let m = sample();
        m.validate().unwrap();
        let bytes = m.canonical_bytes();
        let again = ReleaseManifest::parse(bytes.as_bytes()).unwrap();
        assert_eq!(again, m);
        assert_eq!(again.canonical_bytes(), bytes);
        assert_eq!(again.sha256(), m.sha256());
        assert!(bytes.starts_with(r#"{"artifacts":[{"bytes":10,"path":"source.tar.gz""#));
        assert!(bytes.contains(r#""static":true"#));
    }

    #[test]
    fn o_mesmo_manifesto_escrito_de_outra_maneira_tem_a_mesma_soma() {
        let m = sample();
        let pretty = serde_json::to_string_pretty(&m).unwrap();
        assert_eq!(
            ReleaseManifest::parse(pretty.as_bytes()).unwrap().sha256(),
            m.sha256()
        );
    }

    fn refused(mutate: impl FnOnce(&mut ReleaseManifest)) -> String {
        let mut m = sample();
        mutate(&mut m);
        m.validate().unwrap_err().field
    }

    #[test]
    fn cada_regra_recusa_com_o_seu_campo() {
        assert_eq!(refused(|m| m.schema = 2), "schema");
        assert_eq!(refused(|m| m.product = "x".into()), "product");
        assert_eq!(
            refused(|m| m.release.id = "3F9C2A7D1E04".into()),
            "release.id"
        );
        assert_eq!(
            refused(|m| m.release.commit = "0".repeat(40)),
            "release.commit"
        );
        assert_eq!(
            refused(|m| m.release.created_at = "ontem".into()),
            "release.created_at"
        );
        assert_eq!(refused(|m| m.target.os = "windows".into()), "target.os");
        assert_eq!(
            refused(|m| m.artifacts[0].path = "../etc/passwd".into()),
            "artifacts.path"
        );
        assert_eq!(
            refused(|m| m.artifacts[0].path = "/etc/passwd".into()),
            "artifacts.path"
        );
        assert_eq!(
            refused(|m| m.artifacts[1].path = "source.tar.gz".into()),
            "artifacts.path"
        );
        assert_eq!(
            refused(|m| m.artifacts[0].sha256 = "zz".into()),
            "artifacts.sha256"
        );
        assert_eq!(
            refused(|m| {
                m.artifacts.remove(2);
            }),
            "artifacts"
        );
        assert_eq!(refused(|m| m.images[0].tag = "latest".into()), "images.tag");
        assert_eq!(
            refused(|m| {
                m.images.pop();
            }),
            "images"
        );
        assert_eq!(
            refused(|m| m.third_party_images[0].reference = "nginx:latest".into()),
            "third_party_images.reference"
        );
        assert_eq!(
            refused(|m| m.migrations.latest = "0063".into()),
            "migrations.latest"
        );
        assert_eq!(
            refused(|m| m.compatibility.bootstrap_protocol = 2),
            "compatibility.bootstrap_protocol"
        );
        assert_eq!(
            refused(|m| m
                .compatibility
                .supported_prerequisite_targets
                .push("debian-12".into())),
            "compatibility.supported_prerequisite_targets"
        );
        assert_eq!(
            refused(|m| m.prerequisites.docker_repo_key_fingerprint = "9dc8".into()),
            "prerequisites.docker_repo_key_fingerprint"
        );
        assert_eq!(
            refused(|m| m
                .prerequisites
                .docker_packages
                .push("docker-buildx-plugin".into())),
            "prerequisites.docker_packages"
        );
        assert_eq!(
            refused(|m| m.bootstrap.sha256 = "5".repeat(64)),
            "bootstrap.sha256"
        );
        assert_eq!(refused(|m| m.bootstrap.is_static = false), "bootstrap.path");
    }

    #[test]
    fn campos_desconhecidos_valores_desconhecidos_e_decimais_sao_recusados() {
        let mut v = serde_json::to_value(sample()).unwrap();
        v["signature"] = "x".into();
        assert!(ReleaseManifest::parse(v.to_string().as_bytes()).is_err());
        let mut v = serde_json::to_value(sample()).unwrap();
        v["target"]["arch"] = "riscv64".into();
        assert!(ReleaseManifest::parse(v.to_string().as_bytes()).is_err());
        let mut v = serde_json::to_value(sample()).unwrap();
        v["artifacts"][0]["bytes"] = serde_json::json!(1.5);
        assert!(ReleaseManifest::parse(v.to_string().as_bytes()).is_err());
        assert!(ReleaseManifest::parse(b"not json").is_err());
    }

    #[test]
    fn as_linhas_de_somas_sao_lidas_sem_caminhos_perigosos() {
        let h = "a".repeat(64);
        assert_eq!(
            parse_sums_line(&format!("{h}  ./images/ocinye-core-server.tar")),
            Some(("images/ocinye-core-server.tar".into(), h.clone()))
        );
        assert_eq!(parse_sums_line(&format!("{h}  ../x")), None);
        assert_eq!(parse_sums_line(&format!("{h} x")), None);
        assert!(version_at_least("27.3.1", "24.0"));
        assert!(!version_at_least("20.10.24", "24.0"));
    }

    #[test]
    fn uma_imagem_de_terceiros_so_se_identifica_por_digest() {
        let d = format!("sha256:{}", "a".repeat(64));
        let c = "infra/compose/docker-compose.production.yml";
        // Floating, latest, and malformed references are refused.
        for (r, code) in [
            ("redis:7-alpine", "IMAGE_NOT_PINNED_BY_DIGEST"),
            ("nginx", "IMAGE_NOT_PINNED_BY_DIGEST"),
            (&*format!("nginx:latest@{d}"), "IMAGE_TAG_INVALID"),
            ("nginx:1.30.5-alpine@sha256:abc", "IMAGE_DIGEST_INVALID"),
            (
                &*format!("nginx:1.30.5-alpine@sha512:{}", "a".repeat(128)),
                "IMAGE_DIGEST_INVALID",
            ),
            (&*format!("NGINX:1@{d}"), "IMAGE_REPOSITORY_INVALID"),
            (&*format!("nginx:1 @{d}"), "IMAGE_REFERENCE_INVALID"),
        ] {
            assert_eq!(ThirdPartyImage::from_reference(r, c), Err(code), "{r}");
        }
        // Tag, registry port, and digest-only references parse into parts.
        let t = ThirdPartyImage::from_reference(
            &format!("pgvector/pgvector:0.8.7-pg17-bookworm@{d}"),
            c,
        )
        .unwrap();
        assert_eq!(
            (t.repository.as_str(), t.tag.as_deref(), t.digest.as_str()),
            ("pgvector/pgvector", Some("0.8.7-pg17-bookworm"), d.as_str())
        );
        let p =
            ThirdPartyImage::from_reference(&format!("registry.test:5000/x/y:1.2@{d}"), c).unwrap();
        assert_eq!(
            (p.repository.as_str(), p.tag.as_deref()),
            ("registry.test:5000/x/y", Some("1.2"))
        );
        let o = ThirdPartyImage::from_reference(
            &format!("ghcr.io/ocinye/third-party/minio-server@{d}"),
            c,
        )
        .unwrap();
        assert_eq!(
            (o.repository.as_str(), o.tag),
            ("ghcr.io/ocinye/third-party/minio-server", None)
        );
    }

    #[test]
    fn um_manifesto_cujas_partes_desmentem_a_referencia_e_recusado() {
        let other = format!("sha256:{}", "b".repeat(64));
        assert_eq!(
            refused(|m| m.third_party_images[0].digest = other.clone()),
            "third_party_images.reference"
        );
        assert_eq!(
            refused(|m| m.third_party_images[0].tag = Some("1.31.6-alpine".into())),
            "third_party_images.reference"
        );
        assert_eq!(
            refused(|m| m.third_party_images[0].repository = "nginx-fork".into()),
            "third_party_images.reference"
        );
        assert_eq!(
            refused(|m| m.third_party_images[0].reference = "nginx:1.30.5-alpine".into()),
            "third_party_images.reference"
        );
    }

    #[test]
    fn a_forma_canonica_de_uma_imagem_de_terceiros_e_deterministica() {
        let d = format!("sha256:{}", "c".repeat(64));
        let c = "infra/compose/docker-compose.production.yml";
        let with_tag =
            ThirdPartyImage::from_reference(&format!("redis:7.4.11-alpine@{d}"), c).unwrap();
        let no_tag = ThirdPartyImage::from_reference(&format!("ghcr.io/x/y@{d}"), c).unwrap();
        let a = crate::canonical::to_canonical(&with_tag).unwrap();
        let b = crate::canonical::to_canonical(&with_tag).unwrap();
        assert_eq!(a, b);
        assert_eq!(
            a,
            format!("{{\"digest\":\"{d}\",\"from_compose\":\"{c}\",\"reference\":\"redis:7.4.11-alpine@{d}\",\"repository\":\"redis\",\"tag\":\"7.4.11-alpine\"}}")
        );
        // No tag: the field is absent, not null.
        assert!(!crate::canonical::to_canonical(&no_tag)
            .unwrap()
            .contains("tag"));
    }
}
