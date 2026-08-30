//! Organisation application layer.

use ocinye_contracts::{Classification, UnitRole};
use ocinye_domain::identifiers::validate_unit_code;
use ocinye_domain::policy::{authorize, Action, ResourceContext, ResourceKind};
use ocinye_domain::Principal;
use ocinye_observability::CorrelationIds;
use sqlx::{PgPool, Postgres};
use uuid::Uuid;

use super::model::{Organisation, Unit, UnitMember};
use super::repository as repo;
use crate::audit::{self, action, AuditEntry};
use crate::error::{CoreError, CoreResult};
use crate::Tx;

/// Authorization context for a unit.
///
/// A unit's own existence is `INTERNAL`: every active member may see the shape
/// of the institution. What lives *inside* a unit carries its own
/// classification.
#[must_use]
pub fn unit_context(unit: &Unit) -> ResourceContext {
    ResourceContext::unit(ResourceKind::Unit, unit.organisation_id, unit.id)
        .with_classification(Classification::Internal)
}

/// Details of a new unit.
#[derive(Debug, Clone)]
pub struct NewUnit {
    /// Short code, for example `AI`.
    pub code: String,
    /// Display name.
    pub name: String,
    /// Description.
    pub description: Option<String>,
    /// Declared areas of research.
    pub research_areas: Vec<String>,
}

/// Ensure the organisation exists, creating it on first start.
///
/// Idempotent: repeated startups do not create duplicates. This is the only
/// path that writes without a principal, because it runs before any person can
/// exist; it is audited as an administrative operation.
///
/// # Errors
///
/// Returns an error when the query or insert fails.
pub async fn bootstrap_organisation(
    pool: &PgPool,
    slug: &str,
    name: &str,
    ids: &CorrelationIds,
) -> CoreResult<Organisation> {
    if let Some(existing) = repo::find_organisation_by_slug(pool, slug).await? {
        return Ok(existing);
    }

    let mut tx = pool.begin().await?;
    let organisation = repo::insert_organisation(&mut *tx, slug, name, None).await?;
    audit::record(
        &mut tx,
        None,
        ids,
        AuditEntry::new(action::ADMIN_OPERATION, "organisation")
            .resource(organisation.id)
            .detail("event", "bootstrap")
            .detail("slug", slug),
    )
    .await?;
    tx.commit().await?;

    tracing::info!(slug, "organisation created");
    Ok(organisation)
}

/// Load the institution this deployment serves.
///
/// Reachable by any authenticated member: the shape of the institution is
/// `INTERNAL`, not a secret.
///
/// # Errors
///
/// Returns [`CoreError::NotFound`] when the organisation row is absent, which
/// would mean the deployment was never bootstrapped.
pub async fn get_organisation(pool: &PgPool, organisation_id: Uuid) -> CoreResult<Organisation> {
    sqlx::query_as::<_, Organisation>(
        "SELECT id, slug, name, legal_name, country, description, created_at
           FROM organisations WHERE id = $1",
    )
    .bind(organisation_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| CoreError::NotFound("Organisation not found.".to_owned()))
}

/// List the units of the caller's organisation.
///
/// # Errors
///
/// Returns an error when the caller may not read, or the query fails.
pub async fn list_units(
    pool: &PgPool,
    principal: &Principal,
    include_archived: bool,
) -> CoreResult<Vec<Unit>> {
    let ctx = ResourceContext::organisation(ResourceKind::Unit, principal.organisation_id);
    authorize(principal, Action::Read, &ctx)
        .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    repo::list_units(pool, principal.organisation_id, include_archived).await
}

/// Load a unit the caller may read.
///
/// # Errors
///
/// Returns [`CoreError::NotFound`] when it does not exist or may not be read.
pub async fn get_unit<'e>(
    executor: impl sqlx::Executor<'e, Database = Postgres>,
    principal: &Principal,
    unit_id: Uuid,
) -> CoreResult<Unit> {
    let unit = repo::find_unit(executor, unit_id, principal.organisation_id)
        .await?
        .ok_or_else(|| CoreError::NotFound("Unit not found.".to_owned()))?;

    authorize(principal, Action::Read, &unit_context(&unit))
        .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;
    Ok(unit)
}

/// Create a unit.
///
/// # Errors
///
/// Returns an error when the caller may not create, the code is malformed, or
/// the code is already in use.
pub async fn create_unit(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    request: NewUnit,
) -> CoreResult<Unit> {
    let ctx = ResourceContext::organisation(ResourceKind::Unit, principal.organisation_id);
    authorize(principal, Action::Create, &ctx)
        .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    let code = validate_unit_code(&request.code)?;
    let name = request.name.trim();
    if name.is_empty() {
        return Err(CoreError::Validation("A unit needs a name.".to_owned()));
    }

    if repo::code_taken(&mut **tx, principal.organisation_id, &code).await? {
        return Err(CoreError::Conflict(
            "A unit with this code already exists.".to_owned(),
        ));
    }

    let unit = repo::insert_unit(
        &mut **tx,
        principal.organisation_id,
        &code,
        name,
        request.description.as_deref(),
        &request.research_areas,
        principal.person_id,
    )
    .await?;

    // Quem cria a unidade fica a poder geri-la, na mesma transacção.
    //
    // # Porque isto não é auto-elevação
    //
    // Porque é a autoridade **mínima** de que o domínio precisa para o recurso
    // não nascer ingovernável. Sem ela, criar uma unidade produzia uma unidade
    // que ninguém podia gerir: acrescentar membros exige `ManageMembers` no
    // contexto da unidade, e esse direito vem de ser Manager dela. Quem a criava
    // ficava de fora do que acabara de criar, e a única saída era escrever na
    // base por fora.
    //
    // Quem não pode criar unidades continua sem poder criar nenhuma: a
    // autorização acima não mudou. Isto não abre uma porta — fecha um beco.
    //
    // Na mesma transacção porque o estado intermédio «a unidade existe e não
    // tem quem a governe» não pode ser observável: se o commit falhar a seguir,
    // não fica uma unidade órfã.
    //
    // O ambiente de investigação já fazia isto — `research::create_idea` torna o
    // criador `Lead`. A unidade era a única que não fazia.
    repo::upsert_member(
        &mut **tx,
        unit.id,
        principal.person_id,
        UnitRole::Manager,
        principal.person_id,
    )
    .await?;

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::CREATE, "unit")
            .resource(unit.id)
            .scope(Some(unit.id), None)
            .detail("code", code.as_str()),
    )
    .await?;

    Ok(unit)
}

/// Archive a unit.
///
/// # Errors
///
/// Returns an error when the caller may not archive it.
pub async fn archive_unit(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    unit_id: Uuid,
) -> CoreResult<()> {
    let unit = repo::find_unit(&mut **tx, unit_id, principal.organisation_id)
        .await?
        .ok_or_else(|| CoreError::NotFound("Unit not found.".to_owned()))?;

    authorize(principal, Action::Archive, &unit_context(&unit))
        .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    repo::archive_unit(&mut **tx, unit.id, principal.person_id).await?;

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::ARCHIVE, "unit")
            .resource(unit.id)
            .scope(Some(unit.id), None),
    )
    .await?;
    Ok(())
}

/// List the live members of a unit.
///
/// # Errors
///
/// Returns an error when the caller may not read the unit.
pub async fn list_unit_members(
    pool: &PgPool,
    principal: &Principal,
    unit_id: Uuid,
) -> CoreResult<Vec<UnitMember>> {
    let unit = get_unit(pool, principal, unit_id).await?;
    repo::list_members(pool, unit.id).await
}

/// Add or update a unit membership.
///
/// # Errors
///
/// Returns an error when the caller may not manage members.
pub async fn add_unit_member(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    unit_id: Uuid,
    person_id: Uuid,
    role: UnitRole,
) -> CoreResult<Uuid> {
    let unit = repo::find_unit(&mut **tx, unit_id, principal.organisation_id)
        .await?
        .ok_or_else(|| CoreError::NotFound("Unit not found.".to_owned()))?;

    authorize(principal, Action::ManageMembers, &unit_context(&unit))
        .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    // The person must belong to this organisation. Without this check, a
    // membership could be granted to an identifier from anywhere.
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM people WHERE id = $1 AND organisation_id = $2)",
    )
    .bind(person_id)
    .bind(principal.organisation_id)
    .fetch_one(&mut **tx)
    .await?;
    if !exists {
        return Err(CoreError::NotFound("Person not found.".to_owned()));
    }

    let membership_id =
        repo::upsert_member(&mut **tx, unit.id, person_id, role, principal.person_id).await?;

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::MEMBERSHIP_CHANGE, "unit_membership")
            .resource(membership_id)
            .scope(Some(unit.id), None)
            .detail("person_id", person_id.to_string())
            .detail("role", role.as_str())
            .detail("event", "granted"),
    )
    .await?;

    Ok(membership_id)
}

/// Revoke a unit membership.
///
/// # Errors
///
/// Returns an error when the caller may not manage members, or none is live.
pub async fn revoke_unit_member(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    unit_id: Uuid,
    person_id: Uuid,
) -> CoreResult<()> {
    let unit = repo::find_unit(&mut **tx, unit_id, principal.organisation_id)
        .await?
        .ok_or_else(|| CoreError::NotFound("Unit not found.".to_owned()))?;

    authorize(principal, Action::ManageMembers, &unit_context(&unit))
        .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    // Uma unidade não pode ficar sem quem a governe.
    //
    // Gerir membros exige `ManageMembers` no contexto da unidade, e esse
    // direito vem de ser gestor dela. Remover o último gestor produziria uma
    // unidade que ninguém pode voltar a gerir — o mesmo beco que o bootstrap na
    // criação fechou, aberto pelo outro lado.
    //
    // A recusa é explícita e diz o que fazer, porque quem está a remover pode
    // legitimamente querer sair: nomeia-se outro gestor primeiro.
    let ultimo_gestor: bool = sqlx::query_scalar(
        "SELECT EXISTS (
             SELECT 1 FROM unit_memberships
              WHERE unit_id = $1 AND person_id = $2 AND role = 'manager'
         ) AND (
             SELECT count(*) FROM unit_memberships
              WHERE unit_id = $1 AND role = 'manager'
         ) = 1",
    )
    .bind(unit.id)
    .bind(person_id)
    .fetch_one(&mut **tx)
    .await?;

    if ultimo_gestor {
        return Err(CoreError::Conflict(
            "Esta é a última pessoa que gere a unidade. Nomeie outro gestor \
             antes de a remover."
                .to_owned(),
        ));
    }

    if !repo::revoke_member(&mut **tx, unit.id, person_id, principal.person_id).await? {
        return Err(CoreError::NotFound(
            "This membership does not exist.".to_owned(),
        ));
    }

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::MEMBERSHIP_CHANGE, "unit_membership")
            .scope(Some(unit.id), None)
            .detail("person_id", person_id.to_string())
            .detail("event", "revoked"),
    )
    .await?;
    Ok(())
}
