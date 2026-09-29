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
    if let Some(wm) = ctx.vm.wm.as_mut() {
        for w in wm.windows.iter_mut().filter(|w| w.app_id == app.as_str()) {
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
