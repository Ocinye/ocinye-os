//! Ocinye OS Installer — the window (D011, ADR-0022).
//!
//! Thin: every command is one operator action handed to the controller's
//! dispatcher ([`ocinye_installer_controller::app`]), which validates it. The
//! webview cannot read files, run commands, open dialogs or reach the network:
//! the native pickers and the browser opener are called from here, and the
//! capability (`capabilities/default.json`) grants the webview exactly these
//! commands and nothing else.

#![forbid(unsafe_code)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::Arc;

use ocinye_installer_controller::app::{App, Native};
use serde_json::{json, Value};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

struct Shell(AppHandle);

impl Native for Shell {
    fn pick_folder(&self) -> Option<PathBuf> {
        self.0
            .dialog()
            .file()
            .blocking_pick_folder()
            .and_then(|p| p.into_path().ok())
    }

    fn pick_file(&self, _kind: &str) -> Option<PathBuf> {
        self.0
            .dialog()
            .file()
            .blocking_pick_file()
            .and_then(|p| p.into_path().ok())
    }

    fn save_file(&self, suggested: &str) -> Option<PathBuf> {
        self.0
            .dialog()
            .file()
            .set_file_name(suggested)
            .blocking_save_file()
            .and_then(|p| p.into_path().ok())
    }

    fn open_url(&self, url: &str) {
        let _ = self.0.opener().open_url(url, None::<&str>);
    }
}

struct Window {
    app: Arc<App>,
    native: Arc<dyn Native>,
}

type R = Result<Value, String>;

async fn go(w: &State<'_, Window>, cmd: &str, args: Value) -> R {
    w.app.dispatch(cmd, args, Arc::clone(&w.native)).await
}

macro_rules! plain {
    ($($name:ident),* $(,)?) => {
        $(
            #[tauri::command]
            async fn $name(w: State<'_, Window>) -> R {
                go(&w, stringify!($name), json!({})).await
            }
        )*
    };
}

plain!(
    get_view,
    start,
    back,
    choose_release,
    continue_release,
    choose_key,
    continue_server,
    trust,
    other_server,
    forget_server,
    cancel_sudo,
    preflight,
    continue_preflight,
    continue_hardware,
    continue_distributions,
    check_dns,
    continue_endpoints,
    continue_tls,
    install,
    stop,
    reconnect,
    resume,
    remove_incomplete,
    recheck,
    reveal_credential,
    acknowledge_credential,
    show_credential,
    show_receipt,
    save_receipt,
    open_ocinye,
);

#[tauri::command]
async fn set_lang(w: State<'_, Window>, lang: String) -> R {
    go(&w, "set_lang", json!({ "lang": lang })).await
}

#[tauri::command]
async fn test_connection(
    w: State<'_, Window>,
    host: String,
    port: String,
    user: String,
    auth: String,
    password: Option<String>,
) -> R {
    go(
        &w,
        "test_connection",
        json!({ "host": host, "port": port, "user": user, "auth": auth, "password": password }),
    )
    .await
}

#[tauri::command]
async fn sudo(w: State<'_, Window>, password: String) -> R {
    go(&w, "sudo", json!({ "password": password })).await
}

#[tauri::command]
async fn continue_instance(w: State<'_, Window>, name: String) -> R {
    go(&w, "continue_instance", json!({ "name": name })).await
}

#[tauri::command]
async fn set_distributions(w: State<'_, Window>, ordered: Vec<String>) -> R {
    go(&w, "set_distributions", json!({ "ordered": ordered })).await
}

#[tauri::command]
async fn set_endpoints(w: State<'_, Window>, rows: Value) -> R {
    go(&w, "set_endpoints", json!({ "rows": rows })).await
}

#[tauri::command]
async fn choose_tls_file(w: State<'_, Window>, kind: String) -> R {
    go(&w, "choose_tls_file", json!({ "kind": kind })).await
}

#[tauri::command]
async fn set_tls(w: State<'_, Window>, mode: String) -> R {
    go(&w, "set_tls", json!({ "mode": mode })).await
}

#[tauri::command]
async fn continue_admin(w: State<'_, Window>, fields: [String; 4]) -> R {
    go(&w, "continue_admin", json!({ "fields": fields })).await
}

#[tauri::command]
async fn change(w: State<'_, Window>, section: String) -> R {
    go(&w, "change", json!({ "section": section })).await
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let dir: PathBuf = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            app.manage(Window {
                app: App::new(dir, env!("CARGO_PKG_VERSION")),
                native: Arc::new(Shell(app.handle().clone())),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_view,
            set_lang,
            start,
            back,
            choose_release,
            continue_release,
            choose_key,
            test_connection,
            continue_server,
            trust,
            other_server,
            forget_server,
            sudo,
            cancel_sudo,
            preflight,
            continue_preflight,
            continue_hardware,
            continue_instance,
            continue_distributions,
            set_distributions,
            set_endpoints,
            check_dns,
            continue_endpoints,
            choose_tls_file,
            set_tls,
            continue_tls,
            continue_admin,
            change,
            install,
            stop,
            reconnect,
            resume,
            remove_incomplete,
            recheck,
            reveal_credential,
            acknowledge_credential,
            show_credential,
            show_receipt,
            save_receipt,
            open_ocinye
        ])
        .run(tauri::generate_context!())
        .expect("Ocinye OS Installer");
}
