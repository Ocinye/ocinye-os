//! Collaboration persistence.

use chrono::NaiveDate;
use ocinye_contracts::{Classification, TaskState};
use ocinye_domain::policy::VisibilityFilter;
use sqlx::PgExecutor;
use uuid::Uuid;

use super::model::{ActivityEntry, Comment, Task, TaskPriority};
use crate::error::CoreResult;
use crate::visibility::{contained_in_visible_workspace, to_sql, VisibilityColumns};

const TASK_COLUMNS: &str = "id, organisation_id, unit_id, workspace_id, title, description,
                            state, priority, assignee_id, due_on, closed_at, classification,
                            created_at, updated_at";

/// Insert a task.
///
/// # Errors
///
/// Returns an error when the insert fails.
#[expect(
    clippy::too_many_arguments,
    reason = "um parâmetro por coluna: a alternativa é uma struct que só existe para atravessar esta chamada"
)]
pub async fn insert_task<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    unit_id: Uuid,
    workspace_id: Uuid,
    title: &str,
    description: Option<&str>,
    priority: TaskPriority,
    assignee_id: Option<Uuid>,
    due_on: Option<NaiveDate>,
    classification: Classification,
    created_by: Uuid,
) -> CoreResult<Task> {
    let task = sqlx::query_as::<_, Task>(&format!(
        "INSERT INTO tasks
             (organisation_id, unit_id, workspace_id, title, description, state,
              priority, assignee_id, due_on, classification, created_by_id)
         VALUES ($1, $2, $3, $4, $5, 'todo', $6, $7, $8, $9, $10)
         RETURNING {TASK_COLUMNS}"
    ))
    .bind(organisation_id)
    .bind(unit_id)
    .bind(workspace_id)
    .bind(title)
    .bind(description)
    .bind(priority.as_str())
    .bind(assignee_id)
    .bind(due_on)
    .bind(classification.as_str())
    .bind(created_by)
    .fetch_one(executor)
    .await?;
    Ok(task)
}

/// Load a task.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn find_task<'e>(
    executor: impl PgExecutor<'e>,
    id: Uuid,
    organisation_id: Uuid,
) -> CoreResult<Option<Task>> {
    let task = sqlx::query_as::<_, Task>(&format!(
        "SELECT {TASK_COLUMNS} FROM tasks WHERE id = $1 AND organisation_id = $2"
    ))
    .bind(id)
    .bind(organisation_id)
    .fetch_optional(executor)
    .await?;
    Ok(task)
}

/// Move a task to a new state.
///
/// `closed_at` is set or cleared in the same statement, so the schema's closure
/// consistency constraint always holds.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn update_task_state<'e>(
    executor: impl PgExecutor<'e>,
    task_id: Uuid,
    state: TaskState,
    updated_by: Uuid,
) -> CoreResult<()> {
    sqlx::query(
        "UPDATE tasks
            SET state = $2,
                closed_at = CASE WHEN $2 IN ('done', 'cancelled') THEN now() ELSE NULL END,
                updated_by_id = $3,
                updated_at = now()
          WHERE id = $1",
    )
    .bind(task_id)
    .bind(state.as_str())
    .bind(updated_by)
    .execute(executor)
    .await?;
    Ok(())
}

/// Which tasks a query is asking for.
///
/// Grouped rather than passed as a run of positional arguments: the two
/// `Option<Uuid>` parameters were trivially swappable at a call site.
#[derive(Debug, Clone, Copy, Default)]
pub struct TaskFilter {
    /// Restrict to one research workspace.
    pub workspace_id: Option<Uuid>,
    /// Restrict to one assignee.
    pub assignee_id: Option<Uuid>,
    /// Exclude closed tasks.
    pub open_only: bool,
}

/// Set or clear a task's assignee.
///
/// Deliberately writes one column. The caller has already established that the
/// assignee may read the task; this does not re-decide it, and does not touch
/// anything else about the task.
///
/// # Errors
///
/// Returns an error when the statement fails.
pub async fn set_task_assignee<'e>(
    executor: impl PgExecutor<'e>,
    task_id: Uuid,
    assignee_id: Option<Uuid>,
    updated_by: Uuid,
) -> CoreResult<Task> {
    let task = sqlx::query_as::<_, Task>(&format!(
        "UPDATE tasks
            SET assignee_id = $2, updated_by_id = $3, updated_at = now()
          WHERE id = $1
      RETURNING {TASK_COLUMNS}"
    ))
    .bind(task_id)
    .bind(assignee_id)
    .bind(updated_by)
    .fetch_one(executor)
    .await?;
    Ok(task)
}

/// List tasks the principal may read.
///
/// # Errors
///
/// Returns an error when the query fails.
/// A condição partilhada pela listagem de tarefas e pela sua contagem.
///
/// Uma tarefa pertence a um Research Workspace, e a sua própria classificação
/// não diz nada sobre o ambiente. Sem a segunda metade, uma tarefa `INTERNAL`
/// dentro de um ambiente inalcançável aparecia — com título, descrição, prazo e
/// o identificador de quem a tem atribuída, que qualquer membro resolve para um
/// nome.
///
/// A listagem sem âmbito era, além disso, o vector de descoberta: entregava o
/// `workspace_id` do ambiente fechado, e a partir daí o pedido com âmbito
/// explícito deixava de precisar de adivinhar nada.
fn task_predicate(visibility: &VisibilityFilter) -> String {
    let artefacto = to_sql(visibility, VisibilityColumns::default());
    let contido = contained_in_visible_workspace(visibility, "tasks");
    format!("{artefacto} AND {contido}")
}

pub async fn list_tasks<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    visibility: &VisibilityFilter,
    tasks: TaskFilter,
    limit: i64,
    offset: i64,
) -> CoreResult<Vec<Task>> {
    let predicate = task_predicate(visibility);
    let rows = sqlx::query_as::<_, Task>(&format!(
        "SELECT {TASK_COLUMNS} FROM tasks
          WHERE organisation_id = $1
            AND ($2::uuid IS NULL OR workspace_id = $2)
            AND ($3::uuid IS NULL OR assignee_id = $3)
            AND (NOT $4 OR state NOT IN ('done', 'cancelled'))
            AND {predicate}
          ORDER BY due_on ASC NULLS LAST, created_at DESC
          LIMIT $5 OFFSET $6"
    ))
    .bind(organisation_id)
    .bind(tasks.workspace_id)
    .bind(tasks.assignee_id)
    .bind(tasks.open_only)
    .bind(limit)
    .bind(offset)
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

/// Count tasks the principal may read.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn count_tasks<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    visibility: &VisibilityFilter,
    tasks: TaskFilter,
) -> CoreResult<i64> {
    let predicate = task_predicate(visibility);
    let total = sqlx::query_scalar::<_, i64>(&format!(
        "SELECT COUNT(*) FROM tasks
          WHERE organisation_id = $1
            AND ($2::uuid IS NULL OR workspace_id = $2)
            AND ($3::uuid IS NULL OR assignee_id = $3)
            AND (NOT $4 OR state NOT IN ('done', 'cancelled'))
            AND {predicate}"
    ))
    .bind(organisation_id)
    .bind(tasks.workspace_id)
    .bind(tasks.assignee_id)
    .bind(tasks.open_only)
    .fetch_one(executor)
    .await?;
    Ok(total)
}

/// Insert a comment.
///
/// # Errors
///
/// Returns an error when the insert fails.
#[expect(
    clippy::too_many_arguments,
    reason = "um parâmetro por coluna: a alternativa é uma struct que só existe para atravessar esta chamada"
)]
pub async fn insert_comment<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    unit_id: Uuid,
    workspace_id: Uuid,
    subject_type: &str,
    subject_id: Uuid,
    body: &str,
    classification: Classification,
    created_by: Uuid,
) -> CoreResult<Comment> {
    let comment = sqlx::query_as::<_, Comment>(
        "INSERT INTO comments
             (organisation_id, unit_id, workspace_id, subject_type, subject_id,
              body, classification, created_by_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         RETURNING id, workspace_id, subject_type, subject_id, body, classification,
                   withdrawn_at, created_by_id, created_at",
    )
    .bind(organisation_id)
    .bind(unit_id)
    .bind(workspace_id)
    .bind(subject_type)
    .bind(subject_id)
    .bind(body)
    .bind(classification.as_str())
    .bind(created_by)
    .fetch_one(executor)
    .await?;
    Ok(comment)
}

/// List comments on a subject that the principal may read.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn list_comments<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    filter: &VisibilityFilter,
    workspace_id: Uuid,
    subject_type: &str,
    subject_id: Uuid,
) -> CoreResult<Vec<Comment>> {
    let predicate = to_sql(filter, VisibilityColumns::default());
    let comments = sqlx::query_as::<_, Comment>(&format!(
        "SELECT id, workspace_id, subject_type, subject_id, body, classification,
                withdrawn_at, created_by_id, created_at
           FROM comments
          WHERE organisation_id = $1 AND workspace_id = $2
            AND subject_type = $3 AND subject_id = $4
            AND {predicate}
          ORDER BY created_at"
    ))
    .bind(organisation_id)
    .bind(workspace_id)
    .bind(subject_type)
    .bind(subject_id)
    .fetch_all(executor)
    .await?;
    Ok(comments)
}

/// List activity the principal may read.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn list_activity<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    filter: &VisibilityFilter,
    workspace_id: Option<Uuid>,
    limit: i64,
    offset: i64,
) -> CoreResult<Vec<ActivityEntry>> {
    let predicate = to_sql(
        filter,
        VisibilityColumns::aliased("a.unit_id", "a.workspace_id", "a.classification"),
    );
    let entries = sqlx::query_as::<_, ActivityEntry>(&format!(
        "SELECT a.id, a.workspace_id, a.actor_person_id, p.full_name AS actor_name,
                a.kind, a.subject_type, a.subject_id, a.summary, a.classification, a.created_at
           FROM activity_entries a
           LEFT JOIN people p ON p.id = a.actor_person_id
          WHERE a.organisation_id = $1
            AND ($2::uuid IS NULL OR a.workspace_id = $2)
            AND {predicate}
          ORDER BY a.created_at DESC
          LIMIT $3 OFFSET $4"
    ))
    .bind(organisation_id)
    .bind(workspace_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(executor)
    .await?;
    Ok(entries)
}
