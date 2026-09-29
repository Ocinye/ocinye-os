//! O Desktop (`GET /`): a disposição do membro e os dados de cada widget.
//!
//! A hierarquia da disposição:
//!
//! ```text
//! predefinição do sistema, por Distribuição (registry::system_default, Design)
//!   ↓ predefinição da Instância publicada pela administração (FG-014: ainda não existe)
//!   ↓ disposição do membro (GET/PUT /api/v1/me/desktop)
//! ```
//!
//! Cada widget pede os seus dados ao Core, com o token do membro; o Core decide
//! o que devolve. O que o Core não tem ainda (avisos institucionais, registo de
//! cópias de segurança) diz-se `Unavailable`, e não vazio: «não há avisos» seria
//! uma afirmação que ninguém fez.

use chrono::{DateTime, Utc};
use serde_json::Value;

use super::{reference, Caller, ShellContext, SYSTEM_DIM, SYSTEM_WALLPAPER};
use crate::api::ApiFailure;
use crate::i18n::{t, tp};
use crate::ui::screens::home::registry::{self, KPIS};
use crate::ui::view_models::{
    Ago, Backup, ContinueItem, ContinueKind, CoreError, Count, DefaultSource, DeskWidget,
    DesktopDefault, DesktopVm, Distribution, HealthVm, Load, Metric, PlacedWidget, StorageUse,
    WidgetContent, WidgetItem, WidgetKind,
};
use crate::WorkspaceState;

/// A versão da predefinição do sistema. Sobe quando o Design mudar
/// `registry::system_default`.
pub const SYSTEM_DEFAULT_VERSION: u32 = 1;

/// Quantos itens mostra uma lista de widget.
const LIST_LIMIT: usize = 5;

/// «Continuar trabalho»: até 7 itens, até 30 dias (spec-desktop-widgets).
const CONTINUE_LIMIT: usize = 7;
const CONTINUE_MAX_AGE_SECS: i64 = 30 * 86_400;

/// A predefinição do sistema para a Distribuição (`DefaultSource::System`).
///
/// Ninguém a publicou: não tem nome dado pela administração nem data, e a
/// folha «Repor predefinição» diz que vem com o Ocinye OS (D001.1). Quando a
/// administração publicar uma (FG-014), essa é `DefaultSource::Instance`.
#[must_use]
pub fn system_default(d: Distribution) -> DesktopDefault {
    DesktopDefault {
        source: DefaultSource::System,
        name: String::new(),
        version: SYSTEM_DEFAULT_VERSION,
        published: String::new(),
        wallpaper: SYSTEM_WALLPAPER,
        dim: SYSTEM_DIM,
        widgets: registry::system_default(d),
    }
}

/// A disposição gravada, lida do JSON do Core. Tipos que o registo já não
/// conhece caem; o resto fica pela ordem gravada.
fn placed_from(layout: &Value) -> Vec<PlacedWidget> {
    layout
        .get("widgets")
        .and_then(Value::as_array)
        .map(|ws| {
            ws.iter()
                .filter_map(|w| {
                    let kind = WidgetKind::parse(w.get("kind")?.as_str()?)?;
                    let size = |k: &str| {
                        w.get(k)
                            .and_then(Value::as_u64)
                            .and_then(|n| u8::try_from(n).ok())
                    };
                    Some(PlacedWidget {
                        id: kind.as_str().to_owned(),
                        kind,
                        w: size("w")?,
                        h: size("h")?,
                        minimized: w.get("minimized").and_then(Value::as_bool).unwrap_or(false),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn failed(detail: &str) -> CoreError {
    CoreError {
        reference: reference(detail),
    }
}

/// Uma recusa do Core no estado que a vista desenha.
pub(crate) fn load_of<T>(what: &str, failure: &ApiFailure) -> Load<T> {
    match failure {
        ApiFailure::Forbidden | ApiFailure::Denied => Load::Denied,
        ApiFailure::Unavailable(_) => Load::Unavailable,
        ApiFailure::ApplicationInactive => Load::Inactive,
        other => Load::Failed(failed(&format!("{what}: {other}"))),
    }
}

pub(crate) fn list<T>(items: Vec<T>) -> Load<Vec<T>> {
    if items.is_empty() {
        Load::Empty
    } else {
        Load::Ready(items)
    }
}

pub(crate) fn items(v: &Value) -> &[Value] {
    v.get("items")
        .or_else(|| v.get("files"))
        .and_then(Value::as_array)
        .or_else(|| v.as_array())
        .map_or(&[], Vec::as_slice)
}

pub(crate) fn text<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or_default()
}

pub(crate) fn instant(v: &Value, k: &str) -> Option<DateTime<Utc>> {
    v.get(k)
        .and_then(Value::as_str)
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.with_timezone(&Utc))
}

/// O que os widgets partilham: o relógio e a zona do membro, e o que a casca
/// já apurou sobre o Core e sobre a autoridade de quem vê.
pub(crate) struct Clock {
    pub(crate) now: DateTime<Utc>,
    pub(crate) zone: ocinye_contracts::temporal::TimeZoneName,
    pub(crate) core_ok: bool,
    pub(crate) is_admin: bool,
}

impl Clock {
    pub(crate) fn hhmm(&self, at: DateTime<Utc>) -> String {
        at.with_timezone(&self.zone.zone())
            .format("%H:%M")
            .to_string()
    }
    pub(crate) fn ddmm(&self, at: DateTime<Utc>) -> String {
        at.with_timezone(&self.zone.zone())
            .format("%d/%m")
            .to_string()
    }
    pub(crate) fn is_today(&self, at: DateTime<Utc>) -> bool {
        let z = self.zone.zone();
        at.with_timezone(&z).date_naive() == self.now.with_timezone(&z).date_naive()
    }
    /// «14:05» hoje, «22/09» nos outros dias.
    pub(crate) fn when(&self, at: DateTime<Utc>) -> String {
        if self.is_today(at) {
            self.hhmm(at)
        } else {
            self.ddmm(at)
        }
    }
    pub(crate) fn ago(&self, at: DateTime<Utc>) -> Ago {
        let secs = u64::try_from((self.now - at).num_seconds().max(0)).unwrap_or(0);
        Ago::from_secs(secs, || self.ddmm(at))
    }
    /// «há 5 min», «ontem», «22/09» (chaves `time.*` do Design).
    pub(crate) fn relative(&self, at: DateTime<Utc>) -> String {
        let secs = (self.now - at).num_seconds().max(0);
        match secs {
            0..60 => t("time.now").to_owned(),
            60..3_600 => tp("time.minutes", secs / 60),
            3_600..86_400 => tp("time.hours", secs / 3_600),
            86_400..172_800 => t("time.yesterday").to_owned(),
            172_800..604_800 => tp("time.days", secs / 86_400),
            _ => self.ddmm(at),
        }
    }
}

/// «12,4 GB», com a vírgula ou o ponto do idioma.
pub(crate) fn bytes(n: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    #[allow(clippy::cast_precision_loss, reason = "uma apresentação arredondada")]
    let mut v = n as f64;
    let mut u = 0;
    while v >= 1024.0 && u < UNITS.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    let s = if u == 0 || v >= 100.0 {
        format!("{v:.0}")
    } else {
        format!("{v:.1}")
    };
    let s = if crate::i18n::current() == ocinye_contracts::Locale::En {
        s
    } else {
        s.replace('.', ",")
    };
    format!("{s} {}", UNITS[u])
}

fn extension(name: &str) -> String {
    name.rsplit_once('.')
        .map(|(_, e)| e.to_uppercase())
        .filter(|e| e.len() <= 5)
        .unwrap_or_default()
}

// ── Widgets ──────────────────────────────────────────────────────────────────

async fn total(caller: &Caller<'_>, state: &WorkspaceState, path: &str) -> Result<i64, ApiFailure> {
    let v = caller.get(state, path).await?;
    Ok(v.get("total").and_then(Value::as_i64).unwrap_or(0))
}

async fn active_units(caller: &Caller<'_>, state: &WorkspaceState) -> Result<i64, ApiFailure> {
    let v = caller.get(state, "/api/v1/units").await?;
    Ok(i64::try_from(
        items(&v)
            .iter()
            .filter(|u| text(u, "status") == "active")
            .count(),
    )
    .unwrap_or(i64::MAX))
}

const IDEAS_IN_PROGRESS: &str = "/api/v1/workspaces?kind=idea&in_progress=true&page_size=1";
const PROJECTS_IN_PROGRESS: &str = "/api/v1/workspaces?kind=project&in_progress=true&page_size=1";
const DATASETS: &str = "/api/v1/datasets?page_size=1";

fn count(n: i64, qualifier_key: &str) -> Count {
    Count {
        value: n.to_string(),
        qualifier: tp(qualifier_key, n),
    }
}

async fn kpis(caller: &Caller<'_>, state: &WorkspaceState) -> WidgetContent {
    let (units, ideas, projects, datasets) = tokio::join!(
        active_units(caller, state),
        total(caller, state, IDEAS_IN_PROGRESS),
        total(caller, state, PROJECTS_IN_PROGRESS),
        total(caller, state, DATASETS),
    );
    let mut metrics = Vec::with_capacity(KPIS.len());
    for (&(label, qualifier, icon, href), n) in KPIS.iter().zip([units, ideas, projects, datasets])
    {
        match n {
            Ok(n) => metrics.push(Metric {
                icon,
                label: t(label).to_owned(),
                value: n.to_string(),
                qualifier: tp(qualifier, n),
                href: href.to_owned(),
            }),
            Err(f) => return WidgetContent::Metrics(load_of("kpis", &f)),
        }
    }
    WidgetContent::Metrics(Load::Ready(metrics))
}

async fn tasks(caller: &Caller<'_>, state: &WorkspaceState) -> WidgetContent {
    let path = format!("/api/v1/tasks?mine=true&open_only=true&page_size={LIST_LIMIT}");
    WidgetContent::List(match caller.get(state, &path).await {
        Ok(v) => list(
            items(&v)
                .iter()
                .map(|task| WidgetItem {
                    title: text(task, "title").to_owned(),
                    // `due_on` é uma data civil (AAAA-MM-DD): mostra-se «dd/mm».
                    meta: task
                        .get("due_on")
                        .and_then(Value::as_str)
                        .and_then(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
                        .map_or_else(|| "—".to_owned(), |d| d.format("%d/%m").to_string()),
                    href: format!("/tasks/{}", text(task, "id")),
                })
                .collect(),
        ),
        Err(f) => load_of("tasks", &f),
    })
}

async fn calendar(caller: &Caller<'_>, state: &WorkspaceState, clock: &Clock) -> WidgetContent {
    WidgetContent::List(agenda_today(caller, state, clock, LIST_LIMIT).await)
}

/// Os eventos de hoje no fuso do membro, sem os cancelados: o widget do
/// Calendário e o painel do relógio (D002) mostram a mesma agenda.
pub(crate) async fn agenda_today(
    caller: &Caller<'_>,
    state: &WorkspaceState,
    clock: &Clock,
    limit: usize,
) -> Load<Vec<WidgetItem>> {
    let z = clock.zone.zone();
    let today = clock.now.with_timezone(&z).date_naive();
    let start = ocinye_contracts::temporal::resolve_local(
        today.and_hms_opt(0, 0, 0).unwrap_or_default(),
        clock.zone,
    )
    .unwrap_or(clock.now);
    let end = start + chrono::Duration::days(1);
    let path = format!(
        "/api/v1/calendar/agenda?from={}&to={}",
        start.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        end.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
    );
    match caller.get(state, &path).await {
        Ok(v) => list(
            items(&v)
                .iter()
                .filter(|e| text(e, "state") != "cancelled")
                .take(limit)
                .map(|e| WidgetItem {
                    title: text(e, "title").to_owned(),
                    meta: instant(e, "starts_at")
                        .filter(|_| e.get("all_day").and_then(Value::as_bool) != Some(true))
                        .map(|at| clock.hhmm(at))
                        .unwrap_or_default(),
                    href: format!("/calendar/events/{}", text(e, "id")),
                })
                .collect(),
        ),
        Err(f) => load_of("calendar", &f),
    }
}

async fn notes(caller: &Caller<'_>, state: &WorkspaceState, clock: &Clock) -> WidgetContent {
    let path = format!("/api/v1/me/notes?page_size={LIST_LIMIT}");
    WidgetContent::List(match caller.get(state, &path).await {
        Ok(v) => list(
            items(&v)
                .iter()
                .map(|n| WidgetItem {
                    title: text(n, "title").to_owned(),
                    meta: instant(n, "updated_at")
                        .map(|at| clock.when(at))
                        .unwrap_or_default(),
                    href: format!("/notes/{}", text(n, "id")),
                })
                .collect(),
        ),
        Err(f) => load_of("notes", &f),
    })
}

async fn files(caller: &Caller<'_>, state: &WorkspaceState) -> WidgetContent {
    let path = format!("/api/v1/me/files?view=recents&limit={LIST_LIMIT}");
    WidgetContent::List(match caller.get(state, &path).await {
        Ok(v) => list(
            items(&v)
                .iter()
                .map(|f| {
                    let name = text(f, "name");
                    let size = f.get("size_bytes").and_then(Value::as_u64).map(bytes);
                    let meta = match (extension(name), size) {
                        (e, Some(s)) if !e.is_empty() => format!("{e} · {s}"),
                        (_, Some(s)) => s,
                        (e, None) => e,
                    };
                    // Os ficheiros pessoais abrem no explorador (HANDOFF §2.5).
                    WidgetItem {
                        title: name.to_owned(),
                        meta,
                        href: "/files".to_owned(),
                    }
                })
                .collect(),
        ),
        Err(f) => load_of("files", &f),
    })
}

async fn storage(caller: &Caller<'_>, state: &WorkspaceState) -> WidgetContent {
    WidgetContent::Storage(
        match caller
            .get(state, "/api/v1/me/files?view=recents&limit=1")
            .await
        {
            Ok(v) => {
                let s = v.get("storage");
                let used = s.and_then(|s| s.get("used_bytes")).and_then(Value::as_u64);
                let limit = s.and_then(|s| s.get("limit_bytes")).and_then(Value::as_u64);
                match (used, limit) {
                    (Some(used), Some(limit)) if limit > 0 => Load::Ready(StorageUse {
                        used: bytes(used),
                        total: bytes(limit),
                        percent: u8::try_from((used.saturating_mul(100) / limit).min(100))
                            .unwrap_or(100),
                    }),
                    // Sem quota atribuída, não há percentagem a mostrar.
                    _ => Load::Unavailable,
                }
            }
            Err(f) => load_of("storage", &f),
        },
    )
}

async fn mail(caller: &Caller<'_>, state: &WorkspaceState, clock: &Clock) -> WidgetContent {
    let status = match caller.get(state, "/api/v1/mail/status").await {
        Ok(s) => s,
        Err(f) => return WidgetContent::List(load_of("mail status", &f)),
    };
    let ligada = status.get("mailbox_linked").and_then(Value::as_bool) == Some(true);
    if !ligada || status.get("can_read").and_then(Value::as_bool) != Some(true) {
        // Correio por configurar ou caixa por ligar.
        return WidgetContent::List(Load::Unavailable);
    }
    let caixas = match caller.get(state, "/api/v1/mail/mailboxes").await {
        Ok(c) => c,
        Err(f) => return WidgetContent::List(load_of("mailboxes", &f)),
    };
    let Some(caixa) = items(&caixas)
        .iter()
        .find(|c| c.get("connected").and_then(Value::as_bool) == Some(true))
    else {
        return WidgetContent::List(Load::Unavailable);
    };
    let path = format!(
        "/api/v1/mail/mailboxes/{}/messages?folder=inbox",
        text(caixa, "id")
    );
    WidgetContent::List(match caller.get(state, &path).await {
        Ok(v) => list(
            items(&v)
                .iter()
                .filter(|m| m.get("is_read").and_then(Value::as_bool) == Some(false))
                .take(LIST_LIMIT)
                .map(|m| {
                    let from = Some(text(m, "from_display_name"))
                        .filter(|s| !s.is_empty())
                        .unwrap_or_else(|| text(m, "from_address"));
                    let when = instant(m, "sent_at")
                        .map(|at| clock.when(at))
                        .unwrap_or_default();
                    WidgetItem {
                        title: text(m, "subject").to_owned(),
                        meta: format!("{from} · {when}"),
                        href: format!("/mail/message/{}", text(m, "id")),
                    }
                })
                .collect(),
        ),
        Err(f) => load_of("mail messages", &f),
    })
}

/// Para onde leva um acontecimento da Actividade: o objecto, quando o Workspace
/// tem rota para ele; a Actividade, quando não.
fn activity_href(e: &Value) -> String {
    let id = text(e, "subject_id");
    match text(e, "subject_type") {
        "task" if !id.is_empty() => format!("/tasks/{id}"),
        "dataset" if !id.is_empty() => format!("/datasets/{id}"),
        "workspace" | "idea" | "project" if !text(e, "workspace_id").is_empty() => {
            format!("/workspaces/{}", text(e, "workspace_id"))
        }
        _ => "/activity".to_owned(),
    }
}

async fn activity(caller: &Caller<'_>, state: &WorkspaceState, clock: &Clock) -> WidgetContent {
    let path = format!("/api/v1/activity?page_size={LIST_LIMIT}");
    WidgetContent::List(match caller.get(state, &path).await {
        Ok(v) => list(
            items(&v)
                .iter()
                .take(LIST_LIMIT)
                .map(|e| WidgetItem {
                    title: text(e, "summary").to_owned(),
                    meta: instant(e, "created_at")
                        .map(|at| clock.relative(at))
                        .unwrap_or_default(),
                    href: activity_href(e),
                })
                .collect(),
        ),
        Err(f) => load_of("activity", &f),
    })
}

async fn projects(caller: &Caller<'_>, state: &WorkspaceState, columns: u8) -> WidgetContent {
    if columns == 1 {
        return WidgetContent::Count(match total(caller, state, PROJECTS_IN_PROGRESS).await {
            Ok(n) => Load::Ready(count(n, "desk.kpi.projects_q")),
            Err(f) => load_of("projects count", &f),
        });
    }
    let path = format!("/api/v1/workspaces?kind=project&mine=true&page_size={LIST_LIMIT}");
    WidgetContent::List(match caller.get(state, &path).await {
        Ok(v) => list(
            items(&v)
                .iter()
                .map(|p| WidgetItem {
                    title: text(p, "title").to_owned(),
                    meta: text(p, "code").to_owned(),
                    href: format!("/workspaces/{}", text(p, "id")),
                })
                .collect(),
        ),
        Err(f) => load_of("projects", &f),
    })
}

async fn counter(
    caller: &Caller<'_>,
    state: &WorkspaceState,
    path: &str,
    qualifier: &str,
) -> WidgetContent {
    WidgetContent::Count(match total(caller, state, path).await {
        Ok(n) => Load::Ready(count(n, qualifier)),
        Err(f) => load_of(path, &f),
    })
}

/// «Continuar trabalho» a partir do que o Core já regista: as notas e os
/// ficheiros pessoais que o próprio membro alterou. Ideias, projectos e
/// datasets entram quando houver o registo de abertura/escrita por membro
/// (FUNCTIONAL_GAPS FG-015, `CORE_CONTRACT_REQUIRED`).
async fn continue_working(
    caller: &Caller<'_>,
    state: &WorkspaceState,
    clock: &Clock,
) -> WidgetContent {
    let notes_path = format!("/api/v1/me/notes?page_size={CONTINUE_LIMIT}");
    let files_path = format!("/api/v1/me/files?view=recents&limit={CONTINUE_LIMIT}");
    let (notes, files) = tokio::join!(
        caller.get(state, &notes_path),
        caller.get(state, &files_path),
    );
    let (notes, files) = match (notes, files) {
        (Ok(n), Ok(f)) => (n, f),
        (Err(f), _) | (_, Err(f)) => return WidgetContent::Continue(load_of("continue", &f)),
    };
    let mut all: Vec<(DateTime<Utc>, ContinueItem)> = Vec::new();
    for n in items(&notes) {
        if let Some(at) = instant(n, "updated_at") {
            all.push((
                at,
                ContinueItem {
                    kind: ContinueKind::Note,
                    title: text(n, "title").to_owned(),
                    href: format!("/notes/{}", text(n, "id")),
                    progress: None,
                    when: clock.ago(at),
                },
            ));
        }
    }
    for f in items(&files) {
        if let Some(at) = instant(f, "updated_at") {
            all.push((
                at,
                ContinueItem {
                    kind: ContinueKind::File,
                    title: text(f, "name").to_owned(),
                    href: "/files".to_owned(),
                    progress: None,
                    when: clock.ago(at),
                },
            ));
        }
    }
    all.retain(|(at, _)| (clock.now - *at).num_seconds() <= CONTINUE_MAX_AGE_SECS);
    all.sort_by_key(|a| std::cmp::Reverse(a.0));
    WidgetContent::Continue(list(
        all.into_iter()
            .take(CONTINUE_LIMIT)
            .map(|(_, i)| i)
            .collect(),
    ))
}

/// «Estado do sistema»: o Core pela sonda ao `/ready`, os nós de computação
/// pelo batimento, e a cópia de segurança como `Unknown` — o Core ainda não
/// regista cópias (FG-016), e afirmar êxito, falha ou hora seria inventar. A
/// ausência de registo não degrada o estado (`backup_fresh = true`, D001.1).
/// Só quem administra recebe a ligação ao Monitor.
async fn health(caller: &Caller<'_>, state: &WorkspaceState, clock: &Clock) -> WidgetContent {
    WidgetContent::Health(match caller.get(state, "/api/v1/compute/status").await {
        Ok(v) => {
            let n = |k: &str| {
                v.get(k)
                    .and_then(Value::as_u64)
                    .and_then(|n| u16::try_from(n).ok())
                    .unwrap_or(0)
            };
            let (nodes_up, nodes_total) = (n("online_nodes"), n("registered_nodes"));
            Load::Ready(HealthVm {
                state: HealthVm::derive_state(clock.core_ok, nodes_up, nodes_total, true),
                nodes_up,
                nodes_total,
                backup: Backup::Unknown,
                admin_href: clock.is_admin.then(|| "/admin/monitor".to_owned()),
            })
        }
        Err(f) => load_of("compute status", &f),
    })
}

/// Os dados de um widget, pela forma que o seu tipo desenha.
async fn content(
    placed: &PlacedWidget,
    caller: &Caller<'_>,
    state: &WorkspaceState,
    clock: &Clock,
) -> WidgetContent {
    match placed.kind {
        WidgetKind::Kpis => kpis(caller, state).await,
        // Os avisos institucionais ainda não existem no Core (FG-013).
        WidgetKind::Notice => WidgetContent::List(Load::Unavailable),
        WidgetKind::Continue => continue_working(caller, state, clock).await,
        WidgetKind::Tasks => tasks(caller, state).await,
        WidgetKind::Calendar => calendar(caller, state, clock).await,
        WidgetKind::Notes => notes(caller, state, clock).await,
        WidgetKind::Files => files(caller, state).await,
        WidgetKind::Mail => mail(caller, state, clock).await,
        WidgetKind::Activity => activity(caller, state, clock).await,
        WidgetKind::Projects => projects(caller, state, placed.w).await,
        WidgetKind::Ideas => counter(caller, state, IDEAS_IN_PROGRESS, "desk.kpi.ideas_q").await,
        WidgetKind::Datasets => counter(caller, state, DATASETS, "desk.kpi.datasets_q").await,
        WidgetKind::Storage => storage(caller, state).await,
        WidgetKind::Health => health(caller, state, clock).await,
    }
}

/// `GET /`: o Desktop do membro, com os dados de cada widget.
pub async fn desktop(ctx: ShellContext, caller: &Caller<'_>, state: &WorkspaceState) -> DesktopVm {
    // Sem Distribuição (o Core não respondeu a `/organisation`), a predefinição
    // é a da porta; e sem essa, a de investigação — o único caminho que não
    // deixa o Desktop vazio. Com o Core a responder, nunca se chega aqui.
    let distribution = match ctx.distribution {
        Some(d) => d,
        None => crate::api::instance_door(state)
            .await
            .and_then(|(_, p)| p)
            .and_then(|p| super::distribution_of(p.as_str()))
            .unwrap_or(Distribution::Research),
    };
    let default = system_default(distribution);
    let (version, placed, can_customise) = match &ctx.desktop {
        Ok(d) => {
            let version = d
                .get("version")
                .and_then(Value::as_u64)
                .and_then(|v| u32::try_from(v).ok())
                .unwrap_or(0);
            let placed = d
                .get("layout")
                .filter(|l| !l.is_null())
                .map_or_else(|| default.widgets.clone(), placed_from);
            let can = d.get("can_customise").and_then(Value::as_bool) != Some(false);
            (version, placed, can)
        }
        Err(_) => (0, default.widgets.clone(), true),
    };

    let clock = Clock {
        now: Utc::now(),
        zone: ctx.zone,
        core_ok: ctx.core.operational(),
        is_admin: ctx.is_admin,
    };
    // Um futuro por tipo, todos em paralelo: um Desktop tem no máximo um de cada.
    let find = |k: WidgetKind| placed.iter().find(|p| p.kind == k);
    macro_rules! slot {
        ($k:expr) => {
            async {
                match find($k) {
                    Some(p) => Some(content(p, caller, state, &clock).await),
                    None => None,
                }
            }
        };
    }
    let fetched = tokio::join!(
        slot!(WidgetKind::Kpis),
        slot!(WidgetKind::Notice),
        slot!(WidgetKind::Continue),
        slot!(WidgetKind::Tasks),
        slot!(WidgetKind::Calendar),
        slot!(WidgetKind::Notes),
        slot!(WidgetKind::Files),
        slot!(WidgetKind::Mail),
        slot!(WidgetKind::Activity),
        slot!(WidgetKind::Projects),
        slot!(WidgetKind::Ideas),
        slot!(WidgetKind::Datasets),
        slot!(WidgetKind::Storage),
        slot!(WidgetKind::Health),
    );
    let mut by_kind: Vec<(WidgetKind, WidgetContent)> = [
        (WidgetKind::Kpis, fetched.0),
        (WidgetKind::Notice, fetched.1),
        (WidgetKind::Continue, fetched.2),
        (WidgetKind::Tasks, fetched.3),
        (WidgetKind::Calendar, fetched.4),
        (WidgetKind::Notes, fetched.5),
        (WidgetKind::Files, fetched.6),
        (WidgetKind::Mail, fetched.7),
        (WidgetKind::Activity, fetched.8),
        (WidgetKind::Projects, fetched.9),
        (WidgetKind::Ideas, fetched.10),
        (WidgetKind::Datasets, fetched.11),
        (WidgetKind::Storage, fetched.12),
        (WidgetKind::Health, fetched.13),
    ]
    .into_iter()
    .filter_map(|(k, c)| c.map(|c| (k, c)))
    .collect();

    let widgets = placed
        .into_iter()
        .filter_map(|p| {
            let i = by_kind.iter().position(|(k, _)| *k == p.kind)?;
            let (_, content) = by_kind.swap_remove(i);
            Some(DeskWidget { placed: p, content })
        })
        .collect();

    DesktopVm {
        shell: ctx.vm,
        version,
        widgets,
        // O Core não guarda de que predefinição veio a disposição, e com a do
        // sistema a vista já não anuncia «nova predefinição» (D001.1).
        base_version: None,
        default: Some(default),
        is_admin: ctx.is_admin,
        can_customise,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::view_models::Wallpaper;

    #[test]
    fn a_predefinicao_do_sistema_nao_anuncia_uma_publicacao_da_administracao() {
        let d = system_default(Distribution::Business);
        assert_eq!(d.source, DefaultSource::System);
        assert!(
            d.name.is_empty() && d.published.is_empty(),
            "uma data que ninguém publicou"
        );
        assert_eq!(d.version, SYSTEM_DEFAULT_VERSION);
        assert_eq!(d.widgets, registry::system_default(Distribution::Business));
        assert_eq!((d.wallpaper, d.dim), (Wallpaper::Ocinye, 20));
    }

    #[test]
    fn a_disposicao_gravada_le_se_pela_ordem_e_ignora_tipos_desconhecidos() {
        let v = serde_json::json!({"widgets": [
            {"id": "tasks", "kind": "tasks", "w": 1, "h": 2},
            {"id": "nye", "kind": "nye", "w": 1, "h": 1},
            {"id": "notice", "kind": "notice", "w": 2, "h": 1, "minimized": true}
        ]});
        let p = placed_from(&v);
        assert_eq!(p.len(), 2);
        assert_eq!(
            (p[0].kind, p[1].kind, p[1].minimized),
            (WidgetKind::Tasks, WidgetKind::Notice, true)
        );
    }

    #[test]
    fn os_bytes_dizem_se_na_unidade_certa() {
        assert_eq!(bytes(512), "512 B");
        assert!(bytes(155 * 1024 * 1024 * 1024).starts_with("155"));
        assert!(bytes(12 * 1024 * 1024 * 1024 + 400 * 1024 * 1024).contains("12"));
    }

    /// A tabela de regras do Core (`ocinye_contracts::desktop`) é a mesma do
    /// registo do Design: se o Design mudar um tamanho ou um obrigatório, isto
    /// falha até o Core seguir — senão o Core recusaria disposições que a
    /// interface oferece, ou aceitaria as que ela proíbe.
    #[test]
    fn as_regras_do_core_sao_as_do_registo_do_design() {
        use ocinye_contracts::desktop::WIDGET_KINDS;
        assert_eq!(WIDGET_KINDS.len(), registry::KINDS.len());
        for spec in registry::KINDS {
            let rule = ocinye_contracts::desktop::kind_rule(spec.kind.as_str())
                .unwrap_or_else(|| panic!("{} falta no Core", spec.kind.as_str()));
            assert_eq!(rule.sizes, spec.sizes, "{}", rule.id);
            assert_eq!(rule.mandatory, spec.mandatory, "{}", rule.id);
            assert_eq!(rule.admin_only, spec.admin_only, "{}", rule.id);
        }
        let fundos: Vec<&str> = Wallpaper::ALL.iter().map(|w| w.as_str()).collect();
        assert_eq!(fundos, ocinye_contracts::desktop::WALLPAPERS);
    }

    /// E todas as predefinições do sistema passam a validação do Core.
    #[test]
    fn as_predefinicoes_do_sistema_passam_a_validacao_do_core() {
        for d in [
            Distribution::Research,
            Distribution::Business,
            Distribution::Personal,
            Distribution::Education,
        ] {
            let def = system_default(d);
            let layout = ocinye_contracts::desktop::DesktopLayout {
                wallpaper: def.wallpaper.as_str().to_owned(),
                fit: "fill".to_owned(),
                dim: def.dim,
                widgets: def
                    .widgets
                    .iter()
                    .map(|p| ocinye_contracts::desktop::PlacedWidget {
                        id: p.id.clone(),
                        kind: p.kind.as_str().to_owned(),
                        w: p.w,
                        h: p.h,
                        minimized: p.minimized,
                    })
                    .collect(),
            };
            assert_eq!(layout.validate(false), Ok(()), "{d:?}");
        }
    }
}
