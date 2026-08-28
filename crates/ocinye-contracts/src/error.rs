//! Structured error envelope.
//!
//! Error codes are a stable part of the API contract. Clients branch on
//! [`ErrorCode`], never on prose, and the message never leaks internal
//! structure such as table or column names.

use serde::{Deserialize, Serialize};

/// Stable machine-readable error code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// The request payload failed validation.
    ValidationError,
    /// Authentication is required or the presented token is invalid.
    AuthenticationRequired,
    /// The caller is authenticated but not permitted.
    PermissionDenied,
    /// The resource does not exist, or must be indistinguishable from that.
    NotFound,
    /// The request conflicts with current state.
    Conflict,
    /// The requested workflow transition is not allowed.
    InvalidWorkflowTransition,
    /// Too many requests.
    RateLimited,
    /// A capability is genuinely unavailable — for example, no Ocinye AI node.
    CapabilityUnavailable,
    /// Object storage is not configured or not reachable.
    StorageUnavailable,
    /// Unexpected failure.
    InternalError,
}

impl ErrorCode {
    /// HTTP status this code maps to.
    #[must_use]
    pub const fn status(self) -> u16 {
        match self {
            Self::ValidationError => 422,
            Self::AuthenticationRequired => 401,
            Self::PermissionDenied => 403,
            Self::NotFound => 404,
            Self::Conflict | Self::InvalidWorkflowTransition => 409,
            Self::RateLimited => 429,
            Self::CapabilityUnavailable | Self::StorageUnavailable => 503,
            Self::InternalError => 500,
        }
    }
}

/// Body returned for every non-success response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorBody {
    /// Stable error code.
    pub code: ErrorCode,
    /// Human-readable message, safe to display.
    pub message: String,
    /// Bounded, non-sensitive detail (field names, allowed transitions).
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub details: serde_json::Map<String, serde_json::Value>,
    /// Identifier of this request, for support and log correlation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    /// Identifier correlating this request across services.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
}

impl ErrorBody {
    /// Build an envelope.
    #[must_use]
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: serde_json::Map::new(),
            request_id: None,
            correlation_id: None,
        }
    }

    /// Attach a detail entry.
    #[must_use]
    pub fn with_detail(mut self, key: &str, value: serde_json::Value) -> Self {
        self.details.insert(key.to_owned(), value);
        self
    }

    /// Attach correlation identifiers.
    #[must_use]
    pub fn with_ids(mut self, request_id: Option<String>, correlation_id: Option<String>) -> Self {
        self.request_id = request_id;
        self.correlation_id = correlation_id;
        self
    }
}
