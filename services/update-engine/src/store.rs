//! The journal, the history and the audit record on disk.
//!
//! Every write is a temporary file, `fsync`, `rename`, `fsync` of the
//! directory: after a power cut the file is the old one or the new one. The
//! journal carries a sequence number; one that goes back is refused rather
//! than believed.

use crate::paths::Paths;
use ocinye_update_contracts::journal::{push_history, HistoryEntry, Journal};
use ocinye_update_contracts::UpdateReason;
use std::io::Write;
use std::path::Path;

/// Write `bytes` to `path` so that a crash leaves the old or the new file.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let dir = path
        .parent()
        .ok_or_else(|| std::io::Error::other("no parent"))?;
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join(format!(
        ".{}.tmp",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("file")
    ));
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, path)?;
    std::fs::File::open(dir)?.sync_all()?;
    Ok(())
}

/// The engine's records.
#[derive(Debug, Clone)]
pub struct Store {
    paths: Paths,
}

impl Store {
    /// Records under `paths`.
    #[must_use]
    pub fn new(paths: Paths) -> Self {
        Self { paths }
    }

    /// The journal, or `None` when there is none yet.
    pub fn load_journal(&self) -> Result<Option<Journal>, UpdateReason> {
        match std::fs::read(self.paths.journal()) {
            Ok(bytes) => Journal::parse(&bytes)
                .map(Some)
                .map_err(|_| UpdateReason::JournalInvalid),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(_) => Err(UpdateReason::JournalInvalid),
        }
    }

    /// Save the journal, one sequence number on. The caller's copy is
    /// updated with the number written.
    pub fn save_journal(&self, journal: &mut Journal) -> Result<(), UpdateReason> {
        if let Ok(Some(on_disk)) = self.load_journal() {
            if on_disk.sequence > journal.sequence {
                return Err(UpdateReason::JournalInvalid);
            }
        }
        journal.sequence += 1;
        let bytes = serde_json::to_vec_pretty(journal).map_err(|_| UpdateReason::JournalInvalid)?;
        write_atomic(&self.paths.journal(), &bytes).map_err(|_| UpdateReason::JournalInvalid)
    }

    /// The history, newest first; an unreadable history is empty, never an
    /// error that blocks an update.
    #[must_use]
    pub fn history(&self) -> Vec<HistoryEntry> {
        std::fs::read(self.paths.history())
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    /// Add an entry.
    pub fn push_history(&self, entry: HistoryEntry) -> std::io::Result<()> {
        let mut h = self.history();
        push_history(&mut h, entry);
        write_atomic(&self.paths.history(), &serde_json::to_vec_pretty(&h)?)
    }

    /// Append an audit line. Bounded: past 1 MiB the file restarts, keeping
    /// the previous one as `.1`. Never a secret: only codes, versions, the
    /// opaque actor and the channel.
    pub fn audit(&self, line: &serde_json::Value) {
        let path = self.paths.audit();
        if std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0) > 1 << 20 {
            let _ = std::fs::rename(&path, path.with_extension("jsonl.1"));
        }
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        {
            let _ = writeln!(f, "{line}");
            let _ = f.sync_data();
        }
        println!("OCINYE-UPDATE-AUDIT {line}");
    }
}

/// An exclusive maintenance lock, held by an open file descriptor: the kernel
/// releases it when the process dies, so a crash never leaves it held.
#[derive(Debug)]
pub struct MaintenanceLock {
    _file: std::fs::File,
}

impl MaintenanceLock {
    /// Take the lock now, or say it is busy.
    pub fn try_acquire(path: &Path) -> Result<Self, UpdateReason> {
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d).map_err(|_| UpdateReason::MaintenanceBusy)?;
        }
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(path)
            .map_err(|_| UpdateReason::MaintenanceBusy)?;
        rustix::fs::flock(&file, rustix::fs::FlockOperation::NonBlockingLockExclusive)
            .map_err(|_| UpdateReason::MaintenanceBusy)?;
        Ok(Self { _file: file })
    }
}
