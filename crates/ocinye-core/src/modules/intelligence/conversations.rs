//! AI conversation persistence and typed-provenance turns (ADR-0309).
//!
//! A conversation is the member's own history — private to its owner, like a
//! note — and its turns are the content: the prompt, and the answer with its
//! typed origin. This is distinct from `ai_jobs`, the operational ledger, which
//! deliberately stores neither prompt nor completion.
//!
//! The historical truth: a system answer written because no inference could run
//! is a `system` turn, and a later model answer is a `model` turn. The first is
//! never retroactively represented as a model response.

use ocinye_contracts::{AiReasonCode, InteractionOrigin, InteractionStatus};
use ocinye_domain::Principal;
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::CoreResult;
use crate::Tx;

/// Largest number of characters kept as a conversation's derived title.
const MAX_TITLE_CHARS: usize = 120;

/// A conversation in a member's list.
#[derive(Debug, Clone, Serialize)]
pub struct ConversationSummary {
    /// The conversation's id.
    pub id: Uuid,
    /// A short label, derived from the first prompt.
    pub title: String,
    /// How many turns it holds.
    pub turns: i64,
    /// When it last changed.
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// One turn, as read back.
#[derive(Debug, Clone, Serialize)]
pub struct TurnView {
    /// Order within the conversation, 1-based.
    pub seq: i32,
    /// `member`, or the response's typed origin (`system`/`model`/`tool`/`agent`).
    pub role: String,
    /// The prompt, or the answer.
    pub content: String,
    /// How a response turn concluded (`completed`/`degraded`); absent on a member turn.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// The machine reason, when a response degraded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason_code: Option<String>,
    /// The model that answered, when one did.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// The provider that served the model, when one did.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
}

/// A whole conversation, owner-scoped.
#[derive(Debug, Clone, Serialize)]
pub struct ConversationView {
    /// The conversation's id.
    pub id: Uuid,
    /// A short label, derived from the first prompt.
    pub title: String,
    /// Its turns, in order.
    pub turns: Vec<TurnView>,
}

/// The response half of an interaction, as it is persisted.
///
/// The typed provenance of the answer: who authored it, how it concluded, and —
/// when a model answered — which model, provider and node.
pub struct ResponseTurn<'a> {
    /// Who authored the response.
    pub origin: InteractionOrigin,
    /// How it concluded.
    pub status: InteractionStatus,
    /// The machine reason, when degraded.
    pub reason_code: Option<AiReasonCode>,
    /// The model that answered, when one did.
    pub model: Option<&'a str>,
    /// The provider that served it, when one did.
    pub provider: Option<&'a str>,
    /// The node that ran it, when one did.
    pub compute_node_id: Option<Uuid>,
    /// The answer's text.
    pub content: &'a str,
}

/// Persist one interaction — the member's prompt and the typed response.
///
/// Creates a conversation when `conversation_id` is `None`, deriving a short
/// title from the prompt; otherwise appends to an existing conversation the
/// member owns (a conversation they do not own is treated as absent, so this
/// never reveals another member's conversation). Both turns are written in the
/// caller's transaction, so an interaction is recorded whole or not at all.
///
/// # Errors
///
/// Returns an error when a query fails.
pub async fn record_interaction(
    tx: &mut Tx<'_>,
    principal: &Principal,
    conversation_id: Option<Uuid>,
    workspace_id: Option<Uuid>,
    prompt: &str,
    response: &ResponseTurn<'_>,
) -> CoreResult<Uuid> {
    // Resolve the conversation: an owned existing one, or a new one.
    let (conversation_id, mut seq) = match conversation_id {
        Some(id) => {
            let owned: Option<i32> = sqlx::query_scalar(
                "SELECT COALESCE(MAX(t.seq), 0)
                   FROM ai_conversations c
                   LEFT JOIN ai_conversation_turns t ON t.conversation_id = c.id
                  WHERE c.id = $1 AND c.owner_id = $2
                  GROUP BY c.id",
            )
            .bind(id)
            .bind(principal.person_id)
            .fetch_optional(&mut **tx)
            .await?;
            match owned {
                Some(max_seq) => (id, max_seq + 1),
                None => new_conversation(tx, principal, workspace_id, prompt).await?,
            }
        }
        None => new_conversation(tx, principal, workspace_id, prompt).await?,
    };

    // The member's turn.
    insert_turn(
        tx,
        conversation_id,
        seq,
        "member",
        prompt,
        None,
        None,
        None,
        None,
        None,
    )
    .await?;
    seq += 1;

    // The response turn, with its typed provenance.
    insert_turn(
        tx,
        conversation_id,
        seq,
        role_of(response.origin),
        response.content,
        Some(status_of(response.status)),
        response.reason_code.map(AiReasonCode::as_str),
        response.model,
        response.provider,
        response.compute_node_id,
    )
    .await?;

    sqlx::query("UPDATE ai_conversations SET updated_at = now() WHERE id = $1")
        .bind(conversation_id)
        .execute(&mut **tx)
        .await?;

    Ok(conversation_id)
}

/// Insert a new conversation, returning its id and the first sequence number.
async fn new_conversation(
    tx: &mut Tx<'_>,
    principal: &Principal,
    workspace_id: Option<Uuid>,
    prompt: &str,
) -> CoreResult<(Uuid, i32)> {
    let title: String = prompt.trim().chars().take(MAX_TITLE_CHARS).collect();
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO ai_conversations (organisation_id, owner_id, workspace_id, title)
         VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(principal.organisation_id)
    .bind(principal.person_id)
    .bind(workspace_id)
    .bind(title)
    .fetch_one(&mut **tx)
    .await?;
    Ok((id, 1))
}

#[expect(
    clippy::too_many_arguments,
    reason = "um parâmetro por coluna do turno; a alternativa é uma struct que só atravessa esta chamada"
)]
async fn insert_turn(
    tx: &mut Tx<'_>,
    conversation_id: Uuid,
    seq: i32,
    role: &str,
    content: &str,
    status: Option<&str>,
    reason_code: Option<&str>,
    model: Option<&str>,
    provider: Option<&str>,
    compute_node_id: Option<Uuid>,
) -> CoreResult<()> {
    sqlx::query(
        "INSERT INTO ai_conversation_turns
             (conversation_id, seq, role, content, status, reason_code,
              model, provider, compute_node_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
    )
    .bind(conversation_id)
    .bind(seq)
    .bind(role)
    .bind(content)
    .bind(status)
    .bind(reason_code)
    .bind(model)
    .bind(provider)
    .bind(compute_node_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// A member's own conversations, most recently updated first.
///
/// Owner-scoped: only the caller's conversations, never anyone else's.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn list_conversations(
    pool: &PgPool,
    principal: &Principal,
) -> CoreResult<Vec<ConversationSummary>> {
    let rows = sqlx::query_as::<_, (Uuid, String, i64, chrono::DateTime<chrono::Utc>)>(
        "SELECT c.id, c.title, COUNT(t.id), c.updated_at
           FROM ai_conversations c
           LEFT JOIN ai_conversation_turns t ON t.conversation_id = c.id
          WHERE c.owner_id = $1
          GROUP BY c.id
          ORDER BY c.updated_at DESC
          LIMIT 100",
    )
    .bind(principal.person_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(id, title, turns, updated_at)| ConversationSummary {
            id,
            title,
            turns,
            updated_at,
        })
        .collect())
}

/// One conversation the member owns, with its turns.
///
/// Returns `None` when the conversation does not exist **or** is not the
/// caller's — the two are deliberately indistinguishable, so this never reveals
/// that another member's conversation exists (ADR-0100).
///
/// # Errors
///
/// Returns an error when a query fails.
pub async fn get_conversation(
    pool: &PgPool,
    principal: &Principal,
    conversation_id: Uuid,
) -> CoreResult<Option<ConversationView>> {
    let head: Option<(Uuid, String)> =
        sqlx::query_as("SELECT id, title FROM ai_conversations WHERE id = $1 AND owner_id = $2")
            .bind(conversation_id)
            .bind(principal.person_id)
            .fetch_optional(pool)
            .await?;

    let Some((id, title)) = head else {
        return Ok(None);
    };

    let turns = sqlx::query_as::<
        _,
        (
            i32,
            String,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
        ),
    >(
        "SELECT seq, role, content, status, reason_code, model, provider
           FROM ai_conversation_turns
          WHERE conversation_id = $1
          ORDER BY seq",
    )
    .bind(id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(
        |(seq, role, content, status, reason_code, model, provider)| TurnView {
            seq,
            role,
            content,
            status,
            reason_code,
            model,
            provider,
        },
    )
    .collect();

    Ok(Some(ConversationView { id, title, turns }))
}

/// The persisted role for a response's origin.
const fn role_of(origin: InteractionOrigin) -> &'static str {
    match origin {
        InteractionOrigin::System => "system",
        InteractionOrigin::Model => "model",
        InteractionOrigin::Tool => "tool",
        InteractionOrigin::Agent => "agent",
    }
}

/// The persisted status string.
///
/// `InteractionStatus` is `#[non_exhaustive]`; only `Completed` maps to
/// `completed`, and any other conclusion is a non-completed one, closer to
/// `degraded` than to a model answer.
fn status_of(status: InteractionStatus) -> &'static str {
    match status {
        InteractionStatus::Completed => "completed",
        _ => "degraded",
    }
}
