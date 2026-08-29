//! Conjuntos de embeddings sobre o conteúdo extraído.
//!
//! # A propriedade
//!
//! > **O conteúdo institucional pode ser recuperado lexical e semanticamente
//! > por pessoas e agentes autorizados, através da versão exacta do ficheiro,
//! > sem que embeddings, índices, conteúdo recuperado ou modelos adquiram
//! > autoridade sobre o sistema.**
//!
//! # O que um conjunto não é
//!
//! Não é autoridade: um vector não decide quem o vê. Não é conhecimento: a
//! proximidade entre dois vectores não é uma afirmação institucional. E não é o
//! ficheiro: é uma leitura de uma leitura, feita por um modelo identificado.

use serde_json::json;
use uuid::Uuid;

use crate::error::{CoreError, CoreResult};
use crate::modules::intelligence::embeddings::{
    embed_checked, EmbeddingIdentity, EmbeddingProvider, Locality,
};
use crate::Tx;
use ocinye_contracts::Classification;
use ocinye_observability::CorrelationIds;

/// O nome do evento que põe uma versão na fila de embeddings.
pub const EVENT_EMBED: &str = "file_version.embedding_requested";

/// Como o texto é preparado antes de ser embebido.
///
/// Entra na identidade do conjunto porque dois conjuntos com o mesmo modelo e
/// preparações diferentes não são comparáveis.
pub const PROFILE: &str = "chunks-v1";

/// O estado de um conjunto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Estado {
    /// Na fila.
    Queued,
    /// A ser produzido.
    Processing,
    /// Completo, e elegível para recuperação.
    Available,
    /// Falhou.
    Failed,
}

impl Estado {
    /// Como a base o guarda.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "QUEUED",
            Self::Processing => "PROCESSING",
            Self::Available => "AVAILABLE",
            Self::Failed => "FAILED",
        }
    }

    /// Lê o que a base guardou. Um valor irreconhecível é `Failed`.
    #[must_use]
    pub fn parse(raw: &str) -> Self {
        match raw {
            "QUEUED" => Self::Queued,
            "PROCESSING" => Self::Processing,
            "AVAILABLE" => Self::Available,
            _ => Self::Failed,
        }
    }
}

/// Se este conteúdo pode ser enviado a este provider.
///
/// # A regra, por omissão
///
/// > **Nenhum conteúdo institucional é enviado para um embedding provider
/// > externo sem autorização explícita de deployment.**
///
/// Um provider sob controlo da Ocinye processa segundo a autorização normal,
/// com o mesmo tecto que a inferência local já usa. Um provider externo recebe
/// apenas o que é `PUBLIC` — `INTERNAL` para cima fica dentro, e `CONFIDENTIAL`
/// e `RESTRICTED` falham fechado sem uma decisão institucional que ainda não
/// existe.
///
/// A pergunta é feita **antes** de o texto sair, e não depois: uma verificação
/// a jusante seria uma auditoria de uma coisa que já aconteceu.
#[must_use]
pub fn may_send_to(classification: Classification, locality: Locality) -> bool {
    match locality {
        // O mesmo tecto da inferência local. Um provider da instituição não é
        // um lugar sem regras: é um lugar com as regras da instituição.
        Locality::OcinyeControlled => {
            classification.level() <= Classification::Confidential.level()
        }
        // Fecha em PUBLIC. Alargar isto é uma decisão institucional explícita,
        // e tem de se ver que foi tomada.
        Locality::External => classification.level() <= Classification::Public.level(),
    }
}

/// Põe uma versão na fila de embeddings, dentro da transacção que a pediu.
///
/// Idempotente: pedir duas vezes deixa como está.
///
/// # Errors
///
/// Devolve erro quando a escrita falha.
pub async fn queue(tx: &mut Tx<'_>, file_version_id: Uuid, ids: &CorrelationIds) -> CoreResult<()> {
    crate::outbox::emit(
        tx,
        EVENT_EMBED,
        "file_version",
        file_version_id,
        &ids.correlation_id,
        json!({ "file_version_id": file_version_id }),
    )
    .await?;
    Ok(())
}

/// Produz o conjunto de embeddings de uma versão.
///
/// # Porque o conjunto só fica disponível no fim
///
/// > **A replacement embedding set becomes eligible for retrieval only after
/// > the set is complete.**
///
/// Um conjunto com 37 de 92 pedaços não é «parcialmente útil». É um conjunto que
/// responde mal — e, pior, que responde mal sem dizer que está incompleto. A
/// base recusa `AVAILABLE` com contagens que não fecham; isto nunca as escreve
/// antes de tudo estar lá.
///
/// # O que é erro e o que é estado
///
/// O provider falhar é **erro**: o outbox volta a tentar, e nada mais cai — o
/// ficheiro continua válido, a extracção disponível e a pesquisa lexical
/// intacta. Um conteúdo que a política não deixa sair não é erro nem falha: é
/// uma decisão, e o conjunto não chega a existir.
///
/// # Errors
///
/// Devolve erro quando a base ou o provider falham.
pub async fn process(
    tx: &mut Tx<'_>,
    provider: &dyn EmbeddingProvider,
    file_version_id: Uuid,
) -> CoreResult<Option<Estado>> {
    let identidade = provider.identity();

    // A extracção tem de estar disponível: embeber antes de haver texto seria
    // produzir um conjunto vazio e chamar-lhe completo.
    let extraccao: Option<(Uuid, String)> =
        sqlx::query_as("SELECT id, status FROM file_extractions WHERE file_version_id = $1")
            .bind(file_version_id)
            .fetch_optional(&mut **tx)
            .await?;

    let Some((extraction_id, estado_extraccao)) = extraccao else {
        return Ok(None);
    };
    if estado_extraccao != "AVAILABLE" {
        return Ok(None);
    }

    // A classificação efectiva do ficheiro que contém esta versão, contra o
    // estado corrente — e não a que estava quando o ficheiro foi carregado.
    let classificacao: Option<String> = sqlx::query_scalar(
        "SELECT CASE
                  WHEN f.classification = 'RESTRICTED' OR w.classification = 'RESTRICTED'
                    THEN 'RESTRICTED'
                  WHEN f.classification = 'CONFIDENTIAL' OR w.classification = 'CONFIDENTIAL'
                    THEN 'CONFIDENTIAL'
                  WHEN f.classification = 'INTERNAL' OR w.classification = 'INTERNAL'
                    THEN 'INTERNAL'
                  ELSE 'PUBLIC'
                END
           FROM file_versions v
           JOIN files f ON f.id = v.file_id
           JOIN research_workspaces w ON w.id = f.workspace_id
          WHERE v.id = $1",
    )
    .bind(file_version_id)
    .fetch_optional(&mut **tx)
    .await?;

    let Some(classificacao) = classificacao else {
        return Ok(None);
    };
    let classificacao = Classification::parse(&classificacao).unwrap_or(Classification::Restricted);

    if !may_send_to(classificacao, identidade.locality) {
        // Não é falha. É a política a decidir, e o conjunto não nasce.
        return Ok(None);
    }

    let pedacos: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT id, text FROM file_chunks WHERE extraction_id = $1 ORDER BY ordinal",
    )
    .bind(extraction_id)
    .fetch_all(&mut **tx)
    .await?;

    if pedacos.is_empty() {
        return Ok(None);
    }

    let set_id = upsert_set(
        tx,
        file_version_id,
        extraction_id,
        &identidade,
        i32::try_from(pedacos.len()).unwrap_or(i32::MAX),
    )
    .await?;

    // Já completo: um evento reentregue não refaz trabalho nem duplica nada.
    let ja: Option<String> =
        sqlx::query_scalar("SELECT status FROM embedding_sets WHERE id = $1 FOR UPDATE")
            .bind(set_id)
            .fetch_optional(&mut **tx)
            .await?;
    if ja.as_deref() == Some("AVAILABLE") {
        return Ok(None);
    }

    sqlx::query(
        "UPDATE embedding_sets SET status = 'PROCESSING', updated_at = now() WHERE id = $1",
    )
    .bind(set_id)
    .execute(&mut **tx)
    .await?;

    let lote = provider.max_batch().max(1);
    let mut feitos = 0_i32;

    for grupo in pedacos.chunks(lote) {
        let textos: Vec<String> = grupo.iter().map(|(_, texto)| texto.clone()).collect();
        let vectores = embed_checked(provider, &textos).await.map_err(|erro| {
            CoreError::CapabilityUnavailable(format!("o provider de embeddings falhou: {erro}"))
        })?;

        for ((chunk_id, _), vector) in grupo.iter().zip(vectores.iter()) {
            let como_texto = vector_literal(vector);
            sqlx::query(
                "INSERT INTO chunk_embeddings (embedding_set_id, chunk_id, vector)
                 VALUES ($1, $2, $3::text::vector)
                 ON CONFLICT (embedding_set_id, chunk_id)
                 DO UPDATE SET vector = EXCLUDED.vector",
            )
            .bind(set_id)
            .bind(chunk_id)
            .bind(&como_texto)
            .execute(&mut **tx)
            .await?;
            feitos += 1;
        }
    }

    sqlx::query(
        "UPDATE embedding_sets
            SET status = 'AVAILABLE',
                embedded_chunks = $2,
                completed_at = now(),
                failure_reason = NULL,
                updated_at = now()
          WHERE id = $1",
    )
    .bind(set_id)
    .bind(feitos)
    .execute(&mut **tx)
    .await?;

    Ok(Some(Estado::Available))
}

async fn upsert_set(
    tx: &mut Tx<'_>,
    file_version_id: Uuid,
    extraction_id: Uuid,
    identidade: &EmbeddingIdentity,
    esperados: i32,
) -> CoreResult<Uuid> {
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO embedding_sets
             (file_version_id, extraction_id, provider, model, revision,
              dimensions, locality, profile, status, expected_chunks)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'QUEUED', $9)
         ON CONFLICT (file_version_id, provider, model, revision, profile)
         DO UPDATE SET extraction_id = EXCLUDED.extraction_id,
                       expected_chunks = EXCLUDED.expected_chunks,
                       updated_at = now()
         RETURNING id",
    )
    .bind(file_version_id)
    .bind(extraction_id)
    .bind(&identidade.provider)
    .bind(&identidade.model)
    .bind(&identidade.revision)
    .bind(identidade.dimensions)
    .bind(identidade.locality.as_str())
    .bind(PROFILE)
    .bind(esperados)
    .fetch_one(&mut **tx)
    .await?;
    Ok(id)
}

/// Um vector na sintaxe que o pgvector lê.
#[must_use]
pub fn vector_literal(valores: &[f32]) -> String {
    let mut saida = String::from("[");
    for (indice, valor) in valores.iter().enumerate() {
        if indice > 0 {
            saida.push(',');
        }
        saida.push_str(&format!("{valor}"));
    }
    saida.push(']');
    saida
}

/// O estado do conjunto de uma versão, para esta identidade de modelo.
///
/// # Errors
///
/// Devolve erro quando a consulta falha.
pub async fn status<'e>(
    executor: impl sqlx::PgExecutor<'e>,
    file_version_id: Uuid,
    identidade: &EmbeddingIdentity,
) -> CoreResult<Option<(Estado, i64, i64)>> {
    let linha: Option<(String, i32, i32)> = sqlx::query_as(
        "SELECT status, embedded_chunks, expected_chunks
           FROM embedding_sets
          WHERE file_version_id = $1
            AND provider = $2 AND model = $3 AND revision = $4 AND profile = $5",
    )
    .bind(file_version_id)
    .bind(&identidade.provider)
    .bind(&identidade.model)
    .bind(&identidade.revision)
    .bind(PROFILE)
    .fetch_optional(executor)
    .await?;

    Ok(linha.map(|(estado, feitos, esperados)| {
        (
            Estado::parse(&estado),
            i64::from(feitos),
            i64::from(esperados),
        )
    }))
}
