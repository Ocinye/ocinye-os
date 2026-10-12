//! Typed contracts of the Ocinye Update Engine (ADR-0032).
//!
//! The ISO installs Ocinye; the Update Engine evolves it. Everything the
//! engine, the System Service, the native interface, the headless client and
//! the release publisher exchange about a release is defined here, once:
//!
//! - [`version`]: the release identity — the D011 release id with a revision —
//!   and the publisher's monotonic release number;
//! - [`manifest`]: what a release contains and what it asks of the machine;
//! - [`state`]: the durable state machine and the state the interface reads;
//! - [`journal`]: the on-disk journal and the bounded history;
//! - [`config`]: the machine's update configuration;
//! - [`reason`]: stable reason codes;
//! - [`wire`]: the engine's request/reply protocol;
//! - [`base`]: Ubuntu base maintenance, which is a different thing and says so.
//!
//! Parsers are strict: unknown fields are refused, and every security-relevant
//! value is validated before it is used.

pub mod base;
pub mod config;
pub mod journal;
pub mod manifest;
pub mod reason;
pub mod state;
pub mod version;
pub mod wire;

pub use base::{BaseOutcome, BasePhase, BaseState};
pub use config::{RepositoryEndpoint, UpdateConfig};
pub use journal::{HistoryEntry, Journal, Step, StepRecord, Transaction, HISTORY_LIMIT};
pub use manifest::{
    Arch, Artifact, ArtifactRole, Build, ImageRef, ManifestError, MigrationPlan, MigrationPolicy,
    Notes, SpaceRequirement, Synthetic, UpdateManifest,
};
pub use reason::UpdateReason;
pub use state::{
    AvailableRelease, OfflineBundle, Phase, Progress, ReleaseInfo, ReleaseSource, UpdateHistory,
    UpdateOutcome, UpdateState,
};
pub use version::{Channel, ReleaseNumber, ReleaseVersion};
pub use wire::{ConfigChange, EngineReply, EngineRequest, Operation};

/// The engine's state directory: journal, history, lock, TUF metadata cache,
/// downloads and staged bundles.
pub const STATE_DIR: &str = "/var/lib/ocinye/update";
/// The machine's update configuration.
pub const CONFIG_PATH: &str = "/etc/ocinye/update.json";
/// The trusted TUF roots, one directory per channel, public keys only.
pub const TRUST_DIR: &str = "/usr/share/ocinye/update/trust";
/// Where versioned native releases live; `current` and `previous` are the
/// pointers, swapped by `rename(2)`.
pub const NATIVE_ROOT: &str = "/usr/lib/ocinye";
/// The engine's socket, for root only (the System Service).
pub const SOCKET: &str = "/run/ocinye/update.sock";
/// Where an operator drops offline bundles; removable media mounted under
/// `/media` are also looked at.
pub const INBOX_DIR: &str = "/var/lib/ocinye/update/inbox";
/// The extension of an offline update bundle.
pub const OFFLINE_EXTENSION: &str = "ocu";
/// The TUF target name of a release's manifest, under its release directory.
pub const MANIFEST_TARGET: &str = "manifest.json";

/// How releases are signed on this build.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReleaseSigning {
    /// Development keys only: never reported as production-signed.
    DevOnly,
    /// A production root and release key, from a recorded ceremony.
    Production,
}
