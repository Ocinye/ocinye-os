//! Organisation application layer.

use ocinye_contracts::{Classification, InstanceProfile, UnitRole};
use ocinye_domain::identifiers::{unit_code_stem, validate_unit_code};
use ocinye_domain::policy::{authorize, Action, ResourceContext, ResourceKind};
use ocinye_domain::Principal;
use ocinye_observability::CorrelationIds;
use sqlx::{PgPool, Postgres};
use uuid::Uuid;

use super::model::{Organisation, Unit, UnitMember};
use super::repository as repo;
use crate::audit::{self, action, AuditEntry};
use crate::error::{CoreError, CoreResult};
use crate::modules::search;
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
    /// Short code, for example `UCS-001`.
    ///
    /// `None` asks the Core to generate one from the name — the usual path, so a
    /// person creating a unit names it and never has to invent an identifier.
    /// `Some` supplies an explicit code, for seeding, import, or an administrator
    /// who wants a specific one; it is validated like any other.
    pub code: Option<String>,
    /// Display name.
    pub name: String,
    /// Description.
    pub description: Option<String>,
    /// Declared areas of research.
    pub research_areas: Vec<String>,
}

/// Build the search title and indexable text for a unit.
///
/// A unit is found by name **and** by code — an investigator searches for
/// «Inteligência Artificial» or for «UAI». The research areas are indexed too,
/// so a unit surfaces for the field it works in.
fn unit_index_text(unit: &Unit) -> (String, String) {
    let mut parts = vec![unit.code.clone(), unit.name.clone()];
    if let Some(description) = &unit.description {
        parts.push(description.clone());
    }
    parts.extend(unit.research_areas.iter().cloned());
    (unit.name.clone(), parts.join("\n"))
}

/// Index a unit in the transaction that created or changed it.
///
/// A unit's existence is `INTERNAL` and organisation-wide (every active member
/// sees the shape of the institution), so the row carries no owner, unit or
/// workspace scope — the `INTERNAL` clause admits it to every active member,
/// exactly as [`list_units`] does.
async fn index_unit(tx: &mut Tx<'_>, unit: &Unit) -> CoreResult<()> {
    let (title, text) = unit_index_text(unit);
    search::index_entity(
        tx,
        search::IndexRequest {
            organisation_id: unit.organisation_id,
            owner_id: None,
            unit_id: None,
            workspace_id: None,
            entity_type: "unit",
            entity_id: unit.id,
            title,
            text,
            classification: Classification::Internal,
        },
    )
    .await
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

/// Resolve the one Instance this installation serves (ADR-0013).
///
/// The order is fixed, and the Core never guesses:
///
/// 1. an explicit slug → adopt that organisation, or create it (named `name`,
///    or after its slug, with the given `profile`) when the database has no
///    Instance yet. Creating needs a profile: there is no default (ADR-0014);
/// 2. otherwise the Instance recorded in `instance_identity`;
/// 3. otherwise the only organisation in the database, if there is exactly one;
/// 4. otherwise refuse — an empty database needs an Instance to be named, and a
///    database with several organisations needs to be told which one.
///
/// Whatever is resolved is recorded, so the next start is deterministic. An
/// explicit slug that names a *different* organisation from the recorded one is
/// refused: that would be the installation silently changing whose it is.
///
/// # Errors
///
/// [`CoreError::Configuration`] when the Instance cannot be determined, or
/// contradicts the recorded one; database errors otherwise.
pub async fn resolve_instance(
    pool: &PgPool,
    explicit_slug: Option<&str>,
    name: Option<&str>,
    profile: Option<InstanceProfile>,
    ids: &CorrelationIds,
) -> CoreResult<Organisation> {
    let recorded = repo::recorded_instance(pool).await?;

    let organisation = match (explicit_slug, recorded) {
        (Some(slug), Some(recorded)) if recorded.slug != slug => {
            return Err(CoreError::Configuration(format!(
                "esta instalação serve a instância «{}», e a configuração pede «{slug}». \
                 Uma instalação não muda de instância por configuração.",
                recorded.slug
            )));
        }
        (_, Some(recorded)) => recorded,
        (Some(slug), None) => match repo::find_organisation_by_slug(pool, slug).await? {
            Some(existing) => existing,
            None => {
                let Some(profile) = profile else {
                    return Err(CoreError::Configuration(
                        "criar uma instância exige um perfil: research, business, \
                         education ou personal (OCINYE_INSTANCE_PROFILE, ou --profile)."
                            .to_owned(),
                    ));
                };
                let fallback = default_instance_name(slug);
                let created =
                    bootstrap_organisation(pool, slug, name.unwrap_or(&fallback), ids).await?;
                sqlx::query("UPDATE organisations SET profile = $2 WHERE id = $1")
                    .bind(created.id)
                    .bind(profile.as_str())
                    .execute(pool)
                    .await?;
                // A estrutura inicial é do perfil, e nasce com a Instância
                // (ADR-0014 §7) — venha ela do bootstrap ou do arranque do Core.
                if profile.seeds_initial_units() {
                    seed_initial_units(pool, created.id, ids).await?;
                }
                created
            }
        },
        (None, None) => {
            let mut existing = repo::first_organisations(pool).await?;
            match existing.len() {
                1 => existing.remove(0),
                0 => {
                    return Err(CoreError::Configuration(
                        "esta instalação ainda não tem instância. Defina \
                         OCINYE_INSTANCE_SLUG (e OCINYE_INSTANCE_NAME), ou crie-a com \
                         `ocinye-core-server bootstrap-admin`."
                            .to_owned(),
                    ));
                }
                _ => {
                    return Err(CoreError::Configuration(
                        "a base de dados tem várias organizações e nenhuma instância \
                         registada. Defina OCINYE_INSTANCE_SLUG com a desta instalação."
                            .to_owned(),
                    ));
                }
            }
        }
    };

    repo::record_instance(pool, organisation.id).await?;
    Ok(organisation)
}

/// The name of an Instance, as people read it (ADR-0013).
///
/// What used to be the literal «Ocinye» in the TOTP issuer, the mail signature
/// and the AI's system instruction.
///
/// # Errors
///
/// Returns an error when the query fails or the organisation does not exist.
pub async fn instance_name(pool: &PgPool, organisation_id: Uuid) -> CoreResult<String> {
    let name: String = sqlx::query_scalar("SELECT name FROM organisations WHERE id = $1")
        .bind(organisation_id)
        .fetch_one(pool)
        .await?;
    Ok(name)
}

/// The name an Instance gets when only its slug was given: readable, never
/// the slug itself (`mondrive-lda` → `Mondrive Lda`).
#[must_use]
pub fn default_instance_name(slug: &str) -> String {
    slug.split(['-', '_'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(chars).collect::<String>()
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The institution's initial units — sensible defaults, not a fixed taxonomy.
///
/// These are **seed data**: rows an installation starts with so «Nova Unidade»
/// is not an empty generic form on day one. They are not an enumeration that
/// business logic may branch on — nothing in the system does `if code ==
/// "UAI-001"`, and nothing should (§2, §7). An installation renames, archives,
/// or adds to them freely; a unit that did not exist when Ocinye was compiled is
/// as first-class as any of these.
///
/// The explicit `-001` codes are the institution's chosen identifiers, not what
/// the generator would produce (it would abbreviate «Inteligência Artificial»
/// to `UIA`, not `UAI`); the generator serves the units created afterwards.
const INITIAL_UNITS: &[(&str, &str, &str, &[&str])] = &[
    (
        "UAI-001",
        "Inteligência Artificial",
        "Investigação e engenharia de sistemas de inteligência artificial.",
        &[
            "Aprendizagem automática",
            "Processamento de linguagem natural",
            "Visão computacional",
        ],
    ),
    (
        "UCS-001",
        "Computação e Sistemas",
        "Sistemas computacionais, infraestrutura e engenharia de software.",
        &[
            "Sistemas distribuídos",
            "Computação de alto desempenho",
            "Engenharia de software",
        ],
    ),
    (
        "UDC-001",
        "Dados e Conhecimento",
        "Ciência de dados, gestão de conhecimento e recuperação de informação.",
        &[
            "Ciência de dados",
            "Bases de dados",
            "Recuperação de informação",
        ],
    ),
    (
        "UID-001",
        "Investigação e Desenvolvimento",
        "Investigação aplicada e transferência tecnológica transversais.",
        &[
            "Investigação aplicada",
            "Transferência tecnológica",
            "Prototipagem",
        ],
    ),
];

/// Seed the institution's initial units, if they are not already present.
///
/// Idempotent by design: each unit inserts only when its code is free
/// (`ON CONFLICT (organisation_id, code) DO NOTHING`), so running the bootstrap
/// again, or an installation that already created some of these, changes
/// nothing. A seeded unit has no human author (`created_by_id` null) and no
/// manager — an organisation administrator adopts it, appointing whoever leads
/// it. Each newly seeded unit is indexed for search in the same transaction.
///
/// This is the fresh-install path (a new organisation created by the bootstrap).
/// Existing organisations are seeded by the migration, which also backfills the
/// search index for every unit that predates unit indexing.
///
/// # Errors
///
/// Returns an error when a seed insert, its indexing, or the audit write fails.
pub async fn seed_initial_units(
    pool: &PgPool,
    organisation_id: Uuid,
    ids: &CorrelationIds,
) -> CoreResult<u32> {
    let mut seeded = 0_u32;
    for (code, name, description, areas) in INITIAL_UNITS {
        let areas: Vec<String> = areas.iter().map(|area| (*area).to_owned()).collect();

        let mut tx = pool.begin().await?;
        let Some(unit) = repo::insert_unit_if_absent(
            &mut *tx,
            organisation_id,
            code,
            name,
            Some(description),
            &areas,
        )
        .await?
        else {
            tx.rollback().await?;
            continue;
        };

        index_unit(&mut tx, &unit).await?;
        audit::record(
            &mut tx,
            None,
            ids,
            AuditEntry::new(action::ADMIN_OPERATION, "unit")
                .resource(unit.id)
                .scope(Some(unit.id), None)
                .detail("event", "seed")
                .detail("code", unit.code.as_str()),
        )
        .await?;
        tx.commit().await?;
        seeded += 1;
    }

    if seeded > 0 {
        tracing::info!(organisation_id = %organisation_id, seeded, "initial units seeded");
    }
    Ok(seeded)
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

/// Suggest the code a new unit with this name would receive.
///
/// For a live preview in the «Nova Unidade» form: someone types a name and sees
/// the institutional code that will be generated, before saving. Indicative —
/// the number is only fixed when the unit is created (a unit created in the
/// meantime shifts it), and the form says so. Requires the authority to create a
/// unit, since only that person needs the suggestion.
///
/// # Errors
///
/// Returns an error when the caller may not create a unit, or the query fails.
pub async fn suggest_unit_code(
    pool: &PgPool,
    principal: &Principal,
    name: &str,
) -> CoreResult<String> {
    let ctx = ResourceContext::organisation(ResourceKind::Unit, principal.organisation_id);
    authorize(principal, Action::Create, &ctx)
        .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    let stem = unit_code_stem(name.trim());
    repo::peek_unit_code(pool, principal.organisation_id, &stem).await
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

    let name = request.name.trim();
    if name.is_empty() {
        return Err(CoreError::Validation("A unit needs a name.".to_owned()));
    }

    // The code is either supplied explicitly (seed, import, an administrator who
    // wants a specific one) or generated from the name. Generation allocates
    // `U<ABBREV>-NNN` under a per-organisation lock, so the returned code is
    // already free; an explicit code is validated and checked for collision.
    let code = match request.code.as_deref().map(str::trim) {
        Some(explicit) if !explicit.is_empty() => {
            let code = validate_unit_code(explicit)?;
            if repo::code_taken(&mut **tx, principal.organisation_id, &code).await? {
                return Err(CoreError::Conflict(
                    "A unit with this code already exists.".to_owned(),
                ));
            }
            code
        }
        _ => {
            let stem = unit_code_stem(name);
            repo::next_unit_code(tx, principal.organisation_id, &stem).await?
        }
    };

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

    index_unit(tx, &unit).await?;

    Ok(unit)
}

/// Changes to a unit's editable fields.
#[derive(Debug, Clone)]
pub struct UnitEdit {
    /// New display name.
    pub name: String,
    /// New description.
    pub description: Option<String>,
    /// New declared areas of research.
    pub research_areas: Vec<String>,
}

/// Update a unit's name, description and research areas.
///
/// The **code does not change**: it is institutional identity that appears in
/// citations, and renaming a unit never renumbers it. Editing requires the same
/// authority as classifying or managing members — a unit manager, or an
/// organisation administrator.
///
/// # Errors
///
/// Returns an error when the caller may not manage the unit, the name is empty,
/// or the unit does not exist.
pub async fn update_unit(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    unit_id: Uuid,
    edit: UnitEdit,
) -> CoreResult<Unit> {
    let unit = repo::find_unit(&mut **tx, unit_id, principal.organisation_id)
        .await?
        .ok_or_else(|| CoreError::NotFound("Unit not found.".to_owned()))?;

    // Editing a unit is a membership-and-classification-grade change: the same
    // authority that may manage its members may edit it. `ManageMembers` on the
    // unit context is granted to a unit manager, a workspace lead, or an
    // organisation admin (ADR-0100) — never by title alone.
    authorize(principal, Action::ManageMembers, &unit_context(&unit))
        .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    let name = edit.name.trim();
    if name.is_empty() {
        return Err(CoreError::Validation("A unit needs a name.".to_owned()));
    }

    let updated = repo::update_unit(
        &mut **tx,
        unit.id,
        name,
        edit.description.as_deref(),
        &edit.research_areas,
        principal.person_id,
    )
    .await?;

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::UPDATE, "unit")
            .resource(updated.id)
            .scope(Some(updated.id), None)
            .detail("code", updated.code.as_str()),
    )
    .await?;

    index_unit(tx, &updated).await?;

    Ok(updated)
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

    // An archived unit leaves search: its existence is no longer current
    // structure. The row stays in `units` (archive-only, never deleted); the
    // finding aid simply stops pointing at it.
    search::remove_entity(tx, "unit", unit.id).await?;

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

/// A unit a person belongs to, reduced to what a list needs: its code and name.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PersonUnit {
    /// Short institutional code (e.g. `ESC`).
    pub code: String,
    /// Human name of the unit.
    pub name: String,
}

/// The live units of each person in a set, grouped by person.
///
/// One query for the whole page (never one per row). A person with no live
/// membership simply has no entry. The caller reads units to show *where a
/// member sits now* — the same institutional shape that `list_units` already
/// exposes to any active member (a unit's existence is `INTERNAL`), so this
/// reuses the `Read` authorization on units and adds no new visibility.
///
/// # Errors
///
/// Returns an error when the caller may not read units, or the query fails.
pub async fn units_for_people(
    pool: &PgPool,
    principal: &Principal,
    person_ids: &[Uuid],
) -> CoreResult<std::collections::HashMap<Uuid, Vec<PersonUnit>>> {
    let ctx = ResourceContext::organisation(ResourceKind::Unit, principal.organisation_id);
    authorize(principal, Action::Read, &ctx)
        .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    if person_ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }

    let rows = repo::units_of_people(pool, principal.organisation_id, person_ids).await?;
    let mut por_pessoa: std::collections::HashMap<Uuid, Vec<PersonUnit>> =
        std::collections::HashMap::new();
    for (person_id, code, name) in rows {
        por_pessoa
            .entry(person_id)
            .or_default()
            .push(PersonUnit { code, name });
    }
    Ok(por_pessoa)
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
