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
        <div class="oc-card__head">
            <h2>{title}</h2>
            {action.map(|(label, href)| view! { <a class="oc-card__action" href=href>{label}</a> })}
            {meta.map(|meta| view! { <span class="oc-card__meta">{meta}</span> })}
        </div>
    }
}

/// Um cartão com cabeçalho e corpo.
pub fn card(head: impl IntoView + 'static, body: impl IntoView + 'static) -> impl IntoView {
    view! {
        <section class="oc-card">
            {head}
            <div class="oc-card__body">{body}</div>
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
    let positive = delta.as_ref().is_some_and(|d| d.starts_with('+'));
    let delta_class = if positive {
        "oc-kpi__delta oc-kpi__delta--up"
    } else {
        "oc-kpi__delta oc-kpi__delta--down"
    };

    view! {
        <a
            class="oc-card oc-card--clickable oc-card__body oc-card__body--block"
            class:oc-unavailable=indisponivel
            href=href
            title=indisponivel
                .then(|| "O Ocinye Core não respondeu a esta contagem.".to_owned())
        >
            <div class="oc-row--between oc-gap-5" >
                <span class="oc-t-meta" >
                    {label}
                </span>
                <span class=delta_class>{delta.unwrap_or_else(|| "—".to_owned())}</span>
            </div>
            <div class="oc-t-kpi oc-t-kpi--lg oc-mt-5 oc-mb-2" >
                {value.unwrap_or_else(|| "—".to_owned())}
            </div>
            <div class="oc-t-caption" >
                {if indisponivel { "indisponível".to_owned() } else { hint }}
            </div>
        </a>
    }
}
