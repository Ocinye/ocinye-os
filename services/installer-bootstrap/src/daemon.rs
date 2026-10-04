//! The detached executor: `ocinye-bootstrap run`.
//!
//! An installation must survive the SSH session that started it — the
//! operator's laptop sleeping, a network drop, the Installer being closed.
//! So `Execute` does not run the phases in the SSH session: it starts this
//! process in its own session (`setsid`), and the SSH-side `serve` becomes a
//! relay to its control socket. Losing the relay loses nothing; a new one
//! reattaches with `Attach{from_seq}` and receives what it missed from
//! `events.jsonl`.
//!
//! The executor holds an exclusive lock for its whole life: two Installers
//! (or one Installer twice) cannot run two installations on one server.
//!
//! The one-time credential (P11) lives **only in this process's memory**. It
//! is delivered once, to an attached Installer that asks for it
//! (`ClaimSecret`), and wiped. If nobody claims it within
//! [`SECRET_HOLD`] after the installation ends, it is wiped anyway and the
//! process exits: no privileged process lingers for a credential (the
//! runbook `recover-administrative-access` covers that case).

use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ocinye_installer_contracts::ident::InstallationId;
use ocinye_installer_contracts::journal::{InstallationJournal, JournalState, ResumeDecision};
use ocinye_installer_contracts::manifest::ReleaseManifest;
use ocinye_installer_contracts::plan::{InstallationPlan, PhaseId};
use ocinye_installer_contracts::protocol::{secret_line, Command, Envelope, Event, OneTimeSecret};

use crate::phases::{self, Ctx, Effects, Failure};
use crate::state::{self, StateRoot};

/// How long an unclaimed credential is held after the installation ends.
pub const SECRET_HOLD: Duration = Duration::from_secs(30 * 60);

/// The global lock: one installation at a time on a server.
#[must_use]
pub fn global_lock(root: &StateRoot) -> Option<File> {
    fs::create_dir_all(&root.0).ok()?;
    let f = File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(root.0.join(".lock"))
        .ok()?;
    f.try_lock().ok().map(|()| f)
}

/// Shared between the phase loop and the socket clients.
pub struct Hub {
    dir: PathBuf,
    seq: Mutex<u64>,
    clients: Mutex<Vec<UnixStream>>,
    /// The credential, until claimed.
    pub secret: Mutex<Option<OneTimeSecret>>,
    /// A stop was requested.
    pub stop: AtomicBool,
    /// The phase loop ended.
    pub finished: AtomicBool,
    current: Mutex<Option<PhaseId>>,
}

impl Hub {
    fn new(dir: PathBuf) -> Self {
        let seq = state::last_seq(&dir);
        Self {
            dir,
            seq: Mutex::new(seq),
            clients: Mutex::new(Vec::new()),
            secret: Mutex::new(None),
            stop: AtomicBool::new(false),
            finished: AtomicBool::new(false),
            current: Mutex::new(None),
        }
    }

    /// Sequence, persist, broadcast.
    pub fn emit(&self, event: Event) {
        let mut seq = self
            .seq
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *seq += 1;
        let env = Envelope {
            seq: *seq,
            at: state::now(),
            event,
        };
        let _ = state::append_event(&self.dir, &env);
        let line = serde_json::to_string(&env).unwrap_or_default();
        let mut clients = self
            .clients
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        clients.retain_mut(|c| writeln!(c, "{line}").is_ok());
    }

    fn attach(&self, mut stream: UnixStream, from: u64) {
        // Hold the sequence lock while replaying, so nothing is emitted between
        // the replay and joining the live list.
        let _seq = self
            .seq
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for env in state::events_from(&self.dir, from) {
            if writeln!(
                stream,
                "{}",
                serde_json::to_string(&env).unwrap_or_default()
            )
            .is_err()
            {
                return;
            }
        }
        self.clients
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(stream);
    }
}

fn handle_client(hub: &Arc<Hub>, id: &InstallationId, stream: UnixStream) {
    let Ok(reader) = stream.try_clone() else {
        return;
    };
    for line in BufReader::new(reader).lines() {
        let Ok(line) = line else { break };
        let Ok(cmd) = serde_json::from_str::<Command>(&line) else {
            continue;
        };
        match cmd {
            Command::Attach {
                installation_id,
                from_seq,
            } if installation_id == *id => {
                if let Ok(s) = stream.try_clone() {
                    hub.attach(s, from_seq);
                }
            }
            Command::Stop { installation_id } if installation_id == *id => {
                hub.stop.store(true, Ordering::SeqCst);
                let at = *hub
                    .current
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if let Some(at) = at {
                    hub.emit(Event::StopAccepted { at });
                }
            }
            Command::ClaimSecret { installation_id } if installation_id == *id => {
                let taken = hub
                    .secret
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .take();
                if let (Some(secret), Ok(mut s)) = (taken, stream.try_clone()) {
                    // To this client only, never to events.jsonl; then dropped
                    // (and zeroized) at the end of this scope.
                    let _ = writeln!(s, "{}", secret_line(&secret));
                } else {
                    hub.emit(Event::Refused {
                        code: "SECRET_NOT_AVAILABLE".into(),
                    });
                }
            }
            _ => hub.emit(Event::Refused {
                code: "COMMAND_NOT_ACCEPTED".into(),
            }),
        }
    }
}

fn apply(journal: &mut InstallationJournal, fx: Effects, hub: &Hub) {
    for p in fx.created_paths {
        if !journal.created_paths.contains(&p) {
            journal.created_paths.push(p);
        }
    }
    for p in fx.installed_packages {
        if !journal.installed_packages.contains(&p) {
            journal.installed_packages.push(p);
        }
    }
    for r in fx.firewall_rules {
        if !journal.firewall_rules.contains(&r) {
            journal.firewall_rules.push(r);
        }
    }
    if fx.config_written && journal.config_written_at.is_none() {
        journal.config_written_at = Some(state::now());
    }
    if let Some(fp) = fx.self_signed_cert_sha256 {
        journal.self_signed_cert_sha256 = Some(fp);
    }
    if let Some(secret) = fx.secret {
        *hub.secret
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(secret);
    }
}

fn run_phase(
    phase: PhaseId,
    ctx: &mut Ctx<'_>,
    journal: &InstallationJournal,
) -> Result<Effects, Failure> {
    match phase {
        PhaseId::P01 => phases::p01(ctx),
        PhaseId::P02 => phases::p02(ctx),
        PhaseId::P03 => phases::p03(ctx),
        PhaseId::P04 => phases::p04(ctx),
        PhaseId::P05 => phases::p05(ctx),
        PhaseId::P06 => phases::p06(ctx, journal.config_written_at.is_some()),
        PhaseId::P07 => phases::p07(ctx),
        PhaseId::P08 => phases::p08(ctx),
        PhaseId::P09 => phases::p09(ctx),
        PhaseId::P10 => phases::p10(ctx),
        PhaseId::P11 => phases::p11(ctx),
        PhaseId::P12 => phases::p12(ctx),
        PhaseId::P13 => phases::p13(ctx),
        PhaseId::P14 => phases::p14(ctx),
        PhaseId::P15 => phases::p15(ctx),
        PhaseId::P16 => phases::p16(ctx),
    }
}

/// P11 was interrupted: does the Instance exist? Read the Core, never guess.
fn instance_exists_now(id: &InstallationId) -> bool {
    let o = crate::exec::run_path(
        &phases::staging(id).join("install").join("ocinye"),
        &["phase", "check-instance"],
        crate::exec::secs(300),
    );
    o.ok() && o.stdout.contains("\"slug\"")
}

/// Run the phases of a journalled plan. Returns the final journal.
#[allow(clippy::too_many_lines)]
pub fn run(root: &StateRoot, id: &InstallationId, upload: &Path) -> std::io::Result<()> {
    let _global =
        global_lock(root).ok_or_else(|| std::io::Error::other("INSTALLATION_IN_PROGRESS"))?;
    let dir = root.dir(id);
    let plan: InstallationPlan =
        serde_json::from_slice(&fs::read(dir.join("plan.json"))?).map_err(std::io::Error::other)?;
    let mut journal =
        state::read_journal(&dir).ok_or_else(|| std::io::Error::other("NO_JOURNAL"))?;
    if journal.plan_sha256 != plan.plan_sha256 || !plan.seal_holds() {
        return Err(std::io::Error::other("PLAN_SEAL_MISMATCH"));
    }
    let bundle = if upload.join("bundle/MANIFEST.json").exists() {
        upload.join("bundle")
    } else {
        phases::staging(id)
    };
    let manifest = ReleaseManifest::parse(&fs::read(bundle.join("MANIFEST.json"))?)
        .map_err(|e| std::io::Error::other(e.field))?;
    plan.check(&manifest)
        .map_err(|e| std::io::Error::other(format!("{e:?}")))?;

    let hub = Arc::new(Hub::new(dir.clone()));
    let sock = dir.join("control.sock");
    let _ = fs::remove_file(&sock);
    let listener = UnixListener::bind(&sock)?;
    fs::set_permissions(&sock, std::os::unix::fs::PermissionsExt::from_mode(0o600))?;
    {
        let hub = Arc::clone(&hub);
        let id = id.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let hub = Arc::clone(&hub);
                let id = id.clone();
                std::thread::spawn(move || handle_client(&hub, &id, stream));
            }
        });
    }

    // Resume by safety class (never a blind replay of P11).
    let mut start = match journal.resume_decision() {
        ResumeDecision::Completed => None,
        ResumeDecision::ContinueAt(p) => Some(p),
        ResumeDecision::NotResumable(p) => {
            hub.emit(Event::Refused {
                code: format!("NOT_RESUMABLE:{}", p.code()),
            });
            None
        }
        ResumeDecision::InspectInstance => {
            if instance_exists_now(id) {
                // The Instance exists: P11 completed before the cut. Its
                // credential was never delivered and cannot be recovered.
                journal.completed(PhaseId::P11, &state::now());
                state::write_journal(&dir, &journal)?;
                hub.emit(Event::StepWarning {
                    phase: PhaseId::P11,
                    code: "CREDENTIAL_NOT_RECOVERABLE".into(),
                });
                Some(PhaseId::P12)
            } else {
                Some(PhaseId::P11)
            }
        }
    };

    while let Some(phase) = start {
        if hub.stop.load(Ordering::SeqCst) && phase != PhaseId::P01 {
            journal.state = JournalState::Stopped;
            state::write_journal(&dir, &journal)?;
            let after = PhaseId::ALL[phase.number().saturating_sub(2)];
            hub.emit(Event::Stopped { after });
            break;
        }
        if plan.skips(phase) {
            hub.emit(Event::StepSkipped { phase });
            start = PhaseId::ALL.get(phase.number()).copied();
            continue;
        }
        *hub.current
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(phase);
        journal.started(phase, &state::now());
        state::write_journal(&dir, &journal)?;
        hub.emit(Event::StepStarted { phase });
        let result = {
            let h = Arc::clone(&hub);
            let mut emit = move |e: Event| h.emit(e);
            let mut ctx = Ctx {
                id,
                plan: &plan,
                manifest: &manifest,
                dir: &dir,
                upload,
                emit: &mut emit,
            };
            run_phase(phase, &mut ctx, &journal)
        };
        match result {
            Ok(fx) => {
                apply(&mut journal, fx, &hub);
                journal.completed(phase, &state::now());
                state::write_journal(&dir, &journal)?;
                hub.emit(Event::StepCompleted { phase });
                if phase == PhaseId::P02 {
                    // Everything the session uploaded is now in root's
                    // staging; the upload directory has nothing left to give.
                    discard_upload(upload);
                }
                start = PhaseId::ALL.get(phase.number()).copied();
            }
            Err(f) => {
                journal.failed(phase, &f.code, f.retryable, &state::now());
                state::write_journal(&dir, &journal)?;
                hub.emit(Event::StepFailed {
                    phase,
                    code: f.code,
                    retryable: f.retryable,
                    detail: f.detail,
                });
                break;
            }
        }
    }
    if journal.state == JournalState::Completed {
        hub.emit(Event::InstallationCompleted {
            installation_id: id.clone(),
        });
    }
    hub.finished.store(true, Ordering::SeqCst);

    // Hold an unclaimed credential for a bounded time, then wipe and leave.
    let ended = Instant::now();
    while ended.elapsed() < SECRET_HOLD
        && hub
            .secret
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_some()
    {
        std::thread::sleep(Duration::from_millis(500));
    }
    hub.secret
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take();
    // Give a connected Installer a moment to read the last events.
    std::thread::sleep(Duration::from_secs(2));
    let _ = fs::remove_file(&sock);
    if journal.summary().completed.contains(&PhaseId::P02) {
        discard_upload(upload);
    }
    Ok(())
}

/// Removes a session's upload directory — only ever a `/tmp/ocinye-bootstrap-…`.
fn discard_upload(upload: &Path) {
    if phases::is_upload_dir(upload) {
        let _ = fs::remove_dir_all(upload);
    }
}
