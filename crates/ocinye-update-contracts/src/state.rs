//! The durable state machine, and the state the interface reads.
//!
//! The journal holds a [`Phase`]; the interface holds nothing. Every move from
//! one phase to another is in [`Phase::may_become`], and the engine refuses any
//! other — including from a journal edited by hand.

use crate::manifest::{MigrationPolicy, Notes};
use crate::reason::UpdateReason;
use crate::version::{Channel, ReleaseNumber, ReleaseVersion};
use crate::ReleaseSigning;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Where the engine stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Phase {
    /// Nothing in flight.
    Idle,
    /// Reading the signed metadata.
    Checking,
    /// A newer release is offered on the channel.
    UpdateAvailable,
    /// Fetching artifacts into the partial area.
    Downloading,
    /// Every artifact is on disk, not yet verified as a whole.
    Downloaded,
    /// Checking sizes and hashes against the signed metadata.
    Verifying,
    /// Every artifact matches.
    Verified,
    /// Checking the machine can take the release.
    Preflight,
    /// Extracted and prepared beside the running release.
    Staged,
    /// Ready; waiting for an operator to install.
    AwaitingInstall,
    /// The Instance is moving to the new release.
    Installing,
    /// The native layer is being switched and the services restarted.
    Activating,
    /// Installed; a restart finishes it.
    AwaitingReboot,
    /// The first start of the new release is being checked.
    BootValidating,
    /// Healthy; being recorded.
    Committing,
    /// Done.
    Completed,
    /// A download failed; the running release is untouched.
    DownloadFailed,
    /// An artifact did not match the signed metadata.
    VerificationFailed,
    /// The machine cannot take the release now.
    PreflightFailed,
    /// The Instance did not move; it stayed on the running release.
    InstallFailed,
    /// The new release did not pass its health gate.
    ActivationFailed,
    /// The new release did not start healthily after the restart.
    BootValidationFailed,
    /// Going back to the previous release.
    RollingBack,
    /// Back on the previous release.
    RolledBack,
    /// The engine cannot decide safely; a person must.
    RecoveryRequired,
}

impl Phase {
    /// Phases in which nothing has been changed on the machine yet, so that
    /// the transaction may be dropped.
    #[must_use]
    pub const fn before_mutation(self) -> bool {
        matches!(
            self,
            Self::Idle
                | Self::Checking
                | Self::UpdateAvailable
                | Self::Downloading
                | Self::Downloaded
                | Self::Verifying
                | Self::Verified
                | Self::Preflight
                | Self::Staged
                | Self::AwaitingInstall
                | Self::DownloadFailed
                | Self::VerificationFailed
                | Self::PreflightFailed
        )
    }

    /// Phases from which a new check or download may start.
    #[must_use]
    pub const fn at_rest(self) -> bool {
        matches!(
            self,
            Self::Idle
                | Self::UpdateAvailable
                | Self::Completed
                | Self::DownloadFailed
                | Self::VerificationFailed
                | Self::PreflightFailed
                | Self::InstallFailed
                | Self::RolledBack
        )
    }

    /// The transitions the engine may make, and no others.
    #[must_use]
    pub fn may_become(self, next: Self) -> bool {
        use Phase::*;
        if next == RecoveryRequired {
            // Always allowed: refusing to decide is never a wrong move.
            return true;
        }
        match self {
            Idle | UpdateAvailable | Completed | DownloadFailed | VerificationFailed
            | PreflightFailed | InstallFailed | RolledBack => {
                // `RollingBack` from rest: an operator returns to the previous
                // release.
                matches!(
                    next,
                    Checking | Downloading | Idle | UpdateAvailable | RollingBack
                )
            }
            Checking => matches!(next, Idle | UpdateAvailable),
            Downloading => matches!(next, Downloaded | DownloadFailed | Idle),
            Downloaded => matches!(next, Verifying | Idle),
            Verifying => matches!(next, Verified | VerificationFailed),
            Verified => matches!(next, Preflight),
            Preflight => matches!(next, Staged | PreflightFailed),
            Staged => matches!(next, AwaitingInstall | PreflightFailed | Idle),
            AwaitingInstall => matches!(next, Installing | Preflight | Idle),
            Installing => matches!(next, Activating | InstallFailed | RollingBack),
            Activating => matches!(next, Committing | AwaitingReboot | ActivationFailed),
            ActivationFailed => matches!(next, RollingBack),
            AwaitingReboot => matches!(next, BootValidating),
            BootValidating => matches!(next, Committing | BootValidationFailed | BootValidating),
            BootValidationFailed => matches!(next, RollingBack),
            Committing => matches!(next, Completed),
            RollingBack => matches!(next, RolledBack),
            RecoveryRequired => matches!(next, Idle | RollingBack),
        }
    }
}

/// A release installed on this machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ReleaseInfo {
    /// Identity.
    pub version: ReleaseVersion,
    /// Number on its channel.
    pub release_number: ReleaseNumber,
    /// Channel it came from.
    pub channel: Channel,
    /// When it became active, RFC 3339 UTC; `None` for the release the image
    /// was built with until the first update records it.
    pub activated_at: Option<String>,
}

/// A release the channel offers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct AvailableRelease {
    /// Identity.
    pub version: ReleaseVersion,
    /// Number.
    pub release_number: ReleaseNumber,
    /// Bytes to download.
    #[ts(type = "number")]
    pub download_bytes: u64,
    /// Bytes once installed.
    #[ts(type = "number")]
    pub install_bytes: u64,
    /// Whether a restart finishes it.
    pub reboot_required: bool,
    /// Whether it carries security fixes.
    pub security: bool,
    /// What it does to the database.
    pub migrations: MigrationPolicy,
    /// Notes.
    pub notes: Notes,
    /// A test release.
    pub synthetic: bool,
    /// Where it came from: the configured repository or an offline bundle.
    pub source: ReleaseSource,
}

/// Where an offered release came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseSource {
    /// The configured repository.
    Repository,
    /// An imported offline bundle.
    Offline,
}

/// Progress of a long phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct Progress {
    /// Done so far.
    #[ts(type = "number")]
    pub done: u64,
    /// Total.
    #[ts(type = "number")]
    pub total: u64,
}

/// How the last transaction ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum UpdateOutcome {
    /// Installed and committed.
    Committed,
    /// Refused before anything changed.
    Refused,
    /// Failed before anything changed.
    Failed,
    /// Installed, failed its health gate, and went back.
    RolledBack,
    /// A person must decide.
    RecoveryRequired,
}

/// An offline bundle the engine can see.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct OfflineBundle {
    /// The file name, `<name>.ocu`; what `update.import` names.
    pub name: String,
    /// Size.
    #[ts(type = "number")]
    pub bytes: u64,
    /// On removable media, or in the inbox.
    pub removable: bool,
}

/// Everything the interface shows about updates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct UpdateState {
    /// The running release; `None` on an image without update metadata.
    pub current: Option<ReleaseInfo>,
    /// The release a rollback returns to.
    pub previous: Option<ReleaseInfo>,
    /// The channel followed.
    pub channel: Channel,
    /// Whether a repository is configured; without one, only offline bundles.
    pub repository_configured: bool,
    /// Phase.
    pub phase: Phase,
    /// The release in flight or offered.
    pub available: Option<AvailableRelease>,
    /// Progress of the phase.
    pub progress: Option<Progress>,
    /// Last successful check, RFC 3339 UTC.
    pub last_check: Option<String>,
    /// How the last transaction ended.
    pub last_outcome: Option<UpdateOutcome>,
    /// Why the engine is where it is, when it is not the happy path.
    pub reason: Option<UpdateReason>,
    /// A restart is needed to finish.
    pub restart_required: bool,
    /// Signing of this build.
    pub signing: ReleaseSigning,
    /// Check automatically.
    pub auto_check: bool,
    /// Download automatically after a check finds a release.
    pub auto_download: bool,
    /// Offline bundles found.
    pub offline_bundles: Vec<OfflineBundle>,
}

/// The bounded history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct UpdateHistory {
    /// Most recent first.
    pub entries: Vec<crate::journal::HistoryEntry>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn o_caminho_feliz_e_permitido() {
        use Phase::*;
        let path = [
            Idle,
            Checking,
            UpdateAvailable,
            Downloading,
            Downloaded,
            Verifying,
            Verified,
            Preflight,
            Staged,
            AwaitingInstall,
            Installing,
            Activating,
            Committing,
            Completed,
        ];
        for w in path.windows(2) {
            assert!(w[0].may_become(w[1]), "{:?} → {:?}", w[0], w[1]);
        }
        let reboot = [
            Activating,
            AwaitingReboot,
            BootValidating,
            Committing,
            Completed,
        ];
        for w in reboot.windows(2) {
            assert!(w[0].may_become(w[1]), "{:?} → {:?}", w[0], w[1]);
        }
    }

    #[test]
    fn nao_se_salta_a_verificacao_nem_a_saude() {
        use Phase::*;
        assert!(!Downloading.may_become(Installing));
        assert!(!Downloaded.may_become(Staged));
        assert!(!Verifying.may_become(AwaitingInstall));
        assert!(!Idle.may_become(Installing));
        assert!(!Installing.may_become(Completed));
        assert!(!Activating.may_become(Completed));
        assert!(!ActivationFailed.may_become(Completed));
        assert!(!RollingBack.may_become(Completed));
        // Once something changed, the transaction cannot be dropped.
        assert!(!Installing.may_become(Idle));
        assert!(!Activating.may_become(Idle));
        assert!(!AwaitingReboot.may_become(Idle));
        for p in [Installing, Activating, AwaitingReboot, RollingBack] {
            assert!(!p.before_mutation());
            assert!(p.may_become(RecoveryRequired));
        }
    }
}
