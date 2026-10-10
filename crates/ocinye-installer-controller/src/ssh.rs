//! SSH as **transport**, not as product (D011_SECURITY_MATRIX #1–#8).
//!
//! - The host key is checked against the Installer's own store on every
//!   connection; an unknown key stops the connection after the key exchange,
//!   before any credential is sent; a different key is a hard stop.
//! - Authentication by agent, private key (read locally, never uploaded) or
//!   password (memory only). Ed25519 and ECDSA keys.
//! - Every remote command is built here, from a **constant template** and
//!   values that were validated into a closed character set — the random upload
//!   directory and bundle paths. There is no function in this module that takes
//!   a command line from a caller.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use ocinye_installer_contracts::ident::{SshUser, TargetHost};
use ocinye_installer_contracts::manifest::is_safe_bundle_path;
use ocinye_installer_contracts::paths;
use ocinye_installer_contracts::protocol::{parse_incoming, Incoming};
use ocinye_installer_contracts::secret::SecretText;
use rand::rand_core::UnwrapErr;
use rand::rngs::SysRng;
use rand::RngExt as _;
use russh::client::{self, Handle};
use russh::keys::{HashAlg, PrivateKeyWithHashAlg, PublicKeyOrCertificate};
use russh::{ChannelMsg, ChannelWriteHalf};
use tokio::io::AsyncReadExt;
use tokio::sync::mpsc;

use crate::hostkeys::{key_of, HostKeyDecision, KnownHosts};

/// How the operator authenticates.
pub enum Auth {
    /// The SSH agent (`SSH_AUTH_SOCK`).
    Agent,
    /// A private key file, read locally.
    PrivateKey(PathBuf),
    /// A password, in memory.
    Password(SecretText),
}

/// Where to connect.
#[derive(Debug, Clone)]
pub struct Target {
    /// Address.
    pub host: TargetHost,
    /// Port.
    pub port: u16,
    /// User.
    pub user: SshUser,
}

impl Target {
    /// The store key.
    #[must_use]
    pub fn store_key(&self) -> String {
        key_of(&self.host.as_display(), self.port)
    }
}

/// Why a connection did not give a usable session.
#[derive(Debug)]
pub enum SshError {
    /// I24.
    Unreachable(String),
    /// I04 (first contact) or I26 (mismatch).
    HostKey(HostKeyDecision),
    /// I25.
    AuthRefused,
    /// Something else in the transport.
    Transport(String),
}

impl std::fmt::Display for SshError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unreachable(e) => write!(f, "SSH_UNREACHABLE {e}"),
            Self::HostKey(HostKeyDecision::Mismatch { .. }) => f.write_str("HOST_KEY_MISMATCH"),
            Self::HostKey(_) => f.write_str("HOST_KEY_UNKNOWN"),
            Self::AuthRefused => f.write_str("SSH_AUTH_REFUSED"),
            Self::Transport(e) => write!(f, "SSH_TRANSPORT {e}"),
        }
    }
}

impl std::error::Error for SshError {}

struct Client {
    known: Arc<Mutex<KnownHosts>>,
    target: String,
    seen: Arc<Mutex<Option<(String, String)>>>,
}

impl client::Handler for Client {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        key: &PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        // Certificates are not part of D011's trust model: a key, pinned.
        let PublicKeyOrCertificate::PublicKey { key, .. } = key else {
            return Ok(false);
        };
        let fingerprint = key.fingerprint(HashAlg::Sha256).to_string();
        let algorithm = key.algorithm().to_string();
        let decision = self
            .known
            .lock()
            .map(|k| k.decide(&self.target, &algorithm, &fingerprint))
            .unwrap_or(HostKeyDecision::Mismatch {
                known: crate::hostkeys::Pinned {
                    algorithm: String::new(),
                    fingerprint: String::new(),
                    first_seen: String::new(),
                },
                presented_algorithm: algorithm.clone(),
                presented_fingerprint: fingerprint.clone(),
            });
        if let Ok(mut s) = self.seen.lock() {
            *s = Some((algorithm, fingerprint));
        }
        Ok(decision == HostKeyDecision::Known)
    }
}

/// An authenticated session on a server whose key is pinned.
pub struct Session {
    handle: Handle<Client>,
    /// `(algorithm, fingerprint)` of the server's key.
    pub host_key: (String, String),
}

/// What a remote command produced.
#[derive(Debug)]
pub struct RemoteOutput {
    /// Exit status.
    pub code: Option<u32>,
    /// stdout.
    pub stdout: String,
    /// stderr.
    pub stderr: String,
}

impl RemoteOutput {
    /// Exit 0.
    #[must_use]
    pub fn ok(&self) -> bool {
        self.code == Some(0)
    }
}

/// Look at the server's key without authenticating (I03 → I04). Returns the
/// decision and the key; never sends a credential.
///
/// # Errors
///
/// Unreachable or transport errors.
pub async fn probe_host_key(
    target: &Target,
    known: Arc<Mutex<KnownHosts>>,
) -> Result<(HostKeyDecision, (String, String)), SshError> {
    let seen = Arc::new(Mutex::new(None));
    let handler = Client {
        known: Arc::clone(&known),
        target: target.store_key(),
        seen: Arc::clone(&seen),
    };
    let config = Arc::new(client::Config::default());
    let addr = (target.host.as_display(), target.port);
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(20),
        client::connect(config, addr, handler),
    )
    .await;
    let presented = seen.lock().ok().and_then(|s| s.clone());
    match (result, presented) {
        (Err(_), _) => Err(SshError::Unreachable("timeout".into())),
        (Ok(Err(russh::Error::IO(e))), None) => Err(SshError::Unreachable(e.kind().to_string())),
        (Ok(Ok(handle)), Some(k)) => {
            let _ = handle
                .disconnect(russh::Disconnect::ByApplication, "", "pt")
                .await;
            Ok((HostKeyDecision::Known, k))
        }
        (Ok(Err(_)), Some((alg, fp))) => {
            let decision = known
                .lock()
                .map(|k| k.decide(&target.store_key(), &alg, &fp))
                .map_err(|_| SshError::Transport("store".into()))?;
            Ok((decision, (alg, fp)))
        }
        (Ok(Err(e)), None) => Err(SshError::Unreachable(e.to_string())),
        (Ok(Ok(_)), None) => Err(SshError::Transport("no host key".into())),
    }
}

/// Connect and authenticate. Refuses unless the host key is pinned and equal.
///
/// # Errors
///
/// [`SshError`].
pub async fn connect(
    target: &Target,
    auth: &Auth,
    known: Arc<Mutex<KnownHosts>>,
) -> Result<Session, SshError> {
    let seen = Arc::new(Mutex::new(None));
    let handler = Client {
        known: Arc::clone(&known),
        target: target.store_key(),
        seen: Arc::clone(&seen),
    };
    let config = Arc::new(client::Config {
        inactivity_timeout: None,
        keepalive_interval: Some(std::time::Duration::from_secs(15)),
        keepalive_max: 4,
        ..client::Config::default()
    });
    let addr = (target.host.as_display(), target.port);
    let connected = tokio::time::timeout(
        std::time::Duration::from_secs(20),
        client::connect(config, addr, handler),
    )
    .await
    .map_err(|_| SshError::Unreachable("timeout".into()))?;
    let presented = seen.lock().ok().and_then(|s| s.clone());
    let mut handle = match connected {
        Ok(h) => h,
        Err(e) => {
            if let Some((alg, fp)) = presented {
                let decision = known
                    .lock()
                    .map(|k| k.decide(&target.store_key(), &alg, &fp))
                    .map_err(|_| SshError::Transport("store".into()))?;
                if decision != HostKeyDecision::Known {
                    return Err(SshError::HostKey(decision));
                }
                return Err(SshError::Transport(e.to_string()));
            }
            return Err(SshError::Unreachable(e.to_string()));
        }
    };
    let host_key = presented.ok_or_else(|| SshError::Transport("no host key".into()))?;
    let user = target.user.as_str();
    let ok = match auth {
        Auth::Password(p) => handle
            .authenticate_password(user, p.expose())
            .await
            .map_err(|e| SshError::Transport(e.to_string()))?
            .success(),
        Auth::PrivateKey(path) => {
            let key =
                russh::keys::load_secret_key(path, None).map_err(|_| SshError::AuthRefused)?;
            handle
                .authenticate_publickey(user, PrivateKeyWithHashAlg::new(Arc::new(key), None))
                .await
                .map_err(|e| SshError::Transport(e.to_string()))?
                .success()
        }
        Auth::Agent => {
            let mut agent = russh::keys::agent::client::AgentClient::connect_env()
                .await
                .map_err(|_| SshError::AuthRefused)?;
            let ids = agent
                .request_identities()
                .await
                .map_err(|_| SshError::AuthRefused)?;
            let mut ok = false;
            for id in ids {
                let key = id.public_key().into_owned();
                if handle
                    .authenticate_publickey_with(user, key, None, &mut agent)
                    .await
                    .is_ok_and(|r| r.success())
                {
                    ok = true;
                    break;
                }
            }
            ok
        }
    };
    if !ok {
        return Err(SshError::AuthRefused);
    }
    Ok(Session { handle, host_key })
}

// ── Remote command templates ───────────────────────────────────────────────

fn quoted(token: &str) -> String {
    // Only ever called on values from a closed character set (asserted).
    assert!(
        token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'/' | b'.' | b'_' | b'-')),
        "unsafe token"
    );
    format!("'{token}'")
}

/// The session upload directory: `/tmp/ocinye-bootstrap-<16 random [a-z0-9]>`.
#[must_use]
pub fn new_upload_dir() -> String {
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let bytes: [u8; 16] = UnwrapErr(SysRng).random();
    let tail: String = bytes
        .iter()
        .map(|b| ALPHABET[usize::from(*b) % ALPHABET.len()] as char)
        .collect();
    format!("{}{tail}", paths::UPLOAD_PREFIX)
}

fn upload_dir_ok(dir: &str) -> bool {
    dir.strip_prefix(paths::UPLOAD_PREFIX).is_some_and(|t| {
        t.len() == 16
            && t.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
    })
}

/// The read-only probe after authentication (I03 result card).
pub const PROBE: &str = "printf 'uid=%s\\n' \"$(id -u)\"; printf 'groups=%s\\n' \"$(id -nG)\"; \
if sudo -n true 2>/dev/null; then echo sudo=nopasswd; else echo sudo=no; fi; \
printf 'machine=%s\\n' \"$(uname -m)\"; . /etc/os-release; printf 'os=%s %s\\n' \"$ID\" \"$VERSION_ID\"; \
printf 'pretty=%s\\n' \"$PRETTY_NAME\"; printf 'hostname=%s\\n' \"$(hostname)\"";

/// Validate a sudo password (stdin), without running anything else.
pub const SUDO_VALIDATE: &str = "sudo -S -p '' -v";

/// `mkdir` the session upload directory (0700).
#[must_use]
pub fn mkdir_upload(dir: &str) -> String {
    assert!(upload_dir_ok(dir));
    let d = quoted(dir);
    format!("umask 077 && mkdir -m 700 {d} && mkdir -m 700 {d}/bundle {d}/tls")
}

/// Write stdin into a file under the upload directory.
#[must_use]
pub fn write_file(dir: &str, rel: &str) -> String {
    assert!(upload_dir_ok(dir));
    assert!(rel == "ocinye-bootstrap" || rel.starts_with("bundle/") || rel.starts_with("tls/"));
    assert!(is_safe_bundle_path(
        rel.split_once('/').map_or(rel, |(_, r)| r)
    ));
    let path = quoted(&format!("{dir}/{rel}"));
    let parent = quoted(&format!(
        "{dir}/{}",
        Path::new(rel).parent().and_then(Path::to_str).unwrap_or("")
    ));
    format!("umask 077 && mkdir -p {parent} && cat > {path}")
}

/// `chmod 700` the uploaded bootstrap and print its SHA-256.
#[must_use]
pub fn prepare_bootstrap(dir: &str) -> String {
    assert!(upload_dir_ok(dir));
    let p = quoted(&format!("{dir}/ocinye-bootstrap"));
    format!("chmod 700 {p} && sha256sum {p}")
}

/// How the bootstrap is started.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Elevation {
    /// Already root.
    Root,
    /// `sudo -n`.
    SudoNoPassword,
    /// `sudo -S`, password on the first stdin line.
    SudoPassword,
}

/// Start the bootstrap's protocol on this channel.
#[must_use]
pub fn serve(dir: &str, elevation: Elevation) -> String {
    assert!(upload_dir_ok(dir));
    let p = quoted(&format!("{dir}/ocinye-bootstrap"));
    match elevation {
        Elevation::Root => format!("exec {p} serve"),
        Elevation::SudoNoPassword => format!("exec sudo -n {p} serve"),
        Elevation::SudoPassword => format!("exec sudo -S -p '' {p} serve"),
    }
}

/// Remove the session upload directory (preflight-only sessions).
#[must_use]
pub fn remove_upload(dir: &str) -> String {
    assert!(upload_dir_ok(dir));
    format!("rm -rf {}", quoted(dir))
}

impl Session {
    /// Run one templated command, optionally with stdin.
    ///
    /// # Errors
    ///
    /// Transport errors.
    pub async fn exec(
        &self,
        command: &str,
        stdin: Option<&[u8]>,
    ) -> Result<RemoteOutput, SshError> {
        let mut ch = self
            .handle
            .channel_open_session()
            .await
            .map_err(|e| SshError::Transport(e.to_string()))?;
        ch.exec(true, command)
            .await
            .map_err(|e| SshError::Transport(e.to_string()))?;
        if let Some(data) = stdin {
            ch.data(data)
                .await
                .map_err(|e| SshError::Transport(e.to_string()))?;
        }
        ch.eof()
            .await
            .map_err(|e| SshError::Transport(e.to_string()))?;
        let mut out = Vec::new();
        let mut err = Vec::new();
        let mut code = None;
        while let Some(msg) = ch.wait().await {
            match msg {
                ChannelMsg::Data { data } => out.extend_from_slice(&data),
                ChannelMsg::ExtendedData { data, .. } => err.extend_from_slice(&data),
                ChannelMsg::ExitStatus { exit_status } => code = Some(exit_status),
                _ => {}
            }
        }
        Ok(RemoteOutput {
            code,
            stdout: String::from_utf8_lossy(&out).into_owned(),
            stderr: String::from_utf8_lossy(&err).into_owned(),
        })
    }

    /// Stream a local file to a templated remote path, reporting bytes sent.
    ///
    /// # Errors
    ///
    /// I/O and transport errors.
    pub async fn upload(
        &self,
        dir: &str,
        rel: &str,
        local: &Path,
        progress: &mut (dyn FnMut(u64) + Send),
    ) -> Result<(), SshError> {
        let mut ch = self
            .handle
            .channel_open_session()
            .await
            .map_err(|e| SshError::Transport(e.to_string()))?;
        ch.exec(true, write_file(dir, rel))
            .await
            .map_err(|e| SshError::Transport(e.to_string()))?;
        let mut file = tokio::fs::File::open(local)
            .await
            .map_err(|e| SshError::Transport(e.to_string()))?;
        let mut buf = vec![0u8; 256 * 1024];
        loop {
            let n = file
                .read(&mut buf)
                .await
                .map_err(|e| SshError::Transport(e.to_string()))?;
            if n == 0 {
                break;
            }
            ch.data(&buf[..n])
                .await
                .map_err(|e| SshError::Transport(e.to_string()))?;
            progress(n as u64);
        }
        ch.eof()
            .await
            .map_err(|e| SshError::Transport(e.to_string()))?;
        let mut code = None;
        while let Some(msg) = ch.wait().await {
            if let ChannelMsg::ExitStatus { exit_status } = msg {
                code = Some(exit_status);
            }
        }
        if code == Some(0) {
            Ok(())
        } else {
            Err(SshError::Transport(format!("upload {rel}")))
        }
    }

    /// Start the bootstrap and return its protocol connection.
    ///
    /// # Errors
    ///
    /// Transport errors.
    pub async fn open_bootstrap(
        &self,
        dir: &str,
        elevation: Elevation,
        sudo_password: Option<&SecretText>,
    ) -> Result<BootstrapConn, SshError> {
        let ch = self
            .handle
            .channel_open_session()
            .await
            .map_err(|e| SshError::Transport(e.to_string()))?;
        ch.exec(true, serve(dir, elevation))
            .await
            .map_err(|e| SshError::Transport(e.to_string()))?;
        let (mut read, write) = ch.split();
        if elevation == Elevation::SudoPassword {
            let pw = sudo_password.ok_or(SshError::AuthRefused)?;
            let mut line = Vec::with_capacity(pw.expose().len() + 1);
            line.extend_from_slice(pw.expose().as_bytes());
            line.push(b'\n');
            let r = write.data(&line[..]).await;
            zeroize::Zeroize::zeroize(&mut line);
            r.map_err(|e| SshError::Transport(e.to_string()))?;
        }
        let (tx, rx) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            let mut pending = Vec::new();
            while let Some(msg) = read.wait().await {
                match msg {
                    ChannelMsg::Data { data } => {
                        pending.extend_from_slice(&data);
                        while let Some(i) = pending.iter().position(|b| *b == b'\n') {
                            let line: Vec<u8> = pending.drain(..=i).collect();
                            let text =
                                String::from_utf8_lossy(&line[..line.len() - 1]).into_owned();
                            if let Ok(incoming) = parse_incoming(&text) {
                                if tx.send(incoming).is_err() {
                                    return;
                                }
                            }
                        }
                    }
                    ChannelMsg::Eof | ChannelMsg::Close => break,
                    _ => {}
                }
            }
        });
        Ok(BootstrapConn {
            write: Arc::new(write),
            rx,
        })
    }
}

/// The protocol connection to the bootstrap.
pub struct BootstrapConn {
    write: Arc<ChannelWriteHalf<client::Msg>>,
    /// Incoming events and the one secret. Closed when the channel ends.
    pub rx: mpsc::UnboundedReceiver<Incoming>,
}

/// A cloneable sender of commands to the bootstrap (e.g. «Parar num ponto
/// seguro» while the events are being followed).
#[derive(Clone)]
pub struct Control {
    write: Arc<ChannelWriteHalf<client::Msg>>,
}

impl Control {
    /// Send one command.
    ///
    /// # Errors
    ///
    /// Transport errors.
    pub async fn send(
        &self,
        cmd: &ocinye_installer_contracts::protocol::Command,
    ) -> Result<(), SshError> {
        let mut line = serde_json::to_vec(cmd).map_err(|e| SshError::Transport(e.to_string()))?;
        line.push(b'\n');
        self.write
            .data(&line[..])
            .await
            .map_err(|e| SshError::Transport(e.to_string()))
    }
}

impl BootstrapConn {
    /// A cloneable command sender.
    #[must_use]
    pub fn control(&self) -> Control {
        Control {
            write: Arc::clone(&self.write),
        }
    }

    /// Send one command.
    ///
    /// # Errors
    ///
    /// Transport errors.
    pub async fn send(
        &self,
        cmd: &ocinye_installer_contracts::protocol::Command,
    ) -> Result<(), SshError> {
        let mut line = serde_json::to_vec(cmd).map_err(|e| SshError::Transport(e.to_string()))?;
        line.push(b'\n');
        self.write
            .data(&line[..])
            .await
            .map_err(|e| SshError::Transport(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pasta_de_envio_e_aleatoria_e_da_forma_esperada() {
        let a = new_upload_dir();
        let b = new_upload_dir();
        assert_ne!(a, b);
        assert!(upload_dir_ok(&a));
        assert!(!upload_dir_ok("/tmp/ocinye-bootstrap-../../etc"));
    }

    #[test]
    fn os_comandos_remotos_so_levam_valores_de_um_conjunto_fechado() {
        let d = "/tmp/ocinye-bootstrap-abcdefgh01234567";
        assert_eq!(
            write_file(d, "bundle/images/ocinye-core-server.tar"),
            "umask 077 && mkdir -p '/tmp/ocinye-bootstrap-abcdefgh01234567/bundle/images' && cat > '/tmp/ocinye-bootstrap-abcdefgh01234567/bundle/images/ocinye-core-server.tar'"
        );
        assert_eq!(
            serve(d, Elevation::SudoPassword),
            "exec sudo -S -p '' '/tmp/ocinye-bootstrap-abcdefgh01234567/ocinye-bootstrap' serve"
        );
        // Passwordless sudo is non-interactive (`-n`): if the rule does not
        // hold, sudo fails instead of waiting for a password nobody sends.
        assert_eq!(
            serve(d, Elevation::SudoNoPassword),
            "exec sudo -n '/tmp/ocinye-bootstrap-abcdefgh01234567/ocinye-bootstrap' serve"
        );
        assert_eq!(
            serve(d, Elevation::Root),
            "exec '/tmp/ocinye-bootstrap-abcdefgh01234567/ocinye-bootstrap' serve"
        );
        for bad in [
            "bundle/../../etc/passwd",
            "bundle/a b",
            "bundle/$(id)",
            "bundle/a;rm",
            "/etc/shadow",
        ] {
            assert!(
                std::panic::catch_unwind(|| write_file(d, bad)).is_err(),
                "{bad}"
            );
        }
        assert!(std::panic::catch_unwind(|| mkdir_upload("/tmp/x; rm -rf /")).is_err());
        assert!(std::panic::catch_unwind(|| remove_upload("/")).is_err());
    }
}
