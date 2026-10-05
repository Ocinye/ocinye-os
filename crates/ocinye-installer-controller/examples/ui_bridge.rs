//! Development bridge: the Installer's real renderer (`apps/installer/ui`) and
//! the real controller dispatcher, over HTTP on 127.0.0.1 — so the whole
//! window can be driven from a browser against a disposable test server. Not
//! a product: no native dialogs (the paths come from the environment), binds
//! loopback only, and is not part of any bundle.
//!
//! ```text
//! OCINYE_BRIDGE_UI=apps/installer/ui OCINYE_BRIDGE_STATE=… \
//! OCINYE_BRIDGE_RELEASE=… OCINYE_BRIDGE_KEY=… [OCINYE_BRIDGE_TLS_CERT=… …] \
//! [OCINYE_BRIDGE_RESOLVE=host=ip,host=ip] cargo run --example ui_bridge -- 8766
//! ```
//!
//! Two test-only conveniences keep proof secrets out of whatever drives the
//! browser: `OCINYE_BRIDGE_SUDO_FILE` — a `sudo` call whose password is the
//! placeholder `@file` gets the file's contents instead; and
//! `OCINYE_BRIDGE_CREDENTIAL_FILE` — when the operator reveals the one-time
//! credential, the bridge also keeps it there (0600, never overwritten).

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use ocinye_installer_controller::app::{App, Native};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

struct Env;

fn env_path(k: &str) -> Option<PathBuf> {
    std::env::var(k).ok().map(PathBuf::from)
}

impl Native for Env {
    fn pick_folder(&self) -> Option<PathBuf> {
        env_path("OCINYE_BRIDGE_RELEASE")
    }
    fn pick_file(&self, kind: &str) -> Option<PathBuf> {
        match kind {
            "ssh_key" => env_path("OCINYE_BRIDGE_KEY"),
            "cert" => env_path("OCINYE_BRIDGE_TLS_CERT"),
            "key" => env_path("OCINYE_BRIDGE_TLS_KEY"),
            "chain" => env_path("OCINYE_BRIDGE_TLS_CHAIN"),
            _ => None,
        }
    }
    fn save_file(&self, suggested: &str) -> Option<PathBuf> {
        env_path("OCINYE_BRIDGE_STATE").map(|d| d.join(suggested))
    }
    fn open_url(&self, url: &str) {
        eprintln!("open_url {url}");
    }
}

const SHIM: &str = "window.__TAURI_INTERNALS__ = { invoke: (c, a) => fetch('/ipc/' + c, { method: 'POST', body: JSON.stringify(a || {}) }).then((r) => r.json()) };";

fn mime(p: &str) -> &'static str {
    match p.rsplit('.').next() {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("woff2") => "font/woff2",
        _ => "application/octet-stream",
    }
}

#[tokio::main]
async fn main() {
    let port: u16 = std::env::args()
        .nth(1)
        .and_then(|p| p.parse().ok())
        .unwrap_or(8766);
    let ui = env_path("OCINYE_BRIDGE_UI").expect("OCINYE_BRIDGE_UI");
    let state = env_path("OCINYE_BRIDGE_STATE").expect("OCINYE_BRIDGE_STATE");
    let app = App::new(state, "0.1.0");
    if let Ok(r) = std::env::var("OCINYE_BRIDGE_RESOLVE") {
        let mut fixed = BTreeMap::new();
        for pair in r.split(',').filter(|p| !p.is_empty()) {
            if let Some((h, ip)) = pair.split_once('=') {
                fixed
                    .entry(h.to_owned())
                    .or_insert_with(Vec::new)
                    .push(ip.parse().expect("ip"));
            }
        }
        app.set_test_resolver(fixed).await;
    }
    let native: Arc<dyn Native> = Arc::new(Env);
    let listener = TcpListener::bind(("127.0.0.1", port)).await.expect("bind");
    eprintln!("ui_bridge http://127.0.0.1:{port}/");
    loop {
        let Ok((mut sock, _)) = listener.accept().await else {
            continue;
        };
        let app = Arc::clone(&app);
        let native = Arc::clone(&native);
        let ui = ui.clone();
        tokio::spawn(async move {
            let mut buf = Vec::new();
            let mut chunk = [0u8; 65536];
            let (head_end, len) = loop {
                let Ok(n) = sock.read(&mut chunk).await else {
                    return;
                };
                if n == 0 {
                    return;
                }
                buf.extend_from_slice(&chunk[..n]);
                if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                    let head = String::from_utf8_lossy(&buf[..i]).to_lowercase();
                    let len = head
                        .lines()
                        .find_map(|l| l.strip_prefix("content-length:"))
                        .and_then(|v| v.trim().parse::<usize>().ok())
                        .unwrap_or(0);
                    break (i + 4, len);
                }
            };
            while buf.len() < head_end + len {
                let Ok(n) = sock.read(&mut chunk).await else {
                    return;
                };
                if n == 0 {
                    break;
                }
                buf.extend_from_slice(&chunk[..n]);
            }
            let head = String::from_utf8_lossy(&buf[..head_end]).into_owned();
            let path = head
                .split_whitespace()
                .nth(1)
                .unwrap_or("/")
                .split('?')
                .next()
                .unwrap_or("/")
                .to_owned();
            let (status, ctype, body): (&str, &str, Vec<u8>) = if let Some(cmd) =
                path.strip_prefix("/ipc/")
            {
                let mut args: serde_json::Value =
                    serde_json::from_slice(&buf[head_end..]).unwrap_or_default();
                if cmd == "sudo" && args["password"] == "@file" {
                    if let Some(p) = env_path("OCINYE_BRIDGE_SUDO_FILE") {
                        let pw = std::fs::read_to_string(p).unwrap_or_default();
                        args["password"] = serde_json::Value::String(pw.trim().to_owned());
                    }
                }
                match app.dispatch(cmd, args, native).await {
                    Ok(v) => {
                        if let (true, Some(secret), Some(p)) = (
                            cmd == "reveal_credential",
                            v.as_str(),
                            env_path("OCINYE_BRIDGE_CREDENTIAL_FILE"),
                        ) {
                            use std::io::Write as _;
                            use std::os::unix::fs::OpenOptionsExt as _;
                            if let Ok(mut f) = std::fs::OpenOptions::new()
                                .write(true)
                                .create_new(true)
                                .mode(0o600)
                                .open(p)
                            {
                                let _ = f.write_all(secret.as_bytes());
                            }
                        }
                        ("200 OK", "application/json", v.to_string().into_bytes())
                    }
                    Err(e) => (
                        "400 Bad Request",
                        "application/json",
                        serde_json::json!({ "error": e }).to_string().into_bytes(),
                    ),
                }
            } else if path == "/bridge-ipc.js" {
                ("200 OK", "text/javascript", SHIM.as_bytes().to_vec())
            } else {
                let rel = if path == "/" {
                    "index.html".to_owned()
                } else {
                    path.trim_start_matches('/').to_owned()
                };
                if rel.contains("..") {
                    ("404 Not Found", "text/plain", Vec::new())
                } else {
                    match std::fs::read(ui.join(&rel)) {
                        Ok(mut b) => {
                            if rel == "index.html" {
                                let html = String::from_utf8_lossy(&b)
                                    .replace("connect-src ipc: http://ipc.localhost", "connect-src 'self'")
                                    .replace("<script src=\"app.js\"></script>", "<script src=\"bridge-ipc.js\"></script>\n<script src=\"app.js\"></script>");
                                b = html.into_bytes();
                            }
                            ("200 OK", mime(&rel), b)
                        }
                        Err(_) => ("404 Not Found", "text/plain", Vec::new()),
                    }
                }
            };
            let resp = format!("HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n", body.len());
            let _ = sock.write_all(resp.as_bytes()).await;
            let _ = sock.write_all(&body).await;
        });
    }
}
