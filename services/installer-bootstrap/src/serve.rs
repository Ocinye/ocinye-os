//! `ocinye-bootstrap serve`: the protocol on the SSH channel (stdin/stdout).
//!
//! The first command must be `Hello`, and the bootstrap refuses to go further
//! unless its own executable hashes to what the release manifest says
//! (`BOOTSTRAP_HASH_MISMATCH`, hard stop). Read-only commands are answered here.
//! `Execute`/`Resume` start the detached executor and turn this process into a
//! relay to it.

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Command as Process, Stdio};
use std::time::{Duration, Instant};

use ocinye_installer_contracts::ident::InstallationId;
use ocinye_installer_contracts::journal::InstallationJournal;
use ocinye_installer_contracts::manifest::ReleaseManifest;
use ocinye_installer_contracts::plan::{InstallationPlan, PhaseId};
use ocinye_installer_contracts::protocol::{Command, Envelope, Event};
use ocinye_installer_contracts::verification::VerificationReport;
use ocinye_installer_contracts::{paths, BOOTSTRAP_PROTOCOL};

use crate::exec::{self, program};
use crate::phases;
use crate::state::{self, StateRoot};

struct Out<W: Write> {
    w: W,
}

impl<W: Write> Out<W> {
    fn emit(&mut self, event: Event) {
        let env = Envelope {
            seq: 0,
            at: state::now(),
            event,
        };
        let _ = writeln!(
            self.w,
            "{}",
            serde_json::to_string(&env).unwrap_or_default()
        );
        let _ = self.w.flush();
    }

    fn refuse(&mut self, code: &str) {
        self.emit(Event::Refused { code: code.into() });
    }
}

fn upload_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?.to_path_buf();
    phases::is_upload_dir(&dir).then_some(dir)
}

fn connect(sock: &Path, wait: Duration) -> Option<UnixStream> {
    let started = Instant::now();
    loop {
        if let Ok(s) = UnixStream::connect(sock) {
            return Some(s);
        }
        if started.elapsed() > wait {
            return None;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

fn spawn_executor(dir: &Path, id: &InstallationId, upload: &Path) -> std::io::Result<()> {
    let exe = dir.join("ocinye-bootstrap");
    let log = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("executor.log"))?;
    fs::set_permissions(dir.join("executor.log"), fs::Permissions::from_mode(0o600))?;
    Process::new(program::SETSID)
        .args(["-f"])
        .arg(&exe)
        .args(["run", "--installation", id.as_str(), "--upload"])
        .arg(upload)
        .env_clear()
        .env(
            "PATH",
            "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
        )
        .stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log)
        .spawn()?
        .wait()?;
    Ok(())
}

fn read_bundle_manifest(upload: &Path) -> Option<(Vec<u8>, ReleaseManifest)> {
    let bytes = fs::read(upload.join("bundle/MANIFEST.json")).ok()?;
    let m = ReleaseManifest::parse(&bytes).ok()?;
    Some((bytes, m))
}

fn prepare_dir(
    root: &StateRoot,
    plan: &InstallationPlan,
    manifest_bytes: &[u8],
    expected: &str,
) -> Result<PathBuf, &'static str> {
    let dir = root
        .create(&plan.installation_id)
        .map_err(|_| "FILESYSTEM_ERROR")?;
    let exe = std::env::current_exe().map_err(|_| "FILESYSTEM_ERROR")?;
    let target = dir.join("ocinye-bootstrap");
    fs::copy(&exe, &target).map_err(|_| "FILESYSTEM_ERROR")?;
    fs::set_permissions(&target, fs::Permissions::from_mode(0o700))
        .map_err(|_| "FILESYSTEM_ERROR")?;
    let copied = {
        use sha2::{Digest, Sha256};
        hex::encode(Sha256::digest(
            fs::read(&target).map_err(|_| "FILESYSTEM_ERROR")?,
        ))
    };
    if copied != expected {
        return Err("BOOTSTRAP_HASH_MISMATCH");
    }
    state::write_atomic(
        &dir.join("plan.json"),
        &serde_json::to_vec_pretty(plan).map_err(|_| "FILESYSTEM_ERROR")?,
        0o600,
    )
    .map_err(|_| "FILESYSTEM_ERROR")?;
    state::write_atomic(&dir.join("manifest.json"), manifest_bytes, 0o600)
        .map_err(|_| "FILESYSTEM_ERROR")?;
    Ok(dir)
}

/// The executor is running and answering.
fn executor_alive(dir: &Path) -> Option<UnixStream> {
    connect(&dir.join("control.sock"), Duration::from_millis(500))
}

fn remove_incomplete(journal: &InstallationJournal, dir: &Path) -> Result<(), &'static str> {
    if !journal.removal_allowed() {
        return Err("REMOVAL_NOT_ALLOWED");
    }
    let script = phases::staging(&journal.installation_id).join("install/ocinye");
    if Path::new(&format!("{}/release.env", paths::CONFIG)).exists()
        && Path::new(&format!("{}/current", paths::ROOT)).exists()
        && script.exists()
    {
        let o = exec::run_path(&script, &["uninstall", "--purge"], exec::secs(900));
        if !o.ok() {
            return Err("REMOVAL_FAILED");
        }
    }
    // Only what the journal says the Installer created.
    const OWNED: [&str; 4] = [
        paths::ROOT,
        paths::CONFIG,
        "/etc/apt/keyrings/docker.asc",
        "/etc/apt/sources.list.d/docker.sources",
    ];
    for p in &journal.created_paths {
        if !OWNED.contains(&p.as_str()) {
            continue;
        }
        let path = Path::new(p);
        let _ = if path.is_dir() {
            fs::remove_dir_all(path)
        } else {
            fs::remove_file(path)
        };
    }
    for rule in &journal.firewall_rules {
        if matches!(rule.as_str(), "80/tcp" | "443/tcp") {
            let _ = exec::run(program::UFW, &["delete", "allow", rule], exec::secs(60));
        }
    }
    if !journal.installed_packages.is_empty() {
        let mut args = vec!["-o", "DPkg::Lock::Timeout=300", "-y", "purge"];
        args.extend(journal.installed_packages.iter().map(String::as_str));
        let _ = exec::run(program::APT_GET, &args, exec::secs(900));
    }
    fs::remove_dir_all(dir).map_err(|_| "REMOVAL_FAILED")
}

/// Serve one SSH session.
#[allow(clippy::too_many_lines)]
pub fn serve(stdin: &mut (dyn BufRead + Send), stdout: &mut dyn Write, root: &StateRoot) -> i32 {
    let mut out = Out { w: &mut *stdout };
    let mut hello: Option<String> = None;
    let mut line = String::new();
    loop {
        line.clear();
        match stdin.read_line(&mut line) {
            Ok(0) | Err(_) => return 0,
            Ok(_) => {}
        }
        let Ok(cmd) = serde_json::from_str::<Command>(line.trim()) else {
            out.refuse("COMMAND_UNKNOWN");
            continue;
        };
        let Some(expected) = hello.clone() else {
            let Command::Hello {
                protocol,
                expected_sha256,
            } = cmd
            else {
                out.refuse("HELLO_REQUIRED");
                continue;
            };
            if protocol != BOOTSTRAP_PROTOCOL {
                out.refuse("PROTOCOL_UNSUPPORTED");
                return 3;
            }
            match phases::self_sha256() {
                Ok(own) if own == expected_sha256.as_str() => {}
                _ => {
                    out.refuse("BOOTSTRAP_HASH_MISMATCH");
                    return 3;
                }
            }
            hello = Some(expected_sha256.as_str().to_owned());
            out.emit(Event::Connected {
                protocol: BOOTSTRAP_PROTOCOL,
            });
            continue;
        };
        match cmd {
            Command::Hello { .. } => out.refuse("ALREADY_CONNECTED"),
            Command::Inspect {} => out.emit(Event::Facts {
                facts: crate::facts::read(),
            }),
            Command::Preflight {
                release_arch,
                operator_unix_time,
            } => {
                out.emit(Event::PreflightStarted);
                let facts = crate::facts::read();
                let mut items = Vec::new();
                let report = crate::preflight::run(
                    &facts,
                    release_arch,
                    operator_unix_time,
                    &crate::hardware::Root::system(),
                    root,
                    &mut |i| items.push(i.clone()),
                );
                for item in items {
                    out.emit(Event::PreflightItemResult { item });
                }
                out.emit(Event::PreflightCompleted { report });
            }
            Command::DiscoverHardware {} => {
                out.emit(Event::HardwareDiscoveryStarted);
                let hardware = crate::hardware::discover(
                    &crate::hardware::Root::system(),
                    &crate::facts::machine(),
                );
                out.emit(Event::HardwareDiscoveryCompleted { hardware });
            }
            Command::ReadJournal {} => {
                let journals = root.journals();
                let pick = journals
                    .iter()
                    .find(|j| {
                        j.state != ocinye_installer_contracts::journal::JournalState::Completed
                    })
                    .or_else(|| journals.first());
                out.emit(Event::JournalRead {
                    journal: pick.map(InstallationJournal::summary),
                });
            }
            Command::Execute {
                plan,
                confirmed_sha256,
            } => {
                if crate::facts::effective_uid() != Some(0) {
                    out.refuse("PRIVILEGE_MISSING");
                    continue;
                }
                let Some(upload) = upload_dir() else {
                    out.refuse("UPLOAD_DIR_INVALID");
                    continue;
                };
                if plan.plan_sha256 != confirmed_sha256 {
                    out.refuse("PLAN_NOT_CONFIRMED");
                    continue;
                }
                let Some((bytes, manifest)) = read_bundle_manifest(&upload) else {
                    out.refuse("TRANSFER_INCOMPLETE");
                    continue;
                };
                if manifest.sha256() != plan.release.manifest_sha256 {
                    out.refuse("TRANSFER_CHECKSUM_MISMATCH");
                    continue;
                }
                if let Err(r) = plan.check(&manifest) {
                    out.refuse(&format!("PLAN_REFUSED:{r:?}"));
                    continue;
                }
                if manifest.bootstrap.sha256 != expected {
                    out.refuse("BOOTSTRAP_HASH_MISMATCH");
                    continue;
                }
                if state::read_journal(&root.dir(&plan.installation_id)).is_some() {
                    out.refuse("ALREADY_STARTED");
                    continue;
                }
                match crate::daemon::global_lock(root) {
                    Some(lock) => drop(lock),
                    None => {
                        out.refuse("INSTALLATION_IN_PROGRESS");
                        continue;
                    }
                }
                let dir = match prepare_dir(root, &plan, &bytes, &expected) {
                    Ok(d) => d,
                    Err(code) => {
                        out.refuse(code);
                        continue;
                    }
                };
                let skipped: Vec<PhaseId> = plan
                    .phases
                    .iter()
                    .filter(|p| p.skip)
                    .map(|p| p.phase)
                    .collect();
                let journal = InstallationJournal::new(
                    plan.installation_id.clone(),
                    plan.plan_id.clone(),
                    plan.plan_sha256.clone(),
                    plan.release.id.clone(),
                    plan.target.host_key_sha256.clone(),
                    &skipped,
                    state::now(),
                );
                if state::write_journal(&dir, &journal).is_err()
                    || spawn_executor(&dir, &plan.installation_id, &upload).is_err()
                {
                    out.refuse("EXECUTOR_START_FAILED");
                    continue;
                }
                out.emit(Event::ExecutionAccepted {
                    installation_id: plan.installation_id.clone(),
                });
                let Some(sock) = connect(&dir.join("control.sock"), Duration::from_secs(30)) else {
                    out.refuse("EXECUTOR_UNREACHABLE");
                    continue;
                };
                return relay_session(sock, &plan.installation_id, 1, stdin, stdout);
            }
            Command::Resume {
                plan,
                confirmed_sha256,
            } => {
                let dir = root.dir(&plan.installation_id);
                let Some(journal) = state::read_journal(&dir) else {
                    out.refuse("NO_JOURNAL");
                    continue;
                };
                if plan.plan_sha256 != confirmed_sha256 || journal.plan_sha256 != plan.plan_sha256 {
                    out.refuse("PLAN_NOT_CONFIRMED");
                    continue;
                }
                let from = state::last_seq(&dir) + 1;
                if let Some(sock) = executor_alive(&dir) {
                    return relay_session(sock, &plan.installation_id, from, stdin, stdout);
                }
                let Some(upload) = upload_dir() else {
                    out.refuse("UPLOAD_DIR_INVALID");
                    continue;
                };
                // The executor binary in the state directory may have been
                // removed (P16 never ran, or ran partially): put this verified
                // one there again.
                let manifest_bytes = fs::read(dir.join("manifest.json")).unwrap_or_default();
                if let Err(code) = prepare_dir(root, &plan, &manifest_bytes, &expected) {
                    out.refuse(code);
                    continue;
                }
                if spawn_executor(&dir, &plan.installation_id, &upload).is_err() {
                    out.refuse("EXECUTOR_START_FAILED");
                    continue;
                }
                out.emit(Event::ExecutionAccepted {
                    installation_id: plan.installation_id.clone(),
                });
                let Some(sock) = connect(&dir.join("control.sock"), Duration::from_secs(30)) else {
                    out.refuse("EXECUTOR_UNREACHABLE");
                    continue;
                };
                return relay_session(sock, &plan.installation_id, from, stdin, stdout);
            }
            Command::Attach {
                installation_id,
                from_seq,
            } => {
                let dir = root.dir(&installation_id);
                if let Some(sock) = executor_alive(&dir) {
                    return relay_session(sock, &installation_id, from_seq, stdin, stdout);
                }
                for env in state::events_from(&dir, from_seq) {
                    let _ = writeln!(out.w, "{}", serde_json::to_string(&env).unwrap_or_default());
                }
                out.emit(Event::JournalRead {
                    journal: state::read_journal(&dir).map(|j| j.summary()),
                });
            }
            Command::Verify { installation_id } => {
                let dir = root.dir(&installation_id);
                let plan: Option<InstallationPlan> = fs::read(dir.join("plan.json"))
                    .ok()
                    .and_then(|b| serde_json::from_slice(&b).ok());
                let manifest = fs::read(dir.join("manifest.json"))
                    .ok()
                    .and_then(|b| ReleaseManifest::parse(&b).ok());
                let (Some(plan), Some(manifest)) = (plan, manifest) else {
                    out.refuse("NO_JOURNAL");
                    continue;
                };
                if executor_alive(&dir).is_some() {
                    out.refuse("EXECUTOR_BUSY");
                    continue;
                }
                out.emit(Event::VerificationStarted);
                let mut items = Vec::new();
                crate::verify::run(&plan, &manifest, &mut |i| items.push(i.clone()));
                for item in &items {
                    out.emit(Event::VerificationItem { item: item.clone() });
                }
                out.emit(Event::VerificationCompleted {
                    report: VerificationReport { items },
                });
            }
            Command::Stop { .. } | Command::ClaimSecret { .. } => {
                out.refuse("NO_RUNNING_INSTALLATION")
            }
            Command::RemoveIncomplete { installation_id } => {
                let dir = root.dir(&installation_id);
                if executor_alive(&dir).is_some() {
                    out.refuse("EXECUTOR_BUSY");
                    continue;
                }
                let Some(journal) = state::read_journal(&dir) else {
                    out.refuse("NO_JOURNAL");
                    continue;
                };
                match remove_incomplete(&journal, &dir) {
                    Ok(()) => out.emit(Event::IncompleteRemoved { installation_id }),
                    Err(code) => out.refuse(code),
                }
            }
            Command::Cleanup {} => {
                if let Some(upload) = upload_dir() {
                    let _ = fs::remove_dir_all(upload);
                }
                out.emit(Event::CleanedUp);
                return 0;
            }
        }
    }
}

/// Relay commands from stdin to the executor and events back, until the
/// executor closes the socket or the SSH channel ends.
fn relay_session(
    sock: UnixStream,
    id: &InstallationId,
    from_seq: u64,
    stdin: &mut (dyn BufRead + Send),
    stdout: &mut dyn Write,
) -> i32 {
    let Ok(mut to_executor) = sock.try_clone() else {
        return 1;
    };
    let attach = Command::Attach {
        installation_id: id.clone(),
        from_seq,
    };
    if writeln!(
        to_executor,
        "{}",
        serde_json::to_string(&attach).unwrap_or_default()
    )
    .is_err()
    {
        return 1;
    }
    // Events: executor → stdout, on this thread's companion.
    let (tx, rx) = std::sync::mpsc::channel::<Option<String>>();
    {
        let tx = tx.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(sock).lines() {
                let Ok(line) = line else { break };
                if tx.send(Some(line)).is_err() {
                    return;
                }
            }
            let _ = tx.send(None);
        });
    }
    // Commands: stdin → executor, on a scoped thread (stdin is borrowed).
    std::thread::scope(|scope| {
        scope.spawn(move || {
            let mut line = String::new();
            loop {
                line.clear();
                match stdin.read_line(&mut line) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        // Only parseable commands go through.
                        if serde_json::from_str::<Command>(line.trim()).is_ok()
                            && writeln!(to_executor, "{}", line.trim()).is_err()
                        {
                            break;
                        }
                    }
                }
            }
            let _ = to_executor.shutdown(std::net::Shutdown::Write);
        });
        while let Ok(Some(line)) = rx.recv() {
            if writeln!(stdout, "{line}").is_err() {
                break;
            }
            let _ = stdout.flush();
        }
    });
    0
}
