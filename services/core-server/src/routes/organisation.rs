//! Unit routes.

use axum::extract::{Path, Query, State};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use ocinye_contracts::{ApplicationId, InstanceProfile, UnitRole};
use ocinye_core::modules::organisation;
use ocinye_core::CoreError;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ApiError;
use crate::extract::{CurrentPrincipal, Ids};
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/organisation", get(get_organisation))
        .route("/instance/applications", get(get_instance_applications))
        .route("/instance/profile", put(put_instance_profile))
        .route(
            "/instance/applications/{application_id}",
            put(put_instance_application),
        )
        .route("/units", get(list_units).post(create_unit))
        .route("/units/code-suggestion", get(suggest_code))
        .route(
            "/units/{unit_id}",
            get(get_unit).put(update_unit).delete(archive_unit),
        )
        .route(
            "/units/{unit_id}/members",
            get(list_members).post(add_member),
        )
        .route("/units/{unit_id}/members/{person_id}", post(revoke_member))
}

/// The institution this deployment serves.
#[derive(Serialize)]
struct OrganisationView {
    id: Uuid,
    slug: String,
    name: String,
    country: Option<String>,
    /// The Instance's profile (ADR-0014).
    profile: InstanceProfile,
}

/// Return the institution.
///
/// A deployment serves exactly one organisation, resolved at startup. Returning
/// it explicitly saves the Workspace from inferring institutional identity from
/// a hostname.
async fn get_organisation(
    State(state): State<AppState>,
    CurrentPrincipal(_principal): CurrentPrincipal,
) -> Result<Json<OrganisationView>, ApiError> {
    let organisation = organisation::get_organisation(&state.pool, state.organisation_id).await?;
    let profile = organisation::profile_of(&state.pool, state.organisation_id).await?;
    Ok(Json(OrganisationView {
        id: organisation.id,
        slug: organisation.slug,
        name: organisation.name,
        country: organisation.country,
        profile,
    }))
}

/// The Instance's profile and the state of every application (ADR-0014).
async fn get_instance_applications(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
) -> Result<Json<organisation::InstanceApplications>, ApiError> {
    organisation::instance_applications(&state.pool, &principal)
        .await
        .map(Json)
        .map_err(|error| ApiError::new(error, &ids))
}

#[derive(Deserialize)]
struct ProfileBody {
    profile: InstanceProfile,
}

/// Change the Instance's profile. Explicit decisions stay; nothing is deleted.
async fn put_instance_profile(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
    Json(body): Json<ProfileBody>,
) -> Result<Json<organisation::InstanceApplications>, ApiError> {
    organisation::set_profile(&state.pool, &principal, body.profile, &ids)
        .await
        .map(Json)
        .map_err(|error| ApiError::new(error, &ids))
}

#[derive(Deserialize)]
struct ApplicationBody {
    /// `true` activates, `false` deactivates, `null` returns to the profile.
    active: Option<bool>,
}

/// Activate, deactivate, or return an optional application to its profile.
async fn put_instance_application(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(application_id): Path<String>,
    Json(body): Json<ApplicationBody>,
) -> Result<Json<organisation::InstanceApplications>, ApiError> {
    let application: ApplicationId = application_id.parse().map_err(|_| {
        ApiError::new(
            CoreError::NotFound("Aplicação desconhecida.".to_owned()),
            &ids,
        )
    })?;
    organisation::set_application_active(&state.pool, &principal, application, body.active, &ids)
        .await
        .map(Json)
        .map_err(|error| ApiError::new(error, &ids))
}

#[derive(Serialize)]
struct UnitView {
    id: Uuid,
    code: String,
    name: String,
    description: Option<String>,
    research_areas: Vec<String>,
    status: String,
}

impl From<organisation::Unit> for UnitView {
    fn from(unit: organisation::Unit) -> Self {
        Self {
            id: unit.id,
            code: unit.code,
            name: unit.name,
            description: unit.description,
            research_areas: unit.research_areas,
            status: unit.status,
        }
    }
}

#[derive(Deserialize)]
struct ListUnitsQuery {
    #[serde(default)]
    include_archived: bool,
}

async fn list_units(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Query(query): Query<ListUnitsQuery>,
) -> Result<Json<Vec<UnitView>>, ApiError> {
    let units = organisation::list_units(&state.pool, &principal, query.include_archived).await?;
    Ok(Json(units.into_iter().map(UnitView::from).collect()))
}

async fn get_unit(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(unit_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let unit = organisation::get_unit(&state.pool, &principal, unit_id).await?;

    // A mesma pergunta que `add_unit_member` e `revoke_unit_member` fazem.
    //
    // Vem daqui e não de um palpite sobre o papel: se o controlo de gestão
    // aparecer no ecrã, a operação é autorizável pela mesma política que a vai
    // executar. Continua a ser cortesia de renderização — as duas operações
    // decidem outra vez.
    let may_manage_members = ocinye_domain::policy::authorize(
        &principal,
        ocinye_domain::policy::Action::ManageMembers,
        &ocinye_domain::policy::ResourceContext::unit(
            ocinye_domain::policy::ResourceKind::Unit,
            principal.organisation_id,
            unit.id,
        ),
    )
    .is_ok();

    let mut vista = serde_json::to_value(UnitView::from(unit)).unwrap_or_default();
    if let Some(objecto) = vista.as_object_mut() {
        objecto.insert(
            "may_manage_members".to_owned(),
            serde_json::Value::Bool(may_manage_members),
        );
    }
    Ok(Json(vista))
}

#[derive(Deserialize)]
struct CreateUnitRequest {
    /// Optional: omit to have the Core generate `U<ABBREV>-NNN` from the name.
    #[serde(default)]
    code: Option<String>,
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    research_areas: Vec<String>,
}

async fn create_unit(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Json(request): Json<CreateUnitRequest>,
) -> Result<Json<UnitView>, ApiError> {
    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let unit = organisation::create_unit(
        &mut tx,
        &principal,
        &ids,
        organisation::NewUnit {
            code: request.code,
            name: request.name,
            description: request.description,
            research_areas: request.research_areas,
        },
    )
    .await?;
    tx.commit().await.map_err(CoreError::from)?;
    Ok(Json(UnitView::from(unit)))
}

#[derive(Deserialize)]
struct SuggestCodeQuery {
    name: String,
}

#[derive(Serialize)]
struct SuggestCodeView {
    code: String,
}

/// Preview the code a unit with this name would receive.
///
/// Indicative: the number is confirmed only when the unit is created. Requires
/// the authority to create a unit.
async fn suggest_code(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Query(query): Query<SuggestCodeQuery>,
) -> Result<Json<SuggestCodeView>, ApiError> {
    let code = organisation::suggest_unit_code(&state.pool, &principal, &query.name).await?;
    Ok(Json(SuggestCodeView { code }))
}

#[derive(Deserialize)]
struct UpdateUnitRequest {
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    research_areas: Vec<String>,
}

/// Update a unit's name, description and research areas. The code is immutable.
async fn update_unit(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path(unit_id): Path<Uuid>,
    Json(request): Json<UpdateUnitRequest>,
) -> Result<Json<UnitView>, ApiError> {
    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let unit = organisation::update_unit(
        &mut tx,
        &principal,
        &ids,
        unit_id,
        organisation::UnitEdit {
            name: request.name,
            description: request.description,
            research_areas: request.research_areas,
        },
    )
    .await?;
    tx.commit().await.map_err(CoreError::from)?;
    Ok(Json(UnitView::from(unit)))
}

/// Archive a unit.
///
/// Archival, not deletion: a unit that existed is institutional history
/// (briefing §72).
async fn archive_unit(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path(unit_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    organisation::archive_unit(&mut tx, &principal, &ids, unit_id).await?;
    tx.commit().await.map_err(CoreError::from)?;
    Ok(Json(serde_json::json!({ "archived": true })))
}

#[derive(Serialize)]
struct MemberView {
    person_id: Uuid,
    full_name: String,
    role: String,
}

async fn list_members(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(unit_id): Path<Uuid>,
) -> Result<Json<Vec<MemberView>>, ApiError> {
    let members = organisation::list_unit_members(&state.pool, &principal, unit_id).await?;
    Ok(Json(
        members
            .into_iter()
            .map(|member| MemberView {
                person_id: member.person_id,
                full_name: member.full_name,
                role: member.role,
            })
            .collect(),
    ))
}

#[derive(Deserialize)]
struct AddMemberRequest {
    person_id: Uuid,
    #[serde(default = "default_unit_role")]
    role: String,
}

fn default_unit_role() -> String {
    UnitRole::Member.as_str().to_owned()
}

async fn add_member(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path(unit_id): Path<Uuid>,
    Json(request): Json<AddMemberRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let role = UnitRole::parse(&request.role)
        .ok_or_else(|| CoreError::Validation("Unknown unit role.".to_owned()))?;

    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let membership_id =
        organisation::add_unit_member(&mut tx, &principal, &ids, unit_id, request.person_id, role)
            .await?;
    tx.commit().await.map_err(CoreError::from)?;

    Ok(Json(serde_json::json!({ "membership_id": membership_id })))
}

/// Revoke a unit membership.
///
/// The row is kept: that a person belonged to a unit is institutional memory.
async fn revoke_member(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path((unit_id, person_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    organisation::revoke_unit_member(&mut tx, &principal, &ids, unit_id, person_id).await?;
    tx.commit().await.map_err(CoreError::from)?;
    Ok(Json(serde_json::json!({ "revoked": true })))
}
