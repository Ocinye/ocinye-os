//! Database pool and migrations.

use std::time::Duration;

use sqlx::postgres::{PgPool, PgPoolOptions};

use crate::config::CoreConfig;
use crate::error::CoreResult;

/// Build the connection pool.
///
/// # Errors
///
/// Returns an error when the database cannot be reached.
pub async fn connect(config: &CoreConfig) -> CoreResult<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(config.database_max_connections)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&config.database_url)
        .await?;
    Ok(pool)
}

/// Apply pending migrations.
///
/// Schema changes only ever happen this way. Manual alteration of a production
/// database is forbidden (`CLAUDE.md` §58).
///
/// # Errors
///
/// Returns an error when a migration fails. A failed migration is never
/// swallowed: the service refuses to start rather than run against a schema it
/// does not understand.
pub async fn migrate(pool: &PgPool) -> CoreResult<()> {
    sqlx::migrate!("../../migrations")
        .run(pool)
        .await
        .map_err(|error| crate::error::CoreError::Internal(format!("migration failed: {error}")))?;
    tracing::info!("migrations applied");
    Ok(())
}

/// Result of a database health probe.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DatabaseHealth {
    /// Whether the probe succeeded.
    pub reachable: bool,
    /// Round-trip time in milliseconds, when reachable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u128>,
}

/// Probe the database.
///
/// Performs a real query: a health check never reports healthy on something it
/// did not verify (`CLAUDE.md` §62).
pub async fn health(pool: &PgPool) -> DatabaseHealth {
    let started = std::time::Instant::now();
    match sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(pool)
        .await
    {
        Ok(_) => DatabaseHealth {
            reachable: true,
            latency_ms: Some(started.elapsed().as_millis()),
        },
        Err(error) => {
            tracing::warn!(error = %error, "database health probe failed");
            DatabaseHealth {
                reachable: false,
                latency_ms: None,
            }
        }
    }
}
