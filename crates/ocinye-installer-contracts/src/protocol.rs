//! The protocol between the Installer controller and `ocinye-bootstrap`
//! (D011_STATE_MACHINE › Typed protocol).
//!
//! JSON lines over the SSH channel. The controller sends [`Command`]s — a
//! **closed** enum; nothing in it is a command line, a path chosen by the
//! operator, or a free-form string the bootstrap would execute. The bootstrap
//! answers with [`Envelope`]s carrying [`Event`]s (sequenced, so a reattached
//! controller can ask for what it missed), and, exactly once per installation,
//! a [`OneTimeSecret`] with the temporary administrator credential.
//!
//! The UI never derives state from text: every state change is an event.

use serde::{Deserialize, Serialize};

use crate::hardware::HardwareCapabilities;
use crate::ident::{InstallationId, Sha256Hex};
use crate::journal::JournalSummary;
use crate::manifest::Arch;
use crate::plan::{InstallationPlan, PhaseId};
use crate::preflight::{PreflightItem, PreflightReport, Privilege};
use crate::secret::SecretText;
use crate::verification::{VerificationItem, VerificationReport};

/// Installer → bootstrap.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    /// First message. The bootstrap hashes its own executable and refuses to
    /// continue when it is not `expected_sha256` (the manifest's).
    Hello {
        /// [`crate::BOOTSTRAP_PROTOCOL`].
        protocol: u16,
        /// The manifest's `bootstrap.sha256`.
        expected_sha256: Sha256Hex,
    },
    /// Read-only server facts.
    Inspect {},
    /// Read-only preflight.
    Preflight {
        /// The release's architecture.
        release_arch: Arch,
        /// The operator's clock, Unix seconds (PF-TIME).
        operator_unix_time: i64,
    },
    /// Read-only hardware discovery.
    DiscoverHardware {},
    /// Read the journal of an earlier installation, if any.
    ReadJournal {},
    /// Execute a confirmed plan. Refused unless the seal holds and equals
    /// `confirmed_sha256` — what the operator confirmed on I13.
    Execute {
        /// The plan.
        plan: Box<InstallationPlan>,
        /// The seal captured at confirmation.
        confirmed_sha256: String,
    },
    /// Resume the journalled installation with the same plan.
    Resume {
        /// The plan (its seal must equal the journal's).
        plan: Box<InstallationPlan>,
        /// The seal captured at confirmation.
        confirmed_sha256: String,
    },
    /// Reattach to a running or finished execution, replaying from `from_seq`.
    Attach {
        /// The installation.
        installation_id: InstallationId,
        /// First event sequence wanted.
        from_seq: u64,
    },
    /// Stop at the next safe point.
    Stop {
        /// The installation.
        installation_id: InstallationId,
    },
    /// Re-run the server-side verification (read-only).
    Verify {
        /// The installation.
        installation_id: InstallationId,
    },
    /// Deliver the one-time credential, if still held. Delivered once.
    ClaimSecret {
        /// The installation.
        installation_id: InstallationId,
    },
    /// Remove an incomplete installation (only while no Instance exists).
    RemoveIncomplete {
        /// The installation.
        installation_id: InstallationId,
    },
    /// Remove the session's upload directory (preflight-only sessions).
    Cleanup {},
}

/// `/etc/os-release`, the fields that matter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OsRelease {
    /// `ID`.
    pub id: String,
    /// `VERSION_ID`.
    pub version_id: String,
    /// `PRETTY_NAME`.
    pub pretty: String,
}

/// What the bootstrap reads first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServerFacts {
    /// `hostname`.
    pub hostname: String,
    /// The distribution.
    pub os: OsRelease,
    /// `uname -s`.
    pub kernel_name: String,
    /// `uname -r`.
    pub kernel_release: String,
    /// `uname -m`.
    pub machine: String,
    /// Mapped architecture.
    pub arch: Option<Arch>,
    /// How the bootstrap is running (root via sudo or as root).
    pub privilege: Privilege,
    /// systemd is PID 1.
    pub systemd: bool,
    /// Global IPv4 addresses (for the DNS records shown on I10).
    pub ipv4: Vec<String>,
    /// Global IPv6 addresses.
    pub ipv6: Vec<String>,
}

/// What a running phase is doing (closed; the UI writes it).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Operation {
    /// Verifying the bootstrap's own hash.
    VerifyBootstrap,
    /// Copying and hashing one bundle file.
    VerifyArtifact {
        /// Bundle path.
        path: String,
        /// Files done.
        done: u32,
        /// Files total.
        total: u32,
    },
    /// Re-running the blocking preflight items.
    RecheckPreflight,
    /// Installing Ubuntu packages.
    AptInstall {
        /// Packages.
        packages: Vec<String>,
    },
    /// Downloading and checking Docker's repository key.
    VerifyDockerKey,
    /// Starting Docker.
    EnableDocker,
    /// Extracting the release tree.
    ExtractRelease,
    /// Writing configuration (never the values).
    WriteConfiguration,
    /// Writing the proxy configuration.
    WriteProxy,
    /// Installing or generating TLS material.
    InstallTls,
    /// Adding one ufw rule.
    FirewallAllow {
        /// Port.
        port: u16,
    },
    /// Loading one image.
    LoadImage {
        /// Service.
        service: String,
    },
    /// Starting the database and storage.
    StartPersistence,
    /// Creating the Instance, migrating, creating the administrator.
    CreateInstance,
    /// Seeding one bound endpoint.
    SeedEndpoint {
        /// Host.
        host: String,
    },
    /// Waiting for a service to be healthy.
    WaitHealthy {
        /// Service.
        service: String,
    },
    /// Enabling start on boot.
    EnableSystemd,
    /// Removing the temporary executor.
    RemoveBootstrap,
}

/// Bootstrap → Installer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "PascalCase")]
pub enum Event {
    /// The bootstrap verified itself and speaks this protocol.
    Connected {
        /// Protocol.
        protocol: u16,
    },
    /// Answer to `Inspect`.
    Facts {
        /// The facts.
        facts: ServerFacts,
    },
    /// Preflight began.
    PreflightStarted,
    /// One check.
    PreflightItemResult {
        /// The item.
        item: PreflightItem,
    },
    /// All checks.
    PreflightCompleted {
        /// The report.
        report: PreflightReport,
    },
    /// Discovery began.
    HardwareDiscoveryStarted,
    /// Discovery finished.
    HardwareDiscoveryCompleted {
        /// What was found.
        hardware: HardwareCapabilities,
    },
    /// Answer to `ReadJournal`.
    JournalRead {
        /// The summary, if a journal exists.
        journal: Option<JournalSummary>,
    },
    /// `Execute`/`Resume` accepted; the execution now runs detached from this
    /// SSH session.
    ExecutionAccepted {
        /// The installation.
        installation_id: InstallationId,
    },
    /// A phase began.
    StepStarted {
        /// Phase.
        phase: PhaseId,
    },
    /// Progress.
    StepProgress {
        /// Phase.
        phase: PhaseId,
        /// What it is doing.
        operation: Operation,
    },
    /// A non-blocking warning (closed code, e.g. `IMAGE_PULL_SLOW`).
    StepWarning {
        /// Phase.
        phase: PhaseId,
        /// Code.
        code: String,
    },
    /// Nothing planned for this phase.
    StepSkipped {
        /// Phase.
        phase: PhaseId,
    },
    /// A phase completed.
    StepCompleted {
        /// Phase.
        phase: PhaseId,
    },
    /// A phase failed.
    StepFailed {
        /// Phase.
        phase: PhaseId,
        /// Stable code (D011_FAILURE_MATRIX).
        code: String,
        /// May be retried.
        retryable: bool,
        /// Redacted technical detail.
        detail: Option<String>,
    },
    /// A stop was requested and will take effect at the safe point.
    StopAccepted {
        /// The phase it waits for.
        at: PhaseId,
    },
    /// Stopped at a safe point.
    Stopped {
        /// After this phase.
        after: PhaseId,
    },
    /// Server-side verification began.
    VerificationStarted,
    /// One verification item.
    VerificationItem {
        /// The item.
        item: VerificationItem,
    },
    /// Server-side verification finished.
    VerificationCompleted {
        /// The server items.
        report: VerificationReport,
    },
    /// The credential exists and waits for `ClaimSecret` (never in an event).
    CredentialIssued {
        /// The privileged identity's e-mail.
        user: String,
        /// RFC 3339.
        expires_at: String,
    },
    /// Every phase completed.
    InstallationCompleted {
        /// The installation.
        installation_id: InstallationId,
    },
    /// An incomplete installation was removed.
    IncompleteRemoved {
        /// The installation.
        installation_id: InstallationId,
    },
    /// The upload directory was removed.
    CleanedUp,
    /// A command was refused (closed code).
    Refused {
        /// Code.
        code: String,
    },
}

/// One event on the wire.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Envelope {
    /// Monotonic per installation (0 for preflight-session events).
    pub seq: u64,
    /// RFC 3339.
    pub at: String,
    /// The event.
    pub event: Event,
}

/// The temporary credential, once. Not `Debug`-printable in clear, not part
/// of [`Event`], never journalled.
#[derive(Debug)]
pub struct OneTimeSecret {
    /// The privileged identity's e-mail.
    pub user: String,
    /// RFC 3339.
    pub expires_at: String,
    /// The credential.
    pub value: SecretText,
}

/// A line from the bootstrap.
#[derive(Debug)]
pub enum Incoming {
    /// An event.
    Event(Envelope),
    /// The credential.
    Secret(OneTimeSecret),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SecretWire {
    user: String,
    expires_at: String,
    value: SecretText,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SecretLine {
    one_time_secret: SecretWire,
}

/// Parse one line from the bootstrap.
///
/// # Errors
///
/// A line that is neither an envelope nor the secret message.
pub fn parse_incoming(line: &str) -> Result<Incoming, serde_json::Error> {
    if line.starts_with("{\"one_time_secret\"") {
        let s: SecretLine = serde_json::from_str(line)?;
        return Ok(Incoming::Secret(OneTimeSecret {
            user: s.one_time_secret.user,
            expires_at: s.one_time_secret.expires_at,
            value: s.one_time_secret.value,
        }));
    }
    serde_json::from_str(line).map(Incoming::Event)
}

/// The secret line, built explicitly (the only place a secret is serialised).
#[must_use]
pub fn secret_line(secret: &OneTimeSecret) -> String {
    serde_json::json!({
        "one_time_secret": {
            "user": secret.user,
            "expires_at": secret.expires_at,
            "value": secret.value.expose(),
        }
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_comandos_sao_um_conjunto_fechado() {
        // Um comando inventado não se lê como nenhum comando.
        for bad in [
            r#"{"cmd":"execute_shell","command":"id"}"#,
            r#"{"cmd":"run_remote","argv":["id"]}"#,
            r#"{"cmd":"inspect","extra":"x"}"#,
            r#"{"cmd":"cleanup","path":"/etc"}"#,
        ] {
            assert!(serde_json::from_str::<Command>(bad).is_err(), "{bad}");
        }
        let ok: Command = serde_json::from_str(r#"{"cmd":"inspect"}"#).unwrap();
        assert_eq!(ok, Command::Inspect {});
    }

    #[test]
    fn o_segredo_viaja_numa_linha_propria_e_nao_se_imprime() {
        let s = OneTimeSecret {
            user: "admin@instalacao.test".into(),
            expires_at: "2026-10-05T14:21:00Z".into(),
            value: SecretText::new("Xk3-temporaria".into()),
        };
        let line = secret_line(&s);
        assert!(!format!("{s:?}").contains("Xk3"));
        match parse_incoming(&line).unwrap() {
            Incoming::Secret(back) => assert_eq!(back.value.expose(), "Xk3-temporaria"),
            Incoming::Event(_) => panic!("o segredo leu-se como evento"),
        }
        // Um evento nunca leva o segredo: não há campo onde o pôr.
        let e = Envelope {
            seq: 1,
            at: "t".into(),
            event: Event::CredentialIssued {
                user: s.user.clone(),
                expires_at: s.expires_at.clone(),
            },
        };
        assert!(!serde_json::to_string(&e).unwrap().contains("Xk3"));
    }

    #[test]
    fn um_evento_volta_igual() {
        let e = Envelope {
            seq: 7,
            at: "2026-10-04T14:20:21Z".into(),
            event: Event::StepFailed {
                phase: PhaseId::P11,
                code: "MIGRATION_FAILED".into(),
                retryable: false,
                detail: Some("migration 0041 refused".into()),
            },
        };
        let line = serde_json::to_string(&e).unwrap();
        match parse_incoming(&line).unwrap() {
            Incoming::Event(back) => assert_eq!(back, e),
            Incoming::Secret(_) => panic!(),
        }
    }
}
