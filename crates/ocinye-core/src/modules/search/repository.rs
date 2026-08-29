//! Search persistence.

use ocinye_contracts::Classification;
use ocinye_domain::policy::VisibilityFilter;
use sqlx::PgExecutor;
use uuid::Uuid;

use super::model::SearchHit;
use crate::error::CoreResult;
use crate::visibility::{to_sql, VisibilityColumns};

/// Text search configuration.
///
/// `simple` avoids single-language stemming: the corpus is bilingual
/// (Portuguese content, English terminology) and a Portuguese stemmer would
/// degrade the English half, and vice versa (ADR-0202).
pub const TS_CONFIG: &str = "simple";

/// Longest excerpt retained. The index is a finding aid, not a second corpus.
pub const MAX_EXCERPT: usize = 400;

/// Insert or update the index row for an entity.
///
/// # Errors
///
/// Returns an error when the upsert fails.
#[expect(
    clippy::too_many_arguments,
    reason = "um parâmetro por coluna: a alternativa é uma struct que só existe para atravessar esta chamada"
)]
pub async fn upsert<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    unit_id: Option<Uuid>,
    workspace_id: Option<Uuid>,
    entity_type: &str,
    entity_id: Uuid,
    title: &str,
    text: &str,
    classification: Classification,
) -> CoreResult<()> {
    let excerpt: String = text.chars().take(MAX_EXCERPT).collect();
    let document = format!("{title}\n{text}");

    sqlx::query(
        "INSERT INTO search_documents
             (organisation_id, unit_id, workspace_id, entity_type, entity_id,
              title, excerpt, classification, search_vector, indexed_at)
         VALUES ($1, $2, $3, $4, $5, $6, NULLIF($7, ''), $8, to_tsvector($9::regconfig, $10), now())
         ON CONFLICT (entity_type, entity_id) DO UPDATE
            SET organisation_id = EXCLUDED.organisation_id,
                unit_id = EXCLUDED.unit_id,
                workspace_id = EXCLUDED.workspace_id,
                title = EXCLUDED.title,
                excerpt = EXCLUDED.excerpt,
                classification = EXCLUDED.classification,
                search_vector = EXCLUDED.search_vector,
                indexed_at = now(),
                updated_at = now()",
    )
    .bind(organisation_id)
    .bind(unit_id)
    .bind(workspace_id)
    .bind(entity_type)
    .bind(entity_id)
    .bind(title)
    .bind(excerpt)
    .bind(classification.as_str())
    .bind(TS_CONFIG)
    .bind(document)
    .execute(executor)
    .await?;
    Ok(())
}

/// Remove an entity from the index.
///
/// # Errors
///
/// Returns an error when the delete fails.
pub async fn delete<'e>(
    executor: impl PgExecutor<'e>,
    entity_type: &str,
    entity_id: Uuid,
) -> CoreResult<()> {
    sqlx::query("DELETE FROM search_documents WHERE entity_type = $1 AND entity_id = $2")
        .bind(entity_type)
        .bind(entity_id)
        .execute(executor)
        .await?;
    Ok(())
}

/// What is being searched for.
#[derive(Debug, Clone, Copy)]
pub struct SearchTerms<'a> {
    /// The caller's query text.
    pub query: &'a str,
    /// Restrict to these kinds of artefact.
    pub entity_types: Option<&'a [String]>,
    /// Restrict to one research workspace.
    pub workspace_id: Option<Uuid>,
}

/// Run a permission-aware lexical search.
///
/// # Errors
///
/// Returns an error when the query fails.
/// A classificação **actual**, e não a que ficou no índice.
///
/// # Porque o índice não pode ser autoridade
///
/// `search_documents.classification` é uma cópia do momento em que o recurso
/// foi indexado. Se o ambiente for reclassificado a seguir — de `INTERNAL`
/// para `RESTRICTED` —, a leitura e a descarga passam a recusar de imediato,
/// porque compõem contra o estado actual. A pesquisa continuaria a revelar o
/// recurso até alguém reindexar.
///
/// Sincronizar duas verdades por reindexação seria frágil por natureza: a
/// autoridade muda por reclassificação, por filiação, por papel, por unidade,
/// e nenhuma dessas mudanças toca no artefacto indexado.
///
/// > **A pesquisa usa o índice para descobrir candidatos; a visibilidade
/// > decide-se contra o estado autoritativo actual. Um índice nunca é uma
/// > autoridade de autorização.**
///
/// A expressão compõe a do índice com a **do ambiente lido agora**, pela mais
/// restritiva das duas — a mesma composição que a leitura faz. Um recurso sem
/// ambiente mantém a sua.
const CLASSIFICACAO_EFECTIVA: &str = "CASE
    WHEN sd.classification = 'RESTRICTED' OR w.classification = 'RESTRICTED' THEN 'RESTRICTED'
    WHEN sd.classification = 'CONFIDENTIAL' OR w.classification = 'CONFIDENTIAL' THEN 'CONFIDENTIAL'
    WHEN sd.classification = 'INTERNAL' OR w.classification = 'INTERNAL' THEN 'INTERNAL'
    ELSE 'PUBLIC'
END";

/// As colunas de visibilidade da pesquisa: âmbito do índice, classificação viva.
fn colunas_de_visibilidade() -> VisibilityColumns {
    VisibilityColumns::aliased("sd.unit_id", "sd.workspace_id", CLASSIFICACAO_EFECTIVA)
}

pub async fn search<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    visibility: &VisibilityFilter,
    terms: SearchTerms<'_>,
    limit: i64,
    offset: i64,
) -> CoreResult<Vec<SearchHit>> {
    let predicate = to_sql(visibility, colunas_de_visibilidade());

    let hits = sqlx::query_as::<_, SearchHit>(&format!(
        "SELECT sd.entity_type, sd.entity_id, sd.title, sd.excerpt,
                {CLASSIFICACAO_EFECTIVA} AS classification, sd.workspace_id,
                ts_rank(sd.search_vector, websearch_to_tsquery($2::regconfig, $3)) AS rank
           FROM search_documents sd
           LEFT JOIN research_workspaces w ON w.id = sd.workspace_id
          WHERE sd.organisation_id = $1
            AND sd.search_vector @@ websearch_to_tsquery($2::regconfig, $3)
            AND ($4::text[] IS NULL OR sd.entity_type = ANY($4))
            AND ($5::uuid IS NULL OR sd.workspace_id = $5)
            AND {predicate}
          ORDER BY rank DESC, sd.title
          LIMIT $6 OFFSET $7"
    ))
    .bind(organisation_id)
    .bind(TS_CONFIG)
    .bind(terms.query)
    .bind(terms.entity_types)
    .bind(terms.workspace_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(executor)
    .await?;
    Ok(hits)
}

/// Count matches within the authorised set.
///
/// Uses exactly the same predicate as [`search`], so a total can never reveal
/// rows the caller may not see.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn count<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    visibility: &VisibilityFilter,
    terms: SearchTerms<'_>,
) -> CoreResult<i64> {
    // A contagem partilha o predicado com a listagem, e por isso partilha
    // também a junção que o torna verdadeiro. Um número que conte recursos que
    // as linhas não mostram é, por si só, uma fuga: diz que existe mais do que
    // se vê.
    let predicate = to_sql(visibility, colunas_de_visibilidade());

    let total = sqlx::query_scalar::<_, i64>(&format!(
        "SELECT COUNT(*) FROM search_documents sd
           LEFT JOIN research_workspaces w ON w.id = sd.workspace_id
          WHERE sd.organisation_id = $1
            AND sd.search_vector @@ websearch_to_tsquery($2::regconfig, $3)
            AND ($4::text[] IS NULL OR sd.entity_type = ANY($4))
            AND ($5::uuid IS NULL OR sd.workspace_id = $5)
            AND {predicate}"
    ))
    .bind(organisation_id)
    .bind(TS_CONFIG)
    .bind(terms.query)
    .bind(terms.entity_types)
    .bind(terms.workspace_id)
    .fetch_one(executor)
    .await?;
    Ok(total)
}

/// Number of indexed documents that carry an embedding.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn embedded_count<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
) -> CoreResult<i64> {
    let total = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM search_documents
          WHERE organisation_id = $1 AND embedding IS NOT NULL",
    )
    .bind(organisation_id)
    .fetch_one(executor)
    .await?;
    Ok(total)
}

// ── Pesquisa do corpo ───────────────────────────────────────────────────
//
// > **A pesquisa pode usar um índice para descobrir candidatos, mas a
// > visibilidade decide-se contra o estado autoritativo corrente. Um índice
// > nunca é autoridade de autorização.**
//
// Aqui isso é literal: `file_chunks` não guarda classificação nenhuma. A
// composição é feita na consulta, contra `files` e `research_workspaces` como
// estão **agora** — pelo que restringir um ambiente esconde imediatamente o
// corpo dos seus ficheiros, sem reindexar coisa nenhuma.

/// A classificação efectiva de um ficheiro, composta na consulta.
const CLASSIFICACAO_DO_FICHEIRO: &str = "CASE
    WHEN f.classification = 'RESTRICTED' OR w.classification = 'RESTRICTED' THEN 'RESTRICTED'
    WHEN f.classification = 'CONFIDENTIAL' OR w.classification = 'CONFIDENTIAL' THEN 'CONFIDENTIAL'
    WHEN f.classification = 'INTERNAL' OR w.classification = 'INTERNAL' THEN 'INTERNAL'
    ELSE 'PUBLIC'
END";

fn colunas_do_corpo() -> VisibilityColumns {
    VisibilityColumns::aliased("f.unit_id", "f.workspace_id", CLASSIFICACAO_DO_FICHEIRO)
}

/// Pesquisa o corpo dos ficheiros.
///
/// # A versão corrente, e não todas
///
/// Se a v1 e a v2 contêm a mesma frase, dois resultados aparentemente iguais não
/// ajudam ninguém. A pesquisa institucional normal olha para a versão corrente.
/// A v1 continua a existir, continua indexada, e alcança-se pelo caminho exacto
/// — o que se recusa é apresentá-la como se fosse duas coisas.
///
/// # Errors
///
/// Devolve erro quando a consulta falha.
pub async fn search_bodies<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    visibility: &VisibilityFilter,
    terms: SearchTerms<'_>,
    limit: i64,
    offset: i64,
) -> CoreResult<Vec<super::model::BodyHit>> {
    let predicate = to_sql(visibility, colunas_do_corpo());

    let hits = sqlx::query_as::<_, super::model::BodyHit>(&format!(
        "SELECT f.id AS file_id,
                v.id AS file_version_id,
                v.sequence,
                f.name,
                ts_headline($2::regconfig, c.text,
                            websearch_to_tsquery($2::regconfig, $3),
                            'MaxWords=40, MinWords=15, MaxFragments=1') AS excerpt,
                c.locator,
                {CLASSIFICACAO_DO_FICHEIRO} AS classification,
                f.workspace_id,
                ts_rank(c.search_vector, websearch_to_tsquery($2::regconfig, $3)) AS rank
           FROM file_chunks c
           JOIN file_extractions e ON e.id = c.extraction_id
           JOIN file_versions v ON v.id = e.file_version_id
           JOIN files f ON f.id = v.file_id
           JOIN research_workspaces w ON w.id = f.workspace_id
          WHERE f.organisation_id = $1
            AND c.search_vector @@ websearch_to_tsquery($2::regconfig, $3)
            AND v.sequence = (
                SELECT max(sequence) FROM file_versions WHERE file_id = f.id
            )
            AND ($4::uuid IS NULL OR f.workspace_id = $4)
            AND {predicate}
          ORDER BY rank DESC, f.name, c.ordinal
          LIMIT $5 OFFSET $6"
    ))
    .bind(organisation_id)
    .bind(TS_CONFIG)
    .bind(terms.query)
    .bind(terms.workspace_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(executor)
    .await?;
    Ok(hits)
}

/// Quantos ficheiros o corpo alcança, dentro do conjunto autorizado.
///
/// Usa exactamente o mesmo predicado que [`search_bodies`], pelo que um total
/// nunca pode revelar o que a lista esconde.
///
/// # Errors
///
/// Devolve erro quando a consulta falha.
pub async fn count_bodies<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    visibility: &VisibilityFilter,
    terms: SearchTerms<'_>,
) -> CoreResult<i64> {
    let predicate = to_sql(visibility, colunas_do_corpo());

    let total: i64 = sqlx::query_scalar(&format!(
        "SELECT count(DISTINCT f.id)
           FROM file_chunks c
           JOIN file_extractions e ON e.id = c.extraction_id
           JOIN file_versions v ON v.id = e.file_version_id
           JOIN files f ON f.id = v.file_id
           JOIN research_workspaces w ON w.id = f.workspace_id
          WHERE f.organisation_id = $1
            AND c.search_vector @@ websearch_to_tsquery($2::regconfig, $3)
            AND v.sequence = (
                SELECT max(sequence) FROM file_versions WHERE file_id = f.id
            )
            AND ($4::uuid IS NULL OR f.workspace_id = $4)
            AND {predicate}"
    ))
    .bind(organisation_id)
    .bind(TS_CONFIG)
    .bind(terms.query)
    .bind(terms.workspace_id)
    .fetch_one(executor)
    .await?;
    Ok(total)
}

// ── Recuperação semântica ───────────────────────────────────────────────
//
// > **Authorization precedes observability.**
//
// O que se segue gera candidatos por proximidade de vectores. A visibilidade
// continua a decidir-se contra `files` e `research_workspaces` como estão
// **agora**, exactamente como na pesquisa lexical: um vector não é autoridade.

/// Pesquisa o corpo por proximidade semântica.
///
/// # A identidade tem de coincidir
///
/// A consulta é embebida por um modelo; os candidatos foram embebidos por
/// outro, talvez. Comparar espaços diferentes dá números que parecem distâncias
/// e não são — e a resposta errada não se distingue da certa a olho.
///
/// Por isso o filtro é a identidade **inteira**, e não a dimensão: mesmo
/// provider, mesmo modelo, mesma revisão, mesmo perfil. E só conjuntos
/// `AVAILABLE`: um conjunto a meio responde mal sem dizer que está a meio.
///
/// # Errors
///
/// Devolve erro quando a consulta falha.
#[allow(clippy::too_many_arguments)]
pub async fn search_semantic<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    visibility: &VisibilityFilter,
    workspace_id: Option<Uuid>,
    vector: &str,
    identidade: &crate::modules::intelligence::embeddings::EmbeddingIdentity,
    limit: i64,
) -> CoreResult<Vec<super::model::BodyHit>> {
    let predicate = to_sql(visibility, colunas_do_corpo());

    let hits = sqlx::query_as::<_, super::model::BodyHit>(&format!(
        "SELECT f.id AS file_id,
                v.id AS file_version_id,
                v.sequence,
                f.name,
                left(c.text, 400) AS excerpt,
                c.locator,
                {CLASSIFICACAO_DO_FICHEIRO} AS classification,
                f.workspace_id,
                (1.0 - (ce.vector <=> $2::text::vector))::real AS rank
           FROM chunk_embeddings ce
           JOIN embedding_sets es ON es.id = ce.embedding_set_id
           JOIN file_chunks c ON c.id = ce.chunk_id
           JOIN file_versions v ON v.id = es.file_version_id
           JOIN files f ON f.id = v.file_id
           JOIN research_workspaces w ON w.id = f.workspace_id
          WHERE f.organisation_id = $1
            AND es.status = 'AVAILABLE'
            AND es.provider = $3 AND es.model = $4
            AND es.revision = $5 AND es.dimensions = $6 AND es.profile = $7
            AND v.sequence = (
                SELECT max(sequence) FROM file_versions WHERE file_id = f.id
            )
            AND ($8::uuid IS NULL OR f.workspace_id = $8)
            AND {predicate}
          ORDER BY ce.vector <=> $2::text::vector
          LIMIT $9"
    ))
    .bind(organisation_id)
    .bind(vector)
    .bind(&identidade.provider)
    .bind(&identidade.model)
    .bind(&identidade.revision)
    .bind(identidade.dimensions)
    .bind(crate::modules::files::embedding::PROFILE)
    .bind(workspace_id)
    .bind(limit)
    .fetch_all(executor)
    .await?;
    Ok(hits)
}

/// Quantos conjuntos de embeddings completos esta organização tem.
///
/// # Errors
///
/// Devolve erro quando a consulta falha.
pub async fn embedding_set_count<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
) -> CoreResult<i64> {
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*)
           FROM embedding_sets es
           JOIN file_versions v ON v.id = es.file_version_id
           JOIN files f ON f.id = v.file_id
          WHERE f.organisation_id = $1 AND es.status = 'AVAILABLE'",
    )
    .bind(organisation_id)
    .fetch_one(executor)
    .await?;
    Ok(total)
}
