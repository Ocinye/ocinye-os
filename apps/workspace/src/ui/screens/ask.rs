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
    badge, card, classification_badge, empty_state, section_head, Button, EmptyState, Tone, Variant,
};
use crate::ui::icon::{icon, Icon};

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
        <div class="oc-page">
            <div class="oc-head">
                <div class="oc-head__text">
                    <h1>{crate::i18n::t("ask.title")}</h1>
                    <p>{crate::i18n::t("ask.subtitle")}</p>
                </div>
            </div>

            {command_form(&query, &intent)}

            {if asked {
                result(&outcome, view.may_use_ai).into_any()
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

    let option = |value: &'static str, label: &'static str, hint: &'static str| {
        let checked = selected == value;
        let id = format!("intent-{value}");
        let for_id = id.clone();

        view! {
            <label class="oc-intent" for=for_id>
                <input type="radio" id=id name="intent" value=value checked=checked />
                <span class="oc-intent__label">{label}</span>
                <span class="oc-intent__hint">{hint}</span>
            </label>
        }
    };

    view! {
        <form class="oc-ask" method="get" action="/ask" role="search">
            <div class="oc-ask__field">
                {icon(Icon::Search, 15)}
                <label class="oc-sr" for="ask-q">{crate::i18n::t("ask.field_label")}</label>
                <input
                    class="oc-input"
                    id="ask-q"
                    name="q"
                    type="search"
                    value=query
                    placeholder=crate::i18n::t("ask.placeholder")
                    autocomplete="off"
                />
                <button type="submit" class="oc-btn oc-btn--primary">{crate::i18n::t("ask.submit")}</button>
            </div>

            // Escreva naturalmente: a superfície lê a frase. Os três modos
            // ficam visíveis como controlo e como reserva — e uma leitura
            // ambígua cai sempre para pesquisar, que não altera nada
            // (briefing §31, §189).
            <fieldset class="oc-ask__intents">
                <legend class="oc-ask__legend">
                    {crate::i18n::t("ask.write_naturally")}
                </legend>
                {option("search", crate::i18n::t("ask.mode.search"), crate::i18n::t("ask.find_always"))}
                {option("ask", crate::i18n::t("ask.mode.ask"), crate::i18n::t("ask.ask_something"))}
                {option("act", crate::i18n::t("ask.mode.act"), crate::i18n::t("ask.do_something"))}
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
            <div class="oc-callout oc-ask__note" role="status">
                {icon(Icon::Shield, 15)}
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
                <ul class="oc-ask__results">
                    {sources
                        .into_iter()
                        .map(|source| {
                            let title = text(&source, "title", crate::i18n::t("ask.untitled")).to_owned();
                            let kind = text(&source, "entity_type", "").to_owned();
                            let excerpt = text(&source, "excerpt", "").to_owned();
                            let classification = text(&source, "classification", "").to_owned();

                            view! {
                                <li class="oc-ask__result">
                                    <div class="oc-ask__result-top">
                                        <span class="oc-ask__result-title">{title}</span>
                                        {badge(kind, Tone::Gray)}
                                        {classification_badge(&classification)}
                                    </div>
                                    {(!excerpt.is_empty()).then(|| view! {
                                        <p class="oc-ask__excerpt">{excerpt}</p>
                                    })}
                                </li>
                            }
                        })
                        .collect_view()}
                </ul>
                <p class="oc-ask__count">{crate::i18n::tf("ask.count", &[("count", &count.to_string())])}</p>
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

    card(
        section_head(
            crate::i18n::tf("ask.will_do", &[("count", &count.to_string())]),
            None,
            None,
        ),
        view! {
            // O plano, e não o raciocínio. O que o modelo pensou não é
            // guardado nem mostrado (§48, §183).
            <ol class="oc-plan">
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
                            <li class="oc-plan__step">
                                <span class="oc-plan__summary">{summary}</span>
                                {badge(label, tone)}
                            </li>
                        }
                    })
                    .collect_view()}
            </ol>

            {requires_approval.then(|| view! {
                <p class="oc-ask__note-text">{crate::i18n::t("ask.approval_note")}</p>
            })}

            <div class="oc-plan__actions">
                <form method="post" action=format!("/ask/plans/{plan_id}/execute")>
                    <button type="submit" class="oc-btn oc-btn--primary">{crate::i18n::t("ask.confirm")}</button>
                </form>
                <form method="post" action=format!("/ask/plans/{plan_id}/reject")>
                    <button type="submit" class="oc-btn oc-btn--secondary">{crate::i18n::t("ask.cancel")}</button>
                </form>
            </div>
        },
    )
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

    card(
        section_head(crate::i18n::t("ask.result_title"), None, None),
        view! {
            // Factual, sempre. Nunca «tudo feito» quando não foi (§56, §184).
            <p class="oc-ask__summary">{summary}</p>

            <ol class="oc-plan">
                {steps
                    .into_iter()
                    .map(|step| {
                        let text_summary = text(&step, "summary", "").to_owned();
                        let result = step.get("result").cloned().unwrap_or(Value::Null);
                        let status = text(&result, "status", "not_attempted").to_owned();
                        let detail = text(&result, "detail", "").to_owned();

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
                            <li class="oc-plan__step">
                                <div class="oc-plan__step-body">
                                    <span class="oc-plan__summary">{text_summary}</span>
                                    {(!detail.is_empty()).then(|| view! {
                                        <p class="oc-plan__detail">{detail}</p>
                                    })}
                                </div>
                                {badge(label, tone)}
                            </li>
                        }
                    })
                    .collect_view()}
            </ol>
        },
    )
}

/// Indisponível — e a razão.
fn unavailable(outcome: &Value, may_use_ai: bool) -> impl IntoView {
    let reason = text(outcome, "reason", "").to_owned();
    let alternative = text(outcome, "alternative", "").to_owned();

    empty_state(EmptyState {
        icon: if may_use_ai { Icon::Ai } else { Icon::Shield },
        title: if may_use_ai {
            crate::i18n::t("ask.status.not_available").to_owned()
        } else {
            crate::i18n::t("ask.no_assist_access").to_owned()
        },
        // Não «Oops». A razão que o Core deu, e o que continua a funcionar
        // (§188).
        body: format!("{reason} {alternative}"),
        actions: if may_use_ai {
            vec![Button::new(crate::i18n::t("ask.see_ai_status"), Variant::Secondary).href("/ai")]
        } else {
            Vec::new()
        },
        small: false,
    })
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
