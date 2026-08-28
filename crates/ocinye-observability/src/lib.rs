//! Structured logging and request correlation.
//!
//! Every runtime of the Ocinye OS — Core, Worker, Node Agent, Workspace —
//! initialises logging through this crate, so a request can be followed across
//! process boundaries by a single correlation identifier.
//!
//! # What is never logged
//!
//! Passwords, tokens, cookies, whole documents, dataset contents and prompts
//! (`CLAUDE.md` §62). [`redact`] exists as a backstop for values that reach a
//! log site despite that rule; it is not a licence to log sensitive things.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod correlation;

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub use correlation::{CorrelationIds, CORRELATION_ID_HEADER, REQUEST_ID_HEADER};

/// Marker substituted for a redacted value.
pub const REDACTED: &str = "[redacted]";

/// Field names whose values must never appear in a log line.
pub const SENSITIVE_FIELDS: &[&str] = &[
    "password",
    "passwd",
    "secret",
    "token",
    "access_token",
    "refresh_token",
    "id_token",
    "authorization",
    "cookie",
    "set-cookie",
    "api_key",
    "private_key",
    "prompt",
    "completion",
    "content",
    "body",
    "dsn",
    "database_url",
];

/// Whether a field name is known to carry sensitive material.
#[must_use]
pub fn is_sensitive_field(name: &str) -> bool {
    let lowered = name.to_ascii_lowercase();
    SENSITIVE_FIELDS
        .iter()
        .any(|candidate| lowered == *candidate)
}

/// Replace a value with [`REDACTED`] when its field name is sensitive.
#[must_use]
pub fn redact<'a>(field: &str, value: &'a str) -> &'a str {
    if is_sensitive_field(field) {
        REDACTED
    } else {
        value
    }
}

/// How log records are rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    /// One JSON object per line. The default outside development.
    Json,
    /// Human-readable. Development only.
    Pretty,
}

impl LogFormat {
    /// Parse from configuration, defaulting to [`LogFormat::Json`].
    #[must_use]
    pub fn parse(value: &str) -> Self {
        match value.to_ascii_lowercase().as_str() {
            "pretty" | "text" | "console" => Self::Pretty,
            _ => Self::Json,
        }
    }
}

/// Initialise the global subscriber.
///
/// Safe to call more than once: a second call is ignored rather than panicking,
/// which matters because tests may initialise concurrently.
pub fn init(service_name: &str, level: &str, format: LogFormat) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(format!("{level},sqlx::query=warn,hyper=warn")));

    let registry = tracing_subscriber::registry().with(filter);

    let result = match format {
        LogFormat::Json => registry
            .with(
                tracing_subscriber::fmt::layer()
                    .json()
                    .flatten_event(true)
                    .with_current_span(true)
                    .with_span_list(false),
            )
            .try_init(),
        LogFormat::Pretty => registry
            .with(tracing_subscriber::fmt::layer().with_target(true).compact())
            .try_init(),
    };

    if result.is_ok() {
        tracing::info!(service = service_name, "logging initialised");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sensitive_fields_are_redacted() {
        assert_eq!(redact("password", "hunter2"), REDACTED);
        assert_eq!(redact("Authorization", "Bearer abc"), REDACTED);
        assert_eq!(redact("unit_code", "AI"), "AI");
    }

    #[test]
    fn every_token_shaped_field_is_covered() {
        for field in [
            "access_token",
            "refresh_token",
            "id_token",
            "api_key",
            "private_key",
        ] {
            assert!(
                is_sensitive_field(field),
                "{field} must be treated as sensitive"
            );
        }
    }

    #[test]
    fn json_is_the_default_format() {
        assert_eq!(LogFormat::parse("anything-else"), LogFormat::Json);
        assert_eq!(LogFormat::parse("pretty"), LogFormat::Pretty);
    }
}
