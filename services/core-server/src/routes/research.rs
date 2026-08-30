//! Research routes: workspaces, ideas and projects.

use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use ocinye_contracts::{
    Classification, IdeaState, Page, PageRequest, ProjectState, WorkspaceKind, WorkspaceRole,
};
use ocinye_core::modules::research;
use ocinye_core::CoreError;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ApiError;
use crate::extract::{CurrentPrincipal, Ids};
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/workspaces", get(list_workspaces))
        .route("/workspaces/{workspace_id}", get(workspace_overview))
        .route("/workspaces/{workspace_id}/members", post(add_member))
        // Retirar uma pessoa do ambiente. `POST` e não `DELETE` pela mesma
        // razão que a unidade: o formulário do Workspace fala HTTP de
        // formulário, e uma operação que só existisse para `fetch` seria uma
        // operação que a Experience sem JavaScript não alcança.
        .route(
            "/workspaces/{workspace_id}/members/{person_id}",
            post(remove_member),
        )
        .route(
            "/workspaces/{workspace_id}/classification",
            post(reclassify),
        )
        .route("/ideas", post(create_idea))
        .route("/ideas/{idea_id}", get(get_idea))
        .route("/ideas/{idea_id}/transitions", post(transition_idea))
        .route("/ideas/{idea_id}/promotion", post(promote_idea))
        .route("/projects/{project_id}", get(get_project))
        .route(
            "/projects/{project_id}/transitions",
            post(transition_project),
        )
}

#[derive(Serialize)]
struct WorkspaceView {
    id: Uuid,
    unit_id: Uuid,
    code: String,
    title: String,
    kind: String,
    classification: String,
    /// Se este membro pode criar artefactos neste ambiente, **segundo a política
    /// genérica actual**.
    ///
    /// Existe para que um selector — «em que workspace?» — ofereça apenas
    /// destinos onde a operação seria aceite. Não é autorização: é o que a
    /// interface pode oferecer. O Core decide outra vez quando a criação chega,
    /// e é lá que a garantia vive.
    ///
    /// # O que este booleano é, e o que não é
    ///
    /// Um único valor serve selectores de tipos diferentes — fontes, datasets —
    /// e isso **só é honesto enquanto `Action::Create` não depender do
    /// `ResourceKind`**. Hoje não depende: o ramo consulta classificação e
    /// filiação, e `ctx.kind` não aparece na política.
    ///
    /// Isso é uma propriedade do momento, não uma garantia arquitectural. Está
    /// guardada por `criar_nao_depende_do_tipo_de_recurso`, em `ocinye-domain`,
    /// que quebra assim que a criação passar a distinguir tipos — antes de o
    /// selector começar a oferecer o destino errado.
    ///
    /// Se esse teste falhar, a correcção **não** é ajustá-lo: é servir uma
    /// resposta por operação em vez deste booleano.
    may_create: bool,
}

impl From<&research::ResearchWorkspace> for WorkspaceView {
    fn from(workspace: &research::ResearchWorkspace) -> Self {
        Self {
            id: workspace.id,
            unit_id: workspace.unit_id,
            code: workspace.code.clone(),
            title: workspace.title.clone(),
            kind: workspace.kind.clone(),
            classification: workspace.classification.clone(),
            // Sem principal não se pode afirmar nada sobre o que ele pode: a
            // resposta conservadora é a única honesta.
            may_create: false,
        }
    }
}

impl WorkspaceView {
    /// A mesma vista, sabendo quem pergunta.
    ///
    /// `may_create` é avaliado pela política existente, em memória, sem
    /// consulta adicional — e continua a não ser autorização. É o que a
    /// interface pode oferecer sem prometer uma recusa.
    fn for_principal(
        workspace: &research::ResearchWorkspace,
        principal: &ocinye_domain::Principal,
    ) -> Self {
        let ctx = research::workspace_context(workspace, ocinye_domain::ResourceKind::Dataset);
        Self {
            may_create: ocinye_domain::policy::authorize(
                principal,
                ocinye_domain::Action::Create,
                &ctx,
            )
            .is_ok(),
            ..Self::from(workspace)
        }
    }
}

#[derive(Deserialize)]
struct ListWorkspacesQuery {
    #[serde(default)]
    unit_id: Option<Uuid>,
    /// `idea` ou `project`. Ausente devolve ambos.
    ///
    /// Tipado, e não uma string livre: um valor desconhecido é recusado pelo
    /// desserializador em vez de silenciosamente não filtrar nada — que
    /// devolveria a lista inteira a quem pediu metade.
    #[serde(default)]
    kind: Option<WorkspaceKind>,
    /// Restringe a ideias que a promoção aceitaria hoje.
    ///
    /// Serve o selector de «Novo Projecto»: oferecer uma ideia que a operação
    /// recusaria seria um botão para uma recusa. O Core valida na mesma quando
    /// a promoção chega — isto é o que a interface oferece, não o que ela pode.
    #[serde(default)]
    promotable: Option<bool>,
    /// Restringe aos ambientes onde quem pergunta tem papel.
    ///
    /// «Ver» e «participar» são coisas diferentes: um ecrã que promete a
    /// investigação em que o membro participa não pode responder com tudo o
    /// que ele alcança.
    #[serde(default)]
    mine: Option<bool>,
    #[serde(default)]
    page: Option<u32>,
    #[serde(default)]
    page_size: Option<u32>,
}

fn page_of(page: Option<u32>, page_size: Option<u32>) -> PageRequest {
    PageRequest {
        page: page.unwrap_or(1),
        page_size: page_size.unwrap_or(ocinye_contracts::page::DEFAULT_PAGE_SIZE),
    }
}

async fn list_workspaces(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Query(query): Query<ListWorkspacesQuery>,
) -> Result<Json<Page<WorkspaceView>>, ApiError> {
    let page = page_of(query.page, query.page_size);
    // Os ambientes onde este membro tem papel. Saem do principal, que já os
    // transporta: não é preciso ir à base de dados perguntá-lo outra vez.
    let meus = principal.workspace_ids();
    let (workspaces, total) = research::list_workspaces(
        &state.pool,
        &principal,
        research::WorkspaceQuery {
            unit_id: query.unit_id,
            kind: query.kind,
            promotable_only: query.promotable.unwrap_or(false),
            member_of: query.mine.unwrap_or(false).then_some(meus.as_slice()),
        },
        page,
    )
    .await?;
    Ok(Json(Page::new(
        workspaces
            .iter()
            .map(|w| WorkspaceView::for_principal(w, &principal))
            .collect(),
        page,
        total,
    )))
}

#[derive(Serialize)]
struct IdeaView {
    id: Uuid,
    title: String,
    summary: Option<String>,
    research_question: Option<String>,
    hypothesis: Option<String>,
    motivation: Option<String>,
    keywords: Vec<String>,
    state: String,
    outcome_note: Option<String>,
    /// Set once the idea has become a project. The lineage is kept on both
    /// sides and never rewritten.
    promoted_project_id: Option<Uuid>,
}

impl From<research::Idea> for IdeaView {
    fn from(idea: research::Idea) -> Self {
        Self {
            id: idea.id,
            title: idea.title,
            summary: idea.summary,
            research_question: idea.research_question,
            hypothesis: idea.hypothesis,
            motivation: idea.motivation,
            keywords: idea.keywords,
            state: idea.state,
            outcome_note: idea.outcome_note,
            promoted_project_id: idea.promoted_project_id,
        }
    }
}

#[derive(Serialize)]
struct ProjectView {
    id: Uuid,
    code: String,
    title: String,
    summary: Option<String>,
    objectives: Option<String>,
    state: String,
    /// Which idea originated this project.
    origin_idea_id: Option<Uuid>,
    responsible_person_id: Option<Uuid>,
}

impl From<research::Project> for ProjectView {
    fn from(project: research::Project) -> Self {
        Self {
            id: project.id,
            code: project.code,
            title: project.title,
            summary: project.summary,
            objectives: project.objectives,
            state: project.state,
            origin_idea_id: project.origin_idea_id,
            responsible_person_id: project.responsible_person_id,
        }
    }
}

/// Everything the Research Workspace screen needs in one authorised read.
#[derive(Serialize)]
struct OverviewView {
    workspace: WorkspaceView,
    idea: Option<IdeaView>,
    project: Option<ProjectView>,
    members: Vec<WorkspaceMemberView>,
}

#[derive(Serialize)]
struct WorkspaceMemberView {
    person_id: Uuid,
    full_name: String,
    role: String,
}

async fn workspace_overview(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(workspace_id): Path<Uuid>,
) -> Result<Json<OverviewView>, ApiError> {
    let overview = research::get_workspace_overview(&state.pool, &principal, workspace_id).await?;

    Ok(Json(OverviewView {
        // `for_principal`, e não `from`: a vista sem principal responde
        // `may_create: false` por ser a resposta conservadora quando não se
        // sabe quem pergunta — e aqui sabe-se. Servir o `false` fixo fazia
        // toda a superfície de criação do ambiente desaparecer para toda a
        // gente, incluindo para quem o lidera.
        workspace: WorkspaceView::for_principal(&overview.workspace, &principal),
        idea: overview.idea.map(IdeaView::from),
        project: overview.project.map(ProjectView::from),
        members: overview
            .members
            .into_iter()
            .map(|member| WorkspaceMemberView {
                person_id: member.person_id,
                full_name: member.full_name,
                role: member.role,
            })
            .collect(),
    }))
}

#[derive(Deserialize)]
struct CreateIdeaRequest {
    unit_id: Uuid,
    title: String,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    research_question: Option<String>,
    #[serde(default)]
    hypothesis: Option<String>,
    #[serde(default)]
    motivation: Option<String>,
    #[serde(default)]
    keywords: Vec<String>,
    #[serde(default)]
    classification: Option<String>,
}

fn parse_classification(raw: Option<&str>) -> Result<Option<Classification>, CoreError> {
    raw.map(|value| {
        Classification::parse(value)
            .ok_or_else(|| CoreError::Validation("Unknown classification.".to_owned()))
    })
    .transpose()
}

#[derive(Serialize)]
struct IdeaCreated {
    idea: IdeaView,
    workspace: WorkspaceView,
}

async fn create_idea(
    State(state): State<AppState>,
    CurrentPrincipal(mut principal): CurrentPrincipal,
    Ids(ids): Ids,
    Json(request): Json<CreateIdeaRequest>,
) -> Result<Json<IdeaCreated>, ApiError> {
    let classification = parse_classification(request.classification.as_deref())?;

    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let (idea, workspace) = research::create_idea(
        &mut tx,
        &mut principal,
        &ids,
        research::NewIdea {
            unit_id: request.unit_id,
            title: request.title,
            summary: request.summary,
            research_question: request.research_question,
            hypothesis: request.hypothesis,
            motivation: request.motivation,
            keywords: request.keywords,
            classification,
        },
    )
    .await?;
    tx.commit().await.map_err(CoreError::from)?;

    Ok(Json(IdeaCreated {
        workspace: WorkspaceView::from(&workspace),
        idea: IdeaView::from(idea),
    }))
}

async fn get_idea(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(idea_id): Path<Uuid>,
) -> Result<Json<IdeaCreated>, ApiError> {
    let (idea, workspace) = research::get_idea(&state.pool, &principal, idea_id).await?;
    Ok(Json(IdeaCreated {
        workspace: WorkspaceView::from(&workspace),
        idea: IdeaView::from(idea),
    }))
}

#[derive(Deserialize)]
struct TransitionRequest {
    state: String,
    /// Required when closing an idea: why it was closed is institutional
    /// memory, not noise.
    #[serde(default)]
    outcome_note: Option<String>,
}

async fn transition_idea(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path(idea_id): Path<Uuid>,
    Json(request): Json<TransitionRequest>,
) -> Result<Json<IdeaView>, ApiError> {
    let target = IdeaState::parse(&request.state)
        .ok_or_else(|| CoreError::Validation("Unknown idea state.".to_owned()))?;

    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let idea = research::transition_idea(
        &mut tx,
        &principal,
        &ids,
        idea_id,
        target,
        request.outcome_note.as_deref(),
    )
    .await?;
    tx.commit().await.map_err(CoreError::from)?;

    Ok(Json(IdeaView::from(idea)))
}

#[derive(Deserialize)]
struct PromotionRequest {
    code: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    objectives: Option<String>,
    #[serde(default)]
    responsible_person_id: Option<Uuid>,
}

/// Promote a project candidate into a formal project.
///
/// The research workspace carries over, so everything gathered while exploring
/// stays attached to the project it became.
async fn promote_idea(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path(idea_id): Path<Uuid>,
    Json(request): Json<PromotionRequest>,
) -> Result<Json<ProjectView>, ApiError> {
    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let project = research::promote_idea(
        &mut tx,
        &principal,
        &ids,
        idea_id,
        research::Promotion {
            code: request.code,
            title: request.title,
            objectives: request.objectives,
            responsible_person_id: request.responsible_person_id,
        },
    )
    .await?;
    tx.commit().await.map_err(CoreError::from)?;

    Ok(Json(ProjectView::from(project)))
}

async fn get_project(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(project_id): Path<Uuid>,
) -> Result<Json<ProjectView>, ApiError> {
    let (project, _) = research::get_project(&state.pool, &principal, project_id).await?;
    Ok(Json(ProjectView::from(project)))
}

async fn transition_project(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path(project_id): Path<Uuid>,
    Json(request): Json<TransitionRequest>,
) -> Result<Json<ProjectView>, ApiError> {
    let target = ProjectState::parse(&request.state)
        .ok_or_else(|| CoreError::Validation("Unknown project state.".to_owned()))?;

    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let project =
        research::transition_project(&mut tx, &principal, &ids, project_id, target).await?;
    tx.commit().await.map_err(CoreError::from)?;

    Ok(Json(ProjectView::from(project)))
}

#[derive(Deserialize)]
struct AddWorkspaceMemberRequest {
    person_id: Uuid,
    role: String,
}

/// Retira uma pessoa do ambiente.
async fn remove_member(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path((workspace_id, person_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    research::remove_workspace_member(&mut tx, &principal, &ids, workspace_id, person_id).await?;
    tx.commit().await.map_err(CoreError::from)?;

    Ok(Json(serde_json::json!({ "removed": true })))
}

async fn add_member(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path(workspace_id): Path<Uuid>,
    Json(request): Json<AddWorkspaceMemberRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let role = WorkspaceRole::parse(&request.role)
        .ok_or_else(|| CoreError::Validation("Unknown workspace role.".to_owned()))?;

    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let membership_id = research::add_workspace_member(
        &mut tx,
        &principal,
        &ids,
        workspace_id,
        request.person_id,
        role,
    )
    .await?;
    tx.commit().await.map_err(CoreError::from)?;

    Ok(Json(serde_json::json!({ "membership_id": membership_id })))
}

#[derive(Deserialize)]
struct ReclassifyRequest {
    classification: String,
    /// Required: a classification change is a governance act and must be
    /// explicable months later.
    reason: String,
}

async fn reclassify(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path(workspace_id): Path<Uuid>,
    Json(request): Json<ReclassifyRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let classification = Classification::parse(&request.classification)
        .ok_or_else(|| CoreError::Validation("Unknown classification.".to_owned()))?;

    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    research::reclassify_workspace(
        &mut tx,
        &principal,
        &ids,
        workspace_id,
        classification,
        &request.reason,
    )
    .await?;
    tx.commit().await.map_err(CoreError::from)?;

    Ok(Json(
        serde_json::json!({ "classification": classification.as_str() }),
    ))
}
