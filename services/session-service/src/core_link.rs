//! The Core, over HTTP on the loopback interface.
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
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use ocinye_system_contracts::session::SignedSessionPolicy;
use serde::Deserialize;
use serde_json::json;

use crate::engine::{Core, CoreRefusal, Refresh};

/// Name of the Core's session cookie.
const SESSION_COOKIE: &str = "ocinye_core_session";

/// The most this service reads of one answer. A signed policy is a few
/// hundred bytes; anything near this is not the Core.
const ANSWER_LIMIT: u64 = 64 * 1024;

/// What the Core answers to a sign-in or a satisfied second factor.
#[derive(Deserialize)]
struct Issued {
    session_token: String,
    state: String,
}

/// One answer of the Core: the status, and the body when there is one.
struct Answer {
    status: u16,
    body: Vec<u8>,
}

impl Answer {
    fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }
    fn json<T: for<'de> Deserialize<'de>>(&self) -> Option<T> {
        serde_json::from_slice(&self.body).ok()
    }
}

/// Why a Core address was not accepted: it is not plain HTTP to a loopback
/// address.
#[derive(Debug, PartialEq, Eq)]
pub struct NotLocal;

/// The Core's HTTP API, from this machine.
///
/// The Core of an Ocinye OS machine runs on that machine, and this service
/// reaches it on the loopback interface only: the address is checked when the
/// link is built, and a name that would need resolving is not an address. That
/// is what lets this be plain HTTP/1.1 over a socket, with no TLS stack inside
/// the service that holds session tokens.
pub struct HttpCore {
    address: SocketAddr,
    /// The path prefix of the API, such as `/api/v1`.
    prefix: String,
    machine_id: String,
    /// Where the tokens of the sessions held here are kept.
    path: PathBuf,
    /// Core session id → Core session token.
    tokens: Mutex<BTreeMap<String, String>>,
}

impl HttpCore {
    /// A link to the Core at `base` (`http://127.0.0.1:PORT/PREFIX`), for the
    /// machine `machine_id`, keeping its tokens in `path`.
    ///
    /// # Errors
    ///
    /// [`NotLocal`] when `base` is not plain HTTP to a loopback address.
    pub fn new(base: &str, machine_id: &str, path: PathBuf) -> Result<Self, NotLocal> {
        let rest = base.strip_prefix("http://").ok_or(NotLocal)?;
        let (authority, prefix) = rest.split_once('/').unwrap_or((rest, ""));
        let address: SocketAddr = authority.parse().map_err(|_| NotLocal)?;
        let prefix = prefix.trim_end_matches('/');
        let plain = prefix
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'/' | b'-' | b'_' | b'.'));
        if !address.ip().is_loopback() || !plain {
            return Err(NotLocal);
        }
        let tokens = fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        Ok(Self {
            address,
            prefix: if prefix.is_empty() {
                String::new()
            } else {
                format!("/{prefix}")
            },
            machine_id: machine_id.to_owned(),
            path,
            tokens: Mutex::new(tokens),
        })
    }

    /// One `POST` with a JSON body, one answer, one connection.
    fn post(&self, path: &str, token: Option<&str>, body: &serde_json::Value) -> Option<Answer> {
        // A token is the Core's own hexadecimal; anything else is not sent as
        // a header.
        if token.is_some_and(|t| t.is_empty() || !t.bytes().all(|b| b.is_ascii_alphanumeric())) {
            return None;
        }
        let body = serde_json::to_vec(body).ok()?;
        let mut stream = TcpStream::connect_timeout(&self.address, Duration::from_secs(3)).ok()?;
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .ok()?;
        stream
            .set_write_timeout(Some(Duration::from_secs(10)))
            .ok()?;
        let cookie = token
            .map(|t| format!("Cookie: {SESSION_COOKIE}={t}\r\n"))
            .unwrap_or_default();
        let head = format!(
            "POST {}{path} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\n\
             Content-Length: {}\r\n{cookie}Connection: close\r\n\r\n",
            self.prefix,
            self.address,
            body.len()
        );
        stream.write_all(head.as_bytes()).ok()?;
        stream.write_all(&body).ok()?;
        let mut raw = Vec::new();
        stream.take(ANSWER_LIMIT).read_to_end(&mut raw).ok()?;
        parse(&raw)
    }

    /// The signed policy of the session `token` opens, as the Core sees it now.
    fn policy(&self, token: &str) -> Refresh {
        let Some(answer) = self.post(
            "/auth/machine-session",
            Some(token),
            &json!({ "machine_id": self.machine_id }),
        ) else {
            return Refresh::Unreachable;
        };
        match answer.status {
            200 => answer
                .json::<SignedSessionPolicy>()
                .map_or(Refresh::Unreachable, Refresh::Current),
            // The Core knows the session and says no: it ended there.
            401 | 403 => Refresh::Revoked,
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
        let answer = self
            .post(
                "/auth/login",
                None,
                &json!({ "email": username, "password": password }),
            )
            .ok_or(CoreRefusal::Unreachable)?;
        if answer.status >= 500 {
            return Err(CoreRefusal::Unreachable);
        }
        if !answer.ok() {
            // The Core gives one refusal for every way of not being let in.
            return Err(CoreRefusal::InvalidCredentials);
        }
        let mut issued: Issued = answer.json().ok_or(CoreRefusal::Unreachable)?;

        if issued.state == "mfa_required" {
            let Some(code) = second_factor else {
                self.leave(&issued.session_token);
                return Err(CoreRefusal::SecondFactorRequired);
            };
            let gate = issued.session_token;
            let answer = self
                .post("/auth/mfa/challenge", Some(&gate), &json!({ "code": code }))
                .ok_or(CoreRefusal::Unreachable)?;
            if !answer.ok() {
                self.leave(&gate);
                return Err(if answer.status >= 500 {
                    CoreRefusal::Unreachable
                } else {
                    CoreRefusal::InvalidCredentials
                });
            }
            issued = answer.json().ok_or(CoreRefusal::Unreachable)?;
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

/// Read one HTTP/1.1 answer that was sent whole and then closed: the status
/// line, the headers, and a body delimited by `Content-Length` (or, without
/// one, by the end of the connection). Chunked bodies are not the Core's
/// answers to these requests and are refused rather than guessed at.
fn parse(raw: &[u8]) -> Option<Answer> {
    let split = raw.windows(4).position(|w| w == b"\r\n\r\n")?;
    let head = std::str::from_utf8(&raw[..split]).ok()?;
    let rest = &raw[split + 4..];
    let mut lines = head.split("\r\n");
    let mut status_line = lines.next()?.split(' ');
    if !status_line.next()?.starts_with("HTTP/1.") {
        return None;
    }
    let status: u16 = status_line.next()?.parse().ok()?;
    let mut length = None;
    for line in lines {
        let (name, value) = line.split_once(':')?;
        if name.eq_ignore_ascii_case("transfer-encoding") {
            return None;
        }
        if name.eq_ignore_ascii_case("content-length") {
            length = Some(value.trim().parse::<usize>().ok()?);
        }
    }
    let body = match length {
        Some(n) => rest.get(..n)?.to_vec(),
        None => rest.to_vec(),
    };
    Some(Answer { status, body })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn so_um_endereco_de_loopback_em_http_simples_e_o_core() {
        let at = |base: &str| {
            HttpCore::new(base, "m", "/nonexistent/x".into()).map(|c| (c.address.port(), c.prefix))
        };
        assert_eq!(
            at("http://127.0.0.1:8080/api/v1"),
            Ok((8080, "/api/v1".to_owned()))
        );
        assert_eq!(
            at("http://127.0.0.1:8080/api/v1/"),
            Ok((8080, "/api/v1".to_owned()))
        );
        assert_eq!(at("http://[::1]:8080"), Ok((8080, String::new())));
        for refused in [
            "https://127.0.0.1:8080/api/v1",
            "http://localhost:8080/api/v1",
            "http://192.168.1.10:8080/api/v1",
            "http://core.example:8080/api/v1",
            "http://127.0.0.1/api/v1",
            "http://127.0.0.1:8080/api/v1?x=1",
            "http://127.0.0.1:8080/api v1",
            "127.0.0.1:8080",
        ] {
            assert_eq!(at(refused), Err(NotLocal), "{refused}");
        }
    }

    #[test]
    fn uma_resposta_le_se_pelo_comprimento_dito_e_nada_se_adivinha() {
        let a = parse(
            b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\ncontent-length: 2\r\n\r\n{}trailing",
        )
        .unwrap();
        assert_eq!((a.status, a.body.as_slice()), (200, b"{}".as_slice()));
        let a = parse(b"HTTP/1.0 204 No Content\r\n\r\n").unwrap();
        assert_eq!((a.status, a.body.len()), (204, 0));
        // Shorter than it says, chunked, or not HTTP at all: no answer.
        assert!(parse(b"HTTP/1.1 200 OK\r\nContent-Length: 9\r\n\r\n{}").is_none());
        assert!(parse(
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n2\r\n{}\r\n0\r\n\r\n"
        )
        .is_none());
        assert!(parse(b"SSH-2.0-OpenSSH\r\n\r\n").is_none());
        assert!(parse(b"HTTP/1.1 OK\r\n\r\n").is_none());
    }

    #[test]
    fn um_token_que_nao_e_do_core_nao_vai_num_cabecalho() {
        let core = HttpCore::new("http://127.0.0.1:9/x", "m", "/nonexistent/x".into()).unwrap();
        assert!(core
            .post("/auth/logout", Some("a\r\nX-Injected: 1"), &json!({}))
            .is_none());
        assert!(core.post("/auth/logout", Some(""), &json!({})).is_none());
    }
}
