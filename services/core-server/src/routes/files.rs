//! Institutional file routes: browsing, folders, versions and downloads.
//!
//! Every path here resolves through the same authority: `File`. A folder is a
//! place to arrange files, not a place that decides who may read them; a
//! version has no classification of its own; and knowing an identifier — of a
//! file, of a version, of a folder — grants nothing that the file itself would
//! refuse.
//!
//! For institutional files the Core never proxies bytes: uploads arrive as
//! multipart and downloads leave as short-lived signed URLs. Personal files are
//! the exception the deployment forces — the object store has no public
//! endpoint, so a signed URL would name the internal host and no browser could
//! reach it. There, the Core serves the bytes same-origin (`/me/files/{v}/raw`,
//! `…/preview`, `…/text`), and the storage location never reaches the page.

use axum::extract::{DefaultBodyLimit, Multipart, Path, Query, State};
use axum::handler::Handler;
use axum::routing::{get, post, put};
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
        // ── Carregamento em partes ──────────────────────────────────────
        //
        // O caminho para ficheiros que não cabem num pedido. O limite de corpo
        // destas rotas é o de **um pedaço**, e não o do ficheiro: aceitar aqui
        // seiscentos megabytes permitiria contornar a segmentação e voltar a
        // bater no limite do edge — que é o problema que isto resolve.
        .route("/workspaces/{workspace_id}/uploads", post(begin_upload))
        .route(
            "/uploads/{session_id}",
            get(upload_state).delete(cancel_upload),
        )
        .route(
            "/uploads/{session_id}/parts/{part_number}",
            put(upload_part.layer(DefaultBodyLimit::max(CHUNK_BODY_LIMIT_BYTES))),
        )
        .route("/uploads/{session_id}/complete", post(complete_upload))
        // A vista agregada: `Ficheiros` é um módulo, não a vista de um
        // ambiente. Obrigar a escolher um antes de ver seja o que for faz a
        // aplicação parecer vazia a quem tem trabalho espalhado por vários.
        .route("/files", get(all_files))
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
        // O texto extraído. Uma porta, e a mesma autoridade do ficheiro.
        .route("/files/{file_id}/content", get(file_content))
        // O conteúdo de uma versão exacta. Uma citação aponta para a v2, e
        // abrir a v2 tem de mostrar a v2 — não «o que o ficheiro diz agora».
        .route(
            "/file-versions/{version_id}/content",
            get(file_version_content),
        )
        .route(
            "/file-versions/{version_id}/preview",
            get(preview_file_version),
        )
        // A versão exacta tem caminho próprio porque é um recurso próprio: «o
        // ficheiro» aponta para bytes que mudam, «a versão 3» não.
        .route(
            "/file-versions/{version_id}/download",
            get(download_version),
        )
        .route("/files/{file_id}/folder", post(move_file))
        // ── Meus ficheiros ──────────────────────────────────────────────
        //
        // O espaço pessoal de todo o membro activo. Existe sem unidade,
        // projecto ou ambiente de investigação — e sem IA. É o que faz «Meus
        // ficheiros» nunca estar sem destino (ADR-0207). A autoridade é a
        // posse; a quota é a do armazenamento pessoal já governado.
        .route("/me/files", get(my_files))
        .route(
            "/me/files/uploads",
            post(upload_my_file.layer(DefaultBodyLimit::max(super::UPLOAD_BODY_LIMIT_BYTES))),
        )
        .route("/me/files/rename", post(rename_my_file))
        .route("/me/files/move", post(move_my_file))
        .route("/me/files/trash", get(my_trash))
        .route("/me/files/delete", post(trash_my_file))
        .route("/me/files/restore", post(restore_my_file))
        .route("/me/files/purge", post(purge_my_file))
        .route("/me/files/{version_id}/download", get(download_my_file))
        .route("/me/files/{version_id}/raw", get(raw_my_file))
        .route("/me/files/{version_id}/text", get(text_my_file))
        // Criar/listar/apagar pastas pessoais já vivem em `knowledge` (serviam
        // as notas); aqui só se acrescenta o mudar-nome, que faltava.
        .route("/me/folders/rename", post(rename_my_folder))
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

/// Quantos ficheiros a vista agregada devolve de uma vez.
const ALL_FILES_LIMIT: i64 = 200;

/// Todos os ficheiros que quem pergunta alcança, em todos os ambientes.
async fn all_files(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
) -> Result<Json<serde_json::Value>, ApiError> {
    let tudo = files::all(&state.pool, &principal, ALL_FILES_LIMIT).await?;

    Ok(Json(serde_json::json!({
        "items": tudo
            .files
            .into_iter()
            .map(|f| serde_json::json!({
                "id": f.id,
                "name": f.name,
                "classification": f.classification,
                "content_type": f.content_type,
                "size_bytes": f.size_bytes,
                "versions": f.versions,
                "workspace_id": f.workspace_id,
                "workspace_code": f.workspace_code,
                "workspace_title": f.workspace_title,
                "updated_at": f.updated_at,
            }))
            .collect::<Vec<_>>(),
        // O total sai do **mesmo** predicado da lista: um número maior do que
        // as linhas revelaria a existência do que a lista esconde.
        "total": tudo.total,
        // Onde esta pessoa pode criar. Zero é um estado honesto.
        "destinations": tudo
            .destinos
            .into_iter()
            .map(|(id, etiqueta)| serde_json::json!({ "id": id, "label": etiqueta }))
            .collect::<Vec<_>>(),
    })))
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

// --- Carregamento em partes -------------------------------------------------

/// O maior corpo que uma parte pode ter.
///
/// O pedaço mais a margem do envelope. Deliberadamente perto do pedaço e longe
/// do ficheiro: um limite generoso aqui deixaria alguém mandar o ficheiro
/// inteiro numa parte, e o carregamento voltaria a bater no limite do edge.
const CHUNK_BODY_LIMIT_BYTES: usize = 40 * 1024 * 1024;

#[derive(Deserialize)]
struct BeginUploadRequest {
    filename: String,
    content_type: String,
    size_bytes: i64,
    #[serde(default)]
    classification: Option<String>,
    #[serde(default)]
    folder_id: Option<Uuid>,
    /// Presente quando isto é uma nova versão de um ficheiro que já existe.
    #[serde(default)]
    file_id: Option<Uuid>,
}

async fn begin_upload(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path(workspace_id): Path<Uuid>,
    Json(request): Json<BeginUploadRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let store = state.store()?;
    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let sessao = files::upload::begin(
        &mut tx,
        &principal,
        &ids,
        store,
        &state.config.organisation_slug,
        workspace_id,
        files::upload::NewUpload {
            filename: request.filename,
            content_type: request.content_type,
            size_bytes: request.size_bytes,
            classification: parse_classification(request.classification.as_deref())?,
            folder_id: request.folder_id,
            file_id: request.file_id,
        },
    )
    .await?;
    tx.commit().await.map_err(CoreError::from)?;

    Ok(Json(serde_json::json!({
        "session_id": sessao.id,
        "chunk_size_bytes": sessao.chunk_size_bytes,
        "total_parts": sessao.total_parts,
        "expires_at": sessao.expires_at,
        "received_parts": sessao.received_parts,
    })))
}

/// O que o servidor já recebeu.
///
/// É isto que torna a retoma real e não uma repetição: quem volta — noutro
/// separador, noutro dia — pergunta o que falta em vez de recomeçar. Sem este
/// caminho, «resumível» significaria apenas «repete enquanto a página estiver
/// aberta», que é outra coisa.
async fn upload_state(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(session_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let estado = files::upload::state_of(&mut tx, &principal, session_id).await?;
    tx.commit().await.map_err(CoreError::from)?;

    Ok(Json(serde_json::json!({
        "session_id": session_id,
        "chunk_size_bytes": estado.chunk_size_bytes,
        "total_parts": estado.total_parts,
        "expires_at": estado.expires_at,
        "received_parts": estado.received_parts,
    })))
}

#[derive(Deserialize)]
struct PartQuery {
    /// A soma do pedaço, verificada contra os bytes que chegaram.
    sha256: String,
}

async fn upload_part(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path((session_id, part_number)): Path<(Uuid, i32)>,
    Query(query): Query<PartQuery>,
    body: axum::body::Bytes,
) -> Result<Json<serde_json::Value>, ApiError> {
    let store = state.store()?;
    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let aceite = files::upload::accept_part(
        &mut tx,
        &principal,
        store,
        session_id,
        part_number,
        &query.sha256,
        body.to_vec(),
    )
    .await?;
    tx.commit().await.map_err(CoreError::from)?;

    Ok(Json(serde_json::json!({
        "part_number": aceite.part_number,
        "already_present": aceite.already_present,
        "received_parts": aceite.received_parts,
        "total_parts": aceite.total_parts,
    })))
}

#[derive(Deserialize)]
struct CompleteUploadRequest {
    /// A soma do ficheiro inteiro, verificada contra o objecto montado.
    sha256: String,
}

async fn complete_upload(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path(session_id): Path<Uuid>,
    Json(request): Json<CompleteUploadRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let store = state.store()?;
    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let versao = files::upload::finalise(
        &mut tx,
        &principal,
        &ids,
        store,
        session_id,
        &request.sha256,
    )
    .await?;
    tx.commit().await.map_err(CoreError::from)?;

    Ok(Json(serde_json::json!({
        "file_id": versao.file_id,
        "version_id": versao.version_id,
        "sequence": versao.sequence,
    })))
}

async fn cancel_upload(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(session_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let store = state.store()?;
    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    files::upload::abandon(&mut tx, &principal, store, session_id).await?;
    tx.commit().await.map_err(CoreError::from)?;
    Ok(Json(serde_json::json!({ "cancelled": true })))
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

// --- Meus ficheiros ---------------------------------------------------------
//
// O espaço pessoal, servido a quem pergunta e a mais ninguém. Não há permissão
// de ficheiros a exigir: um ficheiro pessoal é do dono, e cada consulta e cada
// escrita fecham-se sobre `person_id`. Nada aqui depende de IA (ADR-0207).

#[derive(Deserialize)]
struct MyFilesQuery {
    /// A pasta do dono a abrir; ausente é a raiz.
    #[serde(default)]
    folder: Option<Uuid>,
    /// Quantos ficheiros listar, no máximo. Limitado, para uma página não pedir
    /// o espaço inteiro de uma vez.
    #[serde(default)]
    limit: Option<i64>,
}

/// `GET /me/files` — os ficheiros pessoais, as pastas e a quota do membro.
async fn my_files(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Query(query): Query<MyFilesQuery>,
) -> Result<Json<files::PersonalFiles>, ApiError> {
    let limit = query.limit.unwrap_or(200).clamp(1, 500);
    let personal = files::list_personal(&state.pool, &principal, query.folder, limit).await?;
    Ok(Json(personal))
}

#[derive(Deserialize)]
struct RenameFile {
    file_id: Uuid,
    name: String,
}

/// `POST /me/files/rename` — muda o nome de um ficheiro pessoal.
async fn rename_my_file(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Json(request): Json<RenameFile>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    files::rename_personal_file(&mut tx, &principal, &ids, request.file_id, &request.name).await?;
    tx.commit().await.map_err(CoreError::from)?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(Deserialize)]
struct MoveFile {
    file_id: Uuid,
    /// A pasta de destino; ausente/nula move para a raiz.
    #[serde(default)]
    folder_id: Option<Uuid>,
}

/// `POST /me/files/move` — move um ficheiro pessoal para uma pasta (ou raiz).
async fn move_my_file(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Json(request): Json<MoveFile>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    files::move_personal_file(
        &mut tx,
        &principal,
        &ids,
        request.file_id,
        request.folder_id,
    )
    .await?;
    tx.commit().await.map_err(CoreError::from)?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// `GET /me/files/trash` — os ficheiros pessoais no Lixo.
async fn my_trash(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Query(query): Query<MyFilesQuery>,
) -> Result<Json<Vec<files::PersonalFileListing>>, ApiError> {
    let limit = query.limit.unwrap_or(200).clamp(1, 500);
    let lixo = files::list_personal_trash(&state.pool, &principal, limit).await?;
    Ok(Json(lixo))
}

#[derive(Deserialize)]
struct FileAction {
    file_id: Uuid,
}

/// `POST /me/files/delete` — põe um ficheiro pessoal no Lixo (reversível).
async fn trash_my_file(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Json(request): Json<FileAction>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    files::trash_personal_file(&mut tx, &principal, &ids, request.file_id).await?;
    tx.commit().await.map_err(CoreError::from)?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// `POST /me/files/restore` — tira um ficheiro pessoal do Lixo.
async fn restore_my_file(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Json(request): Json<FileAction>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    files::restore_personal_file(&mut tx, &principal, &ids, request.file_id).await?;
    tx.commit().await.map_err(CoreError::from)?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// `POST /me/files/purge` — apaga definitivamente um ficheiro que está no Lixo.
async fn purge_my_file(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Json(request): Json<FileAction>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let store = state.store()?;
    files::purge_personal_file(&state.pool, &principal, &ids, store, request.file_id).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(Deserialize)]
struct RenameFolder {
    folder_id: Uuid,
    name: String,
}

/// `POST /me/folders/rename` — muda o nome de uma pasta pessoal. Criar, listar
/// e apagar pastas pessoais vivem em `knowledge` (serviam as notas); só o
/// mudar-nome faltava, e é o que se acrescenta aqui.
async fn rename_my_folder(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Json(request): Json<RenameFolder>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    files::rename_personal_folder(&mut tx, &principal, &ids, request.folder_id, &request.name)
        .await?;
    tx.commit().await.map_err(CoreError::from)?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// `POST /me/files/uploads` — carrega um ficheiro para o espaço pessoal.
///
/// Aceita a lista geral de tipos (não só imagens, ao contrário do caminho das
/// notas): é a superfície de Ficheiros, não a de uma nota. A quota pessoal é
/// admitida dentro de `create_personal`, e uma admissão recusada aborta a
/// transacção sem consumir espaço.
async fn upload_my_file(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    multipart: Multipart,
) -> Result<Json<serde_json::Value>, ApiError> {
    let upload = super::knowledge::read_upload_public(multipart).await?;
    let store = state.store()?;
    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let version = files::create_personal(
        &mut tx,
        &principal,
        &ids,
        store,
        &state.config.organisation_slug,
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
    })))
}

/// `GET /me/files/{version_id}/download` — uma ligação assinada para a versão.
///
/// A posse é a autoridade: uma versão que não seja do dono responde «não
/// encontrado», e não «recusado» — não se confirma a existência do ficheiro de
/// outra pessoa.
async fn download_my_file(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path(version_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let store = state.store()?;
    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let url = files::download_url_personal(&mut tx, &principal, &ids, store, version_id).await?;
    tx.commit().await.map_err(CoreError::from)?;
    Ok(Json(serde_json::json!({
        "url": url,
        "expires_in_seconds": ocinye_core::storage::DOWNLOAD_URL_TTL.as_secs(),
    })))
}

/// `GET /me/files/{version_id}/raw` — descarrega uma versão pessoal, same-origin.
///
/// A posse é a autoridade (reavaliada no Core). Serve os bytes com
/// `Content-Disposition: attachment` pela origem do Workspace — o armazenamento
/// não tem endpoint público, pelo que uma URL assinada apontaria para um host
/// que o browser não alcança. O nome do ficheiro é higienizado à porta para não
/// poder quebrar o cabeçalho.
async fn raw_my_file(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path(version_id): Path<Uuid>,
) -> Result<axum::response::Response, ApiError> {
    use axum::http::header;
    use axum::response::IntoResponse;

    let store = state.store()?;
    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let vista =
        files::read_version_download_personal(&mut tx, &principal, &ids, store, version_id).await?;
    tx.commit().await.map_err(CoreError::from)?;

    // O nome viaja num cabeçalho: uma aspa ou um carácter de controlo quebra-o.
    // Removem-se à porta, como no caminho assinado.
    let seguro: String = vista
        .filename
        .chars()
        .filter(|c| *c != '"' && *c != '\\' && !c.is_control())
        .collect();
    let disposition = format!("attachment; filename=\"{seguro}\"");
    let etag = format!("\"{}\"", vista.checksum_sha256);

    Ok((
        [
            (header::CONTENT_TYPE, vista.content_type),
            (header::CONTENT_DISPOSITION, disposition),
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

/// `GET /me/files/{version_id}/text` — o texto de uma versão pessoal, inline.
///
/// A posse é a autoridade (o Core reavalia-a): uma versão que não seja do dono
/// responde «não encontrado». Serve same-origin, como `text/plain` escapável
/// pelo cliente — nunca se interpreta o conteúdo. Só tipos textuais e até um
/// tecto de tamanho; fora disso, uma recusa tipada.
async fn text_my_file(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path(version_id): Path<Uuid>,
) -> Result<axum::response::Response, ApiError> {
    use axum::http::header;
    use axum::response::IntoResponse;

    let store = state.store()?;
    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let vista =
        files::read_version_text_personal(&mut tx, &principal, &ids, store, version_id).await?;
    tx.commit().await.map_err(CoreError::from)?;

    let etag = format!("\"{}\"", vista.checksum_sha256);

    Ok((
        [
            (header::CONTENT_TYPE, "text/plain; charset=utf-8".to_owned()),
            (header::CONTENT_DISPOSITION, "inline".to_owned()),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_owned()),
            (
                header::CACHE_CONTROL,
                "private, max-age=0, must-revalidate".to_owned(),
            ),
            (header::ETAG, etag),
        ],
        vista.text,
    )
        .into_response())
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

/// O maior conteúdo que se devolve de uma vez.
const CONTENT_MAX_CHARS: usize = 200_000;

/// O texto extraído da versão corrente.
///
/// Devolve `null` quando não há extracção: um ficheiro por processar e um
/// ficheiro sem leitor não têm texto, e a resposta diz isso em vez de fingir
/// uma cadeia vazia.
async fn file_content(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(file_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut conn = state.pool.acquire().await.map_err(CoreError::from)?;
    let texto = files::content(&mut conn, &principal, file_id, CONTENT_MAX_CHARS).await?;
    Ok(Json(serde_json::json!({ "text": texto })))
}

/// O texto extraído de uma versão determinada.
async fn file_version_content(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(version_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut conn = state.pool.acquire().await.map_err(CoreError::from)?;
    let texto =
        files::content_of_version(&mut conn, &principal, version_id, CONTENT_MAX_CHARS).await?;
    Ok(Json(serde_json::json!({ "text": texto })))
}

/// Serve inline os bytes de uma versão exacta.
async fn preview_file_version(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path(version_id): Path<Uuid>,
) -> Result<axum::response::Response, ApiError> {
    use axum::http::header;
    use axum::response::IntoResponse;

    let store = state.store()?;
    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let vista = files::preview_version(&mut tx, &principal, &ids, store, version_id).await?;
    tx.commit().await.map_err(CoreError::from)?;

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
