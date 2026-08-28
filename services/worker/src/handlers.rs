//! What the Worker does with each event.
//!
//! # Idempotency
//!
//! Every handler must be safe to run twice. An event can be redelivered after a
//! crash between the handler succeeding and the row being marked published, so
//! "ran twice" is a normal case, not an exception (briefing §74).

use ocinye_core::modules::intelligence;
use sqlx::{PgPool, Postgres, Transaction};

use crate::outbox::OutboxEvent;

/// Handle one event.
///
/// # Errors
///
/// Returns an error when handling fails; the event is retried with backoff.
pub async fn handle(
    _tx: &mut Transaction<'_, Postgres>,
    event: &OutboxEvent,
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

    // Search indexing happens inside the originating transaction rather than
    // here, so the index can never describe an artefact that was rolled back.
    // This handler is therefore deliberately thin today: it exists so that
    // deferred work — checksums, previews, embeddings, notifications — has a
    // place to go that is already durable and idempotent.
    Ok(())
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
