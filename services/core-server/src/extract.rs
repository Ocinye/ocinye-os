//! Extractors: correlation identifiers, the session, and the acting principal.
//!
//! # Where the first-login rule is actually enforced
//!
//! In [`CurrentPrincipal`], below. A handler that takes a `CurrentPrincipal`
//! cannot run for someone whose session is in
//! [`SessionState::PasswordChangeRequired`] — the extractor refuses before the
//! handler body exists. That is what makes briefing §24 true for every endpoint
//! at once, including endpoints written next year by someone who never read it.
//!
//! An endpoint that must work *during* the password change takes
//! [`RestrictedSession`] instead, and there are exactly three of them.

use axum::extract::{ConnectInfo, FromRequestParts};
use axum::http::header;
use axum::http::request::Parts;
use ocinye_contracts::Permission;
use ocinye_contracts::SessionState;
use ocinye_core::modules::identity::{self, AttemptContext, Person, StoredSession};
use ocinye_core::password::Secret;
use ocinye_core::CoreError;
use ocinye_domain::Principal;
use ocinye_domain::{can, ResourceContext, ResourceKind};
use ocinye_observability::CorrelationIds;
use std::marker::PhantomData;
use std::net::SocketAddr;

use crate::error::ApiError;
use crate::state::AppState;

/// Name of the session cookie set by the Core.
pub const SESSION_COOKIE: &str = "ocinye_core_session";

/// The identifiers of the current request.
pub struct Ids(pub CorrelationIds);

impl<S: Send + Sync> FromRequestParts<S> for Ids {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(Self(
            parts
                .extensions
                .get::<CorrelationIds>()
                .cloned()
                .unwrap_or_default(),
        ))
    }
}

/// Read the session token from the request.
///
/// Accepts it in the cookie, or as a bearer token so that a CLI, a notebook or
/// a Node Agent can hold a session without a cookie jar (`CLAUDE.md` §3). Both
/// carry the same opaque value and neither is more trusted than the other.
fn session_token(parts: &Parts) -> Option<Secret> {
    if let Some(value) = parts
        .headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
    {
        if let Some((scheme, token)) = value.split_once(' ') {
            if scheme.eq_ignore_ascii_case("bearer") && !token.trim().is_empty() {
                return Some(Secret::new(token.trim()));
            }
        }
    }

    parts
        .headers
        .get(header::COOKIE)
        .and_then(|value| value.to_str().ok())?
        .split(';')
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(name, _)| *name == SESSION_COOKIE)
        .map(|(_, value)| Secret::new(value.trim()))
}

/// Coarse network prefix of the caller.
///
/// The last octet of an IPv4 address, and everything below the /64, are
/// discarded: enough to throttle and to recognise an unfamiliar origin, short
/// of recording where a researcher physically is (briefing §38).
#[must_use]
pub fn client_prefix(parts: &Parts) -> Option<String> {
    let addr = parts.extensions.get::<ConnectInfo<SocketAddr>>()?.0.ip();
    Some(match addr {
        std::net::IpAddr::V4(v4) => {
            let [a, b, c, _] = v4.octets();
            format!("{a}.{b}.{c}.0/24")
        }
        std::net::IpAddr::V6(v6) => {
            let segments = v6.segments();
            format!(
                "{:x}:{:x}:{:x}:{:x}::/64",
                segments[0], segments[1], segments[2], segments[3]
            )
        }
    })
}

/// Context of an attempt, for throttling and session records.
#[must_use]
pub fn attempt_context(parts: &Parts) -> AttemptContext {
    AttemptContext {
        ip_prefix: client_prefix(parts),
        user_agent: parts
            .headers
            .get(header::USER_AGENT)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned),
    }
}

/// A live session of any state, with the person it belongs to.
///
/// Used only by the three endpoints that must work while a password change is
/// outstanding: reading one's own minimal identity, setting the password, and
/// signing out.
pub struct RestrictedSession {
    /// The session record.
    pub session: StoredSession,
    /// Who it belongs to.
    pub person: Person,
}

impl FromRequestParts<AppState> for RestrictedSession {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let ids = parts
            .extensions
            .get::<CorrelationIds>()
            .cloned()
            .unwrap_or_default();

        let unauthenticated = || {
            ApiError::new(
                CoreError::Unauthenticated("É necessária uma sessão válida.".to_owned()),
                &ids,
            )
        };

        let token = session_token(parts).ok_or_else(unauthenticated)?;
        let session = identity::find_session(&state.pool, &token)
            .await
            .map_err(|error| ApiError::new(error, &ids))?
            .ok_or_else(unauthenticated)?;

        let person = identity::person_by_id(&state.pool, session.person_id)
            .await
            .map_err(|error| ApiError::new(error, &ids))?
            .ok_or_else(unauthenticated)?;

        // A session outlives neither suspension nor disabling. Sessions are
        // revoked when those happen, but checking again here means a race
        // between the two cannot produce a usable session.
        if !person.account_status().may_authenticate() {
            return Err(unauthenticated());
        }

        Ok(Self { session, person })
    }
}

/// The authenticated caller, cleared for ordinary work.
///
/// Assembles the principal from institutional state: roles, memberships and
/// live grants all come from the database, never from anything the client
/// sends (ADR-0100).
pub struct CurrentPrincipal(pub Principal);

impl FromRequestParts<AppState> for CurrentPrincipal {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let ids = parts
            .extensions
            .get::<CorrelationIds>()
            .cloned()
            .unwrap_or_default();

        let RestrictedSession { session, person } =
            RestrictedSession::from_request_parts(parts, state).await?;

        // The rule of briefing §22 and §24, in one place, for every endpoint.
        if !session.state.permits_ordinary_work() {
            return Err(ApiError::new(
                CoreError::PermissionDenied(
                    "Defina a sua palavra-passe definitiva antes de continuar.".to_owned(),
                ),
                &ids,
            ));
        }

        // An invited account has by definition never set a password, so it must
        // not hold an ordinary session. Belt and braces with the state check
        // above: two independent facts must both be wrong for this to pass.
        if session.state == SessionState::Active
            && person.account_status() == ocinye_contracts::AccountStatus::Invited
        {
            return Err(ApiError::new(
                CoreError::PermissionDenied(
                    "Defina a sua palavra-passe definitiva antes de continuar.".to_owned(),
                ),
                &ids,
            ));
        }

        let principal = identity::principal_for_person(&state.pool, &person)
            .await
            .map_err(|error| ApiError::new(error, &ids))?;

        if !principal.is_active {
            return Err(ApiError::new(
                CoreError::PermissionDenied("Esta conta não está activa.".to_owned()),
                &ids,
            ));
        }

        // Best-effort activity record; never fails a request.
        if let Err(error) = identity::touch_session(&state.pool, session.id).await {
            tracing::warn!(error = %error, "could not record session activity");
        }

        Ok(Self(principal))
    }
}

/// A permission required by an endpoint, named at the type level.
///
/// # Why a type and not a call in the handler
///
/// Axum runs every extractor before the handler body. A handler that checks its
/// permission in the first line therefore checks it *after* the request body has
/// been parsed — so an unauthorised caller sends `{}`, gets a 422 listing the
/// fields the endpoint wants, and has learned the schema of an operation they
/// may not perform.
///
/// Putting the permission in an extractor puts the check back where it belongs:
/// before the body is looked at, and before the handler exists.
pub trait Requires {
    /// The permission this endpoint needs, at institution scope.
    const PERMISSION: Permission;
}

/// A principal that has already been authorised for `R`.
///
/// Ordering matters at the call site: declare this **before** any body
/// extractor, or the body is still parsed first.
pub struct Authorised<R: Requires> {
    /// The acting principal.
    pub principal: Principal,
    marker: PhantomData<R>,
}

impl<R: Requires> FromRequestParts<AppState> for Authorised<R> {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let ids = parts
            .extensions
            .get::<CorrelationIds>()
            .cloned()
            .unwrap_or_default();

        let CurrentPrincipal(principal) =
            CurrentPrincipal::from_request_parts(parts, state).await?;

        let ctx =
            ResourceContext::organisation(ResourceKind::Organisation, principal.organisation_id);

        if !can(&principal, R::PERMISSION, &ctx, None).allowed {
            return Err(ApiError::new(
                CoreError::PermissionDenied("Não possui acesso a esta operação.".to_owned()),
                &ids,
            ));
        }

        Ok(Self {
            principal,
            marker: PhantomData,
        })
    }
}

/// Declare a marker type for each permission an endpoint can require.
macro_rules! permission_markers {
    ($($name:ident => $permission:ident),* $(,)?) => {
        $(
            #[doc = concat!("Requires `Permission::", stringify!($permission), "`.")]
            pub struct $name;
            impl Requires for $name {
                const PERMISSION: Permission = Permission::$permission;
            }
        )*
    };
}

permission_markers! {
    NeedsMembersCreate     => MembersCreate,
    NeedsMembersManage     => MembersManage,
    NeedsRolesView         => RolesView,
    NeedsPermissionsView   => PermissionsView,
    NeedsPermissionsManage => PermissionsManage,
}
