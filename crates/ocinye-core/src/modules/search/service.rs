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

/// Pesquisa híbrida: lexical e semântica, como geradores independentes.
///
/// # Porque são dois geradores e não um
///
/// Porque encontram coisas diferentes. O lexical encontra a frase exacta; o
/// semântico encontra a paráfrase. Um sistema que só tivesse o primeiro perde a
/// pergunta feita por outras palavras; um que só tivesse o segundo perde o
/// termo técnico raro que o modelo nunca viu.
///
/// A fusão é por posição recíproca (RRF): cada lista contribui `1/(k+posição)`,
/// e um documento que aparece em ambas sobe. Não precisa de calibrar scores
/// entre espaços que não são comparáveis — que é exactamente o problema de
/// somar uma distância de cosseno com um `ts_rank`.
///
/// # O que a fusão não pode fazer
///
/// > **Authorization precedes observability.**
///
/// Nenhuma das duas listas contém o que a autoridade recusa: as duas consultas
/// já aplicam o mesmo predicado. A fusão junta candidatos autorizados, e não
/// tem por onde introduzir um que não esteja.
///
/// Sem provider de embeddings, `semantic` é `None` e isto devolve exactamente o
/// que a pesquisa lexical devolveria. **Não é degradação**: é a capacidade
/// determinística inteira, que continua a ser toda a pesquisa que esta
/// instalação sempre teve.
///
/// # Errors
///
/// Devolve erro quando a consulta é curta de mais ou quando a base falha.
pub async fn search_hybrid(
    pool: &PgPool,
    principal: &Principal,
    query: &str,
    workspace_id: Option<Uuid>,
    page: PageRequest,
    semantic: Option<&dyn crate::modules::intelligence::embeddings::EmbeddingProvider>,
) -> CoreResult<(Vec<crate::modules::search::model::BodyHit>, i64)> {
    let (lexicais, total) = search_bodies(pool, principal, query, workspace_id, page).await?;

    let Some(provider) = semantic else {
        return Ok((lexicais, total));
    };

    let filtro = VisibilityFilter::for_principal(principal);
    if filtro.is_never_satisfiable() {
        return Ok((Vec::new(), 0));
    }

    // A consulta é embebida pelo **mesmo** perfil que os candidatos, porque é
    // esse perfil que a consulta seguinte exige. Um provider diferente aqui
    // produziria uma consulta que não encontra nada — ou, pior, que encontra
    // coisas por acidente.
    let vectores = crate::modules::intelligence::embeddings::embed_checked(
        provider,
        &[query.trim().to_owned()],
    )
    .await;

    let Ok(vectores) = vectores else {
        // O provider falhou. A pesquisa lexical não cai com ele.
        return Ok((lexicais, total));
    };
    let Some(vector) = vectores.first() else {
        return Ok((lexicais, total));
    };

    let semanticos = repo::search_semantic(
        pool,
        principal.organisation_id,
        &filtro,
        workspace_id,
        &crate::modules::files::embedding::vector_literal(vector),
        &provider.identity(),
        page.limit(),
    )
    .await?;

    Ok((fundir(lexicais, semanticos, page.limit()), total))
}

/// Só os candidatos semânticos, para quem precise de os observar isolados.
///
/// A pesquisa normal é a híbrida; isto existe para se poder afirmar coisas
/// sobre o gerador semântico sozinho — que um conjunto de outra revisão não é
/// comparado, que um conjunto incompleto não responde — sem que a lista lexical
/// as esconda.
///
/// # Errors
///
/// Devolve erro quando o provider ou a base falham.
pub async fn semantic_candidates(
    pool: &PgPool,
    principal: &Principal,
    query: &str,
    workspace_id: Option<Uuid>,
    limit: i64,
    provider: &dyn crate::modules::intelligence::embeddings::EmbeddingProvider,
) -> CoreResult<Vec<crate::modules::search::model::BodyHit>> {
    let filtro = VisibilityFilter::for_principal(principal);
    if filtro.is_never_satisfiable() {
        return Ok(Vec::new());
    }

    let vectores = crate::modules::intelligence::embeddings::embed_checked(
        provider,
        &[query.trim().to_owned()],
    )
    .await
    .map_err(|erro| CoreError::CapabilityUnavailable(format!("embeddings: {erro}")))?;

    let Some(vector) = vectores.first() else {
        return Ok(Vec::new());
    };

    repo::search_semantic(
        pool,
        principal.organisation_id,
        &filtro,
        workspace_id,
        &crate::modules::files::embedding::vector_literal(vector),
        &provider.identity(),
        limit,
    )
    .await
}

/// Fusão por posição recíproca.
///
/// A constante amortece o peso das primeiras posições: sem ela, o primeiro
/// resultado de uma lista dominaria a outra inteira.
const RRF_K: f32 = 60.0;

fn fundir(
    lexicais: Vec<crate::modules::search::model::BodyHit>,
    semanticos: Vec<crate::modules::search::model::BodyHit>,
    limite: i64,
) -> Vec<crate::modules::search::model::BodyHit> {
    use std::collections::HashMap;

    let mut pontos: HashMap<(Uuid, i32), f32> = HashMap::new();
    let mut por_chave: HashMap<(Uuid, i32), crate::modules::search::model::BodyHit> =
        HashMap::new();

    for (posicao, hit) in lexicais.into_iter().enumerate() {
        #[allow(clippy::cast_precision_loss)]
        let contributo = 1.0 / (RRF_K + posicao as f32 + 1.0);
        let chave = (hit.file_id, hit.sequence);
        *pontos.entry(chave).or_insert(0.0) += contributo;
        por_chave.entry(chave).or_insert(hit);
    }

    for (posicao, hit) in semanticos.into_iter().enumerate() {
        #[allow(clippy::cast_precision_loss)]
        let contributo = 1.0 / (RRF_K + posicao as f32 + 1.0);
        let chave = (hit.file_id, hit.sequence);
        *pontos.entry(chave).or_insert(0.0) += contributo;
        // O excerto lexical vem com os termos realçados e é melhor de ler;
        // só se usa o semântico quando não há outro.
        por_chave.entry(chave).or_insert(hit);
    }

    let mut ordenados: Vec<_> = por_chave.into_iter().collect();
    ordenados.sort_by(|(a, hit_a), (b, hit_b)| {
        let pa = pontos.get(a).copied().unwrap_or(0.0);
        let pb = pontos.get(b).copied().unwrap_or(0.0);
        pb.partial_cmp(&pa)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| hit_a.name.cmp(&hit_b.name))
    });

    ordenados
        .into_iter()
        .take(usize::try_from(limite).unwrap_or(usize::MAX))
        .map(|(chave, mut hit)| {
            hit.rank = pontos.get(&chave).copied().unwrap_or(0.0);
            hit
        })
        .collect()
}

/// Report whether semantic search can be offered.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn semantic_availability(
    pool: &PgPool,
    principal: &Principal,
    provider: Option<&dyn crate::modules::intelligence::embeddings::EmbeddingProvider>,
) -> CoreResult<SemanticAvailability> {
    let embedded = repo::embedded_count(pool, principal.organisation_id).await?;
    let conjuntos = repo::embedding_set_count(pool, principal.organisation_id).await?;
    let total = embedded + conjuntos;

    // Três estados, e não dois.
    //
    // «Indisponível por política ou por falta de infraestrutura» e «erro» são
    // coisas diferentes, e uma interface que as confunda ensina que a pesquisa
    // está partida quando não está. A pesquisa lexical continua inteira nos
    // dois casos, e a mensagem di-lo.
    let (available, message) = match (provider, total) {
        (None, _) => (
            false,
            "A pesquisa semântica não está disponível nesta instalação: não há \
             nenhum provider de embeddings configurado. A pesquisa textual não é \
             afectada."
                .to_owned(),
        ),
        (Some(_), 0) => (
            false,
            "A pesquisa semântica ainda não tem nada indexado: há um provider \
             configurado, mas nenhum conteúdo foi ainda processado. A pesquisa \
             textual não é afectada."
                .to_owned(),
        ),
        (Some(_), _) => (true, "A pesquisa semântica está disponível.".to_owned()),
    };

    Ok(SemanticAvailability {
        available,
        embedded_documents: total,
        message,
    })
}
