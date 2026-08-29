//! What the Worker does with each event.
//!
//! # Idempotency
//!
//! Every handler must be safe to run twice. An event can be redelivered after a
//! crash between the handler succeeding and the row being marked published, so
//! "ran twice" is a normal case, not an exception (briefing §74).

use ocinye_core::modules::files::{embedding, extraction};
use ocinye_core::modules::intelligence;
use ocinye_core::modules::intelligence::embeddings::EmbeddingProvider;
use ocinye_core::storage::ObjectStore;
use sqlx::{PgPool, Postgres, Transaction};

use crate::outbox::OutboxEvent;

/// Handle one event.
///
/// # Errors
///
/// Returns an error when handling fails; the event is retried with backoff.
pub async fn handle(
    tx: &mut Transaction<'_, Postgres>,
    event: &OutboxEvent,
    store: Option<&ObjectStore>,
    embeddings: Option<&dyn EmbeddingProvider>,
) -> anyhow::Result<()> {
    // Events are logged with their identifiers only. Payloads never carry
    // content, so this line is safe to keep at info level.
    let lag_ms = (chrono::Utc::now() - event.occurred_at).num_milliseconds();

    tracing::info!(
        event = %event.name,
        aggregate = %event.aggregate_type,
        aggregate_id = %event.aggregate_id,
        correlation_id = event.correlation_id.as_deref().unwrap_or("-"),
        // Keys only. Payloads carry identifiers and state transitions, but
        // logging the whole object would make that guarantee depend on every
        // future emitter rather than on this line.
        payload_keys = ?event.payload.as_object().map(|map| map.keys().collect::<Vec<_>>()),
        lag_ms,
        "event"
    );

    // Search indexing of *titles* happens inside the originating transaction
    // rather than here, so the index can never describe an artefact that was
    // rolled back. Reading a body is different work: it needs the bytes, it can
    // take seconds, and it must not hold a request open.
    if event.name == extraction::EVENT_EXTRACT {
        return extrair_conteudo(tx, event, store).await;
    }

    if event.name == embedding::EVENT_EMBED {
        return embeber_conteudo(tx, event, embeddings).await;
    }

    Ok(())
}

/// Produz o conjunto de embeddings de uma versão.
///
/// # Sem provider, o evento espera
///
/// Uma instalação sem embeddings não é uma instalação partida: é a instalação
/// que a Ocinye tem hoje. O evento fica por entregar e o outbox volta a tentar,
/// pelo que o dia em que houver um provider o trabalho está lá — em vez de se
/// ter perdido em silêncio meses antes.
///
/// A pesquisa lexical não depende disto e não cai com isto.
async fn embeber_conteudo(
    tx: &mut Transaction<'_, Postgres>,
    event: &OutboxEvent,
    embeddings: Option<&dyn EmbeddingProvider>,
) -> anyhow::Result<()> {
    let Some(provider) = embeddings else {
        anyhow::bail!("no embedding provider is configured; semantic indexing is deferred");
    };

    match embedding::process(tx, provider, event.aggregate_id).await {
        Ok(Some(estado)) => {
            tracing::info!(
                file_version_id = %event.aggregate_id,
                estado = estado.as_str(),
                // A identidade do modelo entra no registo. O vector nunca.
                provider = %provider.identity().provider,
                model = %provider.identity().model,
                "embedding set settled"
            );
            Ok(())
        }
        // Nada a fazer: já estava feito, a extracção ainda não está pronta, ou
        // a política não deixa este conteúdo sair para este provider.
        Ok(None) => Ok(()),
        Err(erro) => Err(anyhow::anyhow!(erro)),
    }
}

/// Lê o corpo de uma versão e torna-o pesquisável.
///
/// # O que é erro e o que é estado
///
/// Um formato sem extractor, ou um PDF que o leitor não consegue interpretar,
/// **não** são erros deste handler: são estados da extracção, ficam registados,
/// e o evento dá-se por entregue. Voltar a tentar não mudaria nada, e deixar o
/// evento a repetir dez vezes só encheria a fila.
///
/// O armazenamento não responder **é** erro: o outbox volta a tentar com
/// backoff. Marcar `FAILED` aqui afirmaria que o conteúdo não se consegue ler,
/// quando o que aconteceu foi o disco não atender.
///
/// # Idempotência
///
/// `process` reclama a linha com `FOR UPDATE` e devolve `None` quando já não há
/// nada a fazer. Um evento reentregue passa por aqui e sai sem duplicar chunks.
async fn extrair_conteudo(
    tx: &mut Transaction<'_, Postgres>,
    event: &OutboxEvent,
    store: Option<&ObjectStore>,
) -> anyhow::Result<()> {
    let Some(store) = store else {
        // Sem armazenamento configurado não há bytes para ler. Isto é um erro
        // e não um estado: a instalação pode ganhar armazenamento amanhã, e o
        // evento tem de continuar a existir para então ser processado.
        anyhow::bail!("no object store is configured; content cannot be extracted");
    };

    // Os identificadores de correlação vêm do evento que pediu esta leitura, e
    // não de um novo: o trabalho que ela gera a seguir tem de continuar a
    // apontar para o pedido que o começou.
    let ids =
        ocinye_observability::CorrelationIds::from_headers(None, event.correlation_id.as_deref());

    match extraction::process(tx, store, event.aggregate_id, &ids).await {
        Ok(Some(estado)) => {
            tracing::info!(
                file_version_id = %event.aggregate_id,
                estado = estado.as_str(),
                "content extraction settled"
            );
            Ok(())
        }
        Ok(None) => {
            // Já estava lida, ou a versão desapareceu. As duas coisas são
            // razões legítimas para não fazer nada.
            Ok(())
        }
        Err(erro) => Err(anyhow::anyhow!(erro)),
    }
}

/// Refresh state that is derived rather than authoritative.
///
/// # Errors
///
/// Returns an error when the database cannot be reached.
pub async fn refresh_derived_state(
    pool: &PgPool,
    offline_after_seconds: i64,
) -> anyhow::Result<()> {
    // A model is only "available" while the node hosting it is heartbeating.
    // Without this sweep, a node that dies would leave its models advertised as
    // available — exactly the kind of claim the platform must not make
    // (`CLAUDE.md` §69).
    let updated = intelligence::refresh_availability(pool, offline_after_seconds).await?;
    if updated > 0 {
        tracing::info!(
            models = updated,
            "marked models of silent nodes unavailable"
        );
    }
    Ok(())
}
