//! Stable reason codes. The interface translates them; logs and the audit
//! record carry them; nothing carries a secret.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Why the engine refused, failed, rolled back or stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum UpdateReason {
    /// No repository configured and no offline bundle given.
    RepositoryNotConfigured,
    /// The repository could not be reached, or answered an error.
    RepositoryUnreachable,
    /// The signed metadata failed verification (signature, threshold, root).
    MetadataUntrusted,
    /// Metadata expired: a frozen or stale repository.
    MetadataExpired,
    /// Metadata older than what this machine already saw: a rollback attack.
    MetadataRollback,
    /// The manifest is malformed or breaks a rule.
    ManifestInvalid,
    /// The manifest disagrees with the signed targets.
    ManifestMismatch,
    /// The release is not newer than the running one.
    NotNewer,
    /// The release is for another architecture.
    WrongArchitecture,
    /// The running release is older than the release accepts.
    SourceTooOld,
    /// The release targets another base OS.
    BaseOsMismatch,
    /// The release speaks a System Service protocol this machine cannot run.
    ProtocolIncompatible,
    /// An artifact's size or hash does not match.
    ArtifactMismatch,
    /// A download stopped; it can be resumed.
    DownloadInterrupted,
    /// Not enough disk.
    InsufficientSpace,
    /// Another maintenance operation holds the lock.
    MaintenanceBusy,
    /// The first start of the machine is not finished.
    FirstBootIncomplete,
    /// The running system is not healthy; an update is not started over it.
    SystemUnhealthy,
    /// The database is not healthy or not at the expected schema.
    DatabaseUnhealthy,
    /// The release's migration policy cannot be applied from here.
    MigrationIncompatible,
    /// An archive tried to leave its directory, held a link or a device, or
    /// was larger than declared.
    ArchiveUnsafe,
    /// The Instance installer refused or failed; the Instance stayed.
    InstanceUpgradeFailed,
    /// The new release failed its health gate.
    HealthGateFailed,
    /// The new release did not start healthily after the restart.
    BootValidationFailed,
    /// Cancelled by an operator before anything changed.
    Cancelled,
    /// The journal could not be read or names an impossible state.
    JournalInvalid,
    /// A rollback could not be completed safely.
    RollbackUnsafe,
    /// The offline bundle is not a bundle, or holds more than one release.
    BundleInvalid,
    /// The channel would go back to an older release.
    ChannelDowngrade,
}

impl UpdateReason {
    /// The wire code.
    #[must_use]
    pub fn code(self) -> String {
        serde_json::to_value(self)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default()
    }
}
