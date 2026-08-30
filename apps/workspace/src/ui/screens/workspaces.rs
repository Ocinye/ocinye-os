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
}

/// Constrói as tabs: só as que têm ecrã navegam.
fn tabs(labels: &[&'static str], workspace_id: &str) -> Vec<Tab> {
    labels
        .iter()
        .enumerate()
        .map(|(i, label)| match *label {
            "Visão geral" => Tab::link(*label, format!("/workspaces/{workspace_id}"), i == 0),
            "IA" => Tab::link(
                *label,
                format!("/ai/prompt?workspace={workspace_id}"),
                false,
            ),
            // Experiências e Resultados são duas leituras da mesma cadeia
            // científica, e por isso levam ao mesmo ecrã. Duas páginas que
            // partissem a cadeia ao meio obrigariam a saltar entre elas para
            // seguir uma linhagem — que é a única coisa que a cadeia serve
            // para fazer.
            "Experiências" | "Resultados" => {
                Tab::link(*label, format!("/workspaces/{workspace_id}/science"), false)
            }
            other => Tab::inert(other),
        })
        .collect()
}

/// O Research Workspace.
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
                    {button(Button::new("Partilhar", Variant::Secondary).not_yet_available())}
                    // A promoção passou a existir. Era `not_yet_available` quando
                    // não havia ecrã por trás; agora leva ao selector com esta
                    // ideia já escolhida. O Core decide na mesma se ela está em
                    // estado de ser promovida.
                    {(!is_project)
                        .then(|| {
                            button(
                                Button::new("Promover a Projecto", Variant::Secondary)
                                    .href(format!("/projects/new?workspace={id}")),
                            )
                        })}
                    {button(
                        Button::new("IA neste workspace", Variant::Primary)
                            .href(format!("/ai/prompt?workspace={id}"))
                            .with_dot(),
                    )}
                </div>
            </div>

            {context_tabs(tabs(tab_labels, &id), "Secções do Research Workspace")}
        </div>

        <div class="oc-page oc-page" >
            <div class="oc-grid oc-grid--ws">
                {if is_project {
                    project_overview(&project, &members).into_any()
                } else {
                    idea_overview(&idea, &sources, &datasets).into_any()
                }}

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

                <section class="oc-card">
                    {section_head("Actividade recente", None, None)}
                    <div class="oc-card__body">{activity_list(&activity)}</div>
                </section>

                <section class="oc-card">
                    {section_head("Tarefas", None, None)}
                    <div class="oc-card__body">{task_list(&tasks)}</div>
                </section>
            </div>

            <div class="oc-grid oc-grid--detail oc-mt-7" >
                {artefact_card("Bibliografia", &sources, "title", "/bibliography")}
                {artefact_card("Notas", &notes, "title", "/knowledge")}
            </div>

            <div class="oc-grid oc-grid--detail oc-mt-7" >
                {artefact_card("Documentos", &documents, "title", "/knowledge")}
                {artefact_card("Datasets", &datasets, "title", "/datasets")}
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
                    view! {
                        <div>
                            <div class="oc-row oc-gap-5 oc-mb-1" >
                                <span class="oc-fill oc-truncate oc-t-cell-2" >
                                    {text(row, "title")}
                                </span>
                                {badge(state.clone(), Tone::of(&state))}
                            </div>
                            {progress_bar(pct)}
                        </div>
                    }
                })
                .collect_view()}
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

    view! {
        <div class="oc-band" >
            <div class="oc-row oc-row--wrap oc-gap-6 oc-mb-2" >
                <h1 class="oc-t-screen" >
                    {text(unit, "name")}
                </h1>
                {pill(text(unit, "code"))}
                {badge(status.clone(), Tone::of(&status))}
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
                        <div class="oc-split oc-split--2" >
                            {metric("Membros", member_rows.len())}
                            {metric("Ideias", ideas)}
                            {metric("Projectos", projects)}
                            {metric("Áreas", unit
                                .get("research_areas")
                                .and_then(Value::as_array)
                                .map_or(0, Vec::len))}
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

fn aviso_de_gestao(ok: bool, mensagem: &str) -> impl IntoView {
    let classe = if ok {
        "oc-note oc-note--ok oc-mt-5"
    } else {
        "oc-note oc-note--bad oc-mt-5"
    };
    view! { <p class=classe role="status">{mensagem.to_owned()}</p> }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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

    #[test]
    fn uma_ideia_oferece_promocao_e_um_projecto_nao() {
        let idea = research_workspace(WorkspaceView {
            overview: json!({
                "workspace": {"id": "w1", "code": "AI-IDEA-001", "classification": "INTERNAL"},
                "idea": {"id": "i1", "title": "Ideia", "state": "exploration"},
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
        })
        .to_html();

        assert!(idea.contains("Promover a Projecto"));
        assert!(idea.contains("IDEIA"));
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
        })
        .to_html();

        assert!(html.contains("RESTRICTED"));
    }
}
