//! The Core, over HTTP.
//!
//! The Session Service is not an identity provider, so it does not invent a
//! way to sign in: it uses the Core's own sign-in and second-factor endpoints,
//! exactly as any other client does, and then asks the Core for the one thing
//! only this service needs — the signed session policy for this machine.
//!
//! The Core session token stays here, in a file only this service can read,
//! so that a restarted service can still ask the Core about the session it
//! was holding. It is never handed to the interface.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use ocinye_system_contracts::session::SignedSessionPolicy;
use reqwest::blocking::{Client, Response};
use reqwest::header::COOKIE;
use reqwest::StatusCode;
use serde::Deserialize;
use serde_json::json;

use crate::engine::{Core, CoreRefusal, Refresh};

/// Name of the Core's session cookie.
const SESSION_COOKIE: &str = "ocinye_core_session";

/// What the Core answers to a sign-in or a satisfied second factor.
#[derive(Deserialize)]
struct Issued {
    session_token: String,
    state: String,
}

/// The Core's HTTP API, from this machine.
pub struct HttpCore {
    client: Client,
    /// The API's base, such as `http://127.0.0.1:8080/api/v1`.
    base: String,
    machine_id: String,
    /// Where the tokens of the sessions held here are kept.
    path: PathBuf,
    /// Core session id → Core session token.
    tokens: Mutex<BTreeMap<String, String>>,
}

impl HttpCore {
    /// A link to the Core at `base`, for the machine `machine_id`, keeping its
    /// tokens in `path`.
    ///
    /// # Errors
    ///
    /// When the HTTP client cannot be built.
    pub fn new(base: &str, machine_id: &str, path: PathBuf) -> Result<Self, reqwest::Error> {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .connect_timeout(Duration::from_secs(3))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        let tokens = fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        Ok(Self {
            client,
            base: base.trim_end_matches('/').to_owned(),
            machine_id: machine_id.to_owned(),
            path,
            tokens: Mutex::new(tokens),
        })
    }

    fn post(&self, path: &str, token: Option<&str>, body: &serde_json::Value) -> Option<Response> {
        let mut request = self.client.post(format!("{}{path}", self.base)).json(body);
        if let Some(token) = token {
            request = request.header(COOKIE, format!("{SESSION_COOKIE}={token}"));
        }
        request.send().ok()
    }

    /// The signed policy of the session `token` opens, as the Core sees it now.
    fn policy(&self, token: &str) -> Refresh {
        let Some(response) = self.post(
            "/auth/machine-session",
            Some(token),
            &json!({ "machine_id": self.machine_id }),
        ) else {
            return Refresh::Unreachable;
        };
        match response.status() {
            StatusCode::OK => response
                .json::<SignedSessionPolicy>()
                .map_or(Refresh::Unreachable, Refresh::Current),
            // The Core knows the session and says no: it ended there.
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Refresh::Revoked,
            // Anything else is the Core not being able to answer.
            _ => Refresh::Unreachable,
        }
    }

    fn leave(&self, token: &str) {
        let _ = self.post("/auth/logout", Some(token), &json!({}));
    }

    fn remember(&self, session_id: &str, token: Option<String>) {
        let Ok(mut tokens) = self.tokens.lock() else {
            return;
        };
        match token {
            Some(token) => tokens.insert(session_id.to_owned(), token),
            None => tokens.remove(session_id),
        };
        let Ok(bytes) = serde_json::to_vec(&*tokens) else {
            return;
        };
        let temporary = self.path.with_extension("tmp");
        let written = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&temporary)
            .and_then(|mut f| f.write_all(&bytes).and_then(|()| f.sync_all()));
        if written.is_ok() {
            let _ = fs::rename(&temporary, &self.path);
        }
    }

    fn token(&self, session_id: &str) -> Option<String> {
        self.tokens.lock().ok()?.get(session_id).cloned()
    }
}

impl Core for HttpCore {
    fn sign_in(
        &self,
        username: &str,
        password: &str,
        second_factor: Option<&str>,
    ) -> Result<SignedSessionPolicy, CoreRefusal> {
        let response = self
            .post(
                "/auth/login",
                None,
                &json!({ "email": username, "password": password }),
            )
            .ok_or(CoreRefusal::Unreachable)?;
        if response.status().is_server_error() {
            return Err(CoreRefusal::Unreachable);
        }
        if !response.status().is_success() {
            // The Core gives one refusal for every way of not being let in.
            return Err(CoreRefusal::InvalidCredentials);
        }
        let mut issued: Issued = response.json().map_err(|_| CoreRefusal::Unreachable)?;

        if issued.state == "mfa_required" {
            let Some(code) = second_factor else {
                self.leave(&issued.session_token);
                return Err(CoreRefusal::SecondFactorRequired);
            };
            let gate = issued.session_token;
            let response = self
                .post("/auth/mfa/challenge", Some(&gate), &json!({ "code": code }))
                .ok_or(CoreRefusal::Unreachable)?;
            if !response.status().is_success() {
                self.leave(&gate);
                return Err(if response.status().is_server_error() {
                    CoreRefusal::Unreachable
                } else {
                    CoreRefusal::InvalidCredentials
                });
            }
            issued = response.json().map_err(|_| CoreRefusal::Unreachable)?;
        }
        if issued.state != "active" {
            self.leave(&issued.session_token);
            return Err(match issued.state.as_str() {
                "password_change_required" => CoreRefusal::PasswordChangeRequired,
                "mfa_required" => CoreRefusal::SecondFactorRequired,
                _ => CoreRefusal::InvalidCredentials,
            });
        }
        match self.policy(&issued.session_token) {
            Refresh::Current(signed) => {
                self.remember(&signed.policy.session_id, Some(issued.session_token));
                Ok(signed)
            }
            Refresh::Revoked | Refresh::Unreachable => {
                self.leave(&issued.session_token);
                Err(CoreRefusal::Unreachable)
            }
        }
    }

    fn refresh(&self, session_id: &str) -> Refresh {
        // Without the token the Core cannot be asked about this session; the
        // last signed policy goes on governing, within its own limits.
        let Some(token) = self.token(session_id) else {
            return Refresh::Unreachable;
        };
        let answer = self.policy(&token);
        if answer == Refresh::Revoked {
            self.remember(session_id, None);
        }
        answer
    }

    fn sign_out(&self, session_id: &str) {
        if let Some(token) = self.token(session_id) {
            self.leave(&token);
            self.remember(session_id, None);
        }
    }
}
