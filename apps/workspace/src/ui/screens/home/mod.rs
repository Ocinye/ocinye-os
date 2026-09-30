//! O Desktop (D16–D18, G-02/03/04). DESIGN_LOCKED.
//!
//! Grelha de 4 colunas; a ordem dos widgets é a posição e cada widget ocupa
//! um dos tamanhos que o seu tipo permite (`registry::KINDS`). «Personalizar»
//! (JS, `static/oc-desk.js`) move, redimensiona, retira e acrescenta widgets,
//! muda o fundo e o escurecimento, e grava com `PUT /me/desktop`. Repor a
//! predefinição é um `POST /me/desktop/restore` (funciona sem JS).
//! Sem JavaScript o Desktop mostra-se inteiro; só a personalização precisa dele.

use leptos::prelude::*;

pub mod registry;

use crate::i18n::{t, tf, tp};
use crate::ui::components::{core_error, icon};
use crate::ui::shell::shell;
use crate::ui::view_models::{
    Ago, Backup, ContinueItem, DefaultSource, DeskWidget, DesktopDefault, DesktopVm, Health,
    HealthVm, Load, Wallpaper, WidgetContent, WidgetItem, WidgetKind,
};
use registry::{diff, spec, Category, DeskDiff, KINDS};

fn wall_key(w: Wallpaper) -> &'static str {
    match w {
        Wallpaper::Ocinye => "desk.wall.ocinye",
        Wallpaper::Dusk => "desk.wall.dusk",
        Wallpaper::Institutional => "desk.wall.org",
        Wallpaper::Mist => "desk.wall.mist",
        Wallpaper::Slate => "desk.wall.slate",
        Wallpaper::Sand => "desk.wall.sand",
        Wallpaper::Field => "desk.wall.field",
        Wallpaper::Module => "desk.wall.module",
        Wallpaper::Calm => "desk.wall.calm",
        Wallpaper::Lattice => "desk.wall.lattice",
    }
}

fn sizes_attr(kind: WidgetKind) -> String {
    spec(kind)
        .sizes
        .iter()
        .map(|(w, h)| format!("{w}x{h}"))
        .collect::<Vec<_>>()
        .join(",")
}

fn state_line<T>(load: &Load<T>, empty_key: &'static str) -> Option<AnyView> {
    match load {
        Load::Ready(_) => None,
        Load::Loading => Some(view! {
            <div class="oc-dw__loading" role="status" aria-busy="true">
                <span class="oc-sr">{t("state.loading")}</span>
                <ul class="oc-dw__skel" aria-hidden="true"><li></li><li></li><li></li></ul>
            </div>
        }.into_any()),
        Load::Empty => Some(view! { <p class="oc-dw__state">{t(empty_key)}</p> }.into_any()),
        Load::Failed(e) => Some(core_error(&e.reference).into_any()),
        Load::Denied => Some(view! { <p class="oc-dw__state oc-dw__state--denied">{icon("lock")}<span>{t("desk.denied")}</span></p> }.into_any()),
        Load::Unavailable => Some(view! { <p class="oc-dw__state">{t("state.unavailable")}</p> }.into_any()),
        Load::Inactive => Some(view! { <p class="oc-dw__state">{t("state.inactive")}</p> }.into_any()),
    }
}

fn list(items: &[WidgetItem]) -> impl IntoView {
    view! {
        <ul class="oc-dw__list">
            {items.iter().map(|i| {
                let href = i.href.clone();
                view! {
                    <li>
                        <a href=href>
                            <span class="oc-dw__mark" aria-hidden="true"></span>
                            <span class="oc-dw__title">{i.title.clone()}</span>
                            <span class="oc-dw__meta">{i.meta.clone()}</span>
                        </a>
                    </li>
                }
            }).collect_view()}
        </ul>
    }
}

/// «2 h», «agora», «22/09».
fn ago(a: &Ago) -> String {
    let n = |key, v: &u32| tf(key, &[("n", &v.to_string())]);
    match a {
        Ago::Now => t("time.short.now").to_owned(),
        Ago::Minutes(v) => n("time.short.min", v),
        Ago::Hours(v) => n("time.short.hour", v),
        Ago::Days(v) => n("time.short.day", v),
        Ago::Date(d) => d.clone(),
    }
}

/// «IDEIA · 2 h»; projectos: «PROJECTO · 65% · 5 h».
fn continue_meta(i: &ContinueItem) -> String {
    let mut parts = vec![t(i.kind.key()).to_owned()];
    if let Some(p) = i.progress {
        parts.push(format!("{}%", p.min(100)));
    }
    parts.push(ago(&i.when));
    parts.join(" · ")
}

fn continue_list(items: &[ContinueItem]) -> impl IntoView {
    view! {
        <ul class="oc-dw__list">
            {items.iter().map(|i| {
                let sr = i.progress.map(|p| tf("desk.cont.progress_sr", &[("pct", &p.min(100).to_string())]));
                view! {
                    <li data-item-kind=i.kind.as_str()>
                        <a href=i.href.clone()>
                            <span class="oc-dw__mark" aria-hidden="true"></span>
                            <span class="oc-dw__title">{i.title.clone()}</span>
                            <span class="oc-dw__meta">{continue_meta(i)}</span>
                            {sr.map(|s| view! { <span class="oc-sr">{s}</span> })}
                        </a>
                    </li>
                }
            }).collect_view()}
        </ul>
    }
}

/// «Core · 4 nós · cópia 03:00»; sem nós de computação: «Core · cópia 03:00».
fn health_line(h: &HealthVm) -> String {
    let nodes = match (h.nodes_up, h.nodes_total) {
        (_, 0) => None,
        (up, total) if up >= total => Some(tp("health.nodes.all", i64::from(total))),
        (up, total) => Some(tf(
            "health.nodes.partial",
            &[("up", &up.to_string()), ("total", &total.to_string())],
        )),
    };
    let backup = match &h.backup {
        Backup::Today(x) => tf("health.backup.today", &[("time", x)]),
        Backup::Yesterday(x) => tf("health.backup.yesterday", &[("time", x)]),
        Backup::Date(d) => tf("health.backup.date", &[("date", d)]),
        Backup::Failed => t("health.backup.failed").to_owned(),
        Backup::Never => t("health.backup.none").to_owned(),
        Backup::Unknown => t("health.backup.unknown").to_owned(),
    };
    [Some(t("health.core").to_owned()), nodes, Some(backup)]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ")
}

fn health(h: &HealthVm) -> AnyView {
    let (state, key) = match h.state {
        Health::Operational => ("ok", "health.state.ok"),
        Health::Degraded => ("warn", "health.state.degraded"),
        Health::Unavailable => ("down", "health.state.down"),
    };
    let inner = view! {
        <p class="oc-stat">
            <strong><span class="oc-health__dot" aria-hidden="true"></span>{t(key)}</strong>
            <span>{health_line(h)}</span>
        </p>
    };
    match &h.admin_href {
        Some(href) => view! {
            <a class="oc-health oc-health--link" data-state=state href=href.clone() title=t("health.open")>
                {inner}
                <span class="oc-sr">{t("health.open")}</span>
            </a>
        }
        .into_any(),
        None => view! { <div class="oc-health" data-state=state>{inner}</div> }.into_any(),
    }
}

fn body(content: &WidgetContent) -> AnyView {
    match content {
        WidgetContent::List(load) => match load {
            Load::Ready(items) => list(items).into_any(),
            other => state_line(other, "state.empty").unwrap_or_else(|| ().into_any()),
        },
        WidgetContent::Continue(load) => match load {
            Load::Ready(items) => continue_list(items).into_any(),
            other => state_line(other, "desk.cont.empty").unwrap_or_else(|| ().into_any()),
        },
        WidgetContent::Health(load) => match load {
            Load::Ready(h) => health(h),
            other => state_line(other, "state.empty").unwrap_or_else(|| ().into_any()),
        },
        WidgetContent::Metrics(load) => match load {
            Load::Ready(ms) => view! {
                <ul class="oc-kpis">
                    {ms.iter().map(|k| {
                        let href = k.href.clone();
                        view! {
                            <li>
                                <a class="oc-kpi" href=href>
                                    <span class="oc-kpi__head">
                                        <span class="oc-dw__icon">{icon(k.icon)}</span>
                                        <span class="oc-kpi__label">{k.label.clone()}</span>
                                        <span class="oc-kpi__go">{icon("arrow-r")}</span>
                                    </span>
                                    <span class="oc-kpi__value">
                                        <strong>{k.value.clone()}</strong>
                                        <span>{k.qualifier.clone()}</span>
                                    </span>
                                </a>
                            </li>
                        }
                    }).collect_view()}
                </ul>
            }
            .into_any(),
            other => state_line(other, "state.empty").unwrap_or_else(|| ().into_any()),
        },
        WidgetContent::Count(load) => match load {
            Load::Ready(c) => view! {
                <p class="oc-stat"><strong>{c.value.clone()}</strong><span>{c.qualifier.clone()}</span></p>
            }
            .into_any(),
            other => state_line(other, "state.empty").unwrap_or_else(|| ().into_any()),
        },
        WidgetContent::Storage(load) => match load {
            Load::Ready(s) => {
                let pct = (s.percent.min(100) / 5 * 5).to_string();
                let of = tf("desk.storage.of", &[("total", &s.total)]);
                view! {
                    <div class="oc-stat oc-stat--bar">
                        <p><strong>{s.used.clone()}</strong><span>{of}</span></p>
                        <span class="oc-meter" data-pct=pct role="img" aria-label=format!("{} %", s.percent)><span></span></span>
                    </div>
                }
                .into_any()
            }
            other => state_line(other, "state.empty").unwrap_or_else(|| ().into_any()),
        },
    }
}

/// D009 · Um widget cujo pedido o Core recusou (sem autorização) ou cuja
/// aplicação a Instância desactivou não se desenha: um cartão morto não diz
/// nada útil. Fica na disposição (`hidden`) para que gravar não o apague.
fn withheld(c: &WidgetContent) -> bool {
    fn no<T>(l: &Load<T>) -> bool {
        matches!(l, Load::Denied | Load::Inactive)
    }
    match c {
        WidgetContent::List(l) => no(l),
        WidgetContent::Continue(l) => no(l),
        WidgetContent::Health(l) => no(l),
        WidgetContent::Metrics(l) => no(l),
        WidgetContent::Count(l) => no(l),
        WidgetContent::Storage(l) => no(l),
    }
}

fn widget(w: &DeskWidget) -> impl IntoView {
    let s = spec(w.placed.kind);
    let title = t(s.title_key);
    let class = format!("oc-dw oc-dw--c{} oc-dw--r{}", w.placed.w, w.placed.h);
    let bare = w.placed.kind == WidgetKind::Kpis;
    view! {
        <li
            class=class
            data-part="desk-widget"
            data-id=w.placed.id.clone()
            data-kind=w.placed.kind.as_str()
            data-w=w.placed.w.to_string()
            data-h=w.placed.h.to_string()
            data-sizes=sizes_attr(w.placed.kind)
            data-mandatory=s.mandatory.then_some("")
            data-bare=bare.then_some("")
            data-min=w.placed.minimized.then_some("")
            data-withheld=withheld(&w.content).then_some("")
            hidden=withheld(&w.content)
            aria-label=title
        >
            <header class="oc-dw__head">
                <span class="oc-dw__icon">{icon(s.icon)}</span>
                <span class="oc-dw__names">
                    <h2>{title}</h2>
                    {s.subtitle_key.map(|k| view! { <span class="oc-dw__sub">{t(k)}</span> })}
                </span>
                {(!bare).then(|| {
                    let (key, ic) = if w.placed.minimized { ("desk.expand", "chev-d") } else { ("desk.collapse", "chev-u") };
                    view! {
                        <button
                            type="button"
                            class="oc-dw__all"
                            data-oc="dw-min"
                            aria-expanded=if w.placed.minimized { "false" } else { "true" }
                            aria-label=tf(key, &[("name", title)])
                            title=tf(key, &[("name", title)])
                            data-label-collapse=tf("desk.collapse", &[("name", title)])
                            data-label-expand=tf("desk.expand", &[("name", title)])
                        >
                            {icon(ic)}
                        </button>
                    }
                })}
                {s.href.filter(|_| !bare).map(|href| view! {
                    <a class="oc-dw__all" href=href aria-label=tf("desk.open", &[("name", title)]) title=tf("desk.open", &[("name", title)])>{icon("arrow-r")}</a>
                })}
                <span class="oc-dw__tools" data-part="desk-tools">
                    <button type="button" class="oc-dw__tool" data-oc="dw-left" aria-label=tf("desk.move_back", &[("name", title)])>{icon("chev-l")}</button>
                    <button type="button" class="oc-dw__tool" data-oc="dw-right" aria-label=tf("desk.move_forward", &[("name", title)])>{icon("chev-r")}</button>
                    {(s.sizes.len() > 1).then(|| view! {
                        <button type="button" class="oc-dw__tool" data-oc="dw-resize" aria-label=tf("desk.resize", &[("name", title)])>{icon("grid")}</button>
                    })}
                    {if s.mandatory {
                        view! {
                            <button type="button" class="oc-dw__tool" aria-disabled="true" title=t("desk.mandatory") aria-label=t("desk.mandatory")>{icon("lock")}</button>
                        }
                        .into_any()
                    } else {
                        view! {
                            <button type="button" class="oc-dw__tool oc-dw__tool--rm" data-oc="dw-remove" aria-label=tf("desk.remove", &[("name", title)])>{icon("close")}</button>
                        }
                        .into_any()
                    }}
                </span>
            </header>
            <div class="oc-dw__body">{body(&w.content)}</div>
        </li>
    }
}

fn library(vm: &DesktopVm) -> impl IntoView {
    let present: Vec<WidgetKind> = vm.widgets.iter().map(|w| w.placed.kind).collect();
    view! {
        <dialog class="oc-sheet" data-part="desk-library" aria-labelledby="desk-lib-title">
            <header class="oc-sheet__head">
                <h2 id="desk-lib-title">{t("desk.lib.title")}</h2>
                <button type="button" class="oc-round-btn" data-oc="dialog-close" aria-label=t("shell.close")>{icon("close")}</button>
            </header>
            <label class="oc-field">
                <span class="oc-sr">{t("desk.lib.search")}</span>
                {icon("search")}
                <input type="search" data-part="lib-q" placeholder=t("desk.lib.search") autocomplete="off" />
            </label>
            <div class="oc-chips" role="group" aria-label=t("desk.lib.categories")>
                <button type="button" class="oc-chip" data-oc="lib-cat" data-cat="all" aria-pressed="true">{t("desk.cat.all")}</button>
                {Category::ALL.into_iter().map(|c| view! {
                    <button type="button" class="oc-chip" data-oc="lib-cat" data-cat=c.as_str() aria-pressed="false">{t(c.label_key())}</button>
                }).collect_view()}
            </div>
            <ul class="oc-lib">
                {KINDS.iter().filter(|s| s.in_library && (vm.is_admin || !s.admin_only)).map(|s| {
                    let on = present.contains(&s.kind);
                    let name = t(s.title_key);
                    let search = format!("{} {}", name, t(s.desc_key)).to_lowercase();
                    let (w, h) = s.sizes[0];
                    view! {
                        <li data-part="lib-item" data-cat=s.category.as_str() data-search=search>
                            <span class="oc-lib__icon">{icon(s.icon)}</span>
                            <span class="oc-lib__text"><strong>{name}</strong><span>{t(s.desc_key)}</span></span>
                            <button
                                type="button"
                                class="oc-lib__add"
                                data-oc="lib-add"
                                data-kind=s.kind.as_str()
                                data-w=w.to_string()
                                data-h=h.to_string()
                                data-sizes=sizes_attr(s.kind)
                                data-mandatory=s.mandatory.then_some("")
                                aria-pressed=if on { "true" } else { "false" }
                                aria-label=tf("desk.lib.add", &[("name", name)])
                            >
                                {icon("plus")}
                                <span class="oc-lib__on">{icon("check")}</span>
                            </button>
                        </li>
                    }
                }).collect_view()}
            </ul>
            <p class="oc-lib__empty" data-part="lib-empty" hidden>{t("desk.lib.none")}</p>
        </dialog>
    }
}

fn background(vm: &DesktopVm) -> impl IntoView {
    let dim = vm.shell.dim.min(60) / 5 * 5;
    view! {
        <dialog class="oc-sheet" data-part="desk-background" aria-labelledby="desk-bg-title">
            <header class="oc-sheet__head">
                <h2 id="desk-bg-title">{t("desk.bg.title")}</h2>
                <button type="button" class="oc-round-btn" data-oc="dialog-close" aria-label=t("shell.close")>{icon("close")}</button>
            </header>
            <div class="oc-walls" role="group" aria-label=t("desk.bg.title")>
                {Wallpaper::ALL.into_iter().map(|w| {
                    let on = w == vm.shell.wallpaper;
                    view! {
                        <button type="button" class="oc-wall" data-oc="bg-wall" data-wall=w.as_str() aria-pressed=if on { "true" } else { "false" }>
                            <span class="oc-wall__swatch" data-wall=w.as_str() aria-hidden="true"></span>
                            <span>{t(wall_key(w))}</span>
                        </button>
                    }
                }).collect_view()}
            </div>
            <label class="oc-range">
                <span class="oc-range__head"><span>{t("desk.bg.dim")}</span><output data-part="bg-dim-out">{format!("{dim} %")}</output></span>
                <input type="range" min="0" max="60" step="5" value=dim.to_string() data-oc="bg-dim" />
            </label>
            <p class="oc-sheet__note">{t("desk.bg.photo_pending")}</p>
        </dialog>
    }
}

fn diff_lines(d: &DeskDiff) -> impl IntoView {
    let names = |ks: &[WidgetKind]| {
        ks.iter()
            .map(|k| t(spec(*k).title_key))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mut rows: Vec<(&'static str, &'static str, String)> = Vec::new();
    if !d.added.is_empty() {
        rows.push(("add", "desk.diff.added", names(&d.added)));
    }
    if !d.removed.is_empty() {
        rows.push(("rm", "desk.diff.removed", names(&d.removed)));
    }
    if !d.resized.is_empty() {
        let r = d
            .resized
            .iter()
            .map(|(k, (cw, ch), (dw, dh))| {
                format!("{} {cw}×{ch} → {dw}×{dh}", t(spec(*k).title_key))
            })
            .collect::<Vec<_>>()
            .join(", ");
        rows.push(("size", "desk.diff.resized", r));
    }
    if d.moved > 0 {
        rows.push((
            "move",
            "desk.diff.moved",
            crate::i18n::tp(
                "desk.diff.moved_n",
                i64::try_from(d.moved).unwrap_or(i64::MAX),
            ),
        ));
    }
    if d.look_changes {
        rows.push(("look", "desk.diff.look", String::new()));
    }
    view! {
        <ul class="oc-diff">
            {rows.into_iter().map(|(kind, key, text)| view! {
                <li data-kind=kind><strong>{t(key)}</strong>" "<span>{text}</span></li>
            }).collect_view()}
        </ul>
    }
}

fn restore(vm: &DesktopVm, def: &DesktopDefault) -> impl IntoView {
    let current: Vec<_> = vm.widgets.iter().map(|w| w.placed.clone()).collect();
    let look = def.wallpaper != vm.shell.wallpaper || def.dim != vm.shell.dim;
    let d = diff(&current, &def.widgets, look);
    let dist = vm.shell.distribution.map_or(String::new(), |d| {
        t(&format!("dist.{}", d.as_str())).to_owned()
    });
    let (source, origin, name, meta) = match def.source {
        // D009 · a predefinição mínima (Distribuição desconhecida).
        DefaultSource::System => (
            "system",
            t("desk.restore.origin.system"),
            t("desk.restore.system_fallback").to_owned(),
            t("desk.restore.system_fallback_meta").to_owned(),
        ),
        // D009 · a predefinição da Distribuição, com a versão que o Ocinye OS traz.
        DefaultSource::Distribution => (
            "distribution",
            t("desk.restore.origin.distribution"),
            tf("desk.restore.distribution_name", &[("distribution", &dist)]),
            tf(
                "desk.restore.distribution_meta",
                &[("version", &def.version.to_string())],
            ),
        ),
        DefaultSource::Instance => (
            "instance",
            t("desk.restore.origin.instance"),
            def.name.clone(),
            tf(
                "desk.restore.meta",
                &[
                    ("version", &def.version.to_string()),
                    ("date", &def.published),
                ],
            ),
        ),
    };
    view! {
        <dialog class="oc-sheet oc-sheet--narrow" data-part="desk-restore" aria-labelledby="desk-restore-title">
            <header class="oc-sheet__head">
                <h2 id="desk-restore-title">{t("desk.restore.title")}</h2>
                <button type="button" class="oc-round-btn" data-oc="dialog-close" aria-label=t("shell.close")>{icon("close")}</button>
            </header>
            <p class="oc-sheet__origin" data-source=source>{origin}</p>
            <p class="oc-sheet__lead"><strong>{name}</strong>" · "{meta}</p>
            {if d.is_empty() {
                view! { <p class="oc-sheet__lead">{t("desk.restore.same")}</p> }.into_any()
            } else {
                diff_lines(&d).into_any()
            }}
            <p class="oc-sheet__note">{t("desk.restore.keeps")}" "{t("desk.restore.pins_kept")}</p>
            <form class="oc-sheet__actions" method="post" action="/me/desktop/restore" data-oc="desk-restore-form">
                <button type="button" class="oc-btn-line" data-oc="dialog-close">{t("desk.cancel")}</button>
                <button type="submit" class="oc-btn-solid" disabled=d.is_empty()>{icon("restart")}{t("desk.restore.confirm")}</button>
            </form>
        </dialog>
    }
}

/// `GET /` — o Desktop.
pub fn home(vm: &DesktopVm) -> impl IntoView {
    // Só uma publicação da administração pode ser «nova» (D001.1).
    let newer = match (&vm.default, vm.base_version) {
        (Some(d), _) if d.source != DefaultSource::Instance => false,
        (Some(d), Some(b)) => d.version > b,
        (Some(_), None) => true,
        _ => false,
    };
    let can = vm.can_customise;
    let main = view! {
        <div class="oc-home" data-oc="desk" data-version=vm.version.to_string()>
            <h1 class="oc-sr">{t("desk.title")}</h1>
            {if can {
                view! {
                    <button type="button" class="oc-desk-pencil" data-oc="desk-edit" aria-pressed="false" aria-label=t("desk.customise") title=t("desk.customise")>{icon("edit")}</button>
                }
                .into_any()
            } else {
                view! {
                    <button type="button" class="oc-desk-pencil" aria-disabled="true" aria-describedby="home-customise-policy" aria-label=t("desk.customise")>{icon("edit")}</button>
                    <p class="oc-sr" id="home-customise-policy">{t("desk.policy_locked")}</p>
                }
                .into_any()
            }}
            {(newer && can).then(|| view! {
                <p class="oc-desk-notice" role="status">
                    {icon("bell")}
                    <span>{t("desk.new_default")}</span>
                    <button type="button" class="oc-desk-notice__btn" data-oc="desk-restore-open">{t("desk.new_default.see")}</button>
                </p>
            })}
            <div class="oc-editbar" data-part="desk-editbar" role="toolbar" aria-label=t("desk.editbar") hidden>
                <button type="button" class="oc-editbar__btn" data-oc="desk-lib-open">{icon("plus")}{t("desk.add")}</button>
                <button type="button" class="oc-editbar__btn" data-oc="desk-bg-open">{icon("image")}{t("desk.bg.title")}</button>
                {vm.default.is_some().then(|| view! {
                    <button type="button" class="oc-editbar__btn" data-oc="desk-restore-open">{icon("restart")}{t("desk.restore.title")}</button>
                })}
                <span class="oc-editbar__status" data-part="desk-status" role="status" aria-live="polite"></span>
                <button type="button" class="oc-editbar__done" data-oc="desk-edit-done">{icon("check")}{t("desk.done")}</button>
            </div>
            <ol class="oc-dgrid" data-part="desk-grid">{vm.widgets.iter().map(widget).collect_view()}</ol>
            <p class="oc-toast" data-part="desk-toast" role="status" hidden>
                <span data-part="desk-toast-text"></span>
                <button type="button" class="oc-toast__btn" data-oc="desk-undo">{icon("arrow-l")}{t("desk.undo")}</button>
            </p>
            <template data-part="desk-strings">
                <span data-key="saving">{t("desk.status.saving")}</span>
                <span data-key="saved">{t("desk.status.saved")}</span>
                <span data-key="failed">{t("desk.status.failed")}</span>
                <span data-key="conflict">{t("desk.status.conflict")}</span>
                <span data-key="restored">{t("desk.status.restored")}</span>
                <span data-key="removed">{t("desk.status.removed")}</span>
            </template>
            {can.then(|| library(vm))}
            {can.then(|| background(vm))}
            {vm.default.as_ref().filter(|_| can).map(|d| restore(vm, d))}
            {crate::ui::wm::desktop_menu(can, vm.default.is_some())}
        </div>
    }
    .into_any();
    shell(&vm.shell, main)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::testing::assert_contracts;
    use crate::ui::view_models::{
        ContinueKind, CoreError, Count, Distribution, Metric, PlacedWidget, ShellVm, StorageUse,
    };

    fn placed(kind: WidgetKind, w: u8, h: u8) -> PlacedWidget {
        PlacedWidget {
            id: kind.as_str().into(),
            kind,
            w,
            h,
            minimized: false,
        }
    }

    fn vm() -> DesktopVm {
        DesktopVm {
            shell: ShellVm {
                wallpaper: Wallpaper::Institutional,
                dim: 20,
                ..Default::default()
            },
            version: 7,
            widgets: vec![
                DeskWidget {
                    placed: placed(WidgetKind::Kpis, 4, 1),
                    content: WidgetContent::Metrics(Load::Ready(vec![Metric {
                        icon: "units",
                        label: "Unidades".into(),
                        value: "4".into(),
                        qualifier: "activas".into(),
                        href: "/units".into(),
                    }])),
                },
                DeskWidget {
                    placed: placed(WidgetKind::Ideas, 1, 1),
                    content: WidgetContent::Count(Load::Ready(Count {
                        value: "12".into(),
                        qualifier: "em investigação".into(),
                    })),
                },
                DeskWidget {
                    placed: placed(WidgetKind::Tasks, 1, 2),
                    content: WidgetContent::List(Load::Ready(vec![WidgetItem {
                        title: "Rever".into(),
                        meta: "30/09".into(),
                        href: "/tasks/t1".into(),
                    }])),
                },
                DeskWidget {
                    placed: placed(WidgetKind::Notice, 2, 1),
                    content: WidgetContent::List(Load::Failed(CoreError {
                        reference: "OC-9".into(),
                    })),
                },
                DeskWidget {
                    placed: placed(WidgetKind::Storage, 1, 1),
                    content: WidgetContent::Storage(Load::Ready(StorageUse {
                        used: "12 GB".into(),
                        total: "50 GB".into(),
                        percent: 24,
                    })),
                },
            ],
            default: Some(DesktopDefault {
                source: DefaultSource::Instance,
                name: "Research Desktop Default".into(),
                version: 5,
                published: "27/09/2026".into(),
                wallpaper: Wallpaper::Institutional,
                dim: 20,
                widgets: registry::system_default(Distribution::Research),
            }),
            base_version: Some(4),
            is_admin: false,
            can_customise: true,
        }
    }

    #[test]
    fn o_desktop_cumpre_os_contratos_e_desenha_cada_estado() {
        let html = home(&vm()).to_html();
        assert_contracts(&html);
        assert!(html.contains(r#"data-wall="org" data-dim="20""#));
        assert!(
            html.contains("oc-dw--c1 oc-dw--r2") && html.contains(r#"data-sizes="1x2,1x1,2x2""#)
        );
        assert!(html.contains("OC-9") && html.contains(r#"data-pct="20""#));
        assert!(
            html.contains("oc-kpi")
                && html.contains(">activas<")
                && html.contains(r#"href="/units""#)
        );
        assert!(html.contains("oc-stat") && html.contains(">12<"));
        assert!(html.contains(t("desk.sub.tasks")));
        assert!(html.contains("oc-desk-pencil") && html.contains(r#"data-oc="desk-edit""#));
        assert!(!html.contains("<h1>") || html.contains(r#"<h1 class="oc-sr">"#));
    }

    /// D009 · Sem obrigatórios: os avisos retiram-se como os outros.
    #[test]
    fn os_avisos_ja_nao_sao_obrigatorios() {
        let html = home(&vm()).to_html();
        let notice = &html[html.find(r#"data-kind="notice""#).unwrap()..];
        let notice = &notice[..notice.find("</li>").unwrap()];
        assert!(notice.contains(r#"data-oc="dw-remove""#) && !notice.contains("data-mandatory"));
    }

    /// D009 · Um widget recusado pelo Core fica escondido, não morto.
    #[test]
    fn um_widget_sem_autorizacao_nao_se_desenha() {
        let mut v = vm();
        v.widgets[1].content = WidgetContent::Count(Load::Denied);
        let html = home(&v).to_html();
        let ideas = &html[html.find(r#"data-kind="ideas""#).unwrap()..];
        let ideas = &ideas[..ideas.find(">").unwrap()];
        assert!(ideas.contains("data-withheld") && ideas.contains("hidden"));
    }

    #[test]
    fn repor_e_um_post_real_com_as_diferencas() {
        let html = home(&vm()).to_html();
        assert!(html.contains(r#"action="/me/desktop/restore""#));
        assert!(html.contains("oc-diff") && html.contains("oc-desk-notice"));
    }

    #[test]
    fn sem_politica_nao_ha_personalizar_nem_dialogos() {
        let html = home(&DesktopVm {
            can_customise: false,
            ..vm()
        })
        .to_html();
        assert_contracts(&html);
        assert!(!html.contains(r#"data-oc="desk-edit""#) && !html.contains("<dialog"));
        assert!(html.contains(r#"aria-describedby="home-customise-policy""#));
    }

    #[test]
    fn recolher_nao_mostra_o_modelo_cru() {
        let html = home(&vm()).to_html();
        assert!(!html.contains("{name}"));
        assert!(html.contains(&format!(
            r#"title="{}""#,
            tf("desk.collapse", &[("name", t("desk.w.tasks"))])
        )));
    }

    #[test]
    fn a_predefinicao_do_sistema_nao_parece_publicada() {
        let mut v = vm();
        v.shell.distribution = Some(Distribution::Research);
        if let Some(d) = v.default.as_mut() {
            d.source = DefaultSource::System;
            d.published = String::new();
        }
        let html = home(&v).to_html();
        assert_contracts(&html);
        assert!(
            html.contains(r#"data-source="system""#)
                && html.contains(t("desk.restore.system_fallback_meta"))
        );
        assert!(
            !html.contains("oc-desk-notice"),
            "sem publicação não há «nova predefinição»"
        );
        let pub_word = tf("desk.restore.meta", &[("version", "1"), ("date", "")]);
        assert!(!html.contains(pub_word.trim()));
        assert!(home(&vm()).to_html().contains(r#"data-source="instance""#));
    }

    #[test]
    fn copia_sem_registo_nao_afirma_exito_nem_falha() {
        let html = body(&WidgetContent::Health(Load::Ready(HealthVm {
            state: HealthVm::derive_state(true, 0, 0, true),
            nodes_up: 0,
            nodes_total: 0,
            backup: Backup::Unknown,
            admin_href: None,
        })))
        .to_html();
        assert!(html.contains(t("health.backup.unknown")));
        assert!(
            !html.contains(t("health.backup.failed")) && !html.contains(t("health.backup.none"))
        );
    }

    #[test]
    fn o_estado_do_sistema_esta_na_biblioteca_para_todos() {
        assert!(home(&vm()).to_html().contains(r#"data-kind="health""#));
    }

    fn item(kind: ContinueKind, progress: Option<u8>, when: Ago) -> ContinueItem {
        ContinueItem {
            kind,
            title: "x".into(),
            href: format!("/{}/1", kind.as_str()),
            progress,
            when,
        }
    }

    #[test]
    fn continuar_trabalho_compoe_a_meta_sem_strings_na_vista() {
        let items = vec![
            item(ContinueKind::Idea, None, Ago::Hours(2)),
            item(ContinueKind::Project, Some(65), Ago::Hours(5)),
            item(ContinueKind::File, None, Ago::Date("22/09".into())),
            item(ContinueKind::Note, None, Ago::Now),
        ];
        let html = body(&WidgetContent::Continue(Load::Ready(items))).to_html();
        assert!(html.contains(&format!(
            "{} · {}",
            t("desk.cont.type.idea"),
            tf("time.short.hour", &[("n", "2")])
        )));
        assert!(html.contains(&format!("{} · 65% · ", t("desk.cont.type.project"))));
        assert!(html.contains(&tf("desk.cont.progress_sr", &[("pct", "65")])));
        assert!(html.contains(&format!("{} · 22/09", t("desk.cont.type.file"))));
        assert!(html.contains(&format!(
            "{} · {}",
            t("desk.cont.type.note"),
            t("time.short.now")
        )));
        assert!(html.contains(r#"href="/idea/1""#) && html.contains(r#"data-item-kind="project""#));
    }

    #[test]
    fn cada_widget_desenha_carregar_vazio_erro_e_indisponivel() {
        let c = |l| body(&WidgetContent::Continue(l)).to_html();
        assert!(c(Load::Loading).contains(r#"aria-busy="true""#));
        assert!(c(Load::Empty).contains(t("desk.cont.empty")));
        assert!(c(Load::Failed(CoreError {
            reference: "OC-1".into()
        }))
        .contains("OC-1"));
        assert!(c(Load::Unavailable).contains(t("state.unavailable")));
        let h = |l| body(&WidgetContent::Health(l)).to_html();
        assert!(h(Load::Loading).contains("oc-dw__skel"));
        assert!(h(Load::Failed(CoreError {
            reference: "OC-2".into()
        }))
        .contains("OC-2"));
    }

    fn hvm(up: u16, backup: Backup, admin: bool) -> HealthVm {
        HealthVm {
            state: HealthVm::derive_state(true, up, 4, matches!(backup, Backup::Today(_))),
            nodes_up: up,
            nodes_total: 4,
            backup,
            admin_href: admin.then(|| "/admin/monitor".into()),
        }
    }

    #[test]
    fn o_estado_do_sistema_compoe_a_linha_e_so_o_administrador_o_abre() {
        let member = body(&WidgetContent::Health(Load::Ready(hvm(
            4,
            Backup::Today("03:00".into()),
            false,
        ))))
        .to_html();
        let line = format!(
            "{} · {} · {}",
            t("health.core"),
            tp("health.nodes.all", 4),
            tf("health.backup.today", &[("time", "03:00")])
        );
        assert!(member.contains(&line) && member.contains(t("health.state.ok")));
        assert!(
            member.contains(r#"class="oc-health""#)
                && member.contains(r#"data-state="ok""#)
                && !member.contains("<a")
        );
        let admin = body(&WidgetContent::Health(Load::Ready(hvm(
            3,
            Backup::Failed,
            true,
        ))))
        .to_html();
        assert!(
            admin.contains(r#"href="/admin/monitor""#) && admin.contains(r#"data-state="warn""#)
        );
        assert!(admin.contains(&tf("health.nodes.partial", &[("up", "3"), ("total", "4")])));
        assert!(
            admin.contains(t("health.backup.failed")) && admin.contains(t("health.state.degraded"))
        );
        for (b, k) in [
            (Backup::Yesterday("03:00".into()), "health.backup.yesterday"),
            (Backup::Date("25/09".into()), "health.backup.date"),
        ] {
            let html = body(&WidgetContent::Health(Load::Ready(hvm(4, b, false)))).to_html();
            assert!(html.contains(&tf(k, &[("time", "03:00"), ("date", "25/09")])));
        }
        assert!(body(&WidgetContent::Health(Load::Ready(hvm(
            4,
            Backup::Never,
            false
        ))))
        .to_html()
        .contains(t("health.backup.none")));
        let none = HealthVm {
            nodes_up: 0,
            nodes_total: 0,
            ..hvm(4, Backup::Today("03:00".into()), false)
        };
        let html = body(&WidgetContent::Health(Load::Ready(none))).to_html();
        assert!(html.contains(&format!(
            "{} · {}",
            t("health.core"),
            tf("health.backup.today", &[("time", "03:00")])
        )));
        assert!(!html.contains(&tp("health.nodes.all", 0)));
    }

    #[test]
    fn o_estado_agregado_e_o_tempo_curto_seguem_as_regras() {
        assert_eq!(
            HealthVm::derive_state(false, 4, 4, true),
            Health::Unavailable
        );
        assert_eq!(HealthVm::derive_state(true, 3, 4, true), Health::Degraded);
        assert_eq!(HealthVm::derive_state(true, 4, 4, false), Health::Degraded);
        assert_eq!(
            HealthVm::derive_state(true, 4, 4, true),
            Health::Operational
        );
        assert_eq!(
            HealthVm::derive_state(true, 0, 0, true),
            Health::Operational
        );
        let d = || "22/09".to_owned();
        assert_eq!(Ago::from_secs(59, d), Ago::Now);
        assert_eq!(Ago::from_secs(60, d), Ago::Minutes(1));
        assert_eq!(Ago::from_secs(7_200, d), Ago::Hours(2));
        assert_eq!(Ago::from_secs(86_400, d), Ago::Days(1));
        assert_eq!(Ago::from_secs(604_800, d), Ago::Date("22/09".into()));
    }

    #[test]
    fn todas_as_chaves_novas_existem() {
        for k in ContinueKind::ALL {
            assert!(crate::i18n::has(k.key()), "{}", k.key());
        }
        for k in [
            "desk.cont.empty",
            "desk.cont.progress_sr",
            "time.short.now",
            "time.short.min",
            "time.short.hour",
            "time.short.day",
            "health.core",
            "health.state.ok",
            "health.state.degraded",
            "health.state.down",
            "health.nodes.all.one",
            "health.nodes.all.other",
            "health.nodes.partial",
            "health.backup.today",
            "health.backup.yesterday",
            "health.backup.date",
            "health.backup.failed",
            "health.backup.none",
            "health.open",
            "health.backup.unknown",
            "desk.restore.origin.system",
            "desk.restore.origin.instance",
            "desk.restore.system_name",
            "desk.restore.system_meta",
        ] {
            assert!(crate::i18n::has(k), "{k}");
        }
    }
}
