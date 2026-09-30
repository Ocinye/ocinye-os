//! D007.1 · Monitor de Actividade, Resultados e Lixo no Workspace, cada um
//! numa janela gerida (D002) com o ecrã do Design.
//!
//! Aqui só há canal: cada página relê do Core, com a sessão do membro, o que
//! mostra. O Monitor segue a regra do Core de `/system/operations`; um
//! resultado e cada uma das suas ligações relêem-se; o Lixo é o do próprio, e
//! restaurar é a operação do domínio que já existia.

use super::org::render_org;
use super::productivity::{clock_of, open_app, AppWindow};
use super::*;
use crate::controllers::desktop::text;
use crate::controllers::ops as op;
use crate::controllers::productivity::app_error;
use crate::controllers::reg;
use crate::controllers::research as rs;
use crate::i18n::t;
use crate::ui::view_models::{
    AppError, AppLoad, AppPageVm, MonitorVm, ResKind, ResLinkVm, ResListVm, ResOptionVm, ResPane,
    ResultVm, ResultsVm, TrashKind, TrashNotice, TrashRefusal, TrashVm,
};
use ocinye_contracts::ApplicationId;

fn pane(open: bool) -> ResPane {
    if open {
        ResPane::Detail
    } else {
        ResPane::List
    }
}

// ═════════════════════════════════════════════════════════════════════════
// Monitor de Actividade
// ═════════════════════════════════════════════════════════════════════════

#[derive(Deserialize, Default)]
pub(super) struct MonitorQuery {
    #[serde(default)]
    plane: Option<String>,
}

/// `GET /admin/monitor` — consumo e estado operacional, só com a administração
/// da plataforma (a regra do Core de `/system/operations`).
pub(super) async fn monitor_page(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(q): Query<MonitorQuery>,
) -> Response {
    let w = match open_app(&state, &headers, Screen::Monitor, ApplicationId::Monitor).await {
        Ok(w) => w,
        Err(r) => return *r,
    };
    let quem = caller(&w.member);
    let clock = clock_of(&w.ctx);
    let plane = reg::plane_of(q.plane.as_deref());
    let caps = &w.ctx.viewer.capabilities;
    let holds = |p: &str| caps.iter().any(|c| c == p);
    let mut vm = MonitorVm {
        load: AppLoad::Ready,
        error: None,
        read_at: Some(
            clock
                .now
                .with_timezone(&clock.zone.zone())
                .format("%H:%M:%S")
                .to_string(),
        ),
        refresh_href: format!("/admin/monitor?plane={}", plane.id()),
        summary: None,
        planes: reg::REPORTED
            .into_iter()
            .map(|p| (p, format!("/admin/monitor?plane={}", p.id())))
            .collect(),
        unsupported: reg::UNREPORTED.to_vec(),
        plane: Some(plane),
        samples: Vec::new(),
        // Sem inventário tipado de serviços no runtime: a secção diz o que
        // falta, e não há «Parar» (MON-06…MON-09).
        services: None,
        receipt: None,
        refusal: None,
        ai_href: holds("ai.use").then(|| "/ai?nav=providers".to_owned()),
        apps_href: holds("members.manage").then(|| "/admin/instance".to_owned()),
    };
    match quem.get(&state, "/api/v1/system/operations").await {
        Ok(ops) => vm.summary = Some(reg::summary(&ops)),
        Err(ApiFailure::Forbidden | ApiFailure::Denied) => {
            vm.error = Some(AppError::PermissionDenied);
            return render_org(
                &state,
                &w,
                StatusCode::FORBIDDEN,
                ui::apps::monitor::app(&vm),
                None,
                None,
            );
        }
        Err(f) => vm.load = AppLoad::Failed(app_error(&f)),
    }
    if let Ok(nodes) = quem.get(&state, "/api/v1/compute/nodes").await {
        vm.samples = reg::samples(&op::list_of(&nodes), plane, &clock);
    }
    render_org(
        &state,
        &w,
        StatusCode::OK,
        ui::apps::monitor::app(&vm),
        None,
        None,
    )
}

// ═════════════════════════════════════════════════════════════════════════
// Resultados
// ═════════════════════════════════════════════════════════════════════════

#[derive(Deserialize, Default)]
pub(super) struct ResultsQuery {
    #[serde(default)]
    workspace: Option<String>,
    #[serde(default)]
    status: Option<String>,
}

/// Os ambientes que o membro lê: (id, título).
async fn readable_workspaces(state: &WorkspaceState, w: &AppWindow) -> Vec<(String, String)> {
    caller(&w.member)
        .get(state, "/api/v1/workspaces?page_size=100")
        .await
        .map(|v| {
            op::list_of(&v)
                .iter()
                .map(|x| (text(x, "id").to_owned(), text(x, "title").to_owned()))
                .collect()
        })
        .unwrap_or_default()
}

/// A lista: os resultados dos ambientes que o membro lê, cada ambiente pela
/// sua leitura autorizada (o Core já não devolve um resultado acima da
/// classificação que o membro lê). Não há lista entre ambientes no Core
/// (RES-01): a leitura é limitada aos ambientes da primeira página.
async fn results_vm(
    state: &WorkspaceState,
    w: &AppWindow,
    q: &ResultsQuery,
    open: Option<&str>,
) -> ResultsVm {
    let clock = clock_of(&w.ctx);
    let readable = readable_workspaces(state, w).await;
    let ws = q
        .workspace
        .as_deref()
        .filter(|id| readable.iter().any(|(r, _)| r == id));
    let status = q.status.as_deref().filter(|s| reg::STATES.contains(s));
    let targets: Vec<(String, String)> = readable
        .iter()
        .filter(|(id, _)| ws.is_none_or(|x| x == id))
        .cloned()
        .collect();
    let mut set = tokio::task::JoinSet::new();
    for (id, title) in targets {
        let state = state.clone();
        let token = w.member.session.access_token.clone();
        let corr = w.member.correlation_id.clone();
        set.spawn(async move {
            let v = api::get::<Value>(
                &state,
                &token,
                &corr,
                &format!("/api/v1/workspaces/{id}/results"),
            )
            .await
            .map(|v| op::list_of(&v))
            .unwrap_or_default();
            (title, v)
        });
    }
    let mut rows: Vec<(Value, String)> = Vec::new();
    while let Some(Ok((title, list))) = set.join_next().await {
        rows.extend(list.into_iter().map(|r| (r, title.clone())));
    }
    rows.retain(|(r, _)| status.is_none_or(|s| text(r, "status") == s));
    rows.sort_by(|(a, _), (b, _)| text(b, "created_at").cmp(text(a, "created_at")));
    let items = rows
        .iter()
        .filter_map(|(r, ws)| reg::result_item(r, Some(ws), open, &clock))
        .collect();
    let base = |s: Option<&str>| {
        let mut p: Vec<String> = Vec::new();
        if let Some(w) = ws {
            p.push(format!("workspace={}", rs::encode(w)));
        }
        if let Some(s) = s {
            p.push(format!("status={s}"));
        }
        if p.is_empty() {
            "/results".to_owned()
        } else {
            format!("/results?{}", p.join("&"))
        }
    };
    let mut nav = vec![rs::nav(
        "results.nav.all",
        "results",
        base(None),
        status.is_none(),
    )];
    for s in reg::STATES {
        let st = reg::result_status(s).map(crate::ui::apps::results::result_state);
        if let Some(st) = st {
            nav.push(rs::nav(st.key, "results", base(Some(s)), status == Some(s)));
        }
    }
    let mut workspaces = vec![ResOptionVm {
        value: String::new(),
        label: t("res.filter.all").to_owned(),
        selected: ws.is_none(),
    }];
    workspaces.extend(readable.iter().map(|(id, n)| ResOptionVm {
        value: id.clone(),
        label: n.clone(),
        selected: ws == Some(id.as_str()),
    }));
    ResultsVm {
        nav,
        workspaces,
        filter_action: "/results".to_owned(),
        list: ResListVm {
            columns: reg::RESULT_COLUMNS.to_vec(),
            items,
            load: AppLoad::Ready,
            page: AppPageVm {
                more_href: None,
                summary: None,
            },
        },
        pane: pane(open.is_some()),
        list_href: base(status),
        result: None,
        result_error: None,
    }
}

/// `GET /results`
pub(super) async fn results_page(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(q): Query<ResultsQuery>,
) -> Response {
    let w = match open_app(&state, &headers, Screen::Results, ApplicationId::Results).await {
        Ok(w) => w,
        Err(r) => return *r,
    };
    let vm = results_vm(&state, &w, &q, None).await;
    render_org(
        &state,
        &w,
        StatusCode::OK,
        ui::apps::results::app(&vm),
        None,
        None,
    )
}

/// Uma ligação canónica relida com a autoridade de quem vê; o que não se lê
/// não chega à vista.
fn link(
    kind: ResKind,
    kind_label: Option<String>,
    title: String,
    href: Option<String>,
) -> ResLinkVm {
    ResLinkVm {
        kind,
        kind_label,
        title,
        meta: None,
        relation: None,
        by_operation: false,
        href,
    }
}

/// `GET /results/{result_id}` — o resultado, as validações e as ligações,
/// cada uma pela sua leitura.
pub(super) async fn result_page(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(result_id): Path<Uuid>,
    Query(q): Query<ResultsQuery>,
) -> Response {
    let w = match open_app(&state, &headers, Screen::Results, ApplicationId::Results).await {
        Ok(w) => w,
        Err(r) => return *r,
    };
    let id = result_id.to_string();
    let mut vm = results_vm(&state, &w, &q, Some(&id)).await;
    let quem = caller(&w.member);
    let clock = clock_of(&w.ctx);
    match quem.get(&state, &format!("/api/v1/results/{id}")).await {
        Ok(r) => match reg::result_status(text(&r, "status")) {
            Some(status) => {
                let ws_id = text(&r, "workspace_id");
                let workspace = if ws_id.is_empty() {
                    None
                } else {
                    quem.get(&state, &format!("/api/v1/workspaces/{ws_id}"))
                        .await
                        .ok()
                        .map(|x| {
                            let x = x.get("workspace").cloned().unwrap_or(x);
                            link(
                                ResKind::Other,
                                Some(t("results.kind.workspace").to_owned()),
                                text(&x, "title").to_owned(),
                                None,
                            )
                        })
                };
                let ex_id = text(&r, "execution_id");
                let execution = if ex_id.is_empty() {
                    None
                } else {
                    quem.get(&state, &format!("/api/v1/executions/{ex_id}"))
                        .await
                        .ok()
                        .map(|x| {
                            link(
                                ResKind::Other,
                                Some(t("results.kind.execution").to_owned()),
                                reg::execution_label(&x),
                                None,
                            )
                        })
                };
                let sup = text(&r, "superseded_by_id");
                let superseded_by = if sup.is_empty() {
                    None
                } else {
                    quem.get(&state, &format!("/api/v1/results/{sup}"))
                        .await
                        .ok()
                        .map(|x| {
                            link(
                                ResKind::Other,
                                Some(t("prod.res.kind.result").to_owned()),
                                text(&x, "title").to_owned(),
                                Some(format!("/results/{sup}")),
                            )
                        })
                };
                let validations = quem
                    .get(&state, &format!("/api/v1/results/{id}/validations"))
                    .await
                    .map(|v| {
                        op::list_of(&v)
                            .iter()
                            .filter_map(|x| reg::validation(x, &clock))
                            .collect()
                    })
                    .unwrap_or_default();
                let lineage = super::research::relations(&state, &quem, "result", &id).await;
                let created = crate::controllers::desktop::instant(&r, "created_at")
                    .map(|at| rs::day(at, &clock))
                    .unwrap_or_default();
                vm.result = Some(ResultVm {
                    title: text(&r, "title").to_owned(),
                    status,
                    class: rs::classification(text(&r, "classification")),
                    summary: text(&r, "summary").to_owned(),
                    workspace,
                    project: None,
                    execution,
                    superseded_by,
                    created_by: rs::opt(&r, "created_by_name"),
                    updated: crate::controllers::desktop::instant(&r, "updated_at")
                        .map(|at| rs::day(at, &clock))
                        .unwrap_or_else(|| created.clone()),
                    created,
                    validations,
                    lineage,
                    // Registar uma validação precisa de um formulário que o
                    // Design ainda não desenhou: sem destino, não há botão
                    // (RES-04), mesmo quando o Core diria que sim.
                    validate_href: None,
                    nye: Some(rs::nye("result", &id, "results.nye")),
                });
            }
            None => vm.result_error = Some(AppError::Unavailable),
        },
        Err(f) => vm.result_error = Some(app_error(&f)),
    }
    let status = match vm.result_error {
        Some(AppError::NotFound) => StatusCode::NOT_FOUND,
        Some(AppError::PermissionDenied) => StatusCode::FORBIDDEN,
        _ => StatusCode::OK,
    };
    render_org(&state, &w, status, ui::apps::results::app(&vm), None, None)
}

// ═════════════════════════════════════════════════════════════════════════
// Lixo
// ═════════════════════════════════════════════════════════════════════════

#[derive(Deserialize, Default)]
pub(super) struct TrashQuery {
    #[serde(default)]
    nav: Option<String>,
    #[serde(default)]
    open: Option<String>,
    #[serde(default)]
    done: Option<String>,
    #[serde(default)]
    refused: Option<String>,
    /// O item restaurado (`file:<id>`/`note:<id>`), para dizer o nome.
    #[serde(default)]
    item: Option<String>,
}

/// `GET /trash` — o Lixo pessoal do membro: os seus ficheiros e notas
/// apagados, cada lista do Core (só do dono).
pub(super) async fn trash_page(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(q): Query<TrashQuery>,
) -> Response {
    let w = match open_app(&state, &headers, Screen::Trash, ApplicationId::Trash).await {
        Ok(w) => w,
        Err(r) => return *r,
    };
    let quem = caller(&w.member);
    let clock = clock_of(&w.ctx);
    let section = reg::section_of(q.nav.as_deref());
    let files = quem.get(&state, "/api/v1/me/files/trash?limit=200").await;
    let notes = quem
        .get(&state, "/api/v1/me/deleted-notes?page_size=100")
        .await;
    let load = match (&files, &notes) {
        (Err(f), _) | (_, Err(f)) => AppLoad::Failed(app_error(f)),
        _ => AppLoad::Ready,
    };
    let mut items: Vec<crate::ui::view_models::TrashItemVm> = Vec::new();
    let fl = files.as_ref().map(op::list_of).unwrap_or_default();
    let nl = notes.as_ref().map(op::list_of).unwrap_or_default();
    items.extend(
        fl.iter()
            .map(|v| reg::trash_item(TrashKind::File, v, &clock)),
    );
    items.extend(
        nl.iter()
            .map(|v| reg::trash_item(TrashKind::Note, v, &clock)),
    );
    // A ordem é a do instante em que foi apagado, que o Core devolve.
    let when = |kind: TrashKind, id: &str| -> String {
        let src = match kind {
            TrashKind::File => &fl,
            TrashKind::Note => &nl,
        };
        src.iter()
            .find(|v| text(v, "id") == id)
            .map(|v| text(v, "deleted_at").to_owned())
            .unwrap_or_default()
    };
    items.sort_by_key(|i| std::cmp::Reverse(when(i.kind, &i.id)));
    let open = q.open.as_deref();
    let item = open.and_then(|o| items.iter().find(|i| reg::trash_key(i) == o).cloned());
    let visible: Vec<_> = items
        .iter()
        .filter(|i| match section {
            reg::TrashSection::All => true,
            reg::TrashSection::Files => i.kind == TrashKind::File,
            reg::TrashSection::Notes => i.kind == TrashKind::Note,
        })
        .collect();
    let notice = (q.done.as_deref() == Some("restored")).then_some(TrashNotice::Restored);
    // O nome do que se restaurou, lido de volta pela sua leitura (o endereço
    // nunca traz o nome).
    let notice_name = match (notice, q.item.as_deref().and_then(|x| x.split_once(':'))) {
        (Some(_), Some(("file", id))) if Uuid::parse_str(id).is_ok() => quem
            .get(&state, &format!("/api/v1/me/files/{id}"))
            .await
            .ok()
            .map(|v| text(&v, "name").to_owned()),
        (Some(_), Some(("note", id))) if Uuid::parse_str(id).is_ok() => quem
            .get(&state, &format!("/api/v1/me/notes/{id}"))
            .await
            .ok()
            .map(|v| text(&v, "title").to_owned()),
        _ => None,
    };
    let vm = TrashVm {
        nav: reg::trash_nav(section, fl.len(), nl.len()),
        list: ResListVm {
            columns: reg::TRASH_COLUMNS.to_vec(),
            items: visible
                .iter()
                .map(|i| reg::trash_row(i, section, open))
                .collect(),
            load,
            page: AppPageVm {
                more_href: None,
                summary: None,
            },
        },
        pane: pane(item.is_some() || (open.is_some())),
        list_href: reg::trash_href(section, None),
        item_error: (open.is_some() && item.is_none()).then_some(AppError::NotFound),
        item,
        notice,
        notice_name: notice_name.filter(|n| !n.is_empty()),
        refusal: match q.refused.as_deref() {
            Some("gone") => Some(TrashRefusal::Gone),
            Some("denied") => Some(TrashRefusal::Denied),
            Some("conflict") => Some(TrashRefusal::Conflict),
            _ => None,
        },
    };
    let status = if vm.item_error.is_some() {
        StatusCode::NOT_FOUND
    } else {
        StatusCode::OK
    };
    render_org(&state, &w, status, ui::apps::trash::app(&vm), None, None)
}

#[derive(Deserialize)]
pub(super) struct TrashRestoreForm {
    id: Uuid,
}

fn restored(kind: &str, id: Uuid, r: Result<Value, ApiFailure>) -> Response {
    match r {
        Ok(_) => Redirect::to(&format!("/trash?done=restored&item={kind}:{id}")).into_response(),
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        Err(ApiFailure::Denied) => Redirect::to("/trash?refused=gone").into_response(),
        Err(ApiFailure::Forbidden) => Redirect::to("/trash?refused=denied").into_response(),
        Err(ApiFailure::Conflict(_) | ApiFailure::Refused { .. }) => {
            Redirect::to("/trash?refused=conflict").into_response()
        }
        Err(f) => super::failure_response(&f),
    }
}

/// `POST /trash/files/restore` — a operação de ficheiros que já existia; o
/// Core decide (só o dono, só o que está no Lixo).
pub(super) async fn trash_restore_file(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<TrashRestoreForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    // A versão corrente, lida do Lixo do próprio antes de restaurar: é por
    // ela que o nome se relê depois (o endereço nunca leva o nome).
    let version = api::get::<Value>(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        "/api/v1/me/files/trash?limit=500",
    )
    .await
    .ok()
    .and_then(|v| {
        op::list_of(&v)
            .iter()
            .find(|x| text(x, "id") == form.id.to_string())
            .and_then(|x| Uuid::parse_str(text(x, "version_id")).ok())
    });
    let r = api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        "/api/v1/me/files/restore",
        &serde_json::json!({ "file_id": form.id }),
    )
    .await;
    restored("file", version.unwrap_or(form.id), r)
}

/// `POST /trash/notes/restore` — a operação de notas que já existia.
pub(super) async fn trash_restore_note(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<TrashRestoreForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let r = api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/me/notes/{}/restore", form.id),
        &serde_json::json!({}),
    )
    .await;
    restored("note", form.id, r)
}
