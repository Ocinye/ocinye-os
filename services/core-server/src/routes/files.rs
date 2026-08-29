//! Institutional file routes: browsing, folders, versions and downloads.
//!
//! Every path here resolves through the same authority: `File`. A folder is a
//! place to arrange files, not a place that decides who may read them; a
//! version has no classification of its own; and knowing an identifier — of a
//! file, of a version, of a folder — grants nothing that the file itself would
//! refuse.
//!
//! The Core never proxies bytes. Uploads arrive as multipart and downloads
//! leave as short-lived signed URLs, which is the only way to obtain one.

use axum::extract::{DefaultBodyLimit, Multipart, Path, Query, State};
use axum::handler::Handler;
use axum::routing::{get, post};
use axum::{Json, Router};
use ocinye_contracts::Classification;
use ocinye_core::modules::files;
use ocinye_core::CoreError;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ApiError;
use crate::extract::{CurrentPrincipal, Ids};
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        // Navegar e carregar, no mesmo sítio: é a mesma pasta vista e a mesma
        // pasta onde o ficheiro cai.
        .route(
            "/workspaces/{workspace_id}/files",
            get(browse)
                .post(upload_file.layer(DefaultBodyLimit::max(super::UPLOAD_BODY_LIMIT_BYTES))),
        )
        .route("/workspaces/{workspace_id}/folders", post(create_folder))
        .route("/files/{file_id}", get(show_file))
        .route(
            "/files/{file_id}/versions",
            get(list_versions)
                .post(upload_version.layer(DefaultBodyLimit::max(super::UPLOAD_BODY_LIMIT_BYTES))),
        )
        .route("/files/{file_id}/download", get(download_file))
        // A representação inline. Não é a descarga, e não emite ligação
        // assinada nenhuma: os bytes saem por aqui, na origem do Core.
        .route("/files/{file_id}/preview", get(preview_file))
        // A versão exacta tem caminho próprio porque é um recurso próprio: «o
        // ficheiro» aponta para bytes que mudam, «a versão 3» não.
        .route(
            "/file-versions/{version_id}/download",
            get(download_version),
        )
        .route("/files/{file_id}/folder", post(move_file))
}

// --- Views -----------------------------------------------------------------

#[derive(Serialize)]
struct FolderView {
    id: Uuid,
    name: String,
    parent_id: Option<Uuid>,
}

impl From<files::FolderRecord> for FolderView {
    fn from(f: files::FolderRecord) -> Self {
        Self {
            id: f.id,
            name: f.name,
            parent_id: f.parent_id,
        }
    }
}

#[derive(Serialize)]
struct FileView {
    id: Uuid,
    name: String,
    classification: String,
    content_type: String,
    size_bytes: i64,
    versions: i64,
    updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<files::FileListing> for FileView {
    fn from(f: files::FileListing) -> Self {
        Self {
            id: f.id,
            name: f.name,
            classification: f.classification,
            content_type: f.content_type,
            size_bytes: f.size_bytes,
            versions: f.versions,
            updated_at: f.updated_at,
        }
    }
}

#[derive(Serialize)]
struct FolderContentsView {
    /// Se quem navega pode criar aqui. Cortesia de renderização, nunca autoridade.
    may_create: bool,
    /// Da raiz até à pasta actual. Vazio na raiz.
    path: Vec<FolderView>,
    folders: Vec<FolderView>,
    files: Vec<FileView>,
}

#[derive(Serialize)]
struct VersionView {
    id: Uuid,
    sequence: i32,
    /// Se estes bytes se mostram inline.
    ///
    /// Vem daqui e não da Experience: a lista de tipos é uma decisão de
    /// segurança do Core, e um cliente que a recalculasse teria uma segunda
    /// opinião sobre onde um SVG pode ser servido.
    previewable: bool,
    original_filename: String,
    content_type: String,
    size_bytes: i64,
    checksum_sha256: String,
    note: Option<String>,
    created_by: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

impl From<files::VersionListing> for VersionView {
    fn from(v: files::VersionListing) -> Self {
        Self {
            id: v.id,
            sequence: v.sequence,
            previewable: files::PREVIEWABLE_TYPES.contains(&v.content_type.as_str()),
            original_filename: v.original_filename,
            content_type: v.content_type,
            size_bytes: v.size_bytes,
            checksum_sha256: v.checksum_sha256,
            note: v.note,
            created_by: v.created_by,
            created_at: v.created_at,
        }
    }
}

// --- Browsing --------------------------------------------------------------

#[derive(Deserialize)]
struct BrowseQuery {
    /// A pasta a abrir. Ausente é a raiz do ambiente.
    #[serde(default)]
    folder: Option<Uuid>,
}

async fn browse(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(workspace_id): Path<Uuid>,
    Query(query): Query<BrowseQuery>,
) -> Result<Json<FolderContentsView>, ApiError> {
    let contents = files::browse(&state.pool, &principal, workspace_id, query.folder).await?;
    Ok(Json(FolderContentsView {
        may_create: contents.may_create,
        path: contents.path.into_iter().map(FolderView::from).collect(),
        folders: contents.folders.into_iter().map(FolderView::from).collect(),
        files: contents.files.into_iter().map(FileView::from).collect(),
    }))
}

async fn show_file(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(file_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut conn = state.pool.acquire().await.map_err(CoreError::from)?;
    let (ficheiro, workspace) = files::get(&mut conn, &principal, file_id).await?;

    // A mesma pergunta que `upload_version` faz. Vem daqui e não da lista de
    // capacidades do `/me`, que é de âmbito institucional: quem pode escrever
    // neste ficheiro pode não ter esse direito à escala da instituição, e o
    // contrário também é verdade.
    let may_write = files::may_write(&principal, &workspace, ficheiro.classification());

    // O estado da leitura do corpo, para o ecrã poder distinguir «guardado e a
    // processar» de «guardado e não pesquisável» — que não são a mesma coisa, e
    // nenhuma delas é «o carregamento falhou».
    let extraccao = files::extraction::status_of_current(&mut *conn, file_id).await?;

    Ok(Json(serde_json::json!({
        "may_write": may_write,
        "extraction_status": extraccao.map(|(estado, _)| estado.as_str()),
        "extraction_chunks": extraccao.map(|(_, chunks)| chunks),
        "id": ficheiro.id,
        "name": ficheiro.name,
        "classification": ficheiro.classification().as_str(),
        "workspace_id": ficheiro.workspace_id,
        "workspace_name": workspace.title,
        "workspace_classification": workspace.classification.as_str(),
        "unit_id": ficheiro.unit_id,
    })))
}

// --- Folders ---------------------------------------------------------------

#[derive(Deserialize)]
struct CreateFolderRequest {
    name: String,
    #[serde(default)]
    parent_id: Option<Uuid>,
}

async fn create_folder(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(workspace_id): Path<Uuid>,
    Json(request): Json<CreateFolderRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let folder_id = files::create_folder(
        &mut tx,
        &principal,
        workspace_id,
        request.parent_id,
        &request.name,
    )
    .await?;
    tx.commit().await.map_err(CoreError::from)?;
    Ok(Json(serde_json::json!({ "folder_id": folder_id })))
}

#[derive(Deserialize)]
struct MoveRequest {
    /// A pasta de destino, ou `null` para a raiz do mesmo ambiente.
    #[serde(default)]
    folder_id: Option<Uuid>,
}

async fn move_file(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path(file_id): Path<Uuid>,
    Json(request): Json<MoveRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    files::move_to_folder(&mut tx, &principal, &ids, file_id, request.folder_id).await?;
    tx.commit().await.map_err(CoreError::from)?;
    Ok(Json(serde_json::json!({ "moved": true })))
}

// --- Upload ----------------------------------------------------------------

fn parse_classification(raw: Option<&str>) -> Result<Option<Classification>, CoreError> {
    raw.map(|value| {
        Classification::parse(value)
            .ok_or_else(|| CoreError::Validation("Unknown classification.".to_owned()))
    })
    .transpose()
}

async fn upload_file(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path(workspace_id): Path<Uuid>,
    multipart: Multipart,
) -> Result<Json<serde_json::Value>, ApiError> {
    let upload = super::knowledge::read_upload_public(multipart).await?;
    let store = state.store()?;

    let folder_id = upload
        .fields
        .get("folder_id")
        .filter(|raw| !raw.is_empty())
        .map(|raw| {
            Uuid::parse_str(raw).map_err(|_| CoreError::Validation("Invalid folder.".to_owned()))
        })
        .transpose()?;

    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let version = files::create(
        &mut tx,
        &principal,
        &ids,
        store,
        &state.config.organisation_slug,
        workspace_id,
        files::NewFile {
            filename: upload.filename,
            content_type: upload.content_type,
            data: upload.data,
            classification: parse_classification(
                upload.fields.get("classification").map(String::as_str),
            )?,
        },
    )
    .await?;

    // A pasta é escolhida depois de o ficheiro existir, e pela mesma operação
    // que o move mais tarde: arrumar é sempre a mesma coisa, tenha o ficheiro
    // um segundo ou um ano.
    if let Some(folder_id) = folder_id {
        files::move_to_folder(&mut tx, &principal, &ids, version.file_id, Some(folder_id)).await?;
    }
    tx.commit().await.map_err(CoreError::from)?;

    Ok(Json(serde_json::json!({
        "file_id": version.file_id,
        "version_id": version.version_id,
        "sequence": version.sequence,
    })))
}

async fn upload_version(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path(file_id): Path<Uuid>,
    multipart: Multipart,
) -> Result<Json<serde_json::Value>, ApiError> {
    let upload = super::knowledge::read_upload_public(multipart).await?;
    let store = state.store()?;

    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let version = files::upload_version(
        &mut tx,
        &principal,
        &ids,
        store,
        &state.config.organisation_slug,
        file_id,
        files::NewFile {
            filename: upload.filename,
            content_type: upload.content_type,
            data: upload.data,
            classification: None,
        },
    )
    .await?;
    tx.commit().await.map_err(CoreError::from)?;

    Ok(Json(serde_json::json!({
        "file_id": version.file_id,
        "version_id": version.version_id,
        "sequence": version.sequence,
    })))
}

// --- History and downloads -------------------------------------------------

async fn list_versions(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(file_id): Path<Uuid>,
) -> Result<Json<Vec<VersionView>>, ApiError> {
    let mut conn = state.pool.acquire().await.map_err(CoreError::from)?;
    let versions = files::versions(&mut conn, &principal, file_id).await?;
    Ok(Json(versions.into_iter().map(VersionView::from).collect()))
}

async fn download_file(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path(file_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let store = state.store()?;
    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let url = files::download_url(&mut tx, &principal, &ids, store, file_id).await?;
    tx.commit().await.map_err(CoreError::from)?;
    Ok(Json(serde_json::json!({
        "url": url,
        "expires_in_seconds": ocinye_core::storage::DOWNLOAD_URL_TTL.as_secs(),
    })))
}

/// Serve os bytes da versão corrente inline.
///
/// Os cabeçalhos são tão importantes como os bytes. `nosniff` impede o browser
/// de decidir por si que aquilo é outra coisa; `inline` com o tipo validado diz
/// o que é; `private` mantém a resposta fora de qualquer cache partilhada, que
/// numa resposta por membro é o mínimo.
async fn preview_file(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path(file_id): Path<Uuid>,
) -> Result<axum::response::Response, ApiError> {
    use axum::http::header;
    use axum::response::IntoResponse;

    let store = state.store()?;
    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let vista = files::preview(&mut tx, &principal, &ids, store, file_id).await?;
    tx.commit().await.map_err(CoreError::from)?;

    // O `ETag` deriva da soma dos bytes guardados: uma versão nova tem outros
    // bytes e outra soma, pelo que a validação nunca serve conteúdo velho.
    let etag = format!("\"{}\"", vista.checksum_sha256);

    Ok((
        [
            (header::CONTENT_TYPE, vista.content_type),
            (header::CONTENT_DISPOSITION, "inline".to_owned()),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_owned()),
            (
                header::CACHE_CONTROL,
                "private, max-age=0, must-revalidate".to_owned(),
            ),
            (header::ETAG, etag),
        ],
        vista.bytes,
    )
        .into_response())
}

async fn download_version(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path(version_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let store = state.store()?;
    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let url = files::version_download_url(&mut tx, &principal, &ids, store, version_id).await?;
    tx.commit().await.map_err(CoreError::from)?;
    Ok(Json(serde_json::json!({
        "url": url,
        "expires_in_seconds": ocinye_core::storage::DOWNLOAD_URL_TTL.as_secs(),
    })))
}
