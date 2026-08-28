//! Request-scoped middleware: correlation identifiers, security headers and the
//! same-origin guard on writes.

use axum::extract::{Request, State};
use axum::http::{header, HeaderName, HeaderValue, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use ocinye_contracts::{ErrorBody, ErrorCode};
use ocinye_observability::{CorrelationIds, CORRELATION_ID_HEADER, REQUEST_ID_HEADER};

use crate::state::AppState;

/// Headers applied to every response.
///
/// The Core serves JSON only and is never a document source, so the policy is
/// as tight as it can be: nothing may load, frame or embed it.
const SECURITY_HEADERS: &[(&str, &str)] = &[
    ("x-content-type-options", "nosniff"),
    ("x-frame-options", "DENY"),
    ("referrer-policy", "no-referrer"),
    ("cross-origin-opener-policy", "same-origin"),
    ("cross-origin-resource-policy", "same-origin"),
    (
        "content-security-policy",
        "default-src 'none'; frame-ancestors 'none'",
    ),
    (
        "permissions-policy",
        "geolocation=(), microphone=(), camera=()",
    ),
    // Responses are per-principal; a shared cache must never hold one.
    ("cache-control", "no-store"),
];

/// Attach correlation identifiers, security headers and an access log line.
pub async fn apply(mut request: Request, next: Next) -> Response {
    let ids = CorrelationIds::from_headers(
        request
            .headers()
            .get(REQUEST_ID_HEADER)
            .and_then(|value| value.to_str().ok()),
        request
            .headers()
            .get(CORRELATION_ID_HEADER)
            .and_then(|value| value.to_str().ok()),
    );

    let method = request.method().clone();
    let path = request.uri().path().to_owned();

    request.extensions_mut().insert(ids.clone());

    let started = std::time::Instant::now();
    let mut response = next.run(request).await;
    let duration_ms = started.elapsed().as_millis();

    for (name, value) in SECURITY_HEADERS {
        if let (Ok(name), Ok(value)) = (
            HeaderName::from_bytes(name.as_bytes()),
            HeaderValue::from_str(value),
        ) {
            response.headers_mut().entry(name).or_insert(value);
        }
    }
    if let Ok(value) = HeaderValue::from_str(&ids.request_id) {
        response
            .headers_mut()
            .insert(HeaderName::from_static(REQUEST_ID_HEADER), value);
    }
    if let Ok(value) = HeaderValue::from_str(&ids.correlation_id) {
        response
            .headers_mut()
            .insert(HeaderName::from_static(CORRELATION_ID_HEADER), value);
    }

    // The query string is deliberately omitted: it may carry search terms over
    // classified material (`CLAUDE.md` §62).
    tracing::info!(
        method = %method,
        path = %path,
        status = response.status().as_u16(),
        duration_ms,
        request_id = %ids.request_id,
        "request"
    );

    response
}

/// Refuse a state-changing request that a browser sent from another origin.
///
/// # What this adds over the cookie's `SameSite`
///
/// The Core's session cookie is `SameSite=Strict`, which stops a *cross-site*
/// write. «Site» is the registrable domain, not the origin: a page served from
/// any `*.ocinye.com` — and `CLAUDE.md` §5 reserves `ocinye.com` for a future
/// public website — is same-site with the Core, so the browser attaches the
/// cookie to its requests. A subdomain is not a trust boundary
/// (`CLAUDE.md` §16), and an XSS on one would otherwise become a write here.
///
/// # The rule, and why an absent `Origin` passes
///
/// Browsers send `Origin` on every request that can change state. A request
/// without one therefore did not come from a browser's cross-origin path — it
/// came from a CLI, a notebook, an agent or the Workspace server, none of which
/// a hostile page can drive (`CLAUDE.md` §3). Refusing those would close the
/// API to its intended clients while stopping nothing.
///
/// So: an `Origin` that is present and is not ours is refused. Absent is
/// allowed. Safe methods are not checked.
pub async fn same_origin_writes(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let unsafe_method = !matches!(
        *request.method(),
        Method::GET | Method::HEAD | Method::OPTIONS
    );

    if !unsafe_method {
        return next.run(request).await;
    }

    let origin = request
        .headers()
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok());

    let Some(origin) = origin.map(str::trim).filter(|value| !value.is_empty()) else {
        return next.run(request).await;
    };

    let host = request
        .headers()
        .get(header::HOST)
        .and_then(|value| value.to_str().ok());

    if origin_permitted(origin, host, &state.config.cors_allowed_origins) {
        return next.run(request).await;
    }

    let ids = request
        .extensions()
        .get::<CorrelationIds>()
        .cloned()
        .unwrap_or_default();

    // The origin is not echoed back: it is attacker-chosen text, and a
    // response that repeats it is a response that can be made to say anything.
    tracing::warn!(
        method = %request.method(),
        path = %request.uri().path(),
        request_id = %ids.request_id,
        "refused a write from a foreign origin"
    );

    let body = ErrorBody::new(
        ErrorCode::PermissionDenied,
        "Este pedido não veio de uma origem reconhecida.".to_owned(),
    )
    .with_ids(Some(ids.request_id), Some(ids.correlation_id));

    (StatusCode::FORBIDDEN, Json(body)).into_response()
}

/// Whether an `Origin` is one this Core answers writes for.
///
/// Configured browser origins, plus the one whose host is the request's own
/// `Host`. The browser fills `Host` with the real target, so matching it is an
/// **origin** comparison and not a site one.
fn origin_permitted(origin: &str, host: Option<&str>, allowed: &[String]) -> bool {
    // `null` arrives from a sandboxed frame or a `data:` document. It is never
    // this Core.
    if origin.eq_ignore_ascii_case("null") {
        return false;
    }

    let origin = origin.trim_end_matches('/');

    if allowed
        .iter()
        .any(|candidate| candidate.trim_end_matches('/') == origin)
    {
        return true;
    }

    let Some((_scheme, origin_host)) = origin.split_once("://") else {
        return false;
    };

    matches!(host, Some(host) if origin_host.eq_ignore_ascii_case(host))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALLOWED: &[&str] = &["https://workspace.ocinye.com"];

    fn allowed() -> Vec<String> {
        ALLOWED.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn a_sibling_subdomain_is_not_a_permitted_origin() {
        // O que `SameSite=Strict` deixa passar: mesmo sítio, outra origem.
        for hostile in [
            "https://ocinye.com",
            "https://www.ocinye.com",
            "https://workspace.ocinye.com.evil.example",
            "https://evil.example",
            "null",
        ] {
            assert!(
                !origin_permitted(hostile, Some("core.ocinye.com"), &allowed()),
                "{hostile:?} foi aceite"
            );
        }
    }

    #[test]
    fn a_configured_origin_is_permitted() {
        assert!(origin_permitted(
            "https://workspace.ocinye.com",
            Some("core.ocinye.com"),
            &allowed()
        ));
        assert!(origin_permitted(
            "https://workspace.ocinye.com/",
            None,
            &allowed()
        ));
    }

    #[test]
    fn an_origin_that_is_the_requested_host_is_permitted() {
        assert!(origin_permitted(
            "https://core.ocinye.com",
            Some("core.ocinye.com"),
            &[]
        ));
        assert!(!origin_permitted(
            "https://core.ocinye.com",
            Some("outro.ocinye.com"),
            &[]
        ));
    }
}
