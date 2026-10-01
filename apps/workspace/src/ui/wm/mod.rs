//! D002 · Janelas geridas pelo Ocinye e painéis da casca. DESIGN_LOCKED.
//!
//! Só apresentação. O motor (ordem, foco, geometria, encaixe, persistência,
//! política de lançamento, RBAC) é do Claude Code: muda os `data-*` de cada
//! janela e o `oc-wm.js` aplica-os por CSSOM (CSP `style-src 'self'`).
//!
//! Sem JavaScript: só a janela activa aparece, maximizada; os controlos são um
//! `POST /wm/{id}` (`op=minimize|maximize|restore|close`); o alternador abre por
//! âncora (`#oc-switcher`) e cada janela é uma ligação para a sua rota.
//!
//! Zonas (HANDOFF §D002): a janela é dona de minimizar/maximizar/fechar; a
//! barra de aplicações mostra fixadas e em execução; a prateleira mostra as
//! janelas; a barra de cima tem os painéis; o Desktop tem o menu de contexto.

use leptos::prelude::*;

use crate::i18n::{t, tf};
use crate::ui::components::{app_icon, core_error, icon};
use crate::ui::view_models::{
    Ago, Capability, ClockPanelVm, DirtyCloseVm, Health, Load, NotificationsPanelVm, StatusPanelVm,
    WindowContent, WindowState, WindowVm, WmVm,
};

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

const fn health_str(h: Health) -> (&'static str, &'static str) {
    match h {
        Health::Operational => ("ok", "wm.status.state.ok"),
        Health::Degraded => ("warn", "wm.status.state.degraded"),
        Health::Unavailable => ("down", "wm.status.state.down"),
    }
}

fn state_tag(w: &WindowVm) -> Option<&'static str> {
    match w.state {
        WindowState::Minimized => Some("wm.state.minimized"),
        WindowState::Maximized => Some("wm.state.maximized"),
        WindowState::SnappedLeft | WindowState::SnappedRight => Some("wm.state.snapped"),
        WindowState::Normal => None,
    }
}

/// Os controlos da janela: um formulário real (`POST /wm/{id}`) que o
/// `oc-wm.js` intercepta e entrega ao motor como `oc:wm`.
fn controls(w: &WindowVm) -> impl IntoView {
    let name = w.title.clone();
    let max = !matches!(
        w.state,
        WindowState::Maximized | WindowState::SnappedLeft | WindowState::SnappedRight
    );
    let (max_op, max_key, max_icon) = if max {
        ("maximize", "wm.maximize", "win-max")
    } else {
        ("restore", "wm.restore", "win-restore")
    };
    let lbl = |key: &str| tf(key, &[("name", &name)]);
    view! {
        <form class="oc-win__controls" id=format!("win-{}-ctl", w.id) method="post" action=format!("/wm/{}", w.id)>
            <button type="submit" class="oc-win__ctl" name="op" value="minimize" data-oc="wm" data-op="minimize" aria-label=lbl("wm.minimize") title=lbl("wm.minimize")>{icon("win-min")}</button>
            <button type="submit" class="oc-win__ctl" data-part="win-max" name="op" value=max_op data-oc="wm" data-op=max_op aria-label=lbl(max_key) title=lbl(max_key)>{icon(max_icon)}</button>
            <button type="submit" class="oc-win__ctl oc-win__ctl--close" name="op" value="close" data-oc="wm" data-op="close" aria-label=lbl("wm.close") title=lbl("wm.close")>{icon("close")}</button>
        </form>
    }
}

fn content(w: &WindowVm, body: Option<AnyView>) -> AnyView {
    match &w.content {
        WindowContent::Ready => body.unwrap_or_else(|| loading().into_any()),
        WindowContent::Loading => loading().into_any(),
        WindowContent::Pending => view! { <p class="oc-pending oc-win__state" role="status">{icon("clock")}<span>{t("shell.app.pending")}</span></p> }.into_any(),
        WindowContent::Failed(e) => view! { <div class="oc-win__state">{core_error(&e.reference)}</div> }.into_any(),
        WindowContent::Denied => view! { <p class="oc-win__state oc-state">{icon("lock")}<span>{t("state.denied")}</span></p> }.into_any(),
        WindowContent::Unavailable => view! { <p class="oc-win__state oc-state">{icon("warning")}<span>{t("wm.unavailable")}</span></p> }.into_any(),
    }
}

fn loading() -> impl IntoView {
    view! {
        <div class="oc-win__loading" role="status">
            <span class="oc-sr">{t("wm.loading")}</span>
            <span class="oc-win__skel" aria-hidden="true"></span>
            <span class="oc-win__skel" aria-hidden="true"></span>
            <span class="oc-win__skel oc-win__skel--short" aria-hidden="true"></span>
        </div>
    }
}

/// Uma janela gerida. `body` é o conteúdo da aplicação quando `content` é `Ready`.
pub fn window(w: &WindowVm, body: Option<AnyView>) -> impl IntoView {
    let title_id = format!("win-{}-title", w.id);
    let g = w.geometry;
    let loading = matches!(w.content, WindowContent::Loading);
    let edges = ["n", "e", "s", "w", "ne", "nw", "se", "sw"];
    view! {
        <section
            class="oc-win"
            id=format!("win-{}", w.id)
            data-oc="win"
            data-win=w.id.clone()
            data-app=w.app_id
            data-href=w.href.clone()
            data-state=w.state.as_str()
            data-active=w.active.then_some("")
            data-dirty=w.dirty.then_some("")
            data-loading=loading.then_some("")
            data-z=w.z.to_string()
            data-x=g.x.to_string()
            data-y=g.y.to_string()
            data-w=g.w.to_string()
            data-h=g.h.to_string()
            aria-labelledby=title_id.clone()
            aria-current=w.active.then_some("true")
        >
            <header class="oc-win__bar" data-part="win-drag">
                <button type="submit" form=format!("win-{}-ctl", w.id) class="oc-win__back" name="op" value="minimize" data-oc="wm" data-op="minimize" aria-label=t("wm.back")>{icon("chev-l")}</button>
                <span class="oc-win__icon" aria-hidden="true">{icon(app_icon(w.app_href))}</span>
                <span class="oc-win__names">
                    <h2 class="oc-win__title" id=title_id.clone()>{w.title.clone()}</h2>
                    {w.subtitle.clone().map(|s| view! { <span class="oc-win__sub">{s}</span> })}
                </span>
                {w.dirty.then(|| view! { <span class="oc-win__dirty" title=t("wm.unsaved")><span class="oc-sr">{t("wm.unsaved")}</span></span> })}
                <a class="oc-win__switch" href="#oc-switcher" aria-label=t("wm.switcher")>{icon("windows")}</a>
                {controls(w)}
            </header>
            <div class="oc-win__body" data-part="win-body">{content(w, body)}</div>
            <span class="oc-win__size" data-part="win-size" aria-hidden="true"></span>
            {edges.into_iter().map(|e| view! { <span class="oc-win__rz" data-part="win-resize" data-edge=e aria-hidden="true"></span> }).collect_view()}
        </section>
    }
}

/// D002.1 · O alternador é uma camada global da casca (como o lançador e a
/// paleta): desenha-se fora do `.oc-desk` (`isolation: isolate`), para que o
/// véu cubra a barra de cima. `shell_with_window` chama-o depois de `palette`.
pub fn switcher(wm: &WmVm) -> impl IntoView {
    let mut ws: Vec<&WindowVm> = wm.windows.iter().collect();
    ws.sort_by_key(|w| std::cmp::Reverse(w.z));
    let hint = wm
        .switcher_hint
        .as_ref()
        .map(|k| tf("wm.switcher.hint", &[("keys", k)]));
    view! {
        <div class="oc-overlay oc-overlay--center" id="oc-switcher" data-oc="switcher" role="dialog" aria-modal="true" aria-labelledby="oc-switcher-title">
            <a class="oc-overlay__scrim" href="#" aria-label=t("shell.close")></a>
            <div class="oc-switcher">
                <h2 class="oc-switcher__title" id="oc-switcher-title">{t("wm.switcher")}</h2>
                {if ws.is_empty() {
                    view! { <p class="oc-switcher__empty">{t("wm.none")}</p> }.into_any()
                } else {
                    view! {
                        <ul class="oc-switcher__list" data-part="switcher-list">
                            {ws.into_iter().map(|w| {
                                let tag = state_tag(w);
                                view! {
                                    <li>
                                        <a class="oc-switch" href=w.href.clone() data-oc="wm-focus" data-win=w.id.clone() data-part="switcher-item" aria-current=w.active.then_some("true")>
                                            <span class="oc-switch__preview" data-app=w.app_id aria-hidden="true">
                                                <span class="oc-switch__bar"></span>
                                                <span class="oc-switch__glyph">{icon(app_icon(w.app_href))}</span>
                                                <span class="oc-switch__line"></span>
                                                <span class="oc-switch__line oc-switch__line--short"></span>
                                            </span>
                                            <span class="oc-switch__title">{w.title.clone()}</span>
                                            <span class="oc-switch__sub">{w.subtitle.clone().unwrap_or_default()}</span>
                                            {tag.map(|k| view! { <span class="oc-switch__tag">{t(k)}</span> })}
                                        </a>
                                    </li>
                                }
                            }).collect_view()}
                        </ul>
                    }.into_any()
                }}
                {hint.map(|h| view! { <p class="oc-switcher__hint">{h}</p> })}
            </div>
        </div>
    }
}

/// Escolha da janela quando uma aplicação fixada tem mais de uma aberta.
fn choosers(wm: &WmVm) -> impl IntoView {
    let mut apps: Vec<&'static str> = wm.windows.iter().map(|w| w.app_id).collect();
    apps.sort_unstable();
    apps.dedup();
    apps.into_iter()
        .filter(|a| wm.windows.iter().filter(|w| w.app_id == *a).count() > 1)
        .map(|a| {
            let ws: Vec<&WindowVm> = wm.windows.iter().filter(|w| w.app_id == a).collect();
            let name = ws[0].title.clone();
            let href = ws[0].app_href;
            let multi = wm.multi_window_apps.contains(&a);
            let title = tf("wm.chooser", &[("name", &name)]);
            view! {
                <div class="oc-chooser" data-oc="chooser" data-app=a role="dialog" aria-label=title.clone() hidden>
                    <p class="oc-chooser__title">{title.clone()}</p>
                    <ul class="oc-chooser__list">
                        {ws.into_iter().map(|w| view! {
                            <li>
                                <a class="oc-chooser__item" href=w.href.clone() data-oc="wm-focus" data-win=w.id.clone() data-state=w.state.as_str() aria-current=w.active.then_some("true")>
                                    {icon(app_icon(w.app_href))}
                                    <span class="oc-chooser__name">{w.subtitle.clone().unwrap_or_else(|| w.title.clone())}</span>
                                    {state_tag(w).map(|k| view! { <span class="oc-chooser__tag">{t(k)}</span> })}
                                </a>
                            </li>
                        }).collect_view()}
                    </ul>
                    {multi.then(|| view! {
                        <a class="oc-chooser__new" href=format!("{href}?window=new") data-oc="wm-open" data-app=a>{icon("plus")}{t("wm.chooser.new")}</a>
                    })}
                </div>
            }
        })
        .collect_view()
}

/// A camada das janelas, por cima da área de trabalho. `active_body` é o
/// conteúdo da janela `Ready` (a rota pedida); as outras chegam por `?frame=1`.
pub fn layer(wm: &WmVm, active_body: Option<AnyView>) -> impl IntoView {
    let mut body = active_body;
    let mut ws: Vec<&WindowVm> = wm.windows.iter().collect();
    ws.sort_by_key(|w| w.z);
    let count = ws.len().to_string();
    view! {
        <div class="oc-wm" data-oc="wm-layer" data-windows=count>
            {ws.into_iter().map(|w| {
                let b = if w.content == WindowContent::Ready { body.take() } else { None };
                window(w, b)
            }).collect_view()}
            <div class="oc-snap" data-part="snap-preview" aria-hidden="true" hidden></div>
            {choosers(wm)}
            <p class="oc-wm__live oc-sr" data-part="wm-live" role="status" aria-live="polite"></p>
            <template data-part="wm-strings">
                <span data-key="focused">{t("wm.focused")}</span>
                <span data-key="snap-left">{t("wm.snap.left")}</span>
                <span data-key="snap-right">{t("wm.snap.right")}</span>
                <span data-key="snap-max">{t("wm.snap.max")}</span>
            </template>
        </div>
    }
}

/// O indicador de execução de uma aplicação fixada na barra de aplicações:
/// nenhum (só fixada), um ponto (1 janela), dois (2+); `data-active` com foco.
pub fn dock_run(wm: Option<&WmVm>, app_id: &str) -> Option<impl IntoView> {
    let wm = wm?;
    let ws: Vec<&WindowVm> = wm.windows.iter().filter(|w| w.app_id == app_id).collect();
    if ws.is_empty() {
        return None;
    }
    let n = if ws.len() > 1 { "2" } else { "1" };
    let active = ws
        .iter()
        .any(|w| w.active && w.state != WindowState::Minimized);
    Some(
        view! { <span class="oc-dock__run" data-n=n data-active=active.then_some("") aria-hidden="true"></span> },
    )
}

/// O rótulo acessível da aplicação fixada, com as janelas abertas.
#[must_use]
pub fn dock_label(wm: Option<&WmVm>, app_id: &str, label: &str) -> (String, usize) {
    let n = wm.map_or(0, |wm| {
        wm.windows.iter().filter(|w| w.app_id == app_id).count()
    });
    let text = match n {
        0 => label.to_owned(),
        1 => tf("wm.running.one", &[("name", label)]),
        _ => tf("wm.running.many", &[("name", label), ("n", &n.to_string())]),
    };
    (text, n)
}

/// Fechar com trabalho por guardar: Guardar · Não guardar · Cancelar.
/// Só para trabalho por guardar numa aplicação; nunca para operações destrutivas.
pub fn dirty_close(vm: &DirtyCloseVm) -> impl IntoView {
    let save_label = vm.save_label.unwrap_or("wm.dirty.save");
    view! {
        <div class="oc-overlay oc-overlay--center" data-open="" data-oc="dirty-close" data-win=vm.window_id.clone() role="alertdialog" aria-modal="true" aria-labelledby="oc-dirty-title" aria-describedby="oc-dirty-body">
            <form class="oc-dialog" method="post" action=format!("/wm/{}/close", vm.window_id)>
                // Code (D010 · S16): a continuação da mudança de Distribuição.
                {vm.after.clone().map(|after| view! { <input type="hidden" name="after" value=after /> })}
                <span class="oc-dialog__icon" aria-hidden="true">{icon("warning")}</span>
                <h2 class="oc-dialog__title" id="oc-dirty-title">{tf("wm.dirty.title", &[("name", &vm.title)])}</h2>
                <p class="oc-dialog__body" id="oc-dirty-body">{t("wm.dirty.body")}</p>
                {(!vm.can_save).then(|| view! { <p class="oc-dialog__note" id="oc-dirty-nosave">{t("wm.dirty.cannot_save")}</p> })}
                <div class="oc-dialog__actions">
                    <button type="submit" class="oc-btn-plain" name="decision" value="cancel" data-oc="dirty-cancel">{t("wm.dirty.cancel")}</button>
                    <span class="oc-dialog__spacer"></span>
                    <button type="submit" class="oc-btn-line oc-btn-line--danger" name="decision" value="discard">{t("wm.dirty.discard")}</button>
                    {if vm.can_save {
                        // D004 · Com `save_form`, «Guardar» submete o formulário da
                        // aplicação (o texto que só existe no editor), com `then=close`.
                        view! { <button type="submit" class="oc-btn-gold" name=if vm.save_form.is_some() { "then" } else { "decision" } value=if vm.save_form.is_some() { "close" } else { "save" } form=vm.save_form.clone() data-oc="dirty-save">{t(save_label)}</button> }.into_any()
                    } else {
                        view! { <button type="button" class="oc-btn-gold" aria-disabled="true" aria-describedby="oc-dirty-nosave">{t(save_label)}</button> }.into_any()
                    }}
                </div>
            </form>
        </div>
    }
}

/// O menu de contexto do Desktop (só na superfície do Desktop: nunca em
/// widgets, janelas ou barras). Cada item acciona o controlo D001 equivalente.
pub fn desktop_menu(can_customise: bool, has_default: bool) -> impl IntoView {
    let item = |proxy: &'static str, ic: &'static str, key: &'static str| {
        view! { <button type="button" class="oc-menu__item" role="menuitem" data-oc="ctx-item" data-proxy=proxy>{icon(ic)}{t(key)}</button> }
    };
    view! {
        <div class="oc-ctx" data-oc="desk-ctx" role="menu" aria-label=t("wm.ctx.title") hidden>
            {if can_customise {
                view! {
                    {item("desk-lib-open", "plus", "wm.ctx.add")}
                    {item("desk-bg-open", "image", "wm.ctx.bg")}
                    {item("desk-edit", "edit", "wm.ctx.customise")}
                    {has_default.then(|| item("desk-restore-open", "restart", "wm.ctx.restore"))}
                }
                .into_any()
            } else {
                view! { <p class="oc-menu__note">{t("wm.ctx.policy")}</p> }.into_any()
            }}
            <hr class="oc-menu__sep" />
            <a class="oc-menu__item" role="menuitem" href="/">{icon("refresh")}{t("wm.ctx.refresh")}</a>
        </div>
    }
}

/// O painel da pastilha CORE·IA. A IA é opcional: nunca torna o Ocinye OS indisponível.
pub fn status_panel(vm: &StatusPanelVm) -> impl IntoView {
    let (state, _) = health_str(vm.overall);
    let overall = match vm.overall {
        Health::Operational => "wm.status.overall.ok",
        Health::Degraded => "wm.status.overall.degraded",
        Health::Unavailable => "wm.status.overall.down",
    };
    let group = |required: bool| {
        let caps: Vec<_> = vm
            .capabilities
            .iter()
            .filter(|c| c.required == required)
            .collect();
        (!caps.is_empty()).then(|| view! {
            <p class="oc-panel__kicker">{t(if required { "wm.status.required" } else { "wm.status.optional" })}</p>
            <ul class="oc-panel__caps">
                {caps.into_iter().map(|c| {
                    let (s, k) = c.state.map_or(("unknown", "wm.status.state.unknown"), health_str);
                    view! {
                        <li class="oc-cap" data-state=s>
                            <span class="oc-cap__dot" aria-hidden="true"></span>
                            <span class="oc-cap__name">{t(c.kind.key())}</span>
                            <span class="oc-cap__state">{t(k)}{c.detail.clone().map(|d| view! { <span class="oc-cap__detail">" · "{d}</span> })}</span>
                        </li>
                    }
                }).collect_view()}
            </ul>
        })
    };
    let ai_note = vm
        .capabilities
        .iter()
        .any(|c| c.kind == Capability::Ai && c.state != Some(Health::Operational))
        .then(|| view! { <p class="oc-panel__note">{icon("ai")}<span>{t("wm.status.ai_note")}</span></p> });
    let storage = vm.storage.filter(|(_, l)| *l > 0).map(|(u, l)| {
        let pct = (u.saturating_mul(100) / l).min(100);
        view! {
            <div class="oc-panel__storage">
                <span>{tf("wm.status.storage", &[("pct", &pct.to_string())])}</span>
                <meter min="0" max="100" value=pct.to_string() low="80" high="95" optimum="0"></meter>
            </div>
        }
    });
    view! {
        <div class="oc-panel" data-state=state>
            <p class="oc-panel__head"><span class="oc-cap__dot" aria-hidden="true"></span><strong>{t(overall)}</strong></p>
            {group(true)}
            {group(false)}
            {ai_note}
            {storage}
            {vm.detail_href.map(|h| view! { <a class="oc-panel__foot" href=h>{t("wm.status.detail")}{icon("arrow-r")}</a> })}
        </div>
    }
}

/// O painel das notificações.
pub fn notifications_panel(vm: &NotificationsPanelVm) -> impl IntoView {
    let body = match &vm.items {
        Load::Ready(items) if !items.is_empty() => view! {
            <ul class="oc-panel__list">
                {items.iter().take(6).map(|n| view! {
                    <li>
                        <a class="oc-note" href=n.href.clone() data-read=n.read.then_some("")>
                            {(!n.read).then(|| view! { <span class="oc-note__dot"><span class="oc-sr">{t("wm.notif.unread")}</span></span> })}
                            <span class="oc-note__text"><strong>{n.title.clone()}</strong><span>{n.body.clone()}</span></span>
                            <span class="oc-note__when">{ago(&n.when)}</span>
                        </a>
                    </li>
                }).collect_view()}
            </ul>
        }
        .into_any(),
        Load::Ready(_) | Load::Empty => view! { <p class="oc-panel__empty">{t("wm.notif.empty")}</p> }.into_any(),
        Load::Loading => view! { <p class="oc-panel__empty" role="status">{t("state.loading")}</p> }.into_any(),
        Load::Failed(e) => core_error(&e.reference).into_any(),
        _ => view! { <p class="oc-panel__empty">{t("state.unavailable")}</p> }.into_any(),
    };
    let any_unread = matches!(&vm.items, Load::Ready(i) if i.iter().any(|n| !n.read));
    view! {
        <div class="oc-panel">
            <div class="oc-panel__bar">
                <strong>{t("wm.notif.title")}</strong>
                {any_unread.then(|| view! {
                    <form method="post" action="/notifications/read-all">
                        <button type="submit" class="oc-panel__link">{t("wm.notif.read_all")}</button>
                    </form>
                })}
            </div>
            {body}
            <a class="oc-panel__foot" href="/notifications">{t("wm.notif.all")}{icon("arrow-r")}</a>
        </div>
    }
}

/// O painel do relógio: o mês de hoje (sem navegação) e a agenda de hoje.
/// Não é um segundo Calendário: «Abrir o Calendário» leva à aplicação.
pub fn clock_panel(vm: &ClockPanelVm) -> impl IntoView {
    let (year, month, day) = vm.today;
    let month_name = t(match month {
        1 => "wm.month.1",
        2 => "wm.month.2",
        3 => "wm.month.3",
        4 => "wm.month.4",
        5 => "wm.month.5",
        6 => "wm.month.6",
        7 => "wm.month.7",
        8 => "wm.month.8",
        9 => "wm.month.9",
        10 => "wm.month.10",
        11 => "wm.month.11",
        _ => "wm.month.12",
    });
    let heading = tf(
        "wm.clock.month",
        &[("month", month_name), ("year", &year.to_string())],
    );
    let lead = usize::from(vm.first_weekday.min(6));
    let cells: Vec<Option<u8>> = (0..lead)
        .map(|_| None)
        .chain((1..=vm.days_in_month).map(Some))
        .collect();
    let weeks: Vec<Vec<Option<u8>>> = cells
        .chunks(7)
        .map(|c| {
            let mut w = c.to_vec();
            w.resize(7, None);
            w
        })
        .collect();
    let wd = [
        "wm.wd.1", "wm.wd.2", "wm.wd.3", "wm.wd.4", "wm.wd.5", "wm.wd.6", "wm.wd.7",
    ];
    let agenda = match &vm.agenda {
        Load::Ready(items) if !items.is_empty() => view! {
            <ul class="oc-panel__list">
                {items.iter().take(3).map(|i| view! {
                    <li><a class="oc-agenda" href=i.href.clone()><span class="oc-agenda__when">{i.meta.clone()}</span><span>{i.title.clone()}</span></a></li>
                }).collect_view()}
            </ul>
        }
        .into_any(),
        Load::Ready(_) | Load::Empty => view! { <p class="oc-panel__empty">{t("wm.clock.none")}</p> }.into_any(),
        Load::Loading => view! { <p class="oc-panel__empty" role="status">{t("state.loading")}</p> }.into_any(),
        Load::Failed(e) => core_error(&e.reference).into_any(),
        _ => view! { <p class="oc-panel__empty">{t("state.unavailable")}</p> }.into_any(),
    };
    view! {
        <div class="oc-panel">
            <p class="oc-panel__month">{heading.clone()}</p>
            <table class="oc-month">
                <caption class="oc-sr">{heading}</caption>
                <thead><tr>{wd.into_iter().map(|k| view! { <th scope="col"><abbr title=t(&format!("{k}.long"))>{t(k)}</abbr></th> }).collect_view()}</tr></thead>
                <tbody>
                    {weeks.into_iter().map(|w| view! {
                        <tr>{w.into_iter().map(|d| match d {
                            Some(n) => view! { <td data-today=(n == day).then_some("") aria-current=(n == day).then_some("date")>{n.to_string()}</td> }.into_any(),
                            None => view! { <td></td> }.into_any(),
                        }).collect_view()}</tr>
                    }).collect_view()}
                </tbody>
            </table>
            <p class="oc-panel__kicker">{t("wm.clock.today")}</p>
            {agenda}
            <a class="oc-panel__foot" href="/calendar">{t("wm.clock.open")}{icon("arrow-r")}</a>
        </div>
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::ui::testing::assert_contracts;
    use crate::ui::view_models::{
        CapabilityVm, CoreError, NotificationItem, WidgetItem, WindowGeometry,
    };

    pub(crate) fn win(
        id: &str,
        app: &'static str,
        href: &'static str,
        active: bool,
        state: WindowState,
        z: u16,
    ) -> WindowVm {
        WindowVm {
            id: id.into(),
            app_id: app,
            app_href: href,
            href: format!("{href}/{id}"),
            title: app.into(),
            subtitle: Some(format!("{id}.txt")),
            state,
            active,
            z,
            geometry: WindowGeometry::default(),
            dirty: false,
            content: WindowContent::Loading,
        }
    }

    pub(crate) fn wm() -> WmVm {
        let mut a = win("a", "files", "/files", false, WindowState::Normal, 1);
        a.content = WindowContent::Pending;
        let mut b = win("b", "files", "/files", false, WindowState::Minimized, 2);
        b.dirty = true;
        let mut c = win("c", "notes", "/notes", true, WindowState::Maximized, 3);
        c.content = WindowContent::Ready;
        WmVm {
            windows: vec![a, b, c],
            switcher_hint: Some("Alt Tab".into()),
            multi_window_apps: vec!["files"],
        }
    }

    #[test]
    fn a_camada_desenha_todas_as_janelas_com_estado_e_controlos_reais() {
        let html = layer(&wm(), Some(view! { <p>"corpo"</p> }.into_any())).to_html();
        assert_contracts(&html);
        for s in [
            r#"data-state="normal""#,
            r#"data-state="minimized""#,
            r#"data-state="maximized""#,
        ] {
            assert!(html.contains(s), "{s}");
        }
        assert_eq!(html.matches(r#"data-oc="win""#).count(), 3);
        assert!(
            html.contains(r#"action="/wm/c""#)
                && html.contains(r#"value="restore""#)
                && html.contains(r#"value="maximize""#)
        );
        assert!(html.contains("corpo") && html.contains(t("shell.app.pending")));
        // D003 · sem prateleira: as janelas abertas vivem só na barra de aplicações.
        assert!(!html.contains("oc-shelf"));
        assert!(
            !html.contains(r#"id="oc-switcher""#),
            "D002.1: o alternador não pertence à camada das janelas"
        );
        let sw = switcher(&wm()).to_html();
        assert!(sw.contains(r#"id="oc-switcher""#) && sw.contains(r#"role="dialog""#));
        assert!(!html.contains("{name}") && !sw.contains("{keys}"));
    }

    #[test]
    fn so_uma_aplicacao_com_varias_janelas_tem_escolha_e_so_se_aceitar_ha_nova() {
        let html = layer(&wm(), None).to_html();
        assert_eq!(html.matches(r#"data-oc="chooser""#).count(), 1);
        assert!(
            html.contains(r#"data-app="files" role="dialog""#)
                || html.contains(r#"data-oc="chooser" data-app="files""#)
        );
        assert!(html.contains("/files?window=new"));
    }

    #[test]
    fn a_barra_de_aplicacoes_distingue_fixada_em_execucao_e_activa() {
        let w = wm();
        assert!(dock_run(Some(&w), "tasks").is_none());
        assert!(dock_run(Some(&w), "files")
            .map(|v| v.to_html())
            .is_some_and(|h| h.contains(r#"data-n="2""#) && !h.contains("data-active")));
        assert!(dock_run(Some(&w), "notes")
            .map(|v| v.to_html())
            .is_some_and(|h| h.contains(r#"data-n="1""#) && h.contains("data-active")));
        assert_eq!(dock_label(Some(&w), "files", "Ficheiros").1, 2);
    }

    #[test]
    fn fechar_com_alteracoes_tem_guardar_nao_guardar_cancelar() {
        let html = dirty_close(&DirtyCloseVm {
            window_id: "b".into(),
            title: "Notas".into(),
            can_save: true,
            save_label: None,
            save_form: None,
            after: None,
        })
        .to_html();
        assert_contracts(&html);
        assert!(
            html.contains(r#"role="alertdialog""#)
                && html.contains(r#"value="save""#)
                && html.contains(r#"value="discard""#)
                && html.contains(r#"value="cancel""#)
        );
        let no = dirty_close(&DirtyCloseVm {
            window_id: "b".into(),
            title: "Notas".into(),
            can_save: false,
            save_label: None,
            save_form: None,
            after: None,
        })
        .to_html();
        assert!(!no.contains(r#"value="save""#) && no.contains(t("wm.dirty.cannot_save")));
    }

    #[test]
    fn o_menu_do_desktop_respeita_a_politica() {
        let on = desktop_menu(true, true).to_html();
        assert_contracts(&on);
        assert!(
            on.contains(r#"data-proxy="desk-lib-open""#)
                && on.contains(r#"data-proxy="desk-restore-open""#)
        );
        let off = desktop_menu(false, true).to_html();
        assert!(!off.contains("data-proxy") && off.contains(t("wm.ctx.policy")));
    }

    #[test]
    fn a_ia_indisponivel_nao_torna_o_ocinye_indisponivel() {
        let html = status_panel(&StatusPanelVm {
            overall: Health::Operational,
            capabilities: vec![
                CapabilityVm {
                    kind: Capability::Core,
                    required: true,
                    state: Some(Health::Operational),
                    detail: None,
                },
                CapabilityVm {
                    kind: Capability::Backup,
                    required: false,
                    state: None,
                    detail: None,
                },
                CapabilityVm {
                    kind: Capability::Ai,
                    required: false,
                    state: Some(Health::Unavailable),
                    detail: None,
                },
            ],
            storage: Some((40, 100)),
            detail_href: None,
        })
        .to_html();
        assert_contracts(&html);
        assert!(
            html.contains(t("wm.status.overall.ok"))
                && html.contains(t("wm.status.ai_note"))
                && html.contains(t("wm.status.state.unknown"))
        );
        assert!(!html.contains("/admin/monitor"));
    }

    #[test]
    fn notificacoes_e_relogio_desenham_cada_estado() {
        let n = |items| notifications_panel(&NotificationsPanelVm { items }).to_html();
        let one = NotificationItem {
            id: "1".into(),
            title: "T".into(),
            body: "B".into(),
            when: Ago::Minutes(5),
            read: false,
            href: "/x".into(),
        };
        assert!(n(Load::Ready(vec![one])).contains("/notifications/read-all"));
        assert!(n(Load::Empty).contains(t("wm.notif.empty")));
        assert!(n(Load::Failed(CoreError {
            reference: "OC-1".into()
        }))
        .contains("OC-1"));
        let c = clock_panel(&ClockPanelVm {
            today: (2026, 9, 29),
            first_weekday: 1,
            days_in_month: 30,
            agenda: Load::Ready(vec![WidgetItem {
                title: "Reunião".into(),
                meta: "15:00".into(),
                href: "/calendar/1".into(),
            }]),
        })
        .to_html();
        assert_contracts(&c);
        assert!(
            c.contains(r#"aria-current="date""#)
                && c.contains(">29<")
                && c.contains("Reunião")
                && c.contains(r#"href="/calendar""#)
        );
    }
}
