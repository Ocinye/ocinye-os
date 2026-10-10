//! `ocinye-installer-cli` — the headless driver of the Installer controller.
//!
//! The graphical Installer (`apps/installer`) is the product. This driver runs
//! **the same controller** from a JSON configuration, for the automated proofs
//! on disposable test servers (`scripts/installer-e2e.sh`). It decides nothing
//! the controller does not decide, and it is just as strict:
//!
//! - host trust is explicit: the fingerprint must be written in the
//!   configuration (`fingerprint` prints what the server presents);
//! - the credential is never printed: the driver reports that it arrived, and
//!   drops it;
//! - events are printed as they are — non-secret by contract.
//!
//! ```text
//! ocinye-installer-cli fingerprint CONFIG
//! ocinye-installer-cli preflight   CONFIG
//! ocinye-installer-cli install     CONFIG [--stop-after-phase P09 | --kill-after-phase P09]
//! ocinye-installer-cli reattach    CONFIG
//! ocinye-installer-cli resume      CONFIG
//! ocinye-installer-cli remove      CONFIG
//! ocinye-installer-cli recheck     CONFIG
//! ```

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::net::IpAddr;
use std::path::PathBuf;

use ocinye_contracts::Distribution;
use ocinye_installer_contracts::ident::{
    parse_port, EmailAddress, HostNameValue, InstallationId, InstanceName, PersonName, SshUser,
    TargetHost,
};
use ocinye_installer_contracts::plan::{
    AccessEndpoints, BoundEndpoint, Distributions, FirstAdmin, Identity, InstallationConfiguration,
    TlsPlan,
};
use ocinye_installer_contracts::protocol::Event;
use ocinye_installer_contracts::secret::SecretText;
use ocinye_installer_contracts::verification::LifecycleState;
use ocinye_installer_controller::installer::{Ending, Installer, Progress, Refusal, TlsFiles};
use ocinye_installer_controller::ssh::{Auth, Elevation, Target};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Who {
    name: String,
    email: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Admin {
    person: Who,
    privileged: Who,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Bound {
    host: String,
    distribution: String,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Tls {
    SelfSigned(String),
    Provided {
        cert: PathBuf,
        key: PathBuf,
        chain: Option<PathBuf>,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    bundle: PathBuf,
    state_dir: PathBuf,
    host: String,
    port: Option<String>,
    user: String,
    key: Option<PathBuf>,
    sudo_password_file: Option<PathBuf>,
    trust_fingerprint: Option<String>,
    instance_name: String,
    distributions: Vec<String>,
    canonical: String,
    #[serde(default)]
    bound: Vec<Bound>,
    tls: Tls,
    admin: Admin,
    /// Test-only: names made to resolve on purpose (controlled DNS fixture).
    #[serde(default)]
    resolve: BTreeMap<String, Vec<IpAddr>>,
    /// Test-only: where to keep the one-time credential (0600), so a proof
    /// can log in with it without it passing through any output. Only a
    /// plain file name, kept inside `state_dir`.
    #[serde(default)]
    credential_file: Option<String>,
}

fn fail(msg: &str) -> ! {
    eprintln!("RECUSADO {msg}");
    std::process::exit(1);
}

fn out(kind: &str, value: &impl serde::Serialize) {
    println!("{}", serde_json::json!({ "kind": kind, "value": value }));
}

fn config(path: &str) -> Config {
    let bytes = std::fs::read(path).unwrap_or_else(|_| fail("configuração ilegível"));
    serde_json::from_slice(&bytes).unwrap_or_else(|e| fail(&format!("configuração inválida: {e}")))
}

fn installation(c: &Config) -> InstallationConfiguration {
    let dists: Vec<Distribution> = c
        .distributions
        .iter()
        .map(|d| d.parse().unwrap_or_else(|_| fail("distribuição")))
        .collect();
    InstallationConfiguration {
        instance_name: InstanceName::parse(&c.instance_name)
            .unwrap_or_else(|_| fail("instance_name")),
        distributions: Distributions::new(dists).unwrap_or_else(|_| fail("distributions")),
        endpoints: AccessEndpoints {
            canonical: HostNameValue::parse(&c.canonical).unwrap_or_else(|_| fail("canonical")),
            bound: c
                .bound
                .iter()
                .map(|b| BoundEndpoint {
                    host: HostNameValue::parse(&b.host).unwrap_or_else(|_| fail("bound.host")),
                    distribution: b
                        .distribution
                        .parse()
                        .unwrap_or_else(|_| fail("bound.distribution")),
                })
                .collect(),
        },
        tls: TlsPlan::SelfSignedTest,
        admin: FirstAdmin {
            person: Identity {
                name: PersonName::parse(&c.admin.person.name)
                    .unwrap_or_else(|_| fail("person.name")),
                email: EmailAddress::parse(&c.admin.person.email)
                    .unwrap_or_else(|_| fail("person.email")),
            },
            privileged: Identity {
                name: PersonName::parse(&c.admin.privileged.name)
                    .unwrap_or_else(|_| fail("privileged.name")),
                email: EmailAddress::parse(&c.admin.privileged.email)
                    .unwrap_or_else(|_| fail("privileged.email")),
            },
        },
    }
}

fn refusal(r: &Refusal) -> ! {
    out("refused", r);
    std::process::exit(3);
}

async fn connect(c: &Config, inst: &mut Installer) {
    let target = Target {
        host: TargetHost::parse(&c.host).unwrap_or_else(|_| fail("host")),
        port: parse_port(c.port.as_deref().unwrap_or("22"))
            .unwrap_or_else(|_| fail("port"))
            .get(),
        user: SshUser::parse(&c.user).unwrap_or_else(|_| fail("user")),
    };
    let auth = c.key.clone().map_or(Auth::Agent, Auth::PrivateKey);
    inst.set_target(target, auth);
    match inst.test_connection().await {
        Ok(_) => {}
        Err(Refusal::HostKeyUnknown {
            algorithm,
            fingerprint,
        }) => {
            // Trust only the fingerprint the operator wrote down.
            if c.trust_fingerprint.as_deref() != Some(fingerprint.as_str()) {
                out(
                    "host_key_unknown",
                    &serde_json::json!({ "algorithm": algorithm, "fingerprint": fingerprint }),
                );
                std::process::exit(4);
            }
            inst.trust_host(&algorithm, &fingerprint)
                .unwrap_or_else(|e| refusal(&e));
            if let Err(e) = inst.test_connection().await {
                refusal(&e);
            }
        }
        Err(e) => refusal(&e),
    }
    out("probe", &inst.probe);
    if inst.elevation == Some(Elevation::SudoPassword) {
        let file = c
            .sudo_password_file
            .as_ref()
            .unwrap_or_else(|| fail("sudo_password_file"));
        let pw = std::fs::read_to_string(file).unwrap_or_else(|_| fail("sudo_password_file"));
        if let Err(e) = inst
            .set_sudo_password(SecretText::new(pw.trim().to_owned()))
            .await
        {
            refusal(&e);
        }
    }
    if let Err(e) = inst.start_bootstrap().await {
        refusal(&e);
    }
    out("facts", &inst.facts);
}

fn printer() -> (
    tokio::sync::mpsc::UnboundedSender<Progress>,
    tokio::task::JoinHandle<()>,
) {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Progress>();
    let h = tokio::spawn(async move {
        let mut last_upload = 0u64;
        while let Some(p) = rx.recv().await {
            if let Progress::Upload { done, total, .. } = &p {
                // One line per 5%.
                if *done < *total && done.saturating_sub(last_upload) < total / 20 {
                    continue;
                }
                last_upload = *done;
            }
            out("progress", &p);
        }
    });
    (tx, h)
}

fn save_session(c: &Config, inst: &Installer) {
    let s = serde_json::json!({
        "installation_id": inst.installation_id,
        "last_seq": inst.last_seq,
        "served_cert_sha256": inst.served_cert_sha256,
        "server_report": inst.server_report,
    });
    let _ = std::fs::write(c.state_dir.join("session.json"), s.to_string());
}

fn load_session(c: &Config, inst: &mut Installer) {
    let Ok(bytes) = std::fs::read(c.state_dir.join("session.json")) else {
        fail("sem sessão guardada");
    };
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_else(|_| fail("sessão"));
    let id = InstallationId::parse(v["installation_id"].as_str().unwrap_or_default())
        .unwrap_or_else(|_| fail("sessão"));
    inst.installation_id = id.clone();
    inst.plan = inst.saved_plan(&id);
    inst.last_seq = v["last_seq"].as_u64().unwrap_or(0);
    inst.served_cert_sha256 = v["served_cert_sha256"].as_str().map(str::to_owned);
    if let Ok(r) = serde_json::from_value(v["server_report"].clone()) {
        inst.server_report = r;
    }
}

/// The process exit status is the outcome, not the printed text: 0 only for a
/// completed installation, 10 a failed phase, 11 a stop at a safe point, 12 an
/// interrupted channel, 13 a refusal by the executor.
fn keep_credential(c: &Config, name: &str, value: &str) {
    use std::io::Write as _;
    use std::os::unix::fs::OpenOptionsExt as _;
    if name.is_empty() || name.contains('/') || name.starts_with('.') {
        fail("credential_file: só um nome de ficheiro");
    }
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(c.state_dir.join(name))
        .unwrap_or_else(|_| fail("credential_file"));
    f.write_all(value.as_bytes())
        .unwrap_or_else(|_| fail("credential_file"));
}

async fn finish(c: &Config, inst: &mut Installer, ending: Ending) -> i32 {
    save_session(c, inst);
    let code = match ending {
        Ending::Completed => 0,
        Ending::Failed { .. } => 10,
        Ending::Stopped { .. } => 11,
        Ending::Interrupted => 12,
        Ending::Refused(_) => 13,
    };
    out("ending", &ending);
    if let Some(s) = &inst.secret {
        // Never the value.
        out(
            "credential_received",
            &serde_json::json!({ "user": s.user, "expires_at": s.expires_at, "length": s.value.expose().len() }),
        );
    }
    if let (Some(name), Some(s)) = (&c.credential_file, &inst.secret) {
        keep_credential(c, name, s.value.expose());
    }
    inst.acknowledge_credential();
    if ending == Ending::Completed {
        match inst.verify_from_here(None).await {
            Ok(state) => out("lifecycle", state),
            Err(e) => refusal(&e),
        }
        out("verification", &inst.report);
        if let Some(r) = inst.receipt() {
            let path = c.state_dir.join(r.file_name());
            std::fs::write(&path, r.to_json()).unwrap_or_else(|_| fail("recibo"));
            out("receipt", &path);
        }
        save_session(c, inst);
    }
    code
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (cmd, path) = match args.as_slice() {
        [c, p, ..] => (c.as_str(), p.as_str()),
        _ => fail("uso: ocinye-installer-cli <fingerprint|preflight|install|reattach|resume|remove|recheck> CONFIG"),
    };
    let flag = |name: &str| args.windows(2).find(|w| w[0] == name).map(|w| w[1].clone());
    let stop_after = flag("--stop-after-phase");
    let kill_after = flag("--kill-after-phase");
    let c = config(path);
    std::fs::create_dir_all(&c.state_dir).unwrap_or_else(|_| fail("state_dir"));
    let mut inst = Installer::new(c.state_dir.clone());
    let mut code = 0;
    inst.resolver.fixed.clone_from(&c.resolve);

    if cmd == "fingerprint" {
        inst.set_target(
            Target {
                host: TargetHost::parse(&c.host).unwrap_or_else(|_| fail("host")),
                port: 22,
                user: SshUser::parse(&c.user).unwrap_or_else(|_| fail("user")),
            },
            Auth::Agent,
        );
        match inst.test_connection().await {
            Err(Refusal::HostKeyUnknown {
                algorithm,
                fingerprint,
            }) => {
                out(
                    "host_key_unknown",
                    &serde_json::json!({ "algorithm": algorithm, "fingerprint": fingerprint }),
                );
            }
            Err(e) => refusal(&e),
            Ok(_) => out("host_key_known", &inst.host_key),
        }
        return;
    }
    if cmd == "recheck" {
        load_session(&c, &mut inst);
        inst.open_release(&c.bundle).unwrap_or_else(|e| refusal(&e));
        connect(&c, &mut inst).await;
        // Both sides again: the server items from the bootstrap, the rest
        // from here. A report saved by an earlier session is not evidence.
        inst.verify_on_server()
            .await
            .unwrap_or_else(|e| refusal(&e));
        // The receipt states the hardware as found now (read-only).
        inst.discover_hardware()
            .await
            .unwrap_or_else(|e| refusal(&e));
        let complete = match inst.verify_from_here(None).await {
            Ok(state) => {
                out("lifecycle", state);
                *state != LifecycleState::InstallationIncomplete
            }
            Err(e) => refusal(&e),
        };
        out("verification", &inst.report);
        if let Some(r) = inst.receipt() {
            let path = c.state_dir.join(r.file_name());
            std::fs::write(&path, r.to_json()).unwrap_or_else(|_| fail("recibo"));
            out("receipt", &path);
        }
        save_session(&c, &inst);
        inst.end_session().await;
        // 14: the installation does not verify as complete.
        std::process::exit(if complete { 0 } else { 14 });
    }

    match inst.open_release(&c.bundle) {
        Ok(r) => out(
            "release",
            &serde_json::json!({ "id": r.manifest.release.id, "arch": r.manifest.target.arch, "build": r.manifest.release.build, "manifest_sha256": r.manifest.sha256() }),
        ),
        Err(e) => refusal(&e),
    }
    connect(&c, &mut inst).await;
    let (tx, printer) = printer();
    match cmd {
        "preflight" => {
            match inst.run_preflight(Some(&tx)).await {
                Ok(r) => {
                    out("preflight", r);
                    // 15: something blocks installing here.
                    if r.items.iter().any(|i| {
                        i.status == ocinye_installer_contracts::preflight::CheckStatus::Blocked
                    }) {
                        code = 15;
                    }
                }
                Err(e) => refusal(&e),
            }
            match inst.discover_hardware().await {
                Ok(h) => out("hardware", h),
                Err(e) => refusal(&e),
            }
        }
        "install" => {
            let report = inst
                .run_preflight(Some(&tx))
                .await
                .unwrap_or_else(|e| refusal(&e))
                .clone();
            out("preflight", &report);
            if report.incomplete.is_some() {
                out("incomplete_installation", &report.incomplete);
                std::process::exit(5);
            }
            let hw = inst
                .discover_hardware()
                .await
                .unwrap_or_else(|e| refusal(&e))
                .clone();
            out("hardware", &hw);
            let mut cfg = installation(&c);
            let tls_files = match &c.tls {
                Tls::SelfSigned(s) if s == "self-signed" => None,
                Tls::SelfSigned(_) => fail("tls"),
                Tls::Provided { cert, key, chain } => {
                    let hosts: Vec<HostNameValue> =
                        cfg.endpoints.hosts().into_iter().cloned().collect();
                    let v = ocinye_installer_controller::tls::validate(
                        cert,
                        key,
                        chain.as_deref(),
                        &hosts.iter().collect::<Vec<_>>(),
                        chrono::Utc::now(),
                    );
                    out("tls_validation", &v);
                    let Some(plan) = v.plan.clone() else {
                        fail("TLS inválido (I33)")
                    };
                    cfg.tls = plan;
                    Some(TlsFiles {
                        cert: cert.clone(),
                        key: key.clone(),
                        chain: chain.clone(),
                    })
                }
            };
            inst.configure(cfg, tls_files)
                .unwrap_or_else(|e| refusal(&e));
            let plan = inst.review().unwrap_or_else(|e| refusal(&e)).clone();
            out("plan", &plan);
            // Interruption proofs: stop at the safe point after a phase, or
            // kill this process after a phase (the Installer closed).
            let (tx2, mut rx2) = tokio::sync::mpsc::unbounded_channel::<Progress>();
            let control = inst.control().unwrap_or_else(|e| refusal(&e));
            let tx_print = tx.clone();
            let id = inst.installation_id.clone();
            let session = serde_json::json!({ "installation_id": id });
            let _ = std::fs::write(c.state_dir.join("session.json"), session.to_string());
            let watch = tokio::spawn(async move {
                while let Some(p) = rx2.recv().await {
                    let done = match &p {
                        Progress::Remote(env) => match &env.event {
                            Event::StepCompleted { phase } => Some(phase.code().to_owned()),
                            _ => None,
                        },
                        _ => None,
                    };
                    let _ = tx_print.send(p);
                    if done.is_some() && done == stop_after {
                        let _ = control
                            .send(&ocinye_installer_contracts::protocol::Command::Stop {
                                installation_id: id.clone(),
                            })
                            .await;
                    }
                    if done.is_some() && done == kill_after {
                        // As if the Installer were closed: no goodbye.
                        std::process::exit(9);
                    }
                }
            });
            let ending = inst.install(Some(&tx2)).await;
            drop(tx2);
            let _ = watch.await;
            match ending {
                Ok(e) => code = finish(&c, &mut inst, e).await,
                Err(e) => refusal(&e),
            }
        }
        "reattach" => {
            load_session(&c, &mut inst);
            match inst.reattach(Some(&tx)).await {
                Ok(e) => code = finish(&c, &mut inst, e).await,
                Err(e) => refusal(&e),
            }
        }
        "resume" => {
            load_session(&c, &mut inst);
            inst.run_preflight(Some(&tx))
                .await
                .unwrap_or_else(|e| refusal(&e));
            match inst.resume(Some(&tx)).await {
                Ok(e) => code = finish(&c, &mut inst, e).await,
                Err(e) => refusal(&e),
            }
        }
        "remove" => {
            load_session(&c, &mut inst);
            let id = inst.installation_id.clone();
            match inst.remove_incomplete(id).await {
                Ok(()) => out("removed", &true),
                Err(e) => refusal(&e),
            }
        }
        _ => fail("comando desconhecido"),
    }
    inst.end_session().await;
    drop(tx);
    let _ = printer.await;
    std::process::exit(code);
}
