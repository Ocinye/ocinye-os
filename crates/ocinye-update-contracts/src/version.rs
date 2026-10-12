//! Release identity.
//!
//! There is one release identity in Ocinye OS: the D011 release id, the first
//! twelve hex digits of the commit. An update adds the revision (`-r<N>`, the
//! form the D013 images already use), so that the same commit can be
//! republished — a synthetic successor, a re-signed promotion — and the
//! publisher's monotonic release number, which TUF then guarantees never goes
//! back.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use ts_rs::TS;

/// A release channel. The names are the ADR-0026 ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    /// Development builds, signed by a development key.
    Development,
    /// Release candidates.
    Candidate,
    /// The default for a production machine.
    Stable,
}

impl Channel {
    /// The wire name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Development => "development",
            Self::Candidate => "candidate",
            Self::Stable => "stable",
        }
    }

    /// Every channel.
    pub const ALL: [Self; 3] = [Self::Development, Self::Candidate, Self::Stable];

    /// Parse a wire name.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.as_str() == s)
    }
}

/// The publisher's release number: strictly increasing within a channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(transparent)]
#[ts(type = "number")]
pub struct ReleaseNumber(pub u64);

/// `<release_id>-r<revision>`: twelve lower-case hex digits, `-r`, and a
/// revision from 1, without leading zeros.
#[derive(Debug, Clone, PartialEq, Eq, Hash, TS)]
#[ts(type = "string")]
pub struct ReleaseVersion {
    release_id: String,
    revision: u32,
}

impl ReleaseVersion {
    /// Build one from its parts, refusing anything not canonical.
    pub fn new(release_id: &str, revision: u32) -> Result<Self, VersionError> {
        if !is_release_id(release_id) {
            return Err(VersionError::ReleaseId);
        }
        if revision == 0 {
            return Err(VersionError::Revision);
        }
        Ok(Self {
            release_id: release_id.to_owned(),
            revision,
        })
    }

    /// Parse the canonical text form.
    pub fn parse(text: &str) -> Result<Self, VersionError> {
        let (id, rev) = text.split_once("-r").ok_or(VersionError::Form)?;
        if rev.is_empty()
            || rev.len() > 9
            || !rev.bytes().all(|b| b.is_ascii_digit())
            || rev.starts_with('0')
        {
            return Err(VersionError::Revision);
        }
        let revision: u32 = rev.parse().map_err(|_| VersionError::Revision)?;
        Self::new(id, revision)
    }

    /// The D011 release id.
    #[must_use]
    pub fn release_id(&self) -> &str {
        &self.release_id
    }

    /// The revision.
    #[must_use]
    pub const fn revision(&self) -> u32 {
        self.revision
    }
}

impl fmt::Display for ReleaseVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-r{}", self.release_id, self.revision)
    }
}

impl Serialize for ReleaseVersion {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for ReleaseVersion {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        Self::parse(&text).map_err(serde::de::Error::custom)
    }
}

/// Why a version was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionError {
    /// Not `<id>-r<n>`.
    Form,
    /// The release id is not twelve lower-case hex digits.
    ReleaseId,
    /// The revision is zero, not a number, or not canonical.
    Revision,
}

impl fmt::Display for VersionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Form => "a release version is <release_id>-r<revision>",
            Self::ReleaseId => "a release id is twelve lower-case hexadecimal digits",
            Self::Revision => "a revision is a number from 1, without leading zeros",
        })
    }
}

impl std::error::Error for VersionError {}

/// Twelve lower-case hex digits.
#[must_use]
pub fn is_release_id(s: &str) -> bool {
    s.len() == 12 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Forty lower-case hex digits.
#[must_use]
pub fn is_commit(s: &str) -> bool {
    s.len() == 40 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Sixty-four lower-case hex digits.
#[must_use]
pub fn is_sha256(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versao_canonica_e_so_ela() {
        let v = ReleaseVersion::parse("2916a481e178-r3").unwrap();
        assert_eq!(v.release_id(), "2916a481e178");
        assert_eq!(v.revision(), 3);
        assert_eq!(v.to_string(), "2916a481e178-r3");
        for bad in [
            "2916a481e178",
            "2916a481e178-r0",
            "2916a481e178-r01",
            "2916A481E178-r1",
            "2916a481e17-r1",
            "2916a481e178-r",
            "2916a481e178-r1x",
            "2916a481e178-r-1",
            "../../etc-r1",
            "2916a481e178-r99999999999",
        ] {
            assert!(ReleaseVersion::parse(bad).is_err(), "{bad} was accepted");
        }
    }

    #[test]
    fn canal_so_os_tres_nomes() {
        for c in Channel::ALL {
            assert_eq!(Channel::parse(c.as_str()), Some(c));
        }
        assert_eq!(Channel::parse("beta"), None);
        assert_eq!(Channel::parse("Stable"), None);
        assert_eq!(
            serde_json::from_str::<Channel>("\"stable\"").unwrap(),
            Channel::Stable
        );
    }
}
