//! Peças partilhadas por todas as áreas. DESIGN_LOCKED.
//!
//! Só apresentação: recebem dados já prontos e devolvem marcação com classes
//! `oc-*` definidas em `static/oc-base.css`.

use leptos::prelude::*;

/// Um ícone do sprite (`/static/icons.svg#nome`), decorativo.
pub fn icon(name: &'static str) -> impl IntoView {
    let href = format!("/static/icons.svg#{name}");
    view! {
        <svg class="oc-icon" aria-hidden="true" focusable="false">
            <use href=href></use>
        </svg>
    }
}

/// Texto só para leitores de ecrã.
pub fn sr_only(text: impl IntoView + 'static) -> impl IntoView {
    view! { <span class="oc-sr">{text}</span> }
}

/// A referência de um erro do Core: «Não foi possível carregar. Referência OC-…».
pub fn core_error(reference: &str) -> impl IntoView {
    let texto = crate::i18n::tf("state.core_error", &[("ref", reference)]);
    view! {
        <p class="oc-state oc-state--error" role="alert">
            {icon("warning")}
            <span>{texto}</span>
        </p>
    }
}

/// Um estado «ainda não disponível» ligado por `id` aos controlos que explica.
pub fn pending(id: &'static str, text_key: &'static str) -> impl IntoView {
    view! {
        <p class="oc-pending" id=id role="status">
            {icon("clock")}
            <span>{crate::i18n::t(text_key)}</span>
        </p>
    }
}

/// O ícone de uma aplicação, pela sua rota. A escolha visual é do Design.
#[must_use]
pub fn app_icon(href: &str) -> &'static str {
    match href {
        "/" => "home",
        "/my-work" => "work",
        "/notes" => "notes",
        "/calendar" => "calendar",
        "/mail" => "mail",
        "/messages" => "messages",
        "/files" => "files",
        "/knowledge" => "knowledge",
        "/bibliography" => "bibliography",
        "/units" => "units",
        "/ideas" => "idea",
        "/projects" => "project",
        "/datasets" => "data",
        "/ai/prompt" => "nye",
        "/ai" => "ai",
        "/ai/agents" => "agent",
        "/compute" => "compute",
        "/resources" => "workspace",
        "/activity" => "activity",
        "/admin" => "admin",
        "/audit" => "shield",
        "/settings" => "settings",
        "/help" => "help",
        "/terminal" => "terminal",
        // D007.1 · os três que o registo ganhou.
        "/admin/monitor" => "gauge",
        "/results" => "results",
        "/trash" => "trash",
        _ => "apps",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn o_icone_aponta_para_o_sprite_e_e_decorativo() {
        let html = icon("lock").to_html();
        assert!(html.contains(r#"href="/static/icons.svg#lock""#));
        assert!(html.contains(r#"aria-hidden="true""#));
    }

    #[test]
    fn o_erro_do_core_mostra_a_referencia_e_nada_mais() {
        let html = core_error("OC-7F3A").to_html();
        assert!(html.contains("OC-7F3A"));
        assert!(html.contains(r#"role="alert""#));
    }
}
