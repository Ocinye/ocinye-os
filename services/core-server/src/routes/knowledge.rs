//! Knowledge routes: bibliography, notes and documents.

use axum::extract::{DefaultBodyLimit, Multipart, Path, Query, State};
use axum::handler::Handler;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use ocinye_contracts::{Classification, Page, PageRequest};
use ocinye_core::modules::{files, knowledge};
use ocinye_core::realtime::events::{Channel, ServerEvent};
use ocinye_core::CoreError;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ApiError;
use crate::extract::{CurrentPrincipal, Ids};
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/workspaces/{workspace_id}/sources",
            get(list_sources).post(create_source),
        )
        // A bibliografia que o membro alcança, atravessando os seus workspaces.
        //
        // Mesmo recurso, outro âmbito: `/workspaces/{id}/sources` responde «o
        // que há neste ambiente», e este responde «o que alcanço em todos».
        // Não é um substantivo novo nem um endpoint só para um componente.
        .route("/sources", get(list_accessible_sources))
        .route("/documents", get(list_accessible_documents))
        .route(
            "/sources/{source_id}/full-text",
            post(attach_full_text).layer(DefaultBodyLimit::max(super::UPLOAD_BODY_LIMIT_BYTES)),
        )
        // Revisão de bibliografia: uma operação de domínio, e não uma execução.
        //
        // O caminho fala de bibliografia porque é disso que se trata. Que a
        // leitura aconteça dentro do Capability Runtime é uma decisão de
        // implementação do Core, e um endpoint que a expusesse — `/runtime/run`,
        // ou um parâmetro com o nome de um componente — seria um executor de
        // código arbitrário com outro nome.
        .route(
            "/workspaces/{workspace_id}/bibliography/review",
            post(review_bibliography).layer(DefaultBodyLimit::max(
                ocinye_contracts::bibliography::MAX_BIBTEX_BYTES + 4096,
            )),
        )
        .route(
            "/workspaces/{workspace_id}/notes",
            get(list_notes).post(create_note),
        )
        .route("/notes/{note_id}", post(update_note))
        // Personal notes: owned by the acting member, not by a workspace.
        //
        // `/me/…` is the member's own scope. It never takes a `workspace_id`,
        // and the Core resolves the owner from the session, never from the
        // path — a personal note has no identifier a caller could name to
        // reach someone else's (ADR-0413 §4).
        .route(
            "/me/notes",
            get(list_personal_notes).post(create_personal_note),
        )
        .route(
            "/me/notes/{note_id}",
            get(get_personal_note)
                .post(update_personal_note)
                .delete(delete_personal_note),
        )
        // O Lixo: as notas apagadas do membro, e restaurar/eliminar uma.
        .route("/me/deleted-notes", get(list_deleted_notes))
        .route("/me/notes/{note_id}/restore", post(restore_note))
        .route("/me/notes/{note_id}/purge", delete(purge_note))
        .route(
            "/me/notes/{note_id}/revisions",
            get(list_personal_note_revisions),
        )
        // A actividade de uma nota: quem fez o quê (criar, partilhar, apagar…).
        .route("/me/notes/{note_id}/activity", get(note_activity))
        // Uma revisão exacta — o seu conteúdo derivado, para pré-visualizar — e o
        // restauro, que a repõe como uma revisão nova (ADR-0413 §6).
        .route(
            "/me/notes/{note_id}/revisions/{revision}",
            get(get_personal_note_revision),
        )
        .route(
            "/me/notes/{note_id}/revisions/{revision}/restore",
            post(restore_personal_note_revision),
        )
        // Mover uma nota para uma pasta não é editá-la — não cria revisão.
        .route("/me/notes/{note_id}/folder", post(move_personal_note))
        // As notas partilhadas comigo, e a gestão das partilhas de uma nota (dono).
        .route("/me/shared-notes", get(list_shared_notes))
        .route(
            "/me/notes/{note_id}/shares",
            get(list_note_shares).post(share_note),
        )
        .route(
            "/me/notes/{note_id}/shares/{person_id}",
            delete(revoke_note_share),
        )
        // As pastas pessoais, para arrumar as notas.
        .route(
            "/me/folders",
            get(list_personal_folders).post(create_personal_folder),
        )
        .route("/me/folders/{folder_id}", delete(delete_personal_folder))
        // As imagens de uma nota pessoal: carregadas pelo dono, servidas
        // same-origin pela versão exacta. O ficheiro é do membro (ADR-0413 §8).
        .route(
            "/me/files",
            post(upload_personal_file.layer(DefaultBodyLimit::max(super::UPLOAD_BODY_LIMIT_BYTES))),
        )
        .route("/me/files/{version_id}/preview", get(preview_personal_file))
        .route(
            "/workspaces/{workspace_id}/documents",
            get(list_documents)
                .post(upload_document.layer(DefaultBodyLimit::max(super::UPLOAD_BODY_LIMIT_BYTES))),
        )
        .route("/documents/{document_id}/download", get(download_document))
        .route(
            "/workspaces/{workspace_id}/links",
            get(list_links).post(create_link),
        )
}

fn page_of(page: Option<u32>, page_size: Option<u32>) -> PageRequest {
    PageRequest {
        page: page.unwrap_or(1),
        page_size: page_size.unwrap_or(ocinye_contracts::page::DEFAULT_PAGE_SIZE),
    }
}

#[derive(Deserialize)]
struct PageQuery {
    #[serde(default)]
    page: Option<u32>,
    #[serde(default)]
    page_size: Option<u32>,
}

// --- Sources ---------------------------------------------------------------

#[derive(Serialize)]
struct SourceView {
    id: Uuid,
    source_type: String,
    title: String,
    authors: Vec<String>,
    year: Option<i32>,
    container_title: Option<String>,
    doi: Option<String>,
    url: Option<String>,
    keywords: Vec<String>,
    licence: Option<String>,
    /// The recorded legal basis for holding full content.
    content_right: String,
    /// Whether full text is actually held.
    has_full_text: bool,
    classification: String,
}

impl From<knowledge::Source> for SourceView {
    fn from(source: knowledge::Source) -> Self {
        Self {
            id: source.id,
            source_type: source.source_type,
            title: source.title,
            authors: source.authors,
            year: source.year,
            container_title: source.container_title,
            doi: source.doi,
            url: source.url,
            keywords: source.keywords,
            licence: source.licence,
            content_right: source.content_right,
            has_full_text: source.full_text_document_id.is_some(),
            classification: source.classification,
        }
    }
}

#[derive(Deserialize)]
struct CreateSourceRequest {
    title: String,
    #[serde(default)]
    source_type: Option<String>,
    #[serde(default)]
    authors: Vec<String>,
    #[serde(default)]
    year: Option<i32>,
    #[serde(default)]
    container_title: Option<String>,
    #[serde(default)]
    publisher: Option<String>,
    #[serde(default)]
    doi: Option<String>,
    #[serde(default)]
    isbn: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    abstract_text: Option<String>,
    #[serde(default)]
    keywords: Vec<String>,
    #[serde(default)]
    licence: Option<String>,
    #[serde(default)]
    content_right: Option<String>,
    #[serde(default)]
    origin: Option<String>,
    #[serde(default)]
    citation_key: Option<String>,
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

async fn list_sources(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(workspace_id): Path<Uuid>,
    Query(query): Query<PageQuery>,
) -> Result<Json<Page<SourceView>>, ApiError> {
    let page = page_of(query.page, query.page_size);
    let (sources, total) =
        knowledge::list_sources(&state.pool, &principal, workspace_id, page).await?;
    Ok(Json(Page::new(
        sources.into_iter().map(SourceView::from).collect(),
        page,
        total,
    )))
}

/// Toda a bibliografia visível ao membro, sem pedir um workspace.
///
/// A autorização é a mesma da leitura por workspace, e sai do mesmo
/// `VisibilityFilter`: um artefacto só aparece se o membro o puder ver **e**
/// puder ver o ambiente que o contém.
async fn list_accessible_sources(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Query(query): Query<PageQuery>,
) -> Result<Json<Page<SourceView>>, ApiError> {
    let page = page_of(query.page, query.page_size);
    let (sources, total) =
        knowledge::list_accessible_sources(&state.pool, &principal, page).await?;
    Ok(Json(Page::new(
        sources.into_iter().map(SourceView::from).collect(),
        page,
        total,
    )))
}

/// Todos os documentos visíveis ao membro, sem pedir um workspace.
async fn list_accessible_documents(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Query(query): Query<PageQuery>,
) -> Result<Json<Page<DocumentView>>, ApiError> {
    let page = page_of(query.page, query.page_size);
    let (documents, total) =
        knowledge::list_accessible_documents(&state.pool, &principal, page).await?;
    Ok(Json(Page::new(
        documents.into_iter().map(DocumentView::from).collect(),
        page,
        total,
    )))
}

/// O que se pede para rever uma bibliografia.
#[derive(Debug, Deserialize)]
struct ReviewBibliographyRequest {
    /// O BibTeX, tal como foi escrito.
    bibtex: String,
}

/// Rever uma bibliografia BibTeX.
///
/// O limite do corpo é o do contrato mais uma folga para o envelope JSON: o
/// transporte recusa cedo para não gastar o Core, e o Core recusa de qualquer
/// maneira, porque é ele a autoridade sobre o limite.
async fn review_bibliography(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path(workspace_id): Path<Uuid>,
    Json(request): Json<ReviewBibliographyRequest>,
) -> Result<Json<ocinye_contracts::bibliography::BibliographyReview>, ApiError> {
    let revisao = knowledge::review_bibliography(
        &state.pool,
        &state.capabilities,
        &principal,
        workspace_id,
        &request.bibtex,
    )
    .await
    .map_err(|error| ApiError::new(error, &ids))?;

    Ok(Json(revisao))
}

async fn create_source(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path(workspace_id): Path<Uuid>,
    Json(request): Json<CreateSourceRequest>,
) -> Result<Json<SourceView>, ApiError> {
    let source_type = request
        .source_type
        .as_deref()
        .map(|raw| {
            knowledge::SourceType::parse(raw)
                .ok_or_else(|| CoreError::Validation("Unknown source type.".to_owned()))
        })
        .transpose()?;

    let content_right = request
        .content_right
        .as_deref()
        .map(|raw| {
            knowledge::ContentRight::parse(raw)
                .ok_or_else(|| CoreError::Validation("Unknown content right.".to_owned()))
        })
        .transpose()?;

    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let source = knowledge::create_source(
        &mut tx,
        &principal,
        &ids,
        workspace_id,
        knowledge::NewSource {
            source_type,
            title: request.title,
            authors: request.authors,
            year: request.year,
            container_title: request.container_title,
            publisher: request.publisher,
            doi: request.doi,
            isbn: request.isbn,
            url: request.url,
            abstract_text: request.abstract_text,
            keywords: request.keywords,
            licence: request.licence,
            content_right,
            origin: request.origin,
            citation_key: request.citation_key,
            classification: parse_classification(request.classification.as_deref())?,
            raw_metadata: None,
        },
    )
    .await?;
    tx.commit().await.map_err(CoreError::from)?;

    Ok(Json(SourceView::from(source)))
}

/// One part of a multipart upload.
pub(super) struct UploadPart {
    pub filename: String,
    pub content_type: String,
    pub data: Vec<u8>,
    pub fields: std::collections::HashMap<String, String>,
}

/// Read a multipart upload into memory.
///
/// Text fields are bounded so a hostile client cannot exhaust memory through
/// metadata; the file itself is bounded by the service's upload limit, which is
/// checked before anything is stored.
async fn read_upload(mut multipart: Multipart) -> Result<UploadPart, CoreError> {
    const MAX_FIELD_BYTES: usize = 8 * 1024;

    let mut filename = None;
    let mut content_type = None;
    let mut data = None;
    let mut fields = std::collections::HashMap::new();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| CoreError::Validation("The upload is malformed.".to_owned()))?
    {
        let name = field.name().unwrap_or_default().to_owned();

        if name == "file" {
            filename = field.file_name().map(ToOwned::to_owned);
            content_type = field.content_type().map(ToOwned::to_owned);
            data = Some(
                field
                    .bytes()
                    .await
                    .map_err(|_| CoreError::Validation("The upload could not be read.".to_owned()))?
                    .to_vec(),
            );
        } else {
            let value = field
                .text()
                .await
                .map_err(|_| CoreError::Validation("The upload is malformed.".to_owned()))?;
            if value.len() > MAX_FIELD_BYTES {
                return Err(CoreError::Validation(
                    "A form field is too large.".to_owned(),
                ));
            }
            fields.insert(name, value);
        }
    }

    Ok(UploadPart {
        filename: filename
            .ok_or_else(|| CoreError::Validation("A file is required.".to_owned()))?,
        // A missing content type is refused rather than guessed: guessing is
        // how an unexpected type gets stored.
        content_type: content_type
            .ok_or_else(|| CoreError::Validation("A content type is required.".to_owned()))?,
        data: data.ok_or_else(|| CoreError::Validation("A file is required.".to_owned()))?,
        fields,
    })
}

/// Attach the full text of a source.
///
/// Refused unless the source records a legal basis for holding full content
/// (briefing §30).
async fn attach_full_text(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path(source_id): Path<Uuid>,
    multipart: Multipart,
) -> Result<Json<serde_json::Value>, ApiError> {
    let upload = read_upload(multipart).await?;
    let store = state.store()?;

    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let document_id = knowledge::attach_full_text(
        &mut tx,
        &principal,
        &ids,
        store,
        &state.config.organisation_slug,
        source_id,
        knowledge::UploadedFile {
            filename: upload.filename,
            content_type: upload.content_type,
            data: upload.data,
        },
    )
    .await?;
    tx.commit().await.map_err(CoreError::from)?;

    Ok(Json(serde_json::json!({ "document_id": document_id })))
}

// --- Notes -----------------------------------------------------------------

#[derive(Serialize)]
struct NoteView {
    id: Uuid,
    title: String,
    body: String,
    tags: Vec<String>,
    classification: String,
    revision: i32,
}

impl From<knowledge::Note> for NoteView {
    fn from(note: knowledge::Note) -> Self {
        Self {
            id: note.id,
            title: note.title,
            body: note.body,
            tags: note.tags,
            classification: note.classification,
            revision: note.revision,
        }
    }
}

#[derive(Deserialize)]
struct CreateNoteRequest {
    title: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    classification: Option<String>,
}

async fn list_notes(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(workspace_id): Path<Uuid>,
    Query(query): Query<PageQuery>,
) -> Result<Json<Vec<NoteView>>, ApiError> {
    let notes = knowledge::list_notes(
        &state.pool,
        &principal,
        workspace_id,
        page_of(query.page, query.page_size),
    )
    .await?;
    Ok(Json(notes.into_iter().map(NoteView::from).collect()))
}

async fn create_note(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path(workspace_id): Path<Uuid>,
    Json(request): Json<CreateNoteRequest>,
) -> Result<Json<NoteView>, ApiError> {
    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let note = knowledge::create_note(
        &mut tx,
        &principal,
        &ids,
        workspace_id,
        knowledge::NewNote {
            title: request.title,
            body: request.body,
            tags: request.tags,
            classification: parse_classification(request.classification.as_deref())?,
        },
    )
    .await?;
    tx.commit().await.map_err(CoreError::from)?;
    Ok(Json(NoteView::from(note)))
}

#[derive(Deserialize)]
struct UpdateNoteRequest {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    tags: Option<Vec<String>>,
}

/// Update a note. The previous revision is snapshotted first.
async fn update_note(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path(note_id): Path<Uuid>,
    Json(request): Json<UpdateNoteRequest>,
) -> Result<Json<NoteView>, ApiError> {
    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let note = knowledge::update_note(
        &mut tx,
        &principal,
        &ids,
        note_id,
        request.title.as_deref(),
        request.body.as_deref(),
        request.tags,
    )
    .await?;
    tx.commit().await.map_err(CoreError::from)?;
    Ok(Json(NoteView::from(note)))
}

// --- Personal notes --------------------------------------------------------

/// One personal note in full: the canonical structured document travels, so
/// the editor can reconstruct the note exactly, and `revision` is the token
/// the next save must present as its `base_revision`.
#[derive(Serialize)]
struct PersonalNoteView {
    id: Uuid,
    title: String,
    tags: Vec<String>,
    /// The folder that files the note, or `None` at the root — so the editor can
    /// pre-select it.
    folder_id: Option<Uuid>,
    classification: String,
    revision: i32,
    /// How the caller reaches the note: `owner`, `editor` or `viewer`. The
    /// Experience is read-only for a viewer.
    access: String,
    /// The canonical structured document. `None` only for a legacy note that
    /// predates the structured body; the editor treats it as an empty document.
    document: Option<serde_json::Value>,
    schema_version: Option<i32>,
    /// The derived, safe HTML — present only for a viewer, who reads it instead
    /// of editing. Escaped by construction (ADR-0413); the images resolve through
    /// the same-origin preview route.
    html: Option<String>,
    created_at: String,
    updated_at: String,
}

impl PersonalNoteView {
    /// Build the view for the caller's access. A viewer also gets the derived
    /// read-only HTML; an owner or editor gets the document to edit.
    fn for_access(note: knowledge::Note, access: knowledge::NoteAccess) -> Self {
        let html = if matches!(access, knowledge::NoteAccess::Viewer) {
            note.document
                .as_ref()
                .and_then(|doc| knowledge::NoteDocument::from_value(doc.clone()).ok())
                .map(|doc| doc.to_html())
        } else {
            None
        };
        Self {
            id: note.id,
            title: note.title,
            tags: note.tags,
            folder_id: note.folder_id,
            classification: note.classification,
            revision: note.revision,
            access: access.as_str().to_owned(),
            document: note.document,
            schema_version: note.schema_version,
            html,
            created_at: note.created_at.to_rfc3339(),
            updated_at: note.updated_at.to_rfc3339(),
        }
    }
}

impl From<knowledge::Note> for PersonalNoteView {
    /// A note a writer just created or updated: the actor is the owner or an
    /// editor, so there is no read-only HTML to derive.
    fn from(note: knowledge::Note) -> Self {
        Self::for_access(note, knowledge::NoteAccess::Owner)
    }
}

/// A personal note in a list: the document does not travel, only a short
/// plain-text excerpt derived from it, so the list stays cheap and never
/// ships the full body of every note to render a sidebar.
#[derive(Serialize)]
struct PersonalNoteSummary {
    id: Uuid,
    title: String,
    tags: Vec<String>,
    classification: String,
    revision: i32,
    excerpt: String,
    updated_at: String,
}

/// Length of the plain-text excerpt shown in the note list, in characters.
const NOTE_EXCERPT_CHARS: usize = 140;

impl From<knowledge::Note> for PersonalNoteSummary {
    fn from(note: knowledge::Note) -> Self {
        let excerpt: String = note.body.chars().take(NOTE_EXCERPT_CHARS).collect();
        Self {
            id: note.id,
            title: note.title,
            tags: note.tags,
            classification: note.classification,
            revision: note.revision,
            excerpt,
            updated_at: note.updated_at.to_rfc3339(),
        }
    }
}

#[derive(Deserialize)]
struct CreatePersonalNoteRequest {
    title: String,
    /// The canonical structured document, validated by the Core against the
    /// allowed schema before anything is written.
    document: serde_json::Value,
}

#[derive(Deserialize)]
struct UpdatePersonalNoteRequest {
    title: String,
    /// The revision the editor loaded. The save advances the note only if it is
    /// still at this revision; otherwise the Core answers Conflict and nothing
    /// is overwritten (ADR-0413 §5).
    base_revision: i32,
    document: serde_json::Value,
    #[serde(default)]
    tags: Option<Vec<String>>,
}

/// A consulta da lista de notas: paginação e filtros por etiqueta e por pasta.
#[derive(Deserialize)]
struct PersonalNotesQuery {
    #[serde(default)]
    page: Option<u32>,
    #[serde(default)]
    page_size: Option<u32>,
    #[serde(default)]
    tag: Option<String>,
    #[serde(default)]
    folder: Option<Uuid>,
}

async fn list_personal_notes(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Query(query): Query<PersonalNotesQuery>,
) -> Result<Json<Vec<PersonalNoteSummary>>, ApiError> {
    // Uma etiqueta vazia é o mesmo que nenhuma — `?tag=` não recorta.
    let tag = query.tag.as_deref().filter(|t| !t.is_empty());
    let notes = knowledge::list_personal_notes(
        &state.pool,
        &principal,
        tag,
        query.folder,
        page_of(query.page, query.page_size),
    )
    .await?;
    Ok(Json(
        notes.into_iter().map(PersonalNoteSummary::from).collect(),
    ))
}

// --- Personal folders ------------------------------------------------------

#[derive(Deserialize)]
struct CreateFolderRequest {
    name: String,
}

#[derive(Deserialize)]
struct MoveNoteRequest {
    /// The target folder, or `null`/absent to move the note back to the root.
    #[serde(default)]
    folder_id: Option<Uuid>,
}

async fn list_personal_folders(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
) -> Result<Json<Vec<files::PersonalFolder>>, ApiError> {
    let folders = files::list_personal_folders(&state.pool, &principal).await?;
    Ok(Json(folders))
}

async fn create_personal_folder(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Json(request): Json<CreateFolderRequest>,
) -> Result<Json<files::PersonalFolder>, ApiError> {
    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let folder = files::create_personal_folder(&mut tx, &principal, &ids, &request.name).await?;
    tx.commit().await.map_err(CoreError::from)?;
    Ok(Json(folder))
}

async fn delete_personal_folder(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path(folder_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    files::delete_personal_folder(&mut tx, &principal, &ids, folder_id).await?;
    tx.commit().await.map_err(CoreError::from)?;
    Ok(Json(serde_json::json!({ "deleted": true })))
}

async fn move_personal_note(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path(note_id): Path<Uuid>,
    Json(request): Json<MoveNoteRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    knowledge::move_personal_note(&mut tx, &principal, &ids, note_id, request.folder_id).await?;
    tx.commit().await.map_err(CoreError::from)?;
    Ok(Json(serde_json::json!({ "folder_id": request.folder_id })))
}

async fn create_personal_note(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Json(request): Json<CreatePersonalNoteRequest>,
) -> Result<Json<PersonalNoteView>, ApiError> {
    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let note = knowledge::create_personal_note(
        &mut tx,
        &principal,
        &ids,
        &request.title,
        request.document,
    )
    .await?;
    tx.commit().await.map_err(CoreError::from)?;
    Ok(Json(PersonalNoteView::from(note)))
}

async fn get_personal_note(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(note_id): Path<Uuid>,
) -> Result<Json<PersonalNoteView>, ApiError> {
    let (note, access) = knowledge::get_personal_note(&state.pool, &principal, note_id).await?;
    Ok(Json(PersonalNoteView::for_access(note, access)))
}

async fn delete_personal_note(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path(note_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    knowledge::delete_personal_note(&mut tx, &principal, &ids, note_id).await?;
    tx.commit().await.map_err(CoreError::from)?;
    Ok(Json(serde_json::json!({ "deleted": true })))
}

async fn list_deleted_notes(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Query(query): Query<PageQuery>,
) -> Result<Json<Vec<PersonalNoteSummary>>, ApiError> {
    let notes = knowledge::deleted_personal_notes(
        &state.pool,
        &principal,
        page_of(query.page, query.page_size),
    )
    .await?;
    Ok(Json(
        notes.into_iter().map(PersonalNoteSummary::from).collect(),
    ))
}

async fn restore_note(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path(note_id): Path<Uuid>,
) -> Result<Json<PersonalNoteView>, ApiError> {
    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let note = knowledge::restore_personal_note(&mut tx, &principal, &ids, note_id).await?;
    tx.commit().await.map_err(CoreError::from)?;
    Ok(Json(PersonalNoteView::from(note)))
}

async fn purge_note(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path(note_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    knowledge::purge_personal_note(&mut tx, &principal, &ids, note_id).await?;
    tx.commit().await.map_err(CoreError::from)?;
    Ok(Json(serde_json::json!({ "purged": true })))
}

async fn update_personal_note(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path(note_id): Path<Uuid>,
    Json(request): Json<UpdatePersonalNoteRequest>,
) -> Result<Json<PersonalNoteView>, ApiError> {
    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let note = knowledge::update_personal_note(
        &mut tx,
        &principal,
        &ids,
        note_id,
        knowledge::PersonalNoteEdit {
            base_revision: request.base_revision,
            title: request.title,
            document: request.document,
            tags: request.tags,
        },
    )
    .await?;
    tx.commit().await.map_err(CoreError::from)?;

    // Persistido. Agora, e só agora, avisa-se quem mais alcança a nota — o dono
    // e os destinatários vivos, menos quem gravou (ADR-0413 §9). Sem partilha,
    // a lista é vazia e nada sai. Fogo-e-esquece: se o realtime estiver em baixo,
    // a gravação continua verdadeira e o outro lado reconcilia ao recarregar.
    if let Ok(recipients) =
        knowledge::note_notify_recipients(&state.pool, note.id, principal.person_id).await
    {
        for pid in recipients {
            state
                .realtime
                .publish(
                    Channel::Person { id: pid },
                    &ServerEvent::NoteUpdated { note_id: note.id },
                )
                .await;
        }
    }
    Ok(Json(PersonalNoteView::from(note)))
}

async fn list_personal_note_revisions(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(note_id): Path<Uuid>,
) -> Result<Json<Vec<knowledge::NoteRevisionMeta>>, ApiError> {
    let revisions = knowledge::personal_note_revisions(&state.pool, &principal, note_id).await?;
    Ok(Json(revisions))
}

async fn note_activity(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(note_id): Path<Uuid>,
) -> Result<Json<Vec<ocinye_core::modules::collaboration::PersonalActivity>>, ApiError> {
    let feed = knowledge::note_activity(&state.pool, &principal, note_id).await?;
    Ok(Json(feed))
}

/// One revision, rendered read-only: the title and the derived HTML of what it
/// held. It is the preview of what a restore would replay.
#[derive(Serialize)]
struct NoteRevisionView {
    revision: i32,
    title: String,
    html: String,
}

async fn get_personal_note_revision(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path((note_id, revision)): Path<(Uuid, i32)>,
) -> Result<Json<NoteRevisionView>, ApiError> {
    let (title, document) =
        knowledge::personal_note_revision_content(&state.pool, &principal, note_id, revision)
            .await?;
    Ok(Json(NoteRevisionView {
        revision,
        title,
        html: document.to_html(),
    }))
}

async fn restore_personal_note_revision(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path((note_id, revision)): Path<(Uuid, i32)>,
    Json(request): Json<RestoreNoteRequest>,
) -> Result<Json<PersonalNoteView>, ApiError> {
    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let restored = knowledge::restore_personal_note_revision(
        &mut tx,
        &principal,
        &ids,
        note_id,
        revision,
        request.base_revision,
    )
    .await?;
    tx.commit().await.map_err(CoreError::from)?;
    Ok(Json(PersonalNoteView::from(restored)))
}

#[derive(Deserialize)]
struct RestoreNoteRequest {
    base_revision: i32,
}

// --- Note sharing ----------------------------------------------------------

async fn list_shared_notes(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Query(query): Query<PageQuery>,
) -> Result<Json<Vec<PersonalNoteSummary>>, ApiError> {
    let notes = knowledge::notes_shared_with_me(
        &state.pool,
        &principal,
        page_of(query.page, query.page_size),
    )
    .await?;
    Ok(Json(
        notes.into_iter().map(PersonalNoteSummary::from).collect(),
    ))
}

async fn list_note_shares(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(note_id): Path<Uuid>,
) -> Result<Json<Vec<knowledge::NoteShareRow>>, ApiError> {
    let shares = knowledge::list_personal_note_shares(&state.pool, &principal, note_id).await?;
    Ok(Json(shares))
}

#[derive(Deserialize)]
struct ShareNoteRequest {
    person_id: Uuid,
    role: String,
}

async fn share_note(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path(note_id): Path<Uuid>,
    Json(request): Json<ShareNoteRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let role = ocinye_contracts::NoteShareRole::parse(&request.role)
        .ok_or_else(|| CoreError::Validation("Papel de partilha desconhecido.".to_owned()))?;
    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    knowledge::share_personal_note(&mut tx, &principal, &ids, note_id, request.person_id, role)
        .await?;
    tx.commit().await.map_err(CoreError::from)?;
    // Persistido. Avisa-se a pessoa com quem se partilhou, no seu canal
    // (ADR-0413 §9). Fogo-e-esquece, degrada em silêncio.
    state
        .realtime
        .publish(
            Channel::Person {
                id: request.person_id,
            },
            &ServerEvent::NoteShared { note_id },
        )
        .await;
    Ok(Json(serde_json::json!({ "shared": true })))
}

async fn revoke_note_share(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path((note_id, person_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    knowledge::revoke_personal_note_share(&mut tx, &principal, &ids, note_id, person_id).await?;
    tx.commit().await.map_err(CoreError::from)?;
    Ok(Json(serde_json::json!({ "revoked": true })))
}

/// Carrega uma imagem para uma nota pessoal e devolve a sua versão de ficheiro.
///
/// Só imagens que se mostram inline (PNG, JPEG, WebP): um SVG é um documento com
/// script e não se serve inline, e um anexo genérico é de uma fatia posterior. O
/// ficheiro nasce do membro; o documento da nota passa a citar `file_version_id`.
async fn upload_personal_file(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    multipart: Multipart,
) -> Result<Json<serde_json::Value>, ApiError> {
    let upload = read_upload_public(multipart).await?;

    // Só formatos raster que se mostram inline. A validação da lista geral corre
    // à mesma dentro de `create_personal`; esta é a que fecha a nota a SVG/TIFF.
    let tipo = upload
        .content_type
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    if !files::PREVIEWABLE_TYPES.contains(&tipo.as_str()) {
        return Err(CoreError::Validation(
            "Só se podem inserir imagens PNG, JPEG ou WebP numa nota.".to_owned(),
        )
        .into());
    }

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
        "file_version_id": version.version_id,
    })))
}

/// Serve a imagem de uma nota pessoal, inline, pela versão exacta.
///
/// A autoridade é a posse, reavaliada aqui pelo Core: só o dono vê os bytes, e um
/// identificador de versão de outra pessoa responde «não encontrado».
async fn preview_personal_file(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path(version_id): Path<Uuid>,
) -> Result<axum::response::Response, ApiError> {
    use axum::http::header;
    use axum::response::IntoResponse;

    let store = state.store()?;
    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;

    // Autoriza: o dono do ficheiro, ou uma nota partilhada viva com quem pede que
    // referencie esta versão. Como uma nota só referencia ficheiros do próprio
    // dono, uma nota partilhada expõe exactamente as imagens que contém, e mais
    // nenhuma (ADR-0413 §8). Recusa como «não encontrado» — não confirma que a
    // versão existe a quem não a alcança.
    let owns = files::owns_personal_file_version(&mut tx, &principal, version_id).await?;
    let allowed =
        owns || knowledge::member_can_view_note_file(&state.pool, &principal, version_id).await?;
    if !allowed {
        return Err(CoreError::NotFound("Versão não encontrada.".to_owned()).into());
    }

    let vista = files::read_version_preview(&mut tx, &principal, &ids, store, version_id).await?;
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

// --- Documents -------------------------------------------------------------

#[derive(Serialize)]
struct DocumentView {
    id: Uuid,
    kind: String,
    title: String,
    description: Option<String>,
    original_filename: String,
    content_type: String,
    size_bytes: i64,
    checksum_sha256: String,
    classification: String,
}

impl From<knowledge::Document> for DocumentView {
    fn from(document: knowledge::Document) -> Self {
        Self {
            id: document.id,
            kind: document.kind,
            title: document.title,
            description: document.description,
            original_filename: document.original_filename,
            content_type: document.content_type,
            size_bytes: document.size_bytes,
            checksum_sha256: document.checksum_sha256,
            classification: document.classification,
        }
    }
}

async fn list_documents(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(workspace_id): Path<Uuid>,
    Query(query): Query<PageQuery>,
) -> Result<Json<Vec<DocumentView>>, ApiError> {
    let documents = knowledge::list_documents(
        &state.pool,
        &principal,
        workspace_id,
        page_of(query.page, query.page_size),
    )
    .await?;
    Ok(Json(
        documents.into_iter().map(DocumentView::from).collect(),
    ))
}

async fn upload_document(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path(workspace_id): Path<Uuid>,
    multipart: Multipart,
) -> Result<Json<serde_json::Value>, ApiError> {
    let upload = read_upload(multipart).await?;
    let store = state.store()?;

    let kind = upload
        .fields
        .get("kind")
        .map(|raw| {
            knowledge::DocumentKind::parse(raw)
                .ok_or_else(|| CoreError::Validation("Unknown document kind.".to_owned()))
        })
        .transpose()?
        .unwrap_or(knowledge::DocumentKind::Other);

    let title = upload
        .fields
        .get("title")
        .cloned()
        .unwrap_or_else(|| upload.filename.clone());

    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let document_id = knowledge::create_document(
        &mut tx,
        &principal,
        &ids,
        store,
        &state.config.organisation_slug,
        workspace_id,
        knowledge::NewDocument {
            kind,
            title,
            description: upload.fields.get("description").cloned(),
            filename: upload.filename,
            content_type: upload.content_type,
            data: upload.data,
            classification: parse_classification(
                upload.fields.get("classification").map(String::as_str),
            )?,
        },
    )
    .await?;
    tx.commit().await.map_err(CoreError::from)?;

    Ok(Json(serde_json::json!({ "document_id": document_id })))
}

/// Authorise a download and return a short-lived signed URL.
///
/// The Core does not proxy the bytes. Knowing the object key grants nothing:
/// this endpoint is the only way to obtain a usable URL, and it is audited.
async fn download_document(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path(document_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let store = state.store()?;

    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let url = knowledge::issue_download(&mut tx, &principal, &ids, store, document_id).await?;
    tx.commit().await.map_err(CoreError::from)?;

    Ok(Json(serde_json::json!({
        "url": url,
        "expires_in_seconds": ocinye_core::storage::DOWNLOAD_URL_TTL.as_secs(),
    })))
}

// --- Research links --------------------------------------------------------

#[derive(Deserialize)]
struct CreateLinkRequest {
    source_type: String,
    source_id: Uuid,
    relation: String,
    target_type: String,
    target_id: Uuid,
    #[serde(default)]
    note: Option<String>,
}

async fn create_link(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path(workspace_id): Path<Uuid>,
    Json(request): Json<CreateLinkRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let link = knowledge::link_objects(
        &mut tx,
        &state.pool,
        &principal,
        &ids,
        workspace_id,
        &request.source_type,
        request.source_id,
        &request.relation,
        &request.target_type,
        request.target_id,
        request.note.as_deref(),
    )
    .await?;
    tx.commit().await.map_err(CoreError::from)?;

    Ok(Json(serde_json::json!({ "link_id": link.id })))
}

async fn list_links(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(workspace_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let links = knowledge::list_links(&state.pool, &principal, workspace_id).await?;
    Ok(Json(serde_json::to_value(links).unwrap_or_default()))
}

/// Read a multipart upload, for sibling route modules that also accept files.
///
/// Exposed within `routes` so the same bounded, validated reader is used
/// everywhere rather than each surface writing its own.
pub(super) async fn read_upload_public(multipart: Multipart) -> Result<UploadPart, CoreError> {
    read_upload(multipart).await
}
