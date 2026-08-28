//! Research capabilities.

use async_trait::async_trait;
use ocinye_contracts::agentic::{
    ApprovalRequirement, AutonomyLevel, CapabilityDescriptor, CapabilityId, CapabilityResult,
    ExecutionStatus, OperationId, ResourceKind as AgenticKind, ResourceRef, Reversibility,
    RiskLevel,
};
use ocinye_contracts::{Classification, IdeaState, Permission, ProjectState, Scope};

use crate::error::{CoreError, CoreResult};
use crate::modules::agentic::executor::ExecutionContext;
use crate::modules::agentic::registry::CapabilityHandler;
use ocinye_domain::workflow::{idea_targets_from, project_targets_from};

use crate::modules::research::{self, NewIdea};

/// Create an idea, with its research workspace.
///
/// # Reversible on purpose
///
/// An idea can be abandoned, and abandoning one is a legitimate outcome the
/// domain represents (`CLAUDE.md` §9). That is what makes this safe to let an
/// agent propose: the worst case is a spurious idea somebody archives, not a
/// loss.
pub struct CreateIdea;

#[async_trait]
impl CapabilityHandler for CreateIdea {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new("research.idea.create"),
            operation: OperationId::new("research::create_idea"),
            domain: "research".to_owned(),
            summary: "Criar uma Ideia numa unidade científica.".to_owned(),
            permission: Permission::IdeasCreate,
            scope: Scope::Unit,
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
                    "summary": {"type": "string"},
                    "research_question": {"type": "string"},
                    "classification": {"type": "string"}
                }
            }),
        }
    }

    async fn execute(&self, ctx: &ExecutionContext<'_>) -> CoreResult<CapabilityResult> {
        // The unit arrives through `resources`, not through the input.
        //
        // That is not a style choice. The executor authorises a step that names
        // no resource against the *request's* context — the organisation, with
        // no unit — and `ideas.create` is a permission that comes from unit
        // membership, which does not exist there. Taking the identifier from the
        // input therefore made this capability unreachable by exactly the people
        // who hold the permission: it failed closed, and silently (ADR-0306).
        let unit_id = ctx.one(AgenticKind::Unit)?.reference.id;
        let title = ctx.text("title")?;
        let summary: Option<String> = ctx.optional("summary")?;
        let research_question: Option<String> = ctx.optional("research_question")?;

        // A classification a model proposed is a *request*. `create_idea` caps
        // it against what the unit and the actor permit, and refuses what it
        // cannot grant — this is not the place that decides.
        let classification: Option<Classification> = ctx
            .optional::<String>("classification")?
            .and_then(|raw| Classification::parse(&raw));

        if ctx.dry_run {
            return Ok(CapabilityResult {
                capability: self.descriptor().id,
                status: ExecutionStatus::DryRun,
                detail: format!("Seria criada a Ideia «{title}»."),
                resources: Vec::new(),
                reversibility: Reversibility::NothingToUndo,
                output: None,
            });
        }

        // The service owns the invariant. `create_idea` takes a mutable
        // principal because creating makes the author workspace lead, and that
        // has to be reflected for the rest of the transaction.
        let mut principal = ctx.principal.clone();
        let mut tx = ctx.pool.begin().await?;

        let (idea, workspace) = research::create_idea(
            &mut tx,
            &mut principal,
            ctx.ids,
            NewIdea {
                unit_id,
                title: title.clone(),
                summary,
                research_question,
                hypothesis: None,
                motivation: None,
                keywords: Vec::new(),
                classification,
            },
        )
        .await?;

        tx.commit().await?;

        Ok(CapabilityResult {
            capability: self.descriptor().id,
            status: ExecutionStatus::Succeeded,
            detail: format!("Ideia «{title}» criada."),
            resources: vec![
                ResourceRef {
                    kind: AgenticKind::Idea,
                    id: idea.id,
                    label: Some(title),
                },
                ResourceRef {
                    kind: AgenticKind::Workspace,
                    id: workspace.id,
                    label: Some(workspace.code.clone()),
                },
            ],
            reversibility: Reversibility::Reversible,
            output: None,
        })
    }
}

/// Read one idea, with its lifecycle state.
///
/// # Why reading is its own capability
///
/// Search discovers references; this resolves one into content. Keeping them
/// apart is what lets the exposure filter show search to everybody and reading
/// only to those who hold `ideas.view` (briefing §24).
pub struct ReadIdea;

#[async_trait]
impl CapabilityHandler for ReadIdea {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new("research.idea.read"),
            operation: OperationId::new("research::get_idea"),
            domain: "research".to_owned(),
            summary: "Ler uma Ideia e o seu estado.".to_owned(),
            permission: Permission::IdeasView,
            scope: Scope::ResearchWorkspace,
            risk: RiskLevel::ReadOnly,
            approval: ApprovalRequirement::Never,
            max_autonomy: AutonomyLevel::Workflow,
            reversibility: Reversibility::NothingToUndo,
            supports_dry_run: false,
            classification_ceiling: None,
            // Nada. A Ideia é indicada como recurso, que é o canal que o
            // executor resolve e verifica (ADR-0306).
            input_schema: serde_json::json!({"type": "object", "properties": {}}),
        }
    }

    async fn execute(&self, ctx: &ExecutionContext<'_>) -> CoreResult<CapabilityResult> {
        let idea_id = ctx.one(AgenticKind::Idea)?.reference.id;
        let (idea, workspace) = research::get_idea(ctx.pool, ctx.principal, idea_id).await?;

        Ok(CapabilityResult {
            capability: self.descriptor().id,
            status: ExecutionStatus::Succeeded,
            detail: format!("Ideia «{}», em {}.", idea.title, idea.state().as_str()),
            resources: vec![ResourceRef {
                kind: AgenticKind::Idea,
                id: idea.id,
                label: Some(idea.title.clone()),
            }],
            reversibility: Reversibility::NothingToUndo,
            output: Some(serde_json::json!({
                "id": idea.id,
                "title": idea.title,
                "state": idea.state().as_str(),
                "summary": idea.summary,
                "research_question": idea.research_question,
                "hypothesis": idea.hypothesis,
                "motivation": idea.motivation,
                "keywords": idea.keywords,
                "outcome_note": idea.outcome_note,
                "promoted_project_id": idea.promoted_project_id,
                "workspace_id": workspace.id,
                "workspace_code": workspace.code,
                "classification": workspace.classification().as_str(),
                // The transitions the domain actually permits from here. A model
                // that has these does not need to guess, and one that guesses
                // anyway is refused by the service (briefing §8).
                "allowed_transitions": idea_targets_from(idea.state())
                    .iter()
                    .map(|state| state.as_str())
                    .collect::<Vec<_>>(),
            })),
        })
    }
}

/// Read one project, with its lifecycle state and origin.
pub struct ReadProject;

#[async_trait]
impl CapabilityHandler for ReadProject {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new("research.project.read"),
            operation: OperationId::new("research::get_project"),
            domain: "research".to_owned(),
            summary: "Ler um Projecto e o seu estado.".to_owned(),
            permission: Permission::ProjectsView,
            scope: Scope::ResearchWorkspace,
            risk: RiskLevel::ReadOnly,
            approval: ApprovalRequirement::Never,
            max_autonomy: AutonomyLevel::Workflow,
            reversibility: Reversibility::NothingToUndo,
            supports_dry_run: false,
            classification_ceiling: None,
            input_schema: serde_json::json!({"type": "object", "properties": {}}),
        }
    }

    async fn execute(&self, ctx: &ExecutionContext<'_>) -> CoreResult<CapabilityResult> {
        let project_id = ctx.one(AgenticKind::Project)?.reference.id;
        let (project, workspace) =
            research::get_project(ctx.pool, ctx.principal, project_id).await?;

        Ok(CapabilityResult {
            capability: self.descriptor().id,
            status: ExecutionStatus::Succeeded,
            detail: format!(
                "Projecto {} «{}», em {}.",
                project.code,
                project.title,
                project.state().as_str()
            ),
            resources: vec![ResourceRef {
                kind: AgenticKind::Project,
                id: project.id,
                label: Some(project.title.clone()),
            }],
            reversibility: Reversibility::NothingToUndo,
            output: Some(serde_json::json!({
                "id": project.id,
                "code": project.code,
                "title": project.title,
                "state": project.state().as_str(),
                "summary": project.summary,
                "objectives": project.objectives,
                // The lineage, both ways. A project that came from an idea says
                // so here, which is what makes provenance answerable rather than
                // reconstructable (`CLAUDE.md` §10).
                "origin_idea_id": project.origin_idea_id,
                "responsible_person_id": project.responsible_person_id,
                "started_at": project.started_at,
                "completed_at": project.completed_at,
                "workspace_id": workspace.id,
                "workspace_code": workspace.code,
                "classification": workspace.classification().as_str(),
                "allowed_transitions": project_targets_from(project.state())
                    .iter()
                    .map(|state| state.as_str())
                    .collect::<Vec<_>>(),
            })),
        })
    }
}

/// The contextual state of a research workspace.
///
/// # The capability that answers «onde estou»
///
/// One authorised read that returns the idea or project, the members, the
/// artefact counts and the recent activity — the same overview the Research
/// Workspace screen shows. It exists so «Resume o estado deste Projecto» costs
/// one call rather than six, which matters when the budget is a context window.
pub struct WorkspaceOverview;

#[async_trait]
impl CapabilityHandler for WorkspaceOverview {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new("research.workspace.overview"),
            operation: OperationId::new("research::get_workspace_overview"),
            domain: "research".to_owned(),
            summary: "Obter o estado de um Research Workspace.".to_owned(),
            // The floor for entering a workspace at all. What is inside it is
            // then filtered by the reader's own policy.
            permission: Permission::OrganisationView,
            scope: Scope::ResearchWorkspace,
            risk: RiskLevel::ReadOnly,
            approval: ApprovalRequirement::Never,
            max_autonomy: AutonomyLevel::Workflow,
            reversibility: Reversibility::NothingToUndo,
            supports_dry_run: false,
            classification_ceiling: None,
            input_schema: serde_json::json!({"type": "object", "properties": {}}),
        }
    }

    async fn execute(&self, ctx: &ExecutionContext<'_>) -> CoreResult<CapabilityResult> {
        let workspace_id = ctx.one(AgenticKind::Workspace)?.reference.id;
        let overview =
            research::get_workspace_overview(ctx.pool, ctx.principal, workspace_id).await?;

        let workspace = &overview.workspace;
        let mut resources = vec![ResourceRef {
            kind: AgenticKind::Workspace,
            id: workspace.id,
            label: Some(workspace.title.clone()),
        }];
        if let Some(idea) = &overview.idea {
            resources.push(ResourceRef {
                kind: AgenticKind::Idea,
                id: idea.id,
                label: Some(idea.title.clone()),
            });
        }
        if let Some(project) = &overview.project {
            resources.push(ResourceRef {
                kind: AgenticKind::Project,
                id: project.id,
                label: Some(project.title.clone()),
            });
        }

        Ok(CapabilityResult {
            capability: self.descriptor().id,
            status: ExecutionStatus::Succeeded,
            detail: format!(
                "{} «{}», {}.",
                workspace.code,
                workspace.title,
                workspace.classification().as_str()
            ),
            resources,
            reversibility: Reversibility::NothingToUndo,
            output: Some(serde_json::json!({
                "workspace": {
                    "id": workspace.id,
                    "code": workspace.code,
                    "title": workspace.title,
                    "kind": workspace.kind().as_str(),
                    "classification": workspace.classification().as_str(),
                    "archived": workspace.archived_at.is_some(),
                },
                "idea": overview.idea.as_ref().map(|idea| serde_json::json!({
                    "id": idea.id, "title": idea.title, "state": idea.state().as_str(),
                })),
                "project": overview.project.as_ref().map(|project| serde_json::json!({
                    "id": project.id, "code": project.code, "title": project.title,
                    "state": project.state().as_str(),
                    "origin_idea_id": project.origin_idea_id,
                })),
                "members": overview.members.iter().map(|member| serde_json::json!({
                    "person_id": member.person_id,
                    "full_name": member.full_name,
                    "role": member.role,
                })).collect::<Vec<_>>(),
            })),
        })
    }
}

/// Move an idea through its lifecycle.
///
/// # Why this needs a person, and the creation above does not
///
/// Creating an idea adds something. Transitioning one *asserts* something: that
/// the exploration has reached concept, that it is ready for review, that it is
/// abandoned. Those are institutional statements about work, and a member
/// should see the sentence before it is recorded (briefing §31, §32).
///
/// The lifecycle itself is not negotiable here. `transition_idea` decides
/// whether the move is legal; a model that proposes `discovery → promoted` is
/// refused by the domain, not by this handler.
pub struct TransitionIdea;

#[async_trait]
impl CapabilityHandler for TransitionIdea {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new("research.idea.transition"),
            operation: OperationId::new("research::transition_idea"),
            domain: "research".to_owned(),
            summary: "Mudar o estado de uma Ideia.".to_owned(),
            permission: Permission::IdeasTransition,
            scope: Scope::ResearchWorkspace,
            risk: RiskLevel::MaterialMutation,
            approval: ApprovalRequirement::Always,
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
                        "description":
                            "discovery, exploration, concept, review, project_candidate, \
                             rejected, archived",
                    },
                    "outcome_note": {
                        "type": "string",
                        "description": "Obrigatório ao encerrar uma Ideia.",
                    }
                }
            }),
        }
    }

    async fn execute(&self, ctx: &ExecutionContext<'_>) -> CoreResult<CapabilityResult> {
        let idea_id = ctx.one(AgenticKind::Idea)?.reference.id;
        let raw = ctx.text("target_state")?;
        let outcome_note: Option<String> = ctx.optional("outcome_note")?;

        let target = IdeaState::parse(&raw)
            .ok_or_else(|| CoreError::Validation(format!("«{raw}» não é um estado de Ideia.")))?;

        let (idea, _) = research::get_idea(ctx.pool, ctx.principal, idea_id).await?;

        if ctx.dry_run {
            // The domain's own answer, not a guess: if the move is illegal the
            // simulation says so rather than describing something that would
            // fail (briefing §34).
            let permitted = idea_targets_from(idea.state()).contains(&target);
            return Ok(CapabilityResult {
                capability: self.descriptor().id,
                status: ExecutionStatus::DryRun,
                detail: if permitted {
                    format!(
                        "«{}» passaria de {} para {}.",
                        idea.title,
                        idea.state().as_str(),
                        target.as_str()
                    )
                } else {
                    format!(
                        "«{}» está em {} e não pode passar para {}.",
                        idea.title,
                        idea.state().as_str(),
                        target.as_str()
                    )
                },
                resources: Vec::new(),
                reversibility: Reversibility::NothingToUndo,
                output: Some(serde_json::json!({ "permitted": permitted })),
            });
        }

        let mut tx = ctx.pool.begin().await?;
        let moved = research::transition_idea(
            &mut tx,
            ctx.principal,
            ctx.ids,
            idea_id,
            target,
            outcome_note.as_deref(),
        )
        .await?;
        tx.commit().await?;

        Ok(CapabilityResult {
            capability: self.descriptor().id,
            status: ExecutionStatus::Succeeded,
            detail: format!(
                "«{}» passou de {} para {}.",
                moved.title,
                idea.state().as_str(),
                moved.state().as_str()
            ),
            resources: vec![ResourceRef {
                kind: AgenticKind::Idea,
                id: moved.id,
                label: Some(moved.title.clone()),
            }],
            reversibility: Reversibility::Reversible,
            output: Some(serde_json::json!({
                "from": idea.state().as_str(),
                "to": moved.state().as_str(),
            })),
        })
    }
}

/// Move a project through its lifecycle.
pub struct TransitionProject;

#[async_trait]
impl CapabilityHandler for TransitionProject {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new("research.project.transition"),
            operation: OperationId::new("research::transition_project"),
            domain: "research".to_owned(),
            summary: "Mudar o estado de um Projecto.".to_owned(),
            permission: Permission::ProjectsManage,
            scope: Scope::ResearchWorkspace,
            risk: RiskLevel::MaterialMutation,
            approval: ApprovalRequirement::Always,
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
                        "description": "draft, active, on_hold, completed, archived",
                    }
                }
            }),
        }
    }

    async fn execute(&self, ctx: &ExecutionContext<'_>) -> CoreResult<CapabilityResult> {
        let project_id = ctx.one(AgenticKind::Project)?.reference.id;
        let raw = ctx.text("target_state")?;

        let target = ProjectState::parse(&raw).ok_or_else(|| {
            CoreError::Validation(format!("«{raw}» não é um estado de Projecto."))
        })?;

        let (project, _) = research::get_project(ctx.pool, ctx.principal, project_id).await?;

        if ctx.dry_run {
            let permitted = project_targets_from(project.state()).contains(&target);
            return Ok(CapabilityResult {
                capability: self.descriptor().id,
                status: ExecutionStatus::DryRun,
                detail: if permitted {
                    format!(
                        "{} passaria de {} para {}.",
                        project.code,
                        project.state().as_str(),
                        target.as_str()
                    )
                } else {
                    format!(
                        "{} está em {} e não pode passar para {}.",
                        project.code,
                        project.state().as_str(),
                        target.as_str()
                    )
                },
                resources: Vec::new(),
                reversibility: Reversibility::NothingToUndo,
                output: Some(serde_json::json!({ "permitted": permitted })),
            });
        }

        let mut tx = ctx.pool.begin().await?;
        let moved =
            research::transition_project(&mut tx, ctx.principal, ctx.ids, project_id, target)
                .await?;
        tx.commit().await?;

        Ok(CapabilityResult {
            capability: self.descriptor().id,
            status: ExecutionStatus::Succeeded,
            detail: format!(
                "{} passou de {} para {}.",
                moved.code,
                project.state().as_str(),
                moved.state().as_str()
            ),
            resources: vec![ResourceRef {
                kind: AgenticKind::Project,
                id: moved.id,
                label: Some(moved.title.clone()),
            }],
            reversibility: Reversibility::Reversible,
            output: Some(serde_json::json!({
                "from": project.state().as_str(),
                "to": moved.state().as_str(),
            })),
        })
    }
}

/// Promote a project candidate into a formal project.
///
/// # The one operation where the domain distinction is the whole point
///
/// An Idea is not a Project in draft (`CLAUDE.md` §9). Promotion is the moment
/// exploration becomes a formal commitment, and it is irreversible in the sense
/// that matters: the institution has said this work is now a project.
///
/// # Idempotence lives in the domain, not here
///
/// `promote_idea` refuses an idea that already carries a `promoted_project_id`.
/// Running the same confirmed plan twice therefore produces one project and one
/// conflict, never two projects. This handler adds nothing to that guarantee,
/// which is exactly why it is trustworthy: a second implementation of the rule
/// would be a second place for it to be wrong (briefing §11).
pub struct PromoteIdea;

#[async_trait]
impl CapabilityHandler for PromoteIdea {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new("research.idea.promote"),
            operation: OperationId::new("research::promote_idea"),
            domain: "research".to_owned(),
            summary: "Converter uma Ideia candidata num Projecto.".to_owned(),
            // `ProjectsManage`, e não `ProjectsCreate`, porque é o que o
            // domínio realmente exige: `promote_idea` autoriza quem pode
            // transicionar dentro daquele ambiente, o que inclui o líder do
            // Research Workspace. `ProjectsCreate` pertence a gestores de
            // unidade, e exigi-la aqui faria o plano agentic recusar o que a
            // interface permite — uma segunda política, não declarada em lado
            // nenhum (`CLAUDE.md` §3, briefing §81).
            permission: Permission::ProjectsManage,
            scope: Scope::ResearchWorkspace,
            risk: RiskLevel::MaterialMutation,
            approval: ApprovalRequirement::Always,
            max_autonomy: AutonomyLevel::Workflow,
            // A project can be archived; it cannot be un-created, and the idea
            // it came from does not return to being unpromoted.
            reversibility: Reversibility::Irreversible,
            supports_dry_run: true,
            classification_ceiling: None,
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["code"],
                "properties": {
                    "code": {"type": "string", "description": "Código institucional do Projecto."},
                    "title": {"type": "string", "description": "Por omissão, o título da Ideia."},
                    "objectives": {"type": "string"}
                }
            }),
        }
    }

    async fn execute(&self, ctx: &ExecutionContext<'_>) -> CoreResult<CapabilityResult> {
        let idea_id = ctx.one(AgenticKind::Idea)?.reference.id;
        let code = ctx.text("code")?;
        let title: Option<String> = ctx.optional("title")?;
        let objectives: Option<String> = ctx.optional("objectives")?;

        let (idea, workspace) = research::get_idea(ctx.pool, ctx.principal, idea_id).await?;

        if ctx.dry_run {
            let ready = idea.state() == ocinye_domain::workflow::PROMOTABLE_FROM
                && idea.promoted_project_id.is_none();

            return Ok(CapabilityResult {
                capability: self.descriptor().id,
                status: ExecutionStatus::DryRun,
                detail: if ready {
                    format!(
                        "«{}» tornar-se-ia o Projecto {code}, no mesmo ambiente {}, \
                         mantendo tudo o que já reuniu.",
                        idea.title, workspace.code
                    )
                } else if idea.promoted_project_id.is_some() {
                    format!("«{}» já foi convertida num Projecto.", idea.title)
                } else {
                    format!(
                        "«{}» está em {} e só pode ser convertida a partir de \
                         project_candidate.",
                        idea.title,
                        idea.state().as_str()
                    )
                },
                resources: Vec::new(),
                reversibility: Reversibility::NothingToUndo,
                output: Some(serde_json::json!({ "ready": ready })),
            });
        }

        let mut tx = ctx.pool.begin().await?;
        let project = research::promote_idea(
            &mut tx,
            ctx.principal,
            ctx.ids,
            idea_id,
            research::Promotion {
                code: code.clone(),
                title,
                objectives,
                responsible_person_id: None,
            },
        )
        .await?;
        tx.commit().await?;

        Ok(CapabilityResult {
            capability: self.descriptor().id,
            status: ExecutionStatus::Succeeded,
            detail: format!(
                "«{}» tornou-se o Projecto {}, no ambiente {}.",
                idea.title, project.code, workspace.code
            ),
            resources: vec![
                ResourceRef {
                    kind: AgenticKind::Project,
                    id: project.id,
                    label: Some(project.title.clone()),
                },
                ResourceRef {
                    kind: AgenticKind::Idea,
                    id: idea.id,
                    label: Some(idea.title.clone()),
                },
            ],
            reversibility: Reversibility::Irreversible,
            output: Some(serde_json::json!({
                "project_id": project.id,
                "code": project.code,
                // The lineage, returned so an interface can show it without a
                // second call. This is the field that makes the conversion
                // answerable years later (`CLAUDE.md` §10).
                "origin_idea_id": project.origin_idea_id,
                "workspace_id": workspace.id,
            })),
        })
    }
}

/// Revise the descriptive fields of an Idea.
///
/// # Why this is not `research.idea.update`
///
/// Because «update» invites the shape it must not have. A capability that
/// accepted an idea-shaped object and wrote it back would let a model set
/// `state`, `workspace_id` or `promoted_project_id` — three things the domain
/// decides and nobody edits. The schema below names six fields and no others,
/// so the shape of the input is the specification (briefing §12).
///
/// Moving an idea through its lifecycle is [`TransitionIdea`], and it is a
/// different act with a different risk.
pub struct ReviseIdea;

#[async_trait]
impl CapabilityHandler for ReviseIdea {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new("research.idea.revise"),
            operation: OperationId::new("research::update_idea"),
            domain: "research".to_owned(),
            summary: "Rever os campos descritivos de uma Ideia. Não muda o estado.".to_owned(),
            permission: Permission::IdeasEdit,
            scope: Scope::ResearchWorkspace,
            // Reversible and confined to text a member wrote: the previous
            // value is what it was, and the activity trail says it changed.
            risk: RiskLevel::LowImpact,
            approval: ApprovalRequirement::Never,
            max_autonomy: AutonomyLevel::Workflow,
            reversibility: Reversibility::Reversible,
            supports_dry_run: true,
            classification_ceiling: None,
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "title": {"type": "string"},
                    "summary": {"type": "string"},
                    "research_question": {"type": "string"},
                    "hypothesis": {"type": "string"},
                    "motivation": {"type": "string"},
                    "keywords": {"type": "array"}
                }
            }),
        }
    }

    async fn execute(&self, ctx: &ExecutionContext<'_>) -> CoreResult<CapabilityResult> {
        // Addressed through `resources`, never through an identifier in the
        // input: `resources` is the channel the executor resolved and
        // authorised (ADR-0306).
        let idea_id = ctx.one(AgenticKind::Idea)?.reference.id;

        let revision = research::IdeaRevision {
            title: ctx.optional("title")?,
            summary: ctx.optional("summary")?,
            research_question: ctx.optional("research_question")?,
            hypothesis: ctx.optional("hypothesis")?,
            motivation: ctx.optional("motivation")?,
            keywords: ctx.optional("keywords")?,
        };

        if revision.is_empty() {
            return Err(CoreError::Validation(
                "Indique o que pretende alterar na Ideia.".to_owned(),
            ));
        }

        if ctx.dry_run {
            let (idea, _) = research::get_idea(ctx.pool, ctx.principal, idea_id).await?;
            return Ok(CapabilityResult {
                capability: self.descriptor().id,
                status: ExecutionStatus::DryRun,
                detail: format!("«{}» seria revista.", idea.title),
                resources: Vec::new(),
                reversibility: Reversibility::NothingToUndo,
                output: None,
            });
        }

        let mut tx = ctx.pool.begin().await?;
        let (idea, workspace) =
            research::update_idea(&mut tx, ctx.principal, ctx.ids, idea_id, revision).await?;
        tx.commit().await?;

        Ok(CapabilityResult {
            capability: self.descriptor().id,
            status: ExecutionStatus::Succeeded,
            detail: format!("Ideia «{}» revista.", idea.title),
            resources: vec![
                ResourceRef {
                    kind: AgenticKind::Idea,
                    id: idea.id,
                    label: Some(idea.title.clone()),
                },
                ResourceRef {
                    kind: AgenticKind::Workspace,
                    id: workspace.id,
                    label: Some(workspace.title.clone()),
                },
            ],
            reversibility: Reversibility::Reversible,
            output: Some(serde_json::json!({
                "id": idea.id,
                "title": idea.title,
                "state": idea.state().as_str(),
            })),
        })
    }
}
