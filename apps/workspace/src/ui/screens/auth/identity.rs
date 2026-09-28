//! Sessão sem identidade confirmada (D001.1). DESIGN_LOCKED.
//!
//! Há sessão, mas o Core não confirmou quem é o membro (erro técnico no `/me`,
//! não 401). Falha fechado: nenhuma casca, nenhum conteúdo do espaço de
//! trabalho. Oferece «Tentar de novo» e «Terminar sessão»; mostra só a referência.

use leptos::prelude::*;

use crate::i18n::{t, tf};
use crate::ui::components::icon;
use crate::ui::screens::auth::{frame, Foot};
use crate::ui::view_models::IdentityFailVm;

/// A página, servida com 503 pelo servidor.
pub fn identity_unavailable(vm: &IdentityFailVm) -> impl IntoView {
    let card = view! {
        <span class="oc-auth__icon" data-state="identity">{icon("shield")}</span>
        <h1 class="oc-auth__title">{t("auth.identity.title")}</h1>
        <p class="oc-auth__lead">{t("auth.identity.body")}</p>
        <div class="oc-auth__actions">
            <a class="oc-btn oc-btn--gold" href=vm.retry_href.clone()>{t("error.retry")}</a>
        </div>
        {vm.reference.as_ref().map(|r| view! { <p class="oc-auth__ref">{tf("error.ref", &[("ref", r)])}</p> })}
    }
    .into_any();
    frame(&vm.door, "identity", false, card, Foot::SignOut)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::testing::assert_contracts;

    #[test]
    fn falha_fechado_sem_casca_e_com_saida() {
        let html = identity_unavailable(&IdentityFailVm {
            reference: Some("OC-1".into()),
            retry_href: "/".into(),
            ..Default::default()
        })
        .to_html();
        assert_contracts(&html);
        assert!(html.contains(t("auth.identity.title")) && html.contains("OC-1"));
        assert!(
            html.contains(r#"action="/logout""#)
                && !html.contains("oc-shell")
                && !html.contains("oc-dock")
        );
    }
}
