//! Knowledge persistence.

use chrono::{DateTime, Utc};
use ocinye_contracts::Classification;
use ocinye_domain::policy::VisibilityFilter;
use sqlx::PgExecutor;
use uuid::Uuid;

use super::model::{Document, Note, ResearchLink, Source};
use crate::error::CoreResult;
use crate::visibility::{contained_in_visible_workspace, to_sql, VisibilityColumns};

const SOURCE_COLUMNS: &str = "id, unit_id, workspace_id, source_type, title, authors, year,
                              container_title, publisher, doi, isbn, url,
                              abstract AS abstract_text, keywords, licence, content_right,
                              origin, citation_key, classification, full_text_document_id,
                              created_at";

const NOTE_COLUMNS: &str = "id, owner_id, unit_id, workspace_id, folder_id, title, body, tags,
                            classification, revision, document, schema_version, created_at,
                            updated_at";

/// Document columns joined with their stored object, so a caller sees size and
/// checksum without a second query.
///
/// # Porque a junção passa pelo ficheiro
///
/// O objecto de um documento resolve-se agora pela identidade estável do
/// ficheiro e pela sua versão corrente, e não pela coluna que o documento ainda
/// guarda. As duas dizem o mesmo — há um teste que o exige enquanto ambas
/// existirem —, mas só esta continua a dizer a verdade depois de alguém
/// carregar uma versão nova.
///
/// **Corrente é a de maior `sequence`**, e nunca a mais recente por relógio: as
/// datas empatam, e as do preenchimento histórico foram herdadas de outra
/// coisa.
///
/// # Porque a classificação vem do ficheiro
///
/// Porque é o ficheiro que governa o artefacto. `documents.classification`
/// existe ainda ao lado, e as duas dizem o mesmo — o escritor põe o mesmo valor
/// nas duas —, mas só uma delas é a autoridade. Enquanto ambas existirem, um
/// teste exige que coincidam; depois disso, resta uma.
///
/// A escolha é uma junção lateral e não um `DISTINCT ON` porque este texto é
/// prefixo de consultas que trazem a sua própria ordenação — por título, por
/// data —, e o `DISTINCT ON` obrigá-las-ia todas a começar por `d.id`. A
/// lateral escolhe uma linha por documento sem tocar na ordem de quem chama.
const DOCUMENT_SELECT: &str = "SELECT d.id, d.unit_id, d.workspace_id,
                                      v.storage_object_id AS current_storage_object_id,
                                      d.kind, d.title, d.description, d.document_date,
                                      f.classification, o.original_filename, o.content_type,
                                      o.size_bytes, o.checksum_sha256, d.created_at
                                 FROM documents d
                                 JOIN files f ON f.id = d.file_id
                                 JOIN LATERAL (
                                     SELECT fv.storage_object_id
                                       FROM file_versions fv
                                      WHERE fv.file_id = d.file_id
                                      ORDER BY fv.sequence DESC
                                      LIMIT 1
                                 ) v ON TRUE
                                 JOIN storage_objects o ON o.id = v.storage_object_id";

// --- Sources ---------------------------------------------------------------

/// Insert a source.
///
/// # Errors
///
/// Returns an error when the insert fails.
pub async fn insert_source<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    unit_id: Uuid,
    workspace_id: Uuid,
    source: &NewSourceRow<'_>,
    classification: Classification,
    created_by: Uuid,
) -> CoreResult<Source> {
    let row = sqlx::query_as::<_, Source>(&format!(
        "INSERT INTO sources
             (organisation_id, unit_id, workspace_id, source_type, title, authors, year,
              container_title, publisher, doi, isbn, url, abstract, keywords, licence,
              content_right, origin, citation_key, classification, raw_metadata, created_by_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15,
                 $16, $17, $18, $19, $20, $21)
         RETURNING {SOURCE_COLUMNS}"
    ))
    .bind(organisation_id)
    .bind(unit_id)
    .bind(workspace_id)
    .bind(source.source_type)
    .bind(source.title)
    .bind(source.authors)
    .bind(source.year)
    .bind(source.container_title)
    .bind(source.publisher)
    .bind(source.doi)
    .bind(source.isbn)
    .bind(source.url)
    .bind(source.abstract_text)
    .bind(source.keywords)
    .bind(source.licence)
    .bind(source.content_right)
    .bind(source.origin)
    .bind(source.citation_key)
    .bind(classification.as_str())
    .bind(&source.raw_metadata)
    .bind(created_by)
    .fetch_one(executor)
    .await?;
    Ok(row)
}

/// Column values for a new source.
pub struct NewSourceRow<'a> {
    /// Kind of source.
    pub source_type: &'a str,
    /// Title.
    pub title: &'a str,
    /// Authors.
    pub authors: &'a [String],
    /// Year.
    pub year: Option<i32>,
    /// Journal, proceedings or book title.
    pub container_title: Option<&'a str>,
    /// Publisher.
    pub publisher: Option<&'a str>,
    /// DOI.
    pub doi: Option<&'a str>,
    /// ISBN.
    pub isbn: Option<&'a str>,
    /// Authorised link.
    pub url: Option<&'a str>,
    /// Abstract.
    pub abstract_text: Option<&'a str>,
    /// Keywords.
    pub keywords: &'a [String],
    /// Licence.
    pub licence: Option<&'a str>,
    /// Recorded legal basis.
    pub content_right: &'a str,
    /// Where it came from.
    pub origin: Option<&'a str>,
    /// Citation key.
    pub citation_key: Option<&'a str>,
    /// Raw imported record, kept for provenance.
    pub raw_metadata: serde_json::Value,
}

/// Load a source.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn find_source<'e>(
    executor: impl PgExecutor<'e>,
    id: Uuid,
    organisation_id: Uuid,
) -> CoreResult<Option<Source>> {
    let source = sqlx::query_as::<_, Source>(&format!(
        "SELECT {SOURCE_COLUMNS} FROM sources WHERE id = $1 AND organisation_id = $2"
    ))
    .bind(id)
    .bind(organisation_id)
    .fetch_optional(executor)
    .await?;
    Ok(source)
}

/// List sources of a workspace that the principal may read.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn list_sources<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    filter: &VisibilityFilter,
    workspace_id: Uuid,
    limit: i64,
    offset: i64,
) -> CoreResult<Vec<Source>> {
    let predicate = to_sql(filter, VisibilityColumns::default());
    let sources = sqlx::query_as::<_, Source>(&format!(
        "SELECT {SOURCE_COLUMNS} FROM sources
          WHERE organisation_id = $1 AND workspace_id = $2 AND {predicate}
          ORDER BY year DESC NULLS LAST, title
          LIMIT $3 OFFSET $4"
    ))
    .bind(organisation_id)
    .bind(workspace_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(executor)
    .await?;
    Ok(sources)
}

/// Count sources of a workspace that the principal may read.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn count_sources<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    filter: &VisibilityFilter,
    workspace_id: Uuid,
) -> CoreResult<i64> {
    let predicate = to_sql(filter, VisibilityColumns::default());
    let total = sqlx::query_scalar::<_, i64>(&format!(
        "SELECT COUNT(*) FROM sources
          WHERE organisation_id = $1 AND workspace_id = $2 AND {predicate}"
    ))
    .bind(organisation_id)
    .bind(workspace_id)
    .fetch_one(executor)
    .await?;
    Ok(total)
}

/// Attach a full-text document to a source.
///
/// The database also enforces that this cannot happen without a recorded legal
/// basis; this query would fail the constraint rather than succeed.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn set_full_text_document<'e>(
    executor: impl PgExecutor<'e>,
    source_id: Uuid,
    document_id: Uuid,
    updated_by: Uuid,
) -> CoreResult<()> {
    sqlx::query(
        "UPDATE sources SET full_text_document_id = $2, updated_by_id = $3, updated_at = now()
          WHERE id = $1",
    )
    .bind(source_id)
    .bind(document_id)
    .bind(updated_by)
    .execute(executor)
    .await?;
    Ok(())
}

// --- Notes -----------------------------------------------------------------

/// Insert a note.
///
/// # Errors
///
/// Returns an error when the insert fails.
#[expect(
    clippy::too_many_arguments,
    reason = "um parâmetro por coluna: a alternativa é uma struct que só existe para atravessar esta chamada"
)]
pub async fn insert_note<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    unit_id: Uuid,
    workspace_id: Uuid,
    title: &str,
    body: &str,
    tags: &[String],
    classification: Classification,
    created_by: Uuid,
) -> CoreResult<Note> {
    let note = sqlx::query_as::<_, Note>(&format!(
        "INSERT INTO notes
             (organisation_id, unit_id, workspace_id, title, body, tags,
              classification, revision, created_by_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, 1, $8)
         RETURNING {NOTE_COLUMNS}"
    ))
    .bind(organisation_id)
    .bind(unit_id)
    .bind(workspace_id)
    .bind(title)
    .bind(body)
    .bind(tags)
    .bind(classification.as_str())
    .bind(created_by)
    .fetch_one(executor)
    .await?;
    Ok(note)
}

/// Load a note.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn find_note<'e>(
    executor: impl PgExecutor<'e>,
    id: Uuid,
    organisation_id: Uuid,
) -> CoreResult<Option<Note>> {
    let note = sqlx::query_as::<_, Note>(&format!(
        "SELECT {NOTE_COLUMNS} FROM notes WHERE id = $1 AND organisation_id = $2"
    ))
    .bind(id)
    .bind(organisation_id)
    .fetch_optional(executor)
    .await?;
    Ok(note)
}

/// Snapshot the current note into its revision history.
///
/// Taken before every edit, so a note's history is preserved. Copies the row
/// directly, so the immutable revision carries the same structured `document`
/// and its `schema_version`, and records **who wrote it** — the last editor, or
/// the creator for the first revision.
///
/// # Errors
///
/// Returns an error when the insert fails.
pub async fn snapshot_note<'e>(executor: impl PgExecutor<'e>, note_id: Uuid) -> CoreResult<()> {
    sqlx::query(
        "INSERT INTO note_revisions
             (note_id, revision, title, body, document, schema_version, authored_by_id)
         SELECT id, revision, title, body, document, schema_version,
                COALESCE(updated_by_id, created_by_id)
           FROM notes
          WHERE id = $1
         ON CONFLICT (note_id, revision) DO NOTHING",
    )
    .bind(note_id)
    .execute(executor)
    .await?;
    Ok(())
}

/// Insert a personal note — owned by a member, with no workspace.
///
/// # Errors
///
/// Returns an error when the insert fails.
#[expect(
    clippy::too_many_arguments,
    reason = "um parâmetro por coluna: a alternativa é uma struct que só existe para atravessar esta chamada"
)]
pub async fn insert_personal_note<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    owner_id: Uuid,
    title: &str,
    plain_text: &str,
    tags: &[String],
    classification: Classification,
    document: &serde_json::Value,
    schema_version: i32,
    created_by: Uuid,
) -> CoreResult<Note> {
    let note = sqlx::query_as::<_, Note>(&format!(
        "INSERT INTO notes
             (organisation_id, owner_id, title, body, tags, classification,
              revision, document, schema_version, created_by_id, updated_by_id)
         VALUES ($1, $2, $3, $4, $5, $6, 1, $7, $8, $9, $9)
         RETURNING {NOTE_COLUMNS}"
    ))
    .bind(organisation_id)
    .bind(owner_id)
    .bind(title)
    .bind(plain_text)
    .bind(tags)
    .bind(classification.as_str())
    .bind(document)
    .bind(schema_version)
    .bind(created_by)
    .fetch_one(executor)
    .await?;
    Ok(note)
}

/// Update a note only if it is still at the expected revision.
///
/// The base-revision guard is the optimistic-concurrency check (ADR-0413 §5):
/// if someone else advanced the note since this editor loaded it, zero rows
/// match and this returns `None`, and the caller raises a conflict instead of
/// silently clobbering the newer content.
///
/// # Errors
///
/// Returns an error when the statement fails.
#[expect(
    clippy::too_many_arguments,
    reason = "um parâmetro por coluna: a alternativa é uma struct que só existe para atravessar esta chamada"
)]
pub async fn update_note_at_revision<'e>(
    executor: impl PgExecutor<'e>,
    note_id: Uuid,
    base_revision: i32,
    title: &str,
    plain_text: &str,
    tags: Option<&[String]>,
    document: &serde_json::Value,
    schema_version: i32,
    updated_by: Uuid,
) -> CoreResult<Option<Note>> {
    let note = sqlx::query_as::<_, Note>(&format!(
        "UPDATE notes
            SET title = $3,
                body = $4,
                tags = COALESCE($5, tags),
                document = $6,
                schema_version = $7,
                revision = revision + 1,
                updated_by_id = $8,
                updated_at = now()
          WHERE id = $1 AND revision = $2
          RETURNING {NOTE_COLUMNS}"
    ))
    .bind(note_id)
    .bind(base_revision)
    .bind(title)
    .bind(plain_text)
    .bind(tags)
    .bind(document)
    .bind(schema_version)
    .bind(updated_by)
    .fetch_optional(executor)
    .await?;
    Ok(note)
}

/// The personal notes of a member, most recently changed first.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn list_personal_notes<'e>(
    executor: impl PgExecutor<'e>,
    owner_id: Uuid,
    tag: Option<&str>,
    folder_id: Option<Uuid>,
    limit: i64,
    offset: i64,
) -> CoreResult<Vec<Note>> {
    // Os filtros por etiqueta e por pasta são opcionais e independentes; um `$`
    // nulo não recorta. A filtragem é na base, e por isso a paginação conta a
    // partir do conjunto já recortado.
    let notes = sqlx::query_as::<_, Note>(&format!(
        "SELECT {NOTE_COLUMNS} FROM notes
          WHERE owner_id = $1
            AND ($2::text IS NULL OR $2 = ANY(tags))
            AND ($3::uuid IS NULL OR folder_id = $3)
          ORDER BY updated_at DESC
          LIMIT $4 OFFSET $5"
    ))
    .bind(owner_id)
    .bind(tag)
    .bind(folder_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(executor)
    .await?;
    Ok(notes)
}

/// Move uma nota pessoal para uma pasta (ou para a raiz), pelo dono.
///
/// Mover não é editar o conteúdo: não incrementa a revisão nem tira *snapshot*.
/// Devolve se alterou — `false` quando a nota não é do dono.
///
/// # Errors
///
/// Devolve erro quando a consulta falha.
pub async fn set_note_folder<'e>(
    executor: impl PgExecutor<'e>,
    owner_id: Uuid,
    note_id: Uuid,
    folder_id: Option<Uuid>,
) -> CoreResult<bool> {
    let done = sqlx::query(
        "UPDATE notes SET folder_id = $3, updated_at = now()
          WHERE id = $1 AND owner_id = $2",
    )
    .bind(note_id)
    .bind(owner_id)
    .bind(folder_id)
    .execute(executor)
    .await?;
    Ok(done.rows_affected() > 0)
}

// ── Partilha de notas ───────────────────────────────────────────────────

/// Um destinatário de uma partilha viva: quem, com que papel, desde quando.
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct NoteShareRow {
    /// A pessoa com quem se partilhou.
    pub person_id: Uuid,
    /// O nome dessa pessoa, para a lista de destinatários.
    pub person_name: String,
    /// `viewer` ou `editor`.
    pub role: String,
    /// Quando a partilha foi concedida.
    pub granted_at: DateTime<Utc>,
}

/// Partilha uma nota com uma pessoa, ou muda-lhe o papel.
///
/// Revoga qualquer partilha viva anterior da mesma pessoa e insere uma nova —
/// assim mudar de `viewer` para `editor` não deixa duas linhas vivas, e o índice
/// único de partilha viva é respeitado.
///
/// # Errors
///
/// Devolve erro quando a consulta falha.
pub async fn upsert_note_share(
    executor: &mut sqlx::PgConnection,
    note_id: Uuid,
    person_id: Uuid,
    role: &str,
    granted_by: Uuid,
) -> CoreResult<()> {
    sqlx::query(
        "UPDATE note_shares SET revoked_at = now(), revoked_by_id = $3
          WHERE note_id = $1 AND person_id = $2 AND revoked_at IS NULL",
    )
    .bind(note_id)
    .bind(person_id)
    .bind(granted_by)
    .execute(&mut *executor)
    .await?;

    sqlx::query(
        "INSERT INTO note_shares (note_id, person_id, role, granted_by_id)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(note_id)
    .bind(person_id)
    .bind(role)
    .bind(granted_by)
    .execute(&mut *executor)
    .await?;
    Ok(())
}

/// Revoga a partilha viva de uma pessoa sobre uma nota. Devolve se revogou.
///
/// # Errors
///
/// Devolve erro quando a consulta falha.
pub async fn revoke_note_share<'e>(
    executor: impl PgExecutor<'e>,
    note_id: Uuid,
    person_id: Uuid,
    revoked_by: Uuid,
) -> CoreResult<bool> {
    let done = sqlx::query(
        "UPDATE note_shares SET revoked_at = now(), revoked_by_id = $3
          WHERE note_id = $1 AND person_id = $2 AND revoked_at IS NULL",
    )
    .bind(note_id)
    .bind(person_id)
    .bind(revoked_by)
    .execute(executor)
    .await?;
    Ok(done.rows_affected() > 0)
}

/// O papel vivo de uma pessoa sobre uma nota, se houver — a autoridade da
/// partilha, lida agora.
///
/// # Errors
///
/// Devolve erro quando a consulta falha.
pub async fn note_share_role<'e>(
    executor: impl PgExecutor<'e>,
    note_id: Uuid,
    person_id: Uuid,
) -> CoreResult<Option<String>> {
    let role: Option<String> = sqlx::query_scalar(
        "SELECT role FROM note_shares
          WHERE note_id = $1 AND person_id = $2 AND revoked_at IS NULL",
    )
    .bind(note_id)
    .bind(person_id)
    .fetch_optional(executor)
    .await?;
    Ok(role)
}

/// Os destinatários vivos de uma nota.
///
/// # Errors
///
/// Devolve erro quando a consulta falha.
pub async fn list_note_shares<'e>(
    executor: impl PgExecutor<'e>,
    note_id: Uuid,
) -> CoreResult<Vec<NoteShareRow>> {
    let shares = sqlx::query_as::<_, NoteShareRow>(
        "SELECT s.person_id, p.full_name AS person_name, s.role, s.granted_at
           FROM note_shares s
           JOIN people p ON p.id = s.person_id
          WHERE s.note_id = $1 AND s.revoked_at IS NULL
          ORDER BY lower(p.full_name)",
    )
    .bind(note_id)
    .fetch_all(executor)
    .await?;
    Ok(shares)
}

/// As notas vivas partilhadas com uma pessoa — «partilhadas comigo».
///
/// # Errors
///
/// Devolve erro quando a consulta falha.
pub async fn list_notes_shared_with<'e>(
    executor: impl PgExecutor<'e>,
    person_id: Uuid,
    limit: i64,
    offset: i64,
) -> CoreResult<Vec<Note>> {
    // As colunas qualificam-se com o alias da nota: `note_shares` também tem uma
    // coluna `id`, e um `id` sem prefixo seria ambíguo neste JOIN.
    let colunas: String = NOTE_COLUMNS
        .split(',')
        .map(|c| format!("n.{}", c.trim()))
        .collect::<Vec<_>>()
        .join(", ");
    let notes = sqlx::query_as::<_, Note>(&format!(
        "SELECT {colunas} FROM notes n
           JOIN note_shares s ON s.note_id = n.id
          WHERE s.person_id = $1 AND s.revoked_at IS NULL
          ORDER BY n.updated_at DESC
          LIMIT $2 OFFSET $3"
    ))
    .bind(person_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(executor)
    .await?;
    Ok(notes)
}

/// Uma versão de ficheiro é referenciada por uma nota partilhada viva com esta
/// pessoa?
///
/// A pré-visualização de uma imagem numa nota partilhada passa por aqui: como as
/// notas só referenciam ficheiros do próprio dono ([`super::service`]), uma nota
/// partilhada expõe ao destinatário exactamente as imagens que ela contém, e mais
/// nenhuma.
///
/// # Errors
///
/// Devolve erro quando a consulta falha.
pub async fn shared_note_references_version<'e>(
    executor: impl PgExecutor<'e>,
    person_id: Uuid,
    file_version_id: Uuid,
) -> CoreResult<bool> {
    let referencia = serde_json::json!([{ "file_version_id": file_version_id.to_string() }]);
    let existe: Option<bool> = sqlx::query_scalar(
        "SELECT TRUE FROM notes n
           JOIN note_shares s ON s.note_id = n.id
          WHERE s.person_id = $1 AND s.revoked_at IS NULL
            AND n.document -> 'blocks' @> $2
          LIMIT 1",
    )
    .bind(person_id)
    .bind(referencia)
    .fetch_optional(executor)
    .await?;
    Ok(existe.unwrap_or(false))
}

/// One row of a note's revision history: which revision, by whom, and when.
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct NoteRevisionMeta {
    /// The revision number.
    pub revision: i32,
    /// The title as it was at that revision.
    pub title: String,
    /// Who wrote it.
    pub authored_by_id: Option<Uuid>,
    /// That person's name, for a history that reads without identifiers.
    pub author_name: Option<String>,
    /// When.
    pub created_at: DateTime<Utc>,
}

/// The revision history of a note, newest first.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn list_note_revisions<'e>(
    executor: impl PgExecutor<'e>,
    note_id: Uuid,
) -> CoreResult<Vec<NoteRevisionMeta>> {
    let rows = sqlx::query_as::<_, NoteRevisionMeta>(
        "SELECT r.revision, r.title, r.authored_by_id, p.full_name AS author_name, r.created_at
           FROM note_revisions r
           LEFT JOIN people p ON p.id = r.authored_by_id
          WHERE r.note_id = $1
          ORDER BY r.revision DESC",
    )
    .bind(note_id)
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

/// The stored content of one revision: the title and the structured document
/// as they were then. It is what a restore replays, and what a preview renders.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct NoteRevisionContent {
    /// The title at that revision.
    pub title: String,
    /// The structured document at that revision. `None` for a legacy revision
    /// snapshotted before the structured body existed.
    pub document: Option<serde_json::Value>,
    /// The schema version of that document.
    pub schema_version: i32,
}

/// The content of one exact revision of a note.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn note_revision_content<'e>(
    executor: impl PgExecutor<'e>,
    note_id: Uuid,
    revision: i32,
) -> CoreResult<Option<NoteRevisionContent>> {
    let row = sqlx::query_as::<_, NoteRevisionContent>(
        "SELECT title, document, schema_version
           FROM note_revisions
          WHERE note_id = $1 AND revision = $2",
    )
    .bind(note_id)
    .bind(revision)
    .fetch_optional(executor)
    .await?;
    Ok(row)
}

/// Update a note and advance its revision.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn update_note<'e>(
    executor: impl PgExecutor<'e>,
    note_id: Uuid,
    title: Option<&str>,
    body: Option<&str>,
    tags: Option<&[String]>,
    updated_by: Uuid,
) -> CoreResult<Note> {
    let note = sqlx::query_as::<_, Note>(&format!(
        "UPDATE notes
            SET title = COALESCE($2, title),
                body = COALESCE($3, body),
                tags = COALESCE($4, tags),
                revision = revision + 1,
                updated_by_id = $5,
                updated_at = now()
          WHERE id = $1
          RETURNING {NOTE_COLUMNS}"
    ))
    .bind(note_id)
    .bind(title)
    .bind(body)
    .bind(tags)
    .bind(updated_by)
    .fetch_one(executor)
    .await?;
    Ok(note)
}

/// List notes of a workspace that the principal may read.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn list_notes<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    filter: &VisibilityFilter,
    workspace_id: Uuid,
    limit: i64,
    offset: i64,
) -> CoreResult<Vec<Note>> {
    let predicate = to_sql(filter, VisibilityColumns::default());
    let notes = sqlx::query_as::<_, Note>(&format!(
        "SELECT {NOTE_COLUMNS} FROM notes
          WHERE organisation_id = $1 AND workspace_id = $2 AND {predicate}
          ORDER BY updated_at DESC
          LIMIT $3 OFFSET $4"
    ))
    .bind(organisation_id)
    .bind(workspace_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(executor)
    .await?;
    Ok(notes)
}

// --- Documents -------------------------------------------------------------

/// Insert a document referencing an already-stored object.
///
/// # Errors
///
/// Returns an error when the insert fails.
#[expect(
    clippy::too_many_arguments,
    reason = "um parâmetro por coluna: a alternativa é uma struct que só existe para atravessar esta chamada"
)]
pub async fn insert_document<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    unit_id: Uuid,
    workspace_id: Uuid,
    file_id: Uuid,
    kind: &str,
    title: &str,
    description: Option<&str>,
    created_by: Uuid,
) -> CoreResult<Uuid> {
    // Nem o objecto nem a classificação. O primeiro chega-se pela versão
    // corrente do ficheiro; a segunda **é** do ficheiro, que governa o
    // artefacto. As duas colunas homónimas de `documents` desapareceram, e com
    // elas a possibilidade de duas fontes discordarem.
    //
    // O que resta em `documents` é o que só ele sabe: título, espécie,
    // descrição e data documental.
    let id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO documents
             (organisation_id, unit_id, workspace_id, file_id, kind,
              title, description, created_by_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         RETURNING id",
    )
    .bind(organisation_id)
    .bind(unit_id)
    .bind(workspace_id)
    .bind(file_id)
    .bind(kind)
    .bind(title)
    .bind(description)
    .bind(created_by)
    .fetch_one(executor)
    .await?;
    Ok(id)
}

/// Load a document.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn find_document<'e>(
    executor: impl PgExecutor<'e>,
    id: Uuid,
    organisation_id: Uuid,
) -> CoreResult<Option<Document>> {
    let document = sqlx::query_as::<_, Document>(&format!(
        "{DOCUMENT_SELECT} WHERE d.id = $1 AND d.organisation_id = $2"
    ))
    .bind(id)
    .bind(organisation_id)
    .fetch_optional(executor)
    .await?;
    Ok(document)
}

/// List documents of a workspace that the principal may read.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn list_documents<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    filter: &VisibilityFilter,
    workspace_id: Uuid,
    limit: i64,
    offset: i64,
) -> CoreResult<Vec<Document>> {
    let predicate = to_sql(filter, DOCUMENT_VISIBILITY);
    let documents = sqlx::query_as::<_, Document>(&format!(
        "{DOCUMENT_SELECT}
          WHERE d.organisation_id = $1 AND d.workspace_id = $2 AND {predicate}
          ORDER BY d.created_at DESC
          LIMIT $3 OFFSET $4"
    ))
    .bind(organisation_id)
    .bind(workspace_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(executor)
    .await?;
    Ok(documents)
}

/// The object key and filename of a document, for issuing a download.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn object_location<'e>(
    executor: impl PgExecutor<'e>,
    storage_object_id: Uuid,
) -> CoreResult<Option<(String, String)>> {
    let row = sqlx::query_as::<_, (String, String)>(
        "SELECT object_key, original_filename FROM storage_objects
          WHERE id = $1 AND status = 'stored'",
    )
    .bind(storage_object_id)
    .fetch_optional(executor)
    .await?;
    Ok(row)
}

// --- Research links --------------------------------------------------------

/// Insert a typed relation between two research objects.
///
/// # Errors
///
/// Returns an error when the insert fails.
#[expect(
    clippy::too_many_arguments,
    reason = "um parâmetro por coluna: a alternativa é uma struct que só existe para atravessar esta chamada"
)]
pub async fn insert_link<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    // Ausente quando a relação atravessa ambientes.
    //
    // `NULL` diz que a aresta **não está confinada a um ambiente**. Nunca diz
    // que é legível ou escrevível por toda a gente: a autoridade vem sempre
    // das duas pontas e da política corrente.
    workspace_id: Option<Uuid>,
    source_type_name: &str,
    source_id: Uuid,
    relation: &str,
    target_type_name: &str,
    target_id: Uuid,
    note: Option<&str>,
    created_by: Uuid,
    // De onde veio a afirmação: alguém a declarou, ou a operação conhecia-a.
    origin: &str,
) -> CoreResult<ResearchLink> {
    let link = sqlx::query_as::<_, ResearchLink>(
        "INSERT INTO research_links
             (organisation_id, workspace_id, source_type_name, source_id, relation,
              target_type_name, target_id, note, created_by_id, origin)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
         -- Uma relação já afirmada não se afirma outra vez.
         --
         -- Sem isto, repetir a operação criaria uma segunda aresta idêntica, e
         -- a linhagem passaria a mostrar o mesmo facto duas vezes. O índice
         -- único já o impediria — com um erro de integridade, que não é uma
         -- resposta que se mostre a quem só repetiu um pedido.
         ON CONFLICT (source_type_name, source_id, relation, target_type_name, target_id)
         DO UPDATE SET note = COALESCE(EXCLUDED.note, research_links.note)
         RETURNING id, workspace_id, source_type_name, source_id, relation,
                   target_type_name, target_id, note, created_at",
    )
    .bind(organisation_id)
    .bind(workspace_id)
    .bind(source_type_name)
    .bind(source_id)
    .bind(relation)
    .bind(target_type_name)
    .bind(target_id)
    .bind(note)
    .bind(created_by)
    .bind(origin)
    .fetch_one(executor)
    .await?;
    Ok(link)
}

/// List the relations of a workspace.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn list_links<'e>(
    executor: impl PgExecutor<'e>,
    workspace_id: Uuid,
) -> CoreResult<Vec<ResearchLink>> {
    let links = sqlx::query_as::<_, ResearchLink>(
        "SELECT id, workspace_id, source_type_name, source_id, relation,
                target_type_name, target_id, note, created_at
           FROM research_links WHERE workspace_id = $1
          ORDER BY created_at DESC",
    )
    .bind(workspace_id)
    .fetch_all(executor)
    .await?;
    Ok(links)
}

// ── Leitura agregada institucional ───────────────────────────────────────────
//
// # O que esta secção é, e o que não é
//
// A barra lateral tem `Bibliografia` ao nível da instituição, mas uma fonte
// pertence a um Research Workspace e continua a pertencer. Isto **não** move
// ownership: é uma leitura que soma o que o membro já podia ver, um workspace
// de cada vez.
//
// > Vista global não implica acesso global.
//
// # Porque são duas condições e não uma
//
// O artefacto tem de ser visível **e** o workspace que o contém também.
//
// Só a primeira não chega. Uma fonte `INTERNAL` dentro de um workspace
// `CONFIDENTIAL` de que o membro não é membro passaria o teste da sua própria
// classificação — e o membro ficaria a saber que existe trabalho num sítio a
// que não tem acesso. O título de uma referência diz muito sobre a
// investigação que a cita.
//
// Só a segunda também não chega, e é o F-01: um artefacto mais restrito do que
// o seu workspace tem de continuar escondido a quem alcança o workspace.
//
// As duas juntas dão a interseção certa, e ambas saem do mesmo
// `VisibilityFilter` que o resto do sistema usa. Não há aqui uma segunda
// política escrita em SQL.

/// Colunas de visibilidade da fonte, com alias.
const SOURCE_VISIBILITY: VisibilityColumns =
    VisibilityColumns::aliased("s.unit_id", "s.workspace_id", "s.classification");

/// A condição partilhada pela listagem agregada e pela sua contagem.
///
/// Existe como função por uma razão concreta: a lista e o contador têm de
/// responder à mesma pergunta. Quando cada um tem o seu SQL, divergem — foi
/// exactamente assim que os contadores de Ideias e Projectos da Home passaram a
/// mostrar o mesmo número.
fn accessible_sources_predicate(filter: &VisibilityFilter) -> String {
    let artefacto = to_sql(filter, SOURCE_VISIBILITY);
    let contido = contained_in_visible_workspace(filter, "s");
    format!("{artefacto} AND {contido}")
}

/// Todas as fontes que o principal pode ver, atravessando os workspaces.
///
/// # Errors
///
/// Propaga falhas da base de dados.
pub async fn list_accessible_sources<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    filter: &VisibilityFilter,
    limit: i64,
    offset: i64,
) -> CoreResult<Vec<Source>> {
    let predicate = accessible_sources_predicate(filter);
    let colunas = SOURCE_COLUMNS
        .split(',')
        .map(|c| format!("s.{}", c.trim()))
        .collect::<Vec<_>>()
        .join(", ");

    let sources = sqlx::query_as::<_, Source>(&format!(
        "SELECT {colunas} FROM sources s
          WHERE s.organisation_id = $1 AND {predicate}
          ORDER BY s.year DESC NULLS LAST, s.title
          LIMIT $2 OFFSET $3"
    ))
    .bind(organisation_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(executor)
    .await?;
    Ok(sources)
}

/// Quantas fontes o principal pode ver, atravessando os workspaces.
///
/// Usa o mesmo predicado da listagem, de propósito.
///
/// # Errors
///
/// Propaga falhas da base de dados.
pub async fn count_accessible_sources<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    filter: &VisibilityFilter,
) -> CoreResult<i64> {
    let predicate = accessible_sources_predicate(filter);
    let total = sqlx::query_scalar::<_, i64>(&format!(
        "SELECT COUNT(*) FROM sources s
          WHERE s.organisation_id = $1 AND {predicate}"
    ))
    .bind(organisation_id)
    .fetch_one(executor)
    .await?;
    Ok(total)
}

/// Colunas de visibilidade do documento, com alias.
///
/// A classificação vem de `f`, o ficheiro que governa o artefacto, e não da
/// coluna homónima do documento. É a mesma autoridade que a leitura usa: se a
/// listagem filtrasse por outra, um documento poderia aparecer numa lista e
/// recusar-se a abrir — ou pior, o contrário.
///
/// O `DOCUMENT_SELECT` traz `JOIN files f`, por isso o alias existe em todas as
/// consultas que usam estas colunas.
const DOCUMENT_VISIBILITY: VisibilityColumns =
    VisibilityColumns::aliased("d.unit_id", "d.workspace_id", "f.classification");

/// A condição partilhada pela listagem agregada de documentos e pela contagem.
fn accessible_documents_predicate(filter: &VisibilityFilter) -> String {
    let artefacto = to_sql(filter, DOCUMENT_VISIBILITY);
    let contido = contained_in_visible_workspace(filter, "d");
    format!("{artefacto} AND {contido}")
}

/// Todos os documentos que o principal alcança, atravessando os workspaces.
///
/// # Errors
///
/// Propaga falhas da base de dados.
pub async fn list_accessible_documents<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    filter: &VisibilityFilter,
    limit: i64,
    offset: i64,
) -> CoreResult<Vec<Document>> {
    let predicate = accessible_documents_predicate(filter);
    let documents = sqlx::query_as::<_, Document>(&format!(
        "{DOCUMENT_SELECT}
          WHERE d.organisation_id = $1 AND {predicate}
          ORDER BY d.created_at DESC
          LIMIT $2 OFFSET $3"
    ))
    .bind(organisation_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(executor)
    .await?;
    Ok(documents)
}

/// Quantos documentos o principal alcança, atravessando os workspaces.
///
/// # Errors
///
/// Propaga falhas da base de dados.
pub async fn count_accessible_documents<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    filter: &VisibilityFilter,
) -> CoreResult<i64> {
    let predicate = accessible_documents_predicate(filter);
    let total = sqlx::query_scalar::<_, i64>(&format!(
        // O `JOIN files` existe aqui pela mesma razão que existe na listagem:
        // as duas partilham o predicado, e o predicado filtra pela
        // classificação do ficheiro. Uma contagem sem a junção responderia a
        // uma pergunta diferente das linhas por baixo dela.
        "SELECT COUNT(*) FROM documents d JOIN files f ON f.id = d.file_id
          WHERE d.organisation_id = $1 AND {predicate}"
    ))
    .bind(organisation_id)
    .fetch_one(executor)
    .await?;
    Ok(total)
}
