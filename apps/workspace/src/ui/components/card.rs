//! Cartões, cabeçalhos de secção e cartões de KPI.

use leptos::prelude::*;

/// Um cabeçalho de secção: título à esquerda, acção ou legenda à direita.
pub fn section_head(
    title: impl Into<String>,
    action: Option<(String, String)>,
    meta: Option<String>,
) -> impl IntoView {
    let title = title.into();
    view! {
        <header class="ods-widget__head">
            <span class="ods-widget__titles"><h2 class="ods-widget__title">{title}</h2></span>
            {meta.map(|meta| view! { <span class="ods-label">{meta}</span> })}
            {action.map(|(label, href)| view! { <a class="ods-btn ods-btn--ghost ods-btn--sm" href=href>{label}</a> })}
        </header>
    }
}

/// Um cartão com cabeçalho e corpo.
pub fn card(head: impl IntoView + 'static, body: impl IntoView + 'static) -> impl IntoView {
    view! {
        <section class="ods-widget ods-widget-surface" data-part="card">
            {head}
            <div class="ods-widget__body">{body}</div>
        </section>
    }
}

/// Um indicador do topo do painel.
pub struct Kpi {
    /// Rótulo em maiúsculas, mono.
    pub label: String,
    /// O valor, quando a consulta correu.
    ///
    /// `None` significa que o Ocinye Core não respondeu — e é mostrado como
    /// `—`, nunca como `0`. Um zero afirma que a consulta correu e não
    /// encontrou nada, o que é indistinguível de um acervo vazio e falso quando
    /// o que houve foi uma falha.
    pub value: Option<String>,
    /// Variação face ao período anterior. `None` quando não há variação.
    pub delta: Option<String>,
    /// Explicação curta por baixo do valor.
    pub hint: String,
    /// Destino ao clicar.
    pub href: String,
}

/// Um cartão de KPI.
///
/// A variação usa verde apenas quando é positiva; sem variação fica cinzento —
/// o design evita sugerir movimento onde não houve.
pub fn kpi_card(kpi: Kpi) -> impl IntoView {
    let Kpi {
        label,
        value,
        delta,
        hint,
        href,
    } = kpi;
    let indisponivel = value.is_none();
    view! {
        <a
            class="ods-kpi ods-widget-surface"
            data-part="card"
            href=href
            title=indisponivel.then(|| crate::i18n::t("home.kpi.no_answer").to_owned())
        >
            <span class="ods-kpi__head">
                {label}
                {delta.map(|d| view! { <span class="ods-label">{d}</span> })}
            </span>
            <span>
                <span class="ods-kpi__value">{value.unwrap_or_else(|| "—".to_owned())}</span>
                " "
                <span class="ods-kpi__label">
                    {if indisponivel { crate::i18n::t("home.kpi.unavailable").to_owned() } else { hint }}
                </span>
            </span>
            {indisponivel.then(|| crate::ui::ods::estado(
                crate::ui::ods::Estado::Erro,
                crate::i18n::t("ods.state.error").to_owned(),
            ))}
        </a>
    }
}
