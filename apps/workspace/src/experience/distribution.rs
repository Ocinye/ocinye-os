//! D009 · As predefinições de cada Distribuição — configuração de produto tipada.
//!
//! Uma Distribuição (`research`, `business`, `personal`, `education`) é um
//! conjunto de **predefinições** sobre o mesmo Ocinye OS: o mesmo Core, o mesmo
//! registo de aplicações, o mesmo motor de Desktop, a mesma segurança. Decide o
//! ponto de partida — fixações, widgets, fundo, texto de primeiros passos — e
//! nunca a autoridade.
//!
//! > **A Distribuição define o ponto de partida. O Core decide o que o membro pode fazer.**
//! > `DEFAULT ≠ AUTHORITY` · `NOT DEFAULT ≠ FORBIDDEN` · `VISIBLE DEFAULT ≠ AUTHORIZED`
//!
//! # A hierarquia (efectiva, de cima para baixo)
//!
//! ```text
//! disposição do membro      (member_desktop_layouts / member_app_pins; linha presente = personalizou)
//!   ?? predefinição da Instância   (FG-014: ainda não existe — nunca é inventada aqui)
//!   ?? predefinição da Distribuição (DEFAULTS, abaixo; versionada por DISTRIBUTION_DEFAULTS_VERSION)
//!   ?? predefinição do sistema      (SYSTEM_FALLBACK: sem widgets, fixações do manifesto)
//! ```
//!
//! # D009 R2 · uma Instância, uma ou várias Distribuições
//!
//! As predefinições são indexadas pelo **tipo** de Distribuição, nunca pela
//! Instância: uma Instância pode activar uma, algumas ou as quatro, e cada uma
//! traz o seu ponto de partida. Hoje o Core guarda um só perfil por Instância e
//! uma só disposição/lista de fixações por membro (`member_desktop_layouts`,
//! `member_app_pins`, chave `person_id`). Esse âmbito é compatível só enquanto
//! houver uma Distribuição activa; o âmbito-alvo é **membro + Instância +
//! Distribuição** (D010, contrato bloqueante G9-30/31). **Não codificar uma
//! Distribuição por Instância** em código novo: receber sempre a Distribuição
//! activa como argumento.
//!
//! Nada aqui é lido de um ficheiro, de um caminho ou de JSON: um valor de
//! Distribuição desconhecido nunca vira modelo nem caminho (§154–155) — cai na
//! predefinição do sistema, e não em Research (§116).

use ocinye_contracts::{ApplicationId as A, InstanceProfile};

use crate::ui::view_models::{Distribution, PlacedWidget, Wallpaper, WidgetKind as K};

/// A versão das predefinições de Distribuição que este Ocinye OS traz.
///
/// Sobe quando o Design muda uma disposição, as fixações ou o fundo de uma
/// Distribuição. Um membro **sem** linha gravada passa a seguir a nova versão;
/// um membro com linha gravada (personalizou) não é tocado (§23, §129).
/// D001–D008: 1. D009: 2.
pub const DISTRIBUTION_DEFAULTS_VERSION: u32 = 2;

/// As predefinições de uma Distribuição.
#[derive(Debug, Clone, Copy)]
pub struct DistributionDefaults {
    /// A Distribuição.
    pub distribution: Distribution,
    /// O símbolo do sprite (`static/icons.svg`) — nunca uma aplicação.
    pub icon: &'static str,
    /// Chave do nome (`dist.*`, já existente e invariável: «Research»…).
    pub name_key: &'static str,
    /// Chave da descrição curta (`shell.dist.*`, já existente).
    pub desc_key: &'static str,
    /// Chave do título dos primeiros passos.
    pub first_title_key: &'static str,
    /// Chave do texto dos primeiros passos.
    pub first_body_key: &'static str,
    /// As aplicações fixadas para quem nunca escolheu, por esta ordem.
    /// Filtradas pela visibilidade antes de desenhar: uma fixação sem
    /// autorização ou inactiva **cai**, e a seguinte ocupa o lugar (§37).
    pub pins: &'static [A],
    /// Sugeridas nos primeiros passos. Não fixadas, não activadas, não autorizadas.
    pub recommended: &'static [A],
    /// Aplicações de autoridade que **nunca** são fixadas por omissão; aparecem
    /// no Gestor de Aplicações só a quem o Core autoriza.
    pub privileged_conditional: &'static [A],
    /// O fundo por omissão.
    pub wallpaper: Wallpaper,
    /// O escurecimento por omissão (0–60, passos de 5).
    pub dim: u8,
    /// A disposição por omissão: (tipo, colunas, linhas). A ordem é a posição.
    pub widgets: &'static [(K, u8, u8)],
}

const PRIVILEGED: &[A] = &[A::Administration, A::Audit, A::Monitor];

/// As quatro, pela ordem canónica do produto.
pub const DEFAULTS: [DistributionDefaults; 4] = [
    DistributionDefaults {
        distribution: Distribution::Research,
        icon: "dist-research",
        name_key: "dist.research",
        desc_key: "shell.dist.research",
        first_title_key: "dist.first.research.title",
        first_body_key: "dist.first.research.body",
        pins: &[
            A::Work,
            A::Projects,
            A::Ideas,
            A::Datasets,
            A::Results,
            A::Knowledge,
            A::Files,
            A::Notes,
        ],
        recommended: &[A::Bibliography, A::Calendar, A::Messages, A::Compute],
        privileged_conditional: PRIVILEGED,
        wallpaper: Wallpaper::Field,
        dim: 20,
        // Decisão do Fidel (30 set 2026): a predefinição de investigação fica
        // só com os Indicadores e os Projectos.
        widgets: &[(K::Kpis, 4, 1), (K::Projects, 2, 1)],
    },
    DistributionDefaults {
        distribution: Distribution::Business,
        icon: "dist-business",
        name_key: "dist.business",
        desc_key: "shell.dist.business",
        first_title_key: "dist.first.business.title",
        first_body_key: "dist.first.business.body",
        pins: &[
            A::Work,
            A::Calendar,
            A::Mail,
            A::Messages,
            A::Projects,
            A::Files,
            A::Notes,
        ],
        recommended: &[A::Units, A::Activity, A::Browser],
        privileged_conditional: PRIVILEGED,
        wallpaper: Wallpaper::Module,
        dim: 20,
        widgets: &[
            (K::Tasks, 1, 2),
            (K::Calendar, 1, 2),
            (K::Projects, 2, 1),
            (K::Files, 2, 1),
        ],
    },
    DistributionDefaults {
        distribution: Distribution::Personal,
        icon: "dist-personal",
        name_key: "dist.personal",
        desc_key: "shell.dist.personal",
        first_title_key: "dist.first.personal.title",
        first_body_key: "dist.first.personal.body",
        pins: &[
            A::Files,
            A::Notes,
            A::Calendar,
            A::Work,
            A::Resources,
            A::Trash,
        ],
        recommended: &[A::Mail, A::Browser, A::Help],
        // Audit Log não é activado no perfil pessoal (ADR-0014 §4).
        privileged_conditional: &[A::Administration, A::Monitor],
        wallpaper: Wallpaper::Calm,
        dim: 20,
        widgets: &[
            (K::Notes, 2, 1),
            (K::Files, 2, 1),
            (K::Calendar, 2, 1),
            (K::Storage, 1, 1),
        ],
    },
    DistributionDefaults {
        distribution: Distribution::Education,
        icon: "dist-education",
        name_key: "dist.education",
        desc_key: "shell.dist.education",
        first_title_key: "dist.first.education.title",
        first_body_key: "dist.first.education.body",
        pins: &[
            A::Work,
            A::Units,
            A::Projects,
            A::Knowledge,
            A::Bibliography,
            A::Calendar,
            A::Files,
            A::Notes,
        ],
        recommended: &[A::Messages, A::Mail, A::Activity],
        privileged_conditional: PRIVILEGED,
        wallpaper: Wallpaper::Lattice,
        dim: 20,
        widgets: &[
            (K::Calendar, 1, 2),
            (K::Tasks, 1, 2),
            (K::Projects, 2, 1),
            (K::Notes, 2, 1),
        ],
    },
];

/// A predefinição do sistema: o último recurso, quando a Distribuição não se
/// conhece (o Core não respondeu, ou respondeu um valor fora do enum fechado).
/// Sem widgets (um Desktop vazio não mostra cartão nenhum — decisão do Fidel,
/// 30 set 2026), fundo Ocinye, e as fixações do manifesto.
pub struct SystemFallback;

impl SystemFallback {
    /// O fundo.
    pub const WALLPAPER: Wallpaper = Wallpaper::Ocinye;
    /// O escurecimento.
    pub const DIM: u8 = 20;
    /// Os widgets: nenhum.
    pub const WIDGETS: &'static [(K, u8, u8)] = &[];
}

/// As predefinições desta Distribuição.
#[must_use]
pub fn defaults(d: Distribution) -> &'static DistributionDefaults {
    DEFAULTS
        .iter()
        .find(|x| x.distribution == d)
        .expect("as quatro Distribuições estão em DEFAULTS")
}

/// O perfil do Core (`InstanceProfile`) que corresponde a esta Distribuição.
/// O Core ainda lhe chama `profile` (BACKEND_TERMINOLOGY_MIGRATION_REQUIRED).
#[must_use]
pub const fn profile(d: Distribution) -> InstanceProfile {
    match d {
        Distribution::Research => InstanceProfile::Research,
        Distribution::Business => InstanceProfile::Business,
        Distribution::Personal => InstanceProfile::Personal,
        Distribution::Education => InstanceProfile::Education,
    }
}

/// A Distribuição de um identificador persistido — `None` fora do enum fechado.
/// Quem recebe `None` usa [`SystemFallback`], nunca Research (§116).
#[must_use]
pub fn parse(s: &str) -> Option<Distribution> {
    s.parse::<InstanceProfile>().ok().map(|p| match p {
        InstanceProfile::Research => Distribution::Research,
        InstanceProfile::Business => Distribution::Business,
        InstanceProfile::Personal => Distribution::Personal,
        InstanceProfile::Education => Distribution::Education,
    })
}

fn place(&(kind, w, h): &(K, u8, u8)) -> PlacedWidget {
    PlacedWidget {
        id: kind.as_str().to_owned(),
        kind,
        w,
        h,
        minimized: false,
    }
}

/// A disposição por omissão: da Distribuição, ou a do sistema sem ela.
#[must_use]
pub fn widgets(d: Option<Distribution>) -> Vec<PlacedWidget> {
    d.map_or(SystemFallback::WIDGETS, |d| defaults(d).widgets)
        .iter()
        .map(place)
        .collect()
}

/// O fundo e o escurecimento por omissão.
#[must_use]
pub fn look(d: Option<Distribution>) -> (Wallpaper, u8) {
    d.map_or((SystemFallback::WALLPAPER, SystemFallback::DIM), |d| {
        let x = defaults(d);
        (x.wallpaper, x.dim)
    })
}

/// As fixações de quem nunca escolheu. Sem Distribuição: as do manifesto
/// (`ApplicationManifest::default_pin`). Substitui `apps::default_pins_for`.
#[must_use]
pub fn default_pins(d: Option<Distribution>) -> Vec<String> {
    match d {
        Some(d) => defaults(d)
            .pins
            .iter()
            .map(|a| a.as_str().to_owned())
            .collect(),
        None => crate::experience::apps::default_pins(),
    }
}

/// De onde vem o que o membro vê — o que a folha «Repor» e o painel da
/// Distribuição dizem (DIST-10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provenance {
    /// O membro personalizou (há linha gravada no Core).
    Member,
    /// Segue a predefinição publicada pela Instância **para esta Distribuição**
    /// (FG-014: ainda não existe; alvo: Instância + Distribuição, nunca uma só
    /// para todas).
    Instance,
    /// Segue a predefinição da Distribuição, nesta versão.
    Distribution(Distribution, u32),
    /// Segue a predefinição do sistema (Distribuição desconhecida).
    System,
}

/// A predefinição a que «Repor» regressa (DIST-13): a da Instância se houver
/// uma publicada; senão a da Distribuição; senão a do sistema. Nunca a da
/// Distribuição por cima de uma da Instância (§99).
#[must_use]
pub fn restore_target(instance_published: bool, d: Option<Distribution>) -> Provenance {
    match (instance_published, d) {
        (true, _) => Provenance::Instance,
        (false, Some(d)) => Provenance::Distribution(d, DISTRIBUTION_DEFAULTS_VERSION),
        (false, None) => Provenance::System,
    }
}

/// O que o membro segue agora (DIST-10).
#[must_use]
pub fn provenance(
    member_row: bool,
    instance_published: bool,
    d: Option<Distribution>,
) -> Provenance {
    if member_row {
        Provenance::Member
    } else {
        restore_target(instance_published, d)
    }
}

/// A que aplicação(ões) um widget vai buscar dados — para provar que nenhuma
/// predefinição traz um widget cuja aplicação a Distribuição não activa.
#[must_use]
pub const fn widget_apps(k: K) -> &'static [A] {
    match k {
        K::Kpis => &[A::Units, A::Ideas, A::Projects, A::Datasets],
        K::Notice | K::Health => &[],
        K::Continue => &[A::Notes, A::Files],
        K::Tasks => &[A::Work, A::Projects],
        K::Calendar => &[A::Calendar],
        K::Notes => &[A::Notes],
        K::Files | K::Storage => &[A::Files],
        K::Mail => &[A::Mail],
        K::Activity => &[A::Activity],
        K::Projects => &[A::Projects],
        K::Ideas => &[A::Ideas],
        K::Datasets => &[A::Datasets],
    }
}

/// Code (D009 · G9-09/G9-10): um widget desenha-se — e oferece-se na
/// biblioteca — quando o membro vê **pelo menos uma** das suas aplicações.
/// Um widget de várias (Indicadores, Tarefas, Continuar) não desaparece por
/// faltar uma: os Indicadores largam só a métrica dessa. Um sem aplicação
/// (Saúde) segue a sua própria regra.
#[must_use]
pub fn widget_shown(k: K, sees: impl Fn(A) -> bool) -> bool {
    let apps = widget_apps(k);
    apps.is_empty() || apps.iter().any(|a| sees(*a))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::screens::home::registry::{size_allowed, spec};

    const ALL: [Distribution; 4] = [
        Distribution::Research,
        Distribution::Business,
        Distribution::Personal,
        Distribution::Education,
    ];

    #[test]
    fn as_quatro_existem_uma_vez_e_voltam_a_si_mesmas() {
        assert_eq!(DEFAULTS.len(), 4);
        for d in ALL {
            assert_eq!(defaults(d).distribution, d);
            assert_eq!(parse(d.as_str()), Some(d));
            assert_eq!(profile(d).as_str(), d.as_str());
        }
        assert_eq!(
            parse("school"),
            None,
            "fora do enum fechado não vira Research"
        );
        assert_eq!(parse("../research"), None);
    }

    #[test]
    fn as_fixacoes_existem_sao_fixaveis_activas_unicas_e_contidas() {
        for d in ALL {
            let x = defaults(d);
            let mut vistos = std::collections::BTreeSet::new();
            assert!(
                (5..=8).contains(&x.pins.len()),
                "{d:?}: {} fixações",
                x.pins.len()
            );
            for app in x.pins {
                assert!(vistos.insert(app.as_str()), "{d:?}: {app} repetida");
                assert!(
                    crate::experience::apps::is_pinnable(app.as_str()),
                    "{d:?}: {app} não é fixável"
                );
                assert!(
                    profile(d).activates(*app),
                    "{d:?}: {app} fixada mas inactiva no perfil"
                );
                assert!(
                    !PRIVILEGED.contains(app),
                    "{d:?}: aplicação de autoridade fixada por omissão"
                );
                assert!(
                    !matches!(app, A::Terminal | A::Browser | A::Prompt),
                    "{d:?}: {app} fixada sem intenção"
                );
            }
            for app in x.recommended {
                assert!(!x.pins.contains(app), "{d:?}: {app} recomendada e fixada");
                assert!(
                    profile(d).activates(*app),
                    "{d:?}: {app} recomendada mas inactiva"
                );
            }
            for app in x.privileged_conditional {
                assert!(
                    profile(d).activates(*app),
                    "{d:?}: {app} condicional mas inactiva"
                );
            }
        }
    }

    #[test]
    fn os_widgets_existem_cabem_e_nao_dependem_de_aplicacoes_inactivas() {
        for d in ALL {
            let x = defaults(d);
            let mut vistos = std::collections::BTreeSet::new();
            let mut cells = 0u8;
            for &(k, w, h) in x.widgets {
                assert!(vistos.insert(k.as_str()), "{d:?}: {} repetido", k.as_str());
                assert!(size_allowed(k, w, h), "{d:?}: {} {w}×{h}", k.as_str());
                assert!(!spec(k).admin_only, "{d:?}: widget só de administração");
                assert_ne!(k, K::Notice, "{d:?}: avisos sem fonte de dados (FG-013)");
                for app in widget_apps(k) {
                    assert!(
                        profile(d).activates(*app),
                        "{d:?}: {} lê {app}, inactiva",
                        k.as_str()
                    );
                }
                cells += w * h;
            }
            assert!(cells <= 8, "{d:?}: a predefinição cabe em duas linhas");
        }
    }

    #[test]
    fn as_predefinicoes_passam_a_validacao_do_core() {
        for d in ALL.map(Some).into_iter().chain([None]) {
            let (wall, dim) = look(d);
            let layout = ocinye_contracts::desktop::DesktopLayout {
                wallpaper: wall.as_str().to_owned(),
                fit: "fill".to_owned(),
                dim,
                widgets: widgets(d)
                    .into_iter()
                    .map(|p| ocinye_contracts::desktop::PlacedWidget {
                        id: p.id,
                        kind: p.kind.as_str().to_owned(),
                        w: p.w,
                        h: p.h,
                        minimized: false,
                    })
                    .collect(),
            };
            assert_eq!(layout.validate(false), Ok(()), "{d:?}");
        }
    }

    #[test]
    fn cada_distribuicao_tem_fundo_proprio_e_texto_nas_tres_linguas() {
        let fundos: std::collections::BTreeSet<_> = ALL
            .iter()
            .map(|d| defaults(*d).wallpaper.as_str())
            .collect();
        assert_eq!(fundos.len(), 4);
        for d in ALL {
            let x = defaults(d);
            for k in [x.name_key, x.desc_key, x.first_title_key, x.first_body_key] {
                assert!(crate::i18n::has(k), "{d:?}: {k}");
            }
        }
    }

    #[test]
    fn repor_respeita_a_hierarquia() {
        assert_eq!(
            restore_target(true, Some(Distribution::Research)),
            Provenance::Instance
        );
        assert_eq!(
            restore_target(false, Some(Distribution::Business)),
            Provenance::Distribution(Distribution::Business, DISTRIBUTION_DEFAULTS_VERSION)
        );
        assert_eq!(restore_target(false, None), Provenance::System);
        assert_eq!(provenance(true, true, None), Provenance::Member);
        assert!(widgets(None).is_empty());
        assert_eq!(default_pins(None), crate::experience::apps::default_pins());
    }
}
