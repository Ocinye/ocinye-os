//! Collaboration routes: datasets, tasks, comments and activity.
//!
//! Datasets live here rather than in a module of their own because the Data
//! Plane's HTTP surface is small and workspace-scoped like the rest of this
//! group. The domain separation is preserved in [`ocinye_core::modules::data`].

use axum::extract::{DefaultBodyLimit, Multipart, Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use ocinye_contracts::{Page, PageRequest, TaskState};
use ocinye_core::modules::{collaboration, data};
use ocinye_core::CoreError;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ApiError;
use crate::extract::{CurrentPrincipal, Ids};
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/tasks", get(list_tasks))
        .route("/workspaces/{workspace_id}/tasks", post(create_task))
        .route("/tasks/{task_id}/transitions", post(transition_task))
        .route("/workspaces/{workspace_id}/comments", post(create_comment))
        .route(
            "/workspaces/{workspace_id}/comments/list",
            get(list_comments),
        )
        .route("/activity", get(list_activity))
        .route("/datasets", get(list_datasets))
        .route("/workspaces/{workspace_id}/datasets", post(create_dataset))
        .route(
            "/datasets/{dataset_id}/versions",
            get(list_versions).post(create_version),
        )
        .route(
            "/datasets/{dataset_id}/versions/{version_id}/files",
            post(add_file).layer(DefaultBodyLimit::max(super::UPLOAD_BODY_LIMIT_BYTES)),
        )
        .route(
            "/datasets/{dataset_id}/versions/{version_id}/publish",
            post(publish_version),
        )
}

fn page_of(page: Option<u32>, page_size: Option<u32>) -> PageRequest {
    PageRequest {
        page: page.unwrap_or(1),
        page_size: page_size.unwrap_or(ocinye_contracts::page::DEFAULT_PAGE_SIZE),
    }
}

// --- Tasks -----------------------------------------------------------------

#[derive(Serialize)]
struct TaskView {
    id: Uuid,
    workspace_id: Uuid,
    title: String,
    description: Option<String>,
    state: String,
    priority: String,
    assignee_id: Option<Uuid>,
    due_on: Option<chrono::NaiveDate>,
    classification: String,
}

impl From<collaboration::Task> for TaskView {
    fn from(task: collaboration::Task) -> Self {
        Self {
            id: task.id,
            workspace_id: task.workspace_id,
            title: task.title,
            description: task.description,
            state: task.state,
            priority: task.priority,
            assignee_id: task.assignee_id,
            due_on: task.due_on,
            classification: task.classification,
        }
    }
}

#[derive(Deserialize)]
struct ListTasksQuery {
    #[serde(default)]
    workspace_id: Option<Uuid>,
    /// `mine=true` scopes to the caller, which is what the dashboard asks.
    #[serde(default)]
    mine: bool,
    #[serde(default = "default_true")]
    open_only: bool,
    #[serde(default)]
    page: Option<u32>,
    #[serde(default)]
    page_size: Option<u32>,
}

const fn default_true() -> bool {
    true
}

async fn list_tasks(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Query(query): Query<ListTasksQuery>,
) -> Result<Json<Page<TaskView>>, ApiError> {
    let page = page_of(query.page, query.page_size);
    let assignee = query.mine.then_some(principal.person_id);

    let (tasks, total) = collaboration::list_tasks(
        &state.pool,
        &principal,
        query.workspace_id,
        assignee,
        query.open_only,
        page,
    )
    .await?;

    Ok(Json(Page::new(
        tasks.into_iter().map(TaskView::from).collect(),
        page,
        total,
    )))
}

#[derive(Deserialize)]
struct CreateTaskRequest {
    title: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    priority: Option<String>,
    #[serde(default)]
    assignee_id: Option<Uuid>,
    #[serde(default)]
    due_on: Option<chrono::NaiveDate>,
}

async fn create_task(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path(workspace_id): Path<Uuid>,
    Json(request): Json<CreateTaskRequest>,
) -> Result<Json<TaskView>, ApiError> {
    let priority = request
        .priority
        .as_deref()
        .map(|raw| {
            collaboration::TaskPriority::parse(raw)
                .ok_or_else(|| CoreError::Validation("Unknown task priority.".to_owned()))
        })
        .transpose()?
        .unwrap_or_default();

    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let task = collaboration::create_task(
        &mut tx,
        &principal,
        &ids,
        collaboration::NewTask {
            workspace_id,
            title: request.title,
            description: request.description,
            priority,
            assignee_id: request.assignee_id,
            due_on: request.due_on,
        },
    )
    .await?;
    tx.commit().await.map_err(CoreError::from)?;

    Ok(Json(TaskView::from(task)))
}

#[derive(Deserialize)]
struct TransitionTaskRequest {
    state: String,
}

async fn transition_task(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path(task_id): Path<Uuid>,
    Json(request): Json<TransitionTaskRequest>,
) -> Result<Json<TaskView>, ApiError> {
    let target = TaskState::parse(&request.state)
        .ok_or_else(|| CoreError::Validation("Unknown task state.".to_owned()))?;

    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let task = collaboration::transition_task(&mut tx, &principal, &ids, task_id, target).await?;
    tx.commit().await.map_err(CoreError::from)?;

    Ok(Json(TaskView::from(task)))
}

// --- Comments and activity -------------------------------------------------

#[derive(Deserialize)]
struct CreateCommentRequest {
    subject_type: String,
    subject_id: Uuid,
    body: String,
}

async fn create_comment(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(workspace_id): Path<Uuid>,
    Json(request): Json<CreateCommentRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let comment = collaboration::add_comment(
        &mut tx,
        &principal,
        workspace_id,
        &request.subject_type,
        request.subject_id,
        &request.body,
    )
    .await?;
    tx.commit().await.map_err(CoreError::from)?;

    Ok(Json(serde_json::json!({ "comment_id": comment.id })))
}

#[derive(Deserialize)]
struct ListCommentsQuery {
    subject_type: String,
    subject_id: Uuid,
}

#[derive(Serialize)]
struct CommentView {
    id: Uuid,
    body: String,
    created_by_id: Option<Uuid>,
    created_at: chrono::DateTime<chrono::Utc>,
    withdrawn: bool,
}

async fn list_comments(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(workspace_id): Path<Uuid>,
    Query(query): Query<ListCommentsQuery>,
) -> Result<Json<Vec<CommentView>>, ApiError> {
    let comments = collaboration::list_comments(
        &state.pool,
        &principal,
        workspace_id,
        &query.subject_type,
        query.subject_id,
    )
    .await?;

    Ok(Json(
        comments
            .into_iter()
            .map(|comment| CommentView {
                id: comment.id,
                body: comment.body,
                created_by_id: comment.created_by_id,
                created_at: comment.created_at,
                withdrawn: comment.withdrawn_at.is_some(),
            })
            .collect(),
    ))
}

#[derive(Deserialize)]
struct ActivityQuery {
    #[serde(default)]
    workspace_id: Option<Uuid>,
    #[serde(default)]
    page: Option<u32>,
    #[serde(default)]
    page_size: Option<u32>,
}

#[derive(Serialize)]
struct ActivityView {
    id: Uuid,
    workspace_id: Uuid,
    actor_name: Option<String>,
    kind: String,
    subject_type: String,
    subject_id: Option<Uuid>,
    summary: String,
    classification: String,
    created_at: chrono::DateTime<chrono::Utc>,
}

/// The collaboration feed.
///
/// Distinct from the audit trail: this carries only what a colleague may
/// already see, and it is filtered by the same visibility rules as everything
/// else (briefing §45).
async fn list_activity(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Query(query): Query<ActivityQuery>,
) -> Result<Json<Vec<ActivityView>>, ApiError> {
    let entries = collaboration::list_activity(
        &state.pool,
        &principal,
        query.workspace_id,
        page_of(query.page, query.page_size),
    )
    .await?;

    Ok(Json(
        entries
            .into_iter()
            .map(|entry| ActivityView {
                id: entry.id,
                workspace_id: entry.workspace_id,
                actor_name: entry.actor_name,
                kind: entry.kind,
                subject_type: entry.subject_type,
                subject_id: entry.subject_id,
                summary: entry.summary,
                classification: entry.classification,
                created_at: entry.created_at,
            })
            .collect(),
    ))
}

// --- Datasets --------------------------------------------------------------

#[derive(Serialize)]
struct DatasetView {
    id: Uuid,
    workspace_id: Uuid,
    code: String,
    title: String,
    description: Option<String>,
    origin: String,
    licence: Option<String>,
    usage_restrictions: Option<String>,
    keywords: Vec<String>,
    classification: String,
    state: String,
}

impl From<data::Dataset> for DatasetView {
    fn from(dataset: data::Dataset) -> Self {
        Self {
            id: dataset.id,
            workspace_id: dataset.workspace_id,
            code: dataset.code,
            title: dataset.title,
            description: dataset.description,
            origin: dataset.origin,
            licence: dataset.licence,
            usage_restrictions: dataset.usage_restrictions,
            keywords: dataset.keywords,
            classification: dataset.classification,
            state: dataset.state,
        }
    }
}

#[derive(Deserialize)]
struct ListDatasetsQuery {
    #[serde(default)]
    workspace_id: Option<Uuid>,
    #[serde(default)]
    page: Option<u32>,
    #[serde(default)]
    page_size: Option<u32>,
}

async fn list_datasets(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Query(query): Query<ListDatasetsQuery>,
) -> Result<Json<Page<DatasetView>>, ApiError> {
    let page = page_of(query.page, query.page_size);
    let (datasets, total) =
        data::list_datasets(&state.pool, &principal, query.workspace_id, page).await?;
    Ok(Json(Page::new(
        datasets.into_iter().map(DatasetView::from).collect(),
        page,
        total,
    )))
}

#[derive(Deserialize)]
struct CreateDatasetRequest {
    code: String,
    title: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    origin: Option<String>,
    #[serde(default)]
    licence: Option<String>,
    #[serde(default)]
    usage_restrictions: Option<String>,
    #[serde(default)]
    responsible_person_id: Option<Uuid>,
    #[serde(default)]
    acquisition_date: Option<chrono::NaiveDate>,
    #[serde(default)]
    keywords: Vec<String>,
    #[serde(default)]
    classification: Option<String>,
}

async fn create_dataset(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path(workspace_id): Path<Uuid>,
    Json(request): Json<CreateDatasetRequest>,
) -> Result<Json<DatasetView>, ApiError> {
    let origin = request
        .origin
        .as_deref()
        .map(|raw| {
            data::DatasetOrigin::parse(raw)
                .ok_or_else(|| CoreError::Validation("Unknown dataset origin.".to_owned()))
        })
        .transpose()?
        .unwrap_or_default();

    let classification = request
        .classification
        .as_deref()
        .map(|raw| {
            ocinye_contracts::Classification::parse(raw)
                .ok_or_else(|| CoreError::Validation("Unknown classification.".to_owned()))
        })
        .transpose()?;

    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let dataset = data::create_dataset(
        &mut tx,
        &principal,
        &ids,
        workspace_id,
        data::NewDataset {
            code: request.code,
            title: request.title,
            description: request.description,
            origin,
            licence: request.licence,
            usage_restrictions: request.usage_restrictions,
            responsible_person_id: request.responsible_person_id,
            acquisition_date: request.acquisition_date,
            keywords: request.keywords,
            classification,
        },
    )
    .await?;
    tx.commit().await.map_err(CoreError::from)?;

    Ok(Json(DatasetView::from(dataset)))
}

#[derive(Serialize)]
struct VersionView {
    id: Uuid,
    label: String,
    status: String,
    notes: Option<String>,
    provenance: Option<String>,
    file_count: i32,
    total_size_bytes: i64,
    published_at: Option<chrono::DateTime<chrono::Utc>>,
    files: Vec<data::DatasetFile>,
}

async fn list_versions(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(dataset_id): Path<Uuid>,
) -> Result<Json<Vec<VersionView>>, ApiError> {
    let versions = data::list_versions(&state.pool, &principal, dataset_id).await?;
    Ok(Json(
        versions
            .into_iter()
            .map(|(version, files)| VersionView {
                id: version.id,
                label: version.label,
                status: version.status,
                notes: version.notes,
                provenance: version.provenance,
                file_count: version.file_count,
                total_size_bytes: version.total_size_bytes,
                published_at: version.published_at,
                files,
            })
            .collect(),
    ))
}

#[derive(Deserialize)]
struct CreateVersionRequest {
    label: String,
    #[serde(default)]
    notes: Option<String>,
    #[serde(default)]
    provenance: Option<String>,
    #[serde(default)]
    derived_from_version_id: Option<Uuid>,
}

async fn create_version(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path(dataset_id): Path<Uuid>,
    Json(request): Json<CreateVersionRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let version = data::create_version(
        &mut tx,
        &principal,
        &ids,
        dataset_id,
        data::NewVersion {
            label: request.label,
            notes: request.notes,
            provenance: request.provenance,
            derived_from_version_id: request.derived_from_version_id,
        },
    )
    .await?;
    tx.commit().await.map_err(CoreError::from)?;

    Ok(Json(
        serde_json::json!({ "version_id": version.id, "label": version.label }),
    ))
}

async fn add_file(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path((dataset_id, version_id)): Path<(Uuid, Uuid)>,
    multipart: Multipart,
) -> Result<Json<serde_json::Value>, ApiError> {
    let upload = super::knowledge::read_upload_public(multipart).await?;
    let store = state.store()?;

    let path = upload
        .fields
        .get("path")
        .cloned()
        .unwrap_or_else(|| upload.filename.clone());

    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let file_id = data::add_version_file(
        &mut tx,
        &principal,
        &ids,
        store,
        &state.config.organisation_slug,
        dataset_id,
        version_id,
        &path,
        &upload.filename,
        &upload.content_type,
        upload.data,
    )
    .await?;
    tx.commit().await.map_err(CoreError::from)?;

    Ok(Json(serde_json::json!({ "file_id": file_id })))
}

/// Publish a draft version, making it immutable.
///
/// Earlier versions remain readable and citable: a result that cited version 1
/// must stay reproducible after version 2 exists.
async fn publish_version(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path((dataset_id, version_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let version = data::publish_version(&mut tx, &principal, &ids, dataset_id, version_id).await?;
    tx.commit().await.map_err(CoreError::from)?;

    Ok(Json(serde_json::json!({
        "version_id": version.id,
        "label": version.label,
        "status": version.status,
    })))
}
