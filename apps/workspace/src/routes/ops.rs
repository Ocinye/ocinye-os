//! D007 · As nove aplicações de conclusão no Workspace — Mensagens, IA,
//! Agentes, Computação, Meus Recursos, Actividade, Auditoria, Definições e
//! Ajuda —, cada uma numa janela gerida (D002) com o ecrã do Design.
//!
//! Aqui só há canal. Cada página relê do Core, com a sessão do membro, tudo o
//! que mostra; cada alvo de uma relação (o evento da Actividade, o registo da
//! Auditoria, o agente da Nye) relê-se com a autoridade de agora. As acções
//! que reduzem acesso (sair de um grupo, retirar alguém) passam pela
//! confirmação partilhada da D006, e o Core decide outra vez no POST.

use super::org::render_org;
use super::productivity::{clock_of, open_app, AppWindow};
use super::*;
use crate::controllers::desktop::text;
use crate::controllers::ops as op;
use crate::controllers::org as og;
use crate::controllers::productivity::app_error;
use crate::controllers::research as rs;
use crate::i18n::t;
use crate::ui::view_models::{
    ActivityVm, AgentsVm, AiSection, AiVm, AppError, AppLoad, AppPageVm, AuditVm, ComputeVm,
    HelpSection, HelpVm, MessagesVm, MsgComposerVm, MsgKind, MsgNewVm, MsgThreadVm, OrgActionKind,
    OrgConfirmVm, ResListVm, ResOptionVm, ResPane, ResourcesVm, SettingsSection, SettingsVm,
};
use ocinye_contracts::ApplicationId;

fn pane(open: bool) -> ResPane {
    if open {
        ResPane::Detail
    } else {
        ResPane::List
    }
}

fn status_of(e: Option<AppError>) -> StatusCode {
    match e {
        Some(AppError::NotFound) => StatusCode::NOT_FOUND,
        Some(AppError::PermissionDenied) => StatusCode::FORBIDDEN,
        _ => StatusCode::OK,
    }
}

async fn post_json(
    w: &AppWindow,
    state: &WorkspaceState,
    path: &str,
    body: &Value,
) -> Result<Value, ApiFailure> {
    api::post(
        state,
        &w.member.session.access_token,
        &w.member.correlation_id,
        path,
        body,
    )
    .await
}

async fn member_post(
    state: &WorkspaceState,
    member: &Member,
    path: &str,
    body: &Value,
) -> Result<Value, ApiFailure> {
    api::post(
        state,
        &member.session.access_token,
        &member.correlation_id,
        path,
        body,
    )
    .await
}

// ═════════════════════════════════════════════════════════════════════════
// Mensagens
// ═════════════════════════════════════════════════════════════════════════

#[derive(Deserialize, Default)]
pub(super) struct MsgQuery {
    /// «Nova conversa».
    #[serde(default)]
    new: Option<String>,
    /// Mensagens anteriores a este instante.
    #[serde(default)]
    before: Option<String>,
    /// Responder a esta mensagem.
    #[serde(default)]
    reply: Option<String>,
    /// A acção cuja confirmação se abre (`msg_leave`, `msg_remove`).
    #[serde(default)]
    confirm: Option<String>,
    #[serde(default)]
    person: Option<String>,
}

/// O esqueleto da aplicação: a lista de conversas do membro.
async fn messages_vm(state: &WorkspaceState, w: &AppWindow, open: Option<&str>) -> MessagesVm {
    let quem = caller(&w.member);
    let clock = clock_of(&w.ctx);
    let (conversations, load) = match quem.get(state, "/api/v1/messaging/conversations").await {
        Ok(v) => (
            op::conv_rows(&op::list_of(&v), open, &clock),
            AppLoad::Ready,
        ),
        Err(f) => (Vec::new(), AppLoad::Failed(app_error(&f))),
    };
    MessagesVm {
        conversations,
        load,
        pane: pane(open.is_some()),
        thread: None,
        thread_error: None,
        new: None,
        new_href: Some("/messages?new=1".to_owned()),
        list_href: "/messages".to_owned(),
    }
}

/// `GET /messages`
pub(super) async fn messages_page(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(q): Query<MsgQuery>,
) -> Response {
    let w = match open_app(&state, &headers, Screen::Messaging, ApplicationId::Messages).await {
        Ok(w) => w,
        Err(r) => return *r,
    };
    let mut vm = messages_vm(&state, &w, None).await;
    if q.new.is_some() {
        // Começar uma conversa pede os candidatos elegíveis, que o Core ainda
        // não tem (MSG-06): o directório da Instância não se enumera aqui.
        vm.pane = ResPane::Detail;
        vm.new = Some(MsgNewVm {
            search_action: "/messages".to_owned(),
            query: String::new(),
            candidates: None,
            action: "/messages/start".to_owned(),
            unavailable: true,
            error: None,
        });
    }
    render_org(
        &state,
        &w,
        StatusCode::OK,
        ui::apps::messages::app(&vm),
        None,
        None,
    )
}

/// O fio de uma conversa, com o compositor. `draft` = (texto, chave, erro)
/// quando um envio falhou e volta intacto.
async fn thread_of(
    state: &WorkspaceState,
    w: &AppWindow,
    conversation: &str,
    q: &MsgQuery,
    draft: Option<(String, String, Option<String>, AppError)>,
) -> Result<(MsgThreadVm, Option<OrgConfirmVm>), AppError> {
    let quem = caller(&w.member);
    let clock = clock_of(&w.ctx);
    let conv = quem
        .get(
            state,
            &format!("/api/v1/messaging/conversations/{conversation}"),
        )
        .await
        .map_err(|f| app_error(&f))?;
    let before = q
        .before
        .as_deref()
        .filter(|b| chrono::DateTime::parse_from_rfc3339(b).is_ok());
    let path = before.map_or_else(
        || format!("/api/v1/messaging/conversations/{conversation}/messages"),
        |b| {
            format!(
                "/api/v1/messaging/conversations/{conversation}/messages?before={}",
                rs::encode(b)
            )
        },
    );
    let (page, load) = match quem.get(state, &path).await {
        Ok(v) => (op::list_of(&v), AppLoad::Ready),
        Err(f) => (Vec::new(), AppLoad::Failed(app_error(&f))),
    };
    let me = super::eu_id(state, &w.member).await.to_string();
    // Ler é abrir: a marca de leitura avança até à mais recente que se vê.
    if before.is_none() {
        if let Some(newest) = page
            .first()
            .map(|m| text(m, "created_at"))
            .filter(|s| !s.is_empty())
        {
            let _ = member_post(
                state,
                &w.member,
                &format!("/api/v1/messaging/conversations/{conversation}/read"),
                &serde_json::json!({ "until": newest }),
            )
            .await;
        }
    }
    let kind = op::msg_kind(text(&conv, "kind")).unwrap_or(MsgKind::Direct);
    let (members, leave) = op::members(&conv, &me);
    let presence = (kind == MsgKind::Direct)
        .then(|| {
            conv.get("participants")
                .and_then(Value::as_array)
                .and_then(|ps| ps.iter().find(|p| text(p, "id") != me))
                .and_then(|p| op::presence(p.get("presence").and_then(Value::as_str)))
        })
        .flatten();
    let (body, key, reply_id, error) = match draft {
        Some((b, k, r, e)) => (b, k, r, Some(e)),
        None => (
            String::new(),
            Uuid::new_v4().to_string(),
            q.reply.clone(),
            None,
        ),
    };
    let reply = reply_id.as_deref().and_then(|id| op::reply_to(&page, id));
    let confirm = q
        .confirm
        .as_deref()
        .and_then(og::confirm_kind)
        .filter(|k| {
            matches!(
                k,
                OrgActionKind::LeaveConversation | OrgActionKind::RemoveParticipant
            )
        })
        .and_then(|k| op::msg_confirm(&conv, &me, k, q.person.as_deref()));
    Ok((
        MsgThreadVm {
            title: text(&conv, "title").to_owned(),
            kind,
            presence,
            messages: op::messages(conversation, &page, &me, &clock),
            older_href: op::older_href(conversation, &page),
            load,
            composer: MsgComposerVm {
                action: format!("/messages/{conversation}/send"),
                body,
                reply_cancel_href: reply.as_ref().map(|_| format!("/messages/{conversation}")),
                reply,
                idempotency_key: key,
                error,
            },
            members,
            leave,
        },
        confirm,
    ))
}

/// `GET /messages/{conversation}`
pub(super) async fn messages_thread(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(conversation): Path<Uuid>,
    Query(q): Query<MsgQuery>,
) -> Response {
    let w = match open_app(&state, &headers, Screen::Messaging, ApplicationId::Messages).await {
        Ok(w) => w,
        Err(r) => return *r,
    };
    let id = conversation.to_string();
    // O fio primeiro: abrir marca a conversa como lida, e a lista lida
    // depois já não a conta por ler.
    let thread = thread_of(&state, &w, &id, &q, None).await;
    let mut vm = messages_vm(&state, &w, Some(&id)).await;
    let mut confirm = None;
    match thread {
        Ok((th, c)) => {
            vm.thread = Some(th);
            confirm = c;
        }
        Err(e) => vm.thread_error = Some(e),
    }
    let status = status_of(vm.thread_error);
    render_org(
        &state,
        &w,
        status,
        ui::apps::messages::app(&vm),
        confirm.as_ref(),
        None,
    )
}

#[derive(Deserialize)]
pub(super) struct SendForm {
    #[serde(default)]
    body: String,
    #[serde(default)]
    reply_to: Option<String>,
    #[serde(default)]
    idempotency_key: String,
}

/// `POST /messages/{conversation}/send`
///
/// O autor é o principal, no Core. A chave de idempotência vem do rascunho:
/// um duplo-clique ou uma nova tentativa trazem a mesma, e o Core devolve a
/// mensagem que a primeira escreveu. Se falhar, o texto volta intacto ao
/// compositor, com a mesma chave — nada se perde e nada se duplica.
pub(super) async fn messages_send(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(conversation): Path<Uuid>,
    Form(form): Form<SendForm>,
) -> Response {
    let w = match open_app(&state, &headers, Screen::Messaging, ApplicationId::Messages).await {
        Ok(w) => w,
        Err(r) => return *r,
    };
    let id = conversation.to_string();
    let key = form.idempotency_key.trim();
    let key = if key.is_empty() || key.len() > 128 {
        Uuid::new_v4().to_string()
    } else {
        key.to_owned()
    };
    let reply = form
        .reply_to
        .as_deref()
        .and_then(|r| Uuid::parse_str(r).ok());
    let result = post_json(
        &w,
        &state,
        &format!("/api/v1/messaging/conversations/{id}/messages"),
        &serde_json::json!({
            "body": form.body,
            "reply_to": reply,
            "idempotency_key": key,
        }),
    )
    .await;
    match result {
        Ok(_) => Redirect::to(&format!("/messages/{id}")).into_response(),
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        // Uma conversa que o membro já não alcança diz-se como inexistente.
        Err(ApiFailure::Denied) => super::failure_response(&ApiFailure::Denied),
        Err(f) => {
            let err = if matches!(f, ApiFailure::Rejected(_)) {
                AppError::SaveFailed
            } else {
                app_error(&f)
            };
            let mut vm = messages_vm(&state, &w, Some(&id)).await;
            let draft = (form.body, key, reply.map(|r| r.to_string()), err);
            match thread_of(&state, &w, &id, &MsgQuery::default(), Some(draft)).await {
                Ok((th, _)) => vm.thread = Some(th),
                Err(e) => vm.thread_error = Some(e),
            }
            let status = if vm.thread_error.is_some() {
                status_of(vm.thread_error)
            } else {
                StatusCode::UNPROCESSABLE_ENTITY
            };
            render_org(&state, &w, status, ui::apps::messages::app(&vm), None, None)
        }
    }
}

#[derive(Deserialize)]
pub(super) struct ReactForm {
    #[serde(default)]
    emoji: String,
}

/// `POST /messages/{conversation}/messages/{message}/react` — põe ou tira.
pub(super) async fn messages_react(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path((conversation, message)): Path<(Uuid, Uuid)>,
    Form(form): Form<ReactForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    match member_post(
        &state,
        &member,
        &format!("/api/v1/messaging/conversations/{conversation}/messages/{message}/reactions"),
        &serde_json::json!({ "emoji": form.emoji }),
    )
    .await
    {
        Ok(_) => Redirect::to(&format!("/messages/{conversation}#m-{message}")).into_response(),
        Err(f) => super::failure_response(&f),
    }
}

/// `POST /messages/{conversation}/leave` — depois da confirmação.
pub(super) async fn messages_leave(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(conversation): Path<Uuid>,
) -> Response {
    let member = member_or_login!(state, headers);
    let me = super::eu_id(&state, &member).await;
    match api::delete(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/messaging/conversations/{conversation}/members/{me}"),
    )
    .await
    {
        // Depois de sair, a conversa deixa de existir para quem saiu.
        Ok(_) => Redirect::to("/messages").into_response(),
        Err(f) => super::failure_response(&f),
    }
}

#[derive(Deserialize)]
pub(super) struct RemoveForm {
    who: Uuid,
}

/// `POST /messages/{conversation}/remove` — depois da confirmação. Quem pode
/// retirar quem decide o Core (dono ou administrador da conversa).
pub(super) async fn messages_remove(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(conversation): Path<Uuid>,
    Form(form): Form<RemoveForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    match api::delete(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!(
            "/api/v1/messaging/conversations/{conversation}/members/{}",
            form.who
        ),
    )
    .await
    {
        Ok(_) => Redirect::to(&format!("/messages/{conversation}")).into_response(),
        Err(f) => super::failure_response(&f),
    }
}

// ═════════════════════════════════════════════════════════════════════════
// IA
// ═════════════════════════════════════════════════════════════════════════

#[derive(Deserialize, Default)]
pub(super) struct NavQuery {
    #[serde(default)]
    nav: Option<String>,
    #[serde(default)]
    q: Option<String>,
}

fn holds(w: &AppWindow, permission: &str) -> bool {
    w.ctx.viewer.capabilities.iter().any(|c| c == permission)
}

/// `GET /ai` — o Ocinye AI Fabric: estado, modelos, fornecedores. Não é uma
/// conversa: perguntar é na Nye.
pub(super) async fn ai_page(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(q): Query<NavQuery>,
) -> Response {
    let w = match open_app(&state, &headers, Screen::Ai, ApplicationId::Ai).await {
        Ok(w) => w,
        Err(r) => return *r,
    };
    let quem = caller(&w.member);
    let clock = clock_of(&w.ctx);
    // Fornecedores e política só com a autoridade de infraestrutura (o Core
    // recusa na mesma; a secção nem aparece a quem não a tem).
    let infra = holds(&w, "ai.infrastructure.manage");
    let section = match q.nav.as_deref() {
        Some("models") => AiSection::Models,
        Some("providers") => AiSection::Providers,
        _ => AiSection::Overview,
    };
    let status = quem.get(&state, "/api/v1/ai/status").await;
    let models = quem.get(&state, "/api/v1/ai/models").await;
    let providers = if infra {
        quem.get(&state, "/api/v1/ai/providers")
            .await
            .ok()
            .map(|v| op::list_of(&v))
    } else {
        None
    };
    let policy = if infra {
        quem.get(&state, "/api/v1/ai/policy").await.ok()
    } else {
        None
    };
    let plist = providers.clone().unwrap_or_default();
    let (load, st) = match &status {
        Ok(v) => (AppLoad::Ready, v.clone()),
        Err(f) => (AppLoad::Failed(app_error(f)), Value::Null),
    };
    let mut nav = vec![
        rs::nav(
            "ai.nav.overview",
            "ai",
            "/ai".to_owned(),
            section == AiSection::Overview,
        ),
        rs::nav(
            "ai.nav.models",
            "gpu",
            "/ai?nav=models".to_owned(),
            section == AiSection::Models,
        ),
    ];
    if providers.is_some() {
        nav.push(rs::nav(
            "ai.nav.providers",
            "integrations",
            "/ai?nav=providers".to_owned(),
            section == AiSection::Providers,
        ));
    }
    let vm = AiVm {
        section,
        nav,
        available: st.get("available").and_then(Value::as_bool) == Some(true),
        healthy_providers: st
            .get("providers")
            .and_then(Value::as_u64)
            .map_or(0, |n| u32::try_from(n).unwrap_or(u32::MAX)),
        // A frase do Core é para pessoas e está numa só língua: o estado
        // diz-se pelas chaves do Design.
        message: None,
        caps: op::ai_caps(&st, policy.as_ref(), &plist),
        models: models
            .as_ref()
            .map(|v| op::ai_models(&op::list_of(v), &plist))
            .unwrap_or_default(),
        providers: providers.as_deref().map(|p| op::ai_providers(p, &clock)),
        external_max: policy.as_ref().map(op::external_max),
        installation_external: policy.as_ref().and_then(|p| {
            p.get("installation_allows_external")
                .and_then(Value::as_bool)
        }),
        load,
        nye_href: Some("/ai/prompt".to_owned()),
    };
    let code = if section == AiSection::Providers && vm.providers.is_none() {
        StatusCode::FORBIDDEN
    } else {
        StatusCode::OK
    };
    render_org(&state, &w, code, ui::apps::fabric::ai_app(&vm), None, None)
}

// ═════════════════════════════════════════════════════════════════════════
// Agentes
// ═════════════════════════════════════════════════════════════════════════

async fn agents_vm(state: &WorkspaceState, w: &AppWindow, open: Option<&str>) -> (AgentsVm, Value) {
    let quem = caller(&w.member);
    let clock = clock_of(&w.ctx);
    let (items, load, exec) = match quem.get(state, "/api/v1/ai/agents").await {
        Ok(v) => (
            op::agent_items(&op::list_of(&v), open, &clock),
            AppLoad::Ready,
            v.get("execution_available").and_then(Value::as_bool) == Some(true),
        ),
        Err(f) => (Vec::new(), AppLoad::Failed(app_error(&f)), true),
    };
    let caps = quem
        .get(state, "/api/v1/ai/agents/capabilities")
        .await
        .unwrap_or(Value::Null);
    let may_create = op::agent_form(&caps).is_some();
    (
        AgentsVm {
            list: ResListVm {
                columns: op::AGENT_COLUMNS.to_vec(),
                items,
                load,
                page: AppPageVm {
                    more_href: None,
                    summary: None,
                },
            },
            execution_available: exec,
            pane: pane(open.is_some()),
            agent: None,
            agent_error: None,
            form: None,
            new_href: may_create.then(|| "/ai/agents/new".to_owned()),
            list_href: "/ai/agents".to_owned(),
        },
        caps,
    )
}

/// `GET /ai/agents`
pub(super) async fn agents_page(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
) -> Response {
    let w = match open_app(&state, &headers, Screen::Agents, ApplicationId::Agents).await {
        Ok(w) => w,
        Err(r) => return *r,
    };
    let (vm, _) = agents_vm(&state, &w, None).await;
    render_org(
        &state,
        &w,
        StatusCode::OK,
        ui::apps::fabric::agents_app(&vm),
        None,
        None,
    )
}

/// `GET /ai/agents/{agent_id}`
pub(super) async fn agent_page(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(agent_id): Path<Uuid>,
) -> Response {
    let w = match open_app(&state, &headers, Screen::Agents, ApplicationId::Agents).await {
        Ok(w) => w,
        Err(r) => return *r,
    };
    let id = agent_id.to_string();
    let (mut vm, _) = agents_vm(&state, &w, Some(&id)).await;
    let quem = caller(&w.member);
    let clock = clock_of(&w.ctx);
    match quem.get(&state, &format!("/api/v1/ai/agents/{id}")).await {
        Ok(a) => {
            // O alvo de um agente de unidade ou de ambiente, relido com a
            // autoridade de quem vê: o que não lê fica por nomear.
            let sid = text(&a, "scope_id");
            let target = match (text(&a, "scope"), sid.is_empty()) {
                ("unit", false) => quem
                    .get(&state, &format!("/api/v1/units/{sid}"))
                    .await
                    .ok()
                    .map(|u| crate::ui::view_models::ResLinkVm {
                        kind: crate::ui::view_models::ResKind::Other,
                        kind_label: Some(t("agents.scope.unit").to_owned()),
                        title: text(&u, "name").to_owned(),
                        meta: None,
                        relation: None,
                        by_operation: false,
                        href: Some(format!("/units/{sid}")),
                    }),
                ("workspace", false) => quem
                    .get(&state, &format!("/api/v1/workspaces/{sid}"))
                    .await
                    .ok()
                    .map(|x| crate::ui::view_models::ResLinkVm {
                        kind: crate::ui::view_models::ResKind::Other,
                        kind_label: Some(t("agents.scope.workspace").to_owned()),
                        title: text(x.get("workspace").unwrap_or(&x), "title").to_owned(),
                        meta: None,
                        relation: None,
                        by_operation: false,
                        href: None,
                    }),
                _ => None,
            };
            match op::agent(&a, target, &clock) {
                Some(av) => vm.agent = Some(av),
                None => vm.agent_error = Some(AppError::Unavailable),
            }
        }
        Err(f) => vm.agent_error = Some(app_error(&f)),
    }
    let status = status_of(vm.agent_error);
    render_org(
        &state,
        &w,
        status,
        ui::apps::fabric::agents_app(&vm),
        None,
        None,
    )
}

/// `GET /ai/agents/new` — só com os âmbitos que o Core diz que o membro cria.
pub(super) async fn agent_new_page(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
) -> Response {
    let w = match open_app(&state, &headers, Screen::Agents, ApplicationId::Agents).await {
        Ok(w) => w,
        Err(r) => return *r,
    };
    let (mut vm, caps) = agents_vm(&state, &w, None).await;
    let Some(form) = op::agent_form(&caps) else {
        vm.agent_error = Some(AppError::PermissionDenied);
        vm.pane = ResPane::Detail;
        return render_org(
            &state,
            &w,
            StatusCode::FORBIDDEN,
            ui::apps::fabric::agents_app(&vm),
            None,
            None,
        );
    };
    vm.form = Some(form);
    vm.pane = ResPane::Detail;
    render_org(
        &state,
        &w,
        StatusCode::OK,
        ui::apps::fabric::agents_app(&vm),
        None,
        None,
    )
}

#[derive(Deserialize)]
pub(super) struct AgentForm {
    #[serde(default)]
    name: String,
    #[serde(default)]
    purpose: String,
    #[serde(default)]
    instructions: String,
    #[serde(default)]
    capability: String,
    #[serde(default)]
    scope: String,
    #[serde(default)]
    scope_id: String,
    #[serde(default)]
    max_classification: String,
    #[serde(default)]
    uses_bibliography: Option<String>,
    #[serde(default)]
    uses_documents: Option<String>,
    #[serde(default)]
    uses_datasets: Option<String>,
}

/// `POST /ai/agents/new` — o Core decide o âmbito, a capacidade e o tecto.
/// Um âmbito que o formulário nunca ofereceu não chega a sair do Workspace.
pub(super) async fn agent_create(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<AgentForm>,
) -> Response {
    let w = match open_app(&state, &headers, Screen::Agents, ApplicationId::Agents).await {
        Ok(w) => w,
        Err(r) => return *r,
    };
    let (mut vm, caps) = agents_vm(&state, &w, None).await;
    let target = Some(form.scope_id.trim()).filter(|s| !s.is_empty());
    let refuse = |mut vm: AgentsVm, e: AppError, code: StatusCode| {
        let mut f = op::agent_form(&caps);
        if let Some(f) = f.as_mut() {
            f.error = Some(e);
        } else {
            vm.agent_error = Some(e);
        }
        vm.form = f;
        vm.pane = ResPane::Detail;
        render_org(
            &state,
            &w,
            code,
            ui::apps::fabric::agents_app(&vm),
            None,
            None,
        )
    };
    if !op::scope_offered(&caps, &form.scope, target) {
        return refuse(vm, AppError::PermissionDenied, StatusCode::FORBIDDEN);
    }
    let body = serde_json::json!({
        "name": form.name,
        "purpose": Some(form.purpose.trim()).filter(|s| !s.is_empty()),
        "instructions": Some(form.instructions.trim()).filter(|s| !s.is_empty()),
        "capability": form.capability,
        "scope": form.scope,
        "scope_id": target,
        "max_classification": form.max_classification,
        "uses_bibliography": form.uses_bibliography.is_some(),
        "uses_documents": form.uses_documents.is_some(),
        "uses_datasets": form.uses_datasets.is_some(),
    });
    match post_json(&w, &state, "/api/v1/ai/agents", &body).await {
        Ok(v) => {
            let id = text(&v, "id");
            Redirect::to(&if id.is_empty() {
                "/ai/agents".to_owned()
            } else {
                format!("/ai/agents/{id}")
            })
            .into_response()
        }
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        Err(f) => {
            let (e, code) = match f {
                ApiFailure::Forbidden | ApiFailure::Denied => {
                    (AppError::PermissionDenied, StatusCode::FORBIDDEN)
                }
                ApiFailure::Conflict(_) => (AppError::Conflict, StatusCode::CONFLICT),
                ApiFailure::Rejected(_) => (AppError::SaveFailed, StatusCode::UNPROCESSABLE_ENTITY),
                ref other => (app_error(other), StatusCode::BAD_GATEWAY),
            };
            vm.agent = None;
            refuse(vm, e, code)
        }
    }
}

// ═════════════════════════════════════════════════════════════════════════
// Computação
// ═════════════════════════════════════════════════════════════════════════

async fn compute_render(
    state: &WorkspaceState,
    headers: &HeaderMap,
    open: Option<&str>,
) -> Response {
    let w = match open_app(state, headers, Screen::Compute, ApplicationId::Compute).await {
        Ok(w) => w,
        Err(r) => return *r,
    };
    let quem = caller(&w.member);
    let clock = clock_of(&w.ctx);
    let status = quem
        .get(state, "/api/v1/compute/status")
        .await
        .unwrap_or(Value::Null);
    let nodes = quem.get(state, "/api/v1/compute/nodes").await;
    let (list, load) = match &nodes {
        Ok(v) => (op::list_of(v), AppLoad::Ready),
        Err(f) => (Vec::new(), AppLoad::Failed(app_error(f))),
    };
    let n = |k: &str| {
        status
            .get(k)
            .and_then(Value::as_u64)
            .map_or(0, |n| u32::try_from(n).unwrap_or(u32::MAX))
    };
    let mut vm = ComputeVm {
        registered: n("registered_nodes"),
        online: n("online_nodes"),
        message: None,
        list: ResListVm {
            columns: op::NODE_COLUMNS.to_vec(),
            items: op::node_items(&list, open, &clock),
            load,
            page: AppPageVm {
                more_href: None,
                summary: None,
            },
        },
        pane: pane(open.is_some()),
        node: None,
        node_error: None,
        list_href: "/compute".to_owned(),
    };
    if let Some(id) = open {
        match list
            .iter()
            .find(|x| text(x, "id") == id)
            .and_then(|x| op::node(x, &clock))
        {
            Some(nv) => vm.node = Some(nv),
            None => vm.node_error = Some(AppError::NotFound),
        }
    }
    let status = status_of(vm.node_error);
    render_org(
        state,
        &w,
        status,
        ui::apps::fabric::compute_app(&vm),
        None,
        None,
    )
}

/// `GET /compute`
pub(super) async fn compute_page(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
) -> Response {
    compute_render(&state, &headers, None).await
}

/// `GET /compute/nodes/{node_id}` — só um nó que a lista autorizada devolve.
pub(super) async fn compute_node_page(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(node_id): Path<Uuid>,
) -> Response {
    compute_render(&state, &headers, Some(&node_id.to_string())).await
}

// ═════════════════════════════════════════════════════════════════════════
// Meus Recursos
// ═════════════════════════════════════════════════════════════════════════

/// `GET /resources` — o quadro do próprio membro. Não há parâmetro de pessoa:
/// o Core lê o principal.
pub(super) async fn resources_page(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
) -> Response {
    let w = match open_app(
        &state,
        &headers,
        Screen::Resources,
        ApplicationId::Resources,
    )
    .await
    {
        Ok(w) => w,
        Err(r) => return *r,
    };
    let clock = clock_of(&w.ctx);
    let vm = match caller(&w.member).get(&state, "/api/v1/resources/me").await {
        Ok(v) => op::resources(&v, &clock),
        Err(f) => ResourcesVm {
            load: AppLoad::Failed(app_error(&f)),
            used: String::new(),
            reserved: String::new(),
            limit: None,
            available: String::new(),
            used_pct: None,
            reserved_pct: None,
            state: crate::ui::view_models::StorageStateVm::Normal,
            entitlement: String::new(),
            parts: Vec::new(),
            files_href: None,
        },
    };
    render_org(
        &state,
        &w,
        StatusCode::OK,
        ui::apps::ledger::resources_app(&vm),
        None,
        None,
    )
}

// ═════════════════════════════════════════════════════════════════════════
// Actividade
// ═════════════════════════════════════════════════════════════════════════

#[derive(Deserialize, Default)]
pub(super) struct ActivityQuery {
    #[serde(default)]
    workspace: Option<String>,
    #[serde(default)]
    page: Option<u32>,
}

const ACTIVITY_PAGE: usize = 25;

/// `GET /activity` — o feed que o Core projecta, com **cada alvo relido com a
/// autoridade de agora**. O Core filtra pela classificação e pelo ambiente
/// gravados no evento; um alvo reclassificado, ou de um ambiente de onde o
/// membro saiu, só se apanha relendo-o. O que não se lê fica redigido: sem
/// título, sem o resumo (que o cita), sem classificação, sem ambiente.
pub(super) async fn activity_page(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(q): Query<ActivityQuery>,
) -> Response {
    let w = match open_app(&state, &headers, Screen::Activity, ApplicationId::Activity).await {
        Ok(w) => w,
        Err(r) => return *r,
    };
    let quem = caller(&w.member);
    let clock = clock_of(&w.ctx);
    let page = q.page.unwrap_or(1).max(1);
    // Os ambientes que o membro lê: o filtro e o contexto de cada evento.
    let readable: Vec<(String, String)> = quem
        .get(&state, "/api/v1/workspaces?page_size=100")
        .await
        .map(|v| {
            op::list_of(&v)
                .iter()
                .map(|x| (text(x, "id").to_owned(), text(x, "title").to_owned()))
                .collect()
        })
        .unwrap_or_default();
    let ws = q
        .workspace
        .as_deref()
        .filter(|id| readable.iter().any(|(r, _)| r == id));
    let mut path = format!("/api/v1/activity?page={page}&page_size={ACTIVITY_PAGE}");
    if let Some(id) = ws {
        path.push_str(&format!("&workspace_id={}", rs::encode(id)));
    }
    let entries = quem.get(&state, &path).await;
    let (list, load) = match &entries {
        Ok(v) => (op::list_of(v), AppLoad::Ready),
        Err(f) => (Vec::new(), AppLoad::Failed(app_error(f))),
    };
    // Cada alvo, pela sua leitura autorizada, em paralelo e limitado à página.
    let mut set = tokio::task::JoinSet::new();
    for (i, e) in list.iter().enumerate() {
        let wid = text(e, "workspace_id");
        let ws_ok = !wid.is_empty() && readable.iter().any(|(r, _)| r == wid);
        let sid = text(e, "subject_id").to_owned();
        let st = text(e, "subject_type").to_owned();
        let read = op::target_path(&st, &sid).filter(|_| !sid.is_empty());
        let state = state.clone();
        let token = w.member.session.access_token.clone();
        let corr = w.member.correlation_id.clone();
        set.spawn(async move {
            let t = match read {
                Some((path, field)) => {
                    match api::get::<Value>(&state, &token, &corr, &path).await {
                        Ok(v) => {
                            let title = op::target_title(&st, &v, field);
                            if title.is_empty() {
                                op::Target::Redacted
                            } else {
                                op::Target::Visible(title.to_owned())
                            }
                        }
                        Err(_) => op::Target::Redacted,
                    }
                }
                // Sem leitura própria (nota de ambiente, versão, pertença):
                // vale o ambiente.
                None if ws_ok => op::Target::Context,
                None => op::Target::Redacted,
            };
            (i, t)
        });
    }
    let mut targets = vec![op::Target::Redacted; list.len()];
    while let Some(r) = set.join_next().await {
        if let Ok((i, t)) = r {
            targets[i] = t;
        }
    }
    let mut last_day = None;
    let items = list
        .iter()
        .zip(targets.iter())
        .map(|(e, tg)| {
            let wid = text(e, "workspace_id");
            let ctx = readable
                .iter()
                .find(|(r, _)| r == wid)
                .map(|(_, n)| n.clone());
            op::activity_item(e, tg, ctx, &clock, &mut last_day)
        })
        .collect();
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
    let base = ws.map_or_else(
        || "/activity".to_owned(),
        |id| format!("/activity?workspace={}", rs::encode(id)),
    );
    let sep = if base.contains('?') { '&' } else { '?' };
    let vm = ActivityVm {
        workspaces,
        items,
        load,
        page: AppPageVm {
            more_href: (list.len() == ACTIVITY_PAGE)
                .then(|| format!("{base}{sep}page={}", page + 1)),
            summary: None,
        },
        error: None,
    };
    render_org(
        &state,
        &w,
        StatusCode::OK,
        ui::apps::ledger::activity_app(&vm),
        None,
        None,
    )
}

// ═════════════════════════════════════════════════════════════════════════
// Auditoria
// ═════════════════════════════════════════════════════════════════════════

#[derive(Deserialize, Default)]
pub(super) struct AuditQuery {
    #[serde(default)]
    resource_type: Option<String>,
    #[serde(default)]
    since: Option<String>,
    #[serde(default)]
    actor: Option<String>,
    #[serde(default)]
    page: Option<u32>,
    #[serde(default)]
    open: Option<String>,
}

/// `GET /audit` — prova só de leitura. O Core decide quem lê (auditor,
/// administração); aqui escolhe-se só o que se mostra: a metadata por lista
/// branca de chaves, e o resto contado.
pub(super) async fn audit_page(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(q): Query<AuditQuery>,
) -> Response {
    let w = match open_app(&state, &headers, Screen::Audit, ApplicationId::Audit).await {
        Ok(w) => w,
        Err(r) => return *r,
    };
    let quem = caller(&w.member);
    let clock = clock_of(&w.ctx);
    let page = q.page.unwrap_or(1).max(1);
    // Os tipos que o registo tem, pela mesma autoridade que o lê.
    let known: Vec<String> = quem
        .get(&state, "/api/v1/audit/resource-types")
        .await
        .ok()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();
    let rtype = q
        .resource_type
        .as_deref()
        .filter(|t| known.iter().any(|k| k == t));
    let actor = q.actor.as_deref().and_then(|a| Uuid::parse_str(a).ok());
    let since = q
        .since
        .as_deref()
        .filter(|s| op::since_instant(s, &clock).is_some());
    // O endereço da lista com os filtros correntes (e a página).
    let mut params: Vec<String> = Vec::new();
    if let Some(t) = rtype {
        params.push(format!("resource_type={t}"));
    }
    if let Some(s) = since {
        params.push(format!("since={s}"));
    }
    if let Some(a) = actor {
        params.push(format!("actor={a}"));
    }
    let base = if params.is_empty() {
        "/audit".to_owned()
    } else {
        format!("/audit?{}", params.join("&"))
    };
    let mut core = format!("/api/v1/audit?page={page}&page_size=25");
    if let Some(t) = rtype {
        core.push_str(&format!("&resource_type={t}"));
    }
    if let Some(a) = actor {
        core.push_str(&format!("&actor_person_id={a}"));
    }
    if let Some(at) = since.and_then(|s| op::since_instant(s, &clock)) {
        core.push_str(&format!("&since={}", rs::encode(&at.to_rfc3339())));
    }
    let listed = quem.get(&state, &core).await;
    let v = match listed {
        Ok(v) => v,
        // O Core responde como se não existisse a quem não lê auditoria.
        Err(ApiFailure::Denied | ApiFailure::Forbidden) => {
            let vm = AuditVm {
                error: Some(AppError::PermissionDenied),
                filter_action: "/audit".to_owned(),
                types: Vec::new(),
                since: String::new(),
                actor: None,
                rows: Vec::new(),
                load: AppLoad::Ready,
                page: AppPageVm {
                    more_href: None,
                    summary: None,
                },
                pane: ResPane::List,
                detail: None,
                list_href: "/audit".to_owned(),
            };
            return render_org(
                &state,
                &w,
                StatusCode::FORBIDDEN,
                ui::apps::ledger::audit_app(&vm),
                None,
                None,
            );
        }
        Err(f) => {
            let vm = AuditVm {
                error: None,
                filter_action: "/audit".to_owned(),
                types: op::audit_types(&known, rtype),
                since: since.unwrap_or_default().to_owned(),
                actor: None,
                rows: Vec::new(),
                load: AppLoad::Failed(app_error(&f)),
                page: AppPageVm {
                    more_href: None,
                    summary: None,
                },
                pane: ResPane::List,
                detail: None,
                list_href: base,
            };
            return render_org(
                &state,
                &w,
                StatusCode::OK,
                ui::apps::ledger::audit_app(&vm),
                None,
                None,
            );
        }
    };
    let recs = op::list_of(&v);
    let paged = if page > 1 {
        format!(
            "{base}{}page={page}",
            if base.contains('?') { '&' } else { '?' }
        )
    } else {
        base.clone()
    };
    let open = q.open.as_deref();
    let rows: Vec<_> = recs
        .iter()
        .filter_map(|r| op::audit_row(r, &paged, open, &clock))
        .collect();
    // O detalhe é o de um registo desta página: um identificador que a
    // leitura autorizada não devolveu não abre nada.
    let mut detail = None;
    let mut missing = false;
    if let Some(id) = open {
        match recs.iter().find(|r| text(r, "id") == id) {
            Some(r) => {
                let row = op::audit_row(r, &paged, open, &clock);
                let rid = text(r, "resource_id");
                let target = match op::target_path(text(r, "resource_type"), rid) {
                    Some((p, field)) if !rid.is_empty() => {
                        quem.get(&state, &p).await.ok().and_then(|x| {
                            let title = op::target_title(text(r, "resource_type"), &x, field);
                            (!title.is_empty()).then(|| {
                                let (kind, href, kind_label) =
                                    rs::kind_and_href(text(r, "resource_type"), rid);
                                crate::ui::view_models::ResLinkVm {
                                    kind,
                                    kind_label,
                                    title: title.to_owned(),
                                    meta: None,
                                    relation: None,
                                    by_operation: false,
                                    href,
                                }
                            })
                        })
                    }
                    _ => None,
                };
                detail = row.map(|row| op::audit_detail(r, row, target));
            }
            None => missing = true,
        }
    }
    let actor_name = actor.and_then(|a| {
        recs.iter()
            .find(|r| text(r, "actor_person_id") == a.to_string())
            .and_then(|r| rs::opt(r, "actor_name"))
    });
    let clear = {
        let mut p: Vec<String> = Vec::new();
        if let Some(t) = rtype {
            p.push(format!("resource_type={t}"));
        }
        if let Some(s) = since {
            p.push(format!("since={s}"));
        }
        if p.is_empty() {
            "/audit".to_owned()
        } else {
            format!("/audit?{}", p.join("&"))
        }
    };
    let vm = AuditVm {
        error: None,
        filter_action: "/audit".to_owned(),
        types: op::audit_types(&known, rtype),
        since: since.unwrap_or_default().to_owned(),
        actor: actor.map(|_| {
            (
                actor_name.unwrap_or_else(|| t("activity.system").to_owned()),
                clear,
            )
        }),
        rows,
        load: AppLoad::Ready,
        page: rs::page(&v, &base),
        pane: pane(detail.is_some()),
        detail,
        list_href: paged,
    };
    let status = if missing {
        StatusCode::NOT_FOUND
    } else {
        StatusCode::OK
    };
    render_org(
        &state,
        &w,
        status,
        ui::apps::ledger::audit_app(&vm),
        None,
        None,
    )
}

// ═════════════════════════════════════════════════════════════════════════
// Definições
// ═════════════════════════════════════════════════════════════════════════

#[derive(Deserialize, Default)]
pub(super) struct SettingsQuery {
    #[serde(default)]
    ok: Option<String>,
    #[serde(default)]
    avatar: Option<String>,
    #[serde(default)]
    err: Option<String>,
}

async fn settings_render(
    state: &WorkspaceState,
    headers: &HeaderMap,
    section: SettingsSection,
    q: &SettingsQuery,
    error: Option<AppError>,
    code: StatusCode,
) -> Response {
    let w = match open_app(state, headers, Screen::Settings, ApplicationId::Settings).await {
        Ok(w) => w,
        Err(r) => return *r,
    };
    let quem = caller(&w.member);
    let clock = clock_of(&w.ctx);
    let viewer = &w.ctx.viewer;
    let (sessions, mfa) = if section == SettingsSection::Security {
        let s = quem
            .get(state, "/api/v1/auth/sessions")
            .await
            .unwrap_or(Value::Null);
        let m = quem
            .get(state, "/api/v1/auth/mfa")
            .await
            .unwrap_or(Value::Null);
        let mode = text(&m, "mfa_mode");
        (
            op::own_sessions(&op::list_of(&s), &clock),
            (
                mode == "challenge",
                mode != "not_required" && !mode.is_empty(),
            ),
        )
    } else {
        (Vec::new(), (false, false))
    };
    let (pins, source) = if section == SettingsSection::Apps {
        let p = quem
            .get(state, "/api/v1/me/apps/pins")
            .await
            .unwrap_or(Value::Null);
        let visible = crate::experience::apps::visible_to(viewer, w.ctx.core);
        (
            op::pins(&visible, &viewer.pinned),
            op::pins_source(text(&p, "source")),
        )
    } else {
        (Vec::new(), String::new())
    };
    let vm = SettingsVm {
        section,
        nav: op::settings_nav(section),
        saved: q.ok.is_some() || q.avatar.as_deref() == Some("ok"),
        error: error.or_else(|| q.err.as_ref().map(|_| AppError::SaveFailed)),
        name: viewer.name.clone(),
        email: viewer.email.clone().unwrap_or_default(),
        avatar: op::avatar_vm(&viewer.name, &viewer.avatar),
        presets: op::presets(&viewer.avatar),
        locales: op::locales(crate::i18n::current()),
        timezone: w.ctx.zone.as_str().to_owned(),
        mfa,
        password_changed: None,
        sessions,
        pins,
        pins_source: source,
    };
    render_org(
        state,
        &w,
        code,
        ui::apps::member::settings_app(&vm),
        None,
        None,
    )
}

/// `GET /settings`
pub(super) async fn settings_account(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(q): Query<SettingsQuery>,
) -> Response {
    settings_render(
        &state,
        &headers,
        SettingsSection::Account,
        &q,
        None,
        StatusCode::OK,
    )
    .await
}

/// `GET /settings/language`
pub(super) async fn settings_language(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(q): Query<SettingsQuery>,
) -> Response {
    settings_render(
        &state,
        &headers,
        SettingsSection::Language,
        &q,
        None,
        StatusCode::OK,
    )
    .await
}

/// `GET /settings/security`
pub(super) async fn settings_security(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(q): Query<SettingsQuery>,
) -> Response {
    settings_render(
        &state,
        &headers,
        SettingsSection::Security,
        &q,
        None,
        StatusCode::OK,
    )
    .await
}

/// `GET /settings/apps`
pub(super) async fn settings_apps(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(q): Query<SettingsQuery>,
) -> Response {
    settings_render(
        &state,
        &headers,
        SettingsSection::Apps,
        &q,
        None,
        StatusCode::OK,
    )
    .await
}

/// `GET /settings/mfa` — configurar o segundo factor é o ecrã do D001
/// (`/mfa`). A Segurança só o oferece a quem o tem exigido e por configurar;
/// gerir um factor já configurado não tem ecrã (fica por desenhar).
pub(super) async fn settings_mfa() -> Response {
    Redirect::to("/mfa").into_response()
}

#[derive(Deserialize)]
pub(super) struct PasswordForm {
    #[serde(default)]
    current_password: String,
    #[serde(default)]
    new_password: String,
}

/// `POST /settings/password` — a mudança do próprio (`/auth/password/change`).
/// Uma recusa volta à Segurança, sem nenhum valor preenchido.
pub(super) async fn change_password(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<PasswordForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let body = serde_json::json!({
        "current": form.current_password,
        "password": form.new_password,
        "confirmation": form.new_password,
    });
    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        "/api/v1/auth/password/change",
        &body,
    )
    .await
    .and_then(CoreSession::from_payload)
    {
        Ok(session) => super::password_changed(&state, &headers, &member, session),
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        Err(f) => {
            let (e, code) = match f {
                ApiFailure::Rejected(_) | ApiFailure::Forbidden | ApiFailure::Denied => {
                    (AppError::SaveFailed, StatusCode::UNPROCESSABLE_ENTITY)
                }
                ref other => (app_error(other), StatusCode::BAD_GATEWAY),
            };
            settings_render(
                &state,
                &headers,
                SettingsSection::Security,
                &SettingsQuery::default(),
                Some(e),
                code,
            )
            .await
        }
    }
}

// ═════════════════════════════════════════════════════════════════════════
// Ajuda
// ═════════════════════════════════════════════════════════════════════════

/// `GET /help` — conteúdo de primeira parte, versionado com o código: um
/// tópico por aplicação do registo e os atalhos declarados. A pesquisa filtra
/// no servidor esse conjunto e mais nada — nem ficheiros, nem a web, nem a Nye.
pub(super) async fn help_page(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(q): Query<NavQuery>,
) -> Response {
    let w = match open_app(&state, &headers, Screen::Help, ApplicationId::Help).await {
        Ok(w) => w,
        Err(r) => return *r,
    };
    let shortcuts = q.nav.as_deref() == Some("shortcuts");
    let query: String =
        q.q.as_deref()
            .unwrap_or_default()
            .chars()
            .take(80)
            .collect();
    let visible = crate::experience::apps::visible_to(&w.ctx.viewer, w.ctx.core);
    let vm = HelpVm {
        section: if shortcuts {
            HelpSection::Shortcuts
        } else {
            HelpSection::Apps
        },
        nav: op::help_nav(shortcuts),
        query: (!shortcuts).then(|| query.clone()),
        topics: op::help_topics(crate::experience::apps::APPLICATIONS, &visible, &query),
        shortcuts: op::help_shortcuts(),
        nye: Some(op::help_nye(&query)),
    };
    render_org(
        &state,
        &w,
        StatusCode::OK,
        ui::apps::member::help_app(&vm),
        None,
        None,
    )
}
