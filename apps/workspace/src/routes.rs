//! As rotas do Workspace.
//!
//! Todos os ecrãs seguem a mesma forma: resolver a sessão, chamar o Ocinye Core
//! com o token do membro, renderizar. Quando o Core recusa, o Workspace mostra
//! o que o Core disse, em vez de inventar a sua própria versão.
//!
//! O mapa de navegação é o de `design/README.md` §4.

use std::time::{Duration, Instant};

use axum::extract::{DefaultBodyLimit, Form, Multipart, Path, Query, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::Router;
use serde::Deserialize;
use serde_json::Value;
use tower_http::services::ServeDir;
use uuid::Uuid;

use crate::api::{self, ApiFailure};
use crate::session::{self, Session};
use crate::ui;
use crate::ui::shell::{Crumb, Screen, Viewer};
use crate::WorkspaceState;

/// Todos os caminhos que o Workspace serve.
///
/// Existe para que um teste possa afirmar que nenhuma ligação renderizada
/// aponta para fora desta lista. Uma ligação morta num ambiente institucional
/// não é um detalhe estético: é uma promessa que a interface não cumpre.
#[cfg_attr(
    not(test),
    allow(dead_code, reason = "lido pelo teste de ligações mortas")
)]
pub const ROUTES: &[&str] = &[
    "/",
    "/my-work",
    "/messages",
    "/messages/{conversation}",
    "/messages/start",
    "/messages/assist",
    "/messages/people",
    "/messages/{conversation}/typing",
    "/messages/{conversation}/send",
    "/messages/{conversation}/react",
    "/messages/{conversation}/read",
    "/messages/{conversation}/members",
    "/messages/{conversation}/leave",
    "/messages/{conversation}/remove",
    "/mail",
    "/mail/{mailbox_id}",
    "/mail/{mailbox_id}/sync",
    "/mail/message/{message_id}",
    "/mail/message/{message_id}/flags",
    "/mail/compose",
    "/mail/people",
    "/mail/assist",
    "/mail/send",
    "/mail/settings",
    "/mail/{mailbox_id}/connect",
    "/mail/{mailbox_id}/disconnect",
    "/units",
    "/units/{unit_id}",
    "/units/{unit_id}/members",
    "/workspaces/{workspace_id}/members",
    "/workspaces/{workspace_id}/members/remove",
    "/units/{unit_id}/members/role",
    "/units/{unit_id}/members/remove",
    "/ideas",
    "/units/new",
    "/projects/new",
    "/bibliography/new",
    "/datasets/new",
    "/calendar",
    "/calendar/events/new",
    "/calendar/events/{event_id}",
    "/calendar/events/{event_id}/edit",
    "/calendar/events/{event_id}/cancel",
    "/notifications",
    "/notifications/recent",
    "/notifications/{notification_id}/read",
    "/help",
    "/settings",
    "/settings/security",
    "/settings/password",
    "/settings/avatar/preset",
    "/settings/avatar/photo",
    "/settings/avatar/initials",
    "/settings/sessions/{session_id}/revoke",
    "/avatar/me/{version}",
    "/ideas/new",
    "/ideas/{idea_id}",
    "/projects",
    "/projects/{project_id}",
    "/workspaces/{workspace_id}",
    "/workspaces/{workspace_id}/science",
    "/workspaces/{workspace_id}/science/hypotheses/new",
    "/workspaces/{workspace_id}/science/methodologies/new",
    "/workspaces/{workspace_id}/science/studies/new",
    "/methodologies/{methodology_id}",
    "/methodologies/{methodology_id}/versions/new",
    "/studies/{study_id}",
    "/studies/{study_id}/executions/new",
    "/executions/{execution_id}",
    "/executions/{execution_id}/results/new",
    "/results/{result_id}",
    "/results/{result_id}/validate",
    "/knowledge",
    "/files",
    "/files/upload",
    "/files/folder",
    "/files/{file_id}",
    "/files/{file_id}/version",
    "/files/{file_id}/download",
    "/files/{file_id}/preview",
    "/file-versions/{version_id}/preview",
    "/file-versions/{version_id}/download",
    "/bibliography",
    "/bibliography/tools",
    "/datasets",
    "/ai",
    "/ai/agents",
    "/ai/agents/new",
    "/ai/prompt",
    "/compute",
    "/activity",
    "/admin",
    "/admin/members/new",
    "/admin/members/{person_id}",
    "/audit",
    "/search",
    "/ask",
    "/ask/plans/{plan_id}/execute",
    "/ask/plans/{plan_id}/reject",
    "/boot",
    "/login",
    "/first-access",
    "/logout",
    "/health",
];

/// O router do Workspace.
pub fn router(state: WorkspaceState) -> Router {
    Router::new()
        // Pessoal
        .route("/", get(home))
        .route("/my-work", get(my_work))
        // Correio
        // ── Mensagens ───────────────────────────────────────────────────
        .route(ui::screens::messaging::ROUTE, get(messaging))
        .route("/messages/{conversation}", get(messaging_conversation))
        .route("/messages/start", post(messaging_start))
        .route("/messages/assist", post(messaging_assist))
        .route("/messages/people", get(messaging_people))
        .route("/messages/{conversation}/typing", get(messaging_typing))
        .route("/messages/{conversation}/send", post(messaging_send))
        .route("/messages/{conversation}/react", post(messaging_react))
        .route("/messages/{conversation}/read", post(messaging_read))
        .route(
            "/messages/{conversation}/members",
            post(messaging_add_member),
        )
        .route("/messages/{conversation}/leave", post(messaging_leave))
        .route("/messages/{conversation}/remove", post(messaging_remove))
        .route("/mail", get(mail))
        .route("/mail/compose", get(compose))
        .route("/mail/people", get(mail_people))
        .route("/mail/assist", post(assist))
        .route("/mail/send", post(send_mail))
        .route(
            "/mail/settings",
            get(mail_settings).post(save_mail_settings),
        )
        // Declaradas antes de `/mail/{mailbox_id}`: são caminhos literais sob
        // um identificador, e a rota genérica apanhá-las-ia primeiro.
        .route("/mail/{mailbox_id}/connect", post(mail_connect))
        .route("/mail/{mailbox_id}/disconnect", post(mail_disconnect))
        .route("/mail/message/{message_id}", get(mail_message))
        .route("/mail/message/{message_id}/flags", post(mail_flags))
        // Declarada depois das anteriores: `/mail/compose` tem de bater na
        // rota literal, não em `{mailbox_id}`.
        .route("/mail/{mailbox_id}", get(mail_mailbox))
        .route("/mail/{mailbox_id}/sync", post(mail_sync))
        // Investigação
        .route("/units", get(units))
        .route("/units/{unit_id}", get(unit_detail))
        // Gerir quem pertence a uma unidade. Três operações, três caminhos: uma
        // pertença é autoridade, e cada alteração dela é um acto próprio.
        .route(
            "/workspaces/{workspace_id}/members",
            post(workspace_member_add),
        )
        .route(
            "/workspaces/{workspace_id}/members/remove",
            post(workspace_member_remove),
        )
        .route("/units/{unit_id}/members", post(unit_member_add))
        .route("/units/{unit_id}/members/role", post(unit_member_role))
        .route("/units/{unit_id}/members/remove", post(unit_member_remove))
        .route("/ideas", get(ideas))
        .route("/calendar", get(calendar_page))
        .route(
            "/calendar/events/new",
            get(new_event_form).post(create_calendar_event),
        )
        .route("/calendar/events/{event_id}", get(event_detail_page))
        .route(
            "/calendar/events/{event_id}/edit",
            get(edit_event_form).post(update_calendar_event),
        )
        .route(
            "/calendar/events/{event_id}/cancel",
            post(cancel_calendar_event),
        )
        .route("/notifications", get(notifications_page))
        .route("/notifications/recent", get(notifications_recent))
        .route(
            "/notifications/{notification_id}/read",
            post(mark_notification_read),
        )
        .route("/units/new", get(new_unit_form).post(create_unit))
        .route("/projects/new", get(new_project_form).post(promote_idea))
        .route(
            "/bibliography/new",
            get(new_source_form).post(create_source),
        )
        .route("/datasets/new", get(new_dataset_form).post(create_dataset))
        .route("/help", get(help))
        .route("/settings", get(settings_account))
        .route("/settings/security", get(settings_security))
        .route("/settings/password", post(change_password))
        .route("/settings/avatar/preset", post(choose_avatar_preset))
        .route(
            "/settings/avatar/photo",
            post(upload_avatar).layer(DefaultBodyLimit::max(AVATAR_BODY_LIMIT_BYTES)),
        )
        .route("/settings/avatar/initials", post(use_initials_avatar))
        // A fotografia do próprio membro. `me` não é um parâmetro: é a sessão
        // que diz de quem é, e a versão no caminho é só o endereço daquele
        // conteúdo.
        .route("/avatar/me/{version}", get(own_avatar))
        .route(
            "/settings/sessions/{session_id}/revoke",
            post(revoke_session),
        )
        .route("/ideas/new", get(new_idea_form).post(create_idea))
        .route("/ideas/{idea_id}", get(idea_workspace))
        .route("/projects", get(projects))
        .route("/projects/{project_id}", get(project_workspace))
        .route("/workspaces/{workspace_id}", get(research_workspace))
        // A cadeia científica do ambiente, e um resultado com a sua
        // proveniência. `/results/{id}` é raiz e não está debaixo do
        // ambiente: um resultado é citável, e um caminho que exigisse saber
        // em que ambiente ele vive obrigaria quem tem o link a descobri-lo.
        .route("/workspaces/{workspace_id}/science", get(scientific_chain))
        // Cada criação abre a partir do sítio onde a pergunta nasce, e leva o
        // contexto consigo em vez de o pedir. É o que faz a proveniência
        // acontecer sozinha: quem regista um resultado dentro de uma execução
        // não declara depois que aquela execução o produziu.
        .route(
            "/workspaces/{workspace_id}/science/hypotheses/new",
            get(new_hypothesis).post(create_hypothesis),
        )
        .route(
            "/workspaces/{workspace_id}/science/methodologies/new",
            get(new_methodology).post(create_methodology),
        )
        .route(
            "/workspaces/{workspace_id}/science/studies/new",
            get(new_study).post(create_study),
        )
        .route("/methodologies/{methodology_id}", get(methodology_detail))
        .route(
            "/methodologies/{methodology_id}/versions/new",
            get(new_version).post(publish_version),
        )
        .route("/studies/{study_id}", get(study_detail))
        .route(
            "/studies/{study_id}/executions/new",
            get(new_execution).post(record_execution),
        )
        .route("/executions/{execution_id}", get(execution_detail))
        .route(
            "/executions/{execution_id}/results/new",
            get(new_result).post(create_result),
        )
        .route("/results/{result_id}", get(result_detail))
        .route(
            "/results/{result_id}/validate",
            get(validate_result_form).post(record_validation),
        )
        // Conhecimento
        .route("/knowledge", get(knowledge))
        .route("/bibliography", get(bibliography))
        .route(
            "/bibliography/tools",
            get(bibliography_tools).post(review_bibliography),
        )
        .route("/datasets", get(datasets))
        .route("/files", get(files_browse))
        .route(
            "/files/upload",
            post(files_upload).layer(DefaultBodyLimit::max(FILE_BODY_LIMIT_BYTES)),
        )
        .route("/files/folder", post(files_new_folder))
        .route("/files/{file_id}", get(file_detail))
        .route(
            "/files/{file_id}/version",
            post(file_new_version).layer(DefaultBodyLimit::max(FILE_BODY_LIMIT_BYTES)),
        )
        .route("/files/{file_id}/download", get(file_download))
        .route("/files/{file_id}/preview", get(file_preview))
        .route(
            "/file-versions/{version_id}/preview",
            get(file_version_preview),
        )
        .route(
            "/file-versions/{version_id}/download",
            get(version_download),
        )
        // Inteligência
        .route("/ai", get(ai_hub))
        .route("/ai/agents", get(agents))
        .route("/ai/agents/new", get(new_agent).post(create_agent))
        .route("/ai/prompt", get(prompt).post(submit_prompt))
        .route("/compute", get(compute))
        // Institucional
        .route("/activity", get(activity))
        .route("/admin", get(admin))
        .route("/admin/members/new", get(new_member).post(create_member))
        .route("/admin/members/{person_id}", get(member_detail))
        .route("/audit", get(audit))
        .route("/search", get(search))
        // A Universal Command Surface.
        .route("/ask", get(ask))
        .route("/ask/plans/{plan_id}/execute", post(execute_plan))
        .route("/ask/plans/{plan_id}/reject", post(reject_plan))
        // Autenticação
        .route("/boot", get(boot_screen))
        .route("/login", get(login).post(login_submit))
        .route("/first-access", get(first_access).post(first_access_submit))
        .route("/logout", post(logout))
        .route("/health", get(health))
        .nest_service("/static", ServeDir::new(state.config.static_dir.clone()))
        .fallback(not_found)
        // O portão de arranque corre **antes** de qualquer página ser
        // construída. Uma pessoa que abra o Ocinye OS vê o arranque, e não o
        // Workspace a ser escondido depois.
        .layer(axum::middleware::from_fn(boot_gate))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            same_origin_only,
        ))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            security_headers,
        ))
        .with_state(state)
}

/// O portão de arranque.
///
/// # O que faz
///
/// Um pedido de documento que chegue a este Workspace sem ter visto o arranque
/// nesta janela é encaminhado para `/boot`, com o destino original preservado.
///
/// # Porque é que isto é um portão e não um estado dentro das páginas
///
/// Porque a alternativa é cada página decidir por si se já houve arranque — e
/// uma página nova que se esqueça disso passa a ser a porta de trás. Um portão
/// vale para tudo o que passa, incluindo o que ainda não foi escrito.
///
/// # O que não é encaminhado
///
/// O próprio arranque, os estáticos, a sonda de saúde, e tudo o que não é um
/// documento. Um pedido de folha de estilos ou de imagem não é uma pessoa a
/// abrir o Ocinye OS.
///
/// As submissões de formulário também não: encaminhar um `POST` perderia o que
/// alguém escreveu, e o arranque já terá acontecido antes de haver formulário
/// para submeter.
/// Não recebe estado de propósito: o portão decide pelo pedido, e um portão que
/// consultasse o Core a cada navegação seria um monitor contínuo. A observação
/// contínua é da topbar; isto é o ciclo de entrada.
async fn boot_gate(request: axum::extract::Request, next: axum::middleware::Next) -> Response {
    let caminho = request.uri().path().to_owned();
    let metodo = request.method().clone();

    let dispensado = metodo != axum::http::Method::GET
        || caminho == "/boot"
        || caminho == "/health"
        || caminho.starts_with("/static/")
        || caminho.starts_with("/avatar/");

    if dispensado {
        return next.run(request).await;
    }

    // Um pedido que não pede HTML não é uma pessoa a abrir o sistema.
    let quer_html = request
        .headers()
        .get(axum::http::header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .is_none_or(|v| v.contains("text/html") || v.contains("*/*"));
    if !quer_html {
        return next.run(request).await;
    }

    let cookies = request
        .headers()
        .get(axum::http::header::COOKIE)
        .and_then(|v| v.to_str().ok());

    if crate::boot::has_marker(cookies) {
        return next.run(request).await;
    }

    let destino = match request.uri().query() {
        Some(consulta) => format!("{caminho}?{consulta}"),
        None => caminho,
    };
    let destino =
        crate::boot::safe_return_target(&destino, ROUTES).unwrap_or_else(|| "/".to_owned());

    Redirect::to(&format!("/boot?return_to={}", urlencoding_minimo(&destino))).into_response()
}

/// Codifica um destino para caber numa cadeia de consulta.
///
/// Mínimo de propósito: o destino já passou pela validação contra o catálogo, e
/// o que aqui falta é apenas não partir a própria consulta.
fn urlencoding_minimo(valor: &str) -> String {
    valor
        .chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '/' | '-' | '_' | '.' | '~' => c.to_string(),
            outro => outro
                .to_string()
                .bytes()
                .map(|b| format!("%{b:02X}"))
                .collect(),
        })
        .collect()
}

/// Cabeçalhos aplicados a todas as respostas.
///
/// A política de conteúdo permite o stylesheet e o script próprios, as fontes
/// do Google e mais nada: sem origens de terceiros, sem inline, sem frames.
///
/// # O transporte, e porque só em produção
///
/// `Strict-Transport-Security` diz ao browser para nunca mais falar com esta
/// origem em claro. É a defesa contra o primeiro pedido — aquele que acontece
/// antes de qualquer redireccionamento para HTTPS, e onde um intermediário
/// ainda tem uma palavra a dizer.
///
/// Sai apenas quando a instalação é de produção, e aí a configuração já exigiu
/// que `OCINYE_WORKSPACE_PUBLIC_URL` seja `https` e que o cookie de sessão seja
/// `Secure`. Enviá-lo em desenvolvimento, onde o Workspace corre em claro,
/// trancaria o `localhost` do browser de quem desenvolve durante um ano.
async fn security_headers(
    State(state): State<WorkspaceState>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    let producao = state.config.is_production;
    let mut response = next.run(request).await;

    if producao {
        response
            .headers_mut()
            .entry("strict-transport-security")
            .or_insert(HeaderValue::from_static(
                "max-age=31536000; includeSubDomains",
            ));
    }

    const HEADERS: &[(&str, &str)] = &[
        ("x-content-type-options", "nosniff"),
        ("x-frame-options", "DENY"),
        ("referrer-policy", "same-origin"),
        ("cross-origin-opener-policy", "same-origin"),
        (
            "content-security-policy",
            "default-src 'none'; \
             script-src 'self'; \
             style-src 'self' https://fonts.googleapis.com; \
             font-src https://fonts.gstatic.com; \
             img-src 'self' data:; \
             connect-src 'self'; \
             form-action 'self'; base-uri 'none'; frame-ancestors 'none'",
        ),
        (
            "permissions-policy",
            "geolocation=(), microphone=(), camera=()",
        ),
        // As páginas são por membro; uma cache partilhada nunca as pode reter.
        ("cache-control", "no-store"),
    ];

    for (name, value) in HEADERS {
        if let (Ok(name), Ok(value)) = (
            header::HeaderName::from_bytes(name.as_bytes()),
            HeaderValue::from_str(value),
        ) {
            response.headers_mut().entry(name).or_insert(value);
        }
    }
    response
}

/// Recusa escritas que não venham desta origem.
///
/// # Porque `SameSite` não chega
///
/// A sessão do Workspace é um cookie `SameSite=Lax`, e isso bloqueia um `POST`
/// vindo de **outro site**. Mas «site» não é «origem»: `SameSite` compara o
/// domínio registável, por isso uma página em `ocinye.com` — que o `CLAUDE.md`
/// §5 reserva para o futuro website público — é *same-site* com
/// `workspace.ocinye.com`, e o browser envia o cookie com ela.
///
/// O mesmo vale para qualquer XSS num subdomínio irmão: passaria a ser um CSRF
/// contra o Workspace. Um subdomínio não é uma fronteira de confiança
/// (`CLAUDE.md` §16).
///
/// # A regra
///
/// Em métodos que alteram estado, o `Origin` tem de existir e tem de ser esta
/// origem. Os browsers enviam-no em todos os `POST`, incluindo os do próprio
/// sítio, por isso exigi-lo não parte nada — e a sua ausência num pedido do
/// browser é ela própria anómala. `GET` e `HEAD` não alteram estado e não são
/// verificados (nenhuma rota do Workspace muda estado por `GET`, o que este
/// desenho pressupõe e o teste abaixo prende).
async fn same_origin_only(
    State(state): State<WorkspaceState>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    if !changes_state(request.method()) {
        return next.run(request).await;
    }

    let origin = request
        .headers()
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok());

    let host = request
        .headers()
        .get(header::HOST)
        .and_then(|value| value.to_str().ok());

    if origin_is_ours(origin, host, &state.config.public_url) {
        return next.run(request).await;
    }

    // Sem detalhe: quem sondar a fronteira não recebe um mapa dela.
    tracing::warn!(
        path = %request.uri().path(),
        "refused a state-changing request from another origin"
    );
    (
        StatusCode::FORBIDDEN,
        page(
            "Pedido recusado",
            ui::screens::login::login(
                true,
                Some("Este pedido não veio do Ocinye Workspace.".to_owned()),
            ),
        ),
    )
        .into_response()
}

/// Métodos que podem alterar estado.
fn changes_state(method: &axum::http::Method) -> bool {
    !matches!(
        *method,
        axum::http::Method::GET | axum::http::Method::HEAD | axum::http::Method::OPTIONS
    )
}

/// Se um `Origin` é o próprio Workspace.
///
/// A origem pública configurada é a resposta em qualquer deployment real.
///
/// # A tolerância local, e o seu limite
///
/// Fora de produção, aceita-se também um `Origin` cujo host coincida com o
/// `Host` do pedido — o mesmo processo responde em `localhost` e em
/// `127.0.0.1`, e o `Host` é preenchido pelo browser com o alvo real, não pelo
/// atacante. Essa tolerância vale **apenas quando a origem configurada não é
/// `https`**, isto é, em desenvolvimento.
///
/// Sem esse limite, comparar só o host aceitaria `http://` num Workspace
/// servido em `https://`, o que é uma despromoção de esquema: alguém na rede
/// que sirva uma página em claro no mesmo nome de host voltaria a poder
/// escrever. A configuração de produção já exige `https`
/// ([`WorkspaceConfig::validate`](crate::config::WorkspaceConfig)), por isso
/// esta linha e essa dizem a mesma coisa.
fn origin_is_ours(origin: Option<&str>, host: Option<&str>, public_url: &str) -> bool {
    let Some(origin) = origin.map(str::trim).filter(|value| !value.is_empty()) else {
        return false;
    };
    // `null` é o que um browser envia a partir de um `iframe` sandboxed ou de
    // um documento `data:`. Nunca é esta origem.
    if origin.eq_ignore_ascii_case("null") {
        return false;
    }

    if origin.trim_end_matches('/') == public_url.trim_end_matches('/') {
        return true;
    }

    if public_url.starts_with("https://") {
        return false;
    }

    let Some(("http", origin_host)) = origin.split_once("://") else {
        return false;
    };

    matches!(host, Some(host) if origin_host.trim_end_matches('/').eq_ignore_ascii_case(host))
}

async fn health() -> &'static str {
    "ok"
}

// ── Sessão ───────────────────────────────────────────────────────────────

/// O membro activo.
struct Member {
    session: Session,
    correlation_id: String,
    /// A zona em que este membro está a olhar para o sistema.
    ///
    /// Vem do browser. Sem ela, cai em UTC — que é a resposta menos errada
    /// quando não se sabe onde a pessoa está, e não uma preferência.
    zona: ocinye_contracts::temporal::TimeZoneName,
}

fn current_member(state: &WorkspaceState, headers: &HeaderMap) -> Option<Member> {
    let cookie = headers
        .get(header::COOKIE)
        .and_then(|value| value.to_str().ok());
    let id = session::session_id_from_cookies(cookie)?;
    let session = state.sessions.get(&id)?;
    Some(Member {
        session,
        correlation_id: Uuid::new_v4().to_string(),
        zona: ui::tempo::zona_declarada(session::zone_from_cookies(cookie).as_deref()),
    })
}

/// Obtém um valor do Core, devolvendo `Null` quando um painel isolado falha.
///
/// Um ecrã com vários painéis deve continuar a renderizar quando um deles não
/// carrega; o painel mostra o seu próprio estado vazio.
/// Obtém um valor do Core, distinguindo recusa de ausência.
///
/// [`optional`] engole tudo em `Null`, o que é certo para um painel isolado
/// dentro de um ecrã e **errado** para o conteúdo principal: um 403 ou 404
/// renderizado como lista vazia diz «não existe nenhum» a quem apenas não pode
/// ver (briefing §57).
async fn required(
    state: &WorkspaceState,
    member: &Member,
    path: &str,
) -> Result<Value, ApiFailure> {
    api::get::<Value>(
        state,
        &member.session.access_token,
        &member.correlation_id,
        path,
    )
    .await
}

async fn optional(state: &WorkspaceState, member: &Member, path: &str) -> Value {
    api::get::<Value>(
        state,
        &member.session.access_token,
        &member.correlation_id,
        path,
    )
    .await
    .unwrap_or(Value::Null)
}

/// O contexto da shell: quem está a ver, o que pode, e se o Core responde.
/// Se o Intelligence Plane consegue servir alguma coisa, segundo o Core.
///
/// Ausência lê-se como indisponível. O contrário — assumir disponível quando
/// não se confirmou — poria um campo activo à frente de alguém para depois
/// falhar (`docs/ui-core-contract/`).
fn inference_available(status: &Value) -> bool {
    status
        .get("available")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

async fn viewer(state: &WorkspaceState, member: &Member) -> Viewer {
    // A agenda e as notificações vão em paralelo com o resto: a barra superior
    // desenha-se em cada página, e uma consulta em série acrescentaria latência
    // a todas elas.
    let agora = chrono::Utc::now();
    let (organisation, me, temporal, notificacoes) = tokio::join!(
        optional(state, member, "/api/v1/organisation"),
        optional(state, member, "/api/v1/me"),
        calendar_agenda(
            state,
            member,
            agora - chrono::Duration::hours(12),
            agora + chrono::Duration::days(14),
        ),
        optional(state, member, "/api/v1/notifications"),
    );
    // O estado do Core vem do `/ready`, e nunca de um pedido de domínio.
    //
    // Isto era `!organisation.is_null()`: se a consulta de organização
    // respondesse, o Core estaria bem. Um pedido de domínio responde por razões
    // suas, e uma delas não é a prontidão institucional — a base podia estar de
    // pé com a compatibilidade quebrada, e a topbar diria «CORE OK».
    //
    // `Degraded` é `Ok` aqui, e isso não é indulgência: é o que o distintivo
    // diz. Ele diz **CORE**, e `decide()` no Core devolve `Blocked` antes de
    // chegar a `Degraded`, portanto `Degraded` significa, por construção, que
    // todos os componentes críticos estão disponíveis e que algum opcional não
    // está. Um Core inteiro e operacional não fica «limitado» por não haver
    // SMTP configurado nem nenhum nó de computação registado.
    //
    // A prontidão da instalação continua a dizer a verdade: `/ready` responde
    // `degraded` e nomeia os componentes. São duas afirmações diferentes sobre
    // coisas diferentes, e passam a ser ditas em separado.
    let core_status = {
        use crate::boot::BootState;
        match crate::boot::probe(state).await.state {
            BootState::Ready | BootState::Degraded => ui::shell::CoreStatus::Ok,
            BootState::Blocked => ui::shell::CoreStatus::Unavailable,
            BootState::Unreachable | BootState::Uninitialized | BootState::Checking => {
                ui::shell::CoreStatus::Silent
            }
        }
    };

    // Erro e vazio dizem-se de maneiras diferentes, também aqui.
    let (temporal_items, temporal_failure) = match temporal {
        Ok(payload) => (ui::screens::calendar::items_from(&payload), None),
        Err(erro) => (Vec::new(), Some(erro.to_string())),
    };
    let unread = notificacoes
        .get("unread")
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;

    // Sem resposta do Core, a lista fica vazia e a navegação encolhe ao mínimo.
    // É o comportamento certo: não conseguir confirmar o que alguém pode não é
    // razão para lhe mostrar tudo (`CLAUDE.md` §31).
    let capabilities = me
        .get("capabilities")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();

    // Os módulos relevantes, como o Core os projectou. Sem resposta dele a
    // lista fica vazia e a navegação encolhe — a mesma regra das capacidades.
    let modules: Vec<String> = me
        .get("modules")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter(|m| m.get("relevant").and_then(Value::as_bool) == Some(true))
                .filter_map(|m| m.get("module").and_then(Value::as_str))
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();

    Viewer {
        zona: member.zona,
        name: member.session.display_name.clone(),
        // O endereço vem do Core, que é onde o registo vive. A sessão local
        // serve de recurso: é aquele com que a pessoa entrou, e é o mesmo.
        email: me
            .get("email")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .or_else(|| {
                let entrada = member.session.email.trim();
                (!entrada.is_empty()).then(|| entrada.to_owned())
            }),
        session_expires_in: member
            .session
            .expires_at
            .checked_duration_since(std::time::Instant::now()),
        // Sem resposta do Core ficam as iniciais. Não saber qual é a escolha
        // não é razão para inventar uma, e as iniciais não dependem de nada
        // para estarem certas.
        avatar: me
            .get("avatar")
            .and_then(|value| serde_json::from_value(value.clone()).ok())
            .unwrap_or(ocinye_contracts::AvatarChoice::Initials),
        organisation: organisation
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("Ocinye")
            .to_owned(),
        core_status,
        temporal: temporal_items,
        temporal_failure,
        unread,
        capabilities,
        modules,
    }
}

/// Renderiza um documento.
fn page(title: &str, body: impl leptos::IntoView + 'static) -> Response {
    Html(ui::document(title, body)).into_response()
}

/// Renderiza um ecrã dentro da shell.
fn shell_page(
    title: &str,
    viewer: &Viewer,
    active: Screen,
    trail: Vec<Crumb>,
    content: impl leptos::IntoView + 'static,
) -> Response {
    page(
        title,
        ui::shell::shell(viewer, active, trail, title, content),
    )
}

/// Traduz uma recusa do Core em algo sobre que o membro possa agir.
fn failure_response(failure: &ApiFailure) -> Response {
    match failure {
        // Uma sessão expirada é um acontecimento normal, não um erro a explicar.
        ApiFailure::Unauthorised => Redirect::to("/login").into_response(),

        // Recusa e inexistência têm o mesmo aspecto de propósito: revelar que
        // um recurso existe mas está fechado já é informação (ADR-0100).
        //
        // Antes desta auditoria isto renderizava o **ecrã de login**, pelo que
        // um membro com sessão válida via um formulário de autenticação e
        // concluía que a sua sessão tinha terminado (briefing §46, §116).
        ApiFailure::Denied => (
            StatusCode::NOT_FOUND,
            page("Não encontrado", ui::screens::notice::not_found()),
        )
            .into_response(),

        // Uma recusa de autorização não é um erro inesperado, e não deve
        // aparecer como tal: o membro precisa de saber que é uma questão de
        // acesso e o que fazer a seguir (briefing §46, §106).
        ApiFailure::Forbidden => (
            StatusCode::FORBIDDEN,
            page("Sem acesso", ui::screens::notice::access_denied()),
        )
            .into_response(),

        // Uma dependência em falta não é uma avaria, e a página não pode
        // dizer «erro» a quem precisa de saber que a instalação não tem uma
        // peça de pé. A capacidade existe; falta o serviço.
        ApiFailure::Unavailable(razao) => (
            StatusCode::SERVICE_UNAVAILABLE,
            page(
                "Indisponível",
                ui::screens::notice::unavailable(razao.clone()),
            ),
        )
            .into_response(),

        // Uma recusa por conteúdo é uma resposta, e não uma avaria. Quem
        // chega aqui vindo de um formulário devia tê-la apanhado antes, para
        // a mostrar ao lado do campo; esta é a rede para quem não o fez.
        ApiFailure::Rejected(message) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            page("Pedido recusado", ui::screens::notice::rejected(message)),
        )
            .into_response(),

        ApiFailure::Failed(message) => {
            // O detalhe vai para o log, com o identificador que o correlaciona;
            // ao membro vai a referência e nada mais (briefing §47, §69).
            let reference = Uuid::new_v4().to_string();
            tracing::error!(
                reference = %reference,
                detail = %message,
                "a Core call failed"
            );
            (
                StatusCode::BAD_GATEWAY,
                page("Erro", ui::screens::notice::failure(&reference)),
            )
                .into_response()
        }
    }
}

/// Caminho que o Workspace não serve.
///
/// Sem isto, o Axum devolvia um 404 de corpo vazio: uma página em branco com o
/// aspecto do framework e não do Ocinye OS (briefing §75).
async fn not_found() -> Response {
    (
        StatusCode::NOT_FOUND,
        page("Página não encontrada", ui::screens::notice::not_found()),
    )
        .into_response()
}

// ── Arranque ─────────────────────────────────────────────────────────────

/// O que o arranque recebe de quem o pede.
#[derive(Deserialize)]
struct BootQuery {
    /// Para onde seguir quando o Core deixar.
    #[serde(default)]
    return_to: Option<String>,
}

/// O arranque institucional.
///
/// # Porque é que isto é uma rota, e não um estado dentro de outra página
///
/// Porque o arranque acontece antes de haver Workspace. Uma superfície que
/// vivesse dentro do Workspace obrigaria a renderizar o Workspace primeiro e a
/// escondê-lo depois — e um flash de conteúdo protegido é conteúdo protegido
/// mostrado.
///
/// # A prontidão é apurada aqui, no servidor
///
/// Quando esta página chega ao browser, a decisão já foi tomada. Não há
/// percentagens a subir nem etapas a acender: o que se vê é o que o Core disse.
async fn boot_screen(
    State(state): State<WorkspaceState>,
    Query(query): Query<BootQuery>,
) -> Response {
    let destino = query
        .return_to
        .as_deref()
        .and_then(|d| crate::boot::safe_return_target(d, ROUTES))
        .unwrap_or_else(|| "/".to_owned());

    let outcome = crate::boot::probe(&state).await;
    let segue = outcome.state.may_hand_off();

    let corpo = ui::screens::boot::boot(&outcome, &destino);
    let cabeca = ui::screens::boot::handoff_meta(&outcome, &destino);
    let html = ui::document_com_cabeca("A iniciar", corpo, cabeca);

    let mut resposta = Html(html).into_response();

    // O arranque nunca é guardado. Uma prontidão em cache é uma resposta sobre
    // um sistema que já não existe.
    resposta.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store, no-cache, must-revalidate"),
    );

    // O marcador só é gravado quando houve por onde seguir. Gravá-lo num
    // arranque bloqueado faria a tentativa seguinte saltar a apresentação de um
    // problema que continua lá.
    if segue {
        if let Ok(valor) = axum::http::HeaderValue::from_str(&crate::boot::marker_cookie(
            state.config.cookie_secure,
        )) {
            resposta
                .headers_mut()
                .append(axum::http::header::SET_COOKIE, valor);
        }
    }

    resposta
}

/// Macro de guarda: sem sessão, vai para o login.
macro_rules! member_or_login {
    ($state:expr, $headers:expr) => {
        match current_member(&$state, &$headers) {
            // Quem deve ao Core uma palavra-passe definitiva não passa daqui.
            // O Core recusaria na mesma; isto poupa-lhe um erro em vez de um
            // ecrã (briefing §22).
            Some(member) if member.session.must_change_password => {
                return Redirect::to("/first-access").into_response()
            }
            Some(member) => member,
            None => return Redirect::to("/login").into_response(),
        }
    };
}

// ── Pessoal ──────────────────────────────────────────────────────────────

async fn home(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    let member = member_or_login!(state, headers);
    let viewer = viewer(&state, &member).await;

    let (workspaces, tasks, activity, intelligence, units, ideas, projects, datasets) = tokio::join!(
        optional(&state, &member, "/api/v1/workspaces?page_size=6"),
        optional(
            &state,
            &member,
            "/api/v1/tasks?mine=true&open_only=true&page_size=8"
        ),
        optional(&state, &member, "/api/v1/activity?page_size=8"),
        optional(&state, &member, "/api/v1/ai/status"),
        optional(&state, &member, "/api/v1/units"),
        // Cada contador pede o seu tipo, tal como a lista que abre. Ambos
        // chamavam `/workspaces?page_size=1` sem filtro, pelo que mostravam
        // sempre o mesmo total — invisível só enquanto ambos eram zero.
        optional(&state, &member, "/api/v1/workspaces?kind=idea&page_size=1"),
        optional(
            &state,
            &member,
            "/api/v1/workspaces?kind=project&page_size=1"
        ),
        optional(&state, &member, "/api/v1/datasets?page_size=1"),
    );

    let kpis: Vec<ui::components::Kpi> = [
        kpi("UNIDADES", count_of(&units), "activas", "/units"),
        kpi("IDEIAS", count_of(&ideas), "em investigação", "/ideas"),
        kpi("PROJECTOS", count_of(&projects), "em execução", "/projects"),
        kpi("DATASETS", count_of(&datasets), "catalogados", "/datasets"),
    ]
    .into_iter()
    .flatten()
    .collect();

    let content = ui::screens::home::home(ui::screens::home::Dashboard {
        greeting: ui::screens::home::greeting_for(local_hour()).to_owned(),
        name: viewer.name.clone(),
        can_create_idea: viewer.can(ocinye_contracts::Permission::IdeasCreate),
        kpis,
        workspaces,
        tasks,
        activity,
        intelligence,
    });

    shell_page("Home", &viewer, Screen::Home, Vec::new(), content)
}

/// A hora local, para a saudação.
///
/// Deriva de UTC porque o servidor não conhece o fuso do membro; um erro de
/// saudação é preferível a inventar um fuso.
fn local_hour() -> u32 {
    chrono::Utc::now()
        .format("%H")
        .to_string()
        .parse()
        .unwrap_or(9)
}

/// A contagem de uma colecção, quando o Core a devolveu.
///
/// `None` quando não devolveu — o que acontece tanto por indisponibilidade como
/// por recusa de acesso. Distinguir isto de zero importa: `optional` engole a
/// recusa e devolve `Null`, e apresentar `0` diria «não existe nenhum» a quem
/// apenas não pode ver (briefing §57).
fn count_of(payload: &Value) -> Option<String> {
    if payload.is_null() {
        return None;
    }
    payload
        .get("total")
        .and_then(Value::as_i64)
        .or_else(|| {
            payload
                .as_array()
                .map(|a| i64::try_from(a.len()).unwrap_or(0))
        })
        .map(|count| count.to_string())
}

/// Um indicador da Home.
///
/// O cartão aparece sempre. Quando o Core não respondeu à contagem mostra `—`
/// e diz-se indisponível, em vez de desaparecer: um cartão que some não
/// informa ninguém de que algo falhou. E nunca mostra `0`, que afirmaria uma
/// consulta bem-sucedida sem registos — indistinguível de um acervo vazio.
///
/// Antes devolvia `None` quando o Core não respondeu, porque um cartão que
/// diz `0` sobre uma colecção que o membro não pode ver é uma estatística
/// inventada, e um cartão que liga a um ecrã que lhe será recusado é uma
/// ligação morta (briefing §19, §52).
fn kpi(label: &str, value: Option<String>, hint: &str, href: &str) -> Option<ui::components::Kpi> {
    Some(ui::components::Kpi {
        label: label.to_owned(),
        value,
        // O Core ainda não expõe variação entre períodos. Mostrar um delta
        // inventado seria pior do que mostrar nenhum.
        delta: None,
        hint: hint.to_owned(),
        href: href.to_owned(),
    })
}

async fn my_work(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    let member = member_or_login!(state, headers);
    let viewer = viewer(&state, &member).await;

    let (tasks, workspaces, activity) = tokio::join!(
        optional(
            &state,
            &member,
            "/api/v1/tasks?mine=true&open_only=true&page_size=50"
        ),
        // `mine=true`: o cartão promete «investigação em que participo», e ver
        // um ambiente não é participar nele. Sem o recorte, esta secção
        // mostrava tudo o que o membro alcança — que é outra coisa, e mais.
        optional(&state, &member, "/api/v1/workspaces?mine=true&page_size=20"),
        optional(&state, &member, "/api/v1/activity?page_size=20"),
    );

    let content = ui::screens::my_work::my_work(&tasks, &workspaces, &activity);
    shell_page(
        "O Meu Trabalho",
        &viewer,
        Screen::MyWork,
        Vec::new(),
        content,
    )
}

// ── Correio ──────────────────────────────────────────────────────────────
//
// Uma regra atravessa todos estes manipuladores: **apenas `send_mail` fala com
// o serviço de correio**. `assist` devolve texto e volta a desenhar o composer;
// não tem forma de enviar, e não é por convenção — é por não chamar a rota que
// envia (briefing §15).

/// O que uma vista de correio precisa, recolhido em paralelo.
///
/// Estado e caixas são pedidos ao mesmo tempo porque nenhum depende do outro, e
/// a diferença é visível: são duas viagens ao Core em cada ecrã de correio.
async fn mail_context(
    state: &WorkspaceState,
    member: &Member,
    mailbox: Option<String>,
    folder: String,
    query: String,
) -> ui::screens::mail::MailView {
    let (status, mailboxes) = tokio::join!(
        optional(state, member, "/api/v1/mail/status"),
        optional(state, member, "/api/v1/mail/mailboxes"),
    );

    ui::screens::mail::MailView {
        status,
        sync_notice: None,
        mailboxes,
        active_mailbox: mailbox,
        folder,
        query,
    }
}

#[derive(Deserialize)]
struct SyncForm {
    #[serde(default)]
    folder: Option<String>,
}

/// Actualiza uma pasta e volta para ela, com o resultado à vista.
///
/// Redirecciona em vez de renderizar: sem isso, recarregar a página voltaria a
/// submeter o pedido de actualização.
async fn mail_sync(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(mailbox_id): Path<Uuid>,
    Form(form): Form<SyncForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let folder = form.folder.unwrap_or_else(|| "inbox".to_owned());

    let body = serde_json::json!({ "folder": folder });
    let path = format!("/api/v1/mail/mailboxes/{mailbox_id}/sync");

    let outcome = match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &path,
        &body,
    )
    .await
    {
        Ok(result) => {
            let indexed = result.get("indexed").and_then(Value::as_u64).unwrap_or(0);
            format!("{indexed} mensagem(ns) actualizada(s).")
        }
        Err(ApiFailure::Unauthorised) => return Redirect::to("/login").into_response(),
        // A falha volta com a caixa, não num ecrã de erro: a lista continua
        // utilizável, apenas desactualizada, e o membro precisa de saber isso.
        Err(failure) => failure.to_string(),
    };

    Redirect::to(&format!(
        "/mail/{mailbox_id}?folder={folder}&sync={}",
        urlencoding_minimal(&outcome)
    ))
    .into_response()
}

#[derive(Deserialize)]
struct MailQuery {
    #[serde(default)]
    folder: Option<String>,
    #[serde(default)]
    q: Option<String>,
    /// O resultado de uma actualização acabada de pedir, para o mostrar.
    #[serde(default)]
    sync: Option<String>,
}

async fn mail(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(query): Query<MailQuery>,
) -> Response {
    mail_screen(state, headers, None, query, None).await
}

async fn mail_mailbox(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(mailbox_id): Path<String>,
    Query(query): Query<MailQuery>,
) -> Response {
    mail_screen(state, headers, Some(mailbox_id), query, None).await
}

/// O ecrã de correio, com ou sem mensagem aberta.
async fn mail_screen(
    state: WorkspaceState,
    headers: HeaderMap,
    mailbox_id: Option<String>,
    query: MailQuery,
    open: Option<Value>,
) -> Response {
    let member = member_or_login!(state, headers);
    let viewer = viewer(&state, &member).await;

    let folder = query.folder.unwrap_or_else(|| "inbox".to_owned());
    let term = query.q.unwrap_or_default();

    let mut view = mail_context(&state, &member, mailbox_id, folder.clone(), term.clone()).await;
    view.sync_notice = query.sync;

    // Sem caixa resolvida não há lista a pedir. Pedi-la com um identificador
    // vazio produziria uma recusa do Core que não diz nada ao membro.
    let messages = match view
        .mailboxes
        .as_array()
        .and_then(|boxes| {
            view.active_mailbox.as_ref().map_or_else(
                || boxes.first(),
                |wanted| {
                    boxes.iter().find(|mailbox| {
                        mailbox.get("id").and_then(Value::as_str) == Some(wanted.as_str())
                    })
                },
            )
        })
        .and_then(|mailbox| mailbox.get("id"))
        .and_then(Value::as_str)
    {
        Some(id) => {
            let path = if term.trim().is_empty() {
                format!("/api/v1/mail/mailboxes/{id}/messages?folder={folder}")
            } else {
                format!(
                    "/api/v1/mail/mailboxes/{id}/messages?folder={folder}&q={}",
                    urlencoding_minimal(term.trim())
                )
            };
            optional(&state, &member, &path).await
        }
        None => Value::Null,
    };

    shell_page(
        "Correio",
        &viewer,
        Screen::Mail,
        Vec::new(),
        ui::screens::mail::mail(&viewer, &view, &messages, open.as_ref(), None),
    )
}

#[derive(Deserialize)]
struct MessageQuery {
    /// Se o membro pediu explicitamente o conteúdo remoto desta mensagem.
    #[serde(default)]
    remote: Option<String>,
}

async fn mail_message(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(message_id): Path<Uuid>,
    Query(query): Query<MessageQuery>,
) -> Response {
    let member = member_or_login!(state, headers);

    // Nunca por omissão: carregar conteúdo remoto informa quem enviou a
    // mensagem de que ela foi aberta (briefing §12).
    let allow_remote = query.remote.as_deref() == Some("1");
    let path = format!("/api/v1/mail/messages/{message_id}?allow_remote={allow_remote}");

    let opened = match required(&state, &member, &path).await {
        Ok(message) => message,
        Err(failure) => return failure_response(&failure),
    };

    let mailbox_id = opened
        .get("message")
        .and_then(|message| message.get("mailbox_id"))
        .and_then(Value::as_str)
        .map(str::to_owned);

    let folder = opened
        .get("message")
        .and_then(|message| message.get("folder"))
        .and_then(Value::as_str)
        .unwrap_or("inbox")
        .to_owned();

    mail_screen(
        state,
        headers,
        mailbox_id,
        MailQuery {
            folder: Some(folder),
            q: None,
            sync: None,
        },
        Some(opened),
    )
    .await
}

#[derive(Deserialize)]
struct FlagForm {
    field: String,
    value: String,
}

async fn mail_flags(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(message_id): Path<Uuid>,
    Form(form): Form<FlagForm>,
) -> Response {
    let member = member_or_login!(state, headers);

    let value = form.value == "true";
    let body = match form.field.as_str() {
        "starred" => serde_json::json!({ "starred": value }),
        "read" => serde_json::json!({ "read": value }),
        // Um campo desconhecido não é um pedido a interpretar generosamente.
        _ => return failure_response(&ApiFailure::Denied),
    };

    let path = format!("/api/v1/mail/messages/{message_id}/flags");
    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &path,
        &body,
    )
    .await
    {
        Ok(_) | Err(ApiFailure::Denied) => {
            Redirect::to(&format!("/mail/message/{message_id}")).into_response()
        }
        Err(failure) => failure_response(&failure),
    }
}

#[derive(Deserialize)]
struct ComposeQuery {
    #[serde(default)]
    mailbox: Option<String>,
    #[serde(default)]
    reply: Option<Uuid>,
}

async fn compose(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(query): Query<ComposeQuery>,
) -> Response {
    let member = member_or_login!(state, headers);
    let viewer = viewer(&state, &member).await;

    let view = mail_context(
        &state,
        &member,
        query.mailbox.clone(),
        "inbox".to_owned(),
        String::new(),
    )
    .await;

    let mut draft = ui::screens::mail::ComposeDraft {
        mailbox_id: query.mailbox.unwrap_or_default(),
        ..Default::default()
    };

    // Uma resposta traz o destinatário e o assunto já preenchidos, e a citação
    // do que se responde. Nada disto é gerado: é o que a mensagem original diz.
    if let Some(reply_to) = query.reply {
        let path = format!("/api/v1/mail/messages/{reply_to}?allow_remote=false");
        if let Ok(original) = required(&state, &member, &path).await {
            let message = original.get("message").cloned().unwrap_or(Value::Null);
            let subject = message
                .get("subject")
                .and_then(Value::as_str)
                .unwrap_or_default();

            draft.to = message
                .get("from_address")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            draft.subject = if subject.to_lowercase().starts_with("re:") {
                subject.to_owned()
            } else {
                format!("Re: {subject}")
            };
            draft.reply_to = Some(reply_to.to_string());
            if draft.mailbox_id.is_empty() {
                draft.mailbox_id = message
                    .get("mailbox_id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned();
            }
        }
    }

    // O correio por baixo, e o compositor por cima.
    //
    // Era uma página só com o formulário. Escrever passou a acontecer a olhar
    // para a caixa — que é como se escreve: a confirmar um nome, a reler o que
    // se responde, a ver o que entretanto chegou.
    let messages = match view
        .mailboxes
        .as_array()
        .and_then(|caixas| {
            view.active_mailbox.as_ref().map_or_else(
                || caixas.first(),
                |querida| {
                    caixas.iter().find(|caixa| {
                        caixa.get("id").and_then(Value::as_str) == Some(querida.as_str())
                    })
                },
            )
        })
        .and_then(|caixa| caixa.get("id"))
        .and_then(Value::as_str)
    {
        Some(id) => {
            optional(
                &state,
                &member,
                &format!("/api/v1/mail/mailboxes/{id}/messages?folder=inbox"),
            )
            .await
        }
        None => Value::Null,
    };

    shell_page(
        "Nova mensagem",
        &viewer,
        Screen::Mail,
        vec![Crumb::to(Screen::Mail)],
        ui::screens::mail::mail(&viewer, &view, &messages, None, Some(&draft)),
    )
}

/// O formulário do composer, tal como chega das duas rotas que o submetem.
#[derive(Deserialize)]
struct ComposeForm {
    #[serde(default)]
    mailbox_id: String,
    #[serde(default)]
    to: String,
    #[serde(default)]
    cc: String,
    #[serde(default)]
    subject: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    reply_to: Option<String>,
    #[serde(default)]
    action: String,
    #[serde(default)]
    instruction: String,
    #[serde(default)]
    confirmed: Option<String>,
}

impl ComposeForm {
    /// O rascunho tal como está, para o devolver intacto ao ecrã.
    fn draft(&self) -> ui::screens::mail::ComposeDraft {
        ui::screens::mail::ComposeDraft {
            mailbox_id: self.mailbox_id.clone(),
            to: self.to.clone(),
            cc: self.cc.clone(),
            subject: self.subject.clone(),
            body: self.body.clone(),
            reply_to: self.reply_to.clone(),
            instruction: self.instruction.clone(),
            confirmation: None,
            error: None,
            generated: false,
        }
    }
}

/// Gera texto e volta a desenhar o composer.
///
/// **Não envia.** Não chama `/api/v1/mail/send`, e o resultado aterra num campo
/// editável que exige um segundo acto humano (briefing §15).
async fn assist(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<ComposeForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let viewer = viewer(&state, &member).await;

    let view = mail_context(
        &state,
        &member,
        Some(form.mailbox_id.clone()),
        "inbox".to_owned(),
        String::new(),
    )
    .await;

    let body = serde_json::json!({
        "action": form.action,
        "instruction": form.instruction,
        "draft_body": form.body,
        "source_message_id": form.reply_to,
    });

    let mut draft = form.draft();

    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        "/api/v1/mail/assist",
        &body,
    )
    .await
    {
        Ok(result) => {
            if let Some(text) = result.get("text").and_then(Value::as_str) {
                draft.body = text.to_owned();
                draft.generated = true;
            }
        }
        Err(ApiFailure::Unauthorised) => return Redirect::to("/login").into_response(),
        // Uma assistência que falha não perde a mensagem escrita. O rascunho
        // volta como estava, com a razão por cima.
        Err(failure) => draft.error = Some(failure.to_string()),
    }

    shell_page(
        "Nova mensagem",
        &viewer,
        Screen::Mail,
        vec![Crumb::to(Screen::Mail)],
        ui::screens::mail::compose(&view, &draft),
    )
}

/// Envia. A única rota do Workspace que o faz.
async fn send_mail(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<ComposeForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let viewer = viewer(&state, &member).await;

    let split = |raw: &str| -> Vec<String> {
        raw.split([',', ';'])
            .map(str::trim)
            .filter(|address| !address.is_empty())
            .map(str::to_owned)
            .collect()
    };

    let body = serde_json::json!({
        "mailbox_id": form.mailbox_id,
        "to": split(&form.to),
        "cc": split(&form.cc),
        "subject": form.subject,
        "body": form.body,
        "confirmed": form.confirmed.is_some(),
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
        Ok(_) => Redirect::to("/mail").into_response(),
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        Err(failure) => {
            let view = mail_context(
                &state,
                &member,
                Some(form.mailbox_id.clone()),
                "inbox".to_owned(),
                String::new(),
            )
            .await;

            let mut draft = form.draft();
            let reason = failure.to_string();

            // O Core distingue «confirme» de «recusado». A interface tem de
            // distinguir também: um pedido de confirmação mostra a caixa de
            // confirmação, uma recusa não a mostra — confirmar não desfaz uma
            // recusa, e oferecer a caixa sugeriria que sim (briefing §35).
            if reason.contains("Confirme") {
                draft.confirmation = Some(reason);
            } else {
                draft.error = Some(reason);
            }

            (
                StatusCode::UNPROCESSABLE_ENTITY,
                shell_page(
                    "Nova mensagem",
                    &viewer,
                    Screen::Mail,
                    vec![Crumb::to(Screen::Mail)],
                    ui::screens::mail::compose(&view, &draft),
                ),
            )
                .into_response()
        }
    }
}

async fn mail_settings(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    let member = member_or_login!(state, headers);
    let viewer = viewer(&state, &member).await;

    let view = mail_context(&state, &member, None, "inbox".to_owned(), String::new()).await;
    let preferences = optional(&state, &member, "/api/v1/mail/preferences").await;

    shell_page(
        "Definições de correio",
        &viewer,
        Screen::Mail,
        vec![Crumb::to(Screen::Mail)],
        ui::screens::mail::settings(&view, &preferences),
    )
}

// ── Mensagens ────────────────────────────────────────────────────────────

/// A aplicação Mensagens, sem conversa aberta.
async fn messaging(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    let member = member_or_login!(state, headers);
    render_messaging(&state, &member, None).await
}

/// A aplicação Mensagens, com uma conversa aberta.
async fn messaging_conversation(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(conversation): Path<String>,
) -> Response {
    let member = member_or_login!(state, headers);
    render_messaging(&state, &member, Some(&conversation)).await
}

/// Desenha a aplicação.
///
/// # Porque a lista vem sempre
///
/// Porque a aplicação continua a ser as Mensagens mesmo quando não há conversa
/// aberta. Substituir o módulo inteiro por uma frase seria trocar a aplicação
/// por um aviso.
async fn render_messaging(
    state: &WorkspaceState,
    member: &Member,
    conversation: Option<&str>,
) -> Response {
    let viewer = viewer(state, member).await;

    // Erro e vazio não se dizem da mesma maneira. Uma lista que falhou a
    // carregar e aparecesse como «ainda não falou com ninguém» faria alguém
    // concluir que perdeu conversas.
    let (lista, failure) = match required(state, member, "/api/v1/messaging/conversations").await {
        Ok(valor) => (valor.as_array().cloned().unwrap_or_default(), None),
        Err(erro) => (Vec::new(), Some(erro.to_string())),
    };

    // A conversa aberta e as suas mensagens, quando há uma.
    let (aberta, mensagens) = match conversation {
        None => (None, Vec::new()),
        Some(id) => {
            let detalhe = optional(
                state,
                member,
                &format!("/api/v1/messaging/conversations/{id}"),
            )
            .await;
            let historico = optional(
                state,
                member,
                &format!("/api/v1/messaging/conversations/{id}/messages"),
            )
            .await;
            // O Core devolve da mais recente para trás; o fluxo lê-se ao
            // contrário.
            let mut mensagens: Vec<Value> = historico.as_array().cloned().unwrap_or_default();
            mensagens.reverse();
            ((!detalhe.is_null()).then_some(detalhe), mensagens)
        }
    };

    // A assistência só aparece se houver quem a sirva. Um botão que promete
    // melhorar um texto e falha depois é pior do que não existir.
    // A prontidão vem do `/ready`, que é onde ela vive — e não de um pedido de
    // domínio que por acaso falha quando a capacidade não existe.
    let prontidao = api::core_ready(state).await.unwrap_or(Value::Null);
    let disponivel = |componente: &str| {
        prontidao
            .get("components")
            .and_then(Value::as_array)
            .is_some_and(|todos| {
                todos.iter().any(|c| {
                    c.get("component").and_then(Value::as_str) == Some(componente)
                        && c.get("state").and_then(Value::as_str) == Some("available")
                })
            })
    };

    let ai = viewer.can(ocinye_contracts::Permission::MessagingAiUse) && disponivel("intelligence");
    let realtime = disponivel("realtime");

    // Quem está a olhar. O identificador vem do Core, e não da sessão local:
    // é ele que decide quem é o principal.
    let eu = optional(state, member, "/api/v1/me").await;
    let me = quem_sou(&eu);

    let pagina = ui::screens::messaging::messaging(&ui::screens::messaging::MessagingPage {
        conversations: &lista,
        open: aberta.as_ref(),
        messages: &mensagens,
        me,
        zona: member.zona,
        ai,
        realtime,
        failure,
    });

    let trilho = vec![Crumb::to(Screen::Messaging)];
    shell_page("Mensagens", &viewer, Screen::Messaging, trilho, pagina)
}

#[derive(Deserialize)]
struct StartForm {
    #[serde(default)]
    with: Option<Uuid>,
    #[serde(default)]
    name: Option<String>,
    /// Identificadores separados por vírgula, como o formulário os envia.
    #[serde(default)]
    members: String,
}

/// Começa uma conversa — directa ou de grupo — e abre-a.
async fn messaging_start(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<StartForm>,
) -> Response {
    let member = member_or_login!(state, headers);

    let membros: Vec<Uuid> = form
        .members
        .split(',')
        .filter_map(|parte| Uuid::parse_str(parte.trim()).ok())
        .collect();

    let corpo = serde_json::json!({
        "with": form.with,
        "name": form.name.as_deref().map(str::trim).filter(|n| !n.is_empty()),
        "members": membros,
    });

    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        "/api/v1/messaging/conversations",
        &corpo,
    )
    .await
    {
        Ok(valor) => {
            let id = valor.get("id").and_then(Value::as_str).unwrap_or_default();
            Redirect::to(&format!("/messages/{id}")).into_response()
        }
        Err(failure) => failure_response(&failure),
    }
}

#[derive(Deserialize)]
struct ProcuraDePessoas {
    #[serde(default)]
    q: String,
}

/// Procura pessoas da instituição para começar uma conversa.
///
/// # Porque filtra no servidor
///
/// Porque uma instituição não cabe num `select`, e carregá-la inteira para
/// filtrar no browser seria mandar a lista de toda a gente para cada pessoa que
/// abre as Mensagens. O universo continua a ser o que o Core autoriza — este
/// caminho não alarga nada.
async fn messaging_people(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(procura): Query<ProcuraDePessoas>,
) -> Response {
    let member = member_or_login!(state, headers);

    let termo = procura.q.trim().to_lowercase();
    if termo.chars().count() < 2 {
        // Duas letras é o mínimo. Com uma, a resposta seria metade da
        // instituição, e a lista deixaria de ajudar a escolher.
        return axum::Json(serde_json::json!({ "people": [] })).into_response();
    }

    let pagina = optional(&state, &member, "/api/v1/people?page_size=200").await;
    let eu = eu_id(&state, &member).await;

    let pessoas: Vec<Value> = pagina
        .get("items")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|p| {
            // Nunca a própria: uma conversa consigo mesmo não existe, e o Core
            // recusa-a na mesma.
            p.get("id").and_then(Value::as_str) != Some(&eu.to_string())
                && p.get("status").and_then(Value::as_str) != Some("deactivated")
        })
        .filter(|p| {
            let campo = |nome: &str| {
                p.get(nome)
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_lowercase()
            };
            campo("full_name").contains(&termo)
                || campo("display_name").contains(&termo)
                || campo("email").contains(&termo)
        })
        .take(20)
        .map(|p| {
            serde_json::json!({
                "id": p.get("id"),
                "name": p
                    .get("display_name")
                    .and_then(Value::as_str)
                    .filter(|n| !n.is_empty())
                    .or_else(|| p.get("full_name").and_then(Value::as_str))
                    .unwrap_or_default(),
                "email": p.get("email"),
            })
        })
        .collect();

    axum::Json(serde_json::json!({ "people": pessoas })).into_response()
}

/// Quem se pode pôr num «Para».
///
/// # Porque não reutiliza a rota das Mensagens
///
/// Porque as duas respondem a perguntas diferentes. Mensagens procura **com
/// quem conversar**, e por isso exclui a própria pessoa: uma conversa consigo
/// mesmo não existe. Escrever a si próprio existe, e é normal — um lembrete,
/// um teste de configuração, uma cópia de arquivo.
///
/// Partilhar a rota faria uma das duas mentir. Aqui procura-se por **nome ou
/// endereço institucional**, que desde o [ADR-0106] é a identidade humana;
/// nome de utilizador não existe.
///
/// [ADR-0106]: https://github.com/Ocinye/ocinye-os/blob/main/docs/adrs/0106-email-as-the-single-credential.md
async fn mail_people(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(procura): Query<ProcuraDePessoas>,
) -> Response {
    let member = member_or_login!(state, headers);

    let termo = procura.q.trim().to_lowercase();
    if termo.chars().count() < 2 {
        // Com uma letra a resposta seria metade da instituição, e uma lista
        // dessas não ajuda a escolher.
        return axum::Json(serde_json::json!({ "people": [] })).into_response();
    }

    let pagina = optional(&state, &member, "/api/v1/people?page_size=200").await;

    let pessoas: Vec<Value> = pagina
        .get("items")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|p| p.get("status").and_then(Value::as_str) != Some("deactivated"))
        .filter(|p| {
            let campo = |nome: &str| {
                p.get(nome)
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_lowercase()
            };
            campo("full_name").contains(&termo)
                || campo("display_name").contains(&termo)
                || campo("email").contains(&termo)
        })
        // Sem endereço não há para onde escrever, e oferecê-lo seria oferecer
        // um destinatário que não recebe.
        .filter(|p| {
            p.get("email")
                .and_then(Value::as_str)
                .is_some_and(|e| !e.is_empty())
        })
        .take(20)
        .map(|p| {
            serde_json::json!({
                "name": p
                    .get("display_name")
                    .and_then(Value::as_str)
                    .filter(|n| !n.is_empty())
                    .or_else(|| p.get("full_name").and_then(Value::as_str))
                    .unwrap_or_default(),
                "email": p.get("email"),
            })
        })
        .collect();

    axum::Json(serde_json::json!({ "people": pessoas })).into_response()
}

#[derive(Deserialize)]
struct AssistForm {
    action: String,
    draft: String,
}

/// Pede ao Ocinye para trabalhar um rascunho.
///
/// # Porque devolve JSON e não uma página
///
/// Porque o que volta é uma **proposta**, e o rascunho da pessoa tem de ficar
/// exactamente onde estava. Recarregar a página para mostrar uma sugestão
/// perderia o que ela estava a escrever.
async fn messaging_assist(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<AssistForm>,
) -> Response {
    let member = member_or_login!(state, headers);

    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        "/api/v1/messaging/assist",
        &serde_json::json!({ "action": form.action, "draft": form.draft }),
    )
    .await
    {
        Ok(valor) => axum::Json(valor).into_response(),
        // Uma assistência que falha não perde o que estava escrito, e diz
        // porquê em vez de ficar calada.
        Err(falha) => axum::Json(serde_json::json!({
            "text": null,
            "reason": falha.to_string(),
        }))
        .into_response(),
    }
}

/// Quem está a escrever nesta conversa.
async fn messaging_typing(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(conversation): Path<String>,
) -> Response {
    let member = member_or_login!(state, headers);

    let resposta = optional(
        &state,
        &member,
        &format!("/api/v1/messaging/typing?conversation={conversation}"),
    )
    .await;

    axum::Json(resposta).into_response()
}

#[derive(Deserialize)]
struct SendForm {
    #[serde(default)]
    body: String,
    #[serde(default)]
    reply_to: Option<Uuid>,
    /// Identificadores separados por vírgula.
    #[serde(default)]
    mentions: String,
    /// A chave que torna o envio idempotente.
    ///
    /// Vem do formulário porque é o cliente que sabe que **este** é o mesmo
    /// envio que já tentou. Um duplo-clique traz a mesma, e o Core devolve a
    /// mensagem que a primeira escreveu.
    #[serde(default)]
    idempotency_key: String,
}

/// Envia uma mensagem.
async fn messaging_send(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(conversation): Path<String>,
    Form(form): Form<SendForm>,
) -> Response {
    let member = member_or_login!(state, headers);

    let mencoes: Vec<Uuid> = form
        .mentions
        .split(',')
        .filter_map(|parte| Uuid::parse_str(parte.trim()).ok())
        .collect();

    // O autor não vai daqui. Vai do principal, no Core.
    let corpo = serde_json::json!({
        "body": form.body,
        "reply_to": form.reply_to,
        "mentions": mencoes,
        "idempotency_key": (!form.idempotency_key.trim().is_empty())
            .then(|| form.idempotency_key.trim()),
    });

    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/messaging/conversations/{conversation}/messages"),
        &corpo,
    )
    .await
    {
        Ok(_) => Redirect::to(&format!("/messages/{conversation}")).into_response(),
        Err(failure) => failure_response(&failure),
    }
}

#[derive(Deserialize)]
struct ReactForm {
    message: Uuid,
    emoji: String,
}

/// Põe ou tira uma reacção.
async fn messaging_react(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(conversation): Path<String>,
    Form(form): Form<ReactForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let caminho = format!(
        "/api/v1/messaging/conversations/{conversation}/messages/{}/reactions",
        form.message
    );

    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &caminho,
        &serde_json::json!({ "emoji": form.emoji }),
    )
    .await
    {
        Ok(_) => Redirect::to(&format!("/messages/{conversation}")).into_response(),
        Err(failure) => failure_response(&failure),
    }
}

#[derive(Deserialize)]
struct ReadForm {
    until: String,
}

/// Marca a conversa como lida até um instante.
async fn messaging_read(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(conversation): Path<String>,
    Form(form): Form<ReadForm>,
) -> Response {
    let member = member_or_login!(state, headers);

    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/messaging/conversations/{conversation}/read"),
        &serde_json::json!({ "until": form.until }),
    )
    .await
    {
        Ok(_) => Redirect::to(&format!("/messages/{conversation}")).into_response(),
        Err(failure) => failure_response(&failure),
    }
}

#[derive(Deserialize)]
struct MemberForm {
    who: Uuid,
}

/// Acrescenta alguém ao grupo.
async fn messaging_add_member(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(conversation): Path<String>,
    Form(form): Form<MemberForm>,
) -> Response {
    let member = member_or_login!(state, headers);

    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/messaging/conversations/{conversation}/members"),
        &serde_json::json!({ "who": form.who }),
    )
    .await
    {
        Ok(_) => Redirect::to(&format!("/messages/{conversation}")).into_response(),
        Err(failure) => failure_response(&failure),
    }
}

/// Retira alguém do grupo.
async fn messaging_remove(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(conversation): Path<String>,
    Form(form): Form<MemberForm>,
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
        Err(failure) => failure_response(&failure),
    }
}

/// Quem está a agir, tal como o Core o identifica.
async fn eu_id(state: &WorkspaceState, member: &Member) -> Uuid {
    quem_sou(&optional(state, member, "/api/v1/me").await)
}

/// Lê o identificador da pessoa da resposta de `/api/v1/me`.
///
/// # O campo chama-se `person_id`
///
/// E isto esteve a ler `id`. O `unwrap_or_default()` devolvia o UUID nulo, que
/// não é ninguém — e como ninguém é o autor de nada, **nenhuma mensagem era
/// própria**. Sem erro, sem aviso: a conversa inteira aparecia alinhada como se
/// fosse de outra pessoa.
///
/// Uma chave errada num JSON não dá erro. É por isso que existe o guarda em
/// `quem_sou_le_o_campo_que_o_core_escreve`, que compara com a forma real.
fn quem_sou(me: &Value) -> Uuid {
    me.get("person_id")
        .and_then(Value::as_str)
        .and_then(|s| Uuid::parse_str(s).ok())
        .unwrap_or_default()
}

/// Sai do grupo.
async fn messaging_leave(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(conversation): Path<String>,
) -> Response {
    let member = member_or_login!(state, headers);

    match api::delete(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!(
            "/api/v1/messaging/conversations/{conversation}/members/{}",
            eu_id(&state, &member).await
        ),
    )
    .await
    {
        // Depois de sair, a conversa deixa de existir para quem saiu.
        Ok(_) => Redirect::to(ui::screens::messaging::ROUTE).into_response(),
        Err(failure) => failure_response(&failure),
    }
}

#[derive(Deserialize)]
struct MailSettingsForm {
    #[serde(default)]
    signature: String,
    #[serde(default)]
    remote_content_policy: String,
}

async fn save_mail_settings(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<MailSettingsForm>,
) -> Response {
    let member = member_or_login!(state, headers);

    let body = serde_json::json!({
        "signature": form.signature,
        "remote_content_policy": form.remote_content_policy,
    });

    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        "/api/v1/mail/preferences",
        &body,
    )
    .await
    {
        Ok(_) => Redirect::to("/mail/settings").into_response(),
        Err(failure) => failure_response(&failure),
    }
}

/// O que o formulário de ligação envia.
#[derive(serde::Deserialize)]
struct LigacaoDeCaixa {
    /// A senha **da caixa**, no serviço de correio.
    ///
    /// Não é a credencial do Ocinye: essa é o endereço institucional
    /// (ADR-0106), com a palavra-passe do Ocinye OS. Esta é outra, e nenhuma
    /// serve para obter a outra.
    ///
    /// Havia aqui um `username` ao lado, com o argumento de que nem todos os
    /// serviços usam o endereço como conta. É verdade em geral e não é verdade
    /// aqui — e o custo de o manter era deixar o browser escolher a conta com
    /// que o Ocinye se autentica. O Core resolve-a a partir da caixa que já
    /// autorizou (ADR-0409). Se um dia houver um serviço que peça outra coisa,
    /// isso é uma decisão de instalação e não um campo no ecrã de quem liga.
    password: String,
}

/// Liga uma caixa com a credencial de quem a está a ligar.
///
/// # Porque a senha não volta a passar por aqui
///
/// Atravessa este processo uma vez, a caminho do Core, e nunca regressa: o
/// formulário abre sempre vazio, e não há endpoint que a devolva. O que a
/// Experience sabe de uma caixa ligada é que está ligada.
async fn mail_connect(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(mailbox_id): Path<String>,
    Form(form): Form<LigacaoDeCaixa>,
) -> Response {
    let member = member_or_login!(state, headers);

    // Só a senha. O endereço não viaja: o Core resolve-o a partir da caixa que
    // já autorizou para esta pessoa, e um endereço vindo do formulário deixaria
    // o browser escolher a conta com que a sessão de correio abre.
    let body = serde_json::json!({
        "password": form.password,
    });

    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/mail/mailboxes/{mailbox_id}/connect"),
        &body,
    )
    .await
    {
        Ok(_) => Redirect::to("/mail/settings").into_response(),
        Err(failure) => failure_response(&failure),
    }
}

/// Desliga uma caixa e esquece a credencial.
async fn mail_disconnect(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(mailbox_id): Path<String>,
) -> Response {
    let member = member_or_login!(state, headers);

    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/mail/mailboxes/{mailbox_id}/disconnect"),
        &serde_json::json!({}),
    )
    .await
    {
        Ok(_) => Redirect::to("/mail/settings").into_response(),
        Err(failure) => failure_response(&failure),
    }
}

// ── Listas ───────────────────────────────────────────────────────────────

/// Renderiza um ecrã de lista a partir de um endpoint do Core.
macro_rules! list_route {
    ($name:ident, $screen:expr, $title:expr, $path:expr, $render:path) => {
        async fn $name(
            State(state): State<WorkspaceState>,
            headers: HeaderMap,
            Query(slice): Query<ListSlice>,
        ) -> Response {
            let member = member_or_login!(state, headers);
            let viewer = viewer(&state, &member).await;
            let path = com_pagina($path, slice.page);
            // O conteúdo principal do ecrã: uma recusa é mostrada como recusa,
            // e não como lista vazia.
            let payload = match required(&state, &member, &path).await {
                Ok(payload) => payload,
                Err(failure) => return failure_response(&failure),
            };
            shell_page(
                $title,
                &viewer,
                $screen,
                Vec::new(),
                $render(&viewer, &payload),
            )
        }
    };
}

list_route!(
    units,
    Screen::Units,
    "Unidades",
    "/api/v1/units",
    ui::screens::lists::units
);
/// As unidades que o membro pode usar como recorte desta consulta.
///
/// # Porque é uma intersecção
///
/// `/api/v1/me` diz a que unidades o membro **pertence**, sem nomes.
/// `/api/v1/units` traz os nomes das unidades que ele **pode ler**, já filtradas
/// pela política. Nenhuma das duas listas sozinha é a resposta certa:
///
/// - só as memberships dariam identificadores sem nome, e um selector de UUIDs
///   não é um selector;
/// - só a lista institucional daria unidades a que o membro não pertence, e
///   «Da Unidade» passaria a significar «de qualquer unidade».
///
/// O Core continua a ser a autoridade: um `unit_id` escrito à mão no URL não
/// ganha nada por estar aqui, porque é lá que a consulta é decidida.
async fn eligible_units(state: &WorkspaceState, member: &Member) -> Vec<(String, String)> {
    let (me, unidades) = tokio::join!(
        optional(state, member, "/api/v1/me"),
        optional(state, member, "/api/v1/units"),
    );

    let minhas: std::collections::HashSet<String> = me
        .get("units")
        .and_then(Value::as_array)
        .map(|itens| {
            itens
                .iter()
                .filter_map(|m| m.get("id").and_then(Value::as_str))
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();

    unidades
        .get("items")
        .and_then(Value::as_array)
        .map(|itens| {
            itens
                .iter()
                .filter_map(|u| {
                    let id = u.get("id").and_then(Value::as_str)?;
                    if !minhas.contains(id) {
                        return None;
                    }
                    let nome = u
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or_else(|| u.get("code").and_then(Value::as_str).unwrap_or(id));
                    Some((id.to_owned(), nome.to_owned()))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// O recorte e a página pedidos numa lista.
///
/// A página vive no URL e não em estado de sessão: um endereço de segunda
/// página tem de continuar a ser a segunda página quando alguém o guarda,
/// partilha ou recarrega.
#[derive(Debug, Default, Deserialize)]
struct ListSlice {
    /// Apenas aqueles em que o membro participa.
    #[serde(default)]
    mine: bool,
    /// A página pedida, 1-based.
    #[serde(default)]
    page: Option<u32>,
    /// A unidade escolhida como recorte.
    ///
    /// Nunca inferida. Quando o membro pertence a várias, é ele que escolhe —
    /// o Ocinye OS não tem conceito de «unidade principal», e escolher a
    /// primeira, a mais antiga ou a alfabeticamente primeira seria inventar um.
    #[serde(default)]
    unit_id: Option<Uuid>,
    /// Se o recorte por unidade foi pedido sem ainda haver escolha.
    #[serde(default)]
    unit: bool,
}

/// Acrescenta a página a um caminho do Core, se houver.
///
/// O Core normaliza o que receber — `page=0` vira 1, um tamanho absurdo é
/// limitado — porque recusar um parâmetro malformado transformá-lo-ia num
/// vector de negação de serviço contra a base de dados.
fn com_pagina(base: &str, page: Option<u32>) -> String {
    match page {
        Some(n) if n > 1 => {
            // `/api/v1/units` não tem query e `/api/v1/sources?page_size=50`
            // tem. Colar `&page=2` ao primeiro daria um caminho malformado, e
            // bastava alguém escrever `?page=2` num ecrã não paginado para o
            // provocar.
            let junta = if base.contains('?') { '&' } else { '?' };
            format!("{base}{junta}page={n}")
        }
        _ => base.to_owned(),
    }
}

/// A consulta que produz um recorte de workspaces.
///
/// Separada da macro para poder ser verificada: dentro dela, o único modo de
/// provar que `mine=true` chega ao Core seria simular o Core.
///
/// O recorte é do Core, e não daqui. O Workspace não filtra a lista que
/// recebeu — pedir «as minhas» e depois esconder as outras no browser seria
/// mandar ao cliente exactamente o que ele não devia ter.
fn workspace_list_path(kind: &str, slice: &ListSlice) -> String {
    let base = format!("/api/v1/workspaces?kind={kind}&page_size=50");
    let mut path = base;
    if slice.mine {
        path.push_str("&mine=true");
    }
    // A unidade viaja tipada até ao Core, que decide se o membro a pode usar.
    // Aqui é só um parâmetro; a autoridade está do outro lado.
    if let Some(unit_id) = slice.unit_id {
        path.push_str(&format!("&unit_id={unit_id}"));
    }
    path
}

/// Ideias e Projectos partilham o ecrã, a consulta e os recortes.
///
/// `mine=true` chega ao Core como `mine=true`, e é lá que significa alguma
/// coisa: participação efectiva conjugada com o `VisibilityFilter`. O Workspace
/// não filtra nada — passa o pedido e mostra o que voltou.
macro_rules! workspace_list {
    ($name:ident, $screen:expr, $title:expr, $kind:expr, $render:path) => {
        async fn $name(
            State(state): State<WorkspaceState>,
            headers: HeaderMap,
            Query(slice): Query<ListSlice>,
        ) -> Response {
            let member = member_or_login!(state, headers);
            let viewer = viewer(&state, &member).await;
            // O selector só é montado quando o recorte por unidade está em
            // jogo: uma chamada a mais em todas as páginas para um controlo que
            // a maioria delas não mostra seria trabalho por nada.
            let unidades = if slice.unit || slice.unit_id.is_some() {
                eligible_units(&state, &member).await
            } else {
                Vec::new()
            };

            // Uma unidade elegível escolhe-se sozinha: não há ambiguidade para
            // resolver, e obrigar a escolher entre uma opção é cerimónia.
            let mut slice = slice;
            if slice.unit && slice.unit_id.is_none() && unidades.len() == 1 {
                slice.unit_id = unidades[0].0.parse().ok();
            }

            // Pedida a unidade e havendo várias, a escolha é do membro. A lista
            // não é filtrada por uma unidade inventada, nem mostrada inteira
            // como se o recorte tivesse sido aplicado.
            let escolha_pendente = slice.unit && slice.unit_id.is_none();

            let payload = if escolha_pendente {
                Value::Null
            } else {
                let path = com_pagina(&workspace_list_path($kind, &slice), slice.page);
                match required(&state, &member, &path).await {
                    Ok(payload) => payload,
                    Err(failure) => return failure_response(&failure),
                }
            };

            shell_page(
                $title,
                &viewer,
                $screen,
                Vec::new(),
                $render(
                    &viewer,
                    &payload,
                    ui::screens::lists::Slice {
                        mine: slice.mine,
                        unit_id: slice.unit_id.map(|id| id.to_string()),
                        units: unidades,
                        awaiting_unit: escolha_pendente,
                    },
                ),
            )
        }
    };
}

workspace_list!(
    ideas,
    Screen::Ideas,
    "Ideias",
    "idea",
    ui::screens::lists::ideas
);
workspace_list!(
    projects,
    Screen::Projects,
    "Projectos",
    "project",
    ui::screens::lists::projects
);
list_route!(
    datasets,
    Screen::Datasets,
    "Dados",
    "/api/v1/datasets?page_size=50",
    ui::screens::lists::datasets
);
list_route!(
    agents,
    Screen::Agents,
    "Agentes",
    "/api/v1/ai/agents",
    ui::screens::lists::agents
);
list_route!(
    admin,
    Screen::Admin,
    "Administração",
    "/api/v1/people?page_size=50",
    ui::screens::lists::members
);
list_route!(
    audit,
    Screen::Audit,
    "Audit Log",
    "/api/v1/audit?page_size=50",
    ui::screens::lists::audit
);

// ── Administração de membros ─────────────────────────────────────────────

/// `GET /admin/members/new` — formulário de criação.
async fn new_member(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    let member = member_or_login!(state, headers);
    let viewer = viewer(&state, &member).await;
    let units = optional(&state, &member, "/api/v1/units").await;

    shell_page(
        "Adicionar utilizador",
        &viewer,
        Screen::Admin,
        vec![Crumb::to(Screen::Admin)],
        ui::screens::administration::new_member(&units, None),
    )
}

/// Campos do formulário de criação.
#[derive(Deserialize)]
struct NewMemberForm {
    #[serde(default)]
    full_name: String,
    #[serde(default)]
    email: String,
    #[serde(default)]
    position: String,
    #[serde(default)]
    role: String,
    #[serde(default)]
    unit_id: String,
}

/// `POST /admin/members/new` — cria e mostra a credencial, uma única vez.
async fn create_member(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<NewMemberForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let viewer = viewer(&state, &member).await;

    let mut body = serde_json::json!({
        "full_name": form.full_name,
        "email": form.email,
        "role": form.role,
    });
    // Campos opcionais só viajam quando têm valor: enviar `""` faria o Core
    // rejeitar uma posição vazia como posição desconhecida.
    if !form.position.is_empty() {
        body["position"] = Value::String(form.position);
    }
    if !form.unit_id.is_empty() {
        body["unit_id"] = Value::String(form.unit_id);
    }

    let outcome = api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        "/api/v1/administration/members",
        &body,
    )
    .await;

    match outcome {
        Ok(created) => {
            let credential = created.get("credential").cloned().unwrap_or(Value::Null);
            shell_page(
                "Utilizador criado",
                &viewer,
                Screen::Admin,
                vec![Crumb::to(Screen::Admin)],
                ui::screens::administration::issued_credential(
                    // O Core devolve `email`. Lia-se `username`, e desde o
                    // ADR-0106 essa chave não existe: o ecrã que entrega uma
                    // credencial nova mostrava o endereço **em branco**, e
                    // ninguém o via porque um campo vazio parece um campo.
                    credential
                        .get("email")
                        .and_then(Value::as_str)
                        .unwrap_or(""),
                    credential
                        .get("temporary_password")
                        .and_then(Value::as_str)
                        .unwrap_or(""),
                    credential
                        .get("expires_at")
                        .and_then(Value::as_str)
                        .unwrap_or(""),
                ),
            )
        }
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        Err(failure) => {
            let units = optional(&state, &member, "/api/v1/units").await;
            shell_page(
                "Adicionar utilizador",
                &viewer,
                Screen::Admin,
                vec![Crumb::to(Screen::Admin)],
                ui::screens::administration::new_member(&units, Some(failure.to_string())),
            )
        }
    }
}

/// `GET /admin/members/{id}` — detalhe: acesso e segurança.
async fn member_detail(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(person_id): Path<String>,
) -> Response {
    let member = member_or_login!(state, headers);
    let viewer = viewer(&state, &member).await;

    let person_path = format!("/api/v1/people/{person_id}");
    let security_path = format!("/api/v1/administration/members/{person_id}/security");
    let access_path = format!("/api/v1/administration/members/{person_id}/access");

    let (person, security, access) = tokio::join!(
        optional(&state, &member, &person_path),
        optional(&state, &member, &security_path),
        optional(&state, &member, &access_path),
    );

    if person.is_null() {
        return failure_response(&ApiFailure::Denied);
    }

    shell_page(
        "Membro",
        &viewer,
        Screen::Admin,
        vec![Crumb::to(Screen::Admin)],
        ui::screens::administration::member_detail(&person, &security, &access),
    )
}

/// Bibliografia.
///
/// O Core expõe fontes por Research Workspace; sem um workspace escolhido, a
/// lista aparece vazia com a explicação em vez de um erro.
/// A bibliografia institucional.
///
/// Este ecrã passava `Value::Null` ao componente e nunca chamava o Core: a
/// tabela renderizava sempre vazia, e nada dizia porquê. Um ecrã vazio não
/// prova que não há dados — pode provar apenas que ninguém o ligou.
///
/// Usa `required` e não `optional` de propósito: uma falha do Core é mostrada
/// como falha. Com `optional`, um erro voltava como `null` e o ecrã dizia «não
/// há bibliografia» quando o que houve foi uma consulta falhada.
async fn bibliography(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    let member = member_or_login!(state, headers);
    let viewer = viewer(&state, &member).await;
    let payload = match required(&state, &member, "/api/v1/sources?page_size=50").await {
        Ok(payload) => payload,
        Err(failure) => return failure_response(&failure),
    };

    shell_page(
        "Bibliografia",
        &viewer,
        Screen::Bibliography,
        Vec::new(),
        ui::screens::lists::bibliography(&viewer, &payload),
    )
}

/// Ferramentas bibliográficas, em branco.
async fn bibliography_tools(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    let member = member_or_login!(state, headers);
    let viewer = viewer(&state, &member).await;
    let destinos = creation_destinations(&state, &member).await;
    let trail = vec![Crumb::to(Screen::Bibliography)];

    shell_page(
        "Ferramentas bibliográficas",
        &viewer,
        Screen::Bibliography,
        trail,
        ui::screens::lists::bibliography_tools(&destinos, "", None, None),
    )
}

/// O que o formulário das ferramentas envia.
#[derive(Deserialize)]
struct BibliographyToolsForm {
    workspace_id: Uuid,
    #[serde(default)]
    bibtex: String,
}

/// Pede ao Core que reveja a bibliografia, e mostra o que ele respondeu.
///
/// # Porque a Experience não conhece o Capability Runtime
///
/// Porque pede uma operação de domínio. Que a leitura aconteça dentro de um
/// isolamento WebAssembly é decisão do Core, e a Experience não tem — nem deve
/// ter — como saber qual componente corre.
async fn review_bibliography(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<BibliographyToolsForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let viewer = viewer(&state, &member).await;
    let destinos = creation_destinations(&state, &member).await;
    let trail = vec![Crumb::to(Screen::Bibliography)];

    // Recusa antes de gastar um pedido ao Core. O limite canónico é o do
    // contrato, e é o Core que o aplica; isto poupa a viagem.
    if form.bibtex.len() > ocinye_contracts::bibliography::MAX_BIBTEX_BYTES {
        return shell_page(
            "Ferramentas bibliográficas",
            &viewer,
            Screen::Bibliography,
            trail,
            ui::screens::lists::bibliography_tools(
                &destinos,
                "",
                None,
                Some("A bibliografia é demasiado extensa para ser revista de uma vez.".to_owned()),
            ),
        );
    }

    let caminho = format!(
        "/api/v1/workspaces/{}/bibliography/review",
        form.workspace_id
    );
    let corpo = serde_json::json!({ "bibtex": form.bibtex });

    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &caminho,
        &corpo,
    )
    .await
    {
        Ok(payload) => {
            let revisao: Option<ocinye_contracts::bibliography::BibliographyReview> =
                serde_json::from_value(payload).ok();
            let erro = revisao
                .is_none()
                .then(|| "Não foi possível ler o resultado da revisão.".to_owned());

            shell_page(
                "Ferramentas bibliográficas",
                &viewer,
                Screen::Bibliography,
                trail,
                ui::screens::lists::bibliography_tools(
                    &destinos,
                    &form.bibtex,
                    revisao.as_ref(),
                    erro,
                ),
            )
        }
        Err(failure) => shell_page(
            "Ferramentas bibliográficas",
            &viewer,
            Screen::Bibliography,
            trail,
            ui::screens::lists::bibliography_tools(
                &destinos,
                &form.bibtex,
                None,
                Some(failure.to_string()),
            ),
        ),
    }
}

// ── Investigação ─────────────────────────────────────────────────────────

async fn unit_detail(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(unit_id): Path<Uuid>,
    Query(query): Query<FilesQuery>,
) -> Response {
    let member = member_or_login!(state, headers);

    let unit = match api::get::<Value>(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/units/{unit_id}"),
    )
    .await
    {
        Ok(unit) => unit,
        Err(failure) => return failure_response(&failure),
    };

    let viewer = viewer(&state, &member).await;
    let members_path = format!("/api/v1/units/{unit_id}/members");
    let workspaces_path = format!("/api/v1/workspaces?unit_id={unit_id}&page_size=50");

    let (members, workspaces) = tokio::join!(
        optional(&state, &member, &members_path),
        optional(&state, &member, &workspaces_path),
    );

    // Quem já pertence, para não o oferecer outra vez na lista de escolha.
    let ja_pertencem: std::collections::HashSet<String> = members
        .as_array()
        .map(|linhas| {
            linhas
                .iter()
                .filter_map(|m| m.get("person_id").and_then(Value::as_str))
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();

    let pessoas = optional(&state, &member, "/api/v1/people?page_size=200").await;
    let candidatos: Vec<(String, String)> = pessoas
        .get("items")
        .and_then(Value::as_array)
        .map(|linhas| {
            linhas
                .iter()
                .filter_map(|p| {
                    let id = p.get("id").and_then(Value::as_str)?;
                    if ja_pertencem.contains(id) {
                        return None;
                    }
                    let nome = p.get("full_name").and_then(Value::as_str).unwrap_or("—");
                    let email = p.get("email").and_then(Value::as_str).unwrap_or("");
                    Some((id.to_owned(), format!("{nome} · {email}")))
                })
                .collect()
        })
        .unwrap_or_default();

    let gestao = ui::screens::workspaces::GestaoDePessoas {
        // Do Core, e não de um palpite sobre o papel de quem está a ver.
        pode_gerir: unit
            .get("may_manage_members")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        candidatos,
        aviso: aviso_de_pertenca(query.ok.as_deref(), query.erro.as_deref()),
    };

    let trail = vec![Crumb::to(Screen::Units)];
    let content = ui::screens::workspaces::unit_detail(&unit, &members, &workspaces, &gestao);

    shell_page("Detalhe da Unidade", &viewer, Screen::Units, trail, content)
}

/// Uma ideia abre o seu Research Workspace.
async fn idea_workspace(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(idea_id): Path<Uuid>,
) -> Response {
    let member = member_or_login!(state, headers);

    match api::get::<Value>(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/ideas/{idea_id}"),
    )
    .await
    {
        Ok(payload) => {
            let workspace_id = payload
                .get("workspace")
                .and_then(|w| w.get("id"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            Redirect::to(&format!("/workspaces/{workspace_id}")).into_response()
        }
        Err(failure) => failure_response(&failure),
    }
}

/// Um projecto abre o seu Research Workspace.
async fn project_workspace(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(project_id): Path<Uuid>,
) -> Response {
    let member = member_or_login!(state, headers);

    match api::get::<Value>(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/projects/{project_id}"),
    )
    .await
    {
        Ok(payload) => {
            let workspace_id = payload
                .get("workspace_id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            Redirect::to(&format!("/workspaces/{workspace_id}")).into_response()
        }
        Err(failure) => failure_response(&failure),
    }
}

/// O Research Workspace.
async fn research_workspace(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(workspace_id): Path<Uuid>,
    Query(aviso): Query<AvisoQuery>,
) -> Response {
    let member = member_or_login!(state, headers);

    // A visão geral é a leitura que autoriza: se falhar, nada mais é pedido.
    let overview = match api::get::<Value>(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/workspaces/{workspace_id}"),
    )
    .await
    {
        Ok(overview) => overview,
        Err(failure) => return failure_response(&failure),
    };

    let viewer = viewer(&state, &member).await;

    let sources_path = format!("/api/v1/workspaces/{workspace_id}/sources");
    let notes_path = format!("/api/v1/workspaces/{workspace_id}/notes");
    let documents_path = format!("/api/v1/workspaces/{workspace_id}/documents");
    let datasets_path = format!("/api/v1/datasets?workspace_id={workspace_id}");
    let tasks_path = format!("/api/v1/tasks?workspace_id={workspace_id}");
    let activity_path = format!("/api/v1/activity?workspace_id={workspace_id}");

    let (sources, notes, documents, datasets, tasks, activity, ai) = tokio::join!(
        optional(&state, &member, &sources_path),
        optional(&state, &member, &notes_path),
        optional(&state, &member, &documents_path),
        optional(&state, &member, &datasets_path),
        optional(&state, &member, &tasks_path),
        optional(&state, &member, &activity_path),
        // A disponibilidade vem do Core. A interface não a infere, e com o
        // Core em silêncio assume indisponível — que é o estado honesto.
        optional(&state, &member, "/api/v1/ai/status"),
    );

    // O mesmo ecrã serve ideias e projectos, e o trilho segue o que o
    // workspace é — não o caminho por onde se lá chegou.
    let is_project = overview.get("project").is_some_and(|p| !p.is_null());
    let screen = if is_project {
        Screen::Projects
    } else {
        Screen::Ideas
    };

    // Quem já participa, para não voltar a ser oferecido.
    let ja_participam: std::collections::HashSet<String> = overview
        .get("members")
        .and_then(Value::as_array)
        .map(|linhas| {
            linhas
                .iter()
                .filter_map(|m| m.get("person_id").and_then(Value::as_str))
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();

    let pode_gerir = overview
        .get("workspace")
        .and_then(|w| w.get("may_manage_members"))
        .and_then(Value::as_bool)
        .unwrap_or(false);

    // A lista de candidatos só se pede a quem pode usá-la. Pedi-la sempre seria
    // ler a organização inteira para a deitar fora em todos os ecrãs.
    let candidatos: Vec<(String, String)> = if pode_gerir {
        let pessoas = optional(&state, &member, "/api/v1/people?page_size=200").await;
        pessoas
            .get("items")
            .and_then(Value::as_array)
            .map(|linhas| {
                linhas
                    .iter()
                    .filter_map(|p| {
                        let pid = p.get("id").and_then(Value::as_str)?;
                        if ja_participam.contains(pid) {
                            return None;
                        }
                        let nome = p.get("full_name").and_then(Value::as_str).unwrap_or("—");
                        let email = p.get("email").and_then(Value::as_str).unwrap_or("");
                        Some((pid.to_owned(), format!("{nome} · {email}")))
                    })
                    .collect()
            })
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    let gestao = ui::screens::workspaces::GestaoDePessoas {
        // Do Core, e não de um palpite sobre o papel de quem está a ver.
        pode_gerir,
        candidatos,
        aviso: aviso_de_participacao(aviso.ok.as_deref(), aviso.erro.as_deref()),
    };

    let trail = vec![Crumb::to(screen)];

    let content =
        ui::screens::workspaces::research_workspace(ui::screens::workspaces::WorkspaceView {
            overview,
            sources,
            notes,
            documents,
            datasets,
            tasks,
            activity,
            inference_available: inference_available(&ai),
            may_use_assistance: viewer.can(ocinye_contracts::Permission::AiUse),
            gestao,
        });

    shell_page("Research Workspace", &viewer, screen, trail, content)
}

// ── Ciência ──────────────────────────────────────────────────────────────

/// A cadeia científica de um Research Workspace.
async fn scientific_chain(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(workspace_id): Path<Uuid>,
) -> Response {
    let member = member_or_login!(state, headers);

    // A visão geral é a leitura que autoriza. Se o ambiente não é alcançável,
    // nada mais é pedido — e a resposta é a mesma que daria a um identificador
    // inventado.
    let overview = match api::get::<Value>(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/workspaces/{workspace_id}"),
    )
    .await
    {
        Ok(overview) => overview,
        Err(failure) => return failure_response(&failure),
    };

    let viewer = viewer(&state, &member).await;

    let hypotheses_path = format!("/api/v1/workspaces/{workspace_id}/hypotheses");
    let methodologies_path = format!("/api/v1/workspaces/{workspace_id}/methodologies");
    let studies_path = format!("/api/v1/workspaces/{workspace_id}/studies");
    let results_path = format!("/api/v1/workspaces/{workspace_id}/results");

    let (hypotheses, methodologies, studies, results) = tokio::join!(
        optional(&state, &member, &hypotheses_path),
        optional(&state, &member, &methodologies_path),
        optional(&state, &member, &studies_path),
        optional(&state, &member, &results_path),
    );

    let is_project = overview.get("project").is_some_and(|p| !p.is_null());
    let screen = if is_project {
        Screen::Projects
    } else {
        Screen::Ideas
    };
    let trail = vec![
        Crumb::to(screen),
        Crumb {
            label: "Research Workspace".to_owned(),
            href: format!("/workspaces/{workspace_id}"),
        },
    ];

    // Quem decide é o Core, e para este ambiente.
    //
    // `viewer.can` responde no âmbito institucional, e `science.create` chega
    // pela pertença à unidade e ao ambiente — nunca por papel técnico. Usá-lo
    // aqui escondia a criação a toda a gente, incluindo a quem lidera o
    // ambiente. Esconder o botão nunca foi segurança; a operação recusa na
    // mesma. É para não prometer o que não se cumpre.
    let pode_criar = overview
        .get("workspace")
        .and_then(|w| w.get("may_create"))
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let content = ui::screens::science::scientific_chain(ui::screens::science::ChainView {
        overview,
        hypotheses,
        methodologies,
        studies,
        results,
        may_create: pode_criar,
    });

    shell_page("Ciência", &viewer, screen, trail, content)
}

/// De onde veio um resultado, e o que dependeu dele.
async fn result_detail(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(result_id): Path<Uuid>,
    Query(query): Query<LineageQuery>,
) -> Response {
    let member = member_or_login!(state, headers);

    let result = match api::get::<Value>(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/results/{result_id}"),
    )
    .await
    {
        Ok(result) => result,
        Err(failure) => return failure_response(&failure),
    };

    let viewer = viewer(&state, &member).await;

    // As duas travessias são pedidas sempre, e não só a que se mostra: as tabs
    // trocam de sentido sem ir buscar nada, e uma delas vazia é informação
    // — «nada depende disto» — que se quer ver de imediato.
    let validations_path = format!("/api/v1/results/{result_id}/validations");
    let upstream_path = format!("/api/v1/lineage/result/{result_id}?direction=upstream");
    let downstream_path = format!("/api/v1/lineage/result/{result_id}?direction=downstream");

    let (validations, upstream, downstream) = tokio::join!(
        optional(&state, &member, &validations_path),
        optional(&state, &member, &upstream_path),
        optional(&state, &member, &downstream_path),
    );

    let direction = if query.direction.as_deref() == Some("downstream") {
        "downstream"
    } else {
        "upstream"
    };

    let workspace_id = result
        .get("workspace_id")
        .and_then(Value::as_str)
        .map(str::to_owned);

    let mut trail = vec![Crumb::to(Screen::Ideas)];
    if let Some(workspace_id) = workspace_id {
        trail.push(Crumb {
            label: "Ciência".to_owned(),
            href: format!("/workspaces/{workspace_id}/science"),
        });
    }

    let result_may_validate = result
        .get("may_validate")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let content = ui::screens::science::result_detail(ui::screens::science::ResultView {
        result,
        validations,
        upstream,
        downstream,
        direction,
        // Vem do Core, com o contexto deste resultado: `results.validate`
        // chega pela liderança do ambiente ou pela gestão da unidade, e as
        // capacidades que o `/identity/me` publica são as institucionais,
        // onde uma permissão de ambiente nunca aparece.
        may_validate: result_may_validate,
    });

    shell_page("Resultado", &viewer, Screen::Ideas, trail, content)
}

// ── Construir a cadeia ───────────────────────────────────────────────────

/// O ambiente, resolvido pelo Core, que autoriza tudo o que se segue.
///
/// Se ele não é alcançável, nada mais é pedido — e a resposta é a mesma que
/// daria a um identificador inventado.
async fn ambiente_ou_recusa(
    state: &WorkspaceState,
    member: &Member,
    workspace_id: Uuid,
) -> Result<Value, api::ApiFailure> {
    let overview = api::get::<Value>(
        state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/workspaces/{workspace_id}"),
    )
    .await?;
    Ok(overview.get("workspace").cloned().unwrap_or(Value::Null))
}

/// Se este membro pode criar no ambiente que contém aquele recurso.
///
/// A resposta é do Core, e para aquele ambiente. `viewer.can` responde no
/// âmbito institucional, e `science.create` chega pela pertença à unidade e ao
/// ambiente — nunca por papel técnico. Perguntá-lo ao viewer escondia a criação
/// a toda a gente, incluindo a quem lidera o ambiente.
///
/// Sem ambiente conhecido, ou com o Core em silêncio, a resposta é não: é a
/// única conservadora, e não prometer é melhor do que prometer uma recusa.
async fn pode_criar_no_ambiente(state: &WorkspaceState, member: &Member, recurso: &Value) -> bool {
    let Some(workspace_id) = recurso.get("workspace_id").and_then(Value::as_str) else {
        return false;
    };
    optional(state, member, &format!("/api/v1/workspaces/{workspace_id}"))
        .await
        .get("workspace")
        .and_then(|w| w.get("may_create"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

/// A recusa que volta ao formulário, e a que não volta.
///
/// Só o que a pessoa pode resolver preenchendo outra vez: o conteúdo que o
/// Core não aceitou, ou a autoridade que lhe falta. Uma avaria, uma sessão
/// caída ou um recurso inalcançável não se corrigem no campo.
fn motivo_para_o_formulario(failure: &api::ApiFailure) -> Option<String> {
    match failure {
        api::ApiFailure::Rejected(mensagem) => Some(mensagem.clone()),
        api::ApiFailure::Forbidden => {
            Some("Não tem autorização para criar isto neste ambiente.".to_owned())
        }
        _ => None,
    }
}

/// As versões de metodologia publicadas num ambiente, prontas para um selector.
///
/// **Versões**, e nunca metodologias: a matriz de proveniência aceita
/// `Study → MethodologyVersion` e recusa a metodologia mutável. Oferecer a
/// metodologia poria no ecrã uma escolha que o Core recusa, e deixaria o `422`
/// ensinar a regra a quem já tinha preenchido o resto.
async fn versoes_de_metodologia(
    state: &WorkspaceState,
    member: &Member,
    workspace_id: Uuid,
) -> Vec<(String, String)> {
    let metodologias = optional(
        state,
        member,
        &format!("/api/v1/workspaces/{workspace_id}/methodologies"),
    )
    .await;

    let mut opcoes = Vec::new();
    for metodologia in metodologias.as_array().into_iter().flatten() {
        let Some(id) = metodologia.get("id").and_then(Value::as_str) else {
            continue;
        };
        let titulo = metodologia
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("Metodologia");
        let versoes = optional(
            state,
            member,
            &format!("/api/v1/methodologies/{id}/versions"),
        )
        .await;
        for versao in versoes.as_array().into_iter().flatten() {
            // Só publicadas: uma versão em rascunho ainda não é o que a
            // proveniência pode citar.
            if versao.get("status").and_then(Value::as_str) != Some("published") {
                continue;
            }
            let Some(version_id) = versao.get("id").and_then(Value::as_str) else {
                continue;
            };
            let etiqueta = versao
                .get("label")
                .and_then(Value::as_str)
                .unwrap_or("versão");
            opcoes.push((version_id.to_owned(), format!("{titulo} · {etiqueta}")));
        }
    }
    opcoes
}

/// As versões de dataset alcançáveis a partir de um ambiente.
async fn versoes_de_dataset(
    state: &WorkspaceState,
    member: &Member,
    workspace_id: Uuid,
) -> Vec<(String, String)> {
    let datasets = optional(
        state,
        member,
        &format!("/api/v1/datasets?workspace_id={workspace_id}"),
    )
    .await;

    let linhas = datasets
        .get("items")
        .and_then(Value::as_array)
        .cloned()
        .or_else(|| datasets.as_array().cloned())
        .unwrap_or_default();

    let mut opcoes = Vec::new();
    for dataset in &linhas {
        let Some(id) = dataset.get("id").and_then(Value::as_str) else {
            continue;
        };
        let nome = dataset
            .get("title")
            .or_else(|| dataset.get("name"))
            .and_then(Value::as_str)
            .unwrap_or("Dataset");
        let versoes = optional(state, member, &format!("/api/v1/datasets/{id}/versions")).await;
        let linhas_v = versoes
            .get("items")
            .and_then(Value::as_array)
            .cloned()
            .or_else(|| versoes.as_array().cloned())
            .unwrap_or_default();
        for versao in &linhas_v {
            let Some(version_id) = versao.get("id").and_then(Value::as_str) else {
                continue;
            };
            let etiqueta = versao
                .get("label")
                .and_then(Value::as_str)
                .unwrap_or("versão");
            opcoes.push((version_id.to_owned(), format!("{nome} · {etiqueta}")));
        }
    }
    opcoes
}

// ── Hipótese ─────────────────────────────────────────────────────────────

async fn new_hypothesis(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(workspace_id): Path<Uuid>,
) -> Response {
    let member = member_or_login!(state, headers);
    match pagina_de_hipotese(&state, &member, workspace_id, None).await {
        Ok(resposta) => resposta,
        Err(failure) => failure_response(&failure),
    }
}

async fn pagina_de_hipotese(
    state: &WorkspaceState,
    member: &Member,
    workspace_id: Uuid,
    message: Option<String>,
) -> Result<Response, api::ApiFailure> {
    let workspace = ambiente_ou_recusa(state, member, workspace_id).await?;
    let viewer = viewer(state, member).await;
    Ok(shell_page(
        "Nova hipótese",
        &viewer,
        Screen::Ideas,
        trilho_da_ciencia(workspace_id),
        ui::screens::science::nova_hipotese(ui::screens::science::Contexto { workspace, message }),
    ))
}

#[derive(Deserialize)]
struct NovaHipoteseForm {
    #[serde(default)]
    statement: String,
    #[serde(default)]
    rationale: String,
    #[serde(default)]
    classification: String,
}

async fn create_hypothesis(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(workspace_id): Path<Uuid>,
    Form(form): Form<NovaHipoteseForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let corpo = serde_json::json!({
        "statement": form.statement,
        "rationale": blank_to_none(form.rationale),
        "classification": blank_to_none(form.classification),
    });

    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/workspaces/{workspace_id}/hypotheses"),
        &corpo,
    )
    .await
    {
        Ok(_) => Redirect::to(&format!("/workspaces/{workspace_id}/science")).into_response(),
        Err(failure) => match motivo_para_o_formulario(&failure) {
            Some(motivo) => {
                match pagina_de_hipotese(&state, &member, workspace_id, Some(motivo)).await {
                    Ok(resposta) => resposta,
                    Err(failure) => failure_response(&failure),
                }
            }
            None => failure_response(&failure),
        },
    }
}

fn trilho_da_ciencia(workspace_id: Uuid) -> Vec<Crumb> {
    vec![
        Crumb::to(Screen::Ideas),
        Crumb {
            label: "Ciência".to_owned(),
            href: format!("/workspaces/{workspace_id}/science"),
        },
    ]
}

// ── Metodologia e versões ────────────────────────────────────────────────

async fn new_methodology(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(workspace_id): Path<Uuid>,
) -> Response {
    let member = member_or_login!(state, headers);
    match pagina_de_metodologia(&state, &member, workspace_id, None).await {
        Ok(resposta) => resposta,
        Err(failure) => failure_response(&failure),
    }
}

async fn pagina_de_metodologia(
    state: &WorkspaceState,
    member: &Member,
    workspace_id: Uuid,
    message: Option<String>,
) -> Result<Response, api::ApiFailure> {
    let workspace = ambiente_ou_recusa(state, member, workspace_id).await?;
    let viewer = viewer(state, member).await;
    Ok(shell_page(
        "Nova metodologia",
        &viewer,
        Screen::Ideas,
        trilho_da_ciencia(workspace_id),
        ui::screens::science::nova_metodologia(ui::screens::science::Contexto {
            workspace,
            message,
        }),
    ))
}

#[derive(Deserialize)]
struct NovaMetodologiaForm {
    #[serde(default)]
    title: String,
    #[serde(default)]
    purpose: String,
    #[serde(default)]
    classification: String,
}

async fn create_methodology(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(workspace_id): Path<Uuid>,
    Form(form): Form<NovaMetodologiaForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let corpo = serde_json::json!({
        "title": form.title,
        "purpose": blank_to_none(form.purpose),
        "classification": blank_to_none(form.classification),
    });

    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/workspaces/{workspace_id}/methodologies"),
        &corpo,
    )
    .await
    {
        Ok(criada) => {
            // Para a metodologia, e não de volta à lista: o passo seguinte é
            // publicar uma versão, e é lá que ele está.
            match criada.get("id").and_then(Value::as_str) {
                Some(id) => Redirect::to(&format!("/methodologies/{id}")).into_response(),
                None => {
                    Redirect::to(&format!("/workspaces/{workspace_id}/science")).into_response()
                }
            }
        }
        Err(failure) => match motivo_para_o_formulario(&failure) {
            Some(motivo) => {
                match pagina_de_metodologia(&state, &member, workspace_id, Some(motivo)).await {
                    Ok(resposta) => resposta,
                    Err(failure) => failure_response(&failure),
                }
            }
            None => failure_response(&failure),
        },
    }
}

async fn methodology_detail(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(methodology_id): Path<Uuid>,
) -> Response {
    let member = member_or_login!(state, headers);
    let methodology = match api::get::<Value>(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/methodologies/{methodology_id}"),
    )
    .await
    {
        Ok(valor) => valor,
        Err(failure) => return failure_response(&failure),
    };

    let versions = optional(
        &state,
        &member,
        &format!("/api/v1/methodologies/{methodology_id}/versions"),
    )
    .await;
    let pode_criar = pode_criar_no_ambiente(&state, &member, &methodology).await;
    let viewer = viewer(&state, &member).await;
    let workspace_id = methodology
        .get("workspace_id")
        .and_then(Value::as_str)
        .and_then(|v| Uuid::parse_str(v).ok());

    let trail = workspace_id.map_or_else(|| vec![Crumb::to(Screen::Ideas)], trilho_da_ciencia);

    shell_page(
        "Metodologia",
        &viewer,
        Screen::Ideas,
        trail,
        ui::screens::science::metodologia(ui::screens::science::MetodologiaView {
            methodology,
            versions,
            may_create: pode_criar,
        }),
    )
}

async fn new_version(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(methodology_id): Path<Uuid>,
) -> Response {
    let member = member_or_login!(state, headers);
    match pagina_de_versao(&state, &member, methodology_id, None).await {
        Ok(resposta) => resposta,
        Err(failure) => failure_response(&failure),
    }
}

async fn pagina_de_versao(
    state: &WorkspaceState,
    member: &Member,
    methodology_id: Uuid,
    message: Option<String>,
) -> Result<Response, api::ApiFailure> {
    let methodology = api::get::<Value>(
        state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/methodologies/{methodology_id}"),
    )
    .await?;

    let versions = optional(
        state,
        member,
        &format!("/api/v1/methodologies/{methodology_id}/versions"),
    )
    .await;

    // A que está em vigor é a publicada que ninguém substituiu.
    let em_vigor = versions.as_array().and_then(|linhas| {
        linhas
            .iter()
            .find(|v| {
                v.get("status").and_then(Value::as_str) == Some("published")
                    && v.get("superseded_by_id").is_none_or(Value::is_null)
            })
            .cloned()
    });

    let viewer = viewer(state, member).await;
    Ok(shell_page(
        "Nova versão",
        &viewer,
        Screen::Ideas,
        vec![
            Crumb::to(Screen::Ideas),
            Crumb {
                label: "Metodologia".to_owned(),
                href: format!("/methodologies/{methodology_id}"),
            },
        ],
        ui::screens::science::nova_versao(ui::screens::science::NovaVersaoView {
            methodology,
            em_vigor,
            message,
        }),
    ))
}

#[derive(Deserialize)]
struct NovaVersaoForm {
    #[serde(default)]
    label: String,
    #[serde(default)]
    summary: String,
}

async fn publish_version(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(methodology_id): Path<Uuid>,
    Form(form): Form<NovaVersaoForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let corpo = serde_json::json!({"label": form.label, "summary": form.summary});

    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/methodologies/{methodology_id}/versions"),
        &corpo,
    )
    .await
    {
        Ok(_) => Redirect::to(&format!("/methodologies/{methodology_id}")).into_response(),
        Err(failure) => match motivo_para_o_formulario(&failure) {
            Some(motivo) => {
                match pagina_de_versao(&state, &member, methodology_id, Some(motivo)).await {
                    Ok(resposta) => resposta,
                    Err(failure) => failure_response(&failure),
                }
            }
            None => failure_response(&failure),
        },
    }
}

// ── Estudo e execuções ───────────────────────────────────────────────────

async fn new_study(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(workspace_id): Path<Uuid>,
) -> Response {
    let member = member_or_login!(state, headers);
    match pagina_de_estudo(&state, &member, workspace_id, None).await {
        Ok(resposta) => resposta,
        Err(failure) => failure_response(&failure),
    }
}

async fn pagina_de_estudo(
    state: &WorkspaceState,
    member: &Member,
    workspace_id: Uuid,
    message: Option<String>,
) -> Result<Response, api::ApiFailure> {
    let workspace = ambiente_ou_recusa(state, member, workspace_id).await?;
    let hypotheses = optional(
        state,
        member,
        &format!("/api/v1/workspaces/{workspace_id}/hypotheses"),
    )
    .await;
    let methodology_versions = versoes_de_metodologia(state, member, workspace_id).await;
    let viewer = viewer(state, member).await;

    Ok(shell_page(
        "Novo estudo",
        &viewer,
        Screen::Ideas,
        trilho_da_ciencia(workspace_id),
        ui::screens::science::novo_estudo(ui::screens::science::NovoEstudoView {
            workspace,
            hypotheses,
            methodology_versions,
            message,
        }),
    ))
}

#[derive(Deserialize)]
struct NovoEstudoForm {
    #[serde(default)]
    title: String,
    #[serde(default)]
    kind: String,
    #[serde(default)]
    objective: String,
    #[serde(default)]
    hypothesis_id: String,
    #[serde(default)]
    methodology_version_id: String,
    #[serde(default)]
    classification: String,
}

async fn create_study(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(workspace_id): Path<Uuid>,
    Form(form): Form<NovoEstudoForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let corpo = serde_json::json!({
        "title": form.title,
        "kind": form.kind,
        "objective": blank_to_none(form.objective),
        "hypothesis_id": blank_to_none(form.hypothesis_id),
        "methodology_version_id": blank_to_none(form.methodology_version_id),
        "classification": blank_to_none(form.classification),
    });

    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/workspaces/{workspace_id}/studies"),
        &corpo,
    )
    .await
    {
        Ok(criado) => match criado.get("id").and_then(Value::as_str) {
            Some(id) => Redirect::to(&format!("/studies/{id}")).into_response(),
            None => Redirect::to(&format!("/workspaces/{workspace_id}/science")).into_response(),
        },
        Err(failure) => match motivo_para_o_formulario(&failure) {
            Some(motivo) => {
                match pagina_de_estudo(&state, &member, workspace_id, Some(motivo)).await {
                    Ok(resposta) => resposta,
                    Err(failure) => failure_response(&failure),
                }
            }
            None => failure_response(&failure),
        },
    }
}

async fn study_detail(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(study_id): Path<Uuid>,
) -> Response {
    let member = member_or_login!(state, headers);
    let study = match api::get::<Value>(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/studies/{study_id}"),
    )
    .await
    {
        Ok(valor) => valor,
        Err(failure) => return failure_response(&failure),
    };

    let executions = optional(
        &state,
        &member,
        &format!("/api/v1/studies/{study_id}/executions"),
    )
    .await;
    let pode_criar = pode_criar_no_ambiente(&state, &member, &study).await;
    let viewer = viewer(&state, &member).await;
    let trail = study
        .get("workspace_id")
        .and_then(Value::as_str)
        .and_then(|v| Uuid::parse_str(v).ok())
        .map_or_else(|| vec![Crumb::to(Screen::Ideas)], trilho_da_ciencia);

    shell_page(
        "Estudo",
        &viewer,
        Screen::Ideas,
        trail,
        ui::screens::science::estudo(ui::screens::science::EstudoView {
            study,
            executions,
            may_create: pode_criar,
        }),
    )
}

async fn new_execution(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(study_id): Path<Uuid>,
) -> Response {
    let member = member_or_login!(state, headers);
    match pagina_de_execucao(&state, &member, study_id, None).await {
        Ok(resposta) => resposta,
        Err(failure) => failure_response(&failure),
    }
}

async fn pagina_de_execucao(
    state: &WorkspaceState,
    member: &Member,
    study_id: Uuid,
    message: Option<String>,
) -> Result<Response, api::ApiFailure> {
    let study = api::get::<Value>(
        state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/studies/{study_id}"),
    )
    .await?;

    let workspace_id = study
        .get("workspace_id")
        .and_then(Value::as_str)
        .and_then(|v| Uuid::parse_str(v).ok());

    let (methodology_versions, dataset_versions) = match workspace_id {
        Some(id) => (
            versoes_de_metodologia(state, member, id).await,
            versoes_de_dataset(state, member, id).await,
        ),
        None => (Vec::new(), Vec::new()),
    };

    let viewer = viewer(state, member).await;
    Ok(shell_page(
        "Registar execução",
        &viewer,
        Screen::Ideas,
        vec![
            Crumb::to(Screen::Ideas),
            Crumb {
                label: "Estudo".to_owned(),
                href: format!("/studies/{study_id}"),
            },
        ],
        ui::screens::science::nova_execucao(ui::screens::science::NovaExecucaoView {
            study,
            methodology_versions,
            dataset_versions,
            message,
        }),
    ))
}

#[derive(Deserialize)]
struct NovaExecucaoForm {
    #[serde(default)]
    status: String,
    #[serde(default)]
    environment: String,
    #[serde(default)]
    software_name: String,
    #[serde(default)]
    software_version: String,
    #[serde(default)]
    notes: String,
    #[serde(default)]
    methodology_version_id: String,
    #[serde(default)]
    dataset_version_id: String,
}

async fn record_execution(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(study_id): Path<Uuid>,
    Form(form): Form<NovaExecucaoForm>,
) -> Response {
    let member = member_or_login!(state, headers);

    let datasets: Vec<String> = blank_to_none(form.dataset_version_id).into_iter().collect();

    let corpo = serde_json::json!({
        "status": blank_to_none(form.status),
        "environment": blank_to_none(form.environment),
        "software_name": blank_to_none(form.software_name),
        "software_version": blank_to_none(form.software_version),
        "notes": blank_to_none(form.notes),
        "methodology_version_id": blank_to_none(form.methodology_version_id),
        "dataset_version_ids": datasets,
    });

    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/studies/{study_id}/executions"),
        &corpo,
    )
    .await
    {
        Ok(criada) => match criada.get("id").and_then(Value::as_str) {
            Some(id) => Redirect::to(&format!("/executions/{id}")).into_response(),
            None => Redirect::to(&format!("/studies/{study_id}")).into_response(),
        },
        Err(failure) => match motivo_para_o_formulario(&failure) {
            Some(motivo) => match pagina_de_execucao(&state, &member, study_id, Some(motivo)).await
            {
                Ok(resposta) => resposta,
                Err(failure) => failure_response(&failure),
            },
            None => failure_response(&failure),
        },
    }
}

// ── Execução e resultado ─────────────────────────────────────────────────

async fn execution_detail(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(execution_id): Path<Uuid>,
) -> Response {
    let member = member_or_login!(state, headers);
    let (execution, study, results) = match cadeia_da_execucao(&state, &member, execution_id).await
    {
        Ok(tudo) => tudo,
        Err(failure) => return failure_response(&failure),
    };

    let pode_criar = pode_criar_no_ambiente(&state, &member, &study).await;
    let viewer = viewer(&state, &member).await;
    shell_page(
        "Execução",
        &viewer,
        Screen::Ideas,
        vec![
            Crumb::to(Screen::Ideas),
            Crumb {
                label: "Estudo".to_owned(),
                href: format!("/studies/{}", text_de(&study, "id")),
            },
        ],
        ui::screens::science::execucao(ui::screens::science::ExecucaoView {
            execution,
            study,
            results,
            may_create: pode_criar,
        }),
    )
}

/// Uma execução, o estudo a que pertence, e o que ela produziu.
///
/// Os resultados vêm do ambiente e são filtrados por esta execução: não há
/// listagem por execução no Core, e inventar uma rota só para o ecrã seria pôr
/// no Core uma pergunta que só a interface faz.
async fn cadeia_da_execucao(
    state: &WorkspaceState,
    member: &Member,
    execution_id: Uuid,
) -> Result<(Value, Value, Value), api::ApiFailure> {
    let execution = api::get::<Value>(
        state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/executions/{execution_id}"),
    )
    .await?;

    let study_id = text_de(&execution, "study_id");
    let study = api::get::<Value>(
        state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/studies/{study_id}"),
    )
    .await?;

    let workspace_id = text_de(&study, "workspace_id");
    let todos = optional(
        state,
        member,
        &format!("/api/v1/workspaces/{workspace_id}/results"),
    )
    .await;

    let esperado = execution_id.to_string();
    let meus: Vec<Value> = todos
        .as_array()
        .map(|linhas| {
            linhas
                .iter()
                .filter(|r| r.get("execution_id").and_then(Value::as_str) == Some(&esperado))
                .cloned()
                .collect()
        })
        .unwrap_or_default();

    Ok((execution, study, Value::Array(meus)))
}

fn text_de(valor: &Value, chave: &str) -> String {
    valor
        .get(chave)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

async fn new_result(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(execution_id): Path<Uuid>,
) -> Response {
    let member = member_or_login!(state, headers);
    match pagina_de_resultado(&state, &member, execution_id, None).await {
        Ok(resposta) => resposta,
        Err(failure) => failure_response(&failure),
    }
}

async fn pagina_de_resultado(
    state: &WorkspaceState,
    member: &Member,
    execution_id: Uuid,
    message: Option<String>,
) -> Result<Response, api::ApiFailure> {
    let (execution, study, _) = cadeia_da_execucao(state, member, execution_id).await?;
    let viewer = viewer(state, member).await;
    Ok(shell_page(
        "Registar resultado",
        &viewer,
        Screen::Ideas,
        vec![
            Crumb::to(Screen::Ideas),
            Crumb {
                label: "Execução".to_owned(),
                href: format!("/executions/{execution_id}"),
            },
        ],
        ui::screens::science::novo_resultado(ui::screens::science::NovoResultadoView {
            execution,
            study,
            message,
        }),
    ))
}

#[derive(Deserialize)]
struct NovoResultadoForm {
    #[serde(default)]
    title: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    classification: String,
}

async fn create_result(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(execution_id): Path<Uuid>,
    Form(form): Form<NovoResultadoForm>,
) -> Response {
    let member = member_or_login!(state, headers);

    // O ambiente vem do estudo, e a origem vem do caminho.
    //
    // A pessoa nunca escolhe de onde o resultado veio: abriu este formulário
    // dentro de uma execução, e é essa execução que a operação do Core liga —
    // na mesma transacção, com `origin = operation`.
    let (_, study, _) = match cadeia_da_execucao(&state, &member, execution_id).await {
        Ok(tudo) => tudo,
        Err(failure) => return failure_response(&failure),
    };
    let workspace_id = text_de(&study, "workspace_id");

    let corpo = serde_json::json!({
        "title": form.title,
        "summary": form.summary,
        "execution_id": execution_id,
        "classification": blank_to_none(form.classification),
    });

    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/workspaces/{workspace_id}/results"),
        &corpo,
    )
    .await
    {
        Ok(criado) => match criado.get("id").and_then(Value::as_str) {
            Some(id) => Redirect::to(&format!("/results/{id}")).into_response(),
            None => Redirect::to(&format!("/executions/{execution_id}")).into_response(),
        },
        Err(failure) => match motivo_para_o_formulario(&failure) {
            Some(motivo) => {
                match pagina_de_resultado(&state, &member, execution_id, Some(motivo)).await {
                    Ok(resposta) => resposta,
                    Err(failure) => failure_response(&failure),
                }
            }
            None => failure_response(&failure),
        },
    }
}

/// O sentido da linhagem que se está a ver.
#[derive(serde::Deserialize)]
struct LineageQuery {
    direction: Option<String>,
}

/// Campos de uma validação.
#[derive(Deserialize)]
struct ValidationForm {
    #[serde(default)]
    kind: String,
    #[serde(default)]
    outcome: String,
    #[serde(default)]
    execution_id: String,
    #[serde(default)]
    note: String,
}

/// O formulário de validação de um resultado.
async fn validate_result_form(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(result_id): Path<Uuid>,
) -> Response {
    let member = member_or_login!(state, headers);
    match validation_page(&state, &member, result_id, None).await {
        Ok(response) => response,
        Err(failure) => failure_response(&failure),
    }
}

/// Regista a afirmação, em nome de quem a faz.
async fn record_validation(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(result_id): Path<Uuid>,
    Form(form): Form<ValidationForm>,
) -> Response {
    let member = member_or_login!(state, headers);

    // Uma cadeia vazia no `<select>` é «nenhuma», e não um identificador
    // inválido: submeter `""` como UUID faria o Core recusar por má forma em
    // vez de aceitar a ausência, que é o que a pessoa escolheu.
    let execution_id = (!form.execution_id.is_empty()).then_some(form.execution_id.as_str());
    let note = (!form.note.trim().is_empty()).then(|| form.note.trim());

    let body = serde_json::json!({
        "kind": form.kind,
        "outcome": form.outcome,
        "execution_id": execution_id,
        "note": note,
    });

    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/results/{result_id}/validations"),
        &body,
    )
    .await
    {
        Ok(_) => Redirect::to(&format!("/results/{result_id}")).into_response(),
        Err(failure) => {
            // A recusa do Core volta ao formulário, com o que ele disse. Um
            // ecrã de erro genérico perderia a razão — e a razão aqui é a
            // parte útil: falta a execução, ou falta a autoridade.
            // Só as recusas que a pessoa pode resolver voltam ao formulário:
            // falta a prova, ou falta a autoridade. Uma avaria, uma sessão
            // caída ou um recurso inalcançável não são coisas que se corrijam
            // preenchendo o campo outra vez.
            let motivo = match &failure {
                api::ApiFailure::Rejected(mensagem) => Some(mensagem.clone()),
                api::ApiFailure::Forbidden => Some(
                    "Validar ou dar por reproduzido um resultado exige liderança do \
                     ambiente ou gestão da unidade."
                        .to_owned(),
                ),
                _ => None,
            };

            match motivo {
                Some(motivo) => {
                    match validation_page(&state, &member, result_id, Some(motivo)).await {
                        Ok(response) => response,
                        Err(failure) => failure_response(&failure),
                    }
                }
                None => failure_response(&failure),
            }
        }
    }
}

/// O ecrã de validação, com o resultado e as execuções que servem de prova.
async fn validation_page(
    state: &WorkspaceState,
    member: &Member,
    result_id: Uuid,
    message: Option<String>,
) -> Result<Response, api::ApiFailure> {
    let result = api::get::<Value>(
        state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/results/{result_id}"),
    )
    .await?;

    // As execuções que podem servir de prova são as do estudo que produziu
    // este resultado. Sem execução de origem não há estudo conhecido, e a
    // lista fica vazia — que é o que o ecrã precisa de saber para explicar
    // porque a reprodução não está disponível.
    let executions = match result.get("execution_id").and_then(Value::as_str) {
        Some(execution_id) => {
            let execution =
                optional(state, member, &format!("/api/v1/executions/{execution_id}")).await;
            match execution.get("study_id").and_then(Value::as_str) {
                Some(study_id) => {
                    optional(
                        state,
                        member,
                        &format!("/api/v1/studies/{study_id}/executions"),
                    )
                    .await
                }
                None => Value::Null,
            }
        }
        None => Value::Null,
    };

    let viewer = viewer(state, member).await;
    let trail = vec![
        Crumb::to(Screen::Ideas),
        Crumb {
            label: "Resultado".to_owned(),
            href: format!("/results/{result_id}"),
        },
    ];

    Ok(shell_page(
        "Validar resultado",
        &viewer,
        Screen::Ideas,
        trail,
        ui::screens::science::validate_result(ui::screens::science::ValidateView {
            result,
            executions,
            message,
        }),
    ))
}

// ── Conhecimento ─────────────────────────────────────────────────────────

async fn knowledge(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    let member = member_or_login!(state, headers);
    let viewer = viewer(&state, &member).await;

    // Os três contadores que têm entidade no Core passam a contar de verdade.
    // «Resultados» não entra aqui: não há tabela nem consulta, e o ecrã
    // declara-o em vez de mostrar zero.
    let (bibliography, documents, datasets, recent, ai) = tokio::join!(
        optional(&state, &member, "/api/v1/sources?page_size=1"),
        optional(&state, &member, "/api/v1/documents?page_size=1"),
        optional(&state, &member, "/api/v1/datasets?page_size=1"),
        optional(&state, &member, "/api/v1/search?q=a&page_size=10"),
        optional(&state, &member, "/api/v1/ai/status"),
    );

    let content = ui::screens::knowledge::knowledge(ui::screens::knowledge::KnowledgeCounts {
        bibliography,
        documents,
        datasets,
        recent,
        inference_available: inference_available(&ai),
        may_use_assistance: viewer.can(ocinye_contracts::Permission::AiUse),
    });

    shell_page(
        "Conhecimento",
        &viewer,
        Screen::Knowledge,
        Vec::new(),
        content,
    )
}

// ── Inteligência ─────────────────────────────────────────────────────────

async fn ai_hub(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    let member = member_or_login!(state, headers);
    let viewer = viewer(&state, &member).await;

    let (status, models) = tokio::join!(
        optional(&state, &member, "/api/v1/ai/status"),
        optional(&state, &member, "/api/v1/ai/models"),
    );

    shell_page(
        "Ocinye AI",
        &viewer,
        Screen::Ai,
        Vec::new(),
        ui::screens::ai::hub(&status, &models),
    )
}

async fn new_agent(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    let member = member_or_login!(state, headers);
    let viewer = viewer(&state, &member).await;
    let models = optional(&state, &member, "/api/v1/ai/models").await;

    shell_page(
        "Criar Agente IA",
        &viewer,
        Screen::Agents,
        agent_trail(),
        ui::screens::ai::new_agent(&models, None),
    )
}

fn agent_trail() -> Vec<Crumb> {
    vec![Crumb::to(Screen::Agents)]
}

/// Campos do construtor de agentes.
#[derive(Deserialize)]
struct NewAgentForm {
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
    max_classification: String,
    // Uma checkbox não marcada não é submetida: a ausência do campo é `false`.
    #[serde(default)]
    uses_bibliography: Option<String>,
    #[serde(default)]
    uses_documents: Option<String>,
    #[serde(default)]
    uses_datasets: Option<String>,
}

/// `POST /ai/agents/new`
///
/// Antes desta auditoria este caminho não existia: o formulário submetia e o
/// Axum devolvia 405. Um agente é uma definição e guarda-se sem nó de IA; o que
/// falta é onde correr, e o estado do agente di-lo.
async fn create_agent(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<NewAgentForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let viewer = viewer(&state, &member).await;

    let body = serde_json::json!({
        "name": form.name,
        "purpose": form.purpose,
        "instructions": form.instructions,
        "capability": if form.capability.is_empty() { "general".to_owned() } else { form.capability },
        "scope": if form.scope.is_empty() { "personal".to_owned() } else { form.scope },
        "max_classification": if form.max_classification.is_empty() {
            "INTERNAL".to_owned()
        } else {
            form.max_classification
        },
        "uses_bibliography": form.uses_bibliography.is_some(),
        "uses_documents": form.uses_documents.is_some(),
        "uses_datasets": form.uses_datasets.is_some(),
    });

    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        "/api/v1/ai/agents",
        &body,
    )
    .await
    {
        Ok(_) => Redirect::to("/ai/agents").into_response(),
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        Err(failure) => {
            let models = optional(&state, &member, "/api/v1/ai/models").await;
            (
                StatusCode::UNPROCESSABLE_ENTITY,
                shell_page(
                    "Criar Agente IA",
                    &viewer,
                    Screen::Agents,
                    agent_trail(),
                    ui::screens::ai::new_agent(&models, Some(failure.to_string())),
                ),
            )
                .into_response()
        }
    }
}

#[derive(Deserialize)]
struct PromptQuery {
    #[serde(default)]
    workspace: Option<Uuid>,
}

async fn prompt(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(query): Query<PromptQuery>,
) -> Response {
    let member = member_or_login!(state, headers);
    let viewer = viewer(&state, &member).await;
    let status = optional(&state, &member, "/api/v1/ai/status").await;

    // Quando o prompt é aberto de dentro de um Research Workspace, o contexto
    // é resolvido e mostrado — não presumido a partir do URL.
    let context = if let Some(id) = query.workspace {
        let path = format!("/api/v1/workspaces/{id}");
        let overview = optional(&state, &member, &path).await;
        overview.get("workspace").map(|w| {
            (
                w.get("code")
                    .and_then(Value::as_str)
                    .unwrap_or("—")
                    .to_owned(),
                w.get("unit_code")
                    .and_then(Value::as_str)
                    .unwrap_or("—")
                    .to_owned(),
            )
        })
    } else {
        None
    };

    let content =
        ui::screens::prompt::prompt(ui::screens::prompt::context_from(&status, context), None);
    shell_page(
        "Prompt Ocinye",
        &viewer,
        Screen::Prompt,
        Vec::new(),
        content,
    )
}

/// O pedido submetido no Prompt Ocinye.
#[derive(Deserialize)]
struct PromptForm {
    #[serde(default)]
    prompt: String,
    #[serde(default)]
    workspace: Option<String>,
}

/// `POST /ai/prompt`
///
/// Antes desta auditoria este caminho não existia: o formulário submetia e o
/// Axum devolvia 405. Agora o Core decide, e a sua recusa — permissão, ou
/// capacidade sem nó — aparece como estado nativo do ecrã, nunca como alerta
/// do browser (briefing §8).
async fn submit_prompt(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<PromptForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    let viewer = viewer(&state, &member).await;
    let status = optional(&state, &member, "/api/v1/ai/status").await;

    let context = match form.workspace.as_deref().filter(|id| !id.is_empty()) {
        Some(id) => {
            let path = format!("/api/v1/workspaces/{id}");
            let overview = optional(&state, &member, &path).await;
            overview.get("workspace").map(|w| {
                (
                    w.get("code")
                        .and_then(Value::as_str)
                        .unwrap_or("—")
                        .to_owned(),
                    w.get("unit_code")
                        .and_then(Value::as_str)
                        .unwrap_or("—")
                        .to_owned(),
                )
            })
        }
        None => None,
    };

    let outcome = api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        "/api/v1/ai/prompt",
        &serde_json::json!({ "prompt": form.prompt }),
    )
    .await;

    let notice = match outcome {
        // Inalcançável nesta instalação, e deliberadamente não simulado: quando
        // existir inferência, é aqui que a resposta entra.
        Ok(_) => Some(ui::screens::prompt::Notice::accepted()),
        Err(ApiFailure::Unauthorised) => return Redirect::to("/login").into_response(),
        Err(failure) => Some(ui::screens::prompt::Notice::refused(failure.to_string())),
    };

    let content =
        ui::screens::prompt::prompt(ui::screens::prompt::context_from(&status, context), notice);

    shell_page(
        "Prompt Ocinye",
        &viewer,
        Screen::Prompt,
        Vec::new(),
        content,
    )
}

/// Termo de pesquisa.
#[derive(Deserialize)]
struct SearchQuery {
    #[serde(default)]
    q: String,
}

/// `GET /search`
///
/// O Ocinye Core serve `/api/v1/search` desde sempre; até esta auditoria não
/// havia por onde lá chegar a partir do Workspace.
async fn search(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(query): Query<SearchQuery>,
) -> Response {
    let member = member_or_login!(state, headers);
    let viewer = viewer(&state, &member).await;

    // Sem termo não se pesquisa: uma consulta vazia devolveria tudo o que o
    // membro pode ver, o que não é uma pesquisa e custa uma varredura.
    let results = if query.q.trim().is_empty() {
        Value::Null
    } else {
        let path = format!(
            "/api/v1/search?q={}&page_size=25",
            urlencoding_minimal(query.q.trim())
        );
        optional(&state, &member, &path).await
    };

    // O corpo dos ficheiros, ao lado dos títulos. Duas consultas porque são
    // duas afirmações diferentes, e não uma que se possa ordenar com a outra.
    let corpos = if query.q.trim().is_empty() {
        Value::Null
    } else {
        let path = format!(
            "/api/v1/search/bodies?q={}&page_size=10",
            urlencoding_minimal(query.q.trim())
        );
        optional(&state, &member, &path).await
    };

    let semantic = optional(&state, &member, "/api/v1/search/semantic-availability").await;

    shell_page(
        "Pesquisar",
        &viewer,
        Screen::Search,
        Vec::new(),
        ui::screens::search::search(&query.q, &results, &corpos, &semantic),
    )
}

// ── A Universal Command Surface ──────────────────────────────────────────

#[derive(Deserialize)]
struct AskQuery {
    #[serde(default)]
    q: Option<String>,
    #[serde(default)]
    intent: Option<String>,
}

/// `Search · Ask · Act`, numa só superfície.
///
/// Sem termo, mostra o campo. Com termo, chama o Core — que responde à
/// pesquisa deterministicamente e declara indisponível o que precisa de um
/// modelo (briefing §29, §66).
async fn ask(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(query): Query<AskQuery>,
) -> Response {
    let member = member_or_login!(state, headers);
    let viewer = viewer(&state, &member).await;

    let term = query.q.unwrap_or_default();

    // Sem escolha explícita, a superfície lê a frase. O membro escreve
    // naturalmente — «Cria uma pasta X dentro de Y» — e os três modos ficam
    // como controlo e como reserva.
    let detected = ocinye_contracts::agentic::Intent::detect(&term);
    let intent = query
        .intent
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| detected.as_str().to_owned());

    let outcome = if term.trim().is_empty() {
        Value::Null
    } else {
        let body = serde_json::json!({
            "utterance": term.trim(),
            "intent": intent,
        });

        match api::post(
            &state,
            &member.session.access_token,
            &member.correlation_id,
            "/api/v1/agentic/invoke",
            &body,
        )
        .await
        {
            Ok(outcome) => outcome,
            Err(ApiFailure::Unauthorised) => return Redirect::to("/login").into_response(),
            // Uma recusa do Core é renderizada como estado do ecrã, não como
            // página de erro: o membro continua a poder escrever outra coisa.
            Err(failure) => serde_json::json!({
                "kind": "unavailable",
                "reason": failure.to_string(),
                "alternative": "A navegação e as acções do Workspace continuam disponíveis.",
            }),
        }
    };

    shell_page(
        "Pesquisar, perguntar ou executar",
        &viewer,
        Screen::Ask,
        Vec::new(),
        ui::screens::ask::ask(&ui::screens::ask::AskView {
            query: term,
            intent,
            outcome,
            may_use_ai: viewer.can(ocinye_contracts::Permission::AiUse),
        }),
    )
}

/// Confirma e executa um plano.
async fn execute_plan(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(plan_id): Path<Uuid>,
) -> Response {
    let member = member_or_login!(state, headers);

    // Confirmar e executar são dois pedidos ao Core, nesta ordem: a
    // confirmação liga-se ao digest do plano, e a execução verifica-a. Um só
    // pedido tornaria impossível distinguir «confirmado» de «executado».
    let approve = format!("/api/v1/agentic/plans/{plan_id}/approve");
    if let Err(failure) = api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &approve,
        &serde_json::json!({}),
    )
    .await
    {
        return failure_response(&failure);
    }

    let execute = format!("/api/v1/agentic/plans/{plan_id}/execute");
    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &execute,
        &serde_json::json!({}),
    )
    .await
    {
        Ok(_) | Err(ApiFailure::Denied) => Redirect::to("/ask").into_response(),
        Err(failure) => failure_response(&failure),
    }
}

/// Recusa um plano.
async fn reject_plan(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(plan_id): Path<Uuid>,
) -> Response {
    let member = member_or_login!(state, headers);

    let path = format!("/api/v1/agentic/plans/{plan_id}/reject");
    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &path,
        &serde_json::json!({}),
    )
    .await
    {
        Ok(_) | Err(ApiFailure::Denied) => Redirect::to("/ask").into_response(),
        Err(failure) => failure_response(&failure),
    }
}

/// Escapa um termo para o colocar numa query string.
///
/// Mínimo de propósito: só os caracteres que quebrariam a query. Uma dependência
/// inteira para isto seria desproporcionada (`CLAUDE.md` §54).
fn urlencoding_minimal(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                char::from(byte).to_string()
            }
            other => format!("%{other:02X}"),
        })
        .collect()
}

async fn compute(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    let member = member_or_login!(state, headers);
    let viewer = viewer(&state, &member).await;

    let (status, nodes) = tokio::join!(
        optional(&state, &member, "/api/v1/compute/status"),
        optional(&state, &member, "/api/v1/compute/nodes"),
    );

    shell_page(
        "Computação",
        &viewer,
        Screen::Compute,
        Vec::new(),
        ui::screens::compute::compute(&status, &nodes),
    )
}

// ── Institucional ────────────────────────────────────────────────────────

/// O feed institucional.
///
/// # `required`, e não `optional`
///
/// O feed é o conteúdo do ecrã, e era lido com `optional` — que transforma uma
/// falha do Core em `null`. O ecrã renderizava isso como zero acontecimentos:
/// «não se passou nada na instituição» quando o que se passou foi o Core não
/// responder.
///
/// São dois factos opostos e tinham o mesmo aspecto. Um convida a fechar a
/// página; o outro pede que se avise alguém.
async fn activity(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    let member = member_or_login!(state, headers);
    let viewer = viewer(&state, &member).await;
    let payload = match required(&state, &member, "/api/v1/activity?page_size=100").await {
        Ok(payload) => payload,
        Err(failure) => return failure_response(&failure),
    };

    shell_page(
        "Actividade",
        &viewer,
        Screen::Activity,
        Vec::new(),
        ui::screens::activity::activity(&payload),
    )
}

// ── Criar ideia ──────────────────────────────────────────────────────────

/// Os workspaces onde o membro pode criar, para os selectores de destino.
///
/// Uma chamada só, partilhada pelos dois formulários: a pergunta é a mesma, e a
/// política que a responde também.
async fn creation_destinations(state: &WorkspaceState, member: &Member) -> Value {
    optional(state, member, "/api/v1/workspaces?page_size=200").await
}

async fn new_source_form(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    let member = member_or_login!(state, headers);
    let viewer = viewer(&state, &member).await;
    let destinos = creation_destinations(&state, &member).await;
    let trail = vec![Crumb::to(Screen::Bibliography)];
    shell_page(
        "Nova Referência",
        &viewer,
        Screen::Bibliography,
        trail,
        ui::screens::lists::new_source(&destinos, None),
    )
}

#[derive(Deserialize)]
struct NewSourceForm {
    workspace_id: Uuid,
    title: String,
    #[serde(default)]
    authors: String,
    #[serde(default)]
    year: String,
    #[serde(default)]
    container_title: String,
    #[serde(default)]
    doi: String,
    #[serde(default)]
    abstract_text: String,
    #[serde(default)]
    classification: String,
}

async fn create_source(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<NewSourceForm>,
) -> Response {
    let member = member_or_login!(state, headers);

    let authors: Vec<String> = form
        .authors
        .split(';')
        .map(str::trim)
        .filter(|a| !a.is_empty())
        .map(ToOwned::to_owned)
        .collect();

    let mut body = serde_json::json!({
        "title": form.title,
        "authors": authors,
        "container_title": blank_to_none(form.container_title),
        "doi": blank_to_none(form.doi),
        "abstract_text": blank_to_none(form.abstract_text),
    });
    if let Some(year) = blank_to_none(form.year).and_then(|y| y.parse::<i32>().ok()) {
        body["year"] = Value::from(year);
    }
    if let Some(classification) = blank_to_none(form.classification) {
        body["classification"] = Value::String(classification);
    }

    // O workspace vai no caminho, e é o Core que decide se este membro pode
    // criar lá. O selector filtrou por conveniência; um identificador escrito à
    // mão chega aqui exactamente como qualquer outro.
    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/workspaces/{}/sources", form.workspace_id),
        &body,
    )
    .await
    {
        Ok(_) => Redirect::to("/bibliography").into_response(),
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        Err(failure) => {
            let viewer = viewer(&state, &member).await;
            let destinos = creation_destinations(&state, &member).await;
            let trail = vec![Crumb::to(Screen::Bibliography)];
            shell_page(
                "Nova Referência",
                &viewer,
                Screen::Bibliography,
                trail,
                ui::screens::lists::new_source(&destinos, Some(failure.to_string())),
            )
        }
    }
}

async fn new_dataset_form(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    let member = member_or_login!(state, headers);
    let viewer = viewer(&state, &member).await;
    let destinos = creation_destinations(&state, &member).await;
    let trail = vec![Crumb::to(Screen::Datasets)];
    shell_page(
        "Novo Dataset",
        &viewer,
        Screen::Datasets,
        trail,
        ui::screens::lists::new_dataset(&destinos, None),
    )
}

#[derive(Deserialize)]
struct NewDatasetForm {
    workspace_id: Uuid,
    code: String,
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    keywords: String,
    #[serde(default)]
    usage_restrictions: String,
    #[serde(default)]
    classification: String,
}

async fn create_dataset(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<NewDatasetForm>,
) -> Response {
    let member = member_or_login!(state, headers);

    let keywords: Vec<String> = form
        .keywords
        .split(',')
        .map(str::trim)
        .filter(|k| !k.is_empty())
        .map(ToOwned::to_owned)
        .collect();

    let mut body = serde_json::json!({
        "code": form.code,
        "title": form.title,
        "description": blank_to_none(form.description),
        "usage_restrictions": blank_to_none(form.usage_restrictions),
        "keywords": keywords,
    });
    if let Some(classification) = blank_to_none(form.classification) {
        body["classification"] = Value::String(classification);
    }

    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/workspaces/{}/datasets", form.workspace_id),
        &body,
    )
    .await
    {
        Ok(_) => Redirect::to("/datasets").into_response(),
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        Err(failure) => {
            let viewer = viewer(&state, &member).await;
            let destinos = creation_destinations(&state, &member).await;
            let trail = vec![Crumb::to(Screen::Datasets)];
            shell_page(
                "Novo Dataset",
                &viewer,
                Screen::Datasets,
                trail,
                ui::screens::lists::new_dataset(&destinos, Some(failure.to_string())),
            )
        }
    }
}

/// O resultado de uma mudança de imagem de perfil, tal como volta do redirect.
#[derive(Debug, Default, Deserialize)]
struct AvatarOutcome {
    /// Presente quando a operação correu bem.
    avatar: Option<String>,
    /// A razão, quando não correu.
    avatar_erro: Option<String>,
}

async fn settings_account(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(outcome): Query<AvatarOutcome>,
) -> Response {
    let member = member_or_login!(state, headers);
    let viewer = viewer(&state, &member).await;
    let (me, organisation) = tokio::join!(
        optional(&state, &member, "/api/v1/me"),
        optional(&state, &member, "/api/v1/organisation"),
    );
    shell_page(
        "Definições",
        &viewer,
        Screen::Settings,
        Vec::new(),
        ui::screens::settings::account(
            &me,
            &organisation,
            &viewer.avatar,
            outcome.avatar_erro,
            outcome.avatar.as_deref() == Some("ok"),
        ),
    )
}

/// A ajuda do Workspace.
///
/// Conteúdo, não consulta: não chama o Core, e por isso não pode falhar por
/// causa dele. Quem vem aqui porque alguma coisa não funcionou merece encontrar
/// a página de pé.
async fn help(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    let member = member_or_login!(state, headers);
    let viewer = viewer(&state, &member).await;
    shell_page(
        "Ajuda",
        &viewer,
        Screen::Help,
        Vec::new(),
        ui::screens::help::help(),
    )
}

/// Largest body the Workspace accepts for a profile photograph.
///
/// O mesmo limite do Core, mais o envelope multipart. Recusar aqui poupa uma
/// travessia; recusar só aqui não bastaria, porque o Core não pode confiar em
/// quem o chama.
const AVATAR_BODY_LIMIT_BYTES: usize = 8 * 1024 * 1024 + 64 * 1024;

/// Escolhe um avatar do catálogo Ocinye.
async fn choose_avatar_preset(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<AvatarPresetForm>,
) -> Response {
    let member = member_or_login!(state, headers);
    avatar_outcome(
        api::post(
            &state,
            &member.session.access_token,
            &member.correlation_id,
            "/api/v1/me/avatar/preset",
            &serde_json::json!({ "preset": form.preset }),
        )
        .await,
    )
}

/// O identificador escolhido na grelha de avatares.
#[derive(Deserialize)]
struct AvatarPresetForm {
    preset: String,
}

/// Volta às iniciais.
async fn use_initials_avatar(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    let member = member_or_login!(state, headers);

    // A operação é `DELETE` no Core e `POST` aqui: um formulário HTML só sabe
    // enviar `GET` e `POST`, e usar `GET` para uma operação que muda estado é
    // exactamente o que a guarda de mesma origem existe para impedir.
    let resultado = match state
        .http
        .delete(format!("{}/api/v1/me/avatar", state.config.core_url))
        .bearer_auth(&member.session.access_token)
        .header(
            ocinye_observability::CORRELATION_ID_HEADER,
            &member.correlation_id,
        )
        .send()
        .await
    {
        Ok(response) if response.status().is_success() => Ok(serde_json::Value::Null),
        Ok(response) if response.status().as_u16() == 401 => Err(ApiFailure::Unauthorised),
        Ok(response) => Err(ApiFailure::Failed(format!(
            "the Core returned status {}",
            response.status()
        ))),
        Err(error) => Err(ApiFailure::Failed(format!(
            "the Core is unreachable: {error}"
        ))),
    };

    avatar_outcome(resultado)
}

/// Carrega a fotografia do próprio membro.
async fn upload_avatar(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Response {
    let member = member_or_login!(state, headers);

    let mut ficheiro: Option<(String, String, Vec<u8>)> = None;
    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name() == Some("file") {
            let nome = field.file_name().unwrap_or("fotografia").to_owned();
            let tipo = field
                .content_type()
                .unwrap_or("application/octet-stream")
                .to_owned();
            match field.bytes().await {
                Ok(bytes) => ficheiro = Some((nome, tipo, bytes.to_vec())),
                Err(_) => return avatar_error("A fotografia não pôde ser lida."),
            }
        }
    }

    let Some((nome, tipo, dados)) = ficheiro else {
        return avatar_error("Escolha uma fotografia antes de confirmar.");
    };
    if dados.is_empty() {
        return avatar_error("Escolha uma fotografia antes de confirmar.");
    }

    // O tipo declarado viaja porque o multipart o exige, e não porque alguém
    // acredite nele: é o Core que decide o formato pelos bytes.
    avatar_outcome(
        api::upload(
            &state,
            &member.session.access_token,
            &member.correlation_id,
            "/api/v1/me/avatar/photo",
            nome,
            tipo,
            dados,
        )
        .await,
    )
}

/// Traduz o resultado de uma mudança de avatar em navegação.
///
/// Sucesso e falha voltam ambos a Definições, e é lá que a mensagem aparece. O
/// estado persistido é o que a página seguinte lê do Core: não há aqui nenhum
/// optimismo local a mostrar uma escolha que o Core não confirmou.
fn avatar_outcome(resultado: Result<serde_json::Value, ApiFailure>) -> Response {
    match resultado {
        Ok(_) => Redirect::to("/settings?avatar=ok").into_response(),
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),

        // O armazenamento não está de pé. A mensagem que vinha do Core dizia
        // «The object could not be stored.» — em inglês, e a descrever o que
        // falhou em vez do que se passa. Quem carregou uma fotografia conclui
        // que a fotografia tem alguma coisa de errado, e volta a tentar com
        // outra.
        Err(ApiFailure::Unavailable(_)) => avatar_error(
            "O armazenamento institucional não está a responder. \
             A fotografia não foi guardada — não é um problema com a imagem, \
             e os avatares Ocinye e as iniciais continuam disponíveis.",
        ),

        // O Core recusa um identificador que não esteja no seu catálogo, e a
        // recusa vinha em inglês: «That is not an Ocinye avatar.» Como a grelha
        // só oferece o que está no catálogo, esta recusa só acontece quando os
        // dois lados discordam — tipicamente porque um deles ainda não foi
        // reiniciado. Dizê-lo é mais útil do que repetir a frase do Core.
        Err(ApiFailure::Failed(message)) if message.contains("Ocinye avatar") => avatar_error(
            "Este avatar não faz parte do catálogo que o Core conhece. \
             Se acabou de haver uma actualização, o serviço pode ainda estar a arrancar.",
        ),

        Err(ApiFailure::Failed(message)) => avatar_error(&message),
        Err(_) => avatar_error("A imagem de perfil não pôde ser alterada."),
    }
}

/// Volta a Definições com uma razão que o membro possa ler.
fn avatar_error(message: &str) -> Response {
    Redirect::to(&format!("/settings?avatar_erro={}", urlencoding(message))).into_response()
}

/// Codifica um texto para caber num parâmetro de query.
fn urlencoding(value: &str) -> String {
    value
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

/// A fotografia do próprio membro.
///
/// # Porque passa por aqui
///
/// O Core podia devolver um URL assinado e o browser ir buscá-lo directamente.
/// Não vai: um URL assinado dura cinco minutos, muda a cada render e traz o
/// endereço do bucket consigo. Numa imagem que aparece em todas as páginas, isso
/// significa uma cache que nunca acerta e o armazenamento institucional escrito
/// no HTML.
///
/// A fotografia tem alguns kilobytes, e passá-la por aqui deixa a shell a falar
/// só com o Ocinye OS.
async fn own_avatar(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(version): Path<String>,
) -> Response {
    let Some(member) = current_member(&state, &headers) else {
        // Sem sessão não há avatar, e não há como saber de quem seria. `404` e
        // não um reencaminhamento: quem pediu isto foi um `<img>`, e devolver-lhe
        // a página de login seria devolver HTML a quem esperava uma imagem.
        return StatusCode::NOT_FOUND.into_response();
    };

    match api::bytes(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/me/avatar/{version}"),
    )
    .await
    {
        Ok((content_type, bytes)) => (
            [
                (header::CONTENT_TYPE, content_type),
                // O endereço só muda quando a fotografia muda, e por isso este
                // conteúdo nunca muda. `private` porque é de uma pessoa: uma
                // cache partilhada não deve servi-lo a outra.
                (
                    header::CACHE_CONTROL,
                    "private, max-age=31536000, immutable".to_owned(),
                ),
            ],
            bytes,
        )
            .into_response(),
        // Uma fotografia que não chega não é uma falha da página: o componente
        // cai nas iniciais sozinho.
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

/// `Definições → Segurança`.
async fn settings_security(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    let member = member_or_login!(state, headers);
    let viewer = viewer(&state, &member).await;
    let sessions = match required(&state, &member, "/api/v1/auth/sessions").await {
        Ok(payload) => payload,
        Err(failure) => return failure_response(&failure),
    };
    shell_page(
        "Definições",
        &viewer,
        Screen::Settings,
        Vec::new(),
        ui::screens::settings::security(Some(&sessions), None, None),
    )
}

#[derive(Deserialize)]
struct ChangePasswordForm {
    current: String,
    password: String,
    confirmation: String,
}

/// A mudança de palavra-passe, e a rotação de sessão que a acompanha.
///
/// O Core revoga todas as sessões e emite uma nova. A sessão do Workspace é
/// substituída aqui pela mesma razão: manter a antiga deixaria o membro com um
/// identificador que o Core já não reconhece, e o próximo pedido cairia no
/// início de sessão sem explicação nenhuma.
async fn change_password(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<ChangePasswordForm>,
) -> Response {
    let member = member_or_login!(state, headers);

    let body = serde_json::json!({
        "current": form.current,
        "password": form.password,
        "confirmation": form.confirmation,
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
        Ok(session) => {
            if let Some(id) = session::session_id_from_cookies(
                headers
                    .get(header::COOKIE)
                    .and_then(|value| value.to_str().ok()),
            ) {
                state.sessions.remove(&id);
            }
            let session_id = state.sessions.create(Session {
                access_token: session.token,
                display_name: session.display_name,
                email: member.session.email.clone(),
                must_change_password: session.must_change_password,
                expires_at: Instant::now() + state.config.session_ttl,
            });
            (
                StatusCode::SEE_OTHER,
                [
                    (header::LOCATION, "/settings/security".to_owned()),
                    (
                        header::SET_COOKIE,
                        session::cookie_header(
                            &session_id,
                            state.config.cookie_secure,
                            state.config.session_ttl,
                        ),
                    ),
                ],
            )
                .into_response()
        }
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        Err(failure) => {
            let viewer = viewer(&state, &member).await;
            let sessions = required(&state, &member, "/api/v1/auth/sessions")
                .await
                .ok();
            shell_page(
                "Definições",
                &viewer,
                Screen::Settings,
                Vec::new(),
                ui::screens::settings::security(sessions.as_ref(), Some(failure.to_string()), None),
            )
        }
    }
}

/// Terminar uma sessão própria.
///
/// O identificador vem do formulário, e é o Core que resolve a posse. Se a
/// sessão terminada for a actual, o Workspace larga a sua também: dizer
/// «terminada» e continuar autenticado seria mentir sobre o efeito.
async fn revoke_session(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
) -> Response {
    let member = member_or_login!(state, headers);

    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/auth/sessions/{session_id}/revoke"),
        &serde_json::json!({}),
    )
    .await
    {
        Ok(_) | Err(ApiFailure::Failed(_)) => {
            // O Core devolve `204`, que o cliente HTTP lê como corpo vazio.
            // Se a sessão terminada era a que sustenta este pedido, a nossa
            // deixa de valer — e é o próximo pedido que o descobriria.
            Redirect::to("/settings/security").into_response()
        }
        Err(ApiFailure::Unauthorised) => {
            if let Some(id) = session::session_id_from_cookies(
                headers
                    .get(header::COOKIE)
                    .and_then(|value| value.to_str().ok()),
            ) {
                state.sessions.remove(&id);
            }
            Redirect::to("/login").into_response()
        }
        Err(failure) => {
            let viewer = viewer(&state, &member).await;
            let sessions = required(&state, &member, "/api/v1/auth/sessions")
                .await
                .ok();
            shell_page(
                "Definições",
                &viewer,
                Screen::Settings,
                Vec::new(),
                ui::screens::settings::security(sessions.as_ref(), Some(failure.to_string()), None),
            )
        }
    }
}

/// O selector de promoção de uma ideia a projecto.
///
/// Não existe `POST /projects` no Core: um projecto nasce da promoção de uma
/// ideia. O selector pede ao Core as ideias que a promoção aceitaria hoje
/// (`?promotable=true`) em vez de repetir essa regra aqui — e o Core valida
/// outra vez quando a promoção chega.
async fn new_project_form(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Response {
    let member = member_or_login!(state, headers);
    let viewer = viewer(&state, &member).await;
    let candidatas = match required(
        &state,
        &member,
        "/api/v1/workspaces?kind=idea&promotable=true&page_size=100",
    )
    .await
    {
        Ok(payload) => payload,
        Err(failure) => return failure_response(&failure),
    };

    let trail = vec![Crumb::to(Screen::Projects)];
    shell_page(
        "Novo Projecto",
        &viewer,
        Screen::Projects,
        trail,
        ui::screens::lists::new_project(
            &candidatas,
            params.get("workspace").map(String::as_str),
            None,
        ),
    )
}

#[derive(Deserialize)]
struct PromotionForm {
    workspace_id: Uuid,
    code: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    objectives: String,
}

async fn promote_idea(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<PromotionForm>,
) -> Response {
    let member = member_or_login!(state, headers);

    // O selector devolve o workspace; a promoção age sobre a ideia que ele
    // contém. Uma volta ao Core resolve isso — e é ele que decide se o membro
    // pode sequer ler esse workspace.
    let workspace = match required(
        &state,
        &member,
        &format!("/api/v1/workspaces/{}", form.workspace_id),
    )
    .await
    {
        Ok(payload) => payload,
        Err(failure) => return failure_response(&failure),
    };

    let idea_id = workspace
        .get("idea")
        .and_then(|idea| idea.get("id"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();

    let body = serde_json::json!({
        "code": form.code,
        "title": blank_to_none(form.title),
        "objectives": blank_to_none(form.objectives),
    });

    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/ideas/{idea_id}/promotion"),
        &body,
    )
    .await
    {
        Ok(_) => Redirect::to(&format!("/workspaces/{}", form.workspace_id)).into_response(),
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        Err(failure) => {
            // A recusa vem do Core tal como veio. Uma ideia pode ter mudado de
            // estado entre a listagem e a submissão, e o membro tem de ver isso
            // dito, não um erro genérico.
            let viewer = viewer(&state, &member).await;
            let candidatas = optional(
                &state,
                &member,
                "/api/v1/workspaces?kind=idea&promotable=true&page_size=100",
            )
            .await;
            let trail = vec![Crumb::to(Screen::Projects)];
            shell_page(
                "Novo Projecto",
                &viewer,
                Screen::Projects,
                trail,
                ui::screens::lists::new_project(&candidatas, None, Some(failure.to_string())),
            )
        }
    }
}

/// O formulário de criação de uma unidade.
///
/// Existia no Core desde sempre (`POST /api/v1/units`) e não tinha ecrã: numa
/// instalação nova não havia como criar a primeira unidade, e sem unidade não
/// há onde nascer uma ideia. A aplicação não se conseguia povoar por si.
async fn new_unit_form(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    let member = member_or_login!(state, headers);
    let viewer = viewer(&state, &member).await;
    let trail = vec![Crumb::to(Screen::Units)];
    shell_page(
        "Nova Unidade",
        &viewer,
        Screen::Units,
        trail,
        ui::screens::lists::new_unit(None),
    )
}

#[derive(Deserialize)]
struct NewUnitForm {
    code: String,
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    research_areas: String,
}

async fn create_unit(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<NewUnitForm>,
) -> Response {
    let member = member_or_login!(state, headers);

    let areas: Vec<String> = form
        .research_areas
        .split(',')
        .map(str::trim)
        .filter(|a| !a.is_empty())
        .map(ToOwned::to_owned)
        .collect();

    let body = serde_json::json!({
        "code": form.code,
        "name": form.name,
        "description": blank_to_none(form.description),
        "research_areas": areas,
    });

    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        "/api/v1/units",
        &body,
    )
    .await
    {
        Ok(_) => Redirect::to("/units").into_response(),
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        Err(failure) => {
            // A recusa vem do Core e é mostrada tal como veio: um código
            // repetido ou um nome inválido têm de ser legíveis a quem escreveu.
            let viewer = viewer(&state, &member).await;
            let trail = vec![Crumb::to(Screen::Units)];
            shell_page(
                "Nova Unidade",
                &viewer,
                Screen::Units,
                trail,
                ui::screens::lists::new_unit(Some(failure.to_string())),
            )
        }
    }
}

async fn new_idea_form(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    let member = member_or_login!(state, headers);
    let viewer = viewer(&state, &member).await;
    let units = optional(&state, &member, "/api/v1/units").await;

    let trail = vec![Crumb::to(Screen::Ideas)];
    shell_page(
        "Nova Ideia",
        &viewer,
        Screen::Ideas,
        trail,
        ui::screens::lists::new_idea(&units, None),
    )
}

#[derive(Deserialize)]
struct NewIdeaForm {
    unit_id: Uuid,
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
    classification: String,
}

fn blank_to_none(value: String) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

/// Cria uma ideia.
///
/// Uma submissão de outra origem chega sem sessão, porque o cookie é
/// `SameSite=Lax`, e é encaminhada para o login em vez de ser executada.
async fn create_idea(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<NewIdeaForm>,
) -> Response {
    let member = member_or_login!(state, headers);

    let keywords: Vec<String> = form
        .keywords
        .split(',')
        .map(str::trim)
        .filter(|k| !k.is_empty())
        .map(ToOwned::to_owned)
        .collect();

    let mut body = serde_json::json!({
        "unit_id": form.unit_id,
        "title": form.title,
        "summary": blank_to_none(form.summary),
        "research_question": blank_to_none(form.research_question),
        "hypothesis": blank_to_none(form.hypothesis),
        "motivation": blank_to_none(form.motivation),
        "keywords": keywords,
    });
    if let Some(classification) = blank_to_none(form.classification) {
        body["classification"] = Value::String(classification);
    }

    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        "/api/v1/ideas",
        &body,
    )
    .await
    {
        Ok(created) => {
            let workspace_id = created
                .get("workspace")
                .and_then(|w| w.get("id"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            Redirect::to(&format!("/workspaces/{workspace_id}")).into_response()
        }
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        Err(failure) => {
            // O formulário volta com a mensagem do próprio Core: o membro vê
            // porque foi recusado, não uma falha genérica.
            let viewer = viewer(&state, &member).await;
            let units = optional(&state, &member, "/api/v1/units").await;
            let trail = vec![Crumb::to(Screen::Ideas)];
            shell_page(
                "Nova Ideia",
                &viewer,
                Screen::Ideas,
                trail,
                ui::screens::lists::new_idea(&units, Some(failure.to_string())),
            )
        }
    }
}

// ── Autenticação ─────────────────────────────────────────────────────────

/// Mostra o ecrã de início de sessão.
///
/// Sob o ADR-0103 o Workspace deixou de encaminhar para um fornecedor externo:
/// apresenta o formulário e envia as credenciais ao Ocinye Core, que é a
/// autoridade de autenticação. O Workspace nunca vê um verificador nem decide
/// se alguém entra.
async fn login(State(state): State<WorkspaceState>) -> Response {
    // O Core respondeu não significa que o Core está pronto.
    //
    // Isto lia `core_ready(...).is_ok()` — sucesso de transporte. Um `/ready`
    // que responde 503 a dizer que a persistência caiu chegava aqui como
    // «operacional», e o formulário de entrada convidava alguém a autenticar-se
    // num sistema que não podia autenticar ninguém.
    //
    // O que decide é o corpo.
    let ready = crate::boot::probe(&state).await.state.may_hand_off();
    page("Entrar", ui::screens::login::login(ready, None))
}

/// Credenciais submetidas pelo formulário.
#[derive(Deserialize)]
struct LoginForm {
    #[serde(default)]
    email: String,
    #[serde(default)]
    password: String,
}

/// Recebe o formulário e pede ao Core que autentique.
async fn login_submit(
    State(state): State<WorkspaceState>,
    Form(form): Form<LoginForm>,
) -> Response {
    let correlation_id = Uuid::new_v4().to_string();

    let outcome = api::post_unauthenticated(
        &state,
        &correlation_id,
        "/api/v1/auth/login",
        &serde_json::json!({
            "email": form.email,
            "password": form.password,
        }),
    )
    .await;

    let session = match outcome.and_then(CoreSession::from_payload) {
        Ok(session) => session,
        Err(failure) => {
            // A mensagem vem do Core e é a mesma para todas as falhas de
            // credencial. O Workspace não a enriquece: fazê-lo reintroduziria o
            // oráculo que o Core evita (briefing §35).
            let ready = crate::boot::probe(&state).await.state.may_hand_off();
            return (
                StatusCode::UNAUTHORIZED,
                page(
                    "Entrar",
                    ui::screens::login::login(ready, Some(failure.to_string())),
                ),
            )
                .into_response();
        }
    };

    let ttl = if session.must_change_password {
        // Curta, tal como a do Core: existe para completar uma tarefa.
        Duration::from_secs(30 * 60)
    } else {
        state.config.session_ttl
    };

    let session_id = state.sessions.create(Session {
        access_token: session.token,
        display_name: session.display_name,
        email: form.email.clone(),
        must_change_password: session.must_change_password,
        expires_at: Instant::now() + ttl,
    });

    let destination = if session.must_change_password {
        "/first-access"
    } else {
        "/"
    };

    (
        StatusCode::SEE_OTHER,
        [
            (header::LOCATION, destination.to_owned()),
            (
                header::SET_COOKIE,
                session::cookie_header(&session_id, state.config.cookie_secure, ttl),
            ),
        ],
    )
        .into_response()
}

/// A sessão devolvida pelo Core.
struct CoreSession {
    token: String,
    display_name: String,
    must_change_password: bool,
}

impl CoreSession {
    /// Lê a resposta do Core.
    ///
    /// Uma resposta sem token é tratada como falha e não como sessão anónima:
    /// prosseguir sem token deixaria o membro num Workspace que falha em cada
    /// chamada seguinte, sem dizer porquê.
    fn from_payload(payload: Value) -> Result<Self, ApiFailure> {
        let token = payload
            .get("session_token")
            .and_then(Value::as_str)
            .filter(|token| !token.is_empty())
            .ok_or_else(|| ApiFailure::Failed("O Ocinye Core não devolveu uma sessão.".to_owned()))?
            .to_owned();

        Ok(Self {
            token,
            display_name: payload
                .get("display_name")
                .and_then(Value::as_str)
                .unwrap_or("Membro")
                .to_owned(),
            must_change_password: payload
                .get("must_change_password")
                .and_then(Value::as_bool)
                // Sem o campo, assume-se que falta mudar: falhar fechado.
                .unwrap_or(true),
        })
    }
}

/// Ecrã de primeiro acesso: definir a palavra-passe definitiva.
async fn first_access(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    let Some(member) = current_member(&state, &headers) else {
        return Redirect::to("/login").into_response();
    };

    // Quem já tem palavra-passe definitiva não tem nada a fazer aqui.
    if !member.session.must_change_password {
        return Redirect::to("/").into_response();
    }

    page(
        "Defina a sua palavra-passe",
        ui::screens::first_access::first_access(
            &member.session.display_name,
            &member.session.email,
            None,
        ),
    )
}

/// Nova palavra-passe submetida.
#[derive(Deserialize)]
struct PasswordForm {
    #[serde(default)]
    password: String,
    #[serde(default)]
    confirmation: String,
}

/// Envia a nova palavra-passe ao Core e roda a sessão local.
async fn first_access_submit(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<PasswordForm>,
) -> Response {
    let Some(member) = current_member(&state, &headers) else {
        return Redirect::to("/login").into_response();
    };

    let outcome = api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        "/api/v1/auth/password",
        &serde_json::json!({
            "password": form.password,
            "confirmation": form.confirmation,
        }),
    )
    .await;

    let session = match outcome.and_then(CoreSession::from_payload) {
        Ok(session) => session,
        Err(failure) => {
            // A sessão restrita expirou a meio: recomeçar é o único caminho.
            if matches!(failure, ApiFailure::Unauthorised) {
                return Redirect::to("/login").into_response();
            }
            let message = failure.to_string();
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                page(
                    "Defina a sua palavra-passe",
                    ui::screens::first_access::first_access(
                        &member.session.display_name,
                        &member.session.email,
                        Some(message),
                    ),
                ),
            )
                .into_response();
        }
    };

    // O Core revogou a sessão antiga e emitiu outra. A sessão local segue-a:
    // manter a antiga deixaria o Workspace a apontar para um token morto.
    if let Some(id) = session::session_id_from_cookies(
        headers
            .get(header::COOKIE)
            .and_then(|value| value.to_str().ok()),
    ) {
        state.sessions.remove(&id);
    }

    let session_id = state.sessions.create(Session {
        access_token: session.token,
        display_name: session.display_name,
        // O nome de utilizador não muda ao trocar a palavra-passe: vem da
        // sessão anterior, porque este formulário não o pede nem o deveria
        // pedir.
        email: member.session.email.clone(),
        must_change_password: false,
        expires_at: Instant::now() + state.config.session_ttl,
    });

    (
        StatusCode::SEE_OTHER,
        [
            (header::LOCATION, "/".to_owned()),
            (
                header::SET_COOKIE,
                session::cookie_header(
                    &session_id,
                    state.config.cookie_secure,
                    state.config.session_ttl,
                ),
            ),
        ],
    )
        .into_response()
}

/// Termina a sessão.
///
/// Só por `POST`: uma sessão não deve poder ser encerrada por um `GET` que
/// alguém consiga provocar a partir de outra página.
async fn logout(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    let member = current_member(&state, &headers);
    let cookie = headers
        .get(header::COOKIE)
        .and_then(|value| value.to_str().ok());
    if let Some(id) = session::session_id_from_cookies(cookie) {
        state.sessions.remove(&id);
    }

    // Diz ao Core para revogar a sessão do lado dele. Se falhar, a sessão local
    // desaparece na mesma: o pior caso é uma sessão órfã que expira sozinha.
    if let Some(member) = member.as_ref() {
        let _ = api::post(
            &state,
            &member.session.access_token,
            &member.correlation_id,
            "/api/v1/auth/logout",
            &serde_json::json!({}),
        )
        .await;
    }

    let destination = "/login".to_owned();

    (
        StatusCode::SEE_OTHER,
        [
            (header::LOCATION, destination),
            (
                header::SET_COOKIE,
                session::clear_cookie_header(state.config.cookie_secure),
            ),
        ],
    )
        .into_response()
}

#[cfg(test)]
mod csrf_tests {
    use super::*;

    const PUBLIC: &str = "https://workspace.ocinye.com";

    #[test]
    fn a_sibling_subdomain_is_not_this_origin() {
        // O caso que `SameSite` não cobre: `ocinye.com` é *same-site* com
        // `workspace.ocinye.com`, por isso o cookie viajaria com o pedido.
        for hostile in [
            "https://ocinye.com",
            "https://www.ocinye.com",
            "http://workspace.ocinye.com",
            "https://workspace.ocinye.com.evil.example",
            "https://evil.example",
            "null",
            "",
        ] {
            assert!(
                !origin_is_ours(Some(hostile), Some("workspace.ocinye.com"), PUBLIC),
                "{hostile:?} foi aceite como sendo o Ocinye Workspace"
            );
        }
    }

    #[test]
    fn an_absent_origin_is_refused_on_a_write() {
        // Os browsers enviam `Origin` em todos os `POST`. A sua ausência num
        // pedido que altera estado não é o funcionamento normal de um.
        assert!(!origin_is_ours(None, Some("workspace.ocinye.com"), PUBLIC));
    }

    #[test]
    fn the_configured_public_origin_is_accepted() {
        assert!(origin_is_ours(
            Some(PUBLIC),
            Some("workspace.ocinye.com"),
            PUBLIC
        ));
        assert!(origin_is_ours(
            Some("https://workspace.ocinye.com/"),
            None,
            PUBLIC
        ));
    }

    #[test]
    fn an_origin_matching_the_request_host_is_accepted() {
        // Desenvolvimento local: o mesmo processo responde em `localhost` e em
        // `127.0.0.1`, e o `Host` é preenchido pelo browser com o alvo real.
        assert!(origin_is_ours(
            Some("http://127.0.0.1:8090"),
            Some("127.0.0.1:8090"),
            "http://localhost:8090"
        ));
        assert!(!origin_is_ours(
            Some("http://127.0.0.1:8090"),
            Some("localhost:8090"),
            "http://localhost:8090"
        ));
    }

    #[test]
    fn only_state_changing_methods_are_checked() {
        assert!(!changes_state(&axum::http::Method::GET));
        assert!(!changes_state(&axum::http::Method::HEAD));
        assert!(changes_state(&axum::http::Method::POST));
        assert!(changes_state(&axum::http::Method::DELETE));
        assert!(changes_state(&axum::http::Method::PATCH));
        assert!(changes_state(&axum::http::Method::PUT));
    }
}

#[cfg(test)]
mod router_tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Method, Request};
    use tower::ServiceExt;

    /// A origem pública desta instalação, tal como a guarda de mesma origem a
    /// compara.
    const PUBLIC: &str = "https://workspace.ocinye.com";

    /// O estado que a sonda dá ao `Router`.
    ///
    /// Nenhum pedido chega ao Core: sem cookie de sessão os handlers reencaminham
    /// para o login antes de lá tocarem. O cliente HTTP existe porque o estado o
    /// exige, e fica sem uso.
    fn state() -> WorkspaceState {
        WorkspaceState {
            config: std::sync::Arc::new(crate::config::WorkspaceConfig {
                bind_address: "127.0.0.1:0".to_owned(),
                public_url: PUBLIC.to_owned(),
                core_url: "http://127.0.0.1:1".to_owned(),
                session_ttl: std::time::Duration::from_secs(3600),
                cookie_secure: false,
                log_level: "error".to_owned(),
                log_format: "pretty".to_owned(),
                is_production: false,
                static_dir: format!("{}/static", env!("CARGO_MANIFEST_DIR")),
            }),
            sessions: session::SessionStore::new(),
            http: reqwest::Client::new(),
        }
    }

    /// Um código que nenhum handler do Workspace devolve.
    ///
    /// É esta a peça que separa as duas perguntas. `404` responde às duas ao
    /// mesmo tempo — «esta rota não existe» e «esta rota existe e o recurso
    /// não» — e por isso não responde a nenhuma. Trocado o fallback do router
    /// por um código impossível, `418` passa a significar exactamente uma
    /// coisa: **o router não reconheceu o caminho**.
    const SENTINELA: StatusCode = StatusCode::IM_A_TEAPOT;

    /// O `Router` real da aplicação, com o fallback trocado pela sentinela.
    ///
    /// Não é uma segunda tabela de rotas nem uma reconstrução: é
    /// `routes::router`, o mesmo que o `main` monta. A única diferença é para
    /// onde vai um caminho que ele não reconhece.
    fn probe_router() -> Router {
        router(state()).fallback(|| async { SENTINELA })
    }

    /// Um UUID válido que não corresponde a nada.
    ///
    /// Serve para concretizar `{id}`. O handler há-de não encontrar o recurso —
    /// e não encontrar é a resposta certa: prova que o handler foi alcançado.
    const NADA: &str = "00000000-0000-0000-0000-000000000000";

    /// Substitui os parâmetros de um padrão por um UUID válido.
    fn concretizar(rota: &str) -> String {
        rota.split('/')
            .map(|segmento| {
                if segmento.starts_with('{') {
                    NADA
                } else {
                    segmento
                }
            })
            .collect::<Vec<_>>()
            .join("/")
    }

    /// Leva um pedido ao `Router` real e devolve o estado da resposta.
    async fn probe(method: Method, path: &str) -> StatusCode {
        let mut request = Request::builder()
            .method(method.clone())
            .uri(path)
            .header(header::HOST, "workspace.ocinye.com");

        // Um `POST` sem `Origin` é recusado pela guarda de mesma origem antes de
        // chegar à rota, e a sonda ficaria a medir a guarda em vez do router.
        if method != Method::GET {
            request = request.header(header::ORIGIN, PUBLIC);
        }

        // E o marcador de arranque, pela mesma razão: sem ele o portão
        // encaminharia para `/boot` e a sonda mediria o portão em vez das rotas
        // que existe para inventariar.
        request = request.header(header::COOKIE, "oc_boot=1");

        probe_router()
            .oneshot(request.body(Body::empty()).expect("pedido"))
            .await
            .expect("resposta")
            .status()
    }

    /// Todas as rotas do contrato existem no `Router` que a aplicação constrói.
    ///
    /// # O que esta prova mudou
    ///
    /// `ROUTES` era uma cópia à mão do router, e nada as ligava. Uma rota
    /// removida do `Router` e esquecida em `ROUTES` deixava a varredura de
    /// ligações mortas verde enquanto a ligação passava a dar 404 — a auditoria
    /// afirmava um servidor que já não existia.
    ///
    /// Agora `ROUTES` deixa de significar «acreditamos que o servidor conhece
    /// isto» e passa a significar «este é o catálogo do contrato de navegação, e
    /// cada entrada é verificada contra o `Router` real».
    #[tokio::test]
    async fn cada_rota_do_contrato_existe_no_router_real() {
        let mut ausentes: Vec<String> = Vec::new();

        for rota in ROUTES {
            let caminho = concretizar(rota);
            // O contrato é de navegação: percorre-se com `GET`. As operações
            // que só aceitam `POST` respondem `405`, que é o router a
            // reconhecer o caminho — e é isso que aqui se mede.
            let estado = probe(Method::GET, &caminho).await;
            if estado == SENTINELA {
                ausentes.push(format!("{rota} (sondado como {caminho})"));
            }
        }

        assert!(
            ausentes.is_empty(),
            "rotas no contrato que o Router real não reconhece:\n  {}",
            ausentes.join("\n  "),
        );
    }

    /// A sentinela distingue rota ausente de recurso ausente.
    ///
    /// # O controlo positivo
    ///
    /// Sem ele, o teste acima poderia estar a medir outra coisa: se tudo
    /// respondesse `418`, ou se nada respondesse, passaria à mesma.
    ///
    /// A primeira asserção é a que faz o trabalho, e não é decorativa — foi
    /// escrita depois de uma reversão *não* ter falhado. Trocada a sentinela por
    /// `404` e retirado o fallback próprio, o teste continuava verde: as sondas
    /// correm sem sessão, e uma rota conhecida reencaminha para o login antes de
    /// chegar ao seu próprio «não encontrado». As duas respostas nunca chegavam
    /// a colidir nesta fixture, e por isso a sonda não estava a provar que sabia
    /// distingui-las.
    ///
    /// Fixar `SENTINELA != NOT_FOUND` prova a propriedade directamente, e não
    /// por acaso do estado da autenticação: o que a sonda observa é o fallback
    /// que ela própria instalou, e nunca um `404` vindo de outro sítio.
    #[tokio::test]
    async fn a_sentinela_separa_rota_ausente_de_recurso_ausente() {
        assert_ne!(
            SENTINELA,
            StatusCode::NOT_FOUND,
            "a sentinela confundiu-se com o `404` que os handlers também devolvem"
        );

        // Um caminho que o router não conhece.
        assert_eq!(
            probe(Method::GET, "/unidades-antigas").await,
            SENTINELA,
            "um caminho inexistente devia cair no fallback da sonda"
        );

        // Uma rota real com um recurso que não existe: o handler é alcançado.
        let estado = probe(Method::GET, &format!("/units/{NADA}")).await;
        assert_ne!(
            estado, SENTINELA,
            "uma rota real caiu no fallback: a sonda está a medir a coisa errada"
        );

        // E o fallback verdadeiro da aplicação continua a ser o ecrã de 404,
        // que é o que um visitante vê. A sentinela existe só dentro da sonda.
        // Com o marcador de arranque: isto mede encaminhamento, e uma pessoa
        // que chega a uma rota inexistente já passou pelo arranque. Sem ele o
        // portão encaminharia para `/boot` e o teste mediria o portão.
        let aplicacao = router(state())
            .oneshot(
                Request::builder()
                    .uri("/unidades-antigas")
                    .header(header::HOST, "workspace.ocinye.com")
                    .header(header::COOKIE, "oc_boot=1")
                    .body(Body::empty())
                    .expect("pedido"),
            )
            .await
            .expect("resposta");
        assert_eq!(aplicacao.status(), StatusCode::NOT_FOUND);
    }

    /// As operações existem no método que os formulários usam.
    ///
    /// Uma acção não fica provada por existir um `GET` com o mesmo caminho.
    /// `Terminar sessão` submete `POST /logout`, e é o `POST` que tem de ser
    /// reconhecido — um `GET /logout` seria outra coisa, e uma que não
    /// queremos que exista.
    #[tokio::test]
    async fn as_operacoes_existem_no_metodo_que_os_formularios_usam() {
        for (metodo, caminho) in [
            (Method::POST, "/logout".to_owned()),
            (Method::POST, "/units/new".to_owned()),
            (Method::POST, "/ideas/new".to_owned()),
            (Method::POST, "/projects/new".to_owned()),
            (Method::POST, "/bibliography/new".to_owned()),
            (Method::POST, "/datasets/new".to_owned()),
            (Method::POST, "/settings/password".to_owned()),
            (Method::POST, format!("/settings/sessions/{NADA}/revoke")),
            (Method::POST, "/login".to_owned()),
        ] {
            let estado = probe(metodo.clone(), &caminho).await;
            assert_ne!(
                estado, SENTINELA,
                "{metodo} {caminho} não é reconhecido pelo Router real"
            );
            assert_ne!(
                estado,
                StatusCode::METHOD_NOT_ALLOWED,
                "{metodo} {caminho} existe como caminho mas não neste método"
            );
        }

        // O inverso importa tanto: encerrar uma sessão não pode ser provocável
        // por um `GET` que alguém consiga fazer o browser emitir.
        assert_eq!(
            probe(Method::GET, "/logout").await,
            StatusCode::METHOD_NOT_ALLOWED,
            "`/logout` passou a aceitar `GET`"
        );
    }
    /// Cada formulário renderizado submete para uma rota que existe, no método
    /// que ele próprio declara.
    ///
    /// A varredura de ligações mortas cobre os `href`. Não cobria os `action`,
    /// e é aí que vivem as operações: criar uma unidade, mudar a palavra-passe,
    /// revogar uma sessão, terminar sessão. Um `action` para uma rota ausente
    /// não se nota a olho — o botão carrega, a página recarrega, e nada
    /// acontece.
    ///
    /// O método conta. Uma acção não fica provada por existir um `GET` com o
    /// mesmo caminho: `POST /logout` e `GET /logout` são operações diferentes,
    /// e só uma delas devia existir.
    #[tokio::test]
    async fn cada_formulario_renderizado_submete_para_uma_rota_real() {
        let mut mortas: Vec<String> = Vec::new();

        for (ecra, html) in crate::ui::link_tests::catalogue() {
            for pedaco in html.split("<form").skip(1) {
                let abertura = pedaco.split('>').next().unwrap_or_default();

                let Some(action) = atributo(abertura, "action") else {
                    continue;
                };
                // Formulários de pesquisa e destinos externos ficam de fora: o
                // primeiro é navegação, e o segundo não é nosso.
                if !action.starts_with('/') || action.starts_with("//") {
                    continue;
                }

                let metodo = match atributo(abertura, "method").as_deref() {
                    Some("post") | Some("POST") => Method::POST,
                    _ => Method::GET,
                };
                let caminho = action.split('?').next().unwrap_or(&action).to_owned();
                let caminho = concretizar(&caminho);

                let estado = probe(metodo.clone(), &caminho).await;
                if estado == SENTINELA {
                    mortas.push(format!("{ecra}: {metodo} {caminho} — rota inexistente"));
                } else if estado == StatusCode::METHOD_NOT_ALLOWED {
                    mortas.push(format!("{ecra}: {metodo} {caminho} — método não aceite"));
                }
            }
        }

        assert!(
            mortas.is_empty(),
            "formulários que submetem para lado nenhum:\n  {}",
            mortas.join("\n  "),
        );
    }

    /// Lê um atributo de uma abertura de etiqueta.
    fn atributo(abertura: &str, nome: &str) -> Option<String> {
        abertura
            .split(&format!("{nome}=\""))
            .nth(1)
            .and_then(|resto| resto.split('"').next())
            .map(str::to_owned)
    }
}

#[cfg(test)]
mod pagination_tests {
    use super::*;

    /// A página junta-se ao caminho sem o partir.
    #[test]
    fn a_pagina_junta_se_ao_caminho_do_core() {
        assert_eq!(com_pagina("/api/v1/units", None), "/api/v1/units");
        assert_eq!(com_pagina("/api/v1/units", Some(1)), "/api/v1/units");
        assert_eq!(com_pagina("/api/v1/units", Some(2)), "/api/v1/units?page=2");
        assert_eq!(
            com_pagina("/api/v1/sources?page_size=50", Some(3)),
            "/api/v1/sources?page_size=50&page=3"
        );
    }

    /// O recorte sobrevive à mudança de página.
    ///
    /// # Porque isto importa
    ///
    /// Um `?page=2` que esquecesse `mine=true` devolveria a segunda página da
    /// instituição inteira, e quem a lesse concluiria que participa em coisas
    /// em que não participa. A paginação muda de lugar dentro de um conjunto
    /// autorizado; nunca muda o conjunto.
    #[test]
    fn o_recorte_sobrevive_a_mudanca_de_pagina() {
        let vazio = ListSlice::default();
        let minhas = ListSlice {
            mine: true,
            ..ListSlice::default()
        };
        let unidade = ListSlice {
            unit_id: Some(Uuid::nil()),
            ..ListSlice::default()
        };

        let sem = com_pagina(&workspace_list_path("idea", &vazio), Some(2));
        let com = com_pagina(&workspace_list_path("idea", &minhas), Some(2));
        let da_unidade = com_pagina(&workspace_list_path("idea", &unidade), Some(2));

        assert!(sem.contains("kind=idea") && sem.contains("page=2"));
        assert!(!sem.contains("mine=true"));

        assert!(
            com.contains("mine=true"),
            "a segunda página perdeu o recorte: {com}"
        );
        assert!(com.contains("kind=idea") && com.contains("page=2"));

        // E a unidade escolhida viaja tipada, do mesmo modo.
        assert!(
            da_unidade.contains(&format!("unit_id={}", Uuid::nil())),
            "a segunda página perdeu a unidade escolhida: {da_unidade}"
        );
    }
}

// ── Calendário ──────────────────────────────────────────────────────────
//
// O Workspace não decide o que é visível: pede o intervalo ao Core e desenha o
// que ele devolveu. Nenhuma das quatro vistas consulta nada por si — recebem
// todas o mesmo conjunto autorizado (ADR-0410).

/// Que vista e que dia.
#[derive(Deserialize, Default)]
struct CalendarQuery {
    #[serde(default)]
    view: Option<String>,
    #[serde(default)]
    on: Option<chrono::NaiveDate>,
}

/// O intervalo que uma vista precisa, a partir do dia âncora.
///
/// Calculado aqui e enviado ao Core: a vista escolhe **quanto tempo** quer ver,
/// e o Core decide **o que** dele é visível. Se a vista também decidisse o
/// segundo, teríamos quatro políticas de visibilidade.
fn calendar_range(
    view: ui::screens::calendar::CalendarView,
    anchor: chrono::NaiveDate,
    zona: ocinye_contracts::temporal::TimeZoneName,
) -> (chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>) {
    use chrono::{Datelike, Duration, TimeZone, Utc};
    use ui::screens::calendar::{month_grid_start, week_start, CalendarView};

    let inicio = match view {
        CalendarView::Day => anchor,
        CalendarView::Week => week_start(anchor),
        CalendarView::Month => month_grid_start(anchor),
        // O ano inteiro numa consulta só. O tecto do Core são 366 dias, e é
        // exactamente o que um ano pede — doze consultas mensais dariam a mesma
        // resposta doze vezes mais devagar, e trinta e uma vezes mais se
        // alguém as fizesse por dia.
        CalendarView::Year => {
            chrono::NaiveDate::from_ymd_opt(anchor.year(), 1, 1).unwrap_or(anchor)
        }
        CalendarView::Agenda => anchor,
    };
    // A meia-noite **civil**, e não a de Greenwich.
    //
    // O que se pede ao Core é o instante em que o dia começa onde a pessoa
    // está. Com a meia-noite de UTC, a primeira hora de cada dia civil a leste
    // caía fora do pedido, e as margens de doze e vinte e quatro horas abaixo
    // existiam para o tapar — tapavam o sintoma e mantinham o defeito.
    let meia_noite = |d: chrono::NaiveDate| {
        ocinye_contracts::temporal::resolve_local(d.and_hms_opt(0, 0, 0).unwrap_or_default(), zona)
            .unwrap_or_else(|_| {
                // Uma meia-noite que não existe acontece: em algumas zonas o
                // relógio salta de 23:59 para 01:00. Nesse dia o dia civil
                // começa uma hora depois, e insistir na hora que não existe
                // seria pedir um instante que nunca houve.
                Utc.from_utc_datetime(&d.and_hms_opt(0, 0, 0).unwrap_or_default())
            })
    };

    // O Ano pede exactamente o ano, e sem margens.
    //
    // # Porque é o único caso especial
    //
    // As margens — doze horas antes e vinte e quatro depois — existem para
    // apanhar o que cai nas fronteiras quando o fuso de quem marcou não é o de
    // quem olha. O Ano já está ancorado em 1 de Janeiro e acaba em 1 de Janeiro
    // seguinte: as fronteiras são exactas e a margem não acrescenta dia nenhum
    // que a grelha mostre.
    //
    // O que ela acrescentava era um erro: 366 dias de ano bissexto mais 36 horas
    // de margem são 367 dias e meio, e o Core recusa acima de 366. A vista
    // devolvia «Não foi possível ler a agenda» com um 422 por baixo — uma falha
    // de leitura que na verdade era um pedido impossível.
    if view == CalendarView::Year {
        let ano = inicio.year();
        let fim = chrono::NaiveDate::from_ymd_opt(ano + 1, 1, 1).unwrap_or(inicio);
        return (meia_noite(inicio), meia_noite(fim));
    }

    let de = meia_noite(inicio) - Duration::hours(12);
    let ate = de + Duration::days(view.span_days()) + Duration::hours(24);
    (de, ate)
}

/// Um instante RFC 3339 dentro de uma query string.
///
/// Sem dependência nova: um instante só tem dois caracteres que a query string
/// interpreta — os dois-pontos da hora e o mais do fuso.
fn escape_instant(value: &str) -> String {
    value.replace(':', "%3A").replace('+', "%2B")
}

async fn calendar_agenda(
    state: &WorkspaceState,
    member: &Member,
    de: chrono::DateTime<chrono::Utc>,
    ate: chrono::DateTime<chrono::Utc>,
) -> Result<Value, ApiFailure> {
    api::get(
        state,
        &member.session.access_token,
        &member.correlation_id,
        &format!(
            "/api/v1/calendar/agenda?from={}&to={}",
            escape_instant(&de.to_rfc3339()),
            escape_instant(&ate.to_rfc3339())
        ),
    )
    .await
}

async fn calendar_page(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(query): Query<CalendarQuery>,
) -> Response {
    use ui::screens::calendar::{calendar, items_from, CalendarPage, CalendarView};

    let member = member_or_login!(state, headers);
    let viewer = viewer(&state, &member).await;

    // O Mês é a vista por omissão: é a que responde à pergunta com que a maior
    // parte das pessoas abre um calendário — «o que tenho este mês».
    let view = CalendarView::parse(query.view.as_deref().unwrap_or("month"));
    // «Hoje» é hoje onde a pessoa está, e não em Greenwich.
    //
    // Com `Utc::now().date_naive()`, quem abrisse o Calendário às 00:30 em
    // Lisboa via o dia anterior — e o compromisso que tinha acabado de marcar
    // para «hoje» não estava lá.
    let anchor = query
        .on
        .unwrap_or_else(|| ui::tempo::hoje_civil(chrono::Utc::now(), member.zona));
    let (de, ate) = calendar_range(view, anchor, member.zona);

    // Erro e vazio não se dizem da mesma maneira. Uma consulta falhada que
    // aparecesse como «nenhuma actividade» faria alguém faltar a uma reunião.
    let (items, failure) = match calendar_agenda(&state, &member, de, ate).await {
        Ok(payload) => (items_from(&payload), None),
        Err(ApiFailure::Unauthorised) => return Redirect::to("/login").into_response(),
        Err(erro) => (Vec::new(), Some(erro.to_string())),
    };

    let trail = vec![Crumb::to(Screen::Calendar)];
    shell_page(
        "Calendário",
        &viewer,
        Screen::Calendar,
        trail,
        calendar(&CalendarPage {
            view,
            anchor,
            items: &items,
            may_create: viewer.can(ocinye_contracts::Permission::CalendarCreate),
            failure,
            zona: member.zona,
        }),
    )
}

/// O contexto temporal que o Calendário pode trazer consigo.
///
/// # Porque é um parâmetro e não estado
///
/// Uma data escolhida no Calendário é uma decisão de apresentação de quem está a
/// olhar: não pertence à instituição, não se persiste, e não sobrevive à sessão.
/// Viaja no endereço, é validada aqui, e morre quando o formulário fecha.
#[derive(serde::Deserialize)]
struct ContextoDaCriacao {
    /// O dia escolhido no Mês ou no Ano.
    on: Option<chrono::NaiveDate>,
    /// A hora escolhida numa faixa da Semana ou do Dia.
    at: Option<String>,
}

async fn new_event_form(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(contexto): Query<ContextoDaCriacao>,
) -> Response {
    let member = member_or_login!(state, headers);
    let viewer = viewer(&state, &member).await;
    let (units, workspaces, pessoas) = tokio::join!(
        optional(&state, &member, "/api/v1/units"),
        optional(&state, &member, "/api/v1/workspaces"),
        // O universo de participantes, tal como o Core o autoriza a quem marca.
        optional(&state, &member, "/api/v1/people"),
    );

    // A precedência: hora explícita, depois dia escolhido, depois agora.
    //
    // Uma data escolhida no Calendário não é substituída pela data de hoje —
    // quem carregou no dia 28 quer marcar no dia 28. O que a política de
    // omissão decide, quando só há dia, é a hora.
    let agora = chrono::Utc::now().naive_utc();
    let hora = contexto
        .at
        .as_deref()
        .and_then(|h| chrono::NaiveTime::parse_from_str(h, "%H:%M").ok());
    let proposto = ui::screens::calendar::horario_do_editor(contexto.on, hora, agora);

    let trail = vec![Crumb::to(Screen::Calendar)];
    shell_page(
        "Nova actividade",
        &viewer,
        Screen::Calendar,
        trail,
        ui::screens::calendar::event_form(
            None,
            &units,
            &workspaces,
            None,
            Some(proposto),
            &pessoas,
            member.zona,
        ),
    )
}

/// O que o formulário envia.
///
/// # Porque a hora vem sem zona
///
/// Porque a zona vem no seu próprio campo, e é o Core que junta as duas para
/// calcular o instante. Enviar um instante já convertido daria ao browser o
/// direito de decidir o que significa «14:00 em Paris».
#[derive(Deserialize)]
struct EventForm {
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    location: String,
    #[serde(default)]
    all_day: Option<String>,
    #[serde(default)]
    starts_at: String,
    #[serde(default)]
    ends_at: String,
    #[serde(default)]
    timezone: String,
    #[serde(default)]
    starts_on: String,
    #[serde(default)]
    ends_on: String,
    #[serde(default)]
    scope: String,
    #[serde(default)]
    unit_id: String,
    #[serde(default)]
    workspace_id: String,
}

impl EventForm {
    /// A ocorrência, como o Core a espera.
    ///
    /// O último dia que a pessoa escreve é **inclusivo**; a base guarda o dia
    /// seguinte, exclusivo. A conversão é nossa: ninguém deve ter de saber que
    /// um evento de 24 de Agosto se guarda como `24 → 25`.
    fn occurrence(&self) -> Result<Value, String> {
        if self.all_day.is_some() {
            let inicio = chrono::NaiveDate::parse_from_str(&self.starts_on, "%Y-%m-%d")
                .map_err(|_| "Indique o primeiro dia.".to_owned())?;
            let ultimo =
                chrono::NaiveDate::parse_from_str(&self.ends_on, "%Y-%m-%d").unwrap_or(inicio);
            let fim = ultimo
                .succ_opt()
                .ok_or_else(|| "A data de fim não é válida.".to_owned())?;
            Ok(serde_json::json!({
                "kind": "all_day",
                "starts_on": inicio,
                "ends_before": fim,
            }))
        } else {
            let limpar = |valor: &str| valor.trim().to_owned();
            if limpar(&self.starts_at).is_empty() || limpar(&self.ends_at).is_empty() {
                return Err("Indique a hora de início e de fim.".to_owned());
            }
            Ok(serde_json::json!({
                "kind": "timed",
                "starts_at": format!("{}:00", limpar(&self.starts_at)),
                "ends_at": format!("{}:00", limpar(&self.ends_at)),
                "timezone": if self.timezone.trim().is_empty() {
                    "UTC".to_owned()
                } else {
                    limpar(&self.timezone)
                },
            }))
        }
    }
}

async fn create_calendar_event(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<EventForm>,
) -> Response {
    let member = member_or_login!(state, headers);

    let ocorrencia = match form.occurrence() {
        Ok(valor) => valor,
        Err(motivo) => return event_form_error(&state, &member, None, motivo).await,
    };

    let mut body = serde_json::json!({
        "scope": if form.scope.is_empty() { "personal" } else { &form.scope },
        "title": form.title,
        "description": blank_to_none(form.description.clone()),
        "location": blank_to_none(form.location.clone()),
        "occurrence": ocorrencia,
    });
    if !form.unit_id.is_empty() && form.scope == "unit" {
        body["unit_id"] = Value::String(form.unit_id.clone());
    }
    if !form.workspace_id.is_empty() && form.scope == "research_workspace" {
        body["workspace_id"] = Value::String(form.workspace_id.clone());
    }

    match api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        "/api/v1/calendar/events",
        &body,
    )
    .await
    {
        Ok(criado) => {
            let id = criado
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            Redirect::to(&format!("/calendar/events/{id}")).into_response()
        }
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        // A mensagem é a do Core, e não uma genérica: uma hora que não existe
        // por causa da mudança de hora tem de ser dita com essas palavras.
        Err(falha) => event_form_error(&state, &member, None, falha.to_string()).await,
    }
}

async fn event_form_error(
    state: &WorkspaceState,
    member: &Member,
    editing: Option<&ui::screens::calendar::Item>,
    motivo: String,
) -> Response {
    let viewer = viewer(state, member).await;
    let (units, workspaces) = tokio::join!(
        optional(state, member, "/api/v1/units"),
        optional(state, member, "/api/v1/workspaces"),
    );
    let trail = vec![Crumb::to(Screen::Calendar)];
    shell_page(
        "Nova actividade",
        &viewer,
        Screen::Calendar,
        trail,
        ui::screens::calendar::event_form(
            editing,
            &units,
            &workspaces,
            Some(motivo),
            None,
            &serde_json::Value::Null,
            member.zona,
        ),
    )
}

async fn event_detail_page(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(event_id): Path<Uuid>,
) -> Response {
    let member = member_or_login!(state, headers);
    let viewer = viewer(&state, &member).await;

    match api::get(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/calendar/events/{event_id}"),
    )
    .await
    {
        Ok(evento) => {
            let trail = vec![Crumb::to(Screen::Calendar)];
            shell_page(
                "Actividade",
                &viewer,
                Screen::Calendar,
                trail,
                ui::screens::calendar::event_detail(
                    &evento,
                    viewer.can(ocinye_contracts::Permission::CalendarEdit),
                    member.zona,
                ),
            )
        }
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        Err(falha) => failure_response(&falha),
    }
}

async fn edit_event_form(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(event_id): Path<Uuid>,
) -> Response {
    let member = member_or_login!(state, headers);
    let viewer = viewer(&state, &member).await;

    let evento: Value = match api::get(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/calendar/events/{event_id}"),
    )
    .await
    {
        Ok(valor) => valor,
        Err(ApiFailure::Unauthorised) => return Redirect::to("/login").into_response(),
        Err(falha) => return failure_response(&falha),
    };

    let item = ui::screens::calendar::Item {
        kind: "event".to_owned(),
        id: event_id.to_string(),
        title: evento
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        all_day: evento
            .get("all_day")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        starts_at: None,
        ends_at: None,
        timezone: None,
        starts_on: None,
        ends_before: None,
        state: String::new(),
        classification: String::new(),
    };

    let (units, workspaces) = tokio::join!(
        optional(&state, &member, "/api/v1/units"),
        optional(&state, &member, "/api/v1/workspaces"),
    );
    let trail = vec![Crumb::to(Screen::Calendar)];
    shell_page(
        "Alterar actividade",
        &viewer,
        Screen::Calendar,
        trail,
        ui::screens::calendar::event_form(
            Some(&item),
            &units,
            &workspaces,
            None,
            None,
            &serde_json::Value::Null,
            member.zona,
        ),
    )
}

async fn update_calendar_event(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(event_id): Path<Uuid>,
    Form(form): Form<EventForm>,
) -> Response {
    let member = member_or_login!(state, headers);

    // Só o que `EventEdit` aceita. O âmbito, o dono, o contentor e a
    // classificação não vão daqui porque a operação não os muda — e oferecer o
    // campo daria a entender que o pedido faria alguma coisa.
    let mut body = serde_json::json!({
        "title": form.title,
        "description": blank_to_none(form.description.clone()),
        "location": blank_to_none(form.location.clone()),
    });
    if let Ok(ocorrencia) = form.occurrence() {
        body["occurrence"] = ocorrencia;
    }

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
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        Err(falha) => event_form_error(&state, &member, None, falha.to_string()).await,
    }
}

async fn cancel_calendar_event(
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
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        Err(falha) => failure_response(&falha),
    }
}

/// As notificações recentes, para o painel do sino.
///
/// # Porque uma rota própria e não a página
///
/// Porque o painel abre a pedido, e a página é um histórico. Renderizar a lista
/// em cada navegação seria pedir ao Core tudo isto a cada clique, para o
/// esconder quase sempre.
async fn notifications_recent(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    let member = member_or_login!(state, headers);
    let resposta = optional(&state, &member, "/api/v1/notifications?page_size=12").await;
    axum::Json(resposta).into_response()
}

async fn notifications_page(State(state): State<WorkspaceState>, headers: HeaderMap) -> Response {
    let member = member_or_login!(state, headers);
    let viewer = viewer(&state, &member).await;

    let (payload, failure) = match api::get(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        "/api/v1/notifications",
    )
    .await
    {
        Ok(valor) => (valor, None),
        Err(ApiFailure::Unauthorised) => return Redirect::to("/login").into_response(),
        Err(falha) => (Value::Null, Some(falha.to_string())),
    };

    let trail = vec![Crumb::to(Screen::Home)];
    shell_page(
        "Notificações",
        &viewer,
        Screen::Home,
        trail,
        ui::screens::calendar::notifications(&payload, failure),
    )
}

async fn mark_notification_read(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(notification_id): Path<Uuid>,
) -> Response {
    let member = member_or_login!(state, headers);
    let _ = api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/notifications/{notification_id}/read"),
        &serde_json::json!({}),
    )
    .await;
    Redirect::to("/notifications").into_response()
}

#[cfg(test)]
mod intervalos_do_calendario {

    /// A zona destes testes, declarada e não herdada da máquina.
    fn zona_de_teste() -> ocinye_contracts::temporal::TimeZoneName {
        "UTC".to_owned().try_into().expect("fuso conhecido")
    }
    use super::*;
    use chrono::Datelike;
    use ui::screens::calendar::CalendarView;

    /// O tecto que o Core impõe a uma consulta de agenda.
    ///
    /// Repetido aqui de propósito, e não importado: o Core é um serviço, e a
    /// Experience fala com ele por HTTP. O que este número faz é dizer, deste
    /// lado, qual é o contrato — e falhar aqui em vez de deixar a pessoa
    /// descobrir por um `422` disfarçado de «não foi possível ler a agenda».
    const TECTO_DO_CORE: i64 = 366;

    fn dia(a: i32, m: u32, d: u32) -> chrono::NaiveDate {
        chrono::NaiveDate::from_ymd_opt(a, m, d).expect("data válida")
    }

    /// Nenhuma vista pede mais tempo do que o Core aceita responder.
    ///
    /// # O defeito que isto fecha
    ///
    /// O Ano pedia `366 dias` de âmbito mais `12h` antes e `24h` depois — as
    /// margens que apanham o que cai nas fronteiras noutros fusos. São 367 dias
    /// e meio, e o Core recusa acima de 366. A vista mostrava «Não foi possível
    /// ler a agenda», que é a mensagem de uma leitura falhada, quando na verdade
    /// o pedido é que era impossível.
    ///
    /// Um ano bissexto é o caso que aperta, e por isso está aqui.
    #[test]
    fn nenhuma_vista_pede_mais_do_que_o_core_aceita() {
        let ancoras = [
            dia(2026, 1, 1),
            dia(2026, 8, 26),
            dia(2026, 12, 31),
            // Bissextos, nos dois sentidos.
            dia(2028, 1, 1),
            dia(2028, 2, 29),
            dia(2028, 12, 31),
        ];

        for vista in CalendarView::all() {
            for ancora in ancoras {
                let (de, ate) = calendar_range(vista, ancora, zona_de_teste());
                assert!(
                    ate > de,
                    "{vista:?} em {ancora}: o intervalo acaba antes de começar"
                );

                let duracao = ate - de;
                assert!(
                    duracao <= chrono::Duration::days(TECTO_DO_CORE),
                    "{vista:?} em {ancora} pede {} dias, e o Core aceita {TECTO_DO_CORE}",
                    duracao.num_days()
                );
            }
        }
    }

    /// E o Ano cobre o ano inteiro, incluindo o dia a mais dos bissextos.
    ///
    /// Caber no tecto não chega: caberia também um intervalo de um dia. O que
    /// esta vista promete é o ano, e é isso que se verifica.
    #[test]
    fn o_ano_cobre_o_ano_inteiro() {
        for (ancora, dias) in [(dia(2026, 6, 15), 365), (dia(2028, 6, 15), 366)] {
            let (de, ate) = calendar_range(CalendarView::Year, ancora, zona_de_teste());
            assert_eq!(
                (ate - de).num_days(),
                dias,
                "o ano de {} devia cobrir {dias} dias",
                ancora.year()
            );
            assert_eq!(de.date_naive(), dia(ancora.year(), 1, 1));
            assert_eq!(ate.date_naive(), dia(ancora.year() + 1, 1, 1));
        }
    }
}

// ── Ficheiros institucionais ─────────────────────────────────────────────
//
// > **Uma pasta é uma estrutura de navegação dentro de um contentor de
// > autoridade.** Arrumar não classifica, e carregar não afirma conhecimento.
//
// Nada aqui decide acesso. Cada handler leva o membro ao Core e mostra o que o
// Core deixou; um identificador escrito à mão na barra de endereço chega ao
// mesmo sítio que qualquer outro, e recebe a mesma resposta.

/// O maior corpo que o Workspace aceita num ficheiro institucional.
///
/// O mesmo limite do Core, mais o envelope multipart.
const FILE_BODY_LIMIT_BYTES: usize = 640 * 1024 * 1024 + 64 * 1024;

/// O maior conteúdo que a pré-visualização lê.
///
/// Não é o limite do ficheiro: é o limite do que faz sentido desenhar numa
/// página. Acima disto a página diz que é grande de mais, que é verdade, em vez
/// de mostrar um pedaço e deixar acreditar que é o todo.
const PREVIEW_LIMIT_BYTES: usize = 256 * 1024;

#[derive(Deserialize)]
struct FilesQuery {
    #[serde(default)]
    workspace: Option<Uuid>,
    /// A versão exacta a abrir, quando se chega por uma citação.
    ///
    /// # Porque não basta abrir o ficheiro
    ///
    /// Porque uma citação que diga «v2, p. 14» e abra a v4 mente. A resposta
    /// foi construída sobre bytes que já não são os correntes, e clicar tem de
    /// levar aos bytes que foram lidos — senão a citação é decoração.
    #[serde(default)]
    version: Option<Uuid>,
    /// O sítio dentro da versão, quando o formato tem coordenadas.
    #[serde(default)]
    page: Option<i64>,
    #[serde(default)]
    folder: Option<Uuid>,
    #[serde(default)]
    ok: Option<String>,
    #[serde(default)]
    erro: Option<String>,
}

/// Os ambientes que este membro alcança, em pares `(id, nome)`.
fn ambientes(payload: &Value) -> Vec<(String, String)> {
    payload
        .get("items")
        .and_then(Value::as_array)
        .or_else(|| payload.as_array())
        .map(|linhas| {
            linhas
                .iter()
                .filter_map(|linha| {
                    let id = linha.get("id").and_then(Value::as_str)?;
                    let nome = linha
                        .get("title")
                        .and_then(Value::as_str)
                        .unwrap_or("Ambiente");
                    Some((id.to_owned(), nome.to_owned()))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Navegar nos ficheiros de um ambiente.
async fn files_browse(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(query): Query<FilesQuery>,
) -> Response {
    let member = member_or_login!(state, headers);
    let viewer = viewer(&state, &member).await;

    let lista = optional(&state, &member, "/api/v1/workspaces?page_size=100").await;
    let workspaces = ambientes(&lista);

    // Sem ambiente indicado: a vista agregada, que é o que o módulo é.
    let Some(workspace_id) = query.workspace else {
        let tudo = optional(&state, &member, "/api/v1/files").await;
        let content = ui::screens::files::all_files(ui::screens::files::AllFilesView {
            files: tudo
                .get("items")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default(),
            total: tudo.get("total").and_then(Value::as_i64).unwrap_or(0),
            destinos: tudo
                .get("destinations")
                .and_then(Value::as_array)
                .map(|linhas| {
                    linhas
                        .iter()
                        .filter_map(|d| {
                            Some((
                                d.get("id").and_then(Value::as_str)?.to_owned(),
                                d.get("label").and_then(Value::as_str)?.to_owned(),
                            ))
                        })
                        .collect()
                })
                .unwrap_or_default(),
            notice: aviso_de(query.ok.as_deref(), query.erro.as_deref()),
        });
        return shell_page(
            "Ficheiros",
            &viewer,
            Screen::Files,
            vec![Crumb::to(Screen::Files)],
            content,
        );
    };

    let caminho = query.folder.map_or_else(
        || format!("/api/v1/workspaces/{workspace_id}/files"),
        |folder| format!("/api/v1/workspaces/{workspace_id}/files?folder={folder}"),
    );

    // A navegação é a leitura que autoriza. Se o ambiente não é alcançável, a
    // resposta é a mesma que daria um identificador inventado.
    let conteudo = match api::get::<Value>(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &caminho,
    )
    .await
    {
        Ok(conteudo) => conteudo,
        Err(failure) => return failure_response(&failure),
    };

    let nome = workspaces
        .iter()
        .find(|(id, _)| id == &workspace_id.to_string())
        .map_or_else(|| "Ambiente".to_owned(), |(_, nome)| nome.clone());

    let listagem = |chave: &str| {
        conteudo
            .get(chave)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    };

    let content = ui::screens::files::files(ui::screens::files::FilesView {
        workspaces,
        workspace_id: Some(workspace_id.to_string()),
        workspace_name: nome,
        folder_id: query.folder.map(|f| f.to_string()),
        path: listagem("path"),
        folders: listagem("folders"),
        files: listagem("files"),
        // A resposta do Core, e não a lista institucional do `/me`: o direito
        // de carregar é do ambiente, e quem gere uma unidade tem-no lá sem o ter
        // à escala da instituição.
        may_upload: conteudo
            .get("may_create")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        notice: aviso_de(query.ok.as_deref(), query.erro.as_deref()),
    });

    shell_page(
        "Ficheiros",
        &viewer,
        Screen::Files,
        vec![Crumb::to(Screen::Files)],
        content,
    )
}

/// Traduz o resultado da operação anterior numa mensagem.
///
/// Vem da barra de endereço porque a operação anterior foi um POST que
/// redireccionou, e o estado que sobrevive a um redireccionamento é o que vai
/// no caminho. O que **não** vai por aqui é conteúdo institucional: só um
/// código que esta função conhece.
fn aviso_de(ok: Option<&str>, erro: Option<&str>) -> Option<(bool, String)> {
    match (ok, erro) {
        (Some("carregado"), _) => Some((true, "Ficheiro carregado.".to_owned())),
        (Some("pasta"), _) => Some((true, "Pasta criada.".to_owned())),
        (Some("versao"), _) => Some((true, "Nova versão carregada.".to_owned())),
        (_, Some("vazio")) => Some((false, "Escolha um ficheiro antes de confirmar.".to_owned())),
        (_, Some("nome")) => Some((
            false,
            "A pasta precisa de um nome. Já existe uma pasta com esse nome aqui?".to_owned(),
        )),
        (_, Some("recusado")) => Some((
            false,
            "O Core recusou esta operação. Não tem acesso a este ambiente, \
             ou a classificação escolhida não lhe está disponível."
                .to_owned(),
        )),
        (_, Some("armazenamento")) => Some((
            false,
            "O armazenamento institucional não está a responder. O ficheiro não foi guardado."
                .to_owned(),
        )),
        _ => None,
    }
}

/// Lê o ficheiro e os campos de um multipart.
async fn ler_carregamento(
    mut multipart: Multipart,
) -> (
    Option<(String, String, Vec<u8>)>,
    std::collections::HashMap<String, String>,
) {
    let mut ficheiro = None;
    let mut campos = std::collections::HashMap::new();

    while let Ok(Some(field)) = multipart.next_field().await {
        match field.name().map(ToOwned::to_owned) {
            Some(nome) if nome == "file" => {
                let filename = field.file_name().unwrap_or("ficheiro").to_owned();
                let tipo = field
                    .content_type()
                    .unwrap_or("application/octet-stream")
                    .to_owned();
                if let Ok(bytes) = field.bytes().await {
                    ficheiro = Some((filename, tipo, bytes.to_vec()));
                }
            }
            Some(nome) => {
                if let Ok(valor) = field.text().await {
                    campos.insert(nome, valor);
                }
            }
            None => {}
        }
    }

    (ficheiro, campos)
}

/// Para onde voltar depois de uma operação, com uma mensagem.
///
/// O destino vem do formulário, mas nunca se confia nele: só se aceita um
/// caminho desta aplicação. Um `return_to` para outro sítio seria uma redirecção
/// aberta oferecida a quem soubesse construir o formulário.
fn regresso(campos: &std::collections::HashMap<String, String>, sufixo: &str) -> Response {
    let destino = campos
        .get("return_to")
        .filter(|caminho| caminho.starts_with("/files") && !caminho.starts_with("//"))
        .cloned()
        .unwrap_or_else(|| "/files".to_owned());

    let junta = if destino.contains('?') { '&' } else { '?' };
    Redirect::to(&format!("{destino}{junta}{sufixo}")).into_response()
}

/// Carrega um ficheiro institucional.
async fn files_upload(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    multipart: Multipart,
) -> Response {
    let member = member_or_login!(state, headers);
    let (ficheiro, campos) = ler_carregamento(multipart).await;

    let Some(workspace_id) = campos.get("workspace_id").cloned() else {
        return Redirect::to("/files").into_response();
    };
    let Some((nome, tipo, dados)) = ficheiro else {
        return regresso(&campos, "erro=vazio");
    };
    if dados.is_empty() {
        return regresso(&campos, "erro=vazio");
    }

    let resultado = api::upload_with_fields(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/workspaces/{workspace_id}/files"),
        nome,
        tipo,
        dados,
        vec![
            (
                "classification",
                campos.get("classification").cloned().unwrap_or_default(),
            ),
            (
                "folder_id",
                campos.get("folder_id").cloned().unwrap_or_default(),
            ),
        ],
    )
    .await;

    match resultado {
        Ok(_) => regresso(&campos, "ok=carregado"),
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        Err(ApiFailure::Unavailable(_)) => regresso(&campos, "erro=armazenamento"),
        Err(_) => regresso(&campos, "erro=recusado"),
    }
}

#[derive(Deserialize)]
struct NewFolderForm {
    workspace_id: Uuid,
    #[serde(default)]
    parent_id: String,
    name: String,
    #[serde(default)]
    return_to: String,
}

/// Cria uma pasta.
async fn files_new_folder(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Form(form): Form<NewFolderForm>,
) -> Response {
    let member = member_or_login!(state, headers);

    let mut campos = std::collections::HashMap::new();
    campos.insert("return_to".to_owned(), form.return_to);

    if form.name.trim().is_empty() {
        return regresso(&campos, "erro=nome");
    }

    let mut corpo = serde_json::json!({ "name": form.name.trim() });
    if let Ok(pai) = Uuid::parse_str(&form.parent_id) {
        corpo["parent_id"] = serde_json::json!(pai);
    }

    let workspace_id = form.workspace_id;
    let resultado = api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/workspaces/{workspace_id}/folders"),
        &corpo,
    )
    .await;

    match resultado {
        Ok(_) => regresso(&campos, "ok=pasta"),
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        // Um nome repetido entre irmãs é a recusa mais provável, e a mensagem
        // di-lo em vez de falar de restrições da base.
        Err(_) => regresso(&campos, "erro=nome"),
    }
}

/// A página de um ficheiro.
async fn file_detail(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(file_id): Path<Uuid>,
    Query(query): Query<FilesQuery>,
) -> Response {
    let member = member_or_login!(state, headers);
    let viewer = viewer(&state, &member).await;

    let file = match api::get::<Value>(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/files/{file_id}"),
    )
    .await
    {
        Ok(file) => file,
        Err(failure) => return failure_response(&failure),
    };

    let versions = optional(
        &state,
        &member,
        &format!("/api/v1/files/{file_id}/versions"),
    )
    .await
    .as_array()
    .cloned()
    .unwrap_or_default();

    // Se a chegada foi por citação, a versão citada é a que se mostra.
    //
    // Resolve-se contra a lista que o Core acabou de autorizar: um identificador
    // que não esteja aqui não é uma versão deste ficheiro, e é tratado como se
    // não tivesse sido indicado — nunca como um atalho para outro recurso.
    let citada = query.version.and_then(|pedida| {
        versions
            .iter()
            .find(|v| {
                v.get("id")
                    .and_then(Value::as_str)
                    .and_then(|id| Uuid::parse_str(id).ok())
                    == Some(pedida)
            })
            .cloned()
    });

    let mostrada = citada.clone().or_else(|| versions.first().cloned());
    let preview = previsualizar(&state, &member, mostrada.as_ref()).await;

    let citada_view = citada.as_ref().map(|v| ui::screens::files::VersaoCitada {
        sequence: v.get("sequence").and_then(Value::as_i64).unwrap_or(1),
        page: query.page,
        corrente: versions
            .first()
            .and_then(|c| c.get("id").and_then(Value::as_str))
            == v.get("id").and_then(Value::as_str),
    });

    let may_upload = file
        .get("may_write")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let extraction = match file.get("extraction_status").and_then(Value::as_str) {
        Some("AVAILABLE") => ui::screens::files::Extraccao::Pesquisavel(
            file.get("extraction_chunks")
                .and_then(Value::as_i64)
                .unwrap_or(0),
        ),
        Some("QUEUED" | "PROCESSING") => ui::screens::files::Extraccao::AProcessar,
        Some("UNSUPPORTED") => ui::screens::files::Extraccao::SemLeitor,
        Some("FAILED") => ui::screens::files::Extraccao::Falhou,
        _ => ui::screens::files::Extraccao::Nenhuma,
    };

    let content = ui::screens::files::file_detail(ui::screens::files::FileDetailView {
        file,
        versions,
        preview,
        // O que se está a ver, quando não é a versão corrente. A página tem de
        // o dizer: alguém que chegou por uma citação e vê a v2 sem aviso
        // conclui que é o estado actual do ficheiro.
        citada: citada_view,
        extraction,
        // Do Core, pela mesma razão do ecrã de navegação: o direito de
        // acrescentar uma versão é deste ficheiro, não da instituição.
        may_upload,
        notice: aviso_de(query.ok.as_deref(), query.erro.as_deref()),
    });

    shell_page(
        "Ficheiro",
        &viewer,
        Screen::Files,
        vec![Crumb::to(Screen::Files)],
        content,
    )
}

/// O que se pode honestamente mostrar do conteúdo.
///
/// Só texto, e só até um limite. Uma imagem exigiria que a `Content-Security-
/// Policy` desta aplicação — hoje `img-src 'self' data:` — passasse a aceitar o
/// host do armazenamento, que é configurável e pode ser externo. Essa é uma
/// decisão de segurança, e não uma consequência de alguém querer ver uma
/// miniatura.
async fn previsualizar(
    state: &WorkspaceState,
    member: &Member,
    versao: Option<&Value>,
) -> ui::screens::files::Preview {
    use ui::screens::files::Preview;

    let Some(corrente) = versao else {
        return Preview::Unavailable("Este ficheiro ainda não tem versões.".to_owned());
    };

    // A pré-visualização é **da versão que se está a ver**, e não da corrente.
    // Uma citação que abra a v2 e mostre o texto da v4 é a mesma mentira, só
    // que mais difícil de notar.
    let version_id = corrente
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();

    let tipo = corrente
        .get("content_type")
        .and_then(Value::as_str)
        .unwrap_or("application/octet-stream")
        .to_owned();
    let tamanho = corrente
        .get("size_bytes")
        .and_then(Value::as_i64)
        .unwrap_or(0);

    // As imagens vêm por `/files/{id}/preview`, na origem desta aplicação. O
    // navegador pede-as sozinho, com a sessão que já tem, e o Core volta a
    // decidir — pelo que não há aqui nenhuma leitura antecipada de bytes.
    // A lista de tipos que se mostram inline é do Core, e chega como um campo.
    // O Workspace não a recalcula: seria uma segunda opinião sobre uma decisão
    // de segurança, e as duas divergiriam no dia em que uma mudasse.
    if corrente
        .get("previewable")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Preview::Image {
            src: format!("/file-versions/{version_id}/preview"),
            alt: "Pré-visualização do ficheiro".to_owned(),
        };
    }

    // O texto vem da **extracção**, e não de uma segunda leitura dos bytes.
    //
    // Antes, a pesquisa lia pelo extractor e a pré-visualização descarregava o
    // ficheiro e descodificava-o outra vez. Dois caminhos para o mesmo texto
    // divergem, e o dia em que divergissem era o dia em que alguém via no ecrã
    // uma coisa diferente da que a pesquisa tinha encontrado.
    //
    // Isto também torna um PDF pré-visualizável: o que se mostra é exactamente
    // o que se pesquisa.
    match api::get::<Value>(
        state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/file-versions/{version_id}/content"),
    )
    .await
    {
        Ok(resposta) => {
            if let Some(texto) = resposta.get("text").and_then(Value::as_str) {
                if tamanho > PREVIEW_LIMIT_BYTES as i64 {
                    return Preview::TooLarge(tamanho);
                }
                return Preview::Text(texto.to_owned());
            }
        }
        Err(_) => {
            return Preview::Unavailable(
                "O Core não autorizou a leitura do conteúdo agora.".to_owned(),
            )
        }
    }

    // Sem extracção não há texto para mostrar. Distinguir «ainda não foi lido»
    // de «não tem leitor» é trabalho do painel de estado ao lado, que já o faz.
    let e_texto = tipo.starts_with("text/")
        || matches!(
            tipo.as_str(),
            "application/json" | "application/xml" | "application/x-yaml" | "application/yaml"
        );

    // Sem extracção não há texto, e não se vai buscar os bytes para tentar
    // outra vez: era esse o segundo caminho, e é ele que desaparece aqui.
    // Porque não há texto — ainda não foi lido, ou não tem leitor — di-lo o
    // painel de estado ao lado, que sabe distinguir as duas coisas.
    let _ = (e_texto, tamanho);
    Preview::UnsupportedType(tipo)
}

/// Carrega uma versão nova de um ficheiro que já existe.
async fn file_new_version(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(file_id): Path<Uuid>,
    multipart: Multipart,
) -> Response {
    let member = member_or_login!(state, headers);
    let (ficheiro, _) = ler_carregamento(multipart).await;

    let destino = format!("/files/{file_id}");
    let Some((nome, tipo, dados)) = ficheiro else {
        return Redirect::to(&format!("{destino}?erro=vazio")).into_response();
    };
    if dados.is_empty() {
        return Redirect::to(&format!("{destino}?erro=vazio")).into_response();
    }

    let resultado = api::upload_with_fields(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/files/{file_id}/versions"),
        nome,
        tipo,
        dados,
        vec![],
    )
    .await;

    match resultado {
        Ok(_) => Redirect::to(&format!("{destino}?ok=versao")).into_response(),
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        Err(ApiFailure::Unavailable(_)) => {
            Redirect::to(&format!("{destino}?erro=armazenamento")).into_response()
        }
        Err(_) => Redirect::to(&format!("{destino}?erro=recusado")).into_response(),
    }
}

/// Serve a pré-visualização na origem do Workspace.
///
/// # Porque os bytes passam por aqui
///
/// Porque a alternativa era pôr a URL do armazenamento num `<img>` e alargar a
/// `Content-Security-Policy` desta aplicação — hoje `img-src 'self' data:` — ao
/// host do object storage, que é configurável e pode ser externo.
///
/// > **A Experience não precisa de conhecer nem confiar no endpoint físico onde
/// > os bytes institucionais estão guardados.**
///
/// O tipo é o que o Core declarou, contra a lista fechada dele. Este handler
/// não o adivinha nem o corrige: repeti-lo aqui seria uma segunda opinião sobre
/// uma decisão que já foi tomada no sítio certo.
async fn file_preview(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(file_id): Path<Uuid>,
) -> Response {
    let member = member_or_login!(state, headers);

    match api::get_inline(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/files/{file_id}/preview"),
    )
    .await
    {
        Ok((tipo, bytes)) => {
            let Ok(tipo) = HeaderValue::from_str(&tipo) else {
                return StatusCode::BAD_GATEWAY.into_response();
            };
            (
                [
                    (header::CONTENT_TYPE, tipo),
                    (
                        header::CONTENT_DISPOSITION,
                        HeaderValue::from_static("inline"),
                    ),
                    (
                        header::X_CONTENT_TYPE_OPTIONS,
                        HeaderValue::from_static("nosniff"),
                    ),
                    (
                        header::CACHE_CONTROL,
                        HeaderValue::from_static("private, max-age=0, must-revalidate"),
                    ),
                ],
                bytes,
            )
                .into_response()
        }
        Err(failure) => failure_response(&failure),
    }
}

/// Serve inline a pré-visualização de uma versão exacta.
///
/// Pela mesma razão da outra: a CSP continua `img-src 'self'`, e a página nunca
/// aprende onde os bytes estão.
async fn file_version_preview(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(version_id): Path<Uuid>,
) -> Response {
    let member = member_or_login!(state, headers);

    match api::get_inline(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/file-versions/{version_id}/preview"),
    )
    .await
    {
        Ok((tipo, bytes)) => {
            let Ok(tipo) = HeaderValue::from_str(&tipo) else {
                return StatusCode::BAD_GATEWAY.into_response();
            };
            (
                [
                    (header::CONTENT_TYPE, tipo),
                    (
                        header::CONTENT_DISPOSITION,
                        HeaderValue::from_static("inline"),
                    ),
                    (
                        header::X_CONTENT_TYPE_OPTIONS,
                        HeaderValue::from_static("nosniff"),
                    ),
                    (
                        header::CACHE_CONTROL,
                        HeaderValue::from_static("private, max-age=0, must-revalidate"),
                    ),
                ],
                bytes,
            )
                .into_response()
        }
        Err(failure) => failure_response(&failure),
    }
}

/// Descarrega a versão corrente.
async fn file_download(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(file_id): Path<Uuid>,
) -> Response {
    let member = member_or_login!(state, headers);
    ligacao_assinada(
        &state,
        &member,
        &format!("/api/v1/files/{file_id}/download"),
    )
    .await
}

/// Descarrega uma versão exacta.
async fn version_download(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(version_id): Path<Uuid>,
) -> Response {
    let member = member_or_login!(state, headers);
    ligacao_assinada(
        &state,
        &member,
        &format!("/api/v1/file-versions/{version_id}/download"),
    )
    .await
}

/// Pede a ligação ao Core e encaminha para lá.
///
/// A ligação não é escrita na página: é pedida no momento do clique e usada uma
/// vez. Uma URL assinada colada num `href` ficaria no histórico do browser e em
/// qualquer registo pelo caminho, e continuaria a valer depois de a pessoa
/// deixar de ter acesso.
async fn ligacao_assinada(state: &WorkspaceState, member: &Member, caminho: &str) -> Response {
    match api::get::<Value>(
        state,
        &member.session.access_token,
        &member.correlation_id,
        caminho,
    )
    .await
    {
        Ok(resposta) => resposta.get("url").and_then(Value::as_str).map_or_else(
            || failure_response(&ApiFailure::Unavailable(None)),
            |url| Redirect::to(url).into_response(),
        ),
        Err(failure) => failure_response(&failure),
    }
}

// ── Gestão de pertenças ─────────────────────────────────────────────────
//
// Uma pertença **é** autoridade. Acrescentar alguém a uma unidade concede-lhe
// direitos sobre o que lá está; retirá-lo tira-lhos. Nada aqui decide: cada
// operação leva o membro ao Core, que volta a autorizar contra o contentor
// concreto — e recusa a quem tente por HTTP directo o que a interface não lhe
// ofereceu.

#[derive(Deserialize)]
struct MembroDaUnidade {
    person_id: Uuid,
    #[serde(default)]
    role: String,
}

/// O resultado da última operação, de volta pelo endereço.
///
/// Uma struct própria e não `FilesQuery`: o ecrã da unidade reaproveita-a, mas
/// os seus campos são de ficheiros — versão, página, pasta — e nada disso tem
/// significado numa alteração de pertença.
#[derive(Deserialize)]
struct AvisoQuery {
    #[serde(default)]
    ok: Option<String>,
    #[serde(default)]
    erro: Option<String>,
}

/// Traduz o resultado de uma alteração de participação num ambiente.
fn aviso_de_participacao(ok: Option<&str>, erro: Option<&str>) -> Option<(bool, String)> {
    match (ok, erro) {
        (Some("adicionado"), _) => Some((true, "Pessoa adicionada ao ambiente.".to_owned())),
        (Some("removido"), _) => Some((true, "Pessoa removida do ambiente.".to_owned())),
        (_, Some("autoridade")) => Some((
            false,
            "Não tem autoridade para gerir quem participa neste ambiente.".to_owned(),
        )),
        // Um ambiente sem ninguém que o lidere fica ingovernável, e a recusa
        // que o impede merece a sua própria mensagem: quem a lê tem de
        // perceber o que fazer a seguir.
        (_, Some("ultimo")) => Some((
            false,
            "Esta é a última pessoa que lidera o ambiente. Nomeie outro líder \
             antes de a remover."
                .to_owned(),
        )),
        (_, Some(_)) => Some((
            false,
            "A alteração não foi aceite pelo Ocinye Core.".to_owned(),
        )),
        _ => None,
    }
}

/// Traduz o resultado de uma alteração de pertença numa mensagem.
fn aviso_de_pertenca(ok: Option<&str>, erro: Option<&str>) -> Option<(bool, String)> {
    match (ok, erro) {
        (Some("adicionado"), _) => Some((true, "Pessoa adicionada à unidade.".to_owned())),
        (Some("papel"), _) => Some((true, "Papel alterado.".to_owned())),
        (Some("removido"), _) => Some((true, "Pessoa removida da unidade.".to_owned())),
        (_, Some("autoridade")) => Some((
            false,
            "Não tem autoridade para gerir quem pertence a esta unidade.".to_owned(),
        )),
        // A recusa que protege a unidade de ficar ingovernável merece a sua
        // própria mensagem: quem a lê tem de perceber o que fazer a seguir.
        (_, Some("ultimo")) => Some((
            false,
            "Esta é a última pessoa que gere a unidade. Nomeie outro gestor \
             antes de a remover."
                .to_owned(),
        )),
        (_, Some(_)) => Some((
            false,
            "A alteração não foi aceite pelo Ocinye Core.".to_owned(),
        )),
        _ => None,
    }
}

fn de_volta_ao_ambiente(workspace_id: Uuid, sufixo: &str) -> Response {
    Redirect::to(&format!("/workspaces/{workspace_id}?{sufixo}")).into_response()
}

fn de_volta_a_unidade(unit_id: Uuid, sufixo: &str) -> Response {
    Redirect::to(&format!("/units/{unit_id}?{sufixo}")).into_response()
}

/// Traduz a recusa do Core no motivo que a interface mostra.
fn motivo_da_recusa(failure: &ApiFailure) -> &'static str {
    match failure {
        ApiFailure::Forbidden | ApiFailure::Denied => "autoridade",
        // O Core devolve conflito quando a operação deixaria a unidade sem
        // ninguém que a governe.
        ApiFailure::Failed(mensagem) if mensagem.contains("409") => "ultimo",
        _ => "recusado",
    }
}

/// Acrescentar alguém ao Research Workspace.
///
/// A operação vai ao Core pelo mesmo caminho que o ecrã usou para decidir se
/// mostrava o formulário — e o Core decide outra vez. A ausência do controlo
/// nunca foi a defesa.
async fn workspace_member_add(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(workspace_id): Path<Uuid>,
    Form(form): Form<MembroDaUnidade>,
) -> Response {
    let member = member_or_login!(state, headers);

    let resultado = api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/workspaces/{workspace_id}/members"),
        &serde_json::json!({
            "person_id": form.person_id,
            "role": if form.role.is_empty() { "member" } else { &form.role },
        }),
    )
    .await;

    match resultado {
        Ok(_) => de_volta_ao_ambiente(workspace_id, "ok=adicionado"),
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        Err(falha) => {
            de_volta_ao_ambiente(workspace_id, &format!("erro={}", motivo_da_recusa(&falha)))
        }
    }
}

async fn workspace_member_remove(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(workspace_id): Path<Uuid>,
    Form(form): Form<MembroDaUnidade>,
) -> Response {
    let member = member_or_login!(state, headers);
    let person_id = form.person_id;

    let resultado = api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/workspaces/{workspace_id}/members/{person_id}"),
        &serde_json::json!({}),
    )
    .await;

    match resultado {
        Ok(_) => de_volta_ao_ambiente(workspace_id, "ok=removido"),
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        Err(falha) => {
            de_volta_ao_ambiente(workspace_id, &format!("erro={}", motivo_da_recusa(&falha)))
        }
    }
}

async fn unit_member_add(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(unit_id): Path<Uuid>,
    Form(form): Form<MembroDaUnidade>,
) -> Response {
    let member = member_or_login!(state, headers);

    let resultado = api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/units/{unit_id}/members"),
        &serde_json::json!({
            "person_id": form.person_id,
            "role": if form.role.is_empty() { "member" } else { &form.role },
        }),
    )
    .await;

    match resultado {
        Ok(_) => de_volta_a_unidade(unit_id, "ok=adicionado"),
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        Err(falha) => de_volta_a_unidade(unit_id, &format!("erro={}", motivo_da_recusa(&falha))),
    }
}

/// Alterar o papel é a mesma operação que acrescentar: o Core faz upsert.
///
/// Não há aqui um caminho de escrita paralelo — seria uma segunda autoridade
/// com outro nome.
async fn unit_member_role(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(unit_id): Path<Uuid>,
    Form(form): Form<MembroDaUnidade>,
) -> Response {
    let member = member_or_login!(state, headers);

    let resultado = api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/units/{unit_id}/members"),
        &serde_json::json!({ "person_id": form.person_id, "role": form.role }),
    )
    .await;

    match resultado {
        Ok(_) => de_volta_a_unidade(unit_id, "ok=papel"),
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        Err(falha) => de_volta_a_unidade(unit_id, &format!("erro={}", motivo_da_recusa(&falha))),
    }
}

async fn unit_member_remove(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Path(unit_id): Path<Uuid>,
    Form(form): Form<MembroDaUnidade>,
) -> Response {
    let member = member_or_login!(state, headers);
    let person_id = form.person_id;

    let resultado = api::post(
        &state,
        &member.session.access_token,
        &member.correlation_id,
        &format!("/api/v1/units/{unit_id}/members/{person_id}"),
        &serde_json::json!({}),
    )
    .await;

    match resultado {
        Ok(_) => de_volta_a_unidade(unit_id, "ok=removido"),
        Err(ApiFailure::Unauthorised) => Redirect::to("/login").into_response(),
        Err(falha) => de_volta_a_unidade(unit_id, &format!("erro={}", motivo_da_recusa(&falha))),
    }
}
