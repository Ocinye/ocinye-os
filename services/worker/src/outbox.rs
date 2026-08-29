//! Draining the transactional outbox.

use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::handlers;

/// Attempts before an event is left alone for a human to look at.
///
/// The event is never discarded: a stuck event is a signal, and silently
/// dropping it would hide a real problem (`CLAUDE.md` §70).
const MAX_ATTEMPTS: i32 = 10;

/// A claimed outbox event.
#[derive(Debug, Clone, FromRow)]
pub struct OutboxEvent {
    /// Identifier.
    pub id: Uuid,
    /// Event name.
    pub name: String,
    /// Kind of aggregate the event is about.
    pub aggregate_type: String,
    /// Identifier of that aggregate.
    pub aggregate_id: Uuid,
    /// Identifiers and state transitions. Never content.
    pub payload: Value,
    /// Identifier correlating this back to the request that caused it.
    pub correlation_id: Option<String>,
    /// When it happened.
    pub occurred_at: DateTime<Utc>,
    /// How many times delivery has been attempted.
    pub attempts: i32,
}

/// Claim and process a batch of pending events.
///
/// Returns how many were processed. Each event is claimed with
/// `FOR UPDATE SKIP LOCKED`, so several workers can drain concurrently without
/// processing the same event twice.
///
/// # Errors
///
/// Returns an error when the database cannot be reached.
pub async fn drain(
    pool: &PgPool,
    batch_size: i64,
    store: Option<&ocinye_core::storage::ObjectStore>,
    embeddings: Option<&dyn ocinye_core::modules::intelligence::embeddings::EmbeddingProvider>,
) -> anyhow::Result<usize> {
    let mut tx = pool.begin().await?;

    let events = sqlx::query_as::<_, OutboxEvent>(
        "SELECT id, name, aggregate_type, aggregate_id, payload, correlation_id,
                occurred_at, attempts
           FROM outbox_events
          WHERE published_at IS NULL
            AND available_at <= now()
            AND attempts < $2
          ORDER BY occurred_at
          LIMIT $1
          FOR UPDATE SKIP LOCKED",
    )
    .bind(batch_size)
    .bind(MAX_ATTEMPTS)
    .fetch_all(&mut *tx)
    .await?;

    if events.is_empty() {
        tx.rollback().await?;
        return Ok(0);
    }

    let mut processed = 0_usize;

    for event in &events {
        match handlers::handle(&mut tx, event, store, embeddings).await {
            Ok(()) => {
                sqlx::query("UPDATE outbox_events SET published_at = now() WHERE id = $1")
                    .bind(event.id)
                    .execute(&mut *tx)
                    .await?;
                processed += 1;
            }
            Err(error) => {
                // Exponential backoff, capped. The error is recorded on the row
                // so an operator can see why an event is not moving.
                let delay_seconds = f64::from(
                    2_i32.saturating_pow(u32::try_from(event.attempts.max(0)).unwrap_or(0).min(6)),
                );

                tracing::warn!(
                    event = %event.name,
                    attempts = event.attempts + 1,
                    error = %error,
                    "event handling failed; will retry"
                );

                sqlx::query(
                    "UPDATE outbox_events
                        SET attempts = attempts + 1,
                            last_error = $2,
                            available_at = now() + make_interval(secs => $3::double precision)
                      WHERE id = $1",
                )
                .bind(event.id)
                .bind(error.to_string().chars().take(500).collect::<String>())
                .bind(delay_seconds)
                .execute(&mut *tx)
                .await?;
            }
        }
    }

    tx.commit().await?;
    Ok(processed)
}
