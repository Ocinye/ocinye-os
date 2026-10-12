//! The Ocinye update repository (ADR-0032).
//!
//! A repository is static: TUF metadata signed by roles whose keys never live
//! on the server, and immutable targets named by their content. This crate is
//! everything that writes or reads one:
//!
//! - [`keys`]: role keys and the signed root, with rotation;
//! - [`stage`]: a release staged from a D011 bundle and a native layer;
//! - [`publish`]: adding a release to a channel, atomically;
//! - [`verify`]: loading a repository as a client does, and reading its
//!   releases;
//! - [`offline`]: one release in a single file, for a machine without network;
//! - [`archive`]: extracting an archive without letting it leave its directory;
//! - [`transport`]: HTTP(S) for the TUF client, with nothing trusted in it.
//!
//! The publisher (`ocinye-release`) and the engine's tests use the same code,
//! so a repository the tests accept is one the publisher produces.

pub mod archive;
pub mod keys;
pub mod offline;
pub mod publish;
pub mod stage;
pub mod transport;
pub mod verify;

use std::fmt;

/// Every failure of this crate, with a message for an operator. Never carries
/// key material.
#[derive(Debug)]
pub struct Error(pub String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self(format!("i/o: {e}"))
    }
}

impl From<tough::schema::Error> for Error {
    fn from(e: tough::schema::Error) -> Self {
        Self(format!("tuf schema: {e}"))
    }
}

impl From<tough::error::Error> for Error {
    fn from(e: tough::error::Error) -> Self {
        Self(format!("tuf: {e}"))
    }
}

/// Crate result.
pub type Result<T> = std::result::Result<T, Error>;

/// Shorthand for an error with a message.
pub(crate) fn err<T>(msg: impl Into<String>) -> Result<T> {
    Err(Error(msg.into()))
}

/// The TUF target name of a release artifact: `releases/<version>/<target>`.
#[must_use]
pub fn target_name(version: &ocinye_update_contracts::ReleaseVersion, target: &str) -> String {
    format!("releases/{version}/{target}")
}

/// Lower-case hex SHA-256 of a file, streamed.
pub fn sha256_file(path: &std::path::Path) -> Result<(String, u64)> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    let mut n: u64 = 0;
    loop {
        let r = f.read(&mut buf)?;
        if r == 0 {
            break;
        }
        h.update(&buf[..r]);
        n += r as u64;
    }
    Ok((hex::encode(h.finalize()), n))
}
