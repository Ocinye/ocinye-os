//! Ubuntu base maintenance, kept apart from Ocinye releases.
//!
//! An Ocinye release is one signed, coherent state that can be rolled back. A
//! package upgrade of the Ubuntu base cannot be rolled back atomically, and
//! this module does not pretend it can: it reports what is pending, applies it
//! under the same maintenance lock from the approved sources only, recovers an
//! interrupted `dpkg`, and says when a restart is needed. A major Ubuntu
//! upgrade is not done here (`BASE_OS_MAJOR_UPGRADE = DEFERRED`).

use crate::reason::UpdateReason;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Where base maintenance stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BasePhase {
    /// Nothing running.
    Idle,
    /// Refreshing the package lists.
    Checking,
    /// Upgrading packages.
    Upgrading,
    /// A previous upgrade was interrupted; the next action repairs it first.
    Interrupted,
}

/// How the last base action ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BaseOutcome {
    /// Done.
    Succeeded,
    /// Refused before anything changed.
    Refused,
    /// Failed; packages may be half-configured until the repair runs.
    Failed,
}

/// Everything the interface shows about the base.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct BaseState {
    /// Phase.
    pub phase: BasePhase,
    /// The configured package sources are the approved ones.
    pub sources_approved: bool,
    /// Packages that would be upgraded, at the last check.
    pub pending: u32,
    /// Of those, from the security pocket.
    pub security: u32,
    /// Last successful check.
    pub last_check: Option<String>,
    /// The base asks for a restart (`/run/reboot-required`).
    pub reboot_required: bool,
    /// Last outcome.
    pub last_outcome: Option<BaseOutcome>,
    /// Why, when not the happy path.
    pub reason: Option<UpdateReason>,
    /// Always false in v1: a major base upgrade is deferred.
    pub major_upgrade_supported: bool,
}
