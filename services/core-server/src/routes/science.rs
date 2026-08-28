//! Scientific lifecycle routes: hypotheses, methodologies, studies, results.
//!
//! # Os caminhos dizem o que a coisa é
//!
//! Uma versão de metodologia vive em `/methodologies/{id}/versions`, e depois em
//! `/methodology-versions/{id}`. Não é redundância: o primeiro é «as versões
//! desta metodologia», o segundo é «esta versão», que é um recurso próprio e
//! precisa de ser endereçável sem passar pela metodologia — porque é dela que a
//! proveniência fala.
//!
//! # A linhagem não é um recurso
//!
//! `/lineage/{kind}/{id}` é uma **projecção**: não há tabela de linhagem, e
//! cada travessia lê a proveniência agora. O tipo entra no caminho porque a
//! travessia parte de qualquer recurso, e não só de resultados.

use axum::extract::{Path, Query, State};
use axum::routing::get;
use axum::{Json, Router};
use ocinye_contracts::agentic::{ResourceKind as AgenticKind, ResourceRef};
use ocinye_contracts::Classification;
use ocinye_core::modules::science;
use ocinye_core::CoreError;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ApiError;
use crate::extract::{CurrentPrincipal, Ids};
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/workspaces/{workspace_id}/hypotheses",
            get(list_hypotheses).post(create_hypothesis),
        )
        .route("/hypotheses/{hypothesis_id}", get(get_hypothesis))
        .route(
            "/workspaces/{workspace_id}/methodologies",
            get(list_methodologies).post(create_methodology),
        )
        .route("/methodologies/{methodology_id}", get(get_methodology))
        .route(
            "/methodologies/{methodology_id}/versions",
            get(list_methodology_versions).post(publish_methodology_version),
        )
        .route("/methodology-versions/{version_id}", get(get_version))
        .route(
            "/workspaces/{workspace_id}/studies",
            get(list_studies).post(create_study),
        )
        .route("/studies/{study_id}", get(get_study))
        .route(
            "/studies/{study_id}/executions",
            get(list_executions).post(record_execution),
        )
        .route("/executions/{execution_id}", get(get_execution))
        .route(
            "/workspaces/{workspace_id}/results",
            get(list_results).post(create_result),
        )
        .route("/results/{result_id}", get(get_result))
        // Validar não é uma capability, e por isso este caminho existe.
        //
        // `science::record_validation` é `non_delegable`, atrás da
        // `INSTITUTIONAL_CLAIM_BOUNDARY`: nenhum agente a alcança. Uma pessoa
        // alcança-a aqui, com a sua sessão, e é o nome dela que fica no
        // registo (ADR-0307).
        .route(
            "/results/{result_id}/validations",
            get(list_validations).post(record_validation),
        )
        .route("/lineage/{kind}/{resource_id}", get(lineage))
}

// ── Vistas ──────────────────────────────────────────────────────────────
//
// O schema interno não é contrato (`CLAUDE.md` §23). Cada vista escolhe o que
// sai, e `unit_id` fica de fora: é chave interna, e quem lê o ecrã trabalha
// com o ambiente, não com a unidade que o detém.

#[derive(Serialize)]
struct HypothesisView {
    id: Uuid,
    workspace_id: Option<Uuid>,
    statement: String,
    rationale: Option<String>,
    status: String,
    /// Como um membro lê o estado. Vem do domínio, e não de uma tabela do cliente.
    status_label: &'static str,
    classification: String,
    created_at: String,
}

impl From<science::Hypothesis> for HypothesisView {
    fn from(h: science::Hypothesis) -> Self {
        Self {
            status_label: h.status_label(),
            id: h.id,
            workspace_id: h.workspace_id,
            statement: h.statement,
            rationale: h.rationale,
            status: h.status,
            classification: h.classification,
            created_at: h.created_at.to_rfc3339(),
        }
    }
}

#[derive(Serialize)]
struct MethodologyView {
    id: Uuid,
    workspace_id: Option<Uuid>,
    title: String,
    purpose: Option<String>,
    classification: String,
    created_at: String,
}

impl From<science::Methodology> for MethodologyView {
    fn from(m: science::Methodology) -> Self {
        Self {
            id: m.id,
            workspace_id: m.workspace_id,
            title: m.title,
            purpose: m.purpose,
            classification: m.classification,
            created_at: m.created_at.to_rfc3339(),
        }
    }
}

#[derive(Serialize)]
struct MethodologyVersionView {
    id: Uuid,
    methodology_id: Uuid,
    sequence: i32,
    label: String,
    summary: String,
    status: String,
    /// Como um membro lê o estado desta versão.
    status_label: &'static str,
    superseded_by_id: Option<Uuid>,
    published_at: Option<String>,
}

impl From<science::MethodologyVersion> for MethodologyVersionView {
    fn from(v: science::MethodologyVersion) -> Self {
        Self {
            status_label: v.status_label(),
            id: v.id,
            methodology_id: v.methodology_id,
            sequence: v.sequence,
            label: v.label,
            summary: v.summary,
            status: v.status,
            superseded_by_id: v.superseded_by_id,
            published_at: v.published_at.map(|t| t.to_rfc3339()),
        }
    }
}

#[derive(Serialize)]
struct StudyView {
    id: Uuid,
    workspace_id: Option<Uuid>,
    hypothesis_id: Option<Uuid>,
    title: String,
    kind: String,
    kind_label: &'static str,
    objective: Option<String>,
    status: String,
    /// Como um membro lê o estado.
    status_label: &'static str,
    classification: String,
    created_at: String,
}

impl From<science::Study> for StudyView {
    fn from(s: science::Study) -> Self {
        Self {
            kind_label: s.kind_label(),
            status_label: s.status_label(),
            id: s.id,
            workspace_id: s.workspace_id,
            hypothesis_id: s.hypothesis_id,
            title: s.title,
            kind: s.kind,
            objective: s.objective,
            status: s.status,
            classification: s.classification,
            created_at: s.created_at.to_rfc3339(),
        }
    }
}

#[derive(Serialize)]
struct ExecutionView {
    id: Uuid,
    study_id: Uuid,
    sequence: i32,
    status: String,
    started_at: Option<String>,
    finished_at: Option<String>,
    environment: Option<String>,
    software_name: Option<String>,
    software_version: Option<String>,
    software_commit: Option<String>,
    notes: Option<String>,
}

impl From<science::StudyExecution> for ExecutionView {
    fn from(e: science::StudyExecution) -> Self {
        Self {
            id: e.id,
            study_id: e.study_id,
            sequence: e.sequence,
            status: e.status,
            started_at: e.started_at.map(|t| t.to_rfc3339()),
            finished_at: e.finished_at.map(|t| t.to_rfc3339()),
            environment: e.environment,
            software_name: e.software_name,
            software_version: e.software_version,
            software_commit: e.software_commit,
            notes: e.notes,
        }
    }
}

#[derive(Serialize)]
struct ResultView {
    id: Uuid,
    workspace_id: Option<Uuid>,
    execution_id: Option<Uuid>,
    title: String,
    summary: String,
    status: String,
    /// Como um membro lê o estado.
    status_label: &'static str,
    classification: String,
    superseded_by_id: Option<Uuid>,
    created_at: String,
    /// Se quem pergunta pode afirmar que este resultado se confirma.
    ///
    /// Avaliado aqui, com o contexto deste resultado, porque
    /// `results.validate` chega pela liderança do ambiente ou pela gestão da
    /// unidade — e as capacidades que o `/identity/me` publica são as do
    /// âmbito institucional, onde uma permissão de ambiente nunca aparece. Uma
    /// interface a decidir isto sozinha estaria a inventar autorização; uma
    /// interface sem esta resposta esconde o botão a toda a gente.
    ///
    /// **Não é autorização.** É o que a interface pode oferecer sem prometer
    /// uma recusa: a operação volta a exigir a permissão quando correr.
    may_validate: bool,
}

impl From<science::Result> for ResultView {
    fn from(r: science::Result) -> Self {
        Self {
            status_label: r.status_label(),
            id: r.id,
            workspace_id: r.workspace_id,
            execution_id: r.execution_id,
            title: r.title,
            summary: r.summary,
            status: r.status,
            classification: r.classification,
            superseded_by_id: r.superseded_by_id,
            created_at: r.created_at.to_rfc3339(),
            may_validate: false,
        }
    }
}

impl ResultView {
    /// A mesma vista, sabendo quem pergunta e onde o resultado vive.
    fn for_principal(
        result: science::Result,
        workspace: &ocinye_core::modules::research::ResearchWorkspace,
        principal: &ocinye_domain::Principal,
    ) -> Self {
        let ctx = ocinye_core::modules::research::workspace_context(
            workspace,
            ocinye_domain::ResourceKind::Result,
        );
        let id = result.id;
        Self {
            may_validate: ocinye_domain::can(
                principal,
                ocinye_contracts::Permission::ResultsValidate,
                &ctx,
                Some(id),
            )
            .allowed,
            ..Self::from(result)
        }
    }
}

#[derive(Serialize)]
struct ValidationView {
    id: Uuid,
    result_id: Uuid,
    kind: String,
    outcome: String,
    label: String,
    execution_id: Option<Uuid>,
    methodology_version_id: Option<Uuid>,
    note: Option<String>,
    created_at: String,
}

impl From<science::ResultValidation> for ValidationView {
    fn from(v: science::ResultValidation) -> Self {
        Self {
            label: v.label(),
            id: v.id,
            result_id: v.result_id,
            kind: v.kind,
            outcome: v.outcome,
            execution_id: v.execution_id,
            methodology_version_id: v.methodology_version_id,
            note: v.note,
            created_at: v.created_at.to_rfc3339(),
        }
    }
}

// ── Entradas ────────────────────────────────────────────────────────────

fn classificacao(raw: Option<&str>) -> Result<Classification, CoreError> {
    match raw {
        None => Ok(Classification::Internal),
        Some(value) => Classification::parse(value)
            .ok_or_else(|| CoreError::Validation("Classificação desconhecida.".to_owned())),
    }
}

#[derive(Deserialize)]
struct CreateHypothesisRequest {
    statement: String,
    rationale: Option<String>,
    classification: Option<String>,
}

#[derive(Deserialize)]
struct CreateMethodologyRequest {
    title: String,
    purpose: Option<String>,
    classification: Option<String>,
}

#[derive(Deserialize)]
struct PublishVersionRequest {
    label: String,
    summary: String,
    document_id: Option<Uuid>,
}

#[derive(Deserialize)]
struct CreateStudyRequest {
    title: String,
    kind: String,
    objective: Option<String>,
    hypothesis_id: Option<Uuid>,
    /// A **versão** de metodologia que o estudo segue.
    ///
    /// Nunca a metodologia: a matriz de proveniência aceita
    /// `Study → MethodologyVersion` e recusa o resto, porque uma aresta para a
    /// metodologia deixaria de descrever o que foi feito assim que alguém a
    /// melhorasse.
    methodology_version_id: Option<Uuid>,
    classification: Option<String>,
}

#[derive(Deserialize)]
struct RecordExecutionRequest {
    status: Option<String>,
    /// A versão de metodologia que esta corrida seguiu.
    methodology_version_id: Option<Uuid>,
    /// As versões de dataset que entraram nesta corrida.
    #[serde(default)]
    dataset_version_ids: Vec<Uuid>,
    compute_node_id: Option<Uuid>,
    environment: Option<String>,
    software_name: Option<String>,
    software_version: Option<String>,
    software_commit: Option<String>,
    notes: Option<String>,
}

#[derive(Deserialize)]
struct CreateResultRequest {
    title: String,
    summary: String,
    execution_id: Option<Uuid>,
    classification: Option<String>,
}

#[derive(Deserialize)]
struct RecordValidationRequest {
    kind: String,
    outcome: String,
    execution_id: Option<Uuid>,
    note: Option<String>,
}

#[derive(Deserialize)]
struct LineageQuery {
    /// `upstream` ou `downstream`. Por omissão, `upstream`.
    direction: Option<String>,
    /// Quantos saltos, até ao tecto que o Core impõe.
    depth: Option<u8>,
}

// ── Hipóteses ───────────────────────────────────────────────────────────

async fn list_hypotheses(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(workspace_id): Path<Uuid>,
) -> Result<Json<Vec<HypothesisView>>, ApiError> {
    let items = science::list_hypotheses(&state.pool, &principal, workspace_id).await?;
    Ok(Json(items.into_iter().map(HypothesisView::from).collect()))
}

async fn get_hypothesis(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(hypothesis_id): Path<Uuid>,
) -> Result<Json<HypothesisView>, ApiError> {
    let (hypothesis, _) = science::get_hypothesis(&state.pool, &principal, hypothesis_id).await?;
    Ok(Json(HypothesisView::from(hypothesis)))
}

async fn create_hypothesis(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path(workspace_id): Path<Uuid>,
    Json(request): Json<CreateHypothesisRequest>,
) -> Result<Json<HypothesisView>, ApiError> {
    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let hypothesis = science::create_hypothesis(
        &mut tx,
        &principal,
        &ids,
        workspace_id,
        &request.statement,
        request.rationale.as_deref(),
        classificacao(request.classification.as_deref())?,
    )
    .await?;
    tx.commit().await.map_err(CoreError::from)?;
    Ok(Json(HypothesisView::from(hypothesis)))
}

// ── Metodologias ────────────────────────────────────────────────────────

async fn list_methodologies(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(workspace_id): Path<Uuid>,
) -> Result<Json<Vec<MethodologyView>>, ApiError> {
    let items = science::list_methodologies(&state.pool, &principal, workspace_id).await?;
    Ok(Json(items.into_iter().map(MethodologyView::from).collect()))
}

async fn get_methodology(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(methodology_id): Path<Uuid>,
) -> Result<Json<MethodologyView>, ApiError> {
    let (methodology, _) =
        science::get_methodology(&state.pool, &principal, methodology_id).await?;
    Ok(Json(MethodologyView::from(methodology)))
}

async fn create_methodology(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path(workspace_id): Path<Uuid>,
    Json(request): Json<CreateMethodologyRequest>,
) -> Result<Json<MethodologyView>, ApiError> {
    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let methodology = science::create_methodology(
        &mut tx,
        &principal,
        &ids,
        workspace_id,
        &request.title,
        request.purpose.as_deref(),
        classificacao(request.classification.as_deref())?,
    )
    .await?;
    tx.commit().await.map_err(CoreError::from)?;
    Ok(Json(MethodologyView::from(methodology)))
}

async fn list_methodology_versions(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(methodology_id): Path<Uuid>,
) -> Result<Json<Vec<MethodologyVersionView>>, ApiError> {
    let items = science::list_methodology_versions(&state.pool, &principal, methodology_id).await?;
    Ok(Json(
        items
            .into_iter()
            .map(MethodologyVersionView::from)
            .collect(),
    ))
}

async fn get_version(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(version_id): Path<Uuid>,
) -> Result<Json<MethodologyVersionView>, ApiError> {
    let (version, _, _) =
        science::get_methodology_version(&state.pool, &principal, version_id).await?;
    Ok(Json(MethodologyVersionView::from(version)))
}

async fn publish_methodology_version(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path(methodology_id): Path<Uuid>,
    Json(request): Json<PublishVersionRequest>,
) -> Result<Json<MethodologyVersionView>, ApiError> {
    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let version = science::publish_methodology_version(
        &mut tx,
        &state.pool,
        &principal,
        &ids,
        methodology_id,
        &request.label,
        &request.summary,
        request.document_id,
    )
    .await?;
    tx.commit().await.map_err(CoreError::from)?;
    Ok(Json(MethodologyVersionView::from(version)))
}

// ── Estudos e execuções ─────────────────────────────────────────────────

async fn list_studies(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(workspace_id): Path<Uuid>,
) -> Result<Json<Vec<StudyView>>, ApiError> {
    let items = science::list_studies(&state.pool, &principal, workspace_id).await?;
    Ok(Json(items.into_iter().map(StudyView::from).collect()))
}

async fn get_study(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(study_id): Path<Uuid>,
) -> Result<Json<StudyView>, ApiError> {
    let (study, _) = science::get_study(&state.pool, &principal, study_id).await?;
    Ok(Json(StudyView::from(study)))
}

async fn create_study(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path(workspace_id): Path<Uuid>,
    Json(request): Json<CreateStudyRequest>,
) -> Result<Json<StudyView>, ApiError> {
    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let study = science::create_study(
        &mut tx,
        &state.pool,
        &principal,
        &ids,
        workspace_id,
        request.hypothesis_id,
        request.methodology_version_id,
        &request.title,
        &request.kind,
        request.objective.as_deref(),
        classificacao(request.classification.as_deref())?,
    )
    .await?;
    tx.commit().await.map_err(CoreError::from)?;
    Ok(Json(StudyView::from(study)))
}

async fn list_executions(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(study_id): Path<Uuid>,
) -> Result<Json<Vec<ExecutionView>>, ApiError> {
    let items = science::list_executions(&state.pool, &principal, study_id).await?;
    Ok(Json(items.into_iter().map(ExecutionView::from).collect()))
}

async fn get_execution(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(execution_id): Path<Uuid>,
) -> Result<Json<ExecutionView>, ApiError> {
    let (execution, _, _) = science::get_execution(&state.pool, &principal, execution_id).await?;
    Ok(Json(ExecutionView::from(execution)))
}

async fn record_execution(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path(study_id): Path<Uuid>,
    Json(request): Json<RecordExecutionRequest>,
) -> Result<Json<ExecutionView>, ApiError> {
    // Vazio, e não um valor por omissão escrito aqui: quem decide o que uma
    // execução é quando ninguém o diz é o Core.
    let status = request.status.as_deref().unwrap_or_default();
    let record = science::ExecutionRecord {
        status,
        compute_node_id: request.compute_node_id,
        environment: request.environment.as_deref(),
        software_name: request.software_name.as_deref(),
        software_version: request.software_version.as_deref(),
        software_commit: request.software_commit.as_deref(),
        notes: request.notes.as_deref(),
        methodology_version_id: request.methodology_version_id,
        dataset_version_ids: &request.dataset_version_ids,
    };

    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let execution =
        science::record_execution(&mut tx, &state.pool, &principal, &ids, study_id, &record)
            .await?;
    tx.commit().await.map_err(CoreError::from)?;
    Ok(Json(ExecutionView::from(execution)))
}

// ── Resultados e validações ─────────────────────────────────────────────

async fn list_results(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(workspace_id): Path<Uuid>,
) -> Result<Json<Vec<ResultView>>, ApiError> {
    let items = science::list_results(&state.pool, &principal, workspace_id).await?;
    Ok(Json(items.into_iter().map(ResultView::from).collect()))
}

async fn get_result(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(result_id): Path<Uuid>,
) -> Result<Json<ResultView>, ApiError> {
    let (result, workspace) = science::get_result(&state.pool, &principal, result_id).await?;
    Ok(Json(ResultView::for_principal(
        result, &workspace, &principal,
    )))
}

async fn create_result(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path(workspace_id): Path<Uuid>,
    Json(request): Json<CreateResultRequest>,
) -> Result<Json<ResultView>, ApiError> {
    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let result = science::create_result(
        &mut tx,
        &state.pool,
        &principal,
        &ids,
        workspace_id,
        request.execution_id,
        &request.title,
        &request.summary,
        classificacao(request.classification.as_deref())?,
    )
    .await?;
    tx.commit().await.map_err(CoreError::from)?;
    Ok(Json(ResultView::from(result)))
}

async fn list_validations(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(result_id): Path<Uuid>,
) -> Result<Json<Vec<ValidationView>>, ApiError> {
    let items = science::list_validations(&state.pool, &principal, result_id).await?;
    Ok(Json(items.into_iter().map(ValidationView::from).collect()))
}

async fn record_validation(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path(result_id): Path<Uuid>,
    Json(request): Json<RecordValidationRequest>,
) -> Result<Json<ValidationView>, ApiError> {
    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let validation = science::record_validation(
        &mut tx,
        &state.pool,
        &principal,
        &ids,
        result_id,
        &request.kind,
        &request.outcome,
        request.execution_id,
        request.note.as_deref(),
    )
    .await?;
    tx.commit().await.map_err(CoreError::from)?;
    Ok(Json(ValidationView::from(validation)))
}

// ── Linhagem ────────────────────────────────────────────────────────────

async fn lineage(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path((kind, resource_id)): Path<(String, Uuid)>,
    Query(query): Query<LineageQuery>,
) -> Result<Json<science::Linhagem>, ApiError> {
    // Um tipo que o domínio não conhece é uma pergunta sem resposta, e não uma
    // travessia vazia. Devolver vazio diria «este recurso não tem linhagem»,
    // que é outra coisa.
    let kind = AgenticKind::parse(&kind)
        .ok_or_else(|| CoreError::Validation(format!("«{kind}» não é um tipo de recurso.")))?;

    let sentido = match query.direction.as_deref() {
        Some("downstream") => science::Sentido::Jusante,
        Some("upstream") | None => science::Sentido::Montante,
        Some(outro) => {
            return Err(CoreError::Validation(format!(
                "«{outro}» não é um sentido. Usa upstream ou downstream."
            ))
            .into())
        }
    };

    let linhagem = science::percorrer(
        &state.pool,
        &principal,
        &ResourceRef {
            kind,
            id: resource_id,
            label: None,
        },
        sentido,
        query.depth.unwrap_or(science::PROFUNDIDADE_MAXIMA),
    )
    .await?;

    Ok(Json(linhagem))
}
