//! Knowledge persistence.

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

const NOTE_COLUMNS: &str = "id, unit_id, workspace_id, title, body, tags, classification,
                            revision, created_at, updated_at";

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
/// A escolha é uma junção lateral e não um `DISTINCT ON` porque este texto é
/// prefixo de consultas que trazem a sua própria ordenação — por título, por
/// data —, e o `DISTINCT ON` obrigá-las-ia todas a começar por `d.id`. A
/// lateral escolhe uma linha por documento sem tocar na ordem de quem chama.
const DOCUMENT_SELECT: &str = "SELECT d.id, d.unit_id, d.workspace_id, v.storage_object_id,
                                      d.kind, d.title, d.description, d.document_date,
                                      d.classification, o.original_filename, o.content_type,
                                      o.size_bytes, o.checksum_sha256, d.created_at
                                 FROM documents d
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
/// Taken before every edit, so a note's history is preserved.
///
/// # Errors
///
/// Returns an error when the insert fails.
pub async fn snapshot_note<'e>(executor: impl PgExecutor<'e>, note: &Note) -> CoreResult<()> {
    sqlx::query(
        "INSERT INTO note_revisions (note_id, revision, title, body)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (note_id, revision) DO NOTHING",
    )
    .bind(note.id)
    .bind(note.revision)
    .bind(&note.title)
    .bind(&note.body)
    .execute(executor)
    .await?;
    Ok(())
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
    classification: Classification,
    created_by: Uuid,
) -> CoreResult<Uuid> {
    // Só `file_id`. O objecto chega-se pela versão corrente do ficheiro, e a
    // coluna que o guardava directamente desapareceu na migration 0021: já não
    // há duas fontes para poderem discordar.
    let id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO documents
             (organisation_id, unit_id, workspace_id, file_id, kind,
              title, description, classification, created_by_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
         RETURNING id",
    )
    .bind(organisation_id)
    .bind(unit_id)
    .bind(workspace_id)
    .bind(file_id)
    .bind(kind)
    .bind(title)
    .bind(description)
    .bind(classification.as_str())
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
    let predicate = to_sql(
        filter,
        VisibilityColumns::aliased("d.unit_id", "d.workspace_id", "d.classification"),
    );
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
const DOCUMENT_VISIBILITY: VisibilityColumns =
    VisibilityColumns::aliased("d.unit_id", "d.workspace_id", "d.classification");

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
        "SELECT COUNT(*) FROM documents d
          WHERE d.organisation_id = $1 AND {predicate}"
    ))
    .bind(organisation_id)
    .fetch_one(executor)
    .await?;
    Ok(total)
}
