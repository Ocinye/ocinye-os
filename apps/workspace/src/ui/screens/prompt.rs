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

/// O que o Core respondeu ao último pedido.
///
/// Existe para que submeter um prompt nunca produza «nada»: a resposta do Core
/// — incluindo a recusa por não haver nó — aparece como estado do ecrã, nunca
/// como alerta do browser (briefing §8).
pub struct Notice {
    /// Título curto.
    pub title: &'static str,
    /// Explicação, tal como o Core a redigiu.
    pub detail: String,
    /// Se descreve uma recusa.
    pub refused: bool,
}

impl Notice {
    /// O pedido foi aceite pelo Core.
    #[must_use]
    pub fn accepted() -> Self {
        Self {
            title: "Pedido submetido",
            detail: "O pedido foi aceite pelo Ocinye Core.".to_owned(),
            refused: false,
        }
    }

    /// O Core recusou, e diz porquê.
    #[must_use]
    pub fn refused(detail: String) -> Self {
        Self {
            title: "IA ainda não disponível",
            detail,
            refused: true,
        }
    }
}

/// O contexto em que o prompt está a ser usado.
pub struct PromptContext {
    /// Agente seleccionado, quando existe algum.
    pub agent: Option<String>,
    /// Research Workspace vinculado, quando aberto de dentro de um.
    pub workspace: Option<(String, String)>,
    /// Capacidades e a sua disponibilidade real.
    pub capabilities: Vec<(String, bool, bool)>,
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
                    let name = match entry.get("capability").and_then(Value::as_str) {
                        Some("GENERAL") => "Geral",
                        Some("REASONING") => "Raciocínio",
                        Some("CODING") => "Código",
                        Some("EMBEDDING") => "Dados",
                        other => other.unwrap_or("Capacidade"),
                    };
                    let ready = entry
                        .get("available")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    (name.to_owned(), ready, i == 0)
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
pub fn prompt(ctx: PromptContext, notice: Option<Notice>) -> impl IntoView {
    let PromptContext {
        agent,
        workspace,
        capabilities,
        available,
        message,
    } = ctx;
    let agent_label = agent.unwrap_or_else(|| "Sem agente seleccionado".to_owned());
    // Viaja com o formulário para que a submissão preserve o contexto: sem
    // isto, submeter dentro de um Research Workspace devolveria o ecrã
    // institucional e o membro perderia o contexto sem perceber porquê.
    let workspace_id = workspace.as_ref().map(|(code, _)| code.clone());

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

                <div class="oc-caps" role="group" aria-label="Capacidade">
                    {capabilities
                        .into_iter()
                        .map(|(name, ready, first)| {
                            if ready {
                                view! {
                                    <button
                                        type="button"
                                        class="oc-cap"
                                        aria-selected=if first { "true" } else { "false" }
                                    >
                                        {name}
                                    </button>
                                }
                                    .into_any()
                            } else {
                                // Uma capacidade indisponível é mostrada como
                                // tal, com a razão, em vez de escondida.
                                view! {
                                    <span
                                        class="oc-cap"
                                        aria-disabled="true"
                                    >
                                        {name}
                                        <small>"indisponível"</small>
                                    </span>
                                }
                                    .into_any()
                            }
                        })
                        .collect_view()}
                </div>
            </div>

            // ── Conversa ───────────────────────────────────────────────
            <div class="oc-prompt__conv">
                {notice
                    .map(|notice| {
                        let class = if notice.refused {
                            "oc-callout oc-callout--warning oc-prompt__notice"
                        } else {
                            "oc-callout oc-prompt__notice"
                        };
                        view! {
                            <div class=class role="status" aria-live="polite">
                                <strong>{notice.title}</strong>
                                <p>{notice.detail}</p>
                            </div>
                        }
                    })}

                <div class="oc-prompt__hero">
                    <span
                        class="oc-empty__tile oc-empty__tile--prompt"
                    >
                        {icon(Icon::AiHexMd, 26)}
                    </span>
                    <h1>"Interagir com Ocinye"</h1>
                    <p class="oc-t-caption--muted" >
                        {message}
                    </p>
                    <p class="oc-t-soft" >
                        "As respostas respeitarão sempre aquilo a que tem acesso: um modelo nunca
                         recebe um artefacto que não conseguiria abrir."
                    </p>

                </div>

                // Cada sugestão submete o pedido que enuncia. Antes eram
                // botões sem handler: clicá-las não fazia nada, mesmo com IA
                // disponível (briefing §3).
                <div class="oc-prompt__suggestions">
                    {SUGGESTIONS
                        .iter()
                        .map(|text| {
                            view! {
                                <form method="post" action="/ai/prompt">
                                    <input type="hidden" name="prompt" value=*text />
                                    <button
                                        type="submit"
                                        class="oc-suggestion"
                                        disabled=!available
                                        title=if available {
                                            String::new()
                                        } else {
                                            "Nenhuma capacidade de IA compatível está \
                                             actualmente disponível."
                                                .to_owned()
                                        }
                                    >
                                        {*text}
                                    </button>
                                </form>
                            }
                        })
                        .collect_view()}
                </div>
            </div>

            // ── Input ──────────────────────────────────────────────────
            <div class="oc-prompt__dock">
                <form
                    method="post"
                    action="/ai/prompt"
                    class="oc-prompt__input"
                >
                    {workspace_id
                        .map(|id| view! { <input type="hidden" name="workspace" value=id /> })}

                    <label class="oc-sr" for="prompt-input">"Escreva o seu pedido"</label>
                    <textarea
                        id="prompt-input"
                        name="prompt"
                        class="oc-textarea"
                        placeholder="Escreva o seu pedido…"
                        disabled=!available
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
                            title=if available {
                                "Enviar"
                            } else {
                                "Nenhuma capacidade de IA compatível está actualmente disponível."
                            }
                            disabled=!available
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
            "message": "Nenhum nó de IA Ocinye está actualmente disponível.",
            "capabilities": [
                {"capability": "GENERAL", "available": false},
                {"capability": "REASONING", "available": false},
                {"capability": "CODING", "available": false},
                {"capability": "EMBEDDING", "available": false}
            ]
        })
    }

    #[test]
    fn sem_no_o_envio_esta_desactivado_e_o_ecra_diz_porque() {
        let html = prompt(context_from(&unavailable(), None), None).to_html();
        assert!(html.contains("Nenhum nó de IA Ocinye está actualmente disponível"));
        assert!(html.contains("disabled"));
        assert!(html.contains("indisponível"));
    }

    #[test]
    fn as_capacidades_aparecem_em_portugues() {
        let html = prompt(context_from(&unavailable(), None), None).to_html();
        for label in ["Geral", "Raciocínio", "Código", "Dados"] {
            assert!(html.contains(label), "falta a capacidade {label}");
        }
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
