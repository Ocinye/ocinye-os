//! O Gestor de Janelas (D002, ADR-0618): o estado das janelas de uma sessão.
//!
//! # O que é e o que não é
//!
//! É o motor que o Design deixou a Code: ordem, foco, geometria, encaixe,
//! política de lançamento e ciclo de vida de cada janela. **Não é autoridade.**
//! Não decide o que o membro pode abrir — isso é do Core, perguntado em cada
//! página — nem o que uma aplicação mostra. Uma janela é um lugar onde uma rota
//! do Workspace aparece; abri-la não autoriza nada, e o identificador dela não
//! é um token de nada.
//!
//! # Onde vive
//!
//! Numa [`Desk`] por sessão do Workspace, guardada ao lado da sessão
//! ([`crate::session::SessionStore`]) e com a mesma vida: acaba quando a sessão
//! acaba, e o reinício do Workspace (que já termina todas as sessões) leva-a
//! consigo. Não há estado no browser que conte: o `oc-wm.js` e o
//! `wm-engine.js` pedem, o servidor decide e responde com o estado inteiro.
//!
//! # Determinismo
//!
//! Cada operação é uma função pura de `(Desk, pedido)` para `Desk`. A ordem de
//! empilhamento é um relógio lógico da mesa, não o relógio da máquina; a janela
//! activa é **calculada** (a de ordem mais alta que não esteja minimizada),
//! nunca guardada, para que não possa haver duas.

use ocinye_contracts::{ApplicationId, LaunchPolicy};
use serde::Serialize;

/// Janelas abertas de uma vez por sessão. Uma mesa não é um armazém: acima
/// disto o pedido é recusado, e a memória do Workspace fica limitada.
pub const MAX_WINDOWS: usize = 24;

/// O tamanho mínimo de uma janela livre, o mesmo que a vista respeita
/// (`ui::view_models::WINDOW_MIN`).
pub const MIN_W: u32 = 360;
/// Altura mínima.
pub const MIN_H: u32 = 240;

/// Quanto de uma janela fica sempre ao alcance: a barra de título inteira não,
/// mas o suficiente para a agarrar de volta (largura × altura).
const GRIP_W: i32 = 120;
const GRIP_H: i32 = 40;

/// A maior área de trabalho aceite (px). Um valor maior é um erro do cliente,
/// não um ecrã.
const MAX_AREA: (u32, u32) = (7680, 4320);
/// A menor área de trabalho aceite.
const MIN_AREA: (u32, u32) = (MIN_W, MIN_H);

/// A área usada quando o cliente não disse qual é a sua: só limita, não encaixa.
const DEFAULT_AREA: Area = Area {
    w: MAX_AREA.0,
    h: MAX_AREA.1,
};

/// Como uma janela está.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum State {
    /// Livre, com a sua geometria.
    Normal,
    /// Ocupa a área de trabalho.
    Maximized,
    /// Fora da vista, na prateleira.
    Minimized,
    /// Metade esquerda.
    SnapLeft,
    /// Metade direita.
    SnapRight,
}

/// Posição e tamanho, relativos à área de trabalho.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Geometry {
    /// Esquerda.
    pub x: i32,
    /// Cima.
    pub y: i32,
    /// Largura.
    pub w: u32,
    /// Altura.
    pub h: u32,
}

/// A área de trabalho que o cliente diz ter, já validada.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Area {
    w: u32,
    h: u32,
}

impl Area {
    /// Uma área plausível, ou `None`.
    #[must_use]
    pub fn new(w: u32, h: u32) -> Option<Self> {
        ((MIN_AREA.0..=MAX_AREA.0).contains(&w) && (MIN_AREA.1..=MAX_AREA.1).contains(&h))
            .then_some(Self { w, h })
    }
}

impl Geometry {
    /// A geometria dentro da área: tamanho entre o mínimo e a área, posição
    /// com a pega sempre alcançável. Nunca falha; aproxima.
    #[must_use]
    pub fn clamped(self, area: Area) -> Self {
        let w = self.w.clamp(MIN_W, area.w.max(MIN_W));
        let h = self.h.clamp(MIN_H, area.h.max(MIN_H));
        let wi = i32::try_from(w).unwrap_or(i32::MAX);
        let aw = i32::try_from(area.w).unwrap_or(i32::MAX);
        let ah = i32::try_from(area.h).unwrap_or(i32::MAX);
        let x = self.x.clamp(-(wi - GRIP_W), aw - GRIP_W);
        let y = self.y.clamp(0, (ah - GRIP_H).max(0));
        Self { x, y, w, h }
    }
}

/// Uma janela gerida.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Window {
    /// Identificador na mesa (`w1`, `w2`, …). Só tem sentido dentro desta
    /// sessão; noutra, não existe.
    pub id: String,
    /// A aplicação.
    pub app: ApplicationId,
    /// A rota que mostra (caminho e pergunta, já validados).
    pub href: String,
    /// Como está.
    pub state: State,
    /// O estado a que volta ao sair de minimizada.
    before_min: State,
    /// A geometria livre (e a de regresso ao restaurar).
    pub geometry: Geometry,
    /// Ordem de empilhamento: maior à frente.
    pub z: u32,
    /// A aplicação disse que há trabalho por guardar.
    pub dirty: bool,
    /// A aplicação disse que consegue guardar agora.
    pub can_save: bool,
    /// Fechar foi confirmado com «Guardar»: fecha quando a aplicação disser
    /// que guardou (`dirty = false`).
    pub closing_after_save: bool,
}

/// As janelas de uma sessão.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Desk {
    windows: Vec<Window>,
    /// O último identificador dado.
    seq: u64,
    /// O relógio lógico da ordem de empilhamento.
    clock: u32,
}

/// O que um pedido não pode fazer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WmError {
    /// Não há janela com este identificador nesta sessão.
    NoSuchWindow,
    /// A mesa está cheia.
    TooMany,
    /// Há trabalho por guardar: é preciso decidir (Guardar, Não guardar, Cancelar).
    Dirty,
    /// «Guardar» não está disponível para esta janela agora.
    CannotSave,
}

/// Onde encaixar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Zone {
    /// Metade esquerda.
    Left,
    /// Metade direita.
    Right,
    /// Maximizar.
    Max,
}

impl Zone {
    /// Do valor do formulário.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "left" => Some(Self::Left),
            "right" => Some(Self::Right),
            "max" => Some(Self::Max),
            _ => None,
        }
    }
}

/// A decisão ao fechar uma janela com trabalho por guardar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    /// Guardar e fechar.
    Save,
    /// Fechar sem guardar.
    Discard,
    /// Não fechar.
    Cancel,
}

impl Decision {
    /// Do valor do formulário.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "save" => Some(Self::Save),
            "discard" => Some(Self::Discard),
            "cancel" => Some(Self::Cancel),
            _ => None,
        }
    }
}

/// O resultado de abrir.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Opened {
    /// Uma janela nova.
    New(String),
    /// A que já estava aberta (política de uma janela, ou o mesmo recurso).
    Existing(String),
}

impl Opened {
    /// O identificador da janela.
    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            Self::New(id) | Self::Existing(id) => id,
        }
    }
}

/// O resultado de fechar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Closed {
    /// Fechou.
    Closed,
    /// Ficou aberta (Cancelar).
    Kept,
    /// Espera que a aplicação guarde; fecha quando ela disser que guardou.
    AwaitingSave,
}

/// A rota da aplicação a que uma rota do Workspace pertence, e a política.
///
/// A rota tem de ser exactamente a da aplicação, ou começar por ela seguida de
/// `/` ou `?`. Só caminho e pergunta: nada de esquema, anfitrião, `//`, `\`,
/// `..` nem caracteres de controlo — uma janela nunca aponta para fora do
/// Workspace, nem para uma rota que não é da sua aplicação.
#[must_use]
pub fn valid_href(app: ApplicationId, href: &str) -> bool {
    let route = app.manifest().route;
    let ok_chars = href
        .bytes()
        .all(|b| b.is_ascii_graphic() && b != b'\\' && b != b'#');
    let within = href == route
        || (route != "/"
            && (href
                .strip_prefix(route)
                .is_some_and(|rest| rest.starts_with('/') || rest.starts_with('?'))));
    href.len() <= 512
        && href.starts_with('/')
        && !href.starts_with("//")
        && ok_chars
        && !href.split(['/', '?']).any(|seg| seg == ".." || seg == ".")
        && within
}

/// A aplicação cuja rota contém `href`, pela rota mais longa.
#[must_use]
pub fn app_of(href: &str) -> Option<ApplicationId> {
    ocinye_contracts::application::MANIFESTS
        .iter()
        .filter(|m| m.route != "/" && valid_href(m.id, href))
        .max_by_key(|m| m.route.len())
        .map(|m| m.id)
}

impl Desk {
    /// As janelas, pela ordem em que foram abertas.
    #[must_use]
    pub fn windows(&self) -> &[Window] {
        &self.windows
    }

    /// Não há janelas.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.windows.is_empty()
    }

    /// A janela activa: a de ordem mais alta que não está minimizada.
    #[must_use]
    pub fn active(&self) -> Option<&Window> {
        self.windows
            .iter()
            .filter(|w| w.state != State::Minimized)
            .max_by_key(|w| w.z)
    }

    /// A rota que o endereço do browser deve mostrar: a da janela activa, ou
    /// o Desktop.
    #[must_use]
    pub fn current_href(&self) -> &str {
        self.active().map_or("/", |w| w.href.as_str())
    }

    /// Uma janela pelo identificador.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&Window> {
        self.windows.iter().find(|w| w.id == id)
    }

    fn get_mut(&mut self, id: &str) -> Result<&mut Window, WmError> {
        self.windows
            .iter_mut()
            .find(|w| w.id == id)
            .ok_or(WmError::NoSuchWindow)
    }

    fn tick(&mut self) -> u32 {
        self.clock = self.clock.saturating_add(1);
        self.clock
    }

    /// Abre `href` na aplicação `app`, ou foca a janela que já o mostra.
    ///
    /// - Uma janela que já mostra exactamente `href` é focada, qualquer que
    ///   seja a política (a mais recente, se forem várias).
    /// - `SingleInstance`: a janela da aplicação é focada e passa a mostrar
    ///   `href`.
    /// - `MultiWindow`: a rota da aplicação sem recurso foca a janela mais
    ///   recente dela; um recurso novo (outro caminho), ou `new_window`, abre
    ///   outra. A pergunta é estado da vista: o mesmo caminho com outra
    ///   pergunta fica na janela que o mostra.
    /// - `new_window` numa aplicação de uma janela é ignorado: a política é do
    ///   registo, não do pedido.
    ///
    /// # Errors
    ///
    /// [`WmError::TooMany`] quando a mesa está cheia.
    pub fn open(
        &mut self,
        app: ApplicationId,
        href: &str,
        policy: LaunchPolicy,
        new_window: bool,
    ) -> Result<Opened, WmError> {
        debug_assert!(valid_href(app, href));
        // «Nova janela» numa aplicação que a aceita não reutiliza nenhuma.
        let force_new = new_window && policy == LaunchPolicy::MultiWindow;
        let same = self
            .windows
            .iter()
            .filter(|w| !force_new && w.app == app && w.href == href)
            .max_by_key(|w| w.z)
            .map(|w| w.id.clone());
        // A pergunta é estado da vista (`?item=`, `?folder=`, `?sort=`, `?q=`),
        // não um recurso: navegar dentro de uma janela fica nela (D004).
        let path = |h: &str| h.split_once('?').map_or(h, |(p, _)| p).to_owned();
        let same = same.or_else(|| {
            (!force_new && policy == LaunchPolicy::MultiWindow)
                .then(|| {
                    self.windows
                        .iter()
                        .filter(|w| w.app == app && path(&w.href) == path(href))
                        .max_by_key(|w| w.z)
                        .map(|w| w.id.clone())
                })
                .flatten()
        });
        let reuse = same.or_else(|| {
            let latest = || {
                self.windows
                    .iter()
                    .filter(|w| w.app == app)
                    .max_by_key(|w| w.z)
                    .map(|w| w.id.clone())
            };
            match policy {
                LaunchPolicy::SingleInstance => latest(),
                LaunchPolicy::MultiWindow if !new_window && href == app.manifest().route => {
                    latest()
                }
                LaunchPolicy::MultiWindow => None,
            }
        });
        if let Some(id) = reuse {
            let z = self.tick();
            let w = self.get_mut(&id)?;
            w.href = href.to_owned();
            w.z = z;
            if w.state == State::Minimized {
                w.state = w.before_min;
            }
            return Ok(Opened::Existing(id));
        }
        if self.windows.len() >= MAX_WINDOWS {
            return Err(WmError::TooMany);
        }
        self.seq += 1;
        let id = format!("w{}", self.seq);
        // Em cascata, para que a janela nova não tape exactamente a anterior.
        let step = i32::try_from(self.windows.len() % 8).unwrap_or(0) * 28;
        let z = self.tick();
        self.windows.push(Window {
            id: id.clone(),
            app,
            href: href.to_owned(),
            state: State::Normal,
            before_min: State::Normal,
            geometry: Geometry {
                x: 48 + step,
                y: 32 + step,
                w: 760,
                h: 480,
            },
            z,
            dirty: false,
            can_save: false,
            closing_after_save: false,
        });
        Ok(Opened::New(id))
    }

    /// Traz a janela para a frente (e tira-a da prateleira, se lá estava).
    ///
    /// # Errors
    ///
    /// [`WmError::NoSuchWindow`].
    pub fn focus(&mut self, id: &str) -> Result<(), WmError> {
        let z = self.tick();
        let w = self.get_mut(id)?;
        w.z = z;
        if w.state == State::Minimized {
            w.state = w.before_min;
        }
        Ok(())
    }

    /// Para a prateleira. A ordem não muda: ao voltar, volta ao seu lugar.
    ///
    /// # Errors
    ///
    /// [`WmError::NoSuchWindow`].
    pub fn minimize(&mut self, id: &str) -> Result<(), WmError> {
        let w = self.get_mut(id)?;
        if w.state != State::Minimized {
            w.before_min = w.state;
            w.state = State::Minimized;
        }
        Ok(())
    }

    /// Ocupa a área de trabalho, e fica à frente.
    ///
    /// # Errors
    ///
    /// [`WmError::NoSuchWindow`].
    pub fn maximize(&mut self, id: &str) -> Result<(), WmError> {
        self.set_state(id, State::Maximized)
    }

    /// Volta: de minimizada ao estado anterior; de maximizada ou encaixada, a
    /// livre com a geometria que tinha.
    ///
    /// # Errors
    ///
    /// [`WmError::NoSuchWindow`].
    pub fn restore(&mut self, id: &str) -> Result<(), WmError> {
        let z = self.tick();
        let w = self.get_mut(id)?;
        w.state = match w.state {
            State::Minimized => w.before_min,
            _ => State::Normal,
        };
        w.z = z;
        Ok(())
    }

    /// Encaixa numa metade, ou maximiza.
    ///
    /// # Errors
    ///
    /// [`WmError::NoSuchWindow`].
    pub fn snap(&mut self, id: &str, zone: Zone) -> Result<(), WmError> {
        self.set_state(
            id,
            match zone {
                Zone::Left => State::SnapLeft,
                Zone::Right => State::SnapRight,
                Zone::Max => State::Maximized,
            },
        )
    }

    fn set_state(&mut self, id: &str, state: State) -> Result<(), WmError> {
        let z = self.tick();
        let w = self.get_mut(id)?;
        w.state = state;
        w.z = z;
        Ok(())
    }

    /// Move a janela. Arrastar uma janela maximizada ou encaixada solta-a
    /// (volta a livre); a posição fica sempre com a pega ao alcance.
    ///
    /// # Errors
    ///
    /// [`WmError::NoSuchWindow`].
    pub fn move_to(&mut self, id: &str, x: i32, y: i32, area: Option<Area>) -> Result<(), WmError> {
        let z = self.tick();
        let w = self.get_mut(id)?;
        w.state = State::Normal;
        w.geometry = Geometry { x, y, ..w.geometry }.clamped(area.unwrap_or(DEFAULT_AREA));
        w.z = z;
        Ok(())
    }

    /// Redimensiona, entre o mínimo e a área.
    ///
    /// # Errors
    ///
    /// [`WmError::NoSuchWindow`].
    pub fn resize(
        &mut self,
        id: &str,
        geometry: Geometry,
        area: Option<Area>,
    ) -> Result<(), WmError> {
        let z = self.tick();
        let w = self.get_mut(id)?;
        w.state = State::Normal;
        w.geometry = geometry.clamped(area.unwrap_or(DEFAULT_AREA));
        w.z = z;
        Ok(())
    }

    /// Fecha. Uma janela limpa fecha já; uma com trabalho por guardar só com
    /// uma decisão, e a decisão executada é exactamente a confirmada.
    ///
    /// # Errors
    ///
    /// [`WmError::NoSuchWindow`]; [`WmError::Dirty`] sem decisão numa janela
    /// com trabalho por guardar; [`WmError::CannotSave`] para «Guardar» quando
    /// a aplicação disse que não consegue.
    pub fn close(&mut self, id: &str, decision: Option<Decision>) -> Result<Closed, WmError> {
        let w = self.get_mut(id)?;
        match (w.dirty, decision) {
            (_, Some(Decision::Cancel)) => {
                w.closing_after_save = false;
                Ok(Closed::Kept)
            }
            (true, None) => Err(WmError::Dirty),
            (true, Some(Decision::Save)) if !w.can_save => Err(WmError::CannotSave),
            (true, Some(Decision::Save)) => {
                w.closing_after_save = true;
                Ok(Closed::AwaitingSave)
            }
            (false, _) | (true, Some(Decision::Discard)) => {
                self.windows.retain(|w| w.id != id);
                Ok(Closed::Closed)
            }
        }
    }

    /// O que a aplicação diz sobre o seu trabalho. Depois de «Guardar», a
    /// primeira notícia de que já não há nada por guardar fecha a janela.
    ///
    /// # Errors
    ///
    /// [`WmError::NoSuchWindow`].
    pub fn report(&mut self, id: &str, dirty: bool, can_save: bool) -> Result<Closed, WmError> {
        let w = self.get_mut(id)?;
        w.dirty = dirty;
        w.can_save = can_save;
        if w.closing_after_save && !dirty {
            self.windows.retain(|w| w.id != id);
            return Ok(Closed::Closed);
        }
        Ok(Closed::Kept)
    }

    /// Fecha as janelas de aplicações que o membro deixou de poder abrir.
    /// Chamado em cada página, antes de desenhar: uma permissão retirada
    /// nunca reabre uma janela.
    pub fn retain_apps(&mut self, allowed: impl Fn(ApplicationId) -> bool) {
        self.windows.retain(|w| allowed(w.app));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ApplicationId as A;
    use LaunchPolicy::{MultiWindow as Multi, SingleInstance as Single};

    fn area() -> Option<Area> {
        Area::new(1400, 800)
    }

    #[test]
    fn a_rota_tem_de_ser_da_aplicacao_e_do_workspace() {
        assert!(valid_href(A::Files, "/files"));
        assert!(valid_href(A::Files, "/files/abc?view=list"));
        assert!(valid_href(A::Files, "/files?window=new"));
        for mau in [
            "/filesx",
            "/notes",
            "//evil.example/files",
            "https://evil.example/files",
            "/files/../admin",
            "/files/./x",
            "/files\\x",
            "/files#x",
            "/files/a b",
            "files",
        ] {
            assert!(!valid_href(A::Files, mau), "{mau}");
        }
        assert!(!valid_href(
            A::Files,
            &format!("/files/{}", "a".repeat(600))
        ));
        assert_eq!(app_of("/files/abc"), Some(A::Files));
        assert_eq!(app_of("/"), None);
        assert_eq!(app_of("/nada"), None);
    }

    #[test]
    fn uma_janela_so_volta_a_focar_a_que_existe() {
        let mut d = Desk::default();
        let a = d.open(A::Mail, "/mail", Single, false).unwrap();
        let b = d.open(A::Mail, "/mail/x", Single, false).unwrap();
        assert!(matches!(a, Opened::New(_)));
        assert_eq!(b, Opened::Existing(a.id().to_owned()));
        assert_eq!(d.windows().len(), 1);
        assert_eq!(d.windows()[0].href, "/mail/x");
        // «Nova janela» não contorna a política.
        let c = d.open(A::Mail, "/mail", Single, true).unwrap();
        assert_eq!(c.id(), a.id());
    }

    #[test]
    fn varias_janelas_abrem_uma_por_recurso() {
        let mut d = Desk::default();
        let a = d.open(A::Files, "/files", Multi, false).unwrap();
        let b = d.open(A::Files, "/files/x", Multi, false).unwrap();
        let again = d.open(A::Files, "/files/x", Multi, false).unwrap();
        let bare = d.open(A::Files, "/files", Multi, false).unwrap();
        let new = d.open(A::Files, "/files", Multi, true).unwrap();
        assert!(matches!(b, Opened::New(_)));
        assert_eq!(again, Opened::Existing(b.id().to_owned()));
        // A rota sem recurso foca a mais recente, que é a de /files (a própria).
        assert_eq!(bare, Opened::Existing(a.id().to_owned()));
        assert!(matches!(new, Opened::New(_)));
        assert_eq!(d.windows().len(), 3);
    }

    #[test]
    fn a_pergunta_e_estado_da_vista_e_fica_na_mesma_janela() {
        let mut d = Desk::default();
        let a = d.open(A::Files, "/files", Multi, false).unwrap();
        let item = d.open(A::Files, "/files?item=f.x", Multi, false).unwrap();
        let pasta = d
            .open(A::Files, "/files?folder=d.y&sort=name", Multi, false)
            .unwrap();
        assert_eq!(item, Opened::Existing(a.id().to_owned()));
        assert_eq!(pasta, Opened::Existing(a.id().to_owned()));
        assert_eq!(d.windows().len(), 1);
        assert_eq!(d.windows()[0].href, "/files?folder=d.y&sort=name");
        // Outro caminho é outro recurso; «Nova janela» abre sempre outra.
        let nota = d.open(A::Notes, "/notes/n1", Multi, false).unwrap();
        let outra = d.open(A::Notes, "/notes/n2", Multi, false).unwrap();
        assert_ne!(nota.id(), outra.id());
        assert!(matches!(
            d.open(A::Files, "/files?item=f.z", Multi, true).unwrap(),
            Opened::New(_)
        ));
    }

    #[test]
    fn so_ha_uma_activa_e_e_a_da_frente() {
        let mut d = Desk::default();
        let a = d.open(A::Mail, "/mail", Single, false).unwrap();
        let b = d.open(A::Calendar, "/calendar", Single, false).unwrap();
        assert_eq!(d.active().unwrap().id, b.id());
        d.focus(a.id()).unwrap();
        assert_eq!(d.active().unwrap().id, a.id());
        assert_eq!(d.current_href(), "/mail");
        // Minimizar a activa passa o foco à seguinte.
        d.minimize(a.id()).unwrap();
        assert_eq!(d.active().unwrap().id, b.id());
        d.minimize(b.id()).unwrap();
        assert!(d.active().is_none());
        assert_eq!(d.current_href(), "/");
        // Restaurar traz para a frente.
        d.restore(a.id()).unwrap();
        assert_eq!(d.active().unwrap().id, a.id());
    }

    #[test]
    fn minimizar_e_voltar_repoe_o_estado_anterior() {
        let mut d = Desk::default();
        let a = d.open(A::Mail, "/mail", Single, false).unwrap();
        d.snap(a.id(), Zone::Left).unwrap();
        d.minimize(a.id()).unwrap();
        d.minimize(a.id()).unwrap();
        assert_eq!(d.get(a.id()).unwrap().state, State::Minimized);
        d.focus(a.id()).unwrap();
        assert_eq!(d.get(a.id()).unwrap().state, State::SnapLeft);
        d.maximize(a.id()).unwrap();
        d.restore(a.id()).unwrap();
        assert_eq!(d.get(a.id()).unwrap().state, State::Normal);
        d.snap(a.id(), Zone::Right).unwrap();
        assert_eq!(d.get(a.id()).unwrap().state, State::SnapRight);
        d.snap(a.id(), Zone::Max).unwrap();
        assert_eq!(d.get(a.id()).unwrap().state, State::Maximized);
    }

    #[test]
    fn a_geometria_fica_sempre_ao_alcance() {
        let mut d = Desk::default();
        let a = d.open(A::Mail, "/mail", Single, false).unwrap();
        d.maximize(a.id()).unwrap();
        // Arrastar solta a maximizada.
        d.move_to(a.id(), -5000, -300, area()).unwrap();
        let w = d.get(a.id()).unwrap();
        assert_eq!(w.state, State::Normal);
        assert_eq!(w.geometry.y, 0);
        assert_eq!(w.geometry.x, -(760 - GRIP_W));
        d.move_to(a.id(), 99_999, 99_999, area()).unwrap();
        let g = d.get(a.id()).unwrap().geometry;
        assert_eq!((g.x, g.y), (1400 - GRIP_W, 800 - GRIP_H));
        d.resize(
            a.id(),
            Geometry {
                x: 10,
                y: 10,
                w: 10,
                h: 99_999,
            },
            area(),
        )
        .unwrap();
        let g = d.get(a.id()).unwrap().geometry;
        assert_eq!((g.w, g.h), (MIN_W, 800));
        assert!(Area::new(100, 100).is_none() && Area::new(99_999, 800).is_none());
        // Sem área, só os limites absolutos.
        d.resize(
            a.id(),
            Geometry {
                x: 0,
                y: 0,
                w: u32::MAX,
                h: 5,
            },
            None,
        )
        .unwrap();
        let g = d.get(a.id()).unwrap().geometry;
        assert_eq!((g.w, g.h), (MAX_AREA.0, MIN_H));
    }

    #[test]
    fn fechar_limpa_fecha_e_suja_pede_decisao() {
        let mut d = Desk::default();
        let a = d.open(A::Notes, "/notes/1", Multi, false).unwrap();
        let b = d.open(A::Notes, "/notes/2", Multi, false).unwrap();
        assert_eq!(d.close(a.id(), None), Ok(Closed::Closed));
        d.report(b.id(), true, false).unwrap();
        assert_eq!(d.close(b.id(), None), Err(WmError::Dirty));
        assert_eq!(d.close(b.id(), Some(Decision::Cancel)), Ok(Closed::Kept));
        assert_eq!(
            d.close(b.id(), Some(Decision::Save)),
            Err(WmError::CannotSave)
        );
        d.report(b.id(), true, true).unwrap();
        assert_eq!(
            d.close(b.id(), Some(Decision::Save)),
            Ok(Closed::AwaitingSave)
        );
        // Ainda por guardar: fica.
        assert_eq!(d.report(b.id(), true, true), Ok(Closed::Kept));
        assert!(d.get(b.id()).is_some());
        // Guardou: fecha.
        assert_eq!(d.report(b.id(), false, true), Ok(Closed::Closed));
        assert!(d.is_empty());
        assert_eq!(d.close("w9", None), Err(WmError::NoSuchWindow));
    }

    #[test]
    fn nao_guardar_fecha_mesmo_com_trabalho_por_guardar() {
        let mut d = Desk::default();
        let a = d.open(A::Notes, "/notes/1", Multi, false).unwrap();
        d.report(a.id(), true, true).unwrap();
        assert_eq!(d.close(a.id(), Some(Decision::Discard)), Ok(Closed::Closed));
        assert!(d.is_empty());
    }

    #[test]
    fn a_mesa_tem_limite_e_os_identificadores_nao_se_repetem() {
        let mut d = Desk::default();
        for i in 0..MAX_WINDOWS {
            d.open(A::Files, &format!("/files/{i}"), Multi, false)
                .unwrap();
        }
        assert_eq!(
            d.open(A::Files, "/files/mais", Multi, false),
            Err(WmError::TooMany)
        );
        d.close("w1", None).unwrap();
        let n = d.open(A::Files, "/files/mais", Multi, false).unwrap();
        assert_eq!(n.id(), format!("w{}", MAX_WINDOWS + 1));
    }

    #[test]
    fn uma_permissao_retirada_fecha_a_janela() {
        let mut d = Desk::default();
        d.open(A::Mail, "/mail", Single, false).unwrap();
        d.open(A::Files, "/files", Multi, false).unwrap();
        d.retain_apps(|a| a != A::Mail);
        assert_eq!(d.windows().len(), 1);
        assert_eq!(d.windows()[0].app, A::Files);
    }
}
