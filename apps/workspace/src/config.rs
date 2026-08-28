//! Workspace configuration.

use std::env;
use std::time::Duration;

use anyhow::{bail, Context, Result};

/// Configuration read from the environment.
#[derive(Debug, Clone)]
pub struct WorkspaceConfig {
    /// Address to bind.
    pub bind_address: String,
    /// Public base URL of the Workspace.
    pub public_url: String,
    /// Base URL of the Ocinye Core.
    pub core_url: String,
    /// How long a session lives.
    pub session_ttl: Duration,
    /// Whether the session cookie carries the `Secure` attribute.
    pub cookie_secure: bool,
    /// Log level.
    pub log_level: String,
    /// Log format.
    pub log_format: String,
    /// Whether this is a production deployment.
    pub is_production: bool,
    /// Onde estão os ficheiros estáticos.
    ///
    /// # Porque isto é configuração e não uma constante
    ///
    /// Era `"apps/workspace/static"`, relativo ao directório de trabalho: o
    /// servidor só funcionava se lançado da raiz do repositório, e falhava em
    /// silêncio noutro sítio — o HTML chegava e o JS não, o que faz uma
    /// interface parecer partida sem dizer porquê.
    pub static_dir: String,
}

fn var(key: &str) -> Option<String> {
    env::var(key)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

impl WorkspaceConfig {
    /// Read configuration from the environment.
    ///
    /// # Errors
    ///
    /// Returns an error when a required value is missing, or when a production
    /// deployment is configured in a way that would weaken session security.
    pub fn from_env() -> Result<Self> {
        let is_production =
            var("OCINYE_ENVIRONMENT").is_some_and(|value| value.eq_ignore_ascii_case("production"));

        let public_url = var("OCINYE_WORKSPACE_PUBLIC_URL")
            .context("OCINYE_WORKSPACE_PUBLIC_URL is required")?
            .trim_end_matches('/')
            .to_owned();

        let config = Self {
            bind_address: var("OCINYE_WORKSPACE_BIND_ADDRESS")
                .unwrap_or_else(|| "0.0.0.0:8090".to_owned()),
            core_url: var("OCINYE_CORE_URL")
                .context("OCINYE_CORE_URL is required")?
                .trim_end_matches('/')
                .to_owned(),
            session_ttl: Duration::from_secs(
                var("OCINYE_WORKSPACE_SESSION_TTL_SECONDS")
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(8 * 3600),
            ),
            // Secure cookies are required in production and default on
            // elsewhere; only an explicit opt-out for plain-HTTP local work
            // turns them off.
            cookie_secure: is_production
                || !var("OCINYE_WORKSPACE_COOKIE_INSECURE")
                    .is_some_and(|value| value == "1" || value.eq_ignore_ascii_case("true")),
            log_level: var("OCINYE_LOG_LEVEL").unwrap_or_else(|| "info".to_owned()),
            log_format: var("OCINYE_LOG_FORMAT").unwrap_or_else(|| {
                if is_production {
                    "json".into()
                } else {
                    "pretty".into()
                }
            }),
            public_url,
            is_production,
            static_dir: var("OCINYE_WORKSPACE_STATIC_DIR")
                .unwrap_or_else(|| "apps/workspace/static".to_owned()),
        };

        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<()> {
        if !self.is_production {
            return Ok(());
        }

        if !self.public_url.starts_with("https://") {
            bail!("OCINYE_WORKSPACE_PUBLIC_URL must use https in production");
        }
        if !self.cookie_secure {
            bail!("session cookies must be Secure in production");
        }
        // A ligação ao Core transporta credenciais no arranque de sessão. Em
        // produção tem de ser TLS: sob o ADR-0103 este é o troço por onde uma
        // palavra-passe passa (briefing §99).
        if !self.core_url.starts_with("https://") {
            bail!("OCINYE_CORE_URL must use https in production");
        }
        Ok(())
    }

    // Não há aqui um `redirect_uri`, e a rota que ele nomeava também não existe.
    //
    // Devolvia `{public_url}/auth/callback`, e não há `/auth/callback` no
    // catálogo do Workspace: a entrada acontece pelo formulário, contra o Core.
    // Uma configuração que descreve um caminho inexistente é pior do que
    // nenhuma — parece que o fluxo existe.
}
