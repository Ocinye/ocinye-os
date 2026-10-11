//! Ocinye OS — the native system interface (ADR-0031).
//!
//! This process presents the system; it controls nothing. The webview has five
//! commands: ask the Session Service something, ask the System Service
//! something, get the exact form of a request to prove fresh authentication for, say how much of the screen the interface needs, and open the
//! Workspace. Both services take closed, typed requests and decide for
//! themselves, from who is connected and what the session is, whether to
//! answer — nothing this process says counts as authority.
//!
//! The webview cannot read files, run commands or reach the network: the
//! capability (`capabilities/default.json`) grants exactly these commands.

#![forbid(unsafe_code)]

use std::path::Path;
use std::process::{Child, Command};
use std::sync::Mutex;

use ocinye_system_client::{Client, ClientError, SessionClient};
use ocinye_system_contracts::request::Request;
use ocinye_system_contracts::session::{SessionRequest, SESSION_SOCKET_PATH};
use ocinye_system_contracts::SOCKET_PATH;
use serde_json::{json, Value};
use tauri::{State, Url, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

/// Where the installation wrote the address of this machine's Workspace. The
/// interface shows that address and no other: it is not an argument, and the
/// webview cannot name one.
const WORKSPACE_ADDRESS: &str = "/etc/ocinye/workspace-url";

/// The argument that makes this program the Workspace's window instead of the
/// system interface: a separate process, which the compositor therefore does
/// not treat as the trusted surface, with a window that has no command at all.
const WORKSPACE_MODE: &str = "--workspace";

/// The certificate this machine's own Instance serves, where the installation
/// put it. Public by nature: it is what the proxy hands every client.
const INSTANCE_CERTIFICATE: &str = "/etc/ocinye/tls/instance.pem";

/// Open the Workspace in its window, trusting this machine's own Instance.
///
/// An Instance installed without a publicly issued certificate serves one
/// that no authority vouches for, and the webview would refuse the machine's
/// own Workspace. So the window is told that this exact certificate — the
/// file on this machine, read by this process — is acceptable for this exact
/// host, and for nothing else: any other certificate, and any other host,
/// is judged as usual.
#[cfg(target_os = "linux")]
fn open_trusting_instance(window: &WebviewWindow, address: &Url) {
    use webkit2gtk::{WebContextExt, WebViewExt};
    let pem = std::fs::read_to_string(INSTANCE_CERTIFICATE).ok();
    let host = address.host_str().unwrap_or_default().to_owned();
    let address = address.to_string();
    let _ = window.with_webview(move |webview| {
        let view = webview.inner();
        let certificate = pem.and_then(|pem| gio::TlsCertificate::from_pem(&pem).ok());
        if let (Some(certificate), Some(context)) = (certificate, view.context()) {
            context.allow_tls_certificate_for_host(&certificate, &host);
        }
        view.load_uri(&address);
    });
}

#[cfg(not(target_os = "linux"))]
fn open_trusting_instance(window: &WebviewWindow, address: &Url) {
    let _ = window.navigate(address.clone());
}

/// The Workspace's address, when the installation recorded a usable one:
/// HTTPS, or plain HTTP to this machine only.
fn workspace_address() -> Option<Url> {
    let text = std::fs::read_to_string(WORKSPACE_ADDRESS).ok()?;
    let url = Url::parse(text.trim()).ok()?;
    let local = matches!(url.host_str(), Some("127.0.0.1" | "localhost"));
    (url.scheme() == "https" || (url.scheme() == "http" && local)).then_some(url)
}

/// The Workspace's window, while it is open.
struct Workspace(Mutex<Option<Child>>);

/// With `OCINYE_SYSTEM_UI_TRACE` set, the name of each command and how long it
/// took go to standard error. Never an argument and never an answer: a trace
/// must not be a place where a password can end up.
fn trace(command: &str, started: std::time::Instant) {
    if std::env::var_os("OCINYE_SYSTEM_UI_TRACE").is_some() {
        eprintln!(
            "ocinye-system-ui: {command} {} ms",
            started.elapsed().as_millis()
        );
    }
}

/// What the interface is told when a service did not answer: a code it has a
/// sentence for, never the text of an error.
fn unreachable(error: &ClientError) -> Value {
    json!({ "unreachable": error.to_string() })
}

/// One question to the Session Service; the reply as the contract defines it.
#[tauri::command]
async fn session(request: SessionRequest) -> Value {
    // The interface never consumes a grant: it presents one to the System
    // Service, which is the only one the Session Service answers on that.
    if matches!(request, SessionRequest::ConsumeGrant { .. }) {
        return json!({ "unreachable": "NOT_OFFERED" });
    }
    let started = std::time::Instant::now();
    let answer =
        SessionClient::connect(Path::new(SESSION_SOCKET_PATH)).and_then(|mut c| c.ask(&request));
    trace("session", started);
    match answer {
        Ok(reply) => json!({ "reply": reply }),
        Err(error) => unreachable(&error),
    }
}

/// One capability of the System Service; the outcome as the contract defines it.
#[tauri::command]
async fn system(request: Request, grant: Option<String>) -> Value {
    let started = std::time::Instant::now();
    let answer = Client::connect(Path::new(SOCKET_PATH)).and_then(|mut c| c.call(request, grant));
    trace("system", started);
    match answer {
        Ok(outcome) => json!({ "outcome": outcome }),
        Err(error) => unreachable(&error),
    }
}

/// The canonical form of a request, as the System Service will compute it: a
/// fresh-authentication grant is issued for exactly this text, and only the
/// same serialiser can be trusted to produce it.
#[tauri::command]
async fn binding(request: Request) -> String {
    serde_json::to_string(&request).unwrap_or_default()
}

/// How much of the screen the interface needs: all of it (sign-in, lock,
/// Settings, a panel) or only the bar. The compositor reads the window's
/// full-screen state and gives the rest to the Workspace.
#[tauri::command]
async fn surface(window: WebviewWindow, full: bool) -> bool {
    let started = std::time::Instant::now();
    let done = window.set_fullscreen(full).is_ok();
    trace(if full { "surface full" } else { "surface bar" }, started);
    done
}

/// Open the Workspace, if it is not open already.
#[tauri::command]
async fn open_workspace(workspace: State<'_, Workspace>) -> Result<bool, ()> {
    let Ok(mut held) = workspace.0.lock() else {
        return Ok(false);
    };
    if let Some(child) = held.as_mut() {
        if matches!(child.try_wait(), Ok(None)) {
            return Ok(true);
        }
    }
    if workspace_address().is_none() {
        return Ok(false);
    }
    *held = std::env::current_exe()
        .ok()
        .and_then(|me| Command::new(me).arg(WORKSPACE_MODE).spawn().ok());
    Ok(held.is_some())
}

fn main() {
    let mut args = std::env::args().skip(1);
    let workspace = match (args.next().as_deref(), args.next()) {
        (None, _) => false,
        (Some(WORKSPACE_MODE), None) => true,
        _ => {
            eprintln!("usage: ocinye-system-ui [{WORKSPACE_MODE}]");
            std::process::exit(64);
        }
    };
    tauri::Builder::default()
        .manage(Workspace(Mutex::new(None)))
        .invoke_handler(tauri::generate_handler![
            session,
            system,
            binding,
            surface,
            open_workspace
        ])
        .setup(move |app| {
            if workspace {
                let address = workspace_address()
                    .ok_or("no Workspace address is recorded on this machine")?;
                let origin = address.origin();
                // The window is called `workspace`: no capability names it, so
                // none of the commands above exists for it. It stays on the
                // Workspace's own origin.
                let blank: Url = "about:blank".parse()?;
                let window =
                    WebviewWindowBuilder::new(app, "workspace", WebviewUrl::External(blank))
                        .title("Ocinye Workspace")
                        .decorations(false)
                        .on_navigation(move |to| to.scheme() == "about" || to.origin() == origin)
                        .build()?;
                open_trusting_instance(&window, &address);
            } else {
                WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                    .title("Ocinye OS")
                    .decorations(false)
                    .fullscreen(true)
                    .build()?;
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Ocinye OS system interface");
}
