//! Ocinye Node Runtime — the node agent.
//!
//! Runs on a compute node, enrolls it with the Ocinye Core, and reports what
//! the node is and how it is doing.
//!
//! # Status: skeleton
//!
//! This agent **enrolls, reports resources and heartbeats**. It does **not**
//! execute jobs: job dispatch is `PLANNED` and no Ocinye compute node exists yet
//! (ADR-0500). It is declared a skeleton here rather than described as a runtime
//! that does more than it does.
//!
//! # Security shape
//!
//! - The agent has its **own machine identity**. It never uses a person's
//!   credentials.
//! - The connection is **outbound only**. The Core never opens a connection to
//!   the node, and the node accepts no inbound application traffic — the future
//!   topology is `VPS → WireGuard → node` (briefing §58).
//! - The agent token is read from a file with restrictive permissions and is
//!   never logged.

#![forbid(unsafe_code)]

mod config;
mod probe;
mod protocol;

use std::time::Duration;

use anyhow::Context;
use ocinye_observability::LogFormat;
use tokio::signal;

use crate::config::AgentConfig;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = AgentConfig::from_env().context("configuration")?;
    ocinye_observability::init(
        "ocinye-node-agent",
        &config.log_level,
        LogFormat::parse("pretty"),
    );

    tracing::info!(
        core = %config.core_url,
        interval_seconds = config.heartbeat_interval.as_secs(),
        "Ocinye Node Agent starting"
    );

    let client = protocol::Client::new(&config)?;

    // The agent credential is obtained once and then reused. Enrollment tokens
    // are single-use by design, so this must not be repeated on every start.
    let agent_token = match config.load_agent_token()? {
        Some(token) => {
            tracing::info!("using stored agent credential");
            token
        }
        None => {
            let enrollment = config.enrollment_token.clone().context(
                "no agent credential is stored and OCINYE_NODE_ENROLLMENT_TOKEN is not set",
            )?;
            let token = client.enroll(&enrollment).await.context("enrollment")?;
            config
                .store_agent_token(&token)
                .context("storing agent credential")?;
            tracing::info!("enrolled; agent credential stored");
            token
        }
    };

    let mut ticker = tokio::time::interval(config.heartbeat_interval);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            () = shutdown_signal() => {
                tracing::info!("Ocinye Node Agent stopping");
                break;
            }
            _ = ticker.tick() => {
                let report = probe::collect(env!("CARGO_PKG_VERSION"));
                match client.heartbeat(&agent_token, &report).await {
                    Ok(()) => tracing::debug!(
                        cpu_cores = report.resources.cpu_cores,
                        gpus = report.resources.gpus.len(),
                        "heartbeat accepted"
                    ),
                    // A failed heartbeat is not fatal: the node keeps running
                    // and the Core will show it as offline, which is the truth.
                    Err(error) => tracing::warn!(error = %error, "heartbeat failed"),
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

/// Default interval between heartbeats.
pub const DEFAULT_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);
