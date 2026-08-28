//! Domain errors.

use ocinye_contracts::ErrorCode;
use thiserror::Error;

/// Result of a domain operation.
pub(crate) type DomainResult<T> = Result<T, DomainError>;

/// A violated institutional invariant.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DomainError {
    /// Input failed validation.
    #[error("{0}")]
    Validation(String),

    /// A workflow transition that the lifecycle does not permit.
    #[error("cannot move from '{from}' to '{to}'")]
    InvalidTransition {
        /// Current state.
        from: &'static str,
        /// Requested state.
        to: &'static str,
    },

    /// A transition that is only reachable through a dedicated operation.
    #[error("{0}")]
    TransitionRequiresOperation(String),

    /// The caller is not permitted.
    #[error("{0}")]
    PermissionDenied(String),
}

impl DomainError {
    /// Stable error code for the API envelope.
    #[must_use]
    pub const fn code(&self) -> ErrorCode {
        match self {
            Self::Validation(_) => ErrorCode::ValidationError,
            Self::InvalidTransition { .. } | Self::TransitionRequiresOperation(_) => {
                ErrorCode::InvalidWorkflowTransition
            }
            Self::PermissionDenied(_) => ErrorCode::PermissionDenied,
        }
    }
}
