//! Prompt Ocinye.
//!
//! A peça central da interface, e deliberadamente **não** um clone de chat
//! genérico (`design/README.md` §6.11, regra 9).
//!
//! Três zonas: a barra de contexto que diz sempre com que agente, em que
//! Research Workspace e com que capacidade se está a falar; a área de conversa,
//! preparada para texto, fontes, referências, datasets, documentos, código,
//! resultados e tabelas; e o input, a peça mais bem resolvida do ecrã.
//!
//! # Estado real
//!
//! Não existe nenhum nó de IA. O ecrã diz isso com precisão e mantém a
//! arquitectura visual pronta — não simula respostas nem contacta um
//! fornecedor externo.

use leptos::prelude::*;
use serde_json::Value;

use crate::ui::icon::{icon, Icon};

/// Uma capacidade oferecida na barra, e o seu estado.
///
/// A capacidade é **seleccionável** independentemente de estar disponível:
/// autorização e disponibilidade são eixos distintos. Escolher `Código` sem um
/// modelo de código activo é legítimo — o pedido conclui como resposta de
/// sistema, e não é escondido (M5 §10).
pub struct CapabilityChip {
    /// Rótulo em português.
    pub label: &'static str,
    /// Código estável (`GENERAL`, `CODING`, …), submetido com o pedido.
    pub code: String,
    /// Se um modelo saudável a serve agora.
    pub available: bool,
    /// Se é a escolha por omissão.
    pub first: bool,
}

/// Um turno de conversa já concluído: o pedido do membro e a resposta tipada.
///
/// Existe para que o Prompt seja uma superfície de comando, e não um widget de
/// LLM: o pedido submetido aparece como mensagem do membro, e a resposta do
/// Core aparece com a sua **origem** explícita. Uma resposta de sistema —
/// escrita pela plataforma porque não havia inferência — nunca se disfarça de
/// resposta de modelo (M5 §8, §15).
pub struct PromptExchange {
    /// O que o membro pediu.
    pub prompt: String,
    /// Quem redigiu a resposta: `SYSTEM`, `MODEL`, `TOOL` ou `AGENT`.
    pub origin: String,
    /// Como concluiu: `COMPLETED` ou `DEGRADED`.
    pub status: String,
    /// O código-máquina da razão, quando degradou.
    pub reason_code: Option<String>,
    /// O modelo que respondeu, quando existiu.
    pub model: Option<String>,
    /// A resposta, nas palavras de quem a redigiu.
    pub content: String,
}

impl PromptExchange {
    /// A etiqueta de autoria da resposta, derivada da origem.
    ///
    /// `SYSTEM` é «Ocinye · Sistema» — uma resposta determinística da
    /// plataforma. `MODEL` nomeia o modelo. Nunca se confundem.
    #[must_use]
    pub fn author(&self) -> String {
        match self.origin.as_str() {
            "MODEL" => match &self.model {
                Some(model) => format!("Ocinye AI · {model}"),
                None => "Ocinye AI".to_owned(),
            },
            "TOOL" => "Ocinye · Ferramenta".to_owned(),
            "AGENT" => "Ocinye · Agente".to_owned(),
            _ => "Ocinye · Sistema".to_owned(),
        }
    }

    /// Se a resposta é uma conclusão degradada (sem inferência).
    #[must_use]
    pub fn degraded(&self) -> bool {
        self.status == "DEGRADED"
    }
}

/// O contexto em que o prompt está a ser usado.
pub struct PromptContext {
    /// Agente seleccionado, quando existe algum.
    pub agent: Option<String>,
    /// Research Workspace vinculado, quando aberto de dentro de um.
    pub workspace: Option<(String, String)>,
    /// Capacidades oferecidas, seleccionáveis independentemente do estado.
    pub capabilities: Vec<CapabilityChip>,
    /// Se alguma capacidade pode ser servida.
    pub available: bool,
    /// A explicação do estado, vinda do Core.
    pub message: String,
}

/// Constrói o contexto a partir do estado do Intelligence Plane.
///
/// As capacidades vêm do Core com a sua disponibilidade real; a interface não
/// decide o que está disponível.
#[must_use]
pub fn context_from(status: &Value, workspace: Option<(String, String)>) -> PromptContext {
    let available = status
        .get("available")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let capabilities = status
        .get("capabilities")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .enumerate()
                .map(|(i, entry)| {
                    let code = entry
                        .get("capability")
                        .and_then(Value::as_str)
                        .unwrap_or("GENERAL");
                    let label = match code {
                        "REASONING" => "Raciocínio",
                        "CODING" => "Código",
                        "EMBEDDING" => "Dados",
                        _ => "Geral",
                    };
                    let ready = entry
                        .get("available")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    CapabilityChip {
                        label,
                        code: code.to_owned(),
                        available: ready,
                        first: i == 0,
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    PromptContext {
        agent: None,
        workspace,
        capabilities,
        available,
        message: status
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or(
                "Nenhum nó de IA Ocinye está actualmente disponível. Nenhum fornecedor externo é \
                 usado em substituição.",
            )
            .to_owned(),
    }
}

/// As sugestões do design.
const SUGGESTIONS: [&str; 4] = [
    "Resumir investigação sobre hidrogénio verde",
    "Comparar bibliografia de armazenamento",
    "Analisar dataset climático de 2010–2024",
    "Criar estrutura de relatório",
];

/// O ecrã do Prompt Ocinye.
///
/// O Prompt é uma **superfície de comando**, e mantém-se operacional sempre que
/// o Core está saudável e o membro tem autorização — independentemente de haver
/// zero fornecedores, modelos ou nós. O input não é desactivado por ausência de
/// IA: um pedido que precise de inferência conclui com uma resposta de sistema,
/// visível na conversa (M5 §4, §12).
pub fn prompt(ctx: PromptContext, exchange: Option<PromptExchange>) -> impl IntoView {
    let PromptContext {
        agent,
        workspace,
        capabilities,
        available: _available,
        message,
    } = ctx;
    let agent_label = agent.unwrap_or_else(|| "Sem agente seleccionado".to_owned());
    // Viaja com o formulário para que a submissão preserve o contexto: sem
    // isto, submeter dentro de um Research Workspace devolveria o ecrã
    // institucional e o membro perderia o contexto sem perceber porquê.
    let workspace_id = workspace.as_ref().map(|(code, _)| code.clone());
    let has_exchange = exchange.is_some();

    // O selector de capacidade vive **dentro** do formulário do dock: é um campo
    // com destino real (submete com o pedido), sem depender de associação por
    // `form=`. Seleccionável independentemente da disponibilidade — autorização e
    // disponibilidade são eixos distintos (M5 §10).
    let caps_view = capabilities
        .into_iter()
        .map(|chip| {
            let CapabilityChip {
                label,
                code,
                available,
                first,
            } = chip;
            let id = format!("cap-{}", code.to_lowercase());
            view! {
                <input
                    type="radio"
                    class="oc-cap__in"
                    name="capability"
                    id=id.clone()
                    value=code
                    checked=first
                />
                <label class="oc-cap" for=id>
                    {label}
                    {(!available).then(|| view! { <small>"sem modelo activo"</small> })}
                </label>
            }
        })
        .collect_view();

    view! {
        <div class="oc-prompt">
            // ── Barra de contexto ──────────────────────────────────────
            <div class="oc-prompt__bar">
                // Uma ligação para os agentes, e não um `listbox` que não abre.
                // Escolher um agente na barra exige uma lista de agentes que o
                // Core sirva por âmbito; até lá, levar à lista é honesto e
                // funciona (briefing §2).
                <a class="oc-btn oc-btn--secondary" href="/ai/agents">
                    <span class="oc-btn__dot"></span>
                    {agent_label}
                </a>

                {workspace
                    .map(|(code, unit)| {
                        // Contexto preenchido: distingue visualmente uma
                        // conversa vinculada a um Research Workspace de uma
                        // conversa institucional geral.
                        view! {
                            <span class="oc-prompt__context">
                                <i>"CONTEXTO"</i>
                                <b>{format!("{code} · {unit}")}</b>
                            </span>
                        }
                    })}

                <div class="oc-spacer"></div>
            </div>

            // ── Conversa ───────────────────────────────────────────────
            <div class=if has_exchange {
                "oc-prompt__conv oc-prompt__conv--thread"
            } else {
                "oc-prompt__conv"
            }>
                {match exchange {
                    // Um turno concluído: o pedido do membro, e a resposta com a
                    // sua origem explícita. Uma resposta de sistema nunca se
                    // apresenta como resposta de modelo (M5 §8, §15).
                    Some(ex) => {
                        let author = ex.author();
                        let degraded = ex.degraded();
                        let PromptExchange { prompt, content, reason_code, .. } = ex;
                        view! {
                            <div class="oc-prompt__thread">
                                <div class="oc-msg oc-msg--member">
                                    <span class="oc-msg__who">"Você"</span>
                                    <p class="oc-msg__body">{prompt}</p>
                                </div>
                                <div class="oc-msg oc-msg--system">
                                    <span class="oc-msg__who">
                                        {author}
                                        {degraded
                                            .then(|| {
                                                view! {
                                                    <span class="oc-msg__badge">"ESTADO"</span>
                                                }
                                            })}
                                    </span>
                                    <p class="oc-msg__body">{content}</p>
                                    {reason_code
                                        .map(|code| {
                                            view! { <span class="oc-msg__code">{code}</span> }
                                        })}
                                </div>
                            </div>
                        }
                            .into_any()
                    }
                    // Estado vazio: sem conversa ainda. O ecrã diz o que é, com
                    // a explicação vinda do Core — nunca «desactivado».
                    None => {
                        view! {
                            <div class="oc-prompt__hero">
                                <span class="oc-empty__tile oc-empty__tile--prompt">
                                    {icon(Icon::AiHexMd, 26)}
                                </span>
                                <h1>"Interagir com Ocinye"</h1>
                                <p class="oc-t-caption--muted">{message}</p>
                                <p class="oc-t-soft">
                                    "As respostas respeitarão sempre aquilo a que tem acesso: um
                                     modelo nunca recebe um artefacto que não conseguiria abrir."
                                </p>
                            </div>

                            // Cada sugestão submete o pedido que enuncia, e
                            // continua utilizável sem IA: prova que o Prompt é
                            // uma superfície de comando, não um widget de LLM
                            // (M5 §12).
                            <div class="oc-prompt__suggestions">
                                {SUGGESTIONS
                                    .iter()
                                    .map(|text| {
                                        view! {
                                            <form method="post" action="/ai/prompt">
                                                <input type="hidden" name="prompt" value=*text />
                                                <button type="submit" class="oc-suggestion">
                                                    {*text}
                                                </button>
                                            </form>
                                        }
                                    })
                                    .collect_view()}
                            </div>
                        }
                            .into_any()
                    }
                }}
            </div>

            // ── Input ──────────────────────────────────────────────────
            <div class="oc-prompt__dock">
                <form
                    id="oc-prompt-form"
                    method="post"
                    action="/ai/prompt"
                    class="oc-prompt__input"
                >
                    {workspace_id
                        .map(|id| view! { <input type="hidden" name="workspace" value=id /> })}

                    <div class="oc-caps oc-caps--dock" role="radiogroup" aria-label="Capacidade">
                        {caps_view}
                    </div>

                    <label class="oc-sr" for="prompt-input">"Escreva o seu pedido"</label>
                    <textarea
                        id="prompt-input"
                        name="prompt"
                        class="oc-textarea"
                        placeholder="Escreva o seu pedido…"
                    ></textarea>

                    <div class="oc-prompt__actions">
                        // Anexar contexto é do dossier e continua visível, mas
                        // declarado indisponível com a razão: eram botões sem
                        // handler nem endpoint, e um controlo que não faz nada
                        // é pior do que um que diz porque ainda não faz
                        // (briefing §2C, §53).
                        {action_chip(Icon::Attach, "Anexar")}
                        {action_chip(Icon::Dataset, "Dataset")}
                        {action_chip(Icon::Document, "Documento")}
                        {action_chip(Icon::Tools, "Ferramentas")}

                        <div class="oc-spacer"></div>

                        // A afirmação «⏎ enviar» foi retirada: sem JavaScript,
                        // Enter numa textarea insere uma linha. Uma promessa de
                        // atalho que não existe é dead UI escrita.

                        <button
                            type="submit"
                            aria-label="Enviar"
                            title="Enviar"
                            class="oc-prompt__send"
                        >
                            {icon(Icon::Send, 16)}
                        </button>
                    </div>
                </form>

                <p class="oc-prompt__note">
                    "O Ocinye AI pode cometer erros. Verifique informação crítica e consulte as
                     fontes citadas."
                </p>
            </div>
        </div>
    }
}

/// Um chip de contexto do dock.
///
/// Anexar ficheiros, datasets e documentos a um pedido é arquitectura decidida
/// e ainda não construída: o AI Gateway não aceita anexos, e nenhum endpoint os
/// serve. O chip permanece — pertence ao desenho do dock — mas diz o que é, em
/// vez de parecer clicável e não fazer nada.
fn action_chip(kind: Icon, label: &'static str) -> impl IntoView {
    view! {
        <span
            class="oc-chip oc-unavailable"
            aria-disabled="true"
            title="Anexar contexto a um pedido ainda não está disponível nesta instalação."
        >
            {icon(kind, 12)}
            {label}
        </span>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn unavailable() -> Value {
        json!({
            "available": false,
            "providers": 0,
            "message": "O Prompt Ocinye está operacional. Actualmente não existe nenhum nó \
                        Ocinye AI activo. Nenhum fornecedor externo é utilizado em substituição.",
            "capabilities": [
                {"capability": "GENERAL", "available": false},
                {"capability": "REASONING", "available": false},
                {"capability": "CODING", "available": false},
                {"capability": "EMBEDDING", "available": false}
            ]
        })
    }

    fn degraded_exchange() -> PromptExchange {
        PromptExchange {
            prompt: "Cria uma função Rust que some dois números.".to_owned(),
            origin: "SYSTEM".to_owned(),
            status: "DEGRADED".to_owned(),
            reason_code: Some("AI_NO_PROVIDER_AVAILABLE".to_owned()),
            model: None,
            content: "Nenhuma capacidade de inferência está actualmente disponível.".to_owned(),
        }
    }

    #[test]
    fn sem_no_o_prompt_continua_operacional_e_o_ecra_diz_porque() {
        // O contrato M5: com zero modelos, o Prompt está operacional. O input
        // não é desactivado; a razão vive nas capacidades e na resposta.
        let html = prompt(context_from(&unavailable(), None), None).to_html();
        assert!(html.contains("O Prompt Ocinye está operacional"));
        // O atributo booleano `disabled` (precedido de espaço) não aparece em
        // nenhum controlo — distinto de `aria-disabled` dos chips de contexto,
        // que ainda não têm endpoint e são um assunto à parte.
        assert!(
            !html.contains(" disabled"),
            "o input do Prompt nunca é desactivado por ausência de IA"
        );
        // A capacidade continua seleccionável, com o seu estado ao lado.
        assert!(html.contains("sem modelo activo"));
        assert!(html.contains("name=\"capability\""));
    }

    #[test]
    fn as_capacidades_aparecem_em_portugues() {
        let html = prompt(context_from(&unavailable(), None), None).to_html();
        for label in ["Geral", "Raciocínio", "Código", "Dados"] {
            assert!(html.contains(label), "falta a capacidade {label}");
        }
    }

    #[test]
    fn uma_resposta_de_sistema_aparece_como_sistema() {
        // A resposta degradada é do SISTEMA, e nunca se disfarça de modelo.
        let html = prompt(
            context_from(&unavailable(), None),
            Some(degraded_exchange()),
        )
        .to_html();
        assert!(html.contains("Cria uma função Rust")); // o pedido do membro
        assert!(html.contains("Ocinye · Sistema"));
        assert!(html.contains("AI_NO_PROVIDER_AVAILABLE"));
        assert!(
            !html.contains("Ocinye AI ·"),
            "uma resposta de sistema não nomeia um modelo"
        );
    }

    #[test]
    fn dentro_de_um_workspace_o_contexto_e_visivel() {
        let ctx = context_from(
            &unavailable(),
            Some(("IDE-0142".to_owned(), "UENR-001".to_owned())),
        );
        let html = prompt(ctx, None).to_html();

        // Rótulo e valor são elementos distintos, como no protótipo: o rótulo
        // é mono e discreto, o código é o que se lê.
        assert!(html.contains("oc-prompt__context"));
        assert!(html.contains("CONTEXTO"));
        assert!(html.contains("IDE-0142 · UENR-001"));
    }

    #[test]
    fn o_aviso_sobre_erros_esta_sempre_presente() {
        let html = prompt(context_from(&unavailable(), None), None).to_html();
        assert!(html.contains("pode cometer erros"));
    }
}
