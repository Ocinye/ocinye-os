//! Knowledge application layer.

use ocinye_contracts::bibliography::{
    BibliographyEntry, BibliographyReview, MAX_BIBTEX_BYTES, MAX_ENTRIES,
};
use ocinye_contracts::{Classification, PageRequest};
use ocinye_domain::policy::{authorize, Action, ResourceKind, VisibilityFilter};
use ocinye_domain::Principal;
use ocinye_observability::CorrelationIds;
use serde_json::{json, Value};
use sqlx::{PgExecutor, PgPool};
use uuid::Uuid;

use super::model::{ContentRight, Document, DocumentKind, Note, ResearchLink, Source, SourceType};
use super::repository::{self as repo, NewSourceRow};
use crate::audit::{self, action, AuditEntry};
use crate::capabilities::{Capabilities, Component};
use crate::error::{CoreError, CoreResult};
use crate::modules::collaboration::{record_activity, ActivityKind};
use crate::modules::research::{
    artefact_context, get_workspace, readable_artefact_workspace, workspace_context,
    ResearchWorkspace,
};
use crate::modules::search;
use crate::outbox::{self, event};
use crate::storage::{self, ObjectStore};
use crate::Tx;

/// Details of a new bibliographic source.
#[derive(Debug, Clone, Default)]
pub struct NewSource {
    /// Kind of source.
    pub source_type: Option<SourceType>,
    /// Title.
    pub title: String,
    /// Authors.
    pub authors: Vec<String>,
    /// Year.
    pub year: Option<i32>,
    /// Journal, proceedings or book title.
    pub container_title: Option<String>,
    /// Publisher.
    pub publisher: Option<String>,
    /// DOI.
    pub doi: Option<String>,
    /// ISBN.
    pub isbn: Option<String>,
    /// Authorised link.
    pub url: Option<String>,
    /// Abstract.
    pub abstract_text: Option<String>,
    /// Keywords.
    pub keywords: Vec<String>,
    /// Licence.
    pub licence: Option<String>,
    /// Recorded legal basis for full content.
    pub content_right: Option<ContentRight>,
    /// Where the reference came from.
    pub origin: Option<String>,
    /// Citation key.
    pub citation_key: Option<String>,
    /// Requested classification. Never widens the workspace's.
    pub classification: Option<Classification>,
    /// Raw imported record, kept for provenance.
    pub raw_metadata: Option<Value>,
}

/// Effective classification of an artefact inside a workspace.
///
/// An artefact never becomes more open than the workspace holding it: a
/// requested classification can only tighten.
fn effective_classification(
    workspace: &ResearchWorkspace,
    requested: Option<Classification>,
) -> Classification {
    workspace
        .classification()
        .most_restrictive(requested.unwrap_or(Classification::DEFAULT))
}

/// Add a bibliographic source.
///
/// # Errors
///
/// Returns an error when the caller may not write in the workspace, or the
/// source has no title.
pub async fn create_source(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    workspace_id: Uuid,
    request: NewSource,
) -> CoreResult<Source> {
    let workspace = get_workspace(&mut **tx, principal, workspace_id).await?;
    let ctx = workspace_context(&workspace, ResourceKind::Source);
    authorize(principal, Action::Create, &ctx)
        .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    let title = request.title.trim();
    if title.is_empty() {
        return Err(CoreError::Validation("A source needs a title.".to_owned()));
    }

    let classification = effective_classification(&workspace, request.classification);
    let content_right = request.content_right.unwrap_or_default();

    let source = repo::insert_source(
        &mut **tx,
        principal.organisation_id,
        workspace.unit_id,
        workspace.id,
        &NewSourceRow {
            source_type: request.source_type.unwrap_or(SourceType::Article).as_str(),
            title,
            authors: &request.authors,
            year: request.year,
            container_title: request.container_title.as_deref(),
            publisher: request.publisher.as_deref(),
            doi: request
                .doi
                .as_deref()
                .map(str::trim)
                .filter(|doi| !doi.is_empty()),
            isbn: request.isbn.as_deref(),
            url: request.url.as_deref(),
            abstract_text: request.abstract_text.as_deref(),
            keywords: &request.keywords,
            licence: request.licence.as_deref(),
            content_right: content_right.as_str(),
            origin: request.origin.as_deref(),
            citation_key: request.citation_key.as_deref(),
            raw_metadata: request.raw_metadata.unwrap_or_else(|| json!({})),
        },
        classification,
        principal.person_id,
    )
    .await?;

    let indexed = [
        Some(request.authors.join(", ")),
        request.container_title.clone(),
        request.abstract_text.clone(),
        Some(request.keywords.join(" ")),
    ]
    .into_iter()
    .flatten()
    .filter(|part| !part.is_empty())
    .collect::<Vec<_>>()
    .join("\n");

    search::index_entity(
        tx,
        search::IndexRequest {
            organisation_id: principal.organisation_id,
            unit_id: Some(workspace.unit_id),
            workspace_id: Some(workspace.id),
            entity_type: "source",
            entity_id: source.id,
            title: title.to_owned(),
            text: indexed,
            classification,
        },
    )
    .await?;

    outbox::emit(
        tx,
        event::SOURCE_ADDED,
        "source",
        source.id,
        &ids.correlation_id,
        json!({ "workspace_id": workspace.id }),
    )
    .await?;

    record_activity(
        tx,
        principal,
        workspace.id,
        workspace.unit_id,
        ActivityKind::Attached,
        "source",
        Some(source.id),
        &format!("Source added: {title}"),
        classification,
    )
    .await?;

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::CREATE, "source")
            .resource(source.id)
            .context(&ctx)
            .classified(classification)
            .detail("content_right", content_right.as_str()),
    )
    .await?;

    Ok(source)
}

/// List the sources of a workspace.
///
/// # Errors
///
/// Returns an error when the caller may not read the workspace.
pub async fn list_sources(
    pool: &PgPool,
    principal: &Principal,
    workspace_id: Uuid,
    page: PageRequest,
) -> CoreResult<(Vec<Source>, i64)> {
    let workspace = get_workspace(pool, principal, workspace_id).await?;
    let filter = VisibilityFilter::for_principal(principal);

    let sources = repo::list_sources(
        pool,
        principal.organisation_id,
        &filter,
        workspace.id,
        page.limit(),
        page.offset(),
    )
    .await?;
    let total = repo::count_sources(pool, principal.organisation_id, &filter, workspace.id).await?;
    Ok((sources, total))
}

/// A bibliografia que o membro alcança, atravessando os seus workspaces.
///
/// O ecrã `Bibliografia` do Workspace é institucional, mas uma fonte pertence a
/// um Research Workspace e continua a pertencer. Esta leitura não move
/// ownership: soma o que o membro já podia ver, um workspace de cada vez.
///
/// Não recebe `workspace_id` porque a pergunta é outra — não é «o que há neste
/// workspace» mas «o que alcanço em todos». A autorização é a mesma, e sai do
/// mesmo `VisibilityFilter`.
///
/// # Errors
///
/// Propaga falhas da base de dados.
pub async fn list_accessible_sources(
    pool: &PgPool,
    principal: &Principal,
    page: PageRequest,
) -> CoreResult<(Vec<Source>, i64)> {
    let filter = VisibilityFilter::for_principal(principal);
    if filter.is_never_satisfiable() {
        return Ok((Vec::new(), 0));
    }

    let sources = repo::list_accessible_sources(
        pool,
        principal.organisation_id,
        &filter,
        page.limit(),
        page.offset(),
    )
    .await?;
    let total = repo::count_accessible_sources(pool, principal.organisation_id, &filter).await?;
    Ok((sources, total))
}

/// Os documentos que o membro alcança, atravessando os seus workspaces.
///
/// Mesma invariante da bibliografia: o artefacto e o ambiente que o contém têm
/// de ser ambos visíveis.
///
/// # Errors
///
/// Propaga falhas da base de dados.
pub async fn list_accessible_documents(
    pool: &PgPool,
    principal: &Principal,
    page: PageRequest,
) -> CoreResult<(Vec<Document>, i64)> {
    let filter = VisibilityFilter::for_principal(principal);
    if filter.is_never_satisfiable() {
        return Ok((Vec::new(), 0));
    }

    let documents = repo::list_accessible_documents(
        pool,
        principal.organisation_id,
        &filter,
        page.limit(),
        page.offset(),
    )
    .await?;
    let total = repo::count_accessible_documents(pool, principal.organisation_id, &filter).await?;
    Ok((documents, total))
}

/// Details of a new note.
#[derive(Debug, Clone)]
pub struct NewNote {
    /// Title.
    pub title: String,
    /// Body.
    pub body: String,
    /// Tags.
    pub tags: Vec<String>,
    /// Requested classification. Never widens the workspace's.
    pub classification: Option<Classification>,
}

/// Create a note.
///
/// # Errors
///
/// Returns an error when the caller may not write, or the note has no title.
pub async fn create_note(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    workspace_id: Uuid,
    request: NewNote,
) -> CoreResult<Note> {
    let workspace = get_workspace(&mut **tx, principal, workspace_id).await?;
    let ctx = workspace_context(&workspace, ResourceKind::Note);
    authorize(principal, Action::Create, &ctx)
        .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    let title = request.title.trim();
    if title.is_empty() {
        return Err(CoreError::Validation("A note needs a title.".to_owned()));
    }

    let classification = effective_classification(&workspace, request.classification);

    let note = repo::insert_note(
        &mut **tx,
        principal.organisation_id,
        workspace.unit_id,
        workspace.id,
        title,
        &request.body,
        &request.tags,
        classification,
        principal.person_id,
    )
    .await?;

    search::index_entity(
        tx,
        search::IndexRequest {
            organisation_id: principal.organisation_id,
            unit_id: Some(workspace.unit_id),
            workspace_id: Some(workspace.id),
            entity_type: "note",
            entity_id: note.id,
            title: title.to_owned(),
            text: format!("{}\n{}", request.body, request.tags.join(" ")),
            classification,
        },
    )
    .await?;

    outbox::emit(
        tx,
        event::NOTE_CREATED,
        "note",
        note.id,
        &ids.correlation_id,
        json!({ "workspace_id": workspace.id }),
    )
    .await?;

    record_activity(
        tx,
        principal,
        workspace.id,
        workspace.unit_id,
        ActivityKind::Created,
        "note",
        Some(note.id),
        &format!("Note created: {title}"),
        classification,
    )
    .await?;

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::CREATE, "note")
            .resource(note.id)
            .context(&ctx)
            .classified(classification),
    )
    .await?;

    Ok(note)
}

/// Update a note, snapshotting the previous revision first.
///
/// # Errors
///
/// Returns an error when the caller may not write, or the note is absent.
pub async fn update_note(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    note_id: Uuid,
    title: Option<&str>,
    body: Option<&str>,
    tags: Option<Vec<String>>,
) -> CoreResult<Note> {
    let existing = repo::find_note(&mut **tx, note_id, principal.organisation_id)
        .await?
        .ok_or_else(|| CoreError::NotFound("Note not found.".to_owned()))?;
    let workspace = get_workspace(&mut **tx, principal, existing.workspace_id).await?;

    let ctx = artefact_context(&workspace, ResourceKind::Note, existing.classification());
    authorize(principal, Action::Update, &ctx)
        .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    repo::snapshot_note(&mut **tx, &existing).await?;

    let updated = repo::update_note(
        &mut **tx,
        existing.id,
        title.map(str::trim),
        body,
        tags.as_deref(),
        principal.person_id,
    )
    .await?;

    search::index_entity(
        tx,
        search::IndexRequest {
            organisation_id: principal.organisation_id,
            unit_id: Some(workspace.unit_id),
            workspace_id: Some(workspace.id),
            entity_type: "note",
            entity_id: updated.id,
            title: updated.title.clone(),
            text: format!("{}\n{}", updated.body, updated.tags.join(" ")),
            classification: updated.classification(),
        },
    )
    .await?;

    outbox::emit(
        tx,
        event::NOTE_UPDATED,
        "note",
        updated.id,
        &ids.correlation_id,
        json!({ "workspace_id": workspace.id, "revision": updated.revision }),
    )
    .await?;

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::UPDATE, "note")
            .resource(updated.id)
            .context(&ctx)
            .detail("revision", updated.revision),
    )
    .await?;

    Ok(updated)
}

/// List the notes of a workspace.
///
/// # Errors
///
/// Returns an error when the caller may not read the workspace.
pub async fn list_notes(
    pool: &PgPool,
    principal: &Principal,
    workspace_id: Uuid,
    page: PageRequest,
) -> CoreResult<Vec<Note>> {
    let workspace = get_workspace(pool, principal, workspace_id).await?;
    let filter = VisibilityFilter::for_principal(principal);
    repo::list_notes(
        pool,
        principal.organisation_id,
        &filter,
        workspace.id,
        page.limit(),
        page.offset(),
    )
    .await
}

/// A file as it arrived from a caller, before validation.
pub struct UploadedFile {
    /// Original filename as supplied by the uploader.
    pub filename: String,
    /// Declared content type. Checked against the allow-list, never trusted.
    pub content_type: String,
    /// The bytes.
    pub data: Vec<u8>,
}

/// Details of a new document.
pub struct NewDocument {
    /// Kind of document.
    pub kind: DocumentKind,
    /// Title.
    pub title: String,
    /// Description.
    pub description: Option<String>,
    /// Original filename as supplied by the uploader.
    pub filename: String,
    /// Declared content type.
    pub content_type: String,
    /// The bytes.
    pub data: Vec<u8>,
    /// Requested classification. Never widens the workspace's.
    pub classification: Option<Classification>,
}

/// Upload a document into a workspace.
///
/// Validation happens before any byte reaches storage: size, content type,
/// filename, and a computed checksum.
///
/// # Errors
///
/// Returns an error when the caller may not write, validation fails, or storage
/// is unavailable.
pub async fn create_document(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    store: &ObjectStore,
    organisation_slug: &str,
    workspace_id: Uuid,
    request: NewDocument,
) -> CoreResult<Uuid> {
    let workspace = get_workspace(&mut **tx, principal, workspace_id).await?;
    let ctx = workspace_context(&workspace, ResourceKind::Document);
    authorize(principal, Action::Create, &ctx)
        .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    let title = request.title.trim();
    if title.is_empty() {
        return Err(CoreError::Validation(
            "A document needs a title.".to_owned(),
        ));
    }
    if request.data.is_empty() {
        return Err(CoreError::Validation(
            "The uploaded file is empty.".to_owned(),
        ));
    }
    if request.data.len() as u64 > store.max_upload_bytes() {
        return Err(CoreError::Validation(
            "The uploaded file exceeds the maximum permitted size.".to_owned(),
        ));
    }

    let content_type = storage::validate_content_type(&request.content_type)?;
    let filename = storage::normalise_filename(&request.filename)?;
    let checksum = storage::sha256_hex(&request.data);
    let classification = effective_classification(&workspace, request.classification);
    let size = i64::try_from(request.data.len())
        .map_err(|_| CoreError::Validation("The uploaded file is too large.".to_owned()))?;

    let object_id = Uuid::new_v4();
    let object_key = storage::build_object_key(organisation_slug, workspace.id, object_id);

    // The metadata row is written first, in this transaction. If the upload
    // then fails, the transaction rolls back and no dangling row remains.
    let registo = sqlx::query(
        "INSERT INTO storage_objects
             (id, backend_id, organisation_id, unit_id, workspace_id, object_key,
              original_filename, content_type, size_bytes, checksum_sha256,
              classification, status, created_by_id)
         SELECT $1, b.id, $2, $3, $4, $5, $6, $7, $8, $9, $10, 'stored', $11
           FROM storage_backends b
          WHERE b.is_default AND b.is_active",
    )
    .bind(object_id)
    .bind(principal.organisation_id)
    .bind(workspace.unit_id)
    .bind(workspace.id)
    .bind(&object_key)
    .bind(&filename)
    .bind(&content_type)
    .bind(size)
    .bind(&checksum)
    .bind(classification.as_str())
    .bind(principal.person_id)
    .execute(&mut **tx)
    .await?;

    // Sem backend por omissão, o `SELECT` não devolve linha nenhuma e o
    // `INSERT … SELECT` completa com zero — sem erro. O código seguia para o
    // `put`, escrevia o objecto no armazenamento institucional, e só depois
    // falhava numa chave estrangeira sobre um identificador que ninguém
    // reconhecia.
    //
    // Ficava um ficheiro no bucket que nada referenciava, e a causa real — esta
    // instalação não tem armazenamento registado — não aparecia em lado nenhum.
    //
    // A verificação vem antes do `put` porque é aí que ainda não há nada para
    // limpar.
    if registo.rows_affected() == 0 {
        return Err(CoreError::StorageUnavailable(
            "No default storage backend is registered on this deployment.".to_owned(),
        ));
    }

    store
        .put(&object_key, &content_type, &checksum, request.data)
        .await?;

    // ── A identidade do ficheiro, e a sua primeira versão ───────────────
    //
    // Na mesma transacção do documento. Ou nascem os quatro — objecto,
    // ficheiro, versão e documento — ou não nasce nenhum: um documento
    // parcialmente versionado seria pior do que um sem versões, porque
    // pareceria completo.
    //
    // O nome do ficheiro é o do carregamento, e não o título do documento. São
    // coisas diferentes: o título muda quando alguém o corrige, e o ficheiro
    // continua a chamar-se o que se carregou.
    let ficheiro = crate::modules::files::create_with_first_version(
        tx,
        crate::modules::files::FileContext {
            organisation_id: principal.organisation_id,
            unit_id: workspace.unit_id,
            workspace_id: workspace.id,
            classification,
        },
        &filename,
        object_id,
        principal.person_id,
    )
    .await?;

    let document_id = repo::insert_document(
        &mut **tx,
        principal.organisation_id,
        workspace.unit_id,
        workspace.id,
        ficheiro.file_id,
        request.kind.as_str(),
        title,
        request.description.as_deref(),
        classification,
        principal.person_id,
    )
    .await?;

    // Only title and description are indexed. Extracting document bodies into
    // the search index is a separate, explicitly authorised decision.
    search::index_entity(
        tx,
        search::IndexRequest {
            organisation_id: principal.organisation_id,
            unit_id: Some(workspace.unit_id),
            workspace_id: Some(workspace.id),
            entity_type: "document",
            entity_id: document_id,
            title: title.to_owned(),
            text: request.description.clone().unwrap_or_default(),
            classification,
        },
    )
    .await?;

    outbox::emit(
        tx,
        event::DOCUMENT_UPLOADED,
        "document",
        document_id,
        &ids.correlation_id,
        json!({ "workspace_id": workspace.id, "storage_object_id": object_id, "size_bytes": size }),
    )
    .await?;

    record_activity(
        tx,
        principal,
        workspace.id,
        workspace.unit_id,
        ActivityKind::Attached,
        "document",
        Some(document_id),
        &format!("Document uploaded: {title}"),
        classification,
    )
    .await?;

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::UPLOAD, "document")
            .resource(document_id)
            .context(&ctx)
            .classified(classification)
            .detail("content_type", content_type.as_str())
            .detail("size_bytes", size)
            .detail("checksum_sha256", checksum.as_str()),
    )
    .await?;

    Ok(document_id)
}

/// Load a document the caller may read.
///
/// # Errors
///
/// Returns [`CoreError::NotFound`] when absent or not readable.
pub async fn get_document(
    pool: &PgPool,
    principal: &Principal,
    document_id: Uuid,
) -> CoreResult<(Document, ResearchWorkspace)> {
    let document = repo::find_document(pool, document_id, principal.organisation_id)
        .await?
        .ok_or_else(|| CoreError::NotFound("Document not found.".to_owned()))?;
    let workspace = readable_artefact_workspace(
        pool,
        principal,
        document.workspace_id,
        ResourceKind::Document,
        document.classification(),
    )
    .await?;
    Ok((document, workspace))
}

/// Load one bibliographic source, with the workspace that governs it.
///
/// # Errors
///
/// Returns [`CoreError::NotFound`] when absent or not readable — the two are
/// deliberately indistinguishable.
pub async fn get_source(
    pool: &PgPool,
    principal: &Principal,
    source_id: Uuid,
) -> CoreResult<(Source, ResearchWorkspace)> {
    let source = repo::find_source(pool, source_id, principal.organisation_id)
        .await?
        .ok_or_else(|| CoreError::NotFound("Source not found.".to_owned()))?;
    let workspace = readable_artefact_workspace(
        pool,
        principal,
        source.workspace_id,
        ResourceKind::Source,
        source.classification(),
    )
    .await?;
    Ok((source, workspace))
}

/// Load one note, with the workspace that governs it.
///
/// # Errors
///
/// Returns [`CoreError::NotFound`] when absent or not readable.
pub async fn get_note(
    pool: &PgPool,
    principal: &Principal,
    note_id: Uuid,
) -> CoreResult<(Note, ResearchWorkspace)> {
    let note = repo::find_note(pool, note_id, principal.organisation_id)
        .await?
        .ok_or_else(|| CoreError::NotFound("Note not found.".to_owned()))?;
    let workspace = readable_artefact_workspace(
        pool,
        principal,
        note.workspace_id,
        ResourceKind::Note,
        note.classification(),
    )
    .await?;
    Ok((note, workspace))
}

/// List the documents of a workspace.
///
/// # Errors
///
/// Returns an error when the caller may not read the workspace.
pub async fn list_documents(
    pool: &PgPool,
    principal: &Principal,
    workspace_id: Uuid,
    page: PageRequest,
) -> CoreResult<Vec<Document>> {
    let workspace = get_workspace(pool, principal, workspace_id).await?;
    let filter = VisibilityFilter::for_principal(principal);
    repo::list_documents(
        pool,
        principal.organisation_id,
        &filter,
        workspace.id,
        page.limit(),
        page.offset(),
    )
    .await
}

/// Authorise a download and issue a short-lived signed URL.
///
/// Every download is authorised here and recorded in the audit trail: knowing
/// an object key grants nothing.
///
/// # Errors
///
/// Returns an error when the caller may not download it, or storage fails.
pub async fn issue_download(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    store: &ObjectStore,
    document_id: Uuid,
) -> CoreResult<String> {
    let document = repo::find_document(&mut **tx, document_id, principal.organisation_id)
        .await?
        .ok_or_else(|| CoreError::NotFound("Document not found.".to_owned()))?;
    let workspace = get_workspace(&mut **tx, principal, document.workspace_id).await?;

    let ctx = workspace_context(&workspace, ResourceKind::Document)
        .with_classification(document.classification());
    authorize(principal, Action::Download, &ctx)
        .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    let (object_key, filename) =
        repo::object_location(&mut **tx, document.current_storage_object_id)
            .await?
            .ok_or_else(|| {
                CoreError::StorageUnavailable(
                    "This object is not available for download.".to_owned(),
                )
            })?;

    let url = store.presigned_download(&object_key, &filename).await?;

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::DOWNLOAD, "document")
            .resource(document.id)
            .context(&ctx),
    )
    .await?;

    Ok(url)
}

/// Attach the full text of a source, where the recorded legal basis allows it.
///
/// This is the enforcement point for the copyright position. Without a recorded
/// basis the request is refused and the bibliography keeps metadata, citation,
/// notes and an authorised link instead.
///
/// # Errors
///
/// Returns [`CoreError::Validation`] when no legal basis is recorded.
pub async fn attach_full_text(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    store: &ObjectStore,
    organisation_slug: &str,
    source_id: Uuid,
    upload: UploadedFile,
) -> CoreResult<Uuid> {
    let source = repo::find_source(&mut **tx, source_id, principal.organisation_id)
        .await?
        .ok_or_else(|| CoreError::NotFound("Source not found.".to_owned()))?;

    let right = source.content_right();
    if !right.allows_full_content() {
        return Err(CoreError::Validation(
            "Full content cannot be stored for this source: no legal basis is recorded. \
             Keep metadata, citation, notes and an authorised link instead."
                .to_owned(),
        ));
    }
    if right == ContentRight::OpenLicence && source.licence.is_none() {
        return Err(CoreError::Validation(
            "An open licence must be named before full content can be stored.".to_owned(),
        ));
    }

    let document_id = create_document(
        tx,
        principal,
        ids,
        store,
        organisation_slug,
        source.workspace_id,
        NewDocument {
            kind: DocumentKind::SourceFullText,
            title: format!("Full text — {}", source.title)
                .chars()
                .take(255)
                .collect(),
            description: None,
            filename: upload.filename,
            content_type: upload.content_type,
            data: upload.data,
            classification: Some(source.classification()),
        },
    )
    .await?;

    repo::set_full_text_document(&mut **tx, source.id, document_id, principal.person_id).await?;

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::UPDATE, "source")
            .resource(source.id)
            .scope(Some(source.unit_id), Some(source.workspace_id))
            .detail("event", "full_text_attached")
            .detail("content_right", right.as_str()),
    )
    .await?;

    Ok(document_id)
}

/// Relate two research objects.
///
/// # Errors
///
/// Returns an error when the caller may not write, or the relation is unknown.
#[expect(
    clippy::too_many_arguments,
    reason = "um parâmetro por coluna: a alternativa é uma struct que só existe para atravessar esta chamada"
)]
pub async fn link_objects(
    tx: &mut Tx<'_>,
    // O pool, para resolver as pontas.
    //
    // A resolução consulta os serviços que detêm cada leitura, e esses recebem
    // um pool. Fora da transacção é o correcto: o que se pergunta é se o
    // recurso existe e é alcançável **agora**, e não o que a transacção ainda
    // não escreveu.
    pool: &PgPool,
    principal: &Principal,
    ids: &CorrelationIds,
    workspace_id: Uuid,
    source_type_name: &str,
    source_id: Uuid,
    relation: &str,
    target_type_name: &str,
    target_id: Uuid,
    note: Option<&str>,
) -> CoreResult<ResearchLink> {
    let workspace = get_workspace(&mut **tx, principal, workspace_id).await?;
    let ctx = workspace_context(&workspace, ResourceKind::ResearchWorkspace);
    authorize(principal, Action::Create, &ctx)
        .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    let Some(verbo) = ocinye_contracts::provenance::ProvenanceRelation::parse(relation) else {
        return Err(CoreError::Validation(format!(
            "Relação desconhecida. As permitidas são: {}.",
            ocinye_contracts::provenance::ProvenanceRelation::all()
                .iter()
                .map(|r| r.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )));
    };

    // ── As duas pontas são recursos, e são desta pessoa ─────────────────
    //
    // Autorizar o Research Workspace não é autorizar o que se liga a partir
    // dele. Esta operação recebia dois pares `(tipo, identificador)` como
    // texto livre e escrevia-os: conhecer um UUID bastava para afirmar, na
    // memória institucional, que a nota de outra unidade se relaciona com a
    // nossa — e a afirmação passava a aparecer na listagem, com o
    // identificador alheio lá dentro.
    //
    // A guarda existia, e estava **numa das entradas**: a capability agentic
    // resolvia os extremos antes de chamar, e a rota HTTP passava as cadeias
    // cruas. Duas portas para a mesma operação, uma delas sem a autoridade da
    // outra — que é precisamente o que Dual Entry existe para impedir.
    //
    // Resolver aqui recusa três coisas de uma vez: um tipo que o domínio não
    // conhece, um recurso que não existe, e um recurso que esta pessoa não
    // alcança. As três respondem o mesmo, porque distingui-las diria a quem
    // pergunta qual das três era (ADR-0100).
    let ponta = |nome: &str, id: Uuid| -> CoreResult<ocinye_contracts::agentic::ResourceRef> {
        let kind = ocinye_contracts::agentic::ResourceKind::parse(nome).ok_or_else(|| {
            CoreError::NotFound("Um dos recursos da relação não foi encontrado.".to_owned())
        })?;
        Ok(ocinye_contracts::agentic::ResourceRef {
            kind,
            id,
            label: None,
        })
    };

    let origem = ponta(source_type_name, source_id)?;
    let destino = ponta(target_type_name, target_id)?;

    for referencia in [&origem, &destino] {
        crate::resources::resolve(pool, principal, referencia)
            .await
            .map_err(|_| {
                CoreError::NotFound("Um dos recursos da relação não foi encontrado.".to_owned())
            })?;
    }

    // ── E o verbo tem de fazer sentido entre estes dois tipos ───────────
    //
    // Um vocabulário fechado impede verbos inventados; não impede
    // combinações absurdas. Quinze verbos e vinte e cinco tipos dão nove mil
    // pares, e quase todos não querem dizer nada — «uma pessoa produzida por
    // um dataset», «uma hipótese que substitui um nó de computação».
    //
    // Uma afirmação sem sentido na linhagem é indistinguível de uma afirmação
    // errada: as duas dizem oficialmente que uma coisa deriva de outra.
    if !verbo.accepts(origem.kind, destino.kind) {
        return Err(CoreError::Validation(format!(
            "«{}» não é uma relação possível entre {} e {}.",
            verbo.label(),
            origem.kind.label(),
            destino.kind.label()
        )));
    }

    let link = repo::insert_link(
        &mut **tx,
        principal.organisation_id,
        Some(workspace.id),
        source_type_name,
        source_id,
        relation,
        target_type_name,
        target_id,
        note,
        principal.person_id,
        // Declarada: alguém afirmou esta relação.
        //
        // `operation` é reservado às operações que **conhecem** a relação sem
        // ambiguidade, e não se pode fabricar por esta porta: uma rota de
        // declaração manual que pudesse escrever `operation` tornaria
        // indistinguível o que uma pessoa afirmou do que o sistema observou.
        ocinye_contracts::provenance::ProvenanceOrigin::Declared.as_str(),
    )
    .await?;

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::CREATE, "research_link")
            .resource(link.id)
            .context(&ctx)
            .detail("relation", relation),
    )
    .await?;

    Ok(link)
}

/// Record a relation the operation itself knows, inside its transaction.
///
/// # Porque isto existe além de `link_objects`
///
/// Porque são duas coisas diferentes, e confundi-las apagaria a distinção que
/// `origin` existe para guardar.
///
/// `link_objects` é a porta da **declaração**: alguém afirma que dois recursos
/// se relacionam, e por isso tem de provar que alcança ambos, que o verbo faz
/// sentido entre aqueles tipos, e que a relação não é inventada.
///
/// Isto é a porta da **observação**: a operação que produziu o facto já sabe a
/// relação — criar um resultado a partir de uma execução *é* a relação. Não há
/// nada a resolver, porque quem chama acabou de autorizar as duas pontas para
/// as escrever; e não há nada a confirmar, porque pedir a alguém que declare o
/// que acabou de fazer seria pedir que repetisse.
///
/// Vive aqui, e não no módulo científico, porque quem detém `research_links` é
/// o Conhecimento. Um segundo sítio a escrever nessa tabela seria um segundo
/// dono, e dois donos acabam com duas ideias sobre o que lá pode estar.
///
/// # Errors
///
/// Returns an error when the insert fails.
#[allow(clippy::too_many_arguments)]
pub async fn record_operation_provenance(
    tx: &mut Tx<'_>,
    organisation_id: Uuid,
    workspace_id: Option<Uuid>,
    source_kind: ocinye_contracts::agentic::ResourceKind,
    source_id: Uuid,
    relation: ocinye_contracts::provenance::ProvenanceRelation,
    target_kind: ocinye_contracts::agentic::ResourceKind,
    target_id: Uuid,
    created_by: Uuid,
) -> CoreResult<ResearchLink> {
    // A matriz vale aqui também.
    //
    // Uma operação não escreve relações absurdas por ser uma operação: se um
    // par deixar de fazer sentido, é aqui que se descobre — e não meses depois,
    // ao ler a linhagem.
    if !relation.accepts(source_kind, target_kind) {
        return Err(CoreError::Validation(format!(
            "«{}» não é uma relação possível entre {} e {}.",
            relation.label(),
            source_kind.label(),
            target_kind.label()
        )));
    }

    repo::insert_link(
        &mut **tx,
        organisation_id,
        workspace_id,
        source_kind.as_str(),
        source_id,
        relation.as_str(),
        target_kind.as_str(),
        target_id,
        None,
        created_by,
        ocinye_contracts::provenance::ProvenanceOrigin::Operation.as_str(),
    )
    .await
}

/// List the relations of a workspace.
///
/// # Errors
///
/// Returns an error when the caller may not read the workspace.
pub async fn list_links(
    pool: &PgPool,
    principal: &Principal,
    workspace_id: Uuid,
) -> CoreResult<Vec<ResearchLink>> {
    let workspace = get_workspace(pool, principal, workspace_id).await?;
    repo::list_links(pool, workspace.id).await
}
/// Rever uma bibliografia BibTeX: ler o que lá está e escrevê-lo em forma canónica.
///
/// # A primeira operação institucional que atravessa o Capability Runtime
///
/// A leitura acontece dentro do isolamento WASM/WASI: sem rede, sem sistema de
/// ficheiros, sem ambiente, sem base de dados, com combustível e tempo
/// contados. É o desenho certo para código que processa texto que alguém colou
/// — um analisador é a superfície que mais entrada não confiável vê.
///
/// O Runtime executa. Esta função é que decide **quem pode pedir**, **o que
/// entra**, **o que sai** e **o que isso significa**. O componente não sabe
/// quem é o membro, em que workspace está, nem que a Ocinye existe.
///
/// # Porque é preciso poder acrescentar referências
///
/// Rever bibliografia é o passo anterior a acrescentá-la, e não faz sentido
/// noutro sítio. Autoriza-se contra a mesma decisão que `create_source`: quem
/// pode acrescentar uma referência neste workspace pode preparar as suas.
///
/// Não é uma permissão nova. Uma capacidade que não escreve nada não justifica
/// alargar o modelo de acesso, e a que existe descreve exactamente o direito.
///
/// # Não guarda nada
///
/// Nem referência, nem documento, nem rasto do que foi colado. Quem quiser
/// guardar uma referência usa `create_source`, que é uma decisão separada e
/// deliberada.
///
/// # Errors
///
/// - Recusa quando o membro não alcança o workspace, ou não pode acrescentar
///   referências nele.
/// - Recusa uma bibliografia acima de [`MAX_BIBTEX_BYTES`].
/// - Recusa quando o componente não está construído nesta instalação.
pub async fn review_bibliography(
    executor: impl PgExecutor<'_>,
    capabilities: &Capabilities,
    principal: &Principal,
    workspace_id: Uuid,
    bibtex: &str,
) -> CoreResult<BibliographyReview> {
    let workspace = get_workspace(executor, principal, workspace_id).await?;
    let ctx = workspace_context(&workspace, ResourceKind::Source);
    authorize(principal, Action::Create, &ctx)
        .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    // O limite é conferido aqui, e é este o limite. As camadas de fora podem
    // recusar mais cedo para poupar trabalho; nenhuma delas é a autoridade.
    if bibtex.len() > MAX_BIBTEX_BYTES {
        return Err(CoreError::Validation(format!(
            "A bibliografia excede o máximo de {MAX_BIBTEX_BYTES} caracteres."
        )));
    }

    let saida = capabilities
        .run(Component::BibtexImport, bibtex.as_bytes().to_vec())
        .await?;

    // O que o componente escreveu é matéria-prima. Só depois de passar por aqui
    // é que é uma revisão de bibliografia do Ocinye OS.
    interpretar(&saida)
}

/// O que o componente escreveu, lido como uma revisão institucional.
///
/// # Porque isto não é `serde_json::from_slice` e mais nada
///
/// Porque o componente corre isolado **e não é de confiança**. Pode devolver
/// JSON que não é o esperado, uma lista maior do que o razoável, ou nada. Cada
/// um desses casos tem de virar um erro do Core, e nenhum pode virar uma
/// revisão vazia com ar de sucesso.
fn interpretar(saida: &[u8]) -> CoreResult<BibliographyReview> {
    #[derive(serde::Deserialize)]
    struct Registo {
        entry_type: String,
        citation_key: String,
        title: Option<String>,
        #[serde(default)]
        authors: Vec<String>,
        year: Option<i32>,
        container_title: Option<String>,
        doi: Option<String>,
    }

    #[derive(serde::Deserialize)]
    struct Saida {
        #[serde(default)]
        sources: Vec<Registo>,
        #[serde(default)]
        skipped: Vec<String>,
        #[serde(default)]
        normalized: String,
    }

    let cru: Saida = serde_json::from_slice(saida).map_err(|_| {
        CoreError::Internal("A capacidade não devolveu um resultado utilizável.".to_owned())
    })?;

    if cru.sources.len() > MAX_ENTRIES || cru.skipped.len() > MAX_ENTRIES {
        return Err(CoreError::Validation(
            "A bibliografia tem entradas a mais para uma revisão de uma vez.".to_owned(),
        ));
    }

    Ok(BibliographyReview {
        entries: cru
            .sources
            .into_iter()
            .map(|registo| BibliographyEntry {
                entry_type: registo.entry_type,
                citation_key: registo.citation_key,
                title: registo.title,
                authors: registo.authors,
                year: registo.year,
                container_title: registo.container_title,
                doi: registo.doi,
            })
            .collect(),
        unreadable: cru.skipped,
        normalized: cru.normalized,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Uma saída que não é JSON não vira uma revisão vazia.
    ///
    /// A distinção importa: uma revisão vazia diz «a sua bibliografia não tem
    /// nada», e o que aconteceu foi que o componente não cumpriu o contrato.
    #[test]
    fn uma_saida_ilegivel_e_um_erro_e_nao_uma_revisao_vazia() {
        let erro = interpretar(b"isto nao e json").expect_err("nao devia ler");
        assert!(matches!(erro, CoreError::Internal(_)), "veio {erro:?}");
    }

    /// Uma saída com entradas a mais é recusada.
    ///
    /// # Porque o limite de entrada não chega
    ///
    /// Porque a entrada é limitada em **caracteres** e a saída em **entradas**,
    /// e um componente futuro pode ser mais generoso do que este. Isto é o
    /// segundo fecho, do lado de quem lê: o Core não é obrigado a acreditar no
    /// que corre dentro do isolamento.
    #[test]
    fn uma_saida_com_entradas_a_mais_e_recusada() {
        let registo = r#"{"entry_type":"a","citation_key":"k","authors":[]}"#;
        let muitos = std::iter::repeat_n(registo, MAX_ENTRIES + 1)
            .collect::<Vec<_>>()
            .join(",");
        let saida = format!(r#"{{"sources":[{muitos}],"skipped":[],"normalized":""}}"#);

        let erro = interpretar(saida.as_bytes()).expect_err("entradas a mais não passam");
        assert!(matches!(erro, CoreError::Validation(_)), "veio {erro:?}");
    }

    /// Uma saída dentro dos limites lê-se.
    #[test]
    fn uma_saida_valida_le_se() {
        let saida = r#"{"sources":[{"entry_type":"article","citation_key":"k",
                      "title":"T","authors":["A"],"year":2024}],
                      "skipped":["@misc{x"],"normalized":"@article{k,\n}"}"#;
        let revisao = interpretar(saida.as_bytes()).expect("devia ler-se");
        assert_eq!(revisao.read_count(), 1);
        assert!(!revisao.is_complete());
        assert_eq!(revisao.entries[0].citation_key, "k");
    }
}
