//! A cadeia científica de um Research Workspace, e a proveniência de um resultado.
//!
//! # O que este ecrã responde
//!
//! Uma pergunta só: **de onde veio isto?** Tudo o que aparece serve essa
//! pergunta, e o que não a serve não aparece.
//!
//! A cadeia lê-se de cima para baixo — hipótese, metodologia, estudo, execução,
//! resultado — porque é a ordem em que o trabalho acontece. Um resultado no
//! topo obrigaria a ler para trás para perceber como se lá chegou.
//!
//! # Sem UUIDs
//!
//! Nenhum identificador aparece como texto. Um resultado diz «Execução 3 de
//! *Ensaio de carga*», e não `a3f2…`. A pessoa que abre este ecrã quer saber o
//! que aconteceu, e um identificador é a resposta a outra pergunta — a de quem
//! está a depurar uma consulta.
//!
//! # A linhagem, e o que ela cala
//!
//! `Montante` mostra de onde o resultado veio; `Jusante`, o que dependeu dele.
//! Um nó que a política de quem lê recuse **não aparece**, e a travessia
//! termina aí — sem contagem, sem reticências, sem «e mais três».
//!
//! É por isso que este ecrã nunca diz «há mais para lá do que vês» por causa de
//! autorização. Diz-o apenas quando a travessia atingiu o limite de
//! profundidade, que é uma afirmação sobre a consulta e não sobre a pessoa.

use leptos::prelude::*;
use serde_json::Value;

use crate::ui::components::{
    badge, button, card, classification_badge, empty_state, pill, pill_tabs, radio_group,
    section_head, select_labelled, text_field, textarea, Button, EmptyState, RadioOption,
    SelectOption, Tab, Tone, Variant,
};
use crate::ui::icon::{icon, Icon};

fn text(row: &Value, key: &str) -> String {
    row.get(key)
        .and_then(Value::as_str)
        .unwrap_or("—")
        .to_owned()
}

fn maybe(row: &Value, key: &str) -> Option<String> {
    row.get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .filter(|v| !v.is_empty())
}

fn items(payload: &Value) -> Vec<Value> {
    payload.as_array().cloned().unwrap_or_default()
}

/// A cadeia científica de um ambiente.
pub struct ChainView {
    /// A visão geral do ambiente, para o cabeçalho.
    pub overview: Value,
    /// Hipóteses.
    pub hypotheses: Value,
    /// Metodologias.
    pub methodologies: Value,
    /// Estudos.
    pub studies: Value,
    /// Resultados.
    pub results: Value,
    /// Se este membro pode descrever trabalho científico.
    pub may_create: bool,
}

/// A cadeia científica de um Research Workspace.
pub fn scientific_chain(view: ChainView) -> impl IntoView {
    let ChainView {
        overview,
        hypotheses,
        methodologies,
        studies,
        results,
        may_create,
    } = view;

    let workspace = overview.get("workspace").cloned().unwrap_or(Value::Null);
    let id = text(&workspace, "id");
    let contexto = contexto_do_ambiente(&workspace);

    let hypotheses = items(&hypotheses);
    let methodologies = items(&methodologies);
    let studies = items(&studies);
    let results = items(&results);

    let vazia = hypotheses.is_empty()
        && methodologies.is_empty()
        && studies.is_empty()
        && results.is_empty();

    view! {
        <div class="oc-band">
            <div class="oc-row--top oc-gap-11 oc-mb-3">
                <div class="oc-fill">
                    <div class="oc-row oc-row--wrap oc-gap-6">
                        {icon(Icon::Science, 22)}
                        <h1 class="oc-t-screen">"Ciência"</h1>
                    </div>
                    <div class="oc-mono oc-mt-3">{contexto}</div>
                </div>
                <div class="oc-head__actions">
                    {may_create
                        .then(|| {
                            view! {
                                {button(
                                    Button::new("Nova hipótese", Variant::Primary)
                                        .href(
                                            format!("/workspaces/{id}/science/hypotheses/new"),
                                        ),
                                )}
                                {button(
                                    Button::new("Nova metodologia", Variant::Secondary)
                                        .href(
                                            format!("/workspaces/{id}/science/methodologies/new"),
                                        ),
                                )}
                                {button(
                                    Button::new("Novo estudo", Variant::Secondary)
                                        .href(format!("/workspaces/{id}/science/studies/new")),
                                )}
                            }
                        })}
                    {button(
                        Button::new("Voltar ao ambiente", Variant::Secondary)
                            .href(format!("/workspaces/{id}")),
                    )}
                </div>
            </div>
        </div>

        <div class="oc-page">
            {if vazia {
                empty_state(EmptyState {
                    icon: Icon::Science,
                    title: "Ainda não há trabalho científico registado".to_owned(),
                    body: if may_create {
                        "A cadeia começa por uma hipótese: o que se quer testar, e porquê. \
                         Depois vêm a metodologia, o estudo, a execução e o resultado — e \
                         cada um deles guarda de onde veio."
                            .to_owned()
                    } else {
                        "Quando alguém enunciar uma hipótese neste ambiente, a cadeia \
                         aparece aqui."
                            .to_owned()
                    },
                    actions: if may_create {
                        vec![
                            Button::new("Enunciar a primeira hipótese", Variant::Primary)
                                .href(format!("/workspaces/{id}/science/hypotheses/new")),
                        ]
                    } else {
                        Vec::new()
                    },
                    small: false,
                })
                    .into_any()
            } else {
                view! {
                    <div class="oc-grid oc-grid--pares">
                        {etapa("Hipóteses", &hypotheses, "statement", None)}
                        {etapa("Metodologias", &methodologies, "title", Some("/methodologies"))}
                    </div>
                    <div class="oc-grid oc-grid--pares oc-mt-7">
                        {etapa("Estudos", &studies, "title", Some("/studies"))}
                        {etapa("Resultados", &results, "title", Some("/results"))}
                    </div>
                }
                    .into_any()
            }}
        </div>
    }
}

/// Uma etapa da cadeia, com o que já existe dela.
fn etapa(
    titulo: &'static str,
    linhas: &[Value],
    campo: &'static str,
    destino: Option<&'static str>,
) -> impl IntoView {
    let contagem = linhas.len();
    let linhas: Vec<Value> = linhas.to_vec();

    view! {
        <section class="oc-card">
            <div class="oc-card__head">
                <h2>{titulo}</h2>
                <span class="oc-card__meta">{contagem.to_string()}</span>
            </div>
            <div class="oc-card__body">
                {if linhas.is_empty() {
                    view! { <p class="oc-muted">"Ainda nada."</p> }.into_any()
                } else {
                    view! {
                        <div>
                            {linhas
                                .iter()
                                .map(|linha| {
                                    let rotulo = text(linha, campo);
                                    // Uma metodologia não tem estado, e um
                                    // distintivo com um travessão dentro não
                                    // diz nada — diz que alguma coisa faltou.
                                    let estado = maybe(linha, "status_label");
                                    let classificacao = text(linha, "classification");
                                    let href = destino
                                        .map(|base| format!("{base}/{}", text(linha, "id")));
                                    view! {
                                        <div class="oc-list__row">
                                            {match href {
                                                Some(href) => {
                                                    view! {
                                                        <a class="oc-fill oc-truncate oc-t-cell-2" href=href>
                                                            {rotulo}
                                                        </a>
                                                    }
                                                        .into_any()
                                                }
                                                None => {
                                                    view! {
                                                        <span class="oc-fill oc-truncate oc-t-cell-2">
                                                            {rotulo}
                                                        </span>
                                                    }
                                                        .into_any()
                                                }
                                            }}
                                            {estado
                                                .map(|e| {
                                                    let tom = Tone::of(&e);
                                                    badge(e, tom)
                                                })}
                                            {classification_badge(&classificacao)}
                                        </div>
                                    }
                                })
                                .collect_view()}
                        </div>
                    }
                        .into_any()
                }}
            </div>
        </section>
    }
}

/// Um resultado, e de onde veio.
pub struct ResultView {
    /// O resultado.
    pub result: Value,
    /// As validações e reproduções que alguém registou.
    pub validations: Value,
    /// A linhagem a montante.
    pub upstream: Value,
    /// A linhagem a jusante.
    pub downstream: Value,
    /// Qual das duas se está a ver.
    pub direction: &'static str,
    /// Se este membro pode afirmar que o resultado se confirma.
    pub may_validate: bool,
}

/// O detalhe de um resultado, com a sua proveniência.
pub fn result_detail(view: ResultView) -> impl IntoView {
    let ResultView {
        result,
        validations,
        upstream,
        downstream,
        direction,
        may_validate,
    } = view;

    let id = text(&result, "id");
    let title = text(&result, "title");
    let summary = text(&result, "summary");
    let status = text(&result, "status_label");
    let classification = text(&result, "classification");
    let validations = items(&validations);

    let a_montante = direction == "upstream";
    let linhagem = if a_montante { &upstream } else { &downstream };

    view! {
        <div class="oc-band">
            <div class="oc-row--top oc-gap-11 oc-mb-3">
                <div class="oc-fill">
                    <div class="oc-row oc-row--wrap oc-gap-6">
                        <h1 class="oc-t-screen">{title}</h1>
                        {badge(status.clone(), Tone::of(&status))}
                        {classification_badge(&classification)}
                    </div>
                </div>
                <div class="oc-head__actions">
                    // Validar não é uma acção de agente, e não é aqui que se
                    // decide: o Core recusa a quem não pode. O botão só
                    // aparece a quem pode para não prometer o que não cumpre.
                    {may_validate
                        .then(|| {
                            button(
                                Button::new("Validar resultado", Variant::Primary)
                                    .href(format!("/results/{id}/validate")),
                            )
                        })}
                </div>
            </div>
        </div>

        <div class="oc-page">
            <div class="oc-grid oc-grid--ws">
                <section class="oc-card">
                    {section_head("O que este resultado diz", None, None)}
                    <div class="oc-card__body">
                        <p>{summary}</p>
                    </div>
                </section>

                <section class="oc-card">
                    {section_head("Validações e reproduções", None, None)}
                    <div class="oc-card__body">
                        {if validations.is_empty() {
                            view! {
                                <p class="oc-muted">
                                    "Ninguém validou nem reproduziu este resultado."
                                </p>
                            }
                                .into_any()
                        } else {
                            view! {
                                <div>
                                    {validations
                                        .iter()
                                        .map(|v| {
                                            let rotulo = text(v, "label");
                                            let nota = maybe(v, "note");
                                            view! {
                                                <div class="oc-list__row">
                                                    <span class="oc-fill oc-t-cell-2">{rotulo}</span>
                                                    {nota
                                                        .map(|n| {
                                                            view! { <span class="oc-muted oc-truncate">{n}</span> }
                                                        })}
                                                </div>
                                            }
                                        })
                                        .collect_view()}
                                </div>
                            }
                                .into_any()
                        }}
                    </div>
                </section>
            </div>

            <section class="oc-card oc-mt-7">
                {section_head("Proveniência", None, None)}
                <div class="oc-card__body">
                    {pill_tabs(
                        vec![
                            Tab::link("Montante", format!("/results/{id}?direction=upstream"), a_montante),
                            Tab::link(
                                "Jusante",
                                format!("/results/{id}?direction=downstream"),
                                !a_montante,
                            ),
                        ],
                        "Sentido da linhagem",
                    )}
                    {passos(linhagem, a_montante)}
                </div>
            </section>
        </div>
    }
}

/// Os passos de uma travessia, tal como se lêem.
fn passos(linhagem: &Value, a_montante: bool) -> impl IntoView {
    let passos = linhagem
        .get("passos")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let truncada = linhagem
        .get("truncada")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let vazio = if a_montante {
        "Nada aponta para a origem deste resultado."
    } else {
        "Nada depende deste resultado."
    };

    view! {
        {if passos.is_empty() {
            view! { <p class="oc-muted">{vazio}</p> }.into_any()
        } else {
            view! {
                <div>
                    {passos
                        .iter()
                        .map(|passo| {
                            let de = passo
                                .get("de")
                                .map(rotulo_do_recurso)
                                .unwrap_or_else(|| "—".to_owned());
                            let para = passo
                                .get("para")
                                .map(rotulo_do_recurso)
                                .unwrap_or_else(|| "—".to_owned());
                            let relacao = text(passo, "relacao_legivel");
                            // Declarada por alguém, ou observada pela operação.
                            // A diferença importa: uma é uma afirmação, a outra
                            // é um facto que o sistema viu acontecer.
                            let origem = text(passo, "origem");
                            let (tom, etiqueta) = if origem == "operation" {
                                (Tone::Navy, "Observada")
                            } else {
                                (Tone::Gray, "Declarada")
                            };
                            view! {
                                <div class="oc-list__row">
                                    <span class="oc-truncate oc-t-cell-2">{de}</span>
                                    <span class="oc-muted oc-mono">{relacao}</span>
                                    <span class="oc-fill oc-truncate oc-t-cell-2">{para}</span>
                                    {badge(etiqueta, tom)}
                                </div>
                            }
                        })
                        .collect_view()}
                </div>
            }
                .into_any()
        }}
        // Só sobre a consulta, nunca sobre autorização. Um nó que a política
        // recuse é indistinguível de uma folha, e a frase abaixo não aparece
        // por causa dele.
        {truncada
            .then(|| {
                view! {
                    <p class="oc-muted oc-mt-3">
                        "A travessia atingiu o limite de profundidade. Abre um dos recursos \
                         acima para continuar a partir dele."
                    </p>
                }
            })}
    }
}

/// Como se lê um recurso da linhagem: pelo título, e nunca pelo identificador.
fn rotulo_do_recurso(recurso: &Value) -> String {
    recurso
        .get("label")
        .and_then(Value::as_str)
        .filter(|l| !l.is_empty())
        .map_or_else(|| text(recurso, "kind"), str::to_owned)
}

/// O que o formulário de validação precisa de saber.
pub struct ValidateView {
    /// O resultado sobre o qual se vai afirmar alguma coisa.
    pub result: Value,
    /// As execuções do estudo que produziu este resultado, quando há estudo.
    ///
    /// É delas que sai a prova de uma reprodução. Vazia quando o resultado não
    /// nasceu de uma execução que o Ocinye conheça.
    pub executions: Value,
    /// A mensagem do Core, quando recusou.
    pub message: Option<String>,
}

/// Afirmar que um resultado se confirma, se contradiz, ou que foi reproduzido.
///
/// # Porque este ecrã existe, e a capability não
///
/// `science::record_validation` é `non_delegable`: nenhum agente a alcança.
/// Uma pessoa alcança-a aqui, com a sua sessão, e é o nome dela que fica no
/// registo. O ecrã é a porta — a única — e por isso diz o que está a fazer.
pub fn validate_result(view: ValidateView) -> impl IntoView {
    let ValidateView {
        result,
        executions,
        message,
    } = view;

    let id = text(&result, "id");
    let title = text(&result, "title");
    let execucoes = items(&executions);

    // Sem execução conhecida não há reprodução possível, e o controlo diz
    // porquê em vez de recusar depois de a pessoa preencher tudo.
    let sem_execucao = execucoes.is_empty();
    let porque_nao = sem_execucao.then(|| {
        "Este resultado não tem nenhuma execução registada que sirva de prova. \
         Regista a execução que o reproduziu antes de o dar por reproduzido."
            .to_owned()
    });

    let opcoes_de_execucao: Vec<SelectOption> = std::iter::once(SelectOption {
        value: String::new(),
        label: "Nenhuma".to_owned(),
        available: true,
        selected: true,
    })
    .chain(execucoes.iter().map(|e| {
        let sequencia = e.get("sequence").and_then(Value::as_i64).unwrap_or(0);
        SelectOption {
            value: text(e, "id"),
            label: format!("Execução {sequencia} · {}", text(e, "status")),
            available: true,
            selected: false,
        }
    }))
    .collect();

    view! {
        <div class="oc-page oc-page--narrow">
            <div class="oc-head">
                <div class="oc-head__text">
                    <h1>"Validar resultado"</h1>
                    <p>{format!("Sobre «{title}».")}</p>
                </div>
            </div>

            {message
                .map(|texto| {
                    view! { <div class="oc-callout oc-callout--error" role="alert">{texto}</div> }
                })}

            <div class="oc-callout" role="note">
                <strong>"Isto fica em seu nome"</strong>
                <p>
                    "Uma validação é uma afirmação institucional sobre o que a Ocinye sabe. \
                     O registo guarda quem a fez, e é por isso que nenhum agente a pode \
                     fazer por si."
                </p>
            </div>

            <form method="post" action=format!("/results/{id}/validate")>
                {card(
                    section_head("A AFIRMAÇÃO", None, None),
                    view! {
                        {radio_group(
                            "kind",
                            "O que está a registar",
                            vec![
                                RadioOption::new("validation", "Validação", true),
                                RadioOption {
                                    value: "reproduction",
                                    label: "Reprodução",
                                    selected: false,
                                    unavailable_reason: porque_nao,
                                },
                            ],
                        )}
                        {radio_group(
                            "outcome",
                            "Desfecho",
                            vec![
                                RadioOption::new("confirmed", "Confirmou", true),
                                RadioOption::new("contradicted", "Contradisse", false),
                                RadioOption::new("inconclusive", "Foi inconclusiva", false),
                            ],
                        )}
                        {select_labelled(
                            "validation-execution",
                            "A execução que serviu de prova",
                            "execution_id",
                            opcoes_de_execucao,
                        )}
                        {textarea(
                            "validation-note",
                            "O que observou",
                            "note",
                            "O que viu, e em que condições",
                            64,
                        )}
                    },
                )}

                <div class="oc-row oc-gap-6 oc-mt-7">
                    {button(Button::new("Registar", Variant::Primary))}
                    {button(
                        Button::new("Cancelar", Variant::Secondary).href(format!("/results/{id}")),
                    )}
                </div>
            </form>
        </div>
    }
}

// ── Construir a cadeia, como uma pessoa a constrói ──────────────────────
//
// # Porque não são sete CRUDs
//
// Porque ninguém investiga pensando em tabelas. Cada formulário abre a partir
// do sítio onde a pergunta nasce — a hipótese a partir do ambiente, a versão a
// partir da metodologia, a execução a partir do estudo, o resultado a partir da
// execução — e leva consigo o contexto em vez de o pedir.
//
// É também o que faz a proveniência acontecer sozinha: quem regista um
// resultado dentro de uma execução não tem de declarar depois que aquela
// execução o produziu. A operação viu-o.

/// O que um formulário desta família precisa de saber sobre onde está.
pub struct Contexto {
    /// O ambiente de investigação.
    pub workspace: Value,
    /// A recusa do Core, quando houve uma.
    pub message: Option<String>,
}

fn cabecalho(titulo: &'static str, explicacao: &'static str, contexto: &Value) -> impl IntoView {
    view! {
        <div class="oc-head">
            <div class="oc-head__text">
                <h1>{titulo}</h1>
                <p>{explicacao}</p>
            </div>
            <div class="oc-mono">{contexto_do_ambiente(contexto)}</div>
        </div>
    }
}

/// Como se lê o ambiente numa linha.
///
/// O código sozinho quando é o que há. A versão anterior juntava sempre o
/// código da unidade e escrevia «WSBDF2328 · —» quando ele faltava: um
/// travessão solto onde devia estar o contexto lê-se como um ecrã partido, e
/// não como uma ausência.
fn contexto_do_ambiente(contexto: &Value) -> String {
    let code = text(contexto, "code");
    match maybe(contexto, "unit_code") {
        Some(unidade) => format!("{code} · {unidade}"),
        None => code,
    }
}

fn recusa(message: Option<String>) -> impl IntoView {
    message.map(|texto| {
        view! { <div class="oc-callout oc-callout--error" role="alert">{texto}</div> }
    })
}

/// As classificações que uma pessoa pode escolher.
///
/// O Core limita-a contra o ambiente e recusa a que não puder conceder; isto é
/// a lista, não a decisão.
fn classificacoes() -> impl IntoView {
    select_labelled(
        "classificacao",
        "Classificação",
        "classification",
        vec![
            SelectOption {
                value: "INTERNAL".to_owned(),
                label: "Interna".to_owned(),
                available: true,
                selected: true,
            },
            SelectOption {
                value: "PUBLIC".to_owned(),
                label: "Pública".to_owned(),
                available: true,
                selected: false,
            },
            SelectOption {
                value: "CONFIDENTIAL".to_owned(),
                label: "Confidencial".to_owned(),
                available: true,
                selected: false,
            },
            SelectOption {
                value: "RESTRICTED".to_owned(),
                label: "Restrita".to_owned(),
                available: true,
                selected: false,
            },
        ],
    )
}

/// Enunciar uma hipótese.
pub fn nova_hipotese(contexto: Contexto) -> impl IntoView {
    let Contexto { workspace, message } = contexto;
    let id = text(&workspace, "id");

    view! {
        <div class="oc-page oc-page--narrow">
            {cabecalho(
                "Nova hipótese",
                "Uma afirmação que se pode testar. Enunciá-la é o princípio da cadeia — \
                 e uma hipótese que não se sustenta é um desfecho científico, não um erro.",
                &workspace,
            )}
            {recusa(message)}

            <form method="post" action=format!("/workspaces/{id}/science/hypotheses/new")>
                {card(
                    section_head("A AFIRMAÇÃO", None, None),
                    view! {
                        {textarea(
                            "hipotese-afirmacao",
                            "O que se afirma",
                            "statement",
                            "Ex.: a dopagem reduz a resistência de contacto",
                            64,
                        )}
                        {textarea(
                            "hipotese-razao",
                            "Porque vale a pena testar",
                            "rationale",
                            "O que se sabe hoje, e o que falta saber",
                            64,
                        )}
                        {classificacoes()}
                    },
                )}
                {accoes(&format!("/workspaces/{id}/science"), "Enunciar")}
            </form>
        </div>
    }
}

/// Os dois botões de um formulário desta família.
fn accoes(voltar: &str, confirmar: &'static str) -> impl IntoView {
    let voltar = voltar.to_owned();
    view! {
        <div class="oc-row oc-gap-6 oc-mt-7">
            {button(Button::new(confirmar, Variant::Primary))}
            {button(Button::new("Cancelar", Variant::Secondary).href(voltar))}
        </div>
    }
}

/// Criar uma metodologia.
pub fn nova_metodologia(contexto: Contexto) -> impl IntoView {
    let Contexto { workspace, message } = contexto;
    let id = text(&workspace, "id");

    view! {
        <div class="oc-page oc-page--narrow">
            {cabecalho(
                "Nova metodologia",
                "A metodologia é a identidade durável do método: o nome pelo qual a \
                 instituição o conhece daqui a cinco anos. O que ela diz hoje vive numa \
                 versão, e publica-se a seguir.",
                &workspace,
            )}
            {recusa(message)}

            <form method="post" action=format!("/workspaces/{id}/science/methodologies/new")>
                {card(
                    section_head("O MÉTODO", None, None),
                    view! {
                        {text_field(
                            "metodologia-titulo",
                            "Como se chama",
                            "title",
                            "Ex.: medição a quatro pontas",
                            "text",
                        )}
                        {textarea(
                            "metodologia-proposito",
                            "Para que serve",
                            "purpose",
                            "Que pergunta este método responde",
                            64,
                        )}
                        {classificacoes()}
                    },
                )}
                {accoes(&format!("/workspaces/{id}/science"), "Criar")}
            </form>
        </div>
    }
}

/// Uma metodologia e as suas versões.
pub struct MetodologiaView {
    /// A metodologia.
    pub methodology: Value,
    /// As versões, da mais recente para a mais antiga.
    pub versions: Value,
    /// Se este membro pode publicar uma versão.
    pub may_create: bool,
}

/// O detalhe de uma metodologia.
///
/// # Porque a versão publicada não é um formulário
///
/// Porque não é editável. Uma versão publicada é o que a proveniência cita: um
/// resultado produzido com a versão 2 continua a dizer «versão 2» depois de a 5
/// existir. Apresentá-la como um campo por preencher convidaria a alterar
/// aquilo em que outra coisa já se apoia — e o domínio substitui uma versão,
/// não a reescreve.
pub fn metodologia(view: MetodologiaView) -> impl IntoView {
    let MetodologiaView {
        methodology,
        versions,
        may_create,
    } = view;

    let id = text(&methodology, "id");
    let title = text(&methodology, "title");
    let purpose = maybe(&methodology, "purpose");
    let classification = text(&methodology, "classification");
    let versoes = items(&versions);
    let workspace_id = text(&methodology, "workspace_id");

    view! {
        <div class="oc-band">
            <div class="oc-row--top oc-gap-11 oc-mb-3">
                <div class="oc-fill">
                    <div class="oc-row oc-row--wrap oc-gap-6">
                        <h1 class="oc-t-screen">{title}</h1>
                        {classification_badge(&classification)}
                    </div>
                    {purpose.map(|p| view! { <div class="oc-muted oc-mt-3">{p}</div> })}
                </div>
                <div class="oc-head__actions">
                    {may_create
                        .then(|| {
                            button(
                                Button::new("Nova versão", Variant::Primary)
                                    .href(format!("/methodologies/{id}/versions/new")),
                            )
                        })}
                    {button(
                        Button::new("Voltar à ciência", Variant::Secondary)
                            .href(format!("/workspaces/{workspace_id}/science")),
                    )}
                </div>
            </div>
        </div>

        <div class="oc-page">
            <section class="oc-card">
                {section_head("Versões", None, None)}
                <div class="oc-card__body">
                    {if versoes.is_empty() {
                        view! {
                            <p class="oc-muted">
                                "Ainda nenhuma versão. Um estudo só pode seguir uma versão \
                                 publicada, porque é a versão que a proveniência cita."
                            </p>
                        }
                            .into_any()
                    } else {
                        view! {
                            <div>
                                {versoes
                                    .iter()
                                    .map(|v| {
                                        let etiqueta = text(v, "label");
                                        let resumo = text(v, "summary");
                                        // O estado já vem lido do domínio, e
                                        // já sabe se foi substituída.
                                        let estado = text(v, "status_label");
                                        let tom = Tone::of(&estado);
                                        view! {
                                            <div class="oc-list__row">
                                                <span class="oc-mono">{etiqueta}</span>
                                                <span class="oc-fill oc-truncate">{resumo}</span>
                                                {badge(estado, tom)}
                                            </div>
                                        }
                                    })
                                    .collect_view()}
                            </div>
                        }
                            .into_any()
                    }}
                </div>
            </section>
        </div>
    }
}

/// Publicar uma versão de metodologia.
pub struct NovaVersaoView {
    /// A metodologia que ganha a versão.
    pub methodology: Value,
    /// A versão em vigor, quando existe.
    pub em_vigor: Option<Value>,
    /// A recusa do Core, quando houve uma.
    pub message: Option<String>,
}

/// O formulário de uma versão nova.
pub fn nova_versao(view: NovaVersaoView) -> impl IntoView {
    let NovaVersaoView {
        methodology,
        em_vigor,
        message,
    } = view;
    let id = text(&methodology, "id");
    let title = text(&methodology, "title");

    view! {
        <div class="oc-page oc-page--narrow">
            <div class="oc-head">
                <div class="oc-head__text">
                    <h1>"Nova versão"</h1>
                    <p>{format!("De «{title}».")}</p>
                </div>
            </div>
            {recusa(message)}

            // Uma versão publicada não se altera; substitui-se.
            //
            // Dito aqui porque é aqui que a pessoa está prestes a decidir, e
            // não numa página de ajuda que ninguém abre a meio do trabalho.
            {em_vigor
                .map(|v| {
                    let etiqueta = text(&v, "label");
                    view! {
                        <div class="oc-callout" role="note">
                            <strong>{format!("Em vigor: {etiqueta}")}</strong>
                            <p>
                                "Publicar substitui-a. A anterior fica no histórico e continua \
                                 a valer para tudo o que já a citou — um resultado produzido \
                                 com ela continua a dizer que foi com ela."
                            </p>
                        </div>
                    }
                })}

            <form method="post" action=format!("/methodologies/{id}/versions/new")>
                {card(
                    section_head("A VERSÃO", None, None),
                    view! {
                        {text_field(
                            "versao-etiqueta",
                            "Como se chama",
                            "label",
                            "Ex.: v2, 2026-rev-b",
                            "text",
                        )}
                        {textarea(
                            "versao-resumo",
                            "O que esta versão diz",
                            "summary",
                            "O que muda em relação à anterior, ou o que o método faz",
                            80,
                        )}
                    },
                )}
                {accoes(&format!("/methodologies/{id}"), "Publicar")}
            </form>
        </div>
    }
}

/// Desenhar um estudo.
pub struct NovoEstudoView {
    /// O ambiente.
    pub workspace: Value,
    /// As hipóteses que este membro alcança.
    pub hypotheses: Value,
    /// As versões de metodologia publicadas, com o nome do método.
    ///
    /// **Versões**, e nunca metodologias. A matriz de proveniência aceita
    /// `Study → MethodologyVersion` e recusa `Study → Methodology`; oferecer a
    /// metodologia mutável seria pôr no ecrã uma escolha que o Core recusa, e
    /// deixar o `422` ensinar a regra.
    pub methodology_versions: Vec<(String, String)>,
    /// A recusa do Core, quando houve uma.
    pub message: Option<String>,
}

/// O formulário de um estudo.
pub fn novo_estudo(view: NovoEstudoView) -> impl IntoView {
    let NovoEstudoView {
        workspace,
        hypotheses,
        methodology_versions,
        message,
    } = view;
    let id = text(&workspace, "id");

    let hipoteses: Vec<SelectOption> = std::iter::once(SelectOption {
        value: String::new(),
        label: "Nenhuma".to_owned(),
        available: true,
        selected: true,
    })
    .chain(items(&hypotheses).iter().map(|h| SelectOption {
        value: text(h, "id"),
        label: text(h, "statement"),
        available: true,
        selected: false,
    }))
    .collect();

    let sem_versoes = methodology_versions.is_empty();
    let versoes: Vec<SelectOption> = std::iter::once(SelectOption {
        value: String::new(),
        label: if sem_versoes {
            "Nenhuma metodologia publicada neste ambiente".to_owned()
        } else {
            "Nenhuma".to_owned()
        },
        available: true,
        selected: true,
    })
    .chain(
        methodology_versions
            .into_iter()
            .map(|(valor, rotulo)| SelectOption {
                value: valor,
                label: rotulo,
                available: true,
                selected: false,
            }),
    )
    .collect();

    view! {
        <div class="oc-page oc-page--narrow">
            {cabecalho(
                "Novo estudo",
                "Um estudo põe uma hipótese à prova por um método. O que ele seguiu fica \
                 registado com a versão exacta — porque o método melhora, e o que se fez \
                 não muda por isso.",
                &workspace,
            )}
            {recusa(message)}

            <form method="post" action=format!("/workspaces/{id}/science/studies/new")>
                {card(
                    section_head("O ESTUDO", None, None),
                    view! {
                        {text_field(
                            "estudo-titulo",
                            "Como se chama",
                            "title",
                            "Ex.: ensaio de carga em contactos dopados",
                            "text",
                        )}
                        // Género fechado: o vocabulário é do Core, e um campo
                        // livre deixaria uma cadeia de caracteres qualquer
                        // chegar a um `CHECK` da base.
                        {radio_group(
                            "kind",
                            "Género",
                            vec![
                                RadioOption::new("physical_experiment", "Experiência física", true),
                                RadioOption::new("simulation", "Simulação", false),
                                RadioOption::new("analysis", "Análise", false),
                            ],
                        )}
                        {textarea(
                            "estudo-objectivo",
                            "O que se propõe descobrir",
                            "objective",
                            "O que este estudo tem de mostrar para responder à pergunta",
                            64,
                        )}
                        {classificacoes()}
                    },
                )}

                {card(
                    section_head("A CADEIA", None, None),
                    view! {
                        {select_labelled(
                            "estudo-hipotese",
                            "Hipótese que testa",
                            "hypothesis_id",
                            hipoteses,
                        )}
                        {select_labelled(
                            "estudo-metodologia",
                            "Versão de metodologia que segue",
                            "methodology_version_id",
                            versoes,
                        )}
                        {sem_versoes
                            .then(|| {
                                view! {
                                    <p class="oc-muted">
                                        "Nenhuma metodologia deste ambiente tem versão publicada. \
                                         Um estudo pode ficar sem ela, e a linhagem dirá que \
                                         método seguiu quando alguém publicar uma."
                                    </p>
                                }
                            })}
                    },
                )}
                {accoes(&format!("/workspaces/{id}/science"), "Desenhar")}
            </form>
        </div>
    }
}

/// Um estudo e as suas corridas.
pub struct EstudoView {
    /// O estudo.
    pub study: Value,
    /// As execuções, da mais recente para a mais antiga.
    pub executions: Value,
    /// Se este membro pode registar uma execução.
    pub may_create: bool,
}

/// O detalhe de um estudo.
pub fn estudo(view: EstudoView) -> impl IntoView {
    let EstudoView {
        study,
        executions,
        may_create,
    } = view;

    let id = text(&study, "id");
    let title = text(&study, "title");
    let kind_label = text(&study, "kind_label");
    let objective = maybe(&study, "objective");
    let status = text(&study, "status_label");
    let classification = text(&study, "classification");
    let workspace_id = text(&study, "workspace_id");
    let corridas = items(&executions);

    view! {
        <div class="oc-band">
            <div class="oc-row--top oc-gap-11 oc-mb-3">
                <div class="oc-fill">
                    <div class="oc-row oc-row--wrap oc-gap-6">
                        {pill(kind_label)}
                        <h1 class="oc-t-screen">{title}</h1>
                        {badge(status.clone(), Tone::of(&status))}
                        {classification_badge(&classification)}
                    </div>
                    {objective.map(|o| view! { <div class="oc-muted oc-mt-3">{o}</div> })}
                </div>
                <div class="oc-head__actions">
                    {may_create
                        .then(|| {
                            button(
                                Button::new("Registar execução", Variant::Primary)
                                    .href(format!("/studies/{id}/executions/new")),
                            )
                        })}
                    {button(
                        Button::new("Voltar à ciência", Variant::Secondary)
                            .href(format!("/workspaces/{workspace_id}/science")),
                    )}
                </div>
            </div>
        </div>

        <div class="oc-page">
            <section class="oc-card">
                {section_head("Execuções", None, None)}
                <div class="oc-card__body">
                    {if corridas.is_empty() {
                        view! {
                            <p class="oc-muted">
                                "Ainda nenhuma corrida. É a execução, e não o estudo, que \
                                 produz um resultado — e são duas execuções, e não dois \
                                 estudos, que se comparam quando se reproduz."
                            </p>
                        }
                            .into_any()
                    } else {
                        view! {
                            <div>
                                {corridas
                                    .iter()
                                    .map(|e| {
                                        let sequencia = e
                                            .get("sequence")
                                            .and_then(Value::as_i64)
                                            .unwrap_or(0);
                                        let estado = text(e, "status");
                                        let execucao_id = text(e, "id");
                                        let onde = maybe(e, "environment")
                                            .or_else(|| maybe(e, "software_name"));
                                        view! {
                                            <div class="oc-list__row">
                                                <a
                                                    class="oc-mono"
                                                    href=format!("/executions/{execucao_id}")
                                                >
                                                    {format!("Execução {sequencia}")}
                                                </a>
                                                <span class="oc-fill oc-truncate oc-muted">
                                                    {onde.unwrap_or_default()}
                                                </span>
                                                {badge(estado.clone(), Tone::of(&estado))}
                                            </div>
                                        }
                                    })
                                    .collect_view()}
                            </div>
                        }
                            .into_any()
                    }}
                </div>
            </section>
        </div>
    }
}

/// Registar uma corrida.
pub struct NovaExecucaoView {
    /// O estudo que corre.
    pub study: Value,
    /// As versões de metodologia publicadas no ambiente.
    pub methodology_versions: Vec<(String, String)>,
    /// As versões de dataset que este membro alcança, com o nome do conjunto.
    ///
    /// **Versões**, e nunca datasets: a matriz aceita
    /// `DatasetVersion → StudyExecution` e recusa o dataset mutável. Um
    /// conjunto cresce; uma corrida consumiu o que existia naquele dia.
    pub dataset_versions: Vec<(String, String)>,
    /// A recusa do Core, quando houve uma.
    pub message: Option<String>,
}

/// O formulário de uma execução.
pub fn nova_execucao(view: NovaExecucaoView) -> impl IntoView {
    let NovaExecucaoView {
        study,
        methodology_versions,
        dataset_versions,
        message,
    } = view;
    let id = text(&study, "id");
    let title = text(&study, "title");

    let opcoes = |pares: Vec<(String, String)>, vazio: &'static str| -> Vec<SelectOption> {
        let sem = pares.is_empty();
        std::iter::once(SelectOption {
            value: String::new(),
            label: if sem {
                vazio.to_owned()
            } else {
                "Nenhuma".to_owned()
            },
            available: true,
            selected: true,
        })
        .chain(pares.into_iter().map(|(valor, rotulo)| SelectOption {
            value: valor,
            label: rotulo,
            available: true,
            selected: false,
        }))
        .collect()
    };

    view! {
        <div class="oc-page oc-page--narrow">
            <div class="oc-head">
                <div class="oc-head__text">
                    <h1>"Registar execução"</h1>
                    <p>
                        {format!(
                            "Uma corrida de «{title}». É aqui que a reprodutibilidade mora: \
                             o mesmo estudo corre duas vezes e dá duas execuções, e são elas \
                             que se comparam.",
                        )}
                    </p>
                </div>
            </div>
            {recusa(message)}

            <form method="post" action=format!("/studies/{id}/executions/new")>
                {card(
                    section_head("A CORRIDA", None, None),
                    view! {
                        {radio_group(
                            "status",
                            "Estado",
                            vec![
                                RadioOption::new("succeeded", "Correu bem", true),
                                RadioOption::new("running", "A correr", false),
                                RadioOption::new("failed", "Falhou", false),
                                RadioOption::new("aborted", "Interrompida", false),
                                RadioOption::new("recorded", "Só registada", false),
                            ],
                        )}
                        {text_field(
                            "execucao-ambiente",
                            "Onde correu",
                            "environment",
                            "A máquina, o laboratório, o serviço",
                            "text",
                        )}
                        {text_field(
                            "execucao-software",
                            "Que software",
                            "software_name",
                            "Ex.: OpenFOAM",
                            "text",
                        )}
                        {text_field(
                            "execucao-versao",
                            "Que versão do software",
                            "software_version",
                            "Ex.: 11",
                            "text",
                        )}
                        {textarea(
                            "execucao-notas",
                            "O que houve a registar",
                            "notes",
                            "Condições, desvios, o que correu mal",
                            64,
                        )}
                    },
                )}

                {card(
                    section_head("O QUE ESTA CORRIDA USOU", None, None),
                    view! {
                        {select_labelled(
                            "execucao-metodologia",
                            "Versão de metodologia",
                            "methodology_version_id",
                            opcoes(
                                methodology_versions,
                                "Nenhuma metodologia publicada neste ambiente",
                            ),
                        )}
                        {select_labelled(
                            "execucao-dataset",
                            "Versão de dataset",
                            "dataset_version_id",
                            opcoes(dataset_versions, "Nenhum dataset com versão neste ambiente"),
                        )}
                        <p class="oc-muted">
                            "O que escolher aqui fica na proveniência como observado por esta \
                             operação — não como algo que alguém afirmou depois."
                        </p>
                    },
                )}
                {accoes(&format!("/studies/{id}"), "Registar")}
            </form>
        </div>
    }
}

/// Uma corrida, e o que dela saiu.
pub struct ExecucaoView {
    /// A execução.
    pub execution: Value,
    /// O estudo a que pertence.
    pub study: Value,
    /// Os resultados que esta corrida produziu.
    pub results: Value,
    /// Se este membro pode registar um resultado.
    pub may_create: bool,
}

/// O detalhe de uma execução.
pub fn execucao(view: ExecucaoView) -> impl IntoView {
    let ExecucaoView {
        execution,
        study,
        results,
        may_create,
    } = view;

    let id = text(&execution, "id");
    let sequencia = execution
        .get("sequence")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let estado = text(&execution, "status");
    let study_id = text(&study, "id");
    let study_title = text(&study, "title");
    let saidos = items(&results);

    let ficha = [
        ("Onde correu", maybe(&execution, "environment")),
        ("Software", maybe(&execution, "software_name")),
        ("Versão", maybe(&execution, "software_version")),
        ("Commit", maybe(&execution, "software_commit")),
        ("Notas", maybe(&execution, "notes")),
    ];

    view! {
        <div class="oc-band">
            <div class="oc-row--top oc-gap-11 oc-mb-3">
                <div class="oc-fill">
                    <div class="oc-row oc-row--wrap oc-gap-6">
                        <h1 class="oc-t-screen">{format!("Execução {sequencia}")}</h1>
                        {badge(estado.clone(), Tone::of(&estado))}
                    </div>
                    <div class="oc-mono oc-mt-3">
                        <a href=format!("/studies/{study_id}")>{study_title}</a>
                    </div>
                </div>
                <div class="oc-head__actions">
                    // Registar o resultado **aqui** é o que faz a proveniência
                    // nascer sozinha: a operação sabe de que corrida ele veio,
                    // e escreve a aresta na mesma transacção. Não há um segundo
                    // passo a pedir «agora indique a origem».
                    {may_create
                        .then(|| {
                            button(
                                Button::new("Registar resultado", Variant::Primary)
                                    .href(format!("/executions/{id}/results/new")),
                            )
                        })}
                </div>
            </div>
        </div>

        <div class="oc-page">
            <div class="oc-grid oc-grid--pares">
                <section class="oc-card">
                    {section_head("A corrida", None, None)}
                    <div class="oc-card__body">
                        {ficha
                            .into_iter()
                            .filter_map(|(rotulo, valor)| {
                                valor
                                    .map(|v| {
                                        view! {
                                            <div class="oc-list__row">
                                                <span class="oc-muted">{rotulo}</span>
                                                <span class="oc-fill oc-truncate">{v}</span>
                                            </div>
                                        }
                                    })
                            })
                            .collect_view()}
                    </div>
                </section>

                <section class="oc-card">
                    {section_head("O que produziu", None, None)}
                    <div class="oc-card__body">
                        {if saidos.is_empty() {
                            view! { <p class="oc-muted">"Ainda nenhum resultado."</p> }.into_any()
                        } else {
                            view! {
                                <div>
                                    {saidos
                                        .iter()
                                        .map(|r| {
                                            let rid = text(r, "id");
                                            let rtitulo = text(r, "title");
                                            view! {
                                                <div class="oc-list__row">
                                                    <a
                                                        class="oc-fill oc-truncate oc-t-cell-2"
                                                        href=format!("/results/{rid}")
                                                    >
                                                        {rtitulo}
                                                    </a>
                                                    {classification_badge(
                                                        &text(r, "classification"),
                                                    )}
                                                </div>
                                            }
                                        })
                                        .collect_view()}
                                </div>
                            }
                                .into_any()
                        }}
                    </div>
                </section>
            </div>
        </div>
    }
}

/// Registar um resultado a partir da corrida que o produziu.
pub struct NovoResultadoView {
    /// A execução de onde ele vem.
    pub execution: Value,
    /// O estudo, para dizer onde se está.
    pub study: Value,
    /// A recusa do Core, quando houve uma.
    pub message: Option<String>,
}

/// O formulário de um resultado.
///
/// # Não há campo de proveniência
///
/// E é deliberado. A origem já é conhecida: este formulário abre a partir da
/// execução, e a operação do Core escreve `produzido por` na mesma transacção
/// em que escreve o resultado. Um selector de origem aqui pediria à pessoa que
/// confirmasse o que o sistema acabou de observar — e abriria a porta a que ela
/// respondesse outra coisa.
pub fn novo_resultado(view: NovoResultadoView) -> impl IntoView {
    let NovoResultadoView {
        execution,
        study,
        message,
    } = view;
    let id = text(&execution, "id");
    let sequencia = execution
        .get("sequence")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let study_title = text(&study, "title");

    view! {
        <div class="oc-page oc-page--narrow">
            <div class="oc-head">
                <div class="oc-head__text">
                    <h1>"Registar resultado"</h1>
                    <p>
                        "O que esta corrida mostrou — incluindo quando mostrou que a hipótese \
                         não se sustenta, que é um resultado como outro qualquer."
                    </p>
                </div>
                <div class="oc-mono">
                    {format!("{study_title} · execução {sequencia}")}
                </div>
            </div>
            {recusa(message)}

            <div class="oc-callout" role="note">
                <strong>"A origem fica registada sozinha"</strong>
                <p>
                    {format!(
                        "Este resultado nasce da execução {sequencia}, e o Ocinye OS escreve \
                         essa ligação no mesmo acto. Não há um passo a seguir para indicar \
                         de onde veio.",
                    )}
                </p>
            </div>

            <form method="post" action=format!("/executions/{id}/results/new")>
                {card(
                    section_head("O RESULTADO", None, None),
                    view! {
                        {text_field(
                            "resultado-titulo",
                            "Como se chama",
                            "title",
                            "O que se pode dizer numa linha",
                            "text",
                        )}
                        {textarea(
                            "resultado-resumo",
                            "O que diz",
                            "summary",
                            "O que se observou, e em que condições",
                            80,
                        )}
                        {classificacoes()}
                    },
                )}
                {accoes(&format!("/executions/{id}"), "Registar")}
            </form>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn html(vista: impl IntoView) -> String {
        vista.into_view().to_html()
    }

    fn ambiente() -> Value {
        json!({"id": "55555555-5555-5555-5555-555555555555", "code": "AI-P", "unit_code": "AI"})
    }

    /// O selector do estudo oferece **versões**, e não metodologias.
    ///
    /// # A propriedade
    ///
    /// > **A Experience segue o contrato; não espera pelo `422` para ensinar a
    /// > regra.**
    ///
    /// A matriz de proveniência aceita `Study → MethodologyVersion` e recusa
    /// `Study → Methodology`. Um selector que oferecesse a metodologia mutável
    /// poria no ecrã uma escolha que o Core recusa — e a pessoa só descobriria
    /// depois de preencher o resto.
    ///
    /// O que isto mede é o **nome do campo**: `methodology_version_id`. Um
    /// campo chamado `methodology_id` chegaria ao Core como outra coisa, e a
    /// aresta ou não nasceria ou nasceria errada.
    #[test]
    fn o_estudo_escolhe_a_versao_e_nunca_a_metodologia() {
        let saida = html(novo_estudo(NovoEstudoView {
            workspace: ambiente(),
            hypotheses: json!([]),
            methodology_versions: vec![(
                "88888888-8888-8888-8888-888888888883".to_owned(),
                "Medição a quatro pontas · v2".to_owned(),
            )],
            message: None,
        }));

        assert!(
            saida.contains(r#"name="methodology_version_id""#),
            "o estudo não envia a versão de metodologia"
        );
        assert!(
            !saida.contains(r#"name="methodology_id""#),
            "o estudo oferece a metodologia mutável, que a matriz recusa"
        );
        // E o rótulo diz «· v2»: quem escolhe vê que está a escolher uma
        // versão, e não o método em geral.
        assert!(
            saida.contains("· v2"),
            "o selector não mostra qual é a versão"
        );
    }

    /// A execução consome **versões** de dataset.
    #[test]
    fn a_execucao_consome_versoes_de_dataset() {
        let saida = html(nova_execucao(NovaExecucaoView {
            study: json!({"id": "77777777-7777-7777-7777-777777777773", "title": "Ensaio"}),
            methodology_versions: Vec::new(),
            dataset_versions: vec![(
                "99999999-9999-9999-9999-999999999991".to_owned(),
                "SCADA Parque A · v4".to_owned(),
            )],
            message: None,
        }));

        assert!(
            saida.contains(r#"name="dataset_version_id""#),
            "a execução não envia a versão de dataset"
        );
        assert!(
            !saida.contains(r#"name="dataset_id""#),
            "a execução oferece o dataset mutável, que a matriz recusa"
        );
    }

    /// Nenhum formulário desta família oferece a origem da proveniência.
    ///
    /// # Porquê
    ///
    /// Porque `origin` não é uma escolha: `operation` significa que o Core
    /// **observou** a relação acontecer, e uma pessoa a marcá-lo estaria a
    /// afirmar que o sistema viu o que não viu. É a fronteira entre o que se
    /// sugere e o que a instituição registou.
    #[test]
    fn nenhum_formulario_deixa_escolher_a_origem_da_proveniencia() {
        let ecras = [
            html(nova_hipotese(Contexto {
                workspace: ambiente(),
                message: None,
            })),
            html(novo_estudo(NovoEstudoView {
                workspace: ambiente(),
                hypotheses: json!([]),
                methodology_versions: Vec::new(),
                message: None,
            })),
            html(nova_execucao(NovaExecucaoView {
                study: json!({"id": "1", "title": "Ensaio"}),
                methodology_versions: Vec::new(),
                dataset_versions: Vec::new(),
                message: None,
            })),
            html(novo_resultado(NovoResultadoView {
                execution: json!({"id": "1", "sequence": 3}),
                study: json!({"title": "Ensaio"}),
                message: None,
            })),
        ];

        for saida in &ecras {
            assert!(
                !saida.contains(r#"name="origin""#),
                "um formulário deixa escolher a origem da proveniência"
            );
            assert!(
                !saida.contains(r#"value="operation""#),
                "um formulário oferece `operation` como valor submissível"
            );
        }
    }

    /// O resultado nasce da execução, e não pede a origem numa segunda etapa.
    #[test]
    fn o_resultado_ja_sabe_de_onde_vem() {
        let saida = html(novo_resultado(NovoResultadoView {
            execution: json!({"id": "77777777-7777-7777-7777-777777777776", "sequence": 3}),
            study: json!({"title": "Ensaio de carga"}),
            message: None,
        }));

        // Submete para dentro da execução: o caminho carrega a origem.
        assert!(
            saida.contains("/executions/77777777-7777-7777-7777-777777777776/results/new"),
            "o resultado não é registado a partir da execução"
        );
        // E não há campo nenhum para a pessoa nomear a proveniência.
        assert!(
            !saida.contains(r#"name="execution_id""#),
            "o formulário pede à pessoa a origem que o caminho já diz"
        );
    }

    /// O género do estudo é um conjunto fechado, e o do Core.
    ///
    /// Uma cadeia de caracteres livre chegaria ao `CHECK` da base, e um erro de
    /// quem preenche voltaria como avaria.
    #[test]
    fn o_genero_do_estudo_e_o_vocabulario_do_core() {
        let saida = html(novo_estudo(NovoEstudoView {
            workspace: ambiente(),
            hypotheses: json!([]),
            methodology_versions: Vec::new(),
            message: None,
        }));

        for genero in ["physical_experiment", "simulation", "analysis"] {
            assert!(
                saida.contains(&format!(r#"value="{genero}""#)),
                "falta o género «{genero}»"
            );
        }
        assert!(
            !saida.contains(r#"name="kind" type="text""#),
            "o género é um campo livre"
        );
    }

    /// O estado de uma execução também é fechado.
    #[test]
    fn o_estado_da_execucao_e_o_vocabulario_do_core() {
        let saida = html(nova_execucao(NovaExecucaoView {
            study: json!({"id": "1", "title": "Ensaio"}),
            methodology_versions: Vec::new(),
            dataset_versions: Vec::new(),
            message: None,
        }));

        for estado in ["recorded", "running", "succeeded", "failed", "aborted"] {
            assert!(
                saida.contains(&format!(r#"value="{estado}""#)),
                "falta o estado «{estado}»"
            );
        }
        assert!(
            !saida.contains(r#"value="completed""#),
            "o formulário oferece um estado que a base recusa"
        );
    }

    /// Quem não pode criar não vê as acções de criação.
    ///
    /// Esconder o botão **não é** segurança — o Core recusa na mesma. É para
    /// não prometer o que não se cumpre.
    #[test]
    fn sem_autorizacao_a_cadeia_nao_promete_criacao() {
        let saida = html(scientific_chain(ChainView {
            overview: json!({"workspace": ambiente()}),
            hypotheses: json!([]),
            methodologies: json!([]),
            studies: json!([]),
            results: json!([]),
            may_create: false,
        }));

        assert!(
            !saida.contains("/science/hypotheses/new"),
            "a cadeia oferece criar a quem não pode"
        );
    }

    /// A versão publicada não aparece como formulário.
    #[test]
    fn uma_versao_publicada_nao_e_um_campo_por_preencher() {
        let saida = html(metodologia(MetodologiaView {
            methodology: json!({
                "id": "1",
                "workspace_id": "2",
                "title": "Medição a quatro pontas",
                "classification": "INTERNAL"
            }),
            versions: json!([{
                "id": "3", "label": "v2", "summary": "Corrente reduzida.",
                "status": "published"
            }]),
            may_create: true,
        }));

        assert!(
            !saida.contains("<form"),
            "a metodologia apresenta as suas versões como formulário editável"
        );
        assert!(
            saida.contains("/methodologies/1/versions/new"),
            "não há caminho para substituir a versão em vigor"
        );
    }
}
