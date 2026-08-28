//! Research persistence.

use ocinye_contracts::{Classification, IdeaState, ProjectState, WorkspaceKind, WorkspaceRole};
use ocinye_domain::policy::VisibilityFilter;
use sqlx::PgExecutor;
use uuid::Uuid;

use super::model::{Idea, Project, ResearchWorkspace, WorkspaceMember};
use crate::error::CoreResult;
use crate::visibility::{to_sql, VisibilityColumns};

const WORKSPACE_COLUMNS: &str = "id, organisation_id, unit_id, code, title, kind,
                                 classification, archived_at, created_at, updated_at";
const IDEA_COLUMNS: &str = "id, workspace_id, title, summary, research_question, hypothesis,
                            motivation, keywords, state, outcome_note, promoted_project_id,
                            created_at, updated_at";
const PROJECT_COLUMNS: &str = "id, organisation_id, workspace_id, code, title, summary,
                               objectives, state, origin_idea_id, responsible_person_id,
                               started_at, completed_at, created_at, updated_at";

/// Columns of `research_workspaces` as seen by the visibility filter.
///
/// A workspace is its own scope: its `id` is the workspace column.
const WORKSPACE_VISIBILITY: VisibilityColumns =
    VisibilityColumns::aliased("unit_id", "id", "classification");

/// Load a workspace by identifier.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn find_workspace<'e>(
    executor: impl PgExecutor<'e>,
    id: Uuid,
    organisation_id: Uuid,
) -> CoreResult<Option<ResearchWorkspace>> {
    let workspace = sqlx::query_as::<_, ResearchWorkspace>(&format!(
        "SELECT {WORKSPACE_COLUMNS} FROM research_workspaces
          WHERE id = $1 AND organisation_id = $2"
    ))
    .bind(id)
    .bind(organisation_id)
    .fetch_optional(executor)
    .await?;
    Ok(workspace)
}

/// List workspaces the principal may read.
///
/// The authorization predicate is part of the statement, so `LIMIT`, `OFFSET`
/// and the companion count all operate on the authorised set only.
///
/// # Errors
///
/// Returns an error when the query fails.
/// Restringe a workspaces cuja ideia está em estado promovível.
///
/// O selector de «Novo Projecto» precisa de oferecer apenas ideias que a
/// promoção aceitaria. Filtrar aqui evita trazer todas e perguntar o estado de
/// cada uma — o N+1 que a lista faria crescer com a instituição.
///
/// Não substitui a validação: `promote_idea` volta a verificar o estado, e é lá
/// que a garantia vive. Isto é o que a interface oferece, não o que o Core
/// permite.
fn promotable_predicate(only: bool) -> &'static str {
    if only {
        "EXISTS (
           SELECT 1 FROM ideas i
            WHERE i.workspace_id = research_workspaces.id
              AND i.state = 'project_candidate'
              AND i.promoted_project_id IS NULL
         )"
    } else {
        "TRUE"
    }
}

/// Os recortes de uma listagem de research workspaces.
///
/// Agrupados num tipo porque são um conceito só — «que subconjunto» — e porque
/// três parâmetros booleanos e opcionais em fila trocam-se sem o compilador
/// reparar.
#[derive(Debug, Clone, Copy, Default)]
pub struct WorkspaceQuery<'a> {
    /// Restringe a uma unidade.
    pub unit_id: Option<Uuid>,
    /// Restringe a ideias ou a projectos. Ausente devolve ambos.
    pub kind: Option<WorkspaceKind>,
    /// Restringe a ideias que a promoção aceitaria hoje.
    pub promotable_only: bool,
    /// Restringe aos ambientes onde o membro tem papel.
    ///
    /// Distinto de visibilidade: **ver** um Research Workspace e **participar**
    /// nele são coisas diferentes, e um ecrã que promete «a investigação em que
    /// participo» não pode responder com tudo o que o membro alcança.
    ///
    /// Uma lista vazia significa «participo em nenhum», e devolve nada — não
    /// tudo. `None` é que significa «sem este recorte».
    pub member_of: Option<&'a [Uuid]>,
}

/// Restringe aos ambientes indicados, quando o recorte é pedido.
///
/// Uma lista vazia rende `FALSE`, e é a resposta certa: quem não participa em
/// nenhum ambiente não participa em nenhum. Deixar passar tudo nesse caso seria
/// o erro clássico de um `IN ()` vazio tratado como «sem filtro».
fn membership_predicate(ids: Option<&[Uuid]>) -> String {
    match ids {
        None => "TRUE".to_owned(),
        Some([]) => "FALSE".to_owned(),
        Some(ids) => {
            let lista = ids
                .iter()
                .map(|id| format!("'{id}'"))
                .collect::<Vec<_>>()
                .join(", ");
            format!("id IN ({lista})")
        }
    }
}

pub async fn list_workspaces<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    filter: &VisibilityFilter,
    query: WorkspaceQuery<'_>,
    limit: i64,
    offset: i64,
) -> CoreResult<Vec<ResearchWorkspace>> {
    let predicate = to_sql(filter, WORKSPACE_VISIBILITY);
    let promotable = promotable_predicate(query.promotable_only);
    let participacao = membership_predicate(query.member_of);
    let workspaces = sqlx::query_as::<_, ResearchWorkspace>(&format!(
        "SELECT {WORKSPACE_COLUMNS} FROM research_workspaces
          WHERE organisation_id = $1
            AND ($2::uuid IS NULL OR unit_id = $2)
            AND ($3::text IS NULL OR kind = $3)
            AND {promotable}
            AND {participacao}
            AND {predicate}
          ORDER BY created_at DESC
          LIMIT $4 OFFSET $5"
    ))
    .bind(organisation_id)
    .bind(query.unit_id)
    .bind(query.kind.map(WorkspaceKind::as_str))
    .bind(limit)
    .bind(offset)
    .fetch_all(executor)
    .await?;
    Ok(workspaces)
}

/// Count workspaces the principal may read.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn count_workspaces<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    filter: &VisibilityFilter,
    query: WorkspaceQuery<'_>,
) -> CoreResult<i64> {
    let predicate = to_sql(filter, WORKSPACE_VISIBILITY);
    let promotable = promotable_predicate(query.promotable_only);
    let participacao = membership_predicate(query.member_of);
    let total = sqlx::query_scalar::<_, i64>(&format!(
        "SELECT COUNT(*) FROM research_workspaces
          WHERE organisation_id = $1
            AND ($2::uuid IS NULL OR unit_id = $2)
            AND ($3::text IS NULL OR kind = $3)
            AND {promotable}
            AND {participacao}
            AND {predicate}"
    ))
    .bind(organisation_id)
    .bind(query.unit_id)
    .bind(query.kind.map(WorkspaceKind::as_str))
    .fetch_one(executor)
    .await?;
    Ok(total)
}

/// Next workspace code within a unit, for example `AI-IDEA-004`.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn next_workspace_code<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    unit_id: Uuid,
    unit_code: &str,
    prefix: &str,
) -> CoreResult<String> {
    let used = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM research_workspaces
          WHERE organisation_id = $1 AND unit_id = $2",
    )
    .bind(organisation_id)
    .bind(unit_id)
    .fetch_one(executor)
    .await?;
    Ok(format!("{unit_code}-{prefix}-{:03}", used + 1))
}

/// Insert a workspace.
///
/// # Errors
///
/// Returns an error when the insert fails.
pub async fn insert_workspace<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    unit_id: Uuid,
    code: &str,
    title: &str,
    classification: Classification,
    created_by: Uuid,
) -> CoreResult<ResearchWorkspace> {
    let workspace = sqlx::query_as::<_, ResearchWorkspace>(&format!(
        "INSERT INTO research_workspaces
             (organisation_id, unit_id, code, title, kind, classification, created_by_id)
         VALUES ($1, $2, $3, $4, 'idea', $5, $6)
         RETURNING {WORKSPACE_COLUMNS}"
    ))
    .bind(organisation_id)
    .bind(unit_id)
    .bind(code)
    .bind(title)
    .bind(classification.as_str())
    .bind(created_by)
    .fetch_one(executor)
    .await?;
    Ok(workspace)
}

/// Change a workspace's classification.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn set_workspace_classification<'e>(
    executor: impl PgExecutor<'e>,
    workspace_id: Uuid,
    classification: Classification,
    updated_by: Uuid,
) -> CoreResult<()> {
    sqlx::query(
        "UPDATE research_workspaces
            SET classification = $2, updated_by_id = $3, updated_at = now()
          WHERE id = $1",
    )
    .bind(workspace_id)
    .bind(classification.as_str())
    .bind(updated_by)
    .execute(executor)
    .await?;
    Ok(())
}

/// Mark a workspace as now hosting a project.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn mark_workspace_as_project<'e>(
    executor: impl PgExecutor<'e>,
    workspace_id: Uuid,
    updated_by: Uuid,
) -> CoreResult<()> {
    sqlx::query(
        "UPDATE research_workspaces
            SET kind = 'project', updated_by_id = $2, updated_at = now()
          WHERE id = $1",
    )
    .bind(workspace_id)
    .bind(updated_by)
    .execute(executor)
    .await?;
    Ok(())
}

/// Grant or restore a workspace membership.
///
/// # Errors
///
/// Returns an error when the upsert fails.
pub async fn upsert_workspace_member<'e>(
    executor: impl PgExecutor<'e>,
    workspace_id: Uuid,
    person_id: Uuid,
    role: WorkspaceRole,
    actor: Uuid,
) -> CoreResult<Uuid> {
    let id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO workspace_memberships (workspace_id, person_id, role, created_by_id)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (workspace_id, person_id) DO UPDATE
            SET role = EXCLUDED.role, revoked_at = NULL,
                updated_by_id = EXCLUDED.created_by_id, updated_at = now()
         RETURNING id",
    )
    .bind(workspace_id)
    .bind(person_id)
    .bind(role.as_str())
    .bind(actor)
    .fetch_one(executor)
    .await?;
    Ok(id)
}

/// List live workspace members.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn list_workspace_members<'e>(
    executor: impl PgExecutor<'e>,
    workspace_id: Uuid,
) -> CoreResult<Vec<WorkspaceMember>> {
    let members = sqlx::query_as::<_, WorkspaceMember>(
        "SELECT m.id, m.workspace_id, m.person_id, p.full_name, m.role, m.created_at
           FROM workspace_memberships m
           JOIN people p ON p.id = m.person_id
          WHERE m.workspace_id = $1 AND m.revoked_at IS NULL
          ORDER BY p.full_name",
    )
    .bind(workspace_id)
    .fetch_all(executor)
    .await?;
    Ok(members)
}

/// Insert an idea.
///
/// # Errors
///
/// Returns an error when the insert fails.
#[expect(
    clippy::too_many_arguments,
    reason = "um parâmetro por coluna: a alternativa é uma struct que só existe para atravessar esta chamada"
)]
pub async fn insert_idea<'e>(
    executor: impl PgExecutor<'e>,
    workspace_id: Uuid,
    title: &str,
    summary: Option<&str>,
    research_question: Option<&str>,
    hypothesis: Option<&str>,
    motivation: Option<&str>,
    keywords: &[String],
    created_by: Uuid,
) -> CoreResult<Idea> {
    let idea = sqlx::query_as::<_, Idea>(&format!(
        "INSERT INTO ideas
             (workspace_id, title, summary, research_question, hypothesis, motivation,
              keywords, state, created_by_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, 'discovery', $8)
         RETURNING {IDEA_COLUMNS}"
    ))
    .bind(workspace_id)
    .bind(title)
    .bind(summary)
    .bind(research_question)
    .bind(hypothesis)
    .bind(motivation)
    .bind(keywords)
    .bind(created_by)
    .fetch_one(executor)
    .await?;
    Ok(idea)
}

/// Load an idea.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn find_idea<'e>(executor: impl PgExecutor<'e>, id: Uuid) -> CoreResult<Option<Idea>> {
    let idea =
        sqlx::query_as::<_, Idea>(&format!("SELECT {IDEA_COLUMNS} FROM ideas WHERE id = $1"))
            .bind(id)
            .fetch_optional(executor)
            .await?;
    Ok(idea)
}

/// Load the idea of a workspace.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn find_idea_by_workspace<'e>(
    executor: impl PgExecutor<'e>,
    workspace_id: Uuid,
) -> CoreResult<Option<Idea>> {
    let idea = sqlx::query_as::<_, Idea>(&format!(
        "SELECT {IDEA_COLUMNS} FROM ideas WHERE workspace_id = $1"
    ))
    .bind(workspace_id)
    .fetch_optional(executor)
    .await?;
    Ok(idea)
}

/// Update the descriptive fields of an idea.
///
/// # What this cannot touch
///
/// `state`, `workspace_id`, `promoted_project_id`, `created_by_id` and every
/// timestamp. The lifecycle moves through `update_idea_state` and the workflow
/// that guards it; ownership and provenance are not fields anybody edits. A
/// single statement that could write all of them would be the mass assignment
/// this signature exists to make impossible.
///
/// `None` means «leave alone», which is why every argument is an `Option` and
/// `COALESCE` decides. Clearing a field is done by passing an empty string,
/// which the service normalises — not by passing `None`, which would make
/// «unset this» and «do not touch this» the same request.
///
/// # Errors
///
/// Returns an error when the statement fails.
#[expect(
    clippy::too_many_arguments,
    reason = "each field is written explicitly"
)]
pub async fn update_idea_fields<'e>(
    executor: impl PgExecutor<'e>,
    idea_id: Uuid,
    title: Option<&str>,
    summary: Option<&str>,
    research_question: Option<&str>,
    hypothesis: Option<&str>,
    motivation: Option<&str>,
    keywords: Option<&[String]>,
    updated_by: Uuid,
) -> CoreResult<Idea> {
    let idea = sqlx::query_as::<_, Idea>(&format!(
        "UPDATE ideas
            SET title             = COALESCE($2, title),
                summary           = COALESCE($3, summary),
                research_question = COALESCE($4, research_question),
                hypothesis        = COALESCE($5, hypothesis),
                motivation        = COALESCE($6, motivation),
                keywords          = COALESCE($7, keywords),
                updated_by_id     = $8,
                updated_at        = now()
          WHERE id = $1
      RETURNING {IDEA_COLUMNS}"
    ))
    .bind(idea_id)
    .bind(title)
    .bind(summary)
    .bind(research_question)
    .bind(hypothesis)
    .bind(motivation)
    .bind(keywords)
    .bind(updated_by)
    .fetch_one(executor)
    .await?;
    Ok(idea)
}

/// Move an idea to a new state.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn update_idea_state<'e>(
    executor: impl PgExecutor<'e>,
    idea_id: Uuid,
    state: IdeaState,
    outcome_note: Option<&str>,
    updated_by: Uuid,
) -> CoreResult<()> {
    sqlx::query(
        "UPDATE ideas
            SET state = $2,
                outcome_note = COALESCE($3, outcome_note),
                updated_by_id = $4,
                updated_at = now()
          WHERE id = $1",
    )
    .bind(idea_id)
    .bind(state.as_str())
    .bind(outcome_note)
    .bind(updated_by)
    .execute(executor)
    .await?;
    Ok(())
}

/// Record that an idea became a project.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn mark_idea_promoted<'e>(
    executor: impl PgExecutor<'e>,
    idea_id: Uuid,
    project_id: Uuid,
    updated_by: Uuid,
) -> CoreResult<()> {
    sqlx::query(
        "UPDATE ideas
            SET state = 'promoted', promoted_project_id = $2,
                updated_by_id = $3, updated_at = now()
          WHERE id = $1",
    )
    .bind(idea_id)
    .bind(project_id)
    .bind(updated_by)
    .execute(executor)
    .await?;
    Ok(())
}

/// Whether a project code is taken.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn project_code_taken<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    code: &str,
) -> CoreResult<bool> {
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM projects WHERE organisation_id = $1 AND code = $2)",
    )
    .bind(organisation_id)
    .bind(code)
    .fetch_one(executor)
    .await?;
    Ok(exists)
}

/// Insert a project.
///
/// # Errors
///
/// Returns an error when the insert fails.
#[expect(
    clippy::too_many_arguments,
    reason = "um parâmetro por coluna: a alternativa é uma struct que só existe para atravessar esta chamada"
)]
pub async fn insert_project<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    workspace_id: Uuid,
    code: &str,
    title: &str,
    summary: Option<&str>,
    objectives: Option<&str>,
    origin_idea_id: Option<Uuid>,
    responsible_person_id: Uuid,
    created_by: Uuid,
) -> CoreResult<Project> {
    let project = sqlx::query_as::<_, Project>(&format!(
        "INSERT INTO projects
             (organisation_id, workspace_id, code, title, summary, objectives,
              state, origin_idea_id, responsible_person_id, created_by_id)
         VALUES ($1, $2, $3, $4, $5, $6, 'draft', $7, $8, $9)
         RETURNING {PROJECT_COLUMNS}"
    ))
    .bind(organisation_id)
    .bind(workspace_id)
    .bind(code)
    .bind(title)
    .bind(summary)
    .bind(objectives)
    .bind(origin_idea_id)
    .bind(responsible_person_id)
    .bind(created_by)
    .fetch_one(executor)
    .await?;
    Ok(project)
}

/// Load a project.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn find_project<'e>(
    executor: impl PgExecutor<'e>,
    id: Uuid,
    organisation_id: Uuid,
) -> CoreResult<Option<Project>> {
    let project = sqlx::query_as::<_, Project>(&format!(
        "SELECT {PROJECT_COLUMNS} FROM projects WHERE id = $1 AND organisation_id = $2"
    ))
    .bind(id)
    .bind(organisation_id)
    .fetch_optional(executor)
    .await?;
    Ok(project)
}

/// Load the project of a workspace.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn find_project_by_workspace<'e>(
    executor: impl PgExecutor<'e>,
    workspace_id: Uuid,
) -> CoreResult<Option<Project>> {
    let project = sqlx::query_as::<_, Project>(&format!(
        "SELECT {PROJECT_COLUMNS} FROM projects WHERE workspace_id = $1"
    ))
    .bind(workspace_id)
    .fetch_optional(executor)
    .await?;
    Ok(project)
}

/// Move a project to a new state.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn update_project_state<'e>(
    executor: impl PgExecutor<'e>,
    project_id: Uuid,
    state: ProjectState,
    updated_by: Uuid,
) -> CoreResult<()> {
    sqlx::query(
        "UPDATE projects
            SET state = $2,
                started_at = CASE WHEN $2 = 'active' AND started_at IS NULL
                                  THEN now() ELSE started_at END,
                completed_at = CASE WHEN $2 = 'completed' THEN now() ELSE completed_at END,
                updated_by_id = $3,
                updated_at = now()
          WHERE id = $1",
    )
    .bind(project_id)
    .bind(state.as_str())
    .bind(updated_by)
    .execute(executor)
    .await?;
    Ok(())
}
