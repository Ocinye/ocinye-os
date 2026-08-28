//! Transactional outbox.
//!
//! Domain events are written in the same transaction as the state change that
//! produced them, then drained by the Worker. There is no publish-and-hope
//! path: either both commit, or neither does (ADR-0010).

use serde_json::{json, Value};
use uuid::Uuid;

use crate::error::CoreResult;
use crate::Tx;

/// Domain event names.
///
/// Names are part of the contract: versioned, documented, never renamed
/// silently.
pub mod event {
    /// An idea was created.
    pub const IDEA_CREATED: &str = "idea.created";
    /// An idea changed state.
    pub const IDEA_STATE_CHANGED: &str = "idea.state_changed";
    /// A project was created, including by promotion.
    pub const PROJECT_CREATED: &str = "project.created";
    /// A project changed state.
    pub const PROJECT_STATE_CHANGED: &str = "project.state_changed";
    /// A research workspace was created.
    pub const WORKSPACE_CREATED: &str = "research_workspace.created";
    /// Someone was added to a research workspace.
    pub const WORKSPACE_MEMBER_ADDED: &str = "research_workspace.member_added";
    /// A bibliographic source was added.
    pub const SOURCE_ADDED: &str = "source.added";
    /// A note was created.
    pub const NOTE_CREATED: &str = "note.created";
    /// A note was updated.
    pub const NOTE_UPDATED: &str = "note.updated";
    /// A document was uploaded.
    pub const DOCUMENT_UPLOADED: &str = "document.uploaded";
    /// A dataset was catalogued.
    pub const DATASET_CREATED: &str = "dataset.created";
    /// A dataset version was published.
    pub const DATASET_VERSIONED: &str = "dataset.versioned";
    /// A classification changed.
    pub const CLASSIFICATION_CHANGED: &str = "classification.changed";
    /// A task was created.
    pub const TASK_CREATED: &str = "task.created";
    /// Um evento foi marcado.
    pub const CALENDAR_EVENT_CREATED: &str = "calendar_event.created";
    /// A task changed state.
    pub const TASK_STATE_CHANGED: &str = "task.state_changed";
    /// A compute node was enrolled.
    pub const COMPUTE_NODE_ENROLLED: &str = "compute.node.enrolled";
    /// A compute node came online.
    pub const COMPUTE_NODE_ONLINE: &str = "compute.node.online";
    // Não há aqui «nó offline» nem «trabalho de IA concluído».
    //
    // Não são esquecimento: são acontecimentos que nada neste sistema emite
    // hoje. Um nó passa a offline por deixar de reportar, e não existe quem o
    // declare; um trabalho de IA conclui-se num executor que ainda não existe.
    // Um vocabulário de acontecimentos só vale enquanto descreve o que o
    // sistema diz — o resto é roadmap, e roadmap vive nos documentos.
}

/// Keys that must never appear in an event payload.
///
/// Payloads carry identifiers and state transitions, never content.
const FORBIDDEN_PAYLOAD_KEYS: &[&str] = &[
    "content", "body", "abstract", "prompt", "password", "token", "secret",
];

/// Emit a domain event inside the caller's transaction.
///
/// # Errors
///
/// Returns an error when the insert fails, which aborts the state change too.
pub async fn emit(
    tx: &mut Tx<'_>,
    name: &str,
    aggregate_type: &str,
    aggregate_id: Uuid,
    correlation_id: &str,
    payload: Value,
) -> CoreResult<()> {
    let payload = sanitise(payload);

    sqlx::query(
        r"
        INSERT INTO outbox_events (name, aggregate_type, aggregate_id, payload, correlation_id)
        VALUES ($1, $2, $3, $4, $5)
        ",
    )
    .bind(name)
    .bind(aggregate_type)
    .bind(aggregate_id)
    .bind(payload)
    .bind(correlation_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Emit a state-transition event with a uniform payload shape.
///
/// # Errors
///
/// Returns an error when the insert fails.
pub async fn emit_transition(
    tx: &mut Tx<'_>,
    name: &str,
    aggregate_type: &str,
    aggregate_id: Uuid,
    correlation_id: &str,
    from: &str,
    to: &str,
) -> CoreResult<()> {
    emit(
        tx,
        name,
        aggregate_type,
        aggregate_id,
        correlation_id,
        json!({ "from": from, "to": to }),
    )
    .await
}

/// Strip forbidden keys from a payload. A backstop against accidental content.
fn sanitise(payload: Value) -> Value {
    match payload {
        Value::Object(map) => Value::Object(
            map.into_iter()
                .filter(|(key, _)| {
                    !FORBIDDEN_PAYLOAD_KEYS.contains(&key.to_ascii_lowercase().as_str())
                })
                .collect(),
        ),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payloads_never_carry_content() {
        let cleaned = sanitise(json!({
            "workspace_id": "abc",
            "body": "the whole note",
            "abstract": "an abstract",
        }));
        assert_eq!(cleaned["workspace_id"], json!("abc"));
        assert!(cleaned.get("body").is_none());
        assert!(cleaned.get("abstract").is_none());
    }

    #[test]
    fn non_object_payloads_pass_through() {
        assert_eq!(sanitise(json!(42)), json!(42));
    }
}
