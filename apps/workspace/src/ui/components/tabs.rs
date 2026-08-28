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

fn render(tabs: Vec<Tab>, class: &'static str, label: &'static str) -> impl IntoView {
    view! {
        <div class=class role="tablist" aria-label=label>
            {tabs
                .into_iter()
                .map(|tab| {
                    let selected = if tab.active { "true" } else { "false" };
                    tab.href
                        .map_or_else(
                            || {
                                view! {
                                    <span
                                        class="oc-tab oc-unavailable"
                                        role="tab"
                                        aria-selected="false"
                                        aria-disabled="true"
                                        title="Ainda não disponível"
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
    render(tabs, "oc-tabs", label)
}

/// Tabs contextuais, para cabeçalhos de detalhe.
///
/// Ganham scroll horizontal quando excedem a largura: um Research Workspace tem
/// 13 tabs e o design prevê-o explicitamente.
pub fn context_tabs(tabs: Vec<Tab>, label: &'static str) -> impl IntoView {
    render(tabs, "oc-tabs oc-tabs--ctx", label)
}
