//! Research Workspaces e detalhe de Unidade.
//!
//! O Research Workspace é um dos ecrãs mais importantes: tem de transmitir
//! "estou dentro desta investigação" (`design/README.md` §6.6, §6.7).
//!
//! # Ideia não é projecto
//!
//! Uma ideia e um projecto partilham a linguagem contextual mas não o conteúdo
//! nem os estados. Depois da promoção, o **mesmo** workspace passa a hospedar o
//! projecto, e a linhagem fica visível dos dois lados.

use leptos::prelude::*;
use serde_json::Value;

use crate::ui::components::{
    assist, badge, button, classification_badge, donut, pill, progress_bar, section_head, Assist,
    Button, Tone, Variant, IDEA_SUGGESTIONS, PROJECT_SUGGESTIONS,
};
use crate::ui::components::{context_tabs, Tab};

fn text(row: &Value, key: &str) -> String {
    row.get(key)
        .and_then(Value::as_str)
        .unwrap_or("—")
        .to_owned()
}

fn items(payload: &Value) -> Vec<Value> {
    payload
        .get("items")
        .and_then(Value::as_array)
        .or_else(|| payload.as_array())
        .cloned()
        .unwrap_or_default()
}

/// As 13 tabs de uma Ideia.
const IDEA_TABS: [&str; 13] = [
    "Visão geral",
    "Bibliografia",
    "Fontes",
    "Notas",
    "Documentos",
    "Datasets",
    "Código",
    "Experiências",
    "Resultados",
    "Tarefas",
    "IA",
    "Actividade",
    "Histórico",
];

/// As 13 tabs de um Projecto.
const PROJECT_TABS: [&str; 13] = [
    "Visão geral",
    "Membros",
    "Planeamento",
    "Bibliografia",
    "Documentos",
    "Dados",
    "Código",
    "Experiências",
    "Resultados",
    "Tarefas",
    "Financiamento",
    "IA",
    "Histórico",
];

/// Tudo o que o Research Workspace mostra.
pub struct WorkspaceView {
    /// Visão geral do workspace, vinda do Core.
    pub overview: Value,
    /// Bibliografia.
    pub sources: Value,
    /// Notas.
    pub notes: Value,
    /// Documentos.
    pub documents: Value,
    /// Datasets.
    pub datasets: Value,
    /// Tarefas.
    pub tasks: Value,
    /// Actividade.
    pub activity: Value,
    /// Se alguma capacidade de inferência pode ser servida, segundo o Core.
    pub inference_available: bool,
    /// Se este membro pode usar assistência.
    pub may_use_assistance: bool,
    /// Quem pertence ao ambiente, e se quem vê pode alterá-lo.
    pub gestao: GestaoDePessoas,
}

/// O destino de um separador do Research Workspace.
///
/// Um separador ou salta para uma secção deste ecrã (âncora `#…`, e o conteúdo
/// já está renderizado por baixo), ou leva a outro ecrã (cadeia científica, IA),
/// ou **não existe ainda** — e nesse caso não é um separador. Um separador
/// inerte que não navega é um controlo morto, e era o defeito (F-11).
fn tab_destination(label: &str, workspace_id: &str) -> Option<String> {
    Some(match label {
        // Secções deste ecrã — âncora para o conteúdo já renderizado.
        "Visão geral" => "#ws-visao-geral".to_owned(),
        "Membros" => "#ws-membros".to_owned(),
        // «Fontes» e «Bibliografia» são a mesma coisa: as referências do ambiente.
        "Bibliografia" | "Fontes" => "#ws-bibliografia".to_owned(),
        "Notas" => "#ws-notas".to_owned(),
        "Documentos" => "#ws-documentos".to_owned(),
        "Datasets" | "Dados" => "#ws-datasets".to_owned(),
        "Tarefas" => "#ws-tarefas".to_owned(),
        // A actividade é a história do ambiente.
        "Actividade" | "Histórico" => "#ws-actividade".to_owned(),
        // Outros ecrãs.
        "IA" => format!("/ai/prompt?workspace={workspace_id}"),
        // Experiências e Resultados são duas leituras da mesma cadeia
        // científica, e por isso levam ao mesmo ecrã.
        "Experiências" | "Resultados" => format!("/workspaces/{workspace_id}/science"),
        // «Código», «Planeamento», «Financiamento» ainda não existem como ecrã;
        // não se mostram como separador morto.
        _ => return None,
    })
}

/// Constrói os separadores: só os que têm destino real. Os que não têm ficam de
/// fora, em vez de aparecerem inertes.
fn tabs(labels: &[&'static str], workspace_id: &str) -> Vec<Tab> {
    labels
        .iter()
        .filter_map(|label| {
            tab_destination(label, workspace_id)
                .map(|href| Tab::link(*label, href, *label == "Visão geral"))
        })
        .collect()
}

/// O rótulo em português de um estado de ideia.
fn idea_state_label(code: &str) -> &'static str {
    match code {
        "discovery" => "Descoberta",
        "exploration" => "Exploração",
        "concept" => "Conceito",
        "review" => "Revisão",
        "project_candidate" => "Candidata a Projecto",
        "promoted" => "Promovida",
        "rejected" => "Rejeitada",
        "archived" => "Arquivada",
        _ => "Estado",
    }
}

/// O verbo do botão que move a ideia para um estado.
fn idea_transition_verb(code: &str) -> String {
    match code {
        "rejected" => "Rejeitar".to_owned(),
        "archived" => "Arquivar".to_owned(),
        "discovery" => "Reabrir".to_owned(),
        "project_candidate" => "Marcar como candidata a projecto".to_owned(),
        other => format!("Avançar para {}", idea_state_label(other)),
    }
}

/// Os controlos do ciclo de vida de uma ideia.
///
/// A lista de movimentos é do Core (`available_transitions`), não da interface:
/// o ecrã só oferece o que o domínio permite para esta ideia, agora. Fechar
/// (rejeitar/arquivar) pede a razão, que é memória institucional. «Promover a
/// Projecto» é a acção primária quando a ideia chega a candidata.
fn idea_lifecycle_actions(id: &str, idea: &Value, workspace: &Value) -> impl IntoView {
    let may_transition = workspace
        .get("may_transition")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let promotable = idea
        .get("promotable")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let transitions: Vec<(String, bool)> = idea
        .get("available_transitions")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|t| {
                    let state = t.get("state").and_then(Value::as_str)?.to_owned();
                    let requires_note = t
                        .get("requires_note")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    Some((state, requires_note))
                })
                .collect()
        })
        .unwrap_or_default();

    // Sem autoridade para transitar e sem promoção possível, não há nada a
    // mostrar — e um strip vazio seria ruído.
    if !may_transition && !promotable {
        return view! { <span hidden></span> }.into_any();
    }

    // A transição é sobre a **ideia** (`/ideas/{idea}/transition`); a promoção
    // pré-escolhe o **ambiente** no selector (`?workspace={ws}`). São dois ids.
    let idea_id = text(idea, "id");
    let action = format!("/ideas/{idea_id}/transition");
    let promote_href = format!("/projects/new?workspace={id}");

    view! {
        <div class="oc-lifecycle" role="group" aria-label="Ciclo de vida da ideia">
            <span class="oc-lifecycle__label">"Ciclo de vida"</span>

            {promotable.then(|| {
                button(Button::new("Promover a Projecto", Variant::Gold).href(promote_href.clone()))
            })}

            {may_transition.then(|| {
                transitions
                    .clone()
                    .into_iter()
                    .map(|(estado, requires_note)| {
                        let verbo = idea_transition_verb(&estado);
                        let accao = action.clone();
                        if requires_note {
                            view! {
                                <details class="oc-lifecycle__close">
                                    <summary class="oc-btn oc-btn--sm oc-btn--secondary">
                                        {verbo}
                                    </summary>
                                    <form
                                        method="post"
                                        action=accao
                                        class="oc-row oc-row--wrap oc-gap-3 oc-mt-3"
                                    >
                                        <input type="hidden" name="state" value=estado />
                                        <input
                                            class="oc-input"
                                            type="text"
                                            name="outcome_note"
                                            required
                                            minlength="3"
                                            placeholder="Razão (fica no registo)"
                                        />
                                        <button
                                            class="oc-btn oc-btn--sm oc-btn--danger"
                                            type="submit"
                                        >
                                            "Confirmar"
                                        </button>
                                    </form>
                                </details>
                            }
                            .into_any()
                        } else {
                            view! {
                                <form method="post" action=accao class="oc-lifecycle__step">
                                    <input type="hidden" name="state" value=estado />
                                    <button
                                        class="oc-btn oc-btn--sm oc-btn--secondary"
                                        type="submit"
                                    >
                                        {verbo}
                                    </button>
                                </form>
                            }
                            .into_any()
                        }
                    })
                    .collect_view()
            })}
        </div>
    }
    .into_any()
}

/// O Research Workspace — o detalhe partilhado de uma Ideia ou de um Projecto.
pub fn research_workspace(view: WorkspaceView) -> impl IntoView {
    let WorkspaceView {
        overview,
        sources,
        notes,
        documents,
        datasets,
        tasks,
        activity,
        inference_available,
        may_use_assistance,
        gestao,
    } = view;

    let workspace = overview.get("workspace").cloned().unwrap_or(Value::Null);
    let idea = overview.get("idea").cloned().unwrap_or(Value::Null);
    let project = overview.get("project").cloned().unwrap_or(Value::Null);
    let members = items(&overview.get("members").cloned().unwrap_or(Value::Null));

    let id = text(&workspace, "id");
    let code = text(&workspace, "code");
    let classification = text(&workspace, "classification");
    let is_project = !project.is_null();

    // O projecto, quando existe, dá o título e o código; caso contrário é a
    // ideia. Depois da promoção ambos existem, e a linhagem fica visível.
    let title = if is_project {
        text(&project, "title")
    } else {
        text(&idea, "title")
    };
    let state = if is_project {
        text(&project, "state")
    } else {
        text(&idea, "state")
    };
    let kind_label = if is_project { "PROJECTO" } else { "IDEIA" };
    let tab_labels: &[&'static str] = if is_project {
        &PROJECT_TABS
    } else {
        &IDEA_TABS
    };

    let unit_code = text(&workspace, "unit_code");
    let meta = format!("{code} · {unit_code}");

    view! {
        <div class="oc-band" >
            <div class="oc-row--top oc-gap-11 oc-mb-3" >
                <div class="oc-fill" >
                    <div class="oc-row oc-row--wrap oc-gap-6" >
                        {pill(kind_label)}
                        <h1 class="oc-t-screen" >
                            {title}
                        </h1>
                        {badge(state.clone(), Tone::of(&state))}
                        {classification_badge(&classification)}
                    </div>
                    <div class="oc-mono oc-mt-3" >{meta}</div>
                </div>

                <div class="oc-head__actions">
                    {button(
                        Button::new("IA neste workspace", Variant::Primary)
                            .href(format!("/ai/prompt?workspace={id}"))
                            .with_dot(),
                    )}
                </div>
            </div>

            // O ciclo de vida da ideia: avançar de estado, marcá-la candidata,
            // promovê-la, ou fechá-la — só os movimentos que o Core devolveu como
            // legais, e só a quem os pode fazer. Era isto que faltava para uma
            // ideia poder chegar a projecto pelo produto (F-10).
            {(!is_project).then(|| idea_lifecycle_actions(&id, &idea, &workspace))}

            {context_tabs(tabs(tab_labels, &id), "Secções do Research Workspace")}
        </div>

        <div class="oc-page oc-page" id="ws-visao-geral" >
            <div class="oc-grid oc-grid--ws">
                {if is_project {
                    project_overview(&project, &members).into_any()
                } else {
                    idea_overview(&idea, &sources, &datasets).into_any()
                }}

                <div id="ws-membros">{pessoas_do_ambiente(&id, &members, &gestao)}</div>

                {assist(Assist {
                    here: if is_project { "este Projecto" } else { "esta Ideia" },
                    workspace_id: Some(id.clone()),
                    resource: if is_project {
                        Some(("project", text(&project, "id")))
                    } else {
                        Some(("idea", text(&idea, "id")))
                    },
                    suggestions: if is_project {
                        PROJECT_SUGGESTIONS
                    } else {
                        IDEA_SUGGESTIONS
                    },
                    inference_available,
                    may_use: may_use_assistance,
                })}

                <section class="oc-card" id="ws-actividade">
                    {section_head("Actividade recente", None, None)}
                    <div class="oc-card__body">{activity_list(&activity)}</div>
                </section>

                <section class="oc-card" id="ws-tarefas">
                    {section_head("Tarefas", None, None)}
                    <div class="oc-card__body">{task_list(&tasks)}</div>
                </section>
            </div>

            <div class="oc-grid oc-grid--detail oc-mt-7" >
                <div id="ws-bibliografia">
                    {artefact_card("Bibliografia", &sources, "title", "/bibliography")}
                </div>
                <div id="ws-notas">
                    {artefact_card("Notas", &notes, "title", "/knowledge")}
                </div>
            </div>

            <div class="oc-grid oc-grid--detail oc-mt-7" >
                <div id="ws-documentos">
                    {artefact_card("Documentos", &documents, "title", "/knowledge")}
                </div>
                <div id="ws-datasets">
                    {artefact_card("Datasets", &datasets, "title", "/datasets")}
                </div>
            </div>
        </div>
    }
}

fn idea_overview(idea: &Value, sources: &Value, datasets: &Value) -> impl IntoView {
    let keywords = idea
        .get("keywords")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let source_count = items(sources).len();
    let dataset_count = items(datasets).len();

    view! {
        <section class="oc-card">
            {section_head("Descrição", None, None)}
            <div class="oc-card__body">
                <p class="oc-t-body" >
                    {text(idea, "summary")}
                </p>

                <h3 class="oc-t-group oc-mt-9 oc-mb-3" >
                    "PALAVRAS-CHAVE"
                </h3>
                {if keywords.is_empty() {
                    view! { <span class="oc-muted" >"—"</span> }.into_any()
                } else {
                    view! {
                        <div class="oc-row oc-row--wrap oc-gap-3" >
                            {keywords
                                .iter()
                                .filter_map(Value::as_str)
                                .map(|word| {
                                    view! {
                                        <span class="oc-tag" >
                                            {word.to_owned()}
                                        </span>
                                    }
                                })
                                .collect_view()}
                        </div>
                    }
                        .into_any()
                }}

                <div class="oc-split oc-split--3" >
                    {metric("Referências", source_count)}
                    {metric("Datasets", dataset_count)}
                    {metric("Experiências", 0)}
                </div>
            </div>
        </section>
    }
}

fn project_overview(project: &Value, members: &[Value]) -> impl IntoView {
    let progress = project
        .get("progress")
        .and_then(Value::as_i64)
        .and_then(|p| u8::try_from(p).ok())
        .unwrap_or(0);
    let state = text(project, "state");
    let from_idea = project
        .get("origin_idea_id")
        .and_then(Value::as_str)
        .is_some();
    let members = members.to_vec();

    view! {
        <section class="oc-card">
            {section_head("Descrição", None, None)}
            <div class="oc-card__body">
                <p class="oc-t-body" >
                    {text(project, "summary")}
                </p>

                <h3 class="oc-t-group oc-mt-9 oc-mb-3" >
                    "OBJECTIVOS"
                </h3>
                <p class="oc-t-body" >
                    {text(project, "objectives")}
                </p>

                <div class="oc-row oc-gap-11 oc-mt-10" >
                    {donut(progress)}
                    <div>
                        <div class="oc-row oc-gap-4" >
                            {badge(state.clone(), Tone::of(&state))}
                        </div>
                        {from_idea
                            .then(|| {
                                view! {
                                    <p class="oc-t-caption--muted oc-mt-4" >
                                        "Este projecto teve origem numa ideia desta unidade. A
                                         linhagem está preservada e não é reescrita."
                                    </p>
                                }
                            })}
                    </div>
                </div>

                {(!members.is_empty())
                    .then(|| {
                        view! {
                            <h3 class="oc-t-group oc-mt-10 oc-mb-3" >
                                "EQUIPA"
                            </h3>
                            <div class="oc-col oc-gap-5" >
                                {members
                                    .iter()
                                    .map(|member| {
                                        let name = text(member, "full_name");
                                        let role = text(member, "role");
                                        view! {
                                            <div class="oc-row oc-gap-6" >
                                                <span class="oc-avatar oc-avatar--sm" >
                                                    {crate::ui::initials(&name)}
                                                </span>
                                                <span class="oc-fill oc-t-cell-2" >
                                                    {name}
                                                </span>
                                                {badge(role.clone(), Tone::of(&role))}
                                            </div>
                                        }
                                    })
                                    .collect_view()}
                            </div>
                        }
                    })}
            </div>
        </section>
    }
}

fn metric(label: &'static str, value: usize) -> impl IntoView {
    view! {
        <div class="oc-split__cell" >
            <div class="oc-t-screen" >
                {value.to_string()}
            </div>
            <div class="oc-t-hint oc-mt-1" >
                {label}
            </div>
        </div>
    }
}

/// Como [`metric`], mas para um valor textual (estado, responsável, prazo).
fn metric_text(label: &'static str, value: &str) -> impl IntoView {
    let value = value.to_owned();
    view! {
        <div class="oc-split__cell" >
            <div class="oc-t-cell-2" >{value}</div>
            <div class="oc-t-hint oc-mt-1" >{label}</div>
        </div>
    }
}

fn activity_list(payload: &Value) -> AnyView {
    let rows = items(payload);
    if rows.is_empty() {
        return view! { <p class="oc-muted">"Sem actividade."</p> }.into_any();
    }

    view! {
        <div class="oc-col oc-gap-8" >
            {rows
                .iter()
                .take(10)
                .map(|row| {
                    view! {
                        <div>
                            <div class="oc-t-note" >
                                {text(row, "summary")}
                            </div>
                            <div class="oc-mono oc-t-ghost" >
                                {text(row, "actor_name")}
                            </div>
                        </div>
                    }
                })
                .collect_view()}
        </div>
    }
    .into_any()
}

fn task_list(payload: &Value) -> AnyView {
    let rows = items(payload);
    if rows.is_empty() {
        return view! { <p class="oc-muted">"Sem tarefas."</p> }.into_any();
    }

    view! {
        <div class="oc-col oc-gap-9" >
            {rows
                .iter()
                .take(8)
                .map(|row| {
                    let state = text(row, "state");
                    // Uma tarefa fechada conta como concluída; as restantes
                    // ainda não têm percentagem no Core, e mostrar uma
                    // inventada seria pior do que mostrar zero.
                    let pct = if state == "done" { 100 } else { 0 };
                    // A tarefa abre no seu detalhe, onde muda de estado e ganha
                    // responsável — deixou de ser uma linha só de leitura (F-14).
                    let href = format!("/tasks/{}", text(row, "id"));
                    view! {
                        <a class="oc-task-row" href=href>
                            <div class="oc-row oc-gap-5 oc-mb-1" >
                                <span class="oc-fill oc-truncate oc-t-cell-2" >
                                    {text(row, "title")}
                                </span>
                                {badge(state.clone(), Tone::of(&state))}
                            </div>
                            {progress_bar(pct)}
                        </a>
                    }
                })
                .collect_view()}
        </div>
    }
    .into_any()
}

/// O rótulo em português de um estado de tarefa.
fn task_state_label(code: &str) -> &'static str {
    match code {
        "todo" => "A fazer",
        "in_progress" => "Em curso",
        "blocked" => "Bloqueada",
        "in_review" => "Em revisão",
        "done" => "Concluída",
        "cancelled" => "Cancelada",
        _ => "Estado",
    }
}

/// O detalhe de uma tarefa: o que é, e as acções sobre ela.
///
/// Uma tarefa deixou de ser uma linha só de leitura: aqui muda de estado (pelos
/// movimentos que o Core devolveu como legais) e ganha ou perde responsável. A
/// autoridade real é do Core; os controlos só aparecem a quem escreve no
/// ambiente.
pub fn task_detail(
    task: &Value,
    overview: &Value,
    ok: Option<&str>,
    erro: Option<&str>,
) -> AnyView {
    let task_id = text(task, "id");
    let state = text(task, "state");
    let priority = text(task, "priority");
    let workspace = overview.get("workspace").cloned().unwrap_or(Value::Null);
    let workspace_id = text(task, "workspace_id");
    let may_act = workspace
        .get("may_create")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let members = items(&overview.get("members").cloned().unwrap_or(Value::Null));

    let assignee_id = task
        .get("assignee_id")
        .and_then(Value::as_str)
        .unwrap_or("");
    let assignee_name = members
        .iter()
        .find(|m| m.get("person_id").and_then(Value::as_str) == Some(assignee_id))
        .map_or_else(|| "Sem responsável".to_owned(), |m| text(m, "full_name"));

    let transitions: Vec<String> = task
        .get("available_transitions")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|t| t.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();

    let transition_action = format!("/tasks/{task_id}/transition");
    let assign_action = format!("/tasks/{task_id}/assignee");

    view! {
        <div class="oc-band">
            <div class="oc-row oc-row--wrap oc-gap-6 oc-mb-2">
                {pill("TAREFA")}
                <h1 class="oc-t-screen">{text(task, "title")}</h1>
                {badge(task_state_label(&state).to_owned(), Tone::of(&state))}
                {badge(priority.clone(), Tone::of(&priority))}
            </div>
            <div class="oc-mono oc-mb-5">
                <a href=format!("/workspaces/{workspace_id}")>"← Voltar ao ambiente"</a>
            </div>
        </div>

        <div class="oc-page">
            {ok.filter(|s| !s.is_empty()).map(|m| view! {
                <div class="oc-card oc-note" role="status">{m.to_owned()}</div>
            })}
            {erro.filter(|s| !s.is_empty()).map(|m| view! {
                <div class="oc-card oc-alert" role="alert">{m.to_owned()}</div>
            })}

            <div class="oc-grid oc-grid--detail">
                <section class="oc-card">
                    {section_head("Sobre a tarefa", None, None)}
                    <div class="oc-card__body">
                        <p class="oc-t-body">{text(task, "description")}</p>
                        <div class="oc-split oc-split--2 oc-mt-5">
                            {metric_text("Estado", task_state_label(&state))}
                            {metric_text("Prioridade", &priority)}
                            {metric_text("Prazo", &text(task, "due_on"))}
                            {metric_text("Responsável", &assignee_name)}
                        </div>
                    </div>
                </section>

                {may_act.then(|| view! {
                    <section class="oc-card">
                        {section_head("Acções", None, None)}
                        <div class="oc-card__body">
                            <div class="oc-field__label">"Mudar estado"</div>
                            <div class="oc-lifecycle">
                                {if transitions.is_empty() {
                                    view! {
                                        <span class="oc-muted">
                                            "Esta tarefa não tem mais movimentos."
                                        </span>
                                    }.into_any()
                                } else {
                                    transitions.clone().into_iter().map(|estado| {
                                        let accao = transition_action.clone();
                                        let rotulo = format!("Marcar «{}»", task_state_label(&estado));
                                        view! {
                                            <form method="post" action=accao class="oc-lifecycle__step">
                                                <input type="hidden" name="state" value=estado />
                                                <button class="oc-btn oc-btn--sm oc-btn--secondary" type="submit">
                                                    {rotulo}
                                                </button>
                                            </form>
                                        }
                                    }).collect_view().into_any()
                                }}
                            </div>

                            <div class="oc-field__label oc-mt-6">"Responsável"</div>
                            <form method="post" action=assign_action class="oc-row oc-row--wrap oc-gap-3">
                                <select class="oc-select" name="assignee_id">
                                    <option value="">"Sem responsável"</option>
                                    {members.iter().map(|m| {
                                        let pid = text(m, "person_id");
                                        let nome = text(m, "full_name");
                                        let escolhido = pid == assignee_id;
                                        view! { <option value=pid selected=escolhido>{nome}</option> }
                                    }).collect_view()}
                                </select>
                                <button class="oc-btn oc-btn--sm oc-btn--primary" type="submit">
                                    "Atribuir"
                                </button>
                            </form>
                        </div>
                    </section>
                })}
            </div>
        </div>
    }
    .into_any()
}

/// Bytes em unidade legível — o material de uma versão de dataset tem tamanho,
/// e mostrá-lo em bytes crus não informa ninguém.
fn human_bytes(bytes: i64) -> String {
    const UNITS: [&str; 5] = ["B", "kB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// O detalhe de um dataset: a sua governança e as suas versões.
///
/// Um dataset é uma entidade de domínio (§14), não um upload: tem código,
/// classificação, origem, licença e restrições de uso, e uma história de versões
/// material. A autoridade é do Core — abrir por identificador reautoriza pela
/// posse e pela classificação do próprio dataset (F-13).
pub fn dataset_detail(dataset: &Value, versions: &Value) -> AnyView {
    let workspace_id = text(dataset, "workspace_id");
    let classification = text(dataset, "classification");
    let state = text(dataset, "state");
    let keywords = dataset
        .get("keywords")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|k| k.as_str())
                .collect::<Vec<_>>()
                .join(" · ")
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "—".to_owned());
    let versoes = items(versions);

    view! {
        <div class="oc-band">
            <div class="oc-row oc-row--wrap oc-gap-6 oc-mb-2">
                {pill("DATASET")}
                <h1 class="oc-t-screen">{text(dataset, "title")}</h1>
                {classification_badge(&classification)}
                {badge(state.clone(), Tone::of(&state))}
            </div>
            <div class="oc-mono oc-mb-5">
                <a href=format!("/workspaces/{workspace_id}")>"← Voltar ao ambiente"</a>
            </div>
        </div>

        <div class="oc-page">
            <div class="oc-grid oc-grid--detail">
                <section class="oc-card">
                    {section_head("Sobre o dataset", None, None)}
                    <div class="oc-card__body">
                        <p class="oc-t-body">{text(dataset, "description")}</p>
                        <div class="oc-split oc-split--2 oc-mt-5">
                            {metric_text("Código", &text(dataset, "code"))}
                            {metric_text("Classificação", &classification)}
                            {metric_text("Estado", &state)}
                            {metric_text("Origem", &text(dataset, "origin"))}
                            {metric_text("Licença", &text(dataset, "licence"))}
                            {metric_text("Restrições de uso", &text(dataset, "usage_restrictions"))}
                        </div>
                        <div class="oc-field__label oc-mt-6">"Palavras-chave"</div>
                        <p class="oc-t-note">{keywords}</p>
                    </div>
                </section>

                <section class="oc-card">
                    {section_head("Versões", None, None)}
                    <div class="oc-card__body">
                        {if versoes.is_empty() {
                            view! {
                                <p class="oc-muted">
                                    "Este dataset ainda não tem versões. Uma versão \
                                     agrupa os ficheiros materiais de um estado do dataset."
                                </p>
                            }.into_any()
                        } else {
                            view! {
                                <div class="oc-col oc-gap-9">
                                    {versoes.iter().map(|v| {
                                        let status = text(v, "status");
                                        let ficheiros = v.get("file_count")
                                            .and_then(Value::as_i64).unwrap_or(0);
                                        let tamanho = human_bytes(
                                            v.get("total_size_bytes")
                                                .and_then(Value::as_i64).unwrap_or(0),
                                        );
                                        let publicada = text(v, "published_at");
                                        view! {
                                            <div>
                                                <div class="oc-row oc-gap-5 oc-mb-1">
                                                    <span class="oc-fill oc-t-cell-2">
                                                        {text(v, "label")}
                                                    </span>
                                                    {badge(status.clone(), Tone::of(&status))}
                                                </div>
                                                <div class="oc-mono oc-t-ghost">
                                                    {format!("{ficheiros} ficheiro(s) · {tamanho}")}
                                                    {(!publicada.is_empty()).then(
                                                        || format!(" · publicada {publicada}"))}
                                                </div>
                                                {(!text(v, "provenance").is_empty()).then(|| view! {
                                                    <div class="oc-t-note">{text(v, "provenance")}</div>
                                                })}
                                            </div>
                                        }
                                    }).collect_view()}
                                </div>
                            }.into_any()
                        }}
                    </div>
                </section>
            </div>
        </div>
    }
    .into_any()
}

fn artefact_card(
    title: &'static str,
    payload: &Value,
    key: &'static str,
    href: &'static str,
) -> impl IntoView {
    let rows = items(payload);
    let count = rows.len();

    view! {
        <section class="oc-card">
            <div class="oc-card__head">
                <h2>{title}</h2>
                <span class="oc-card__meta">{count.to_string()}</span>
            </div>
            <div class="oc-card__body">
                {if rows.is_empty() {
                    view! { <p class="oc-muted">"Sem registos."</p> }.into_any()
                } else {
                    view! {
                        <div>
                            {rows
                                .iter()
                                .take(6)
                                .map(|row| {
                                    view! {
                                        <div class="oc-list__row" >
                                            <span class="oc-fill oc-truncate oc-t-cell-2" >
                                                {text(row, key)}
                                            </span>
                                            {classification_badge(&text(row, "classification"))}
                                        </div>
                                    }
                                })
                                .collect_view()}
                            <a class="oc-card__action oc-mt-5 oc-inline-block" href=href>
                                "Ver tudo"
                            </a>
                        </div>
                    }
                        .into_any()
                }}
            </div>
        </section>
    }
}

// ── Detalhe da Unidade ───────────────────────────────────────────────────

/// As 9 tabs de uma unidade.
const UNIT_TABS: [&str; 9] = [
    "Visão geral",
    "Membros",
    "Ideias",
    "Projectos",
    "Bibliografia",
    "Dados",
    "Documentos",
    "Actividade",
    "Configuração",
];

/// Detalhe de uma unidade.
/// A gestão de pessoas de um contentor de autoridade.
///
/// # Porque isto é uma funcionalidade de segurança
///
/// Porque uma pertença **é** autoridade. Acrescentar alguém a uma unidade
/// concede-lhe direitos sobre o que lá está; retirá-lo tira-lhos. Não é um CRUD
/// secundário, e a interface que o faz não é um formulário improvisado.
///
/// `pode_gerir` vem do Core e não de um palpite sobre o papel: se o controlo
/// aparece, a operação é autorizável pela mesma política que a vai executar.
pub struct GestaoDePessoas {
    /// Se quem está a ver pode alterar quem pertence.
    pub pode_gerir: bool,
    /// Pessoas da organização que ainda não pertencem, para escolher.
    pub candidatos: Vec<(String, String)>,
    /// Uma mensagem da operação anterior.
    pub aviso: Option<(bool, String)>,
}

pub fn unit_detail(
    unit: &Value,
    members: &Value,
    workspaces: &Value,
    gestao: &GestaoDePessoas,
) -> impl IntoView {
    let id = text(unit, "id");
    let status = text(unit, "status");
    let member_rows = items(members);
    let workspace_rows = items(workspaces);

    let ideas = workspace_rows
        .iter()
        .filter(|w| text(w, "kind") == "idea")
        .count();
    let projects = workspace_rows
        .iter()
        .filter(|w| text(w, "kind") == "project")
        .count();

    let unit_tabs: Vec<Tab> = UNIT_TABS
        .iter()
        .enumerate()
        .map(|(i, label)| match *label {
            "Visão geral" => Tab::link(*label, format!("/units/{id}"), i == 0),
            "Ideias" => Tab::link(*label, "/ideas", false),
            "Projectos" => Tab::link(*label, "/projects", false),
            "Bibliografia" => Tab::link(*label, "/bibliography", false),
            "Dados" => Tab::link(*label, "/datasets", false),
            other => Tab::inert(other),
        })
        .collect();

    let research_areas: Vec<String> = unit
        .get("research_areas")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default();

    view! {
        <div class="oc-band" >
            <div class="oc-row oc-row--wrap oc-gap-6 oc-mb-2" >
                <h1 class="oc-t-screen" >
                    {text(unit, "name")}
                </h1>
                {pill(text(unit, "code"))}
                {badge(status.clone(), Tone::of(&status))}
                // Editar só a quem pode gerir a unidade. A ausência não é a
                // defesa: o Core recusa o mesmo PUT a quem o tente directamente.
                {gestao.pode_gerir.then(|| {
                    let href = format!("/units/{id}/edit");
                    view! {
                        <span class="oc-row__spacer" ></span>
                        {button(Button::new("Editar", Variant::Secondary).href(href))}
                    }
                })}
            </div>
            <div class="oc-mono oc-mb-5" >
                {format!("{} membros · {ideas} ideias · {projects} projectos", member_rows.len())}
            </div>
            {context_tabs(unit_tabs, "Secções da unidade")}
        </div>

        <div class="oc-page oc-page" >
            <div class="oc-grid oc-grid--detail">
                <section class="oc-card">
                    {section_head("Sobre a unidade", None, None)}
                    <div class="oc-card__body">
                        <p class="oc-t-body" >
                            {text(unit, "description")}
                        </p>
                        {(!research_areas.is_empty()).then(|| {
                            view! {
                                <div class="oc-field__label oc-mt-5" >"Áreas de investigação"</div>
                                <div class="oc-chips oc-chips--static" >
                                    {research_areas
                                        .iter()
                                        .map(|area| view! {
                                            <span class="oc-chip" >{area.clone()}</span>
                                        })
                                        .collect_view()}
                                </div>
                            }
                        })}
                        <div class="oc-split oc-split--2 oc-mt-5" >
                            {metric("Membros", member_rows.len())}
                            {metric("Ideias", ideas)}
                            {metric("Projectos", projects)}
                            {metric("Áreas", research_areas.len())}
                        </div>
                    </div>
                </section>

                <section class="oc-card">
                    <div class="oc-card__head">
                        <h2>"Membros"</h2>
                        <span class="oc-card__meta">{member_rows.len().to_string()}</span>
                    </div>
                    <div class="oc-card__body">
                        {if member_rows.is_empty() {
                            view! { <p class="oc-muted">"Sem membros."</p> }.into_any()
                        } else {
                            view! {
                                <div class="oc-col oc-gap-6" >
                                    {member_rows
                                        .iter()
                                        .map(|member| {
                                            let name = text(member, "full_name");
                                            let role = text(member, "role");
                                            let email = text(member, "email");
                                            let person_id = text(member, "person_id");
                                            let unidade = id.clone();
                                            let pode = gestao.pode_gerir;
                                            view! {
                                                <div class="oc-pessoa" >
                                                    <span class="oc-avatar oc-avatar--sm" >
                                                        {crate::ui::initials(&name)}
                                                    </span>
                                                    <span class="oc-pessoa__quem" >
                                                        <span class="oc-t-cell-2" >{name}</span>
                                                        <span class="oc-t-caption--muted" >
                                                            {email}
                                                        </span>
                                                    </span>
                                                    {badge(role.clone(), Tone::of(&role))}
                                                    {pode
                                                        .then(|| gerir_pessoa(
                                                            &unidade, &person_id, &role,
                                                        ))}
                                                </div>
                                            }
                                        })
                                        .collect_view()}
                                </div>
                            }
                                .into_any()
                        }}
                        {gestao.aviso.as_ref().map(|(ok, m)| aviso_de_gestao(*ok, m))}
                        {gestao
                            .pode_gerir
                            .then(|| acrescentar_pessoa(&id, &gestao.candidatos))}
                    </div>
                </section>
            </div>
        </div>
    }
}

/// Os controlos que alteram a autoridade de uma pessoa numa unidade.
///
/// Só aparecem a quem pode geri-la — e a ausência deles não é a defesa: o Core
/// recusa a mesma operação a quem a tente por HTTP directo.
fn gerir_pessoa(unit_id: &str, person_id: &str, role: &str) -> impl IntoView {
    let promover = role != "manager";
    let novo = if promover { "manager" } else { "member" };
    let rotulo = if promover {
        "Tornar gestor"
    } else {
        "Tornar membro"
    };

    view! {
        <span class="oc-pessoa__accoes">
            <form method="post" action=format!("/units/{unit_id}/members/role")>
                <input type="hidden" name="person_id" value=person_id.to_owned() />
                <input type="hidden" name="role" value=novo />
                <button class="oc-btn oc-btn--ghost" type="submit">{rotulo}</button>
            </form>
            <form method="post" action=format!("/units/{unit_id}/members/remove")>
                <input type="hidden" name="person_id" value=person_id.to_owned() />
                <button class="oc-btn oc-btn--ghost" type="submit">"Remover"</button>
            </form>
        </span>
    }
}

/// Acrescentar alguém da organização à unidade.
///
/// A lista é de pessoas reais que ainda não pertencem, e os papéis são os dois
/// que a unidade tem — não uma lista maior que o Core depois recusaria.
fn acrescentar_pessoa(unit_id: &str, candidatos: &[(String, String)]) -> impl IntoView {
    if candidatos.is_empty() {
        return view! {
            <p class="oc-t-caption--muted oc-mt-5">
                "Todas as pessoas da organização já pertencem a esta unidade."
            </p>
        }
        .into_any();
    }

    let opcoes = candidatos
        .iter()
        .map(|(pid, etiqueta)| {
            view! { <option value=pid.clone()>{etiqueta.clone()}</option> }
        })
        .collect_view();

    view! {
        <form
            class="oc-pessoa__acrescentar oc-mt-5"
            method="post"
            action=format!("/units/{unit_id}/members")
        >
            <label class="oc-sr" for="oc-unit-person">"Pessoa"</label>
            <select class="oc-select" id="oc-unit-person" name="person_id" required>
                {opcoes}
            </select>
            <label class="oc-sr" for="oc-unit-role">"Papel"</label>
            <select class="oc-select" id="oc-unit-role" name="role">
                <option value="member">"Membro"</option>
                <option value="manager">"Gestor"</option>
            </select>
            <button class="oc-btn oc-btn--primary" type="submit">"Adicionar"</button>
        </form>
    }
    .into_any()
}

/// Quem pertence ao Research Workspace, e os controlos de quem o lidera.
///
/// # Porque isto é uma secção do ambiente e não da unidade
///
/// A pertença a uma unidade e a pertença a um ambiente são duas autoridades
/// diferentes. Gerir o ambiente a partir do ecrã da unidade obrigaria quem
/// lidera uma ideia a ter também autoridade sobre a unidade inteira — que é
/// mais do que liderar uma ideia exige, e mais do que o Core concede.
///
/// Os controlos só aparecem a quem pode alterar. A ausência deles **não é a
/// defesa**: o Core recusa a mesma operação a quem a tente por HTTP directo, e
/// há uma viagem que o exige.
fn pessoas_do_ambiente(
    workspace_id: &str,
    membros: &[Value],
    gestao: &GestaoDePessoas,
) -> impl IntoView {
    let id = workspace_id.to_owned();
    let linhas = membros.to_vec();

    view! {
        <section class="oc-card">
            <div class="oc-card__head">
                <h2>"Pessoas"</h2>
                <span class="oc-card__meta">{linhas.len().to_string()}</span>
            </div>
            <div class="oc-card__body">
                {if linhas.is_empty() {
                    view! { <p class="oc-muted">"Sem pessoas."</p> }.into_any()
                } else {
                    view! {
                        <div class="oc-col oc-gap-6">
                            {linhas
                                .iter()
                                .map(|membro| {
                                    let nome = text(membro, "full_name");
                                    let papel = text(membro, "role");
                                    let person_id = text(membro, "person_id");
                                    let ambiente = id.clone();
                                    let pode = gestao.pode_gerir;
                                    view! {
                                        <div class="oc-pessoa">
                                            <span class="oc-avatar oc-avatar--sm">
                                                {crate::ui::initials(&nome)}
                                            </span>
                                            <span class="oc-pessoa__quem">
                                                <span class="oc-t-cell-2">{nome}</span>
                                            </span>
                                            {badge(papel.clone(), Tone::of(&papel))}
                                            {pode
                                                .then(|| remover_do_ambiente(&ambiente, &person_id))}
                                        </div>
                                    }
                                })
                                .collect_view()}
                        </div>
                    }
                        .into_any()
                }}
                {gestao.aviso.as_ref().map(|(ok, m)| aviso_de_gestao(*ok, m))}
                {gestao
                    .pode_gerir
                    .then(|| acrescentar_ao_ambiente(&id, &gestao.candidatos))}
            </div>
        </section>
    }
}

/// Retirar alguém do ambiente.
///
/// Não há aqui promoção nem despromoção: acrescentar com outro papel é a mesma
/// operação, e o Core faz upsert. Um segundo caminho de escrita seria uma
/// segunda autoridade com outro nome.
fn remover_do_ambiente(workspace_id: &str, person_id: &str) -> impl IntoView {
    view! {
        <span class="oc-pessoa__accoes">
            <form method="post" action=format!("/workspaces/{workspace_id}/members/remove")>
                <input type="hidden" name="person_id" value=person_id.to_owned() />
                <button class="oc-btn oc-btn--ghost" type="submit">"Remover"</button>
            </form>
        </span>
    }
}

/// Acrescentar alguém ao ambiente.
///
/// Os papéis são os três que um Research Workspace tem — não uma lista maior
/// que o Core depois recusaria, nem uma menor que escondesse autoridade que
/// existe.
fn acrescentar_ao_ambiente(workspace_id: &str, candidatos: &[(String, String)]) -> impl IntoView {
    if candidatos.is_empty() {
        return view! {
            <p class="oc-t-caption--muted oc-mt-5">
                "Todas as pessoas da organização já participam neste ambiente."
            </p>
        }
        .into_any();
    }

    let opcoes = candidatos
        .iter()
        .map(|(pid, etiqueta)| {
            view! { <option value=pid.clone()>{etiqueta.clone()}</option> }
        })
        .collect_view();

    view! {
        <form
            class="oc-pessoa__acrescentar oc-mt-5"
            method="post"
            action=format!("/workspaces/{workspace_id}/members")
        >
            <label class="oc-sr" for="oc-ws-person">"Pessoa"</label>
            <select class="oc-select" id="oc-ws-person" name="person_id" required>
                {opcoes}
            </select>
            <label class="oc-sr" for="oc-ws-role">"Papel"</label>
            <select class="oc-select" id="oc-ws-role" name="role">
                <option value="member">"Membro"</option>
                <option value="lead">"Líder"</option>
                <option value="viewer">"Observador"</option>
            </select>
            <button class="oc-btn oc-btn--primary" type="submit">"Adicionar"</button>
        </form>
    }
    .into_any()
}

fn aviso_de_gestao(ok: bool, mensagem: &str) -> impl IntoView {
    let classe = if ok {
        "oc-note oc-note--ok oc-mt-5"
    } else {
        "oc-note oc-note--bad oc-mt-5"
    };
    view! { <p class=classe role="status">{mensagem.to_owned()}</p> }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use serde_json::json;

    /// Uma gestão de prova **com** controlos.
    ///
    /// A varredura de ligações mortas percorre o HTML que estes ecrãs produzem.
    /// Com `pode_gerir: false` os formulários novos não seriam renderizados, e
    /// a varredura passaria a afirmar que caminhos que nunca viu estão bons.
    pub(crate) fn gestao_de_prova() -> GestaoDePessoas {
        GestaoDePessoas {
            pode_gerir: true,
            candidatos: vec![(
                "11111111-1111-4111-8111-111111111111".to_owned(),
                "Alguém · alguem@ocinye.com".to_owned(),
            )],
            aviso: None,
        }
    }

    #[test]
    fn uma_ideia_e_um_projecto_tem_treze_tabs_cada() {
        assert_eq!(IDEA_TABS.len(), 13);
        assert_eq!(PROJECT_TABS.len(), 13);
    }

    #[test]
    fn a_tab_de_ia_abre_o_prompt_vinculado_ao_workspace() {
        let built = tabs(&IDEA_TABS, "abc-123");
        let ai = built
            .iter()
            .find(|t| t.label == "IA")
            .expect("a tab IA existe");
        assert_eq!(ai.href.as_deref(), Some("/ai/prompt?workspace=abc-123"));
    }

    /// Nenhum separador do ambiente é inerte: cada um ou salta para uma secção
    /// deste ecrã (âncora), ou leva a outro ecrã. Os que não têm destino ficam
    /// fora, em vez de aparecerem mortos (F-11).
    #[test]
    fn nenhum_separador_do_ambiente_e_morto() {
        let built = tabs(&IDEA_TABS, "abc-123");
        // Todos os separadores construídos navegam.
        assert!(
            built.iter().all(|t| t.href.is_some()),
            "um separador ficou sem destino"
        );
        // As secções deste ecrã são âncoras.
        for (label, ancora) in [
            ("Notas", "#ws-notas"),
            ("Documentos", "#ws-documentos"),
            ("Datasets", "#ws-datasets"),
            ("Tarefas", "#ws-tarefas"),
            ("Actividade", "#ws-actividade"),
            ("Bibliografia", "#ws-bibliografia"),
        ] {
            let tab = built
                .iter()
                .find(|t| t.label == label)
                .unwrap_or_else(|| panic!("falta o separador {label}"));
            assert_eq!(tab.href.as_deref(), Some(ancora));
        }
        // «Código» ainda não existe: não aparece como separador.
        assert!(
            built.iter().all(|t| t.label != "Código"),
            "«Código» não tem ecrã e não devia ser um separador"
        );

        // E o ecrã não renderiza nenhum separador «Ainda não disponível».
        let html = idea_ws("exploration", json!([]), false, false);
        assert!(!html.contains("Ainda não disponível"));
        // As secções âncora existem no corpo.
        for id in ["ws-notas", "ws-tarefas", "ws-actividade", "ws-datasets"] {
            assert!(
                html.contains(&format!("id=\"{id}\"")),
                "falta a secção {id}"
            );
        }
    }

    /// O detalhe de uma tarefa oferece os movimentos legais e a atribuição —
    /// e nada disso a quem não escreve no ambiente (F-14).
    #[test]
    fn o_detalhe_de_uma_tarefa_permite_mudar_estado_e_atribuir() {
        let task = json!({
            "id": "33333333-3333-4333-8333-333333333333",
            "workspace_id": "w1",
            "title": "Calibrar",
            "description": "Detalhe",
            "state": "todo",
            "priority": "normal",
            "assignee_id": null,
            "available_transitions": ["in_progress", "blocked", "cancelled"],
        });
        let overview = json!({
            "workspace": {"id": "w1", "may_create": true},
            "members": [{"person_id": "p1", "full_name": "Ana"}]
        });
        let html = task_detail(&task, &overview, None, None).to_html();
        assert!(html.contains("A fazer"), "falta o rótulo do estado");
        assert!(
            html.contains("Marcar «Em curso»"),
            "falta o botão de transição"
        );
        assert!(html.contains(r#"action="/tasks/33333333-3333-4333-8333-333333333333/transition""#));
        assert!(html.contains(r#"action="/tasks/33333333-3333-4333-8333-333333333333/assignee""#));
        assert!(html.contains("Ana"), "falta o candidato a responsável");
        assert!(html.contains("Sem responsável"));

        // Sem autoridade para escrever, os controlos não aparecem.
        let so_leitura = json!({"workspace": {"id": "w1", "may_create": false}, "members": []});
        let read = task_detail(&task, &so_leitura, None, None).to_html();
        assert!(!read.contains("Marcar «Em curso»"));
        assert!(!read.contains("Atribuir"));
    }

    /// O detalhe de um dataset mostra a sua governança e as suas versões, e
    /// liga de volta ao ambiente — deixou de ser uma linha morta na lista (F-13).
    #[test]
    fn o_detalhe_de_um_dataset_mostra_governanca_e_versoes() {
        let dataset = json!({
            "id": "44444444-4444-4444-8444-444444444444",
            "workspace_id": "w7",
            "code": "DS-001",
            "title": "Leituras de campo",
            "description": "Séries temporais.",
            "origin": "measured",
            "licence": "CC-BY-4.0",
            "usage_restrictions": "Uso interno",
            "keywords": ["clima", "sensor"],
            "classification": "INTERNAL",
            "state": "active",
        });
        let versions = json!([
            {"id": "v1", "label": "v1", "status": "published", "provenance": "Recolha inicial",
             "file_count": 3, "total_size_bytes": 2048, "published_at": "2026-09-01"}
        ]);
        let html = dataset_detail(&dataset, &versions).to_html();
        assert!(html.contains("DS-001"), "falta o código");
        assert!(html.contains("CC-BY-4.0"), "falta a licença");
        assert!(html.contains("clima · sensor"), "faltam as palavras-chave");
        assert!(html.contains("Versões"), "falta a secção de versões");
        assert!(
            html.contains("3 ficheiro(s) · 2.0 kB"),
            "falta o material da versão"
        );
        assert!(html.contains("Recolha inicial"), "falta a proveniência");
        assert!(
            html.contains(r#"href="/workspaces/w7""#),
            "falta a ligação de volta ao ambiente"
        );

        // Sem versões, o dataset explica o que uma versão é em vez de ficar vazio.
        let vazio = dataset_detail(&dataset, &json!([])).to_html();
        assert!(vazio.contains("ainda não tem versões"));
    }

    /// Helper: render an idea workspace with a given state, transitions and
    /// promotable/may_transition flags.
    fn idea_ws(state: &str, transitions: Value, promotable: bool, may_transition: bool) -> String {
        research_workspace(WorkspaceView {
            overview: json!({
                "workspace": {"id": "w1", "code": "AI-IDEA-001", "classification": "INTERNAL",
                               "may_transition": may_transition},
                "idea": {"id": "i1", "title": "Ideia", "state": state,
                          "available_transitions": transitions, "promotable": promotable},
                "project": null,
                "members": []
            }),
            sources: json!({"items": []}),
            notes: json!([]),
            documents: json!([]),
            datasets: json!({"items": []}),
            tasks: json!({"items": []}),
            activity: json!([]),
            inference_available: false,
            may_use_assistance: true,
            gestao: gestao_de_prova(),
        })
        .to_html()
    }

    /// Uma ideia por promover oferece os movimentos legais do Core — e **não**
    /// «Promover a Projecto», que só surge quando ela é candidata.
    #[test]
    fn uma_ideia_oferece_o_ciclo_de_vida_que_o_core_permite() {
        // Em exploração: pode avançar para Conceito; ainda não é promovível.
        let cedo = idea_ws(
            "exploration",
            json!([
                {"state": "concept", "requires_note": false},
                {"state": "rejected", "requires_note": true}
            ]),
            false,
            true,
        );
        assert!(
            cedo.contains("Avançar para Conceito"),
            "falta o avanço de estado"
        );
        assert!(cedo.contains("Rejeitar"), "falta a acção de fechar");
        assert!(cedo.contains(r#"action="/ideas/i1/transition""#));
        assert!(
            !cedo.contains("Promover a Projecto"),
            "uma ideia não-candidata não deve oferecer promoção"
        );

        // Candidata a projecto: agora sim, «Promover a Projecto» é a acção primária.
        let madura = idea_ws(
            "project_candidate",
            json!([{"state": "review", "requires_note": false}]),
            true,
            true,
        );
        assert!(madura.contains("Promover a Projecto"));
        assert!(madura.contains(r#"href="/projects/new?workspace=w1""#));

        // Sem autoridade e sem promoção, o strip não aparece.
        let sem_poder = idea_ws("exploration", json!([]), false, false);
        assert!(!sem_poder.contains("Ciclo de vida"));
    }

    #[test]
    fn a_classificacao_do_workspace_esta_sempre_visivel() {
        let html = research_workspace(WorkspaceView {
            overview: json!({
                "workspace": {"id": "w1", "code": "AI-IDEA-001", "classification": "RESTRICTED"},
                "idea": {"id": "i1", "title": "Ideia", "state": "concept"},
                "project": null,
                "members": []
            }),
            sources: json!({"items": []}),
            notes: json!([]),
            documents: json!([]),
            datasets: json!({"items": []}),
            tasks: json!({"items": []}),
            activity: json!([]),
            inference_available: false,
            may_use_assistance: true,
            gestao: gestao_de_prova(),
        })
        .to_html();

        assert!(html.contains("RESTRICTED"));
    }
}
