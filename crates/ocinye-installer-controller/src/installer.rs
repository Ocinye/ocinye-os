//! The Installer controller: the state the UI renders and the operations it
//! may request (D011_STATE_MACHINE). The graphical Installer and the headless
//! driver call the same methods.
//!
//! Order is enforced here, not in the UI: no bootstrap before the host key is
//! pinned and the release verified; no plan without a preflight that allows
//! it; no `Execute` without a confirmed plan whose seal is captured at
//! confirmation; no OPERATIONAL without every verification item.

use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use ocinye_installer_contracts::hardware::{HardwareCapabilities, ProviderMode};
use ocinye_installer_contracts::ident::{InstallationId, Sha256Hex};
use ocinye_installer_contracts::journal::JournalSummary;
use ocinye_installer_contracts::manifest::Arch;
use ocinye_installer_contracts::plan::{
    InstallationConfiguration, InstallationPlan, PhaseId, TargetIdentity, TlsPlan,
};
use ocinye_installer_contracts::preflight::PreflightReport;
use ocinye_installer_contracts::protocol::{
    Command, Envelope, Event, Incoming, OneTimeSecret, ServerFacts,
};
use ocinye_installer_contracts::receipt::{InstallationReceipt, RECEIPT_SCHEMA};
use ocinye_installer_contracts::secret::SecretText;
use ocinye_installer_contracts::verification::{LifecycleState, TlsMode, VerificationReport};
use ocinye_installer_contracts::BOOTSTRAP_PROTOCOL;
use serde::Serialize;
use tokio::sync::mpsc::UnboundedSender;

use crate::bundle::{self, ReleaseRejection, VerifiedRelease};
use crate::hostkeys::{HostKeyDecision, KnownHosts};
use crate::opverify::{self, Expectation, Resolver};
use crate::planner;
use crate::ssh::{self, Auth, BootstrapConn, Elevation, Session, SshError, Target};

/// What the operator sees happening.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Progress {
    /// A remote event.
    Remote(Box<Envelope>),
    /// Bytes of the release sent (P02, operator side).
    Upload {
        /// The file.
        file: String,
        /// Bytes sent of all files.
        done: u64,
        /// Bytes in total.
        total: u64,
    },
    /// An operator-side verification item.
    Operator(ocinye_installer_contracts::verification::VerificationItem),
}

/// What the read-only probe after authentication found (I03 result).
#[derive(Debug, Clone, Default, Serialize)]
pub struct Probe {
    /// `id -u`.
    pub uid: Option<u32>,
    /// `id -nG`.
    pub groups: Vec<String>,
    /// `sudo -n true` succeeded.
    pub sudo_nopasswd: bool,
    /// `uname -m`.
    pub machine: String,
    /// `ID VERSION_ID`.
    pub os: String,
    /// `PRETTY_NAME`.
    pub pretty: String,
    /// `hostname`.
    pub hostname: String,
}

impl Probe {
    fn parse(text: &str) -> Self {
        let mut p = Self::default();
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            match k {
                "uid" => p.uid = v.trim().parse().ok(),
                "groups" => p.groups = v.split_whitespace().map(str::to_owned).collect(),
                "sudo" => p.sudo_nopasswd = v.trim() == "nopasswd",
                "machine" => v.trim().clone_into(&mut p.machine),
                "os" => v.trim().clone_into(&mut p.os),
                "pretty" => v.trim().clone_into(&mut p.pretty),
                "hostname" => v.trim().clone_into(&mut p.hostname),
                _ => {}
            }
        }
        p
    }

    /// How the bootstrap can run.
    #[must_use]
    pub fn elevation(&self) -> Option<Elevation> {
        if self.uid == Some(0) {
            Some(Elevation::Root)
        } else if self.sudo_nopasswd {
            Some(Elevation::SudoNoPassword)
        } else if self
            .groups
            .iter()
            .any(|g| matches!(g.as_str(), "sudo" | "admin" | "wheel"))
        {
            Some(Elevation::SudoPassword)
        } else {
            None
        }
    }
}

/// Why an operation was refused (closed codes the UI maps to its states).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "code", content = "detail", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Refusal {
    /// Out of order.
    NotReady(&'static str),
    /// I20–I22.
    Release(ReleaseRejection),
    /// I23.
    UnsupportedTarget {
        /// The package's.
        release: String,
        /// The server's.
        server: String,
    },
    /// I24.
    SshUnreachable(String),
    /// I25.
    SshAuthRefused,
    /// I04 — explicit trust needed.
    HostKeyUnknown {
        /// Algorithm.
        algorithm: String,
        /// Fingerprint.
        fingerprint: String,
    },
    /// I26 — hard stop.
    HostKeyMismatch {
        /// Pinned.
        known: String,
        /// Presented.
        presented: String,
    },
    /// I27.
    PrivilegeMissing,
    /// I05 — the sudo password was refused.
    SudoRefused,
    /// The bootstrap refused (closed code).
    Bootstrap(String),
    /// The configuration breaks a rule.
    Configuration(String),
    /// Transport.
    Transport(String),
}

impl From<SshError> for Refusal {
    fn from(e: SshError) -> Self {
        match e {
            SshError::Unreachable(d) => Self::SshUnreachable(d),
            SshError::AuthRefused => Self::SshAuthRefused,
            SshError::HostKey(HostKeyDecision::FirstContact {
                algorithm,
                fingerprint,
            }) => Self::HostKeyUnknown {
                algorithm,
                fingerprint,
            },
            SshError::HostKey(HostKeyDecision::Mismatch {
                known,
                presented_fingerprint,
                ..
            }) => Self::HostKeyMismatch {
                known: known.fingerprint,
                presented: presented_fingerprint,
            },
            SshError::HostKey(HostKeyDecision::Known) => Self::Transport("host key".into()),
            SshError::Transport(d) => Self::Transport(d),
        }
    }
}

/// How an execution ended (as far as this session saw it).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "outcome", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Ending {
    /// Every phase completed.
    Completed,
    /// A phase failed.
    Failed {
        /// Phase.
        phase: PhaseId,
        /// Code.
        code: String,
        /// Retryable.
        retryable: bool,
    },
    /// Stopped at a safe point.
    Stopped {
        /// After this phase.
        after: PhaseId,
    },
    /// The channel ended without a terminal event (I36).
    Interrupted,
    /// The executor refused.
    Refused(String),
}

/// Local TLS files (operator-supplied).
#[derive(Debug, Clone)]
pub struct TlsFiles {
    /// Certificate (PEM).
    pub cert: PathBuf,
    /// Private key (PEM). Read for the key match, then sent once.
    pub key: PathBuf,
    /// Optional chain.
    pub chain: Option<PathBuf>,
}

/// The controller.
pub struct Installer {
    known: Arc<Mutex<KnownHosts>>,
    state_dir: PathBuf,
    /// The verified release.
    pub release: Option<VerifiedRelease>,
    /// The target.
    pub target: Option<Target>,
    auth: Option<Auth>,
    /// The pinned host key `(algorithm, fingerprint)`.
    pub host_key: Option<(String, String)>,
    /// How the bootstrap runs.
    pub elevation: Option<Elevation>,
    sudo: Option<SecretText>,
    session: Option<Session>,
    upload_dir: Option<String>,
    conn: Option<BootstrapConn>,
    /// The probe after authentication.
    pub probe: Option<Probe>,
    /// Server facts.
    pub facts: Option<ServerFacts>,
    /// Preflight.
    pub preflight: Option<PreflightReport>,
    /// Hardware.
    pub hardware: Option<HardwareCapabilities>,
    /// What the operator configured.
    pub configuration: Option<InstallationConfiguration>,
    tls_files: Option<TlsFiles>,
    /// The installation id (stable across plans and resume).
    pub installation_id: InstallationId,
    /// The current plan (draft until confirmed).
    pub plan: Option<InstallationPlan>,
    confirmed: Option<String>,
    /// The temporary credential, until shown and acknowledged.
    pub secret: Option<OneTimeSecret>,
    /// Self-signed certificate fingerprint (from `TlsInstalled`).
    pub served_cert_sha256: Option<String>,
    /// Server-side verification.
    pub server_report: VerificationReport,
    /// Everything (server + operator).
    pub report: VerificationReport,
    /// The lifecycle.
    pub lifecycle: Option<LifecycleState>,
    /// An earlier installation found on the server.
    pub journal: Option<JournalSummary>,
    /// Events seen (non-secret).
    pub events: Vec<Envelope>,
    /// Last event sequence seen (for reattach).
    pub last_seq: u64,
    /// RFC 3339.
    pub started_at: Option<String>,
    /// RFC 3339.
    pub ended_at: Option<String>,
    /// Resolver (system; fixed entries only in controlled tests).
    pub resolver: Resolver,
}

fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

impl Installer {
    /// A fresh controller with its known-hosts store and state directory.
    #[must_use]
    pub fn new(state_dir: PathBuf) -> Self {
        let known = KnownHosts::load(state_dir.join("known_hosts.json"));
        Self {
            known: Arc::new(Mutex::new(known)),
            state_dir,
            release: None,
            target: None,
            auth: None,
            host_key: None,
            elevation: None,
            sudo: None,
            session: None,
            upload_dir: None,
            conn: None,
            probe: None,
            facts: None,
            preflight: None,
            hardware: None,
            configuration: None,
            tls_files: None,
            installation_id: planner::new_installation_id(),
            plan: None,
            confirmed: None,
            secret: None,
            served_cert_sha256: None,
            server_report: VerificationReport::default(),
            report: VerificationReport::default(),
            lifecycle: None,
            journal: None,
            events: Vec::new(),
            last_seq: 0,
            started_at: None,
            ended_at: None,
            resolver: Resolver::default(),
        }
    }

    /// I02 · verify a bundle locally.
    ///
    /// # Errors
    ///
    /// I20–I22.
    pub fn open_release(&mut self, dir: &Path) -> Result<&VerifiedRelease, Refusal> {
        self.release = None;
        let r = bundle::verify(dir).map_err(Refusal::Release)?;
        Ok(self.release.insert(r))
    }

    /// I03 · where and how.
    pub fn set_target(&mut self, target: Target, auth: Auth) {
        self.target = Some(target);
        self.auth = Some(auth);
        self.session = None;
        self.host_key = None;
        self.probe = None;
    }

    /// I03 «Testar ligação»: host key first; authenticate only if pinned.
    ///
    /// # Errors
    ///
    /// I04 (unknown key), I26 (mismatch), I24, I25, I23.
    pub async fn test_connection(&mut self) -> Result<&Probe, Refusal> {
        let target = self.target.clone().ok_or(Refusal::NotReady("target"))?;
        let (decision, key) = ssh::probe_host_key(&target, Arc::clone(&self.known)).await?;
        match decision {
            HostKeyDecision::Known => {}
            HostKeyDecision::FirstContact {
                algorithm,
                fingerprint,
            } => {
                return Err(Refusal::HostKeyUnknown {
                    algorithm,
                    fingerprint,
                });
            }
            HostKeyDecision::Mismatch {
                known,
                presented_fingerprint,
                ..
            } => {
                return Err(Refusal::HostKeyMismatch {
                    known: known.fingerprint,
                    presented: presented_fingerprint,
                });
            }
        }
        let auth = self.auth.as_ref().ok_or(Refusal::NotReady("auth"))?;
        let session = ssh::connect(&target, auth, Arc::clone(&self.known)).await?;
        let out = session.exec(ssh::PROBE, None).await?;
        let probe = Probe::parse(&out.stdout);
        self.host_key = Some(key.clone());
        // I23 · the package's architecture against the server's.
        if let Some(r) = &self.release {
            let server = Arch::from_uname(&probe.machine);
            if server != Some(r.manifest.target.arch) {
                return Err(Refusal::UnsupportedTarget {
                    release: r.manifest.target.arch.as_str().to_owned(),
                    server: probe.machine.clone(),
                });
            }
        }
        self.elevation = probe.elevation();
        self.session = Some(session);
        Ok(self.probe.insert(probe))
    }

    /// I04 «Confiar neste servidor»: pin exactly the fingerprint shown.
    ///
    /// # Errors
    ///
    /// When another key is pinned (that is I26, not trust).
    pub fn trust_host(&mut self, algorithm: &str, fingerprint: &str) -> Result<(), Refusal> {
        let target = self.target.as_ref().ok_or(Refusal::NotReady("target"))?;
        self.known
            .lock()
            .map_err(|_| Refusal::Transport("store".into()))?
            .trust(&target.store_key(), algorithm, fingerprint)
            .map_err(|_| Refusal::HostKeyMismatch {
                known: String::new(),
                presented: fingerprint.to_owned(),
            })
    }

    /// The governed «Esquecer servidor» (settings, after I26).
    ///
    /// # Errors
    ///
    /// When the store cannot be written.
    pub fn forget_host(&mut self) -> Result<(), Refusal> {
        let target = self.target.as_ref().ok_or(Refusal::NotReady("target"))?;
        self.known
            .lock()
            .map_err(|_| Refusal::Transport("store".into()))?
            .forget(&target.store_key())
            .map_err(|e| Refusal::Transport(e.to_string()))
    }

    /// I05 · the sudo password, validated by `sudo -v`, kept in memory.
    ///
    /// # Errors
    ///
    /// I05 refusal when wrong.
    pub async fn set_sudo_password(&mut self, password: SecretText) -> Result<(), Refusal> {
        let session = self.session.as_ref().ok_or(Refusal::NotReady("session"))?;
        let mut line = Vec::with_capacity(password.expose().len() + 1);
        line.extend_from_slice(password.expose().as_bytes());
        line.push(b'\n');
        let out = session.exec(ssh::SUDO_VALIDATE, Some(&line)).await;
        zeroize::Zeroize::zeroize(&mut line);
        if out?.ok() {
            self.sudo = Some(password);
            Ok(())
        } else {
            Err(Refusal::SudoRefused)
        }
    }

    /// A sudo password was validated in this session.
    #[must_use]
    pub fn sudo_ready(&self) -> bool {
        self.sudo.is_some()
    }

    /// Upload the verified bootstrap, check its sum remotely, start it, `Hello`.
    ///
    /// # Errors
    ///
    /// Order, privilege, transport, or the bootstrap's refusal.
    pub async fn start_bootstrap(&mut self) -> Result<(), Refusal> {
        let release = self.release.as_ref().ok_or(Refusal::NotReady("release"))?;
        let session = self.session.as_ref().ok_or(Refusal::NotReady("session"))?;
        let elevation = self.elevation.ok_or(Refusal::PrivilegeMissing)?;
        if elevation == Elevation::SudoPassword && self.sudo.is_none() {
            return Err(Refusal::NotReady("sudo"));
        }
        let dir = ssh::new_upload_dir();
        let out = session.exec(&ssh::mkdir_upload(&dir), None).await?;
        if !out.ok() {
            return Err(Refusal::Transport("mkdir".into()));
        }
        session
            .upload(
                &dir,
                "ocinye-bootstrap",
                &release.dir.join("ocinye-bootstrap"),
                &mut |_| {},
            )
            .await?;
        let sum = session.exec(&ssh::prepare_bootstrap(&dir), None).await?;
        let expected = &release.manifest.bootstrap.sha256;
        if sum.stdout.split_whitespace().next() != Some(expected.as_str()) {
            return Err(Refusal::Bootstrap("BOOTSTRAP_HASH_MISMATCH".into()));
        }
        let mut conn = session
            .open_bootstrap(&dir, elevation, self.sudo.as_ref())
            .await?;
        conn.send(&Command::Hello {
            protocol: BOOTSTRAP_PROTOCOL,
            expected_sha256: Sha256Hex::parse(expected)
                .map_err(|_| Refusal::Bootstrap("HASH".into()))?,
        })
        .await?;
        match next_event(&mut conn).await {
            Some(Event::Connected { .. }) => {}
            Some(Event::Refused { code }) => return Err(Refusal::Bootstrap(code)),
            _ => return Err(Refusal::Bootstrap("NO_HELLO".into())),
        }
        conn.send(&Command::Inspect {}).await?;
        if let Some(Event::Facts { facts }) = next_event(&mut conn).await {
            self.facts = Some(facts);
        }
        self.upload_dir = Some(dir);
        self.conn = Some(conn);
        Ok(())
    }

    fn conn(&mut self) -> Result<&mut BootstrapConn, Refusal> {
        self.conn.as_mut().ok_or(Refusal::NotReady("bootstrap"))
    }

    /// I06 · read-only preflight; then the journal (I19).
    ///
    /// # Errors
    ///
    /// Order or transport.
    pub async fn run_preflight(
        &mut self,
        sink: Option<&UnboundedSender<Progress>>,
    ) -> Result<&PreflightReport, Refusal> {
        let arch = self
            .release
            .as_ref()
            .ok_or(Refusal::NotReady("release"))?
            .manifest
            .target
            .arch;
        let conn = self.conn()?;
        conn.send(&Command::Preflight {
            release_arch: arch,
            operator_unix_time: chrono::Utc::now().timestamp(),
        })
        .await?;
        let report = loop {
            match conn.rx.recv().await {
                Some(Incoming::Event(env)) => {
                    if let Some(s) = sink {
                        let _ = s.send(Progress::Remote(Box::new(env.clone())));
                    }
                    match env.event {
                        Event::PreflightCompleted { report } => break report,
                        Event::Refused { code } => return Err(Refusal::Bootstrap(code)),
                        _ => {}
                    }
                }
                Some(Incoming::Secret(_)) | None => {
                    return Err(Refusal::Transport("preflight".into()))
                }
            }
        };
        self.journal = report.incomplete.clone();
        Ok(self.preflight.insert(report))
    }

    /// I07 · read-only hardware discovery.
    ///
    /// # Errors
    ///
    /// Order or transport.
    pub async fn discover_hardware(&mut self) -> Result<&HardwareCapabilities, Refusal> {
        let conn = self.conn()?;
        conn.send(&Command::DiscoverHardware {}).await?;
        loop {
            match next_event(conn).await {
                Some(Event::HardwareDiscoveryCompleted { hardware }) => {
                    return Ok(self.hardware.insert(hardware));
                }
                Some(Event::HardwareDiscoveryStarted) => {}
                Some(Event::Refused { code }) => return Err(Refusal::Bootstrap(code)),
                _ => return Err(Refusal::Transport("hardware".into())),
            }
        }
    }

    /// I08–I12 · the configuration (validated; TLS files checked by I11).
    ///
    /// # Errors
    ///
    /// The configuration rule broken.
    pub fn configure(
        &mut self,
        cfg: InstallationConfiguration,
        tls: Option<TlsFiles>,
    ) -> Result<(), Refusal> {
        cfg.validate()
            .map_err(|e| Refusal::Configuration(format!("{e:?}")))?;
        if matches!(cfg.tls, TlsPlan::OperatorSupplied { .. }) && tls.is_none() {
            return Err(Refusal::Configuration("TLS_FILES".into()));
        }
        self.configuration = Some(cfg);
        self.tls_files = tls;
        // Any change discards the draft: a new plan, a new plan_id, reviewed again.
        self.plan = None;
        self.confirmed = None;
        Ok(())
    }

    /// I13 · build and seal a **new** plan.
    ///
    /// # Errors
    ///
    /// Order, or a preflight that does not allow installing.
    pub fn review(&mut self) -> Result<&InstallationPlan, Refusal> {
        let release = self.release.as_ref().ok_or(Refusal::NotReady("release"))?;
        let report = self
            .preflight
            .as_ref()
            .ok_or(Refusal::NotReady("preflight"))?;
        if !report.install_allowed() {
            return Err(Refusal::NotReady("preflight_blocked"));
        }
        let hardware = self
            .hardware
            .as_ref()
            .ok_or(Refusal::NotReady("hardware"))?;
        let target = self.target.as_ref().ok_or(Refusal::NotReady("target"))?;
        let (algorithm, fingerprint) =
            self.host_key.clone().ok_or(Refusal::NotReady("host_key"))?;
        let cfg = self
            .configuration
            .clone()
            .ok_or(Refusal::NotReady("configuration"))?;
        let plan = planner::build(
            self.installation_id.clone(),
            &release.manifest,
            TargetIdentity {
                host: target.host.clone(),
                port: target.port,
                user: target.user.clone(),
                host_key_algorithm: algorithm,
                host_key_sha256: fingerprint,
            },
            cfg,
            report,
            hardware,
        );
        self.confirmed = None;
        Ok(self.plan.insert(plan))
    }

    fn save_plan(&self, plan: &InstallationPlan) {
        let dir = self.state_dir.join("plans");
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(
            dir.join(format!("{}.json", plan.installation_id)),
            serde_json::to_vec_pretty(plan).unwrap_or_default(),
        );
    }

    /// A plan saved by an earlier session, for I19.
    #[must_use]
    pub fn saved_plan(&self, id: &InstallationId) -> Option<InstallationPlan> {
        let bytes = std::fs::read(self.state_dir.join("plans").join(format!("{id}.json"))).ok()?;
        serde_json::from_slice::<InstallationPlan>(&bytes)
            .ok()
            .filter(InstallationPlan::seal_holds)
    }

    /// «Instalar Ocinye OS»: confirm **this** plan, transfer, execute, follow.
    ///
    /// # Errors
    ///
    /// Order or transport (failures of phases are an [`Ending`]).
    pub async fn install(
        &mut self,
        sink: Option<&UnboundedSender<Progress>>,
    ) -> Result<Ending, Refusal> {
        let plan = self.plan.clone().ok_or(Refusal::NotReady("plan"))?;
        if !plan.seal_holds() {
            return Err(Refusal::NotReady("plan_seal"));
        }
        // The confirmation captures the seal; nothing after this reads the
        // configuration again.
        let confirmed = plan.plan_sha256.clone();
        self.confirmed = Some(confirmed.clone());
        self.save_plan(&plan);
        self.started_at = Some(now());
        self.transfer(sink).await?;
        self.conn()?
            .send(&Command::Execute {
                plan: Box::new(plan),
                confirmed_sha256: confirmed,
            })
            .await?;
        self.follow(sink).await
    }

    async fn transfer(&mut self, sink: Option<&UnboundedSender<Progress>>) -> Result<(), Refusal> {
        let release = self.release.as_ref().ok_or(Refusal::NotReady("release"))?;
        let session = self.session.as_ref().ok_or(Refusal::NotReady("session"))?;
        let dir = self.upload_dir.clone().ok_or(Refusal::NotReady("upload"))?;
        let total: u64 = release.files.iter().map(|(_, n)| n).sum();
        let mut done = 0u64;
        for (rel, _) in &release.files {
            let file = rel.clone();
            let mut report = |n: u64| {
                done += n;
                if let Some(s) = sink {
                    let _ = s.send(Progress::Upload {
                        file: file.clone(),
                        done,
                        total,
                    });
                }
            };
            session
                .upload(
                    &dir,
                    &format!("bundle/{rel}"),
                    &release.dir.join(rel),
                    &mut report,
                )
                .await?;
        }
        if let Some(tls) = &self.tls_files {
            // Certificate and chain in one file (as the proxy serves them).
            let mut pem =
                std::fs::read(&tls.cert).map_err(|e| Refusal::Transport(e.to_string()))?;
            if let Some(chain) = &tls.chain {
                pem.push(b'\n');
                pem.extend(std::fs::read(chain).map_err(|e| Refusal::Transport(e.to_string()))?);
            }
            let tmp = self
                .state_dir
                .join(format!("tls-{}.pem", self.installation_id));
            std::fs::write(&tmp, &pem).map_err(|e| Refusal::Transport(e.to_string()))?;
            let r1 = session
                .upload(&dir, "tls/instance.pem", &tmp, &mut |_| {})
                .await;
            let _ = std::fs::remove_file(&tmp);
            r1?;
            // The key goes straight from the operator's file; no local copy.
            session
                .upload(&dir, "tls/instance.key", &tls.key, &mut |_| {})
                .await?;
        }
        Ok(())
    }

    /// Follow events until a terminal one, claiming the credential when it is
    /// issued.
    ///
    /// # Errors
    ///
    /// Order.
    pub async fn follow(
        &mut self,
        sink: Option<&UnboundedSender<Progress>>,
    ) -> Result<Ending, Refusal> {
        let installation_id = self.installation_id.clone();
        let mut ending = None;
        loop {
            let Some(msg) = self.conn()?.rx.recv().await else {
                break;
            };
            match msg {
                Incoming::Secret(secret) => {
                    // Held for the one display; never logged, never stored.
                    self.secret = Some(secret);
                }
                Incoming::Event(env) => {
                    self.last_seq = self.last_seq.max(env.seq);
                    if let Some(s) = sink {
                        let _ = s.send(Progress::Remote(Box::new(env.clone())));
                    }
                    match &env.event {
                        Event::CredentialIssued { .. } => {
                            self.conn()?
                                .send(&Command::ClaimSecret {
                                    installation_id: installation_id.clone(),
                                })
                                .await?;
                        }
                        Event::TlsInstalled { cert_sha256 } => {
                            self.served_cert_sha256 = Some(cert_sha256.clone());
                        }
                        Event::VerificationCompleted { report } => {
                            self.server_report = report.clone();
                        }
                        Event::InstallationCompleted { .. } => ending = Some(Ending::Completed),
                        Event::StepFailed {
                            phase,
                            code,
                            retryable,
                            ..
                        } => {
                            ending = Some(Ending::Failed {
                                phase: *phase,
                                code: code.clone(),
                                retryable: *retryable,
                            });
                        }
                        Event::Stopped { after } => {
                            ending = Some(Ending::Stopped { after: *after })
                        }
                        Event::Refused { code }
                            if ending.is_none() && !code.starts_with("SECRET") =>
                        {
                            ending = Some(Ending::Refused(code.clone()));
                        }
                        _ => {}
                    }
                    self.events.push(env);
                    if matches!(ending, Some(Ending::Completed)) && self.secret.is_some() {
                        break;
                    }
                    if matches!(
                        ending,
                        Some(Ending::Failed { .. } | Ending::Stopped { .. } | Ending::Refused(_))
                    ) {
                        break;
                    }
                }
            }
        }
        self.ended_at = Some(now());
        Ok(ending.unwrap_or(Ending::Interrupted))
    }

    /// A cloneable sender, for stopping while [`Self::install`] follows.
    ///
    /// # Errors
    ///
    /// Order.
    pub fn control(&self) -> Result<ssh::Control, Refusal> {
        self.conn
            .as_ref()
            .map(BootstrapConn::control)
            .ok_or(Refusal::NotReady("bootstrap"))
    }

    /// I14 «Parar num ponto seguro».
    ///
    /// # Errors
    ///
    /// Transport.
    pub async fn stop(&mut self) -> Result<(), Refusal> {
        let id = self.installation_id.clone();
        self.conn()?
            .send(&Command::Stop {
                installation_id: id,
            })
            .await?;
        Ok(())
    }

    /// I36 · reconnect after an interruption and reattach to the executor.
    ///
    /// # Errors
    ///
    /// The connection errors of a fresh connection.
    pub async fn reattach(
        &mut self,
        sink: Option<&UnboundedSender<Progress>>,
    ) -> Result<Ending, Refusal> {
        self.conn = None;
        self.session = None;
        self.test_connection().await?;
        self.start_bootstrap().await?;
        let from = self.last_seq + 1;
        let id = self.installation_id.clone();
        self.conn()?
            .send(&Command::Attach {
                installation_id: id,
                from_seq: from,
            })
            .await?;
        self.follow(sink).await
    }

    /// I19 «Retomar instalação» with the journalled plan.
    ///
    /// # Errors
    ///
    /// Order (no saved plan) or transport.
    pub async fn resume(
        &mut self,
        sink: Option<&UnboundedSender<Progress>>,
    ) -> Result<Ending, Refusal> {
        let plan = self.plan.clone().ok_or(Refusal::NotReady("plan"))?;
        self.installation_id = plan.installation_id.clone();
        let confirmed = plan.plan_sha256.clone();
        if self
            .journal
            .as_ref()
            .is_some_and(|j| !j.completed.contains(&PhaseId::P02))
        {
            self.transfer(sink).await?;
        }
        self.conn()?
            .send(&Command::Resume {
                plan: Box::new(plan),
                confirmed_sha256: confirmed,
            })
            .await?;
        self.follow(sink).await
    }

    /// I19 «Remover instalação incompleta» (only while no Instance exists).
    ///
    /// # Errors
    ///
    /// The bootstrap's refusal.
    pub async fn remove_incomplete(&mut self, id: InstallationId) -> Result<(), Refusal> {
        let conn = self.conn()?;
        conn.send(&Command::RemoveIncomplete {
            installation_id: id,
        })
        .await?;
        match next_event(conn).await {
            Some(Event::IncompleteRemoved { .. }) => Ok(()),
            Some(Event::Refused { code }) => Err(Refusal::Bootstrap(code)),
            _ => Err(Refusal::Transport("remove".into())),
        }
    }

    fn expectation(&self) -> Option<Expectation> {
        let plan = self.plan.as_ref()?;
        let mut ips: Vec<IpAddr> = Vec::new();
        if let Some(t) = &self.target {
            match &t.host {
                ocinye_installer_contracts::ident::TargetHost::V4(ip) => ips.push(IpAddr::V4(*ip)),
                ocinye_installer_contracts::ident::TargetHost::V6(ip) => ips.push(IpAddr::V6(*ip)),
                ocinye_installer_contracts::ident::TargetHost::Name(_) => {}
            }
        }
        if let Some(f) = &self.facts {
            ips.extend(
                f.ipv4
                    .iter()
                    .chain(&f.ipv6)
                    .filter_map(|a| a.parse::<IpAddr>().ok()),
            );
        }
        ips.dedup();
        let (cert_sha256, operator_supplied) = match &plan.configuration.tls {
            TlsPlan::OperatorSupplied { cert_sha256, .. } => (cert_sha256.clone(), true),
            TlsPlan::SelfSignedTest => (self.served_cert_sha256.clone()?, false),
        };
        Some(Expectation {
            target_ips: ips,
            hosts: plan
                .configuration
                .endpoints
                .hosts()
                .into_iter()
                .cloned()
                .collect(),
            cert_sha256,
            operator_supplied,
        })
    }

    /// I15/I16 · the operator-side items and the lifecycle. Read-only; this is
    /// also «Verificar novamente».
    ///
    /// # Errors
    ///
    /// Order.
    pub async fn verify_from_here(
        &mut self,
        sink: Option<&UnboundedSender<Progress>>,
    ) -> Result<&LifecycleState, Refusal> {
        let e = self.expectation().ok_or(Refusal::NotReady("expectation"))?;
        let items = opverify::run(&e, &self.resolver).await;
        if let Some(s) = sink {
            for i in &items {
                let _ = s.send(Progress::Operator(i.clone()));
            }
        }
        let mut report = self.server_report.clone();
        report.replace(items);
        let tls = if e.operator_supplied {
            TlsMode::OperatorSupplied
        } else {
            TlsMode::SelfSignedTest
        };
        self.lifecycle = Some(LifecycleState::derive(&report, tls));
        self.report = report;
        self.lifecycle
            .as_ref()
            .ok_or(Refusal::NotReady("lifecycle"))
    }

    /// I18 · the receipt (non-secret).
    #[must_use]
    pub fn receipt(&self) -> Option<InstallationReceipt> {
        let plan = self.plan.as_ref()?;
        let release = self.release.as_ref()?;
        let c = &plan.configuration;
        let first_access_pending = self
            .report
            .items
            .iter()
            .any(|i| i.evidence == "FIRST_ACCESS_PENDING");
        let mut warnings: Vec<String> = self
            .preflight
            .as_ref()
            .map(|r| {
                r.items
                    .iter()
                    .filter(|i| {
                        i.status == ocinye_installer_contracts::preflight::CheckStatus::Warning
                    })
                    .map(|i| format!("{:?}", i.id))
                    .collect()
            })
            .unwrap_or_default();
        warnings.extend(self.events.iter().filter_map(|e| match &e.event {
            Event::StepWarning { phase, code } => Some(format!("{}:{code}", phase.code())),
            _ => None,
        }));
        Some(InstallationReceipt {
            schema: RECEIPT_SCHEMA,
            installation_id: plan.installation_id.clone(),
            plan_id: plan.plan_id.clone(),
            plan_sha256: plan.plan_sha256.clone(),
            release: plan.release.id.clone(),
            commit: plan.release.commit.clone(),
            build: plan.release.build,
            manifest_sha256: plan.release.manifest_sha256.clone(),
            artifacts: release.manifest.artifacts.clone(),
            target_host: plan.target.host.as_display(),
            host_key_sha256: plan.target.host_key_sha256.clone(),
            instance_name: c.instance_name.as_str().to_owned(),
            instance_slug: c.instance_name.derived_slug(),
            distributions: c
                .distributions
                .as_slice()
                .iter()
                .map(|d| d.as_str().to_owned())
                .collect(),
            endpoints: c.endpoints.clone(),
            tls_mode: if matches!(c.tls, TlsPlan::OperatorSupplied { .. }) {
                TlsMode::OperatorSupplied
            } else {
                TlsMode::SelfSignedTest
            },
            tls_not_after: match &c.tls {
                TlsPlan::OperatorSupplied { not_after, .. } => Some(not_after.clone()),
                TlsPlan::SelfSignedTest => None,
            },
            hardware: self.hardware.clone()?,
            provider_mode: ProviderMode::Off,
            installed_packages: Vec::new(),
            firewall_rules: plan
                .system_changes
                .iter()
                .filter_map(|ch| match ch {
                    ocinye_installer_contracts::plan::SystemChange::FirewallAllow {
                        port, ..
                    } => Some(format!("{port}/tcp")),
                    _ => None,
                })
                .collect(),
            warnings,
            started_at: self.started_at.clone().unwrap_or_default(),
            ended_at: self.ended_at.clone().unwrap_or_default(),
            verification: self.report.clone(),
            lifecycle: self
                .lifecycle
                .clone()
                .unwrap_or(LifecycleState::InstallationIncomplete),
            first_access_pending,
        })
    }

    /// «Já a guardei»: the credential is dropped (and zeroized).
    pub fn acknowledge_credential(&mut self) {
        self.secret = None;
    }
}

async fn next_event(conn: &mut BootstrapConn) -> Option<Event> {
    match tokio::time::timeout(std::time::Duration::from_secs(600), conn.rx.recv()).await {
        Ok(Some(Incoming::Event(env))) => Some(env.event),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sondagem_decide_como_o_executor_corre() {
        let p = Probe::parse(
            "uid=1000\ngroups=operador sudo\nsudo=no\nmachine=aarch64\nos=ubuntu 24.04\n",
        );
        assert_eq!(p.elevation(), Some(Elevation::SudoPassword));
        let p = Probe::parse("uid=0\ngroups=root\nsudo=no\n");
        assert_eq!(p.elevation(), Some(Elevation::Root));
        let p = Probe::parse("uid=1000\ngroups=operador\nsudo=nopasswd\n");
        assert_eq!(p.elevation(), Some(Elevation::SudoNoPassword));
        let p = Probe::parse("uid=1000\ngroups=operador users\nsudo=no\n");
        assert_eq!(p.elevation(), None, "sem sudo: I27");
    }
}
