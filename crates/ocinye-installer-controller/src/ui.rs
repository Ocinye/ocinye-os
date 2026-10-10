//! The UI session: which screen, what the operator typed, what failed — and
//! the view the renderer draws (D011_SCREEN_MATRIX).
//!
//! The renderer (the Installer's webview) never decides anything: it shows a
//! [`View`] and sends actions. Every action is validated here, with the same
//! typed contracts the server uses, before it changes anything. A screen is
//! reached only in order; an alert carries a closed code and its parameters,
//! and the renderer writes it in the operator's language.

use std::collections::BTreeMap;
use std::net::IpAddr;
use std::path::PathBuf;

use ocinye_contracts::Distribution;
use ocinye_installer_contracts::ident::{
    parse_port, EmailAddress, HostNameValue, InstanceName, PersonName, SshUser, TargetHost,
};
use ocinye_installer_contracts::plan::{
    AccessEndpoints, BoundEndpoint, Distributions, FirstAdmin, Identity, InstallationConfiguration,
    TlsPlan,
};
use ocinye_installer_contracts::secret::SecretText;
use serde::Serialize;
use serde_json::{json, Value};

use crate::installer::{Ending, Installer, Refusal, TlsFiles};
use crate::ssh::{Auth, Elevation, Target};
use crate::tls::TlsValidation;

/// Screens (the reference's state families).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Screen {
    /// I01.
    Welcome,
    /// I02 (+ I20–I22).
    Release,
    /// I03 (+ I23–I25, I40).
    Server,
    /// I04.
    HostTrust,
    /// I26.
    HostMismatch,
    /// I06 (+ I05 dialog, I27–I30, I41, I42).
    Preflight,
    /// I19.
    Incomplete,
    /// I07 (+ I31).
    Hardware,
    /// I08.
    Instance,
    /// I09.
    Distributions,
    /// I10 (+ I32).
    Endpoints,
    /// I11 (+ I33).
    Tls,
    /// I12.
    Admin,
    /// I13.
    Review,
    /// I14.
    Installing,
    /// I15 (+ I37).
    Verifying,
    /// I34, I35, I38, I39, I43.
    Failed,
    /// I36.
    Interrupted,
    /// I16.
    Complete,
    /// I17.
    Credential,
    /// I18.
    Receipt,
}

/// An alert: a closed code and its parameters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Alert {
    /// Code.
    pub code: String,
    /// Parameters (non-secret).
    pub params: BTreeMap<String, String>,
}

fn alert(code: &str, params: &[(&str, &str)]) -> Alert {
    Alert {
        code: code.to_owned(),
        params: params
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect(),
    }
}

/// How the operator authenticates (the I03 segmented control).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthKind {
    /// SSH agent.
    Agent,
    /// Private key file.
    Key,
    /// Password.
    Password,
}

/// One endpoint row on I10.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, serde::Deserialize)]
pub struct EndpointRow {
    /// Host as typed.
    pub host: String,
    /// `None` for the canonical (generic).
    pub distribution: Option<String>,
    /// DNS observation (`DNS_REQUIRED`, `DNS_OK`, `DNS_UNRESOLVED`, `DNS_WRONG_TARGET`).
    #[serde(default)]
    pub dns: String,
    /// What it resolved to.
    #[serde(default)]
    pub seen: Vec<String>,
}

/// What the operator typed (never a secret).
#[derive(Debug, Clone, Serialize)]
pub struct Draft {
    /// Server.
    pub host: String,
    /// Port.
    pub port: String,
    /// User.
    pub user: String,
    /// Auth kind.
    pub auth: AuthKind,
    /// Key file (local path, shown by name).
    pub key_path: Option<String>,
    /// Instance name.
    pub instance_name: String,
    /// Distributions, ordered.
    pub distributions: Vec<String>,
    /// Endpoint rows (canonical first).
    pub endpoints: Vec<EndpointRow>,
    /// `provided` or `self`.
    pub tls_mode: String,
    /// Certificate file.
    pub tls_cert: Option<String>,
    /// Key file (never read here except by the validator).
    pub tls_key: Option<String>,
    /// Chain file.
    pub tls_chain: Option<String>,
    /// Person name / email, privileged name / email.
    pub admin: [String; 4],
}

impl Default for Draft {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: "22".into(),
            user: String::new(),
            auth: AuthKind::Agent,
            key_path: None,
            instance_name: String::new(),
            distributions: vec!["research".into()],
            endpoints: vec![EndpointRow {
                host: String::new(),
                distribution: None,
                dns: "DNS_REQUIRED".into(),
                seen: vec![],
            }],
            tls_mode: "provided".into(),
            tls_cert: None,
            tls_key: None,
            tls_chain: None,
            admin: Default::default(),
        }
    }
}

/// The UI session.
pub struct UiSession {
    /// The controller.
    pub inst: Installer,
    /// Current screen.
    pub screen: Screen,
    /// `pt` (canonical), `en`, `fr`.
    pub lang: String,
    /// The alert on the current screen.
    pub alert: Option<Alert>,
    /// Field errors (`field` → code).
    pub field_errors: BTreeMap<String, String>,
    /// What the operator typed.
    pub draft: Draft,
    /// The sudo dialog is open (I05).
    pub sudo_dialog: bool,
    /// The first-contact key on I04.
    pub presented_key: Option<(String, String)>,
    /// The mismatch on I26: (known, presented).
    pub mismatch: Option<(String, String)>,
    /// The I11 validation.
    pub tls_validation: Option<TlsValidation>,
    /// How the execution ended.
    pub ending: Option<Ending>,
    /// A stop was requested (I14 → «A aguardar um ponto seguro…»).
    pub stopping: bool,
    /// The credential was acknowledged (I17 → the value is gone).
    pub credential_acknowledged: bool,
}

impl UiSession {
    /// A session at I01.
    #[must_use]
    pub fn new(inst: Installer) -> Self {
        Self {
            inst,
            screen: Screen::Welcome,
            lang: "pt".into(),
            alert: None,
            field_errors: BTreeMap::new(),
            draft: Draft::default(),
            sudo_dialog: false,
            presented_key: None,
            mismatch: None,
            tls_validation: None,
            ending: None,
            stopping: false,
            credential_acknowledged: false,
        }
    }

    fn go(&mut self, s: Screen) {
        self.screen = s;
        self.alert = None;
        self.field_errors.clear();
    }

    /// I01 «Começar».
    pub fn start(&mut self) {
        self.go(Screen::Release);
    }

    /// I02 · a package chosen in the native picker.
    pub fn choose_release(&mut self, dir: PathBuf) {
        self.alert = match self.inst.open_release(&dir) {
            Ok(_) => None,
            Err(Refusal::Release(r)) => Some(match r {
                crate::bundle::ReleaseRejection::ChecksumFailed { file } => {
                    alert("RELEASE_CHECKSUM_FAILED", &[("file", &file)])
                }
                crate::bundle::ReleaseRejection::UnsupportedArchitecture { arch } => {
                    alert("RELEASE_UNSUPPORTED_ARCH", &[("arch", &arch)])
                }
                crate::bundle::ReleaseRejection::InvalidManifest { field } => {
                    alert("INVALID_MANIFEST", &[("field", &field)])
                }
            }),
            Err(other) => Some(alert(
                "RELEASE_UNREADABLE",
                &[("detail", &format!("{other:?}"))],
            )),
        };
    }

    /// I02 «Continuar».
    pub fn continue_release(&mut self) {
        if self.inst.release.is_some() {
            self.go(Screen::Server);
        }
    }

    /// I03 · fields.
    pub fn set_server(
        &mut self,
        host: String,
        port: String,
        user: String,
        auth: AuthKind,
        key_path: Option<String>,
    ) {
        self.draft.host = host;
        self.draft.port = port;
        self.draft.user = user;
        self.draft.auth = auth;
        self.draft.key_path = key_path;
    }

    fn target(&mut self) -> Option<Target> {
        self.field_errors.clear();
        let host = TargetHost::parse(&self.draft.host);
        let port = parse_port(&self.draft.port);
        let user = SshUser::parse(&self.draft.user);
        if host.is_err() {
            self.field_errors
                .insert("host".into(), "INPUT_INVALID_HOST".into());
        }
        if port.is_err() {
            self.field_errors
                .insert("port".into(), "INPUT_INVALID_PORT".into());
        }
        if user.is_err() {
            self.field_errors
                .insert("user".into(), "INPUT_INVALID_USER".into());
        }
        Some(Target {
            host: host.ok()?,
            port: port.ok()?.get(),
            user: user.ok()?,
        })
    }

    /// I03 «Testar ligação». `password` only for [`AuthKind::Password`].
    pub async fn test_connection(&mut self, password: Option<SecretText>) {
        self.alert = None;
        let Some(target) = self.target() else { return };
        let auth = match self.draft.auth {
            AuthKind::Agent => Auth::Agent,
            AuthKind::Key => match &self.draft.key_path {
                Some(p) => Auth::PrivateKey(PathBuf::from(p)),
                None => {
                    self.field_errors
                        .insert("key".into(), "KEY_REQUIRED".into());
                    return;
                }
            },
            AuthKind::Password => match password {
                Some(p) if !p.is_empty() => Auth::Password(p),
                _ => {
                    self.field_errors
                        .insert("pw".into(), "PASSWORD_REQUIRED".into());
                    return;
                }
            },
        };
        self.inst.set_target(target, auth);
        self.after_connect().await;
    }

    async fn after_connect(&mut self) {
        match self.inst.test_connection().await {
            Ok(_) => {}
            Err(Refusal::HostKeyUnknown {
                algorithm,
                fingerprint,
            }) => {
                self.presented_key = Some((algorithm, fingerprint));
                self.go(Screen::HostTrust);
            }
            Err(Refusal::HostKeyMismatch { known, presented }) => {
                self.mismatch = Some((known, presented));
                self.go(Screen::HostMismatch);
            }
            Err(Refusal::SshUnreachable(_)) => self.alert = Some(alert("SSH_UNREACHABLE", &[])),
            Err(Refusal::SshAuthRefused) => self.alert = Some(alert("SSH_AUTH_REFUSED", &[])),
            Err(Refusal::UnsupportedTarget { release, server }) => {
                self.alert = Some(alert(
                    "RELEASE_UNSUPPORTED_TARGET",
                    &[("release", &release), ("server", &server)],
                ));
            }
            Err(e) => self.alert = Some(alert("SSH_TRANSPORT", &[("detail", &format!("{e:?}"))])),
        }
    }

    /// I03 «Continuar» (after a successful test).
    pub async fn continue_server(&mut self) {
        if self.inst.probe.is_none() {
            return;
        }
        match self.inst.elevation {
            None => {
                self.go(Screen::Preflight);
                self.alert = Some(alert("PRIVILEGE_MISSING", &[]));
            }
            Some(Elevation::SudoPassword) if !self.sudo_ready() => {
                self.go(Screen::Preflight);
                self.sudo_dialog = true;
            }
            Some(_) => {
                self.go(Screen::Preflight);
                self.begin_preflight().await;
            }
        }
    }

    fn sudo_ready(&self) -> bool {
        self.inst.sudo_ready()
    }

    /// I04 «Confiar neste servidor»: pins exactly what is shown, reconnects.
    pub async fn trust(&mut self) {
        let Some((alg, fp)) = self.presented_key.take() else {
            return;
        };
        if let Err(e) = self.inst.trust_host(&alg, &fp) {
            self.alert = Some(alert("HOST_KEY_MISMATCH", &[("detail", &format!("{e:?}"))]));
            return;
        }
        self.go(Screen::Server);
        self.after_connect().await;
    }

    /// I26 «Esquecer servidor» (governed: the UI shows both fingerprints and
    /// asks a second confirmation before calling this).
    pub fn forget_server(&mut self) {
        let _ = self.inst.forget_host();
        self.mismatch = None;
        self.go(Screen::Server);
    }

    /// I05 · the sudo password.
    pub async fn sudo(&mut self, password: SecretText) {
        match self.inst.set_sudo_password(password).await {
            Ok(()) => {
                self.sudo_dialog = false;
                self.begin_preflight().await;
            }
            Err(_) => {
                self.field_errors
                    .insert("sudo".into(), "SUDO_REFUSED".into());
            }
        }
    }

    /// I05 · Esc.
    pub fn cancel_sudo(&mut self) {
        self.sudo_dialog = false;
        self.alert = Some(alert("PRIVILEGE_MISSING", &[]));
    }

    async fn begin_preflight(&mut self) {
        if let Err(e) = self.inst.start_bootstrap().await {
            self.alert = Some(alert("BOOTSTRAP_REFUSED", &[("detail", &format!("{e:?}"))]));
            return;
        }
        self.preflight().await;
    }

    /// I06 «Verificar novamente».
    pub async fn preflight(&mut self) {
        self.alert = None;
        match self.inst.run_preflight(None).await {
            Ok(r) => {
                if r.incomplete.is_some() {
                    self.go(Screen::Incomplete);
                }
            }
            Err(e) => {
                self.alert = Some(alert("PREFLIGHT_FAILED", &[("detail", &format!("{e:?}"))]))
            }
        }
    }

    /// I06 «Continuar».
    pub async fn continue_preflight(&mut self) {
        if !self
            .inst
            .preflight
            .as_ref()
            .is_some_and(ocinye_installer_contracts::preflight::PreflightReport::install_allowed)
        {
            return;
        }
        self.go(Screen::Hardware);
        if let Err(e) = self.inst.discover_hardware().await {
            self.alert = Some(alert(
                "GPU_DISCOVERY_ERROR",
                &[("detail", &format!("{e:?}"))],
            ));
        }
    }

    /// I07 «Continuar».
    pub fn continue_hardware(&mut self) {
        if self.inst.hardware.is_some() {
            self.go(Screen::Instance);
        }
    }

    /// I08.
    pub fn set_instance(&mut self, name: String) {
        self.draft.instance_name = name;
        self.field_errors.remove("iname");
    }

    /// I08 «Continuar».
    pub fn continue_instance(&mut self) {
        if InstanceName::parse(&self.draft.instance_name).is_err() {
            self.field_errors
                .insert("iname".into(), "INPUT_INVALID_TEXT".into());
            return;
        }
        self.go(Screen::Distributions);
    }

    /// I09 · ordered, 1..4.
    pub fn set_distributions(&mut self, ordered: Vec<String>) {
        self.draft.distributions = ordered
            .into_iter()
            .filter(|d| d.parse::<Distribution>().is_ok())
            .fold(Vec::new(), |mut acc, d| {
                if !acc.contains(&d) {
                    acc.push(d);
                }
                acc
            });
        self.field_errors.remove("dist");
    }

    /// I09 «Continuar».
    pub fn continue_distributions(&mut self) {
        if self.draft.distributions.is_empty() {
            self.field_errors
                .insert("dist".into(), "NO_DISTRIBUTION".into());
            return;
        }
        // A bound endpoint whose Distribution was disabled falls back to none.
        let enabled = self.draft.distributions.clone();
        for row in self.draft.endpoints.iter_mut().skip(1) {
            if row
                .distribution
                .as_ref()
                .is_some_and(|d| !enabled.contains(d))
            {
                row.distribution = Some(enabled[0].clone());
            }
        }
        self.go(Screen::Endpoints);
    }

    /// I10 · rows (canonical first).
    pub fn set_endpoints(&mut self, rows: Vec<EndpointRow>) {
        self.draft.endpoints = rows
            .into_iter()
            .enumerate()
            .map(|(i, mut r)| {
                r.host = r.host.trim().to_lowercase();
                if i == 0 {
                    r.distribution = None;
                }
                r.dns = "DNS_REQUIRED".into();
                r.seen.clear();
                r
            })
            .collect();
        self.field_errors.clear();
    }

    fn target_ips(&self) -> Vec<IpAddr> {
        let mut ips = Vec::new();
        if let Some(t) = &self.inst.target {
            match &t.host {
                TargetHost::V4(ip) => ips.push(IpAddr::V4(*ip)),
                TargetHost::V6(ip) => ips.push(IpAddr::V6(*ip)),
                TargetHost::Name(_) => {}
            }
        }
        if let Some(f) = &self.inst.facts {
            ips.extend(
                f.ipv4
                    .iter()
                    .chain(&f.ipv6)
                    .filter_map(|a| a.parse::<IpAddr>().ok()),
            );
        }
        ips
    }

    /// I10 «Verificar novamente» (local DNS observation).
    pub async fn check_dns(&mut self) {
        let ips = self.target_ips();
        for row in &mut self.draft.endpoints {
            if HostNameValue::parse(&row.host).is_err() {
                row.dns = "DNS_REQUIRED".into();
                continue;
            }
            let seen: Vec<IpAddr> = match self.inst.resolver.fixed.get(&row.host) {
                Some(v) => v.clone(),
                None => tokio::time::timeout(
                    std::time::Duration::from_secs(8),
                    tokio::net::lookup_host((row.host.as_str(), 443)),
                )
                .await
                .ok()
                .and_then(Result::ok)
                .map(|it| it.map(|a| a.ip()).collect())
                .unwrap_or_default(),
            };
            row.seen = seen.iter().map(ToString::to_string).collect();
            row.dns = if seen.is_empty() {
                "DNS_UNRESOLVED"
            } else if seen.iter().any(|a| ips.contains(a)) {
                "DNS_OK"
            } else {
                "DNS_WRONG_TARGET"
            }
            .into();
        }
    }

    fn endpoints(&mut self) -> Option<AccessEndpoints> {
        let mut ok = true;
        let mut canonical = None;
        let mut bound = Vec::new();
        for (i, row) in self.draft.endpoints.iter().enumerate() {
            let Ok(host) = HostNameValue::parse(&row.host) else {
                self.field_errors
                    .insert(format!("ep{i}"), "INPUT_INVALID_HOST".into());
                ok = false;
                continue;
            };
            if i == 0 {
                canonical = Some(host);
            } else {
                let Some(d) = row
                    .distribution
                    .as_deref()
                    .and_then(|d| d.parse::<Distribution>().ok())
                else {
                    self.field_errors
                        .insert(format!("ep{i}"), "ENDPOINT_DISTRIBUTION".into());
                    ok = false;
                    continue;
                };
                bound.push(BoundEndpoint {
                    host,
                    distribution: d,
                });
            }
        }
        if !ok {
            return None;
        }
        Some(AccessEndpoints {
            canonical: canonical?,
            bound,
        })
    }

    /// I10 «Continuar». A wrong DNS target blocks; pending DNS does not.
    pub fn continue_endpoints(&mut self) {
        if self.endpoints().is_none() {
            return;
        }
        if self
            .draft
            .endpoints
            .iter()
            .any(|r| r.dns == "DNS_WRONG_TARGET")
        {
            self.alert = Some(alert("DNS_WRONG_TARGET", &[]));
            return;
        }
        self.go(Screen::Tls);
    }

    /// I11 · mode and files.
    pub fn set_tls(
        &mut self,
        mode: String,
        cert: Option<String>,
        key: Option<String>,
        chain: Option<String>,
    ) {
        self.draft.tls_mode = if mode == "self" {
            "self".into()
        } else {
            "provided".into()
        };
        self.draft.tls_cert = cert;
        self.draft.tls_key = key;
        self.draft.tls_chain = chain;
        self.tls_validation = None;
        if self.draft.tls_mode == "provided" {
            if let (Some(c), Some(k), Some(eps)) = (
                self.draft.tls_cert.clone(),
                self.draft.tls_key.clone(),
                self.endpoints(),
            ) {
                let hosts: Vec<&HostNameValue> = eps.hosts();
                self.tls_validation = Some(crate::tls::validate(
                    &PathBuf::from(c),
                    &PathBuf::from(k),
                    self.draft.tls_chain.as_ref().map(PathBuf::from).as_deref(),
                    &hosts,
                    chrono::Utc::now(),
                ));
            }
        }
    }

    /// I11 «Continuar».
    pub fn continue_tls(&mut self) {
        if self.draft.tls_mode == "provided"
            && !self
                .tls_validation
                .as_ref()
                .is_some_and(TlsValidation::valid)
        {
            self.alert = Some(alert("TLS_INVALID", &[]));
            return;
        }
        self.go(Screen::Admin);
    }

    /// I12.
    pub fn set_admin(&mut self, fields: [String; 4]) {
        self.draft.admin = fields;
        self.field_errors.clear();
    }

    fn configuration(&mut self) -> Option<(InstallationConfiguration, Option<TlsFiles>)> {
        let endpoints = self.endpoints()?;
        let [pn, pe, an, ae] = self.draft.admin.clone();
        let person = PersonName::parse(&pn);
        let person_email = EmailAddress::parse(&pe);
        let admin_name = PersonName::parse(&an);
        let admin_email = EmailAddress::parse(&ae);
        for (f, ok) in [
            ("pn", person.is_ok()),
            ("pe", person_email.is_ok()),
            ("an", admin_name.is_ok()),
            ("ae", admin_email.is_ok()),
        ] {
            if !ok {
                self.field_errors.insert(f.into(), "INPUT_INVALID".into());
            }
        }
        let dists = Distributions::new(
            self.draft
                .distributions
                .iter()
                .filter_map(|d| d.parse().ok())
                .collect(),
        )
        .ok()?;
        let (tls, files) = if self.draft.tls_mode == "self" {
            (TlsPlan::SelfSignedTest, None)
        } else {
            let plan = self.tls_validation.as_ref()?.plan.clone()?;
            (
                plan,
                Some(TlsFiles {
                    cert: PathBuf::from(self.draft.tls_cert.clone()?),
                    key: PathBuf::from(self.draft.tls_key.clone()?),
                    chain: self.draft.tls_chain.clone().map(PathBuf::from),
                }),
            )
        };
        let cfg = InstallationConfiguration {
            instance_name: InstanceName::parse(&self.draft.instance_name).ok()?,
            distributions: dists,
            endpoints,
            tls,
            admin: FirstAdmin {
                person: Identity {
                    name: person.ok()?,
                    email: person_email.ok()?,
                },
                privileged: Identity {
                    name: admin_name.ok()?,
                    email: admin_email.ok()?,
                },
            },
        };
        Some((cfg, files))
    }

    /// I12 «Continuar» → I13 with a new, sealed plan.
    pub fn continue_admin(&mut self) {
        let Some((cfg, files)) = self.configuration() else {
            return;
        };
        if let Err(Refusal::Configuration(code)) = self.inst.configure(cfg, files) {
            if code.contains("AdminNotDistinct") {
                self.field_errors
                    .insert("ae".into(), "ADMIN_NOT_DISTINCT".into());
            } else {
                self.alert = Some(alert("CONFIGURATION", &[("detail", &code)]));
            }
            return;
        }
        match self.inst.review() {
            Ok(_) => self.go(Screen::Review),
            Err(e) => self.alert = Some(alert("CONFIGURATION", &[("detail", &format!("{e:?}"))])),
        }
    }

    /// I13 «Alterar» → back to a section; the draft plan is discarded.
    pub fn change(&mut self, section: &str) {
        self.inst.plan = None;
        self.go(match section {
            "release" => Screen::Release,
            "server" => Screen::Server,
            "instance" => Screen::Instance,
            "distributions" => Screen::Distributions,
            "endpoints" => Screen::Endpoints,
            "tls" => Screen::Tls,
            _ => Screen::Admin,
        });
    }

    /// «Voltar».
    pub fn back(&mut self) {
        let to = match self.screen {
            Screen::Release => Screen::Welcome,
            Screen::Server | Screen::HostTrust => Screen::Release,
            Screen::Preflight => Screen::Server,
            Screen::Hardware => Screen::Preflight,
            Screen::Instance => Screen::Hardware,
            Screen::Distributions => Screen::Instance,
            Screen::Endpoints => Screen::Distributions,
            Screen::Tls => Screen::Endpoints,
            Screen::Admin => Screen::Tls,
            Screen::Review => {
                self.inst.plan = None;
                Screen::Admin
            }
            Screen::Receipt => Screen::Complete,
            other => other,
        };
        self.go(to);
    }

    /// The outcome of an execution, mapped to screens.
    pub fn ended(&mut self, ending: Ending) {
        self.stopping = false;
        self.go(match &ending {
            Ending::Completed => Screen::Verifying,
            Ending::Interrupted => Screen::Interrupted,
            Ending::Stopped { .. } => Screen::Incomplete,
            Ending::Failed { .. } | Ending::Refused(_) => Screen::Failed,
        });
        self.ending = Some(ending);
    }

    /// After the operator-side verification.
    pub fn verified(&mut self) {
        let ok = self
            .inst
            .lifecycle
            .as_ref()
            .is_some_and(ocinye_installer_contracts::verification::LifecycleState::installed);
        if ok {
            self.go(Screen::Complete);
        } else {
            self.alert = Some(alert("VERIFICATION_FAILED", &[]));
        }
    }

    /// I17 · the credential, for the one display. The view never carries it;
    /// this returns it once, and «Já a guardei» drops it.
    #[must_use]
    pub fn reveal_credential(&self) -> Option<String> {
        if self.credential_acknowledged {
            return None;
        }
        self.inst
            .secret
            .as_ref()
            .map(|s| s.value.expose().to_owned())
    }

    /// I17 «Já a guardei».
    pub fn acknowledge_credential(&mut self) {
        self.inst.acknowledge_credential();
        self.credential_acknowledged = true;
        self.go(Screen::Complete);
    }

    /// The view the renderer draws.
    #[must_use]
    pub fn view(&self, live: &Value) -> Value {
        let i = &self.inst;
        let release = i.release.as_ref().map(|r| {
            json!({
                "id": r.manifest.release.id,
                "commit": r.manifest.release.commit,
                "build": r.manifest.release.build,
                "arch": r.manifest.target.arch,
                "migrations": { "count": r.manifest.migrations.count, "latest": r.manifest.migrations.latest },
                "artifacts": r.manifest.artifacts.iter().map(|a| a.path.clone()).collect::<Vec<_>>(),
                "files": r.files.len(),
                "package": r.dir.file_name().map(|n| n.to_string_lossy().into_owned()),
            })
        });
        json!({
            "screen": self.screen,
            "lang": self.lang,
            "alert": self.alert,
            "field_errors": self.field_errors,
            "draft": self.draft,
            "sudo_dialog": self.sudo_dialog,
            "presented_key": self.presented_key,
            "pinned_key": i.host_key,
            "mismatch": self.mismatch,
            "release": release,
            "probe": i.probe,
            "elevation": i.elevation.map(|e| format!("{e:?}")),
            "facts": i.facts,
            "preflight": i.preflight,
            "hardware": i.hardware,
            "plan": i.plan,
            "tls_validation": self.tls_validation,
            "ending": self.ending,
            "stopping": self.stopping,
            "events": i.events.iter().rev().take(200).collect::<Vec<_>>(),
            "verification": i.report,
            "lifecycle": i.lifecycle,
            "journal": i.journal,
            "credential": i.secret.as_ref().map(|s| json!({ "user": s.user, "expires_at": s.expires_at })),
            "credential_acknowledged": self.credential_acknowledged,
            "receipt": i.receipt(),
            "live": live,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> UiSession {
        UiSession::new(Installer::new(
            std::env::temp_dir().join(format!("ocinye-ui-{}", std::process::id())),
        ))
    }

    #[test]
    fn os_ecras_seguem_por_ordem_e_nada_salta() {
        let mut s = session();
        s.start();
        assert_eq!(s.screen, Screen::Release);
        s.continue_release();
        assert_eq!(
            s.screen,
            Screen::Release,
            "sem pacote verificado não se avança"
        );
        let dir = crate::bundle::tests::fixture("ui");
        s.choose_release(dir.clone());
        assert!(s.alert.is_none());
        s.continue_release();
        assert_eq!(s.screen, Screen::Server);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn um_pacote_alterado_mostra_i20_com_o_ficheiro() {
        let mut s = session();
        let dir = crate::bundle::tests::fixture("ui-bad");
        std::fs::write(dir.join("images/ocinye-worker.tar"), b"x").unwrap();
        s.choose_release(dir.clone());
        let a = s.alert.clone().unwrap();
        assert_eq!(a.code, "RELEASE_CHECKSUM_FAILED");
        assert_eq!(a.params["file"], "images/ocinye-worker.tar");
        s.continue_release();
        assert_eq!(s.screen, Screen::Welcome, "um pacote recusado não se usa");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn um_endereco_com_um_fragmento_de_shell_e_recusado_no_campo() {
        let mut s = session();
        s.set_server(
            "srv-01.empresa.test; rm".into(),
            "22".into(),
            "operador".into(),
            AuthKind::Agent,
            None,
        );
        assert!(s.target().is_none());
        assert_eq!(s.field_errors["host"], "INPUT_INVALID_HOST");
    }

    #[test]
    fn nenhuma_distribuicao_bloqueia_e_a_ordem_conta() {
        let mut s = session();
        s.set_distributions(vec![]);
        s.continue_distributions();
        assert_eq!(s.field_errors["dist"], "NO_DISTRIBUTION");
        s.set_distributions(vec![
            "business".into(),
            "research".into(),
            "business".into(),
            "x".into(),
        ]);
        assert_eq!(s.draft.distributions, ["business", "research"]);
    }

    #[test]
    fn o_canonico_nunca_fica_fixo() {
        let mut s = session();
        s.set_endpoints(vec![EndpointRow {
            host: "OS.Empresa.test".into(),
            distribution: Some("business".into()),
            dns: String::new(),
            seen: vec![],
        }]);
        assert_eq!(s.draft.endpoints[0].distribution, None);
        assert_eq!(s.draft.endpoints[0].host, "os.empresa.test");
    }

    #[test]
    fn a_vista_nunca_leva_o_segredo() {
        let mut s = session();
        s.inst.secret = Some(ocinye_installer_contracts::protocol::OneTimeSecret {
            user: "admin@x.test".into(),
            expires_at: "t".into(),
            value: SecretText::new("Segredo-123".into()),
        });
        let v = s.view(&Value::Null).to_string();
        assert!(!v.contains("Segredo-123"));
        assert!(v.contains("admin@x.test"));
    }
}
