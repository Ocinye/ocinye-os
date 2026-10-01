//! D010 · Entrada por Distribuição, bloqueio, e os estados de meio da sessão.
//!
//! - `GET /distribution` — S09 (várias), ou a decisão directa (uma), ou S10;
//!   `POST /distribution` — entrar na escolhida (o Core decide).
//! - `GET /lock` · `POST /lock` · `POST /unlock` — S22.
//!
//! O seletor ignora a Distribuição que a sessão tinha guardada quando ela
//! deixou de valer: é por aqui que «Voltar à entrada» (S18, S39) volta.

use axum::extract::{Form, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use leptos::prelude::*;
use serde::Deserialize;

use super::*;
use crate::controllers::{self, Entry};
use crate::ui::screens::auth::access as screens;
use crate::ui::view_models::Surface;

fn dist_of(d: ocinye_contracts::Distribution) -> crate::ui::view_models::Distribution {
    controllers::distribution_of(d.as_str())
        .unwrap_or(crate::ui::view_models::Distribution::Research)
}

/// Fecha todas as janelas desta sessão: a Distribuição em que estavam já não
/// vale (S18, S39) ou mudou (S16).
pub(crate) fn close_all_windows(state: &WorkspaceState, session_id: &str) {
    state
        .sessions
        .with_desk(session_id, |desk| desk.retain_apps(|_| false));
}

/// O que uma página mostra quando a sessão não está (ou já não está) numa
/// Distribuição.
pub(crate) async fn entry_response(
    state: &WorkspaceState,
    headers: &HeaderMap,
    entry: Entry,
) -> Response {
    if let Entry::Choose(_) = entry {
        return Redirect::to("/distribution").into_response();
    }
    let door = controllers::door(state).await;
    if matches!(entry, Entry::Revoked(_) | Entry::DisabledLive(_)) {
        if let Some(member) = current_member(state, headers) {
            close_all_windows(state, &member.session_id);
        }
    }
    let (title, body): (String, leptos::prelude::AnyView) = match entry {
        Entry::Choose(_) => unreachable!(),
        Entry::Zero => (
            crate::i18n::t("dist.zero.title").to_owned(),
            screens::zero(&door).into_any(),
        ),
        Entry::BoundNoAccess(d) => (
            crate::i18n::tf(
                "dist.bound.noaccess.title",
                &[("distribution", screens::dist_name(dist_of(d)))],
            ),
            screens::bound_no_access(&door, dist_of(d)).into_any(),
        ),
        Entry::BoundDisabled(d) => (
            crate::i18n::tf(
                "dist.bound.disabled.title",
                &[("distribution", screens::dist_name(dist_of(d)))],
            ),
            screens::bound_disabled(&door, dist_of(d)).into_any(),
        ),
        Entry::Revoked(d) => (
            crate::i18n::tf(
                "dist.revoked.title",
                &[("distribution", screens::dist_name(dist_of(d)))],
            ),
            screens::revoked(&door, dist_of(d)).into_any(),
        ),
        Entry::DisabledLive(d) => (
            crate::i18n::tf(
                "dist.disabled_live.title",
                &[("distribution", screens::dist_name(dist_of(d)))],
            ),
            screens::disabled_live(&door, dist_of(d)).into_any(),
        ),
    };
    (StatusCode::FORBIDDEN, html(&title, Surface::Auth, body)).into_response()
}

/// `GET /distribution` — S09: escolher entre as acessíveis (nunca contextos).
pub(crate) async fn choose_page(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
) -> Response {
    let member = member_or_login!(state, headers);
    let quem = caller(&member);
    let mine = match quem.get(&state, "/api/v1/me/distributions").await {
        Ok(v) => v,
        Err(ApiFailure::Unauthorised) => return session_ended(&state, &headers),
        Err(other) => {
            return identity_indeterminate(&state, controllers::reference(&format!("{other}")), "/")
                .await
        }
    };
    let accessible: Vec<ocinye_contracts::Distribution> = mine
        .get("accessible")
        .and_then(serde_json::Value::as_array)
        .map(|a| a.iter().filter_map(|v| v.as_str()?.parse().ok()).collect())
        .unwrap_or_default();
    // Num ponto fixo não há escolha: a página da Distribuição do ponto decide.
    if crate::access::current().and_then(|e| e.bound()).is_some() {
        return Redirect::to("/").into_response();
    }
    match accessible.as_slice() {
        [] => entry_response(&state, &headers, Entry::Zero).await,
        [only] => match controllers::enter(&state, &quem, *only).await {
            Ok(()) => Redirect::to("/").into_response(),
            Err(controllers::Shell::Entry(e)) => entry_response(&state, &headers, e).await,
            Err(_) => session_ended(&state, &headers),
        },
        many => {
            let door = controllers::door(&state).await;
            let choices: Vec<_> = many.iter().copied().map(dist_of).collect();
            html(
                crate::i18n::t("dist.select.title"),
                Surface::Auth,
                screens::selector(&door, &choices),
            )
        }
    }
}

#[derive(Deserialize)]
pub(crate) struct ChooseForm {
    distribution: String,
}

/// `POST /distribution` — entrar na escolhida. O Core recusa a que o membro
/// não pode abrir; um valor fora das quatro é recusado aqui.
pub(crate) async fn choose_submit(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<ChooseForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let quem = caller(&member);
    let Ok(d) = form.distribution.parse::<ocinye_contracts::Distribution>() else {
        return Redirect::to("/distribution").into_response();
    };
    if let Some(bound) = crate::access::current().and_then(|e| e.bound()) {
        if bound != d {
            return Redirect::to("/").into_response();
        }
    }
    // Só se escolhe entre as que o seleccionador mostrou; outra volta a ele.
    match controllers::ensure_entry(&state, &quem).await {
        Ok(dists) if !dists.accessible.contains(&d) => {
            return Redirect::to("/distribution").into_response()
        }
        Err(controllers::Shell::Entry(controllers::Entry::Choose(list))) if !list.contains(&d) => {
            return Redirect::to("/distribution").into_response()
        }
        Ok(_) | Err(controllers::Shell::Entry(controllers::Entry::Choose(_))) => {}
        Err(controllers::Shell::Entry(e)) => return entry_response(&state, &headers, e).await,
        Err(_) => return session_ended(&state, &headers),
    }
    match controllers::enter(&state, &quem, d).await {
        Ok(()) => {
            close_all_windows(&state, &member.session_id);
            Redirect::to("/").into_response()
        }
        Err(controllers::Shell::Entry(e)) => entry_response(&state, &headers, e).await,
        Err(_) => session_ended(&state, &headers),
    }
}

/// `POST /lock` — S22: bloqueia o ecrã. As janelas ficam, escondidas.
pub(crate) async fn lock_submit(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
) -> Response {
    if let Some(member) = current_member(&state, &headers) {
        state
            .sessions
            .update(&member.session_id, |s| s.locked = true);
    }
    Redirect::to("/lock").into_response()
}

#[derive(Deserialize, Default)]
pub(crate) struct LockQuery {
    failed: Option<String>,
}

/// `GET /lock` — o ecrã de bloqueio.
pub(crate) async fn lock_page(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    axum::extract::Query(q): axum::extract::Query<LockQuery>,
) -> Response {
    let Some(member) = current_member(&state, &headers) else {
        return Redirect::to("/login").into_response();
    };
    if !member.session.locked {
        return Redirect::to("/").into_response();
    }
    let door = controllers::door(&state).await;
    let mut response = html(
        crate::i18n::t("lock.title"),
        Surface::Auth,
        screens::lock(&door, &member.session.display_name, q.failed.is_some()),
    );
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    response
}

#[derive(Deserialize)]
pub(crate) struct UnlockForm {
    password: String,
}

/// Quantas falhas de desbloqueio numa sessão antes de a terminar.
const UNLOCK_ATTEMPTS: u8 = 5;

/// `POST /unlock` — o Core confirma a palavra-passe (com o limite da entrada).
pub(crate) async fn unlock_submit(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<UnlockForm>,
) -> Response {
    let Some(member) = current_member(&state, &headers) else {
        return Redirect::to("/login").into_response();
    };
    let answer = api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        "/api/v1/auth/reauthenticate",
        &serde_json::json!({ "password": form.password }),
    )
    .await;
    match answer {
        Ok(_) => {
            state.sessions.update(&member.session_id, |s| {
                s.locked = false;
                s.unlock_failures = 0;
            });
            Redirect::to("/").into_response()
        }
        Err(ApiFailure::Forbidden | ApiFailure::Denied)
            if member.session.unlock_failures + 1 < UNLOCK_ATTEMPTS =>
        {
            state
                .sessions
                .update(&member.session_id, |s| s.unlock_failures += 1);
            Redirect::to("/lock?failed=1").into_response()
        }
        // À quinta falha, ou com o limite do Core atingido, a sessão termina.
        _ => {
            let _ = api::post(
                &state,
                &member.session.access_token,
                &member.correlation_id,
                "/api/v1/auth/logout",
                &serde_json::json!({}),
            )
            .await;
            session_ended(&state, &headers)
        }
    }
}

/// Um caminho local para voltar depois de uma mudança: começa por `/`, não é
/// um endereço de outro sítio (`//…`, `\`), sem esquema. Nunca um open redirect.
#[must_use]
pub(crate) fn local_path(raw: &str) -> Option<String> {
    let p = raw.trim();
    (p.starts_with('/') && !p.starts_with("//") && !p.contains('\\') && !p.contains("://"))
        .then(|| p.to_owned())
}

#[derive(Deserialize)]
pub(crate) struct SwitchQuery {
    to: String,
    #[serde(default, rename = "return")]
    back: Option<String>,
}

/// `GET /distribution/switch?to=…` — S15 → S16: a confirmação, por cima do
/// Desktop. Não muda nada (um `GET` nunca muda estado).
pub(crate) async fn switch_page(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    axum::extract::Query(q): axum::extract::Query<SwitchQuery>,
) -> Response {
    let _member = member_or_login!(state, headers);
    let Ok(d) = q.to.parse::<ocinye_contracts::Distribution>() else {
        return Redirect::to("/").into_response();
    };
    let back = q
        .back
        .as_deref()
        .and_then(local_path)
        .map(|b| format!("&return={}", crate::controllers::research::encode(&b)))
        .unwrap_or_default();
    Redirect::to(&format!("/?switch={}{back}", d.as_str())).into_response()
}

#[derive(Deserialize)]
pub(crate) struct SwitchForm {
    to: String,
    #[serde(default, rename = "return")]
    back: Option<String>,
}

/// `POST /distribution/switch` — S16 confirmada. Primeiro o Core autoriza o
/// destino (recusa → S17, nada fecha); depois cada janela por gravar passa
/// pelo `dirty_close` do D002 (cancelar uma aborta); só então as janelas da
/// anterior fecham, o contexto é reposto (pelo Core) e o Desktop do destino
/// abre numa entrada nova do histórico.
pub(crate) async fn switch_submit(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<SwitchForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let quem = caller(&member);
    let Ok(d) = form.to.parse::<ocinye_contracts::Distribution>() else {
        return Redirect::to("/").into_response();
    };
    // Num ponto fixo não se muda localmente (ADR-0625 §4): o painel oferece
    // os endereços configurados.
    if crate::access::current().and_then(|e| e.bound()).is_some() {
        return Redirect::to("/").into_response();
    }
    let dists = match controllers::ensure_entry(&state, &quem).await {
        Ok(d) => d,
        Err(controllers::Shell::Entry(e)) => return entry_response(&state, &headers, e).await,
        Err(_) => return session_ended(&state, &headers),
    };
    if dists.active == Some(d) {
        return Redirect::to("/").into_response();
    }
    if !dists.accessible.contains(&d) {
        return Redirect::to(&format!("/?switch_refused={}", d.as_str())).into_response();
    }
    // Uma janela por gravar de cada vez, pelo diálogo do D002.
    let dirty = state
        .sessions
        .with_desk(&member.session_id, |desk| {
            desk.windows()
                .iter()
                .find(|w| w.dirty)
                .map(|w| w.id.clone())
        })
        .flatten();
    if let Some(id) = dirty {
        return Redirect::to(&format!("/?close={id}&after=switch:{}", d.as_str())).into_response();
    }
    match controllers::enter(&state, &quem, d).await {
        Ok(()) => {
            close_all_windows(&state, &member.session_id);
            let back = form
                .back
                .as_deref()
                .and_then(local_path)
                .unwrap_or_else(|| "/".to_owned());
            Redirect::to(&back).into_response()
        }
        Err(controllers::Shell::SignIn) => session_ended(&state, &headers),
        Err(_) => Redirect::to(&format!("/?switch_refused={}", d.as_str())).into_response(),
    }
}

#[derive(Deserialize)]
pub(crate) struct ContextForm {
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    id: Option<String>,
}

/// `POST /context` — S19: escolher o contexto (o Core decide se é do membro).
/// Não muda a Distribuição. Volta à página de onde veio (só um caminho local).
pub(crate) async fn context_submit(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<ContextForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let body = serde_json::json!({
        "kind": form.kind.as_deref().filter(|k| !k.is_empty()),
        "id": form.id.as_deref().filter(|i| !i.is_empty()),
    });
    let _ = api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        "/api/v1/me/context",
        &body,
    )
    .await;
    let back = headers
        .get(header::REFERER)
        .and_then(|v| v.to_str().ok())
        .and_then(|r| r.split_once("://").map(|(_, rest)| rest))
        .and_then(|rest| rest.find('/').map(|i| &rest[i..]))
        .and_then(local_path)
        .unwrap_or_else(|| "/".to_owned());
    Redirect::to(&back).into_response()
}

/// Os pedidos do Desktop que a D010 acrescenta (`/?switch=…` e afins).
#[derive(Deserialize, Default)]
pub(crate) struct HomeQuery {
    #[serde(default)]
    switch: Option<String>,
    #[serde(default)]
    switch_refused: Option<String>,
    #[serde(default)]
    switched: Option<String>,
    #[serde(default)]
    deeplink: Option<String>,
    #[serde(default, rename = "return")]
    back: Option<String>,
    #[serde(default)]
    after: Option<String>,
}

impl HomeQuery {
    /// A continuação de uma mudança, para o diálogo de fechar do D002.
    pub(crate) fn after(&self) -> Option<String> {
        self.after
            .as_deref()
            .filter(|a| {
                a.strip_prefix("switch:")
                    .is_some_and(|d| d.parse::<ocinye_contracts::Distribution>().is_ok())
            })
            .map(str::to_owned)
    }
}

/// O diálogo D010 a desenhar por cima do Desktop, se o pedido traz um.
pub(crate) fn home_dialog(
    state: &WorkspaceState,
    session_id: &str,
    ctx: &controllers::ShellContext,
    q: &HomeQuery,
) -> Option<AnyView> {
    use crate::i18n::{t, tf};
    use crate::ui::components::icon;
    let current = ctx.distribution?;
    let cur_name = screens::dist_name(current);
    let parse = |v: &Option<String>| {
        v.as_deref()
            .and_then(|d| d.parse::<ocinye_contracts::Distribution>().ok())
    };
    let sheet = |part: &'static str,
                 title: String,
                 lead: String,
                 extra: AnyView,
                 actions: AnyView| {
        view! {
            <dialog class="oc-sheet oc-sheet--narrow" data-part=part data-d010="" data-oc="auto-open" data-cancel="/" aria-labelledby=format!("{part}-t")>
                <header class="oc-sheet__head">
                    <h2 id=format!("{part}-t")>{title}</h2>
                    <a class="oc-round-btn" href="/" aria-label=t("shell.close")>{icon("close")}</a>
                </header>
                <p class="oc-sheet__lead">{lead}</p>
                {extra}
                {actions}
            </dialog>
        }
        .into_any()
    };
    // Num ponto fixo não se muda localmente (ADR-0625 §4): nem a folha de
    // mudar nem a da ligação profunda — o painel do distintivo oferece os
    // endereços configurados (S17), e o `POST` recusa de qualquer modo.
    let bound = crate::access::current().and_then(|e| e.bound()).is_some();
    if let Some(d) = parse(&q.switch) {
        if bound || !ctx.dists.accessible.contains(&d) || ctx.dists.active == Some(d) {
            return None;
        }
        let name = screens::dist_name(dist_of(d));
        let (windows, dirty): (usize, Vec<String>) = state
            .sessions
            .with_desk(session_id, |desk| {
                let w = desk.windows();
                (
                    w.len(),
                    w.iter()
                        .filter(|x| x.dirty)
                        .map(|x| {
                            crate::controllers::windows::application_of_id(x.app)
                                .map_or_else(|| x.app.as_str().to_owned(), |a| a.label().to_owned())
                        })
                        .collect(),
                )
            })
            .unwrap_or_default();
        let back = q.back.as_deref().and_then(local_path).unwrap_or_default();
        let extra = view! {
            <p class="oc-sheet__origin" data-source="distribution">{t("dist.switch.title").to_uppercase()}</p>
            {(windows > 0).then(|| view! { <p class="oc-sheet__lead" data-part="dist-switch-windows">{tf("dist.switch.windows", &[("n", windows.to_string().as_str())])}</p> })}
            {(!dirty.is_empty()).then(|| view! {
                <div class="oc-app-note" data-tone="warn" data-part="dist-switch-dirty" role="note">
                    {icon("warning")}
                    <span>
                        <strong>{t("dist.switch.dirty.title")}</strong>" "{tf("dist.switch.dirty.body", &[("distribution", name)])}
                        <ul class="oc-access-list">{dirty.iter().map(|l| view! { <li>{l.clone()}</li> }).collect_view()}</ul>
                    </span>
                </div>
            })}
            <p class="oc-sheet__note">{t("dist.authority")}</p>
        }
        .into_any();
        let go = if dirty.is_empty() {
            t("dist.switch.confirm.go")
        } else {
            t("dist.switch.dirty.review")
        };
        let go_icon = if dirty.is_empty() {
            crate::experience::iconography::dist_icon_id(dist_of(d))
        } else {
            "edit"
        };
        let actions = view! {
            <form class="oc-sheet__actions" method="post" action="/distribution/switch">
                <input type="hidden" name="to" value=d.as_str() />
                {(!back.is_empty()).then(|| view! { <input type="hidden" name="return" value=back.clone() /> })}
                <a class="oc-btn-line" href="/" data-oc="dialog-close">{t("desk.cancel")}</a>
                <button type="submit" class="oc-btn-solid">{icon(go_icon)}{go}</button>
            </form>
        }
        .into_any();
        return Some(sheet(
            "dist-switch-confirm",
            tf("dist.switch.confirm.title", &[("distribution", name)]),
            tf(
                "dist.switch.confirm.body",
                &[("distribution", name), ("current", cur_name)],
            ),
            extra,
            actions,
        ));
    }
    if let Some(d) = parse(&q.switch_refused) {
        let name = screens::dist_name(dist_of(d));
        return Some(sheet(
            "dist-target",
            tf("dist.target.title", &[("distribution", name)]),
            tf("dist.target.body", &[("distribution", name), ("current", cur_name)]),
            ().into_any(),
            view! { <div class="oc-sheet__actions"><a class="oc-btn-solid" href="/" data-oc="dialog-close">{t("shell.close")}</a></div> }.into_any(),
        ));
    }
    if q.deeplink.as_deref() == Some("deny") {
        return Some(sheet(
            "deeplink-deny",
            t("deeplink.deny.title").to_owned(),
            t("deeplink.deny.body").to_owned(),
            ().into_any(),
            view! { <div class="oc-sheet__actions"><a class="oc-btn-solid" href="/" data-oc="dialog-close">{t("shell.close")}</a></div> }.into_any(),
        ));
    }
    if let Some(d) = parse(&q.deeplink) {
        if bound || !ctx.dists.accessible.contains(&d) {
            return None;
        }
        let name = screens::dist_name(dist_of(d));
        let windows = state
            .sessions
            .with_desk(session_id, |desk| desk.windows().len())
            .unwrap_or_default();
        let back = q
            .back
            .as_deref()
            .and_then(local_path)
            .unwrap_or_else(|| "/".to_owned());
        return Some(sheet(
            "deeplink-switch",
            tf("deeplink.switch.title", &[("distribution", name)]),
            tf("deeplink.switch.body", &[("distribution", name), ("current", cur_name)]),
            view! { <p class="oc-sheet__note">{tf("dist.switch.windows", &[("n", windows.to_string().as_str())])}" "{t("dist.authority")}</p> }.into_any(),
            view! {
                <form class="oc-sheet__actions" method="post" action="/distribution/switch">
                    <input type="hidden" name="to" value=d.as_str() />
                    <input type="hidden" name="return" value=back />
                    <a class="oc-btn-line" href="/" data-oc="dialog-close">{t("desk.cancel")}</a>
                    <button type="submit" class="oc-btn-solid">{icon(crate::experience::iconography::dist_icon_id(dist_of(d)))}{t("deeplink.switch.go")}</button>
                </form>
            }
            .into_any(),
        ));
    }
    if q.switched.as_deref() == Some("aborted") {
        return Some(
            view! {
                <p class="oc-app-note oc-access-toast" role="status" data-part="dist-switch-aborted" data-d010="">
                    {icon("check")}<span>{tf("dist.switch.aborted", &[("distribution", cur_name)])}</span>
                </p>
            }
            .into_any(),
        );
    }
    None
}
