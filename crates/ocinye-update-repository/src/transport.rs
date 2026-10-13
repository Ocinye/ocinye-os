//! HTTP(S) for the TUF client.
//!
//! The transport is not trusted for anything: every byte it returns is checked
//! against signed metadata by `tough` (metadata) or by the engine (targets).
//! It only maps a missing file to `FileNotFound`, as TUF's root-update step
//! needs, and bounds the time a request may take.

use futures::StreamExt;
use std::time::Duration;
use tough::{Transport, TransportError, TransportErrorKind, TransportStream};
use url::Url;

/// `http://` and `https://`, and `file://` for an offline bundle.
#[derive(Debug, Clone)]
pub struct HttpTransport {
    client: reqwest::Client,
}

impl HttpTransport {
    /// A transport with bounded connect and read times, no redirects to
    /// another scheme, and no proxy taken from the environment.
    pub fn new() -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(20))
            .read_timeout(Duration::from_secs(60))
            .no_proxy()
            .redirect(reqwest::redirect::Policy::limited(3))
            .https_only(false)
            .user_agent("ocinye-update-engine/1")
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self { client })
    }

    /// The underlying client, for the engine's resumable downloads.
    #[must_use]
    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }
}

#[tough::async_trait]
impl Transport for HttpTransport {
    async fn fetch(&self, url: Url) -> Result<TransportStream, TransportError> {
        match url.scheme() {
            "file" => tough::FilesystemTransport.fetch(url).await,
            "http" | "https" => {
                let response = self.client.get(url.clone()).send().await.map_err(|e| {
                    TransportError::new_with_cause(TransportErrorKind::Other, url.clone(), e)
                })?;
                let status = response.status();
                if status == reqwest::StatusCode::NOT_FOUND
                    || status == reqwest::StatusCode::FORBIDDEN
                {
                    return Err(TransportError::new(TransportErrorKind::FileNotFound, url));
                }
                if !status.is_success() {
                    return Err(TransportError::new(TransportErrorKind::Other, url));
                }
                let stream = response.bytes_stream().map(move |r| {
                    r.map_err(|e| {
                        TransportError::new_with_cause(TransportErrorKind::Other, url.clone(), e)
                    })
                });
                Ok(Box::pin(stream))
            }
            _ => Err(TransportError::new(
                TransportErrorKind::UnsupportedUrlScheme,
                url,
            )),
        }
    }
}
