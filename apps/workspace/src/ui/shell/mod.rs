//! A casca comum das páginas autenticadas (D16–D21). DESIGN_LOCKED.
//!
//! Barra de cima (conta, contexto, pesquisa/Nye, «+ Criar», estado Instância·IA,
//! notificações, relógio), barra de aplicações, lançador e paleta de comandos.
//! Sem JavaScript: a barra de aplicações fica sempre visível e o lançador e a
//! paleta abrem por âncora (`#oc-launcher`, `#oc-palette`, CSS `:target`).
//!
//! Lacunas desenhadas com o estado honesto: bloquear ecrã (G-01), janelas
//! (G-05: as aplicações abrem como página inteira) e recentes (G-06). Fixar
//! aplicações usa `PUT /apps/pins` (JS); sem JS, `/settings/apps`.
//! Sem selector de contexto: não há unidade activa global (CLAUDE.md §34.3).

use leptos::prelude::*;

use crate::experience::{distribution, iconography};
use crate::i18n::{t, tf};
use crate::ui::components::{app_icon, icon};
use crate::ui::nye;
use crate::ui::view_models::{Distribution, Health, ShellVm};
use crate::ui::wm;

/// «+ Criar»: só rotas GET que existem.
const CREATE: &[(&str, &str, &str)] = &[
    ("/tasks/new", "shell.create.task", "tasks"),
    ("/ideas/new", "shell.create.idea", "idea"),
    ("/projects/new", "shell.create.project", "project"),
    ("/calendar/events/new", "shell.create.event", "calendar"),
    ("/mail/compose", "shell.create.mail", "mail"),
    (
        "/bibliography/new",
        "shell.create.reference",
        "bibliography",
    ),
    ("/datasets/new", "shell.create.dataset", "data"),
];

fn health_chip(core: Option<Health>, ai: Option<Health>) -> impl IntoView {
    let part = |h: Option<Health>, label: &'static str| {
        h.map(|h| {
            let (state, key) = match h {
                Health::Operational => ("ok", "shell.status.ok"),
                Health::Degraded => ("warn", "shell.status.degraded"),
                Health::Unavailable => ("down", "shell.status.down"),
            };
            view! {
                <span class="oc-status__part" data-state=state>
                    <span class="oc-status__dot" aria-hidden="true"></span>
                    {t(label)}
                    <span class="oc-sr">" · "{t(key)}</span>
                </span>
            }
        })
    };
    (core.is_some() || ai.is_some()).then(|| {
        view! {
            <a class="oc-status" href="/activity" title=t("shell.status.title")>
                {part(core, "shell.status.core")}
                {part(ai, "shell.status.ai")}
            </a>
        }
    })
}

/// D002 · Com painel, a pastilha abre-o; sem painel, é a ligação D001.
fn status_control(vm: &ShellVm) -> AnyView {
    match &vm.panels.status {
        None => health_chip(vm.core, vm.ai).into_any(),
        Some(p) => {
            let part = |h: Option<Health>, label: &'static str| {
                h.map(|h| {
                    let state = match h {
                        Health::Operational => "ok",
                        Health::Degraded => "warn",
                        Health::Unavailable => "down",
                    };
                    view! {
                        <span class="oc-status__part" data-state=state>
                            <span class="oc-status__dot" aria-hidden="true"></span>
                            {t(label)}
                        </span>
                    }
                })
            };
            view! {
                <details class="oc-menu oc-panel-menu" data-oc="menu">
                    <summary class="oc-status" aria-label=t("wm.status.title") title=t("shell.status.title")>
                        {part(vm.core, "shell.status.core")}
                        {part(vm.ai, "shell.status.ai")}
                    </summary>
                    <div class="oc-menu__pop oc-panel__pop" role="dialog" aria-label=t("wm.status.title")>{wm::status_panel(p)}</div>
                </details>
            }
            .into_any()
        }
    }
}

/// D002 · O sino abre o painel quando existe; sem painel, a ligação D001.
fn bell(vm: &ShellVm) -> AnyView {
    let badge = vm
        .unread
        .filter(|n| *n > 0)
        .map(|n| view! { <span class="oc-badge">{n.to_string()}</span> });
    match &vm.panels.notifications {
        None => view! {
            <a class="oc-round-btn" href="/notifications" aria-label=t("shell.notifications")>
                {icon("bell")}
                {badge}
            </a>
        }
        .into_any(),
        Some(p) => view! {
            <details class="oc-menu oc-panel-menu" data-oc="menu">
                <summary class="oc-round-btn" aria-label=t("shell.notifications")>{icon("bell")}{badge}</summary>
                <div class="oc-menu__pop oc-panel__pop" role="dialog" aria-label=t("wm.notif.title")>{wm::notifications_panel(p)}</div>
            </details>
        }
        .into_any(),
    }
}

/// D002 · O relógio abre o painel do mês quando existe.
fn clock(vm: &ShellVm) -> AnyView {
    let face = || {
        view! {
            <span class="oc-clock" data-oc="clock" data-format="short">
                <span data-part="clock-date"></span>
                <span data-part="clock-time"></span>
            </span>
        }
    };
    match &vm.panels.clock {
        None => face().into_any(),
        Some(p) => view! {
            <details class="oc-menu oc-panel-menu" data-oc="menu">
                <summary class="oc-clock-btn" aria-label=t("wm.clock.title")>{face()}</summary>
                <div class="oc-menu__pop oc-panel__pop" role="dialog" aria-label=t("wm.clock.title")>{wm::clock_panel(p)}</div>
            </details>
        }
        .into_any(),
    }
}

fn distribution_key(d: Distribution) -> (&'static str, &'static str) {
    match d {
        Distribution::Research => ("dist.research", "shell.dist.research"),
        Distribution::Business => ("dist.business", "shell.dist.business"),
        Distribution::Personal => ("dist.personal", "shell.dist.personal"),
        Distribution::Education => ("dist.education", "shell.dist.education"),
    }
}

/// Code (D010 · S15/S17, transcrito da referência `fixture.js`): mudar de
/// Distribuição, no painel do distintivo, junto dos primeiros passos. Num
/// ponto fixo não se muda aqui: diz-se onde, por endereços configurados.
fn switcher(vm: &ShellVm, active: Distribution) -> Option<AnyView> {
    let sw = vm.dist_switch.as_ref()?;
    if let Some(bound) = &sw.bound {
        let generic = bound.generic_host.clone().unwrap_or_else(|| "—".to_owned());
        let name = t(distribution_key(active).0);
        let gotos = bound
            .targets
            .iter()
            .map(|(_, host, url)| {
                view! {
                    <a class="oc-dist-go" data-part="dist-goto" href=url.clone()>{icon("link")}{tf("dist.switch.goto", &[("host", host.as_str())])}</a>
                }
            })
            .collect_view();
        return Some(
            view! {
                <p class="oc-dist-bound" data-part="dist-bound">{tf("dist.switch.bound", &[("distribution", name), ("generic", generic.as_str())])}</p>
                {gotos}
                {(!bound.targets.is_empty()).then(|| view! { <p class="oc-menu__foot">{t("dist.switch.goto.note")}</p> })}
            }
            .into_any(),
        );
    }
    let items = sw
        .choices
        .iter()
        .map(|d| {
            let d = *d;
            let current = d == active;
            view! {
                <li>
                    <form method="get" action="/distribution/switch">
                        <button type="submit" name="to" value=d.as_str() data-distribution=d.as_str() aria-current=if current { "true" } else { "false" }>
                            {icon(iconography::dist_icon_id(d))}<span>{t(distribution_key(d).0)}</span>
                        </button>
                    </form>
                </li>
            }
        })
        .collect_view();
    Some(
        view! {
            <p class="oc-dist-sub">{t("dist.switch.title")}</p>
            <ul class="oc-dist-sw" data-part="dist-switch">{items}</ul>
        }
        .into_any(),
    )
}

fn context_kind_key(kind: &str) -> &'static str {
    match kind {
        "unit" => "ctx.units",
        "project" => "ctx.projects",
        "personal" => "ctx.personal",
        _ => "ctx.org",
    }
}

fn context_icon(kind: &str) -> &'static str {
    match kind {
        "unit" => "org-tree",
        "project" => "project",
        "personal" => "user",
        _ => "organization",
    }
}

/// Code (D010 · S19–S21, transcrito da referência): o chip de contexto —
/// tipo + nome — e os contextos que o Core diz serem do membro. Distinto do
/// distintivo da Distribuição; mudar de contexto não muda a Distribuição.
fn context_chip(vm: &ShellVm) -> Option<AnyView> {
    let ctx = vm.context.as_ref()?;
    let d = vm.distribution?;
    let dname = t(distribution_key(d).0);
    let (k, n, label) = match &ctx.active {
        Some(a) => {
            let name = if a.kind == "personal" {
                t("ctx.personal").to_owned()
            } else {
                a.name.clone()
            };
            (
                t(context_kind_key(&a.kind)).to_uppercase(),
                name.clone(),
                tf("ctx.change", &[("name", name.as_str())]),
            )
        }
        None => (
            "—".to_owned(),
            t("ctx.none.chip").to_owned(),
            t("ctx.choose").to_owned(),
        ),
    };
    let groups = ["organisation", "unit", "project", "personal"]
        .into_iter()
        .filter_map(|kind| {
            let items: Vec<_> = ctx.items.iter().filter(|c| c.kind == kind).collect();
            if items.is_empty() {
                return None;
            }
            let buttons = items
                .into_iter()
                .map(|c| {
                    let current = ctx.active.as_ref().is_some_and(|a| a.kind == c.kind && a.id == c.id);
                    let name = if c.kind == "personal" { t("ctx.personal").to_owned() } else { c.name.clone() };
                    view! {
                        <form method="post" action="/context">
                            <input type="hidden" name="kind" value=c.kind.clone() />
                            {c.id.clone().map(|id| view! { <input type="hidden" name="id" value=id /> })}
                            <button type="submit" class="oc-ctx-i" aria-current=if current { "true" } else { "false" }>{icon(context_icon(&c.kind))}{name}</button>
                        </form>
                    }
                })
                .collect_view();
            Some(view! { <p class="oc-ctx-g">{t(context_kind_key(kind))}</p>{buttons} })
        })
        .collect_view();
    // S21: o contexto activo deixou de ser do membro — o Core já o repôs; a
    // folha di-lo uma vez e leva a escolher outro (nunca mostra o antigo).
    let revoked = ctx.revoked.clone().map(|name| {
        view! {
            <dialog class="oc-sheet oc-sheet--narrow" data-part="ctx-revoked" data-d010="" data-oc="auto-open" data-cancel="/" aria-labelledby="ctx-revoked-t">
                <header class="oc-sheet__head">
                    <h2 id="ctx-revoked-t">{tf("ctx.revoked.title", &[("name", name.as_str())])}</h2>
                    <a class="oc-round-btn" href="/" aria-label=t("shell.close")>{icon("close")}</a>
                </header>
                <p class="oc-sheet__lead">{t("ctx.revoked.body")}</p>
                <div class="oc-sheet__actions">
                    <a class="oc-btn-solid" href="/" data-oc="dialog-close">{icon("org-tree")}{t("ctx.choose")}</a>
                </div>
            </dialog>
        }
    });
    Some(
        view! {
            {revoked}
            <details class="oc-menu" data-oc="menu">
                <summary class="oc-ctxsw" data-part="ctx-switcher" aria-label=label>
                    <span class="oc-ctxsw__k">{k}</span>
                    <span class="oc-ctxsw__n">{n}</span>
                    {icon("chev-d")}
                </summary>
                <div class="oc-menu__pop oc-menu__pop--ctx" role="dialog" aria-label=tf("ctx.kicker", &[("distribution", dname)])>
                    <p class="oc-dist-sub">{tf("ctx.kicker", &[("distribution", dname.to_uppercase().as_str())])}</p>
                    {groups}
                    <p class="oc-menu__foot">{t("ctx.note")}</p>
                </div>
            </details>
        }
        .into_any(),
    )
}

fn top_bar(vm: &ShellVm) -> impl IntoView {
    // D009 · o distintivo leva o ícone da Distribuição (DIST-03), e o painel
    // diz o que a Distribuição define — primeiros passos, fixações — e que
    // isso não é autorização. Não abre sozinho: não há estado a gravar (§93).
    let dist = vm.distribution.map(|d| {
        let (name_key, desc_key) = distribution_key(d);
        let name = t(name_key);
        let x = distribution::defaults(d);
        let ic = iconography::dist_icon_id(d);
        // Só o que este membro vê: uma fixação sem autorização não aparece (§37).
        let visible = |ids: &[ocinye_contracts::ApplicationId]| -> Vec<(&'static str, String)> {
            ids.iter()
                .filter_map(|p| vm.apps.iter().find(|a| a.id == p.as_str()))
                .map(|a| (app_icon(a.href), a.label.clone()))
                .collect()
        };
        let pins = visible(x.pins);
        let more = visible(x.recommended);
        let chips = |list: Vec<(&'static str, String)>| view! {
            <ul class="oc-dist-pins">
                {list.into_iter().map(|(i, l)| view! { <li>{icon(i)}<span>{l}</span></li> }).collect_view()}
            </ul>
        };
        view! {
            <details class="oc-menu" data-oc="menu">
                <summary class="oc-dist" data-distribution=d.as_str() aria-label=format!("{} · {}", t("auth.distribution"), name) title=name>{icon(ic)}</summary>
                <div class="oc-menu__pop oc-menu__pop--wide oc-menu__pop--dist" role="dialog" aria-label=t("shell.dist.title")>
                    <div class="oc-dist-card">
                        <span class="oc-dist oc-dist--lg" aria-hidden="true">{icon(ic)}</span>
                        <span>
                            <span class="oc-menu__kicker">{t("shell.dist.title")}</span>
                            <strong>{name}</strong>
                        </span>
                    </div>
                    <p class="oc-menu__body">{t(desc_key)}</p>
                    <p class="oc-dist-sub">{t(x.first_title_key)}</p>
                    <ul class="oc-dist-first">
                        <li>{icon("apps")}<span>{t(x.first_body_key)}</span></li>
                        <li>{icon("edit")}<span>{t("dist.first.desk")}</span></li>
                        <li>{icon("nye")}<span>{t("dist.first.nye")}</span></li>
                    </ul>
                    {(!pins.is_empty()).then(|| view! { <p class="oc-dist-sub">{t("dist.pins")}</p>{chips(pins)} })}
                    {(!more.is_empty()).then(|| view! { <p class="oc-dist-sub">{t("dist.first.recommended")}</p>{chips(more)} })}
                    <a class="oc-dist-go" href="#oc-launcher" data-oc="launcher-open">{icon("apps")}{t("dist.first.open_apps")}</a>
                    {switcher(vm, d)}
                    <p class="oc-menu__foot">{t("dist.authority")}" "{t("shell.dist.note")}</p>
                </div>
            </details>
        }
    });
    let initials: String = vm
        .display_name
        .split_whitespace()
        .filter_map(|p| p.chars().next())
        .take(2)
        .flat_map(char::to_uppercase)
        .collect();
    let dist_line = vm.distribution.map(|d| {
        let (name_key, _) = distribution_key(d);
        view! { <span class="oc-account__dist">{t("auth.product")}" · "{t(name_key).to_uppercase()}</span> }
    });
    let crumb = (!vm.crumb.is_empty()).then(|| {
        view! {
            <span class="oc-crumb" aria-hidden="true">"/"</span>
            <span class="oc-crumb oc-crumb--app">{vm.crumb.clone()}</span>
        }
    });
    view! {
        <header class="oc-top">
            <details class="oc-menu" data-oc="menu">
                <summary class="oc-logo" aria-label=t("shell.user.menu") title=vm.display_name.clone()>
                    <img src="/static/ocinye-logo.png" alt="" width="44" height="44" />
                </summary>
                <div class="oc-menu__pop oc-menu__pop--account" role="menu">
                    <div class="oc-account">
                        <span class="oc-account__initials" aria-hidden="true">{initials}</span>
                        <span class="oc-account__text">
                            <strong>{vm.display_name.clone()}</strong>
                            <small>{vm.email.clone()}</small>
                            {dist_line}
                        </span>
                    </div>
                    <a class="oc-menu__item" role="menuitem" href="/settings">{icon("user")}{t("shell.user.account")}</a>
                    <a class="oc-menu__item" role="menuitem" href="/settings">{icon(app_icon("/settings"))}{t("shell.user.settings")}</a>
                    <a class="oc-menu__item" role="menuitem" href="/#appearance" data-oc="appearance-open">{icon("appearance")}{t("shell.user.appearance")}</a>
                    <a class="oc-menu__item" role="menuitem" href="/help">{icon(app_icon("/help"))}{t("shell.user.help")}</a>
                    // Code (D010 · S22): o bloqueio passou a existir — o botão deixa de
                    // estar desactivado e a nota «ainda não disponível» sai.
                    <form method="post" action="/lock" data-oc="lock-form">
                        <button type="submit" class="oc-menu__item" role="menuitem">
                            {icon("lock")}{t("shell.user.lock")}<kbd class="oc-menu__kbd" aria-hidden="true">"⌘ L"</kbd>
                        </button>
                    </form>
                    <hr class="oc-menu__sep" />
                    <form method="post" action="/logout">
                        <button type="submit" class="oc-menu__item oc-menu__item--danger" role="menuitem">{icon("logout")}{t("shell.user.sign_out")}</button>
                    </form>
                </div>
            </details>
            {dist}
            {context_chip(vm)}
            {crumb}
            <form class="oc-nyebar" method="get" action="/ask" role="search">
                <label class="oc-sr" for="oc-q">{t("shell.search.ask")}</label>
                {icon("nye")}
                <input id="oc-q" name="q" type="search" value=vm.query.clone() placeholder=t("shell.search.placeholder") autocomplete="off" />
                {nyebar_mic(vm)}
                <kbd class="oc-kbd" aria-hidden="true">"⌘K"</kbd>
            </form>
            <details class="oc-menu oc-create" data-oc="menu">
                <summary class="oc-create__btn">{icon("plus")}{t("shell.create")}</summary>
                <div class="oc-menu__pop" role="menu">
                    <p class="oc-menu__kicker">{t("shell.create.title")}</p>
                    {CREATE.iter().map(|(href, key, ic)| view! {
                        <a class="oc-menu__item" role="menuitem" href=*href>{icon(ic)}{t(key)}</a>
                    }).collect_view()}
                </div>
            </details>
            {status_control(vm)}
            {bell(vm)}
            {clock(vm)}
        </header>
    }
}

/// A barra de aplicações. D003 · É o único sítio onde as janelas abertas
/// aparecem (não há prateleira em baixo): primeiro as fixadas; depois, a seguir
/// a um separador, as aplicações em execução que não estão fixadas (Nye, …),
/// pela ordem do registo. Fechada a última janela, a não fixada desaparece.
fn dock(vm: &ShellVm) -> impl IntoView {
    let running = |id: &str| {
        vm.wm
            .as_ref()
            .is_some_and(|wm| wm.windows.iter().any(|w| w.app_id == id))
    };
    // Code (D009): as fixações pela ordem fixada (a do membro ou a da
    // Distribuição), e não pela do registo.
    let pinned: Vec<_> = if vm.pin_order.is_empty() {
        vm.apps.iter().filter(|a| a.pinned).cloned().collect()
    } else {
        vm.pin_order
            .iter()
            .filter_map(|id| vm.apps.iter().find(|a| a.pinned && a.id == *id))
            .cloned()
            .collect()
    };
    let extra: Vec<_> = vm
        .apps
        .iter()
        .filter(|a| !a.pinned && running(a.id))
        .cloned()
        .collect();
    let has_extra = !extra.is_empty();
    view! {
        <nav class="oc-dock" data-oc="dock" aria-label=t("shell.dock")>
            <a class="oc-dock__btn" href="/" aria-label=t("shell.dock.home") aria-current=vm.apps.iter().any(|a| a.active && a.href == "/").then_some("page")>{icon("home")}</a>
            <a class="oc-dock__btn oc-dock__brand" href="#oc-launcher" data-oc="launcher-open" aria-label=t("shell.launcher")>{icon("apps-brand-dark")}</a>
            <span class="oc-dock__sep" aria-hidden="true"></span>
            {pinned.into_iter().map(|a| {
                // D002: fixada ≠ em execução ≠ activa. O ponto diz «em execução»;
                // dois pontos, várias janelas; o fundo azul continua a ser «activa».
                let (label, n) = wm::dock_label(vm.wm.as_ref(), a.id, &a.label);
                let run = wm::dock_run(vm.wm.as_ref(), a.id);
                view! {
                    <a
                        class="oc-dock__btn"
                        href=a.href
                        aria-label=label.clone()
                        title=label
                        aria-current=a.active.then_some("page")
                        data-oc=(n > 0).then_some("dock-app")
                        data-app=a.id
                        data-windows=(n > 0).then(|| n.to_string())
                    >
                        {icon(app_icon(a.href))}
                        {run}
                    </a>
                }
            }).collect_view()}
            {has_extra.then(|| view! { <span class="oc-dock__sep" data-part="dock-running" aria-hidden="true"></span> })}
            {extra.into_iter().map(|a| {
                let (label, n) = wm::dock_label(vm.wm.as_ref(), a.id, &a.label);
                let run = wm::dock_run(vm.wm.as_ref(), a.id);
                view! {
                    <a
                        class="oc-dock__btn"
                        href=a.href
                        aria-label=label.clone()
                        title=label
                        aria-current=a.active.then_some("page")
                        data-oc="dock-app"
                        data-app=a.id
                        data-windows=n.to_string()
                        data-running=""
                    >
                        {icon(app_icon(a.href))}
                        {run}
                    </a>
                }
            }).collect_view()}
        </nav>
        <button type="button" class="oc-dock-toggle" data-oc="dock-toggle" aria-label=t("shell.dock.show") hidden>
            {icon("apps-brand-dark")}
        </button>
    }
}

fn launcher(vm: &ShellVm) -> impl IntoView {
    view! {
        <div class="oc-overlay" id="oc-launcher" data-oc="launcher" role="dialog" aria-modal="true" aria-labelledby="oc-launcher-title">
            <a class="oc-overlay__scrim" href="#" aria-label=t("shell.close")></a>
            <div class="oc-launcher">
                <div class="oc-launcher__head">
                    <h2 id="oc-launcher-title">{t("shell.launcher")}</h2>
                    <label class="oc-field">
                        <span class="oc-sr">{t("shell.launcher.search")}</span>
                        {icon("search")}
                        <input type="search" data-part="launcher-q" placeholder=t("shell.launcher.search") autocomplete="off" />
                    </label>
                    <a class="oc-round-btn" href="#" aria-label=t("shell.close")>{icon("close")}</a>
                </div>
                <ul class="oc-launcher__grid">
                    {vm.apps.iter().map(|a| {
                        let label = a.label.clone();
                        let search = format!("{} {}", a.label, a.description).to_lowercase();
                        view! {
                            <li data-part="launcher-item" data-search=search>
                                <a class="oc-tile" href=a.href aria-current=a.active.then_some("page")>
                                    <span class="oc-tile__icon">{icon(app_icon(a.href))}</span>
                                    <span class="oc-tile__name">{label}</span>
                                    <span class="oc-tile__desc">{a.description.clone()}</span>
                                    </a>
                                {a.pinnable.then(|| {
                                    let key = if a.pinned { "shell.launcher.unpin" } else { "shell.launcher.pin" };
                                    view! {
                                        <button
                                            type="button"
                                            class="oc-tile__pin"
                                            data-oc="pin"
                                            data-app=a.id
                                            aria-pressed=if a.pinned { "true" } else { "false" }
                                            aria-label=format!("{} · {}", t(key), a.label)
                                            title=t(key)
                                        >
                                            {icon("star")}
                                        </button>
                                    }
                                })}
                            </li>
                        }
                    }).collect_view()}
                </ul>
                <p class="oc-launcher__empty" data-part="launcher-empty" hidden>{t("shell.launcher.none")}</p>
                {vm.distribution.map(|d| view! {
                    <p class="oc-launcher__dist">
                        {icon(iconography::dist_icon_id(d))}
                        <span>{tf("shell.launcher.dist", &[("distribution", t(distribution_key(d).0))])}" · "{t("dist.authority")}</span>
                    </p>
                })}
                <p class="oc-launcher__note"><a href="/settings/apps">{t("shell.launcher.manage")}</a></p>
            </div>
        </div>
    }
}

/// D003 · Com voz disponível (NYE-11), o microfone abre a Nye em modo de voz;
/// sem ela, é o controlo D001 com a razão.
fn nyebar_mic(vm: &ShellVm) -> AnyView {
    let voice = vm
        .nye
        .as_ref()
        .is_some_and(|n| n.availability.voice_input.is_available());
    if voice {
        return view! {
            <a class="oc-nyebar__mic" href="/ai/prompt?voice=1" aria-label=t("shell.voice") title=t("shell.voice")>{icon("mic")}</a>
        }
        .into_any();
    }
    view! {
        <button type="button" class="oc-nyebar__mic" aria-disabled="true" aria-describedby="oc-voice-pending" aria-label=t("shell.voice") title=t("shell.voice_pending")>{icon("mic")}</button>
        <span class="oc-sr" id="oc-voice-pending">{t("shell.voice_pending")}</span>
    }
    .into_any()
}

/// A paleta de comandos. D003 · D001_COMPONENT_EXTENSION: com `vm.nye` é a
/// superfície universal da Nye (`nye::surface`), que preserva os ganchos, o
/// filtro de aplicações, ⌘K e Esc; com `None` é exactamente a paleta D001.
fn palette(vm: &ShellVm) -> AnyView {
    if let Some(n) = &vm.nye {
        return nye::surface(vm, n).into_any();
    }
    view! {
        <div class="oc-overlay" id="oc-palette" data-oc="palette" role="dialog" aria-modal="true" aria-label=t("shell.palette")>
            <a class="oc-overlay__scrim" href="#" aria-label=t("shell.close")></a>
            <form class="oc-palette" method="get" action="/search">
                <label class="oc-field oc-field--lg">
                    <span class="oc-sr">{t("shell.palette")}</span>
                    {icon("search")}
                    <input name="q" type="search" data-part="palette-q" placeholder=t("shell.palette.placeholder") autocomplete="off" />
                </label>
                <ul class="oc-palette__list">
                    {vm.apps.iter().map(|a| {
                        let search = a.label.to_lowercase();
                        view! {
                            <li data-part="palette-item" data-search=search>
                                <a href=a.href>{icon(app_icon(a.href))}<span>{a.label.clone()}</span></a>
                            </li>
                        }
                    }).collect_view()}
                </ul>
                <p class="oc-palette__foot">
                    <button type="submit" class="oc-btn-plain">{icon("search")}{t("shell.palette.search")}</button>
                    <a href="/ask">{icon("nye")}{t("shell.search.ask")}</a>
                </p>
            </form>
        </div>
    }
    .into_any()
}

/// A casca com `main` dentro. `title` e `icon_name` desenham a barra da janela
/// da aplicação (G-05: sem janelas, a aplicação ocupa a área de trabalho).
pub fn shell(vm: &ShellVm, main: AnyView) -> impl IntoView {
    shell_with_window(vm, main, None)
}

/// D002 · A casca com o gestor de janelas: `desk` fica por baixo (o Desktop, ou
/// só o fundo numa ligação profunda) e `active_body` é o corpo da janela
/// `Ready`. Com `vm.wm = None` o resultado é exactamente o D001.
pub fn shell_with_window(
    vm: &ShellVm,
    desk: AnyView,
    active_body: Option<AnyView>,
) -> impl IntoView {
    let work = match &vm.wm {
        None => view! { <main class="oc-desk__main" id="oc-main">{desk}</main> }.into_any(),
        Some(w) => view! {
            <div class="oc-desk__work">
                <main class="oc-desk__main" id="oc-main">{desk}</main>
                {wm::layer(w, active_body)}
            </div>
        }
        .into_any(),
    };
    view! {
        <div class="oc-shell">
            {top_bar(vm)}
            <div class="oc-desk" data-wall=vm.wallpaper.as_str() data-dim=(vm.dim.min(60) / 5 * 5).to_string() data-wm=vm.wm.is_some().then_some("")>
                {dock(vm)}
                {work}
            </div>
            {launcher(vm)}
            {palette(vm)}
            {vm.wm.as_ref().map(wm::switcher)}
        </div>
    }
}

/// Uma aplicação dentro da casca, com a sua barra de título.
pub fn app_window(title: String, href: &'static str, body: AnyView) -> impl IntoView {
    view! {
        <section class="oc-window" aria-labelledby="oc-window-title">
            <header class="oc-window__bar">
                <span class="oc-window__icon">{icon(app_icon(href))}</span>
                <h1 id="oc-window-title">{title}</h1>
                <span class="oc-top__spacer"></span>
                <a class="oc-round-btn oc-round-btn--sm" href="/" aria-label=t("shell.close")>{icon("close")}</a>
            </header>
            <div class="oc-window__body">{body}</div>
        </section>
    }
}

/// Uma aplicação cujo ecrã ainda não foi entregue: a janela com o estado honesto,
/// para que nenhum «Ver tudo» ou item leve a uma ligação morta (HANDOFF · «14»).
/// D002: com o gestor de janelas, a janela da aplicação vem em `vm.wm` com
/// `WindowContent::Pending`, e esta página só dá a casca por baixo.
pub fn app_pending(vm: &ShellVm, title: String, href: &'static str) -> AnyView {
    if vm.wm.is_some() {
        return shell(vm, ().into_any()).into_any();
    }
    let body = crate::ui::components::pending("oc-app-pending", "shell.app.pending").into_any();
    shell(vm, app_window(title, href, body).into_any()).into_any()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::ui::testing::assert_contracts;
    use crate::ui::view_models::AppTile;

    pub(crate) fn vm() -> ShellVm {
        ShellVm {
            display_name: "Fidel Monteiro".into(),
            email: "f@o.pt".into(),
            apps: vec![
                AppTile {
                    id: "files",
                    href: "/files",
                    label: "Ficheiros".into(),
                    description: "d".into(),
                    pinned: true,
                    pinnable: true,
                    active: false,
                },
                AppTile {
                    id: "notes",
                    href: "/notes",
                    label: "Notas".into(),
                    description: "d".into(),
                    pinned: false,
                    pinnable: true,
                    active: true,
                },
            ],
            unread: None,
            core: Some(Health::Operational),
            ai: Some(Health::Unavailable),
            query: String::new(),
            ..Default::default()
        }
    }

    #[test]
    fn sem_gestor_de_janelas_a_casca_e_a_d001() {
        let html = shell(&vm(), ().into_any()).to_html();
        assert!(
            !html.contains("oc-desk__work")
                && !html.contains("data-wm")
                && !html.contains("oc-dock__run")
        );
        assert!(html.contains(r#"href="/notifications""#) && html.contains(r#"href="/activity""#));
    }

    #[test]
    fn com_janelas_a_barra_mostra_execucao_e_os_paineis_abrem() {
        use crate::ui::view_models::{
            Capability, CapabilityVm, StatusPanelVm, TopPanels, WindowState, WmVm,
        };
        let mut v = vm();
        let mut w = crate::ui::wm::tests::win("a", "files", "/files", true, WindowState::Normal, 1);
        w.content = crate::ui::view_models::WindowContent::Pending;
        v.wm = Some(WmVm {
            windows: vec![w],
            ..Default::default()
        });
        v.panels = TopPanels {
            status: Some(StatusPanelVm {
                overall: Health::Operational,
                capabilities: vec![CapabilityVm {
                    kind: Capability::Core,
                    required: true,
                    state: Some(Health::Operational),
                    detail: None,
                }],
                storage: None,
                detail_href: None,
            }),
            ..Default::default()
        };
        let html = app_pending(&v, "Ficheiros".into(), "/files").to_html();
        assert_contracts(&html);
        assert!(
            html.contains("oc-desk__work")
                && html.contains(r#"data-oc="win""#)
                && html.contains(t("shell.app.pending"))
        );
        assert!(
            html.contains(r#"data-oc="dock-app""#)
                && html.contains(r#"data-windows="1""#)
                && html.contains("oc-dock__run")
        );
        assert!(html.contains("oc-panel__pop") && html.contains(t("wm.status.overall.ok")));
        // D002.1 · o alternador vem depois do .oc-desk (fora do isolation: isolate)
        let desk = html.find(r#"class="oc-desk""#).expect("oc-desk");
        let palette = html.find(r#"data-oc="palette""#).expect("palette");
        let sw = html.find(r#"id="oc-switcher""#).expect("switcher");
        assert!(
            sw > desk && sw > palette,
            "alternador fora do .oc-desk, depois da paleta"
        );
        assert_eq!(html.matches(r#"id="oc-switcher""#).count(), 1);
        assert!(
            !html.contains(r#"class="oc-window""#),
            "sem a janela D001 de página inteira"
        );
    }

    #[test]
    fn a_aplicacao_por_entregar_e_uma_janela_honesta() {
        let html = app_pending(&vm(), "Monitor".into(), "/admin/monitor").to_html();
        assert_contracts(&html);
        assert!(
            html.contains("oc-window")
                && html.contains(t("shell.app.pending"))
                && html.contains(r#"href="/""#)
        );
    }

    #[test]
    fn a_casca_cumpre_os_contratos() {
        let html = shell(&vm(), view! { <p>"x"</p> }.into_any()).to_html();
        assert_contracts(&html);
        assert!(html.contains(r#"action="/ask""#) && html.contains(r#"action="/logout""#));
        assert!(
            html.contains(r#"data-format="short""#) && html.contains("/static/ocinye-logo.png")
        );
        assert!(html.contains(r##"href="#oc-launcher""##));
    }

    #[test]
    fn sem_resposta_do_core_nao_ha_contador() {
        let html = shell(&vm(), view! { <p>"x"</p> }.into_any()).to_html();
        assert!(!html.contains("oc-badge"));
    }

    #[test]
    fn so_as_fixadas_vao_para_a_barra_e_a_activa_tem_aria_current() {
        let html = dock(&vm()).to_html();
        assert!(html.contains(r#"href="/files""#) && !html.contains(r#"href="/notes""#));
        let l = launcher(&vm()).to_html();
        assert!(l.contains(r#"href="/notes" aria-current="page""#));
    }

    #[test]
    fn bloquear_ecra_e_uma_accao_real() {
        // D010 (S22): deixou de ser a lacuna declarada da D001 — é um `POST`.
        let html = top_bar(&vm()).to_html();
        assert!(html.contains(r#"action="/lock""#) && html.contains(r#"data-oc="lock-form""#));
        assert!(!html.contains("oc-lock-pending"));
    }

    #[test]
    fn criar_so_aponta_para_rotas_reais() {
        let html = top_bar(&vm()).to_html();
        for (href, _, _) in CREATE {
            assert!(html.contains(&format!(r#"href="{href}""#)));
        }
    }

    /// D003 · contrato de regressão: sem Nye, a paleta é a D001; com Nye, a
    /// mesma âncora, os mesmos ganchos e a mesma ordem das camadas globais.
    #[test]
    fn a_nye_alarga_a_paleta_sem_mudar_as_camadas() {
        use crate::ui::view_models::{
            NyeAvail, NyeAvailability, NyeLink, NyeReason, NyeSurfaceVm, WindowState, WmVm,
        };
        let base = shell(&vm(), ().into_any()).to_html();
        assert!(base.contains(r#"action="/search""#) && !base.contains("data-nye"));
        let mut v = vm();
        v.nye = Some(NyeSurfaceVm {
            availability: NyeAvailability {
                search: NyeAvail::Available,
                ask: NyeAvail::Unavailable(NyeReason::NoInference),
                act: NyeAvail::Unavailable(NyeReason::NoInference),
                voice_input: NyeAvail::Unavailable(NyeReason::VoiceUnavailable),
                voice_output: NyeAvail::Unavailable(NyeReason::VoiceUnavailable),
                attachments: NyeAvail::Unavailable(NyeReason::CapabilityUnavailable),
                link: NyeLink::Connected,
            },
            intent: None,
            detected: None,
            query: String::new(),
            hits: vec![],
            answer: None,
            context: None,
            continue_href: "/ai/prompt".into(),
            shortcut: None,
            open: false,
        });
        v.wm = Some(WmVm {
            windows: vec![crate::ui::wm::tests::win(
                "a",
                "files",
                "/files",
                true,
                WindowState::Normal,
                1,
            )],
            ..Default::default()
        });
        let html = shell_with_window(&v, ().into_any(), None).to_html();
        assert_contracts(&html);
        assert_eq!(html.matches(r#"id="oc-palette""#).count(), 1);
        assert!(html.contains(r#"data-oc="palette""#) && html.contains(r#"data-part="palette-q""#));
        let desk = html.find(r#"class="oc-desk""#).expect("oc-desk");
        let pal = html.find(r#"id="oc-palette""#).expect("palette");
        let sw = html.find(r#"id="oc-switcher""#).expect("switcher");
        assert!(
            desk < pal && pal < sw,
            "Nye fora do .oc-desk, antes do alternador"
        );
        assert!(html.contains(r#"aria-describedby="oc-voice-pending""#));
        assert!(
            !html.contains("oc-shelf"),
            "sem prateleira: janelas só na barra de aplicações"
        );
    }
}
