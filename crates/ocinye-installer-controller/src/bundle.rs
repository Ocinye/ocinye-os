//! The release bundle, verified **locally, before anything is sent** (I02).
//!
//! `MANIFEST.json` must parse and validate; every artifact must exist with the
//! size and SHA-256 the manifest gives; `SHA256SUMS` must cover every file and
//! agree; the architecture must be one Ocinye runs on. Any failure is a hard
//! stop for this package (I20 · I21 · I22): nothing reaches a server.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use ocinye_installer_contracts::canonical::sha256_hex;
use ocinye_installer_contracts::manifest::{self, ReleaseManifest};
use serde::Serialize;
use sha2::{Digest, Sha256};

/// A bundle that passed every local check.
#[derive(Debug, Clone)]
pub struct VerifiedRelease {
    /// The bundle directory.
    pub dir: PathBuf,
    /// The manifest.
    pub manifest: ReleaseManifest,
    /// Its exact bytes (sent as is).
    pub manifest_bytes: Vec<u8>,
    /// Every file to transfer, relative, with its size.
    pub files: Vec<(String, u64)>,
}

/// Why a bundle was refused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReleaseRejection {
    /// I20.
    ChecksumFailed {
        /// The file.
        file: String,
    },
    /// I21.
    UnsupportedArchitecture {
        /// What the manifest says.
        arch: String,
    },
    /// I22.
    InvalidManifest {
        /// The field.
        field: String,
    },
}

fn sha256_file(path: &Path) -> std::io::Result<(String, u64)> {
    let mut f = fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    let mut n = 0u64;
    loop {
        let r = f.read(&mut buf)?;
        if r == 0 {
            break;
        }
        n += r as u64;
        h.update(&buf[..r]);
    }
    Ok((hex::encode(h.finalize()), n))
}

fn walk(dir: &Path, base: &Path, out: &mut Vec<String>) -> std::io::Result<()> {
    for e in fs::read_dir(dir)? {
        let e = e?;
        let p = e.path();
        let ty = e.file_type()?;
        if ty.is_symlink() {
            // A link inside a bundle points somewhere the sums do not cover.
            return Err(std::io::Error::other("symlink"));
        }
        if ty.is_dir() {
            walk(&p, base, out)?;
        } else if let Ok(rel) = p.strip_prefix(base) {
            out.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(())
}

/// Verify a bundle directory.
///
/// # Errors
///
/// The [`ReleaseRejection`].
pub fn verify(dir: &Path) -> Result<VerifiedRelease, ReleaseRejection> {
    let invalid = |field: &str| ReleaseRejection::InvalidManifest {
        field: field.to_owned(),
    };
    let bytes = fs::read(dir.join("MANIFEST.json")).map_err(|_| invalid("MANIFEST.json"))?;
    // The architecture first, to say I21 rather than a generic I22.
    if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&bytes) {
        if let Some(arch) = v.pointer("/target/arch").and_then(|a| a.as_str()) {
            if !matches!(arch, "amd64" | "arm64") {
                return Err(ReleaseRejection::UnsupportedArchitecture {
                    arch: arch.to_owned(),
                });
            }
        }
    }
    let manifest = ReleaseManifest::parse(&bytes).map_err(|e| invalid(&e.field))?;

    let mut listed = Vec::new();
    walk(dir, dir, &mut listed).map_err(|_| invalid("bundle"))?;
    listed.sort();
    let sums = fs::read_to_string(dir.join("SHA256SUMS")).map_err(|_| invalid("SHA256SUMS"))?;
    let mut summed = Vec::new();
    for line in sums.lines().filter(|l| !l.trim().is_empty()) {
        let (path, sum) = manifest::parse_sums_line(line).ok_or_else(|| invalid("SHA256SUMS"))?;
        let (got, _) = sha256_file(&dir.join(&path))
            .map_err(|_| ReleaseRejection::ChecksumFailed { file: path.clone() })?;
        if got != sum {
            return Err(ReleaseRejection::ChecksumFailed { file: path });
        }
        summed.push(path);
    }
    summed.sort();
    // Every file but SHA256SUMS is summed, and nothing summed is missing.
    let expected: Vec<String> = listed
        .iter()
        .filter(|p| *p != "SHA256SUMS")
        .cloned()
        .collect();
    if summed != expected {
        return Err(invalid("SHA256SUMS"));
    }
    for a in &manifest.artifacts {
        let (got, n) =
            sha256_file(&dir.join(&a.path)).map_err(|_| ReleaseRejection::ChecksumFailed {
                file: a.path.clone(),
            })?;
        if got != a.sha256 || n != a.bytes {
            return Err(ReleaseRejection::ChecksumFailed {
                file: a.path.clone(),
            });
        }
    }
    if sha256_hex(&bytes) != manifest.sha256() {
        // The file on disk is not the canonical form the sums were taken over.
        return Err(invalid("MANIFEST.json"));
    }
    let files = expected
        .iter()
        .chain(std::iter::once(&"SHA256SUMS".to_owned()))
        .map(|p| {
            let n = fs::metadata(dir.join(p)).map(|m| m.len()).unwrap_or(0);
            (p.clone(), n)
        })
        .collect();
    Ok(VerifiedRelease {
        dir: dir.to_path_buf(),
        manifest,
        manifest_bytes: bytes,
        files,
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use ocinye_installer_contracts::manifest::*;

    /// A complete, valid bundle in a temporary directory.
    pub(crate) fn fixture(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ocinye-bundle-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let write = |p: &str, c: &[u8]| {
            let path = dir.join(p);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, c).unwrap();
        };
        let mut files: Vec<(String, Vec<u8>)> = vec![
            ("source.tar.gz".into(), b"source".to_vec()),
            ("install/ocinye".into(), b"#!/bin/bash\n".to_vec()),
            ("ocinye-bootstrap".into(), b"bootstrap".to_vec()),
        ];
        for svc in SERVICES {
            files.push((
                format!("images/ocinye-{svc}.tar"),
                format!("image {svc}").into_bytes(),
            ));
        }
        let artifacts: Vec<Artifact> = files
            .iter()
            .map(|(p, c)| {
                write(p, c);
                Artifact {
                    path: p.clone(),
                    sha256: sha256_hex(c),
                    bytes: c.len() as u64,
                }
            })
            .collect();
        let boot = artifacts
            .iter()
            .find(|a| a.path == "ocinye-bootstrap")
            .unwrap()
            .sha256
            .clone();
        let m = ReleaseManifest {
            schema: 1,
            product: "ocinye-os".into(),
            release: ReleaseInfo {
                id: "3f9c2a7d1e04".into(),
                commit: "3f9c2a7d1e04b7c19e6a5d20f81c4e7a9b03d6f2".into(),
                build: BuildKind::Proof,
                created_at: "2026-10-04T12:00:00Z".into(),
                source_date_epoch: 1_790_000_000,
            },
            target: Target {
                os: "linux".into(),
                arch: Arch::Arm64,
            },
            artifacts,
            images: SERVICES
                .iter()
                .map(|svc| Image {
                    name: format!("ocinye/ocinye-{svc}"),
                    tag: "3f9c2a7d1e04".into(),
                    archive: format!("images/ocinye-{svc}.tar"),
                    image_id: format!("sha256:{}", "9".repeat(64)),
                })
                .collect(),
            third_party_images: vec![],
            migrations: Migrations {
                count: 64,
                latest: "0064".into(),
                files_sha256: "4".repeat(64),
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
                sha256: boot,
                is_static: true,
            },
        };
        let bytes = m.canonical_bytes();
        write("MANIFEST.json", bytes.as_bytes());
        write("RELEASE", b"3f9c2a7d1e04\n");
        files.push(("MANIFEST.json".into(), bytes.into_bytes()));
        files.push(("RELEASE".into(), b"3f9c2a7d1e04\n".to_vec()));
        files.sort();
        let sums: String = files
            .iter()
            .map(|(p, c)| format!("{}  ./{p}\n", sha256_hex(c)))
            .collect();
        write("SHA256SUMS", sums.as_bytes());
        dir
    }

    #[test]
    fn um_pacote_integro_passa() {
        let dir = fixture("ok");
        let r = verify(&dir).unwrap();
        assert_eq!(r.manifest.release.id, "3f9c2a7d1e04");
        assert!(r.files.iter().any(|(p, _)| p == "SHA256SUMS"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn um_byte_alterado_numa_imagem_e_recusado() {
        let dir = fixture("checksum");
        fs::write(
            dir.join("images/ocinye-core-server.tar"),
            b"image core-server!",
        )
        .unwrap();
        assert_eq!(
            verify(&dir).unwrap_err(),
            ReleaseRejection::ChecksumFailed {
                file: "images/ocinye-core-server.tar".into()
            }
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn um_ficheiro_a_mais_sem_soma_e_recusado() {
        let dir = fixture("extra");
        fs::write(dir.join("images/intruso.tar"), b"x").unwrap();
        assert!(matches!(
            verify(&dir),
            Err(ReleaseRejection::InvalidManifest { .. })
        ));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn uma_arquitectura_desconhecida_e_i21() {
        let dir = fixture("arch");
        let mut v: serde_json::Value =
            serde_json::from_slice(&fs::read(dir.join("MANIFEST.json")).unwrap()).unwrap();
        v["target"]["arch"] = "riscv64".into();
        fs::write(dir.join("MANIFEST.json"), v.to_string()).unwrap();
        assert_eq!(
            verify(&dir).unwrap_err(),
            ReleaseRejection::UnsupportedArchitecture {
                arch: "riscv64".into()
            }
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn um_manifesto_invalido_diz_o_campo() {
        let dir = fixture("manifest");
        let mut v: serde_json::Value =
            serde_json::from_slice(&fs::read(dir.join("MANIFEST.json")).unwrap()).unwrap();
        v["migrations"]["latest"] = "0063".into();
        fs::write(dir.join("MANIFEST.json"), v.to_string()).unwrap();
        assert_eq!(
            verify(&dir).unwrap_err(),
            ReleaseRejection::InvalidManifest {
                field: "migrations.latest".into()
            }
        );
        let _ = fs::remove_dir_all(dir);
    }
}
