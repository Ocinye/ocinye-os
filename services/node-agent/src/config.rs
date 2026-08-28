//! Node agent configuration.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};

use crate::DEFAULT_HEARTBEAT_INTERVAL;

/// Configuration read from the environment.
#[derive(Debug, Clone)]
pub struct AgentConfig {
    /// Base URL of the Ocinye Core.
    pub core_url: String,
    /// Single-use enrollment token, needed only on first start.
    pub enrollment_token: Option<String>,
    /// Where the long-lived agent credential is kept.
    pub token_path: PathBuf,
    /// Interval between heartbeats.
    pub heartbeat_interval: Duration,
    /// Log level.
    pub log_level: String,
}

impl AgentConfig {
    /// Read configuration from the environment.
    ///
    /// # Errors
    ///
    /// Returns an error when the Core URL is not set or is not HTTPS outside
    /// local development.
    pub fn from_env() -> Result<Self> {
        let core_url = env::var("OCINYE_CORE_URL")
            .context("OCINYE_CORE_URL is required")?
            .trim_end_matches('/')
            .to_owned();

        // A node credential must not travel in clear text. Loopback is allowed
        // so the agent can be exercised locally without a certificate.
        let is_local =
            core_url.starts_with("http://localhost") || core_url.starts_with("http://127.0.0.1");
        if !core_url.starts_with("https://") && !is_local {
            anyhow::bail!("OCINYE_CORE_URL must use https outside local development");
        }

        Ok(Self {
            core_url,
            enrollment_token: env::var("OCINYE_NODE_ENROLLMENT_TOKEN")
                .ok()
                .filter(|token| !token.trim().is_empty()),
            token_path: env::var("OCINYE_NODE_TOKEN_PATH")
                .unwrap_or_else(|_| "/var/lib/ocinye/agent.token".to_owned())
                .into(),
            heartbeat_interval: env::var("OCINYE_NODE_HEARTBEAT_SECONDS")
                .ok()
                .and_then(|value| value.parse().ok())
                .map_or(DEFAULT_HEARTBEAT_INTERVAL, Duration::from_secs),
            log_level: env::var("OCINYE_LOG_LEVEL").unwrap_or_else(|_| "info".to_owned()),
        })
    }

    /// Read the stored agent credential, if there is one.
    ///
    /// # Errors
    ///
    /// Returns an error when the file exists but cannot be read.
    pub fn load_agent_token(&self) -> Result<Option<String>> {
        if !self.token_path.exists() {
            return Ok(None);
        }
        let token = fs::read_to_string(&self.token_path)
            .with_context(|| format!("reading {}", self.token_path.display()))?;
        Ok(Some(token.trim().to_owned()).filter(|token| !token.is_empty()))
    }

    /// Persist the agent credential with owner-only permissions.
    ///
    /// # Why the mode is set when the file is created, not afterwards
    ///
    /// Writing first and tightening after leaves the credential on disk under
    /// the process umask — typically world-readable — for as long as the two
    /// calls take. On a node shared with any other local account, that window
    /// is enough: the file is small, its path is known, and the reader only has
    /// to be looking. A compute node is a machine the threat model already
    /// treats as potentially hostile (`CLAUDE.md` §30, §32), so the credential
    /// on it must never be exposed even briefly.
    ///
    /// `OpenOptions::mode` applies the permissions at creation, so there is no
    /// moment at which the file exists and is readable by anyone else.
    ///
    /// # Errors
    ///
    /// Returns an error when the file cannot be written.
    pub fn store_agent_token(&self, token: &str) -> Result<()> {
        if let Some(parent) = self.token_path.parent() {
            fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        }

        #[cfg(unix)]
        {
            use std::io::Write;
            use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

            let mut file = fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&self.token_path)
                .with_context(|| format!("writing {}", self.token_path.display()))?;

            file.write_all(token.as_bytes())
                .with_context(|| format!("writing {}", self.token_path.display()))?;

            // `mode` above applies only when the file is *created*. Re-enrolling
            // over a file that already exists — from an earlier version, or from
            // an operator's copy — keeps whatever mode it had, so it is set
            // again here. Belt and braces: the creation path never needs it.
            fs::set_permissions(&self.token_path, fs::Permissions::from_mode(0o600))
                .with_context(|| format!("securing {}", self.token_path.display()))?;
        }

        #[cfg(not(unix))]
        fs::write(&self.token_path, token)
            .with_context(|| format!("writing {}", self.token_path.display()))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_at(token_path: PathBuf) -> AgentConfig {
        AgentConfig {
            core_url: "http://localhost:8080".to_owned(),
            token_path,
            enrollment_token: None,
            heartbeat_interval: std::time::Duration::from_secs(30),
            log_level: "info".to_owned(),
        }
    }

    /// A credencial da máquina nunca existe legível por outra conta local.
    ///
    /// # Porque este teste existe
    ///
    /// A versão anterior escrevia o ficheiro e só depois lhe apertava as
    /// permissões. Entre as duas chamadas a credencial ficava em disco sob a
    /// umask do processo — tipicamente `0644`. Um nó de computação é uma
    /// máquina que o modelo de ameaças já trata como potencialmente hostil, e
    /// uma janela é uma janela.
    #[cfg(unix)]
    #[test]
    fn the_agent_credential_is_never_readable_by_anyone_else() {
        use std::os::unix::fs::PermissionsExt;

        let dir = std::env::temp_dir().join(format!("ocinye-node-{}", std::process::id()));
        let path = dir.join("agent.token");
        let config = config_at(path.clone());

        config
            .store_agent_token("um-token-de-teste")
            .expect("write");

        let mode = fs::metadata(&path).expect("metadata").permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "a credencial ficou com o modo {mode:o}");

        assert_eq!(
            config.load_agent_token().expect("read").as_deref(),
            Some("um-token-de-teste")
        );

        // Reenrolar por cima de um ficheiro já existente e demasiado aberto
        // volta a apertá-lo, em vez de herdar o modo anterior.
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).expect("loosen");
        config.store_agent_token("outro-token").expect("rewrite");
        let mode = fs::metadata(&path).expect("metadata").permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "reenrolar deixou o modo {mode:o}");

        fs::remove_dir_all(&dir).ok();
    }
}
