//! A release staged for publication: a D011 bundle and a native layer, with
//! the update manifest that describes them.
//!
//! ```text
//! <out>/manifest.json
//! <out>/bundle/<every file of the D011 bundle>
//! <out>/native/native-<arch>.tar
//! ```

use crate::{err, sha256_file, Error, Result};
use ocinye_installer_contracts::manifest::{
    parse_sums_line, Arch as BundleArch, BuildKind, ReleaseManifest,
};
use ocinye_update_contracts::manifest::{
    Arch, Artifact, ArtifactRole, Build, ImageRef, MigrationPlan, MigrationPolicy, Notes,
    SpaceRequirement, Synthetic, UpdateManifest, BASE_OS, PRODUCT, SCHEMA,
};
use ocinye_update_contracts::{Channel, ReleaseNumber, ReleaseVersion};
use std::path::{Path, PathBuf};

/// What the publisher decides about a release; the rest comes from the bundle.
#[derive(Debug, Clone)]
pub struct StageInput {
    /// The D011 bundle directory (`ocinye-os-<id>/`).
    pub bundle_dir: PathBuf,
    /// The native layer tar.
    pub native_tar: PathBuf,
    /// Output directory; must not exist.
    pub out_dir: PathBuf,
    /// Revision of this release id.
    pub revision: u32,
    /// Monotonic number on the channel.
    pub release_number: u64,
    /// Oldest release number it may be applied over.
    pub minimum_source: u64,
    /// Channel.
    pub channel: Channel,
    /// `YYYY-MM-DDTHH:MM:SSZ`.
    pub created_at: String,
    /// Migration policy against the previous release.
    pub migrations: MigrationPolicy,
    /// A restart finishes it.
    pub reboot_required: bool,
    /// Carries security fixes.
    pub security: bool,
    /// Notes.
    pub notes: Notes,
    /// For a test release only.
    pub synthetic: Option<String>,
    /// The System Service protocol the release speaks.
    pub system_protocol: u32,
}

fn copy(from: &Path, to: &Path) -> Result<(String, u64)> {
    if let Some(p) = to.parent() {
        std::fs::create_dir_all(p)?;
    }
    std::fs::copy(from, to)?;
    sha256_file(to)
}

/// Stage a release; returns its manifest, already validated.
pub fn stage(input: &StageInput) -> Result<UpdateManifest> {
    if input.out_dir.exists() {
        return err(format!("{} already exists", input.out_dir.display()));
    }
    let manifest_bytes = std::fs::read(input.bundle_dir.join("MANIFEST.json"))?;
    let d011 = ReleaseManifest::parse(&manifest_bytes)
        .map_err(|e| Error(format!("the bundle's MANIFEST.json: {e:?}")))?;
    let sums = std::fs::read_to_string(input.bundle_dir.join("SHA256SUMS"))?;
    let mut bundle_files: Vec<(String, String)> = Vec::new();
    for line in sums.lines().filter(|l| !l.trim().is_empty()) {
        let (path, hash) =
            parse_sums_line(line).ok_or_else(|| Error(format!("SHA256SUMS line {line:?}")))?;
        bundle_files.push((path, hash));
    }
    let arch = match d011.target.arch {
        BundleArch::Amd64 => Arch::Amd64,
        BundleArch::Arm64 => Arch::Arm64,
    };
    let version =
        ReleaseVersion::new(&d011.release.id, input.revision).map_err(|e| Error(e.to_string()))?;
    std::fs::create_dir_all(&input.out_dir)?;
    let result = (|| -> Result<UpdateManifest> {
        let mut artifacts = Vec::new();
        // MANIFEST.json first (release_manifest_sha256 names it), then what
        // SHA256SUMS lists, then SHA256SUMS itself; each file once.
        let sums_hash = |name: &str| {
            bundle_files
                .iter()
                .find(|(p, _)| p == name)
                .map(|(_, h)| h.clone())
        };
        let mut listed: Vec<(String, Option<String>)> =
            vec![("MANIFEST.json".to_owned(), sums_hash("MANIFEST.json"))];
        for (p, h) in &bundle_files {
            if p != "MANIFEST.json" && p != "SHA256SUMS" {
                listed.push((p.clone(), Some(h.clone())));
            }
        }
        listed.push(("SHA256SUMS".to_owned(), None));
        for (path, expected) in listed {
            let (sha, bytes) = copy(
                &input.bundle_dir.join(&path),
                &input.out_dir.join("bundle").join(&path),
            )?;
            if let Some(e) = expected {
                if e != sha {
                    return err(format!("{path} does not match the bundle's SHA256SUMS"));
                }
            }
            artifacts.push(Artifact {
                target: format!("bundle/{path}"),
                role: ArtifactRole::ReleaseBundle,
                sha256: sha,
                bytes,
            });
        }
        let native_target = format!("native/native-{}.tar", arch.as_str());
        let (sha, bytes) = copy(&input.native_tar, &input.out_dir.join(&native_target))?;
        artifacts.push(Artifact {
            target: native_target,
            role: ArtifactRole::Native,
            sha256: sha,
            bytes,
        });
        let release_manifest_sha256 = artifacts[0].sha256.clone();
        let download_bytes = artifacts.iter().map(|a| a.bytes).sum();
        let images = d011
            .images
            .iter()
            .map(|i| ImageRef {
                name: i
                    .name
                    .strip_prefix("ocinye/ocinye-")
                    .unwrap_or(&i.name)
                    .to_owned(),
                image_id: i.image_id.clone(),
            })
            .collect();
        let m = UpdateManifest {
            schema: SCHEMA,
            product: PRODUCT.into(),
            version: version.clone(),
            release_number: ReleaseNumber(input.release_number),
            channel: input.channel,
            created_at: input.created_at.clone(),
            arch,
            commit: d011.release.commit.clone(),
            build: match d011.release.build {
                BuildKind::Release => Build::Release,
                BuildKind::Proof => Build::Proof,
            },
            base_os: BASE_OS.into(),
            minimum_source: ReleaseNumber(input.minimum_source),
            system_protocol: input.system_protocol,
            artifacts,
            release_manifest_sha256,
            images,
            migrations: MigrationPlan {
                schema_version: d011.migrations.count,
                policy: input.migrations,
            },
            space: SpaceRequirement {
                download_bytes,
                // Unpacked images take about as much again, plus the tree.
                install_bytes: download_bytes.saturating_mul(2),
            },
            reboot_required: input.reboot_required,
            security: input.security,
            notes: input.notes.clone(),
            synthetic: input.synthetic.clone().map(|purpose| Synthetic { purpose }),
        };
        m.validate().map_err(|e| Error(e.to_string()))?;
        let bytes = serde_json::to_vec_pretty(&m).map_err(|e| Error(e.to_string()))?;
        std::fs::write(input.out_dir.join("manifest.json"), bytes)?;
        Ok(m)
    })();
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&input.out_dir);
    }
    result
}
