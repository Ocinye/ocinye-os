//! Ocinye AI — hub e criação de agentes.
//!
//! **A infraestrutura de IA não existe.** Estes ecrãs mostram estados vazios
//! institucionais, com a arquitectura visual pronta para quando existirem nós —
//! e sem inventar nós, modelos ou métricas (`design/README.md` §6.9, regra 7).

use leptos::prelude::*;
use serde_json::Value;

use crate::ui::components::{
    button, card, empty_state, named_checkbox, radio_group, section_head, select, text_field,
    textarea, Button, EmptyState, RadioOption, Variant,
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
        .unwrap_or(
            "Nenhum nó de IA Ocinye está actualmente disponível. A plataforma funciona \
             integralmente sem um, e nenhum fornecedor externo é usado em substituição.",
        )
        .to_owned();
    let model_count = items(models).len();

    let tabs = vec![
        Tab::link("Visão geral", "/ai", true),
        Tab::inert("Arquitectura"),
        Tab::inert("Capacidades"),
        Tab::inert("Modelos"),
    ];

    view! {
        <div class="oc-band" >
            <div class="oc-head oc-mb-7" >
                <div class="oc-head__text">
                    <h1>"Ocinye AI"</h1>
                    <p>"A inteligência artificial é uma capacidade transversal da Ocinye."</p>
                </div>
                <div class="oc-head__actions">
                    {button(Button::new("Criar Agente", Variant::Secondary).href("/ai/agents/new"))}
                    {button(Button::new("Abrir Prompt", Variant::Primary).href("/ai/prompt").with_dot())}
                </div>
            </div>
            {context_tabs(tabs, "Secções de Ocinye AI")}
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
                            title: "Inteligência ainda não disponível".to_owned(),
                            body: message.clone(),
                            actions: vec![
                                Button::new("Configurar IA", Variant::Gold).href("/ai/agents/new"),
                                Button::new("Ver computação", Variant::Secondary).href("/compute"),
                            ],
                            small: false,
                        })
                        .into_any()
                }}
            </section>

            <div class="oc-grid oc-grid--4">
                {counter("AGENTES IA", items(&Value::Null).len(), "Ver agentes", "/ai/agents")}
                {counter("MODELOS", model_count, "Ver modelos", "/ai")}
                {counter("CONVERSAS", 0, "Abrir prompt", "/ai/prompt")}
                {counter(
                    "RECURSOS",
                    usize::try_from(providers).unwrap_or(0),
                    "Ver computação",
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
    let no_capability = "Nenhum nó de IA Ocinye está registado. O agente será guardado e \
                         ficará executável quando uma capacidade compatível estiver activa.";

    view! {
        <div class="oc-page oc-page--narrow">
            <div class="oc-head">
                <div class="oc-head__text">
                    <h1>"Criar Agente IA"</h1>
                    <p>"Um agente actua dentro do âmbito e da classificação que lhe forem dados."</p>
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
                            <strong>"Sem capacidade de execução"</strong>
                            <p>
                                "Nenhum nó de IA Ocinye está actualmente registado. O agente será
                                 guardado e ficará disponível para execução quando uma capacidade
                                 de IA compatível estiver activa."
                            </p>
                        </div>
                    }
                })}

            <form method="post" action="/ai/agents/new">
                <div class="oc-grid oc-grid--form">
                    <div>
                        {card(
                            section_head("IDENTIDADE", None, None),
                            view! {
                                {text_field(
                                    "agent-name",
                                    "Nome do agente",
                                    "name",
                                    "Ex.: Assistente de Pesquisa",
                                    "text",
                                )}
                                {textarea(
                                    "agent-purpose",
                                    "Propósito",
                                    "purpose",
                                    "Para que serve este agente",
                                    64,
                                )}
                                {textarea(
                                    "agent-instructions",
                                    "Instruções gerais",
                                    "instructions",
                                    "Como deve responder e a que se deve limitar",
                                    92,
                                )}
                                // Capacidade, e não modelo. O campo «Modelo
                                // base» foi retirado: acoplava a UX a nomes de
                                // modelo, contra o §41 do `CLAUDE.md`.
                                {select(
                                    "agent-capability",
                                    "Capacidade principal",
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
                                    "O agente pede uma capacidade. O Ocinye AI Gateway escolhe o
                                     modelo que a serve, como configuração."
                                </p>
                            },
                        )}
                    </div>

                    <div>
                        {card(
                            section_head("ÂMBITO DE ACESSO", None, None),
                            view! {
                                {radio_group(
                                    "scope",
                                    "Âmbito do agente",
                                    vec![
                                        RadioOption::new("personal", "Pessoal", true),
                                        RadioOption::new("unit", "Unidade", false),
                                        RadioOption::new(
                                            "institutional",
                                            "Institucional",
                                            false,
                                        ),
                                    ],
                                )}
                                <p class="oc-muted oc-t-caption--muted oc-mt-6" >
                                    "O âmbito de Research Workspace fica disponível ao criar o
                                     agente dentro de um workspace. O Ocinye Core recusa um âmbito
                                     para o qual não possua a permissão correspondente."
                                </p>
                            },
                        )}

                        <div class="oc-vspace" ></div>

                        {card(
                            section_head("CONHECIMENTO", None, None),
                            view! {
                                {named_checkbox(
                                    "k-bib",
                                    "uses_bibliography",
                                    "Bibliografia",
                                    true,
                                )}
                                {named_checkbox(
                                    "k-docs",
                                    "uses_documents",
                                    "Documentos institucionais",
                                    false,
                                )}
                                {named_checkbox("k-data", "uses_datasets", "Datasets", false)}
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
                                        "Segurança"
                                    </div>
                                    <p class="oc-t-caption--muted" >
                                        "O agente lê apenas até INTERNAL, e nunca mais do que quem
                                         o cria. Material CONFIDENTIAL e RESTRICTED fica
                                         inacessível, independentemente do que for pedido. Cada
                                         acesso a dados classificados é registado no Audit Log."
                                    </p>
                                </div>
                            </div>
                        </section>

                        <div class="oc-row--end oc-gap-5 oc-mt-8" >
                            {button(Button::new("Cancelar", Variant::Secondary).href("/ai/agents"))}
                            <button type="submit" class="oc-btn oc-btn--gold">
                                "Criar Agente"
                            </button>
                        </div>
                    </div>
                </div>
            </form>

            <p class="oc-muted oc-t-caption--muted oc-mt-6">
                {if has_models {
                    "O agente fica disponível para execução assim que for criado."
                } else {
                    no_capability
                }}
            </p>
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
