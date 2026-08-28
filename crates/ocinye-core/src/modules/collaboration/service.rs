//! Collaboration application layer.

use chrono::NaiveDate;
use ocinye_contracts::{PageRequest, TaskState};
use ocinye_domain::policy::{
    authorize, evaluate, Action, ResourceContext, ResourceKind, VisibilityFilter,
};
use ocinye_domain::workflow::assert_task_transition;
use ocinye_domain::Principal;
use ocinye_observability::CorrelationIds;
use sqlx::PgPool;
use uuid::Uuid;

use super::activity::{record_activity, ActivityKind};
use super::model::{ActivityEntry, Comment, Task, TaskPriority};
use super::repository as repo;
use crate::audit::{self, action, AuditEntry};
use crate::error::{CoreError, CoreResult};
use crate::modules::research::{
    artefact_context, get_workspace, readable_artefact_workspace, workspace_context,
    ResearchWorkspace,
};
use crate::outbox::{self, event};
use crate::Tx;

/// Details of a new task.
#[derive(Debug, Clone)]
pub struct NewTask {
    /// Workspace the task belongs to.
    pub workspace_id: Uuid,
    /// Title.
    pub title: String,
    /// Description.
    pub description: Option<String>,
    /// Priority.
    pub priority: TaskPriority,
    /// Person responsible.
    pub assignee_id: Option<Uuid>,
    /// Due date.
    pub due_on: Option<NaiveDate>,
}

/// Whether this person may be given this task, and refuse if not.
///
/// # The rule
///
/// > **A task may only be assigned to somebody who could read it.**
///
/// Nothing checked the assignee before. `assignee_id` travelled from the
/// request into the column, and the only guard was the foreign key — which
/// proves the identifier names *a* person, and nothing else. A task in one
/// organisation could therefore name somebody from another as its assignee,
/// which crosses the tenancy boundary every other decision in the Core
/// respects; and because a real identifier succeeded where an invented one
/// failed, it also answered «is this UUID a person here?» to anyone who asked.
///
/// The rule above is not a new policy. It is [`evaluate`] with `Action::Read`
/// against the task's own context — the same function that decides whether the
/// assignee could open the task once it exists. Assigning work to somebody who
/// cannot see it is not a narrower permission problem; it is an incoherent
/// state, and refusing it is what keeps the two halves agreeing.
///
/// # Errors
///
/// Returns [`CoreError::Validation`] when the person does not exist or could
/// not read the task. The two are one answer on purpose: distinguishing them
/// would restore the oracle this closes (`CLAUDE.md` §60).
async fn assert_assignable(
    tx: &mut Tx<'_>,
    ctx: &ResourceContext,
    assignee_id: Uuid,
) -> CoreResult<()> {
    let refuse =
        || CoreError::Validation("Não é possível atribuir esta tarefa a essa pessoa.".to_owned());

    let assignee = crate::modules::identity::principal_within(tx, assignee_id)
        .await?
        .ok_or_else(refuse)?;

    if !evaluate(&assignee, Action::Read, ctx).allowed {
        return Err(refuse());
    }

    Ok(())
}

/// Create a task.
///
/// A task inherits the workspace's classification: a task about restricted work
/// is itself restricted, including its title.
///
/// # Errors
///
/// Returns an error when the caller may not write in the workspace.
pub async fn create_task(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    request: NewTask,
) -> CoreResult<Task> {
    let workspace = get_workspace(&mut **tx, principal, request.workspace_id).await?;
    let ctx = workspace_context(&workspace, ResourceKind::Task);
    authorize(principal, Action::Create, &ctx)
        .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    let title = request.title.trim();
    if title.is_empty() {
        return Err(CoreError::Validation("A task needs a title.".to_owned()));
    }

    let classification = workspace.classification();

    // The assignee is a claim in the request until this says otherwise.
    if let Some(assignee_id) = request.assignee_id {
        assert_assignable(tx, &ctx, assignee_id).await?;
    }

    let task = repo::insert_task(
        &mut **tx,
        principal.organisation_id,
        workspace.unit_id,
        workspace.id,
        title,
        request.description.as_deref(),
        request.priority,
        request.assignee_id,
        request.due_on,
        classification,
        principal.person_id,
    )
    .await?;

    outbox::emit(
        tx,
        event::TASK_CREATED,
        "task",
        task.id,
        &ids.correlation_id,
        serde_json::json!({ "workspace_id": workspace.id }),
    )
    .await?;

    record_activity(
        tx,
        principal,
        workspace.id,
        workspace.unit_id,
        ActivityKind::Created,
        "task",
        Some(task.id),
        &format!("Task created: {title}"),
        classification,
    )
    .await?;

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::CREATE, "task")
            .resource(task.id)
            .context(&ctx),
    )
    .await?;

    Ok(task)
}

/// Load one task, with the workspace that governs it.
///
/// # Errors
///
/// Returns [`CoreError::NotFound`] when absent or not readable.
pub async fn get_task(
    pool: &PgPool,
    principal: &Principal,
    task_id: Uuid,
) -> CoreResult<(Task, ResearchWorkspace)> {
    let task = repo::find_task(pool, task_id, principal.organisation_id)
        .await?
        .ok_or_else(|| CoreError::NotFound("Task not found.".to_owned()))?;
    let workspace = readable_artefact_workspace(
        pool,
        principal,
        task.workspace_id,
        ResourceKind::Task,
        task.classification(),
    )
    .await?;
    Ok((task, workspace))
}

/// Move a task through its lifecycle.
///
/// # Errors
///
/// Returns an error when the caller may not transition it, or the lifecycle
/// forbids the move.
pub async fn transition_task(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    task_id: Uuid,
    target: TaskState,
) -> CoreResult<Task> {
    let task = repo::find_task(&mut **tx, task_id, principal.organisation_id)
        .await?
        .ok_or_else(|| CoreError::NotFound("Task not found.".to_owned()))?;
    let workspace = get_workspace(&mut **tx, principal, task.workspace_id).await?;

    let ctx = artefact_context(&workspace, ResourceKind::Task, task.classification());
    authorize(principal, Action::Transition, &ctx)
        .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    let current = task.state();
    assert_task_transition(current, target)?;

    repo::update_task_state(&mut **tx, task.id, target, principal.person_id).await?;

    outbox::emit_transition(
        tx,
        event::TASK_STATE_CHANGED,
        "task",
        task.id,
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
        "task",
        Some(task.id),
        &format!(
            "Task moved from {} to {}",
            current.as_str(),
            target.as_str()
        ),
        task.classification(),
    )
    .await?;

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::TRANSITION, "task")
            .resource(task.id)
            .context(&ctx)
            .detail("from", current.as_str())
            .detail("to", target.as_str()),
    )
    .await?;

    repo::find_task(&mut **tx, task.id, principal.organisation_id)
        .await?
        .ok_or_else(|| CoreError::Internal("task vanished during transition".to_owned()))
}

/// List tasks the caller may read.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn list_tasks(
    pool: &PgPool,
    principal: &Principal,
    workspace_id: Option<Uuid>,
    assignee_id: Option<Uuid>,
    open_only: bool,
    page: PageRequest,
) -> CoreResult<(Vec<Task>, i64)> {
    // Um `workspace_id` vindo do pedido restringe uma operação já autorizada;
    // não confere autoridade para entrar no ambiente. Resolvê-lo aqui é o que
    // transforma um identificador numa fronteira — `get_workspace` autoriza a
    // leitura do ambiente e recusa com a semântica canónica quando não pode.
    //
    // É a forma que `knowledge::list_sources` já usava, e a que faltava aqui.
    if let Some(workspace_id) = workspace_id {
        crate::modules::research::get_workspace(pool, principal, workspace_id).await?;
    }

    let filter = VisibilityFilter::for_principal(principal);
    if filter.is_never_satisfiable() {
        return Ok((Vec::new(), 0));
    }

    let query = repo::TaskFilter {
        workspace_id,
        assignee_id,
        open_only,
    };

    let tasks = repo::list_tasks(
        pool,
        principal.organisation_id,
        &filter,
        query,
        page.limit(),
        page.offset(),
    )
    .await?;
    let total = repo::count_tasks(pool, principal.organisation_id, &filter, query).await?;
    Ok((tasks, total))
}

/// Change who is responsible for a task, or clear it.
///
/// # Why assignment is its own operation
///
/// «Atribui isto ao Carlos» is a sentence a member actually says, and it is a
/// different institutional act from renaming a task or moving it through its
/// lifecycle. Folding it into a general update would mean one capability that
/// changes responsibility *and* content, which is a wider thing to confirm than
/// either.
///
/// The rule from [`assert_assignable`] applies: work is only given to somebody
/// who could see it.
///
/// # Errors
///
/// Returns an error when the caller may not write in the workspace, when the
/// task is not reachable, or when the assignee could not read it.
pub async fn assign_task(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    task_id: Uuid,
    assignee_id: Option<Uuid>,
) -> CoreResult<Task> {
    let task = repo::find_task(&mut **tx, task_id, principal.organisation_id)
        .await?
        .ok_or_else(|| CoreError::NotFound("Task not found.".to_owned()))?;
    let workspace = get_workspace(&mut **tx, principal, task.workspace_id).await?;

    let ctx = artefact_context(&workspace, ResourceKind::Task, task.classification());
    authorize(principal, Action::Update, &ctx)
        .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    if let Some(assignee_id) = assignee_id {
        assert_assignable(tx, &ctx, assignee_id).await?;
    }

    let updated =
        repo::set_task_assignee(&mut **tx, task.id, assignee_id, principal.person_id).await?;

    record_activity(
        tx,
        principal,
        workspace.id,
        workspace.unit_id,
        ActivityKind::Updated,
        "task",
        Some(updated.id),
        &format!("Task reassigned: {}", updated.title),
        task.classification(),
    )
    .await?;

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::UPDATE, "task")
            .resource(updated.id)
            .context(&ctx)
            .detail("event", "assigned")
            // The identifier, not the name: who somebody is belongs to the
            // people table, and the audit trail references rather than copies.
            .detail(
                "assignee",
                assignee_id.map_or_else(|| "cleared".to_owned(), |id| id.to_string()),
            ),
    )
    .await?;

    Ok(updated)
}

/// Add a comment to a research object.
///
/// # Errors
///
/// Returns an error when the caller may not write in the workspace, or the
/// comment is empty.
pub async fn add_comment(
    tx: &mut Tx<'_>,
    principal: &Principal,
    workspace_id: Uuid,
    subject_type: &str,
    subject_id: Uuid,
    body: &str,
) -> CoreResult<Comment> {
    let workspace = get_workspace(&mut **tx, principal, workspace_id).await?;
    let ctx = workspace_context(&workspace, ResourceKind::Comment);
    authorize(principal, Action::Create, &ctx)
        .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    let body = body.trim();
    if body.is_empty() {
        return Err(CoreError::Validation(
            "A comment cannot be empty.".to_owned(),
        ));
    }

    let classification = workspace.classification();
    let comment = repo::insert_comment(
        &mut **tx,
        principal.organisation_id,
        workspace.unit_id,
        workspace.id,
        subject_type,
        subject_id,
        body,
        classification,
        principal.person_id,
    )
    .await?;

    // The activity entry names the subject but never quotes the comment: the
    // feed must not become a second, less protected copy of the conversation.
    record_activity(
        tx,
        principal,
        workspace.id,
        workspace.unit_id,
        ActivityKind::Commented,
        subject_type,
        Some(subject_id),
        "A comment was added",
        classification,
    )
    .await?;

    Ok(comment)
}

/// List comments on a subject.
///
/// # Errors
///
/// Returns an error when the caller may not read the workspace.
pub async fn list_comments(
    pool: &PgPool,
    principal: &Principal,
    workspace_id: Uuid,
    subject_type: &str,
    subject_id: Uuid,
) -> CoreResult<Vec<Comment>> {
    let workspace: ResearchWorkspace = get_workspace(pool, principal, workspace_id).await?;
    let filter = VisibilityFilter::for_principal(principal);
    repo::list_comments(
        pool,
        principal.organisation_id,
        &filter,
        workspace.id,
        subject_type,
        subject_id,
    )
    .await
}

/// List activity the caller may read.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn list_activity(
    pool: &PgPool,
    principal: &Principal,
    workspace_id: Option<Uuid>,
    page: PageRequest,
) -> CoreResult<Vec<ActivityEntry>> {
    let filter = VisibilityFilter::for_principal(principal);
    if filter.is_never_satisfiable() {
        return Ok(Vec::new());
    }

    // Scoping to a workspace goes through the read check first, so an
    // unauthorised workspace id yields "not found" rather than an empty feed.
    if let Some(id) = workspace_id {
        get_workspace(pool, principal, id).await?;
    }

    repo::list_activity(
        pool,
        principal.organisation_id,
        &filter,
        workspace_id,
        page.limit(),
        page.offset(),
    )
    .await
}
