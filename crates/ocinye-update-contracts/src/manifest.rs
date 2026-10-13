//! The update manifest: what a release is, and what it asks of the machine.
//!
//! The manifest is itself a TUF target, so its bytes are covered by the signed
//! `targets` metadata; every artifact it lists is also a TUF target, with the
//! same length and SHA-256. The engine refuses a release whose manifest and
//! TUF metadata disagree.

use crate::version::{is_commit, is_release_id, is_sha256, Channel, ReleaseNumber, ReleaseVersion};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;
use ts_rs::TS;

/// The only schema this engine reads.
pub const SCHEMA: u32 = 1;
/// The product name.
pub const PRODUCT: &str = "ocinye-os";
/// The base OS every v1 release targets. A different base is a major base
/// upgrade, which v1 does not do (`BASE_OS_MAJOR_UPGRADE = DEFERRED`).
pub const BASE_OS: &str = "ubuntu-24.04";
/// Limits that bound what a manifest may ask for.
pub const MAX_ARTIFACTS: usize = 64;
/// No single artifact larger than 16 GiB.
pub const MAX_ARTIFACT_BYTES: u64 = 16 << 30;
/// Notes are short; long ones go in a notes artifact.
pub const MAX_NOTE_CHARS: usize = 2000;

/// Target architecture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum Arch {
    /// x86-64.
    Amd64,
    /// AArch64.
    Arm64,
}

impl Arch {
    /// The wire name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Amd64 => "amd64",
            Self::Arm64 => "arm64",
        }
    }
}

/// How the binaries were built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum Build {
    /// Optimised release build.
    Release,
    /// Proof build: the same code without LTO; never on `stable`.
    Proof,
}

/// What an artifact is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactRole {
    /// A file of the D011 release bundle, at `bundle/<its path>`.
    ReleaseBundle,
    /// The native layer: `native/native-<arch>.tar`.
    Native,
}

/// One artifact. `target` is the TUF target name, relative to the release's
/// directory in the repository.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    /// Target path, e.g. `bundle/images/ocinye-core-server.tar`.
    pub target: String,
    /// Role.
    pub role: ArtifactRole,
    /// Lower-case hex SHA-256.
    pub sha256: String,
    /// Size in bytes.
    #[ts(type = "number")]
    pub bytes: u64,
}

/// A release image, by the exact image id the D011 manifest pins.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ImageRef {
    /// Service name, e.g. `core-server`.
    pub name: String,
    /// `sha256:<64 hex>`.
    pub image_id: String,
}

/// What the release does to the database schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case", tag = "kind", deny_unknown_fields)]
pub enum MigrationPolicy {
    /// No schema change against the previous release.
    None,
    /// Additive only: the previous release still runs on the new schema
    /// during the rollback window.
    Expand,
    /// Removes or rewrites schema. Accepted only from a source release at or
    /// after `requires_source`, which no longer depends on what is removed;
    /// rolling back across it is a checkpoint restore, said as such.
    Contract {
        /// The oldest source release this contraction is safe from.
        #[ts(type = "number")]
        requires_source: u64,
    },
}

/// The schema the release brings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct MigrationPlan {
    /// The number of the latest migration the release carries.
    pub schema_version: u32,
    /// Policy against the previous release.
    pub policy: MigrationPolicy,
}

/// Disk the release needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct SpaceRequirement {
    /// Bytes downloaded (the sum of the artifacts).
    #[ts(type = "number")]
    pub download_bytes: u64,
    /// Bytes once staged and installed, beyond the download.
    #[ts(type = "number")]
    pub install_bytes: u64,
}

/// Short notes, in the three product languages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct Notes {
    /// Portuguese.
    pub pt: String,
    /// English.
    pub en: String,
    /// French.
    pub fr: String,
}

/// A release made for a test, never for a machine in use.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct Synthetic {
    /// Why it exists, e.g. `successor-of-the-same-commit`, `unhealthy-native`.
    pub purpose: String,
}

/// The manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct UpdateManifest {
    /// Always [`SCHEMA`].
    pub schema: u32,
    /// Always [`PRODUCT`].
    pub product: String,
    /// Release identity.
    pub version: ReleaseVersion,
    /// Monotonic number within the channel.
    pub release_number: ReleaseNumber,
    /// The channel it is published on.
    pub channel: Channel,
    /// RFC 3339 UTC, `YYYY-MM-DDTHH:MM:SSZ`.
    pub created_at: String,
    /// Architecture.
    pub arch: Arch,
    /// The commit it was built from; starts with the release id.
    pub commit: String,
    /// Build kind.
    pub build: Build,
    /// Base OS.
    pub base_os: String,
    /// The oldest installed release number this one may be applied over.
    pub minimum_source: ReleaseNumber,
    /// The System Service wire protocol the release speaks.
    pub system_protocol: u32,
    /// Artifacts.
    pub artifacts: Vec<Artifact>,
    /// SHA-256 of the D011 `MANIFEST.json` among the artifacts.
    pub release_manifest_sha256: String,
    /// The release images, exactly.
    pub images: Vec<ImageRef>,
    /// Schema.
    pub migrations: MigrationPlan,
    /// Disk.
    pub space: SpaceRequirement,
    /// Whether the machine must restart to finish the release.
    pub reboot_required: bool,
    /// Whether it carries security fixes.
    pub security: bool,
    /// Notes.
    pub notes: Notes,
    /// Present only on a test release.
    pub synthetic: Option<Synthetic>,
}

/// Why a manifest was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestError {
    /// The JSON is not a manifest of this schema.
    Malformed(String),
    /// A field breaks a rule; the name of the rule.
    Invalid(&'static str),
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed(e) => write!(f, "malformed manifest: {e}"),
            Self::Invalid(rule) => write!(f, "invalid manifest: {rule}"),
        }
    }
}

impl std::error::Error for ManifestError {}

impl UpdateManifest {
    /// Parse and validate. A duplicate key is refused too: `serde_json` keeps
    /// the last value, which would let a signed manifest say two things.
    pub fn parse(bytes: &[u8]) -> Result<Self, ManifestError> {
        if bytes.len() > 1 << 20 {
            return Err(ManifestError::Invalid("a manifest is at most 1 MiB"));
        }
        let text =
            std::str::from_utf8(bytes).map_err(|_| ManifestError::Malformed("not UTF-8".into()))?;
        reject_duplicate_keys(text)?;
        let m: Self =
            serde_json::from_str(text).map_err(|e| ManifestError::Malformed(e.to_string()))?;
        m.validate()?;
        Ok(m)
    }

    /// Every rule, fail closed.
    pub fn validate(&self) -> Result<(), ManifestError> {
        use ManifestError::Invalid;
        if self.schema != SCHEMA {
            return Err(Invalid("unknown schema"));
        }
        if self.product != PRODUCT {
            return Err(Invalid("product is not ocinye-os"));
        }
        if self.release_number.0 == 0 {
            return Err(Invalid("release_number starts at 1"));
        }
        if self.minimum_source.0 >= self.release_number.0 {
            return Err(Invalid("minimum_source is older than the release"));
        }
        if !is_utc_timestamp(&self.created_at) {
            return Err(Invalid("created_at is YYYY-MM-DDTHH:MM:SSZ"));
        }
        if !is_commit(&self.commit) || !self.commit.starts_with(self.version.release_id()) {
            return Err(Invalid(
                "commit is 40 hex digits starting with the release id",
            ));
        }
        if !is_release_id(self.version.release_id()) {
            return Err(Invalid("release id"));
        }
        if self.build == Build::Proof && self.channel != Channel::Development {
            return Err(Invalid("a proof build is published on development only"));
        }
        if self.synthetic.is_some() && self.channel != Channel::Development {
            return Err(Invalid(
                "a synthetic release is published on development only",
            ));
        }
        if self.base_os != BASE_OS {
            return Err(Invalid("base_os is not ubuntu-24.04"));
        }
        if self.system_protocol == 0 {
            return Err(Invalid("system_protocol"));
        }
        if self.artifacts.is_empty() || self.artifacts.len() > MAX_ARTIFACTS {
            return Err(Invalid("between 1 and 64 artifacts"));
        }
        let mut seen = BTreeSet::new();
        let mut total: u64 = 0;
        let mut natives = 0;
        let native_name = format!("native/native-{}.tar", self.arch.as_str());
        for a in &self.artifacts {
            if !is_safe_target(&a.target) {
                return Err(Invalid("artifact target is not a safe relative path"));
            }
            if !seen.insert(a.target.as_str()) {
                return Err(Invalid("artifact listed twice"));
            }
            if !is_sha256(&a.sha256) {
                return Err(Invalid("artifact sha256"));
            }
            if a.bytes == 0 || a.bytes > MAX_ARTIFACT_BYTES {
                return Err(Invalid("artifact size"));
            }
            total = total
                .checked_add(a.bytes)
                .ok_or(Invalid("artifact sizes overflow"))?;
            match a.role {
                ArtifactRole::Native => {
                    if a.target != native_name {
                        return Err(Invalid("the native layer is native/native-<arch>.tar"));
                    }
                    natives += 1;
                }
                ArtifactRole::ReleaseBundle => {
                    if !a.target.starts_with("bundle/") {
                        return Err(Invalid("a release bundle file lives under bundle/"));
                    }
                }
            }
        }
        if natives != 1 {
            return Err(Invalid("exactly one native layer"));
        }
        let d011 = self
            .artifacts
            .iter()
            .find(|a| a.target == "bundle/MANIFEST.json")
            .ok_or(Invalid("the D011 MANIFEST.json is an artifact"))?;
        if d011.sha256 != self.release_manifest_sha256 {
            return Err(Invalid(
                "release_manifest_sha256 is not the MANIFEST.json artifact",
            ));
        }
        for required in [
            "bundle/RELEASE",
            "bundle/SHA256SUMS",
            "bundle/install/ocinye",
        ] {
            if !seen.contains(required) {
                return Err(Invalid("the release bundle is incomplete"));
            }
        }
        if self.space.download_bytes != total {
            return Err(Invalid("download_bytes is the sum of the artifacts"));
        }
        if self.images.is_empty() {
            return Err(Invalid("a release has images"));
        }
        let mut names = BTreeSet::new();
        for i in &self.images {
            if !is_service_name(&i.name) || !names.insert(i.name.as_str()) {
                return Err(Invalid("image name"));
            }
            match i.image_id.strip_prefix("sha256:") {
                Some(h) if is_sha256(h) => {}
                _ => return Err(Invalid("image_id is sha256:<64 hex>")),
            }
        }
        if let MigrationPolicy::Contract { requires_source } = self.migrations.policy {
            if requires_source == 0 || requires_source >= self.release_number.0 {
                return Err(Invalid("a contraction names an older source release"));
            }
            if requires_source > self.minimum_source.0 {
                // A machine older than `requires_source` must not be offered it.
                return Err(Invalid("minimum_source is at least requires_source"));
            }
        }
        for n in [&self.notes.pt, &self.notes.en, &self.notes.fr] {
            if n.trim().is_empty() || n.chars().count() > MAX_NOTE_CHARS || n.contains('\0') {
                return Err(Invalid("notes are present, short, and text"));
            }
        }
        if let Some(s) = &self.synthetic {
            if !is_service_name(&s.purpose) {
                return Err(Invalid("synthetic purpose"));
            }
        }
        Ok(())
    }

    /// Whether the release's database schema can be left under the previous
    /// release, so that a rollback needs no checkpoint restore.
    #[must_use]
    pub const fn rollback_keeps_schema(&self) -> bool {
        !matches!(self.migrations.policy, MigrationPolicy::Contract { .. })
    }

    /// The artifact with this target.
    #[must_use]
    pub fn artifact(&self, target: &str) -> Option<&Artifact> {
        self.artifacts.iter().find(|a| a.target == target)
    }
}

/// `YYYY-MM-DDTHH:MM:SSZ`.
#[must_use]
pub fn is_utc_timestamp(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 20
        && b.iter().enumerate().all(|(i, c)| match i {
            4 | 7 => *c == b'-',
            10 => *c == b'T',
            13 | 16 => *c == b':',
            19 => *c == b'Z',
            _ => c.is_ascii_digit(),
        })
}

/// A relative path of plain components: letters, digits, `.`, `_`, `-`; no
/// empty component, no `.` or `..`, nothing absolute, at most 8 deep.
#[must_use]
pub fn is_safe_target(s: &str) -> bool {
    if s.is_empty() || s.len() > 200 || s.starts_with('/') {
        return false;
    }
    let parts: Vec<&str> = s.split('/').collect();
    parts.len() <= 8
        && parts.iter().all(|p| {
            !p.is_empty()
                && *p != "."
                && *p != ".."
                && p.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        })
}

/// Lower-case letters, digits and `-`, 1–40 characters.
#[must_use]
pub fn is_service_name(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 40
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// Refuse a JSON text in which any object repeats a key.
fn reject_duplicate_keys(text: &str) -> Result<(), ManifestError> {
    use serde::de::{DeserializeSeed, IgnoredAny, MapAccess, SeqAccess, Visitor};

    struct Checker;
    impl<'de> DeserializeSeed<'de> for Checker {
        type Value = ();
        fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
            d.deserialize_any(self)
        }
    }
    impl<'de> Visitor<'de> for Checker {
        type Value = ();
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("JSON")
        }
        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
            let mut keys = BTreeSet::new();
            while let Some(k) = map.next_key::<String>()? {
                if !keys.insert(k.clone()) {
                    return Err(serde::de::Error::custom(format!("duplicate key {k}")));
                }
                map.next_value_seed(Checker)?;
            }
            Ok(())
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
            while seq.next_element_seed(Checker)?.is_some() {}
            Ok(())
        }
        fn visit_bool<E>(self, _: bool) -> Result<(), E> {
            Ok(())
        }
        fn visit_i64<E>(self, _: i64) -> Result<(), E> {
            Ok(())
        }
        fn visit_u64<E>(self, _: u64) -> Result<(), E> {
            Ok(())
        }
        fn visit_f64<E>(self, _: f64) -> Result<(), E> {
            Ok(())
        }
        fn visit_str<E>(self, _: &str) -> Result<(), E> {
            Ok(())
        }
        fn visit_unit<E>(self) -> Result<(), E> {
            Ok(())
        }
        fn visit_none<E>(self) -> Result<(), E> {
            Ok(())
        }
        fn visit_some<D: serde::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
            d.deserialize_any(Checker)
        }
    }
    let _ = IgnoredAny;
    let mut de = serde_json::Deserializer::from_str(text);
    Checker
        .deserialize(&mut de)
        .and_then(|()| de.end())
        .map_err(|e| ManifestError::Malformed(e.to_string()))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn sample() -> UpdateManifest {
        let h = |c: char| c.to_string().repeat(64);
        let artifacts = vec![
            Artifact {
                target: "bundle/MANIFEST.json".into(),
                role: ArtifactRole::ReleaseBundle,
                sha256: h('a'),
                bytes: 10,
            },
            Artifact {
                target: "bundle/RELEASE".into(),
                role: ArtifactRole::ReleaseBundle,
                sha256: h('b'),
                bytes: 13,
            },
            Artifact {
                target: "bundle/SHA256SUMS".into(),
                role: ArtifactRole::ReleaseBundle,
                sha256: h('c'),
                bytes: 20,
            },
            Artifact {
                target: "bundle/install/ocinye".into(),
                role: ArtifactRole::ReleaseBundle,
                sha256: h('d'),
                bytes: 30,
            },
            Artifact {
                target: "native/native-amd64.tar".into(),
                role: ArtifactRole::Native,
                sha256: h('e'),
                bytes: 40,
            },
        ];
        UpdateManifest {
            schema: SCHEMA,
            product: PRODUCT.into(),
            version: ReleaseVersion::parse("d34bbb0a141d-r1").unwrap(),
            release_number: ReleaseNumber(7),
            channel: Channel::Development,
            created_at: "2026-10-12T10:00:00Z".into(),
            arch: Arch::Amd64,
            commit: "d34bbb0a141de787f21833f740eebff9fdf722f3".into(),
            build: Build::Release,
            base_os: BASE_OS.into(),
            minimum_source: ReleaseNumber(1),
            system_protocol: 1,
            artifacts,
            release_manifest_sha256: h('a'),
            images: vec![ImageRef {
                name: "core-server".into(),
                image_id: format!("sha256:{}", h('f')),
            }],
            migrations: MigrationPlan {
                schema_version: 64,
                policy: MigrationPolicy::Expand,
            },
            space: SpaceRequirement {
                download_bytes: 113,
                install_bytes: 1000,
            },
            reboot_required: false,
            security: false,
            notes: Notes {
                pt: "Correcções.".into(),
                en: "Fixes.".into(),
                fr: "Corrections.".into(),
            },
            synthetic: None,
        }
    }

    fn refused(m: &UpdateManifest) -> bool {
        let bytes = serde_json::to_vec(m).unwrap();
        UpdateManifest::parse(&bytes).is_err()
    }

    #[test]
    fn o_exemplo_e_valido_e_volta_igual() {
        let m = sample();
        let bytes = serde_json::to_vec(&m).unwrap();
        assert_eq!(UpdateManifest::parse(&bytes).unwrap(), m);
    }

    #[test]
    fn recusa_campos_desconhecidos_e_chaves_repetidas() {
        let mut v = serde_json::to_value(sample()).unwrap();
        v["extra"] = serde_json::json!(true);
        assert!(UpdateManifest::parse(&serde_json::to_vec(&v).unwrap()).is_err());
        let text = serde_json::to_string(&sample()).unwrap();
        let twice = text.replacen(
            "\"reboot_required\":false",
            "\"reboot_required\":false,\"reboot_required\":true",
            1,
        );
        assert_ne!(text, twice);
        assert!(matches!(
            UpdateManifest::parse(twice.as_bytes()),
            Err(ManifestError::Malformed(_))
        ));
    }

    #[test]
    fn recusa_caminhos_que_saem_do_release() {
        for bad in [
            "../etc/passwd",
            "/etc/shadow",
            "bundle/../../x",
            "bundle//x",
            "bundle/./x",
            "bundle/x y",
            "bundle/\u{0}x",
            "",
        ] {
            let mut m = sample();
            m.artifacts[1].target = bad.into();
            assert!(refused(&m), "{bad:?} was accepted");
        }
    }

    #[test]
    fn recusa_regras_de_publicacao() {
        let mut m = sample();
        m.build = Build::Proof;
        m.channel = Channel::Stable;
        assert!(refused(&m));

        let mut m = sample();
        m.synthetic = Some(Synthetic {
            purpose: "unhealthy-native".into(),
        });
        m.channel = Channel::Candidate;
        assert!(refused(&m));

        let mut m = sample();
        m.minimum_source = ReleaseNumber(7);
        assert!(refused(&m));

        let mut m = sample();
        m.commit = "ffffffffffff".to_owned() + &"0".repeat(28);
        assert!(refused(&m));

        let mut m = sample();
        m.space.download_bytes += 1;
        assert!(refused(&m));

        let mut m = sample();
        m.artifacts.push(m.artifacts[0].clone());
        assert!(refused(&m));

        let mut m = sample();
        m.artifacts.retain(|a| a.role != ArtifactRole::Native);
        assert!(refused(&m));

        let mut m = sample();
        m.artifacts[4].target = "native/native-arm64.tar".into();
        assert!(refused(&m));

        let mut m = sample();
        m.artifacts[0].bytes = u64::MAX;
        assert!(refused(&m));

        let mut m = sample();
        m.images[0].image_id = "ocinye/core:latest".into();
        assert!(refused(&m));

        let mut m = sample();
        m.base_os = "ubuntu-26.04".into();
        assert!(refused(&m));
    }

    #[test]
    fn contracao_so_de_uma_origem_que_ja_nao_depende_dela() {
        let mut m = sample();
        m.migrations.policy = MigrationPolicy::Contract { requires_source: 5 };
        assert!(refused(&m), "minimum_source 1 < requires_source 5");
        m.minimum_source = ReleaseNumber(5);
        assert!(!refused(&m));
        assert!(!m.rollback_keeps_schema());
        m.migrations.policy = MigrationPolicy::Contract { requires_source: 7 };
        assert!(refused(&m));
    }

    #[test]
    fn recusa_o_que_nao_e_utf8_nem_json() {
        assert!(UpdateManifest::parse(b"\xff\xfe").is_err());
        assert!(UpdateManifest::parse(b"{").is_err());
        assert!(UpdateManifest::parse(&vec![b' '; (1 << 20) + 1]).is_err());
    }
}
