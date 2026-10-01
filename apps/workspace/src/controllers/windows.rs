//! O Gestor de Janelas ligado às páginas (D002 · FG-010, FG-026 a FG-028).
//!
//! O motor ([`crate::window_manager`]) guarda o estado; aqui decide-se o que
//! cada página mostra dele e em nome de quem:
//!
//! - só as janelas de aplicações que o membro **ainda** pode abrir, segundo o
//!   Core nesta página ([`Application::visible_to`]); as outras fecham-se
//!   antes de desenhar, para que uma permissão retirada nunca reabra nada;
//! - a rota de cada janela é a que o pedido trouxe, validada contra a rota da
//!   aplicação ([`window_manager::valid_href`]);
//! - as aplicações cujo ecrã o Design ainda não entregou mostram
//!   `WindowContent::Pending` — a mesma linguagem do `app_pending` D001, agora
//!   dentro da janela.

use std::future::Future;

use ocinye_contracts::{ApplicationId, LaunchPolicy};
use serde_json::{json, Value};

use crate::controllers::ShellContext;
use crate::experience::apps::{Application, APPLICATIONS};
use crate::experience::navigation::Screen;
use crate::session::SessionStore;
use crate::ui::view_models::{
    DirtyCloseVm, WindowContent, WindowGeometry, WindowState, WindowVm, WmVm,
};
use crate::window_manager::{self, Desk, Opened, State, WmError};

tokio::task_local! {
    /// O caminho e a pergunta do pedido em curso, para a rota da janela.
    static REQUEST: String;
}

/// Corre `f` com o caminho e a pergunta do pedido ao alcance de
/// [`page_query`]. Os manipuladores das aplicações recebem só os cabeçalhos; a
/// rota real (`/files/abc?view=list`) chega-lhes por aqui, sem mudar 70
/// assinaturas.
pub async fn with_request<F: Future>(path_and_query: String, f: F) -> F::Output {
    REQUEST.scope(path_and_query, f).await
}

/// Os parâmetros do motor, que não são parte da rota da janela.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PageQuery {
    /// A rota da janela: o caminho e a pergunta sem os parâmetros do motor.
    pub href: String,
    /// `?window=new`: «Nova janela» (só vale onde a política deixa).
    pub new_window: bool,
    /// `?close=w3`: desenhar a confirmação de fechar esta janela.
    pub close: Option<String>,
    /// `?frame=1`: só o corpo da janela (WM-4).
    pub frame: bool,
}

/// Separa a rota da janela dos parâmetros do motor, a partir do pedido em
/// curso (ou de `fallback`, fora de um pedido).
#[must_use]
pub fn page_query(fallback: &str) -> PageQuery {
    let raw = REQUEST
        .try_with(Clone::clone)
        .unwrap_or_else(|_| fallback.to_owned());
    parse_query(&raw)
}

fn parse_query(raw: &str) -> PageQuery {
    let (path, query) = raw.split_once('?').unwrap_or((raw, ""));
    let mut q = PageQuery::default();
    let mut kept = Vec::new();
    for pair in query.split('&').filter(|p| !p.is_empty()) {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        match k {
            "window" => q.new_window = v == "new",
            "close" => q.close = Some(v.to_owned()),
            "frame" => q.frame = v == "1",
            _ => kept.push(pair),
        }
    }
    q.href = if kept.is_empty() {
        path.to_owned()
    } else {
        format!("{path}?{}", kept.join("&"))
    };
    q
}

/// A aplicação que abre este ecrã, se ele for uma aplicação.
#[must_use]
pub fn application_of(screen: Screen) -> Option<&'static Application> {
    APPLICATIONS.iter().find(|a| a.screen == screen)
}

/// A aplicação do registo com este identificador.
#[must_use]
pub fn application_of_id(id: ApplicationId) -> Option<&'static Application> {
    APPLICATIONS.iter().find(|a| a.id() == id.as_str())
}

fn application(id: ApplicationId) -> Option<&'static Application> {
    application_of_id(id)
}

/// A aplicação é visível a quem está nesta página (o mesmo filtro do lançador).
fn visible(ctx: &ShellContext, id: ApplicationId) -> bool {
    application(id).is_some_and(|a| a.visible_to(&ctx.viewer, ctx.core))
}

/// A dica do alternador. Neutra de plataforma de propósito: só o `runtime.js`
/// pergunta ao ambiente (ADR-0611), e `Alt` + `W` é a mesma tecla física em
/// todos os teclados (`KeyW`) e não está reservada por nenhum browser.
pub const SWITCHER_HINT: &str = crate::experience::shortcuts::SWITCHER.keys;

/// Abre `href` numa janela da aplicação (ou foca a que já o mostra).
///
/// `None` quando a sessão já não existe no registo; a página segue sem
/// janelas. A autorização não é daqui: quem chama já passou pelo portão da
/// aplicação ([`visible`] via `screen_open`).
pub fn open(
    sessions: &SessionStore,
    session_id: &str,
    app: ApplicationId,
    href: &str,
    new_window: bool,
) -> Option<Result<Opened, WmError>> {
    let href = if window_manager::valid_href(app, href) {
        href
    } else {
        app.manifest().route
    };
    let policy = app.manifest().launch;
    sessions.with_desk(session_id, |d| d.open(app, href, policy, new_window))
}

fn state_vm(s: State) -> WindowState {
    match s {
        State::Normal => WindowState::Normal,
        State::Maximized => WindowState::Maximized,
        State::Minimized => WindowState::Minimized,
        State::SnapLeft => WindowState::SnappedLeft,
        State::SnapRight => WindowState::SnappedRight,
    }
}

/// O `WmVm` desta página, ou `None` sem janelas (a casca é então a D001).
///
/// Fecha primeiro as janelas de aplicações que o membro deixou de poder
/// abrir: falha fechado.
#[must_use]
pub fn view(sessions: &SessionStore, session_id: &str, ctx: &ShellContext) -> Option<WmVm> {
    let desk = sessions.with_desk(session_id, |d| {
        d.retain_apps(|a| visible(ctx, a));
        d.clone()
    })?;
    wm_vm(&desk, |a| visible(ctx, a))
}

fn wm_vm(desk: &Desk, allowed: impl Fn(ApplicationId) -> bool) -> Option<WmVm> {
    if desk.is_empty() {
        return None;
    }
    let active = desk.active().map(|w| w.id.clone());
    let mut order: Vec<(u32, &str)> = desk
        .windows()
        .iter()
        .map(|w| (w.z, w.id.as_str()))
        .collect();
    order.sort_unstable();
    let rank = |id: &str| {
        order
            .iter()
            .position(|(_, i)| *i == id)
            .and_then(|p| u16::try_from(p + 1).ok())
            .unwrap_or(u16::MAX)
    };
    let windows = desk
        .windows()
        .iter()
        .map(|w| WindowVm {
            id: w.id.clone(),
            app_id: w.app.as_str(),
            app_href: w.app.manifest().route,
            href: w.href.clone(),
            title: application(w.app)
                .map_or_else(|| w.app.as_str().to_owned(), |a| a.label().to_owned()),
            subtitle: None,
            state: state_vm(w.state),
            active: active.as_deref() == Some(w.id.as_str()),
            z: rank(&w.id),
            geometry: WindowGeometry {
                x: w.geometry.x,
                y: w.geometry.y,
                w: w.geometry.w,
                h: w.geometry.h,
            },
            dirty: w.dirty,
            // Uma aplicação com ecrã do Design (D003 Nye, D004 Ficheiros, Notas,
            // Calendário, Correio) carrega o corpo por `?frame=1`; as outras
            // mostram o estado honesto do `app_pending`. A janela da rota pedida
            // passa a `Ready` no handler que a desenha.
            content: if has_screen(w.app) {
                WindowContent::Loading
            } else {
                WindowContent::Pending
            },
        })
        .collect();
    let multi_window_apps = ocinye_contracts::application::MANIFESTS
        .iter()
        .filter(|m| m.launch == LaunchPolicy::MultiWindow && allowed(m.id))
        .map(|m| m.id.as_str())
        .collect();
    Some(WmVm {
        windows,
        switcher_hint: Some(SWITCHER_HINT.to_owned()),
        multi_window_apps,
    })
}

/// As aplicações com ecrã do Design: o corpo pede-se por `?frame=1`. O
/// Terminal e o Browser respondem-lhe sem corpo (o cliente de cada um só
/// existe na sua rota, ADR-0623), e a janela de fundo mostra a ligação para o
/// seu endereço. Depois da D008 nenhuma aplicação registada fica `app_pending`.
#[must_use]
pub fn has_screen(app: ApplicationId) -> bool {
    matches!(
        app,
        ApplicationId::Files
            | ApplicationId::Notes
            | ApplicationId::Calendar
            | ApplicationId::Mail
            | ApplicationId::Prompt
            | ApplicationId::Projects
            | ApplicationId::Work
            | ApplicationId::Ideas
            | ApplicationId::Datasets
            | ApplicationId::Knowledge
            | ApplicationId::Units
            | ApplicationId::Administration
            | ApplicationId::Messages
            | ApplicationId::Ai
            | ApplicationId::Agents
            | ApplicationId::Compute
            | ApplicationId::Resources
            | ApplicationId::Activity
            | ApplicationId::Audit
            | ApplicationId::Settings
            | ApplicationId::Help
            | ApplicationId::Monitor
            | ApplicationId::Results
            | ApplicationId::Trash
            | ApplicationId::Terminal
            | ApplicationId::Browser
    )
}

/// A confirmação de fechar, quando a janela existe e tem trabalho por guardar.
#[must_use]
pub fn dirty_close(sessions: &SessionStore, session_id: &str, id: &str) -> Option<DirtyCloseVm> {
    sessions
        .with_desk(session_id, |d| {
            d.get(id).filter(|w| w.dirty).map(|w| DirtyCloseVm {
                window_id: w.id.clone(),
                title: application(w.app)
                    .map_or_else(|| w.app.as_str().to_owned(), |a| a.label().to_owned()),
                can_save: w.can_save,
                save_label: None,
                save_form: None,
                after: None,
            })
        })
        .flatten()
}

/// O estado inteiro, para o `wm-engine.js` reconciliar (WM-1).
#[must_use]
pub fn state_json(desk: &Desk) -> Value {
    let active = desk.active().map(|w| w.id.clone());
    json!({
        "href": desk.current_href(),
        "active": active,
        "windows": desk.windows().iter().map(|w| json!({
            "id": w.id,
            "app_id": w.app.as_str(),
            "href": w.href,
            "state": w.state,
            "active": active.as_deref() == Some(w.id.as_str()),
            "z": w.z,
            "x": w.geometry.x,
            "y": w.geometry.y,
            "w": w.geometry.w,
            "h": w.geometry.h,
            "dirty": w.dirty,
        })).collect::<Vec<_>>(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ocinye_contracts::LaunchPolicy::{MultiWindow, SingleInstance};

    #[test]
    fn os_parametros_do_motor_nao_entram_na_rota_da_janela() {
        let q = parse_query("/files/abc?view=list&window=new&close=w3&frame=1&sort=name");
        assert_eq!(q.href, "/files/abc?view=list&sort=name");
        assert!(q.new_window && q.frame);
        assert_eq!(q.close.as_deref(), Some("w3"));
        assert_eq!(parse_query("/files").href, "/files");
        assert_eq!(parse_query("/files?frame=0").href, "/files");
        assert!(!parse_query("/files?frame=0").frame);
    }

    #[test]
    fn sem_janelas_nao_ha_gestor_e_a_casca_e_a_d001() {
        assert!(wm_vm(&Desk::default(), |_| true).is_none());
    }

    #[test]
    fn a_vista_tem_uma_activa_ordem_e_a_politica_do_registo() {
        let mut d = Desk::default();
        d.open(ApplicationId::Files, "/files/a", MultiWindow, false)
            .unwrap();
        d.open(ApplicationId::Mail, "/mail", SingleInstance, false)
            .unwrap();
        let vm = wm_vm(&d, |a| a != ApplicationId::Notes).unwrap();
        assert_eq!(vm.windows.len(), 2);
        let active: Vec<_> = vm.windows.iter().filter(|w| w.active).collect();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].app_id, "mail");
        assert_eq!((vm.windows[0].z, vm.windows[1].z), (1, 2));
        // D004: Ficheiros e Correio têm ecrã — o corpo chega por `?frame=1`.
        assert!(vm
            .windows
            .iter()
            .all(|w| w.content == WindowContent::Loading));
        // Depois da D008 nenhuma aplicação registada fica `app_pending`: o
        // Terminal também pede o seu corpo (e responde-lhe sem corpo).
        d.open(ApplicationId::Terminal, "/terminal", SingleInstance, false)
            .unwrap();
        d.open(ApplicationId::Datasets, "/datasets", SingleInstance, false)
            .unwrap();
        let vm = wm_vm(&d, |a| a != ApplicationId::Notes).unwrap();
        let terminal = vm.windows.iter().find(|w| w.app_id == "terminal").unwrap();
        assert_eq!(terminal.content, WindowContent::Loading);
        assert!(has_screen(ApplicationId::Browser));
        let dados = vm.windows.iter().find(|w| w.app_id == "datasets").unwrap();
        assert_eq!(dados.content, WindowContent::Loading);
        // Notas aceita várias janelas, mas não é visível: não se oferece.
        assert_eq!(vm.multi_window_apps, ["files"]);
        assert_eq!(vm.switcher_hint.as_deref(), Some(SWITCHER_HINT));
    }
}
