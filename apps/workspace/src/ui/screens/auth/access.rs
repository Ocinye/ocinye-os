//! D010 · Ecrãs de entrada por Distribuição e de ponto de acesso.
//!
//! Code (D010): o pacote do Claude Design não trouxe estas funções em Rust —
//! trouxe a marcação em `reference/d010/d010.js` e `fixture.js` (só classes de
//! produção), e o HANDOFF pede que «as funções Rust produzam a mesma
//! estrutura». Isto é essa transcrição, sem desenho novo: `stop()`, o
//! seletor (S09), os estados sem acesso (S10–S12), os de meio da sessão (S18,
//! S39), o bloqueio (S22) e as páginas servidas fora da Instância (S13, S14,
//! S36, S40).

use leptos::prelude::*;

use super::{frame, Foot};
use crate::access::Stop;
use crate::i18n::{t, tf};
use crate::ui::components::icon;
use crate::ui::view_models::{Distribution, DocumentVm, DoorVm, Surface, Theme};

fn dist_key(d: Distribution) -> &'static str {
    match d {
        Distribution::Research => "dist.research",
        Distribution::Business => "dist.business",
        Distribution::Personal => "dist.personal",
        Distribution::Education => "dist.education",
    }
}

fn dist_lead_key(d: Distribution) -> &'static str {
    match d {
        Distribution::Research => "shell.dist.research",
        Distribution::Business => "shell.dist.business",
        Distribution::Personal => "shell.dist.personal",
        Distribution::Education => "shell.dist.education",
    }
}

/// O nome de uma Distribuição, traduzido.
#[must_use]
pub fn dist_name(d: Distribution) -> &'static str {
    t(dist_key(d))
}

/// `stop()` da referência: o ícone, o título, o texto e o que vem a seguir.
fn stop(icn: &'static str, title: String, body: String, extra: AnyView) -> AnyView {
    view! {
        <span class="oc-auth__stop" aria-hidden="true">{icon(icn)}</span>
        <h1 class="oc-auth__title">{title}</h1>
        <p class="oc-auth__lead">{body}</p>
        {extra}
    }
    .into_any()
}

fn sign_out() -> AnyView {
    view! {
        <form method="post" action="/logout">
            <button type="submit" class="oc-auth__ghost">{icon("logout")}{t("auth.sign_out")}</button>
        </form>
    }
    .into_any()
}

fn back_and_sign_out() -> AnyView {
    view! {
        <div class="oc-access-actions" data-d010="">
            <a class="oc-auth__ghost" href="/distribution">{icon("arrow-l")}{t("dist.back_to_entry")}</a>
            {sign_out()}
        </div>
    }
    .into_any()
}

/// A página de entrada (moldura de autenticação) com um cartão.
fn page(door: &DoorVm, part: &'static str, card: AnyView) -> impl IntoView {
    frame(door, part, false, card, Foot::Language("/"))
}

/// S09 · Escolher a Distribuição: só as activadas ∩ acessíveis, nunca contextos.
pub fn selector(door: &DoorVm, choices: &[Distribution]) -> impl IntoView {
    let items = choices
        .iter()
        .map(|d| {
            let d = *d;
            let name = dist_name(d);
            view! {
                <li>
                    <form method="post" action="/distribution">
                        <input type="hidden" name="distribution" value=d.as_str() />
                        <button
                            type="submit"
                            class="oc-dsel__btn"
                            data-distribution=d.as_str()
                            aria-label=tf("dist.select.enter", &[("distribution", name)])
                        >
                            <span class="oc-dsel__ic" aria-hidden="true">{icon(crate::experience::iconography::dist_icon_id(d))}</span>
                            <span class="oc-dsel__txt"><strong>{name}</strong><span>{t(dist_lead_key(d))}</span></span>
                            {icon("arrow-r")}
                        </button>
                    </form>
                </li>
            }
        })
        .collect_view();
    let card = view! {
        <h1 class="oc-auth__title">{t("dist.select.title")}</h1>
        <p class="oc-auth__lead">{t("dist.select.lead")}</p>
        <ul class="oc-dsel" data-part="dist-selector">{items}</ul>
        {sign_out()}
    }
    .into_any();
    page(door, "auth-r2", card)
}

/// S10 · Conta válida, zero Distribuições acessíveis.
pub fn zero(door: &DoorVm) -> impl IntoView {
    page(
        door,
        "auth-r2",
        stop(
            "lock",
            t("dist.zero.title").to_owned(),
            t("dist.zero.body").to_owned(),
            sign_out(),
        ),
    )
}

/// S11 · Ponto fixo, membro sem acesso a essa Distribuição: recusa, sem
/// oferecer outra.
pub fn bound_no_access(door: &DoorVm, d: Distribution) -> impl IntoView {
    let n = dist_name(d);
    page(
        door,
        "auth-r2",
        stop(
            "lock",
            tf("dist.bound.noaccess.title", &[("distribution", n)]),
            tf("dist.bound.noaccess.body", &[("distribution", n)]),
            sign_out(),
        ),
    )
}

/// S12 · Ponto fixo numa Distribuição desactivada: sem redireccionamento.
pub fn bound_disabled(door: &DoorVm, d: Distribution) -> impl IntoView {
    let n = dist_name(d);
    page(
        door,
        "auth-r2",
        stop(
            "warning",
            tf("dist.bound.disabled.title", &[("distribution", n)]),
            tf("dist.bound.disabled.body", &[("distribution", n)]),
            ().into_any(),
        ),
    )
}

/// S18 · O acesso a esta Distribuição foi retirado a meio da sessão.
pub fn revoked(door: &DoorVm, d: Distribution) -> impl IntoView {
    let n = dist_name(d);
    page(
        door,
        "auth-r2",
        stop(
            "lock",
            tf("dist.revoked.title", &[("distribution", n)]),
            tf("dist.revoked.body", &[("distribution", n)]),
            back_and_sign_out(),
        ),
    )
}

/// S39 · A Distribuição foi desactivada a meio da sessão.
pub fn disabled_live(door: &DoorVm, d: Distribution) -> impl IntoView {
    let n = dist_name(d);
    page(
        door,
        "auth-r2",
        stop(
            "warning",
            tf("dist.disabled_live.title", &[("distribution", n)]),
            tf("dist.disabled_live.body", &[("distribution", n)]),
            back_and_sign_out(),
        ),
    )
}

/// S22 · Sessão bloqueada.
pub fn lock(door: &DoorVm, name: &str, failed: bool) -> impl IntoView {
    let error = failed.then(|| {
        view! {
            <p class="oc-auth__error" role="alert">{icon("warning")}<span>{t("lock.failed")}</span></p>
        }
    });
    let card = view! {
        <h1 class="oc-auth__title">{t("lock.title")}</h1>
        <p class="oc-auth__lead">{format!("{name} · {}", t("lock.body"))}</p>
        {error}
        <form class="oc-auth__form" method="post" action="/unlock" data-d010="">
            <label class="oc-pill">
                <span class="oc-sr">{t("lock.password")}</span>
                {icon("lock")}
                <input id="unlock-password" name="password" type="password" autocomplete="current-password" placeholder=t("lock.password") required="" autofocus="" />
                <button type="submit" class="oc-go" aria-label=t("lock.unlock")>{icon("arrow-r")}</button>
            </label>
        </form>
        <form method="post" action="/logout" data-d010="">
            <button type="submit" class="oc-auth__ghost">{icon("logout")}{tf("lock.other", &[("name", name)])}</button>
        </form>
    }
    .into_any();
    page(door, "auth-r2", card)
}

/// As páginas servidas **fora** da Instância: um documento inteiro, sem
/// identidade da Instância, sem dados, sem redireccionamento.
#[must_use]
pub fn stop_document(which: &Stop) -> String {
    let door = DoorVm::default();
    let (title_key, card) = match which {
        Stop::Unknown(host) => (
            "endpoint.unknown.title",
            stop(
                "warning",
                t("endpoint.unknown.title").to_owned(),
                t("endpoint.unknown.body").to_owned(),
                view! {
                    <p class="oc-auth__host" data-part="host">{host.clone()}</p>
                    <p class="oc-auth__lead">{t("endpoint.unknown.hint")}</p>
                }
                .into_any(),
            ),
        ),
        Stop::Misconfigured(host) => (
            "endpoint.misconfig.title",
            stop(
                "warning",
                t("endpoint.misconfig.title").to_owned(),
                t("endpoint.misconfig.body").to_owned(),
                view! {
                    <p class="oc-auth__host" data-part="host">{host.clone()}</p>
                    <p class="oc-auth__lead">{t("endpoint.misconfig.hint")}</p>
                }
                .into_any(),
            ),
        ),
        Stop::CoreDown => (
            "core.host.title",
            stop(
                "warning",
                t("core.host.title").to_owned(),
                t("core.host.body").to_owned(),
                view! {
                    <div class="oc-access-actions" data-d010="">
                        <a class="oc-btn-solid" href="/">{icon("refresh")}{t("core.host.retry")}</a>
                    </div>
                }
                .into_any(),
            ),
        ),
        Stop::EndpointDisabled(_) => (
            "endpoint.disabled_live.title",
            stop(
                "warning",
                t("endpoint.disabled_live.title").to_owned(),
                t("endpoint.disabled_live.body").to_owned(),
                view! { <p class="oc-auth__lead">{t("endpoint.disabled_live.hint")}</p> }
                    .into_any(),
            ),
        ),
    };
    let doc = DocumentVm {
        title: t(title_key).to_owned(),
        surface: Surface::Auth,
        theme: Theme::Dark,
    };
    crate::ui::document::render(
        &doc,
        frame(&door, "auth-r2", false, card, Foot::Language("/")),
    )
}
