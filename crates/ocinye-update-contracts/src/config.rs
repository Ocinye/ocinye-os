//! The machine's update configuration (`/etc/ocinye/update.json`).
//!
//! The repository address is configuration, never compiled into a release: a
//! release built today must still update from the server that exists
//! tomorrow. The address is not an authority — trust comes from the signed
//! metadata — but it is still validated, so that it cannot name a local file
//! or carry credentials.

use crate::version::Channel;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Configuration schema.
pub const CONFIG_SCHEMA: u32 = 1;

/// Where the signed repository of the channel is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct RepositoryEndpoint {
    /// Base URL of the TUF metadata (`…/metadata/`).
    pub metadata_url: String,
    /// Base URL of the targets (`…/targets/`).
    pub targets_url: String,
}

impl RepositoryEndpoint {
    /// Both URLs valid.
    pub fn validate(&self) -> Result<(), &'static str> {
        validate_url(&self.metadata_url)?;
        validate_url(&self.targets_url)
    }
}

/// The configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct UpdateConfig {
    /// [`CONFIG_SCHEMA`].
    pub schema: u32,
    /// The channel followed.
    pub channel: Channel,
    /// The repository; `None` until an operator configures one.
    pub repository: Option<RepositoryEndpoint>,
    /// Check by itself, at most once a day.
    pub auto_check: bool,
    /// Download a release a check found. Never installs by itself.
    pub auto_download: bool,
}

impl UpdateConfig {
    /// The default of an image of this channel: no repository, check and
    /// download off until someone configures them.
    #[must_use]
    pub const fn default_for(channel: Channel) -> Self {
        Self {
            schema: CONFIG_SCHEMA,
            channel,
            repository: None,
            auto_check: false,
            auto_download: false,
        }
    }

    /// Parse and validate.
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let c: Self = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        c.validate().map_err(str::to_owned)?;
        Ok(c)
    }

    /// Every rule.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.schema != CONFIG_SCHEMA {
            return Err("unknown configuration schema");
        }
        if let Some(r) = &self.repository {
            r.validate()?;
        }
        if self.auto_download && !self.auto_check {
            return Err("automatic download needs automatic check");
        }
        Ok(())
    }
}

/// `http://` or `https://`, a host, a path ending in `/`; no credentials, no
/// query, no fragment, no whitespace, at most 512 characters.
pub fn validate_url(url: &str) -> Result<(), &'static str> {
    if url.len() > 512 || url.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err("a repository URL is at most 512 printable characters");
    }
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .ok_or("a repository URL is http:// or https://")?;
    let (authority, path) = rest.split_once('/').ok_or("a repository URL has a path")?;
    if authority.is_empty() || authority.contains('@') {
        return Err("a repository URL has a host and no credentials");
    }
    if !authority
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b':' | b'[' | b']'))
    {
        return Err("a repository host is a name or an address");
    }
    if url.contains('?') || url.contains('#') {
        return Err("a repository URL has no query or fragment");
    }
    if !url.ends_with('/') {
        return Err("a repository URL ends with /");
    }
    if path.split('/').any(|p| p == "..") {
        return Err("a repository URL does not go up");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enderecos_aceites_e_recusados() {
        for ok in [
            "https://updates.empresa.com/ocinye/stable/metadata/",
            "http://10.0.2.2:8088/development/targets/",
            "https://[2001:db8::1]/m/",
        ] {
            assert!(validate_url(ok).is_ok(), "{ok}");
        }
        for bad in [
            "file:///etc/ocinye/",
            "ftp://x.com/",
            "https://user:pw@x.com/m/",
            "https://x.com/m",
            "https://x.com/m/?a=b/",
            "https://x.com/../m/",
            "https:///m/",
            "https://x .com/m/",
        ] {
            assert!(validate_url(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn descarregar_sozinho_exige_verificar_sozinho() {
        let mut c = UpdateConfig::default_for(Channel::Stable);
        c.auto_download = true;
        assert!(c.validate().is_err());
        c.auto_check = true;
        assert!(c.validate().is_ok());
    }
}
