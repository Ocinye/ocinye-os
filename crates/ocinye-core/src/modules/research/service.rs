//! Research application layer.

use ocinye_contracts::{Classification, IdeaState, PageRequest, ProjectState, WorkspaceRole};
use ocinye_domain::identifiers::validate_project_code;
use ocinye_domain::policy::{authorize, Action, ResourceContext, ResourceKind, VisibilityFilter};
use ocinye_domain::workflow::{assert_idea_transition, assert_project_transition, PROMOTABLE_FROM};
use ocinye_domain::Principal;
use ocinye_observability::CorrelationIds;
use serde_json::json;
use sqlx::{PgPool, Postgres};
use uuid::Uuid;

use super::model::{Idea, Project, ResearchWorkspace, WorkspaceMember};
use super::repository as repo;
use crate::audit::{self, action, AuditEntry};
use crate::error::{CoreError, CoreResult};
use crate::modules::collaboration::{record_activity, ActivityKind};
use crate::modules::organisation;
use crate::modules::search;
use crate::outbox::{self, event};
use crate::Tx;

/// Authorization context for anything inside a research workspace.
///
/// Every artefact in a workspace derives its context from here, which is what
/// makes a single membership decision govern the whole research environment.
///
/// # This carries the *workspace's* classification
///
/// Correct for the workspace itself, and for asking «may this person work in
/// here at all». It is **not** the context in which to decide access to an
/// artefact that carries a classification of its own — use
/// [`artefact_context`] for those.
#[must_use]
pub fn workspace_context(workspace: &ResearchWorkspace, kind: ResourceKind) -> ResourceContext {
    ResourceContext::workspace(
        kind,
        workspace.organisation_id,
        workspace.unit_id,
        workspace.id,
        workspace.classification(),
    )
}

/// Authorization context for an artefact that carries its own classification.
///
/// # Why the stricter of the two governs
///
/// An artefact can sit **above** the workspace holding it, and does so by
/// design: `effective_classification` takes the most restrictive of the
/// requested classification and the workspace's, so a `RESTRICTED` note inside
/// an `INTERNAL` workspace is an ordinary, intended state. Reclassifying a
/// workspace downwards produces the same shape without anyone asking for it:
/// the workspace becomes `INTERNAL` and the material it already holds keeps the
/// classification it was given.
///
/// Deciding access to such an artefact against the *workspace's* classification
/// asks the wrong question, and answers it too generously. The listing side has
/// always used the artefact's own classification
/// ([`VisibilityFilter`](ocinye_domain::policy::VisibilityFilter)), so the two
/// sides disagreed: the row was hidden from a listing and returned by
/// identifier.
///
/// Taking the stricter of the two makes the decision at least as tight as
/// either input, and makes the direct read agree with the listing.
#[must_use]
pub fn artefact_context(
    workspace: &ResearchWorkspace,
    kind: ResourceKind,
    classification: Classification,
) -> ResourceContext {
    workspace_context(workspace, kind)
        .with_classification(workspace.classification().most_restrictive(classification))
}

/// Load an artefact's workspace **and** check the artefact's own classification.
///
/// The one function every read of a classified artefact goes through, so the
/// rule above cannot be forgotten by one module and remembered by four.
///
/// # Errors
///
/// Returns [`CoreError::NotFound`] when the workspace is not readable, or when
/// the artefact's own classification places it out of reach — the two are
/// deliberately indistinguishable.
pub async fn readable_artefact_workspace<'e>(
    executor: impl sqlx::Executor<'e, Database = Postgres>,
    principal: &Principal,
    workspace_id: Uuid,
    kind: ResourceKind,
    classification: Classification,
) -> CoreResult<ResearchWorkspace> {
    let workspace = get_workspace(executor, principal, workspace_id).await?;

    authorize(
        principal,
        Action::Read,
        &artefact_context(&workspace, kind, classification),
    )
    .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    Ok(workspace)
}

/// Load a workspace the caller may read.
///
/// # Errors
///
/// Returns [`CoreError::NotFound`] when absent or not readable — the two are
/// deliberately indistinguishable.
pub async fn get_workspace<'e>(
    executor: impl sqlx::Executor<'e, Database = Postgres>,
    principal: &Principal,
    workspace_id: Uuid,
) -> CoreResult<ResearchWorkspace> {
    let workspace = repo::find_workspace(executor, workspace_id, principal.organisation_id)
        .await?
        .ok_or_else(|| CoreError::NotFound("Research workspace not found.".to_owned()))?;

    authorize(
        principal,
        Action::Read,
        &workspace_context(&workspace, ResourceKind::ResearchWorkspace),
    )
    .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    Ok(workspace)
}

/// List readable workspaces.
///
/// # Errors
///
/// Returns an error when the query fails.
/// Lista os research workspaces que o principal pode ver.
///
/// `kind` distingue ideias de projectos. **Ausente significa ambos**, que é o
/// comportamento que os chamadores anteriores esperavam e continuam a ter.
///
/// A contagem usa exactamente o mesmo filtro que a listagem. Sem isso, o
/// número no ecrã e as linhas por baixo dele respondiam a perguntas
/// diferentes — foi assim que os contadores de Ideias e Projectos da Home
/// passaram a mostrar o mesmo total.
pub async fn list_workspaces(
    pool: &PgPool,
    principal: &Principal,
    query: repo::WorkspaceQuery<'_>,
    page: PageRequest,
) -> CoreResult<(Vec<ResearchWorkspace>, i64)> {
    let filter = VisibilityFilter::for_principal(principal);
    if filter.is_never_satisfiable() {
        return Ok((Vec::new(), 0));
    }

    let workspaces = repo::list_workspaces(
        pool,
        principal.organisation_id,
        &filter,
        query,
        page.limit(),
        page.offset(),
    )
    .await?;
    let total = repo::count_workspaces(pool, principal.organisation_id, &filter, query).await?;
    Ok((workspaces, total))
}

/// Details of a new idea.
#[derive(Debug, Clone)]
pub struct NewIdea {
    /// Unit the idea belongs to.
    pub unit_id: Uuid,
    /// Title.
    pub title: String,
    /// Summary.
    pub summary: Option<String>,
    /// The question being asked.
    pub research_question: Option<String>,
    /// The hypothesis, when one has formed.
    pub hypothesis: Option<String>,
    /// Why it matters.
    pub motivation: Option<String>,
    /// Keywords.
    pub keywords: Vec<String>,
    /// Requested classification. Never widens the unit's default.
    pub classification: Option<Classification>,
}

/// Create an idea together with its research workspace.
///
/// The workspace is created first because it is the authorization context for
/// everything the idea will accumulate. The author becomes workspace lead, so
/// the idea is workable the moment it exists.
///
/// # Errors
///
/// Returns an error when the caller may not create in that unit, or input is
/// invalid.
pub async fn create_idea(
    tx: &mut Tx<'_>,
    principal: &mut Principal,
    ids: &CorrelationIds,
    request: NewIdea,
) -> CoreResult<(Idea, ResearchWorkspace)> {
    let unit = organisation::get_unit(&mut **tx, principal, request.unit_id).await?;

    let classification = request.classification.unwrap_or(Classification::DEFAULT);
    let ctx = ResourceContext::unit(ResourceKind::Idea, unit.organisation_id, unit.id)
        .with_classification(classification);
    authorize(principal, Action::Create, &ctx)
        .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    let title = request.title.trim();
    if title.is_empty() {
        return Err(CoreError::Validation("An idea needs a title.".to_owned()));
    }

    let code = repo::next_workspace_code(
        &mut **tx,
        principal.organisation_id,
        unit.id,
        &unit.code,
        "IDEA",
    )
    .await?;

    let workspace = repo::insert_workspace(
        &mut **tx,
        principal.organisation_id,
        unit.id,
        &code,
        title,
        classification,
        principal.person_id,
    )
    .await?;

    repo::upsert_workspace_member(
        &mut **tx,
        workspace.id,
        principal.person_id,
        WorkspaceRole::Lead,
        principal.person_id,
    )
    .await?;

    // The caller must be able to act on the workspace it just created within
    // this same request, so its membership map is updated in place.
    principal
        .workspace_roles
        .insert(workspace.id, WorkspaceRole::Lead);

    let idea = repo::insert_idea(
        &mut **tx,
        workspace.id,
        title,
        request.summary.as_deref(),
        request.research_question.as_deref(),
        request.hypothesis.as_deref(),
        request.motivation.as_deref(),
        &request.keywords,
        principal.person_id,
    )
    .await?;

    let indexed_text = [
        request.summary.as_deref(),
        request.research_question.as_deref(),
        request.hypothesis.as_deref(),
        request.motivation.as_deref(),
    ]
    .into_iter()
    .flatten()
    .chain(request.keywords.iter().map(String::as_str))
    .collect::<Vec<_>>()
    .join("\n");

    search::index_entity(
        tx,
        search::IndexRequest {
            organisation_id: principal.organisation_id,
            unit_id: Some(unit.id),
            workspace_id: Some(workspace.id),
            entity_type: "idea",
            entity_id: idea.id,
            title: title.to_owned(),
            text: indexed_text,
            classification,
        },
    )
    .await?;

    outbox::emit(
        tx,
        event::WORKSPACE_CREATED,
        "research_workspace",
        workspace.id,
        &ids.correlation_id,
        json!({ "unit_id": unit.id, "kind": "idea" }),
    )
    .await?;
    outbox::emit(
        tx,
        event::IDEA_CREATED,
        "idea",
        idea.id,
        &ids.correlation_id,
        json!({ "workspace_id": workspace.id, "unit_id": unit.id }),
    )
    .await?;

    record_activity(
        tx,
        principal,
        workspace.id,
        unit.id,
        ActivityKind::Created,
        "idea",
        Some(idea.id),
        &format!("Idea created: {title}"),
        classification,
    )
    .await?;

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::CREATE, "idea")
            .resource(idea.id)
            .context(&workspace_context(&workspace, ResourceKind::Idea))
            .detail("workspace_code", code.as_str()),
    )
    .await?;

    Ok((idea, workspace))
}

/// Load an idea together with its workspace.
///
/// # Errors
///
/// Returns [`CoreError::NotFound`] when absent or not readable.
pub async fn get_idea(
    pool: &PgPool,
    principal: &Principal,
    idea_id: Uuid,
) -> CoreResult<(Idea, ResearchWorkspace)> {
    let idea = repo::find_idea(pool, idea_id)
        .await?
        .ok_or_else(|| CoreError::NotFound("Idea not found.".to_owned()))?;
    let workspace = get_workspace(pool, principal, idea.workspace_id).await?;
    Ok((idea, workspace))
}

/// The descriptive fields of an idea a member may revise.
///
/// # Why a command and not the entity
///
/// Taking an `Idea` and writing it back would let any caller set `state`,
/// `workspace_id` or `promoted_project_id` — three things the domain decides
/// and nobody edits. A command names exactly what may change, so the type is
/// the specification (`CLAUDE.md` §53, briefing §12).
///
/// Every field is optional and means «leave alone». Clearing one is done by
/// sending it empty, which normalises to `NULL`; that keeps «unset this» and
/// «do not touch this» distinguishable, which they are.
#[derive(Debug, Clone, Default)]
pub struct IdeaRevision {
    /// New title. Refused when it trims to nothing: an idea without a title is
    /// unfindable, and silently keeping the old one would be a lie.
    pub title: Option<String>,
    /// What the idea is about.
    pub summary: Option<String>,
    /// The question being asked.
    pub research_question: Option<String>,
    /// What is believed, and why it is worth testing.
    pub hypothesis: Option<String>,
    /// Why the institution should care.
    pub motivation: Option<String>,
    /// Terms that make it findable.
    pub keywords: Option<Vec<String>>,
}

impl IdeaRevision {
    /// Whether this revision asks for anything at all.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.title.is_none()
            && self.summary.is_none()
            && self.research_question.is_none()
            && self.hypothesis.is_none()
            && self.motivation.is_none()
            && self.keywords.is_none()
    }
}

/// Longest a free-text field of an idea may be.
///
/// Generous for a research question and finite, which a value arriving from a
/// model is not.
const MAX_IDEA_TEXT: usize = 8_000;

/// Revise the descriptive fields of an idea.
///
/// # What this does not do
///
/// It does not move the lifecycle — that is [`transition_idea`], and the
/// workflow decides. It does not reclassify, does not change ownership, and
/// does not touch provenance.
///
/// # Errors
///
/// Returns an error when the caller may not write in the idea's workspace, when
/// the revision is empty, when a field exceeds its bound, or when a new title
/// is blank.
pub async fn update_idea(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    idea_id: Uuid,
    revision: IdeaRevision,
) -> CoreResult<(Idea, ResearchWorkspace)> {
    // Loaded the way `transition_idea` loads it: an idea carries no
    // classification of its own — the workspace it *is* carries it — so the
    // workspace read is the whole gate.
    let existing = repo::find_idea(&mut **tx, idea_id)
        .await?
        .ok_or_else(|| CoreError::NotFound("Idea not found.".to_owned()))?;
    let workspace = get_workspace(&mut **tx, principal, existing.workspace_id).await?;

    let ctx = workspace_context(&workspace, ResourceKind::Idea);
    authorize(principal, Action::Update, &ctx)
        .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    if revision.is_empty() {
        return Err(CoreError::Validation(
            "Indique o que pretende alterar.".to_owned(),
        ));
    }

    // A promoted idea is history: it named a project, and rewriting what it
    // said afterwards would rewrite where that project came from.
    if existing.state() == IdeaState::Promoted {
        return Err(CoreError::Validation(
            "Uma ideia promovida já não é editável: o Projecto que dela nasceu \
             aponta para o que ela dizia."
                .to_owned(),
        ));
    }

    let bounded = |value: &str, field: &str| -> CoreResult<()> {
        if value.chars().count() > MAX_IDEA_TEXT {
            return Err(CoreError::Validation(format!(
                "O campo «{field}» excede {MAX_IDEA_TEXT} caracteres."
            )));
        }
        Ok(())
    };

    let title = match revision.title.as_deref().map(str::trim) {
        None => None,
        Some("") => {
            return Err(CoreError::Validation(
                "O título de uma ideia não pode ficar vazio.".to_owned(),
            ))
        }
        Some(value) => {
            bounded(value, "title")?;
            Some(value.to_owned())
        }
    };

    // An empty string clears the field; that is the difference between «unset»
    // and «leave alone», and both have to be expressible.
    let optional = |value: Option<String>, field: &str| -> CoreResult<Option<String>> {
        match value.as_deref().map(str::trim) {
            None => Ok(None),
            Some("") => Ok(Some(String::new())),
            Some(value) => {
                bounded(value, field)?;
                Ok(Some(value.to_owned()))
            }
        }
    };

    let summary = optional(revision.summary, "summary")?;
    let research_question = optional(revision.research_question, "research_question")?;
    let hypothesis = optional(revision.hypothesis, "hypothesis")?;
    let motivation = optional(revision.motivation, "motivation")?;

    let keywords = revision.keywords.map(|words| {
        words
            .into_iter()
            .map(|word| word.trim().to_owned())
            .filter(|word| !word.is_empty())
            .take(32)
            .collect::<Vec<_>>()
    });

    let updated = repo::update_idea_fields(
        &mut **tx,
        existing.id,
        title.as_deref(),
        summary.as_deref(),
        research_question.as_deref(),
        hypothesis.as_deref(),
        motivation.as_deref(),
        keywords.as_deref(),
        principal.person_id,
    )
    .await?;

    let indexed_text = [
        updated.summary.as_deref(),
        updated.research_question.as_deref(),
        updated.hypothesis.as_deref(),
        updated.motivation.as_deref(),
    ]
    .into_iter()
    .flatten()
    .chain(updated.keywords.iter().map(String::as_str))
    .collect::<Vec<_>>()
    .join("\n");

    search::index_entity(
        tx,
        search::IndexRequest {
            organisation_id: principal.organisation_id,
            unit_id: Some(workspace.unit_id),
            workspace_id: Some(workspace.id),
            entity_type: "idea",
            entity_id: updated.id,
            title: updated.title.clone(),
            text: indexed_text,
            classification: workspace.classification(),
        },
    )
    .await?;

    record_activity(
        tx,
        principal,
        workspace.id,
        workspace.unit_id,
        ActivityKind::Updated,
        "idea",
        Some(updated.id),
        &format!("Idea revised: {}", updated.title),
        workspace.classification(),
    )
    .await?;

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::UPDATE, "idea")
            .resource(updated.id)
            .context(&ctx)
            // Which fields moved, never what they now say: an idea's text is
            // the member's own words (briefing §48).
            .detail(
                "fields",
                changed_fields(&revision_shape(
                    &title,
                    &summary,
                    &research_question,
                    &hypothesis,
                    &motivation,
                    &keywords,
                )),
            ),
    )
    .await?;

    Ok((updated, workspace))
}

/// The shape of what a revision touched, for the audit trail.
fn revision_shape(
    title: &Option<String>,
    summary: &Option<String>,
    research_question: &Option<String>,
    hypothesis: &Option<String>,
    motivation: &Option<String>,
    keywords: &Option<Vec<String>>,
) -> [(&'static str, bool); 6] {
    [
        ("title", title.is_some()),
        ("summary", summary.is_some()),
        ("research_question", research_question.is_some()),
        ("hypothesis", hypothesis.is_some()),
        ("motivation", motivation.is_some()),
        ("keywords", keywords.is_some()),
    ]
}

/// Name the fields a revision touched.
fn changed_fields(shape: &[(&'static str, bool)]) -> String {
    shape
        .iter()
        .filter(|(_, touched)| *touched)
        .map(|(name, _)| *name)
        .collect::<Vec<_>>()
        .join(",")
}

/// Move an idea through its lifecycle.
///
/// # Errors
///
/// Returns an error when the caller may not transition it, or the lifecycle
/// forbids the move.
pub async fn transition_idea(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    idea_id: Uuid,
    target: IdeaState,
    outcome_note: Option<&str>,
) -> CoreResult<Idea> {
    let idea = repo::find_idea(&mut **tx, idea_id)
        .await?
        .ok_or_else(|| CoreError::NotFound("Idea not found.".to_owned()))?;
    let workspace = get_workspace(&mut **tx, principal, idea.workspace_id).await?;

    let ctx = workspace_context(&workspace, ResourceKind::Idea);
    authorize(principal, Action::Transition, &ctx)
        .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    let current = idea.state();
    assert_idea_transition(current, target, outcome_note)?;

    repo::update_idea_state(
        &mut **tx,
        idea.id,
        target,
        outcome_note,
        principal.person_id,
    )
    .await?;

    outbox::emit_transition(
        tx,
        event::IDEA_STATE_CHANGED,
        "idea",
        idea.id,
        &ids.correlation_id,
        current.as_str(),
        target.as_str(),
    )
    .await?;

    record_activity(
        tx,
        principal,
        workspace.id,
        workspace.unit_id,
        ActivityKind::StateChanged,
        "idea",
        Some(idea.id),
        &format!(
            "Idea moved from {} to {}",
            current.as_str(),
            target.as_str()
        ),
        workspace.classification(),
    )
    .await?;

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::TRANSITION, "idea")
            .resource(idea.id)
            .context(&ctx)
            .detail("from", current.as_str())
            .detail("to", target.as_str()),
    )
    .await?;

    repo::find_idea(&mut **tx, idea.id)
        .await?
        .ok_or_else(|| CoreError::Internal("idea vanished during transition".to_owned()))
}

/// Details of a promotion.
#[derive(Debug, Clone)]
pub struct Promotion {
    /// Institutional project code.
    pub code: String,
    /// Title, defaulting to the idea's.
    pub title: Option<String>,
    /// Objectives of the project.
    pub objectives: Option<String>,
    /// Person accountable, defaulting to the promoter.
    pub responsible_person_id: Option<Uuid>,
}

/// Promote a project candidate into a formal project.
///
/// The workspace carries over rather than being recreated, so every source,
/// note, document and dataset gathered while exploring stays attached and the
/// lineage is recorded on both sides.
///
/// # Errors
///
/// Returns an error when the idea is not a candidate, has already been
/// promoted, the code is taken, or the caller may not promote.
pub async fn promote_idea(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    idea_id: Uuid,
    request: Promotion,
) -> CoreResult<Project> {
    let idea = repo::find_idea(&mut **tx, idea_id)
        .await?
        .ok_or_else(|| CoreError::NotFound("Idea not found.".to_owned()))?;
    let workspace = get_workspace(&mut **tx, principal, idea.workspace_id).await?;

    let ctx = workspace_context(&workspace, ResourceKind::Project);
    authorize(principal, Action::Transition, &ctx)
        .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    if idea.state() != PROMOTABLE_FROM {
        return Err(CoreError::Conflict(
            "Only an idea in 'project_candidate' may be promoted to a project.".to_owned(),
        ));
    }
    if idea.promoted_project_id.is_some() {
        return Err(CoreError::Conflict(
            "This idea has already been promoted.".to_owned(),
        ));
    }

    let code = validate_project_code(&request.code)?;
    if repo::project_code_taken(&mut **tx, principal.organisation_id, &code).await? {
        return Err(CoreError::Conflict(
            "A project with this code already exists.".to_owned(),
        ));
    }

    let title = request
        .title
        .as_deref()
        .unwrap_or(&idea.title)
        .trim()
        .to_owned();

    let project = repo::insert_project(
        &mut **tx,
        principal.organisation_id,
        workspace.id,
        &code,
        &title,
        idea.summary.as_deref(),
        request.objectives.as_deref(),
        Some(idea.id),
        request.responsible_person_id.unwrap_or(principal.person_id),
        principal.person_id,
    )
    .await?;

    repo::mark_idea_promoted(&mut **tx, idea.id, project.id, principal.person_id).await?;
    repo::mark_workspace_as_project(&mut **tx, workspace.id, principal.person_id).await?;

    search::index_entity(
        tx,
        search::IndexRequest {
            organisation_id: principal.organisation_id,
            unit_id: Some(workspace.unit_id),
            workspace_id: Some(workspace.id),
            entity_type: "project",
            entity_id: project.id,
            title: title.clone(),
            text: [
                idea.summary.as_deref(),
                request.objectives.as_deref(),
                Some(code.as_str()),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join("\n"),
            classification: workspace.classification(),
        },
    )
    .await?;

    outbox::emit(
        tx,
        event::PROJECT_CREATED,
        "project",
        project.id,
        &ids.correlation_id,
        json!({ "origin_idea_id": idea.id, "workspace_id": workspace.id, "code": code }),
    )
    .await?;

    record_activity(
        tx,
        principal,
        workspace.id,
        workspace.unit_id,
        ActivityKind::StateChanged,
        "project",
        Some(project.id),
        &format!("Idea promoted to project {code}"),
        workspace.classification(),
    )
    .await?;

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::APPROVE, "project")
            .resource(project.id)
            .context(&ctx)
            .detail("origin_idea_id", idea.id.to_string())
            .detail("code", code.as_str()),
    )
    .await?;

    Ok(project)
}

/// Load a project together with its workspace.
///
/// # Errors
///
/// Returns [`CoreError::NotFound`] when absent or not readable.
pub async fn get_project(
    pool: &PgPool,
    principal: &Principal,
    project_id: Uuid,
) -> CoreResult<(Project, ResearchWorkspace)> {
    let project = repo::find_project(pool, project_id, principal.organisation_id)
        .await?
        .ok_or_else(|| CoreError::NotFound("Project not found.".to_owned()))?;
    let workspace = get_workspace(pool, principal, project.workspace_id).await?;
    Ok((project, workspace))
}

/// Move a project through its lifecycle.
///
/// # Errors
///
/// Returns an error when the caller may not transition it, or the lifecycle
/// forbids the move.
pub async fn transition_project(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    project_id: Uuid,
    target: ProjectState,
) -> CoreResult<Project> {
    let project = repo::find_project(&mut **tx, project_id, principal.organisation_id)
        .await?
        .ok_or_else(|| CoreError::NotFound("Project not found.".to_owned()))?;
    let workspace = get_workspace(&mut **tx, principal, project.workspace_id).await?;

    let ctx = workspace_context(&workspace, ResourceKind::Project);
    authorize(principal, Action::Transition, &ctx)
        .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    let current = project.state();
    assert_project_transition(current, target)?;

    repo::update_project_state(&mut **tx, project.id, target, principal.person_id).await?;

    outbox::emit_transition(
        tx,
        event::PROJECT_STATE_CHANGED,
        "project",
        project.id,
        &ids.correlation_id,
        current.as_str(),
        target.as_str(),
    )
    .await?;

    record_activity(
        tx,
        principal,
        workspace.id,
        workspace.unit_id,
        ActivityKind::StateChanged,
        "project",
        Some(project.id),
        &format!(
            "Project moved from {} to {}",
            current.as_str(),
            target.as_str()
        ),
        workspace.classification(),
    )
    .await?;

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::TRANSITION, "project")
            .resource(project.id)
            .context(&ctx)
            .detail("from", current.as_str())
            .detail("to", target.as_str()),
    )
    .await?;

    repo::find_project(&mut **tx, project.id, principal.organisation_id)
        .await?
        .ok_or_else(|| CoreError::Internal("project vanished during transition".to_owned()))
}

/// Add or update a workspace membership.
///
/// # Errors
///
/// Returns an error when the caller may not manage members.
pub async fn add_workspace_member(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    workspace_id: Uuid,
    person_id: Uuid,
    role: WorkspaceRole,
) -> CoreResult<Uuid> {
    let workspace = get_workspace(&mut **tx, principal, workspace_id).await?;
    let ctx = workspace_context(&workspace, ResourceKind::ResearchWorkspace);
    authorize(principal, Action::ManageMembers, &ctx)
        .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

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

    let membership_id = repo::upsert_workspace_member(
        &mut **tx,
        workspace.id,
        person_id,
        role,
        principal.person_id,
    )
    .await?;

    outbox::emit(
        tx,
        event::WORKSPACE_MEMBER_ADDED,
        "research_workspace",
        workspace.id,
        &ids.correlation_id,
        json!({ "person_id": person_id, "role": role.as_str() }),
    )
    .await?;

    record_activity(
        tx,
        principal,
        workspace.id,
        workspace.unit_id,
        ActivityKind::MemberAdded,
        "workspace_membership",
        Some(membership_id),
        "A member was added to the workspace",
        workspace.classification(),
    )
    .await?;

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::MEMBERSHIP_CHANGE, "workspace_membership")
            .resource(membership_id)
            .context(&ctx)
            .detail("person_id", person_id.to_string())
            .detail("role", role.as_str()),
    )
    .await?;

    Ok(membership_id)
}

/// Change a workspace's classification.
///
/// # Errors
///
/// Returns an error when the caller may not classify, or no reason is given.
pub async fn reclassify_workspace(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    workspace_id: Uuid,
    classification: Classification,
    reason: &str,
) -> CoreResult<()> {
    let workspace = get_workspace(&mut **tx, principal, workspace_id).await?;
    let ctx = workspace_context(&workspace, ResourceKind::ResearchWorkspace);
    authorize(principal, Action::Classify, &ctx)
        .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    if reason.trim().is_empty() {
        return Err(CoreError::Validation(
            "Changing a classification requires a reason.".to_owned(),
        ));
    }

    let previous = workspace.classification();
    if previous == classification {
        return Ok(());
    }

    repo::set_workspace_classification(
        &mut **tx,
        workspace.id,
        classification,
        principal.person_id,
    )
    .await?;

    outbox::emit(
        tx,
        event::CLASSIFICATION_CHANGED,
        "research_workspace",
        workspace.id,
        &ids.correlation_id,
        json!({ "from": previous.as_str(), "to": classification.as_str() }),
    )
    .await?;

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::CLASSIFY, "research_workspace")
            .resource(workspace.id)
            .context(&ctx)
            .classified(classification)
            .detail("from", previous.as_str())
            .detail("to", classification.as_str())
            .detail(
                "reason",
                reason.trim().chars().take(200).collect::<String>(),
            ),
    )
    .await?;

    Ok(())
}

/// Everything the Research Workspace screen needs in one authorised read.
///
/// A workspace holds either an idea or a project — after promotion it holds
/// both, because the lineage is kept.
#[derive(Debug, Clone)]
pub struct WorkspaceOverview {
    /// The workspace itself.
    pub workspace: ResearchWorkspace,
    /// The idea, when the workspace started as one.
    pub idea: Option<Idea>,
    /// The project, once the idea has been promoted.
    pub project: Option<Project>,
    /// Current members.
    pub members: Vec<WorkspaceMember>,
}

/// Load the contextual overview of a research workspace.
///
/// One authorization check governs the whole environment, which is what lets a
/// researcher stay inside the scientific context instead of re-entering it per
/// artefact (briefing §22, §25).
///
/// # Errors
///
/// Returns [`CoreError::NotFound`] when absent or not readable.
pub async fn get_workspace_overview(
    pool: &PgPool,
    principal: &Principal,
    workspace_id: Uuid,
) -> CoreResult<WorkspaceOverview> {
    let workspace = get_workspace(pool, principal, workspace_id).await?;

    Ok(WorkspaceOverview {
        idea: repo::find_idea_by_workspace(pool, workspace.id).await?,
        project: repo::find_project_by_workspace(pool, workspace.id).await?,
        members: repo::list_workspace_members(pool, workspace.id).await?,
        workspace,
    })
}
