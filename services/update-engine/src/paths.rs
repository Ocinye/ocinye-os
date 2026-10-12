//! Where the engine reads and writes. Every path hangs off [`Paths`], so the
//! tests run the real engine against a tree in a temporary directory.

use std::path::{Path, PathBuf};

/// The engine's paths.
#[derive(Debug, Clone)]
pub struct Paths {
    /// `/var/lib/ocinye/update`: journal, history, downloads, TUF cache.
    pub state: PathBuf,
    /// `/etc/ocinye/update.json`.
    pub config: PathBuf,
    /// `/usr/share/ocinye/update/trust`: `<channel>/root.json`.
    pub trust: PathBuf,
    /// `/usr/lib/ocinye`: `releases/<version>/`, `current`, `previous`.
    pub native_root: PathBuf,
    /// `/usr/lib/ocinye/release`: the release the image carries for local
    /// activation, exactly one directory.
    pub carried_release: PathBuf,
    /// `/etc/systemd/system`.
    pub units: PathBuf,
    /// `/etc/ocinye/release.env`: present when an Instance is installed.
    pub release_env: PathBuf,
    /// `/media`: removable media an operator mounted.
    pub media: PathBuf,
    /// `/run/lock/ocinye-maintenance.lock`.
    pub lock: PathBuf,
    /// `/run/ocinye/update.sock`.
    pub socket: PathBuf,
    /// `/run/reboot-required`.
    pub reboot_required: PathBuf,
    /// `/etc/apt`: the package sources base maintenance checks.
    pub apt: PathBuf,
    /// `/var/lib/dpkg/updates`: not empty while a `dpkg` run is unfinished.
    pub dpkg_updates: PathBuf,
    /// `/etc/ocinye/image.json`: what the image says about itself.
    pub image_facts: PathBuf,
    /// `/var/lib/ocinye-firstboot/identity.json`: written last by the first
    /// start, so its presence means the first start finished.
    pub firstboot_state: PathBuf,
}

impl Paths {
    /// The installed system.
    #[must_use]
    pub fn system() -> Self {
        Self::under(Path::new("/"))
    }

    /// The same layout under `root` (tests).
    #[must_use]
    pub fn under(root: &Path) -> Self {
        let p = |s: &str| root.join(s.trim_start_matches('/'));
        Self {
            state: p(ocinye_update_contracts::STATE_DIR),
            config: p(ocinye_update_contracts::CONFIG_PATH),
            trust: p(ocinye_update_contracts::TRUST_DIR),
            native_root: p(ocinye_update_contracts::NATIVE_ROOT),
            carried_release: p("/usr/lib/ocinye/release"),
            units: p("/etc/systemd/system"),
            release_env: p("/etc/ocinye/release.env"),
            media: p("/media"),
            lock: p("/run/lock/ocinye-maintenance.lock"),
            socket: p(ocinye_update_contracts::SOCKET),
            reboot_required: p("/run/reboot-required"),
            apt: p("/etc/apt"),
            dpkg_updates: p("/var/lib/dpkg/updates"),
            image_facts: p("/etc/ocinye/image.json"),
            firstboot_state: p("/var/lib/ocinye-firstboot/identity.json"),
        }
    }

    /// The journal.
    #[must_use]
    pub fn journal(&self) -> PathBuf {
        self.state.join("journal.json")
    }
    /// The history.
    #[must_use]
    pub fn history(&self) -> PathBuf {
        self.state.join("history.json")
    }
    /// The audit record (JSON lines, bounded).
    #[must_use]
    pub fn audit(&self) -> PathBuf {
        self.state.join("audit.jsonl")
    }
    /// The base maintenance record.
    #[must_use]
    pub fn base(&self) -> PathBuf {
        self.state.join("base.json")
    }
    /// The TUF metadata cache of a channel: what this machine has seen, so
    /// that older metadata is refused.
    #[must_use]
    pub fn tuf(&self, channel: ocinye_update_contracts::Channel) -> PathBuf {
        self.state.join("tuf").join(channel.as_str())
    }
    /// Partial downloads, by SHA-256: nothing here is trusted.
    #[must_use]
    pub fn partial(&self) -> PathBuf {
        self.state.join("partial")
    }
    /// Verified artifacts of a release, by target path.
    #[must_use]
    pub fn verified(&self, version: &str) -> PathBuf {
        self.state.join("verified").join(version)
    }
    /// An unpacked offline bundle.
    #[must_use]
    pub fn offline(&self) -> PathBuf {
        self.state.join("offline")
    }
    /// The operator's inbox for offline bundles.
    #[must_use]
    pub fn inbox(&self) -> PathBuf {
        self.state.join("inbox")
    }
    /// A native release.
    #[must_use]
    pub fn native_release(&self, version: &str) -> PathBuf {
        self.native_root.join("releases").join(version)
    }
    /// The `current` pointer.
    #[must_use]
    pub fn current(&self) -> PathBuf {
        self.native_root.join("current")
    }
    /// The `previous` pointer.
    #[must_use]
    pub fn previous(&self) -> PathBuf {
        self.native_root.join("previous")
    }
    /// Where a replaced carried release is kept until the update commits.
    #[must_use]
    pub fn carried_previous(&self) -> PathBuf {
        self.state.join("carried-previous")
    }
}
