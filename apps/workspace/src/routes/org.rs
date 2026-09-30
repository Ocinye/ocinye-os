//! D006 · Organização no Workspace: Unidades e Administração (Membros, Papéis,
//! Instância), cada uma numa janela gerida (D002) com o ecrã do Design.
//!
//! Aqui só há canal. Cada página relê do Core, com a sessão do membro, tudo o
//! que mostra — incluindo a autoridade: um administrador despromovido entre
//! dois pedidos recebe a recusa no segundo. As acções abrem a confirmação
//! partilhada (`?confirm=…`, GET) só quando o Core as oferece; o POST vai para
//! a operação que já existe, e o Core decide outra vez. Uma recusa volta à
//! confirmação com o motivo tipado; um sucesso volta ao recurso com o aviso.
//!
//! A credencial temporária é a excepção deliberada: desenha-se directamente na
//! resposta do POST que a emitiu (`Cache-Control: no-store`), nunca num
//! endereço, num registo ou noutra página.

use leptos::prelude::*;

use super::productivity::{clock_of, dirty_template, open_app, AppWindow};
use super::*;
use crate::controllers::desktop::{instant, text};
use crate::controllers::org as og;
use crate::controllers::productivity::app_error;
use crate::controllers::research as rs;
use crate::i18n::t;
use crate::ui::view_models::{
    AdminSection, AdminVm, AppError, AppLoad, AppPageVm, DirtyCloseVm, OrgActionKind,
    OrgAppStateVm, OrgConfirmVm, OrgCredentialOnceVm, OrgInstanceVm, OrgMemberVm, OrgMembersListVm,
    OrgNewMemberVm, OrgPositionFormVm, OrgRefusal, OrgUnitStatus, ResListVm, ResOptionVm, ResPane,
    UnitAddMemberVm, UnitFormVm, UnitVm, UnitsSection, UnitsVm,
};
use ocinye_contracts::{ApplicationId, Permission};

/// A pergunta comum às duas aplicações.
#[derive(Deserialize, Default)]
pub(super) struct OrgQuery {
    #[serde(default)]
    nav: Option<String>,
    #[serde(default)]
    page: Option<u32>,
    /// A acção cuja confirmação se abre.
    #[serde(default)]
    confirm: Option<String>,
    #[serde(default)]
    person: Option<String>,
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    grant: Option<String>,
    #[serde(default)]
    session: Option<String>,
    /// O que acabou de acontecer (vocabulário fechado).
    #[serde(default)]
    done: Option<String>,
    /// A recusa do Core à última tentativa (vocabulário fechado).
    #[serde(default)]
    refused: Option<String>,
    /// Outra falha da última tentativa.
    #[serde(default)]
    err: Option<String>,
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

/// Desenha a aplicação: a confirmação partilhada vai **depois da casca**, como
/// o diálogo de fechar com trabalho por guardar; numa janela carregada por
/// `?frame=1`, vai no corpo, e o `OcApps.init(root)` liga-a lá.
fn render_org(
    _state: &WorkspaceState,
    w: &AppWindow,
    status: StatusCode,
    content: AnyView,
    confirm: Option<&OrgConfirmVm>,
    dirty: Option<DirtyCloseVm>,
) -> Response {
    let extra = dirty.as_ref().map(dirty_template);
    let dialog = confirm.map(|c| ui::apps::org::confirm(c).into_any());
    if w.page.frame {
        let title = w.title.clone();
        let body = view! {
            <template data-part="win-title">{title}</template>
            {content}
            {extra}
            {dialog}
        };
        return (
            status,
            [(header::CACHE_CONTROL, "no-store")],
            Html(body.to_html()),
        )
            .into_response();
    }
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

/// A resposta que traz a credencial temporária: nenhum intermediário a guarda.
fn no_store(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

/// Depois de um POST: de volta ao recurso, com o aviso ou com a recusa.
///
/// A recusa reabre a mesma confirmação (o mesmo alvo, a mesma acção) com o
/// motivo; uma autoridade perdida ou um alvo que desapareceu voltam ao recurso,
/// que se relê e o diz. Nada do texto do Core viaja no endereço.
fn after(
    base: &str,
    kind: OrgActionKind,
    param: Option<(&str, &str)>,
    result: Result<Value, ApiFailure>,
    notice: &str,
) -> Response {
    let reopen = |extra: &str| {
        let p = param.map_or_else(String::new, |(k, v)| format!("&{k}={}", rs::encode(v)));
        Redirect::to(&format!(
            "{base}?confirm={}{p}&{extra}",
            og::confirm_code(kind)
        ))
        .into_response()
    };
    match result {
        Ok(_) => Redirect::to(&format!("{base}?done={notice}")).into_response(),
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        Err(f) => match og::refusal_of(&f) {
            Some(r) => reopen(&format!("refused={}", og::refusal_code(r))),
            None => match f {
                // Só um convite nunca usado se apaga: a recusa do Core a apagar
                // uma conta usada é esta, pela operação (M-19).
                ApiFailure::Rejected(_) if kind == OrgActionKind::DeleteInvite => reopen(&format!(
                    "refused={}",
                    og::refusal_code(OrgRefusal::NotDeletable)
                )),
                // O conteúdo recusado (uma razão curta de mais) volta à mesma
                // confirmação, para se corrigir.
                ApiFailure::Rejected(_) => reopen("err=save"),
                // O resto é o contrato do D001: 403 e 404 com a página do
                // Design, e uma avaria com a referência para o administrador.
                other => super::failure_response(&other),
            },
        },
    }
}

fn confirm_error(v: Option<&str>) -> Option<AppError> {
    Some(match v? {
        "save" => AppError::SaveFailed,
        _ => AppError::Unavailable,
    })
}

// ═════════════════════════════════════════════════════════════════════════
// Administração
// ═════════════════════════════════════════════════════════════════════════

fn admin_nav(
    ctx: &controllers::ShellContext,
    section: AdminSection,
) -> Vec<crate::ui::view_models::AppNavVm> {
    let mut nav = vec![rs::nav(
        "admin.nav.members",
        "user",
        "/admin".to_owned(),
        section == AdminSection::Members,
    )];
    if ctx.viewer.can(Permission::RolesView) {
        nav.push(rs::nav(
            "admin.nav.roles",
            "shield",
            "/admin/roles".to_owned(),
            section == AdminSection::Roles,
        ));
    }
    if ctx.viewer.can(Permission::OrganisationView) {
        nav.push(rs::nav(
            "admin.nav.instance",
            "settings",
            "/admin/instance".to_owned(),
            section == AdminSection::Instance,
        ));
    }
    nav
}

fn admin_vm(section: AdminSection) -> AdminVm {
    AdminVm {
        section,
        nav: Vec::new(),
        error: None,
        members: OrgMembersListVm {
            rows: Vec::new(),
            load: AppLoad::Ready,
            page: AppPageVm {
                more_href: None,
                summary: None,
            },
        },
        pane: ResPane::List,
        member: None,
        member_error: None,
        new_member: None,
        credential: None,
        new_href: None,
        roles: Vec::new(),
        instance: None,
        list_href: "/admin".to_owned(),
    }
}

/// A aplicação inteira recusada: nem navegação, nem dados (A-10).
fn admin_denied(state: &WorkspaceState, w: &AppWindow, e: AppError) -> Response {
    let mut vm = admin_vm(AdminSection::Members);
    vm.error = Some(e);
    let status = match e {
        AppError::NotFound => StatusCode::NOT_FOUND,
        AppError::PermissionDenied => StatusCode::FORBIDDEN,
        _ => StatusCode::SERVICE_UNAVAILABLE,
    };
    render_org(state, w, status, ui::apps::admin::app(&vm), None, None)
}

async fn me_id(state: &WorkspaceState, quem: &controllers::Caller<'_>) -> String {
    quem.get(state, "/api/v1/me")
        .await
        .map(|v| text(&v, "person_id").to_owned())
        .unwrap_or_default()
}

/// O roster, na página pedida. Relê a autoridade: sem `MembersManage` no Core,
/// a aplicação inteira é recusada, mesmo que a janela já estivesse aberta.
async fn roster(
    state: &WorkspaceState,
    w: &AppWindow,
    vm: &mut AdminVm,
    open: Option<&str>,
    me: &str,
    page: u32,
) -> Result<(), AppError> {
    let quem = caller(&w.member);
    let clock = clock_of(&w.ctx);
    let list_query = if page > 1 {
        format!("?page={page}")
    } else {
        String::new()
    };
    vm.list_href = format!("/admin{list_query}");
    let v = quem
        .get(
            state,
            &format!("/api/v1/administration/members?page={page}&page_size=50"),
        )
        .await
        .map_err(|f| match f {
            ApiFailure::Denied | ApiFailure::Forbidden | ApiFailure::Unauthorised => {
                AppError::PermissionDenied
            }
            other => app_error(&other),
        })?;
    vm.members.rows = og::roster_rows(&items_of(&v), open, me, &list_query, &clock);
    vm.members.page = rs::page(&v, "/admin");
    vm.nav = admin_nav(&w.ctx, AdminSection::Members);
    vm.new_href = w
        .ctx
        .viewer
        .can(Permission::MembersCreate)
        .then(|| "/admin/members/new".to_owned());
    Ok(())
}

/// O membro aberto, composto de três leituras do Core, cada uma com a sua
/// autoridade: `/people/{id}` (a pessoa), `/security` (`MembersManage`, e os
/// sinais do actor) e `/access` (só com `RolesView`).
async fn member_vm(
    state: &WorkspaceState,
    w: &AppWindow,
    id: &str,
    me: &str,
    q: &OrgQuery,
) -> Result<(OrgMemberVm, Option<OrgConfirmVm>), AppError> {
    let quem = caller(&w.member);
    let clock = clock_of(&w.ctx);
    let base = format!("/admin/members/{id}");
    let enc = rs::encode(id);
    // A segurança primeiro: é ela que exige `MembersManage` sobre outrem.
    let sec = quem
        .get(
            state,
            &format!("/api/v1/administration/members/{enc}/security"),
        )
        .await
        .map_err(|f| app_error(&f))?;
    let person = quem
        .get(state, &format!("/api/v1/people/{enc}"))
        .await
        .map_err(|f| app_error(&f))?;
    let access = if w.ctx.viewer.can(Permission::RolesView) {
        quem.get(
            state,
            &format!("/api/v1/administration/members/{enc}/access"),
        )
        .await
        .ok()
    } else {
        None
    };
    let units = quem
        .get(state, "/api/v1/units?include_archived=true")
        .await
        .map(|v| items_of(&v))
        .unwrap_or_default();
    let is_self = id == me;
    let st = og::status(text(&person, "status")).ok_or(AppError::Unavailable)?;
    let flags = og::AccountFlags::of(&sec);
    let name = text(&person, "full_name").to_owned();
    let workspaces = match &access {
        Some(a) => {
            let visible = quem
                .get(state, "/api/v1/workspaces?page_size=100")
                .await
                .map(|v| items_of(&v))
                .unwrap_or_default();
            og::member_workspaces(a, &visible)
        }
        None => Vec::new(),
    };
    let access_vm = access
        .as_ref()
        .map(|a| og::access(a, &base, is_self, &units, &clock));
    let security_vm = og::security(&sec, &base, is_self, &clock);
    let actions = og::account_actions(&base, st, flags, is_self);
    let position_code = rs::opt(&person, "institutional_position");
    let confirm = q
        .confirm
        .as_deref()
        .and_then(og::confirm_kind)
        .and_then(|k| {
            let param = match k {
                OrgActionKind::GrantRole | OrgActionKind::RevokeRole => q.role.as_deref(),
                OrgActionKind::RevokeGrant => q.grant.as_deref(),
                OrgActionKind::RevokeSession => q.session.as_deref(),
                _ => None,
            };
            let mut c = og::member_confirm(
                &og::MemberTarget {
                    base: &base,
                    name: &name,
                    offered: &actions,
                    access: access_vm.as_ref(),
                    security: Some(&security_vm),
                },
                k,
                param,
            )?;
            c.refusal = q.refused.as_deref().and_then(og::refusal);
            c.error = confirm_error(q.err.as_deref());
            Some(c)
        });
    let vm = OrgMemberVm {
        avatar: og::avatar(&name),
        name,
        email: rs::opt(&person, "email"),
        status: st,
        position: position_code.as_deref().and_then(og::position),
        is_self,
        joined: None,
        last_seen: None,
        units: og::member_units(&person, access.as_ref(), &units),
        workspaces,
        access: access_vm,
        security: Some(security_vm),
        account_actions: actions,
        position_form: (sec.get("may_change_position").and_then(Value::as_bool) == Some(true))
            .then(|| OrgPositionFormVm {
                action: format!("{base}/position"),
                options: og::position_options(position_code.as_deref(), "org.pos.none"),
            }),
        // A recusa sem confirmação aberta (a acção deixou de existir entretanto).
        refusal: if confirm.is_none() {
            q.refused.as_deref().and_then(og::refusal)
        } else {
            None
        },
        notice: q.done.as_deref().and_then(og::notice),
    };
    Ok((vm, confirm))
}

pub(super) async fn admin_page(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(q): Query<OrgQuery>,
) -> Response {
    admin_members(&state, &headers, None, &q).await
}

pub(super) async fn admin_member_page(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(person_id): Path<Uuid>,
    Query(q): Query<OrgQuery>,
) -> Response {
    admin_members(&state, &headers, Some(person_id.to_string()), &q).await
}

async fn admin_members(
    state: &WorkspaceState,
    headers: &HeaderMap,
    open: Option<String>,
    q: &OrgQuery,
) -> Response {
    let w = match open_app(state, headers, Screen::Admin, ApplicationId::Administration).await {
        Ok(w) => w,
        Err(r) => return *r,
    };
    let quem = caller(&w.member);
    let me = me_id(state, &quem).await;
    let mut vm = admin_vm(AdminSection::Members);
    if let Err(e) = roster(
        state,
        &w,
        &mut vm,
        open.as_deref(),
        &me,
        q.page.unwrap_or(1),
    )
    .await
    {
        return admin_denied(state, &w, e);
    }
    let mut confirm = None;
    if let Some(id) = open.as_deref() {
        vm.pane = pane(true);
        match member_vm(state, &w, id, &me, q).await {
            Ok((mut m, c)) => {
                // O registo e a última actividade vêm da linha do roster.
                if let Some(r) = vm.members.rows.iter().find(|r| r.active) {
                    m.joined.clone_from(&r.joined);
                    m.last_seen.clone_from(&r.last_seen);
                }
                vm.member = Some(m);
                confirm = c;
            }
            Err(AppError::PermissionDenied) => {
                return admin_denied(state, &w, AppError::PermissionDenied)
            }
            Err(e) => vm.member_error = Some(e),
        }
    }
    let status = if vm.member_error == Some(AppError::NotFound) {
        StatusCode::NOT_FOUND
    } else {
        StatusCode::OK
    };
    render_org(
        state,
        &w,
        status,
        ui::apps::admin::app(&vm),
        confirm.as_ref(),
        None,
    )
}

// ── Criar membro · credencial uma vez ────────────────────────────────────

fn new_member_form(
    ctx: &controllers::ShellContext,
    units: &[Value],
    f: Option<&NewMemberForm>,
) -> OrgNewMemberVm {
    let pos = f.map(|f| f.position.as_str());
    let role = f.map_or("research_member", |f| f.role.as_str());
    let unit = f.map(|f| f.unit_id.as_str()).filter(|u| !u.is_empty());
    // O Core recusa criar um administrador da plataforma a quem não o é; o
    // selector só o oferece a quem detém essa autoridade (M-08).
    let may_platform = ctx.viewer.can(Permission::PlatformAdminister);
    let mut unit_options = vec![ResOptionVm {
        value: String::new(),
        label: t("prod.org.unit.none").to_owned(),
        selected: unit.is_none(),
    }];
    unit_options.extend(
        units
            .iter()
            .filter(|u| text(u, "status") == "active")
            .map(|u| ResOptionVm {
                value: text(u, "id").to_owned(),
                label: format!("{} · {}", text(u, "code"), text(u, "name")),
                selected: Some(text(u, "id")) == unit,
            }),
    );
    OrgNewMemberVm {
        action: "/admin/members/new".to_owned(),
        full_name: f.map(|f| f.full_name.clone()).unwrap_or_default(),
        email: f.map(|f| f.email.clone()).unwrap_or_default(),
        positions: og::position_options(pos, "org.pos.none"),
        roles: og::TECH_ROLES
            .into_iter()
            .filter(|r| may_platform || *r != crate::ui::view_models::OrgTechRole::PlatformAdmin)
            .map(|r| ResOptionVm {
                value: r.as_str().to_owned(),
                label: t(r.key()).to_owned(),
                selected: r.as_str() == role,
            })
            .collect(),
        units: unit_options,
        error: None,
        refusal: None,
    }
}

fn admin_dirty(w: &AppWindow) -> Option<DirtyCloseVm> {
    w.window.as_ref().map(|win| DirtyCloseVm {
        window_id: win.clone(),
        title: t("admin.new").to_owned(),
        can_save: true,
        save_label: Some("admin.new.do"),
        save_form: Some(ui::apps::doc_form_id("admin", "new")),
    })
}

pub(super) async fn admin_new_page(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
) -> Response {
    let w = match open_app(
        &state,
        &headers,
        Screen::Admin,
        ApplicationId::Administration,
    )
    .await
    {
        Ok(w) => w,
        Err(r) => return *r,
    };
    if !w.ctx.viewer.can(Permission::MembersCreate) {
        return admin_denied(&state, &w, AppError::PermissionDenied);
    }
    let quem = caller(&w.member);
    let me = me_id(&state, &quem).await;
    let mut vm = admin_vm(AdminSection::Members);
    if let Err(e) = roster(&state, &w, &mut vm, None, &me, 1).await {
        return admin_denied(&state, &w, e);
    }
    let units = quem
        .get(&state, "/api/v1/units")
        .await
        .map(|v| items_of(&v))
        .unwrap_or_default();
    vm.pane = pane(true);
    vm.new_member = Some(new_member_form(&w.ctx, &units, None));
    let dirty = admin_dirty(&w);
    render_org(
        &state,
        &w,
        StatusCode::OK,
        ui::apps::admin::app(&vm),
        None,
        dirty,
    )
}

#[derive(Deserialize, Default)]
pub(super) struct NewMemberForm {
    #[serde(default)]
    full_name: String,
    #[serde(default)]
    email: String,
    #[serde(default)]
    role: String,
    #[serde(default)]
    position: String,
    #[serde(default)]
    unit_id: String,
}

/// A credencial devolvida pelo Core (criar, repor, dar acesso), como o único VM
/// que traz um segredo. Nunca registada nem guardada.
fn credential(
    v: &Value,
    origin: OrgActionKind,
    name: String,
    done_href: String,
    clock: &controllers::desktop::Clock,
) -> Option<OrgCredentialOnceVm> {
    let c = v.get("credential").unwrap_or(v);
    let secret = text(c, "temporary_password");
    if secret.is_empty() {
        return None;
    }
    Some(OrgCredentialOnceVm {
        origin,
        name,
        email: text(c, "email").to_owned(),
        secret: secret.to_owned(),
        expires: instant(c, "expires_at")
            .map(|at| format!("{} {}", rs::day(at, clock), clock.hhmm(at)))
            .unwrap_or_default(),
        done_href,
    })
}

/// Mostra a credencial acabada de emitir, na resposta do próprio POST.
async fn show_credential(
    state: &WorkspaceState,
    w: &AppWindow,
    cred: OrgCredentialOnceVm,
) -> Response {
    let quem = caller(&w.member);
    let me = me_id(state, &quem).await;
    let mut vm = admin_vm(AdminSection::Members);
    if let Err(e) = roster(state, w, &mut vm, None, &me, 1).await {
        return admin_denied(state, w, e);
    }
    vm.pane = pane(true);
    vm.credential = Some(cred);
    no_store(render_org(
        state,
        w,
        StatusCode::OK,
        ui::apps::admin::app(&vm),
        None,
        None,
    ))
}

pub(super) async fn admin_create_member(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<NewMemberForm>,
) -> Response {
    let w = match open_app(
        &state,
        &headers,
        Screen::Admin,
        ApplicationId::Administration,
    )
    .await
    {
        Ok(w) => w,
        Err(r) => return *r,
    };
    let quem = caller(&w.member);
    let clock = clock_of(&w.ctx);
    let mut body = serde_json::json!({
        "full_name": form.full_name.trim(),
        "email": form.email.trim(),
        "role": form.role,
    });
    if !form.position.is_empty() {
        body["position"] = Value::String(form.position.clone());
    }
    if !form.unit_id.is_empty() {
        body["unit_id"] = Value::String(form.unit_id.clone());
    }
    match api::post(
        &state,
        &w.member.session.access_token,
        &w.member.correlation_id,
        "/api/v1/administration/members",
        &body,
    )
    .await
    {
        Ok(v) => {
            let id = text(&v, "person_id").to_owned();
            match credential(
                &v,
                OrgActionKind::Provision,
                form.full_name.trim().to_owned(),
                format!("/admin/members/{id}"),
                &clock,
            ) {
                Some(c) => show_credential(&state, &w, c).await,
                // O Core criou a conta mas não devolveu credencial: o membro
                // existe; o detalhe diz o que há.
                None => Redirect::to(&format!("/admin/members/{id}")).into_response(),
            }
        }
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        Err(f) => {
            // A conta não foi criada: o formulário volta com o que se escreveu.
            let me = me_id(&state, &quem).await;
            let mut vm = admin_vm(AdminSection::Members);
            if let Err(e) = roster(&state, &w, &mut vm, None, &me, 1).await {
                return admin_denied(&state, &w, e);
            }
            let units = quem
                .get(&state, "/api/v1/units")
                .await
                .map(|v| items_of(&v))
                .unwrap_or_default();
            let mut nf = new_member_form(&w.ctx, &units, Some(&form));
            nf.refusal = og::refusal_of(&f);
            nf.error = nf.refusal.is_none().then(|| match f {
                ApiFailure::Rejected(_) | ApiFailure::Conflict(_) => AppError::SaveFailed,
                ApiFailure::Forbidden => AppError::PermissionDenied,
                other => app_error(&other),
            });
            vm.pane = pane(true);
            vm.new_member = Some(nf);
            let dirty = admin_dirty(&w);
            render_org(
                &state,
                &w,
                StatusCode::UNPROCESSABLE_ENTITY,
                ui::apps::admin::app(&vm),
                None,
                dirty,
            )
        }
    }
}

/// Repor ou dar acesso: o Core devolve a credencial uma vez, e ela aparece na
/// resposta deste POST. Uma recusa volta à confirmação.
async fn issue(
    state: &WorkspaceState,
    headers: &HeaderMap,
    person_id: Uuid,
    path: &str,
    kind: OrgActionKind,
) -> Response {
    let w = match open_app(state, headers, Screen::Admin, ApplicationId::Administration).await {
        Ok(w) => w,
        Err(r) => return *r,
    };
    let quem = caller(&w.member);
    let clock = clock_of(&w.ctx);
    let base = format!("/admin/members/{person_id}");
    let result = api::post(
        state,
        &w.member.session.access_token,
        &w.member.correlation_id,
        &format!("/api/v1/administration/members/{person_id}/{path}"),
        &serde_json::json!({}),
    )
    .await;
    match result {
        Ok(v) => {
            let name = quem
                .get(state, &format!("/api/v1/people/{person_id}"))
                .await
                .map(|p| text(&p, "full_name").to_owned())
                .unwrap_or_default();
            match credential(&v, kind, name, base.clone(), &clock) {
                Some(c) => show_credential(state, &w, c).await,
                None => Redirect::to(&base).into_response(),
            }
        }
        other => after(&base, kind, None, other, ""),
    }
}

pub(super) async fn admin_reset_password(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(person_id): Path<Uuid>,
) -> Response {
    issue(
        &state,
        &headers,
        person_id,
        "password-reset",
        OrgActionKind::ResetPassword,
    )
    .await
}

pub(super) async fn admin_provision(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(person_id): Path<Uuid>,
) -> Response {
    // «Dar acesso» e «Reemitir» são a mesma operação do Core; a confirmação
    // que a abriu é a que volta numa recusa.
    issue(
        &state,
        &headers,
        person_id,
        "provision",
        OrgActionKind::Provision,
    )
    .await
}

// ── As outras acções de conta ────────────────────────────────────────────

#[derive(Deserialize, Default)]
pub(super) struct StatusForm {
    #[serde(default)]
    status: String,
    #[serde(default)]
    reason: String,
}

pub(super) async fn admin_status(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(person_id): Path<Uuid>,
    Form(form): Form<StatusForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let base = format!("/admin/members/{person_id}");
    // Só os três destinos que uma confirmação oferece; o Core decide o resto.
    let kind = match form.status.as_str() {
        "suspended" => OrgActionKind::Suspend,
        "disabled" => OrgActionKind::Disable,
        "active" => OrgActionKind::Reactivate,
        _ => return Redirect::to(&base).into_response(),
    };
    let result = api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/administration/members/{person_id}/status"),
        &serde_json::json!({ "status": form.status, "reason": form.reason.trim() }),
    )
    .await;
    after(&base, kind, None, result, "status")
}

#[derive(Deserialize, Default)]
pub(super) struct PositionForm {
    #[serde(default)]
    position: String,
}

pub(super) async fn admin_position(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(person_id): Path<Uuid>,
    Form(form): Form<PositionForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let base = format!("/admin/members/{person_id}");
    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/administration/members/{person_id}/position"),
        &serde_json::json!({ "position": form.position }),
    )
    .await
    {
        Ok(_) => Redirect::to(&format!("{base}?done=position")).into_response(),
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        Err(f) => match og::refusal_of(&f) {
            Some(r) => {
                Redirect::to(&format!("{base}?refused={}", og::refusal_code(r))).into_response()
            }
            None => Redirect::to(&base).into_response(),
        },
    }
}

pub(super) async fn admin_delete(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(person_id): Path<Uuid>,
) -> Response {
    let member = member_or_login!(state, headers);
    let base = format!("/admin/members/{person_id}");
    let result = api::delete(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/administration/members/{person_id}"),
    )
    .await;
    match result {
        // O membro deixou de existir: volta-se ao roster, não a um detalhe 404.
        Ok(_) => Redirect::to("/admin").into_response(),
        other => after(&base, OrgActionKind::DeleteInvite, None, other, ""),
    }
}

#[derive(Deserialize, Default)]
pub(super) struct RoleForm {
    #[serde(default)]
    role: String,
    #[serde(default)]
    reason: String,
}

pub(super) async fn admin_role_grant(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(person_id): Path<Uuid>,
    Form(form): Form<RoleForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let base = format!("/admin/members/{person_id}");
    // O papel viaja como o identificador estável; um valor que o catálogo não
    // conhece nem chega ao Core (e o Core recusa-o na mesma).
    if og::tech_role(&form.role).is_none() {
        return Redirect::to(&format!("{base}?refused=option")).into_response();
    }
    let result = api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/people/{person_id}/roles"),
        &serde_json::json!({ "role": form.role, "reason": form.reason.trim() }),
    )
    .await;
    after(
        &base,
        OrgActionKind::GrantRole,
        Some(("role", &form.role)),
        result,
        "role_granted",
    )
}

pub(super) async fn admin_role_revoke(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path((person_id, role)): Path<(Uuid, String)>,
) -> Response {
    let member = member_or_login!(state, headers);
    let base = format!("/admin/members/{person_id}");
    if og::tech_role(&role).is_none() {
        return Redirect::to(&format!("{base}?refused=option")).into_response();
    }
    let result = api::delete_with_body(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/people/{person_id}/roles"),
        &serde_json::json!({ "role": role }),
    )
    .await;
    after(
        &base,
        OrgActionKind::RevokeRole,
        Some(("role", &role)),
        result,
        "role_revoked",
    )
}

#[derive(Deserialize, Default)]
pub(super) struct ReasonForm {
    #[serde(default)]
    reason: String,
}

pub(super) async fn admin_grant_revoke(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path((person_id, grant_id)): Path<(Uuid, Uuid)>,
    Form(form): Form<ReasonForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let base = format!("/admin/members/{person_id}");
    let result = api::delete_with_body(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/administration/grants/{grant_id}"),
        &serde_json::json!({ "reason": form.reason.trim() }),
    )
    .await;
    let g = grant_id.to_string();
    after(
        &base,
        OrgActionKind::RevokeGrant,
        Some(("grant", &g)),
        result,
        "grant_revoked",
    )
}

pub(super) async fn admin_session_revoke(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path((person_id, session_id)): Path<(Uuid, Uuid)>,
) -> Response {
    let member = member_or_login!(state, headers);
    let base = format!("/admin/members/{person_id}");
    let result = api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/administration/members/{person_id}/sessions/{session_id}/revoke"),
        &serde_json::json!({}),
    )
    .await;
    let s = session_id.to_string();
    after(
        &base,
        OrgActionKind::RevokeSession,
        Some(("session", &s)),
        result,
        "session_revoked",
    )
}

// ── Papéis · Instância ───────────────────────────────────────────────────

pub(super) async fn admin_roles_page(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
) -> Response {
    let w = match open_app(
        &state,
        &headers,
        Screen::Admin,
        ApplicationId::Administration,
    )
    .await
    {
        Ok(w) => w,
        Err(r) => return *r,
    };
    if !w.ctx.viewer.can(Permission::RolesView) {
        return admin_denied(&state, &w, AppError::PermissionDenied);
    }
    let quem = caller(&w.member);
    let v = match quem.get(&state, "/api/v1/administration/roles").await {
        Ok(v) => v,
        Err(ApiFailure::Forbidden | ApiFailure::Denied) => {
            return admin_denied(&state, &w, AppError::PermissionDenied)
        }
        Err(f) => return admin_denied(&state, &w, app_error(&f)),
    };
    let mut vm = admin_vm(AdminSection::Roles);
    vm.nav = admin_nav(&w.ctx, AdminSection::Roles);
    vm.roles = og::role_catalogue(&v);
    render_org(
        &state,
        &w,
        StatusCode::OK,
        ui::apps::admin::app(&vm),
        None,
        None,
    )
}

fn distribution(v: &str) -> crate::ui::view_models::Distribution {
    use crate::ui::view_models::Distribution as D;
    match v {
        "business" => D::Business,
        "personal" => D::Personal,
        "education" => D::Education,
        _ => D::Research,
    }
}

pub(super) async fn admin_instance_page(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(q): Query<OrgQuery>,
) -> Response {
    let w = match open_app(
        &state,
        &headers,
        Screen::Admin,
        ApplicationId::Administration,
    )
    .await
    {
        Ok(w) => w,
        Err(r) => return *r,
    };
    if !w.ctx.viewer.can(Permission::OrganisationView) {
        return admin_denied(&state, &w, AppError::PermissionDenied);
    }
    let quem = caller(&w.member);
    let clock = clock_of(&w.ctx);
    let (org, settings, apps) = (
        quem.get(&state, "/api/v1/organisation").await,
        quem.get(&state, "/api/v1/instance/settings").await,
        quem.get(&state, "/api/v1/instance/applications").await,
    );
    let (org, settings, apps) = match (org, settings, apps) {
        (Ok(o), Ok(s), Ok(a)) => (o, s, a),
        (Err(ApiFailure::Forbidden | ApiFailure::Denied), _, _)
        | (_, _, Err(ApiFailure::Forbidden | ApiFailure::Denied)) => {
            return admin_denied(&state, &w, AppError::PermissionDenied)
        }
        _ => return admin_denied(&state, &w, AppError::Unavailable),
    };
    let locale = text(&settings, "default_locale");
    let apps_vm = apps
        .get("applications")
        .and_then(Value::as_array)
        .map(|l| {
            l.iter()
                .filter_map(|a| {
                    let id = text(a, "id");
                    let reg = experience::apps::by_id(id)?;
                    let b = |k: &str| a.get(k).and_then(Value::as_bool) == Some(true);
                    Some(OrgAppStateVm {
                        id: id.to_owned(),
                        label: reg.screen.label().to_owned(),
                        icon: ui::components::app_icon(reg.screen.path()),
                        essential: b("essential"),
                        active: b("active"),
                        explicit: b("explicit"),
                        profile_default: b("profile_default"),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let mut vm = admin_vm(AdminSection::Instance);
    vm.nav = admin_nav(&w.ctx, AdminSection::Instance);
    vm.instance = Some(OrgInstanceVm {
        name: text(&org, "name").to_owned(),
        distribution: distribution(text(&apps, "profile")),
        default_locale: t(&format!("prod.org.locale.{locale}")).to_owned(),
        timezone: text(&settings, "timezone").to_owned(),
        updated: instant(&settings, "updated_at").map(|at| rs::day(at, &clock)),
        apps: apps_vm,
        apps_action: w
            .ctx
            .viewer
            .can(Permission::OrganisationManage)
            .then(|| "/admin/instance".to_owned()),
        load: AppLoad::Ready,
        notice: q.done.as_deref().and_then(og::notice),
    });
    render_org(
        &state,
        &w,
        StatusCode::OK,
        ui::apps::admin::app(&vm),
        None,
        None,
    )
}

/// Grava as decisões por aplicação que mudaram (`app:<id>` = `profile`,
/// `active`, `inactive`). A Distribuição não muda aqui (A-07, adiado): um
/// campo `profile` no pedido é ignorado. As fixações dos membros não se tocam.
pub(super) async fn admin_instance_save(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    corpo: axum::body::Bytes,
) -> Response {
    let member = member_or_login!(state, headers);
    let quem = caller(&member);
    let actual = match quem.get(&state, "/api/v1/instance/applications").await {
        Ok(v) => v,
        Err(ApiFailure::Unauthorised) => return Redirect::to("/login").into_response(),
        Err(_) => return Redirect::to("/admin/instance").into_response(),
    };
    let estado_de = |id: &str| -> Option<Option<bool>> {
        actual
            .get("applications")
            .and_then(Value::as_array)?
            .iter()
            .find(|a| text(a, "id") == id)
            .map(|a| {
                if a.get("explicit").and_then(Value::as_bool) == Some(true) {
                    a.get("active").and_then(Value::as_bool)
                } else {
                    None
                }
            })
    };
    for (chave, valor) in url::form_urlencoded::parse(&corpo) {
        let Some(id) = chave.strip_prefix("app:") else {
            continue;
        };
        // O id entra no caminho de um pedido ao Core: só um id que o registo
        // conhece, nunca o texto do formulário tal como veio.
        if experience::apps::by_id(id).is_none() {
            continue;
        }
        let pedido = match valor.as_ref() {
            "active" => Some(true),
            "inactive" => Some(false),
            _ => None,
        };
        if estado_de(id) == Some(pedido) {
            continue;
        }
        match api::put(
            &state,
            &member.session.access_token,
            &member.correlation_id,
            &format!("/api/v1/instance/applications/{id}"),
            &serde_json::json!({ "active": pedido }),
        )
        .await
        {
            Ok(_) => {}
            Err(ApiFailure::Unauthorised) => return Redirect::to("/login").into_response(),
            Err(_) => return Redirect::to("/admin/instance").into_response(),
        }
    }
    Redirect::to("/admin/instance?done=settings").into_response()
}

// ═════════════════════════════════════════════════════════════════════════
// Unidades
// ═════════════════════════════════════════════════════════════════════════

fn units_nav(section: UnitsSection) -> Vec<crate::ui::view_models::AppNavVm> {
    vec![
        rs::nav(
            "units.nav.active",
            "units",
            "/units".to_owned(),
            section == UnitsSection::Active,
        ),
        rs::nav(
            "units.nav.archived",
            "archive",
            "/units?nav=archived".to_owned(),
            section == UnitsSection::Archived,
        ),
    ]
}

/// A lista de unidades da secção, e se o membro pode criar (`/units/capabilities`).
async fn units_vm(
    state: &WorkspaceState,
    w: &AppWindow,
    section: UnitsSection,
    open: Option<&str>,
) -> UnitsVm {
    let quem = caller(&w.member);
    let archived = section == UnitsSection::Archived;
    let path = if archived {
        "/api/v1/units?include_archived=true"
    } else {
        "/api/v1/units"
    };
    let (items, load) = match quem.get(state, path).await {
        Ok(v) => (
            og::unit_items(&items_of(&v), open, archived),
            AppLoad::Ready,
        ),
        Err(_) => (Vec::new(), AppLoad::Failed(AppError::Unavailable)),
    };
    let may_create = quem
        .get(state, "/api/v1/units/capabilities")
        .await
        .ok()
        .and_then(|v| v.get("may_create").and_then(Value::as_bool))
        == Some(true);
    UnitsVm {
        section,
        nav: units_nav(section),
        list: ResListVm {
            columns: vec!["units.col.areas"],
            items,
            load,
            page: AppPageVm {
                more_href: None,
                summary: None,
            },
        },
        pane: pane(open.is_some()),
        unit: None,
        unit_error: None,
        form: None,
        new_href: may_create.then(|| "/units/new".to_owned()),
        list_href: if archived {
            "/units?nav=archived".to_owned()
        } else {
            "/units".to_owned()
        },
    }
}

fn section_of(q: &OrgQuery) -> UnitsSection {
    if q.nav.as_deref() == Some("archived") {
        UnitsSection::Archived
    } else {
        UnitsSection::Active
    }
}

pub(super) async fn units_page(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(q): Query<OrgQuery>,
) -> Response {
    let w = match open_app(&state, &headers, Screen::Units, ApplicationId::Units).await {
        Ok(w) => w,
        Err(r) => return *r,
    };
    let vm = units_vm(&state, &w, section_of(&q), None).await;
    render_org(
        &state,
        &w,
        StatusCode::OK,
        ui::apps::units::app(&vm),
        None,
        None,
    )
}

/// A unidade aberta: a leitura (com os sinais do Core) e os membros.
async fn unit_vm(
    state: &WorkspaceState,
    w: &AppWindow,
    id: &str,
    q: &OrgQuery,
) -> Result<(UnitVm, Option<OrgConfirmVm>), AppError> {
    let quem = caller(&w.member);
    let enc = rs::encode(id);
    let u = quem
        .get(state, &format!("/api/v1/units/{enc}"))
        .await
        .map_err(|f| app_error(&f))?;
    let status = og::unit_status(text(&u, "status")).ok_or(AppError::Unavailable)?;
    let may_manage = u.get("may_manage_members").and_then(Value::as_bool) == Some(true)
        && status == OrgUnitStatus::Active;
    let may_archive = u.get("may_archive").and_then(Value::as_bool) == Some(true);
    let base = format!("/units/{id}");
    let (members, members_load) = match quem
        .get(state, &format!("/api/v1/units/{enc}/members"))
        .await
    {
        Ok(v) => (
            og::unit_members(
                &items_of(&v),
                &base,
                may_manage,
                w.ctx.viewer.can(Permission::MembersManage),
            ),
            AppLoad::Ready,
        ),
        Err(_) => (Vec::new(), AppLoad::Failed(AppError::Unavailable)),
    };
    let label = format!("{} · {}", text(&u, "code"), text(&u, "name"));
    let archive = may_archive.then(|| crate::ui::view_models::OrgActionVm {
        kind: OrgActionKind::ArchiveUnit,
        href: format!(
            "{base}?confirm={}",
            og::confirm_code(OrgActionKind::ArchiveUnit)
        ),
    });
    let confirm = q
        .confirm
        .as_deref()
        .and_then(og::confirm_kind)
        .and_then(|k| {
            let mut c = og::unit_confirm(
                &base,
                &label,
                &members,
                archive.as_ref(),
                k,
                q.person.as_deref(),
            )?;
            c.refusal = q.refused.as_deref().and_then(og::refusal);
            c.error = confirm_error(q.err.as_deref());
            Some(c)
        });
    let vm = UnitVm {
        code: text(&u, "code").to_owned(),
        name: text(&u, "name").to_owned(),
        status,
        description: rs::opt(&u, "description"),
        areas: og::areas(&u),
        members,
        members_load,
        may_manage,
        edit_href: may_manage.then(|| format!("{base}/edit")),
        archive,
        // Sem contrato de candidatos elegíveis (U-09): o bloco diz porquê, e
        // nenhum pedido sai daqui. O browser nunca enumera pessoas.
        add: may_manage.then(|| UnitAddMemberVm {
            search_action: base.clone(),
            query: String::new(),
            candidates: None,
            add_action: String::new(),
            roles: Vec::new(),
            unavailable: true,
        }),
        nye: Some(rs::nye("unit", id, "units.nye")),
        refusal: if confirm.is_none() {
            q.refused.as_deref().and_then(og::refusal)
        } else {
            None
        },
        notice: q.done.as_deref().and_then(og::notice),
    };
    Ok((vm, confirm))
}

pub(super) async fn unit_page(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(unit_id): Path<Uuid>,
    Query(q): Query<OrgQuery>,
) -> Response {
    let w = match open_app(&state, &headers, Screen::Units, ApplicationId::Units).await {
        Ok(w) => w,
        Err(r) => return *r,
    };
    let id = unit_id.to_string();
    let mut vm = units_vm(&state, &w, section_of(&q), Some(&id)).await;
    let mut confirm = None;
    match unit_vm(&state, &w, &id, &q).await {
        Ok((u, c)) => {
            vm.unit = Some(u);
            confirm = c;
        }
        Err(e) => vm.unit_error = Some(e),
    }
    let status = if vm.unit_error == Some(AppError::NotFound) {
        StatusCode::NOT_FOUND
    } else {
        StatusCode::OK
    };
    render_org(
        &state,
        &w,
        status,
        ui::apps::units::app(&vm),
        confirm.as_ref(),
        None,
    )
}

fn units_dirty(w: &AppWindow, doc: &str, save: &'static str) -> Option<DirtyCloseVm> {
    w.window.as_ref().map(|win| DirtyCloseVm {
        window_id: win.clone(),
        title: t("nav.units").to_owned(),
        can_save: true,
        save_label: Some(save),
        save_form: Some(ui::apps::doc_form_id("units", doc)),
    })
}

#[derive(Deserialize, Default)]
pub(super) struct UnitForm {
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    research_areas: String,
}

pub(super) async fn unit_new_page(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
) -> Response {
    let w = match open_app(&state, &headers, Screen::Units, ApplicationId::Units).await {
        Ok(w) => w,
        Err(r) => return *r,
    };
    let mut vm = units_vm(&state, &w, UnitsSection::Active, None).await;
    if vm.new_href.is_none() {
        vm.unit_error = Some(AppError::PermissionDenied);
        vm.pane = pane(true);
        return render_org(
            &state,
            &w,
            StatusCode::FORBIDDEN,
            ui::apps::units::app(&vm),
            None,
            None,
        );
    }
    vm.pane = pane(true);
    vm.form = Some(UnitFormVm {
        action: "/units/new".to_owned(),
        is_new: true,
        code: None,
        code_suggestion: None,
        name: String::new(),
        description: String::new(),
        areas: String::new(),
        error: None,
    });
    let dirty = units_dirty(&w, "new", "units.create");
    render_org(
        &state,
        &w,
        StatusCode::OK,
        ui::apps::units::app(&vm),
        None,
        dirty,
    )
}

pub(super) async fn unit_create(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<UnitForm>,
) -> Response {
    let w = match open_app(&state, &headers, Screen::Units, ApplicationId::Units).await {
        Ok(w) => w,
        Err(r) => return *r,
    };
    let body = serde_json::json!({
        "name": form.name.trim(),
        "description": rs::opt(&serde_json::json!({ "d": form.description }), "d"),
        "research_areas": rs::split_keywords(&form.research_areas),
    });
    match api::post(
        &state,
        &w.member.session.access_token,
        &w.member.correlation_id,
        "/api/v1/units",
        &body,
    )
    .await
    {
        Ok(v) => {
            Redirect::to(&format!("/units/{}?done=unit_saved", text(&v, "id"))).into_response()
        }
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        Err(f) => {
            let quem = caller(&w.member);
            let suggestion = quem
                .get(
                    &state,
                    &format!(
                        "/api/v1/units/code-suggestion?name={}",
                        rs::encode(form.name.trim())
                    ),
                )
                .await
                .ok()
                .map(|v| text(&v, "code").to_owned())
                .filter(|c| !c.is_empty());
            let mut vm = units_vm(&state, &w, UnitsSection::Active, None).await;
            vm.pane = pane(true);
            vm.form = Some(UnitFormVm {
                action: "/units/new".to_owned(),
                is_new: true,
                code: None,
                code_suggestion: suggestion,
                name: form.name,
                description: form.description,
                areas: form.research_areas,
                error: Some(match f {
                    ApiFailure::Rejected(_) | ApiFailure::Conflict(_) => AppError::SaveFailed,
                    other => app_error(&other),
                }),
            });
            let dirty = units_dirty(&w, "new", "units.create");
            render_org(
                &state,
                &w,
                StatusCode::UNPROCESSABLE_ENTITY,
                ui::apps::units::app(&vm),
                None,
                dirty,
            )
        }
    }
}

pub(super) async fn unit_edit_page(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(unit_id): Path<Uuid>,
) -> Response {
    let w = match open_app(&state, &headers, Screen::Units, ApplicationId::Units).await {
        Ok(w) => w,
        Err(r) => return *r,
    };
    let id = unit_id.to_string();
    let mut vm = units_vm(&state, &w, UnitsSection::Active, Some(&id)).await;
    match unit_vm(&state, &w, &id, &OrgQuery::default()).await {
        Ok((u, _)) if u.edit_href.is_some() => {
            vm.form = Some(UnitFormVm {
                action: format!("/units/{id}/edit"),
                is_new: false,
                code: Some(u.code.clone()),
                code_suggestion: None,
                name: u.name.clone(),
                description: u.description.clone().unwrap_or_default(),
                areas: u.areas.join(", "),
                error: None,
            });
            let dirty = units_dirty(&w, "edit", "units.save");
            render_org(
                &state,
                &w,
                StatusCode::OK,
                ui::apps::units::app(&vm),
                None,
                dirty,
            )
        }
        Ok(_) => {
            vm.unit_error = Some(AppError::PermissionDenied);
            render_org(
                &state,
                &w,
                StatusCode::FORBIDDEN,
                ui::apps::units::app(&vm),
                None,
                None,
            )
        }
        Err(e) => {
            vm.unit_error = Some(e);
            render_org(
                &state,
                &w,
                StatusCode::NOT_FOUND,
                ui::apps::units::app(&vm),
                None,
                None,
            )
        }
    }
}

pub(super) async fn unit_update(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(unit_id): Path<Uuid>,
    Form(form): Form<UnitForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let body = serde_json::json!({
        "name": form.name.trim(),
        "description": rs::opt(&serde_json::json!({ "d": form.description }), "d"),
        "research_areas": rs::split_keywords(&form.research_areas),
    });
    match api::put(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/units/{unit_id}"),
        &body,
    )
    .await
    {
        Ok(_) => Redirect::to(&format!("/units/{unit_id}?done=unit_saved")).into_response(),
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        Err(_) => Redirect::to(&format!("/units/{unit_id}/edit")).into_response(),
    }
}

#[derive(Deserialize, Default)]
pub(super) struct UnitMemberForm {
    #[serde(default)]
    person_id: String,
    #[serde(default)]
    role: String,
}

pub(super) async fn unit_member_role(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(unit_id): Path<Uuid>,
    Form(form): Form<UnitMemberForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let base = format!("/units/{unit_id}");
    if og::unit_role(&form.role).is_none() || Uuid::parse_str(&form.person_id).is_err() {
        return Redirect::to(&format!("{base}?refused=option")).into_response();
    }
    let result = api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/units/{unit_id}/members"),
        &serde_json::json!({ "person_id": form.person_id, "role": form.role }),
    )
    .await;
    after(
        &base,
        OrgActionKind::ChangeUnitRole,
        Some(("person", &form.person_id)),
        result,
        "unit_role",
    )
}

pub(super) async fn unit_member_remove(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(unit_id): Path<Uuid>,
    Form(form): Form<UnitMemberForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let base = format!("/units/{unit_id}");
    let Ok(person) = Uuid::parse_str(&form.person_id) else {
        return Redirect::to(&format!("{base}?refused=option")).into_response();
    };
    let result = api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/units/{unit_id}/members/{person}"),
        &serde_json::json!({}),
    )
    .await;
    after(
        &base,
        OrgActionKind::RemoveUnitMember,
        Some(("person", &form.person_id)),
        result,
        "member_removed",
    )
}

pub(super) async fn unit_archive(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(unit_id): Path<Uuid>,
) -> Response {
    let member = member_or_login!(state, headers);
    let base = format!("/units/{unit_id}");
    let result = api::delete(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/units/{unit_id}"),
    )
    .await;
    match result {
        Ok(_) => Redirect::to(&format!("{base}?nav=archived&done=unit_archived")).into_response(),
        other => after(&base, OrgActionKind::ArchiveUnit, None, other, ""),
    }
}
