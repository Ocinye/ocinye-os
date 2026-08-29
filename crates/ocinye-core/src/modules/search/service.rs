//! Search application layer.

use ocinye_contracts::{Classification, PageRequest};
use ocinye_domain::policy::VisibilityFilter;
use ocinye_domain::Principal;
use sqlx::PgPool;
use uuid::Uuid;

use super::model::{SearchHit, SemanticAvailability};
use super::repository as repo;
use crate::error::{CoreError, CoreResult};
use crate::Tx;

/// Shortest query accepted. Below this, a search matches too much to be useful
/// and costs more than it returns.
const MIN_QUERY_LENGTH: usize = 2;

/// What to index for an entity.
#[derive(Debug, Clone)]
pub struct IndexRequest {
    /// Organisation.
    pub organisation_id: Uuid,
    /// Owning unit.
    pub unit_id: Option<Uuid>,
    /// Owning workspace.
    pub workspace_id: Option<Uuid>,
    /// Kind of artefact.
    pub entity_type: &'static str,
    /// Identifier of the artefact.
    pub entity_id: Uuid,
    /// Title.
    pub title: String,
    /// Indexable text. Never a full document body without an explicit decision.
    pub text: String,
    /// Classification, carried into the index so queries can filter on it.
    pub classification: Classification,
}

/// Index an entity inside the caller's transaction.
///
/// Indexing shares the transaction of the change that caused it, so the index
/// cannot describe an artefact that was never committed.
///
/// # Errors
///
/// Returns an error when the upsert fails.
pub async fn index_entity(tx: &mut Tx<'_>, request: IndexRequest) -> CoreResult<()> {
    repo::upsert(
        &mut **tx,
        request.organisation_id,
        request.unit_id,
        request.workspace_id,
        request.entity_type,
        request.entity_id,
        &request.title,
        &request.text,
        request.classification,
    )
    .await
}

/// Remove an entity from the index.
///
/// # Errors
///
/// Returns an error when the delete fails.
pub async fn remove_entity(tx: &mut Tx<'_>, entity_type: &str, entity_id: Uuid) -> CoreResult<()> {
    repo::delete(&mut **tx, entity_type, entity_id).await
}

/// Run a permission-aware search.
///
/// # Errors
///
/// Returns [`CoreError::Validation`] when the query is too short.
pub async fn search(
    pool: &PgPool,
    principal: &Principal,
    query: &str,
    entity_types: Option<Vec<String>>,
    workspace_id: Option<Uuid>,
    page: PageRequest,
) -> CoreResult<(Vec<SearchHit>, i64)> {
    let query = query.trim();
    if query.chars().count() < MIN_QUERY_LENGTH {
        return Err(CoreError::Validation(
            "A search needs at least two characters.".to_owned(),
        ));
    }

    let filter = VisibilityFilter::for_principal(principal);
    if filter.is_never_satisfiable() {
        return Ok((Vec::new(), 0));
    }

    let types = entity_types.filter(|types| !types.is_empty());

    let terms = repo::SearchTerms {
        query,
        entity_types: types.as_deref(),
        workspace_id,
    };

    let hits = repo::search(
        pool,
        principal.organisation_id,
        &filter,
        terms,
        page.limit(),
        page.offset(),
    )
    .await?;
    let total = repo::count(pool, principal.organisation_id, &filter, terms).await?;

    Ok((hits, total))
}

/// Pesquisa o **corpo** dos ficheiros institucionais.
///
/// Separada de [`search`] e não misturada com ela, por duas razões.
///
/// A primeira é honestidade: um resultado de corpo diz «esta frase está na
/// página 4 da versão 2 deste ficheiro», e um resultado de título diz «este
/// artefacto chama-se assim». Fundi-los num ranking só faria a interface
/// escolher qual das duas afirmações mostrar.
///
/// A segunda é que isto não precisa de nenhum modelo. A pesquisa do corpo é
/// lexical, funciona sem IA, e continuará a funcionar quando houver embeddings
/// — que serão outra coisa, ao lado desta, e não em vez dela.
///
/// # Errors
///
/// Devolve erro quando a consulta é curta de mais ou quando a base falha.
pub async fn search_bodies(
    pool: &PgPool,
    principal: &Principal,
    query: &str,
    workspace_id: Option<Uuid>,
    page: PageRequest,
) -> CoreResult<(Vec<crate::modules::search::model::BodyHit>, i64)> {
    let query = query.trim();
    if query.chars().count() < MIN_QUERY_LENGTH {
        return Err(CoreError::Validation(
            "A search needs at least two characters.".to_owned(),
        ));
    }

    let filter = VisibilityFilter::for_principal(principal);
    if filter.is_never_satisfiable() {
        return Ok((Vec::new(), 0));
    }

    let terms = repo::SearchTerms {
        query,
        entity_types: None,
        workspace_id,
    };

    let hits = repo::search_bodies(
        pool,
        principal.organisation_id,
        &filter,
        terms,
        page.limit(),
        page.offset(),
    )
    .await?;
    let total = repo::count_bodies(pool, principal.organisation_id, &filter, terms).await?;

    Ok((hits, total))
}

/// Report whether semantic search can be offered.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn semantic_availability(
    pool: &PgPool,
    principal: &Principal,
) -> CoreResult<SemanticAvailability> {
    let embedded = repo::embedded_count(pool, principal.organisation_id).await?;

    Ok(SemanticAvailability {
        available: embedded > 0,
        embedded_documents: embedded,
        message: if embedded > 0 {
            "Semantic search is available.".to_owned()
        } else {
            "Semantic search is unavailable: no embeddings exist, because no Ocinye AI node \
             is enrolled. Lexical search is unaffected."
                .to_owned()
        },
    })
}
