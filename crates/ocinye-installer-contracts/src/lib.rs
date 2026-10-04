//! Typed contracts of the Ocinye OS Installer (D011, ADR-0022 a ADR-0025).
//!
//! The graphical Installer runs on the operator's computer; a temporary,
//! constrained executor — `ocinye-bootstrap` — runs on the target server for
//! the duration of the installation. They speak **only** the types in this
//! crate: a closed command enum one way, typed events the other, JSON lines over
//! the SSH channel. There is no `execute_shell`, `run_remote` or free-form
//! command anywhere, and nothing here can carry one.
//!
//! # What belongs here
//!
//! Validated identifiers ([`ident`]), the release manifest and its canonical
//! serialisation ([`manifest`], [`canonical`]), hardware and preflight facts
//! ([`hardware`], [`preflight`]), the immutable installation plan ([`plan`]),
//! the protocol ([`protocol`]), the server-side journal ([`journal`]), product
//! verification and the installation lifecycle ([`verification`]), the receipt
//! ([`receipt`]) and the secret wrappers that keep credentials out of logs
//! ([`secret`]).
//!
//! # What does not belong here
//!
//! I/O. Reading the server, talking SSH or running a package manager is the
//! bootstrap's and the controller's job; this crate decides what those facts
//! *mean*, so both ends decide it the same way.
//!
//! # The Compute boundary
//!
//! Hardware is **detected**, never enrolled. There is no scheduler, provider
//! wallet, settlement or marketplace type here, and [`hardware::ProviderMode`]
//! has one value: `Off` (D012 / future).

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod canonical;
pub mod core_output;
pub mod hardware;
pub mod ident;
pub mod journal;
pub mod manifest;
pub mod openpgp;
pub mod plan;
pub mod preflight;
pub mod protocol;
pub mod receipt;
pub mod secret;
pub mod verification;

/// The version of the Installer ↔ bootstrap protocol this crate speaks.
pub const BOOTSTRAP_PROTOCOL: u16 = 1;

/// The only server platform of Ocinye OS v1 (product decision, 2026-10-04):
/// Ubuntu Server 24.04 LTS, minimal installation, no graphical environment.
pub const SUPPORTED_TARGET: &str = "ubuntu-24.04";

/// Fixed remote locations. The operator never provides a remote path.
pub mod paths {
    /// The release tree and data root.
    pub const ROOT: &str = "/srv/ocinye";
    /// Configuration and secrets (0600 files).
    pub const CONFIG: &str = "/etc/ocinye";
    /// The installation journal, events and the session socket.
    pub const INSTALLER_STATE: &str = "/var/lib/ocinye-installer";
    /// Where the bootstrap is uploaded for one session (`<prefix><random>`).
    pub const UPLOAD_PREFIX: &str = "/tmp/ocinye-bootstrap-";
}
