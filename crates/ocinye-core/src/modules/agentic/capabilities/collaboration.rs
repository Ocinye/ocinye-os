//! Collaboration capabilities.

use async_trait::async_trait;
use chrono::NaiveDate;
use ocinye_contracts::agentic::{
    ApprovalRequirement, AutonomyLevel, CapabilityDescriptor, CapabilityId, CapabilityResult,
    ExecutionStatus, OperationId, ResourceKind as AgenticKind, ResourceRef, Reversibility,
    RiskLevel,
};
use ocinye_contracts::{PageRequest, Permission, Scope, TaskState};
use uuid::Uuid;

use crate::error::{CoreError, CoreResult};
use crate::modules::agentic::executor::ExecutionContext;
use crate::modules::agentic::registry::CapabilityHandler;
use crate::modules::collaboration::{self, NewTask, TaskPriority};

/// Create a task in a research workspace.
///
/// # The everyday case
///
/// «Cria uma tarefa para o Carlos rever este documento até sexta» is the
/// request this exists for. Note what it does *not* do: it does not resolve
/// «Carlos». An `assignee_id` is a resolved identifier, and when two people
/// share a name the surface asks rather than guessing (briefing §189).
pub struct CreateTask;

#[async_trait]
impl CapabilityHandler for CreateTask {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new("collaboration.task.create"),
            operation: OperationId::new("collaboration::create_task"),
            domain: "collaboration".to_owned(),
            summary: "Criar uma tarefa num Research Workspace.".to_owned(),
            permission: Permission::TasksCreate,
            scope: Scope::ResearchWorkspace,
            risk: RiskLevel::LowImpact,
            approval: ApprovalRequirement::Never,
            max_autonomy: AutonomyLevel::Workflow,
            reversibility: Reversibility::Reversible,
            supports_dry_run: true,
            classification_ceiling: None,
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["title"],
                "properties": {
                    "title": {"type": "string"},
                    "description": {"type": "string"},
                    "assignee_id": {"type": "string", "description": "Identificador resolvido, nunca um nome."},
                    "due_on": {"type": "string", "description": "AAAA-MM-DD."},
                    "priority": {"type": "string"}
                }
            }),
        }
    }

    async fn execute(&self, ctx: &ExecutionContext<'_>) -> CoreResult<CapabilityResult> {
        // Through `resources`, for the same reason `research.idea.create` takes
        // its unit that way: `tasks.create` comes from workspace membership, and
        // a step that names no resource is authorised against the organisation,
        // where that membership is not consulted (ADR-0306).
        let workspace_id = ctx.one(AgenticKind::Workspace)?.reference.id;
        let title = ctx.text("title")?;
        let description: Option<String> = ctx.optional("description")?;
        let assignee_id: Option<Uuid> = ctx.optional("assignee_id")?;

        // A malformed date is refused rather than silently dropped: a task that
        // quietly loses its deadline is worse than one that was not created.
        let due_on: Option<NaiveDate> = match ctx.optional::<String>("due_on")? {
            None => None,
            Some(raw) => Some(raw.parse().map_err(|_| {
                crate::error::CoreError::Validation("A data limite deve ser AAAA-MM-DD.".to_owned())
            })?),
        };

        let priority = ctx
            .optional::<String>("priority")?
            .and_then(|raw| TaskPriority::parse(&raw))
            .unwrap_or(TaskPriority::Normal);

        if ctx.dry_run {
            return Ok(CapabilityResult {
                capability: self.descriptor().id,
                status: ExecutionStatus::DryRun,
                detail: format!("Seria criada a tarefa «{title}»."),
                resources: Vec::new(),
                reversibility: Reversibility::NothingToUndo,
                output: None,
            });
        }

        let mut tx = ctx.pool.begin().await?;
        let task = collaboration::create_task(
            &mut tx,
            ctx.principal,
            ctx.ids,
            NewTask {
                workspace_id,
                title: title.clone(),
                description,
                priority,
                assignee_id,
                due_on,
            },
        )
        .await?;
        tx.commit().await?;

        Ok(CapabilityResult {
            capability: self.descriptor().id,
            status: ExecutionStatus::Succeeded,
            detail: format!("Tarefa «{title}» criada."),
            resources: vec![ResourceRef {
                kind: AgenticKind::Task,
                id: task.id,
                label: Some(title),
            }],
            reversibility: Reversibility::Reversible,
            output: None,
        })
    }
}

/// List the tasks of a research workspace.
///
/// # Why this is not search
///
/// Tasks are not in the institutional index, and putting them there would make
/// «pesquisar» return work items alongside sources and documents. «Que tarefas
/// continuam abertas?» is a question about one workspace, answered from the
/// task table with the reader's own policy applied (briefing §62).
pub struct ListTasks;

#[async_trait]
impl CapabilityHandler for ListTasks {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new("collaboration.task.list"),
            operation: OperationId::new("collaboration::list_tasks"),
            domain: "collaboration".to_owned(),
            summary: "Listar as tarefas de um Research Workspace.".to_owned(),
            permission: Permission::TasksView,
            scope: Scope::ResearchWorkspace,
            risk: RiskLevel::ReadOnly,
            approval: ApprovalRequirement::Never,
            max_autonomy: AutonomyLevel::Workflow,
            reversibility: Reversibility::NothingToUndo,
            supports_dry_run: false,
            classification_ceiling: None,
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "open_only": {"type": "boolean", "description": "Só as que continuam abertas."}
                }
            }),
        }
    }

    async fn execute(&self, ctx: &ExecutionContext<'_>) -> CoreResult<CapabilityResult> {
        let workspace_id = ctx.one(AgenticKind::Workspace)?.reference.id;
        let open_only: bool = ctx.optional("open_only")?.unwrap_or(false);

        let (tasks, total) = collaboration::list_tasks(
            ctx.pool,
            ctx.principal,
            Some(workspace_id),
            None,
            open_only,
            PageRequest {
                page: 1,
                page_size: 50,
            },
        )
        .await?;

        Ok(CapabilityResult {
            capability: self.descriptor().id,
            status: ExecutionStatus::Succeeded,
            detail: match total {
                0 => "Nenhuma tarefa.".to_owned(),
                1 => "1 tarefa.".to_owned(),
                other => format!("{other} tarefas."),
            },
            resources: tasks
                .iter()
                .map(|task| ResourceRef {
                    kind: AgenticKind::Task,
                    id: task.id,
                    label: Some(task.title.clone()),
                })
                .collect(),
            reversibility: Reversibility::NothingToUndo,
            output: Some(serde_json::json!({
                "total": total,
                "items": tasks.iter().map(|task| serde_json::json!({
                    "id": task.id,
                    "title": task.title,
                    "state": task.state().as_str(),
                    "priority": task.priority,
                    "assignee_id": task.assignee_id,
                    "due_on": task.due_on,
                    "classification": task.classification().as_str(),
                })).collect::<Vec<_>>(),
            })),
        })
    }
}

/// Move a task through its lifecycle.
///
/// # The domain decides, not the model
///
/// A model asked to «fechar isto» will propose `done` from any state. Whether
/// that move is legal is [`assert_task_transition`](ocinye_domain::workflow),
/// and this capability does not know the answer — it asks. A transition the
/// workflow forbids comes back as a refusal with the real reason, not as a
/// silently written column (briefing §34).
pub struct TransitionTask;

#[async_trait]
impl CapabilityHandler for TransitionTask {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new("collaboration.task.transition"),
            operation: OperationId::new("collaboration::transition_task"),
            domain: "collaboration".to_owned(),
            summary: "Mudar o estado de uma tarefa.".to_owned(),
            permission: Permission::TasksEdit,
            scope: Scope::ResearchWorkspace,
            risk: RiskLevel::LowImpact,
            approval: ApprovalRequirement::Never,
            max_autonomy: AutonomyLevel::Workflow,
            reversibility: Reversibility::Reversible,
            supports_dry_run: true,
            classification_ceiling: None,
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["target_state"],
                "properties": {
                    "target_state": {
                        "type": "string",
                        "description": "todo, in_progress, blocked, in_review, done, cancelled",
                    }
                }
            }),
        }
    }

    async fn execute(&self, ctx: &ExecutionContext<'_>) -> CoreResult<CapabilityResult> {
        let task_id = ctx.one(AgenticKind::Task)?.reference.id;
        let raw = ctx.text("target_state")?;

        let target = TaskState::parse(&raw)
            .ok_or_else(|| CoreError::Validation(format!("«{raw}» não é um estado de tarefa.")))?;

        if ctx.dry_run {
            let (task, _) = collaboration::get_task(ctx.pool, ctx.principal, task_id).await?;
            return Ok(CapabilityResult {
                capability: self.descriptor().id,
                status: ExecutionStatus::DryRun,
                detail: format!(
                    "«{}» passaria de {} para {}.",
                    task.title,
                    task.state().as_str(),
                    target.as_str()
                ),
                resources: Vec::new(),
                reversibility: Reversibility::NothingToUndo,
                output: None,
            });
        }

        let mut tx = ctx.pool.begin().await?;
        let moved =
            collaboration::transition_task(&mut tx, ctx.principal, ctx.ids, task_id, target)
                .await?;
        tx.commit().await?;

        Ok(CapabilityResult {
            capability: self.descriptor().id,
            status: ExecutionStatus::Succeeded,
            detail: format!(
                "«{}» está agora em {}.",
                moved.title,
                moved.state().as_str()
            ),
            resources: vec![ResourceRef {
                kind: AgenticKind::Task,
                id: moved.id,
                label: Some(moved.title.clone()),
            }],
            reversibility: Reversibility::Reversible,
            output: Some(serde_json::json!({
                "task_id": moved.id,
                "state": moved.state().as_str(),
            })),
        })
    }
}

/// Change who is responsible for a task.
///
/// # The property that makes this safe to delegate
///
/// > **Work is only given to somebody who could see it.**
///
/// A model that writes an identifier into `assignee_id` is making a claim, and
/// the identifier being real is not evidence of anything. `assign_task` checks
/// that the named person could read the task — the same decision that governs
/// whether they could open it — so a task cannot be handed to somebody outside
/// the workspace, the unit or the organisation.
///
/// The assignee is named by identifier and never by name. «Atribui ao Carlos»
/// is a resolution the interface performs against people the member can already
/// see; a capability that accepted a name would be guessing which Carlos.
pub struct AssignTask;

#[async_trait]
impl CapabilityHandler for AssignTask {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new("collaboration.task.assign"),
            operation: OperationId::new("collaboration::assign_task"),
            domain: "collaboration".to_owned(),
            summary: "Atribuir uma tarefa, ou retirar a atribuição.".to_owned(),
            permission: Permission::TasksEdit,
            scope: Scope::ResearchWorkspace,
            risk: RiskLevel::LowImpact,
            approval: ApprovalRequirement::Never,
            max_autonomy: AutonomyLevel::Workflow,
            reversibility: Reversibility::Reversible,
            supports_dry_run: true,
            classification_ceiling: None,
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "assignee_id": {
                        "type": "string",
                        "description":
                            "Identificador resolvido. Ausente ou nulo retira a atribuição.",
                    }
                }
            }),
        }
    }

    async fn execute(&self, ctx: &ExecutionContext<'_>) -> CoreResult<CapabilityResult> {
        let task_id = ctx.one(AgenticKind::Task)?.reference.id;
        let assignee_id: Option<Uuid> = ctx.optional("assignee_id")?;

        if ctx.dry_run {
            let (task, _) = collaboration::get_task(ctx.pool, ctx.principal, task_id).await?;
            return Ok(CapabilityResult {
                capability: self.descriptor().id,
                status: ExecutionStatus::DryRun,
                detail: match assignee_id {
                    Some(_) => format!("«{}» passaria a ter responsável.", task.title),
                    None => format!("«{}» ficaria sem responsável.", task.title),
                },
                resources: Vec::new(),
                reversibility: Reversibility::NothingToUndo,
                output: None,
            });
        }

        let mut tx = ctx.pool.begin().await?;
        let task =
            collaboration::assign_task(&mut tx, ctx.principal, ctx.ids, task_id, assignee_id)
                .await?;
        tx.commit().await?;

        Ok(CapabilityResult {
            capability: self.descriptor().id,
            status: ExecutionStatus::Succeeded,
            detail: match task.assignee_id {
                Some(_) => format!("«{}» foi atribuída.", task.title),
                None => format!("«{}» ficou sem responsável.", task.title),
            },
            resources: vec![ResourceRef {
                kind: AgenticKind::Task,
                id: task.id,
                label: Some(task.title.clone()),
            }],
            reversibility: Reversibility::Reversible,
            output: Some(serde_json::json!({
                "task_id": task.id,
                "assigned": task.assignee_id.is_some(),
            })),
        })
    }
}
