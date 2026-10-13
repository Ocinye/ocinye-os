//! The on-disk journal and the bounded history.
//!
//! The journal is the truth about an update; it is written before the effect
//! it announces (temporary file, `fsync`, `rename`, `fsync` of the directory),
//! so that after a power cut the engine finds either the old record or the new
//! one, never half of one.

use crate::reason::UpdateReason;
use crate::state::{AvailableRelease, Phase, ReleaseInfo, ReleaseSource, UpdateOutcome};
use crate::version::{Channel, ReleaseNumber, ReleaseVersion};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The journal schema this engine writes and reads.
pub const JOURNAL_SCHEMA: u32 = 1;
/// The history keeps this many entries.
pub const HISTORY_LIMIT: usize = 50;
/// A boot of a new release is tried this many times before rolling back.
pub const BOOT_ATTEMPTS: u32 = 3;

/// One step of an activation, recorded when it is about to run and again when
/// it has run, so that a restart knows which side of the effect it is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum Step {
    /// The maintenance lock is held and the system was healthy.
    Entered,
    /// The new release's images are loaded into the container engine.
    ImagesLoaded,
    /// The Instance installer moved the Instance (its own checkpoint first).
    InstanceUpgraded,
    /// The image's carried release (used by local activation) was replaced.
    CarriedReleaseReplaced,
    /// `previous` and `current` point at the old and the new native release.
    NativeSwitched,
    /// The new release's units are in place and systemd reloaded.
    UnitsInstalled,
    /// The services were restarted in order.
    ServicesRestarted,
    /// The health gate passed.
    HealthPassed,
}

/// A step and where it stands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct StepRecord {
    /// Step.
    pub step: Step,
    /// `false` when recorded before the effect, `true` after.
    pub done: bool,
    /// When, RFC 3339 UTC.
    pub at: String,
}

/// The update in flight.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct Transaction {
    /// Random identifier, 16 hex digits; carried into the audit record.
    pub id: String,
    /// The release being installed.
    pub target: ReleaseVersion,
    /// Its number.
    pub target_number: ReleaseNumber,
    /// Its channel.
    pub channel: Channel,
    /// Repository or offline bundle.
    pub source: ReleaseSource,
    /// The release that was running when the transaction began.
    pub from: Option<ReleaseInfo>,
    /// When it began.
    pub started_at: String,
    /// Steps, in order.
    pub steps: Vec<StepRecord>,
    /// Boots of the new release tried so far.
    pub boot_attempts: u32,
    /// Who asked (an opaque member or local actor reference, never a name).
    pub actor: String,
    /// Through what: `settings`, `cli`, `recovery`, `boot`.
    pub via: String,
}

impl Transaction {
    /// Whether a step completed.
    #[must_use]
    pub fn done(&self, step: Step) -> bool {
        self.steps.iter().any(|s| s.step == step && s.done)
    }

    /// Whether a step was begun but not recorded as done.
    #[must_use]
    pub fn begun_not_done(&self, step: Step) -> bool {
        self.steps.iter().any(|s| s.step == step && !s.done) && !self.done(step)
    }
}

/// The journal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct Journal {
    /// [`JOURNAL_SCHEMA`].
    pub schema: u32,
    /// Incremented on every write; a journal that goes back is refused.
    #[ts(type = "number")]
    pub sequence: u64,
    /// Phase.
    pub phase: Phase,
    /// Running release.
    pub current: Option<ReleaseInfo>,
    /// Rollback target.
    pub previous: Option<ReleaseInfo>,
    /// The update in flight, if any.
    pub transaction: Option<Transaction>,
    /// Last successful check.
    pub last_check: Option<String>,
    /// How the last transaction ended.
    pub last_outcome: Option<UpdateOutcome>,
    /// Why, when not the happy path.
    pub reason: Option<UpdateReason>,
    /// The release the last check offered, kept across restarts.
    pub offered: Option<AvailableRelease>,
}

impl Journal {
    /// A journal for a machine that has never updated.
    #[must_use]
    pub fn fresh(current: Option<ReleaseInfo>) -> Self {
        Self {
            schema: JOURNAL_SCHEMA,
            sequence: 0,
            phase: Phase::Idle,
            current,
            previous: None,
            transaction: None,
            last_check: None,
            last_outcome: None,
            reason: None,
            offered: None,
        }
    }

    /// Parse, refusing another schema.
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let j: Self = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        if j.schema != JOURNAL_SCHEMA {
            return Err("unknown journal schema".into());
        }
        if j.phase.before_mutation() && j.phase != Phase::Idle && j.transaction.is_none() {
            // A download or a staged release always has its transaction.
            if !matches!(
                j.phase,
                Phase::UpdateAvailable
                    | Phase::DownloadFailed
                    | Phase::VerificationFailed
                    | Phase::PreflightFailed
                    | Phase::Checking
            ) {
                return Err("phase without a transaction".into());
            }
        }
        if !j.phase.before_mutation()
            && !matches!(
                j.phase,
                Phase::Completed
                    | Phase::InstallFailed
                    | Phase::RolledBack
                    | Phase::RecoveryRequired
            )
            && j.transaction.is_none()
        {
            return Err("phase without a transaction".into());
        }
        Ok(j)
    }
}

/// One finished transaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct HistoryEntry {
    /// Transaction id.
    pub id: String,
    /// Release installed, or attempted.
    pub version: ReleaseVersion,
    /// Its number.
    pub release_number: ReleaseNumber,
    /// Its channel.
    pub channel: Channel,
    /// The release that was running.
    pub from: Option<ReleaseVersion>,
    /// Began.
    pub started_at: String,
    /// Ended.
    pub finished_at: String,
    /// Outcome.
    pub outcome: UpdateOutcome,
    /// Reason, when not committed.
    pub reason: Option<UpdateReason>,
    /// A restart was part of it.
    pub rebooted: bool,
    /// Repository or offline.
    pub source: ReleaseSource,
    /// `settings`, `cli`, `recovery`, `boot`.
    pub via: String,
}

/// Keep the newest [`HISTORY_LIMIT`] entries, newest first.
pub fn push_history(history: &mut Vec<HistoryEntry>, entry: HistoryEntry) {
    history.insert(0, entry);
    history.truncate(HISTORY_LIMIT);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn um_diario_sem_transaccao_em_plena_instalacao_e_recusado() {
        let mut j = Journal::fresh(None);
        j.phase = Phase::Activating;
        let bytes = serde_json::to_vec(&j).unwrap();
        assert!(Journal::parse(&bytes).is_err());
        j.phase = Phase::Idle;
        assert!(Journal::parse(&serde_json::to_vec(&j).unwrap()).is_ok());
        let mut v = serde_json::to_value(&j).unwrap();
        v["schema"] = 2.into();
        assert!(Journal::parse(&serde_json::to_vec(&v).unwrap()).is_err());
    }

    #[test]
    fn historia_limitada_e_mais_recente_primeiro() {
        let mut h = Vec::new();
        for n in 1..=(HISTORY_LIMIT as u64 + 5) {
            push_history(
                &mut h,
                HistoryEntry {
                    id: format!("{n:016x}"),
                    version: ReleaseVersion::parse("d34bbb0a141d-r1").unwrap(),
                    release_number: ReleaseNumber(n),
                    channel: Channel::Development,
                    from: None,
                    started_at: "2026-10-12T10:00:00Z".into(),
                    finished_at: "2026-10-12T10:05:00Z".into(),
                    outcome: UpdateOutcome::Committed,
                    reason: None,
                    rebooted: false,
                    source: ReleaseSource::Repository,
                    via: "cli".into(),
                },
            );
        }
        assert_eq!(h.len(), HISTORY_LIMIT);
        assert_eq!(h[0].release_number, ReleaseNumber(HISTORY_LIMIT as u64 + 5));
    }
}
