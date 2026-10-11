//! The audit trail of the System Service: append-only JSON lines, owned by
//! root, written by this process only.
//!
//! A record names who, which capability, when, with what result, and the
//! non-secret summary a request offers of itself. It is built from
//! [`Request::audit_summary`], never by serialising the request: a passphrase
//! has no path into this file.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use ocinye_system_contracts::request::Request;
use serde::Serialize;

/// One audited action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AuditRecord {
    /// Unix seconds.
    pub at: u64,
    /// `presence`, `member:<id>` or `recovery-operator`.
    pub actor: String,
    /// The capability's dotted name.
    pub capability: &'static str,
    /// `OK` or the reason code.
    pub result: String,
    /// Safe metadata.
    pub summary: String,
}

impl AuditRecord {
    /// A record for a request, now.
    #[must_use]
    pub fn new(actor: &str, request: &Request, result: &str) -> Self {
        Self {
            at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| d.as_secs()),
            actor: actor.to_owned(),
            capability: request.capability().as_str(),
            result: result.to_owned(),
            summary: request.audit_summary(),
        }
    }
}

/// Where records go.
pub trait AuditSink: Send + Sync {
    /// Record one action. Must not fail the request: a full disk is reported
    /// by health, not by refusing to restart the machine.
    fn record(&self, record: &AuditRecord);
}

/// The file sink.
pub struct FileAudit {
    path: PathBuf,
    lock: Mutex<()>,
}

impl FileAudit {
    /// A sink appending to `path`.
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            lock: Mutex::new(()),
        }
    }
}

impl AuditSink for FileAudit {
    fn record(&self, record: &AuditRecord) {
        let Ok(line) = serde_json::to_string(record) else {
            return;
        };
        let _guard = self.lock.lock();
        if let Ok(mut f) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
        {
            let _ = writeln!(f, "{line}");
        }
    }
}
