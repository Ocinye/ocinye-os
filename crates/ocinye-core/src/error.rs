//! Core errors and their mapping to the API envelope.

use ocinye_contracts::{ErrorBody, ErrorCode};
use ocinye_domain::policy::{Decision, Denial};
use ocinye_domain::DomainError;
use thiserror::Error;

/// Result of a Core operation.
pub type CoreResult<T> = Result<T, CoreError>;

/// Anything that can go wrong inside the Core.
#[derive(Debug, Error)]
pub enum CoreError {
    /// A violated institutional invariant.
    #[error(transparent)]
    Domain(#[from] DomainError),

    /// Input failed validation before reaching the domain.
    #[error("{0}")]
    Validation(String),

    /// The resource does not exist, or must be indistinguishable from that.
    ///
    /// Denied reads are surfaced this way so that the existence of a resource
    /// is not disclosed to an unauthorised caller (ADR-0100).
    #[error("{0}")]
    NotFound(String),

    /// Authentication is required or the presented token is not valid.
    #[error("{0}")]
    Unauthenticated(String),

    /// The caller is authenticated but not permitted.
    #[error("{0}")]
    PermissionDenied(String),

    /// The request conflicts with the current state.
    #[error("{0}")]
    Conflict(String),

    /// An institutional invariant refused the operation.
    ///
    /// `reason` is a stable, machine-readable code that travels in the error
    /// envelope's `details.reason`, so a client maps the refusal without
    /// reading the prose; `code` keeps the HTTP class each call site already
    /// had. Narrow on purpose: only invariants a client must explain get one
    /// (`last_platform_admin`, `last_unit_manager`, `self_lockout`).
    #[error("{message}")]
    Invariant {
        /// The envelope code (and so the HTTP status).
        code: ErrorCode,
        /// Stable reason, never translated.
        reason: &'static str,
        /// The explanation for people.
        message: String,
    },

    /// The application the operation belongs to is not active in this
    /// Instance (ADR-0014). Its data is intact; an administrator can activate it.
    #[error("{0}")]
    ApplicationInactive(String),

    /// No Ocinye node provides the requested capability.
    ///
    /// A legitimate, expected state while the physical layer does not exist
    /// (ADR-0300). Never masked by an external provider.
    #[error("{0}")]
    CapabilityUnavailable(String),

    /// Object storage is not configured or not reachable.
    #[error("{0}")]
    StorageUnavailable(String),

    /// Too many attempts. Carries no detail about which signal tripped.
    #[error("{0}")]
    RateLimited(String),

    /// The deployment is misconfigured.
    ///
    /// Raised at startup, never mid-request: a Core that cannot hash passwords
    /// safely must refuse to start rather than run degraded (`CLAUDE.md` §55).
    #[error("configuration error: {0}")]
    Configuration(String),

    /// A database failure. Never surfaced to the caller in detail.
    #[error("database error")]
    Database(#[from] sqlx::Error),

    /// An unexpected internal failure.
    #[error("internal error")]
    Internal(String),
}

impl CoreError {
    /// Stable error code for the API envelope.
    #[must_use]
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::Domain(error) => error.code(),
            Self::Validation(_) => ErrorCode::ValidationError,
            Self::NotFound(_) => ErrorCode::NotFound,
            Self::Unauthenticated(_) => ErrorCode::AuthenticationRequired,
            Self::PermissionDenied(_) => ErrorCode::PermissionDenied,
            Self::Conflict(_) => ErrorCode::Conflict,
            Self::Invariant { code, .. } => *code,
            Self::ApplicationInactive(_) => ErrorCode::ApplicationInactive,
            Self::CapabilityUnavailable(_) => ErrorCode::CapabilityUnavailable,
            Self::StorageUnavailable(_) => ErrorCode::StorageUnavailable,
            Self::RateLimited(_) => ErrorCode::RateLimited,
            Self::Configuration(_) | Self::Database(_) | Self::Internal(_) => {
                ErrorCode::InternalError
            }
        }
    }

    /// Message safe to return to the caller.
    ///
    /// Database and internal failures are deliberately opaque: their detail
    /// belongs in the log, correlated by request id, not in the response.
    #[must_use]
    pub fn public_message(&self) -> String {
        match self {
            // A configuration message names environment variables. That is
            // operator-facing detail, and it stays in the log.
            Self::Database(_) | Self::Internal(_) | Self::Configuration(_) => {
                "An unexpected error occurred.".to_owned()
            }
            other => other.to_string(),
        }
    }

    /// Whether this error should be logged at error level.
    #[must_use]
    pub const fn is_unexpected(&self) -> bool {
        matches!(self, Self::Database(_) | Self::Internal(_))
    }

    /// Build the response envelope.
    #[must_use]
    pub fn to_body(&self, request_id: Option<String>, correlation_id: Option<String>) -> ErrorBody {
        let mut body = ErrorBody::new(self.code(), self.public_message());
        if let Self::Domain(DomainError::InvalidTransition { from, to }) = self {
            body = body
                .with_detail("from", serde_json::Value::from(*from))
                .with_detail("to", serde_json::Value::from(*to));
        }
        if let Self::Invariant { reason, .. } = self {
            body = body.with_detail("reason", serde_json::Value::from(*reason));
        }
        body.with_ids(request_id, correlation_id)
    }

    /// Translate an authorization denial into the error the caller should see.
    ///
    /// The denial reason is deliberately not included: it is recorded in the
    /// audit trail, where a reviewer can read it, rather than handed to the
    /// caller as a hint about what would have worked.
    #[must_use]
    pub fn from_denial(denial: Denial, _decision: &Decision) -> Self {
        match denial {
            Denial::NotFound => Self::NotFound("Resource not found.".to_owned()),
            Denial::Forbidden => Self::PermissionDenied(
                "You do not have permission to perform this action.".to_owned(),
            ),
        }
    }
}

/// The stable reasons carried by [`CoreError::Invariant`].
pub mod refusal {
    /// It would leave the institution without a platform administrator able to
    /// sign in.
    pub const LAST_PLATFORM_ADMIN: &str = "last_platform_admin";
    /// It would leave a unit without a live manager.
    pub const LAST_UNIT_MANAGER: &str = "last_unit_manager";
    /// The actor would bar their own account.
    pub const SELF_LOCKOUT: &str = "self_lockout";
    /// D010 · It would leave the Instance with no enabled Distribution.
    pub const LAST_ENABLED_DISTRIBUTION: &str = "last_enabled_distribution";
    /// D010 · It would leave no administrator able to enter any Distribution.
    pub const LAST_ADMINISTRATOR_ACCESS: &str = "last_administrator_access";
    /// D010 · The Distribution is not enabled on this Instance.
    pub const DISTRIBUTION_NOT_ENABLED: &str = "distribution_not_enabled";
    /// D010 · The member has no access to this Distribution.
    pub const DISTRIBUTION_NO_ACCESS: &str = "distribution_no_access";
    /// D010 · The host name is already an access endpoint.
    pub const ENDPOINT_CONFLICT: &str = "endpoint_conflict";
    /// D010 · The host name is not valid.
    pub const ENDPOINT_INVALID: &str = "endpoint_invalid";
    /// D010 · It is the canonical endpoint, or the last active one.
    pub const ENDPOINT_LAST_OR_CANONICAL: &str = "endpoint_last_or_canonical";
    /// D010 · The context is not one the member may use.
    pub const CONTEXT_UNAVAILABLE: &str = "context_unavailable";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_invariant_keeps_its_code_and_carries_a_stable_reason() {
        let error = CoreError::Invariant {
            code: ErrorCode::Conflict,
            reason: refusal::LAST_UNIT_MANAGER,
            message: "Nomeie outro gestor primeiro.".to_owned(),
        };
        assert_eq!(error.code(), ErrorCode::Conflict);
        let body = error.to_body(None, None);
        assert_eq!(
            body.details.get("reason"),
            Some(&serde_json::Value::from("last_unit_manager"))
        );
        assert_eq!(error.public_message(), "Nomeie outro gestor primeiro.");
    }

    #[test]
    fn database_failures_do_not_leak_detail() {
        let error = CoreError::Database(sqlx::Error::RowNotFound);
        assert_eq!(error.public_message(), "An unexpected error occurred.");
        assert_eq!(error.code(), ErrorCode::InternalError);
    }

    #[test]
    fn a_denied_read_is_indistinguishable_from_absence() {
        let decision = Decision {
            allowed: false,
            reason: "no membership",
        };
        let error = CoreError::from_denial(Denial::NotFound, &decision);
        assert_eq!(error.code(), ErrorCode::NotFound);
        assert!(!error.public_message().contains("membership"));
    }

    #[test]
    fn denial_reasons_never_reach_the_caller() {
        let decision = Decision {
            allowed: false,
            reason: "workspace viewers are read-only",
        };
        for denial in [Denial::NotFound, Denial::Forbidden] {
            let message = CoreError::from_denial(denial, &decision).public_message();
            assert!(
                !message.contains("viewers"),
                "denial reason leaked: {message}"
            );
        }
    }
}
