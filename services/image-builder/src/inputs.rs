//! B01–B03: the source, the D011 release bundle and the Ubuntu base, each
//! verified before anything is assembled from it. Fail closed.

use std::fs;
use std::path::{Path, PathBuf};

use ocinye_image_contracts::build::{BuildInput, BuildInputKind, ImageBuildError};
use ocinye_image_contracts::manifest::{Arch, Sha256Hex, UbuntuBaseIdentity};
use ocinye_installer_contracts::manifest::ReleaseManifest;
use serde::Deserialize;

use crate::cmd::{self, p};

/// `infra/image/base.json`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BaseConfig {
    pub apt_snapshot: String,
    pub curtin: CurtinPin,
    pub codename: String,
    pub images: std::collections::BTreeMap<String, BaseImage>,
    pub keyring: String,
    pub keyring_sha256: String,
    pub release: String,
    pub serial: String,
    pub sums_sha256: String,
    pub sums_signing_key_fingerprint: String,
    pub url_base: String,
    pub variant: String,
}

/// curtin for the OIE: noble ships it only inside the subiquity snap, so the
/// installation environment takes the upstream tree at a pinned commit.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CurtinPin {
    pub commit: String,
    pub license: String,
    pub repository: String,
    pub tag: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BaseImage {
    pub file: String,
    pub sha256: String,
}

/// B01: the commit is exact and the tree it came from was clean.
pub fn source(commit: &str, clean: bool) -> Result<(), ImageBuildError> {
    if !ocinye_image_contracts::is_lower_hex(commit, 40) {
        return Err(ImageBuildError::SourceNotCertified);
    }
    if !clean {
        return Err(ImageBuildError::DirtySourceTree);
    }
    Ok(())
}

/// The verified bundle.
pub struct Bundle {
    pub manifest: ReleaseManifest,
    /// Canonical digest of `MANIFEST.json` (as D011 computes it).
    pub manifest_sha256: Sha256Hex,
    pub bootstrap_sha256: Sha256Hex,
}

fn bad(field: &str) -> ImageBuildError {
    ImageBuildError::ReleaseManifestInvalid {
        field: field.into(),
    }
}

/// B02: `MANIFEST.json` validates, every listed file has its digest, the
/// architecture is the image's, and every third-party image is pinned by
/// digest.
pub fn bundle(dir: &Path, arch: Arch) -> Result<Bundle, ImageBuildError> {
    let bytes = fs::read(dir.join("MANIFEST.json")).map_err(|_| bad("MANIFEST.json"))?;
    let manifest = ReleaseManifest::parse(&bytes).map_err(|e| bad(&format!("{e:?}")))?;
    let want = match arch {
        Arch::Amd64 => ocinye_installer_contracts::manifest::Arch::Amd64,
        Arch::Arm64 => ocinye_installer_contracts::manifest::Arch::Arm64,
    };
    if manifest.target.arch != want {
        return Err(ImageBuildError::ReleaseArchMismatch);
    }
    for a in &manifest.artifacts {
        match cmd::sha256_file(&dir.join(&a.path)) {
            Ok((h, n)) if h == a.sha256 && n == a.bytes => {}
            _ => return Err(bad(&a.path)),
        }
    }
    for t in &manifest.third_party_images {
        if !t.reference.contains("@sha256:")
            || !ocinye_image_contracts::is_lower_hex(t.digest.trim_start_matches("sha256:"), 64)
        {
            return Err(ImageBuildError::FloatingImageReference {
                reference: t.reference.clone(),
            });
        }
    }
    let manifest_sha256 = Sha256Hex(
        ocinye_installer_contracts::canonical::sha256_of(&manifest)
            .map_err(|_| bad("canonical"))?,
    );
    let bootstrap_sha256 = Sha256Hex(manifest.bootstrap.sha256.clone());
    Ok(Bundle {
        manifest,
        manifest_sha256,
        bootstrap_sha256,
    })
}

/// The verified base image, in the cache.
pub struct Base {
    pub image: PathBuf,
    pub identity: UbuntuBaseIdentity,
    pub inputs: Vec<BuildInput>,
}

/// B03: `SHA256SUMS` is the pinned one and Canonical's signature over it
/// verifies with the pinned keyring; the image digest is the pinned one and
/// the one `SHA256SUMS` lists. A cached file is re-verified like a download.
pub fn base(
    cfg: &BaseConfig,
    image_dir: &Path,
    cache: &Path,
    arch: Arch,
) -> Result<Base, ImageBuildError> {
    let step = "B03";
    let img = cfg
        .images
        .get(arch.as_str())
        .ok_or(ImageBuildError::UnsupportedArchitecture {
            arch: arch.as_str().into(),
        })?;
    let dir = cache.join("ubuntu").join(&cfg.serial);
    fs::create_dir_all(&dir).map_err(|_| ImageBuildError::ImageAssemblyFailed {
        step: "cache".into(),
    })?;
    let keyring = image_dir.join(&cfg.keyring);
    let kr = cmd::sha256_file(&keyring)
        .map(|(h, _)| h)
        .unwrap_or_default();
    if kr != cfg.keyring_sha256 {
        return Err(ImageBuildError::BaseSignatureInvalid);
    }
    for f in ["SHA256SUMS", "SHA256SUMS.gpg", img.file.as_str()] {
        let dst = dir.join(f);
        if !dst.exists() {
            let url = format!("{}/{f}", cfg.url_base);
            let part = dir.join(format!("{f}.part"));
            cmd::run(
                step,
                "curl",
                &["-fsSL", "--retry", "3", "-o", p(&part), &url],
                || ImageBuildError::NetworkPolicyViolation {
                    host: "cloud-images.ubuntu.com".into(),
                },
            )?;
            fs::rename(&part, &dst).map_err(|_| ImageBuildError::ImageAssemblyFailed {
                step: "cache".into(),
            })?;
        }
    }
    let sums = dir.join("SHA256SUMS");
    if cmd::sha256_file(&sums).map(|(h, _)| h).unwrap_or_default() != cfg.sums_sha256 {
        return Err(ImageBuildError::BaseSignatureInvalid);
    }
    // gpgv prints the signer; the status-fd line carries the full fingerprint.
    let status = cmd::output(
        step,
        "gpgv",
        &[
            "--status-fd",
            "1",
            "--keyring",
            p(&keyring),
            p(&dir.join("SHA256SUMS.gpg")),
            p(&sums),
        ],
        || ImageBuildError::BaseSignatureInvalid,
    )?;
    let status = String::from_utf8_lossy(&status);
    let valid = status.lines().any(|l| {
        l.strip_prefix("[GNUPG:] VALIDSIG ").is_some_and(|rest| {
            rest.split(' ').next() == Some(cfg.sums_signing_key_fingerprint.as_str())
                || rest.split(' ').nth(9) == Some(cfg.sums_signing_key_fingerprint.as_str())
        })
    });
    if !valid {
        return Err(ImageBuildError::BaseSignatureInvalid);
    }
    let listed = fs::read_to_string(&sums)
        .unwrap_or_default()
        .lines()
        .find_map(|l| {
            l.split_once(" *")
                .filter(|(_, f)| *f == img.file)
                .map(|(h, _)| h.to_owned())
        })
        .unwrap_or_default();
    let (actual, _) = cmd::sha256_file(&dir.join(&img.file)).map_err(|_| {
        ImageBuildError::BaseDigestMismatch {
            expected: img.sha256.clone(),
            actual: String::new(),
        }
    })?;
    if listed != img.sha256 || actual != img.sha256 {
        // A corrupt cache entry is removed so the next build downloads again.
        let _ = fs::remove_file(dir.join(&img.file));
        return Err(ImageBuildError::BaseDigestMismatch {
            expected: img.sha256.clone(),
            actual,
        });
    }
    let identity = UbuntuBaseIdentity {
        release: cfg.release.clone(),
        codename: cfg.codename.clone(),
        variant: cfg.variant.clone(),
        serial: cfg.serial.clone(),
        arch,
        url_path: format!(
            "{}/{}",
            cfg.url_base
                .trim_start_matches("https://cloud-images.ubuntu.com/"),
            img.file
        ),
        sha256: Sha256Hex(img.sha256.clone()),
        sums_sha256: Sha256Hex(cfg.sums_sha256.clone()),
        sums_signing_key_fingerprint: cfg.sums_signing_key_fingerprint.clone(),
    };
    let inputs = vec![BuildInput {
        kind: BuildInputKind::UbuntuBase,
        identity: format!(
            "ubuntu-{}-{}-{}-{}",
            cfg.release,
            cfg.variant,
            cfg.serial,
            arch.as_str()
        ),
        digest: Sha256Hex(img.sha256.clone()),
        trust: format!("openpgp:{}", cfg.sums_signing_key_fingerprint),
    }];
    Ok(Base {
        image: dir.join(&img.file),
        identity,
        inputs,
    })
}

/// The curtin tree at the pinned commit (cloned once into the cache, and
/// refused unless `HEAD` is exactly that commit and the tree is clean).
pub fn curtin(pin: &CurtinPin, cache: &Path) -> Result<(PathBuf, BuildInput), ImageBuildError> {
    let step = "B03";
    let fail = || ImageBuildError::ImageAssemblyFailed {
        step: "curtin".into(),
    };
    if !ocinye_image_contracts::is_lower_hex(&pin.commit, 40) {
        return Err(fail());
    }
    let dir = cache.join("curtin").join(&pin.commit);
    let head = |d: &Path| {
        cmd::output(step, "git", &["-C", p(d), "rev-parse", "HEAD"], fail)
            .map(|o| String::from_utf8_lossy(&o).trim().to_owned())
    };
    if !dir.join(".git").exists() {
        let _ = fs::remove_dir_all(&dir);
        cmd::run(
            step,
            "git",
            &[
                "clone",
                "-q",
                "--depth",
                "1",
                "--branch",
                &pin.tag,
                &pin.repository,
                p(&dir),
            ],
            fail,
        )?;
    }
    let clean =
        cmd::output(step, "git", &["-C", p(&dir), "status", "--porcelain"], fail)?.is_empty();
    if head(&dir)? != pin.commit || !clean {
        let _ = fs::remove_dir_all(&dir);
        return Err(fail());
    }
    let tar = cmd::output(
        step,
        "git",
        &["-C", p(&dir), "archive", "--format=tar", "HEAD"],
        fail,
    )?;
    use sha2::Digest as _;
    let input = BuildInput {
        kind: BuildInputKind::Tool,
        identity: format!("curtin {} ({})", pin.tag, pin.license),
        digest: Sha256Hex(hex::encode(sha2::Sha256::digest(&tar))),
        trust: format!("git-commit:{}", pin.commit),
    };
    Ok((dir, input))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fonte_suja_ou_sem_commit_exacto_e_recusada() {
        assert_eq!(
            source("abc", true),
            Err(ImageBuildError::SourceNotCertified)
        );
        assert_eq!(
            source(&"a".repeat(40), false),
            Err(ImageBuildError::DirtySourceTree)
        );
        assert!(source(&"a".repeat(40), true).is_ok());
    }

    #[test]
    fn a_configuracao_da_base_do_repositorio_le_se() {
        let cfg: BaseConfig =
            serde_json::from_str(include_str!("../../../infra/image/base.json")).unwrap();
        assert_eq!(cfg.serial, "20261001");
        assert!(cfg.images.contains_key("amd64") && cfg.images.contains_key("arm64"));
        assert_eq!(cfg.sums_signing_key_fingerprint.len(), 40);
    }
}
