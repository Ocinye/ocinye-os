//! D004 · As aplicações de produtividade no Workspace: Notas, Ficheiros,
//! Calendário e Correio, cada uma numa janela gerida (D002) com o ecrã do
//! Design (`ui::apps`).
//!
//! Aqui só há canal: cada página pede ao Core, com a sessão do membro, o que
//! mostra, e passa-o pelos controladores `controllers::productivity` para os
//! ViewModels do Design. A autoridade é do Core; o que ele recusa aparece como
//! erro tipado, nunca como conteúdo.

use leptos::prelude::*;

use super::*;
use crate::controllers::productivity::{app_error, note_markdown, notes};
use crate::i18n::t;
use crate::ui::view_models::{
    AppError, AppLoad, AppPageVm, AppSaveState, DirtyCloseVm, NotesVm, WindowContent,
};

/// Uma aplicação aberta na sua janela, pronta para desenhar.
pub(super) struct AppWindow {
    pub(super) member: Member,
    pub(super) ctx: Box<controllers::ShellContext>,
    pub(super) page: controllers::windows::PageQuery,
    pub(super) title: String,
    /// A janela que mostra esta rota, quando a mesa a tem.
    pub(super) window: Option<String>,
}

/// O membro desta sessão, ou a resposta que o leva a entrar (a mesma regra de
/// `member_or_login!`, numa função que devolve `Result`).
fn member_or_response(
    state: &WorkspaceState,
    headers: &HeaderMap,
) -> Result<Member, Box<Response>> {
    match current_member(state, headers) {
        Some(m) if m.session.must_change_password => {
            Err(Box::new(Redirect::to("/first-access").into_response()))
        }
        Some(m) if m.session.mfa_required => Err(Box::new(Redirect::to("/mfa").into_response())),
        Some(m) => Ok(m),
        None => Err(Box::new(
            Redirect::to(destino_de_entrada(headers)).into_response(),
        )),
    }
}

/// Abre (ou foca) a janela da aplicação para o endereço pedido, depois do
/// portão da aplicação. Com `?frame=1` não mexe na mesa: só se desenha o corpo.
pub(super) async fn open_app(
    state: &WorkspaceState,
    headers: &HeaderMap,
    screen: Screen,
    app: ApplicationId,
) -> Result<AppWindow, Box<Response>> {
    let member = member_or_response(state, headers)?;
    let quem = caller(&member);
    let title = screen.label().to_owned();
    let href = screen.path();
    let mut ctx = match controllers::shell(state, &quem, href, title.clone()).await {
        Shell::Ready(ctx) => ctx,
        Shell::SignIn => return Err(Box::new(session_ended(state, headers))),
        Shell::Indeterminate(reference) => {
            return Err(Box::new(
                identity_indeterminate(state, reference, href).await,
            ))
        }
    };
    if !screen_open(&ctx, screen) {
        let vm = ErrorVm {
            kind: ErrorKind::NotFound,
            reference: None,
            retry_href: None,
        };
        return Err(Box::new(
            (
                StatusCode::NOT_FOUND,
                html(
                    error_title(vm.kind),
                    Surface::Shell,
                    ui::screens::error::in_shell(&ctx.vm, &vm),
                ),
            )
                .into_response(),
        ));
    }
    let page = controllers::windows::page_query(href);
    let mut window = None;
    if !page.frame {
        let opened = controllers::windows::open(
            &state.sessions,
            &member.session_id,
            app,
            &page.href,
            page.new_window,
        );
        match opened {
            Some(Err(window_manager::WmError::TooMany)) => {
                return Err(Box::new(
                    (StatusCode::CONFLICT, Redirect::to("/")).into_response(),
                ));
            }
            Some(Ok(o)) => window = Some(o.id().to_owned()),
            _ => {}
        }
        if page.new_window {
            return Err(Box::new(Redirect::to(&page.href).into_response()));
        }
    } else {
        window = state
            .sessions
            .with_desk(&member.session_id, |d| {
                d.windows()
                    .iter()
                    .find(|w| w.app == app && w.href == page.href)
                    .map(|w| w.id.clone())
            })
            .flatten();
    }
    ctx.vm.wm = controllers::windows::view(&state.sessions, &member.session_id, &ctx);
    // Só a janela deste pedido recebe o corpo; as outras da mesma aplicação
    // (Ficheiros, Notas e Correio abrem várias) carregam o seu por `?frame=1`.
    // O corpo vai para a primeira janela `Ready` — marcar todas punha o
    // conteúdo de uma janela noutra.
    if let (Some(wm), Some(id)) = (ctx.vm.wm.as_mut(), window.as_deref()) {
        if let Some(w) = wm.windows.iter_mut().find(|w| w.id == id) {
            w.content = WindowContent::Ready;
        }
    }
    Ok(AppWindow {
        member,
        ctx,
        page,
        title,
        window,
    })
}

/// Desenha a aplicação: só o corpo com `?frame=1`, senão a casca com a janela.
/// `extra` é marcação inerte que acompanha o corpo (o modelo do fecho com
/// alterações); `dirty` é o diálogo do fecho pedido pelo endereço (`?close=`).
pub(super) fn render_app(
    state: &WorkspaceState,
    w: &AppWindow,
    status: StatusCode,
    content: AnyView,
    extra: Option<AnyView>,
    dirty: Option<DirtyCloseVm>,
) -> Response {
    if w.page.frame {
        let title = w.title.clone();
        let body = view! {
            <template data-part="win-title">{title}</template>
            {content}
            {extra}
        };
        return (
            status,
            [(header::CACHE_CONTROL, "no-store")],
            Html(body.to_html()),
        )
            .into_response();
    }
    let dialog = w
        .page
        .close
        .as_deref()
        .and_then(|id| {
            dirty.filter(|d| d.window_id == id).or_else(|| {
                controllers::windows::dirty_close(&state.sessions, &w.member.session_id, id)
            })
        })
        .map(|d| dirty_dialog(&d));
    let engine = w.ctx.vm.wm.is_some();
    let body = ui::shell::shell_with_window(
        &w.ctx.vm,
        ().into_any(),
        Some(view! { {content}{extra} }.into_any()),
    );
    let mut response = shell_page(&w.title, engine, body, dialog);
    *response.status_mut() = status;
    response
}

/// O relógio do membro para formatar datas e horas na sua zona.
pub(super) fn clock_of(ctx: &controllers::ShellContext) -> controllers::desktop::Clock {
    controllers::desktop::Clock {
        now: chrono::Utc::now(),
        zone: ctx.zone,
        core_ok: ctx.core.operational(),
        is_admin: ctx.is_admin,
    }
}

/// O modelo do fecho com alterações para uma janela de documento: o diálogo
/// D002 (`wm::dirty_close`), num `<template>` inerte que o `oc-apps.js` monta no
/// sítio global de sempre, depois da casca. «Guardar» submete o formulário da
/// aplicação (`save_form`), com `then=close`.
pub(super) fn dirty_template(vm: &DirtyCloseVm) -> AnyView {
    let win = vm.window_id.clone();
    let dialog = ui::wm::dirty_close(vm);
    view! { <template data-part="app-dirty" data-win=win>{dialog}</template> }.into_any()
}

// ── Notas ────────────────────────────────────────────────────────────────

#[derive(Deserialize, Default)]
pub(super) struct NotesQuery {
    #[serde(default)]
    q: Option<String>,
    #[serde(default)]
    saved: Option<String>,
}

/// O que a página de Notas mostra no editor, além da nota lida do Core.
struct EditorState {
    save: AppSaveState,
    pending: Option<notes::Pending>,
}

async fn notes_page(
    state: &WorkspaceState,
    headers: &HeaderMap,
    section: notes::Section,
    note: Option<Uuid>,
    query: NotesQuery,
    editor_state: Option<EditorState>,
    status: StatusCode,
) -> Response {
    let w = match open_app(state, headers, Screen::Notes, ApplicationId::Notes).await {
        Ok(w) => w,
        Err(response) => return *response,
    };
    let quem = caller(&w.member);
    let clock = clock_of(&w.ctx);
    let active = note.map(|n| n.to_string());
    let q = query.q.clone().unwrap_or_default();
    let q = q.trim().to_owned();

    // A lista: a secção, ou a pesquisa em Notas (as do próprio membro).
    let (items, load) = if q.is_empty() || section != notes::Section::Mine {
        match quem.get(state, section.api()).await {
            Ok(v) => (
                notes::items(
                    v.as_array().map_or(&[][..], Vec::as_slice),
                    section,
                    active.as_deref(),
                    &clock,
                ),
                AppLoad::Ready,
            ),
            Err(ApiFailure::Unauthorised) => return session_ended(state, headers),
            Err(f) => (Vec::new(), AppLoad::Failed(app_error(&f))),
        }
    } else {
        let path = format!(
            "/api/v1/search?q={}&entity_types=note&page_size=100",
            urlencoding_minimal(&q)
        );
        match quem.get(state, &path).await {
            Ok(v) => (
                notes::search_items(
                    v.get("items")
                        .and_then(Value::as_array)
                        .map_or(&[][..], Vec::as_slice),
                    active.as_deref(),
                ),
                AppLoad::Ready,
            ),
            Err(ApiFailure::Unauthorised) => return session_ended(state, headers),
            Err(f) => (Vec::new(), AppLoad::Failed(app_error(&f))),
        }
    };

    // A nota aberta, relida do Core agora: uma nota que o membro deixou de
    // poder ver não aparece (nem o que estava no ecrã antes).
    let (editor, editor_error) = match note {
        None => (None, None),
        Some(id) => match quem.get(state, &format!("/api/v1/me/notes/{id}")).await {
            Ok(view) => {
                let (save, pending) = match editor_state {
                    Some(s) => (s.save, s.pending),
                    None if query.saved.is_some() => {
                        (AppSaveState::Saved(clock.hhmm(clock.now)), None)
                    }
                    None => (AppSaveState::Clean, None),
                };
                match notes::editor(&view, save, pending) {
                    Some(e) => (Some(e), None),
                    None => (None, Some(AppError::NotFound)),
                }
            }
            Err(ApiFailure::Unauthorised) => return session_ended(state, headers),
            Err(f) => (None, Some(app_error(&f))),
        },
    };

    let dirty = editor
        .as_ref()
        .zip(w.window.as_ref())
        .map(|(e, win)| DirtyCloseVm {
            window_id: win.clone(),
            title: if e.title.trim().is_empty() {
                t("notes.untitled").to_owned()
            } else {
                e.title.clone()
            },
            can_save: !e.read_only,
            save_label: None,
            save_form: (!e.read_only).then(|| ui::apps::doc_form_id("notes", &e.id)),
        });
    let vm = NotesVm {
        nav: notes::nav(section),
        query: q,
        items,
        load,
        page: AppPageVm::default(),
        editor,
        editor_error,
        create_action: (section == notes::Section::Mine).then(|| "/notes/new".to_owned()),
    };
    let template = dirty.as_ref().map(dirty_template);
    render_app(
        state,
        &w,
        status,
        ui::apps::notes::app(&vm),
        template,
        dirty,
    )
}

/// `GET /notes`: as notas do membro (ou a pesquisa em Notas).
pub(super) async fn notes_list(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(query): Query<NotesQuery>,
) -> Response {
    notes_page(
        &state,
        &headers,
        notes::Section::Mine,
        None,
        query,
        None,
        StatusCode::OK,
    )
    .await
}

/// `GET /notes/partilhadas`: as que outra pessoa partilhou com o membro.
pub(super) async fn shared_notes_page(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(query): Query<NotesQuery>,
) -> Response {
    notes_page(
        &state,
        &headers,
        notes::Section::Shared,
        None,
        query,
        None,
        StatusCode::OK,
    )
    .await
}

/// `GET /notes/lixo`: as apagadas.
pub(super) async fn notes_trash_page(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(query): Query<NotesQuery>,
) -> Response {
    notes_page(
        &state,
        &headers,
        notes::Section::Trash,
        None,
        query,
        None,
        StatusCode::OK,
    )
    .await
}

/// `GET /notes/{id}`: a nota aberta no editor, com a lista ao lado.
pub(super) async fn note_editor(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(note_id): Path<Uuid>,
    Query(query): Query<NotesQuery>,
) -> Response {
    notes_page(
        &state,
        &headers,
        notes::Section::Mine,
        Some(note_id),
        query,
        None,
        StatusCode::OK,
    )
    .await
}

/// O formulário do editor do Design (`NoteEditorVm`): título, texto, a revisão
/// em que abriu e, vindo do fecho com alterações, `then=close`.
#[derive(Deserialize)]
pub(super) struct NoteSaveForm {
    #[serde(default)]
    title: String,
    #[serde(default)]
    body: String,
    base_revision: i32,
    #[serde(default)]
    then: Option<String>,
}

/// `POST /notes/{id}/gravar`: grava o texto como documento, na revisão em que
/// o editor abriu. Outra revisão entretanto: conflito tipado, e o texto do
/// membro volta ao editor tal como estava — nada se sobrepõe em silêncio.
pub(super) async fn save_personal_note(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(note_id): Path<Uuid>,
    Form(form): Form<NoteSaveForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let title = if form.title.trim().is_empty() {
        t("notes.untitled").to_owned()
    } else {
        form.title.trim().to_owned()
    };
    let body = serde_json::json!({
        "title": title,
        "base_revision": form.base_revision,
        "document": note_markdown::from_markdown(&form.body),
    });
    let result = api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/me/notes/{note_id}"),
        &body,
    )
    .await;
    match result {
        Ok(_) if form.then.as_deref() == Some("close") => {
            // Gravado: a janela desta nota fica limpa e fecha, como o membro
            // confirmou no diálogo do gestor de janelas.
            let path = format!("/notes/{note_id}");
            let next = state
                .sessions
                .with_desk(&member.session_id, |d| {
                    let ids: Vec<String> = d
                        .windows()
                        .iter()
                        .filter(|w| {
                            w.app == ApplicationId::Notes
                                && w.href.split('?').next() == Some(path.as_str())
                        })
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
        Ok(_) => Redirect::to(&format!("/notes/{note_id}?saved=1")).into_response(),
        Err(ApiFailure::Unauthorised) => session_ended(&state, &headers),
        Err(failure) => {
            let status = match failure {
                ApiFailure::Conflict(_) => StatusCode::CONFLICT,
                ApiFailure::Rejected(_) => StatusCode::UNPROCESSABLE_ENTITY,
                ApiFailure::Denied => StatusCode::NOT_FOUND,
                ApiFailure::Forbidden => StatusCode::FORBIDDEN,
                _ => StatusCode::BAD_GATEWAY,
            };
            let err = match app_error(&failure) {
                AppError::NotFound | AppError::PermissionDenied => AppError::SaveFailed,
                e => e,
            };
            let pending = notes::Pending {
                title: form.title,
                body: form.body,
                base_revision: form.base_revision,
            };
            notes_page(
                &state,
                &headers,
                notes::Section::Mine,
                Some(note_id),
                NotesQuery::default(),
                Some(EditorState {
                    save: AppSaveState::Failed(err),
                    pending: Some(pending),
                }),
                status,
            )
            .await
        }
    }
}

// ── Calendário ───────────────────────────────────────────────────────────

#[derive(Deserialize, Default)]
pub(super) struct CalendarQuery {
    #[serde(default)]
    view: Option<String>,
    #[serde(default)]
    date: Option<String>,
}

/// O que o inspector do Calendário mostra.
enum CalInspector {
    None,
    Event(Uuid),
    New,
    Edit(Uuid),
    /// Um formulário devolvido com um erro de validação (o que o membro
    /// escreveu volta tal como estava).
    Invalid(crate::ui::view_models::CalFormVm),
}

async fn calendar_view(
    state: &WorkspaceState,
    headers: &HeaderMap,
    query: CalendarQuery,
    inspector: CalInspector,
    status: StatusCode,
) -> Response {
    use crate::controllers::productivity::calendar as cal;
    let w = match open_app(state, headers, Screen::Calendar, ApplicationId::Calendar).await {
        Ok(w) => w,
        Err(response) => return *response,
    };
    let quem = caller(&w.member);
    let clock = clock_of(&w.ctx);
    let view = cal::view_of(query.view.as_deref());
    let (today, day) = cal::anchor(query.date.as_deref(), &clock);
    let (start, n) = cal::days_of(view, day);
    let (laid, load) = match quem
        .get(state, &cal::agenda_path(start, n, clock.zone))
        .await
    {
        Ok(v) => (
            cal::lay_out(view, day, today, controllers::desktop::items(&v), &clock),
            AppLoad::Ready,
        ),
        Err(ApiFailure::Unauthorised) => return session_ended(state, headers),
        Err(f) => (
            cal::lay_out(view, day, today, &[], &clock),
            AppLoad::Failed(app_error(&f)),
        ),
    };
    let (prev_href, next_href, today_href) = cal::nav_hrefs(view, day, today);

    let mut details = None;
    let mut form = None;
    let mut error = None;
    match inspector {
        CalInspector::None => {}
        CalInspector::New => form = Some(cal::form(None, day, &clock)),
        CalInspector::Invalid(f) => form = Some(f),
        CalInspector::Event(id) | CalInspector::Edit(id) => {
            match quem
                .get(state, &format!("/api/v1/calendar/events/{id}"))
                .await
            {
                Ok(e) if matches!(inspector, CalInspector::Edit(_)) => {
                    form = Some(cal::form(Some(&e), day, &clock));
                }
                Ok(e) => details = Some(event_details(state, &quem, &e, &clock, &query).await),
                Err(ApiFailure::Unauthorised) => return session_ended(state, headers),
                Err(f) => error = Some(app_error(&f)),
            }
        }
    }
    let vm = crate::ui::view_models::CalendarVm {
        view,
        range_label: cal::range_label(view, day),
        prev_href,
        next_href,
        today_href,
        weekdays: laid.weekdays,
        days: laid.days,
        agenda: laid.agenda,
        now_minute: laid.now_minute,
        hours: (0, 24),
        timezone: Some(clock.zone.as_str().to_owned()),
        load: match (load, error) {
            (AppLoad::Ready, Some(e)) => AppLoad::Failed(e),
            (l, _) => l,
        },
        details,
        form,
        new_href: Some(format!(
            "/calendar/events/new?date={}",
            day.format("%Y-%m-%d")
        )),
    };
    render_app(state, &w, status, ui::apps::calendar::app(&vm), None, None)
}

/// O detalhe de um evento, com o nome do contexto (unidade ou espaço) quando
/// o Core o deixa ler.
async fn event_details(
    state: &WorkspaceState,
    quem: &Caller<'_>,
    e: &Value,
    clock: &controllers::desktop::Clock,
    query: &CalendarQuery,
) -> crate::ui::view_models::CalEventDetailsVm {
    use crate::controllers::desktop::text;
    use crate::controllers::productivity::calendar as cal;
    let id = text(e, "id").to_owned();
    let mut item = e.clone();
    item["kind"] = Value::String("event".to_owned());
    let day = e
        .get("starts_on")
        .and_then(Value::as_str)
        .and_then(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
        .or_else(|| {
            controllers::desktop::instant(e, "starts_at")
                .map(|at| at.with_timezone(&clock.zone.zone()).date_naive())
        })
        .unwrap_or_else(|| clock.now.date_naive());
    let laid = cal::lay_out(
        crate::ui::view_models::CalView::Day,
        day,
        day,
        std::slice::from_ref(&item),
        clock,
    );
    let mut event = laid
        .days
        .into_iter()
        .next()
        .and_then(|d| d.events.into_iter().next())
        .unwrap_or_else(|| crate::ui::view_models::CalEventVm {
            id: id.clone(),
            title: text(e, "title").to_owned(),
            time: String::new(),
            location: None,
            scope: crate::ui::view_models::CalScope::Personal,
            cancelled: false,
            all_day: false,
            span: None,
            lane: (0, 1),
            href: String::new(),
            origin_tz: None,
        });
    event.href = format!("/calendar/events/{id}");
    let context = if let Some(u) = e.get("unit_id").and_then(Value::as_str) {
        quem.get(state, &format!("/api/v1/units/{u}"))
            .await
            .ok()
            .map(|v| text(&v, "name").to_owned())
    } else if let Some(ws) = e.get("workspace_id").and_then(Value::as_str) {
        quem.get(state, &format!("/api/v1/workspaces/{ws}"))
            .await
            .ok()
            .map(|v| text(&v, "title").to_owned())
    } else {
        None
    }
    .filter(|c| !c.is_empty());
    let cancelled = event.cancelled;
    let ctx = format!(
        "?view={}&date={}",
        query.view.as_deref().unwrap_or("month"),
        query.date.as_deref().unwrap_or("")
    );
    crate::ui::view_models::CalEventDetailsVm {
        date: cal::full_date(day),
        description: e
            .get("description")
            .and_then(Value::as_str)
            .filter(|d| !d.is_empty())
            .map(str::to_owned),
        // O Core guarda os participantes mas ainda não os devolve.
        participants: Vec::new(),
        context,
        edit_href: (!cancelled).then(|| format!("/calendar/events/{id}/edit{ctx}")),
        cancel_action: (!cancelled).then(|| format!("/calendar/events/{id}/cancel")),
        nye: Some(crate::controllers::productivity::nye(
            "event",
            &id,
            "prod.nye.event",
        )),
        event,
    }
}

/// `GET /calendar`.
pub(super) async fn calendar_page(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(query): Query<CalendarQuery>,
) -> Response {
    calendar_view(&state, &headers, query, CalInspector::None, StatusCode::OK).await
}

/// `GET /calendar/events/new`.
pub(super) async fn new_event_form(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(query): Query<CalendarQuery>,
) -> Response {
    calendar_view(&state, &headers, query, CalInspector::New, StatusCode::OK).await
}

/// `GET /calendar/events/{id}`.
pub(super) async fn event_detail_page(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(event_id): Path<Uuid>,
    Query(query): Query<CalendarQuery>,
) -> Response {
    calendar_view(
        &state,
        &headers,
        query,
        CalInspector::Event(event_id),
        StatusCode::OK,
    )
    .await
}

/// `GET /calendar/events/{id}/edit`.
pub(super) async fn edit_event_form(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(event_id): Path<Uuid>,
    Query(query): Query<CalendarQuery>,
) -> Response {
    calendar_view(
        &state,
        &headers,
        query,
        CalInspector::Edit(event_id),
        StatusCode::OK,
    )
    .await
}

/// O formulário do Design (`CalFormVm`).
#[derive(Deserialize)]
pub(super) struct CalEventForm {
    #[serde(default)]
    title: String,
    #[serde(default)]
    all_day: Option<String>,
    #[serde(default)]
    start: String,
    #[serde(default)]
    end: String,
    #[serde(default)]
    location: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    scope: String,
}

/// O fuso do membro (o da Instância, pelo `/me`), para interpretar as horas
/// que ele escreveu. Nunca UTC por omissão silenciosa quando o `/me` responde.
async fn member_zone(
    state: &WorkspaceState,
    member: &Member,
) -> ocinye_contracts::temporal::TimeZoneName {
    let utc = ocinye_contracts::temporal::TimeZoneName::parse("UTC").expect("UTC");
    caller(member)
        .get(state, "/api/v1/me")
        .await
        .ok()
        .and_then(|me| {
            me.get("timezone")
                .and_then(Value::as_str)
                .and_then(|z| ocinye_contracts::temporal::TimeZoneName::parse(z).ok())
        })
        .unwrap_or(utc)
}

fn invalid_form(
    f: &CalEventForm,
    action: String,
    key: &'static str,
) -> crate::ui::view_models::CalFormVm {
    crate::ui::view_models::CalFormVm {
        action,
        title: f.title.clone(),
        start: f.start.clone(),
        end: f.end.clone(),
        all_day: f.all_day.is_some(),
        location: f.location.clone(),
        description: f.description.clone(),
        scopes: vec![(
            "personal".to_owned(),
            t("prod.cal.scope.personal").to_owned(),
        )],
        scope: "personal".to_owned(),
        error: Some(key),
    }
}

/// `POST /calendar/events/new`: um evento pessoal, no fuso do membro.
pub(super) async fn create_calendar_event(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<CalEventForm>,
) -> Response {
    use crate::controllers::productivity::calendar as cal;
    let member = member_or_login!(state, headers);
    let zone = member_zone(&state, &member).await;
    let occurrence = match cal::occurrence(form.all_day.is_some(), &form.start, &form.end, zone) {
        Ok(o) => o,
        Err(key) => {
            let f = invalid_form(&form, "/calendar/events/new".to_owned(), key);
            return calendar_view(
                &state,
                &headers,
                CalendarQuery::default(),
                CalInspector::Invalid(f),
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await;
        }
    };
    // Só o âmbito pessoal se oferece; outro valor não se aceita pelo formulário.
    if form.scope != "personal" {
        return (StatusCode::UNPROCESSABLE_ENTITY, "scope").into_response();
    }
    let body = serde_json::json!({
        "scope": "personal",
        "title": form.title.trim(),
        "description": (!form.description.trim().is_empty()).then(|| form.description.clone()),
        "location": (!form.location.trim().is_empty()).then(|| form.location.clone()),
        "occurrence": occurrence,
    });
    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        "/api/v1/calendar/events",
        &body,
    )
    .await
    {
        Ok(created) => {
            let id = created
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default();
            Redirect::to(&format!("/calendar/events/{id}")).into_response()
        }
        Err(ApiFailure::Unauthorised) => session_ended(&state, &headers),
        Err(ApiFailure::Rejected(_)) => {
            let f = invalid_form(
                &form,
                "/calendar/events/new".to_owned(),
                "prod.cal.err.dates",
            );
            calendar_view(
                &state,
                &headers,
                CalendarQuery::default(),
                CalInspector::Invalid(f),
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await
        }
        Err(f) => failure_response(&f),
    }
}

/// `POST /calendar/events/{id}/edit`: título, local, descrição e ocorrência.
/// O âmbito não muda depois de criado.
pub(super) async fn update_calendar_event(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(event_id): Path<Uuid>,
    Form(form): Form<CalEventForm>,
) -> Response {
    use crate::controllers::productivity::calendar as cal;
    let member = member_or_login!(state, headers);
    let zone = member_zone(&state, &member).await;
    let action = format!("/calendar/events/{event_id}/edit");
    let occurrence = match cal::occurrence(form.all_day.is_some(), &form.start, &form.end, zone) {
        Ok(o) => o,
        Err(key) => {
            let f = invalid_form(&form, action, key);
            return calendar_view(
                &state,
                &headers,
                CalendarQuery::default(),
                CalInspector::Invalid(f),
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await;
        }
    };
    let body = serde_json::json!({
        "title": form.title.trim(),
        "description": if form.description.trim().is_empty() { Value::Null } else { Value::String(form.description.clone()) },
        "location": if form.location.trim().is_empty() { Value::Null } else { Value::String(form.location.clone()) },
        "occurrence": occurrence,
    });
    match api::patch(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/calendar/events/{event_id}"),
        &body,
    )
    .await
    {
        Ok(_) => Redirect::to(&format!("/calendar/events/{event_id}")).into_response(),
        Err(ApiFailure::Unauthorised) => session_ended(&state, &headers),
        Err(f) => failure_response(&f),
    }
}

/// `POST /calendar/events/{id}/cancel`: o evento fica, riscado (o Core não o
/// apaga), e volta-se ao detalhe.
pub(super) async fn cancel_calendar_event(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(event_id): Path<Uuid>,
) -> Response {
    let member = member_or_login!(state, headers);
    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/calendar/events/{event_id}/cancel"),
        &serde_json::json!({}),
    )
    .await
    {
        Ok(_) => Redirect::to(&format!("/calendar/events/{event_id}")).into_response(),
        Err(ApiFailure::Unauthorised) => session_ended(&state, &headers),
        Err(f) => failure_response(&f),
    }
}

// ── Ficheiros ────────────────────────────────────────────────────────────

#[derive(Deserialize, Default)]
pub(super) struct FilesQuery {
    #[serde(default)]
    section: Option<String>,
    #[serde(default)]
    folder: Option<Uuid>,
    #[serde(default)]
    item: Option<String>,
    #[serde(default)]
    sort: Option<String>,
    #[serde(default)]
    dir: Option<String>,
    #[serde(default)]
    view: Option<String>,
    #[serde(default)]
    q: Option<String>,
    #[serde(default)]
    preview: Option<String>,
}

/// `GET /files`: o espaço pessoal (secção, pasta, item aberto no inspector).
pub(super) async fn files_page(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(query): Query<FilesQuery>,
) -> Response {
    use crate::controllers::productivity::files as fl;
    use crate::ui::view_models::{FilePreviewVm, FilesSection, FilesView, FilesVm};
    let w = match open_app(&state, &headers, Screen::Files, ApplicationId::Files).await {
        Ok(w) => w,
        Err(response) => return *response,
    };
    let quem = caller(&w.member);
    let clock = clock_of(&w.ctx);
    let section = fl::section_of(query.section.as_deref());
    let folder = query.folder.filter(|_| section == FilesSection::Mine);
    let sort = fl::sort_of(query.sort.as_deref(), query.dir.as_deref());
    let q = query.q.clone().unwrap_or_default();
    let (listing, load) = match quem.get(&state, &fl::list_api(section, folder)).await {
        Ok(v) => (v, AppLoad::Ready),
        Err(ApiFailure::Unauthorised) => return session_ended(&state, &headers),
        Err(f) => (Value::Null, AppLoad::Failed(app_error(&f))),
    };
    let open = query.item.as_deref();
    let items = fl::items(&listing, section, folder, open, sort, &q, &clock);
    // O inspector: só um ficheiro que esta listagem (do Core, agora) contém.
    let mut details = None;
    if let (Some(id), Some(raw)) = (open, open.and_then(|id| fl::raw_of(&listing, id))) {
        let version = raw.get("version_id").and_then(Value::as_str).unwrap_or("");
        let name = raw.get("name").and_then(Value::as_str).unwrap_or("");
        let ct = raw
            .get("content_type")
            .and_then(Value::as_str)
            .unwrap_or("");
        let size = raw.get("size_bytes").and_then(Value::as_i64).unwrap_or(0);
        let preview = match fl::preview_for(ct, name, version) {
            Some(p) => Some(p),
            None if size > 512 * 1024 => Some(FilePreviewVm::Unsupported),
            None => match api::bytes(
                &state,
                &w.member.session.access_token,
                &w.member.correlation_id,
                &format!("/api/v1/me/files/{version}/text"),
            )
            .await
            {
                Ok((_, body)) => Some(match String::from_utf8(body) {
                    Ok(text) => FilePreviewVm::Text {
                        text,
                        lang: None,
                        truncated: false,
                    },
                    Err(_) => FilePreviewVm::Unsupported,
                }),
                Err(_) => Some(FilePreviewVm::Unsupported),
            },
        };
        if let Some(item) = items.iter().find(|i| i.id == id) {
            details = Some(fl::details(item, raw, preview, &clock));
        }
    }
    let can_write = section == FilesSection::Mine;
    let vm = FilesVm {
        section,
        nav: fl::nav(section),
        crumbs: fl::crumbs(section, folder, &listing),
        view: if query.view.as_deref() == Some("grid") {
            FilesView::Grid
        } else {
            FilesView::List
        },
        sort,
        query: q,
        items,
        load,
        page: AppPageVm::default(),
        details,
        uploads: Vec::new(),
        // A pasta onde se envia e cria: `root` na raiz (o motor de envio e o
        // formulário de «Nova pasta» traduzem-no para «sem pasta»).
        folder_id: can_write.then(|| folder.map_or_else(|| "root".to_owned(), |f| f.to_string())),
        can_write,
        preview_open: query.preview.as_deref() == Some("1"),
    };
    let strings = upload_strings();
    render_app(
        &state,
        &w,
        StatusCode::OK,
        ui::apps::files::app(&vm),
        Some(strings),
        None,
    )
}

/// Os textos da bandeja de envios, na língua do membro, para o motor de envio
/// (`files-engine.js`), que não traduz: um elemento inerte com `data-*`.
fn upload_strings() -> AnyView {
    view! {
        <template
            data-part="files-up-strings"
            data-queued=t("files.up.queued")
            data-checking=t("files.up.checking")
            data-done=t("files.up.done")
            data-cancelled=t("files.up.cancelled")
            data-failed=t("app.err.upload.title")
            data-full=t("prod.files.up.full")
            data-type=t("prod.files.up.type")
            data-cancel=t("files.up.cancel")
            data-retry=t("files.up.retry")
            data-uploads=t("files.uploads")
        ></template>
    }
    .into_any()
}

#[derive(Deserialize)]
pub(super) struct NewFolderForm {
    #[serde(default)]
    name: String,
    #[serde(default)]
    parent: String,
}

/// `POST /files/folder`: uma pasta pessoal nova (as pastas pessoais são
/// planas; `parent` só diz de onde se veio).
pub(super) async fn files_new_folder(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<NewFolderForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let name = form.name.trim();
    if name.is_empty() {
        return Redirect::to("/files").into_response();
    }
    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        "/api/v1/me/folders",
        &serde_json::json!({ "name": name }),
    )
    .await
    {
        Ok(created) => {
            let id = created
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let _ = form.parent;
            Redirect::to(&format!("/files?folder={id}")).into_response()
        }
        Err(ApiFailure::Unauthorised) => session_ended(&state, &headers),
        Err(f) => failure_response(&f),
    }
}

#[derive(Deserialize)]
pub(super) struct RenameForm {
    #[serde(default)]
    name: String,
}

/// `POST /files/{id}/rename`: mudar o nome de um ficheiro do próprio.
pub(super) async fn files_rename(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(file_id): Path<String>,
    Form(form): Form<RenameForm>,
) -> Response {
    use crate::controllers::productivity::files as fl;
    let member = member_or_login!(state, headers);
    let item = file_id;
    let Some(fl::Ref::File(file, _)) = fl::parse_ref(&item) else {
        return failure_response(&ApiFailure::Denied);
    };
    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        "/api/v1/me/files/rename",
        &serde_json::json!({ "file_id": file, "name": form.name.trim() }),
    )
    .await
    {
        Ok(_) => Redirect::to(&format!("/files?item={item}")).into_response(),
        Err(ApiFailure::Unauthorised) => session_ended(&state, &headers),
        Err(f) => failure_response(&f),
    }
}

/// `POST /files/selection`: uma acção sobre os itens seleccionados. Cada item é
/// autorizado pelo Core, um a um; a selecção não autoriza nada.
pub(super) async fn files_selection(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    use crate::controllers::productivity::files as fl;
    let member = member_or_login!(state, headers);
    // `item` repete-se: lê-se o corpo à mão (o `Form` do axum não junta chaves).
    let pairs: Vec<(String, String)> = url::form_urlencoded::parse(&body).into_owned().collect();
    let op = pairs
        .iter()
        .find(|(k, _)| k == "op")
        .map(|(_, v)| v.clone())
        .unwrap_or_default();
    let refs: Vec<fl::Ref> = pairs
        .iter()
        .filter(|(k, _)| k == "item")
        .filter_map(|(_, v)| fl::parse_ref(v))
        .collect();
    let files: Vec<(Uuid, Uuid)> = refs
        .iter()
        .filter_map(|r| match r {
            fl::Ref::File(f, v) => Some((*f, *v)),
            fl::Ref::Folder(_) => None,
        })
        .collect();
    let call = |path: &'static str, id: Uuid| {
        let state = state.clone();
        let token = member.session.access_token.clone();
        let corr = member.correlation_id.clone();
        async move {
            api::post(
                &state,
                &token,
                &corr,
                path,
                &serde_json::json!({ "file_id": id }),
            )
            .await
        }
    };
    let back = match op.as_str() {
        "restore" | "purge" => "/files?section=trash",
        _ => "/files",
    };
    match op.as_str() {
        "download" => match files.as_slice() {
            [(_, v)] => Redirect::to(&format!("/me/files/{v}/download")).into_response(),
            _ => failure_response(&ApiFailure::Rejected(
                t("prod.files.err.download_many").to_owned(),
            )),
        },
        "favourite" | "trash" | "restore" => {
            let path = match op.as_str() {
                "favourite" => "/api/v1/me/files/favourite",
                "trash" => "/api/v1/me/files/delete",
                _ => "/api/v1/me/files/restore",
            };
            for (f, _) in &files {
                match call(path, *f).await {
                    Err(ApiFailure::Unauthorised) => return session_ended(&state, &headers),
                    Err(e) => return failure_response(&e),
                    Ok(_) => {}
                }
            }
            if op == "trash" {
                for r in &refs {
                    if let fl::Ref::Folder(d) = r {
                        let _ = api::delete(
                            &state,
                            &member.session.access_token,
                            &member.correlation_id,
                            &format!("/api/v1/me/folders/{d}"),
                        )
                        .await;
                    }
                }
            }
            Redirect::to(back).into_response()
        }
        // Eliminar para sempre não se faz sem uma confirmação, e o Design ainda
        // não a desenhou (MISSING_DESIGN_STATE): recusa-se, e nada se apaga.
        "purge" => failure_response(&ApiFailure::Rejected(
            t("prod.files.err.purge_confirmation").to_owned(),
        )),
        // Mover pela barra precisa de um destino, e o formulário do Design não o
        // tem; mover faz-se arrastando para uma pasta.
        "move" => failure_response(&ApiFailure::Rejected(
            t("prod.files.err.move_target").to_owned(),
        )),
        _ => failure_response(&ApiFailure::Rejected(String::new())),
    }
}

// ── Correio ──────────────────────────────────────────────────────────────

#[derive(Deserialize, Default)]
pub(super) struct MailQuery {
    #[serde(default)]
    r#box: Option<Uuid>,
    #[serde(default)]
    folder: Option<String>,
    #[serde(default)]
    q: Option<String>,
    #[serde(default)]
    draft: Option<Uuid>,
    #[serde(default)]
    saved: Option<String>,
}

/// O que o painel de leitura mostra.
enum MailPane {
    List,
    Message(Uuid),
    Compose,
    /// O compositor devolvido depois de uma gravação ou de um envio que não
    /// passou, com o que o membro escreveu.
    Returned(Box<crate::ui::view_models::MailComposeVm>),
}

async fn mail_view(
    state: &WorkspaceState,
    headers: &HeaderMap,
    query: MailQuery,
    pane: MailPane,
    status: StatusCode,
) -> Response {
    use crate::controllers::productivity::mail as ml;
    use crate::ui::view_models::{MailComposeVm, MailSendState};
    let w = match open_app(state, headers, Screen::Mail, ApplicationId::Mail).await {
        Ok(w) => w,
        Err(response) => return *response,
    };
    let quem = caller(&w.member);
    let clock = clock_of(&w.ctx);
    let (folder_vm, folder, folder_key) = ml::folder_of(query.folder.as_deref());
    let boxes_raw = match quem.get(state, "/api/v1/mail/mailboxes").await {
        Ok(v) => v.as_array().cloned().unwrap_or_default(),
        Err(ApiFailure::Unauthorised) => return session_ended(state, headers),
        // Uma recusa ou uma falha não é «nenhuma caixa»: diz-se o erro. O
        // `MailVm` do Design não tem este estado (sem caixas é sempre «ligar
        // uma caixa»), por isso desenha-se o erro tipado das aplicações.
        Err(f) => {
            return render_app(
                state,
                &w,
                StatusCode::OK,
                ui::apps::error(app_error(&f)).into_any(),
                None,
                None,
            )
        }
    };
    let current = query
        .r#box
        .map(|b| b.to_string())
        .or_else(|| {
            boxes_raw
                .first()
                .map(|b| controllers::desktop::text(b, "id").to_owned())
        })
        .unwrap_or_default();
    let mailboxes = ml::mailboxes(&boxes_raw, &current, folder, &clock);
    // D004.1 · Uma caixa do membro sem credenciais ligadas tem índice e
    // rascunhos, mas o corpo e o envio vêm do servidor de correio: isso é
    // `NotConnected`, e não uma falha passageira. Uma caixa que não é do membro
    // não tem estado aqui — vai ao Core, que a recusa.
    let connected = boxes_raw
        .iter()
        .find(|b| controllers::desktop::text(b, "id") == current)
        .map(|b| b.get("connected").and_then(Value::as_bool).unwrap_or(false));
    let q = query.q.clone().unwrap_or_default();
    let open = match &pane {
        MailPane::Message(id) => Some(id.to_string()),
        _ => None,
    };
    let (items, load) = if current.is_empty() {
        (Vec::new(), AppLoad::Ready)
    } else {
        let mut path = format!("/api/v1/mail/mailboxes/{current}/messages?folder={folder}");
        if !q.trim().is_empty() {
            path.push_str(&format!("&q={}", urlencoding_minimal(q.trim())));
        }
        match quem.get(state, &path).await {
            Ok(v) => (
                ml::items(
                    v.get("items")
                        .and_then(Value::as_array)
                        .map_or(&[][..], Vec::as_slice),
                    &current,
                    folder,
                    open.as_deref(),
                    &clock,
                ),
                AppLoad::Ready,
            ),
            Err(ApiFailure::Unauthorised) => return session_ended(state, headers),
            Err(f) => (Vec::new(), AppLoad::Failed(app_error(&f))),
        }
    };
    let ctx = format!("box={current}&folder={folder}");
    let mut message = None;
    let mut message_error = None;
    let mut compose = None;
    match pane {
        MailPane::List => {}
        MailPane::Message(_) if connected == Some(false) => {
            message_error = Some(AppError::NotConnected);
        }
        MailPane::Message(id) => {
            match quem
                .get(
                    state,
                    &format!("/api/v1/mail/messages/{id}?allow_remote=false"),
                )
                .await
            {
                Ok(m) => message = Some(message_vm(&m, &id, &ctx, &clock)),
                Err(ApiFailure::Unauthorised) => return session_ended(state, headers),
                Err(f) => message_error = Some(app_error(&f)),
            }
        }
        MailPane::Returned(c) => compose = Some(*c),
        MailPane::Compose => {
            let base = |to: String, cc: String, bcc: String, subject: String, body: String| {
                MailComposeVm {
                    draft_id: query.draft.map(|d| d.to_string()),
                    save_action: compose_action("save", &current, query.draft),
                    send_action: compose_action("send", &current, query.draft),
                    to,
                    cc,
                    bcc,
                    subject,
                    body,
                    attachments: Vec::new(),
                    attach_href: None,
                    save: if query.saved.is_some() {
                        AppSaveState::Saved(clock.hhmm(clock.now))
                    } else {
                        AppSaveState::Clean
                    },
                    send: MailSendState::Idle,
                    external_count: 0,
                    nye: None,
                    error: None,
                }
            };
            compose = Some(match query.draft {
                None => base(
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                ),
                Some(d) => match quem.get(state, &format!("/api/v1/mail/drafts/{d}")).await {
                    Ok(v) => {
                        let join = |k: &str| {
                            v.get(k)
                                .and_then(Value::as_array)
                                .map(|a| {
                                    a.iter()
                                        .filter_map(Value::as_str)
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                })
                                .unwrap_or_default()
                        };
                        base(
                            join("to"),
                            join("cc"),
                            join("bcc"),
                            controllers::desktop::text(&v, "subject").to_owned(),
                            controllers::desktop::text(&v, "body").to_owned(),
                        )
                    }
                    Err(ApiFailure::Unauthorised) => return session_ended(state, headers),
                    Err(f) => {
                        message_error = Some(app_error(&f));
                        return render_mail(
                            state,
                            &w,
                            status,
                            &mailboxes,
                            folder_vm,
                            folder_key,
                            q,
                            items,
                            load,
                            None,
                            message_error,
                            None,
                            &current,
                        );
                    }
                },
            });
        }
    }
    render_mail(
        state,
        &w,
        status,
        &mailboxes,
        folder_vm,
        folder_key,
        q,
        items,
        load,
        message,
        message_error,
        compose,
        &current,
    )
}

#[allow(clippy::too_many_arguments)]
fn render_mail(
    state: &WorkspaceState,
    w: &AppWindow,
    status: StatusCode,
    mailboxes: &[crate::ui::view_models::MailboxVm],
    folder: crate::ui::view_models::MailFolderVm,
    folder_key: &str,
    query: String,
    items: Vec<crate::ui::view_models::MailItemVm>,
    load: AppLoad,
    message: Option<crate::ui::view_models::MailMessageVm>,
    message_error: Option<AppError>,
    compose: Option<crate::ui::view_models::MailComposeVm>,
    current: &str,
) -> Response {
    // «Guardar rascunho» no fecho D002: submete o formulário do compositor.
    let dirty = compose
        .as_ref()
        .zip(w.window.as_ref())
        .map(|(c, win)| DirtyCloseVm {
            window_id: win.clone(),
            title: if c.subject.trim().is_empty() {
                t("mail.compose").to_owned()
            } else {
                c.subject.clone()
            },
            can_save: true,
            save_label: Some("mail.draft.save"),
            save_form: Some(ui::apps::doc_form_id(
                "mail",
                c.draft_id.as_deref().unwrap_or("new"),
            )),
        });
    let vm = crate::ui::view_models::MailVm {
        mailboxes: mailboxes.to_vec(),
        connect_href: None,
        folder,
        folder_label: t(folder_key).to_owned(),
        query,
        items,
        load,
        page: AppPageVm::default(),
        message,
        message_error,
        compose,
        compose_href: format!("/mail/compose?box={current}"),
    };
    let template = dirty.as_ref().map(dirty_template);
    render_app(state, w, status, ui::apps::mail::app(&vm), template, dirty)
}

fn compose_action(what: &str, mailbox: &str, draft: Option<Uuid>) -> String {
    match draft {
        Some(d) => format!("/mail/compose/{what}?box={mailbox}&draft={d}"),
        None => format!("/mail/compose/{what}?box={mailbox}"),
    }
}

/// A mensagem aberta: cabeçalhos e o corpo como parágrafos de texto simples.
/// Responder e reencaminhar não aparecem: o Core ainda não liga a resposta ao
/// fio da conversa.
fn message_vm(
    m: &Value,
    id: &Uuid,
    ctx: &str,
    clock: &controllers::desktop::Clock,
) -> crate::ui::view_models::MailMessageVm {
    use crate::controllers::desktop::{instant, text};
    use crate::controllers::productivity::mail as ml;
    let head = m.get("message").cloned().unwrap_or(Value::Null);
    let list = |k: &str| {
        m.get(k)
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default()
    };
    let from = {
        let (n, a) = (
            text(&head, "from_display_name"),
            text(&head, "from_address"),
        );
        if n.is_empty() {
            a.to_owned()
        } else {
            format!("{n} <{a}>")
        }
    };
    let blocked = m
        .get("blocked_remote_count")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        > 0;
    crate::ui::view_models::MailMessageVm {
        subject: text(&head, "subject").to_owned(),
        from,
        to: list("to"),
        cc: list("cc"),
        date: instant(&head, "sent_at")
            .map(|at| format!("{} · {}", clock.ddmm(at), clock.hhmm(at)))
            .unwrap_or_default(),
        body: ml::paragraphs(text(m, "body_html")),
        remote_blocked: blocked,
        remote_href: None,
        attachments: Vec::new(),
        reply_href: None,
        reply_all_href: None,
        forward_href: None,
        flags_action: Some(format!("/mail/message/{id}/op?{ctx}")),
        nye: Some(crate::controllers::productivity::nye(
            "message",
            &id.to_string(),
            "prod.nye.message",
        )),
    }
}

/// `GET /mail`.
pub(super) async fn mail_page(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(query): Query<MailQuery>,
) -> Response {
    mail_view(&state, &headers, query, MailPane::List, StatusCode::OK).await
}

/// `GET /mail/message/{id}`.
/// Se a caixa do membro está ligada ao servidor de correio; `None` quando não
/// é uma caixa do membro (ou a lista falhou) — aí decide o Core.
async fn mailbox_connected(state: &WorkspaceState, member: &Member, mailbox: Uuid) -> Option<bool> {
    let list = caller(member)
        .get(state, "/api/v1/mail/mailboxes")
        .await
        .ok()?;
    let id = mailbox.to_string();
    list.as_array()?
        .iter()
        .find(|b| controllers::desktop::text(b, "id") == id)
        .map(|b| b.get("connected").and_then(Value::as_bool).unwrap_or(false))
}

pub(super) async fn mail_message_page(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(message_id): Path<Uuid>,
    Query(query): Query<MailQuery>,
) -> Response {
    mail_view(
        &state,
        &headers,
        query,
        MailPane::Message(message_id),
        StatusCode::OK,
    )
    .await
}

/// `GET /mail/compose`.
pub(super) async fn mail_compose_page(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(query): Query<MailQuery>,
) -> Response {
    mail_view(&state, &headers, query, MailPane::Compose, StatusCode::OK).await
}

/// O formulário do compositor do Design.
#[derive(Deserialize)]
pub(super) struct ComposeForm {
    #[serde(default)]
    to: String,
    #[serde(default)]
    cc: String,
    #[serde(default)]
    bcc: String,
    #[serde(default)]
    subject: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    then: Option<String>,
}

/// Grava o rascunho no Core (novo ou o existente) e devolve o seu id.
async fn save_draft(
    state: &WorkspaceState,
    member: &Member,
    mailbox: Uuid,
    draft: Option<Uuid>,
    f: &ComposeForm,
) -> Result<String, ApiFailure> {
    use crate::controllers::productivity::mail::addresses;
    let body = serde_json::json!({
        "mailbox_id": mailbox,
        "to": addresses(&f.to),
        "cc": addresses(&f.cc),
        "bcc": addresses(&f.bcc),
        "subject": f.subject,
        "body": f.body,
    });
    let result = match draft {
        Some(d) => {
            api::put(
                state,
                &member.session.access_token,
                &member.correlation_id,
                &format!("/api/v1/mail/drafts/{d}"),
                &body,
            )
            .await
        }
        None => {
            api::post(
                state,
                &member.session.access_token,
                &member.correlation_id,
                "/api/v1/mail/drafts",
                &body,
            )
            .await
        }
    }?;
    Ok(result.get("id").and_then(Value::as_str).map_or_else(
        || draft.map(|d| d.to_string()).unwrap_or_default(),
        str::to_owned,
    ))
}

/// O compositor devolvido com o que o membro escreveu.
fn returned(
    f: ComposeForm,
    mailbox: Uuid,
    draft: Option<Uuid>,
    save: AppSaveState,
    send: crate::ui::view_models::MailSendState,
    error: Option<AppError>,
) -> crate::ui::view_models::MailComposeVm {
    crate::ui::view_models::MailComposeVm {
        draft_id: draft.map(|d| d.to_string()),
        save_action: compose_action("save", &mailbox.to_string(), draft),
        send_action: compose_action("send", &mailbox.to_string(), draft),
        to: f.to,
        cc: f.cc,
        bcc: f.bcc,
        subject: f.subject,
        body: f.body,
        attachments: Vec::new(),
        attach_href: None,
        save,
        send,
        external_count: 0,
        nye: None,
        error,
    }
}

/// `POST /mail/compose/save`: guardar o rascunho. Com `then=close` (o fecho
/// D002), a janela fecha depois de guardado.
pub(super) async fn mail_compose_save(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(query): Query<MailQuery>,
    Form(form): Form<ComposeForm>,
) -> Response {
    use crate::ui::view_models::MailSendState;
    let member = member_or_login!(state, headers);
    let Some(mailbox) = query.r#box else {
        return failure_response(&ApiFailure::Denied);
    };
    match save_draft(&state, &member, mailbox, query.draft, &form).await {
        Ok(_) if form.then.as_deref() == Some("close") => {
            let next = state
                .sessions
                .with_desk(&member.session_id, |d| {
                    let ids: Vec<String> = d
                        .windows()
                        .iter()
                        .filter(|w| w.app == ApplicationId::Mail)
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
        Ok(id) => {
            Redirect::to(&format!("/mail/compose?box={mailbox}&draft={id}&saved=1")).into_response()
        }
        Err(ApiFailure::Unauthorised) => session_ended(&state, &headers),
        Err(f) => {
            let err = app_error(&f);
            let c = returned(
                form,
                mailbox,
                query.draft,
                AppSaveState::Failed(AppError::SaveFailed),
                MailSendState::Idle,
                Some(err),
            );
            mail_view(
                &state,
                &headers,
                MailQuery {
                    r#box: Some(mailbox),
                    ..MailQuery::default()
                },
                MailPane::Returned(Box::new(c)),
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await
        }
    }
}

/// `POST /mail/compose/send`: guarda o rascunho e envia pelo Core. Se o envio
/// falhar, o rascunho fica, com o texto do membro — nunca se perde.
pub(super) async fn mail_compose_send(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(query): Query<MailQuery>,
    Form(form): Form<ComposeForm>,
) -> Response {
    use crate::controllers::productivity::mail::addresses;
    use crate::ui::view_models::MailSendState;
    let member = member_or_login!(state, headers);
    let Some(mailbox) = query.r#box else {
        return failure_response(&ApiFailure::Denied);
    };
    let draft = match save_draft(&state, &member, mailbox, query.draft, &form).await {
        Ok(id) => Uuid::parse_str(&id).ok(),
        Err(ApiFailure::Unauthorised) => return session_ended(&state, &headers),
        Err(f) => {
            let err = app_error(&f);
            let c = returned(
                form,
                mailbox,
                query.draft,
                AppSaveState::Failed(AppError::SaveFailed),
                MailSendState::Failed,
                Some(err),
            );
            return mail_view(
                &state,
                &headers,
                MailQuery {
                    r#box: Some(mailbox),
                    ..MailQuery::default()
                },
                MailPane::Returned(Box::new(c)),
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await;
        }
    };
    // D004.1 · Sem ligação ao servidor de correio não há envio: diz-se
    // `NotConnected`, o rascunho acabado de guardar fica, e o Core nem é chamado.
    if mailbox_connected(&state, &member, mailbox).await == Some(false) {
        let saved = AppSaveState::Saved(clock_now_hhmm(&state, &member).await);
        let c = returned(
            form,
            mailbox,
            draft,
            saved,
            MailSendState::Failed,
            Some(AppError::NotConnected),
        );
        return mail_view(
            &state,
            &headers,
            MailQuery {
                r#box: Some(mailbox),
                ..MailQuery::default()
            },
            MailPane::Returned(Box::new(c)),
            StatusCode::CONFLICT,
        )
        .await;
    }
    let body = serde_json::json!({
        "mailbox_id": mailbox,
        "to": addresses(&form.to),
        "cc": addresses(&form.cc),
        "bcc": addresses(&form.bcc),
        "subject": form.subject,
        "body": form.body,
        "draft_id": draft,
        "confirmed": false,
    });
    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        "/api/v1/mail/send",
        &body,
    )
    .await
    {
        Ok(_) => {
            if let Some(d) = draft {
                let _ = api::delete(
                    &state,
                    &member.session.access_token,
                    &member.correlation_id,
                    &format!("/api/v1/mail/drafts/{d}"),
                )
                .await;
            }
            Redirect::to(&format!("/mail?box={mailbox}&folder=sent")).into_response()
        }
        Err(ApiFailure::Unauthorised) => session_ended(&state, &headers),
        Err(f) => {
            let err = match f {
                ApiFailure::Unavailable(_) | ApiFailure::Failed(_) => AppError::TransportFailed,
                ref other => app_error(other),
            };
            let saved = AppSaveState::Saved(clock_now_hhmm(&state, &member).await);
            let c = returned(
                form,
                mailbox,
                draft,
                saved,
                MailSendState::Failed,
                Some(err),
            );
            mail_view(
                &state,
                &headers,
                MailQuery {
                    r#box: Some(mailbox),
                    ..MailQuery::default()
                },
                MailPane::Returned(Box::new(c)),
                StatusCode::BAD_GATEWAY,
            )
            .await
        }
    }
}

async fn clock_now_hhmm(state: &WorkspaceState, member: &Member) -> String {
    let zone = member_zone(state, member).await;
    chrono::Utc::now()
        .with_timezone(&zone.zone())
        .format("%H:%M")
        .to_string()
}

#[derive(Deserialize)]
pub(super) struct MailOpForm {
    #[serde(default)]
    op: String,
}

/// `POST /mail/message/{id}/op`: as acções da mensagem aberta. Marcar por ler
/// é uma marca do Core; arquivar e pôr no lixo ainda não têm contrato no Core,
/// e dizem-no em vez de fingir.
pub(super) async fn mail_message_op(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(message_id): Path<Uuid>,
    Query(query): Query<MailQuery>,
    Form(form): Form<MailOpForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let back = format!(
        "/mail?box={}&folder={}",
        query.r#box.map(|b| b.to_string()).unwrap_or_default(),
        query.folder.as_deref().unwrap_or("inbox")
    );
    match form.op.as_str() {
        "unread" => match api::post(
            &state,
            &member.session.access_token,
            &member.correlation_id,
            &format!("/api/v1/mail/messages/{message_id}/flags"),
            &serde_json::json!({ "read": false }),
        )
        .await
        {
            Ok(_) => Redirect::to(&back).into_response(),
            Err(ApiFailure::Unauthorised) => session_ended(&state, &headers),
            Err(f) => failure_response(&f),
        },
        _ => failure_response(&ApiFailure::Rejected(t("prod.mail.err.move").to_owned())),
    }
}

// ── Nye contextual ───────────────────────────────────────────────────────

/// O texto com que a Nye abre a partir de uma aplicação («Sobre a nota
/// «Torre 2»: »). A referência relê-se com a sessão do membro: uma nota, um
/// evento, um ficheiro ou uma mensagem que ele não pode ver não dá texto
/// nenhum — nem o nome. A referência não concede nada; o Core decide de novo a
/// cada pedido que a Nye faça.
pub(super) async fn nye_reference(
    state: &WorkspaceState,
    member: &Member,
    reference: Option<&str>,
) -> Option<String> {
    use crate::controllers::desktop::text;
    let (kind, id) = reference?.split_once(':')?;
    let id = Uuid::parse_str(id).ok()?;
    let quem = caller(member);
    let (path, field, key) = match kind {
        "note" => (
            format!("/api/v1/me/notes/{id}"),
            "title",
            "prod.nye.about.note",
        ),
        "event" => (
            format!("/api/v1/calendar/events/{id}"),
            "title",
            "prod.nye.about.event",
        ),
        "file" => (
            format!("/api/v1/me/files/{id}"),
            "name",
            "prod.nye.about.file",
        ),
        "message" => (
            format!("/api/v1/mail/messages/{id}?allow_remote=false&mark_read=false"),
            "subject",
            "prod.nye.about.message",
        ),
        _ => return None,
    };
    let v = quem.get(state, &path).await.ok()?;
    let name = match kind {
        "message" => text(v.get("message")?, field).to_owned(),
        _ => text(&v, field).to_owned(),
    };
    (!name.trim().is_empty()).then(|| crate::i18n::tf(key, &[("name", name.trim())]))
}
