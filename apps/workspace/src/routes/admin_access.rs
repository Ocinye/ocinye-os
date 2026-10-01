//! D010 · Administração › Distribuições, Acesso, Pontos de acesso,
//! Predefinições — os pedidos (S26–S33, S37, S38).
//!
//! Cada página lê o Core e mostra; cada acção governada passa por uma
//! confirmação (um `GET ?confirm=…` que só desenha) e chega ao Core por `POST`.
//! A recusa é sempre do Core: o Workspace desenha o motivo que ele deu
//! (`details.reason`), e uma recusa conhecida de antemão tira o botão.

use leptos::prelude::*;
use serde_json::json;

use super::org::{admin_denied, admin_nav, admin_vm};
use super::productivity::{open_app, AppWindow};
use super::*;
use crate::controllers::desktop::text;
use crate::i18n::{t, tf};
use crate::ui::apps::admin_access::{binding_text, dist_name};
use crate::ui::view_models::{
    AdmAccessCellVm, AdmAccessRowVm, AdmAddEndpointVm, AdmConfirmVm, AdmDistRowVm,
    AdmEndpointRowVm, AdmFact, AdminD010Vm, AdminSection, AdminVm, AppError, Distribution,
};
use ocinye_contracts::{ApplicationId, Permission};

fn dist(raw: &str) -> Option<Distribution> {
    match raw {
        "research" => Some(Distribution::Research),
        "business" => Some(Distribution::Business),
        "personal" => Some(Distribution::Personal),
        "education" => Some(Distribution::Education),
        _ => None,
    }
}

/// Um id que vai entrar no caminho de um pedido ao Core: só um UUID.
fn uuid(raw: &str) -> Option<String> {
    uuid::Uuid::parse_str(raw).ok().map(|u| u.to_string())
}

/// A frase de uma recusa que o Core deu por motivo estável.
fn refusal_text(f: &ApiFailure) -> String {
    match f {
        ApiFailure::Refused { reason, message } => match reason.as_str() {
            "last_enabled_distribution" => t("adm.dist.last").to_owned(),
            "last_administrator_access" => t("adm.access.last").to_owned(),
            "endpoint_last_or_canonical" => t("adm.ep.disable.last").to_owned(),
            "endpoint_conflict" => t("adm.ep.err.conflict").to_owned(),
            "endpoint_invalid" => t("adm.ep.err.invalid").to_owned(),
            _ => message.clone(),
        },
        other => other.to_string(),
    }
}

/// A página D010 inteira: a janela, e a confirmação por cima quando há uma.
fn render(w: &AppWindow, vm: &AdminVm, dialog: Option<AnyView>, status: StatusCode) -> Response {
    let content = ui::apps::admin::app(vm);
    if w.page.frame {
        // Igual a `render_org`: uma janela de fundo reposta nunca traz a
        // confirmação (pertence só à janela do pedido).
        let title = w.title.clone();
        let body = view! { <template data-part="win-title">{title}</template>{content} };
        return (
            status,
            [(header::CACHE_CONTROL, "no-store")],
            Html(body.to_html()),
        )
            .into_response();
    }
    let engine = w.ctx.vm.wm.is_some();
    let body = ui::shell::shell_with_window(&w.ctx.vm, ().into_any(), Some(content));
    let mut response = shell_page(&w.title, engine, body, dialog);
    *response.status_mut() = status;
    response
}

async fn window(state: &WorkspaceState, headers: &HeaderMap) -> Result<AppWindow, Response> {
    open_app(state, headers, Screen::Admin, ApplicationId::Administration)
        .await
        .map_err(|r| *r)
}

fn denied_of(state: &WorkspaceState, w: &AppWindow, f: &ApiFailure) -> Response {
    admin_denied(
        state,
        w,
        match f {
            ApiFailure::Forbidden | ApiFailure::Denied => AppError::PermissionDenied,
            _ => AppError::Unavailable,
        },
    )
}

// ── S26 / S38 · Distribuições ──────────────────────────────────────────────

/// `?confirm=disable&d=…`
#[derive(Debug, Default, Deserialize)]
pub(super) struct DistQuery {
    confirm: Option<String>,
    d: Option<String>,
}

fn dist_rows(v: &Value) -> Vec<AdmDistRowVm> {
    let list = v.as_array().cloned().unwrap_or_default();
    let enabled = list
        .iter()
        .filter(|r| text(r, "state") == "enabled")
        .count();
    list.iter()
        .filter_map(|r| {
            let state = text(r, "state").to_owned();
            Some(AdmDistRowVm {
                distribution: dist(text(r, "distribution"))?,
                last: state == "enabled" && enabled == 1,
                state,
                members: r
                    .get("members_with_access")
                    .and_then(Value::as_i64)
                    .unwrap_or(0),
                sessions: r
                    .get("active_sessions")
                    .and_then(Value::as_i64)
                    .unwrap_or(0),
                endpoints: r
                    .get("endpoints")
                    .and_then(Value::as_array)
                    .map(|l| {
                        l.iter()
                            .filter_map(Value::as_str)
                            .map(str::to_owned)
                            .collect()
                    })
                    .unwrap_or_default(),
            })
        })
        .collect()
}

fn dist_disable_confirm(row: &AdmDistRowVm, refusal: Option<String>) -> AdmConfirmVm {
    let name = dist_name(row.distribution);
    AdmConfirmVm {
        icon: "shield",
        title: tf("adm.dist.disable.title", &[("distribution", name)]),
        body: tf("adm.dist.disable.body", &[("distribution", name)]),
        facts: vec![
            (
                t("adm.dist.fact.sessions").to_owned(),
                AdmFact::Text(row.sessions.to_string()),
            ),
            (
                t("adm.dist.fact.endpoints").to_owned(),
                AdmFact::Text(if row.endpoints.is_empty() {
                    "—".to_owned()
                } else {
                    row.endpoints.join(", ")
                }),
            ),
            (
                t("adm.dist.fact.kept").to_owned(),
                AdmFact::Strong(t("adm.dist.fact.kept.value").to_owned()),
            ),
        ],
        action: format!("/admin/distributions/{}/disable", row.distribution.as_str()),
        hidden: Vec::new(),
        refusal: refusal.or_else(|| row.last.then(|| t("adm.dist.last").to_owned())),
        ok_key: "adm.dist.disable",
        cancel: "/admin/distributions".to_owned(),
    }
}

async fn distributions_view(
    state: &WorkspaceState,
    headers: &HeaderMap,
    q: &DistQuery,
    refusal: Option<String>,
) -> Response {
    let w = match window(state, headers).await {
        Ok(w) => w,
        Err(r) => return r,
    };
    if !w.ctx.viewer.can(Permission::OrganisationView) {
        return admin_denied(state, &w, AppError::PermissionDenied);
    }
    let v = match caller(&w.member)
        .get(state, "/api/v1/instance/distributions")
        .await
    {
        Ok(v) => v,
        Err(f) => return denied_of(state, &w, &f),
    };
    let rows = dist_rows(&v);
    let manage = w.ctx.viewer.can(Permission::OrganisationManage);
    let dialog = (manage && q.confirm.as_deref() == Some("disable"))
        .then(|| q.d.as_deref().and_then(dist))
        .flatten()
        .and_then(|d| {
            rows.iter()
                .find(|r| r.distribution == d && r.state == "enabled")
        })
        .map(|row| {
            ui::apps::admin_access::confirm(&dist_disable_confirm(row, refusal.clone())).into_any()
        });
    let status = if refusal.is_some() {
        StatusCode::CONFLICT
    } else {
        StatusCode::OK
    };
    let mut vm = admin_vm(AdminSection::Distributions);
    vm.nav = admin_nav(&w.ctx, AdminSection::Distributions);
    vm.d010 = Some(AdminD010Vm::Distributions { rows, manage });
    render(&w, &vm, dialog, status)
}

pub(super) async fn distributions_page(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(q): Query<DistQuery>,
) -> Response {
    distributions_view(&state, &headers, &q, None).await
}

pub(super) async fn distribution_enable(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(raw): Path<String>,
) -> Response {
    let member = member_or_login!(state, headers);
    let Some(d) = dist(&raw) else {
        return Redirect::to("/admin/distributions").into_response();
    };
    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/instance/distributions/{}/enable", d.as_str()),
        &json!({}),
    )
    .await
    {
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        _ => Redirect::to("/admin/distributions").into_response(),
    }
}

pub(super) async fn distribution_disable(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(raw): Path<String>,
) -> Response {
    let member = member_or_login!(state, headers);
    let Some(d) = dist(&raw) else {
        return Redirect::to("/admin/distributions").into_response();
    };
    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/instance/distributions/{}/disable", d.as_str()),
        &json!({}),
    )
    .await
    {
        Ok(_) => Redirect::to("/admin/distributions").into_response(),
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        Err(f) => {
            let q = DistQuery {
                confirm: Some("disable".to_owned()),
                d: Some(d.as_str().to_owned()),
            };
            distributions_view(&state, &headers, &q, Some(refusal_text(&f))).await
        }
    }
}

// ── S37 · Acesso a Distribuições ───────────────────────────────────────────

/// `?confirm=revoke&person=…&d=…`
#[derive(Debug, Default, Deserialize)]
pub(super) struct AccessQuery {
    confirm: Option<String>,
    person: Option<String>,
    d: Option<String>,
}

fn access_rows(v: &Value, enabled: &[Distribution]) -> Vec<AdmAccessRowVm> {
    let list = v.as_array().cloned().unwrap_or_default();
    // A última via de administração: a única (administrador, Distribuição
    // activada) que resta não se retira (S37). O Core recusa de qualquer modo.
    let admin_pairs: Vec<(String, Distribution)> = list
        .iter()
        .filter(|r| r.get("administrator").and_then(Value::as_bool) == Some(true))
        .flat_map(|r| {
            let id = text(r, "person_id").to_owned();
            r.get("distributions")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter_map(move |d| d.as_str().and_then(dist).map(|d| (id.clone(), d)))
                .collect::<Vec<_>>()
        })
        .collect();
    list.iter()
        .map(|r| {
            let id = text(r, "person_id").to_owned();
            let has: Vec<Distribution> = r
                .get("distributions")
                .and_then(Value::as_array)
                .map(|l| l.iter().filter_map(|d| d.as_str().and_then(dist)).collect())
                .unwrap_or_default();
            AdmAccessRowVm {
                name: text(r, "full_name").to_owned(),
                cells: enabled
                    .iter()
                    .map(|d| AdmAccessCellVm {
                        distribution: *d,
                        has: has.contains(d),
                        protected: admin_pairs.len() == 1 && admin_pairs[0] == (id.clone(), *d),
                        sessions: r
                            .get("active_sessions")
                            .and_then(|s| s.get(d.as_str()))
                            .and_then(Value::as_i64)
                            .unwrap_or(0),
                    })
                    .collect(),
                person_id: id,
            }
        })
        .collect()
}

async fn access_view(
    state: &WorkspaceState,
    headers: &HeaderMap,
    q: &AccessQuery,
    refusal: Option<String>,
) -> Response {
    let w = match window(state, headers).await {
        Ok(w) => w,
        Err(r) => return r,
    };
    if !w.ctx.viewer.can(Permission::OrganisationManage) {
        return admin_denied(state, &w, AppError::PermissionDenied);
    }
    let quem = caller(&w.member);
    let (ds, matrix) = match (
        quem.get(state, "/api/v1/instance/distributions").await,
        quem.get(state, "/api/v1/instance/distribution-access")
            .await,
    ) {
        (Ok(a), Ok(b)) => (a, b),
        (Err(f), _) | (_, Err(f)) => return denied_of(state, &w, &f),
    };
    let enabled: Vec<Distribution> = dist_rows(&ds)
        .into_iter()
        .filter(|r| r.state == "enabled")
        .map(|r| r.distribution)
        .collect();
    let rows = access_rows(&matrix, &enabled);
    let dialog = (q.confirm.as_deref() == Some("revoke"))
        .then(|| {
            let d = q.d.as_deref().and_then(dist)?;
            let p = q.person.as_deref().and_then(uuid)?;
            let row = rows.iter().find(|r| r.person_id == p)?;
            let cell = row.cells.iter().find(|c| c.distribution == d && c.has)?;
            Some(AdmConfirmVm {
                icon: "shield",
                title: format!("{} · {}", t("adm.access.revoke"), dist_name(d)),
                body: tf("dist.revoked.body", &[("distribution", dist_name(d))]),
                facts: vec![
                    (
                        t("org.col.member").to_owned(),
                        AdmFact::Text(row.name.clone()),
                    ),
                    (
                        t("auth.distribution").to_owned(),
                        AdmFact::Text(dist_name(d).to_owned()),
                    ),
                    (
                        t("adm.dist.fact.sessions").to_owned(),
                        AdmFact::Text(cell.sessions.to_string()),
                    ),
                ],
                action: format!("/admin/distribution-access/{p}/{}/revoke", d.as_str()),
                hidden: Vec::new(),
                refusal: refusal
                    .clone()
                    .or_else(|| cell.protected.then(|| t("adm.access.last").to_owned())),
                ok_key: "adm.access.revoke",
                cancel: "/admin/distribution-access".to_owned(),
            })
        })
        .flatten()
        .map(|c| ui::apps::admin_access::confirm(&c).into_any());
    let status = if refusal.is_some() {
        StatusCode::CONFLICT
    } else {
        StatusCode::OK
    };
    let mut vm = admin_vm(AdminSection::DistributionAccess);
    vm.nav = admin_nav(&w.ctx, AdminSection::DistributionAccess);
    vm.d010 = Some(AdminD010Vm::Access {
        dists: enabled,
        rows,
    });
    render(&w, &vm, dialog, status)
}

pub(super) async fn access_page(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(q): Query<AccessQuery>,
) -> Response {
    access_view(&state, &headers, &q, None).await
}

pub(super) async fn access_grant(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path((person, raw)): Path<(String, String)>,
) -> Response {
    let member = member_or_login!(state, headers);
    let (Some(p), Some(d)) = (uuid(&person), dist(&raw)) else {
        return Redirect::to("/admin/distribution-access").into_response();
    };
    match api::put(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/instance/distribution-access/{p}/{}", d.as_str()),
        &json!({}),
    )
    .await
    {
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        _ => Redirect::to("/admin/distribution-access").into_response(),
    }
}

pub(super) async fn access_revoke(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path((person, raw)): Path<(String, String)>,
) -> Response {
    let member = member_or_login!(state, headers);
    let (Some(p), Some(d)) = (uuid(&person), dist(&raw)) else {
        return Redirect::to("/admin/distribution-access").into_response();
    };
    match api::delete(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/instance/distribution-access/{p}/{}", d.as_str()),
    )
    .await
    {
        Ok(_) => Redirect::to("/admin/distribution-access").into_response(),
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        Err(f) => {
            let q = AccessQuery {
                confirm: Some("revoke".to_owned()),
                person: Some(p),
                d: Some(d.as_str().to_owned()),
            };
            access_view(&state, &headers, &q, Some(refusal_text(&f))).await
        }
    }
}

// ── S27–S32 · Pontos de acesso ─────────────────────────────────────────────

/// `?open=…`, `?add=1`, `?confirm=bind&id=…&to=…`, `?confirm=disable&id=…`
#[derive(Debug, Default, Deserialize)]
pub(super) struct EndpointQuery {
    open: Option<String>,
    add: Option<String>,
    confirm: Option<String>,
    id: Option<String>,
    to: Option<String>,
}

fn endpoint_rows(v: &Value) -> Vec<AdmEndpointRowVm> {
    v.as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|e| AdmEndpointRowVm {
            id: text(e, "id").to_owned(),
            host: text(e, "hostname").to_owned(),
            binding: e
                .get("binding_distribution")
                .and_then(Value::as_str)
                .and_then(dist),
            state: text(e, "state").to_owned(),
            tls: text(e, "tls_observation").to_owned(),
            dns: text(e, "dns_observation").to_owned(),
            canonical: e.get("canonical").and_then(Value::as_bool) == Some(true),
        })
        .collect()
}

/// O que um pedido falhado deixa por cima da lista.
enum Overlay {
    None,
    Add(AdmAddEndpointVm),
    Refusal(String),
}

async fn endpoints_view(
    state: &WorkspaceState,
    headers: &HeaderMap,
    q: &EndpointQuery,
    overlay: Overlay,
) -> Response {
    let w = match window(state, headers).await {
        Ok(w) => w,
        Err(r) => return r,
    };
    if !w.ctx.viewer.can(Permission::OrganisationManage) {
        return admin_denied(state, &w, AppError::PermissionDenied);
    }
    let quem = caller(&w.member);
    let (list, ds) = match (
        quem.get(state, "/api/v1/access-endpoints").await,
        quem.get(state, "/api/v1/instance/distributions").await,
    ) {
        (Ok(a), Ok(b)) => (a, b),
        (Err(f), _) | (_, Err(f)) => return denied_of(state, &w, &f),
    };
    let rows = endpoint_rows(&list);
    let enabled: Vec<Distribution> = dist_rows(&ds)
        .into_iter()
        .filter(|r| r.state == "enabled")
        .map(|r| r.distribution)
        .collect();
    let target =
        q.id.as_deref()
            .or(q.open.as_deref())
            .and_then(uuid)
            .and_then(|id| rows.iter().find(|r| r.id == id).cloned());
    let mut status = StatusCode::OK;
    let dialog = match overlay {
        Overlay::Add(form) => {
            status = StatusCode::UNPROCESSABLE_ENTITY;
            Some(ui::apps::admin_access::add_endpoint(&form).into_any())
        }
        overlay => {
            let refusal = match overlay {
                Overlay::Refusal(r) => {
                    status = StatusCode::CONFLICT;
                    Some(r)
                }
                _ => None,
            };
            if q.add.is_some() {
                Some(
                    ui::apps::admin_access::add_endpoint(&AdmAddEndpointVm {
                        host: String::new(),
                        binding: "generic".to_owned(),
                        error: None,
                        dists: enabled.clone(),
                    })
                    .into_any(),
                )
            } else {
                match (q.confirm.as_deref(), &target) {
                    (Some("bind"), Some(e)) => {
                        let to = match q.to.as_deref() {
                            Some("generic") | None => None,
                            Some(raw) => dist(raw),
                        };
                        Some(AdmConfirmVm {
                            icon: "link",
                            title: tf("adm.ep.bind.title", &[("host", e.host.as_str())]),
                            body: tf("adm.ep.bind.body", &[("host", e.host.as_str())]),
                            facts: vec![
                                (
                                    t("org.confirm.current").to_owned(),
                                    AdmFact::Text(binding_text(e.binding)),
                                ),
                                (
                                    t("org.confirm.proposed").to_owned(),
                                    AdmFact::Strong(binding_text(to)),
                                ),
                            ],
                            action: format!("/admin/endpoints/{}/binding", e.id),
                            hidden: vec![(
                                "binding".to_owned(),
                                to.map_or("generic", Distribution::as_str).to_owned(),
                            )],
                            refusal,
                            ok_key: "adm.ep.bind.go",
                            cancel: format!("/admin/endpoints?open={}", e.id),
                        })
                    }
                    (Some("disable"), Some(e)) => Some(AdmConfirmVm {
                        icon: "shield",
                        title: tf("adm.ep.disable.title", &[("host", e.host.as_str())]),
                        body: t("adm.ep.disable.body").to_owned(),
                        facts: vec![
                            (
                                t("adm.ep.col.host").to_owned(),
                                AdmFact::Code(e.host.clone()),
                            ),
                            (
                                t("adm.ep.col.binding").to_owned(),
                                AdmFact::Text(binding_text(e.binding)),
                            ),
                        ],
                        action: format!("/admin/endpoints/{}/disable", e.id),
                        hidden: Vec::new(),
                        refusal: refusal.or_else(|| {
                            let others = rows.iter().any(|r| r.id != e.id && r.state == "active");
                            (e.canonical || (e.state == "active" && !others))
                                .then(|| t("adm.ep.disable.last").to_owned())
                        }),
                        ok_key: "adm.dist.disable",
                        cancel: format!("/admin/endpoints?open={}", e.id),
                    }),
                    _ => None,
                }
                .map(|c| ui::apps::admin_access::confirm(&c).into_any())
            }
        }
    };
    let mut vm = admin_vm(AdminSection::Endpoints);
    vm.nav = admin_nav(&w.ctx, AdminSection::Endpoints);
    vm.d010 = Some(AdminD010Vm::Endpoints {
        rows,
        open: target,
        enabled,
        manage: true,
    });
    render(&w, &vm, dialog, status)
}

pub(super) async fn endpoints_page(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(q): Query<EndpointQuery>,
) -> Response {
    endpoints_view(&state, &headers, &q, Overlay::None).await
}

pub(super) async fn endpoint_create(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    corpo: axum::body::Bytes,
) -> Response {
    let member = member_or_login!(state, headers);
    let mut host = String::new();
    let mut binding = "generic".to_owned();
    for (k, v) in url::form_urlencoded::parse(&corpo) {
        match k.as_ref() {
            "host" => host = v.trim().to_owned(),
            "binding" if v == "generic" || dist(&v).is_some() => binding = v.into_owned(),
            _ => {}
        }
    }
    let result = api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        "/api/v1/access-endpoints",
        &json!({ "hostname": host, "binding": binding }),
    )
    .await;
    match result {
        Ok(e) => Redirect::to(&format!("/admin/endpoints?open={}", text(&e, "id"))).into_response(),
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        Err(f) => {
            let error = match &f {
                ApiFailure::Refused { reason, .. } if reason == "endpoint_conflict" => {
                    "adm.ep.err.conflict"
                }
                ApiFailure::Refused { reason, .. } if reason == "endpoint_invalid" => {
                    "adm.ep.err.invalid"
                }
                ApiFailure::Rejected(_) => "adm.ep.err.invalid",
                ApiFailure::Forbidden | ApiFailure::Denied => {
                    return Redirect::to("/admin/endpoints").into_response()
                }
                _ => "adm.ep.err.invalid",
            };
            // As Distribuições activadas para os botões de rádio: as mesmas que a
            // página mostraria.
            let dists = api::get::<Value>(
                &state,
                &member.session.access_token,
                &member.correlation_id,
                "/api/v1/instance/distributions",
            )
            .await
            .map(|v| {
                dist_rows(&v)
                    .into_iter()
                    .filter(|r| r.state == "enabled")
                    .map(|r| r.distribution)
                    .collect()
            })
            .unwrap_or_default();
            endpoints_view(
                &state,
                &headers,
                &EndpointQuery::default(),
                Overlay::Add(AdmAddEndpointVm {
                    host,
                    binding,
                    error: Some(error),
                    dists,
                }),
            )
            .await
        }
    }
}

/// `observe` | `activate` | `binding` | `disable`.
pub(super) async fn endpoint_action(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path((id, action)): Path<(String, String)>,
    corpo: axum::body::Bytes,
) -> Response {
    let member = member_or_login!(state, headers);
    let Some(id) = uuid(&id) else {
        return Redirect::to("/admin/endpoints").into_response();
    };
    let body = match action.as_str() {
        "observe" | "activate" | "disable" => json!({}),
        "binding" => {
            let raw = url::form_urlencoded::parse(&corpo)
                .find(|(k, _)| k == "binding")
                .map(|(_, v)| v.into_owned())
                .unwrap_or_default();
            if raw != "generic" && dist(&raw).is_none() {
                return Redirect::to(&format!("/admin/endpoints?open={id}")).into_response();
            }
            json!({ "binding": raw })
        }
        _ => return Redirect::to("/admin/endpoints").into_response(),
    };
    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/access-endpoints/{id}/{action}"),
        &body,
    )
    .await
    {
        Ok(_) => Redirect::to(&format!("/admin/endpoints?open={id}")).into_response(),
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        Err(f) if matches!(action.as_str(), "binding" | "disable") => {
            let to = body
                .get("binding")
                .and_then(Value::as_str)
                .map(str::to_owned);
            let q = EndpointQuery {
                confirm: Some(
                    if action == "binding" {
                        "bind"
                    } else {
                        "disable"
                    }
                    .to_owned(),
                ),
                id: Some(id),
                to,
                ..EndpointQuery::default()
            };
            endpoints_view(&state, &headers, &q, Overlay::Refusal(refusal_text(&f))).await
        }
        Err(_) => Redirect::to(&format!("/admin/endpoints?open={id}")).into_response(),
    }
}

// ── S33 · Predefinições ────────────────────────────────────────────────────

pub(super) async fn defaults_page(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
) -> Response {
    let w = match window(&state, &headers).await {
        Ok(w) => w,
        Err(r) => return r,
    };
    if !w.ctx.viewer.can(Permission::OrganisationView) {
        return admin_denied(&state, &w, AppError::PermissionDenied);
    }
    let ds = match caller(&w.member)
        .get(&state, "/api/v1/instance/distributions")
        .await
    {
        Ok(v) => v,
        Err(f) => return denied_of(&state, &w, &f),
    };
    let dists = dist_rows(&ds)
        .into_iter()
        .filter(|r| r.state == "enabled")
        .map(|r| r.distribution)
        .collect();
    let mut vm = admin_vm(AdminSection::Defaults);
    vm.nav = admin_nav(&w.ctx, AdminSection::Defaults);
    vm.d010 = Some(AdminD010Vm::Defaults { dists });
    render(&w, &vm, None, StatusCode::OK)
}
