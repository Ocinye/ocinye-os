//! O documento base: `<!doctype html>`, `<head>` e o `<body>` que recebe a vista.
//!
//! DESIGN_LOCKED. Tudo same-origin: fontes em `/static/fonts/`, estilos e
//! scripts em `/static/`. O `runtime.js` carrega antes de qualquer JS de área.

use leptos::prelude::*;

use crate::ui::view_models::{DocumentVm, Surface};

/// Os estilos de cada superfície, por ordem de carga (a base primeiro).
fn stylesheets(surface: Surface) -> &'static [&'static str] {
    match surface {
        Surface::Auth => &["/static/oc-base.css", "/static/oc-auth.css"],
        Surface::Shell => &[
            "/static/oc-base.css",
            "/static/oc-shell.css",
            "/static/oc-desk.css",
            "/static/oc-wm.css",
            "/static/oc-nye.css",
        ],
    }
}

/// Os scripts de cada superfície, sempre depois de `runtime.js`, com `defer`.
fn scripts(surface: Surface) -> &'static [&'static str] {
    match surface {
        Surface::Auth => &["/static/oc-base.js", "/static/oc-auth.js"],
        Surface::Shell => &[
            "/static/oc-base.js",
            "/static/oc-shell.js",
            "/static/oc-desk.js",
            "/static/oc-wm.js",
            "/static/oc-nye.js",
        ],
    }
}

/// Renderiza o documento completo com `body` dentro.
///
/// O título da página chega já traduzido no ViewModel; o sufixo «Ocinye OS»
/// vem do catálogo.
pub fn render(doc: &DocumentVm, body: impl IntoView + 'static) -> String {
    let lang = crate::i18n::current().bcp47();
    let title = crate::i18n::tf("doc.title", &[("page", &doc.title)]);
    let theme = doc.theme.as_str();
    let surface = doc.surface;
    let tree = view! {
        <html lang=lang data-theme=theme data-surface=surface.as_str()>
            <head>
                <meta charset="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1" />
                <meta name="color-scheme" content="light dark" />
                <title>{title}</title>
                <link rel="icon" href="/static/avatars/ocinye.png" />
                <link rel="preload" href="/static/fonts/IBMPlexSans-Regular.woff2" r#as="font" type="font/woff2" crossorigin="" />
                <link rel="preload" href="/static/fonts/IBMPlexSans-SemiBold.woff2" r#as="font" type="font/woff2" crossorigin="" />
                {stylesheets(surface)
                    .iter()
                    .map(|href| view! { <link rel="stylesheet" href=*href /> })
                    .collect_view()}
                <script src="/static/runtime.js" defer></script>
                {scripts(surface)
                    .iter()
                    .map(|src| view! { <script src=*src defer></script> })
                    .collect_view()}
            </head>
            <body>
                <a class="oc-skip" href="#oc-main">{crate::i18n::t("doc.skip")}</a>
                {body}
            </body>
        </html>
    };
    format!("<!doctype html>{}", tree.to_html())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::view_models::Theme;

    fn doc() -> DocumentVm {
        DocumentVm {
            title: "Página".to_owned(),
            surface: Surface::Auth,
            theme: Theme::Dark,
        }
    }

    #[test]
    fn o_documento_nao_tem_estilo_nem_script_inline() {
        let html = render(&doc(), view! { <main id="oc-main"></main> });
        assert!(html.starts_with("<!doctype html>"));
        assert!(!html.contains(&["sty", "le="].concat()));
        assert!(!html.contains("<style"));
        // Todos os <script> têm src.
        assert_eq!(
            html.matches("<script").count(),
            html.matches("<script src=").count()
        );
        assert!(!html.contains(" onclick="));
        crate::ui::testing::assert_contracts(&html);
    }

    #[test]
    fn o_runtime_carrega_antes_do_resto() {
        let html = render(&doc(), view! { <main id="oc-main"></main> });
        let runtime = html.find("/static/runtime.js").expect("runtime.js");
        let base = html.find("/static/oc-base.js").expect("oc-base.js");
        assert!(runtime < base);
    }

    #[test]
    fn tudo_e_same_origin() {
        let html = render(&doc(), view! { <main id="oc-main"></main> });
        assert!(!html.contains("http://") && !html.contains("https://"));
    }
}
