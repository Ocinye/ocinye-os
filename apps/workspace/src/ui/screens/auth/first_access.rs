//! Primeiro acesso: definir a palavra-passe definitiva (D11). DESIGN_LOCKED.
//!
//! O único sítio onde «Mostrar» existe: ao definir uma palavra-passe nova.

use leptos::prelude::*;

use super::{core_message, frame, Foot};
use crate::i18n::{t, tf};
use crate::ui::components::icon;
use crate::ui::view_models::FirstAccessVm;

fn password_field(id: &'static str, name: &'static str, label: &'static str) -> impl IntoView {
    view! {
        <label class="oc-pill">
            <span class="oc-sr">{t(label)}</span>
            {icon("lock")}
            <input id=id name=name type="password" autocomplete="new-password" placeholder=t(label) required />
            <button
                type="button"
                class="oc-auth__reveal"
                data-oc="reveal"
                data-oc-target=id
                data-label-show=t("auth.first.show")
                data-label-hide=t("auth.first.hide")
                aria-controls=id
                aria-pressed="false"
            >
                {icon("eye")}
                <span data-part="reveal-label">{t("auth.first.show")}</span>
            </button>
        </label>
    }
}

/// `GET /first-access` e a resposta a um `POST /first-access` recusado.
pub fn first_access(vm: &FirstAccessVm) -> impl IntoView {
    let min = vm.min_length.to_string();
    let card = view! {
        <p class="oc-auth__kicker">{t("auth.first.kicker")}</p>
        <h1 class="oc-auth__title">{t("auth.first.title")}</h1>
        <p class="oc-auth__lead">{t("auth.first.body")}</p>
        <p class="oc-auth__who oc-auth__who--static">
            <span class="oc-auth__who-initial" aria-hidden="true">{vm.display_name.chars().next().map(String::from).unwrap_or_default()}</span>
            <span>{vm.email.clone()}</span>
        </p>
        <form class="oc-auth__form" method="post" action="/first-access">
            {core_message(&vm.error)}
            {password_field("first-password", "password", "auth.first.new")}
            {password_field("first-confirmation", "confirmation", "auth.first.confirm")}
            <ul class="oc-auth__rules" id="first-rules">
                <li>{tf("auth.first.rule_min", &[("min", &min)])}</li>
                <li>{t("auth.first.rule_match")}</li>
            </ul>
            <button type="submit" class="oc-btn oc-btn--gold" aria-describedby="first-rules">
                {t("auth.first.submit")}
                {icon("arrow-r")}
            </button>
        </form>
    }
    .into_any();
    frame(&vm.door, "first-access", false, card, Foot::SignOut)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::testing::assert_contracts;

    fn vm() -> FirstAccessVm {
        FirstAccessVm {
            display_name: "Fidel".into(),
            email: "f@o.pt".into(),
            min_length: 12,
            ..Default::default()
        }
    }

    #[test]
    fn submete_os_nomes_que_o_servidor_le() {
        let html = first_access(&vm()).to_html();
        assert_contracts(&html);
        assert!(html.contains(r#"action="/first-access""#));
        assert!(html.contains(r#"name="password""#) && html.contains(r#"name="confirmation""#));
        assert_eq!(html.matches(r#"autocomplete="new-password""#).count(), 2);
    }

    #[test]
    fn mostrar_existe_e_troca_o_rotulo_traduzido() {
        let html = first_access(&vm()).to_html();
        assert_eq!(html.matches(r#"data-oc="reveal""#).count(), 2);
        assert!(html.contains("data-label-hide="));
    }

    #[test]
    fn tem_saida_e_nao_tem_seletor_de_idioma() {
        let html = first_access(&vm()).to_html();
        assert!(html.contains(r#"action="/logout""#));
        assert!(
            !html.contains("/login/language"),
            "o idioma à porta perdia a sessão restrita"
        );
    }
}
