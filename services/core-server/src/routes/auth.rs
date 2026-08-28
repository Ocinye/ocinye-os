//! Authentication endpoints.
//!
//! These are the only routes reachable without a session, and the only ones
//! that work while a password change is outstanding. Everything else in the API
//! takes [`CurrentPrincipal`](crate::extract::CurrentPrincipal) and is therefore
//! closed during that window (briefing §24).
//!
//! # Passwords never travel in a URL
//!
//! Every endpoint here is `POST` with a JSON body. A password in a query string
//! ends up in access logs, browser history and referrer headers (briefing §99).

use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use ocinye_core::modules::identity::{self, IssuedSession};
use ocinye_core::password::{policy, Secret};
use ocinye_core::CoreError;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ApiError;
use crate::extract::{attempt_context, CurrentPrincipal, Ids, RestrictedSession, SESSION_COOKIE};
use crate::state::AppState;

/// Authentication routes.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/auth/login", post(login))
        .route("/auth/logout", post(logout))
        .route("/auth/session", get(session))
        .route("/auth/password", post(set_password))
        .route("/auth/password/change", post(change_password))
        .route("/auth/sessions", get(own_sessions))
        .route(
            "/auth/sessions/{session_id}/revoke",
            post(revoke_own_session),
        )
        .route("/auth/password/assess", post(assess_password))
}

/// Credentials presented at sign-in.
#[derive(Deserialize)]
struct LoginRequest {
    username: String,
    password: Secret,
}

/// What a caller learns after signing in.
#[derive(Serialize)]
struct SessionResponse {
    /// The session token.
    ///
    /// Also set as a cookie. Returned in the body as well because the Ocinye
    /// Workspace is a server-side client that holds the token on the member's
    /// behalf, as are the CLI, notebooks and agents the Core must serve
    /// (`CLAUDE.md` §3). A browser calling this directly simply ignores it and
    /// uses the cookie.
    session_token: String,
    /// Session state; `password_change_required` means nothing else will work.
    state: &'static str,
    /// Display name.
    display_name: String,
    /// Whether the caller must set a password before continuing.
    must_change_password: bool,
}

impl SessionResponse {
    fn from_issued(issued: &IssuedSession) -> Self {
        Self {
            session_token: issued.token.expose().to_owned(),
            state: issued.state.as_str(),
            display_name: issued.display_name.clone(),
            must_change_password: !issued.state.permits_ordinary_work(),
        }
    }
}

/// Build the `Set-Cookie` header for a session.
///
/// `HttpOnly` keeps the token away from scripts, `SameSite=Strict` is possible
/// here because the Core is never the target of a cross-site navigation, and
/// `Secure` is on outside development.
fn session_cookie(token: &Secret, secure: bool, max_age_seconds: i64) -> HeaderValue {
    let mut cookie = format!(
        "{SESSION_COOKIE}={}; Path=/; HttpOnly; SameSite=Strict; Max-Age={max_age_seconds}",
        token.expose()
    );
    if secure {
        cookie.push_str("; Secure");
    }
    HeaderValue::from_str(&cookie).unwrap_or_else(|_| HeaderValue::from_static(""))
}

fn cleared_cookie(secure: bool) -> HeaderValue {
    let mut cookie = format!("{SESSION_COOKIE}=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0");
    if secure {
        cookie.push_str("; Secure");
    }
    HeaderValue::from_str(&cookie).unwrap_or_else(|_| HeaderValue::from_static(""))
}

fn with_session_cookie(
    state: &AppState,
    issued: &IssuedSession,
    body: SessionResponse,
) -> Response {
    let seconds = if issued.state.permits_ordinary_work() {
        identity::SESSION_LIFETIME_HOURS * 3600
    } else {
        identity::PASSWORD_CHANGE_SESSION_MINUTES * 60
    };

    let mut response = (StatusCode::OK, Json(body)).into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        session_cookie(
            &issued.token,
            state.config.environment.is_production(),
            seconds,
        ),
    );
    // A session response must never sit in a shared cache.
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, private"),
    );
    response
}

/// `POST /auth/login`
///
/// Returns the same refusal for every failure mode. The distinction between
/// "no such account", "wrong password", "expired credential" and "suspended"
/// exists only in the evidence trail.
async fn login(
    State(state): State<AppState>,
    Ids(ids): Ids,
    headers: HeaderMap,
    Json(request): Json<LoginRequest>,
) -> Result<Response, ApiError> {
    let mut parts = axum::http::Request::new(());
    *parts.headers_mut() = headers;
    let (parts, ()) = parts.into_parts();
    let context = attempt_context(&parts);

    let issued = state
        .authenticator
        .sign_in(
            &state.pool,
            &request.username,
            &request.password,
            &context,
            &ids,
        )
        .await
        .map_err(|error| ApiError::new(error, &ids))?;

    let body = SessionResponse::from_issued(&issued);
    Ok(with_session_cookie(&state, &issued, body))
}

/// `POST /auth/logout`
///
/// Works on a restricted session too: someone who cannot complete the password
/// change must still be able to leave.
async fn logout(
    State(state): State<AppState>,
    Ids(ids): Ids,
    RestrictedSession { session, person }: RestrictedSession,
) -> Result<Response, ApiError> {
    identity::revoke_session(&state.pool, session.id, "signed_out")
        .await
        .map_err(|error| ApiError::new(error, &ids))?;

    ocinye_core::audit::record_standalone(
        &state.pool,
        &ids,
        person.id,
        person.organisation_id,
        ocinye_core::audit::action::SIGN_OUT,
        "person",
        person.id,
    )
    .await;

    let mut response = StatusCode::NO_CONTENT.into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        cleared_cookie(state.config.environment.is_production()),
    );
    Ok(response)
}

/// Minimal identity, available on a restricted session.
#[derive(Serialize)]
struct RestrictedIdentity {
    display_name: String,
    username: Option<String>,
    state: &'static str,
    must_change_password: bool,
    /// Minimum password length, so the interface can state the rule without
    /// hardcoding a number that the Core might change.
    minimum_password_length: usize,
}

/// `GET /auth/session`
///
/// The one read permitted during a password change: enough to greet the person
/// and state the rule, and nothing institutional (briefing §22).
async fn session(
    RestrictedSession { session, person }: RestrictedSession,
) -> Json<RestrictedIdentity> {
    Json(RestrictedIdentity {
        display_name: person.preferred_name().to_owned(),
        username: person.username.clone(),
        state: session.state.as_str(),
        must_change_password: !session.state.permits_ordinary_work(),
        minimum_password_length: policy::MIN_LENGTH,
    })
}

/// O corpo de uma mudança voluntária de palavra-passe.
#[derive(Deserialize)]
struct ChangePasswordRequest {
    current: Secret,
    password: Secret,
    confirmation: Secret,
}

/// `POST /auth/password/change`
///
/// Mudança voluntária, por quem já trabalha no sistema. Distinta de
/// [`set_password`], que serve o primeiro acesso: aqui a sessão aberta não é
/// prova suficiente, e a palavra-passe actual é obrigatória.
///
/// A conta é a da sessão. Não há campo por onde escolher outra.
///
/// **A rotação da sessão faz parte do sucesso.** O Core revoga todas as sessões
/// e emite uma nova; esta resposta instala-a no mesmo `Set-Cookie` que o login
/// usa. Devolver `200` sem a instalar deixaria o cliente com um cookie que o
/// próprio pedido acabou de invalidar.
async fn change_password(
    State(state): State<AppState>,
    Ids(ids): Ids,
    headers: HeaderMap,
    RestrictedSession { session, person }: RestrictedSession,
    Json(request): Json<ChangePasswordRequest>,
) -> Result<Response, ApiError> {
    // Uma sessão restrita muda a palavra-passe pelo primeiro acesso, que não
    // pede a actual porque ela é a credencial temporária. Deixar os dois
    // caminhos abertos à mesma sessão tornaria a confirmação contornável.
    if !session.state.permits_ordinary_work() {
        return Err(ApiError::new(
            CoreError::PermissionDenied(
                "Esta sessão tem de definir a palavra-passe pelo primeiro acesso.".to_owned(),
            ),
            &ids,
        ));
    }

    if request.password.expose() != request.confirmation.expose() {
        return Err(ApiError::new(
            CoreError::Validation("As palavras-passe não coincidem.".to_owned()),
            &ids,
        ));
    }

    let mut parts = axum::http::Request::new(());
    *parts.headers_mut() = headers;
    let (parts, ()) = parts.into_parts();
    let context = attempt_context(&parts);

    let issued = identity::change_own_password(
        &state.pool,
        &state.authenticator,
        &person,
        &request.current,
        &request.password,
        &context,
        &ids,
    )
    .await
    .map_err(|error| ApiError::new(error, &ids))?;

    let body = SessionResponse::from_issued(&issued);
    Ok(with_session_cookie(&state, &issued, body))
}

/// Uma sessão, tal como o seu dono a vê.
#[derive(Serialize)]
struct OwnSessionView {
    id: Uuid,
    state: &'static str,
    issued_at: chrono::DateTime<chrono::Utc>,
    last_seen_at: chrono::DateTime<chrono::Utc>,
    expires_at: chrono::DateTime<chrono::Utc>,
    /// O que o cliente enviou como `User-Agent`, quando o enviou.
    user_agent: Option<String>,
    /// Prefixo de rede, e não o endereço: identifica a origem sem a apontar.
    ip_prefix: Option<String>,
    /// A sessão a partir da qual este pedido chegou.
    is_current: bool,
}

/// `GET /auth/sessions`
///
/// As sessões de quem pergunta. Não aceita identificador de pessoa: a lista sai
/// do principal, e um selector de conta seria, nesta superfície, a própria
/// vulnerabilidade.
///
/// Não devolve token nem o seu digest. O que o dono precisa de saber é quando a
/// sessão começou, de onde, e qual é a actual.
async fn own_sessions(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
    RestrictedSession { session, .. }: RestrictedSession,
) -> Result<Json<Vec<OwnSessionView>>, ApiError> {
    let sessions = identity::list_own_sessions(&state.pool, &principal)
        .await
        .map_err(|error| ApiError::new(error, &ids))?;

    Ok(Json(
        sessions
            .into_iter()
            .map(|s| OwnSessionView {
                is_current: s.id == session.id,
                id: s.id,
                state: s.state.as_str(),
                issued_at: s.issued_at,
                last_seen_at: s.last_seen_at,
                expires_at: s.expires_at,
                user_agent: s.user_agent,
                ip_prefix: s.ip_prefix,
            })
            .collect(),
    ))
}

/// `POST /auth/sessions/{session_id}/revoke`
///
/// Termina uma sessão **do próprio**. O identificador vem do cliente, e por isso
/// a posse é resolvida no Core antes de qualquer alteração — um UUID identifica
/// a sessão, nunca autoriza a operação.
///
/// Terminar a sessão actual é permitido, e nesse caso a resposta limpa o cookie:
/// dizer «terminada» e deixar o cliente autenticado seria mentir sobre o efeito.
async fn revoke_own_session(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
    RestrictedSession { session, .. }: RestrictedSession,
    Path(session_id): Path<Uuid>,
) -> Result<Response, ApiError> {
    identity::revoke_own_session(&state.pool, &principal, session_id, "revoked_by_holder")
        .await
        .map_err(|error| ApiError::new(error, &ids))?;

    if session_id == session.id {
        let mut response = StatusCode::NO_CONTENT.into_response();
        response.headers_mut().insert(
            header::SET_COOKIE,
            cleared_cookie(state.config.environment.is_production()),
        );
        return Ok(response);
    }

    Ok(StatusCode::NO_CONTENT.into_response())
}

/// A new permanent password.

#[derive(Deserialize)]
struct PasswordRequest {
    password: Secret,
    confirmation: Secret,
}

/// `POST /auth/password`
///
/// Sets the permanent password and rotates the session. The session that made
/// this call is revoked along with every other, and a fresh one is issued
/// (briefing §29, §30).
async fn set_password(
    State(state): State<AppState>,
    Ids(ids): Ids,
    headers: HeaderMap,
    RestrictedSession { session, person }: RestrictedSession,
    Json(request): Json<PasswordRequest>,
) -> Result<Response, ApiError> {
    let _ = session;

    if request.password.expose() != request.confirmation.expose() {
        return Err(ApiError::new(
            CoreError::Validation("As palavras-passe não coincidem.".to_owned()),
            &ids,
        ));
    }

    let mut parts = axum::http::Request::new(());
    *parts.headers_mut() = headers;
    let (parts, ()) = parts.into_parts();
    let context = attempt_context(&parts);

    let issued = identity::set_permanent_password(
        &state.pool,
        &state.authenticator,
        &person,
        &request.password,
        &context,
        &ids,
    )
    .await
    .map_err(|error| ApiError::new(error, &ids))?;

    let body = SessionResponse::from_issued(&issued);
    Ok(with_session_cookie(&state, &issued, body))
}

/// A candidate being typed.
#[derive(Deserialize)]
struct AssessRequest {
    password: Secret,
}

/// How the candidate reads.
#[derive(Serialize)]
struct AssessResponse {
    strength: policy::Strength,
    minimum_password_length: usize,
}

/// `POST /auth/password/assess`
///
/// Feeds the strength indicator. Advisory only: [`set_password`] validates
/// again, and its answer is the one that counts (briefing §27).
async fn assess_password(
    RestrictedSession { .. }: RestrictedSession,
    Json(request): Json<AssessRequest>,
) -> Json<AssessResponse> {
    Json(AssessResponse {
        strength: policy::assess(&request.password),
        minimum_password_length: policy::MIN_LENGTH,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_session_cookie_is_httponly_strict_and_secure_outside_development() {
        let token = Secret::new("abc123");
        let secure = session_cookie(&token, true, 3600);
        let value = secure.to_str().unwrap();
        assert!(value.contains("HttpOnly"));
        assert!(value.contains("SameSite=Strict"));
        assert!(value.contains("Secure"));
        assert!(value.contains("Max-Age=3600"));

        let insecure = session_cookie(&token, false, 3600);
        assert!(!insecure.to_str().unwrap().contains("Secure"));
    }

    #[test]
    fn clearing_the_cookie_carries_no_token() {
        let cleared = cleared_cookie(true);
        let value = cleared.to_str().unwrap();
        assert!(value.contains("Max-Age=0"));
        assert!(value.starts_with(&format!("{SESSION_COOKIE}=;")));
    }
}
