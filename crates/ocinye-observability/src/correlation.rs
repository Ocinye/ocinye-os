//! Request and correlation identifiers.
//!
//! A **request id** identifies one HTTP request. A **correlation id** follows a
//! logical operation across the Workspace, the Core, the Worker and, in future,
//! a compute node. Both are propagated end to end and returned to the caller,
//! so a member reporting a problem can quote an identifier that finds the
//! matching log lines.

use uuid::Uuid;

/// Header carrying the per-request identifier.
pub const REQUEST_ID_HEADER: &str = "x-request-id";
/// Header carrying the cross-service correlation identifier.
pub const CORRELATION_ID_HEADER: &str = "x-correlation-id";

/// The identifiers attached to the current operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorrelationIds {
    /// Identifier of this request.
    pub request_id: String,
    /// Identifier shared across every hop of the logical operation.
    pub correlation_id: String,
}

impl CorrelationIds {
    /// Derive identifiers from inbound headers, generating what is missing.
    ///
    /// An inbound value is accepted only if it looks like an identifier we
    /// issued. Echoing arbitrary client input into logs would make log
    /// injection trivial.
    #[must_use]
    pub fn from_headers(request_id: Option<&str>, correlation_id: Option<&str>) -> Self {
        let request_id = request_id
            .filter(|value| is_acceptable(value))
            .map_or_else(new_id, ToOwned::to_owned);

        let correlation_id = correlation_id
            .filter(|value| is_acceptable(value))
            .map_or_else(|| request_id.clone(), ToOwned::to_owned);

        Self {
            request_id,
            correlation_id,
        }
    }

    /// Fresh identifiers for an operation that did not start from a request.
    #[must_use]
    pub fn generate() -> Self {
        let id = new_id();
        Self {
            request_id: id.clone(),
            correlation_id: id,
        }
    }
}

impl Default for CorrelationIds {
    fn default() -> Self {
        Self::generate()
    }
}

fn new_id() -> String {
    Uuid::new_v4().to_string()
}

/// Whether an inbound identifier is safe to adopt and log.
fn is_acceptable(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inbound_identifiers_are_adopted_when_well_formed() {
        let ids = CorrelationIds::from_headers(Some("req-1"), Some("corr-1"));
        assert_eq!(ids.request_id, "req-1");
        assert_eq!(ids.correlation_id, "corr-1");
    }

    #[test]
    fn correlation_defaults_to_the_request_id() {
        let ids = CorrelationIds::from_headers(Some("req-1"), None);
        assert_eq!(ids.correlation_id, "req-1");
    }

    #[test]
    fn hostile_inbound_values_are_replaced_not_echoed() {
        for hostile in [
            "line\nbreak",
            "\u{1b}[31mred",
            "spaces here",
            "quote\"inject",
            &"x".repeat(65),
            "",
        ] {
            let ids = CorrelationIds::from_headers(Some(hostile), None);
            assert_ne!(ids.request_id, hostile, "must not adopt {hostile:?}");
            assert!(Uuid::parse_str(&ids.request_id).is_ok());
        }
    }
}
