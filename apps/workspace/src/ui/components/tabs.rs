//! Tabs.
//!
//! Duas formas: pill nas listas e contextual nos cabeçalhos de detalhe
//! (`design/README.md` §7.6).
//!
//! Uma tab que muda os dados mostrados navega para um URL próprio, para
//! continuar a funcionar sem JavaScript e para ser partilhável. Só as tabs cujo
//! conteúdo já veio do servidor alternam no cliente.

use leptos::prelude::*;

/// Uma tab.
pub struct Tab {
    /// Rótulo visível.
    pub label: String,
    /// Destino. `None` quando a tab alterna conteúdo já renderizado.
    pub href: Option<String>,
    /// Se está seleccionada.
    pub active: bool,
}

impl Tab {
    /// Uma tab que navega.
    #[must_use]
    pub fn link(label: impl Into<String>, href: impl Into<String>, active: bool) -> Self {
        Self {
            label: label.into(),
            href: Some(href.into()),
            active,
        }
    }

    /// Uma tab ainda sem destino.
    ///
    /// Renderizada como desactivada em vez de aparentar funcionar: o design
    /// especifica 13 tabs por Research Workspace, e nem todas têm ecrã.
    #[must_use]
    pub fn inert(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            href: None,
            active: false,
        }
    }
}

fn render(
    tabs: Vec<Tab>,
    class: &'static str,
    label: &'static str,
    section: bool,
) -> impl IntoView {
    view! {
        // `data-oc-section-nav`: quando os separadores são âncoras (`#…`) para
        // secções deste ecrã, o `app.js` marca o activo à medida que se rola. Não
        // faz nada para separadores que levam a outro ecrã — não têm `href^="#"`.
        <div class=class role="tablist" aria-label=label data-oc-section-nav="">

            {tabs
                .into_iter()
                .map(|tab| {
                    let selected = if tab.active { "true" } else { "false" };
                    // Numa navegação por secções, o indicador activo canónico é
                    // `aria-current="location"` — assim o separador por omissão
                    // fica marcado **sem JavaScript**, e o `app.js` só o move com
                    // o scroll. Nas pílulas, o activo é `aria-selected`.
                    let current = (section && tab.active).then_some("location");
                    tab.href
                        .map_or_else(
                            || {
                                view! {
                                    <span
                                        class="oc-tab oc-unavailable"
                                        role="tab"
                                        aria-selected="false"
                                        aria-disabled="true"
                                        title=crate::i18n::t("action.not_yet_available")
                                    >
                                        {tab.label.clone()}
                                    </span>
                                }
                                    .into_any()
                            },
                            |href| {
                                view! {
                                    <a
                                        class="oc-tab"
                                        role="tab"
                                        aria-selected=selected
                                        aria-current=current
                                        href=href
                                    >
                                        {tab.label.clone()}
                                    </a>
                                }
                                    .into_any()
                            },
                        )
                })
                .collect_view()}
        </div>
    }
}

/// Tabs em pill, para listas.
pub fn pill_tabs(tabs: Vec<Tab>, label: &'static str) -> impl IntoView {
    render(tabs, "oc-tabs", label, false)
}

/// Tabs contextuais, para cabeçalhos de detalhe.
///
/// Ganham scroll horizontal quando excedem a largura: um Research Workspace tem
/// 13 tabs e o design prevê-o explicitamente. O separador activo por omissão é
/// marcado com `aria-current="location"` para funcionar sem JavaScript.
pub fn context_tabs(tabs: Vec<Tab>, label: &'static str) -> impl IntoView {
    render(tabs, "oc-tabs oc-tabs--ctx", label, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn html(view: impl IntoView) -> String {
        view.into_any().to_html()
    }

    /// Contrato do estado activo de navegação (CLAUDE.md §45, design §7.7).
    ///
    /// A regra visual — superfície azul + branco, sem dourado — vive no CSS; o
    /// que se garante aqui é a **semântica** de que o CSS depende, e que os
    /// leitores de ecrã anunciam: a pílula activa é `aria-selected="true"`, o
    /// separador de secção activo é `aria-current="location"`, e um separador de
    /// secção **não** marca o activo por `aria-selected` (senão competiria com a
    /// âncora e reapareceria o segundo indicador).
    #[test]
    fn o_activo_de_navegacao_e_semantico_e_unico() {
        // Pílula: o activo é `aria-selected="true"`, e não usa `aria-current`.
        let pilulas = html(pill_tabs(
            vec![
                Tab::link("Todos", "/x", true),
                Tab::link("Minhas", "/y", false),
            ],
            "Recortes",
        ));
        assert!(
            pilulas.contains(r#"aria-selected="true""#),
            "a pílula activa não é aria-selected"
        );
        assert!(
            !pilulas.contains("aria-current"),
            "a pílula não devia usar aria-current"
        );

        // Secção: o activo por omissão é `aria-current="location"` — marcado pelo
        // servidor, para funcionar sem JavaScript —, e não fica também
        // aria-selected a competir (o único indicador é a âncora corrente).
        let seccao = html(context_tabs(
            vec![
                Tab::link("Visão geral", "#ws-visao-geral", true),
                Tab::link("Notas", "#ws-notas", false),
            ],
            "Secções",
        ));
        assert!(
            seccao.contains(r#"aria-current="location""#),
            "o separador de secção activo não é aria-current=location (quebraria sem JS)"
        );

        // Nenhum separador emite um sublinhado dourado inline — o dourado como
        // indicador de navegação activa deixou de existir.
        assert!(!seccao.to_lowercase().contains("gold"));
        assert!(!seccao.contains("box-shadow"));
    }
}
