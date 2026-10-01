//! D005 · Investigação e trabalho no Workspace: Projectos, O Meu Trabalho,
//! Ideias, Dados e Conhecimento (a Bibliografia abre Conhecimento), cada uma
//! numa janela gerida (D002) com o ecrã do Design (`ui::apps`).
//!
//! Aqui só há canal. Cada página relê, com a sessão do membro, tudo o que
//! mostra — um recurso revogado entretanto já não aparece —, e passa-o pelos
//! controladores `controllers::research`. O Core autoriza cada acção; uma
//! relação só aparece se o Core resolveu as duas pontas para este membro.

use std::collections::HashMap;

use leptos::prelude::*;

use super::productivity::{clock_of, dirty_template, open_app, render_app, AppWindow};
use super::*;
use crate::controllers::desktop::{instant, text};
use crate::controllers::productivity::app_error;
use crate::controllers::research::{self as rs, datasets as ds, ideas as id, knowledge as kn};
use crate::controllers::research::{projects as pj, work as wk};
use crate::i18n::t;
use crate::ui::view_models::{
    AppError, AppLoad, AppPageVm, DatasetFormVm, DatasetVm, DatasetsVm, DirtyCloseVm, IdeaFormVm,
    IdeaPromotionVm, IdeaVm, IdeasVm, KnowledgeDocumentVm, KnowledgeSection, KnowledgeVm,
    ProjectVm, ProjectsVm, ResKind, ResLinkVm, ResListVm, ResOptionVm, ResPane, SourceFormVm,
    SourceVm, TaskAssignVm, TaskFormVm, TaskPriorityLevel, TaskVm, WorkVm,
};

/// A pergunta comum às cinco listas.
#[derive(Deserialize, Default)]
pub(super) struct ResQuery {
    #[serde(default)]
    nav: Option<String>,
    #[serde(default)]
    workspace: Option<String>,
    #[serde(default)]
    page: Option<u32>,
    #[serde(default)]
    q: Option<String>,
    #[serde(default)]
    v: Option<String>,
    /// O resultado de uma acção que o Core recusou (`?err=`).
    #[serde(default)]
    err: Option<String>,
}

/// O erro de uma acção devolvido pelo endereço: um vocabulário fechado.
fn action_error(v: Option<&str>) -> Option<AppError> {
    Some(match v? {
        "conflict" => AppError::Conflict,
        "denied" => AppError::PermissionDenied,
        "save" => AppError::SaveFailed,
        "gone" => AppError::NotFound,
        _ => AppError::Unavailable,
    })
}

fn action_code(f: &ApiFailure) -> &'static str {
    match f {
        ApiFailure::Conflict(_) => "conflict",
        ApiFailure::Forbidden => "denied",
        ApiFailure::Denied => "gone",
        ApiFailure::Rejected(_) => "save",
        _ => "unavailable",
    }
}

/// Um campo de formulário para o Core: o texto, ou `null` quando vazio.
fn text_or_null(s: &str) -> Value {
    let s = s.trim();
    if s.is_empty() {
        Value::Null
    } else {
        Value::String(s.to_owned())
    }
}

fn items_of(v: &Value) -> Vec<Value> {
    v.get("items")
        .and_then(Value::as_array)
        .cloned()
        .or_else(|| v.as_array().cloned())
        .unwrap_or_default()
}

fn pane(open: bool) -> ResPane {
    if open {
        ResPane::Detail
    } else {
        ResPane::List
    }
}

/// Os ambientes que o membro vê (títulos e `may_create`), para os nomes das
/// linhas e para os selectores governados.
async fn visible_workspaces(state: &WorkspaceState, quem: &controllers::Caller<'_>) -> Vec<Value> {
    quem.get(state, "/api/v1/workspaces?page_size=100")
        .await
        .map(|v| items_of(&v))
        .unwrap_or_default()
}

fn workspace_titles(list: &[Value]) -> HashMap<String, String> {
    list.iter()
        .map(|w| (text(w, "id").to_owned(), text(w, "title").to_owned()))
        .collect()
}

/// Os ambientes onde o membro pode criar, como opções.
fn creatable(list: &[Value], selected: Option<&str>) -> Vec<ResOptionVm> {
    list.iter()
        .filter(|w| w.get("may_create").and_then(Value::as_bool) == Some(true))
        .map(|w| ResOptionVm {
            value: text(w, "id").to_owned(),
            label: text(w, "title").to_owned(),
            selected: Some(text(w, "id")) == selected,
        })
        .collect()
}

/// O filtro por ambiente: «Todos» primeiro, e os ambientes que o membro vê.
fn filter_options(list: &[Value], selected: Option<&str>) -> Vec<ResOptionVm> {
    let mut o = vec![ResOptionVm {
        value: String::new(),
        label: t("res.filter.all").to_owned(),
        selected: selected.is_none(),
    }];
    o.extend(list.iter().map(|w| ResOptionVm {
        value: text(w, "id").to_owned(),
        label: text(w, "title").to_owned(),
        selected: Some(text(w, "id")) == selected,
    }));
    o
}

/// As linhas abrem o recurso **com os filtros da lista**: abrir um item não
/// muda o que a lista mostra ao lado.
fn keep_filters(items: &mut [crate::ui::view_models::ResItemVm], list_href: &str) {
    if let Some((_, q)) = list_href.split_once('?') {
        for i in items {
            i.href = format!("{}?{q}", i.href);
        }
    }
}

/// O ambiente como ligação canónica: o projecto, ou a ideia.
fn workspace_link(overview: &Value) -> Option<ResLinkVm> {
    let w = overview.get("workspace")?;
    let title = text(w, "title").to_owned();
    if let Some(p) = overview.get("project").filter(|p| !p.is_null()) {
        return Some(ResLinkVm {
            kind: ResKind::Project,
            kind_label: None,
            title,
            meta: Some(text(p, "code").to_owned()),
            relation: None,
            by_operation: false,
            href: Some(format!("/projects/{}", text(p, "id"))),
        });
    }
    overview
        .get("idea")
        .filter(|i| !i.is_null())
        .map(|i| ResLinkVm {
            kind: ResKind::Idea,
            kind_label: None,
            title,
            meta: Some(text(w, "code").to_owned()),
            relation: None,
            by_operation: false,
            href: Some(format!("/ideas/{}", text(i, "id"))),
        })
}

fn flag(overview: &Value, k: &str) -> bool {
    overview
        .get("workspace")
        .and_then(|w| w.get(k))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

/// As relações tipadas de um recurso, pela linhagem (as duas pontas
/// resolvidas pelo Core para este membro), nos dois sentidos.
pub(super) async fn relations(
    state: &WorkspaceState,
    quem: &controllers::Caller<'_>,
    kind: &str,
    id: &str,
) -> Vec<ResLinkVm> {
    let mut out = Vec::new();
    for sentido in ["upstream", "downstream"] {
        if let Ok(l) = quem
            .get(
                state,
                &format!("/api/v1/lineage/{kind}/{id}?direction={sentido}&depth=1"),
            )
            .await
        {
            out.extend(rs::links(&l));
        }
    }
    out
}

/// Os nomes das pessoas dos ambientes indicados (pela leitura de cada um).
async fn people_of(
    state: &WorkspaceState,
    quem: &controllers::Caller<'_>,
    workspaces: Vec<String>,
) -> HashMap<String, String> {
    let mut nomes = HashMap::new();
    let mut vistos = std::collections::BTreeSet::new();
    for w in workspaces {
        if w.is_empty() || !vistos.insert(w.clone()) {
            continue;
        }
        if let Ok(o) = quem.get(state, &format!("/api/v1/workspaces/{w}")).await {
            for m in o
                .get("members")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                nomes.insert(
                    text(m, "person_id").to_owned(),
                    text(m, "full_name").to_owned(),
                );
            }
        }
    }
    nomes
}

/// Fecha a janela da aplicação (o «Guardar» do diálogo de fecho D002 envia
/// `then=close`) e volta ao que a mesa mostra a seguir.
fn close_app(state: &WorkspaceState, member: &Member, app: ApplicationId) -> Response {
    let next = state
        .sessions
        .with_desk(&member.session_id, |d| {
            let ids: Vec<String> = d
                .windows()
                .iter()
                .filter(|w| w.app == app)
                .map(|w| w.id.clone())
                .collect();
            for id in ids {
                let _ = d.report(&id, false, true);
                let _ = d.close(&id, None);
            }
            d.current_href().to_owned()
        })
        .unwrap_or_else(|| "/".to_owned());
    Redirect::to(&next).into_response()
}

fn dirty_for(
    w: &AppWindow,
    app: &str,
    title_key: &str,
    save_label: &'static str,
) -> Option<DirtyCloseVm> {
    w.window.as_ref().map(|win| DirtyCloseVm {
        window_id: win.clone(),
        title: t(title_key).to_owned(),
        can_save: true,
        save_label: Some(save_label),
        save_form: Some(ui::apps::doc_form_id(app, "new")),
        after: None,
    })
}

fn render_with_dirty(
    state: &WorkspaceState,
    w: &AppWindow,
    status: StatusCode,
    body: AnyView,
    dirty: Option<DirtyCloseVm>,
) -> Response {
    let extra = dirty.as_ref().map(dirty_template);
    render_app(state, w, status, body, extra, dirty)
}

#[derive(Deserialize)]
pub(super) struct TransitionForm {
    state: String,
    #[serde(default)]
    outcome_note: Option<String>,
}

/// Uma transição pedida ao Core. O que ele responde é a verdade: em caso de
/// recusa, a página volta a ler o recurso e diz porquê.
async fn transition(
    state: &WorkspaceState,
    headers: &HeaderMap,
    core_path: String,
    back: String,
    form: TransitionForm,
) -> Response {
    let member = member_or_login!(state, headers);
    let mut body = serde_json::json!({ "state": form.state });
    if let Some(n) = form.outcome_note.filter(|n| !n.trim().is_empty()) {
        body["outcome_note"] = Value::String(n);
    }
    match api::post(
        state,
        &member.session.access_token,
        &member.correlation_id,
        &core_path,
        &body,
    )
    .await
    {
        Ok(_) => Redirect::to(&back).into_response(),
        Err(ApiFailure::Unauthorised) => session_ended(state, headers),
        Err(f) => Redirect::to(&format!("{back}?err={}", action_code(&f))).into_response(),
    }
}

// ── Projectos ────────────────────────────────────────────────────────────

pub(super) async fn projects_page(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(q): Query<ResQuery>,
) -> Response {
    projects_view(&state, &headers, q, None).await
}

pub(super) async fn project_page(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(project): Path<Uuid>,
    Query(q): Query<ResQuery>,
) -> Response {
    projects_view(&state, &headers, q, Some(project)).await
}

async fn projects_view(
    state: &WorkspaceState,
    headers: &HeaderMap,
    q: ResQuery,
    open: Option<Uuid>,
) -> Response {
    let w = match open_app(state, headers, Screen::Projects, ApplicationId::Projects).await {
        Ok(w) => w,
        Err(r) => return *r,
    };
    let quem = caller(&w.member);
    let clock = clock_of(&w.ctx);
    let f = pj::filter_of(q.nav.as_deref());
    let aberto = open.map(|o| o.to_string());
    let (items, load, page, linhas) = match quem
        .get(state, &pj::list_path(f, q.page.unwrap_or(1).max(1)))
        .await
    {
        Ok(v) => {
            let linhas = items_of(&v);
            (
                pj::items(&linhas, aberto.as_deref(), &clock),
                AppLoad::Ready,
                rs::page(&v, &pj::list_href(f)),
                linhas,
            )
        }
        Err(ApiFailure::Unauthorised) => return session_ended(state, headers),
        Err(e) => (
            Vec::new(),
            AppLoad::Failed(app_error(&e)),
            AppPageVm::default(),
            Vec::new(),
        ),
    };
    let (project, mut project_error) = match aberto.as_deref() {
        None => (None, None),
        Some(pid) => match project_detail(state, &quem, pid, &linhas, &clock).await {
            Ok(p) => (Some(p), None),
            Err(ApiFailure::Unauthorised) => return session_ended(state, headers),
            Err(e) => (None, Some(app_error(&e))),
        },
    };
    if let Some(e) = action_error(q.err.as_deref()) {
        project_error = Some(e);
    }
    let mut items = items;
    keep_filters(&mut items, &pj::list_href(f));
    let vm = ProjectsVm {
        nav: pj::navigation(f),
        list: ResListVm {
            columns: pj::COLUMNS.to_vec(),
            items,
            load,
            page,
        },
        pane: pane(open.is_some()),
        project: project_error.is_none().then_some(project).flatten(),
        project_error,
        ideas_href: screen_open(&w.ctx, Screen::Ideas).then(|| "/ideas?nav=candidates".to_owned()),
        list_href: pj::list_href(f),
    };
    render_app(
        state,
        &w,
        StatusCode::OK,
        ui::apps::projects::app(&vm),
        None,
        None,
    )
}

async fn project_detail(
    state: &WorkspaceState,
    quem: &controllers::Caller<'_>,
    pid: &str,
    linhas: &[Value],
    clock: &controllers::desktop::Clock,
) -> Result<ProjectVm, ApiFailure> {
    let p = quem.get(state, &format!("/api/v1/projects/{pid}")).await?;
    let ws = text(&p, "workspace_id").to_owned();
    let ov = quem.get(state, &format!("/api/v1/workspaces/{ws}")).await?;
    let estado = pj::status(text(&p, "state"))
        .ok_or_else(|| ApiFailure::Failed("estado de projecto desconhecido".to_owned()))?;
    let resumo = linhas
        .iter()
        .find(|l| text(l, "id") == ws)
        .and_then(|l| l.get("summary"));
    let responsavel = text(&p, "responsible_person_id");
    let origem = match rs::opt(&p, "origin_idea_id") {
        Some(iid) => quem
            .get(state, &format!("/api/v1/ideas/{iid}"))
            .await
            .ok()
            .and_then(|v| {
                let i = v.get("idea")?;
                Some(ResLinkVm {
                    kind: ResKind::Idea,
                    kind_label: None,
                    title: text(i, "title").to_owned(),
                    meta: id::stage(text(i, "state")).map(|s| t(id::state_vm(s).key).to_owned()),
                    relation: None,
                    by_operation: false,
                    href: Some(format!("/ideas/{iid}")),
                })
            }),
        None => None,
    };
    let tarefas = quem
        .get(
            state,
            &format!("/api/v1/tasks?workspace_id={ws}&open_only=true&page_size=5"),
        )
        .await
        .map(|v| items_of(&v))
        .unwrap_or_default()
        .iter()
        .map(|x| ResLinkVm {
            kind: ResKind::Task,
            kind_label: None,
            title: text(x, "title").to_owned(),
            meta: wk::status(text(x, "state")).map(|s| t(wk::state_vm(s).key).to_owned()),
            relation: None,
            by_operation: false,
            href: Some(format!("/my-work/{}", text(x, "id"))),
        })
        .collect();
    let datasets = quem
        .get(
            state,
            &format!("/api/v1/datasets?workspace_id={ws}&page_size=5"),
        )
        .await
        .map(|v| items_of(&v))
        .unwrap_or_default()
        .iter()
        .map(|x| ResLinkVm {
            kind: ResKind::Dataset,
            kind_label: None,
            title: text(x, "title").to_owned(),
            meta: Some(text(x, "code").to_owned()),
            relation: None,
            by_operation: false,
            href: Some(format!("/datasets/{}", text(x, "id"))),
        })
        .collect();
    let mut conhecimento: Vec<ResLinkVm> = Vec::new();
    for (caminho, kind, seg) in [
        (
            format!("/api/v1/workspaces/{ws}/sources?page_size=5"),
            ResKind::Source,
            "sources",
        ),
        (
            format!("/api/v1/workspaces/{ws}/documents?page_size=5"),
            ResKind::Document,
            "documents",
        ),
    ] {
        if let Ok(v) = quem.get(state, &caminho).await {
            conhecimento.extend(items_of(&v).iter().take(5).map(|x| ResLinkVm {
                kind,
                kind_label: None,
                title: text(x, "title").to_owned(),
                meta: None,
                relation: None,
                by_operation: false,
                href: Some(format!("/knowledge/{seg}/{}", text(x, "id"))),
            }));
        }
    }
    let wsv = ov.get("workspace").cloned().unwrap_or_default();
    Ok(ProjectVm {
        code: text(&p, "code").to_owned(),
        title: text(&p, "title").to_owned(),
        state: estado,
        classification: rs::classification(text(&wsv, "classification")),
        summary: rs::opt(&p, "summary"),
        objectives: rs::opt(&p, "objectives"),
        unit: resumo.and_then(|s| rs::opt(s, "unit_name")),
        responsible: ov
            .get("members")
            .and_then(Value::as_array)
            .and_then(|m| m.iter().find(|x| text(x, "person_id") == responsavel))
            .map(|x| text(x, "full_name").to_owned())
            .or_else(|| resumo.and_then(|s| rs::opt(s, "responsible_name"))),
        started: instant(&p, "started_at").map(|at| rs::day(at, clock)),
        completed: instant(&p, "completed_at").map(|at| rs::day(at, clock)),
        origin_idea: origem,
        members: rs::people(&ov),
        tasks: tarefas,
        tasks_href: Some(format!("/my-work?nav=open&workspace={ws}")),
        new_task_href: flag(&ov, "may_create").then(|| format!("/my-work/new?workspace={ws}")),
        datasets,
        knowledge: conhecimento,
        links: relations(state, quem, "project", pid).await,
        transitions: pj::transitions(&p, flag(&ov, "may_transition"), pid),
        nye: Some(rs::nye("project", pid, "projects.nye")),
    })
}

pub(super) async fn project_transition(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(project): Path<Uuid>,
    Form(form): Form<TransitionForm>,
) -> Response {
    transition(
        &state,
        &headers,
        format!("/api/v1/projects/{project}/transitions"),
        format!("/projects/{project}"),
        form,
    )
    .await
}

// ── O Meu Trabalho ───────────────────────────────────────────────────────

pub(super) async fn work_page(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(q): Query<ResQuery>,
) -> Response {
    work_view(&state, &headers, q, None, None, StatusCode::OK).await
}

pub(super) async fn task_page(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(task): Path<Uuid>,
    Query(q): Query<ResQuery>,
) -> Response {
    work_view(&state, &headers, q, Some(task), None, StatusCode::OK).await
}

pub(super) async fn task_new_page(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(q): Query<ResQuery>,
) -> Response {
    let form = TaskFormVm {
        action: "/my-work/new".to_owned(),
        workspaces: Vec::new(),
        title: String::new(),
        description: String::new(),
        priority: TaskPriorityLevel::Normal,
        due: String::new(),
        error: None,
        cancel_href: wk::list_href(wk::Filter::Mine, None),
    };
    work_view(&state, &headers, q, None, Some(form), StatusCode::OK).await
}

async fn work_view(
    state: &WorkspaceState,
    headers: &HeaderMap,
    q: ResQuery,
    open: Option<Uuid>,
    form: Option<TaskFormVm>,
    status: StatusCode,
) -> Response {
    let w = match open_app(state, headers, Screen::MyWork, ApplicationId::Work).await {
        Ok(w) => w,
        Err(r) => return *r,
    };
    let quem = caller(&w.member);
    let clock = clock_of(&w.ctx);
    let f = wk::filter_of(q.nav.as_deref());
    let ambiente = q
        .workspace
        .as_deref()
        .filter(|x| Uuid::parse_str(x).is_ok());
    let ambientes = visible_workspaces(state, &quem).await;
    let titulos = workspace_titles(&ambientes);
    let aberto = open.map(|o| o.to_string());
    let (items, load, page) = match quem
        .get(
            state,
            &wk::list_path(f, ambiente, q.page.unwrap_or(1).max(1)),
        )
        .await
    {
        Ok(v) => {
            let linhas = items_of(&v);
            let ids: Vec<String> = linhas
                .iter()
                .map(|l| text(l, "workspace_id").to_owned())
                .collect();
            let pessoas = people_of(state, &quem, ids).await;
            (
                wk::items(&linhas, aberto.as_deref(), &titulos, &pessoas, &clock),
                AppLoad::Ready,
                rs::page(&v, &wk::list_href(f, ambiente)),
            )
        }
        Err(ApiFailure::Unauthorised) => return session_ended(state, headers),
        Err(e) => (
            Vec::new(),
            AppLoad::Failed(app_error(&e)),
            AppPageVm::default(),
        ),
    };
    let (task, mut task_error) = match aberto.as_deref() {
        None => (None, None),
        Some(tid) => match task_detail(state, &quem, tid, &clock).await {
            Ok(x) => (Some(x), None),
            Err(ApiFailure::Unauthorised) => return session_ended(state, headers),
            Err(e) => (None, Some(app_error(&e))),
        },
    };
    if let Some(e) = action_error(q.err.as_deref()) {
        task_error = Some(e);
    }
    let pode_criar = !creatable(&ambientes, None).is_empty();
    let form = form.map(|mut f| {
        f.workspaces = creatable(&ambientes, ambiente.or(Some("")).filter(|x| !x.is_empty()));
        f
    });
    let dirty = form
        .as_ref()
        .and_then(|_| dirty_for(&w, "work", "work.new", "work.create"));
    let mut items = items;
    keep_filters(&mut items, &wk::list_href(f, ambiente));
    let vm = WorkVm {
        nav: wk::navigation(f, ambiente),
        workspace_filter: filter_options(&ambientes, ambiente),
        list: ResListVm {
            columns: wk::COLUMNS.to_vec(),
            items,
            load,
            page,
        },
        pane: pane(open.is_some() || form.is_some()),
        task: task_error.is_none().then_some(task).flatten(),
        task_error,
        form,
        new_href: pode_criar.then(|| "/my-work/new".to_owned()),
        list_href: wk::list_href(f, ambiente),
    };
    render_with_dirty(state, &w, status, ui::apps::work::app(&vm), dirty)
}

async fn task_detail(
    state: &WorkspaceState,
    quem: &controllers::Caller<'_>,
    tid: &str,
    clock: &controllers::desktop::Clock,
) -> Result<TaskVm, ApiFailure> {
    let x = quem.get(state, &format!("/api/v1/tasks/{tid}")).await?;
    let ws = text(&x, "workspace_id").to_owned();
    let ov = quem.get(state, &format!("/api/v1/workspaces/{ws}")).await?;
    let estado = wk::status(text(&x, "state"))
        .ok_or_else(|| ApiFailure::Failed("estado de tarefa desconhecido".to_owned()))?;
    let prioridade = wk::priority(text(&x, "priority"))
        .ok_or_else(|| ApiFailure::Failed("prioridade desconhecida".to_owned()))?;
    let due = chrono::NaiveDate::parse_from_str(text(&x, "due_on"), "%Y-%m-%d").ok();
    let atribuida = text(&x, "assignee_id");
    let pessoa = ov
        .get("members")
        .and_then(Value::as_array)
        .and_then(|m| m.iter().find(|p| text(p, "person_id") == atribuida))
        .map(|p| text(p, "full_name").to_owned());
    let assign = flag(&ov, "may_create").then(|| {
        let mut candidates = vec![ResOptionVm {
            value: String::new(),
            label: t("prod.work.nobody").to_owned(),
            selected: atribuida.is_empty(),
        }];
        candidates.extend(rs::member_options(&ov, Some(atribuida)));
        TaskAssignVm {
            action: format!("/my-work/{tid}/assignee"),
            candidates,
        }
    });
    Ok(TaskVm {
        title: text(&x, "title").to_owned(),
        state: estado,
        priority: prioridade,
        classification: rs::classification(text(&x, "classification")),
        description: rs::opt(&x, "description"),
        due: due.map(|d| d.format("%d/%m/%Y").to_string()),
        overdue: wk::overdue(due, Some(estado), clock),
        assignee: pessoa,
        workspace: workspace_link(&ov),
        closed: None,
        transitions: flag(&ov, "may_transition")
            .then(|| wk::transitions(&x, tid))
            .flatten(),
        assign,
        links: relations(state, quem, "task", tid).await,
        nye: Some(rs::nye("task", tid, "work.nye")),
    })
}

#[derive(Deserialize)]
pub(super) struct TaskCreateForm {
    #[serde(default)]
    workspace: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    priority: String,
    #[serde(default)]
    due_on: String,
    #[serde(default)]
    then: Option<String>,
}

pub(super) async fn task_create(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<TaskCreateForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let prioridade = wk::priority(&form.priority).unwrap_or(TaskPriorityLevel::Normal);
    let resultado = match Uuid::parse_str(form.workspace.trim()) {
        Ok(ws) => {
            let mut body = serde_json::json!({
                "title": form.title.trim(),
                "priority": wk::priority_str(prioridade),
            });
            if !form.description.trim().is_empty() {
                body["description"] = Value::String(form.description.clone());
            }
            if !form.due_on.trim().is_empty() {
                body["due_on"] = Value::String(form.due_on.trim().to_owned());
            }
            api::post(
                &state,
                &member.session.access_token,
                &member.correlation_id,
                &format!("/api/v1/workspaces/{ws}/tasks"),
                &body,
            )
            .await
        }
        Err(_) => Err(ApiFailure::Rejected("sem ambiente".to_owned())),
    };
    match resultado {
        Ok(_) if form.then.as_deref() == Some("close") => {
            close_app(&state, &member, ApplicationId::Work)
        }
        Ok(v) => Redirect::to(&format!("/my-work/{}", text(&v, "id"))).into_response(),
        Err(ApiFailure::Unauthorised) => session_ended(&state, &headers),
        Err(e) => {
            let f = TaskFormVm {
                action: "/my-work/new".to_owned(),
                workspaces: Vec::new(),
                title: form.title,
                description: form.description,
                priority: prioridade,
                due: form.due_on,
                error: Some(match e {
                    ApiFailure::Forbidden => AppError::PermissionDenied,
                    _ => AppError::SaveFailed,
                }),
                cancel_href: wk::list_href(wk::Filter::Mine, None),
            };
            let q = ResQuery {
                workspace: Some(form.workspace),
                ..ResQuery::default()
            };
            work_view(
                &state,
                &headers,
                q,
                None,
                Some(f),
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await
        }
    }
}

pub(super) async fn task_transition(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(task): Path<Uuid>,
    Form(form): Form<TransitionForm>,
) -> Response {
    transition(
        &state,
        &headers,
        format!("/api/v1/tasks/{task}/transitions"),
        format!("/my-work/{task}"),
        form,
    )
    .await
}

#[derive(Deserialize)]
pub(super) struct AssignForm {
    #[serde(default)]
    assignee_id: String,
}

/// Atribuir: só a uma pessoa do ambiente da tarefa (lead ou member). A BFF
/// recusa o resto antes de o pedido sair; o Core volta a validar a escolha.
pub(super) async fn task_assign(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(task): Path<Uuid>,
    Form(form): Form<AssignForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let quem = caller(&member);
    let back = format!("/my-work/{task}");
    let alvo = form.assignee_id.trim();
    let assignee = if alvo.is_empty() {
        Value::Null
    } else {
        let Ok(pessoa) = Uuid::parse_str(alvo) else {
            return Redirect::to(&format!("{back}?err=save")).into_response();
        };
        let ov = match quem.get(&state, &format!("/api/v1/tasks/{task}")).await {
            Ok(x) => {
                quem.get(
                    &state,
                    &format!("/api/v1/workspaces/{}", text(&x, "workspace_id")),
                )
                .await
            }
            Err(e) => Err(e),
        };
        match ov {
            Ok(ov) if rs::is_member(&ov, &pessoa.to_string()) => Value::String(pessoa.to_string()),
            Ok(_) => return Redirect::to(&format!("{back}?err=save")).into_response(),
            Err(ApiFailure::Unauthorised) => return session_ended(&state, &headers),
            Err(e) => {
                return Redirect::to(&format!("{back}?err={}", action_code(&e))).into_response()
            }
        }
    };
    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/tasks/{task}/assignee"),
        &serde_json::json!({ "assignee_id": assignee }),
    )
    .await
    {
        Ok(_) => Redirect::to(&back).into_response(),
        Err(ApiFailure::Unauthorised) => session_ended(&state, &headers),
        Err(e) => Redirect::to(&format!("{back}?err={}", action_code(&e))).into_response(),
    }
}

/// Endereços antigos das tarefas: a tarefa abre em «O Meu Trabalho».
pub(super) async fn task_legacy(Path(task): Path<Uuid>) -> Response {
    Redirect::to(&format!("/my-work/{task}")).into_response()
}

// ── Ideias ───────────────────────────────────────────────────────────────

pub(super) async fn ideas_page(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(q): Query<ResQuery>,
) -> Response {
    ideas_view(&state, &headers, q, None, None, StatusCode::OK).await
}

pub(super) async fn idea_page(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(idea): Path<Uuid>,
    Query(q): Query<ResQuery>,
) -> Response {
    ideas_view(&state, &headers, q, Some(idea), None, StatusCode::OK).await
}

fn empty_idea_form() -> IdeaFormVm {
    IdeaFormVm {
        action: "/ideas/new".to_owned(),
        units: Vec::new(),
        classifications: Vec::new(),
        title: String::new(),
        summary: String::new(),
        research_question: String::new(),
        hypothesis: String::new(),
        motivation: String::new(),
        keywords: String::new(),
        error: None,
        cancel_href: id::list_href(id::Group::Developing),
    }
}

pub(super) async fn idea_new_page(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(q): Query<ResQuery>,
) -> Response {
    ideas_view(
        &state,
        &headers,
        q,
        None,
        Some(empty_idea_form()),
        StatusCode::OK,
    )
    .await
}

/// As unidades onde o membro pode registar uma ideia, pelo nome: é a mesma
/// pergunta que o Core faz (`Action::Create` na unidade) — gerir a unidade,
/// ou administrar a organisation. Ser membro da unidade não chega.
async fn my_units(
    state: &WorkspaceState,
    quem: &controllers::Caller<'_>,
    selected: &str,
) -> Vec<ResOptionVm> {
    let Ok(me) = quem.get(state, "/api/v1/me").await else {
        return Vec::new();
    };
    let admin = me.get("roles").and_then(Value::as_array).is_some_and(|r| {
        r.iter()
            .filter_map(Value::as_str)
            .any(|x| x == "organisation_admin" || x == "platform_admin")
    });
    let geridas: Vec<String> = me
        .get("units")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter(|u| text(u, "role") == "manager")
        .map(|u| text(u, "id").to_owned())
        .collect();
    if !admin && geridas.is_empty() {
        return Vec::new();
    }
    let todas = quem
        .get(state, "/api/v1/units?page_size=100")
        .await
        .map(|v| items_of(&v))
        .unwrap_or_default();
    todas
        .iter()
        .filter(|u| admin || geridas.iter().any(|m| m == text(u, "id")))
        .map(|u| ResOptionVm {
            value: text(u, "id").to_owned(),
            label: text(u, "name").to_owned(),
            selected: text(u, "id") == selected,
        })
        .collect()
}

fn classifications(selected: &str) -> Vec<ResOptionVm> {
    let sel = if selected.is_empty() {
        "INTERNAL"
    } else {
        selected
    };
    [
        ("PUBLIC", "res.class.public"),
        ("INTERNAL", "res.class.internal"),
        ("CONFIDENTIAL", "res.class.confidential"),
        ("RESTRICTED", "res.class.restricted"),
    ]
    .into_iter()
    .map(|(v, k)| ResOptionVm {
        value: v.to_owned(),
        label: t(k).to_owned(),
        selected: v == sel,
    })
    .collect()
}

async fn ideas_view(
    state: &WorkspaceState,
    headers: &HeaderMap,
    q: ResQuery,
    open: Option<Uuid>,
    form: Option<IdeaFormVm>,
    status: StatusCode,
) -> Response {
    let w = match open_app(state, headers, Screen::Ideas, ApplicationId::Ideas).await {
        Ok(w) => w,
        Err(r) => return *r,
    };
    let quem = caller(&w.member);
    let clock = clock_of(&w.ctx);
    let g = id::group_of(q.nav.as_deref());
    let aberto = open.map(|o| o.to_string());
    let (items, load, page, linhas) = match quem
        .get(state, &id::list_path(g, q.page.unwrap_or(1).max(1)))
        .await
    {
        Ok(v) => {
            let linhas = items_of(&v);
            (
                id::items(&linhas, aberto.as_deref(), &clock),
                AppLoad::Ready,
                rs::page(&v, &id::list_href(g)),
                linhas,
            )
        }
        Err(ApiFailure::Unauthorised) => return session_ended(state, headers),
        Err(e) => (
            Vec::new(),
            AppLoad::Failed(app_error(&e)),
            AppPageVm::default(),
            Vec::new(),
        ),
    };
    let (idea, mut idea_error) = match aberto.as_deref() {
        None => (None, None),
        Some(iid) => match idea_detail(state, &quem, iid, &linhas).await {
            Ok(i) => (Some(i), None),
            Err(ApiFailure::Unauthorised) => return session_ended(state, headers),
            Err(e) => (None, Some(app_error(&e))),
        },
    };
    if let Some(e) = action_error(q.err.as_deref()) {
        idea_error = Some(e);
    }
    let unidades = my_units(state, &quem, "").await;
    let form = match form {
        Some(mut f) => {
            let sel = f
                .units
                .iter()
                .find(|u| u.selected)
                .map(|u| u.value.clone())
                .unwrap_or_default();
            let class = f
                .classifications
                .iter()
                .find(|c| c.selected)
                .map(|c| c.value.clone())
                .unwrap_or_default();
            f.units = my_units(state, &quem, &sel).await;
            f.classifications = classifications(&class);
            Some(f)
        }
        None => None,
    };
    let dirty = form
        .as_ref()
        .and_then(|_| dirty_for(&w, "ideas", "ideas.new", "ideas.create"));
    let mut items = items;
    keep_filters(&mut items, &id::list_href(g));
    let vm = IdeasVm {
        nav: id::navigation(g),
        list: ResListVm {
            columns: id::COLUMNS.to_vec(),
            items,
            load,
            page,
        },
        pane: pane(open.is_some() || form.is_some()),
        idea: idea_error.is_none().then_some(idea).flatten(),
        idea_error,
        form,
        new_href: (!unidades.is_empty()).then(|| "/ideas/new".to_owned()),
        list_href: id::list_href(g),
    };
    render_with_dirty(state, &w, status, ui::apps::ideas::app(&vm), dirty)
}

async fn idea_detail(
    state: &WorkspaceState,
    quem: &controllers::Caller<'_>,
    iid: &str,
    linhas: &[Value],
) -> Result<IdeaVm, ApiFailure> {
    let v = quem.get(state, &format!("/api/v1/ideas/{iid}")).await?;
    let i = v.get("idea").cloned().unwrap_or_default();
    let ws = v
        .get("workspace")
        .map(|w| text(w, "id").to_owned())
        .unwrap_or_default();
    let ov = quem.get(state, &format!("/api/v1/workspaces/{ws}")).await?;
    let estadio = id::stage(text(&i, "state"))
        .ok_or_else(|| ApiFailure::Failed("estádio de ideia desconhecido".to_owned()))?;
    let pode = flag(&ov, "may_transition");
    let promovida = match rs::opt(&i, "promoted_project_id") {
        Some(pid) => quem
            .get(state, &format!("/api/v1/projects/{pid}"))
            .await
            .ok()
            .map(|p| ResLinkVm {
                kind: ResKind::Project,
                kind_label: None,
                title: text(&p, "title").to_owned(),
                meta: pj::status(text(&p, "state")).map(|s| t(pj::state_vm(s).key).to_owned()),
                relation: None,
                by_operation: false,
                href: Some(format!("/projects/{pid}")),
            }),
        None => None,
    };
    let wsv = ov.get("workspace").cloned().unwrap_or_default();
    let resumo = linhas
        .iter()
        .find(|l| text(l, "id") == ws)
        .and_then(|l| l.get("summary"));
    let promotable = i.get("promotable").and_then(Value::as_bool) == Some(true);
    Ok(IdeaVm {
        title: text(&i, "title").to_owned(),
        state: estadio,
        classification: rs::classification(text(&wsv, "classification")),
        unit: resumo.and_then(|s| rs::opt(s, "unit_name")),
        summary: rs::opt(&i, "summary"),
        research_question: rs::opt(&i, "research_question"),
        hypothesis: rs::opt(&i, "hypothesis"),
        motivation: rs::opt(&i, "motivation"),
        keywords: rs::keywords(&i),
        outcome_note: rs::opt(&i, "outcome_note"),
        promoted_project: promovida,
        transitions: id::transitions(&i, pode, iid),
        promotion: (promotable && pode).then(|| IdeaPromotionVm {
            action: format!("/ideas/{iid}/promotion"),
            responsible: rs::member_options(&ov, None),
        }),
        members: rs::people(&ov),
        links: relations(state, quem, "idea", iid).await,
        nye: Some(rs::nye("idea", iid, "ideas.nye")),
        created: None,
    })
}

#[derive(Deserialize)]
pub(super) struct IdeaCreateForm {
    #[serde(default)]
    unit_id: String,
    #[serde(default)]
    classification: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    research_question: String,
    #[serde(default)]
    hypothesis: String,
    #[serde(default)]
    motivation: String,
    #[serde(default)]
    keywords: String,
    #[serde(default)]
    then: Option<String>,
}

pub(super) async fn idea_create(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<IdeaCreateForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let texto = text_or_null;
    let resultado = match Uuid::parse_str(form.unit_id.trim()) {
        Ok(unit) => {
            api::post(
                &state,
                &member.session.access_token,
                &member.correlation_id,
                "/api/v1/ideas",
                &serde_json::json!({
                    "unit_id": unit,
                    "title": form.title.trim(),
                    "summary": texto(&form.summary),
                    "research_question": texto(&form.research_question),
                    "hypothesis": texto(&form.hypothesis),
                    "motivation": texto(&form.motivation),
                    "keywords": rs::split_keywords(&form.keywords),
                    "classification": texto(&form.classification),
                }),
            )
            .await
        }
        Err(_) => Err(ApiFailure::Rejected("sem unidade".to_owned())),
    };
    match resultado {
        Ok(_) if form.then.as_deref() == Some("close") => {
            close_app(&state, &member, ApplicationId::Ideas)
        }
        Ok(v) => {
            let iid = v
                .get("idea")
                .map(|i| text(i, "id").to_owned())
                .unwrap_or_default();
            Redirect::to(&format!("/ideas/{iid}")).into_response()
        }
        Err(ApiFailure::Unauthorised) => session_ended(&state, &headers),
        Err(e) => {
            let mut f = empty_idea_form();
            f.units = vec![ResOptionVm {
                value: form.unit_id.clone(),
                label: String::new(),
                selected: true,
            }];
            f.classifications = vec![ResOptionVm {
                value: form.classification.clone(),
                label: String::new(),
                selected: true,
            }];
            f.title = form.title;
            f.summary = form.summary;
            f.research_question = form.research_question;
            f.hypothesis = form.hypothesis;
            f.motivation = form.motivation;
            f.keywords = form.keywords;
            f.error = Some(match e {
                ApiFailure::Forbidden => AppError::PermissionDenied,
                _ => AppError::SaveFailed,
            });
            ideas_view(
                &state,
                &headers,
                ResQuery::default(),
                None,
                Some(f),
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await
        }
    }
}

pub(super) async fn idea_transition(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(idea): Path<Uuid>,
    Form(form): Form<TransitionForm>,
) -> Response {
    transition(
        &state,
        &headers,
        format!("/api/v1/ideas/{idea}/transitions"),
        format!("/ideas/{idea}"),
        form,
    )
    .await
}

#[derive(Deserialize)]
pub(super) struct PromotionForm {
    #[serde(default)]
    code: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    objectives: String,
    #[serde(default)]
    responsible_person_id: String,
}

/// Promover: o Core cria o projecto no mesmo ambiente e a ideia fica. Um
/// segundo envio é recusado pelo Core (a ideia já não é candidata) — não nasce
/// outro projecto. O responsável tem de ser pessoa do ambiente.
pub(super) async fn idea_promote(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(idea): Path<Uuid>,
    Form(form): Form<PromotionForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let quem = caller(&member);
    let back = format!("/ideas/{idea}");
    let mut body = serde_json::json!({ "code": form.code.trim() });
    if !form.title.trim().is_empty() {
        body["title"] = Value::String(form.title.trim().to_owned());
    }
    if !form.objectives.trim().is_empty() {
        body["objectives"] = Value::String(form.objectives.clone());
    }
    let resp = form.responsible_person_id.trim();
    if !resp.is_empty() {
        let ws = match quem.get(&state, &format!("/api/v1/ideas/{idea}")).await {
            Ok(v) => v
                .get("workspace")
                .map(|w| text(w, "id").to_owned())
                .unwrap_or_default(),
            Err(ApiFailure::Unauthorised) => return session_ended(&state, &headers),
            Err(e) => {
                return Redirect::to(&format!("{back}?err={}", action_code(&e))).into_response()
            }
        };
        let membro = quem
            .get(&state, &format!("/api/v1/workspaces/{ws}"))
            .await
            .is_ok_and(|ov| rs::is_member(&ov, resp));
        if !membro {
            return Redirect::to(&format!("{back}?err=save")).into_response();
        }
        body["responsible_person_id"] = Value::String(resp.to_owned());
    }
    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/ideas/{idea}/promotion"),
        &body,
    )
    .await
    {
        Ok(p) => Redirect::to(&format!("/projects/{}", text(&p, "id"))).into_response(),
        Err(ApiFailure::Unauthorised) => session_ended(&state, &headers),
        Err(e) => Redirect::to(&format!("{back}?err={}", action_code(&e))).into_response(),
    }
}

// ── Dados ────────────────────────────────────────────────────────────────

pub(super) async fn datasets_page(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(q): Query<ResQuery>,
) -> Response {
    datasets_view(&state, &headers, q, None, None, StatusCode::OK).await
}

pub(super) async fn dataset_page(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(dataset): Path<Uuid>,
    Query(q): Query<ResQuery>,
) -> Response {
    datasets_view(&state, &headers, q, Some(dataset), None, StatusCode::OK).await
}

fn empty_dataset_form() -> DatasetFormVm {
    DatasetFormVm {
        action: "/datasets/new".to_owned(),
        workspaces: Vec::new(),
        origins: ds::origins("collected_by_ocinye"),
        code: String::new(),
        title: String::new(),
        description: String::new(),
        licence: String::new(),
        usage_restrictions: String::new(),
        keywords: String::new(),
        error: None,
        cancel_href: ds::list_href(None),
    }
}

pub(super) async fn dataset_new_page(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(q): Query<ResQuery>,
) -> Response {
    datasets_view(
        &state,
        &headers,
        q,
        None,
        Some(empty_dataset_form()),
        StatusCode::OK,
    )
    .await
}

async fn datasets_view(
    state: &WorkspaceState,
    headers: &HeaderMap,
    q: ResQuery,
    open: Option<Uuid>,
    form: Option<DatasetFormVm>,
    status: StatusCode,
) -> Response {
    let w = match open_app(state, headers, Screen::Datasets, ApplicationId::Datasets).await {
        Ok(w) => w,
        Err(r) => return *r,
    };
    let quem = caller(&w.member);
    let clock = clock_of(&w.ctx);
    let ambiente = q
        .workspace
        .as_deref()
        .filter(|x| Uuid::parse_str(x).is_ok());
    let ambientes = visible_workspaces(state, &quem).await;
    let titulos = workspace_titles(&ambientes);
    let aberto = open.map(|o| o.to_string());
    let (items, load, page) = match quem
        .get(state, &ds::list_path(ambiente, q.page.unwrap_or(1).max(1)))
        .await
    {
        Ok(v) => (
            ds::items(&items_of(&v), aberto.as_deref(), &titulos),
            AppLoad::Ready,
            rs::page(&v, &ds::list_href(ambiente)),
        ),
        Err(ApiFailure::Unauthorised) => return session_ended(state, headers),
        Err(e) => (
            Vec::new(),
            AppLoad::Failed(app_error(&e)),
            AppPageVm::default(),
        ),
    };
    let (dataset, mut dataset_error) = match aberto.as_deref() {
        None => (None, None),
        Some(did) => match dataset_detail(state, &quem, did, q.v.as_deref(), &clock).await {
            Ok(d) => (Some(d), None),
            Err(ApiFailure::Unauthorised) => return session_ended(state, headers),
            Err(e) => (None, Some(app_error(&e))),
        },
    };
    if let Some(e) = action_error(q.err.as_deref()) {
        dataset_error = Some(e);
    }
    let pode_criar = !creatable(&ambientes, None).is_empty();
    let form = form.map(|mut f| {
        f.workspaces = creatable(&ambientes, ambiente);
        f
    });
    let dirty = form
        .as_ref()
        .and_then(|_| dirty_for(&w, "datasets", "data.new", "data.create"));
    let mut items = items;
    keep_filters(&mut items, &ds::list_href(ambiente));
    let vm = DatasetsVm {
        workspace_filter: filter_options(&ambientes, ambiente),
        list: ResListVm {
            columns: ds::COLUMNS.to_vec(),
            items,
            load,
            page,
        },
        pane: pane(open.is_some() || form.is_some()),
        dataset: dataset_error.is_none().then_some(dataset).flatten(),
        dataset_error,
        form,
        new_href: pode_criar.then(|| "/datasets/new".to_owned()),
        list_href: ds::list_href(ambiente),
    };
    render_with_dirty(state, &w, status, ui::apps::datasets::app(&vm), dirty)
}

async fn dataset_detail(
    state: &WorkspaceState,
    quem: &controllers::Caller<'_>,
    did: &str,
    versao: Option<&str>,
    clock: &controllers::desktop::Clock,
) -> Result<DatasetVm, ApiFailure> {
    let d = quem.get(state, &format!("/api/v1/datasets/{did}")).await?;
    let ov = quem
        .get(
            state,
            &format!("/api/v1/workspaces/{}", text(&d, "workspace_id")),
        )
        .await?;
    let estado = ds::status(text(&d, "state"))
        .ok_or_else(|| ApiFailure::Failed("estado de dataset desconhecido".to_owned()))?;
    let pode = flag(&ov, "may_create");
    let versoes = quem
        .get(state, &format!("/api/v1/datasets/{did}/versions"))
        .await
        .map(|v| items_of(&v))
        .unwrap_or_default();
    Ok(DatasetVm {
        code: text(&d, "code").to_owned(),
        title: text(&d, "title").to_owned(),
        state: estado,
        classification: rs::classification(text(&d, "classification")),
        description: rs::opt(&d, "description"),
        origin: ds::origin_key(text(&d, "origin"))
            .map(|k| t(k).to_owned())
            .unwrap_or_default(),
        licence: rs::opt(&d, "licence"),
        usage_restrictions: rs::opt(&d, "usage_restrictions"),
        keywords: rs::keywords(&d),
        // O Core não devolve o responsável nem a data de aquisição (FG-D5-40).
        responsible: None,
        acquired: None,
        workspace: workspace_link(&ov),
        versions: ds::versions(&versoes, did, versao, pode, clock),
        new_version_action: pode.then(|| format!("/datasets/{did}/versions")),
        links: relations(state, quem, "dataset", did).await,
        nye: Some(rs::nye("dataset", did, "data.nye")),
    })
}

#[derive(Deserialize)]
pub(super) struct DatasetCreateForm {
    #[serde(default)]
    workspace: String,
    #[serde(default)]
    code: String,
    #[serde(default)]
    origin: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    licence: String,
    #[serde(default)]
    usage_restrictions: String,
    #[serde(default)]
    keywords: String,
    #[serde(default)]
    then: Option<String>,
}

pub(super) async fn dataset_create(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<DatasetCreateForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let texto = text_or_null;
    let resultado = match Uuid::parse_str(form.workspace.trim()) {
        Ok(ws) => {
            api::post(
                &state,
                &member.session.access_token,
                &member.correlation_id,
                &format!("/api/v1/workspaces/{ws}/datasets"),
                &serde_json::json!({
                    "code": form.code.trim(),
                    "title": form.title.trim(),
                    "description": texto(&form.description),
                    "origin": texto(&form.origin),
                    "licence": texto(&form.licence),
                    "usage_restrictions": texto(&form.usage_restrictions),
                    "keywords": rs::split_keywords(&form.keywords),
                }),
            )
            .await
        }
        Err(_) => Err(ApiFailure::Rejected("sem ambiente".to_owned())),
    };
    match resultado {
        Ok(_) if form.then.as_deref() == Some("close") => {
            close_app(&state, &member, ApplicationId::Datasets)
        }
        Ok(v) => Redirect::to(&format!("/datasets/{}", text(&v, "id"))).into_response(),
        Err(ApiFailure::Unauthorised) => session_ended(&state, &headers),
        Err(e) => {
            let mut f = empty_dataset_form();
            f.origins = ds::origins(&form.origin);
            f.code = form.code;
            f.title = form.title;
            f.description = form.description;
            f.licence = form.licence;
            f.usage_restrictions = form.usage_restrictions;
            f.keywords = form.keywords;
            f.error = Some(match e {
                ApiFailure::Forbidden => AppError::PermissionDenied,
                ApiFailure::Conflict(_) => AppError::Conflict,
                _ => AppError::SaveFailed,
            });
            let q = ResQuery {
                workspace: Some(form.workspace),
                ..ResQuery::default()
            };
            datasets_view(
                &state,
                &headers,
                q,
                None,
                Some(f),
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await
        }
    }
}

#[derive(Deserialize)]
pub(super) struct VersionForm {
    #[serde(default)]
    label: String,
    #[serde(default)]
    provenance: String,
    #[serde(default)]
    notes: String,
}

pub(super) async fn dataset_version_create(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(dataset): Path<Uuid>,
    Form(form): Form<VersionForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let mut body = serde_json::json!({ "label": form.label.trim() });
    if !form.provenance.trim().is_empty() {
        body["provenance"] = Value::String(form.provenance.clone());
    }
    if !form.notes.trim().is_empty() {
        body["notes"] = Value::String(form.notes.clone());
    }
    let back = format!("/datasets/{dataset}");
    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/datasets/{dataset}/versions"),
        &body,
    )
    .await
    {
        Ok(v) => {
            Redirect::to(&format!("{back}?v={}", rs::encode(text(&v, "label")))).into_response()
        }
        Err(ApiFailure::Unauthorised) => session_ended(&state, &headers),
        Err(e) => Redirect::to(&format!("{back}?err={}", action_code(&e))).into_response(),
    }
}

/// Acrescentar um ficheiro a uma versão em rascunho. O Core guarda-o como
/// objecto do dataset e decide se a versão o aceita.
pub(super) async fn dataset_file_add(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path((dataset, version)): Path<(Uuid, Uuid)>,
    mut multipart: Multipart,
) -> Response {
    let member = member_or_login!(state, headers);
    let back = format!("/datasets/{dataset}");
    let mut ficheiro: Option<(String, String, Vec<u8>)> = None;
    let mut caminho = String::new();
    while let Ok(Some(field)) = multipart.next_field().await {
        match field.name() {
            Some("file") => {
                let nome = field.file_name().unwrap_or("ficheiro").to_owned();
                let tipo = field
                    .content_type()
                    .unwrap_or("application/octet-stream")
                    .to_owned();
                match field.bytes().await {
                    Ok(b) => ficheiro = Some((nome, tipo, b.to_vec())),
                    Err(_) => return Redirect::to(&format!("{back}?err=save")).into_response(),
                }
            }
            Some("path") => caminho = field.text().await.unwrap_or_default(),
            _ => {}
        }
    }
    let Some((nome, tipo, dados)) = ficheiro.filter(|(_, _, d)| !d.is_empty()) else {
        return Redirect::to(&format!("{back}?err=save")).into_response();
    };
    match api::upload_with_fields(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/datasets/{dataset}/versions/{version}/files"),
        nome,
        tipo,
        dados,
        vec![("path", caminho.trim().to_owned())],
    )
    .await
    {
        Ok(_) => Redirect::to(&back).into_response(),
        Err(ApiFailure::Unauthorised) => session_ended(&state, &headers),
        Err(e) => Redirect::to(&format!("{back}?err={}", action_code(&e))).into_response(),
    }
}

pub(super) async fn dataset_version_publish(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path((dataset, version)): Path<(Uuid, Uuid)>,
) -> Response {
    let member = member_or_login!(state, headers);
    let back = format!("/datasets/{dataset}");
    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/datasets/{dataset}/versions/{version}/publish"),
        &serde_json::json!({}),
    )
    .await
    {
        Ok(_) => Redirect::to(&back).into_response(),
        Err(ApiFailure::Unauthorised) => session_ended(&state, &headers),
        Err(e) => Redirect::to(&format!("{back}?err={}", action_code(&e))).into_response(),
    }
}

// ── Conhecimento ─────────────────────────────────────────────────────────

pub(super) async fn knowledge_root() -> Response {
    Redirect::to("/knowledge/documents").into_response()
}

/// A Bibliografia é a mesma vista de Conhecimento, na secção de fontes.
pub(super) async fn bibliography_entry() -> Response {
    Redirect::to("/knowledge/sources").into_response()
}

pub(super) async fn bibliography_new_entry() -> Response {
    Redirect::to("/knowledge/sources/new").into_response()
}

pub(super) async fn knowledge_section(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(section): Path<String>,
    Query(q): Query<ResQuery>,
) -> Response {
    let Some(s) = kn::section_of(&section) else {
        return not_found().await;
    };
    knowledge_view(&state, &headers, s, q, None, None, StatusCode::OK).await
}

pub(super) async fn knowledge_item(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path((section, item)): Path<(String, Uuid)>,
    Query(q): Query<ResQuery>,
) -> Response {
    let Some(s) = kn::section_of(&section) else {
        return not_found().await;
    };
    knowledge_view(&state, &headers, s, q, Some(item), None, StatusCode::OK).await
}

fn empty_source_form() -> SourceFormVm {
    SourceFormVm {
        action: "/knowledge/sources/new".to_owned(),
        workspaces: Vec::new(),
        types: kn::source_types("article"),
        rights: kn::rights("metadata_only"),
        title: String::new(),
        authors: String::new(),
        year: String::new(),
        container_title: String::new(),
        doi: String::new(),
        url: String::new(),
        error: None,
        cancel_href: "/knowledge/sources".to_owned(),
    }
}

pub(super) async fn source_new_page(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(q): Query<ResQuery>,
) -> Response {
    knowledge_view(
        &state,
        &headers,
        KnowledgeSection::Sources,
        q,
        None,
        Some(empty_source_form()),
        StatusCode::OK,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn knowledge_view(
    state: &WorkspaceState,
    headers: &HeaderMap,
    s: KnowledgeSection,
    q: ResQuery,
    open: Option<Uuid>,
    form: Option<SourceFormVm>,
    status: StatusCode,
) -> Response {
    let w = match open_app(state, headers, Screen::Knowledge, ApplicationId::Knowledge).await {
        Ok(w) => w,
        Err(r) => return *r,
    };
    let quem = caller(&w.member);
    let clock = clock_of(&w.ctx);
    let pergunta = q.q.clone().unwrap_or_default();
    let procura = pergunta.trim().chars().count() >= 2;
    let aberto = open.map(|o| o.to_string());
    let base = if procura {
        format!(
            "/knowledge/{}?q={}",
            kn::segment(s),
            rs::encode(pergunta.trim())
        )
    } else {
        format!("/knowledge/{}", kn::segment(s))
    };
    let (items, load, page) = match quem
        .get(
            state,
            &kn::list_path(s, Some(&pergunta), q.page.unwrap_or(1).max(1)),
        )
        .await
    {
        Ok(v) => (
            if procura {
                kn::hits(s, &items_of(&v), aberto.as_deref())
            } else {
                kn::items(s, &items_of(&v), aberto.as_deref())
            },
            AppLoad::Ready,
            rs::page(&v, &base),
        ),
        Err(ApiFailure::Unauthorised) => return session_ended(state, headers),
        Err(e) => (
            Vec::new(),
            AppLoad::Failed(app_error(&e)),
            AppPageVm::default(),
        ),
    };
    let mut source = None;
    let mut document = None;
    let mut open_error = None;
    if let Some(i) = aberto.as_deref() {
        let r = match s {
            KnowledgeSection::Sources => source_detail(state, &quem, i)
                .await
                .map(|x| source = Some(x)),
            KnowledgeSection::Documents => document_detail(state, &quem, i, &clock)
                .await
                .map(|x| document = Some(x)),
        };
        match r {
            Ok(()) => {}
            Err(ApiFailure::Unauthorised) => return session_ended(state, headers),
            Err(e) => open_error = Some(app_error(&e)),
        }
    }
    if let Some(e) = action_error(q.err.as_deref()) {
        open_error = Some(e);
        source = None;
        document = None;
    }
    let ambientes = if form.is_some() || s == KnowledgeSection::Sources {
        visible_workspaces(state, &quem).await
    } else {
        Vec::new()
    };
    let pode_criar = !creatable(&ambientes, None).is_empty();
    let form = form.map(|mut f| {
        f.workspaces = creatable(&ambientes, q.workspace.as_deref());
        f
    });
    let dirty = form
        .as_ref()
        .and_then(|_| dirty_for(&w, "knowledge", "know.new", "know.create"));
    let mut items = items;
    keep_filters(&mut items, &base.clone());
    let vm = KnowledgeVm {
        section: s,
        nav: kn::navigation(s),
        query: Some(pergunta),
        list: ResListVm {
            columns: kn::columns(s),
            items,
            load,
            page,
        },
        pane: pane(open.is_some() || form.is_some()),
        source,
        document,
        open_error,
        form,
        new_href: (s == KnowledgeSection::Sources && pode_criar)
            .then(|| "/knowledge/sources/new".to_owned()),
        list_href: base,
    };
    render_with_dirty(state, &w, status, ui::apps::knowledge::app(&vm), dirty)
}

async fn source_detail(
    state: &WorkspaceState,
    quem: &controllers::Caller<'_>,
    sid: &str,
) -> Result<SourceVm, ApiFailure> {
    let x = quem.get(state, &format!("/api/v1/sources/{sid}")).await?;
    let ov = quem
        .get(
            state,
            &format!("/api/v1/workspaces/{}", text(&x, "workspace_id")),
        )
        .await
        .ok();
    let direito = kn::content_right(text(&x, "content_right"))
        .ok_or_else(|| ApiFailure::Failed("base legal desconhecida".to_owned()))?;
    // O texto integral só aparece se o documento também se resolve para este
    // membro: a ligação da fonte não o abre.
    let integral = match rs::opt(&x, "full_text_document_id") {
        Some(did) => quem
            .get(state, &format!("/api/v1/documents/{did}"))
            .await
            .ok()
            .map(|d| ResLinkVm {
                kind: ResKind::Document,
                kind_label: None,
                title: text(&d, "title").to_owned(),
                meta: Some(kn::content_type(text(&d, "content_type"))),
                relation: None,
                by_operation: false,
                href: Some(format!("/knowledge/documents/{did}")),
            }),
        None => None,
    };
    Ok(SourceVm {
        source_type: t(kn::source_type_key(text(&x, "source_type"))).to_owned(),
        title: text(&x, "title").to_owned(),
        authors: x
            .get("authors")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default(),
        year: x.get("year").and_then(Value::as_i64).map(|y| y.to_string()),
        container_title: rs::opt(&x, "container_title"),
        publisher: rs::opt(&x, "publisher"),
        doi: rs::opt(&x, "doi"),
        isbn: rs::opt(&x, "isbn"),
        url: rs::opt(&x, "url"),
        abstract_text: rs::opt(&x, "abstract_text"),
        keywords: rs::keywords(&x),
        licence: rs::opt(&x, "licence"),
        content_right: direito,
        full_text: integral,
        citation_key: rs::opt(&x, "citation_key"),
        origin: rs::opt(&x, "origin"),
        classification: rs::classification(text(&x, "classification")),
        workspace: ov.as_ref().and_then(workspace_link),
        links: relations(state, quem, "source", sid).await,
        nye: Some(rs::nye("source", sid, "know.nye.source")),
    })
}

async fn document_detail(
    state: &WorkspaceState,
    quem: &controllers::Caller<'_>,
    did: &str,
    _clock: &controllers::desktop::Clock,
) -> Result<KnowledgeDocumentVm, ApiFailure> {
    let x = quem.get(state, &format!("/api/v1/documents/{did}")).await?;
    let ov = quem
        .get(
            state,
            &format!("/api/v1/workspaces/{}", text(&x, "workspace_id")),
        )
        .await
        .ok();
    Ok(KnowledgeDocumentVm {
        title: text(&x, "title").to_owned(),
        kind: t(kn::document_kind_key(text(&x, "kind"))).to_owned(),
        description: rs::opt(&x, "description"),
        date: rs::date(text(&x, "document_date")),
        filename: text(&x, "original_filename").to_owned(),
        content_type: kn::content_type(text(&x, "content_type")),
        size: kn::size(&x),
        checksum: kn::checksum(text(&x, "checksum_sha256")),
        classification: rs::classification(text(&x, "classification")),
        // O ficheiro institucional não tem ecrã em Ficheiros (que é pessoal):
        // sem elo. Os bytes saem pela descarga same-origin, que o Core volta a
        // autorizar no ficheiro.
        file: None,
        download_href: rs::opt(&x, "file_id").map(|f| format!("/files/{f}/download")),
        workspace: ov.as_ref().and_then(workspace_link),
        links: relations(state, quem, "document", did).await,
        nye: Some(rs::nye("document", did, "know.nye.document")),
    })
}

#[derive(Deserialize)]
pub(super) struct SourceCreateForm {
    #[serde(default)]
    workspace: String,
    #[serde(default)]
    source_type: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    authors: String,
    #[serde(default)]
    year: String,
    #[serde(default)]
    doi: String,
    #[serde(default)]
    container_title: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    content_right: String,
    #[serde(default)]
    then: Option<String>,
}

pub(super) async fn source_create(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<SourceCreateForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let texto = text_or_null;
    let autores: Vec<String> = form
        .authors
        .lines()
        .map(str::trim)
        .filter(|a| !a.is_empty())
        .map(str::to_owned)
        .collect();
    let resultado = match Uuid::parse_str(form.workspace.trim()) {
        Ok(ws) => {
            api::post(
                &state,
                &member.session.access_token,
                &member.correlation_id,
                &format!("/api/v1/workspaces/{ws}/sources"),
                &serde_json::json!({
                    "title": form.title.trim(),
                    "source_type": texto(&form.source_type),
                    "authors": autores,
                    "year": form.year.trim().parse::<i32>().ok(),
                    "doi": texto(&form.doi),
                    "container_title": texto(&form.container_title),
                    "url": texto(&form.url),
                    "content_right": texto(&form.content_right),
                }),
            )
            .await
        }
        Err(_) => Err(ApiFailure::Rejected("sem ambiente".to_owned())),
    };
    match resultado {
        Ok(_) if form.then.as_deref() == Some("close") => {
            close_app(&state, &member, ApplicationId::Knowledge)
        }
        Ok(v) => Redirect::to(&format!("/knowledge/sources/{}", text(&v, "id"))).into_response(),
        Err(ApiFailure::Unauthorised) => session_ended(&state, &headers),
        Err(e) => {
            let mut f = empty_source_form();
            f.types = kn::source_types(&form.source_type);
            f.rights = kn::rights(&form.content_right);
            f.title = form.title;
            f.authors = form.authors;
            f.year = form.year;
            f.doi = form.doi;
            f.container_title = form.container_title;
            f.url = form.url;
            f.error = Some(match e {
                ApiFailure::Forbidden => AppError::PermissionDenied,
                _ => AppError::SaveFailed,
            });
            let q = ResQuery {
                workspace: Some(form.workspace),
                ..ResQuery::default()
            };
            knowledge_view(
                &state,
                &headers,
                KnowledgeSection::Sources,
                q,
                None,
                Some(f),
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await
        }
    }
}

// ── Endereços antigos ────────────────────────────────────────────────────

/// Um ambiente abre no seu projecto, ou na sua ideia.
pub(super) async fn workspace_entry(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(workspace): Path<Uuid>,
) -> Response {
    let member = member_or_login!(state, headers);
    match caller(&member)
        .get(&state, &format!("/api/v1/workspaces/{workspace}"))
        .await
    {
        Ok(ov) => match workspace_link(&ov).and_then(|l| l.href) {
            Some(h) => Redirect::to(&h).into_response(),
            None => not_found().await,
        },
        Err(ApiFailure::Unauthorised) => session_ended(&state, &headers),
        Err(_) => not_found().await,
    }
}

/// Um projecto nasce da promoção de uma ideia: «Novo projecto» leva às ideias
/// candidatas, onde a promoção acontece.
pub(super) async fn project_new_entry() -> Response {
    Redirect::to("/ideas?nav=candidates").into_response()
}
