//! The command layer of the Installer window: one dispatcher for every
//! operator action, shared by the Tauri shell (`apps/installer`) and the local
//! development bridge (`examples/ui_bridge.rs`). The window adds only what is
//! native — file pickers, the save dialog, the browser opener — through
//! [`Native`].
//!
//! Long operations (install, reconnect, resume) run in the background; the
//! window keeps polling `get_view`, which returns the last full view with the
//! live progress while the session is busy.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use ocinye_installer_contracts::plan::PhaseId;
use ocinye_installer_contracts::protocol::{Command, Event};
use ocinye_installer_contracts::secret::SecretText;
use serde_json::{json, Value};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

use crate::installer::{Ending, Installer, Progress};
use crate::ssh::Control;
use crate::ui::{Alert, AuthKind, EndpointRow, Screen, UiSession};

/// What only the native shell can do. Blocking: called on a blocking thread.
pub trait Native: Send + Sync {
    /// Pick a folder (the release bundle).
    fn pick_folder(&self) -> Option<PathBuf>;
    /// Pick a file (an SSH key, a certificate, a key, a chain).
    /// `kind`: `ssh_key`, or the TLS file — `cert`, `key`, `chain`.
    fn pick_file(&self, kind: &str) -> Option<PathBuf>;
    /// Choose where to save a file.
    fn save_file(&self, suggested: &str) -> Option<PathBuf>;
    /// Open a URL in the operator's browser.
    fn open_url(&self, url: &str);
}

/// Live progress while the session is busy.
#[derive(Default)]
struct Live {
    started: Option<Instant>,
    phases: BTreeMap<String, String>,
    ops: BTreeMap<String, Value>,
    current: Option<String>,
    upload: Option<(u64, u64)>,
    warnings: u32,
    events: Vec<Value>,
    verification: Vec<Value>,
}

impl Live {
    fn json(&self) -> Value {
        json!({
            "elapsed_s": self.started.map(|s| s.elapsed().as_secs()),
            "phases": self.phases,
            "ops": self.ops,
            "current": self.current,
            "upload": self.upload.map(|(d, t)| json!({ "done": d, "total": t })),
            "warnings": self.warnings,
            "events": self.events.iter().rev().take(80).collect::<Vec<_>>(),
            "verification": self.verification,
        })
    }

    fn take(&mut self, p: &Progress) {
        match p {
            Progress::Upload { done, total, .. } => {
                self.upload = Some((*done, *total));
                self.phases
                    .entry("P02".into())
                    .or_insert_with(|| "active".into());
                self.current.get_or_insert_with(|| "P02".into());
            }
            Progress::Operator(item) => self.verification.push(json!(item)),
            Progress::Remote(env) => {
                let code = |p: &PhaseId| p.code().to_owned();
                match &env.event {
                    Event::StepStarted { phase } => {
                        self.phases.insert(code(phase), "active".into());
                        self.current = Some(code(phase));
                    }
                    Event::StepProgress { phase, operation } => {
                        self.ops.insert(code(phase), json!(operation));
                    }
                    Event::StepCompleted { phase } => {
                        let warned = self.phases.get(&code(phase)).is_some_and(|s| s == "warn");
                        self.phases
                            .insert(code(phase), if warned { "warn" } else { "done" }.into());
                    }
                    Event::StepSkipped { phase } => {
                        self.phases.insert(code(phase), "skip".into());
                    }
                    Event::StepWarning { phase, .. } => {
                        self.warnings += 1;
                        self.phases.insert(code(phase), "warn".into());
                    }
                    Event::StepFailed { phase, .. } => {
                        self.phases.insert(code(phase), "fail".into());
                    }
                    Event::VerificationItem { item } => self.verification.push(json!(item)),
                    _ => {}
                }
                self.events
                    .push(json!({ "at": env.at, "event": env.event }));
            }
        }
    }
}

/// The window's state.
pub struct App {
    session: Arc<tokio::sync::Mutex<UiSession>>,
    cache: std::sync::Mutex<Value>,
    live: Arc<std::sync::Mutex<Live>>,
    control: std::sync::Mutex<Option<Control>>,
    platform: String,
    version: String,
}

fn s_arg(args: &Value, k: &str) -> String {
    args.get(k)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

impl App {
    /// A window at I01, its state in `state_dir`.
    #[must_use]
    pub fn new(state_dir: PathBuf, version: &str) -> Arc<Self> {
        Arc::new(Self {
            session: Arc::new(tokio::sync::Mutex::new(UiSession::new(Installer::new(
                state_dir,
            )))),
            cache: std::sync::Mutex::new(Value::Null),
            live: Arc::new(std::sync::Mutex::new(Live::default())),
            control: std::sync::Mutex::new(None),
            platform: format!("{} · {}", std::env::consts::OS, std::env::consts::ARCH),
            version: version.to_owned(),
        })
    }

    /// The window is closing: end the bootstrap session (and its `/tmp`
    /// upload). A session busy following an installation is left alone — the
    /// executor carries on detached and removes the upload itself.
    pub async fn shutdown(&self) {
        if let Ok(mut s) = self.session.try_lock() {
            s.inst.end_session().await;
        }
    }

    /// Test-only resolver entries (controlled DNS fixtures; never in the product).
    pub async fn set_test_resolver(&self, fixed: BTreeMap<String, Vec<std::net::IpAddr>>) {
        self.session.lock().await.inst.resolver.fixed = fixed;
    }

    fn snapshot(&self, s: &UiSession) -> Value {
        let live = self.live.lock().map(|l| l.json()).unwrap_or(Value::Null);
        let mut v = s.view(&live);
        v["app"] = json!({ "version": self.version, "platform": self.platform });
        v["busy"] = json!(false);
        if let Ok(mut c) = self.cache.lock() {
            c.clone_from(&v);
        }
        v
    }

    fn busy_view(&self) -> Value {
        let mut v = self.cache.lock().map(|c| c.clone()).unwrap_or(Value::Null);
        v["live"] = self.live.lock().map(|l| l.json()).unwrap_or(Value::Null);
        v["busy"] = json!(true);
        v
    }

    async fn background<F, Fut>(self: &Arc<Self>, f: F) -> Value
    where
        F: FnOnce(Arc<tokio::sync::Mutex<UiSession>>, UnboundedSender<Progress>) -> Fut
            + Send
            + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        {
            let s = self.session.lock().await;
            if let Ok(mut c) = self.control.lock() {
                *c = s.inst.control().ok();
            }
            if let Ok(mut l) = self.live.lock() {
                *l = Live {
                    started: Some(Instant::now()),
                    ..Live::default()
                };
            }
            let _ = self.snapshot(&s);
        }
        let (tx, mut rx) = unbounded_channel::<Progress>();
        let live = Arc::clone(&self.live);
        tokio::spawn(async move {
            while let Some(p) = rx.recv().await {
                if let Ok(mut l) = live.lock() {
                    l.take(&p);
                }
            }
        });
        tokio::spawn(f(Arc::clone(&self.session), tx));
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        self.busy_view()
    }

    /// Handle one operator action.
    ///
    /// # Errors
    ///
    /// An unknown command (the IPC surface is closed).
    #[allow(clippy::too_many_lines)]
    pub async fn dispatch(
        self: &Arc<Self>,
        cmd: &str,
        args: Value,
        native: Arc<dyn Native>,
    ) -> Result<Value, String> {
        if cmd == "get_view" {
            return Ok(match self.session.try_lock() {
                Ok(s) => self.snapshot(&s),
                Err(_) => self.busy_view(),
            });
        }
        if cmd == "stop" {
            let control = self.control.lock().ok().and_then(|c| c.clone());
            let id = self
                .cache
                .lock()
                .ok()
                .and_then(|c| c["plan"]["installation_id"].as_str().map(str::to_owned));
            if let (Some(control), Some(id)) = (control, id) {
                if let Ok(id) = ocinye_installer_contracts::ident::InstallationId::parse(&id) {
                    let _ = control
                        .send(&Command::Stop {
                            installation_id: id,
                        })
                        .await;
                }
            }
            if let Ok(mut c) = self.cache.lock() {
                c["stopping"] = json!(true);
            }
            return Ok(self.busy_view());
        }
        if cmd == "reveal_credential" {
            let s = self.session.lock().await;
            return Ok(json!(s.reveal_credential()));
        }
        // Pickers run before the session lock.
        let picked = match cmd {
            "choose_release" => {
                let n = Arc::clone(&native);
                tokio::task::spawn_blocking(move || n.pick_folder())
                    .await
                    .ok()
                    .flatten()
            }
            "choose_key" => {
                let n = Arc::clone(&native);
                tokio::task::spawn_blocking(move || n.pick_file("ssh_key"))
                    .await
                    .ok()
                    .flatten()
            }
            "choose_tls_file" => {
                let n = Arc::clone(&native);
                let kind = s_arg(&args, "kind");
                tokio::task::spawn_blocking(move || n.pick_file(&kind))
                    .await
                    .ok()
                    .flatten()
            }
            _ => None,
        };
        match cmd {
            "install" | "reconnect" | "resume" => return Ok(self.execute(cmd).await),
            _ => {}
        }
        let mut s = self.session.lock().await;
        match cmd {
            "set_lang" => {
                let l = s_arg(&args, "lang");
                if matches!(l.as_str(), "pt" | "en" | "fr") {
                    s.lang = l;
                }
            }
            "start" => s.start(),
            "back" => s.back(),
            "choose_release" => {
                if let Some(p) = picked {
                    s.choose_release(p);
                }
            }
            "continue_release" => s.continue_release(),
            "choose_key" => {
                if let Some(p) = picked {
                    s.draft.key_path = Some(p.to_string_lossy().into_owned());
                    s.draft.auth = AuthKind::Key;
                }
            }
            "test_connection" => {
                let auth: AuthKind =
                    serde_json::from_value(args["auth"].clone()).unwrap_or(AuthKind::Agent);
                let key = s.draft.key_path.clone();
                s.set_server(
                    s_arg(&args, "host"),
                    s_arg(&args, "port"),
                    s_arg(&args, "user"),
                    auth,
                    key,
                );
                let pw = args["password"]
                    .as_str()
                    .filter(|p| !p.is_empty())
                    .map(|p| SecretText::new(p.to_owned()));
                s.test_connection(pw).await;
            }
            "continue_server" => s.continue_server().await,
            "trust" => s.trust().await,
            "other_server" => {
                s.mismatch = None;
                s.screen = Screen::Server;
                s.alert = None;
            }
            "forget_server" => s.forget_server(),
            "sudo" => s.sudo(SecretText::new(s_arg(&args, "password"))).await,
            "cancel_sudo" => s.cancel_sudo(),
            "preflight" => s.preflight().await,
            "continue_preflight" => s.continue_preflight().await,
            "continue_hardware" => s.continue_hardware(),
            "continue_instance" => {
                s.set_instance(s_arg(&args, "name"));
                s.continue_instance();
            }
            "set_distributions" => {
                let v: Vec<String> =
                    serde_json::from_value(args["ordered"].clone()).unwrap_or_default();
                s.set_distributions(v);
            }
            "continue_distributions" => s.continue_distributions(),
            "set_endpoints" => {
                let rows: Vec<EndpointRow> =
                    serde_json::from_value(args["rows"].clone()).unwrap_or_default();
                s.set_endpoints(rows);
            }
            "check_dns" => s.check_dns().await,
            "continue_endpoints" => s.continue_endpoints(),
            "choose_tls_file" => {
                if let Some(p) = picked {
                    let p = Some(p.to_string_lossy().into_owned());
                    let (mut c, mut k, mut ch) = (
                        s.draft.tls_cert.clone(),
                        s.draft.tls_key.clone(),
                        s.draft.tls_chain.clone(),
                    );
                    match s_arg(&args, "kind").as_str() {
                        "cert" => c = p,
                        "key" => k = p,
                        "chain" => ch = p,
                        _ => {}
                    }
                    s.set_tls("provided".into(), c, k, ch);
                }
            }
            "set_tls" => {
                let (c, k, ch) = (
                    s.draft.tls_cert.clone(),
                    s.draft.tls_key.clone(),
                    s.draft.tls_chain.clone(),
                );
                s.set_tls(s_arg(&args, "mode"), c, k, ch);
            }
            "continue_tls" => s.continue_tls(),
            "continue_admin" => {
                let f: [String; 4] =
                    serde_json::from_value(args["fields"].clone()).unwrap_or_default();
                s.set_admin(f);
                s.continue_admin();
            }
            "change" => s.change(&s_arg(&args, "section")),
            "remove_incomplete" => {
                let id = s.inst.journal.as_ref().map(|j| j.installation_id.clone());
                if let Some(id) = id {
                    if s.inst.remove_incomplete(id).await.is_ok() {
                        s.inst.journal = None;
                        s.preflight().await;
                        s.screen = Screen::Preflight;
                    }
                }
            }
            "recheck" => {
                if let Ok(mut l) = self.live.lock() {
                    l.verification.clear();
                }
                let _ = s.inst.verify_from_here(None).await;
                s.verified();
            }
            "acknowledge_credential" => s.acknowledge_credential(),
            "show_credential" => {
                if s.inst.secret.is_some() {
                    s.screen = Screen::Credential;
                }
            }
            "show_receipt" => s.screen = Screen::Receipt,
            "save_receipt" => {
                if let Some(r) = s.inst.receipt() {
                    let n = Arc::clone(&native);
                    let name = r.file_name();
                    let target = tokio::task::spawn_blocking(move || n.save_file(&name))
                        .await
                        .ok()
                        .flatten();
                    if let Some(p) = target {
                        std::fs::write(p, r.to_json()).map_err(|e| e.to_string())?;
                    }
                }
            }
            "open_ocinye" => {
                // Only a verified endpoint, in the operator's own browser.
                if s.inst
                    .lifecycle
                    .as_ref()
                    .is_some_and(ocinye_installer_contracts::verification::LifecycleState::may_open)
                {
                    if let Some(plan) = &s.inst.plan {
                        native.open_url(&format!(
                            "https://{}/",
                            plan.configuration.endpoints.canonical.as_str()
                        ));
                    }
                }
            }
            other => return Err(format!("unknown command {other}")),
        }
        Ok(self.snapshot(&s))
    }

    async fn execute(self: &Arc<Self>, cmd: &str) -> Value {
        {
            let mut s = self.session.lock().await;
            if cmd == "resume" && s.inst.plan.is_none() {
                let id = s.inst.journal.as_ref().map(|j| j.installation_id.clone());
                if let Some(id) = id {
                    s.inst.plan = s.inst.saved_plan(&id);
                }
            }
            if s.inst.plan.is_none() {
                if cmd == "resume" {
                    s.alert = Some(Alert {
                        code: "PLAN_NOT_AVAILABLE".into(),
                        params: BTreeMap::new(),
                    });
                }
                return self.snapshot(&s);
            }
            s.screen = Screen::Installing;
            s.alert = None;
            s.stopping = false;
        }
        let cmd = cmd.to_owned();
        self.background(move |session, tx| async move {
            let mut s = session.lock().await;
            let r = match cmd.as_str() {
                "install" => s.inst.install(Some(&tx)).await,
                "reconnect" => s.inst.reattach(Some(&tx)).await,
                _ => s.inst.resume(Some(&tx)).await,
            };
            match r {
                Ok(ending) => {
                    let completed = ending == Ending::Completed;
                    s.ended(ending);
                    if completed {
                        let _ = s.inst.verify_from_here(Some(&tx)).await;
                        s.verified();
                        if s.inst.secret.is_some() {
                            s.screen = Screen::Credential;
                        }
                    }
                }
                Err(e) => {
                    s.screen = if cmd == "reconnect" {
                        Screen::Interrupted
                    } else {
                        Screen::Failed
                    };
                    s.ending = Some(Ending::Refused(format!("{e:?}")));
                }
            }
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NoNative;
    impl Native for NoNative {
        fn pick_folder(&self) -> Option<PathBuf> {
            None
        }
        fn pick_file(&self, _: &str) -> Option<PathBuf> {
            None
        }
        fn save_file(&self, _: &str) -> Option<PathBuf> {
            None
        }
        fn open_url(&self, _: &str) {
            panic!("nada se abre sem verificação");
        }
    }

    #[tokio::test]
    async fn a_superficie_de_comandos_e_fechada() {
        let app = App::new(std::env::temp_dir().join("ocinye-app-test"), "0.1.0");
        let n: Arc<dyn Native> = Arc::new(NoNative);
        for bad in ["execute_shell", "run_remote", "read_file", "eval"] {
            assert!(
                app.dispatch(bad, json!({}), Arc::clone(&n)).await.is_err(),
                "{bad}"
            );
        }
        let v = app
            .dispatch("get_view", json!({}), Arc::clone(&n))
            .await
            .unwrap();
        assert_eq!(v["screen"], "welcome");
        // Abrir sem uma Instância verificada não abre nada (o Native entra em pânico se abrir).
        let _ = app
            .dispatch("open_ocinye", json!({}), Arc::clone(&n))
            .await
            .unwrap();
        let v = app.dispatch("start", json!({}), n).await.unwrap();
        assert_eq!(v["screen"], "release");
    }
}
