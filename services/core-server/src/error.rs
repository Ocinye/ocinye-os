//! Rendering errors as the single API envelope.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use ocinye_core::CoreError;
use ocinye_observability::CorrelationIds;

/// A Core error together with the identifiers of the request that produced it.
///
/// Carrying the identifiers into the response is what lets a member quote a
/// request id that finds the matching log lines.
pub struct ApiError {
    error: CoreError,
    ids: Option<CorrelationIds>,
}

impl ApiError {
    /// Wrap an error with the current request's identifiers.
    pub fn new(error: CoreError, ids: &CorrelationIds) -> Self {
        Self {
            error,
            ids: Some(ids.clone()),
        }
    }
}

impl From<CoreError> for ApiError {
    fn from(error: CoreError) -> Self {
        Self { error, ids: None }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (request_id, correlation_id) = self
            .ids
            .map(|ids| (Some(ids.request_id), Some(ids.correlation_id)))
            .unwrap_or((None, None));

        // Unexpected failures are logged with their detail; the response keeps
        // none of it, so an internal error never becomes an information leak.
        if self.error.is_unexpected() {
            tracing::error!(error = %self.error, ?request_id, "request failed");
        }

        let body = self.error.to_body(request_id, correlation_id);
        let status = StatusCode::from_u16(self.error.code().status())
            .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

        (status, Json(body)).into_response()
    }
}
