//! The engine's protocol: one JSON request per connection, one JSON reply.
//!
//! Only root may connect (the System Service); the socket is `0600 root`. The
//! System Service has already authorised the member for the capability; the
//! engine records who asked and through what, and decides nothing about
//! people.

use crate::base::BaseState;
use crate::config::RepositoryEndpoint;
use crate::reason::UpdateReason;
use crate::state::{UpdateHistory, UpdateState};
use crate::version::Channel;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Protocol version.
pub const PROTOCOL: u32 = 1;
/// A request or a reply is at most 64 KiB on the wire.
pub const MAX_FRAME: usize = 64 * 1024;

/// What the configuration change touches; absent fields stay as they are.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ConfigChange {
    /// New channel.
    pub channel: Option<Channel>,
    /// Set the repository.
    pub repository: Option<RepositoryEndpoint>,
    /// Remove the repository.
    #[serde(default)]
    pub clear_repository: bool,
    /// Check by itself.
    pub auto_check: Option<bool>,
    /// Download by itself.
    pub auto_download: Option<bool>,
}

/// A request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    /// The state.
    Status,
    /// Read the signed metadata of the channel.
    Check,
    /// Download, verify, preflight and stage the offered release.
    Download,
    /// Install the staged release.
    Install,
    /// Drop a transaction that has not changed anything yet.
    Cancel,
    /// The history.
    History,
    /// Go back to the previous release.
    Rollback,
    /// Change the configuration.
    Configure {
        /// What changes.
        change: ConfigChange,
    },
    /// Offer the release in an offline bundle the engine listed.
    Import {
        /// Its name, as listed.
        bundle: String,
    },
    /// Base maintenance state.
    BaseStatus,
    /// Refresh the base package lists from the approved sources.
    BaseCheck,
    /// Apply pending base package upgrades.
    BaseApply,
}

/// The envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct EngineRequest {
    /// [`PROTOCOL`].
    pub protocol: u32,
    /// The operation.
    pub operation: Operation,
    /// Opaque reference to who asked (member id, or `local`); never a name.
    pub actor: String,
    /// `settings`, `cli`, `recovery`, `nye`.
    pub via: String,
}

/// A reply.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "reply", rename_all = "snake_case", deny_unknown_fields)]
pub enum EngineReply {
    /// The update state after the operation.
    State {
        /// State.
        state: Box<UpdateState>,
    },
    /// The history.
    History {
        /// History.
        history: UpdateHistory,
    },
    /// The base state.
    Base {
        /// State.
        state: BaseState,
    },
    /// Refused or failed, with the reason; the state is unchanged unless the
    /// reason says otherwise.
    Refused {
        /// Reason.
        reason: UpdateReason,
    },
}

/// `via` values the engine records.
pub const VIA: [&str; 5] = ["settings", "cli", "recovery", "nye", "boot"];

impl EngineRequest {
    /// Envelope rules.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.protocol != PROTOCOL {
            return Err("protocol");
        }
        if !VIA.contains(&self.via.as_str()) {
            return Err("via");
        }
        if self.actor.is_empty()
            || self.actor.len() > 64
            || !self
                .actor
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err("actor");
        }
        if let Operation::Import { bundle } = &self.operation {
            if !crate::manifest::is_safe_target(bundle) || bundle.contains('/') {
                return Err("bundle name");
            }
        }
        if let Operation::Configure { change } = &self.operation {
            if let Some(r) = &change.repository {
                r.validate()?;
            }
            if change.clear_repository && change.repository.is_some() {
                return Err("set or clear the repository, not both");
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recusa_o_que_nao_e_o_protocolo() {
        let ok = EngineRequest {
            protocol: PROTOCOL,
            operation: Operation::Import {
                bundle: "ocinye-os-d34bbb0a141d-r2.ocu".into(),
            },
            actor: "local".into(),
            via: "cli".into(),
        };
        assert!(ok.validate().is_ok());
        let mut bad = ok.clone();
        bad.operation = Operation::Import {
            bundle: "../../etc/shadow".into(),
        };
        assert!(bad.validate().is_err());
        let mut bad = ok.clone();
        bad.via = "shell".into();
        assert!(bad.validate().is_err());
        let mut bad = ok.clone();
        bad.actor = "Fidel Monteiro".into();
        assert!(bad.validate().is_err());
        let text = r#"{"protocol":1,"operation":{"op":"exec","command":"rm -rf /"},"actor":"local","via":"cli"}"#;
        assert!(serde_json::from_str::<EngineRequest>(text).is_err());
    }
}
