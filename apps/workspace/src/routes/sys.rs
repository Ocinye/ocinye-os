//! D008 · Ocinye Terminal (ocsh) e Ocinye Browser no Workspace.
//!
//! Duas fronteiras, nenhuma ponte (ADR-0623). O Terminal leva a linha ao Core
//! (`POST /api/v1/commands/exec`), que faz o parse que conta, autoriza e
//! executa; aqui só se traduz e se redige o eco. O Browser não fala com o Core
//! por conta de um site: o `<iframe>` isolado pede a página directamente ao
//! navegador da pessoa, e o Workspace só classifica o endereço pedido.
//!
//! Cada uma carrega o seu cliente **só na sua rota** (`oc-terminal.js`,
//! `oc-browser.js`), e só a resposta de `/browser` aceita molduras `https:`.
//! Por isso nenhuma das duas se desenha como corpo de uma janela de fundo
//! (`?frame=1`) dentro da página de outra aplicação: o Gestor de Janelas mostra
//! essa janela como uma ligação para o seu endereço.

use super::productivity::{open_app, AppWindow};
use super::*;
use crate::terminal as term;
use crate::ui::view_models::{
    BrowserNavigationVm, BrowserNoticeVm, BrowserNyeVm, BrowserPageStateVm, BrowserRuntime,
    BrowserSecurity, BrowserSideVm, BrowserTabState, BrowserTabVm, BrowserVm, TermBlockVm,
    TermContextVm, TermEntryVm, TermTone, TerminalVm,
};
use axum::body::Bytes;
use ocinye_contracts::ocsh::wire::ExecResponse;

/// A política de `/browser`: a de todas as páginas, com molduras `https:` — e
/// só elas: `http:` abre pelo recurso honesto (um separador do navegador).
pub(super) fn browser_csp() -> String {
    CONTENT_SECURITY_POLICY.replace("frame-src 'self'", "frame-src 'self' https:")
}

/// Uma janela de fundo destas aplicações não recebe corpo por `?frame=1`: o
/// cliente de cada uma só existe na sua rota. Sem corpo, o `wm-engine.js`
/// mostra a ligação para o endereço da janela (D004 · o recurso já desenhado).
fn frame_elsewhere() -> Response {
    (
        StatusCode::NO_CONTENT,
        [(header::CACHE_CONTROL, "no-store")],
    )
        .into_response()
}

/// A página da aplicação: a casca com a janela e o cliente desta rota.
fn page(w: &AppWindow, content: leptos::prelude::AnyView, client: &'static str) -> Response {
    let engine = w.ctx.vm.wm.is_some();
    let body = ui::shell::shell_with_window(
        &w.ctx.vm,
        leptos::prelude::IntoAny::into_any(()),
        Some(content),
    );
    shell_page_with(&w.title, engine, body, None, Some(client))
}

// ═════════════════════════════════════════════════════════════════════════
// Terminal
// ═════════════════════════════════════════════════════════════════════════

/// Uma linha ao Core, em nome do membro.
async fn exec(
    state: &WorkspaceState,
    member: &Member,
    line: &str,
    context: Option<&str>,
) -> Result<ExecResponse, ApiFailure> {
    let body = serde_json::json!({ "line": line, "context": context });
    let value = api::post(
        state,
        &member.session.access_token,
        &member.correlation_id,
        "/api/v1/commands/exec",
        &body,
    )
    .await?;
    serde_json::from_value(value).map_err(|_| ApiFailure::Unavailable(None))
}

/// A ponte explícita: a pergunta vai à Nye pelo caminho canónico, onde o Core
/// decide outra vez (permissão de IA, política, disponibilidade).
async fn ask_nye(state: &WorkspaceState, member: &Member, question: &str) -> (u8, TermBlockVm) {
    let reply = api::post(
        state,
        &member.session.access_token,
        &member.correlation_id,
        "/api/v1/ai/prompt",
        &serde_json::json!({ "prompt": question }),
    )
    .await;
    let refusal = |exit, tone, key: &str| {
        (
            exit,
            TermBlockVm::Note {
                tone,
                title: crate::i18n::t(key).to_owned(),
                body: None,
                suggestions: vec![],
            },
        )
    };
    match reply {
        Ok(r) => term::nye_reply(&r),
        Err(ApiFailure::Forbidden | ApiFailure::Denied) => {
            refusal(77, TermTone::Deny, "nye.reason.permission_denied")
        }
        Err(_) => refusal(69, TermTone::Warn, "ocsh.err.unavailable"),
    }
}

/// O Terminal: a sessão nova, ou a entrada que um envio sem JavaScript deixou.
async fn terminal_screen(
    state: &WorkspaceState,
    w: &AppWindow,
    scrollback: Vec<TermEntryVm>,
) -> Response {
    // A descoberta é a ajuda **do Core** para esta pessoa: os mesmos comandos
    // que `help` lhe mostra. Sem resposta do Core, o prompt não envia nada.
    let help = exec(state, &w.member, "help", None).await;
    let vm = TerminalVm {
        context: TermContextVm {
            id: None,
            label: crate::i18n::t("terminal.context.personal").to_owned(),
        },
        core_online: help.is_ok(),
        version: term::OCSH_VERSION,
        registry: help.as_ref().map(term::registry_of).unwrap_or_default(),
        scrollback,
    };
    page(
        w,
        ui::apps::terminal::terminal(&vm),
        "/static/oc-terminal.js",
    )
}

/// `GET /terminal` — o Ocinye Terminal (D008-A), uma sessão por janela.
pub(super) async fn terminal_page(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
) -> Response {
    let w = match open_app(&state, &headers, Screen::Terminal, ApplicationId::Terminal).await {
        Ok(w) => w,
        Err(r) => return *r,
    };
    if w.page.frame {
        return frame_elsewhere();
    }
    terminal_screen(&state, &w, vec![]).await
}

#[derive(Deserialize)]
struct PromptForm {
    line: String,
    #[serde(default)]
    context: Option<String>,
}

/// O formulário do prompt (`application/x-www-form-urlencoded`).
fn form_of(body: &[u8]) -> Option<PromptForm> {
    let mut line = None;
    let mut context = None;
    for (k, v) in url::form_urlencoded::parse(body) {
        match k.as_ref() {
            "line" => line = Some(v.into_owned()),
            "context" => context = Some(v.into_owned()),
            _ => {}
        }
    }
    Some(PromptForm {
        line: line?,
        context,
    })
}

/// `POST /terminal/exec` — leva uma linha ao Core e devolve-a localizada.
///
/// Não faz parse que conte nem decide nada: o Core faz os dois (ADR-0312 §2).
/// Com `oc-terminal.js` (JSON) responde com a forma do Design (T-06), e o
/// `echo` é a linha redigida — a única que o cliente guarda. Sem JavaScript (o
/// formulário do prompt) desenha o Terminal com a entrada.
pub(super) async fn terminal_exec(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let json = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("application/json"));
    let request: Option<PromptForm> = if json {
        serde_json::from_slice(&body).ok()
    } else {
        form_of(&body)
    };
    let Some(request) = request else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    if !json {
        return terminal_form(&state, &headers, &request).await;
    }
    let Some(member) = current_member(&state, &headers) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "sem sessão" })),
        )
            .into_response();
    };
    let echo = ocinye_contracts::ocsh::redact(&request.line);
    match exec(&state, &member, &request.line, request.context.as_deref()).await {
        Ok(r) => {
            let mut out = term::localize(&r, &echo);
            if let Some(q) = term::question(&r) {
                let (exit, block) = ask_nye(&state, &member, q).await;
                out["exit"] = exit.into();
                out["blocks"] = serde_json::json!([term::vm_json(&block)]);
            }
            Json(out).into_response()
        }
        Err(ApiFailure::Unauthorised) => (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "sem sessão" })),
        )
            .into_response(),
        Err(ApiFailure::ApplicationInactive | ApiFailure::Unavailable(_)) => {
            Json(term::transport_failure("ocsh.err.unavailable", 69, &echo)).into_response()
        }
        Err(ApiFailure::Forbidden | ApiFailure::Denied) => {
            Json(term::transport_failure("ocsh.denied", 77, &echo)).into_response()
        }
        Err(_) => Json(term::transport_failure("ocsh.err.network", 1, &echo)).into_response(),
    }
}

/// O percurso sem JavaScript: o mesmo pedido ao Core, desenhado no servidor.
async fn terminal_form(
    state: &WorkspaceState,
    headers: &HeaderMap,
    request: &PromptForm,
) -> Response {
    let w = match open_app(state, headers, Screen::Terminal, ApplicationId::Terminal).await {
        Ok(w) => w,
        Err(r) => return *r,
    };
    let echo = ocinye_contracts::ocsh::redact(&request.line);
    let entry = match exec(state, &w.member, &request.line, request.context.as_deref()).await {
        Ok(r) => {
            let mut e = term::entry(&r, echo);
            if let Some(q) = term::question(&r) {
                let (exit, block) = ask_nye(state, &w.member, q).await;
                e.exit = Some(exit);
                e.blocks = vec![block];
            }
            e
        }
        Err(ApiFailure::Unauthorised) => return session_ended(state, headers),
        Err(f) => TermEntryVm {
            echo,
            context_label: crate::i18n::t("terminal.context.personal").to_owned(),
            exit: Some(if matches!(f, ApiFailure::Forbidden | ApiFailure::Denied) {
                77
            } else {
                69
            }),
            ms: None,
            capability: None,
            blocks: vec![TermBlockVm::Note {
                tone: TermTone::Err,
                title: crate::i18n::t("ocsh.err.unavailable").to_owned(),
                body: None,
                suggestions: vec![],
            }],
        },
    };
    terminal_screen(state, &w, vec![entry]).await
}

// ═════════════════════════════════════════════════════════════════════════
// Browser
// ═════════════════════════════════════════════════════════════════════════

#[derive(Deserialize, Default)]
pub(super) struct BrowserQuery {
    #[serde(default)]
    url: Option<String>,
    /// `nye` abre o painel da Nye, que na Web diz porque não está disponível.
    #[serde(default)]
    side: Option<String>,
}

/// O que a pessoa escreveu na barra, classificado como o `classify()` do
/// `oc-browser.js`: só `http`/`https` navegam; outro esquema é bloqueado; texto
/// que não é endereço fica «Não é um endereço» (sem motor de pesquisa).
pub(super) fn classify(raw: &str) -> BrowserPageStateVm {
    let raw = raw.trim();
    if raw.is_empty() {
        return BrowserPageStateVm::NewTab;
    }
    let has_scheme = raw
        .split_once(':')
        .is_some_and(|(s, rest)| {
            !s.is_empty()
                && s.starts_with(|c: char| c.is_ascii_alphabetic())
                && s.chars().all(|c| c.is_ascii_alphanumeric() || "+.-".contains(c))
                // `host:8080/…` não é um esquema.
                && !rest.split('/').next().is_some_and(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
        });
    let parsed = if has_scheme {
        match url::Url::parse(raw) {
            Ok(u) => u,
            Err(_) => {
                return BrowserPageStateVm::Invalid {
                    text: raw.to_owned(),
                }
            }
        }
    } else {
        let looks_like_host = !raw.contains(char::is_whitespace)
            && raw
                .split(['/', '?', '#'])
                .next()
                .and_then(|h| h.split(':').next())
                .and_then(|h| h.rsplit_once('.'))
                .is_some_and(|(_, tld)| tld.len() >= 2);
        match looks_like_host.then(|| url::Url::parse(&format!("https://{raw}"))) {
            Some(Ok(u)) => u,
            _ => {
                return BrowserPageStateVm::Invalid {
                    text: raw.to_owned(),
                }
            }
        }
    };
    match parsed.scheme() {
        "https" | "http" if parsed.host_str().is_some() => {}
        "https" | "http" => {
            return BrowserPageStateVm::Invalid {
                text: raw.to_owned(),
            }
        }
        other => {
            return BrowserPageStateVm::BlockedScheme {
                scheme: format!("{other}:"),
            }
        }
    }
    if parsed.scheme() == "http" {
        // Uma moldura `http:` não é permitida: o recurso honesto é o separador.
        return BrowserPageStateVm::WebMayBeBlocked {
            url: parsed.to_string(),
        };
    }
    BrowserPageStateVm::External {
        origin: parsed.origin().ascii_serialization(),
    }
}

/// `GET /browser` — o Ocinye Browser (D008-B) no runtime Web: uma aba com o
/// endereço pedido numa moldura isolada, ou o estado honesto.
pub(super) async fn browser_page(
    State(state): State<WorkspaceState>,
    headers: HeaderMap,
    Query(q): Query<BrowserQuery>,
) -> Response {
    let w = match open_app(&state, &headers, Screen::Browser, ApplicationId::Browser).await {
        Ok(w) => w,
        Err(r) => return *r,
    };
    if w.page.frame {
        return frame_elsewhere();
    }
    let typed = q.url.unwrap_or_default();
    let page_state = classify(&typed);
    let parsed = url::Url::parse(typed.trim())
        .or_else(|_| url::Url::parse(&format!("https://{}", typed.trim())))
        .ok()
        .filter(|u| matches!(u.scheme(), "http" | "https"));
    let (url, host) = match (&page_state, &parsed) {
        (
            BrowserPageStateVm::External { .. } | BrowserPageStateVm::WebMayBeBlocked { .. },
            Some(u),
        ) => (u.to_string(), u.host_str().map(str::to_owned)),
        _ => (String::new(), None),
    };
    let security = match &page_state {
        BrowserPageStateVm::External { .. } => Some(BrowserSecurity::Https),
        BrowserPageStateVm::WebMayBeBlocked { .. } => Some(BrowserSecurity::Http),
        BrowserPageStateVm::BlockedScheme { .. } => Some(BrowserSecurity::Blocked),
        _ => None,
    };
    let showing = matches!(page_state, BrowserPageStateVm::External { .. });
    let new = matches!(page_state, BrowserPageStateVm::NewTab);
    let vm = BrowserVm {
        runtime: BrowserRuntime::Web,
        tabs: vec![BrowserTabVm {
            id: "t0".to_owned(),
            title: host
                .clone()
                .unwrap_or_else(|| crate::i18n::t("brw.tab.new").to_owned()),
            origin: host.clone(),
            state: if new {
                BrowserTabState::New
            } else {
                BrowserTabState::Ready
            },
        }],
        active: 0,
        navigation: BrowserNavigationVm {
            can_back: None,
            can_forward: None,
            loading: false,
            url: url.clone(),
            host,
            security,
            typed: (!showing && !new).then(|| typed.trim().to_owned()),
        },
        page: page_state,
        notices: if showing {
            vec![BrowserNoticeVm::WebLimits { url }]
        } else {
            vec![]
        },
        // Na Web a Nye não lê páginas (sem extracção fora da casca, ADR-0616):
        // o painel diz porquê.
        side: (q.side.as_deref() == Some("nye"))
            .then_some(BrowserSideVm::Nye(BrowserNyeVm::WebUnavailable)),
        downloads_supported: false,
    };
    let mut response = page(&w, ui::apps::browser::browser(&vm), "/static/oc-browser.js");
    if let Ok(v) = HeaderValue::from_str(&browser_csp()) {
        response
            .headers_mut()
            .insert(header::CONTENT_SECURITY_POLICY, v);
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn so_http_e_https_navegam() {
        assert!(matches!(
            classify("https://example.org/a"),
            BrowserPageStateVm::External { ref origin } if origin == "https://example.org"
        ));
        assert!(matches!(
            classify("example.org"),
            BrowserPageStateVm::External { .. }
        ));
        assert!(matches!(
            classify("localhost:8080/x"),
            BrowserPageStateVm::Invalid { .. }
        ));
        for (bad, scheme) in [
            ("javascript:alert(1)", "javascript:"),
            ("data:text/html,<b>x</b>", "data:"),
            ("file:///etc/passwd", "file:"),
            ("blob:https://example.org/x", "blob:"),
            ("ocinye://x", "ocinye:"),
        ] {
            assert!(
                matches!(classify(bad), BrowserPageStateVm::BlockedScheme { scheme: ref s } if s == scheme),
                "{bad}"
            );
        }
        assert!(matches!(
            classify("como está o tempo"),
            BrowserPageStateVm::Invalid { .. }
        ));
        assert!(matches!(classify(""), BrowserPageStateVm::NewTab));
        // `http:` não entra numa moldura: o recurso é o separador do navegador.
        assert!(matches!(
            classify("http://example.org"),
            BrowserPageStateVm::WebMayBeBlocked { .. }
        ));
    }

    /// A política de `/browser` é a de todas as páginas mais `https:` nas
    /// molduras — nada mais muda.
    #[test]
    fn a_politica_do_browser_so_abre_molduras_https() {
        let b = browser_csp();
        assert!(b.contains("frame-src 'self' https:;"));
        assert!(!b.contains("http:;") && !b.contains(" *"));
        assert_eq!(
            b.replace("frame-src 'self' https:", "frame-src 'self'"),
            CONTENT_SECURITY_POLICY
        );
        assert!(CONTENT_SECURITY_POLICY.contains("frame-src 'self';"));
    }
}
