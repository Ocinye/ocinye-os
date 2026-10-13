//! The Update Engine, as the System Service reaches it (ADR-0032).
//!
//! The engine is a separate root service because an update restarts this one.
//! The System Service stays the only door: it authorises the member for the
//! capability, then forwards a typed request over the engine's root-only
//! socket and returns the engine's typed answer. No path, URL or command is
//! built here from anything but the validated request.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

use ocinye_system_contracts::reason::{AdapterFailure, Reason};
use ocinye_system_contracts::request::{Payload, Request};
use ocinye_update_contracts::wire::{EngineReply, EngineRequest, Operation, MAX_FRAME, PROTOCOL};

/// The engine's socket.
#[derive(Debug, Clone)]
pub struct UpdateLink {
    socket: PathBuf,
}

impl UpdateLink {
    /// A link to the engine at `socket`.
    #[must_use]
    pub fn new(socket: PathBuf) -> Self {
        Self { socket }
    }

    /// The engine operation a request maps to; `None` for a request that is
    /// not an update or base capability.
    #[must_use]
    pub fn operation(request: &Request) -> Option<Operation> {
        Some(match request {
            Request::UpdateStatus {} => Operation::Status,
            Request::UpdateCheck {} => Operation::Check,
            Request::UpdateDownload {} => Operation::Download,
            Request::UpdateInstall {} => Operation::Install,
            Request::UpdateCancel {} => Operation::Cancel,
            Request::UpdateHistory {} => Operation::History,
            Request::UpdateRollback {} => Operation::Rollback,
            Request::UpdateConfigure { change } => Operation::Configure {
                change: change.clone(),
            },
            Request::UpdateImport { bundle } => Operation::Import {
                bundle: bundle.clone(),
            },
            Request::BaseStatus {} => Operation::BaseStatus,
            Request::BaseCheck {} => Operation::BaseCheck,
            Request::BaseApply {} => Operation::BaseApply,
            _ => return None,
        })
    }

    /// Ask the engine. `actor` is the audit actor; the engine keeps an opaque
    /// form of it (letters, digits, `-`, `_`).
    pub fn ask(&self, operation: Operation, actor: &str, via: &str) -> Result<EngineReply, Reason> {
        let unreachable = || Reason::AdapterFailed {
            cause: AdapterFailure::Unreachable,
        };
        let opaque: String = actor
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '-'
                }
            })
            .take(64)
            .collect();
        let request = EngineRequest {
            protocol: PROTOCOL,
            operation,
            actor: if opaque.is_empty() {
                "unknown".into()
            } else {
                opaque
            },
            via: via.to_owned(),
        };
        let mut stream = UnixStream::connect(&self.socket).map_err(|_| unreachable())?;
        // Status and checks answer quickly; long work (download, install)
        // runs in the engine and is followed through `update.status`.
        stream
            .set_read_timeout(Some(Duration::from_secs(120)))
            .map_err(|_| unreachable())?;
        let mut line = serde_json::to_vec(&request).map_err(|_| Reason::MalformedRequest)?;
        line.push(b'\n');
        stream.write_all(&line).map_err(|_| unreachable())?;
        stream
            .shutdown(std::net::Shutdown::Write)
            .map_err(|_| unreachable())?;
        let mut reply = Vec::new();
        stream
            .take(MAX_FRAME as u64 + 1)
            .read_to_end(&mut reply)
            .map_err(|_| Reason::AdapterFailed {
                cause: AdapterFailure::Timeout,
            })?;
        if reply.len() > MAX_FRAME {
            return Err(Reason::AdapterFailed {
                cause: AdapterFailure::UnexpectedReply,
            });
        }
        serde_json::from_slice(&reply).map_err(|_| Reason::AdapterFailed {
            cause: AdapterFailure::UnexpectedReply,
        })
    }

    /// Carry out an update or base request.
    pub fn perform(&self, request: &Request, actor: &str, via: &str) -> Result<Payload, Reason> {
        let op = Self::operation(request).ok_or(Reason::NotImplemented)?;
        match self.ask(op, actor, via)? {
            EngineReply::State { state } => Ok(Payload::Update(state)),
            EngineReply::History { history } => Ok(Payload::UpdateHistory(history)),
            EngineReply::Base { state } => Ok(Payload::Base(state)),
            EngineReply::Refused { reason } => Err(Reason::UpdateRefused { update: reason }),
        }
    }
}
