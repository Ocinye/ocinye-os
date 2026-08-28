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
                    <h1>"Pesquisar, perguntar ou executar"</h1>
                    <p>
                        "Escreva o que procura, o que quer saber, ou o que pretende que seja
                         feito. Nada é executado sem a sua confirmação."
                    </p>
                </div>
            </div>

            {command_form(&query, &intent)}

            {if asked {
                result(&outcome, view.may_use_ai).into_any()
            } else {
                empty_state(EmptyState {
                    icon: Icon::Search,
                    title: "Escreva o que procura".to_owned(),
                    body: "Pesquisar funciona sempre. Perguntar e executar dependem de uma \
                           capacidade de IA do Ocinye OS."
                        .to_owned(),
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
                <label class="oc-sr" for="ask-q">"O que procura ou pretende"</label>
                <input
                    class="oc-input"
                    id="ask-q"
                    name="q"
                    type="search"
                    value=query
                    placeholder="Pesquisar, perguntar ou executar no Ocinye…"
                    autocomplete="off"
                />
                <button type="submit" class="oc-btn oc-btn--primary">"Executar"</button>
            </div>

            // Escreva naturalmente: a superfície lê a frase. Os três modos
            // ficam visíveis como controlo e como reserva — e uma leitura
            // ambígua cai sempre para pesquisar, que não altera nada
            // (briefing §31, §189).
            <fieldset class="oc-ask__intents">
                <legend class="oc-ask__legend">
                    "Escreva naturalmente. Pode também escolher o que pretende:"
                </legend>
                {option("search", "Pesquisar", "Encontrar. Funciona sempre.")}
                {option("ask", "Perguntar", "Sobre o trabalho da instituição.")}
                {option("act", "Executar", "Pedir que algo seja feito.")}
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
            title: "O Ocinye Core não respondeu".to_owned(),
            body: "O pedido não foi concluído. Nada foi alterado.".to_owned(),
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
                    {format!(
                        "{withheld} resultado(s) que pode consultar não podem ser \
                         processados por um modelo, pela sua classificação."
                    )}
                </p>
            </div>
        })}

        {if sources.is_empty() {
            empty_state(EmptyState {
                icon: Icon::EmptyState,
                title: "Nenhum resultado".to_owned(),
                body: "Nada no acervo institucional a que tenha acesso corresponde a \
                       este termo."
                    .to_owned(),
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
                            let title = text(&source, "title", "(sem título)").to_owned();
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
                <p class="oc-ask__count">{format!("{count} resultado(s).")}</p>
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
            format!("O Ocinye vai realizar {count} acção(ões)"),
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
                            "read_only" => "Consulta",
                            "low_impact" => "Alteração menor",
                            "material_mutation" => "Alteração institucional",
                            "external_effect" => "Efeito externo",
                            _ => "Privilegiada",
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
                <p class="oc-ask__note-text">
                    "Uma ou mais destas acções têm efeito externo ou alteram estado
                     institucional. Nada acontece sem a sua confirmação."
                </p>
            })}

            <div class="oc-plan__actions">
                <form method="post" action=format!("/ask/plans/{plan_id}/execute")>
                    <button type="submit" class="oc-btn oc-btn--primary">"Confirmar"</button>
                </form>
                <form method="post" action=format!("/ask/plans/{plan_id}/reject")>
                    <button type="submit" class="oc-btn oc-btn--secondary">"Cancelar"</button>
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
        section_head("Resultado", None, None),
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
                            "succeeded" => (Tone::Ok, "Concluída"),
                            "dry_run" => (Tone::Blue, "Simulação"),
                            "permission_denied" => (Tone::Err, "Sem acesso"),
                            "capability_unavailable" => (Tone::Warn, "Indisponível"),
                            "not_attempted" => (Tone::Gray, "Não executada"),
                            "approval_required" => (Tone::Warn, "Aguarda confirmação"),
                            _ => (Tone::Err, "Falhou"),
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
            "Ainda não disponível".to_owned()
        } else {
            "Não possui acesso à assistência".to_owned()
        },
        // Não «Oops». A razão que o Core deu, e o que continua a funcionar
        // (§188).
        body: format!("{reason} {alternative}"),
        actions: if may_use_ai {
            vec![Button::new("Ver o estado da inteligência", Variant::Secondary).href("/ai")]
        } else {
            Vec::new()
        },
        small: false,
    })
}
