//! Ocinye AI — hub e criação de agentes.
//!
//! **A infraestrutura de IA não existe.** Estes ecrãs mostram estados vazios
//! institucionais, com a arquitectura visual pronta para quando existirem nós —
//! e sem inventar nós, modelos ou métricas (`design/README.md` §6.9, regra 7).

use leptos::prelude::*;
use serde_json::Value;

use crate::ui::components::{
    badge, button, card, classification_badge, empty_state, named_checkbox, pill, radio_group,
    section_head, select, text_field, textarea, Button, EmptyState, RadioOption, Tone, Variant,
};
use crate::ui::components::{context_tabs, Tab};
use crate::ui::icon::{icon, Icon};

fn items(payload: &Value) -> Vec<Value> {
    payload
        .get("items")
        .and_then(Value::as_array)
        .or_else(|| payload.as_array())
        .cloned()
        .unwrap_or_default()
}

/// O hub de IA.
pub fn hub(status: &Value, models: &Value) -> impl IntoView {
    let available = status
        .get("available")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let providers = status.get("providers").and_then(Value::as_i64).unwrap_or(0);
    let message = status
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or(crate::i18n::t("ai.none_available_full"))
        .to_owned();
    let model_count = items(models).len();

    let tabs = vec![
        Tab::link(crate::i18n::t("ai.tab.overview"), "/ai", true),
        Tab::inert(crate::i18n::t("ai.tab.architecture")),
        Tab::inert(crate::i18n::t("ai.tab.capabilities")),
        Tab::inert(crate::i18n::t("ai.tab.models")),
    ];

    view! {
        <div class="oc-band" >
            <div class="oc-head oc-mb-7" >
                <div class="oc-head__text">
                    <h1>{crate::i18n::t("nav.ai")}</h1>
                    <p>{crate::i18n::t("ai.subtitle")}</p>
                </div>
                <div class="oc-head__actions">
                    {button(Button::new(crate::i18n::t("ai.create_agent"), Variant::Secondary).href("/ai/agents/new"))}
                    {button(Button::new(crate::i18n::t("ai.open_prompt"), Variant::Primary).href("/ai/prompt").with_dot())}
                </div>
            </div>
            {context_tabs(tabs, crate::i18n::t("ai.sections"))}
        </div>

        <div class="oc-page">
            <section class="oc-card oc-mb-5" >
                {if available {
                    view! {
                        <div class="oc-card__body">
                            <p>{message.clone()}</p>
                        </div>
                    }
                        .into_any()
                } else {
                    empty_state(EmptyState {
                            icon: Icon::AiHexLg,
                            // O corpo já traz a frase do Core sobre o estado; o
                            // título nomeia-o, em vez de a repetir à letra.
                            title: crate::i18n::t("ai.unavailable_title").to_owned(),
                            body: message.clone(),
                            actions: vec![
                                Button::new(crate::i18n::t("ai.configure"), Variant::Gold).href("/ai/agents/new"),
                                Button::new(crate::i18n::t("ai.view_compute"), Variant::Secondary).href("/compute"),
                            ],
                            small: false,
                        })
                        .into_any()
                }}
            </section>

            <div class="oc-grid oc-grid--4">
                {counter(crate::i18n::t("ai.counter.agents"), items(&Value::Null).len(), crate::i18n::t("ai.view_agents"), "/ai/agents")}
                {counter(crate::i18n::t("ai.counter.models"), model_count, crate::i18n::t("ai.view_models"), "/ai")}
                {counter(crate::i18n::t("ai.counter.conversations"), 0, crate::i18n::t("ai.open_prompt_action"), "/ai/prompt")}
                {counter(
                    crate::i18n::t("ai.counter.resources"),
                    usize::try_from(providers).unwrap_or(0),
                    crate::i18n::t("ai.view_compute"),
                    "/compute",
                )}
            </div>
        </div>
    }
}

fn counter(
    label: &'static str,
    value: usize,
    action: &'static str,
    href: &'static str,
) -> impl IntoView {
    view! {
        <a class="oc-card oc-card--clickable oc-card__body oc-card__body--block" href=href >
            <div class="oc-t-meta" >
                {label}
            </div>
            <div class="oc-t-kpi oc-mt-5 oc-mb-3" >
                {value.to_string()}
            </div>
            <div class="oc-card__action">{action}</div>
        </a>
    }
}

/// Criar Agente IA.
///
/// # Um agente define-se sem modelo
///
/// O formulário pede **capacidade**, nunca um modelo: o AI Gateway mapeia
/// capacidade para modelo como configuração (ADR-0300, briefing §11). É por
/// isso que este ecrã continua utilizável com zero nós registados — o que falta
/// é onde correr, e o estado do agente di-lo depois de criado.
///
/// # O que este ecrã não tem
///
/// Nenhum controlo decorativo. Antes desta auditoria o âmbito era um grupo de
/// `<button>` sem `name` — parecia uma escolha e não submetia nada — e os nomes
/// dos campos não correspondiam ao contrato do Core, pelo que o formulário
/// aparentava guardar sem persistir (briefing §3).
pub fn new_agent(models: &Value, message: Option<String>) -> impl IntoView {
    let has_models = !items(models).is_empty();

    // A razão pela qual a execução não está disponível, dita uma vez e reusada:
    // um controlo desactivado sem explicação é opaco (briefing §53).
    let no_capability = crate::i18n::t("ai.no_capability");

    view! {
        <div class="oc-page oc-page--narrow">
            <div class="oc-head">
                <div class="oc-head__text">
                    <h1>{crate::i18n::t("ai.create_agent_title")}</h1>
                    <p>{crate::i18n::t("ai.new.subtitle")}</p>
                </div>
            </div>

            {message
                .map(|text| {
                    view! { <div class="oc-callout oc-callout--error" role="alert">{text}</div> }
                })}

            {(!has_models)
                .then(|| {
                    view! {
                        <div class="oc-callout oc-callout--warning" role="status">
                            <strong>{crate::i18n::t("ai.no_capability_title")}</strong>
                            <p>
                                {crate::i18n::t("ai.no_capability_body")}
                            </p>
                        </div>
                    }
                })}

            <form method="post" action="/ai/agents/new">
                <div class="oc-grid oc-grid--form">
                    <div>
                        {card(
                            section_head(crate::i18n::t("ai.section.identity"), None, None),
                            view! {
                                {text_field(
                                    "agent-name",
                                    crate::i18n::t("ai.agent.name"),
                                    "name",
                                    crate::i18n::t("ai.agent.name_placeholder"),
                                    "text",
                                )}
                                {textarea(
                                    "agent-purpose",
                                    crate::i18n::t("ai.agent.purpose"),
                                    "purpose",
                                    crate::i18n::t("ai.agent.purpose_placeholder"),
                                    64,
                                )}
                                {textarea(
                                    "agent-instructions",
                                    crate::i18n::t("ai.agent.instructions"),
                                    "instructions",
                                    crate::i18n::t("ai.agent.instructions_placeholder"),
                                    92,
                                )}
                                // Capacidade, e não modelo. O campo «Modelo
                                // base» foi retirado: acoplava a UX a nomes de
                                // modelo, contra o §41 do `CLAUDE.md`.
                                {select(
                                    "agent-capability",
                                    crate::i18n::t("ai.agent.capability"),
                                    "capability",
                                    // Maiúsculas: é a representação de
                                    // `AiCapability` no contrato.
                                    vec![
                                        ("GENERAL".to_owned(), true),
                                        ("CODING".to_owned(), true),
                                        ("REASONING".to_owned(), true),
                                        ("EMBEDDING".to_owned(), true),
                                    ],
                                )}
                                <p class="oc-field__hint">
                                    {crate::i18n::t("ai.capability_hint")}
                                </p>
                            },
                        )}
                    </div>

                    <div>
                        {card(
                            section_head(crate::i18n::t("ai.section.scope"), None, None),
                            view! {
                                {radio_group(
                                    "scope",
                                    crate::i18n::t("ai.scope.legend"),
                                    vec![
                                        RadioOption::new("personal", crate::i18n::t("ai.scope.personal"), true),
                                        RadioOption::new("unit", crate::i18n::t("ai.scope.unit"), false),
                                        RadioOption::new(
                                            "institutional",
                                            crate::i18n::t("ai.scope.institutional"),
                                            false,
                                        ),
                                    ],
                                )}
                                <p class="oc-muted oc-t-caption--muted oc-mt-6" >
                                    {crate::i18n::t("ai.scope.help")}
                                </p>
                            },
                        )}

                        <div class="oc-vspace" ></div>

                        {card(
                            section_head(crate::i18n::t("ai.section.knowledge"), None, None),
                            view! {
                                {named_checkbox(
                                    "k-bib",
                                    "uses_bibliography",
                                    crate::i18n::t("nav.bibliography"),
                                    true,
                                )}
                                {named_checkbox(
                                    "k-docs",
                                    "uses_documents",
                                    crate::i18n::t("ai.knowledge.documents"),
                                    false,
                                )}
                                {named_checkbox("k-data", "uses_datasets", crate::i18n::t("ai.source.datasets"), false)}
                            },
                        )}

                        <div class="oc-vspace" ></div>

                        <section
                            class="oc-card oc-card__body oc-card__body--subtle"
                        >
                            <div class="oc-flex oc-gap-7">
                                <span class="oc-ink">{icon(Icon::Shield, 16)}</span>
                                <div>
                                    <div class="oc-t-strong oc-mb-2" >
                                        {crate::i18n::t("ai.security.title")}
                                    </div>
                                    <p class="oc-t-caption--muted" >
                                        {crate::i18n::t("ai.security.body")}
                                    </p>
                                </div>
                            </div>
                        </section>

                        <div class="oc-row--end oc-gap-5 oc-mt-8" >
                            {button(Button::new(crate::i18n::t("ask.cancel"), Variant::Secondary).href("/ai/agents"))}
                            <button type="submit" class="oc-btn oc-btn--gold">
                                {crate::i18n::t("ai.create_agent")}
                            </button>
                        </div>
                    </div>
                </div>
            </form>

            <p class="oc-muted oc-t-caption--muted oc-mt-6">
                {if has_models {
                    crate::i18n::t("ai.available_when_created")
                } else {
                    no_capability
                }}
            </p>
        </div>
    }
}

fn campo(agent: &Value, key: &str) -> String {
    agent
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("—")
        .to_owned()
}

/// O rótulo em português de uma fonte de conhecimento que o agente usa.
fn fonte(agent: &Value, key: &str, rotulo: &str) -> Option<String> {
    agent
        .get(key)
        .and_then(Value::as_bool)
        .unwrap_or(false)
        .then(|| rotulo.to_owned())
}

/// O detalhe de um agente: a sua definição, e o seu estado real.
///
/// Um agente é definível e persistido **sem nó de IA**; o seu estado é derivado
/// da disponibilidade real (§9). Aqui vê-se o que ele é — capacidade, âmbito,
/// tecto de classificação, fontes de conhecimento — e porque ainda não corre,
/// quando não há capacidade que o sirva. Deixou de ser uma linha morta na lista
/// (F-15).
pub fn agent_detail(agent: &Value) -> impl IntoView {
    let state = campo(agent, "state");
    let state_label = agent
        .get("state_label")
        .and_then(Value::as_str)
        .unwrap_or(&state)
        .to_owned();
    let scope = campo(agent, "scope");
    let classification = campo(agent, "max_classification");
    let execution_available = agent
        .get("execution_available")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let fontes: Vec<String> = [
        fonte(
            agent,
            "uses_bibliography",
            crate::i18n::t("nav.bibliography"),
        ),
        fonte(
            agent,
            "uses_documents",
            crate::i18n::t("ai.source.documents"),
        ),
        fonte(agent, "uses_datasets", crate::i18n::t("ai.source.datasets")),
    ]
    .into_iter()
    .flatten()
    .collect();
    let fontes_texto = if fontes.is_empty() {
        crate::i18n::t("ai.sources.none").to_owned()
    } else {
        fontes.join(" · ")
    };

    let purpose = campo(agent, "purpose");
    let instructions = campo(agent, "instructions");

    view! {
        <div class="oc-band">
            <div class="oc-row oc-row--wrap oc-gap-6 oc-mb-2">
                {pill(crate::i18n::t("ai.detail.pill"))}
                <h1 class="oc-t-screen">{campo(agent, "name")}</h1>
                {badge(state_label, Tone::of(&state))}
            </div>
            <div class="oc-mono oc-mb-5">
                <a href="/ai/agents">{crate::i18n::t("ai.back_to_agents")}</a>
            </div>
        </div>

        <div class="oc-page">
            <div class="oc-grid oc-grid--detail">
                <section class="oc-card">
                    {section_head(crate::i18n::t("ai.detail.definition"), None, None)}
                    <div class="oc-card__body">
                        <p class="oc-t-body">{purpose}</p>
                        <div class="oc-split oc-split--2 oc-mt-5">
                            {metric(crate::i18n::t("ai.metric.capability"), &campo(agent, "capability"))}
                            {metric(crate::i18n::t("ai.metric.scope"), &scope)}
                            {metric(crate::i18n::t("ai.metric.classification_ceiling"), &classification)}
                            {metric(crate::i18n::t("ai.metric.knowledge_sources"), &fontes_texto)}
                            {metric(crate::i18n::t("ai.metric.created_by"), &campo(agent, "created_by_name"))}
                        </div>
                        <div class="oc-field__label oc-mt-6">{crate::i18n::t("ai.detail.instructions")}</div>
                        <p class="oc-t-note">{instructions}</p>
                    </div>
                </section>

                <section class="oc-card">
                    {section_head(crate::i18n::t("ai.detail.execution"), None, None)}
                    <div class="oc-card__body">
                        <div class="oc-row oc-row--wrap oc-gap-6">
                            {classification_badge(&classification)}
                        </div>
                        <p class="oc-t-note oc-mt-5">
                            {if execution_available {
                                crate::i18n::t("ai.execution.available")
                            } else {
                                crate::i18n::t("ai.execution.unavailable")
                            }}
                        </p>
                    </div>
                </section>
            </div>
        </div>
    }
}

/// Uma métrica rotulada, no padrão dos ecrãs de detalhe.
fn metric(label: &'static str, value: &str) -> impl IntoView {
    let value = value.to_owned();
    view! {
        <div class="oc-split__cell">
            <div class="oc-t-cell-2">{value}</div>
            <div class="oc-t-hint oc-mt-1">{label}</div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn o_hub_nao_inventa_nos_nem_modelos() {
        let html = hub(
            &json!({"available": false, "providers": 0, "message": "Nenhum nó de IA Ocinye está actualmente disponível."}),
            &json!({"items": []}),
        )
        .to_html();

        assert!(html.contains("Nenhum nó de IA Ocinye está actualmente disponível"));
        for invented in ["Qwen", "DeepSeek", "GPT", "Claude"] {
            assert!(!html.contains(invented), "o hub não deve nomear {invented}");
        }
    }

    #[test]
    fn o_detalhe_de_um_agente_mostra_a_definicao_e_o_estado_real() {
        let agent = json!({
            "id": "55555555-5555-4555-8555-555555555555",
            "name": "Assistente de Metodologia",
            "purpose": "Ajudar a redigir metodologias.",
            "instructions": "Responde em português; nunca inventa fontes.",
            "capability": "REASONING",
            "scope": "personal",
            "max_classification": "INTERNAL",
            "uses_bibliography": true,
            "uses_documents": false,
            "uses_datasets": true,
            "state": "configured",
            "state_label": "Configurado — sem capacidade disponível",
            "created_by_name": "Ana Fernandes",
            "execution_available": false,
        });
        let html = agent_detail(&agent).to_html();
        assert!(html.contains("Assistente de Metodologia"));
        assert!(html.contains("REASONING"), "falta a capacidade");
        assert!(html.contains("Configurado — sem capacidade disponível"));
        assert!(html.contains("Bibliografia · Datasets"), "faltam as fontes");
        assert!(html.contains("Ana Fernandes"));
        assert!(
            html.contains("correrá assim que existir uma"),
            "falta a explicação de porque não corre"
        );
        assert!(
            html.contains(r#"href="/ai/agents""#),
            "falta a ligação de volta"
        );
        // Nunca inventa um modelo nem um nó.
        for invented in ["Qwen", "DeepSeek", "GPT"] {
            assert!(!html.contains(invented));
        }
    }

    #[test]
    fn o_formulario_pede_capacidade_e_nunca_um_modelo() {
        // Invertido por esta auditoria: existia um selector «Modelo base» que
        // acoplava a UX a nomes de modelo, contra o §41 do `CLAUDE.md` e o §11
        // do briefing. O agente pede uma capacidade; o Gateway escolhe o modelo.
        let html = new_agent(&json!({"items": []}), None).to_html();

        assert!(html.contains(r#"name="capability""#));
        assert!(html.contains(r#"value="GENERAL""#));
        assert!(html.contains(r#"value="REASONING""#));
        assert!(
            !html.contains(r#"name="model""#),
            "o formulário voltou a pedir um modelo"
        );
        assert!(!html.contains("Modelo base"));
    }

    #[test]
    fn sem_no_o_formulario_explica_que_o_agente_fica_guardado() {
        // A decisão do briefing §10: criar é permitido, e o ecrã diz o que
        // acontece a seguir em vez de deixar o membro deduzir.
        let html = new_agent(&json!({"items": []}), None).to_html();
        assert!(html.contains("Sem capacidade de execução"));
        assert!(html.contains("guardado"));
        assert!(html.contains("quando uma capacidade"));
    }

    #[test]
    fn os_campos_do_formulario_usam_o_vocabulario_do_core() {
        // O formulário submetia `description`, `k-bib`, `k-docs`, `k-data` — e
        // o âmbito não era submetido de todo, por ser um grupo de `<button>`
        // sem `name`. Aparentava guardar sem persistir (briefing §3).
        let html = new_agent(&json!({"items": []}), None).to_html();
        for field in [
            r#"name="name""#,
            r#"name="purpose""#,
            r#"name="instructions""#,
            r#"name="capability""#,
            r#"name="scope""#,
            r#"name="uses_bibliography""#,
            r#"name="uses_documents""#,
            r#"name="uses_datasets""#,
        ] {
            assert!(html.contains(field), "falta o campo {field}");
        }
        assert!(!html.contains(r#"name="description""#));
        assert!(!html.contains(r#"name="k-bib""#));
    }

    #[test]
    fn o_ambito_e_submissivel_e_nao_decorativo() {
        let html = new_agent(&json!({"items": []}), None).to_html();
        // Radios, não botões: um `<button type="button">` sem `name` tem
        // aparência de escolha e não submete nada.
        assert!(html.contains(r#"type="radio""#));
        assert!(html.contains(r#"value="personal""#));
        assert!(html.contains(r#"value="institutional""#));
    }

    #[test]
    fn o_botao_criar_submete_o_formulario() {
        let html = new_agent(&json!({"items": []}), None).to_html();
        let form = &html[html.find("<form").expect("formulário")..];
        let form = &form[..form.find("</form>").expect("fim do formulário")];
        assert!(
            form.contains(r#"type="submit""#),
            "«Criar Agente» tem de submeter o formulário"
        );
        assert!(form.contains(r#"action="/ai/agents/new""#));
        assert!(form.contains(r#"method="post""#));
    }

    #[test]
    fn a_recusa_do_core_e_mostrada_no_ecra() {
        let html = new_agent(
            &json!({"items": []}),
            Some("Já existe um agente com este nome neste âmbito.".to_owned()),
        )
        .to_html();
        assert!(html.contains("Já existe um agente com este nome neste âmbito."));
        assert!(html.contains(r#"role="alert""#));
    }

    #[test]
    fn o_painel_de_seguranca_declara_o_limite_real() {
        let html = new_agent(&json!({"items": []}), None).to_html();
        assert!(html.contains("apenas até INTERNAL"));
        assert!(html.contains("Audit Log"));
    }
}

#[cfg(test)]
mod pureza_i18n {
    use super::*;
    use serde_json::json;

    /// Um ecrã, um idioma: o hub, a criação e o detalhe de um agente em francês,
    /// sem chrome português. Cobre o cabeçalho, o formulário e o detalhe — as
    /// três superfícies deste ficheiro.
    #[tokio::test]
    async fn o_ai_nao_mistura_linguas() {
        use crate::i18n::{with_locale, Locale};
        let fr = with_locale(Locale::Fr, async {
            let hub_html = hub(
                &json!({"available": false, "providers": 0}),
                &json!({"items": []}),
            )
            .to_html();
            let novo = new_agent(&json!({"items": []}), None).to_html();
            let detalhe = agent_detail(&json!({
                "name": "Agent",
                "capability": "REASONING",
                "scope": "personal",
                "max_classification": "INTERNAL",
                "uses_bibliography": true,
                "uses_datasets": true,
                "state": "configured",
                "state_label": "Configuré",
                "created_by_name": "Ana",
                "execution_available": false,
            }))
            .to_html();
            format!("{hub_html}{novo}{detalhe}")
        })
        .await;

        for francesa in [
            "Vue d’ensemble",
            "Créer un agent",
            "Périmètre d’accès",
            "Nom de l’agent",
            "Sécurité",
            "Retour aux agents",
            "Sources de connaissance",
        ] {
            assert!(fr.contains(francesa), "fr: falta «{francesa}»");
        }
        for portuguesa in [
            "Visão geral",
            "Nome do agente",
            "Âmbito de acesso",
            "Voltar aos agentes",
            "Segurança",
        ] {
            assert!(
                !fr.contains(portuguesa),
                "fr: chrome português «{portuguesa}» sobreviveu"
            );
        }
    }
}
