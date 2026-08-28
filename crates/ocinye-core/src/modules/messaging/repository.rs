//! As conversas, tal como estão guardadas.
//!
//! # A pergunta que este ficheiro faz mais vezes
//!
//! «Esta pessoa participa nesta conversa **agora**?» Está em todas as consultas,
//! e não como precaução: é o que faz uma conversa ser privada. Uma versão sem
//! ela devolveria as mesmas linhas a quem soubesse um identificador, e as duas
//! versões parecem iguais na interface.

use chrono::{DateTime, Utc};
use sqlx::{PgExecutor, Row};
use uuid::Uuid;

use crate::CoreResult;

/// Uma conversa que a pessoa alcança.
#[derive(Debug, Clone)]
pub struct Conversation {
    /// Identificador.
    pub id: Uuid,
    /// `direct` ou `group`.
    pub kind: String,
    /// O nome, para um grupo.
    pub name: Option<String>,
    /// O papel que a pessoa tem **nesta conversa**, e em mais lado nenhum.
    pub role: String,
    /// Até onde leu.
    pub last_read_at: Option<DateTime<Utc>>,
    /// Quando a conversa foi tocada pela última vez.
    pub updated_at: DateTime<Utc>,
}

/// Uma mensagem, tal como está guardada.
#[derive(Debug, Clone)]
pub struct Message {
    /// Identificador.
    pub id: Uuid,
    /// Onde.
    pub conversation_id: Uuid,
    /// Quem escreveu.
    pub author_id: Uuid,
    /// O que escreveu. Texto, e só texto.
    pub body: String,
    /// A mensagem a que responde.
    pub reply_to_id: Option<Uuid>,
    /// Quando foi alterada, se foi.
    pub edited_at: Option<DateTime<Utc>>,
    /// Quando foi retirada, se foi.
    pub deleted_at: Option<DateTime<Utc>>,
    /// Quando foi escrita.
    pub created_at: DateTime<Utc>,
}

/// A chave que identifica uma conversa directa entre duas pessoas.
///
/// # Porque ordenada
///
/// Para que a conversa de A com B e a de B com A sejam a mesma. Sem uma ordem
/// canónica, cada lado abriria a sua, e o histórico partia-se em dois pedaços
/// que nunca mais se juntam.
#[must_use]
pub fn direct_key(a: Uuid, b: Uuid) -> String {
    let (primeiro, segundo) = if a <= b { (a, b) } else { (b, a) };
    format!("{primeiro}:{segundo}")
}

/// Se esta pessoa participa nesta conversa agora.
///
/// # Errors
///
/// Devolve erro quando a leitura falha.
pub async fn participates<'e>(
    executor: impl PgExecutor<'e>,
    conversation_id: Uuid,
    person_id: Uuid,
) -> CoreResult<bool> {
    let quantos: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM conversation_participants
          WHERE conversation_id = $1 AND person_id = $2 AND left_at IS NULL",
    )
    .bind(conversation_id)
    .bind(person_id)
    .fetch_one(executor)
    .await?;
    Ok(quantos > 0)
}

/// O papel desta pessoa nesta conversa, se participar.
///
/// # Errors
///
/// Devolve erro quando a leitura falha.
pub async fn role_of<'e>(
    executor: impl PgExecutor<'e>,
    conversation_id: Uuid,
    person_id: Uuid,
) -> CoreResult<Option<String>> {
    Ok(sqlx::query_scalar(
        "SELECT role FROM conversation_participants
          WHERE conversation_id = $1 AND person_id = $2 AND left_at IS NULL",
    )
    .bind(conversation_id)
    .bind(person_id)
    .fetch_optional(executor)
    .await?)
}

/// Quem participa numa conversa agora.
///
/// # Errors
///
/// Devolve erro quando a leitura falha.
pub async fn participants<'e>(
    executor: impl PgExecutor<'e>,
    conversation_id: Uuid,
) -> CoreResult<Vec<Uuid>> {
    Ok(sqlx::query_scalar(
        "SELECT person_id FROM conversation_participants
          WHERE conversation_id = $1 AND left_at IS NULL
          ORDER BY joined_at",
    )
    .bind(conversation_id)
    .fetch_all(executor)
    .await?)
}

/// A conversa, se a pessoa a alcançar.
///
/// # Errors
///
/// Devolve erro quando a leitura falha.
pub async fn reachable<'e>(
    executor: impl PgExecutor<'e>,
    conversation_id: Uuid,
    person_id: Uuid,
) -> CoreResult<Option<Conversation>> {
    let linha = sqlx::query(
        "SELECT c.id, c.kind, c.name, c.updated_at, p.role, p.last_read_at
           FROM conversations c
           JOIN conversation_participants p ON p.conversation_id = c.id
          WHERE c.id = $1 AND p.person_id = $2 AND p.left_at IS NULL",
    )
    .bind(conversation_id)
    .bind(person_id)
    .fetch_optional(executor)
    .await?;

    linha
        .map(|row| {
            Ok(Conversation {
                id: row.try_get("id")?,
                kind: row.try_get("kind")?,
                name: row.try_get("name")?,
                role: row.try_get("role")?,
                last_read_at: row.try_get("last_read_at")?,
                updated_at: row.try_get("updated_at")?,
            })
        })
        .transpose()
}

/// Uma conversa da lista, com o que a lista precisa de mostrar.
#[derive(Debug, Clone)]
pub struct Listed {
    /// A conversa.
    pub conversation: Conversation,
    /// O texto da última mensagem, truncado para a lista.
    pub last_body: Option<String>,
    /// Quando foi.
    pub last_at: Option<DateTime<Utc>>,
    /// Quem a escreveu.
    pub last_author_id: Option<Uuid>,
    /// Quantas ainda não foram lidas.
    pub unread: i64,
    /// Quantas dessas mencionam a pessoa.
    pub unread_mentions: i64,
    /// A outra pessoa, numa conversa directa.
    pub other_id: Option<Uuid>,
}

/// As conversas desta pessoa.
///
/// # Porque é uma consulta só
///
/// Porque a alternativa — uma consulta por conversa para a última mensagem, e
/// outra para a contagem — é o `N+1` que torna uma lista de trinta conversas em
/// sessenta e uma idas à base.
///
/// # Errors
///
/// Devolve erro quando a leitura falha.
pub async fn conversations<'e>(
    executor: impl PgExecutor<'e>,
    person_id: Uuid,
) -> CoreResult<Vec<Listed>> {
    let linhas = sqlx::query(
        "SELECT c.id, c.kind, c.name, c.updated_at, p.role, p.last_read_at,
                u.body        AS last_body,
                u.created_at  AS last_at,
                u.author_id   AS last_author_id,
                COALESCE(n.unread, 0)   AS unread,
                COALESCE(n.mencoes, 0)  AS unread_mentions,
                o.person_id   AS other_id
           FROM conversations c
           JOIN conversation_participants p
                  ON p.conversation_id = c.id
                 AND p.person_id = $1
                 AND p.left_at IS NULL

           -- A última mensagem de cada conversa, e não todas elas.
           LEFT JOIN LATERAL (
                SELECT m.body, m.created_at, m.author_id
                  FROM messages m
                 WHERE m.conversation_id = c.id AND m.deleted_at IS NULL
                 ORDER BY m.created_at DESC, m.id DESC
                 LIMIT 1
           ) u ON TRUE

           -- Por ler, e por ler com menção, na mesma passagem.
           LEFT JOIN LATERAL (
                SELECT count(*) AS unread,
                       count(*) FILTER (
                           WHERE EXISTS (
                               SELECT 1 FROM message_mentions x
                                WHERE x.message_id = m.id AND x.person_id = $1
                           )
                       ) AS mencoes
                  FROM messages m
                 WHERE m.conversation_id = c.id
                   AND m.deleted_at IS NULL
                   AND m.author_id <> $1
                   AND (p.last_read_at IS NULL OR m.created_at > p.last_read_at)
           ) n ON TRUE

           -- Numa directa, quem está do outro lado.
           LEFT JOIN LATERAL (
                SELECT q.person_id
                  FROM conversation_participants q
                 WHERE q.conversation_id = c.id
                   AND q.person_id <> $1
                   AND q.left_at IS NULL
                 LIMIT 1
           ) o ON c.kind = 'direct'

          ORDER BY COALESCE(u.created_at, c.updated_at) DESC",
    )
    .bind(person_id)
    .fetch_all(executor)
    .await?;

    linhas
        .into_iter()
        .map(|row| {
            Ok(Listed {
                conversation: Conversation {
                    id: row.try_get("id")?,
                    kind: row.try_get("kind")?,
                    name: row.try_get("name")?,
                    role: row.try_get("role")?,
                    last_read_at: row.try_get("last_read_at")?,
                    updated_at: row.try_get("updated_at")?,
                },
                last_body: row.try_get("last_body")?,
                last_at: row.try_get("last_at")?,
                last_author_id: row.try_get("last_author_id")?,
                unread: row.try_get("unread")?,
                unread_mentions: row.try_get("unread_mentions")?,
                other_id: row.try_get("other_id")?,
            })
        })
        .collect()
}

/// Uma página de mensagens, da mais recente para trás.
///
/// # Errors
///
/// Devolve erro quando a leitura falha.
pub async fn messages<'e>(
    executor: impl PgExecutor<'e>,
    conversation_id: Uuid,
    antes: Option<DateTime<Utc>>,
    limite: i64,
) -> CoreResult<Vec<Message>> {
    let linhas = sqlx::query(
        "SELECT id, conversation_id, author_id, body, reply_to_id,
                edited_at, deleted_at, created_at
           FROM messages
          WHERE conversation_id = $1
            AND ($2::timestamptz IS NULL OR created_at < $2)
          ORDER BY created_at DESC, id DESC
          LIMIT $3",
    )
    .bind(conversation_id)
    .bind(antes)
    .bind(limite)
    .fetch_all(executor)
    .await?;

    linhas
        .into_iter()
        .map(|row| {
            Ok(Message {
                id: row.try_get("id")?,
                conversation_id: row.try_get("conversation_id")?,
                author_id: row.try_get("author_id")?,
                body: row.try_get("body")?,
                reply_to_id: row.try_get("reply_to_id")?,
                edited_at: row.try_get("edited_at")?,
                deleted_at: row.try_get("deleted_at")?,
                created_at: row.try_get("created_at")?,
            })
        })
        .collect()
}

/// Uma mensagem, se estiver nesta conversa.
///
/// # Errors
///
/// Devolve erro quando a leitura falha.
pub async fn message_in<'e>(
    executor: impl PgExecutor<'e>,
    conversation_id: Uuid,
    message_id: Uuid,
) -> CoreResult<Option<Message>> {
    let linha = sqlx::query(
        "SELECT id, conversation_id, author_id, body, reply_to_id,
                edited_at, deleted_at, created_at
           FROM messages WHERE id = $1 AND conversation_id = $2",
    )
    .bind(message_id)
    .bind(conversation_id)
    .fetch_optional(executor)
    .await?;

    linha
        .map(|row| {
            Ok(Message {
                id: row.try_get("id")?,
                conversation_id: row.try_get("conversation_id")?,
                author_id: row.try_get("author_id")?,
                body: row.try_get("body")?,
                reply_to_id: row.try_get("reply_to_id")?,
                edited_at: row.try_get("edited_at")?,
                deleted_at: row.try_get("deleted_at")?,
                created_at: row.try_get("created_at")?,
            })
        })
        .transpose()
}

/// Quem está mencionado em cada uma destas mensagens.
///
/// # Errors
///
/// Devolve erro quando a leitura falha.
pub async fn mentions_of<'e>(
    executor: impl PgExecutor<'e>,
    ids: &[Uuid],
) -> CoreResult<Vec<(Uuid, Uuid)>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let linhas = sqlx::query(
        "SELECT message_id, person_id FROM message_mentions WHERE message_id = ANY($1)",
    )
    .bind(ids)
    .fetch_all(executor)
    .await?;

    linhas
        .into_iter()
        .map(|row| Ok((row.try_get("message_id")?, row.try_get("person_id")?)))
        .collect()
}

/// As reacções destas mensagens, agregadas.
///
/// # Errors
///
/// Devolve erro quando a leitura falha.
pub async fn reactions_of<'e>(
    executor: impl PgExecutor<'e>,
    ids: &[Uuid],
) -> CoreResult<Vec<(Uuid, String, i64, Vec<Uuid>)>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let linhas = sqlx::query(
        "SELECT message_id, emoji, count(*) AS quantas,
                array_agg(person_id) AS quem
           FROM message_reactions
          WHERE message_id = ANY($1)
          GROUP BY message_id, emoji
          ORDER BY min(created_at)",
    )
    .bind(ids)
    .fetch_all(executor)
    .await?;

    linhas
        .into_iter()
        .map(|row| {
            Ok((
                row.try_get("message_id")?,
                row.try_get("emoji")?,
                row.try_get("quantas")?,
                row.try_get("quem")?,
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chave_de_uma_directa_e_a_mesma_dos_dois_lados() {
        // Sem isto, cada lado abria a sua conversa e o histórico partia-se em
        // dois pedaços que nunca mais se juntam.
        let ana = Uuid::from_u128(2);
        let dario = Uuid::from_u128(9);
        assert_eq!(direct_key(ana, dario), direct_key(dario, ana));
    }

    #[test]
    fn duas_pessoas_diferentes_dao_chaves_diferentes() {
        let a = Uuid::from_u128(1);
        let b = Uuid::from_u128(2);
        let c = Uuid::from_u128(3);
        assert_ne!(direct_key(a, b), direct_key(a, c));
    }
}
