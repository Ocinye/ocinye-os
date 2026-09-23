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
                        <h1 class="oc-t-screen">{crate::i18n::t("science.title")}</h1>
                    </div>
                    <div class="oc-mono oc-mt-3">{contexto}</div>
                </div>
                <div class="oc-head__actions">
                    {may_create
                        .then(|| {
                            view! {
                                {button(
                                    Button::new(crate::i18n::t("science.new_hypothesis"), Variant::Primary)
                                        .href(
                                            format!("/workspaces/{id}/science/hypotheses/new"),
                                        ),
                                )}
                                {button(
                                    Button::new(crate::i18n::t("science.new_methodology"), Variant::Secondary)
                                        .href(
                                            format!("/workspaces/{id}/science/methodologies/new"),
                                        ),
                                )}
                                {button(
                                    Button::new(crate::i18n::t("science.new_study"), Variant::Secondary)
                                        .href(format!("/workspaces/{id}/science/studies/new")),
                                )}
                            }
                        })}
                    {button(
                        Button::new(crate::i18n::t("science.back_to_workspace"), Variant::Secondary)
                            .href(format!("/workspaces/{id}")),
                    )}
                </div>
            </div>
        </div>

        <div class="oc-page">
            {if vazia {
                empty_state(EmptyState {
                    icon: Icon::Science,
                    title: crate::i18n::t("science.empty.title").to_owned(),
                    body: if may_create {
                        crate::i18n::t("science.empty.body_creator").to_owned()
                    } else {
                        crate::i18n::t("science.empty.body_viewer").to_owned()
                    },
                    actions: if may_create {
                        vec![
                            Button::new(
                                crate::i18n::t("science.empty.first_hypothesis"),
                                Variant::Primary,
                            )
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
                        {etapa(crate::i18n::t("science.stage.hypotheses"), &hypotheses, "statement", None)}
                        {etapa(crate::i18n::t("science.stage.methodologies"), &methodologies, "title", Some("/methodologies"))}
                    </div>
                    <div class="oc-grid oc-grid--pares oc-mt-7">
                        {etapa(crate::i18n::t("science.stage.studies"), &studies, "title", Some("/studies"))}
                        {etapa(crate::i18n::t("science.stage.results"), &results, "title", Some("/results"))}
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
                    view! { <p class="oc-muted">{crate::i18n::t("science.stage.empty")}</p> }.into_any()
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
                                Button::new(crate::i18n::t("science.validate_result"), Variant::Primary)
                                    .href(format!("/results/{id}/validate")),
                            )
                        })}
                </div>
            </div>
        </div>

        <div class="oc-page">
            <div class="oc-grid oc-grid--ws">
                <section class="oc-card">
                    {section_head(crate::i18n::t("science.result.summary_head"), None, None)}
                    <div class="oc-card__body">
                        <p>{summary}</p>
                    </div>
                </section>

                <section class="oc-card">
                    {section_head(crate::i18n::t("science.result.validations_head"), None, None)}
                    <div class="oc-card__body">
                        {if validations.is_empty() {
                            view! {
                                <p class="oc-muted">
                                    {crate::i18n::t("science.result.no_validations")}
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
                {section_head(crate::i18n::t("science.result.provenance_head"), None, None)}
                <div class="oc-card__body">
                    {pill_tabs(
                        vec![
                            Tab::link(crate::i18n::t("science.lineage.upstream"), format!("/results/{id}?direction=upstream"), a_montante),
                            Tab::link(
                                crate::i18n::t("science.lineage.downstream"),
                                format!("/results/{id}?direction=downstream"),
                                !a_montante,
                            ),
                        ],
                        crate::i18n::t("science.lineage.aria"),
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
        crate::i18n::t("science.lineage.empty_upstream")
    } else {
        crate::i18n::t("science.lineage.empty_downstream")
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
                                (Tone::Navy, crate::i18n::t("science.provenance.observed"))
                            } else {
                                (Tone::Gray, crate::i18n::t("science.provenance.declared"))
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
                        {crate::i18n::t("science.lineage.truncated")}
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
    let porque_nao =
        sem_execucao.then(|| crate::i18n::t("science.validate.no_execution").to_owned());

    let opcoes_de_execucao: Vec<SelectOption> = std::iter::once(SelectOption {
        value: String::new(),
        label: crate::i18n::t("science.option.none").to_owned(),
        available: true,
        selected: true,
    })
    .chain(execucoes.iter().map(|e| {
        let sequencia = e.get("sequence").and_then(Value::as_i64).unwrap_or(0);
        SelectOption {
            value: text(e, "id"),
            label: format!(
                "{} · {}",
                crate::i18n::tf(
                    "science.execution.numbered",
                    &[("sequence", &sequencia.to_string())],
                ),
                text(e, "status"),
            ),
            available: true,
            selected: false,
        }
    }))
    .collect();

    view! {
        <div class="oc-page oc-page--narrow">
            <div class="oc-head">
                <div class="oc-head__text">
                    <h1>{crate::i18n::t("science.validate_result")}</h1>
                    <p>{crate::i18n::tf("science.validate.subtitle", &[("title", &title)])}</p>
                </div>
            </div>

            {message
                .map(|texto| {
                    view! { <div class="oc-callout oc-callout--error" role="alert">{texto}</div> }
                })}

            <div class="oc-callout" role="note">
                <strong>{crate::i18n::t("science.validate.callout_title")}</strong>
                <p>
                    {crate::i18n::t("science.validate.callout_body")}
                </p>
            </div>

            <form method="post" action=format!("/results/{id}/validate")>
                {card(
                    section_head(crate::i18n::t("science.claim_head"), None, None),
                    view! {
                        {radio_group(
                            "kind",
                            crate::i18n::t("science.validate.kind_label"),
                            vec![
                                RadioOption::new("validation", crate::i18n::t("science.validate.kind.validation"), true),
                                RadioOption {
                                    value: "reproduction",
                                    label: crate::i18n::t("science.validate.kind.reproduction"),
                                    selected: false,
                                    unavailable_reason: porque_nao,
                                },
                            ],
                        )}
                        {radio_group(
                            "outcome",
                            crate::i18n::t("science.validate.outcome_label"),
                            vec![
                                RadioOption::new("confirmed", crate::i18n::t("science.validate.outcome.confirmed"), true),
                                RadioOption::new("contradicted", crate::i18n::t("science.validate.outcome.contradicted"), false),
                                RadioOption::new("inconclusive", crate::i18n::t("science.validate.outcome.inconclusive"), false),
                            ],
                        )}
                        {select_labelled(
                            "validation-execution",
                            crate::i18n::t("science.validate.execution_label"),
                            "execution_id",
                            opcoes_de_execucao,
                        )}
                        {textarea(
                            "validation-note",
                            crate::i18n::t("science.validate.note_label"),
                            "note",
                            crate::i18n::t("science.validate.note_placeholder"),
                            64,
                        )}
                    },
                )}

                <div class="oc-row oc-gap-6 oc-mt-7">
                    {button(Button::new(crate::i18n::t("science.action.record"), Variant::Primary))}
                    {button(
                        Button::new(crate::i18n::t("action.cancel"), Variant::Secondary).href(format!("/results/{id}")),
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
        crate::i18n::t("classification.unknown"),
        "classification",
        vec![
            SelectOption {
                value: "INTERNAL".to_owned(),
                label: crate::i18n::t("science.class.internal").to_owned(),
                available: true,
                selected: true,
            },
            SelectOption {
                value: "PUBLIC".to_owned(),
                label: crate::i18n::t("science.class.public").to_owned(),
                available: true,
                selected: false,
            },
            SelectOption {
                value: "CONFIDENTIAL".to_owned(),
                label: crate::i18n::t("science.class.confidential").to_owned(),
                available: true,
                selected: false,
            },
            SelectOption {
                value: "RESTRICTED".to_owned(),
                label: crate::i18n::t("science.class.restricted").to_owned(),
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
                crate::i18n::t("science.new_hypothesis"),
                crate::i18n::t("science.hypothesis.subtitle"),
                &workspace,
            )}
            {recusa(message)}

            <form method="post" action=format!("/workspaces/{id}/science/hypotheses/new")>
                {card(
                    section_head(crate::i18n::t("science.claim_head"), None, None),
                    view! {
                        {textarea(
                            "hipotese-afirmacao",
                            crate::i18n::t("science.hypothesis.statement_label"),
                            "statement",
                            crate::i18n::t("science.hypothesis.statement_placeholder"),
                            64,
                        )}
                        {textarea(
                            "hipotese-razao",
                            crate::i18n::t("science.hypothesis.rationale_label"),
                            "rationale",
                            crate::i18n::t("science.hypothesis.rationale_placeholder"),
                            64,
                        )}
                        {classificacoes()}
                    },
                )}
                {accoes(&format!("/workspaces/{id}/science"), crate::i18n::t("science.hypothesis.submit"))}
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
            {button(Button::new(crate::i18n::t("action.cancel"), Variant::Secondary).href(voltar))}
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
                crate::i18n::t("science.new_methodology"),
                crate::i18n::t("science.methodology.subtitle"),
                &workspace,
            )}
            {recusa(message)}

            <form method="post" action=format!("/workspaces/{id}/science/methodologies/new")>
                {card(
                    section_head(crate::i18n::t("science.methodology.method_head"), None, None),
                    view! {
                        {text_field(
                            "metodologia-titulo",
                            crate::i18n::t("science.field.name_label"),
                            "title",
                            crate::i18n::t("science.methodology.title_placeholder"),
                            "text",
                        )}
                        {textarea(
                            "metodologia-proposito",
                            crate::i18n::t("science.methodology.purpose_label"),
                            "purpose",
                            crate::i18n::t("science.methodology.purpose_placeholder"),
                            64,
                        )}
                        {classificacoes()}
                    },
                )}
                {accoes(&format!("/workspaces/{id}/science"), crate::i18n::t("action.create"))}
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
                                Button::new(crate::i18n::t("science.new_version"), Variant::Primary)
                                    .href(format!("/methodologies/{id}/versions/new")),
                            )
                        })}
                    {button(
                        Button::new(crate::i18n::t("science.back_to_science"), Variant::Secondary)
                            .href(format!("/workspaces/{workspace_id}/science")),
                    )}
                </div>
            </div>
        </div>

        <div class="oc-page">
            <section class="oc-card">
                {section_head(crate::i18n::t("science.versions_head"), None, None)}
                <div class="oc-card__body">
                    {if versoes.is_empty() {
                        view! {
                            <p class="oc-muted">
                                {crate::i18n::t("science.methodology.no_versions")}
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
                    <h1>{crate::i18n::t("science.new_version")}</h1>
                    <p>{crate::i18n::tf("science.version.subtitle", &[("title", &title)])}</p>
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
                            <strong>{crate::i18n::tf("science.version.in_force", &[("label", &etiqueta)])}</strong>
                            <p>
                                {crate::i18n::t("science.version.in_force_body")}
                            </p>
                        </div>
                    }
                })}

            <form method="post" action=format!("/methodologies/{id}/versions/new")>
                {card(
                    section_head(crate::i18n::t("science.version.version_head"), None, None),
                    view! {
                        {text_field(
                            "versao-etiqueta",
                            crate::i18n::t("science.field.name_label"),
                            "label",
                            crate::i18n::t("science.version.label_placeholder"),
                            "text",
                        )}
                        {textarea(
                            "versao-resumo",
                            crate::i18n::t("science.version.summary_label"),
                            "summary",
                            crate::i18n::t("science.version.summary_placeholder"),
                            80,
                        )}
                    },
                )}
                {accoes(&format!("/methodologies/{id}"), crate::i18n::t("science.version.submit"))}
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
        label: crate::i18n::t("science.option.none").to_owned(),
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
            crate::i18n::t("science.no_published_methodology").to_owned()
        } else {
            crate::i18n::t("science.option.none").to_owned()
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
                crate::i18n::t("science.new_study"),
                crate::i18n::t("science.study.subtitle"),
                &workspace,
            )}
            {recusa(message)}

            <form method="post" action=format!("/workspaces/{id}/science/studies/new")>
                {card(
                    section_head(crate::i18n::t("science.study.study_head"), None, None),
                    view! {
                        {text_field(
                            "estudo-titulo",
                            crate::i18n::t("science.field.name_label"),
                            "title",
                            crate::i18n::t("science.study.title_placeholder"),
                            "text",
                        )}
                        // Género fechado: o vocabulário é do Core, e um campo
                        // livre deixaria uma cadeia de caracteres qualquer
                        // chegar a um `CHECK` da base.
                        {radio_group(
                            "kind",
                            crate::i18n::t("science.study.kind_label"),
                            vec![
                                RadioOption::new("physical_experiment", crate::i18n::t("science.study.kind.physical"), true),
                                RadioOption::new("simulation", crate::i18n::t("science.study.kind.simulation"), false),
                                RadioOption::new("analysis", crate::i18n::t("science.study.kind.analysis"), false),
                            ],
                        )}
                        {textarea(
                            "estudo-objectivo",
                            crate::i18n::t("science.study.objective_label"),
                            "objective",
                            crate::i18n::t("science.study.objective_placeholder"),
                            64,
                        )}
                        {classificacoes()}
                    },
                )}

                {card(
                    section_head(crate::i18n::t("science.study.chain_head"), None, None),
                    view! {
                        {select_labelled(
                            "estudo-hipotese",
                            crate::i18n::t("science.study.hypothesis_label"),
                            "hypothesis_id",
                            hipoteses,
                        )}
                        {select_labelled(
                            "estudo-metodologia",
                            crate::i18n::t("science.study.methodology_label"),
                            "methodology_version_id",
                            versoes,
                        )}
                        {sem_versoes
                            .then(|| {
                                view! {
                                    <p class="oc-muted">
                                        {crate::i18n::t("science.study.no_methodology_hint")}
                                    </p>
                                }
                            })}
                    },
                )}
                {accoes(&format!("/workspaces/{id}/science"), crate::i18n::t("science.study.submit"))}
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
                                Button::new(crate::i18n::t("science.record_execution"), Variant::Primary)
                                    .href(format!("/studies/{id}/executions/new")),
                            )
                        })}
                    {button(
                        Button::new(crate::i18n::t("science.back_to_science"), Variant::Secondary)
                            .href(format!("/workspaces/{workspace_id}/science")),
                    )}
                </div>
            </div>
        </div>

        <div class="oc-page">
            <section class="oc-card">
                {section_head(crate::i18n::t("science.executions_head"), None, None)}
                <div class="oc-card__body">
                    {if corridas.is_empty() {
                        view! {
                            <p class="oc-muted">
                                {crate::i18n::t("science.study.no_executions")}
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
                                                    {crate::i18n::tf("science.execution.numbered", &[("sequence", &sequencia.to_string())])}
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
                crate::i18n::t("science.option.none").to_owned()
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
                    <h1>{crate::i18n::t("science.record_execution")}</h1>
                    <p>
                        {crate::i18n::tf("science.execution.subtitle", &[("title", &title)])}
                    </p>
                </div>
            </div>
            {recusa(message)}

            <form method="post" action=format!("/studies/{id}/executions/new")>
                {card(
                    section_head(crate::i18n::t("science.run_head"), None, None),
                    view! {
                        {radio_group(
                            "status",
                            crate::i18n::t("science.execution.status_label"),
                            vec![
                                RadioOption::new("succeeded", crate::i18n::t("science.execution.status.succeeded"), true),
                                RadioOption::new("running", crate::i18n::t("science.execution.status.running"), false),
                                RadioOption::new("failed", crate::i18n::t("science.execution.status.failed"), false),
                                RadioOption::new("aborted", crate::i18n::t("science.execution.status.aborted"), false),
                                RadioOption::new("recorded", crate::i18n::t("science.execution.status.recorded"), false),
                            ],
                        )}
                        {text_field(
                            "execucao-ambiente",
                            crate::i18n::t("science.execution.environment"),
                            "environment",
                            crate::i18n::t("science.execution.environment_placeholder"),
                            "text",
                        )}
                        {text_field(
                            "execucao-software",
                            crate::i18n::t("science.execution.software_label"),
                            "software_name",
                            crate::i18n::t("science.execution.software_placeholder"),
                            "text",
                        )}
                        {text_field(
                            "execucao-versao",
                            crate::i18n::t("science.execution.software_version_label"),
                            "software_version",
                            crate::i18n::t("science.execution.software_version_placeholder"),
                            "text",
                        )}
                        {textarea(
                            "execucao-notas",
                            crate::i18n::t("science.execution.notes_label"),
                            "notes",
                            crate::i18n::t("science.execution.notes_placeholder"),
                            64,
                        )}
                    },
                )}

                {card(
                    section_head(crate::i18n::t("science.execution.used_head"), None, None),
                    view! {
                        {select_labelled(
                            "execucao-metodologia",
                            crate::i18n::t("science.execution.methodology_version_label"),
                            "methodology_version_id",
                            opcoes(
                                methodology_versions,
                                crate::i18n::t("science.no_published_methodology"),
                            ),
                        )}
                        {select_labelled(
                            "execucao-dataset",
                            crate::i18n::t("science.execution.dataset_version_label"),
                            "dataset_version_id",
                            opcoes(dataset_versions, crate::i18n::t("science.execution.no_dataset_version")),
                        )}
                        <p class="oc-muted">
                            {crate::i18n::t("science.execution.provenance_hint")}
                        </p>
                    },
                )}
                {accoes(&format!("/studies/{id}"), crate::i18n::t("science.action.record"))}
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
        (
            crate::i18n::t("science.execution.environment"),
            maybe(&execution, "environment"),
        ),
        (
            crate::i18n::t("science.execution.software"),
            maybe(&execution, "software_name"),
        ),
        (
            crate::i18n::t("science.execution.version"),
            maybe(&execution, "software_version"),
        ),
        (
            crate::i18n::t("science.execution.commit"),
            maybe(&execution, "software_commit"),
        ),
        (
            crate::i18n::t("science.execution.notes"),
            maybe(&execution, "notes"),
        ),
    ];

    view! {
        <div class="oc-band">
            <div class="oc-row--top oc-gap-11 oc-mb-3">
                <div class="oc-fill">
                    <div class="oc-row oc-row--wrap oc-gap-6">
                        <h1 class="oc-t-screen">{crate::i18n::tf("science.execution.numbered", &[("sequence", &sequencia.to_string())])}</h1>
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
                                Button::new(crate::i18n::t("science.record_result"), Variant::Primary)
                                    .href(format!("/executions/{id}/results/new")),
                            )
                        })}
                </div>
            </div>
        </div>

        <div class="oc-page">
            <div class="oc-grid oc-grid--pares">
                <section class="oc-card">
                    {section_head(crate::i18n::t("science.run_head"), None, None)}
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
                    {section_head(crate::i18n::t("science.execution.produced_head"), None, None)}
                    <div class="oc-card__body">
                        {if saidos.is_empty() {
                            view! { <p class="oc-muted">{crate::i18n::t("science.execution.no_results")}</p> }.into_any()
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
                    <h1>{crate::i18n::t("science.record_result")}</h1>
                    <p>
                        {crate::i18n::t("science.result.subtitle")}
                    </p>
                </div>
                <div class="oc-mono">
                    {format!(
                        "{study_title} · {}",
                        crate::i18n::tf(
                            "science.execution.lower_numbered",
                            &[("sequence", &sequencia.to_string())],
                        ),
                    )}
                </div>
            </div>
            {recusa(message)}

            <div class="oc-callout" role="note">
                <strong>{crate::i18n::t("science.result.origin_callout_title")}</strong>
                <p>
                    {crate::i18n::tf(
                        "science.result.origin_callout_body",
                        &[("sequence", &sequencia.to_string())],
                    )}
                </p>
            </div>

            <form method="post" action=format!("/executions/{id}/results/new")>
                {card(
                    section_head(crate::i18n::t("science.result.result_head"), None, None),
                    view! {
                        {text_field(
                            "resultado-titulo",
                            crate::i18n::t("science.field.name_label"),
                            "title",
                            crate::i18n::t("science.result.title_placeholder"),
                            "text",
                        )}
                        {textarea(
                            "resultado-resumo",
                            crate::i18n::t("science.result.summary_label"),
                            "summary",
                            crate::i18n::t("science.result.summary_placeholder"),
                            80,
                        )}
                        {classificacoes()}
                    },
                )}
                {accoes(&format!("/executions/{id}"), crate::i18n::t("science.action.record"))}
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

/// Um ecrã, um idioma: a cadeia científica inteira renderizada em francês não
/// deixa passar chrome português. Cobre as três superfícies do ficheiro — a
/// cadeia e o seu estado vazio, o detalhe de um resultado com proveniência dos
/// dois lados, os formulários de hipótese/metodologia/versão/estudo/execução/
/// resultado, e o de validação — porque uma língua misturada esconde-se sempre
/// no ramo que o teste de agulhas não visitou (i18n §84, guarda de chrome).
#[cfg(test)]
mod pureza_i18n {
    use super::*;
    use serde_json::json;

    fn ambiente() -> Value {
        json!({"id": "55555555-5555-5555-5555-555555555555", "code": "AI-P", "unit_code": "AI"})
    }

    #[tokio::test]
    async fn a_ciencia_nao_mistura_linguas() {
        use crate::i18n::{with_locale, Locale};

        let fr = with_locale(Locale::Fr, async {
            let cadeia = scientific_chain(ChainView {
                overview: json!({"workspace": ambiente()}),
                hypotheses: json!([{
                    "id": "1", "statement": "Hypothèse A",
                    "status_label": "Ouverte", "classification": "INTERNAL"
                }]),
                methodologies: json!([]),
                studies: json!([]),
                results: json!([]),
                may_create: true,
            })
            .to_html();

            let vazia = scientific_chain(ChainView {
                overview: json!({"workspace": ambiente()}),
                hypotheses: json!([]),
                methodologies: json!([]),
                studies: json!([]),
                results: json!([]),
                may_create: true,
            })
            .to_html();

            let resultado = result_detail(ResultView {
                result: json!({
                    "id": "9", "title": "Titre du résultat", "summary": "Résumé.",
                    "status_label": "Confirmé", "classification": "INTERNAL"
                }),
                validations: json!([]),
                upstream: json!({"passos": [
                    {
                        "de": {"kind": "study_execution", "label": "Essai · exécution 3"},
                        "para": {"kind": "result", "label": "Titre du résultat"},
                        "relacao_legivel": "a produit", "origem": "operation"
                    },
                    {
                        "de": {"kind": "dataset_version", "label": "SCADA · v4"},
                        "para": {"kind": "study_execution", "label": "Essai · exécution 3"},
                        "relacao_legivel": "a utilisé", "origem": "declared"
                    }
                ], "truncada": true}),
                downstream: json!({"passos": [], "truncada": false}),
                direction: "upstream",
                may_validate: true,
            })
            .to_html();

            let validar = validate_result(ValidateView {
                result: json!({"id": "9", "title": "Titre du résultat"}),
                executions: json!([{"id": "e1", "sequence": 3, "status": "succeeded"}]),
                message: None,
            })
            .to_html();

            let hipotese = nova_hipotese(Contexto {
                workspace: ambiente(),
                message: None,
            })
            .to_html();

            let metodologia_nova = nova_metodologia(Contexto {
                workspace: ambiente(),
                message: None,
            })
            .to_html();

            let metodologia_detalhe = metodologia(MetodologiaView {
                methodology: json!({
                    "id": "1", "workspace_id": "2", "title": "Méthode",
                    "classification": "INTERNAL"
                }),
                versions: json!([]),
                may_create: true,
            })
            .to_html();

            let versao = nova_versao(NovaVersaoView {
                methodology: json!({"id": "1", "title": "Méthode"}),
                em_vigor: Some(json!({"label": "v1"})),
                message: None,
            })
            .to_html();

            let estudo_novo = novo_estudo(NovoEstudoView {
                workspace: ambiente(),
                hypotheses: json!([]),
                methodology_versions: Vec::new(),
                message: None,
            })
            .to_html();

            let estudo_detalhe = estudo(EstudoView {
                study: json!({
                    "id": "1", "workspace_id": "2", "title": "Essai",
                    "kind_label": "Analyse", "status_label": "Ouvert",
                    "classification": "INTERNAL"
                }),
                executions: json!([]),
                may_create: true,
            })
            .to_html();

            let execucao_nova = nova_execucao(NovaExecucaoView {
                study: json!({"id": "1", "title": "Essai"}),
                methodology_versions: Vec::new(),
                dataset_versions: Vec::new(),
                message: None,
            })
            .to_html();

            let execucao_detalhe = execucao(ExecucaoView {
                execution: json!({
                    "id": "1", "sequence": 3, "status": "succeeded",
                    "environment": "Labo", "notes": "RAS"
                }),
                study: json!({"id": "1", "title": "Essai"}),
                results: json!([]),
                may_create: true,
            })
            .to_html();

            let resultado_novo = novo_resultado(NovoResultadoView {
                execution: json!({"id": "1", "sequence": 3}),
                study: json!({"title": "Essai"}),
                message: None,
            })
            .to_html();

            [
                cadeia,
                vazia,
                resultado,
                validar,
                hipotese,
                metodologia_nova,
                metodologia_detalhe,
                versao,
                estudo_novo,
                estudo_detalhe,
                execucao_nova,
                execucao_detalhe,
                resultado_novo,
            ]
            .join("")
        })
        .await;

        for francesa in [
            "Nouvelle hypothèse",
            "Nouvelle méthodologie",
            "Nouvelle étude",
            "Retour à l’espace",
            "Hypothèses",
            "Provenance",
            "En amont",
            "En aval",
            "Observée",
            "Déclarée",
            "Sens de la lignée",
            "Valider le résultat",
            "Ce que vous enregistrez",
            "L’affirmation",
            "La méthode",
            "La chaîne",
            "Ce que cette exécution a utilisé",
            "Le résultat",
            "Concevoir",
            "Publier",
            "Énoncer",
            "Aucune",
        ] {
            assert!(fr.contains(francesa), "fr: falta «{francesa}»");
        }

        for portuguesa in [
            "Ciência",
            "Nova hipótese",
            "Voltar ao ambiente",
            "Proveniência",
            "Montante",
            "Jusante",
            "Observada",
            "Declarada",
            "Validar resultado",
            "A afirmação",
            "O que esta corrida usou",
            "Desenhar",
            "Enunciar a primeira hipótese",
            "Nenhuma",
        ] {
            assert!(
                !fr.contains(portuguesa),
                "fr: chrome português «{portuguesa}» sobreviveu"
            );
        }
    }
}
