//! Unit routes.

use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use ocinye_contracts::UnitRole;
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
        .route("/units", get(list_units).post(create_unit))
        .route("/units/{unit_id}", get(get_unit).delete(archive_unit))
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
    Ok(Json(OrganisationView {
        id: organisation.id,
        slug: organisation.slug,
        name: organisation.name,
        country: organisation.country,
    }))
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
) -> Result<Json<UnitView>, ApiError> {
    let unit = organisation::get_unit(&state.pool, &principal, unit_id).await?;
    Ok(Json(UnitView::from(unit)))
}

#[derive(Deserialize)]
struct CreateUnitRequest {
    code: String,
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
