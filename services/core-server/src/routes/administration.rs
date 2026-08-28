//! Member administration: creation, reset, suspension, roles and grants.
//!
//! Every handler here authorises against a named [`Permission`] before doing
//! anything. None of them consults a role directly — that is the whole reason
//! the permission layer exists (briefing §58).

use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use ocinye_contracts::{AccountStatus, InstitutionalPosition, Permission, Scope, TechnicalRole};
use ocinye_core::modules::governance::grants;
use ocinye_core::modules::identity::{self, NewMember, TemporaryCredential};
use ocinye_core::CoreError;
use ocinye_domain::{can, explain, ResourceContext, ResourceKind};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ApiError;
use crate::extract::{
    Authorised, CurrentPrincipal, Ids, NeedsMembersCreate, NeedsMembersManage,
    NeedsPermissionsManage, NeedsPermissionsView, NeedsRolesView,
};
use crate::state::AppState;

/// Administration routes.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/administration/members", post(create_member))
        .route(
            "/administration/members/{person_id}/password-reset",
            post(reset_password),
        )
        .route(
            "/administration/members/{person_id}/status",
            post(set_status),
        )
        .route(
            "/administration/members/{person_id}/security",
            get(security_overview),
        )
        .route(
            "/administration/members/{person_id}/access",
            get(access_overview),
        )
        .route(
            "/administration/grants",
            post(create_grant).get(list_grants),
        )
        .route(
            "/administration/grants/{grant_id}",
            axum::routing::delete(revoke_grant),
        )
        .route("/administration/roles", get(list_roles))
        .route("/administration/permissions", get(list_permissions))
}

/// Authorise a named permission at institution scope, or fail closed.
///
/// Used only where the requirement depends on the *target* — reading one's own
/// security metadata needs nothing, reading someone else's needs the
/// permission. Every endpoint with a fixed requirement uses [`Authorised`]
/// instead, so the check happens before the body is parsed.
fn require(
    principal: &ocinye_domain::Principal,
    permission: Permission,
    ids: &ocinye_observability::CorrelationIds,
) -> Result<(), ApiError> {
    let ctx = ResourceContext::organisation(ResourceKind::Organisation, principal.organisation_id);
    if can(principal, permission, &ctx, None).allowed {
        Ok(())
    } else {
        Err(ApiError::new(
            CoreError::PermissionDenied("Não possui acesso a esta operação.".to_owned()),
            ids,
        ))
    }
}

// ── Member creation ─────────────────────────────────────────────────────

#[derive(Deserialize)]
struct CreateMemberRequest {
    full_name: String,
    username: String,
    email: String,
    #[serde(default)]
    position: Option<String>,
    role: String,
    #[serde(default)]
    unit_id: Option<Uuid>,
}

/// A temporary credential, returned exactly once.
///
/// There is no endpoint that reads this back. After this response the value
/// exists only in whatever the administrator did with it (briefing §18, §19).
#[derive(Serialize)]
struct IssuedCredential {
    username: String,
    /// Shown once. Never stored, never recoverable.
    temporary_password: String,
    expires_at: DateTime<Utc>,
    /// Stated in the response so the interface need not repeat the policy.
    shown_once: bool,
}

impl IssuedCredential {
    fn new(credential: TemporaryCredential) -> Self {
        Self {
            username: credential.username,
            temporary_password: credential.secret.expose().to_owned(),
            expires_at: credential.expires_at,
            shown_once: true,
        }
    }
}

#[derive(Serialize)]
struct CreatedMember {
    person_id: Uuid,
    credential: IssuedCredential,
}

/// `POST /administration/members`
async fn create_member(
    State(state): State<AppState>,
    Ids(ids): Ids,
    // Declared before `Json`: the permission is decided before the body is
    // parsed, so an unauthorised caller learns nothing about the schema.
    Authorised { principal, .. }: Authorised<NeedsMembersCreate>,
    Json(request): Json<CreateMemberRequest>,
) -> Result<Json<CreatedMember>, ApiError> {
    let role = TechnicalRole::parse(&request.role).ok_or_else(|| {
        ApiError::new(
            CoreError::Validation("Papel técnico desconhecido.".to_owned()),
            &ids,
        )
    })?;

    // Granting a role you do not hold yourself is privilege escalation with
    // extra steps. Only a platform administrator may create one.
    if role == TechnicalRole::PlatformAdmin
        && !can(
            &principal,
            Permission::PlatformAdminister,
            &ResourceContext::organisation(ResourceKind::Organisation, principal.organisation_id),
            None,
        )
        .allowed
    {
        return Err(ApiError::new(
            CoreError::PermissionDenied(
                "Criar um administrador de plataforma exige o mesmo papel.".to_owned(),
            ),
            &ids,
        ));
    }

    let position = request
        .position
        .as_deref()
        .and_then(InstitutionalPosition::parse);

    let (person, credential) = identity::create_member(
        &state.pool,
        &state.authenticator,
        &principal,
        &NewMember {
            full_name: request.full_name,
            username: request.username,
            email: request.email,
            position,
            role,
            unit_id: request.unit_id,
        },
        &ids,
    )
    .await
    .map_err(|error| ApiError::new(error, &ids))?;

    Ok(Json(CreatedMember {
        person_id: person.id,
        credential: IssuedCredential::new(credential),
    }))
}

// ── Password reset ──────────────────────────────────────────────────────

/// `POST /administration/members/{person_id}/password-reset`
async fn reset_password(
    State(state): State<AppState>,
    Ids(ids): Ids,
    Authorised { principal, .. }: Authorised<NeedsMembersManage>,
    Path(person_id): Path<Uuid>,
) -> Result<Json<IssuedCredential>, ApiError> {
    let person = scoped_person(&state, &principal, person_id, &ids).await?;

    let credential =
        identity::reset_password(&state.pool, &state.authenticator, &principal, &person, &ids)
            .await
            .map_err(|error| ApiError::new(error, &ids))?;

    Ok(Json(IssuedCredential::new(credential)))
}

// ── Account status ──────────────────────────────────────────────────────

#[derive(Deserialize)]
struct StatusRequest {
    status: String,
    reason: String,
}

/// `POST /administration/members/{person_id}/status`
async fn set_status(
    State(state): State<AppState>,
    Ids(ids): Ids,
    Authorised { principal, .. }: Authorised<NeedsMembersManage>,
    Path(person_id): Path<Uuid>,
    Json(request): Json<StatusRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let status = AccountStatus::parse(&request.status).ok_or_else(|| {
        ApiError::new(
            CoreError::Validation("Estado de conta desconhecido.".to_owned()),
            &ids,
        )
    })?;

    if request.reason.trim().len() < 4 {
        return Err(ApiError::new(
            CoreError::Validation("Indique a razão da alteração.".to_owned()),
            &ids,
        ));
    }

    let person = scoped_person(&state, &principal, person_id, &ids).await?;

    identity::set_account_status(
        &state.pool,
        &principal,
        &person,
        status,
        request.reason.trim(),
        &ids,
    )
    .await
    .map_err(|error| ApiError::new(error, &ids))?;

    Ok(Json(serde_json::json!({ "status": status.as_str() })))
}

// ── Security overview ───────────────────────────────────────────────────

/// Safe metadata about an account's credentials.
///
/// Note what is absent: no verifier, no password, no hash, not even the length
/// of one (briefing §73).
#[derive(Serialize)]
struct SecurityOverview {
    account_status: &'static str,
    has_permanent_password: bool,
    password_changed_at: Option<DateTime<Utc>>,
    temporary_credential_expires_at: Option<DateTime<Utc>>,
    last_successful_sign_in: Option<DateTime<Utc>>,
    recent_failed_attempts: i64,
    live_sessions: Vec<SessionSummary>,
}

#[derive(Serialize)]
struct SessionSummary {
    id: Uuid,
    state: &'static str,
    issued_at: DateTime<Utc>,
    last_seen_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    user_agent: Option<String>,
    ip_prefix: Option<String>,
}

/// `GET /administration/members/{person_id}/security`
async fn security_overview(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(person_id): Path<Uuid>,
) -> Result<Json<SecurityOverview>, ApiError> {
    // Anyone may read their own security metadata; reading someone else's needs
    // the permission.
    if person_id != principal.person_id {
        require(&principal, Permission::MembersManage, &ids)?;
    }

    let person = scoped_person(&state, &principal, person_id, &ids).await?;

    let credentials = identity::live_credentials_for(&state.pool, person.id)
        .await
        .map_err(|error| ApiError::new(error, &ids))?;

    let permanent = credentials
        .iter()
        .find(|c| c.kind == ocinye_contracts::CredentialKind::Permanent);
    let temporary = credentials
        .iter()
        .find(|c| c.kind == ocinye_contracts::CredentialKind::Temporary);

    let sessions = identity::list_sessions(&state.pool, person.id)
        .await
        .map_err(|error| ApiError::new(error, &ids))?;

    let (last_sign_in, failures) = identity::attempt_summary(&state.pool, &person)
        .await
        .map_err(|error| ApiError::new(error, &ids))?;

    Ok(Json(SecurityOverview {
        account_status: person.account_status().as_str(),
        has_permanent_password: permanent.is_some(),
        password_changed_at: permanent.map(|c| c.created_at),
        temporary_credential_expires_at: temporary.and_then(|c| c.expires_at),
        last_successful_sign_in: last_sign_in,
        recent_failed_attempts: failures,
        live_sessions: sessions
            .into_iter()
            .map(|s| SessionSummary {
                id: s.id,
                state: s.state.as_str(),
                issued_at: s.issued_at,
                last_seen_at: s.last_seen_at,
                expires_at: s.expires_at,
                user_agent: s.user_agent,
                ip_prefix: s.ip_prefix,
            })
            .collect(),
    }))
}

// ── Access overview ─────────────────────────────────────────────────────

/// Why a person has the access they have.
#[derive(Serialize)]
struct AccessOverview {
    roles: Vec<&'static str>,
    units: Vec<ScopedRole>,
    workspaces: Vec<ScopedRole>,
    grants: Vec<grants::GrantView>,
    /// Institution-scope permissions, each with the source that confers it.
    institution_permissions: Vec<PermissionSource>,
}

#[derive(Serialize)]
struct ScopedRole {
    id: Uuid,
    role: &'static str,
}

#[derive(Serialize)]
struct PermissionSource {
    permission: &'static str,
    source: &'static str,
}

/// `GET /administration/members/{person_id}/access`
async fn access_overview(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(person_id): Path<Uuid>,
) -> Result<Json<AccessOverview>, ApiError> {
    if person_id != principal.person_id {
        require(&principal, Permission::RolesView, &ids)?;
    }

    let person = scoped_person(&state, &principal, person_id, &ids).await?;
    let subject = identity::principal_for_person(&state.pool, &person)
        .await
        .map_err(|error| ApiError::new(error, &ids))?;

    let held = grants::for_subject(&state.pool, principal.organisation_id, person.id)
        .await
        .map_err(|error| ApiError::new(error, &ids))?;

    let ctx = ResourceContext::organisation(ResourceKind::Organisation, principal.organisation_id);

    let mut roles: Vec<&'static str> = subject.roles.iter().map(|r| r.as_str()).collect();
    roles.sort_unstable();

    // The answer to "why can this person do this?" — computed, not guessed.
    let institution_permissions = Permission::all()
        .into_iter()
        .filter(|p| can(&subject, *p, &ctx, None).allowed)
        .filter_map(|p| {
            explain(&subject, p, &ctx, None).map(|source| PermissionSource {
                permission: p.as_str(),
                source: source.label(),
            })
        })
        .collect();

    Ok(Json(AccessOverview {
        roles,
        units: subject
            .unit_roles
            .iter()
            .map(|(id, role)| ScopedRole {
                id: *id,
                role: role.as_str(),
            })
            .collect(),
        workspaces: subject
            .workspace_roles
            .iter()
            .map(|(id, role)| ScopedRole {
                id: *id,
                role: role.as_str(),
            })
            .collect(),
        grants: held,
        institution_permissions,
    }))
}

// ── Grants ──────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct CreateGrantRequest {
    subject_id: Uuid,
    permission: String,
    scope: String,
    #[serde(default)]
    scope_id: Option<Uuid>,
    reason: String,
    #[serde(default)]
    expires_at: Option<DateTime<Utc>>,
}

/// `POST /administration/grants`
async fn create_grant(
    State(state): State<AppState>,
    Ids(ids): Ids,
    Authorised { principal, .. }: Authorised<NeedsPermissionsManage>,
    Json(request): Json<CreateGrantRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let permission = Permission::parse(&request.permission).ok_or_else(|| {
        ApiError::new(
            CoreError::Validation("Permissão desconhecida.".to_owned()),
            &ids,
        )
    })?;
    let scope = Scope::parse(&request.scope).ok_or_else(|| {
        ApiError::new(
            CoreError::Validation("Âmbito desconhecido.".to_owned()),
            &ids,
        )
    })?;

    // Nobody may grant what they do not themselves hold. Without this, anyone
    // with `PermissionsManage` could grant themselves everything.
    let target_ctx = match scope {
        Scope::Unit => ResourceContext::unit(
            ResourceKind::Unit,
            principal.organisation_id,
            request.scope_id.unwrap_or_default(),
        ),
        _ => ResourceContext::organisation(ResourceKind::Organisation, principal.organisation_id),
    };
    if !can(&principal, permission, &target_ctx, request.scope_id).allowed {
        return Err(ApiError::new(
            CoreError::PermissionDenied("Não pode conceder um acesso que não possui.".to_owned()),
            &ids,
        ));
    }

    let id = grants::create(
        &state.pool,
        &principal,
        &grants::NewGrant {
            subject_id: request.subject_id,
            permission,
            scope,
            scope_id: request.scope_id,
            reason: request.reason,
            expires_at: request.expires_at,
        },
        &ids,
    )
    .await
    .map_err(|error| ApiError::new(error, &ids))?;

    Ok(Json(serde_json::json!({ "id": id })))
}

#[derive(Deserialize)]
struct GrantQuery {
    subject_id: Uuid,
}

/// `GET /administration/grants?subject_id=…`
async fn list_grants(
    State(state): State<AppState>,
    Ids(ids): Ids,
    Authorised { principal, .. }: Authorised<NeedsPermissionsView>,
    Query(query): Query<GrantQuery>,
) -> Result<Json<Vec<grants::GrantView>>, ApiError> {
    let held = grants::for_subject(&state.pool, principal.organisation_id, query.subject_id)
        .await
        .map_err(|error| ApiError::new(error, &ids))?;
    Ok(Json(held))
}

#[derive(Deserialize)]
struct RevokeGrantRequest {
    reason: String,
}

/// `DELETE /administration/grants/{grant_id}`
async fn revoke_grant(
    State(state): State<AppState>,
    Ids(ids): Ids,
    Authorised { principal, .. }: Authorised<NeedsPermissionsManage>,
    Path(grant_id): Path<Uuid>,
    Json(request): Json<RevokeGrantRequest>,
) -> Result<axum::http::StatusCode, ApiError> {
    grants::revoke(&state.pool, &principal, grant_id, &request.reason, &ids)
        .await
        .map_err(|error| ApiError::new(error, &ids))?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ── Catalogue ───────────────────────────────────────────────────────────

#[derive(Serialize)]
struct RoleView {
    role: &'static str,
    permissions: Vec<&'static str>,
    /// System roles are defined in code and cannot be edited at runtime
    /// (briefing §75).
    system: bool,
}

/// `GET /administration/roles`
async fn list_roles(
    _authorised: Authorised<NeedsRolesView>,
) -> Result<Json<Vec<RoleView>>, ApiError> {
    Ok(Json(
        TechnicalRole::all()
            .into_iter()
            .map(|role| RoleView {
                role: role.as_str(),
                permissions: ocinye_domain::policy::permissions::role_permissions(role)
                    .iter()
                    .map(|p| p.as_str())
                    .collect(),
                system: true,
            })
            .collect(),
    ))
}

/// `GET /administration/permissions`
async fn list_permissions(
    _authorised: Authorised<NeedsPermissionsView>,
) -> Result<Json<Vec<&'static str>>, ApiError> {
    Ok(Json(
        Permission::all().into_iter().map(|p| p.as_str()).collect(),
    ))
}

// ── Shared ──────────────────────────────────────────────────────────────

/// Load a person, refusing anyone outside the caller's organisation.
///
/// Reported as "not found" rather than "forbidden": whether a person exists in
/// another institution is not the caller's business (ADR-0100).
async fn scoped_person(
    state: &AppState,
    principal: &ocinye_domain::Principal,
    person_id: Uuid,
    ids: &ocinye_observability::CorrelationIds,
) -> Result<ocinye_core::modules::identity::Person, ApiError> {
    let person = identity::person_by_id(&state.pool, person_id)
        .await
        .map_err(|error| ApiError::new(error, ids))?
        .filter(|p| p.organisation_id == principal.organisation_id)
        .ok_or_else(|| {
            ApiError::new(
                CoreError::NotFound("Membro não encontrado.".to_owned()),
                ids,
            )
        })?;
    Ok(person)
}
