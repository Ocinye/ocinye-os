//! Configuration of the Ocinye Core.
//!
//! Everything is environment-driven. Nothing is hardcoded: no secrets, no
//! production URLs, no physical node names (`CLAUDE.md` §55). A required value
//! that is missing causes a startup failure rather than a silent default.

use std::collections::BTreeMap;
use std::env;
use std::time::Duration;

use ocinye_contracts::{AiCapability, Residency};

use crate::error::{CoreError, CoreResult};

/// Deployment environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    /// Local development.
    Development,
    /// Pre-production.
    Staging,
    /// Production.
    Production,
}

impl Environment {
    fn parse(value: &str) -> Self {
        match value.to_ascii_lowercase().as_str() {
            "production" | "prod" => Self::Production,
            "staging" => Self::Staging,
            _ => Self::Development,
        }
    }

    /// Whether this is production.
    #[must_use]
    pub const fn is_production(self) -> bool {
        matches!(self, Self::Production)
    }

    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Development => "development",
            Self::Staging => "staging",
            Self::Production => "production",
        }
    }
}

/// Identity provider settings.
#[derive(Debug, Clone)]
pub struct OidcConfig {
    /// Issuer URL. Discovery and JWKS are derived from it.
    pub issuer: String,
    /// Audience the Core requires in a token.
    pub audience: String,
    /// How long a fetched JWKS is reused.
    pub jwks_cache: Duration,
}

/// Object storage settings.
#[derive(Debug, Clone)]
pub struct StorageConfig {
    /// S3-compatible endpoint.
    pub endpoint_url: String,
    /// Region label the endpoint expects.
    pub region: String,
    /// Access key.
    pub access_key: String,
    /// Secret key.
    pub secret_key: String,
    /// Bucket holding institutional artefacts.
    pub bucket: String,
    /// Code identifying this backend in the registry.
    pub backend_code: String,
    /// Human label of the physical location.
    pub location_label: String,
    /// Declared physical residency. Defaults to `UNDECLARED` (ADR-0201).
    pub residency: Residency,
    /// Largest accepted upload.
    pub max_upload_bytes: u64,
}

impl StorageConfig {
    /// Whether enough is configured to attempt a connection.
    #[must_use]
    pub fn is_configured(&self) -> bool {
        !self.endpoint_url.is_empty() && !self.access_key.is_empty()
    }
}

/// AI Gateway settings.
#[derive(Debug, Clone)]
pub struct AiConfig {
    /// SystemCapability to model-name mapping. **Configuration, never code**
    /// (ADR-0300). Empty by default: with no node enrolled, AI is unavailable.
    pub capability_map: BTreeMap<AiCapability, String>,
    /// Whether an explicitly registered external provider may be selected.
    /// Off by default; turning it on is an institutional decision.
    pub allow_external_providers: bool,
    /// Qual provider de embeddings esta instalação usa.
    ///
    /// `none` por omissão: sem embeddings, a pesquisa semântica é declarada
    /// indisponível e a lexical continua inteira. **Isso não é degradação** —
    /// é a instalação que a Ocinye tem hoje.
    ///
    /// `deterministic` liga o provider de prova, que não é um modelo e diz que
    /// não é: a identidade que ele grava chama-se `not-a-model`, para que
    /// ninguém confunda um registo de proveniência de teste com um real.
    /// Existe para que a CI possa exercer o caminho inteiro — Core, contrato,
    /// pgvector, recuperação — sem depender de um serviço externo.
    pub embedding_provider: String,
}

/// Compute Plane settings.
#[derive(Debug, Clone)]
pub struct ComputeConfig {
    /// Lifetime of a single-use enrollment token.
    pub enrollment_token_ttl: Duration,
    /// Silence after which a node is considered offline.
    pub node_offline_after: Duration,
}

/// Authentication configuration.
///
/// Under ADR-0103 the Core is the authentication authority, so the cost of
/// hashing and the shape of throttling are deployment decisions, not constants
/// (briefing §31, §37).
#[derive(Debug, Clone)]
pub struct AuthConfig {
    /// Argon2id memory cost, in kibibytes.
    pub argon2_memory_kib: u32,
    /// Argon2id passes.
    pub argon2_iterations: u32,
    /// Argon2id lanes.
    pub argon2_parallelism: u32,
    /// How long a temporary credential remains valid, in hours.
    pub temporary_credential_hours: i64,
    /// Failed attempts from one origin before refusal.
    pub throttle_per_ip: i64,
    /// Failed attempts against one account before refusal.
    pub throttle_per_email: i64,
    /// Window over which failures are counted, in minutes.
    pub throttle_window_minutes: i64,
}

/// How a mail connection is protected.
///
/// # Why this is not a boolean
///
/// `TLS=true` is ambiguous between the two ways mail is actually encrypted:
/// wrapping the socket from the first byte (port 993/465) and upgrading a
/// plain socket with `STARTTLS` (port 143/587). Getting it wrong does not fail
/// gracefully — it hangs, or worse, it connects and the credential crosses the
/// wire in the clear.
///
/// # There is no third variant
///
/// Deliberately. `false`, `none` and `off` are **rejected at startup** rather
/// than quietly accepted, because an unencrypted mail connection sends the
/// mailbox password as plaintext (`CLAUDE.md` §38).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MailSecurity {
    /// TLS from the first byte. IMAP 993, SMTP 465.
    ImplicitTls,
    /// Plain socket upgraded with `STARTTLS`. IMAP 143, SMTP 587.
    StartTls,
}

impl MailSecurity {
    /// Stable representation, for the administration screen.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ImplicitTls => "tls",
            Self::StartTls => "starttls",
        }
    }

    /// Parse a configured value.
    ///
    /// Returns `Err` with a readable reason for anything that would mean *no
    /// encryption*, so the Core refuses to start rather than sending the
    /// mailbox password in the clear.
    ///
    /// # Errors
    ///
    /// Returns the offending value when it is unrecognised or asks for
    /// plaintext.
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "true" | "tls" | "ssl" | "ssl/tls" | "implicit" | "implicit_tls" => {
                Ok(Self::ImplicitTls)
            }
            "starttls" | "start_tls" => Ok(Self::StartTls),
            "false" | "none" | "off" | "plain" | "" => Err(format!(
                "«{value}» desligaria a cifra: a password da caixa atravessaria a rede \
                 em claro. Use `tls` (porto 993/465) ou `starttls` (porto 143/587)."
            )),
            other => Err(format!(
                "«{other}» não é uma opção de segurança de correio. Use `tls` ou `starttls`."
            )),
        }
    }

    /// The convention for a port, when nothing was configured.
    ///
    /// A default is only a convenience: the explicit setting always wins, and
    /// both defaults encrypt.
    #[must_use]
    pub const fn for_port(port: u16) -> Self {
        match port {
            143 | 587 => Self::StartTls,
            _ => Self::ImplicitTls,
        }
    }
}

/// Ocinye Mail settings.
///
/// # No credential has a default
///
/// Every other block in this file has sensible fallbacks. This one does not:
/// an absent credential means mail is **not configured**, which is a true and
/// perfectly valid state, and inventing a value would replace it with a
/// broken one (briefing §58, `CLAUDE.md` §55).
#[derive(Clone)]
pub struct MailConfig {
    /// Domains that count as inside the institution.
    ///
    /// Everything else is external, and the send policy treats it as such. An
    /// empty list makes **every** recipient external, which fails closed
    /// (briefing §36).
    pub institutional_domains: Vec<String>,
    /// IMAP host. Empty when mail is not configured.
    pub imap_host: String,
    /// IMAP port. 993 is implicit TLS, 143 is STARTTLS.
    pub imap_port: u16,
    /// How the IMAP connection is protected.
    pub imap_security: MailSecurity,
    /// SMTP host. Empty when mail is not configured.
    pub smtp_host: String,
    /// SMTP port. 465 is implicit TLS, 587 is STARTTLS.
    pub smtp_port: u16,
    /// How the SMTP connection is protected.
    pub smtp_security: MailSecurity,
    /// The account the adapter authenticates as.
    pub username: String,
    /// Its password or application token.
    ///
    /// Read from the environment at startup and never written anywhere: not to
    /// the database, not to a log, not to `.env.example` (briefing §57, §58).
    pub password: String,
    /// Largest message the composer accepts, attachments included.
    pub max_message_bytes: u64,
    /// A chave que cifra as credenciais de cada caixa, ausente quando esta
    /// instalação não a configurou.
    ///
    /// # Porque é `Option` e não um valor gerado
    ///
    /// Uma chave gerada ao arranque abriria as credenciais desta execução e de
    /// mais nenhuma: ao reiniciar, todas as caixas ligadas ficariam ilegíveis
    /// sem que ninguém tivesse pedido nada. Ausente, ligar uma caixa recusa com
    /// a razão dita — que é o comportamento correcto de uma instalação sem
    /// chave, e não um sítio por preencher (ADR-0409).
    pub sealing_key: Option<crate::password::sealed::SealingKey>,
}

impl std::fmt::Debug for MailConfig {
    /// `CoreConfig` derives `Debug` and is logged at startup. Without this the
    /// mail password would be in the first line of every boot log.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MailConfig")
            .field("institutional_domains", &self.institutional_domains)
            .field("imap_host", &self.imap_host)
            .field("imap_port", &self.imap_port)
            .field("imap_security", &self.imap_security.as_str())
            .field("smtp_host", &self.smtp_host)
            .field("smtp_port", &self.smtp_port)
            .field("smtp_security", &self.smtp_security.as_str())
            .field("username", &self.username)
            .field("password", &"<redacted>")
            .field("max_message_bytes", &self.max_message_bytes)
            .field("sealing_key", &self.sealing_key)
            .finish()
    }
}

impl MailConfig {
    /// Whether the installation knows **where** the mail service is.
    ///
    /// # Transporte e credencial são coisas separadas
    ///
    /// Esta distinção não existia: as quatro variáveis eram um bloco, e uma
    /// instalação com anfitriões mas sem conta de serviço era recusada como
    /// «meio configurada». Isso é anterior ao [ADR-0409], que trouxe a
    /// credencial de cada membro — e desde então há uma instalação
    /// perfeitamente coerente que o modelo antigo não sabia exprimir:
    ///
    /// > o Ocinye OS sabe onde é o servidor, e **cada pessoa entra com a sua
    /// > própria senha**.
    ///
    /// Nessa instalação não existe conta de serviço institucional, e não é uma
    /// falta: é uma decisão. Ninguém guarda uma senha que abre a caixa de toda
    /// a gente.
    ///
    /// [ADR-0409]: https://github.com/Ocinye/ocinye-os/blob/main/docs/adrs/0409-mailbox-credentials-per-member.md
    #[must_use]
    pub fn transport_configured(&self) -> bool {
        !self.imap_host.is_empty() && !self.smtp_host.is_empty()
    }

    /// Whether an institutional service account exists.
    ///
    /// A conta com que o `worker` indexa e com que o trabalho agentic age. Não
    /// pertence a ninguém, e por isso não pode representar ninguém — é por
    /// isso que a sua ausência não impede um membro de ler o seu correio.
    #[must_use]
    pub fn has_institutional_credential(&self) -> bool {
        !self.username.is_empty() && !self.password.is_empty()
    }

    /// Whether enough is configured to build a real institutional adapter.
    #[must_use]
    pub fn is_configured(&self) -> bool {
        self.transport_configured() && self.has_institutional_credential()
    }

    /// Whether some parts are set and others are not.
    ///
    /// Duas metades independentes, e cada uma tem de estar inteira:
    ///
    /// - **transporte** — os dois anfitriões, ou nenhum. Um sem o outro deixa
    ///   metade do correio a apontar para lado nenhum.
    /// - **credencial** — utilizador e senha, ou nenhum. Um sem o outro nunca
    ///   autentica.
    ///
    /// E uma credencial sem transporte é uma credencial para lado nenhum.
    #[must_use]
    pub fn is_partially_configured(&self) -> bool {
        let transporte_meio = !self.imap_host.is_empty() != !self.smtp_host.is_empty();
        let credencial_meio = !self.username.is_empty() != !self.password.is_empty();
        let credencial_sem_destino =
            self.has_institutional_credential() && !self.transport_configured();

        transporte_meio || credencial_meio || credencial_sem_destino
    }
}

/// Complete Core configuration.
#[derive(Debug, Clone)]
pub struct CoreConfig {
    /// Deployment environment.
    pub environment: Environment,
    /// Address the HTTP server binds to.
    pub bind_address: String,
    /// Log level.
    pub log_level: String,
    /// Log format.
    pub log_format: String,
    /// Slug of the organisation this deployment serves.
    pub organisation_slug: String,
    /// PostgreSQL connection string.
    pub database_url: String,
    /// Maximum database connections.
    pub database_max_connections: u32,
    /// Redis connection string.
    pub redis_url: String,
    /// Identity provider. Vestigial under ADR-0103; retained for future
    /// federation.
    pub oidc: OidcConfig,
    /// Authentication and password handling.
    pub auth: AuthConfig,
    /// Object storage.
    pub storage: StorageConfig,
    /// AI Gateway.
    pub ai: AiConfig,
    /// Compute Plane.
    pub compute: ComputeConfig,
    /// Ocinye Mail.
    pub mail: MailConfig,
    /// Origins permitted to call the API from a browser.
    pub cors_allowed_origins: Vec<String>,
    /// Onde estão os componentes do Capability Runtime.
    ///
    /// # Porque é configuração e não um caminho escrito no código
    ///
    /// Porque o directório muda com a instalação: em desenvolvimento os
    /// componentes saem para o directório de build partilhado do repositório, e
    /// num servidor virão de onde quem instala os puser.
    ///
    /// O que **não** muda, e não é configurável, é qual o componente que cada
    /// operação usa. Isso é decisão do Core: um cliente pede uma operação de
    /// domínio, e nunca escolhe o que se executa.
    pub capability_components_dir: String,
}

fn optional(key: &str) -> Option<String> {
    env::var(key)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn or_default(key: &str, fallback: &str) -> String {
    optional(key).unwrap_or_else(|| fallback.to_owned())
}

fn required(key: &str) -> CoreResult<String> {
    optional(key)
        .ok_or_else(|| CoreError::Internal(format!("required configuration `{key}` is not set")))
}

fn parse_number<T: std::str::FromStr>(key: &str, fallback: T) -> T {
    optional(key)
        .and_then(|value| value.parse().ok())
        .unwrap_or(fallback)
}

/// Resolve a mail transport security setting.
///
/// Absent means *the convention for this port*, which always encrypts. A value
/// that would mean plaintext stops the Core from starting, with the reason —
/// silently downgrading to an unencrypted mail connection would put the mailbox
/// password on the wire in the clear.
fn mail_security(key: &str, port: u16) -> CoreResult<MailSecurity> {
    optional(key).map_or_else(
        || Ok(MailSecurity::for_port(port)),
        |value| {
            MailSecurity::parse(&value)
                .map_err(|reason| CoreError::Configuration(format!("{key}: {reason}")))
        },
    )
}

fn parse_flag(key: &str, fallback: bool) -> bool {
    optional(key).map_or(fallback, |value| {
        matches!(
            value.to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

/// Parse the capability map from `GENERAL=model,CODING=other`.
///
/// Unknown capability names are ignored rather than fatal: a future capability
/// appearing in configuration must not stop the Core from starting.
#[must_use]
pub fn parse_capability_map(raw: &str) -> BTreeMap<AiCapability, String> {
    raw.split(',')
        .filter_map(|entry| entry.split_once('='))
        .filter_map(|(key, value)| {
            let capability = AiCapability::parse(key.trim().to_ascii_uppercase().as_str())?;
            let model = value.trim();
            (!model.is_empty()).then(|| (capability, model.to_owned()))
        })
        .collect()
}

impl CoreConfig {
    /// Load configuration from the environment.
    ///
    /// # Errors
    ///
    /// Returns an error when a required value is missing, or when a production
    /// deployment is misconfigured in a way that would weaken security.
    pub fn from_env() -> CoreResult<Self> {
        let environment = Environment::parse(&or_default("OCINYE_ENVIRONMENT", "development"));

        // Read once: the security setting defaults from the port, so the port
        // has to be known before it is resolved.
        let imap_port: u16 = parse_number("OCINYE_MAIL_IMAP_PORT", 993);
        let smtp_port: u16 = parse_number("OCINYE_MAIL_SMTP_PORT", 587);

        let config = Self {
            environment,
            bind_address: or_default("OCINYE_BIND_ADDRESS", "0.0.0.0:8080"),
            log_level: or_default("OCINYE_LOG_LEVEL", "info"),
            log_format: or_default(
                "OCINYE_LOG_FORMAT",
                if environment.is_production() {
                    "json"
                } else {
                    "pretty"
                },
            ),
            organisation_slug: or_default("OCINYE_ORGANISATION_SLUG", "ocinye"),
            database_url: required("OCINYE_DATABASE_URL")?,
            database_max_connections: parse_number("OCINYE_DATABASE_MAX_CONNECTIONS", 10),
            redis_url: or_default("OCINYE_REDIS_URL", "redis://localhost:6379"),
            oidc: OidcConfig {
                issuer: or_default("OCINYE_OIDC_ISSUER", ""),
                audience: or_default("OCINYE_OIDC_AUDIENCE", "ocinye-core"),
                jwks_cache: Duration::from_secs(parse_number(
                    "OCINYE_OIDC_JWKS_CACHE_SECONDS",
                    300,
                )),
            },
            auth: AuthConfig {
                argon2_memory_kib: parse_number("OCINYE_ARGON2_MEMORY_KIB", 19 * 1024),
                argon2_iterations: parse_number("OCINYE_ARGON2_ITERATIONS", 2),
                argon2_parallelism: parse_number("OCINYE_ARGON2_PARALLELISM", 1),
                temporary_credential_hours: parse_number("OCINYE_TEMPORARY_CREDENTIAL_HOURS", 24),
                throttle_per_ip: parse_number("OCINYE_THROTTLE_PER_IP", 20),
                throttle_per_email: parse_number("OCINYE_THROTTLE_PER_EMAIL", 10),
                throttle_window_minutes: parse_number("OCINYE_THROTTLE_WINDOW_MINUTES", 15),
            },
            storage: StorageConfig {
                endpoint_url: or_default("OCINYE_STORAGE_ENDPOINT_URL", ""),
                region: or_default("OCINYE_STORAGE_REGION", "us-east-1"),
                access_key: or_default("OCINYE_STORAGE_ACCESS_KEY", ""),
                secret_key: or_default("OCINYE_STORAGE_SECRET_KEY", ""),
                bucket: or_default("OCINYE_STORAGE_BUCKET", "ocinye-artifacts"),
                backend_code: or_default("OCINYE_STORAGE_BACKEND_CODE", "local-minio"),
                location_label: or_default("OCINYE_STORAGE_LOCATION_LABEL", "local-development"),
                residency: Residency::parse(&or_default("OCINYE_STORAGE_RESIDENCY", "UNDECLARED"))
                    .unwrap_or_default(),
                max_upload_bytes: parse_number(
                    "OCINYE_STORAGE_MAX_UPLOAD_BYTES",
                    512 * 1024 * 1024,
                ),
            },
            ai: AiConfig {
                capability_map: parse_capability_map(&or_default("OCINYE_AI_CAPABILITY_MAP", "")),
                allow_external_providers: parse_flag("OCINYE_AI_ALLOW_EXTERNAL_PROVIDERS", false),
                embedding_provider: or_default("OCINYE_AI_EMBEDDING_PROVIDER", "none"),
            },
            compute: ComputeConfig {
                enrollment_token_ttl: Duration::from_secs(parse_number(
                    "OCINYE_COMPUTE_ENROLLMENT_TOKEN_TTL_SECONDS",
                    3600,
                )),
                node_offline_after: Duration::from_secs(parse_number(
                    "OCINYE_COMPUTE_NODE_OFFLINE_AFTER_SECONDS",
                    120,
                )),
            },
            mail: MailConfig {
                institutional_domains: or_default("OCINYE_MAIL_INSTITUTIONAL_DOMAINS", "")
                    .split(',')
                    .map(|domain| domain.trim().to_ascii_lowercase())
                    .filter(|domain| !domain.is_empty())
                    .collect(),
                imap_host: or_default("OCINYE_MAIL_IMAP_HOST", ""),
                imap_port,
                imap_security: mail_security("OCINYE_MAIL_IMAP_TLS", imap_port)?,
                smtp_host: or_default("OCINYE_MAIL_SMTP_HOST", ""),
                smtp_port,
                smtp_security: mail_security("OCINYE_MAIL_SMTP_TLS", smtp_port)?,
                username: or_default("OCINYE_MAIL_USERNAME", ""),
                password: or_default("OCINYE_MAIL_PASSWORD", ""),
                max_message_bytes: parse_number("OCINYE_MAIL_MAX_MESSAGE_BYTES", 25 * 1024 * 1024),
                sealing_key: match optional("OCINYE_MAIL_KEY") {
                    None => None,
                    Some(valor) => Some(crate::password::sealed::SealingKey::from_base64(&valor)?),
                },
            },
            cors_allowed_origins: or_default("OCINYE_CORS_ALLOWED_ORIGINS", "")
                .split(',')
                .map(str::trim)
                .filter(|origin| !origin.is_empty())
                .map(ToOwned::to_owned)
                .collect(),
            capability_components_dir: or_default(
                "OCINYE_CAPABILITY_COMPONENTS_DIR",
                "target/wasm32-wasip1/release",
            ),
        };

        config.validate()?;
        Ok(config)
    }

    /// Reject configurations that would weaken security.
    ///
    /// The hashing parameters are checked in **every** environment: a weak
    /// verifier written in development is a weak verifier that survives into
    /// the first real deployment. Only the transport-level rules are
    /// production-only.
    fn validate(&self) -> CoreResult<()> {
        crate::password::HashingParams {
            memory_kib: self.auth.argon2_memory_kib,
            iterations: self.auth.argon2_iterations,
            parallelism: self.auth.argon2_parallelism,
        }
        .validate()?;

        if self.auth.temporary_credential_hours <= 0 {
            return Err(CoreError::Configuration(
                "OCINYE_TEMPORARY_CREDENTIAL_HOURS must be positive: a credential that never \
                 expires is a permanent password"
                    .to_owned(),
            ));
        }
        if self.auth.throttle_per_email <= 0 || self.auth.throttle_per_ip <= 0 {
            return Err(CoreError::Configuration(
                "throttling thresholds must be positive".to_owned(),
            ));
        }

        // Half-configured mail is worse than no mail: it looks available and
        // fails at the moment somebody presses Enviar.
        if self.mail.is_partially_configured() {
            return Err(CoreError::Configuration(
                "Ocinye Mail is partially configured. Transport and credential are \
                 separate: OCINYE_MAIL_IMAP_HOST and OCINYE_MAIL_SMTP_HOST are both \
                 set or both absent, and OCINYE_MAIL_USERNAME and \
                 OCINYE_MAIL_PASSWORD are both set or both absent. Transport \
                 without an institutional credential is a valid installation: \
                 every member connects their own mailbox (ADR-0409)."
                    .to_owned(),
            ));
        }
        if self.mail.transport_configured() && self.mail.institutional_domains.is_empty() {
            return Err(CoreError::Configuration(
                "OCINYE_MAIL_INSTITUTIONAL_DOMAINS must be set when mail transport is configured: \
                 without it every recipient counts as external"
                    .to_owned(),
            ));
        }

        if !self.environment.is_production() {
            return Ok(());
        }

        // ADR-0103 moved authentication into the Core, so an issuer is no
        // longer required to start. If one *is* configured — for future
        // federation — it must still be https.
        if !self.oidc.issuer.is_empty() && !self.oidc.issuer.starts_with("https://") {
            return Err(CoreError::Configuration(
                "OCINYE_OIDC_ISSUER must use https in production".to_owned(),
            ));
        }
        if self.cors_allowed_origins.iter().any(|origin| origin == "*") {
            return Err(CoreError::Configuration(
                "a wildcard CORS origin is not permitted in production".to_owned(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_map_parses_configured_pairs() {
        let map = parse_capability_map("GENERAL=qwen2.5, CODING=qwen2.5-coder");
        assert_eq!(
            map.get(&AiCapability::General).map(String::as_str),
            Some("qwen2.5")
        );
        assert_eq!(
            map.get(&AiCapability::Coding).map(String::as_str),
            Some("qwen2.5-coder")
        );
        assert!(!map.contains_key(&AiCapability::Reasoning));
    }

    #[test]
    fn an_empty_capability_map_is_the_default() {
        // No node enrolled means no capability is served. That is the true
        // state, and it must be the state the Core starts in.
        assert!(parse_capability_map("").is_empty());
        assert!(parse_capability_map("nonsense").is_empty());
        assert!(parse_capability_map("UNKNOWN=x").is_empty());
    }

    /// A complete, valid configuration to vary one field at a time.
    fn config_fixture(environment: Environment) -> CoreConfig {
        CoreConfig {
            environment,
            bind_address: "0.0.0.0:8080".into(),
            log_level: "info".into(),
            log_format: "json".into(),
            organisation_slug: "ocinye".into(),
            database_url: "postgres://x".into(),
            database_max_connections: 10,
            redis_url: "redis://x".into(),
            oidc: OidcConfig {
                issuer: "https://id.example.org/realms/ocinye".into(),
                audience: "ocinye-core".into(),
                jwks_cache: Duration::from_secs(300),
            },
            auth: AuthConfig {
                argon2_memory_kib: 19 * 1024,
                argon2_iterations: 2,
                argon2_parallelism: 1,
                temporary_credential_hours: 24,
                throttle_per_ip: 20,
                throttle_per_email: 10,
                throttle_window_minutes: 15,
            },
            storage: StorageConfig {
                endpoint_url: String::new(),
                region: "us-east-1".into(),
                access_key: String::new(),
                secret_key: String::new(),
                bucket: "b".into(),
                backend_code: "c".into(),
                location_label: "l".into(),
                residency: Residency::Undeclared,
                max_upload_bytes: 1024,
            },
            ai: AiConfig {
                capability_map: BTreeMap::new(),
                allow_external_providers: false,
                embedding_provider: "none".to_owned(),
            },
            compute: ComputeConfig {
                enrollment_token_ttl: Duration::from_secs(60),
                node_offline_after: Duration::from_secs(120),
            },
            mail: MailConfig {
                institutional_domains: vec!["ocinye.com".into()],
                imap_host: String::new(),
                imap_port: 993,
                imap_security: MailSecurity::ImplicitTls,
                smtp_host: String::new(),
                smtp_port: 587,
                smtp_security: MailSecurity::StartTls,
                username: String::new(),
                password: String::new(),
                max_message_bytes: 1024,
                sealing_key: None,
            },
            cors_allowed_origins: vec![],
            capability_components_dir: "target/wasm32-wasip1/release".to_owned(),
        }
    }

    #[test]
    fn production_rejects_plain_http_federation_and_wildcard_cors() {
        let base = config_fixture(Environment::Production);
        assert!(base.validate().is_ok());

        let mut plain_http = base.clone();
        plain_http.oidc.issuer = "http://id.example.org".into();
        assert!(plain_http.validate().is_err());

        // Under ADR-0103 the Core authenticates on its own, so an absent
        // issuer is the normal case and must not stop production starting.
        let mut no_issuer = base.clone();
        no_issuer.oidc.issuer = String::new();
        assert!(no_issuer.validate().is_ok());

        let mut wildcard = base.clone();
        wildcard.cors_allowed_origins = vec!["*".into()];
        assert!(wildcard.validate().is_err());
    }

    #[test]
    fn weak_hashing_parameters_stop_the_core_from_starting_in_any_environment() {
        let mut weak = config_fixture(Environment::Development);
        weak.auth.argon2_memory_kib = 1024;
        assert!(
            weak.validate().is_err(),
            "a Core that hashes weakly must refuse to start"
        );

        let mut no_expiry = config_fixture(Environment::Development);
        no_expiry.auth.temporary_credential_hours = 0;
        assert!(
            no_expiry.validate().is_err(),
            "a temporary credential that never expires is a permanent password"
        );

        let mut no_throttle = config_fixture(Environment::Development);
        no_throttle.auth.throttle_per_email = 0;
        assert!(no_throttle.validate().is_err());
    }

    #[test]
    fn external_ai_providers_are_off_unless_explicitly_enabled() {
        assert!(!parse_flag("OCINYE_TEST_UNSET_FLAG_XYZ", false));
    }

    #[test]
    fn half_configured_mail_stops_the_core_from_starting() {
        let mut half = config_fixture(Environment::Development);
        half.mail.imap_host = "imap.example.org".into();
        half.mail.smtp_host = "smtp.example.org".into();
        half.mail.username = "conta".into();
        // Sem password.
        assert!(
            half.validate().is_err(),
            "correio meio configurado parece disponível e falha ao enviar"
        );
    }

    #[test]
    fn configured_mail_without_institutional_domains_is_refused() {
        // Sem domínios, todos os destinatários são externos e a política de
        // classificação passa a barrar correio interno perfeitamente normal.
        let mut no_domains = config_fixture(Environment::Development);
        no_domains.mail.imap_host = "imap.example.org".into();
        no_domains.mail.smtp_host = "smtp.example.org".into();
        no_domains.mail.username = "conta".into();
        no_domains.mail.password = "token".into();
        no_domains.mail.institutional_domains = vec![];
        assert!(no_domains.validate().is_err());

        no_domains.mail.institutional_domains = vec!["ocinye.com".into()];
        assert!(no_domains.validate().is_ok());
    }

    #[test]
    fn mail_is_unconfigured_by_default_and_that_is_valid() {
        let base = config_fixture(Environment::Production);
        assert!(!base.mail.is_configured());
        assert!(!base.mail.is_partially_configured());
        assert!(base.validate().is_ok());
    }

    #[test]
    fn the_mail_password_never_appears_in_debug_output() {
        // `CoreConfig` deriva `Debug` e é registado no arranque.
        let mut config = config_fixture(Environment::Production);
        // O valor é deliberadamente um marcador reconhecível: o varrimento de
        // segredos de `scripts/verify.sh` percorre os `.rs`, e uma cadeia com
        // aspecto de credencial num teste faria o sweep falhar por uma razão
        // que não é a que ele existe para detectar.
        const PLACEHOLDER: &str = "placeholder-que-nao-deve-aparecer";
        config.mail.password = PLACEHOLDER.into();

        let rendered = format!("{config:?}");
        assert!(!rendered.contains(PLACEHOLDER));
        assert!(rendered.contains("<redacted>"));
    }

    #[test]
    fn plaintext_mail_is_refused_rather_than_silently_accepted() {
        // A garantia que mais importa nesta configuração: nenhum valor
        // desliga a cifra. Um `false` mal copiado de outra documentação
        // colocaria a password da caixa na rede em claro.
        for value in ["false", "none", "off", "plain", ""] {
            assert!(
                MailSecurity::parse(value).is_err(),
                "«{value}» foi aceite e desligaria o TLS"
            );
        }

        for value in ["true", "TLS", "ssl", "SSL/TLS", "implicit"] {
            assert_eq!(
                MailSecurity::parse(value),
                Ok(MailSecurity::ImplicitTls),
                "«{value}» devia significar TLS implícito"
            );
        }

        assert_eq!(MailSecurity::parse("starttls"), Ok(MailSecurity::StartTls));
        assert!(MailSecurity::parse("disparate").is_err());
    }

    #[test]
    fn the_port_decides_the_convention_when_nothing_is_configured() {
        // LWS: IMAP 993 e SMTP 465, ambos TLS implícito.
        assert_eq!(MailSecurity::for_port(993), MailSecurity::ImplicitTls);
        assert_eq!(MailSecurity::for_port(465), MailSecurity::ImplicitTls);
        // Portos de submissão e IMAP em claro sobem por STARTTLS.
        assert_eq!(MailSecurity::for_port(587), MailSecurity::StartTls);
        assert_eq!(MailSecurity::for_port(143), MailSecurity::StartTls);
        // Um porto desconhecido assume o mais seguro dos dois.
        assert_eq!(MailSecurity::for_port(50_000), MailSecurity::ImplicitTls);
    }

    #[test]
    fn storage_residency_defaults_to_undeclared() {
        assert_eq!(
            Residency::parse("nonsense").unwrap_or_default(),
            Residency::Undeclared
        );
    }
}
