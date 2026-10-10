//! `ocinye-bootstrap` — the temporary, constrained executor of the Ocinye OS
//! Installer on the target server (D011, ADR-0022).
//!
//! Uploaded by the Installer to `/tmp/ocinye-bootstrap-<random>/` for one
//! session, verified against the release manifest before it does anything,
//! and removed at the end (P16). It is not a service and not a shell: it
//! answers a closed set of typed commands (`ocinye-installer-contracts`) and
//! maps each to fixed operations — read-only inspection before the plan is
//! confirmed, the sealed plan's phases after.
//!
//! ```text
//! ocinye-bootstrap serve                                  protocol on stdin/stdout (SSH)
//! ocinye-bootstrap run --installation ID --upload DIR     the detached executor
//! ocinye-bootstrap --version
//! ```

#![forbid(unsafe_code)]

mod daemon;
mod exec;
mod facts;
mod hardware;
mod phases;
mod preflight;
mod serve;
mod state;
mod verify;

use std::io::BufReader;
use std::path::PathBuf;

use ocinye_installer_contracts::ident::InstallationId;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["serve"] => {
            let mut stdin = BufReader::new(std::io::stdin());
            let mut stdout = std::io::stdout();
            serve::serve(&mut stdin, &mut stdout, &state::StateRoot::system())
        }
        ["run", "--installation", id, "--upload", upload] => {
            let Ok(id) = InstallationId::parse(id) else {
                eprintln!("--installation inválido");
                std::process::exit(2);
            };
            let upload = PathBuf::from(upload);
            if !phases::is_upload_dir(&upload) {
                eprintln!("--upload inválido");
                std::process::exit(2);
            }
            match daemon::run(&state::StateRoot::system(), &id, &upload) {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("executor: {e}");
                    1
                }
            }
        }
        ["--version"] => {
            println!(
                "ocinye-bootstrap {} (protocol {})",
                env!("CARGO_PKG_VERSION"),
                ocinye_installer_contracts::BOOTSTRAP_PROTOCOL
            );
            0
        }
        _ => {
            eprintln!(
                "uso: ocinye-bootstrap serve | run --installation ID --upload DIR | --version"
            );
            2
        }
    };
    std::process::exit(code);
}

#[cfg(test)]
pub(crate) mod tests {
    //! A bundle with a valid manifest, for the tests of the executor.

    use std::fs;
    use std::path::Path;

    use ocinye_installer_contracts::canonical::sha256_hex;
    use ocinye_installer_contracts::manifest::*;

    pub(crate) fn fixture_plan() -> ocinye_installer_contracts::plan::InstallationPlan {
        use ocinye_contracts::Distribution;
        use ocinye_installer_contracts::hardware::*;
        use ocinye_installer_contracts::ident::*;
        use ocinye_installer_contracts::plan::*;
        let h = |s: &str| HostNameValue::parse(s).unwrap();
        // One directory per call: tests run in parallel in one process, and a
        // shared one is created by one test while another removes it.
        static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("ocinye-plan-{}-{n}", std::process::id()));
        let m = fixture_manifest(&dir);
        let _ = fs::remove_dir_all(&dir);
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
            configuration: InstallationConfiguration {
                instance_name: InstanceName::parse("Empresa Exemplo").unwrap(),
                distributions: Distributions::new(vec![
                    Distribution::Business,
                    Distribution::Research,
                ])
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
            },
            system_changes: vec![],
            preflight: vec![],
            hardware: HardwareSummary {
                threads: 2,
                memory_mb: 4096,
                gpus: 0,
                compute_readiness: ComputeReadiness::CpuOnlyFutureCandidate,
                provider_mode: ProviderMode::Off,
            },
            phases: InstallationPlan::phases_for(&[]),
            plan_sha256: String::new(),
        }
        .seal()
    }

    pub(crate) fn fixture_manifest(dir: &Path) -> ReleaseManifest {
        let write = |p: &str, content: &[u8]| {
            let path = dir.join(p);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, content).unwrap();
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
        let mut artifacts = Vec::new();
        for (p, c) in &files {
            write(p, c);
            artifacts.push(Artifact {
                path: p.clone(),
                sha256: sha256_hex(c),
                bytes: c.len() as u64,
            });
        }
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
        m.validate().unwrap();
        let bytes = m.canonical_bytes();
        write("MANIFEST.json", bytes.as_bytes());
        let mut sums = String::new();
        let mut all: Vec<(String, Vec<u8>)> = files;
        all.push(("MANIFEST.json".into(), bytes.into_bytes()));
        all.sort();
        for (p, c) in all {
            sums.push_str(&format!("{}  ./{p}\n", sha256_hex(&c)));
        }
        write("SHA256SUMS", sums.as_bytes());
        m
    }
}
