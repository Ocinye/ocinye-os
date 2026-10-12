//! The state machine.
//!
//! Every transition is checked against [`Phase::may_become`] and written to
//! the journal before the effect it announces. Activation records each step
//! before and after it runs. At start the engine reconciles: whatever was in
//! flight when the process or the machine stopped is resolved one way —
//! dropped if nothing changed yet, rolled back if something did, committed if
//! the commit had begun — and never guessed.

use crate::host::{images_present, Host, RESTART_ORDER};
use crate::native::{self, NativeInfo};
use crate::paths::Paths;
use crate::source::{Origin, Source};
use crate::store::{write_atomic, MaintenanceLock, Store};
use ocinye_update_contracts::config::UpdateConfig;
use ocinye_update_contracts::journal::{
    HistoryEntry, Journal, Step, StepRecord, Transaction, BOOT_ATTEMPTS,
};
use ocinye_update_contracts::manifest::{Arch, MigrationPolicy};
use ocinye_update_contracts::state::{
    AvailableRelease, OfflineBundle, Phase, Progress, ReleaseInfo, ReleaseSource, UpdateHistory,
    UpdateOutcome, UpdateState,
};
use ocinye_update_contracts::wire::ConfigChange;
use ocinye_update_contracts::{
    Channel, ReleaseSigning, ReleaseVersion, UpdateManifest, UpdateReason, OFFLINE_EXTENSION,
};
use ocinye_update_repository::{offline, sha256_file};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

/// What the image says about itself.
#[derive(Debug, Clone)]
pub struct ImageFacts {
    /// The channel followed when nothing is configured.
    pub default_channel: Channel,
    /// Signing of this build.
    pub signing: ReleaseSigning,
    /// Architecture.
    pub arch: Arch,
    /// The System Service protocol this machine runs.
    pub system_protocol: u32,
}

/// Points at which a test stops the engine as a power cut would: the
/// process is gone, the disk keeps what was written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    /// The manifest is verified, no artifact yet.
    AfterManifest,
    /// One artifact is in, the rest not.
    MidArtifacts,
    /// Every artifact verified.
    AfterVerify,
    /// The native layer is staged.
    AfterStage,
    /// The lock is held and `Entered` recorded.
    AfterEntered,
    /// The images are loaded.
    AfterImages,
    /// The Instance moved.
    AfterInstance,
    /// The native pointers switched.
    AfterNativeSwitch,
    /// The units are in place.
    AfterUnits,
    /// The services were restarted.
    AfterRestart,
    /// The health gate passed; commit not begun.
    BeforeCommit,
    /// `Committing` recorded; nothing else of the commit done.
    InCommit,
}

/// Why an operation stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stop {
    /// Refused or failed, with the reason recorded.
    Reason(UpdateReason),
    /// A test cut the power here.
    Crashed(Fault),
}

impl From<UpdateReason> for Stop {
    fn from(r: UpdateReason) -> Self {
        Self::Reason(r)
    }
}

type Fallible<T> = Result<T, Stop>;

/// The engine.
pub struct Engine<H: Host> {
    paths: Paths,
    host: H,
    store: Store,
    facts: ImageFacts,
    rt: tokio::runtime::Runtime,
    journal: Mutex<Journal>,
    job: Mutex<Option<&'static str>>,
    cancel: AtomicBool,
    done: AtomicU64,
    total: AtomicU64,
    fault: Option<Box<dyn Fn(Fault) -> bool + Send + Sync>>,
    /// Health checks before the gate gives up, and the pause between them.
    health_attempts: u32,
    health_pause: Duration,
}

/// Sixteen random hex digits.
fn random_id() -> String {
    use std::io::Read;
    let mut b = [0u8; 8];
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        let _ = f.read_exact(&mut b);
    }
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn info_of_native(n: &NativeInfo, at: Option<String>) -> ReleaseInfo {
    ReleaseInfo {
        version: n.version.clone(),
        release_number: n.release_number,
        channel: n.channel,
        activated_at: at,
    }
}

fn offered_of(m: &UpdateManifest, source: ReleaseSource) -> AvailableRelease {
    AvailableRelease {
        version: m.version.clone(),
        release_number: m.release_number,
        download_bytes: m.space.download_bytes,
        install_bytes: m.space.install_bytes,
        reboot_required: m.reboot_required,
        security: m.security,
        migrations: m.migrations.policy,
        notes: m.notes.clone(),
        synthetic: m.synthetic.is_some(),
        source,
    }
}

impl<H: Host> Engine<H> {
    /// Open the engine and reconcile whatever was in flight.
    pub fn open(paths: Paths, host: H, facts: ImageFacts) -> Fallible<Self> {
        Self::open_with(paths, host, facts, None, (10, Duration::from_secs(6)))
    }

    /// The same, with a fault injector (tests only build one) and the health
    /// gate's attempts and pause.
    pub fn open_with(
        paths: Paths,
        host: H,
        facts: ImageFacts,
        fault: Option<Box<dyn Fn(Fault) -> bool + Send + Sync>>,
        health: (u32, Duration),
    ) -> Fallible<Self> {
        let store = Store::new(paths.clone());
        let journal = match store.load_journal()? {
            Some(j) => j,
            None => {
                let current = native::info(&paths.current()).map(|n| info_of_native(&n, None));
                let mut j = Journal::fresh(current);
                store.save_journal(&mut j)?;
                j
            }
        };
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .map_err(|_| UpdateReason::JournalInvalid)?;
        let engine = Self {
            paths,
            host,
            store,
            facts,
            rt,
            journal: Mutex::new(journal),
            job: Mutex::new(None),
            cancel: AtomicBool::new(false),
            done: AtomicU64::new(0),
            total: AtomicU64::new(0),
            fault,
            health_attempts: health.0,
            health_pause: health.1,
        };
        engine.reconcile()?;
        Ok(engine)
    }

    fn crash(&self, at: Fault) -> Fallible<()> {
        match &self.fault {
            Some(f) if f(at) => Err(Stop::Crashed(at)),
            _ => Ok(()),
        }
    }

    fn snapshot(&self) -> Journal {
        self.journal
            .lock()
            .map(|j| j.clone())
            .unwrap_or_else(|p| p.into_inner().clone())
    }

    fn save(&self, j: &mut Journal) -> Fallible<()> {
        self.store.save_journal(j)?;
        if let Ok(mut shared) = self.journal.lock() {
            *shared = j.clone();
        }
        Ok(())
    }

    fn go(&self, j: &mut Journal, next: Phase) -> Fallible<()> {
        if j.phase != next && !j.phase.may_become(next) {
            return Err(Stop::Reason(UpdateReason::JournalInvalid));
        }
        j.phase = next;
        self.save(j)
    }

    fn step(&self, j: &mut Journal, step: Step, done: bool) -> Fallible<()> {
        let at = self.host.now();
        if let Some(t) = j.transaction.as_mut() {
            t.steps.push(StepRecord { step, done, at });
        }
        self.save(j)
    }

    fn config(&self) -> UpdateConfig {
        std::fs::read(&self.paths.config)
            .ok()
            .and_then(|b| UpdateConfig::parse(&b).ok())
            .unwrap_or_else(|| UpdateConfig::default_for(self.facts.default_channel))
    }

    fn audit(&self, j: &Journal, event: &str, outcome: &str, reason: Option<UpdateReason>) {
        self.audit_with(j, j.transaction.as_ref(), event, outcome, reason);
    }

    /// Audit with the transaction that just ended (already taken off the
    /// journal), so the record still says who, from where, and to what.
    fn audit_with(
        &self,
        j: &Journal,
        t: Option<&Transaction>,
        event: &str,
        outcome: &str,
        reason: Option<UpdateReason>,
    ) {
        self.store.audit(&serde_json::json!({
            "at": self.host.now(),
            "event": event,
            "outcome": outcome,
            "reason": reason.map(UpdateReason::code),
            "transaction": t.map(|t| t.id.clone()),
            "actor": t.map(|t| t.actor.clone()),
            "via": t.map(|t| t.via.clone()),
            "from": j.current.as_ref().map(|c| c.version.to_string()),
            "to": t.map(|t| t.target.to_string()),
            "channel": t.map(|t| t.channel.as_str()),
        }));
    }

    // ── the long-operation slot ──────────────────────────────────────────

    /// Reserve the engine for one long operation.
    pub fn reserve(&self, what: &'static str) -> Result<(), UpdateReason> {
        let mut job = self.job.lock().map_err(|_| UpdateReason::MaintenanceBusy)?;
        if job.is_some() {
            return Err(UpdateReason::MaintenanceBusy);
        }
        *job = Some(what);
        self.cancel.store(false, Ordering::SeqCst);
        Ok(())
    }

    /// Release the slot.
    pub fn release(&self) {
        if let Ok(mut job) = self.job.lock() {
            *job = None;
        }
    }

    /// Whether a long operation is running.
    #[must_use]
    pub fn busy(&self) -> bool {
        self.job.lock().map(|j| j.is_some()).unwrap_or(true)
    }

    // ── reads ────────────────────────────────────────────────────────────

    /// The state the interface shows.
    #[must_use]
    pub fn status(&self) -> UpdateState {
        let j = self.snapshot();
        let config = self.config();
        let progress = matches!(j.phase, Phase::Downloading | Phase::Verifying).then(|| Progress {
            done: self.done.load(Ordering::Relaxed),
            total: self.total.load(Ordering::Relaxed),
        });
        UpdateState {
            current: j.current.clone(),
            previous: j.previous.clone(),
            channel: config.channel,
            repository_configured: config.repository.is_some(),
            phase: j.phase,
            available: j.offered.clone(),
            progress,
            last_check: j.last_check.clone(),
            last_outcome: j.last_outcome,
            reason: j.reason,
            restart_required: j.phase == Phase::AwaitingReboot,
            signing: self.facts.signing,
            auto_check: config.auto_check,
            auto_download: config.auto_download,
            offline_bundles: self.offline_bundles(),
        }
    }

    /// The history.
    #[must_use]
    pub fn history(&self) -> UpdateHistory {
        UpdateHistory {
            entries: self.store.history(),
        }
    }

    fn offline_bundles(&self) -> Vec<OfflineBundle> {
        let mut out = Vec::new();
        let mut look = |dir: &Path, removable: bool| {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for e in entries.flatten() {
                    let name = e.file_name().to_string_lossy().into_owned();
                    let is_file = e.file_type().is_ok_and(|t| t.is_file());
                    if is_file
                        && name.ends_with(&format!(".{OFFLINE_EXTENSION}"))
                        && ocinye_update_contracts::manifest::is_safe_target(&name)
                    {
                        let bytes = e.metadata().map(|m| m.len()).unwrap_or(0);
                        out.push((
                            OfflineBundle {
                                name,
                                bytes,
                                removable,
                            },
                            e.path(),
                        ));
                    }
                }
            }
        };
        look(&self.paths.inbox(), false);
        if let Ok(media) = std::fs::read_dir(&self.paths.media) {
            for m in media.flatten() {
                if m.file_type().is_ok_and(|t| t.is_dir()) {
                    look(&m.path(), true);
                }
            }
        }
        out.sort_by(|a, b| a.0.name.cmp(&b.0.name));
        out.into_iter().map(|(b, _)| b).collect()
    }

    fn find_bundle(&self, name: &str) -> Option<PathBuf> {
        let mut candidates = vec![self.paths.inbox().join(name)];
        if let Ok(media) = std::fs::read_dir(&self.paths.media) {
            for m in media.flatten() {
                candidates.push(m.path().join(name));
            }
        }
        candidates
            .into_iter()
            .find(|p| std::fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_file()))
    }

    // ── checking ─────────────────────────────────────────────────────────

    fn choose(
        &self,
        j: &Journal,
        config: &UpdateConfig,
        releases: &[UpdateManifest],
    ) -> Result<Option<UpdateManifest>, UpdateReason> {
        let current = j.current.as_ref().map_or(0, |c| c.release_number.0);
        let newer: Vec<&UpdateManifest> = releases
            .iter()
            .filter(|m| {
                m.arch == self.facts.arch
                    && m.channel == config.channel
                    && m.release_number.0 > current
            })
            .collect();
        if newer.is_empty() {
            return Ok(None);
        }
        let mut last_reason = UpdateReason::NotNewer;
        for m in newer {
            if m.minimum_source.0 > current {
                last_reason = UpdateReason::SourceTooOld;
                continue;
            }
            if m.system_protocol != self.facts.system_protocol {
                last_reason = UpdateReason::ProtocolIncompatible;
                continue;
            }
            if let MigrationPolicy::Contract { requires_source } = m.migrations.policy {
                if current < requires_source {
                    last_reason = UpdateReason::MigrationIncompatible;
                    continue;
                }
            }
            return Ok(Some(m.clone()));
        }
        Err(last_reason)
    }

    /// Read the channel's signed metadata and offer the newest release this
    /// machine can take.
    pub fn check(&self) -> Fallible<UpdateState> {
        let mut j = self.snapshot();
        if !j.phase.at_rest() {
            return Err(Stop::Reason(UpdateReason::MaintenanceBusy));
        }
        let config = self.config();
        let Some(endpoint) = config.repository.clone() else {
            return Err(Stop::Reason(UpdateReason::RepositoryNotConfigured));
        };
        let rest = j.phase;
        self.go(&mut j, Phase::Checking)?;
        let outcome = Source::open(
            &self.rt,
            &self.paths,
            config.channel,
            Origin::Repository(endpoint),
        )
        .and_then(|s| s.releases(&self.rt))
        .and_then(|rs| self.choose(&j, &config, &rs));
        j.last_check = Some(self.host.now());
        match outcome {
            Ok(Some(m)) => {
                j.offered = Some(offered_of(&m, ReleaseSource::Repository));
                j.reason = None;
                self.go(&mut j, Phase::UpdateAvailable)?;
            }
            Ok(None) => {
                j.offered = None;
                j.reason = None;
                self.go(&mut j, Phase::Idle)?;
            }
            Err(reason) => {
                j.last_check = None;
                j.reason = Some(reason);
                let back = if rest == Phase::UpdateAvailable && j.offered.is_some() {
                    Phase::UpdateAvailable
                } else {
                    Phase::Idle
                };
                self.go(&mut j, back)?;
                self.audit(&j, "check", "refused", Some(reason));
                return Err(Stop::Reason(reason));
            }
        }
        self.audit(&j, "check", "ok", None);
        Ok(self.status())
    }

    /// Offer the release of an offline bundle the engine lists.
    pub fn import(&self, bundle: &str) -> Fallible<UpdateState> {
        let mut j = self.snapshot();
        if !j.phase.at_rest() {
            return Err(Stop::Reason(UpdateReason::MaintenanceBusy));
        }
        let path = self
            .find_bundle(bundle)
            .ok_or(UpdateReason::BundleInvalid)?;
        let dir = self.paths.offline();
        if dir.exists() {
            std::fs::remove_dir_all(&dir).map_err(|_| UpdateReason::BundleInvalid)?;
        }
        let unpacked = offline::unpack(&path, &dir).map_err(|_| UpdateReason::BundleInvalid)?;
        let config = self.config();
        if unpacked.channel != config.channel {
            let _ = std::fs::remove_dir_all(&dir);
            return Err(Stop::Reason(UpdateReason::BundleInvalid));
        }
        let result = Source::open(
            &self.rt,
            &self.paths,
            config.channel,
            Origin::Offline(dir.clone()),
        )
        .and_then(|s| s.manifest(&self.rt, &unpacked.version))
        .and_then(|m| self.choose(&j, &config, &[m]));
        match result {
            Ok(Some(m)) => {
                j.offered = Some(offered_of(&m, ReleaseSource::Offline));
                j.reason = None;
                self.go(&mut j, Phase::UpdateAvailable)?;
                self.audit(&j, "import", "ok", None);
                Ok(self.status())
            }
            Ok(None) => {
                let _ = std::fs::remove_dir_all(&dir);
                Err(Stop::Reason(UpdateReason::NotNewer))
            }
            Err(reason) => {
                let _ = std::fs::remove_dir_all(&dir);
                j.reason = Some(reason);
                self.save(&mut j)?;
                self.audit(&j, "import", "refused", Some(reason));
                Err(Stop::Reason(reason))
            }
        }
    }

    // ── download, verify, preflight, stage ───────────────────────────────

    fn origin(&self, source: ReleaseSource) -> Result<Origin, UpdateReason> {
        match source {
            ReleaseSource::Repository => self
                .config()
                .repository
                .map(Origin::Repository)
                .ok_or(UpdateReason::RepositoryNotConfigured),
            ReleaseSource::Offline => {
                let d = self.paths.offline();
                if d.exists() {
                    Ok(Origin::Offline(d))
                } else {
                    Err(UpdateReason::BundleInvalid)
                }
            }
        }
    }

    /// Download, verify, preflight and stage the offered release. Blocking:
    /// the server runs it on its own thread after [`Self::reserve`].
    pub fn download(&self, actor: &str, via: &str) -> Fallible<UpdateState> {
        let mut j = self.snapshot();
        let offered = j.offered.clone().ok_or(UpdateReason::NotNewer)?;
        if !matches!(
            j.phase,
            Phase::UpdateAvailable
                | Phase::DownloadFailed
                | Phase::VerificationFailed
                | Phase::PreflightFailed
        ) {
            return Err(Stop::Reason(UpdateReason::MaintenanceBusy));
        }
        let config = self.config();
        let failed = |engine: &Self,
                      j: &mut Journal,
                      phase: Phase,
                      reason: UpdateReason|
         -> Fallible<UpdateState> {
            j.reason = Some(reason);
            j.last_outcome = Some(UpdateOutcome::Failed);
            engine.go(j, phase)?;
            engine.audit(j, "download", "failed", Some(reason));
            Err(Stop::Reason(reason))
        };
        if j.transaction
            .as_ref()
            .is_none_or(|t| t.target != offered.version)
        {
            j.transaction = Some(Transaction {
                id: random_id(),
                target: offered.version.clone(),
                target_number: offered.release_number,
                channel: config.channel,
                source: offered.source,
                from: j.current.clone(),
                started_at: self.host.now(),
                steps: Vec::new(),
                boot_attempts: 0,
                actor: actor.to_owned(),
                via: via.to_owned(),
            });
        }
        j.reason = None;
        self.go(&mut j, Phase::Downloading)?;
        self.done.store(0, Ordering::Relaxed);
        self.total.store(offered.download_bytes, Ordering::Relaxed);
        let source = match self
            .origin(offered.source)
            .and_then(|o| Source::open(&self.rt, &self.paths, config.channel, o))
        {
            Ok(s) => s,
            Err(r) => return failed(self, &mut j, Phase::DownloadFailed, r),
        };
        let manifest = match source.manifest(&self.rt, &offered.version) {
            Ok(m) => m,
            Err(r) => return failed(self, &mut j, Phase::DownloadFailed, r),
        };
        self.crash(Fault::AfterManifest)?;
        let version = manifest.version.to_string();
        let verified = self.paths.verified(&version);
        if self.host.free_bytes(&self.paths.state) < manifest.space.download_bytes {
            return failed(
                self,
                &mut j,
                Phase::DownloadFailed,
                UpdateReason::InsufficientSpace,
            );
        }
        for (i, a) in manifest.artifacts.iter().enumerate() {
            let r = source.fetch(
                &self.rt,
                &manifest.version,
                &a.target,
                &crate::source::Destination {
                    partial_dir: &self.paths.partial(),
                    verified: &verified.join(&a.target),
                },
                &self.cancel,
                &self.done,
            );
            if let Err(reason) = r {
                if reason == UpdateReason::Cancelled {
                    return self.cancelled(&mut j);
                }
                return failed(self, &mut j, Phase::DownloadFailed, reason);
            }
            if i == 0 {
                self.crash(Fault::MidArtifacts)?;
            }
        }
        let bytes =
            serde_json::to_vec_pretty(&manifest).map_err(|_| UpdateReason::ManifestInvalid)?;
        write_atomic(&verified.join("manifest.json"), &bytes)
            .map_err(|_| UpdateReason::InsufficientSpace)?;
        self.go(&mut j, Phase::Downloaded)?;
        self.go(&mut j, Phase::Verifying)?;
        for a in &manifest.artifacts {
            match sha256_file(&verified.join(&a.target)) {
                Ok((h, n)) if h == a.sha256 && n == a.bytes => {}
                _ => {
                    return failed(
                        self,
                        &mut j,
                        Phase::VerificationFailed,
                        UpdateReason::ArtifactMismatch,
                    )
                }
            }
        }
        self.go(&mut j, Phase::Verified)?;
        self.crash(Fault::AfterVerify)?;
        self.go(&mut j, Phase::Preflight)?;
        if let Err(reason) = self.preflight(&j, &manifest) {
            return failed(self, &mut j, Phase::PreflightFailed, reason);
        }
        let native_tar = verified.join(format!("native/native-{}.tar", manifest.arch.as_str()));
        if let Err(reason) = native::stage(&self.paths, &manifest.version, &native_tar) {
            return failed(self, &mut j, Phase::PreflightFailed, reason);
        }
        self.go(&mut j, Phase::Staged)?;
        self.crash(Fault::AfterStage)?;
        let _ = std::fs::remove_dir_all(self.paths.partial());
        self.go(&mut j, Phase::AwaitingInstall)?;
        self.audit(&j, "download", "staged", None);
        Ok(self.status())
    }

    fn cancelled(&self, j: &mut Journal) -> Fallible<UpdateState> {
        j.reason = Some(UpdateReason::Cancelled);
        j.last_outcome = Some(UpdateOutcome::Refused);
        self.go(j, Phase::DownloadFailed)?;
        self.audit(j, "download", "cancelled", Some(UpdateReason::Cancelled));
        Err(Stop::Reason(UpdateReason::Cancelled))
    }

    fn instance_release(&self) -> Option<String> {
        let text = std::fs::read_to_string(&self.paths.release_env).ok()?;
        text.lines()
            .find_map(|l| l.strip_prefix("OCINYE_RELEASE_SHA="))
            .map(|v| v.trim().to_owned())
            .filter(|v| ocinye_update_contracts::version::is_release_id(v))
    }

    fn carried_release(&self) -> Option<String> {
        let mut ids: Vec<String> = std::fs::read_dir(&self.paths.carried_release)
            .ok()?
            .flatten()
            .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| !n.starts_with('.'))
            .collect();
        ids.sort();
        (ids.len() == 1).then(|| ids.remove(0))
    }

    /// What must hold before anything is changed.
    fn preflight(&self, j: &Journal, m: &UpdateManifest) -> Result<(), UpdateReason> {
        if m.arch != self.facts.arch {
            return Err(UpdateReason::WrongArchitecture);
        }
        if m.base_os != ocinye_update_contracts::manifest::BASE_OS {
            return Err(UpdateReason::BaseOsMismatch);
        }
        if m.system_protocol != self.facts.system_protocol {
            return Err(UpdateReason::ProtocolIncompatible);
        }
        let current = j.current.as_ref().map_or(0, |c| c.release_number.0);
        if m.release_number.0 <= current {
            return Err(UpdateReason::NotNewer);
        }
        if m.minimum_source.0 > current {
            return Err(UpdateReason::SourceTooOld);
        }
        if let MigrationPolicy::Contract { requires_source } = m.migrations.policy {
            if current < requires_source {
                return Err(UpdateReason::MigrationIncompatible);
            }
        }
        if !self.paths.firstboot_state.exists() {
            return Err(UpdateReason::FirstBootIncomplete);
        }
        self.host.system_healthy()?;
        if self.instance_release().is_some() {
            self.host.instance_healthy()?;
        }
        if self.host.free_bytes(&self.paths.native_root) < m.space.install_bytes / 4
            || self.host.free_bytes(&self.paths.state) < m.space.install_bytes / 2
        {
            return Err(UpdateReason::InsufficientSpace);
        }
        Ok(())
    }

    // ── install and activation ───────────────────────────────────────────

    fn verified_manifest(&self, version: &ReleaseVersion) -> Result<UpdateManifest, UpdateReason> {
        let bytes = std::fs::read(
            self.paths
                .verified(&version.to_string())
                .join("manifest.json"),
        )
        .map_err(|_| UpdateReason::ManifestInvalid)?;
        UpdateManifest::parse(&bytes).map_err(|_| UpdateReason::ManifestInvalid)
    }

    /// Install the staged release. Blocking, like [`Self::download`].
    pub fn install(&self, actor: &str, via: &str) -> Fallible<UpdateState> {
        let mut j = self.snapshot();
        if j.phase != Phase::AwaitingInstall {
            return Err(Stop::Reason(UpdateReason::MaintenanceBusy));
        }
        let lock = MaintenanceLock::try_acquire(&self.paths.lock)?;
        let target = j
            .transaction
            .as_ref()
            .map(|t| t.target.clone())
            .ok_or(UpdateReason::JournalInvalid)?;
        let manifest = self.verified_manifest(&target)?;
        if let Some(t) = j.transaction.as_mut() {
            t.actor = actor.to_owned();
            t.via = via.to_owned();
        }
        if let Err(reason) = self.preflight(&j, &manifest) {
            j.reason = Some(reason);
            self.go(&mut j, Phase::Preflight)?;
            self.go(&mut j, Phase::PreflightFailed)?;
            self.audit(&j, "install", "refused", Some(reason));
            return Err(Stop::Reason(reason));
        }
        self.go(&mut j, Phase::Installing)?;
        self.step(&mut j, Step::Entered, true)?;
        self.crash(Fault::AfterEntered)?;
        let result = self.activate(&mut j, &manifest);
        drop(lock);
        result
    }

    fn activate(&self, j: &mut Journal, m: &UpdateManifest) -> Fallible<UpdateState> {
        let version = m.version.to_string();
        let bundle = self.paths.verified(&version).join("bundle");
        let target_id = m.version.release_id().to_owned();
        let instance = self.instance_release();
        let moves_instance = instance.as_ref().is_some_and(|i| *i != target_id);
        let moves_carried =
            instance.is_none() && self.carried_release().is_some_and(|c| c != target_id);
        if moves_instance || moves_carried {
            self.step(j, Step::ImagesLoaded, false)?;
            if !images_present(&self.host, &m.images) {
                if let Ok(entries) = std::fs::read_dir(bundle.join("images")) {
                    let mut tars: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
                    tars.sort();
                    for t in tars {
                        if let Err(reason) = self.host.load_image(&t) {
                            return self.install_failed(j, reason);
                        }
                    }
                }
                if !images_present(&self.host, &m.images) {
                    return self.install_failed(j, UpdateReason::ArtifactMismatch);
                }
            }
            self.step(j, Step::ImagesLoaded, true)?;
            self.crash(Fault::AfterImages)?;
        }
        if moves_instance {
            self.step(j, Step::InstanceUpgraded, false)?;
            if let Err(reason) = self.host.instance_upgrade(&bundle) {
                // The installer's own gate failed and it went back on its own:
                // the Instance stayed on the running release.
                return self.install_failed(j, reason);
            }
            self.step(j, Step::InstanceUpgraded, true)?;
            self.crash(Fault::AfterInstance)?;
        }
        if moves_carried {
            self.step(j, Step::CarriedReleaseReplaced, false)?;
            self.replace_carried(&bundle, &target_id)?;
            self.step(j, Step::CarriedReleaseReplaced, true)?;
        }
        self.go(j, Phase::Activating)?;
        let from = j.transaction.as_ref().and_then(|t| t.from.clone());
        self.step(j, Step::NativeSwitched, false)?;
        if let Some(f) = &from {
            native::set_pointer(&self.paths, &self.paths.previous(), &f.version.to_string())
                .map_err(|_| UpdateReason::HealthGateFailed)?;
        }
        native::set_pointer(&self.paths, &self.paths.current(), &version)
            .map_err(|_| UpdateReason::HealthGateFailed)?;
        self.step(j, Step::NativeSwitched, true)?;
        self.crash(Fault::AfterNativeSwitch)?;
        self.step(j, Step::UnitsInstalled, false)?;
        let units =
            native::install_units(&self.paths, &version).and_then(|_| self.host.reload_units());
        if let Err(reason) = units {
            return self.activation_failed(j, reason);
        }
        self.step(j, Step::UnitsInstalled, true)?;
        self.crash(Fault::AfterUnits)?;
        self.step(j, Step::ServicesRestarted, false)?;
        if let Err(reason) = self.restart_services() {
            return self.activation_failed(j, reason);
        }
        self.step(j, Step::ServicesRestarted, true)?;
        self.crash(Fault::AfterRestart)?;
        if let Err(reason) = self.health_gate(&target_id, m) {
            return self.activation_failed(j, reason);
        }
        self.step(j, Step::HealthPassed, true)?;
        if m.reboot_required {
            write_atomic(
                &self.paths.state.join("awaiting-boot"),
                self.host.boot_id().as_bytes(),
            )
            .map_err(|_| UpdateReason::JournalInvalid)?;
            self.go(j, Phase::AwaitingReboot)?;
            self.audit(j, "install", "awaiting-reboot", None);
            return Ok(self.status());
        }
        self.crash(Fault::BeforeCommit)?;
        self.commit(j)
    }

    fn restart_services(&self) -> Result<(), UpdateReason> {
        for unit in RESTART_ORDER {
            if unit == "ocinye-display.service" && !self.host.unit_active(unit) {
                continue;
            }
            self.host.restart_unit(unit)?;
        }
        Ok(())
    }

    fn health_gate(&self, target_id: &str, m: &UpdateManifest) -> Result<(), UpdateReason> {
        let mut last = UpdateReason::HealthGateFailed;
        for attempt in 0..self.health_attempts.max(1) {
            if attempt > 0 {
                std::thread::sleep(self.health_pause);
            }
            let pointer_ok =
                native::pointer(&self.paths.current()).as_deref() == Some(&*m.version.to_string());
            if !pointer_ok {
                return Err(UpdateReason::HealthGateFailed);
            }
            if let Err(r) = self.host.system_healthy() {
                last = r;
                continue;
            }
            if let Some(instance) = self.instance_release() {
                if instance != target_id {
                    last = UpdateReason::HealthGateFailed;
                    continue;
                }
                if let Err(r) = self.host.instance_healthy() {
                    last = r;
                    continue;
                }
                if !images_present(&self.host, &m.images) {
                    last = UpdateReason::HealthGateFailed;
                    continue;
                }
            }
            return Ok(());
        }
        Err(if last == UpdateReason::SystemUnhealthy {
            UpdateReason::HealthGateFailed
        } else {
            last
        })
    }

    fn replace_carried(&self, bundle: &Path, target_id: &str) -> Fallible<()> {
        let old = self.carried_release();
        let staging = self
            .paths
            .carried_release
            .join(format!(".{target_id}.staging"));
        let _ = std::fs::remove_dir_all(&staging);
        copy_tree(bundle, &staging).map_err(|_| UpdateReason::InsufficientSpace)?;
        let keep = self.paths.carried_previous();
        let _ = std::fs::remove_dir_all(&keep);
        std::fs::create_dir_all(&keep).map_err(|_| UpdateReason::InsufficientSpace)?;
        if let Some(old) = old {
            std::fs::rename(self.paths.carried_release.join(&old), keep.join(&old))
                .map_err(|_| UpdateReason::InsufficientSpace)?;
        }
        std::fs::rename(&staging, self.paths.carried_release.join(target_id))
            .map_err(|_| UpdateReason::InsufficientSpace)?;
        Ok(())
    }

    fn install_failed(&self, j: &mut Journal, reason: UpdateReason) -> Fallible<UpdateState> {
        j.reason = Some(reason);
        j.last_outcome = Some(UpdateOutcome::Failed);
        self.go(j, Phase::InstallFailed)?;
        self.finish_history(j, UpdateOutcome::Failed, Some(reason));
        let ended = j.transaction.take();
        self.save(j)?;
        self.audit_with(j, ended.as_ref(), "install", "failed", Some(reason));
        Err(Stop::Reason(reason))
    }

    fn activation_failed(&self, j: &mut Journal, reason: UpdateReason) -> Fallible<UpdateState> {
        j.reason = Some(reason);
        self.go(j, Phase::ActivationFailed)?;
        self.roll_back(j, reason)
    }

    fn finish_history(&self, j: &Journal, outcome: UpdateOutcome, reason: Option<UpdateReason>) {
        if let Some(t) = &j.transaction {
            let _ = self.store.push_history(HistoryEntry {
                id: t.id.clone(),
                version: t.target.clone(),
                release_number: t.target_number,
                channel: t.channel,
                from: t.from.as_ref().map(|f| f.version.clone()),
                started_at: t.started_at.clone(),
                finished_at: self.host.now(),
                outcome,
                reason,
                rebooted: t.boot_attempts > 0,
                source: t.source,
                via: t.via.clone(),
            });
        }
    }

    fn commit(&self, j: &mut Journal) -> Fallible<UpdateState> {
        self.go(j, Phase::Committing)?;
        self.crash(Fault::InCommit)?;
        self.finish_commit(j)
    }

    fn finish_commit(&self, j: &mut Journal) -> Fallible<UpdateState> {
        let t = j.transaction.clone().ok_or(UpdateReason::JournalInvalid)?;
        let target_info = native::info(&self.paths.native_release(&t.target.to_string()))
            .map(|n| info_of_native(&n, Some(self.host.now())))
            .unwrap_or(ReleaseInfo {
                version: t.target.clone(),
                release_number: t.target_number,
                channel: t.channel,
                activated_at: Some(self.host.now()),
            });
        self.finish_history(j, UpdateOutcome::Committed, None);
        j.previous = t.from.clone();
        j.current = Some(target_info);
        j.offered = None;
        j.reason = None;
        j.last_outcome = Some(UpdateOutcome::Committed);
        let ended = j.transaction.take();
        self.go(j, Phase::Completed)?;
        // Retention: the active release, the rollback target, nothing else.
        let keep: Vec<String> = [j.current.as_ref(), j.previous.as_ref()]
            .into_iter()
            .flatten()
            .map(|r| r.version.to_string())
            .collect();
        native::collect(&self.paths, &keep);
        let _ = std::fs::remove_dir_all(self.paths.state.join("verified"));
        let _ = std::fs::remove_dir_all(self.paths.partial());
        let _ = std::fs::remove_dir_all(self.paths.offline());
        let _ = std::fs::remove_dir_all(self.paths.carried_previous());
        let _ = std::fs::remove_file(self.paths.state.join("awaiting-boot"));
        self.audit_with(j, ended.as_ref(), "commit", "committed", None);
        self.host.restart_engine_later();
        Ok(self.status())
    }

    // ── rollback ─────────────────────────────────────────────────────────

    /// Return to the release before the transaction, from wherever the
    /// activation stopped.
    fn roll_back(&self, j: &mut Journal, reason: UpdateReason) -> Fallible<UpdateState> {
        if j.phase != Phase::RollingBack {
            self.go(j, Phase::RollingBack)?;
        }
        let t = j.transaction.clone().ok_or(UpdateReason::JournalInvalid)?;
        let from = t.from.clone();
        let mut safe = true;
        let instance_touched =
            t.done(Step::InstanceUpgraded) || t.begun_not_done(Step::InstanceUpgraded);
        if instance_touched
            && self.instance_release().as_deref() == Some(t.target.release_id())
            && self.host.instance_rollback().is_err()
        {
            safe = false;
        }
        if t.done(Step::CarriedReleaseReplaced) || t.begun_not_done(Step::CarriedReleaseReplaced) {
            safe &= self.restore_carried(t.target.release_id());
        }
        let switched = t.done(Step::NativeSwitched) || t.begun_not_done(Step::NativeSwitched);
        match &from {
            Some(f) => {
                let v = f.version.to_string();
                if native::info(&self.paths.native_release(&v)).is_some() {
                    safe &= native::set_pointer(&self.paths, &self.paths.current(), &v).is_ok();
                    safe &= native::install_units(&self.paths, &v).is_ok();
                } else if switched {
                    safe = false;
                }
            }
            // Nothing to return the native layer to: only safe if it never
            // moved.
            None => safe &= !switched,
        }
        // The release that failed is no rollback target.
        if native::pointer(&self.paths.previous()).as_deref() == Some(&*t.target.to_string()) {
            native::clear_pointer(&self.paths, &self.paths.previous());
        }
        let _ = self.host.reload_units();
        let restarted = self.restart_services().is_ok();
        let healthy = restarted && self.health_after_rollback(from.as_ref());
        let _ = std::fs::remove_dir_all(self.paths.native_release(&t.target.to_string()));
        if safe && healthy {
            j.reason = Some(reason);
            j.last_outcome = Some(UpdateOutcome::RolledBack);
            self.go(j, Phase::RolledBack)?;
            self.finish_history(j, UpdateOutcome::RolledBack, Some(reason));
            let ended = j.transaction.take();
            self.save(j)?;
            self.audit_with(j, ended.as_ref(), "rollback", "rolled-back", Some(reason));
            Err(Stop::Reason(reason))
        } else {
            j.reason = Some(UpdateReason::RollbackUnsafe);
            j.last_outcome = Some(UpdateOutcome::RecoveryRequired);
            self.go(j, Phase::RecoveryRequired)?;
            self.finish_history(
                j,
                UpdateOutcome::RecoveryRequired,
                Some(UpdateReason::RollbackUnsafe),
            );
            self.audit(
                j,
                "rollback",
                "recovery-required",
                Some(UpdateReason::RollbackUnsafe),
            );
            Err(Stop::Reason(UpdateReason::RollbackUnsafe))
        }
    }

    fn health_after_rollback(&self, from: Option<&ReleaseInfo>) -> bool {
        for attempt in 0..self.health_attempts.max(1) {
            if attempt > 0 {
                std::thread::sleep(self.health_pause);
            }
            let pointer_ok = from.is_none_or(|f| {
                native::pointer(&self.paths.current()).as_deref() == Some(&*f.version.to_string())
            });
            let instance_ok =
                self.instance_release().is_none() || self.host.instance_healthy().is_ok();
            if pointer_ok && self.host.system_healthy().is_ok() && instance_ok {
                return true;
            }
        }
        false
    }

    fn restore_carried(&self, target_id: &str) -> bool {
        let keep = self.paths.carried_previous();
        let Ok(mut olds) = std::fs::read_dir(&keep).map(|d| d.flatten().collect::<Vec<_>>()) else {
            return true;
        };
        let Some(old) = olds.pop() else { return true };
        let _ = std::fs::remove_dir_all(self.paths.carried_release.join(target_id));
        std::fs::rename(old.path(), self.paths.carried_release.join(old.file_name())).is_ok()
    }

    /// An operator returns the machine to the previous release.
    pub fn rollback(&self, actor: &str, via: &str) -> Fallible<UpdateState> {
        let mut j = self.snapshot();
        if !j.phase.at_rest() && j.phase != Phase::RecoveryRequired {
            return Err(Stop::Reason(UpdateReason::MaintenanceBusy));
        }
        let previous = j.previous.clone().ok_or(UpdateReason::RollbackUnsafe)?;
        let current = j.current.clone().ok_or(UpdateReason::RollbackUnsafe)?;
        let pv = previous.version.to_string();
        if native::info(&self.paths.native_release(&pv)).is_none() {
            return Err(Stop::Reason(UpdateReason::RollbackUnsafe));
        }
        let _lock = MaintenanceLock::try_acquire(&self.paths.lock)?;
        j.transaction = Some(Transaction {
            id: random_id(),
            target: previous.version.clone(),
            target_number: previous.release_number,
            channel: previous.channel,
            source: ReleaseSource::Repository,
            from: Some(current.clone()),
            started_at: self.host.now(),
            steps: Vec::new(),
            boot_attempts: 0,
            actor: actor.to_owned(),
            via: via.to_owned(),
        });
        self.go(&mut j, Phase::RollingBack)?;
        let mut safe = true;
        if self
            .instance_release()
            .is_some_and(|i| i != previous.version.release_id())
        {
            safe &= self.host.instance_rollback().is_ok();
        }
        safe &= native::set_pointer(&self.paths, &self.paths.current(), &pv).is_ok();
        safe &= native::set_pointer(
            &self.paths,
            &self.paths.previous(),
            &current.version.to_string(),
        )
        .is_ok();
        safe &= native::install_units(&self.paths, &pv).is_ok();
        let _ = self.host.reload_units();
        let healthy =
            self.restart_services().is_ok() && self.health_after_rollback(Some(&previous));
        if safe && healthy {
            j.current = Some(ReleaseInfo {
                activated_at: Some(self.host.now()),
                ..previous
            });
            j.previous = Some(current);
            j.last_outcome = Some(UpdateOutcome::RolledBack);
            j.reason = None;
            self.go(&mut j, Phase::RolledBack)?;
            self.finish_history(&j, UpdateOutcome::RolledBack, None);
            let ended = j.transaction.take();
            self.save(&mut j)?;
            self.audit_with(&j, ended.as_ref(), "rollback", "rolled-back", None);
            self.host.restart_engine_later();
            Ok(self.status())
        } else {
            j.reason = Some(UpdateReason::RollbackUnsafe);
            j.last_outcome = Some(UpdateOutcome::RecoveryRequired);
            self.go(&mut j, Phase::RecoveryRequired)?;
            self.audit(
                &j,
                "rollback",
                "recovery-required",
                Some(UpdateReason::RollbackUnsafe),
            );
            Err(Stop::Reason(UpdateReason::RollbackUnsafe))
        }
    }

    /// The kernel parameter of the installed system's recovery boot entry:
    /// return to the release kept as previous.
    pub const RECOVERY_PARAM: &'static str = "ocinye.update=rollback";

    /// Act on the recovery boot entry, once per boot.
    ///
    /// Whoever chose the entry in the boot menu is at the machine, which is
    /// the authority the menu already demands; the rollback is the same
    /// journaled one Settings offers, recorded with the boot menu as actor.
    /// The boot is marked before acting, so the engine restarting itself
    /// after the rollback does not roll back again. `None`: nothing asked.
    pub fn recovery_request(&self, cmdline: &str) -> Option<Fallible<UpdateState>> {
        let asked = cmdline
            .split_ascii_whitespace()
            .take_while(|a| *a != "--" && *a != "---")
            .any(|a| a == Self::RECOVERY_PARAM);
        if !asked {
            return None;
        }
        let boot = self.host.boot_id();
        let marker = self.paths.recovery_boot();
        if std::fs::read_to_string(&marker).is_ok_and(|b| b.trim() == boot) {
            return None;
        }
        if write_atomic(&marker, boot.as_bytes()).is_err() {
            return Some(Err(Stop::Reason(UpdateReason::JournalInvalid)));
        }
        Some(self.rollback("boot-menu", "recovery"))
    }

    // ── cancel, configure ────────────────────────────────────────────────

    /// Drop a transaction that changed nothing yet.
    pub fn cancel(&self) -> Fallible<UpdateState> {
        if self.busy() {
            self.cancel.store(true, Ordering::SeqCst);
            return Ok(self.status());
        }
        let mut j = self.snapshot();
        if !j.phase.before_mutation() {
            return Err(Stop::Reason(UpdateReason::MaintenanceBusy));
        }
        if let Some(t) = &j.transaction {
            let v = t.target.to_string();
            let current = j.current.as_ref().map(|c| c.version.to_string());
            if current.as_deref() != Some(&*v) {
                let _ = std::fs::remove_dir_all(self.paths.native_release(&v));
            }
        }
        let _ = std::fs::remove_dir_all(self.paths.state.join("verified"));
        let _ = std::fs::remove_dir_all(self.paths.partial());
        if j.transaction.is_some() {
            j.reason = Some(UpdateReason::Cancelled);
            j.last_outcome = Some(UpdateOutcome::Refused);
            self.audit(&j, "cancel", "cancelled", Some(UpdateReason::Cancelled));
        }
        j.transaction = None;
        j.phase = if j.offered.is_some() {
            Phase::UpdateAvailable
        } else {
            Phase::Idle
        };
        self.save(&mut j)?;
        Ok(self.status())
    }

    /// Change the configuration.
    pub fn configure(&self, change: &ConfigChange) -> Fallible<UpdateState> {
        let mut j = self.snapshot();
        let mut config = self.config();
        if let Some(channel) = change.channel {
            if channel != config.channel {
                if !j.phase.at_rest() || self.busy() {
                    return Err(Stop::Reason(UpdateReason::MaintenanceBusy));
                }
                if !self
                    .paths
                    .trust
                    .join(channel.as_str())
                    .join("root.json")
                    .is_file()
                {
                    return Err(Stop::Reason(UpdateReason::MetadataUntrusted));
                }
                config.channel = channel;
                j.offered = None;
                j.phase = Phase::Idle;
            }
        }
        if let Some(r) = &change.repository {
            r.validate()
                .map_err(|_| UpdateReason::RepositoryNotConfigured)?;
            config.repository = Some(r.clone());
        }
        if change.clear_repository {
            config.repository = None;
        }
        if let Some(v) = change.auto_check {
            config.auto_check = v;
            if !v {
                config.auto_download = false;
            }
        }
        if let Some(v) = change.auto_download {
            config.auto_download = v;
            if v {
                config.auto_check = true;
            }
        }
        config
            .validate()
            .map_err(|_| UpdateReason::RepositoryNotConfigured)?;
        let bytes = serde_json::to_vec_pretty(&config).map_err(|_| UpdateReason::JournalInvalid)?;
        write_atomic(&self.paths.config, &bytes).map_err(|_| UpdateReason::JournalInvalid)?;
        self.save(&mut j)?;
        self.store.audit(&serde_json::json!({
            "at": self.host.now(),
            "event": "configure",
            "channel": config.channel.as_str(),
            "repository": config.repository.is_some(),
            "auto_check": config.auto_check,
            "auto_download": config.auto_download,
        }));
        Ok(self.status())
    }

    /// Whether a check should run now by itself (once a day at most).
    #[must_use]
    pub fn auto_check_due(&self, now_secs: u64, last_auto: Option<u64>) -> bool {
        let c = self.config();
        c.auto_check
            && c.repository.is_some()
            && last_auto.is_none_or(|l| now_secs.saturating_sub(l) >= 86_400)
    }

    /// Whether the configuration asks for automatic download.
    #[must_use]
    pub fn auto_download(&self) -> bool {
        self.config().auto_download
    }

    // ── reconciliation and boot validation ───────────────────────────────

    fn reconcile(&self) -> Fallible<()> {
        let mut j = self.snapshot();
        match j.phase {
            Phase::Checking => {
                j.phase = if j.offered.is_some() {
                    Phase::UpdateAvailable
                } else {
                    Phase::Idle
                };
                self.save(&mut j)?;
            }
            Phase::Downloading
            | Phase::Downloaded
            | Phase::Verifying
            | Phase::Verified
            | Phase::Preflight => {
                // Partial downloads stay, isolated, for a resume; nothing
                // unverified moves on.
                j.reason = Some(UpdateReason::DownloadInterrupted);
                j.phase = Phase::DownloadFailed;
                self.save(&mut j)?;
            }
            Phase::Staged | Phase::AwaitingInstall => {
                let staged = j.transaction.as_ref().is_some_and(|t| {
                    native::info(&self.paths.native_release(&t.target.to_string())).is_some()
                });
                j.phase = if staged {
                    Phase::AwaitingInstall
                } else {
                    Phase::DownloadFailed
                };
                if !staged {
                    j.reason = Some(UpdateReason::DownloadInterrupted);
                }
                self.save(&mut j)?;
            }
            Phase::Installing
            | Phase::Activating
            | Phase::ActivationFailed
            | Phase::RollingBack => {
                let _lock = MaintenanceLock::try_acquire(&self.paths.lock)?;
                let t = j.transaction.clone().ok_or(UpdateReason::JournalInvalid)?;
                let nothing_changed = !t.steps.iter().any(|s| {
                    matches!(
                        s.step,
                        Step::InstanceUpgraded
                            | Step::CarriedReleaseReplaced
                            | Step::NativeSwitched
                    )
                });
                if nothing_changed && j.phase == Phase::Installing {
                    // Stopped before any effect: the running release is whole.
                    // Back to waiting; the operator decides again.
                    let staged =
                        native::info(&self.paths.native_release(&t.target.to_string())).is_some();
                    j.phase = if staged {
                        Phase::AwaitingInstall
                    } else {
                        Phase::DownloadFailed
                    };
                    j.reason = Some(UpdateReason::DownloadInterrupted);
                    self.save(&mut j)?;
                } else {
                    j.phase = Phase::RollingBack;
                    self.save(&mut j)?;
                    let _ = self.roll_back(&mut j, UpdateReason::HealthGateFailed);
                }
            }
            Phase::AwaitingReboot | Phase::BootValidating => {
                let waiting = std::fs::read_to_string(self.paths.state.join("awaiting-boot"))
                    .unwrap_or_default();
                if j.phase == Phase::BootValidating || waiting.trim() != self.host.boot_id().trim()
                {
                    // The outcome (committed, rolled back, recovery) is in
                    // the journal; only a test's power cut stops the opening.
                    if let Err(Stop::Crashed(f)) = self.boot_validate() {
                        return Err(Stop::Crashed(f));
                    }
                }
            }
            Phase::Committing => {
                let _ = self.finish_commit(&mut j);
            }
            _ => {}
        }
        Ok(())
    }

    /// After the restart a release asked for: commit if healthy, else count
    /// the attempt and, past [`BOOT_ATTEMPTS`], roll back.
    pub fn boot_validate(&self) -> Fallible<()> {
        let mut j = self.snapshot();
        if !matches!(j.phase, Phase::AwaitingReboot | Phase::BootValidating) {
            return Ok(());
        }
        let _lock = MaintenanceLock::try_acquire(&self.paths.lock)?;
        let attempts = j.transaction.as_ref().map_or(0, |t| t.boot_attempts) + 1;
        if let Some(t) = j.transaction.as_mut() {
            t.boot_attempts = attempts;
        }
        if j.phase == Phase::AwaitingReboot {
            self.go(&mut j, Phase::BootValidating)?;
        } else {
            self.save(&mut j)?;
        }
        let target = j
            .transaction
            .as_ref()
            .map(|t| t.target.clone())
            .ok_or(UpdateReason::JournalInvalid)?;
        let healthy = self
            .verified_manifest(&target)
            .ok()
            .is_some_and(|m| self.health_gate(target.release_id(), &m).is_ok());
        if healthy {
            self.commit(&mut j).map(|_| ())
        } else if attempts >= BOOT_ATTEMPTS {
            j.reason = Some(UpdateReason::BootValidationFailed);
            self.go(&mut j, Phase::BootValidationFailed)?;
            self.roll_back(&mut j, UpdateReason::BootValidationFailed)
                .map(|_| ())
        } else {
            Ok(())
        }
    }
}

fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)? {
        let e = e?;
        let t = e.file_type()?;
        let dest = to.join(e.file_name());
        if t.is_dir() {
            copy_tree(&e.path(), &dest)?;
        } else if t.is_file() {
            std::fs::copy(e.path(), &dest)?;
        }
    }
    Ok(())
}
