//! Ocinye Worker Runtime.
//!
//! Drains the transactional outbox and runs work that has no business blocking
//! a request: propagating domain events, refreshing derived state, keeping the
//! Intelligence Plane's view of availability honest, and delivering the
//! reminders somebody set for themselves.
//!
//! # Why polling, and why that is enough
//!
//! Events are durable in PostgreSQL (ADR-0010), so the only cost of polling is
//! latency, not loss. `FOR UPDATE SKIP LOCKED` lets several workers run
//! concurrently without any coordination beyond the database.

#![forbid(unsafe_code)]

mod handlers;
mod outbox;
mod reminders;

use std::time::Duration;

use anyhow::Context;
use ocinye_core::config::CoreConfig;
use ocinye_core::db;
use ocinye_observability::LogFormat;
use tokio::signal;

/// How often the outbox is polled when it was empty last time.
const IDLE_POLL_INTERVAL: Duration = Duration::from_secs(2);
/// How often derived state is refreshed.
const MAINTENANCE_INTERVAL: Duration = Duration::from_secs(30);
/// Events claimed per pass.
const BATCH_SIZE: i64 = 50;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = CoreConfig::from_env().context("configuration")?;
    ocinye_observability::init(
        "ocinye-worker",
        &config.log_level,
        LogFormat::parse(&config.log_format),
    );

    let pool = db::connect(&config).await.context("database connection")?;
    tracing::info!("Ocinye Worker started");

    let offline_after = i64::try_from(config.compute.node_offline_after.as_secs()).unwrap_or(120);

    let mut maintenance = tokio::time::interval(MAINTENANCE_INTERVAL);
    maintenance.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    // `Skip` e não `Burst`: se o processo esteve parado uma hora, o que interessa
    // é entregar o que está por entregar **agora**, e não correr cento e vinte
    // passagens seguidas a recuperar batidas perdidas.
    let mut lembretes = tokio::time::interval(reminders::POLL_INTERVAL);
    lembretes.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            () = shutdown_signal() => {
                tracing::info!("Ocinye Worker stopping");
                break;
            }
            _ = maintenance.tick() => {
                if let Err(error) = handlers::refresh_derived_state(&pool, offline_after).await {
                    tracing::error!(error = %error, "maintenance pass failed");
                }
            }
            _ = lembretes.tick() => {
                // Uma passagem que falha regista-se e não derruba o worker: a
                // entrega de lembretes não pode levar consigo o escoamento do
                // outbox, que serve o resto da instituição.
                match ocinye_core::modules::calendar::delivery::deliver_due(&pool).await {
                    Ok(0) => {}
                    Ok(count) => tracing::info!(count, "reminders delivered"),
                    Err(error) => tracing::error!(error = %error, "reminder pass failed"),
                }
            }
            drained = outbox::drain(&pool, BATCH_SIZE) => {
                match drained {
                    // An empty pass means idle: back off rather than spin.
                    Ok(0) => tokio::time::sleep(IDLE_POLL_INTERVAL).await,
                    Ok(count) => tracing::debug!(count, "events processed"),
                    Err(error) => {
                        tracing::error!(error = %error, "outbox drain failed");
                        tokio::time::sleep(IDLE_POLL_INTERVAL).await;
                    }
                }
            }
        }
    }

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c().await.expect("install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
}
