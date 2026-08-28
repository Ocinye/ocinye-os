//! A superfície contextual de assistência.
//!
//! # Porque não é uma janela de conversa
//!
//! **Prompt everywhere, not chat everywhere.** Um investigador dentro de um
//! Projecto não quer abrir um chat, descrever onde está e depois pedir. Quer
//! escrever a frase ali, onde o contexto já é evidente para ele e passa a
//! ser evidente para o Ocinye Core.
//!
//! O que esta peça acrescenta ao ecrã é uma linha de input e algumas sugestões
//! do próprio domínio. O que **não** acrescenta é histórico de mensagens,
//! avatares, ou a sugestão de que a aplicação é o modelo (`design/README.md`
//! §6.11).
//!
//! # Porque não decide nada
//!
//! Submete para `/ask`, a mesma superfície universal, com o contexto no
//! endereço. Não chama capabilities, não interpreta a frase, não esconde nada
//! por sua conta: o que aparece a seguir é a resposta do Core.
//!
//! # Estado honesto
//!
//! Com zero fornecedores de inferência, `Perguntar` e `Executar` não podem ser
//! servidos — e isto diz isso, em vez de aceitar a frase e falhar depois. A
//! pesquisa continua, porque a pesquisa não precisa de modelo.

use leptos::prelude::*;

use crate::ui::icon::{icon, Icon};

/// O contexto em que a superfície aparece.
pub struct Assist {
    /// Onde o membro está, em linguagem do domínio: «esta Ideia», «este Projecto».
    pub here: &'static str,
    /// O Research Workspace, quando existe.
    pub workspace_id: Option<String>,
    /// O recurso em foco, quando existe: tipo e identificador.
    pub resource: Option<(&'static str, String)>,
    /// Frases que fazem sentido aqui, e que o Core consegue servir.
    pub suggestions: &'static [&'static str],
    /// Se alguma capacidade de inferência está disponível.
    ///
    /// Vem do Core. A interface não decide isto.
    pub inference_available: bool,
    /// Se o membro sequer pode usar assistência.
    pub may_use: bool,
}

/// Renderiza a superfície, ou nada.
///
/// Quem não tem permissão não vê o painel — é a alínea B do contrato: o Core
/// recusaria na mesma, e mostrar o campo seria convidar a uma recusa.
pub fn assist(spec: Assist) -> impl IntoView {
    let Assist {
        here,
        workspace_id,
        resource,
        suggestions,
        inference_available,
        may_use,
    } = spec;

    if !may_use {
        return ().into_any();
    }

    // O contexto viaja no endereço, para que `/ask` o receba sem estado de
    // sessão e para que a ligação seja partilhável.
    let workspace_field = workspace_id.map(|id| {
        view! { <input type="hidden" name="workspace" value=id /> }
    });
    let resource_fields = resource.map(|(kind, id)| {
        view! {
            <>
                <input type="hidden" name="resource_kind" value=kind />
                <input type="hidden" name="resource_id" value=id />
            </>
        }
    });

    let placeholder = format!("Perguntar ou pedir algo sobre {here}…");

    view! {
        <section class="oc-assist" aria-labelledby="assist-title">
            <div class="oc-assist__head">
                {icon(Icon::Ai, 15)}
                <h2 class="oc-assist__title" id="assist-title">"Assistência do Ocinye"</h2>
                <span class="oc-assist__scope">{here}</span>
            </div>

            <form class="oc-assist__form" method="get" action="/ask">
                {workspace_field}
                {resource_fields}
                <label class="oc-sr" for="assist-q">{placeholder.clone()}</label>
                <input
                    class="oc-input"
                    id="assist-q"
                    name="q"
                    type="search"
                    placeholder=placeholder
                    autocomplete="off"
                />
                <button type="submit" class="oc-btn oc-btn--primary">"Pedir"</button>
            </form>

            // As sugestões são ligações, não botões: cada uma leva à mesma
            // superfície com a frase já escrita, e o membro vê o que vai
            // acontecer antes de acontecer.
            <ul class="oc-assist__suggestions">
                {suggestions
                    .iter()
                    .map(|phrase| {
                        let href = format!("/ask?q={}", urlencode(phrase));
                        view! {
                            <li>
                                <a class="oc-chip" href=href>{*phrase}</a>
                            </li>
                        }
                    })
                    .collect::<Vec<_>>()}
            </ul>

            {(!inference_available)
                .then(|| {
                    view! {
                        <p class="oc-assist__state" role="status">
                            "Nenhum nó de IA está disponível nesta instalação, por isso perguntar
                             e executar ainda não podem ser servidos. A pesquisa funciona, e todas
                             as acções deste ecrã continuam disponíveis."
                        </p>
                    }
                })}
        </section>
    }
    .into_any()
}

/// Percent-encoding do que vai numa query string.
///
/// Pequeno de propósito: o que aqui passa são frases constantes deste ficheiro,
/// não entrada de utilizador. Uma dependência para isto seria desproporcionada
/// (`CLAUDE.md` §54).
fn urlencode(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 8);
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            b' ' => out.push('+'),
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

/// Sugestões para uma Ideia.
pub const IDEA_SUGGESTIONS: &[&str] = &[
    "Resume o estado desta Ideia",
    "Que fontes estão relacionadas com esta Ideia?",
    "O que falta antes de passar a revisão?",
    "Cria uma tarefa para rever a bibliografia",
];

/// Sugestões para um Projecto.
pub const PROJECT_SUGGESTIONS: &[&str] = &[
    "Resume o estado deste Projecto",
    "Que tarefas continuam abertas?",
    "Que documentos estão ligados a este Projecto?",
    "Cria uma nota de decisão",
];

/// Sugestões no acervo de conhecimento.
pub const KNOWLEDGE_SUGGESTIONS: &[&str] = &[
    "Encontra fontes sobre armazenamento",
    "Que notas existem sobre este tema?",
    "Resume as notas deste ambiente",
];
