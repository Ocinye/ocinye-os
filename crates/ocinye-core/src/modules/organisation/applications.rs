//! O perfil de uma Instância e as aplicações que tem activas (ADR-0014).
//!
//! # A regra
//!
//! Uma aplicação opcional está activa se a Instância o decidiu explicitamente
//! (`instance_applications`), ou, sem decisão, se o perfil a traz. Uma essencial
//! está sempre activa, e não se regista decisão sobre ela.
//!
//! Nada aqui concede autoridade: ler exige `organisation.view`, mudar exige
//! `organisation.manage`, e o que cada membro pode fazer dentro de uma aplicação
//! activa continua a ser da política de sempre.

use std::collections::BTreeMap;

use ocinye_contracts::{ApplicationClass, ApplicationId, InstanceProfile, Permission};
use ocinye_domain::policy::permissions::can;
use ocinye_domain::policy::{ResourceContext, ResourceKind};
use ocinye_domain::Principal;
use ocinye_observability::CorrelationIds;
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::audit::{self, action, AuditEntry};
use crate::error::{CoreError, CoreResult};

/// O estado de uma aplicação nesta Instância.
#[derive(Debug, Clone, Serialize)]
pub struct ApplicationState {
    /// O identificador técnico.
    pub id: ApplicationId,
    /// Essencial ou opcional.
    pub class: ApplicationClass,
    /// Se está activa agora.
    pub active: bool,
    /// Se o estado vem de uma decisão explícita da Instância, e não do perfil.
    pub explicit: bool,
    /// O que o perfil diria, sem decisão explícita.
    pub profile_default: bool,
}

/// A configuração de aplicações de uma Instância.
#[derive(Debug, Clone, Serialize)]
pub struct InstanceApplications {
    /// O perfil.
    pub profile: InstanceProfile,
    /// Todas as aplicações, na ordem do registo.
    pub applications: Vec<ApplicationState>,
}

fn require(principal: &Principal, permission: Permission) -> CoreResult<()> {
    let ctx = ResourceContext::organisation(ResourceKind::Organisation, principal.organisation_id);
    if can(principal, permission, &ctx, None).allowed {
        Ok(())
    } else {
        Err(CoreError::PermissionDenied(
            "Não possui acesso à configuração da instância.".to_owned(),
        ))
    }
}

/// O perfil de uma Instância.
///
/// # Errors
///
/// Returns an error when the query fails, or the stored value is not a profile.
pub async fn profile_of(pool: &PgPool, organisation_id: Uuid) -> CoreResult<InstanceProfile> {
    let stored: String = sqlx::query_scalar("SELECT profile FROM organisations WHERE id = $1")
        .bind(organisation_id)
        .fetch_one(pool)
        .await?;
    stored
        .parse()
        .map_err(|error: ocinye_contracts::UnknownProfile| CoreError::Internal(error.to_string()))
}

async fn explicit_decisions(
    pool: &PgPool,
    organisation_id: Uuid,
) -> CoreResult<BTreeMap<ApplicationId, bool>> {
    let rows: Vec<(String, bool)> = sqlx::query_as(
        "SELECT application_id, active FROM instance_applications WHERE organisation_id = $1",
    )
    .bind(organisation_id)
    .fetch_all(pool)
    .await?;
    // Um identificador que o catálogo já não conhece é ignorado, e não um erro:
    // uma aplicação retirada do código não pode partir a leitura das outras.
    Ok(rows
        .into_iter()
        .filter_map(|(id, active)| id.parse().ok().map(|app| (app, active)))
        .collect())
}

/// O estado de todas as aplicações desta Instância. Sem autorização: é o
/// cálculo que `me` e o ecrã de configuração partilham.
///
/// # Errors
///
/// Returns an error when a query fails.
pub async fn application_states(
    pool: &PgPool,
    organisation_id: Uuid,
) -> CoreResult<InstanceApplications> {
    let profile = profile_of(pool, organisation_id).await?;
    let decisions = explicit_decisions(pool, organisation_id).await?;
    let applications = ApplicationId::ALL
        .into_iter()
        .map(|id| {
            let profile_default = profile.activates(id);
            let decided = if id.is_optional() {
                decisions.get(&id).copied()
            } else {
                None
            };
            ApplicationState {
                id,
                class: id.class(),
                active: decided.unwrap_or(profile_default),
                explicit: decided.is_some(),
                profile_default,
            }
        })
        .collect();
    Ok(InstanceApplications {
        profile,
        applications,
    })
}

/// As aplicações **inactivas** desta Instância — o que o Workspace esconde.
///
/// # Errors
///
/// Returns an error when a query fails.
pub async fn inactive_applications(
    pool: &PgPool,
    organisation_id: Uuid,
) -> CoreResult<Vec<ApplicationId>> {
    Ok(application_states(pool, organisation_id)
        .await?
        .applications
        .into_iter()
        .filter(|state| !state.active)
        .map(|state| state.id)
        .collect())
}

/// A configuração de aplicações, para quem a pode ler.
///
/// # Errors
///
/// [`CoreError::PermissionDenied`] without `organisation.view`; database errors.
pub async fn instance_applications(
    pool: &PgPool,
    principal: &Principal,
) -> CoreResult<InstanceApplications> {
    require(principal, Permission::OrganisationView)?;
    application_states(pool, principal.organisation_id).await
}

/// Muda o perfil da Instância. As decisões explícitas mantêm-se; as aplicações
/// sem decisão passam a seguir o perfil novo. Nada se apaga.
///
/// # Errors
///
/// [`CoreError::PermissionDenied`] without `organisation.manage`; database errors.
pub async fn set_profile(
    pool: &PgPool,
    principal: &Principal,
    profile: InstanceProfile,
    ids: &CorrelationIds,
) -> CoreResult<InstanceApplications> {
    require(principal, Permission::OrganisationManage)?;
    let mut tx = pool.begin().await?;
    let anterior: String =
        sqlx::query_scalar("SELECT profile FROM organisations WHERE id = $1 FOR UPDATE")
            .bind(principal.organisation_id)
            .fetch_one(&mut *tx)
            .await?;
    sqlx::query("UPDATE organisations SET profile = $2, updated_at = now() WHERE id = $1")
        .bind(principal.organisation_id)
        .bind(profile.as_str())
        .execute(&mut *tx)
        .await?;
    audit::record(
        &mut tx,
        Some(principal),
        ids,
        AuditEntry::new(action::ADMIN_OPERATION, "instance_profile")
            .resource(principal.organisation_id)
            .detail("from", anterior.as_str())
            .detail("to", profile.as_str()),
    )
    .await?;
    tx.commit().await?;
    application_states(pool, principal.organisation_id).await
}

/// Activa, desactiva, ou devolve ao perfil (`None`) uma aplicação opcional.
///
/// Desactivar esconde; não apaga. Uma aplicação essencial recusa-se, porque
/// sem ela não há sistema operativo.
///
/// # Errors
///
/// [`CoreError::PermissionDenied`] without `organisation.manage`;
/// [`CoreError::Conflict`] for an essential application; database errors.
pub async fn set_application_active(
    pool: &PgPool,
    principal: &Principal,
    application: ApplicationId,
    active: Option<bool>,
    ids: &CorrelationIds,
) -> CoreResult<InstanceApplications> {
    require(principal, Permission::OrganisationManage)?;
    if !application.is_optional() {
        return Err(CoreError::Conflict(format!(
            "«{application}» é uma aplicação essencial e não se desactiva."
        )));
    }
    let mut tx = pool.begin().await?;
    match active {
        Some(active) => {
            sqlx::query(
                "INSERT INTO instance_applications
                     (organisation_id, application_id, active, updated_by_id)
                 VALUES ($1, $2, $3, $4)
                 ON CONFLICT (organisation_id, application_id)
                 DO UPDATE SET active = EXCLUDED.active,
                               updated_by_id = EXCLUDED.updated_by_id,
                               updated_at = now()",
            )
            .bind(principal.organisation_id)
            .bind(application.as_str())
            .bind(active)
            .bind(principal.person_id)
            .execute(&mut *tx)
            .await?;
        }
        None => {
            sqlx::query(
                "DELETE FROM instance_applications
                  WHERE organisation_id = $1 AND application_id = $2",
            )
            .bind(principal.organisation_id)
            .bind(application.as_str())
            .execute(&mut *tx)
            .await?;
        }
    }
    audit::record(
        &mut tx,
        Some(principal),
        ids,
        AuditEntry::new(action::ADMIN_OPERATION, "instance_application")
            .resource(principal.organisation_id)
            .detail("application", application.as_str())
            .detail(
                "state",
                match active {
                    Some(true) => "active",
                    Some(false) => "inactive",
                    None => "profile_default",
                },
            ),
    )
    .await?;
    tx.commit().await?;
    application_states(pool, principal.organisation_id).await
}
