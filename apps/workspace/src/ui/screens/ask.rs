//! A Universal Command Surface — o ecrã de `Search · Ask · Act`.
//!
//! # Porque não é uma janela de conversa
//!
//! O briefing é explícito: **Prompt Everywhere, not Chat Everywhere**
//! (§33). O que aqui aparece são resultados, planos e confirmações — coisas do
//! Ocinye OS —, não um histórico de mensagens. Uma caixa de conversa grande
//! diria que a aplicação é o modelo, e a aplicação não é o modelo (§2).
//!
//! # Porque funciona hoje
//!
//! Esta instalação não tem nenhum nó de IA. `Pesquisar` é determinístico e
//! responde na mesma; `Perguntar` e `Executar` declaram-se indisponíveis com a
//! razão que o Core deu. É a diferença entre AI-native e AI-dependent, e é
//! visível neste ecrã (§66, §188).

use leptos::prelude::*;
use serde_json::Value;

use crate::ui::components::{
    badge, button, classification_badge, empty_state, Button, EmptyState, Tone, Variant,
};
use crate::ui::icon::Icon;
use crate::ui::ods::icone;

/// O que a superfície devolveu.
pub struct AskView {
    /// O que o membro escreveu.
    pub query: String,
    /// A intenção escolhida.
    pub intent: String,
    /// A resposta do Core, ou `Null` quando ainda não se perguntou nada.
    pub outcome: Value,
    /// Se o membro pode sequer usar a assistência.
    pub may_use_ai: bool,
}

/// Texto de um campo, com alternativa.
fn text<'a>(value: &'a Value, key: &str, fallback: &'a str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or(fallback)
}

/// O ecrã.
pub fn ask(view: &AskView) -> impl IntoView {
    let query = view.query.clone();
    let intent = view.intent.clone();
    let outcome = view.outcome.clone();
    let asked = !query.trim().is_empty();

    view! {
        <div class="ods-page">
            <div class="ods-page__head">
                <div>
                    <h1 class="ods-page__title">{crate::i18n::t("ask.title")}</h1>
                    <p class="ods-page__sub">{crate::i18n::t("ask.subtitle")}</p>
                </div>
            </div>

            {command_form(&query, &intent)}

            {if asked {
                // O marcador por onde o Nye (D7) lê a resposta do Core sem
                // depender da apresentação desta página.
                view! {
                    <div data-oc="ask-result" data-kind=text(&outcome, "kind", "").to_owned()>
                        {result(&outcome, view.may_use_ai)}
                    </div>
                }
                .into_any()
            } else {
                empty_state(EmptyState {
                    icon: Icon::Search,
                    title: crate::i18n::t("ask.write_what").to_owned(),
                    body: crate::i18n::t("ask.empty_body").to_owned(),
                    actions: Vec::new(),
                    small: false,
                })
                .into_any()
            }}
        </div>
    }
}

/// O formulário, com as três intenções como escolha explícita.
///
/// Explícita, e não adivinhada: transformar uma frase ambígua em `Executar`
/// é como uma pergunta se torna uma acção que ninguém pediu (§31, §189).
fn command_form(query: &str, intent: &str) -> impl IntoView {
    let query = query.to_owned();
    let selected = intent.to_owned();

    let option_in_form = |value: &'static str, label: &'static str, hint: &'static str| {
        let checked = selected == value;
        let id = format!("intent-{value}");
        let for_id = id.clone();

        view! {
            <label class="ods-seg__opt" for=for_id title=hint>
                <input class="ods-radio" type="radio" id=id name="intent" value=value checked=checked />
                <span>{label}</span>
            </label>
        }
    };

    view! {
        // D12 põe `.ods-search` no próprio formulário. As três intenções ficam
        // por baixo, num `.ods-seg` — escolha explícita e visível, que o D12 não
        // desenha e que não se pode tirar (Q-29) —, e por isso o formulário
        // envolve as duas coisas e a barra é um bloco dentro dele.
        <form class="ods-d12-ask__form" method="get" action="/ask" role="search">
            <div class="ods-search">
                {icone("nye", "")}
                <label class="ods-sr-only" for="ask-q">{crate::i18n::t("ask.field_label")}</label>
                <input
                    class="ods-search__input"
                    id="ask-q"
                    name="q"
                    type="search"
                    value=query
                    placeholder=crate::i18n::t("ask.placeholder")
                    autocomplete="off"
                />
                <button type="submit" class="ods-btn ods-btn--navy ods-btn--sm" data-part="btn">{crate::i18n::t("ask.submit")}</button>
            </div>

            // Escreva naturalmente: a superfície lê a frase. Os três modos
            // ficam visíveis como controlo e como reserva — e uma leitura
            // ambígua cai sempre para pesquisar, que não altera nada
            // (briefing §31, §189).
            <fieldset class="ods-field">
                <legend class="ods-field__label">{crate::i18n::t("ask.write_naturally")}</legend>
                <div class="ods-seg" role="radiogroup">
                    {option_in_form("search", crate::i18n::t("ask.mode.search"), crate::i18n::t("ask.find_always"))}
                    {option_in_form("ask", crate::i18n::t("ask.mode.ask"), crate::i18n::t("ask.ask_something"))}
                    {option_in_form("act", crate::i18n::t("ask.mode.act"), crate::i18n::t("ask.do_something"))}
                </div>
            </fieldset>
        </form>
    }
}

/// A resposta.
fn result(outcome: &Value, may_use_ai: bool) -> impl IntoView {
    match text(outcome, "kind", "") {
        "results" => results(outcome).into_any(),
        "planned" => planned(outcome).into_any(),
        "executed" => executed(outcome).into_any(),
        "unavailable" => unavailable(outcome, may_use_ai).into_any(),
        // Sem resposta reconhecível, o Core não respondeu. Dizê-lo é melhor do
        // que renderizar um vazio que parece «não há nada».
        _ => empty_state(EmptyState {
            icon: Icon::Shield,
            title: crate::i18n::t("ask.core_no_answer").to_owned(),
            body: crate::i18n::t("ask.not_done").to_owned(),
            actions: Vec::new(),
            small: true,
        })
        .into_any(),
    }
}

/// Resultados de pesquisa. Determinísticos, sem modelo.
fn results(outcome: &Value) -> impl IntoView {
    let sources = outcome
        .get("sources")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let withheld = outcome
        .get("withheld_from_inference")
        .and_then(Value::as_u64)
        .unwrap_or(0);

    let count = sources.len();

    view! {
        {(withheld > 0).then(|| view! {
            // «Encontrei coisas que não posso enviar a um modelo» é diferente
            // de «não encontrei nada» (§188).
            <div class="ods-notice" role="status">
                <p>
                    {crate::i18n::tf("ask.withheld", &[("count", &withheld.to_string())])}
                </p>
            </div>
        })}

        {if sources.is_empty() {
            empty_state(EmptyState {
                icon: Icon::EmptyState,
                title: crate::i18n::t("ask.no_results").to_owned(),
                body: crate::i18n::t("ask.no_results_body").to_owned(),
                actions: Vec::new(),
                small: true,
            })
            .into_any()
        } else {
            view! {
                <ol class="ods-d12-results">
                    {sources
                        .into_iter()
                        .map(|source| {
                            let title = text(&source, "title", crate::i18n::t("ask.untitled")).to_owned();
                            let kind = text(&source, "entity_type", "").to_owned();
                            let excerpt = text(&source, "excerpt", "").to_owned();
                            let classification = text(&source, "classification", "").to_owned();

                            view! {
                                <li class="ods-d12-result">
                                    <span>
                                        <span data-oc-content="1">{title}</span>
                                        {(!excerpt.is_empty()).then(|| view! {
                                            <p class="ods-field__hint">{excerpt}</p>
                                        })}
                                    </span>
                                    <span class="ods-app__toolbar-spacer"></span>
                                    {badge(kind, Tone::Gray)}
                                    {classification_badge(&classification)}
                                </li>
                            }
                        })
                        .collect_view()}
                </ol>
                <p class="ods-field__hint">{crate::i18n::tf("ask.count", &[("count", &count.to_string())])}</p>
            }
            .into_any()
        }}
    }
}

/// Um plano, à espera do membro.
fn planned(outcome: &Value) -> impl IntoView {
    let plan = outcome.get("plan").cloned().unwrap_or(Value::Null);
    let requires_approval = outcome
        .get("requires_approval")
        .and_then(Value::as_bool)
        .unwrap_or(true);

    let steps = plan
        .get("steps")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let plan_id = text(&plan, "id", "").to_owned();
    let count = steps.len();

    view! {
        <section class="ods-d12-plan ods-window-surface">
            <h2 class="ods-widget__title">
                {crate::i18n::tf("ask.will_do", &[("count", &count.to_string())])}
            </h2>
            // O plano, e não o raciocínio. O que o modelo pensou não é
            // guardado nem mostrado (§48, §183).
            <ol class="ods-d12-plan__steps">
                {steps
                    .into_iter()
                    .map(|step| {
                        let summary = text(&step, "summary", "").to_owned();
                        let risk = step
                            .get("risk")
                            .and_then(Value::as_str)
                            .unwrap_or("read_only")
                            .to_owned();
                        let tone = match risk.as_str() {
                            "external_effect" | "privileged" => Tone::Warn,
                            "material_mutation" => Tone::Navy,
                            _ => Tone::Gray,
                        };
                        let label = match risk.as_str() {
                            "read_only" => crate::i18n::t("ask.risk.read_only"),
                            "low_impact" => crate::i18n::t("ask.kind.minor"),
                            "material_mutation" => crate::i18n::t("ask.kind.institutional"),
                            "external_effect" => crate::i18n::t("ask.kind.external"),
                            _ => crate::i18n::t("ask.risk.privileged"),
                        };

                        view! {
                            <li>
                                <span data-oc-content="1">{summary}</span>
                                " "
                                {badge(label, tone)}
                            </li>
                        }
                    })
                    .collect_view()}
            </ol>

            {requires_approval.then(|| view! {
                <p class="ods-field__hint">{crate::i18n::t("ask.approval_note")}</p>
            })}

            <div class="ods-settings__actions">
                <form method="post" action=format!("/ask/plans/{plan_id}/execute")>
                    <button type="submit" class="ods-btn ods-btn--primary" data-part="btn">{crate::i18n::t("ask.confirm")}</button>
                </form>
                <form method="post" action=format!("/ask/plans/{plan_id}/reject")>
                    <button type="submit" class="ods-btn" data-part="btn">{crate::i18n::t("ask.cancel")}</button>
                </form>
            </div>
        </section>
    }
}

/// Um plano que correu.
fn executed(outcome: &Value) -> impl IntoView {
    let summary = text(outcome, "summary", "").to_owned();
    let steps = outcome
        .get("plan")
        .and_then(|plan| plan.get("steps"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    view! {
        <section class="ods-d12-plan ods-window-surface" data-done="">
            <h2 class="ods-widget__title">{crate::i18n::t("ask.result_title")}</h2>
            // Factual, sempre. Nunca «tudo feito» quando não foi (§56, §184).
            <p data-oc-content="1">{summary}</p>

            <ol class="ods-d12-plan__steps">
                {steps
                    .into_iter()
                    .map(|step| {
                        let text_summary = text(&step, "summary", "").to_owned();
                        let result = step.get("result").cloned().unwrap_or(Value::Null);
                        let status = text(&result, "status", "not_attempted").to_owned();
                        let detail = text(&result, "detail", "").to_owned();

                        let feito = status == "succeeded";
                        let (tone, label) = match status.as_str() {
                            "succeeded" => (Tone::Ok, crate::i18n::t("ask.status.done")),
                            "dry_run" => (Tone::Blue, crate::i18n::t("ask.kind.simulation")),
                            "permission_denied" => (Tone::Err, crate::i18n::t("ask.no_access")),
                            "capability_unavailable" => (Tone::Warn, crate::i18n::t("ask.status.unavailable")),
                            "not_attempted" => (Tone::Gray, crate::i18n::t("ask.status.not_run")),
                            "approval_required" => (Tone::Warn, crate::i18n::t("ask.status.awaiting")),
                            _ => (Tone::Err, crate::i18n::t("ask.status.failed")),
                        };

                        view! {
                            <li>
                                {feito.then(|| icone("check", ""))}
                                <span data-oc-content="1">{text_summary}</span>
                                " "
                                {badge(label, tone)}
                                {(!detail.is_empty()).then(|| view! {
                                    <p class="ods-field__hint">{detail}</p>
                                })}
                            </li>
                        }
                    })
                    .collect_view()}
            </ol>
        </section>
    }
}

/// Indisponível — e a razão.
fn unavailable(outcome: &Value, may_use_ai: bool) -> impl IntoView {
    let reason = text(outcome, "reason", "").to_owned();
    let alternative = text(outcome, "alternative", "").to_owned();

    // Não «Oops». A razão que o Core deu, e o que continua a funcionar
    // (§188).
    let titulo = if may_use_ai {
        crate::i18n::t("ask.status.not_available")
    } else {
        crate::i18n::t("ask.no_assist_access")
    };
    view! {
        {crate::ui::ods::estado(
            if may_use_ai { crate::ui::ods::Estado::Indisponivel } else { crate::ui::ods::Estado::Recusado },
            titulo.to_owned(),
        )}
        <p class="ods-page__sub" data-oc-content="1">{format!("{reason} {alternative}")}</p>
        {may_use_ai.then(|| button(
            Button::new(crate::i18n::t("ask.see_ai_status"), Variant::Secondary).href("/ai"),
        ))}
    }
}

#[cfg(test)]
mod pureza_i18n {
    use super::*;

    /// Um ecrã, um idioma: a superfície de comando em francês, sem português.
    #[tokio::test]
    async fn a_superficie_de_comando_nao_mistura_linguas() {
        use crate::i18n::{with_locale, Locale};
        let view = AskView {
            query: String::new(),
            intent: "search".to_owned(),
            outcome: Value::Null,
            may_use_ai: true,
        };
        let fr = with_locale(Locale::Fr, async { ask(&view).to_html() }).await;
        for francesa in ["Rechercher, demander ou exécuter", "Demander", "Exécuter"] {
            assert!(fr.contains(francesa), "fr: falta «{francesa}»");
        }
        assert!(!fr.contains("Perguntar"), "fr: chrome português");
        assert!(!fr.contains("Escreva"), "fr: chrome português");
    }
}
