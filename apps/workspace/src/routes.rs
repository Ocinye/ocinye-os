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
    "/mail",
    "/mail/{mailbox_id}",
    "/mail/{mailbox_id}/sync",
    "/mail/message/{message_id}",
    "/mail/message/{message_id}/flags",
    "/mail/compose",
    "/mail/assist",
    "/mail/send",
    "/mail/settings",
    "/units",
    "/units/{unit_id}",
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
    "/knowledge",
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
        .route("/mail", get(mail))
        .route("/mail/compose", get(compose))
        .route("/mail/assist", post(assist))
        .route("/mail/send", post(send_mail))
        .route(
            "/mail/settings",
            get(mail_settings).post(save_mail_settings),
        )
        .route("/mail/message/{message_id}", get(mail_message))
        .route("/mail/message/{message_id}/flags", post(mail_flags))
        // Declarada depois das anteriores: `/mail/compose` tem de bater na
        // rota literal, não em `{mailbox_id}`.
        .route("/mail/{mailbox_id}", get(mail_mailbox))
        .route("/mail/{mailbox_id}/sync", post(mail_sync))
        // Investigação
        .route("/units", get(units))
        .route("/units/{unit_id}", get(unit_detail))
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
        // Conhecimento
        .route("/knowledge", get(knowledge))
        .route("/bibliography", get(bibliography))
        .route(
            "/bibliography/tools",
            get(bibliography_tools).post(review_bibliography),
        )
        .route("/datasets", get(datasets))
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

    Viewer {
        name: member.session.display_name.clone(),
        // O username vem da sessão local — é aquele com que a pessoa entrou.
        // O Core confirma-o em `/api/v1/me`; quando responde, é a resposta dele
        // que manda, porque é lá que o registo vive.
        username: me
            .get("username")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .or_else(|| {
                let entrada = member.session.username.trim();
                (!entrada.is_empty()).then(|| entrada.to_owned())
            }),
        email: me
            .get("email")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .map(str::to_owned),
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
        ApiFailure::Unavailable => (
            StatusCode::SERVICE_UNAVAILABLE,
            page("Indisponível", ui::screens::notice::unavailable()),
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
        ui::screens::mail::mail(&viewer, &view, &messages, open.as_ref()),
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

    shell_page(
        "Nova mensagem",
        &viewer,
        Screen::Mail,
        vec![Crumb::to(Screen::Mail)],
        ui::screens::mail::compose(&view, &draft),
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
    username: String,
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
        "username": form.username,
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
                    credential
                        .get("username")
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

    let trail = vec![Crumb::to(Screen::Units)];
    let content = ui::screens::workspaces::unit_detail(&unit, &members, &workspaces);

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
        });

    shell_page("Research Workspace", &viewer, screen, trail, content)
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

    let semantic = optional(&state, &member, "/api/v1/search/semantic-availability").await;

    shell_page(
        "Pesquisar",
        &viewer,
        Screen::Search,
        Vec::new(),
        ui::screens::search::search(&query.q, &results, &semantic),
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
        Err(ApiFailure::Unavailable) => avatar_error(
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
                username: member.session.username.clone(),
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
    username: String,
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
            "username": form.username,
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
        username: form.username.clone(),
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
            &member.session.username,
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
                        &member.session.username,
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
        username: member.session.username.clone(),
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
    let meia_noite =
        |d: chrono::NaiveDate| Utc.from_utc_datetime(&d.and_hms_opt(0, 0, 0).unwrap_or_default());

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
    let anchor = query.on.unwrap_or_else(|| chrono::Utc::now().date_naive());
    let (de, ate) = calendar_range(view, anchor);

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
                let (de, ate) = calendar_range(vista, ancora);
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
            let (de, ate) = calendar_range(CalendarView::Year, ancora);
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
