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

use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use ocinye_contracts::SessionState;
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
        // ── O segundo factor (ADR-0107) ─────────────────────────────────────
        // Todos operam sobre uma sessão restrita — a `mfa_required` que o login
        // deixou —, e o sucesso de um desafio fecha o portão emitindo uma sessão
        // nova, `active` e com a garantia de MFA, no mesmo `Set-Cookie`.
        .route("/auth/mfa", get(mfa_status))
        .route("/auth/mfa/enroll", post(mfa_enroll))
        .route("/auth/mfa/enroll/confirm", post(mfa_confirm))
        .route("/auth/mfa/acknowledge", post(mfa_acknowledge))
        .route("/auth/mfa/challenge", post(mfa_challenge))
        .route("/auth/mfa/recovery", post(mfa_recovery))
        .route("/auth/mfa/recovery/regenerate", post(mfa_regenerate))
}

/// Credentials presented at sign-in.
#[derive(Deserialize)]
struct LoginRequest {
    email: String,
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
            // Precisely the password-change state, not merely "not ordinary
            // work": an `mfa_required` session also cannot do ordinary work, but
            // it does not owe a password (ADR-0107). Clients route on `state`.
            must_change_password: issued.state == SessionState::PasswordChangeRequired,
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
            &request.email,
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
        state: session.state.as_str(),
        // The password-change state precisely, not any restricted state: an
        // `mfa_required` session owes a second factor, not a password.
        must_change_password: session.state == SessionState::PasswordChangeRequired,
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

// ── O segundo factor (ADR-0107) ─────────────────────────────────────────────

/// O emissor mostrado no autenticador. Sem espaços, para não precisar de
/// codificação na etiqueta `otpauth`.
const MFA_ISSUER: &str = "Ocinye";

/// Em que ponto do MFA esta sessão está — para a Experience mostrar o ecrã certo
/// sem adivinhar (ADR-0107).
#[derive(Serialize)]
struct MfaStatus {
    /// `enrollment` (falta enrolar), `challenge` (enrolado, falta o código) ou
    /// `not_required` (esta identidade não exige MFA).
    mfa_mode: &'static str,
}

/// `GET /auth/mfa`
async fn mfa_status(
    State(state): State<AppState>,
    Ids(ids): Ids,
    RestrictedSession { person, .. }: RestrictedSession,
) -> Result<Json<MfaStatus>, ApiError> {
    let principal = identity::principal_for_person(&state.pool, &person)
        .await
        .map_err(|error| ApiError::new(error, &ids))?;
    let mode = if !identity::mfa_required(&principal) {
        "not_required"
    } else if identity::has_confirmed_totp(&state.pool, person.id)
        .await
        .map_err(|error| ApiError::new(error, &ids))?
    {
        "challenge"
    } else {
        "enrollment"
    };
    Ok(Json(MfaStatus { mfa_mode: mode }))
}

#[derive(Deserialize)]
struct EnrollQuery {
    /// Só com `reveal=1` é que a chave manual em base32 volta no corpo. Por
    /// omissão vai só a URI `otpauth` — o QR —, e o segredo em texto fica de fora
    /// até a pessoa o pedir por acção explícita (ADR-0107).
    #[serde(default)]
    reveal: bool,
}

#[derive(Serialize)]
struct EnrollResponse {
    otpauth_uri: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    secret_base32: Option<String>,
}

/// `POST /auth/mfa/enroll`
///
/// Idempotente: devolve sempre o **mesmo** seed por confirmar enquanto o
/// enrolamento não fecha, para que o QR e a chave manual nunca divirjam.
async fn mfa_enroll(
    State(state): State<AppState>,
    Ids(ids): Ids,
    RestrictedSession { person, .. }: RestrictedSession,
    Query(query): Query<EnrollQuery>,
) -> Result<Json<EnrollResponse>, ApiError> {
    let enrollment = identity::begin_enrollment(
        &state.pool,
        state.config.sealing_key.as_ref(),
        &person,
        MFA_ISSUER,
    )
    .await
    .map_err(|error| ApiError::new(error, &ids))?;

    Ok(Json(EnrollResponse {
        otpauth_uri: enrollment.otpauth_uri,
        secret_base32: query.reveal.then_some(enrollment.secret_base32),
    }))
}

#[derive(Deserialize)]
struct CodeRequest {
    code: String,
}

#[derive(Serialize)]
struct RecoveryCodesResponse {
    recovery_codes: Vec<String>,
}

/// `POST /auth/mfa/enroll/confirm`
///
/// Confirma o enrolamento com um código e devolve os códigos de recuperação
/// **uma única vez**. Não fecha o portão: a sessão continua `mfa_required` até o
/// acknowledgement. TOTP válido ≠ enrolamento concluído (ADR-0107).
async fn mfa_confirm(
    State(state): State<AppState>,
    Ids(ids): Ids,
    RestrictedSession { person, .. }: RestrictedSession,
    Json(request): Json<CodeRequest>,
) -> Result<Json<RecoveryCodesResponse>, ApiError> {
    let principal = identity::principal_for_person(&state.pool, &person)
        .await
        .map_err(|error| ApiError::new(error, &ids))?;
    let recovery_codes = identity::confirm_enrollment(
        &state.pool,
        state.config.sealing_key.as_ref(),
        &state.authenticator.hasher,
        &principal,
        &person,
        &request.code,
        &ids,
    )
    .await
    .map_err(|error| ApiError::new(error, &ids))?;

    Ok(Json(RecoveryCodesResponse { recovery_codes }))
}

/// `POST /auth/mfa/acknowledge`
///
/// A pessoa confirmou que guardou os códigos de recuperação. Só aqui o
/// enrolamento se conclui e o portão se fecha: revoga a sessão-portão e emite
/// uma sessão nova, `active` e assegurada, no mesmo `Set-Cookie` do login.
async fn mfa_acknowledge(
    State(state): State<AppState>,
    Ids(ids): Ids,
    headers: HeaderMap,
    RestrictedSession { session, person }: RestrictedSession,
) -> Result<Response, ApiError> {
    if session.state != SessionState::MfaRequired {
        return Err(ApiError::new(
            CoreError::PermissionDenied("Não há enrolamento de MFA por concluir.".to_owned()),
            &ids,
        ));
    }
    if !identity::has_confirmed_totp(&state.pool, person.id)
        .await
        .map_err(|error| ApiError::new(error, &ids))?
    {
        return Err(ApiError::new(
            CoreError::Validation("Confirme o autenticador antes de concluir.".to_owned()),
            &ids,
        ));
    }
    let context = context_from(headers);
    let issued = identity::issue_assured_session(&state.pool, &person, session.id, &context, &ids)
        .await
        .map_err(|error| ApiError::new(error, &ids))?;
    Ok(with_session_cookie(
        &state,
        &issued,
        SessionResponse::from_issued(&issued),
    ))
}

/// `POST /auth/mfa/challenge`
async fn mfa_challenge(
    State(state): State<AppState>,
    Ids(ids): Ids,
    headers: HeaderMap,
    RestrictedSession { session, person }: RestrictedSession,
    Json(request): Json<CodeRequest>,
) -> Result<Response, ApiError> {
    if session.state != SessionState::MfaRequired {
        return Err(ApiError::new(
            CoreError::PermissionDenied("Não há desafio de MFA pendente nesta sessão.".to_owned()),
            &ids,
        ));
    }
    let ok = identity::verify_challenge(
        &state.pool,
        state.config.sealing_key.as_ref(),
        person.id,
        &request.code,
    )
    .await
    .map_err(|error| ApiError::new(error, &ids))?;
    if !ok {
        return Err(ApiError::new(
            CoreError::Validation(
                "O código não confere. Verifique a hora do dispositivo e tente o código actual."
                    .to_owned(),
            ),
            &ids,
        ));
    }
    let context = context_from(headers);
    let issued = identity::issue_assured_session(&state.pool, &person, session.id, &context, &ids)
        .await
        .map_err(|error| ApiError::new(error, &ids))?;
    Ok(with_session_cookie(
        &state,
        &issued,
        SessionResponse::from_issued(&issued),
    ))
}

/// `POST /auth/mfa/recovery`
///
/// Um código de recuperação satisfaz o desafio quando o autenticador não está à
/// mão. Uso único: consumi-lo é atómico, e vale exactamente uma vez.
async fn mfa_recovery(
    State(state): State<AppState>,
    Ids(ids): Ids,
    headers: HeaderMap,
    RestrictedSession { session, person }: RestrictedSession,
    Json(request): Json<CodeRequest>,
) -> Result<Response, ApiError> {
    if session.state != SessionState::MfaRequired {
        return Err(ApiError::new(
            CoreError::PermissionDenied("Não há desafio de MFA pendente nesta sessão.".to_owned()),
            &ids,
        ));
    }
    let principal = identity::principal_for_person(&state.pool, &person)
        .await
        .map_err(|error| ApiError::new(error, &ids))?;
    let ok = identity::consume_recovery_code(
        &state.pool,
        &state.authenticator.hasher,
        &principal,
        person.id,
        &request.code,
        &ids,
    )
    .await
    .map_err(|error| ApiError::new(error, &ids))?;
    if !ok {
        return Err(ApiError::new(
            CoreError::Validation("Código de recuperação inválido ou já usado.".to_owned()),
            &ids,
        ));
    }
    let context = context_from(headers);
    let issued = identity::issue_assured_session(&state.pool, &person, session.id, &context, &ids)
        .await
        .map_err(|error| ApiError::new(error, &ids))?;
    Ok(with_session_cookie(
        &state,
        &issued,
        SessionResponse::from_issued(&issued),
    ))
}

/// O que a regeneração de códigos de recuperação exige: a reautenticação.
#[derive(Deserialize)]
struct RegenerateRequest {
    /// A palavra-passe actual — step-up para uma acção sensível.
    password: Secret,
    /// Um código do autenticador — prova de que o factor está na mão de quem
    /// pede, e não só que a sessão está aberta.
    code: String,
}

/// `POST /auth/mfa/recovery/regenerate`
///
/// Emite dez códigos novos e invalida os anteriores, para uma sessão já
/// assegurada, depois de reautenticar com a palavra-passe **e** o factor actual
/// (ADR-0107). Os antigos não se recuperam.
async fn mfa_regenerate(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
    Json(request): Json<RegenerateRequest>,
) -> Result<Json<RecoveryCodesResponse>, ApiError> {
    let person = identity::get_own_person(&state.pool, &principal)
        .await
        .map_err(|error| ApiError::new(error, &ids))?;

    // Reautenticação, os dois factores. A ausência de credencial e a
    // palavra-passe errada dão a mesma resposta.
    let credenciais = identity::live_credentials_for(&state.pool, person.id)
        .await
        .map_err(|error| ApiError::new(error, &ids))?;
    let now = chrono::Utc::now();
    let senha_confere = credenciais.iter().any(|c| {
        c.kind == ocinye_contracts::CredentialKind::Permanent
            && c.is_usable(now)
            && state
                .authenticator
                .hasher
                .verify(&request.password, &c.verifier)
    });
    if !senha_confere {
        return Err(ApiError::new(
            CoreError::PermissionDenied("A palavra-passe actual não confere.".to_owned()),
            &ids,
        ));
    }

    let factor_confere = identity::verify_challenge(
        &state.pool,
        state.config.sealing_key.as_ref(),
        person.id,
        &request.code,
    )
    .await
    .map_err(|error| ApiError::new(error, &ids))?;
    if !factor_confere {
        return Err(ApiError::new(
            CoreError::Validation("O código do autenticador não confere.".to_owned()),
            &ids,
        ));
    }

    let recovery_codes = identity::regenerate_recovery_codes(
        &state.pool,
        &state.authenticator.hasher,
        &principal,
        &person,
        &ids,
    )
    .await
    .map_err(|error| ApiError::new(error, &ids))?;

    Ok(Json(RecoveryCodesResponse { recovery_codes }))
}

/// Build the attempt context from a request's headers.
fn context_from(headers: HeaderMap) -> identity::AttemptContext {
    let mut parts = axum::http::Request::new(());
    *parts.headers_mut() = headers;
    let (parts, ()) = parts.into_parts();
    attempt_context(&parts)
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
