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

use crate::i18n::t;
use crate::ui::components::{app_icon, icon};
use crate::ui::view_models::{Distribution, Health, ShellVm};

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

fn distribution_key(d: Distribution) -> (&'static str, &'static str) {
    match d {
        Distribution::Research => ("dist.research", "shell.dist.research"),
        Distribution::Business => ("dist.business", "shell.dist.business"),
        Distribution::Personal => ("dist.personal", "shell.dist.personal"),
        Distribution::Education => ("dist.education", "shell.dist.education"),
    }
}

fn top_bar(vm: &ShellVm) -> impl IntoView {
    let dist = vm.distribution.map(|d| {
        let (name_key, desc_key) = distribution_key(d);
        let name = t(name_key);
        let code: String = name.chars().take(2).collect();
        view! {
            <details class="oc-menu" data-oc="menu">
                <summary class="oc-dist" aria-label=format!("{} · {}", t("auth.distribution"), name) title=name>{code.clone()}</summary>
                <div class="oc-menu__pop oc-menu__pop--wide" role="dialog" aria-label=t("shell.dist.title")>
                    <div class="oc-dist-card">
                        <span class="oc-dist oc-dist--lg" aria-hidden="true">{code}</span>
                        <span>
                            <span class="oc-menu__kicker">{t("shell.dist.title")}</span>
                            <strong>{name}</strong>
                        </span>
                    </div>
                    <p class="oc-menu__body">{t(desc_key)}</p>
                    <p class="oc-menu__foot">{t("shell.dist.note")}</p>
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
    let unread = vm.unread;
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
                    <a class="oc-menu__item" role="menuitem" href="/settings">{icon("settings")}{t("shell.user.settings")}</a>
                    <a class="oc-menu__item" role="menuitem" href="/#appearance" data-oc="appearance-open">{icon("appearance")}{t("shell.user.appearance")}</a>
                    <a class="oc-menu__item" role="menuitem" href="/help">{icon("help")}{t("shell.user.help")}</a>
                    <button type="button" class="oc-menu__item" role="menuitem" aria-disabled="true" aria-describedby="oc-lock-pending">
                        {icon("lock")}{t("shell.user.lock")}<kbd class="oc-menu__kbd" aria-hidden="true">"⌘ L"</kbd>
                    </button>
                    <p class="oc-menu__note" id="oc-lock-pending">{t("shell.user.lock_pending")}</p>
                    <hr class="oc-menu__sep" />
                    <form method="post" action="/logout">
                        <button type="submit" class="oc-menu__item oc-menu__item--danger" role="menuitem">{icon("logout")}{t("shell.user.sign_out")}</button>
                    </form>
                </div>
            </details>
            {dist}
            {crumb}
            <form class="oc-nyebar" method="get" action="/ask" role="search">
                <label class="oc-sr" for="oc-q">{t("shell.search.ask")}</label>
                {icon("nye")}
                <input id="oc-q" name="q" type="search" value=vm.query.clone() placeholder=t("shell.search.placeholder") autocomplete="off" />
                <button type="button" class="oc-nyebar__mic" aria-disabled="true" aria-describedby="oc-voice-pending" aria-label=t("shell.voice") title=t("shell.voice_pending")>{icon("mic")}</button>
                <span class="oc-sr" id="oc-voice-pending">{t("shell.voice_pending")}</span>
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
            {health_chip(vm.core, vm.ai)}
            <a class="oc-round-btn" href="/notifications" aria-label=t("shell.notifications")>
                {icon("bell")}
                {unread.filter(|n| *n > 0).map(|n| view! { <span class="oc-badge">{n.to_string()}</span> })}
            </a>
            <span class="oc-clock" data-oc="clock" data-format="short">
                <span data-part="clock-date"></span>
                <span data-part="clock-time"></span>
            </span>
        </header>
    }
}

fn dock(vm: &ShellVm) -> impl IntoView {
    let pinned: Vec<_> = vm.apps.iter().filter(|a| a.pinned).cloned().collect();
    view! {
        <nav class="oc-dock" data-oc="dock" aria-label=t("shell.dock")>
            <a class="oc-dock__btn" href="/" aria-label=t("shell.dock.home") aria-current=vm.apps.iter().any(|a| a.active && a.href == "/").then_some("page")>{icon("home")}</a>
            <a class="oc-dock__btn oc-dock__brand" href="#oc-launcher" data-oc="launcher-open" aria-label=t("shell.launcher")>{icon("apps-brand-dark")}</a>
            <span class="oc-dock__sep" aria-hidden="true"></span>
            {pinned.into_iter().map(|a| {
                let label = a.label.clone();
                view! {
                    <a class="oc-dock__btn" href=a.href aria-label=label.clone() title=label aria-current=a.active.then_some("page")>
                        {icon(app_icon(a.href))}
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
                <p class="oc-launcher__note"><a href="/settings/apps">{t("shell.launcher.manage")}</a></p>
            </div>
        </div>
    }
}

fn palette(vm: &ShellVm) -> impl IntoView {
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
}

/// A casca com `main` dentro. `title` e `icon_name` desenham a barra da janela
/// da aplicação (G-05: sem janelas, a aplicação ocupa a área de trabalho).
pub fn shell(vm: &ShellVm, main: AnyView) -> impl IntoView {
    view! {
        <div class="oc-shell">
            {top_bar(vm)}
            <div class="oc-desk" data-wall=vm.wallpaper.as_str() data-dim=(vm.dim.min(60) / 5 * 5).to_string()>
                {dock(vm)}
                <main class="oc-desk__main" id="oc-main">{main}</main>
            </div>
            {launcher(vm)}
            {palette(vm)}
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
pub fn app_pending(vm: &ShellVm, title: String, href: &'static str) -> impl IntoView {
    let body = crate::ui::components::pending("oc-app-pending", "shell.app.pending").into_any();
    shell(vm, app_window(title, href, body).into_any())
}

#[cfg(test)]
mod tests {
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
    fn bloquear_ecra_e_uma_lacuna_honesta() {
        let html = top_bar(&vm()).to_html();
        assert!(html.contains(r#"aria-describedby="oc-lock-pending""#));
    }

    #[test]
    fn criar_so_aponta_para_rotas_reais() {
        let html = top_bar(&vm()).to_html();
        for (href, _, _) in CREATE {
            assert!(html.contains(&format!(r#"href="{href}""#)));
        }
    }
}
