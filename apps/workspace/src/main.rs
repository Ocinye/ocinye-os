#![forbid(unsafe_code)]

use ocinye_workspace::config::WorkspaceConfig;
use ocinye_workspace::session::SessionStore;
use ocinye_workspace::{routes, WorkspaceState};

use anyhow::Context;
use ocinye_observability::LogFormat;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = WorkspaceConfig::from_env().context("configuration")?;
    ocinye_observability::init(
        "ocinye-workspace",
        &config.log_level,
        LogFormat::parse(&config.log_format),
    );

    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .context("http client")?;

    let bind_address = config.bind_address.clone();
    let state = WorkspaceState {
        config: std::sync::Arc::new(config),
        sessions: SessionStore::new(),
        http,
    };

    // Expired sessions are swept rather than left to accumulate.
    state.sessions.clone().spawn_sweeper();

    let app = routes::router(state);
    let listener = TcpListener::bind(&bind_address).await.context("bind")?;
    tracing::info!(address = %bind_address, "Ocinye Workspace listening");

    axum::serve(listener, app).await.context("server")?;
    Ok(())
}
