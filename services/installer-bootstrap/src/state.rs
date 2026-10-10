//! The installer's state on the server: `/var/lib/ocinye-installer/<id>/`.
//!
//! ```text
//! journal.json   the journal (no secrets)            0600
//! plan.json      the confirmed plan (no secrets)     0600
//! events.jsonl   every event, sequenced (no secrets) 0600
//! control.sock   the running executor's socket       0600
//! ocinye-bootstrap  this executable, for the run     0700 (removed in P16)
//! tls/           operator TLS material, until P07    0700 (key removed after)
//! ```
//!
//! Writes are atomic (temp file + rename) and the directory is root-only, so
//! a reader never sees half a journal, and a non-root user never sees any.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use ocinye_installer_contracts::ident::InstallationId;
use ocinye_installer_contracts::journal::InstallationJournal;
use ocinye_installer_contracts::paths;
use ocinye_installer_contracts::protocol::Envelope;

/// The state root (`/var/lib/ocinye-installer`), overridable for tests.
#[derive(Debug, Clone)]
pub struct StateRoot(pub PathBuf);

impl StateRoot {
    /// The real one.
    #[must_use]
    pub fn system() -> Self {
        Self(PathBuf::from(paths::INSTALLER_STATE))
    }

    /// One installation's directory.
    #[must_use]
    pub fn dir(&self, id: &InstallationId) -> PathBuf {
        self.0.join(id.as_str())
    }

    /// Create the root and an installation directory, root-only.
    ///
    /// # Errors
    ///
    /// I/O errors.
    pub fn create(&self, id: &InstallationId) -> std::io::Result<PathBuf> {
        fs::create_dir_all(&self.0)?;
        fs::set_permissions(&self.0, fs::Permissions::from_mode(0o700))?;
        let dir = self.dir(id);
        fs::create_dir_all(&dir)?;
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))?;
        Ok(dir)
    }

    /// Every installation that left a journal, newest first.
    #[must_use]
    pub fn journals(&self) -> Vec<InstallationJournal> {
        let mut out: Vec<InstallationJournal> = fs::read_dir(&self.0)
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .filter_map(|e| {
                let id = InstallationId::parse(&e.file_name().to_string_lossy()).ok()?;
                read_journal(&self.dir(&id))
            })
            .collect();
        out.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        out
    }
}

/// Write a file atomically with the given mode.
///
/// # Errors
///
/// I/O errors.
pub fn write_atomic(path: &Path, bytes: &[u8], mode: u32) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    {
        let mut f = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(mode)
            .open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    fs::set_permissions(&tmp, fs::Permissions::from_mode(mode))?;
    fs::rename(tmp, path)
}

/// Read a journal.
#[must_use]
pub fn read_journal(dir: &Path) -> Option<InstallationJournal> {
    serde_json::from_slice(&fs::read(dir.join("journal.json")).ok()?).ok()
}

/// Write a journal.
///
/// # Errors
///
/// I/O errors.
pub fn write_journal(dir: &Path, journal: &InstallationJournal) -> std::io::Result<()> {
    let bytes = serde_json::to_vec_pretty(journal).map_err(std::io::Error::other)?;
    write_atomic(&dir.join("journal.json"), &bytes, 0o600)
}

/// Append one event to `events.jsonl`.
///
/// # Errors
///
/// I/O errors.
pub fn append_event(dir: &Path, envelope: &Envelope) -> std::io::Result<()> {
    let mut f = OpenOptions::new()
        .append(true)
        .create(true)
        .mode(0o600)
        .open(dir.join("events.jsonl"))?;
    let line = serde_json::to_string(envelope).map_err(std::io::Error::other)?;
    writeln!(f, "{line}")?;
    f.sync_data()
}

/// Every event with `seq >= from`.
#[must_use]
pub fn events_from(dir: &Path, from: u64) -> Vec<Envelope> {
    fs::read_to_string(dir.join("events.jsonl"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str::<Envelope>(l).ok())
        .filter(|e| e.seq >= from)
        .collect()
}

/// The last sequence number written.
#[must_use]
pub fn last_seq(dir: &Path) -> u64 {
    events_from(dir, 0).last().map_or(0, |e| e.seq)
}

/// RFC 3339 now.
#[must_use]
pub fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ocinye_installer_contracts::ident::PlanId;
    use ocinye_installer_contracts::protocol::Event;

    #[test]
    fn o_diario_escreve_se_inteiro_so_para_root_e_le_se_de_volta() {
        let root =
            StateRoot(std::env::temp_dir().join(format!("ocinye-state-{}", std::process::id())));
        let id = InstallationId::parse("inst-0123456789abcdef").unwrap();
        let dir = root.create(&id).unwrap();
        assert_eq!(
            fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
            0o700
        );
        let j = InstallationJournal::new(
            id.clone(),
            PlanId::parse("pl-0123456789abcdef").unwrap(),
            "a".repeat(64),
            "3f9c2a7d1e04".into(),
            "SHA256:x".into(),
            &[],
            now(),
        );
        write_journal(&dir, &j).unwrap();
        let meta = fs::metadata(dir.join("journal.json")).unwrap();
        assert_eq!(meta.permissions().mode() & 0o777, 0o600);
        assert_eq!(read_journal(&dir).unwrap(), j);
        assert_eq!(root.journals().len(), 1);

        for seq in 1..=3 {
            append_event(
                &dir,
                &Envelope {
                    seq,
                    at: now(),
                    event: Event::CleanedUp,
                },
            )
            .unwrap();
        }
        assert_eq!(events_from(&dir, 2).len(), 2);
        assert_eq!(last_seq(&dir), 3);
        let _ = fs::remove_dir_all(&root.0);
    }
}
