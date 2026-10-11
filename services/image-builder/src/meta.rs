//! B16–B17: inventory, SBOM, provenance, the image manifest, `SHA256SUMS`,
//! the development signature, and the re-verification of everything that
//! was written — including the signature, by the same code the Installer
//! will use (`ocinye_image_contracts::signing`).

use std::fs;
use std::path::{Path, PathBuf};

use ocinye_image_contracts::build::{Channel, ImageBuildError};
use ocinye_image_contracts::manifest::{Arch, OcinyeImageVersion, Sha256Hex};
use ocinye_image_contracts::signing::{
    self, Expected, KeyRole, Revocations, SignatureVerification, SignedStatement, TrustedKey,
    TrustedKeys,
};

use crate::cmd::{self, p};

/// Write canonical JSON and return its digest.
pub fn write_canonical<T: serde::Serialize>(
    path: &Path,
    v: &T,
) -> Result<Sha256Hex, ImageBuildError> {
    let s = ocinye_installer_contracts::canonical::to_canonical(v).map_err(|_| {
        ImageBuildError::ManifestFailed {
            field: path.display().to_string(),
        }
    })?;
    if let Some(d) = path.parent() {
        fs::create_dir_all(d).map_err(|_| ImageBuildError::ManifestFailed {
            field: "dir".into(),
        })?;
    }
    fs::write(path, &s).map_err(|_| ImageBuildError::ManifestFailed {
        field: path.display().to_string(),
    })?;
    Ok(Sha256Hex(
        cmd::sha256_file(path)
            .map_err(|_| ImageBuildError::ManifestFailed {
                field: "hash".into(),
            })?
            .0,
    ))
}

/// The syft invocation: a host-side scan of the mounted rootfs as a `dir:`
/// source. Syft reads the files; it never chroots into the target, nor runs a
/// binary from it, so the builder's own syft (its architecture) scans a
/// target rootfs of any architecture. Kept separate so a test can assert the
/// shape without running syft.
fn syft_scan_args(target: &str, out_spec: &str, name: &str) -> Vec<String> {
    vec![
        "scan".into(),
        target.into(),
        "-q".into(),
        "-o".into(),
        out_spec.into(),
        "--source-name".into(),
        name.into(),
    ]
}

/// SPDX 2.3 JSON of a mounted root filesystem, by syft.
pub fn sbom(root: &Path, out: &Path, name: &str) -> Result<Sha256Hex, ImageBuildError> {
    let target = format!("dir:{}", root.display());
    let o = format!("spdx-json={}", out.display());
    let args = syft_scan_args(&target, &o, name);
    let argv: Vec<&str> = args.iter().map(String::as_str).collect();
    cmd::run("B16", "syft", &argv, || ImageBuildError::SbomFailed {
        reason: "syft".into(),
    })?;
    // Fail closed: syft must have written a parseable SPDX 2.3 document. A
    // missing file reads as empty bytes and fails the parse — never a silent
    // empty digest.
    require_spdx_2_3(&fs::read(out).unwrap_or_default())?;
    Ok(Sha256Hex(
        cmd::sha256_file(out)
            .map_err(|_| ImageBuildError::SbomFailed {
                reason: "hash".into(),
            })?
            .0,
    ))
}

/// The SBOM document is SPDX 2.3 JSON (the only format D013 publishes).
///
/// # Errors
///
/// `SbomFailed` if the bytes are not JSON or `spdxVersion` is not `SPDX-2.3`.
fn require_spdx_2_3(bytes: &[u8]) -> Result<(), ImageBuildError> {
    let v: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|_| ImageBuildError::SbomFailed {
            reason: "json".into(),
        })?;
    if v["spdxVersion"].as_str() != Some("SPDX-2.3") {
        return Err(ImageBuildError::SbomFailed {
            reason: format!("spdxVersion {}", v["spdxVersion"]),
        });
    }
    Ok(())
}

/// Every file under `dir` (relative, sorted), except the signature itself.
pub fn published_files(dir: &Path) -> Vec<PathBuf> {
    fn walk(d: &Path, base: &Path, out: &mut Vec<PathBuf>) {
        if let Ok(entries) = fs::read_dir(d) {
            for e in entries.flatten() {
                let pth = e.path();
                if pth.is_dir() {
                    walk(&pth, base, out);
                } else if let Ok(rel) = pth.strip_prefix(base) {
                    out.push(rel.to_path_buf());
                }
            }
        }
    }
    let mut out = vec![];
    walk(dir, dir, &mut out);
    out.retain(|f| f != Path::new("SHA256SUMS") && !f.starts_with("signatures"));
    out.sort();
    out
}

/// `SHA256SUMS` over every published file, by relative path.
pub fn sha256sums(dir: &Path) -> Result<Vec<(String, Sha256Hex)>, ImageBuildError> {
    let mut entries = vec![];
    for rel in published_files(dir) {
        let name = rel.to_string_lossy().into_owned();
        let (h, _) = cmd::sha256_file(&dir.join(&rel))
            .map_err(|_| ImageBuildError::ArtifactVerificationFailed { file: name.clone() })?;
        entries.push((name, Sha256Hex(h)));
    }
    Ok(entries)
}

/// The development key: generated once per developer, outside the
/// repository and outside every image.
pub fn dev_key(dir: &Path) -> Result<(PathBuf, PathBuf), ImageBuildError> {
    let sk = dir.join("ocinye-dev.key");
    let pk = dir.join("ocinye-dev.pub");
    if !sk.exists() {
        fs::create_dir_all(dir).map_err(|_| ImageBuildError::SigningFailed)?;
        cmd::run(
            "B16",
            "minisign",
            &["-G", "-W", "-p", p(&pk), "-s", p(&sk)],
            || ImageBuildError::SigningFailed,
        )?;
    }
    Ok((sk, pk))
}

/// Sign `SHA256SUMS` with the development key (channel `development` only).
pub fn sign_dev(
    out: &Path,
    sk: &Path,
    image: &OcinyeImageVersion,
    arch: Arch,
    now: i64,
) -> Result<(), ImageBuildError> {
    let sums = out.join("SHA256SUMS");
    let digest = Sha256Hex(
        cmd::sha256_file(&sums)
            .map_err(|_| ImageBuildError::SigningFailed)?
            .0,
    );
    let tag = image.name().trim_start_matches("ocinye-os-").to_owned();
    let st = SignedStatement {
        image: tag,
        arch,
        channel: Channel::Development,
        sha256sums: digest,
        ts: now,
    };
    let sig_dir = out.join("signatures");
    fs::create_dir_all(&sig_dir).map_err(|_| ImageBuildError::SigningFailed)?;
    let comment = st.render();
    cmd::run(
        "B16",
        "minisign",
        &[
            "-S",
            "-H",
            "-s",
            p(sk),
            "-m",
            p(&sums),
            "-x",
            p(&sig_dir.join("SHA256SUMS.minisig")),
            "-t",
            &comment,
        ],
        || ImageBuildError::SigningFailed,
    )
}

/// B17: every file matches `SHA256SUMS`, and the signature verifies with
/// the development key for the development channel and this image.
pub fn verify(
    out: &Path,
    pk: &Path,
    image: &OcinyeImageVersion,
    arch: Arch,
    now: i64,
) -> Result<SignatureVerification, ImageBuildError> {
    let sums = fs::read(out.join("SHA256SUMS")).map_err(|_| {
        ImageBuildError::ArtifactVerificationFailed {
            file: "SHA256SUMS".into(),
        }
    })?;
    let listed =
        signing::parse_sha256sums(&sums).ok_or(ImageBuildError::ArtifactVerificationFailed {
            file: "SHA256SUMS".into(),
        })?;
    let actual = sha256sums(out)?;
    if listed != actual {
        let file = actual
            .iter()
            .find(|a| !listed.contains(a))
            .map_or_else(|| "missing".into(), |a| a.0.clone());
        return Err(ImageBuildError::ArtifactVerificationFailed { file });
    }
    let pub_line = fs::read_to_string(pk).map_err(|_| ImageBuildError::SigningFailed)?;
    let b64 = pub_line
        .lines()
        .nth(1)
        .unwrap_or_default()
        .trim()
        .to_owned();
    let key_id = signing::key_id_of_b64(&b64).ok_or(ImageBuildError::SigningFailed)?;
    let trusted = TrustedKeys {
        schema: 1,
        issued_at: ocinye_image_contracts::claim::rfc3339(now),
        keys: vec![TrustedKey {
            key_id,
            public_key: b64,
            role: KeyRole::Development,
            channels: vec![Channel::Development],
            not_before: ocinye_image_contracts::claim::rfc3339(now - 86_400),
            not_after: ocinye_image_contracts::claim::rfc3339(now + 86_400 * 395),
        }],
    };
    let revoked = Revocations {
        schema: 1,
        issued_at: ocinye_image_contracts::claim::rfc3339(now),
        keys: vec![],
        artifacts: vec![],
    };
    let minisig = fs::read_to_string(out.join("signatures/SHA256SUMS.minisig"))
        .map_err(|_| ImageBuildError::SigningFailed)?;
    let tag = image.name().trim_start_matches("ocinye-os-").to_owned();
    let v = signing::verify_release(
        &sums,
        &minisig,
        &trusted,
        &revoked,
        &Expected {
            image: &tag,
            arch,
            artifacts: &actual,
        },
        now,
    );
    match v {
        SignatureVerification::Valid { .. } => Ok(v),
        _ => Err(ImageBuildError::ArtifactVerificationFailed {
            file: format!(
                "signature: {}",
                serde_json::to_string(&v).unwrap_or_default()
            ),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Syft scans the mounted rootfs as a `dir:` source — host-side, never a
    /// chroot and never a binary executed from the target rootfs. This is what
    /// lets an arm64 builder syft produce an amd64 image's SBOM.
    #[test]
    fn syft_le_o_rootfs_montado_como_dir_sem_chroot() {
        let args = syft_scan_args(
            "dir:/work/mnt-metal",
            "spdx-json=/out/x-metal.spdx.json",
            "x",
        );
        assert_eq!(args[0], "scan");
        assert!(args[1].starts_with("dir:"), "source is a filesystem dir");
        assert!(
            !args.iter().any(|a| a == "chroot" || a.contains("chroot")),
            "no chroot into the target"
        );
        // The target rootfs path is only ever a `dir:` data source, never an
        // executable argument.
        assert!(args.iter().all(|a| a != "/work/mnt-metal"));
    }

    /// E: a syft document declaring SPDX 2.3 is accepted.
    #[test]
    fn spdx_2_3_e_aceite() {
        let doc = br#"{"spdxVersion":"SPDX-2.3","name":"x"}"#;
        require_spdx_2_3(doc).unwrap();
    }

    /// D (meta layer): another SPDX version, non-JSON, and the empty bytes an
    /// unwritten/missing SBOM file reads as all fail closed with `SbomFailed`.
    #[test]
    fn sbom_nao_2_3_ou_ausente_falha_fechado() {
        let older = br#"{"spdxVersion":"SPDX-2.2"}"#;
        assert!(matches!(
            require_spdx_2_3(older),
            Err(ImageBuildError::SbomFailed { .. })
        ));
        assert!(matches!(
            require_spdx_2_3(b"not json"),
            Err(ImageBuildError::SbomFailed { .. })
        ));
        // A missing output file reads as empty bytes (fs::read(..).unwrap_or_default()).
        assert!(matches!(
            require_spdx_2_3(b""),
            Err(ImageBuildError::SbomFailed { .. })
        ));
    }
}
