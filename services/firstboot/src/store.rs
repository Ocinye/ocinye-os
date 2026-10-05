//! Where first boot keeps its state, and how it writes it.
//!
//! Persistent (`/var/lib/ocinye-firstboot`, root 0700): identity, claim state,
//! provisioned keys, first-boot result, journal. Volatile
//! (`/run/ocinye-firstboot`, tmpfs): the pairing code (root 0600), the
//! `claimable` marker the unprivileged `AuthorizedKeysCommand` reads, and the
//! offered keys it stashes. Every write is temp → fsync → rename → fsync dir,
//! and every claim transition happens under one `flock`, so there is no
//! half-state and no second winner.

use std::fs::{self, File, OpenOptions};
use std::io::Write as _;
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};

use ocinye_image_contracts::firstboot::KeyFingerprint;
use serde::{de::DeserializeOwned, Serialize};

/// The file system roots first boot works under (real or a test directory).
#[derive(Debug, Clone)]
pub struct Root {
    /// Persistent state.
    pub state: PathBuf,
    /// Volatile state (tmpfs).
    pub run: PathBuf,
    /// `/` of the machine (for `/etc`, `/home/ocinye`, the release root).
    pub sys: PathBuf,
}

impl Root {
    /// The machine.
    pub fn system() -> Self {
        Self {
            state: PathBuf::from(ocinye_image_contracts::paths::FIRSTBOOT_STATE),
            run: PathBuf::from(ocinye_image_contracts::paths::FIRSTBOOT_RUN),
            sys: PathBuf::from("/"),
        }
    }

    /// A path under the machine root.
    pub fn sys_path(&self, abs: &str) -> PathBuf {
        self.sys.join(abs.trim_start_matches('/'))
    }

    pub fn identity(&self) -> PathBuf {
        self.state.join("identity.json")
    }
    pub fn claim_state(&self) -> PathBuf {
        self.state.join("state.json")
    }
    pub fn provisioned(&self) -> PathBuf {
        self.state.join("provisioned-keys.json")
    }
    pub fn firstboot(&self) -> PathBuf {
        self.state.join("firstboot.json")
    }
    pub fn journal(&self) -> PathBuf {
        self.state.join("journal.jsonl")
    }
    pub fn lock(&self) -> PathBuf {
        self.state.join("state.lock")
    }
    pub fn code(&self) -> PathBuf {
        self.run.join("code.json")
    }
    /// Present only while `Unclaimed` and not locked; world-readable, no content.
    pub fn claimable(&self) -> PathBuf {
        self.run.join("claimable")
    }
    /// Offered keys, by fingerprint digest (owned by the AKC user).
    pub fn offered(&self) -> PathBuf {
        self.run.join("offered")
    }
    /// `~ocinye/.ssh/authorized_keys`.
    pub fn ocinye_keys(&self) -> PathBuf {
        self.sys_path("/home/ocinye/.ssh/authorized_keys")
    }

    /// Create the state directories with their modes.
    pub fn prepare(&self) -> std::io::Result<()> {
        fs::create_dir_all(&self.state)?;
        fs::set_permissions(&self.state, fs::Permissions::from_mode(0o700))?;
        fs::create_dir_all(&self.run)?;
        fs::set_permissions(&self.run, fs::Permissions::from_mode(0o755))?;
        Ok(())
    }
}

/// File name for an offered key: hex SHA-256 of the fingerprint text, so a
/// fingerprint never becomes a path.
pub fn offered_name(fp: &KeyFingerprint) -> String {
    use sha2::Digest as _;
    format!("{}.pub", hex::encode(sha2::Sha256::digest(fp.0.as_bytes())))
}

/// Write `bytes` atomically with `mode`.
pub fn write_atomic(path: &Path, bytes: &[u8], mode: u32) -> std::io::Result<()> {
    let dir = path
        .parent()
        .ok_or_else(|| std::io::Error::other("no parent"))?;
    let tmp = dir.join(format!(
        ".{}.tmp",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("state")
    ));
    {
        let mut f = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(mode)
            .open(&tmp)?;
        f.set_permissions(fs::Permissions::from_mode(mode))?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    fs::rename(&tmp, path)?;
    File::open(dir)?.sync_all()
}

/// Canonical JSON, atomically.
pub fn write_json<T: Serialize>(path: &Path, v: &T, mode: u32) -> std::io::Result<()> {
    let s = ocinye_installer_contracts::canonical::to_canonical(v)
        .map_err(|e| std::io::Error::other(format!("{e:?}")))?;
    write_atomic(path, s.as_bytes(), mode)
}

/// `None` when absent; an error when present and unreadable or malformed.
pub fn read_json<T: DeserializeOwned>(path: &Path) -> std::io::Result<Option<T>> {
    match fs::read(path) {
        Ok(b) => serde_json::from_slice(&b)
            .map(Some)
            .map_err(std::io::Error::other),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

pub fn remove(path: &Path) -> std::io::Result<()> {
    match fs::remove_file(path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

/// The single state lock, held for the whole transition.
pub struct Locked {
    _file: File,
}

pub fn lock(root: &Root) -> std::io::Result<Locked> {
    let f = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(root.lock())?;
    f.lock()?;
    Ok(Locked { _file: f })
}

/// One journal line. Never carries a code, a key blob or a secret: event
/// name, generations, fingerprints, source address, time.
#[derive(Debug, Serialize)]
pub struct JournalEntry<'a> {
    pub at: String,
    pub event: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<serde_json::Value>,
}

pub fn journal(root: &Root, event: &str, detail: Option<serde_json::Value>) {
    let e = JournalEntry {
        at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        event,
        detail,
    };
    if let Ok(line) = ocinye_installer_contracts::canonical::to_canonical(&e) {
        if let Ok(mut f) = OpenOptions::new()
            .append(true)
            .create(true)
            .mode(0o600)
            .open(root.journal())
        {
            let _ = writeln!(f, "{line}");
        }
    }
}

#[cfg(test)]
pub mod testing {
    use super::Root;

    /// A disposable root under the target directory.
    pub fn root(name: &str) -> Root {
        let base = std::env::temp_dir().join(format!(
            "ocinye-firstboot-test-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&base);
        let r = Root {
            state: base.join("state"),
            run: base.join("run"),
            sys: base.join("sys"),
        };
        r.prepare().unwrap();
        std::fs::create_dir_all(r.sys.join("home/ocinye/.ssh")).unwrap();
        r
    }
}
