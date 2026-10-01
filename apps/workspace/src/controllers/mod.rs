//! Os controladores entre as rotas e as vistas do Claude Design (D001).
//!
//! ```text
//! vista do Design (ui::…)  ←  ViewModel  ←  controlador (aqui)  ←  Core
//! ```
//!
//! As vistas são funções puras de um ViewModel e não sabem que o Core existe.
//! Aqui é que se pergunta ao Core, se traduz a resposta para a forma que o
//! Design fixou, e se diz a verdade quando a resposta não veio: `None` e
//! `Load::Failed`, nunca um zero inventado. Nada aqui decide autorização — o
//! Core recusa ou devolve, e a interface mostra o que ele disse.

pub mod desktop;
pub mod nye;
pub mod ops;
pub mod org;
pub mod panels;
pub mod productivity;
pub mod reg;
pub mod research;
pub mod windows;

use serde_json::Value;

use crate::api::{self, ApiFailure};
use crate::boot::BootState as ProbeState;
use crate::experience::apps;
use crate::experience::navigation::{CoreStatus, ResolucaoSessao, Viewer};
use crate::i18n::t;
use crate::session::Session;
use crate::ui::view_models::{
    AppTile, BootComponent, BootState, BootVm, Distribution, DoorVm, Health, ShellVm, TopPanels,
    Wallpaper,
};
use crate::WorkspaceState;

/// Quem pede, com o token que o Core reconhece.
pub struct Caller<'a> {
    /// A sessão do Workspace.
    pub session: &'a Session,
    /// O identificador que correlaciona este pedido nos registos do Core.
    pub correlation_id: &'a str,
}

impl Caller<'_> {
    /// `GET` autenticado ao Core.
    pub async fn get(&self, state: &WorkspaceState, path: &str) -> Result<Value, ApiFailure> {
        api::get::<Value>(state, &self.session.access_token, self.correlation_id, path).await
    }
}

/// Uma referência curta que a pessoa pode dar ao administrador (`OC-7F3A0C21`),
/// registada no log com o detalhe que não vai para o ecrã.
#[must_use]
pub fn reference(detail: &str) -> String {
    let r = format!(
        "OC-{}",
        uuid::Uuid::new_v4().simple().to_string()[..8].to_uppercase()
    );
    tracing::error!(reference = %r, detail = %detail, "a Core call failed");
    r
}

/// O `profile` do Core (`InstanceProfile`) é a Distribuição da Instância. A
/// conversão faz-se aqui, na fronteira (BACKEND_TERMINOLOGY_MIGRATION_REQUIRED).
#[must_use]
pub fn distribution_of(profile: &str) -> Option<Distribution> {
    match profile {
        "research" => Some(Distribution::Research),
        "business" => Some(Distribution::Business),
        "personal" => Some(Distribution::Personal),
        "education" => Some(Distribution::Education),
        _ => None,
    }
}

/// O estado da Instância à porta e na barra, a partir da sonda ao `/ready`.
///
/// `Degraded` no `/ready` quer dizer, por construção, que todos os componentes
/// críticos respondem e falta algum opcional (correio, inferência, computação).
/// A Instância está operacional; o que falta diz-se onde se fala dele. É a mesma
/// leitura que o distintivo CORE já fazia antes do apagamento da UI.
#[must_use]
pub const fn instance_health(probe: &ProbeState) -> Health {
    match probe {
        ProbeState::Ready | ProbeState::Degraded => Health::Operational,
        ProbeState::Blocked
        | ProbeState::Unreachable
        | ProbeState::Uninitialized
        | ProbeState::Checking => Health::Unavailable,
    }
}

/// O que a porta mostra antes de haver sessão: a Distribuição (do
/// `GET /instance/branding`, público) e o estado sondado agora.
pub async fn door(state: &WorkspaceState) -> DoorVm {
    // D010 (S07/S08): o que a porta mostra vem do ponto de acesso deste pedido
    // — a Instância, o endereço e, se o ponto é fixo, a sua Distribuição.
    // Num ponto genérico, nenhuma Distribuição antes da entrada.
    let probe = crate::boot::probe(state).await;
    let endpoint = crate::access::current();
    DoorVm {
        distribution: endpoint
            .as_ref()
            .and_then(crate::access::Endpoint::bound)
            .and_then(|d| distribution_of(d.as_str())),
        core: Some(instance_health(&probe.state)),
        instance: endpoint.as_ref().map(|e| e.instance_name.clone()),
        host: endpoint.map(|e| e.host),
    }
}

/// `GET /boot`: o que o `/ready` disse, componente a componente.
pub async fn boot(state: &WorkspaceState) -> (BootVm, bool) {
    use ocinye_contracts::readiness::Criticality;
    use ocinye_contracts::system_capability::SystemCapabilityState;

    let outcome = crate::boot::probe(state).await;
    let segue = outcome.state.may_hand_off();
    let components = outcome
        .readiness
        .as_ref()
        .map(|r| {
            r.components
                .iter()
                .map(|c| BootComponent {
                    name: t(&format!("boot.component.{}", c.component.as_str())).to_owned(),
                    health: match (c.state, c.criticality) {
                        (SystemCapabilityState::Available, _) => Health::Operational,
                        (_, Criticality::Optional) => Health::Degraded,
                        (_, Criticality::Critical) => Health::Unavailable,
                    },
                    // A razão do Core vem numa só língua; a linha já diz o estado.
                    note: None,
                })
                .collect()
        })
        .unwrap_or_default();
    let reference = (!segue).then(|| reference(&format!("boot probe: {:?}", outcome.state)));
    let vm = BootVm {
        door: {
            // D010: a porta do arranque é a do ponto de acesso (S07/S08).
            let endpoint = crate::access::current();
            DoorVm {
                distribution: endpoint
                    .as_ref()
                    .and_then(crate::access::Endpoint::bound)
                    .and_then(|d| distribution_of(d.as_str())),
                core: Some(instance_health(&outcome.state)),
                instance: endpoint.as_ref().map(|e| e.instance_name.clone()),
                host: endpoint.map(|e| e.host),
            }
        },
        state: if segue {
            BootState::Ready
        } else {
            BootState::Blocked
        },
        components,
        reference,
    };
    (vm, segue)
}

/// Tudo o que uma página autenticada sabe depois de perguntar ao Core.
pub struct ShellContext {
    /// A casca, pronta para a vista.
    pub vm: ShellVm,
    /// O modelo de navegação (visibilidade das aplicações).
    pub viewer: Viewer,
    /// O estado do Core para a navegação.
    pub core: CoreStatus,
    /// Tem autoridade de administração agora.
    pub is_admin: bool,
    /// A Distribuição, quando o Core a disse.
    pub distribution: Option<Distribution>,
    /// A resposta de `GET /me/desktop` (também dá o fundo à casca).
    pub desktop: Result<Value, ApiFailure>,
    /// A zona horária do membro (a da Instância, pelo `/me`).
    pub zone: ocinye_contracts::temporal::TimeZoneName,
    /// D010: as Distribuições desta sessão, como o Core as decidiu.
    pub dists: Distributions,
}

/// D010 · As Distribuições desta sessão, como o Core as decidiu.
#[derive(Clone, Debug, Default)]
pub struct Distributions {
    /// A activa (já revalidada).
    pub active: Option<ocinye_contracts::Distribution>,
    /// Activadas ∩ acesso do membro.
    pub accessible: Vec<ocinye_contracts::Distribution>,
    /// O ponto genérico canónico, para mudar a partir de um ponto fixo.
    pub generic_host: Option<String>,
    /// Os pontos fixos activos das acessíveis.
    pub bound_hosts: Vec<(ocinye_contracts::Distribution, String)>,
    /// Aplicações que a Distribuição activa não traz (ADR-0019 §9).
    pub inactive_here: Vec<String>,
}

/// D010 · Porque a sessão não entra (ainda) numa Distribuição.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Entry {
    /// Várias acessíveis: S09.
    Choose(Vec<ocinye_contracts::Distribution>),
    /// Nenhuma acessível: S10.
    Zero,
    /// Ponto fixo, sem acesso a essa Distribuição: S11.
    BoundNoAccess(ocinye_contracts::Distribution),
    /// Ponto fixo numa Distribuição desactivada: S12.
    BoundDisabled(ocinye_contracts::Distribution),
    /// A Distribuição da sessão deixou de estar acessível: S18.
    Revoked(ocinye_contracts::Distribution),
    /// A Distribuição da sessão foi desactivada: S39.
    DisabledLive(ocinye_contracts::Distribution),
}

fn dist_list(v: Option<&Value>) -> Vec<ocinye_contracts::Distribution> {
    v.and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .filter_map(|d| d.parse().ok())
                .collect()
        })
        .unwrap_or_default()
}

fn one_dist(v: Option<&Value>) -> Option<ocinye_contracts::Distribution> {
    v.and_then(Value::as_str).and_then(|d| d.parse().ok())
}

fn parse_mine(v: &Value) -> Distributions {
    Distributions {
        active: one_dist(v.get("active")),
        accessible: dist_list(v.get("accessible")),
        generic_host: v
            .get("generic_host")
            .and_then(Value::as_str)
            .map(str::to_owned),
        bound_hosts: v
            .get("bound_hosts")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|pair| {
                        let d = one_dist(pair.get(0))?;
                        let h = pair.get(1)?.as_str()?.to_owned();
                        Some((d, h))
                    })
                    .collect()
            })
            .unwrap_or_default(),
        inactive_here: strings(v.get("inactive_here")),
    }
}

/// D010 (ADR-0625 §1): a sessão tem de estar numa Distribuição antes de
/// qualquer página. Ponto fixo → a dele, se o Core deixar (S11/S12); ponto
/// genérico → 0 / 1 / várias (S10 / directo / S09). Uma Distribuição que
/// deixou de valer → S18 ou S39. O Core decide; aqui só se pergunta e se
/// mostra.
///
/// # Errors
///
/// [`Shell`] to render instead of the page.
pub async fn ensure_entry(
    state: &WorkspaceState,
    caller: &Caller<'_>,
) -> Result<Distributions, Shell> {
    let mine = match caller.get(state, "/api/v1/me/distributions").await {
        Ok(v) => v,
        Err(ApiFailure::Unauthorised) => return Err(Shell::SignIn),
        Err(other) => {
            return Err(Shell::Indeterminate(reference(&format!(
                "/me/distributions: {other}"
            ))))
        }
    };
    let enabled = dist_list(mine.get("enabled"));
    let parsed = parse_mine(&mine);
    if let (Some(stored), Some(why)) = (
        one_dist(mine.get("stored")),
        mine.get("stored_refusal").and_then(Value::as_str),
    ) {
        return Err(Shell::Entry(if why == "distribution_not_enabled" {
            Entry::DisabledLive(stored)
        } else {
            Entry::Revoked(stored)
        }));
    }
    let bound = crate::access::current().and_then(|e| e.bound());
    let target = match (bound, parsed.active) {
        // Um ponto fixo nunca mostra outra Distribuição.
        (Some(b), Some(a)) if a == b => return Ok(parsed),
        (Some(b), _) => {
            if !enabled.contains(&b) {
                return Err(Shell::Entry(Entry::BoundDisabled(b)));
            }
            if !parsed.accessible.contains(&b) {
                return Err(Shell::Entry(Entry::BoundNoAccess(b)));
            }
            b
        }
        (None, Some(_)) => return Ok(parsed),
        (None, None) => match parsed.accessible.as_slice() {
            [] => return Err(Shell::Entry(Entry::Zero)),
            [only] => *only,
            many => return Err(Shell::Entry(Entry::Choose(many.to_vec()))),
        },
    };
    enter(state, caller, target).await?;
    match caller.get(state, "/api/v1/me/distributions").await {
        Ok(v) => Ok(parse_mine(&v)),
        Err(ApiFailure::Unauthorised) => Err(Shell::SignIn),
        Err(other) => Err(Shell::Indeterminate(reference(&format!(
            "/me/distributions: {other}"
        )))),
    }
}

/// Entra numa Distribuição nesta sessão (o Core decide).
///
/// # Errors
///
/// [`Shell`] when the Core refuses or does not answer.
pub async fn enter(
    state: &WorkspaceState,
    caller: &Caller<'_>,
    d: ocinye_contracts::Distribution,
) -> Result<(), Shell> {
    match api::post(
        state,
        &caller.session.access_token,
        caller.correlation_id,
        "/api/v1/me/distribution",
        &serde_json::json!({ "distribution": d.as_str() }),
    )
    .await
    {
        Ok(_) => Ok(()),
        Err(ApiFailure::Unauthorised) => Err(Shell::SignIn),
        Err(ApiFailure::Forbidden | ApiFailure::Denied) => Err(Shell::Entry(Entry::Revoked(d))),
        // A recusa tipada do Core (a escolha mudou por baixo do pedido): é a
        // mesma página de quem a perdeu a meio, nunca uma sessão terminada.
        Err(ApiFailure::Refused { reason, .. }) if reason == "distribution_not_enabled" => {
            Err(Shell::Entry(Entry::DisabledLive(d)))
        }
        Err(ApiFailure::Refused { reason, .. }) if reason == "distribution_no_access" => {
            Err(Shell::Entry(Entry::Revoked(d)))
        }
        Err(other) => Err(Shell::Indeterminate(reference(&format!(
            "/me/distribution: {other}"
        )))),
    }
}

/// D010 · S15/S17: a mudança de Distribuição — só com mais de uma acessível.
/// Num ponto fixo não se muda aqui: os endereços **configurados** das outras
/// (o fixo dela, ou o genérico), nunca um URL vindo do pedido.
fn switch_of(
    state: &WorkspaceState,
    d: &Distributions,
) -> Option<crate::ui::view_models::DistSwitchVm> {
    use crate::ui::view_models::{BoundSwitchVm, DistSwitchVm};
    if d.accessible.len() < 2 {
        return None;
    }
    let to_vm = |x: ocinye_contracts::Distribution| distribution_of(x.as_str());
    let url = |host: &str| {
        format!(
            "{}/",
            crate::access::expected_origin(&state.config.public_url, host)
        )
    };
    let bound = crate::access::current()
        .and_then(|e| e.bound())
        .map(|_| BoundSwitchVm {
            generic_host: d.generic_host.clone(),
            targets: d
                .accessible
                .iter()
                .filter(|x| Some(**x) != d.active)
                .filter_map(|x| {
                    let host = d
                        .bound_hosts
                        .iter()
                        .find(|(b, _)| b == x)
                        .map(|(_, h)| h.clone())
                        .or_else(|| d.generic_host.clone())?;
                    Some((to_vm(*x)?, host.clone(), url(&host)))
                })
                .collect(),
        });
    Some(DistSwitchVm {
        choices: d.accessible.iter().filter_map(|x| to_vm(*x)).collect(),
        bound,
    })
}

/// D010 · S19–S21: o chip de contexto.
fn context_of(v: &Value) -> crate::ui::view_models::ContextVm {
    use crate::ui::view_models::{ContextItemVm, ContextVm};
    let item = |c: &Value| ContextItemVm {
        kind: c
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        id: c.get("id").and_then(Value::as_str).map(str::to_owned),
        name: c
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
    };
    ContextVm {
        active: v.get("active").filter(|a| !a.is_null()).map(item),
        items: v
            .get("contexts")
            .and_then(Value::as_array)
            .map(|a| a.iter().map(item).collect())
            .unwrap_or_default(),
        revoked: v.get("revoked").and_then(Value::as_str).map(str::to_owned),
    }
}

/// O resultado de estabelecer a identidade da sessão contra o Core.
pub enum Shell {
    /// Identidade estabelecida: a casca desenha-se.
    Ready(Box<ShellContext>),
    /// D010: a sessão ainda não está (ou já não está) numa Distribuição.
    Entry(Entry),
    /// O Core recusou o token: caminho de início de sessão.
    SignIn,
    /// O Core não deu resposta autoritária sobre a sessão. Falha fechado: nenhuma
    /// casca autenticada se desenha sobre uma identidade por estabelecer.
    Indeterminate(String),
}

fn strings(v: Option<&Value>) -> Vec<String> {
    v.and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// O fundo e o escurecimento da predefinição do sistema (o do protótipo D001).
pub const SYSTEM_WALLPAPER: Wallpaper = Wallpaper::Ocinye;
/// O escurecimento da predefinição do sistema.
pub const SYSTEM_DIM: u8 = 20;

/// Estabelece a identidade e prepara a casca para a aplicação em `active_href`.
///
/// `/me` é a única chamada cujo erro não se engole: uma falha técnica ao ler a
/// identidade **não** é prova de sessão normal (`ResolucaoSessao`). As outras
/// alimentam a barra e degradam-se para «não se sabe» (`None`), sem inventar.
pub async fn shell(
    state: &WorkspaceState,
    caller: &Caller<'_>,
    active_href: &str,
    crumb: String,
) -> Shell {
    // D010: antes de tudo, a Distribuição desta sessão (o Core decide).
    let dists = match ensure_entry(state, caller).await {
        Ok(d) => d,
        Err(shell) => return shell,
    };
    let (me, organisation, notifications, pins, desktop, probe, ai, compute, storage, contexts) = tokio::join!(
        caller.get(state, "/api/v1/me"),
        caller.get(state, "/api/v1/organisation"),
        caller.get(state, "/api/v1/notifications"),
        caller.get(state, "/api/v1/me/apps/pins"),
        caller.get(state, "/api/v1/me/desktop"),
        crate::boot::probe(state),
        caller.get(state, "/api/v1/ai/status"),
        caller.get(state, "/api/v1/compute/status"),
        caller.get(state, "/api/v1/me/files?view=recents&limit=1"),
        caller.get(state, "/api/v1/me/contexts"),
    );
    let me = match me {
        Ok(me) => me,
        Err(ApiFailure::Unauthorised) => return Shell::SignIn,
        Err(other) => return Shell::Indeterminate(reference(&format!("/me: {other}"))),
    };

    let core = match probe.state {
        ProbeState::Ready | ProbeState::Degraded => CoreStatus::Ok,
        ProbeState::Blocked => CoreStatus::Unavailable,
        _ => CoreStatus::Silent,
    };
    // D010: a Distribuição é a **activa desta sessão**, não «a» da Instância
    // (`organisations.profile` é só um espelho, ADR-0019).
    let perfil = dists.active.map(|d| d.as_str().to_owned());
    let distribution = perfil.as_deref().and_then(distribution_of);
    let roles = strings(me.get("roles"));
    let is_admin = roles
        .iter()
        .any(|r| r == "platform_admin" || r == "organisation_admin");
    // `pinned: null` (ou sem resposta): o membro nunca escolheu e aplica-se o
    // conjunto por omissão; uma lista, mesmo vazia, é a escolha dele.
    let pinned = pins
        .as_ref()
        .ok()
        .and_then(|p| p.get("pinned"))
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        // D009 · sem escolha do membro: as fixações da Distribuição.
        .unwrap_or_else(|| apps::default_pins_for(perfil.as_deref()));
    // D009 · o fundo de quem nunca personalizou é o da Distribuição (§59:
    // um fundo escolhido pelo membro, gravado, nunca é substituído).
    let (dist_wall, dist_dim) = crate::experience::distribution::look(
        perfil
            .as_deref()
            .and_then(crate::experience::distribution::parse),
    );
    let zone = me
        .get("timezone")
        .and_then(Value::as_str)
        .and_then(|z| ocinye_contracts::temporal::TimeZoneName::parse(z).ok())
        .unwrap_or_else(ocinye_contracts::temporal::TimeZoneName::utc);

    let viewer = Viewer {
        resolucao: ResolucaoSessao::Resolvida,
        zona: zone,
        name: caller.session.display_name.clone(),
        sessao_privilegiada: me.get("identity_kind").and_then(Value::as_str) == Some("privileged"),
        administra: is_admin,
        organisation: organisation
            .as_ref()
            .ok()
            .and_then(|o| o.get("name"))
            .and_then(Value::as_str)
            .unwrap_or("Ocinye OS")
            .to_owned(),
        email: me
            .get("email")
            .and_then(Value::as_str)
            .filter(|e| !e.is_empty())
            .map(str::to_owned),
        session_expires_in: None,
        avatar: me
            .get("avatar")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or(ocinye_contracts::AvatarChoice::Initials),
        core_status: core,
        unread: 0,
        modules: me
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
            .unwrap_or_default(),
        capabilities: strings(me.get("capabilities")),
        pinned: pinned.clone(),
        // D010 (ADR-0019 §9): também as que a Distribuição activa não traz.
        inactive_apps: strings(me.get("inactive_applications"))
            .into_iter()
            .chain(dists.inactive_here.iter().cloned())
            .collect(),
        perfil,
    };

    let tiles = apps::visible_to(&viewer, core)
        .into_iter()
        .map(|a| AppTile {
            id: a.id(),
            href: a.route(),
            label: a.label().to_owned(),
            description: a.description().to_owned(),
            pinned: pinned.iter().any(|p| p == a.id()),
            pinnable: a.can_pin(),
            active: a.route() == active_href,
        })
        .collect();

    let layout = desktop
        .as_ref()
        .ok()
        .and_then(|d| d.get("layout"))
        .filter(|l| !l.is_null());
    let wallpaper = layout
        .and_then(|l| l.get("wallpaper"))
        .and_then(Value::as_str)
        .and_then(Wallpaper::parse)
        .unwrap_or(dist_wall);
    let dim = layout
        .and_then(|l| l.get("dim"))
        .and_then(Value::as_u64)
        .and_then(|d| u8::try_from(d).ok())
        .unwrap_or(dist_dim);

    // D002: os painéis da barra de cima, com os factos que o Core já deu.
    let core_health = instance_health(&probe.state);
    let ai_health = ai.as_ref().ok().map(|s| {
        if s.get("available").and_then(Value::as_bool) == Some(true) {
            Health::Operational
        } else {
            Health::Unavailable
        }
    });
    // O Monitor é da administração da plataforma: só a ela o painel liga.
    let monitor = strings(me.get("capabilities"))
        .iter()
        .any(|c| c == "platform.administer");
    let clock = desktop::Clock {
        now: chrono::Utc::now(),
        zone,
        core_ok: core.operational(),
        is_admin: monitor,
    };
    let agenda = desktop::agenda_today(caller, state, &clock, panels::AGENDA).await;
    let top_panels = TopPanels {
        status: Some(panels::status(
            core_health,
            &compute,
            ai_health,
            &storage,
            monitor,
        )),
        notifications: Some(panels::notifications(&notifications, &clock)),
        clock: Some(panels::clock(&clock, agenda)),
    };

    let dist_switch = switch_of(state, &dists);
    let context = contexts.ok().map(|c| context_of(&c));
    let vm = ShellVm {
        display_name: me
            .get("display_name")
            .and_then(Value::as_str)
            .unwrap_or(&caller.session.display_name)
            .to_owned(),
        email: viewer
            .email
            .clone()
            .unwrap_or_else(|| caller.session.email.clone()),
        apps: tiles,
        unread: notifications
            .as_ref()
            .ok()
            .and_then(|n| n.get("unread"))
            .and_then(Value::as_u64)
            .and_then(|n| u32::try_from(n).ok()),
        core: Some(core_health),
        ai: ai_health,
        query: String::new(),
        distribution,
        crumb,
        wallpaper,
        dim,
        // As janelas entram por página (controllers::windows), depois do
        // portão da aplicação; sem janelas, `None` é a casca D001.
        wm: None,
        panels: top_panels,
        // D003: a superfície da Nye, fechada, com a disponibilidade real.
        nye: Some(nye::surface(
            nye::availability(
                nye::may_use_ai(&viewer),
                core.operational(),
                ai.as_ref().ok(),
            ),
            "",
            None,
            false,
        )),
        // D009 · a ordem da barra é a das fixações (do membro ou da
        // Distribuição), já filtrada pelo que este membro vê.
        dist_switch,
        context,
        pin_order: apps::pinned_visible(&pinned, &viewer, core)
            .into_iter()
            .map(|a| a.id())
            .collect(),
    };

    Shell::Ready(Box::new(ShellContext {
        vm,
        viewer,
        core,
        is_admin,
        distribution,
        desktop,
        zone,
        dists,
    }))
}
