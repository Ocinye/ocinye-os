//! Páginas de erro: 404, 403, 502 (D001.1). DESIGN_LOCKED.
//!
//! Dois invólucros para o mesmo conteúdo: dentro da casca, quando há membro
//! (`in_shell`), e à porta, quando não há (`at_door`). Nunca mostram detalhe
//! técnico: só o código, uma explicação e a referência para o administrador.
//! O estado HTTP e o encaminhamento são do servidor.

use leptos::prelude::*;

use crate::i18n::{t, tf};
use crate::ui::components::icon;
use crate::ui::screens::auth::{frame, Foot};
use crate::ui::shell::shell;
use crate::ui::view_models::{DoorVm, ErrorKind, ErrorVm, ShellVm};

fn keys(kind: ErrorKind) -> (&'static str, &'static str, &'static str) {
    match kind {
        ErrorKind::NotFound => ("search", "error.404.title", "error.404.body"),
        ErrorKind::Forbidden => ("lock", "error.403.title", "error.403.body"),
        ErrorKind::Upstream => ("warning", "error.502.title", "error.502.body"),
    }
}

fn reference(e: &ErrorVm, class: &'static str) -> Option<impl IntoView> {
    e.reference
        .as_ref()
        .map(|r| view! { <p class=class>{tf("error.ref", &[("ref", r)])}</p> })
}

/// Dentro da casca (membro autenticado).
pub fn in_shell(vm: &ShellVm, e: &ErrorVm) -> impl IntoView {
    let (ic, title, body) = keys(e.kind);
    let retry = e
        .retry_href
        .clone()
        .filter(|_| e.kind == ErrorKind::Upstream);
    let main = view! {
        <div class="oc-error" data-kind=e.kind.as_str()>
            <section class="oc-error__card" aria-labelledby="oc-error-title">
                <span class="oc-error__icon">{icon(ic)}</span>
                <p class="oc-error__code">{tf("error.code", &[("code", e.kind.code())])}</p>
                <h1 class="oc-error__title" id="oc-error-title">{t(title)}</h1>
                <p class="oc-error__body">{t(body)}</p>
                <div class="oc-error__actions">
                    {retry.map(|href| view! { <a class="oc-btn-line" href=href>{icon("restart")}{t("error.retry")}</a> })}
                    <a class="oc-btn-solid" href="/">{icon("home")}{t("error.home")}</a>
                </div>
                {reference(e, "oc-error__ref")}
            </section>
        </div>
    }
    .into_any();
    shell(vm, main)
}

/// À porta (sem sessão, ou sem casca possível).
pub fn at_door(door: &DoorVm, e: &ErrorVm) -> impl IntoView {
    let (ic, title, body) = keys(e.kind);
    let retry = e
        .retry_href
        .clone()
        .filter(|_| e.kind == ErrorKind::Upstream);
    let card = view! {
        <span class="oc-auth__icon" data-state=e.kind.as_str()>{icon(ic)}</span>
        <p class="oc-auth__ref">{tf("error.code", &[("code", e.kind.code())])}</p>
        <h1 class="oc-auth__title">{t(title)}</h1>
        <p class="oc-auth__lead">{t(body)}</p>
        <div class="oc-auth__actions">
            {retry.map(|href| view! { <a class="oc-btn oc-btn--gold" href=href>{t("error.retry")}</a> })}
            <a class="oc-btn oc-btn--line" href="/login">{t("error.sign_in")}</a>
        </div>
        {reference(e, "oc-auth__ref")}
    }
    .into_any();
    frame(door, "error", false, card, Foot::Language("/login"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::testing::assert_contracts;

    fn e(kind: ErrorKind) -> ErrorVm {
        ErrorVm {
            kind,
            reference: Some("OC-7F3A".into()),
            retry_href: Some("/files".into()),
        }
    }

    #[test]
    fn cada_erro_tem_codigo_texto_e_referencia_sem_detalhe_tecnico() {
        for k in [
            ErrorKind::NotFound,
            ErrorKind::Forbidden,
            ErrorKind::Upstream,
        ] {
            let (_, title, body) = keys(k);
            for html in [
                in_shell(&ShellVm::default(), &e(k)).to_html(),
                at_door(&DoorVm::default(), &e(k)).to_html(),
            ] {
                assert_contracts(&html);
                assert!(
                    html.contains(k.code()) && html.contains(t(title)) && html.contains(t(body))
                );
                assert!(html.contains("OC-7F3A"));
                let kind = format!(r#"data-kind="{}""#, k.as_str());
                let state = format!(r#"data-state="{}""#, k.as_str());
                assert!(html.contains(&kind) || html.contains(&state));
            }
        }
    }

    #[test]
    fn so_o_502_oferece_tentar_de_novo() {
        assert!(in_shell(&ShellVm::default(), &e(ErrorKind::Upstream))
            .to_html()
            .contains(r#"href="/files""#));
        assert!(!in_shell(&ShellVm::default(), &e(ErrorKind::NotFound))
            .to_html()
            .contains(r#"href="/files""#));
        assert!(!at_door(&DoorVm::default(), &e(ErrorKind::Forbidden))
            .to_html()
            .contains(r#"href="/files""#));
    }
}
