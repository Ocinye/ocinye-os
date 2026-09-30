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
        // D008 · o Browser.
        "/browser" => "browser",
        // D007.1 · os três que o registo ganhou.
        "/admin/monitor" => "gauge",
        "/results" => "results",
        "/trash" => "trash",
        _ => "apps",
    }
}

/// Texto de dados com os caracteres de controlo (C0, C1, ESC, bidi) visíveis
/// como símbolos, nunca interpretados. Igual a `visible()` em `oc-terminal.js`.
///
/// D008 · vive aqui, e não no Terminal, para que o Terminal e o Browser não se
/// refiram um ao outro (ADR-0623 §5): é uma função pura de texto.
#[must_use]
pub fn visible(s: &str) -> String {
    s.chars()
        .map(|c| {
            let n = c as u32;
            match n {
                0x1b => "␛".to_owned(),
                0x00..=0x08 | 0x0b..=0x1f => {
                    char::from_u32(0x2400 + n).map_or_else(String::new, String::from)
                }
                0x7f => "␡".to_owned(),
                0x80..=0x9f | 0x200e | 0x200f | 0x202a..=0x202e | 0x2066..=0x2069 => {
                    format!("⟨U+{n:04X}⟩")
                }
                _ => c.to_string(),
            }
        })
        .collect()
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
