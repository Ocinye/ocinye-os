//! The node protocol client.
//!
//! Two calls: enroll once, then heartbeat. Both are outbound; the node never
//! listens for the Core.

use anyhow::{bail, Context, Result};
use serde::Deserialize;

use crate::config::AgentConfig;
use crate::probe::Heartbeat;

/// Header carrying the node's own credential.
const NODE_TOKEN_HEADER: &str = "x-ocinye-node-token";

#[derive(Deserialize)]
struct EnrollResponse {
    agent_token: String,
}

/// Client for the node protocol.
pub struct Client {
    http: reqwest::Client,
    base_url: String,
}

impl Client {
    /// Build a client.
    ///
    /// # Errors
    ///
    /// Returns an error when the HTTP client cannot be constructed.
    pub fn new(config: &AgentConfig) -> Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            // The Core is reached at a configured URL; following a redirect
            // would mean sending a machine credential somewhere else.
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .context("building HTTP client")?;

        Ok(Self {
            http,
            base_url: config.core_url.clone(),
        })
    }

    /// Exchange a single-use enrollment token for a long-lived credential.
    ///
    /// # Errors
    ///
    /// Returns an error when the Core refuses the token or is unreachable.
    pub async fn enroll(&self, enrollment_token: &str) -> Result<String> {
        let response = self
            .http
            .post(format!("{}/api/v1/compute/enroll", self.base_url))
            .json(&serde_json::json!({ "enrollment_token": enrollment_token }))
            .send()
            .await
            .context("contacting the Core")?;

        if !response.status().is_success() {
            // The token itself is never included in an error message.
            bail!("enrollment refused with status {}", response.status());
        }

        let body: EnrollResponse = response
            .json()
            .await
            .context("reading enrollment response")?;
        Ok(body.agent_token)
    }

    /// Send a heartbeat.
    ///
    /// # Errors
    ///
    /// Returns an error when the Core refuses the credential or is unreachable.
    pub async fn heartbeat(&self, agent_token: &str, report: &Heartbeat) -> Result<()> {
        let response = self
            .http
            .post(format!("{}/api/v1/compute/heartbeat", self.base_url))
            .header(NODE_TOKEN_HEADER, agent_token)
            .json(report)
            .send()
            .await
            .context("contacting the Core")?;

        if !response.status().is_success() {
            bail!("heartbeat refused with status {}", response.status());
        }
        Ok(())
    }
}
