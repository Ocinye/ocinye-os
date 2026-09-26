//! O registo de aplicações do Ocinye OS — a fonte autoritativa do que existe.
//!
//! # Porque um registo, e não mais uma lista
//!
//! O Ocinye deixou de ser «uma aplicação web com uma barra lateral cada vez mais
//! cheia» para ser um ambiente com uma **camada de aplicações**: as aplicações
//! são entidades de primeira classe, descobertas e lançadas no Gestor de
//! Aplicações, e a barra lateral passa a ser navegação essencial mais as
//! aplicações que o membro **fixou** (uma fatia posterior).
//!
//! # A identidade da aplicação existe **uma vez**
//!
//! Cada aplicação é um [`Application`] que se apoia num [`Screen`] tipado — de
//! onde herda `id`, rota, rótulo, ícone e o direito que a revela — e acrescenta
//! o que é próprio da camada de aplicações: a **categoria**, a descrição, as
//! palavras de pesquisa e a política de fixação. Não há uma segunda tabela de
//! rótulos, rotas ou ícones: o lançador, a pesquisa e (depois) a barra lateral
//! lêem todos deste registo. Um teste prova que cada ecrã-aplicação tem aqui um
//! registo completo, e que nenhuma rota aponta para o vazio.
//!
//! # A descoberta nunca contorna a autorização
//!
//! A visibilidade de uma aplicação segue exactamente o mesmo filtro da barra
//! lateral — permissão institucional ou relevância de módulo, resolvidas pelo
//! Core em `GET /me` (`shell::screen_permission` / `shell::screen_module`). O
//! registo esconde o que o membro não pode alcançar; a autoridade continua no
//! Core, que recusa quem escrever a rota à mão (`CLAUDE.md` §4, §59).

use crate::ui::icon::Icon;
use crate::ui::shell::{screen_module, screen_permission, CoreStatus, Screen, Viewer};

/// A taxonomia de categorias — metadados de produto, nunca texto traduzido.
///
/// A identidade é semântica (`RESEARCH`); a apresentação resolve-se por chave
/// i18n (`Investigação` / `Research` / `Recherche`). «Todos» não é uma categoria:
/// é o filtro que mostra tudo, e vive só na interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    /// Produtividade pessoal — o que o membro usa para trabalhar todos os dias.
    Productivity,
    /// Investigação — unidades, ideias, projectos, dados e a inteligência.
    Research,
    /// Conhecimento — o acervo, os ficheiros e a bibliografia.
    Knowledge,
    /// Comunicação — correio e mensagens.
    Communication,
    /// Administração — recursos, definições e a consola institucional.
    Administration,
}

impl Category {
    /// O identificador técnico estável da categoria, para o atributo do filtro.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Productivity => "productivity",
            Self::Research => "research",
            Self::Knowledge => "knowledge",
            Self::Communication => "communication",
            Self::Administration => "administration",
        }
    }

    /// A chave i18n do rótulo da categoria.
    #[must_use]
    pub const fn label_key(self) -> &'static str {
        match self {
            Self::Productivity => "apps.category.productivity",
            Self::Research => "apps.category.research",
            Self::Knowledge => "apps.category.knowledge",
            Self::Communication => "apps.category.communication",
            Self::Administration => "apps.category.administration",
        }
    }

    /// As categorias, na ordem em que os filtros aparecem.
    #[must_use]
    pub const fn all() -> [Category; 5] {
        [
            Self::Productivity,
            Self::Research,
            Self::Knowledge,
            Self::Communication,
            Self::Administration,
        ]
    }
}

/// Uma aplicação do Ocinye OS.
///
/// Apoia-se num [`Screen`] tipado (identidade, rota, rótulo, ícone, direito) e
/// acrescenta o que a camada de aplicações precisa.
#[derive(Debug, Clone, Copy)]
pub struct Application {
    /// O ecrã que esta aplicação abre — a origem única da identidade e da rota.
    pub screen: Screen,
    /// A categoria a que pertence, para o filtro do lançador.
    pub category: Category,
    /// A chave i18n de uma descrição curta, mostrada na ficha do lançador.
    pub description_key: &'static str,
    /// Palavras de pesquisa estáveis, independentes do idioma, para além do
    /// rótulo e da descrição já traduzidos — para que «file», «fichier» e
    /// «ficheiro» encontrem todos os Ficheiros.
    pub keywords: &'static [&'static str],
    /// Se o membro pode fixar esta aplicação na barra lateral. As estruturais
    /// (Home, O Meu Trabalho) não se fixam — já são navegação essencial.
    pub can_pin: bool,
    /// Se entra na barra lateral por omissão, antes de o membro escolher.
    pub default_pin: bool,
}

impl Application {
    /// O identificador técnico estável — o do ecrã que abre.
    #[must_use]
    pub const fn id(&self) -> &'static str {
        self.screen.id()
    }

    /// A rota canónica que o lançador abre.
    #[must_use]
    pub const fn route(&self) -> &'static str {
        self.screen.path()
    }

    /// O ícone da ficha.
    #[must_use]
    pub const fn icon(&self) -> Icon {
        self.screen.icon()
    }

    /// O rótulo no idioma corrente.
    #[must_use]
    pub fn label(&self) -> &'static str {
        self.screen.label()
    }

    /// A descrição no idioma corrente.
    #[must_use]
    pub fn description(&self) -> &'static str {
        crate::i18n::t(self.description_key)
    }

    /// Se esta aplicação é visível a este membro, com o Core no estado dado.
    ///
    /// Espelha exactamente o filtro da barra lateral: um módulo governado por
    /// contentor aparece por relevância; um ecrã com direito institucional
    /// aparece quando o membro o tem; um ecrã sem direito aparece a qualquer
    /// membro autenticado. Sem o Core, encolhe ao que não exige direito nenhum —
    /// não sabemos o que o membro pode, e não se afirma um acesso por verificar.
    ///
    /// O lançador **não** mostra o que o membro não pode abrir: uma grelha de
    /// aplicações que recusam à entrada seria ruído, e anunciar a consola de
    /// administração a quem não a tem seria um oráculo de existência (§25, §59).
    #[must_use]
    pub fn visible_to(&self, viewer: &Viewer, core: CoreStatus) -> bool {
        // Uma aplicação que a Instância desactivou não se oferece a ninguém
        // (ADR-0014). Desactivar não é desinstalar: os dados ficam, e a
        // autorização continua a decidir o resto.
        if viewer.inactive_apps.iter().any(|id| id == self.id()) {
            return false;
        }
        match screen_module(self.screen) {
            // Governado dentro de um contentor: presença por relevância, e só
            // com o Core a confirmar o que o membro alcança.
            Some(m) => core.operational() && viewer.modules.iter().any(|rel| rel == m),
            None => match screen_permission(self.screen) {
                None => true,
                Some(_) if !core.operational() => false,
                Some(p) => viewer.can(p),
            },
        }
    }
}

/// O registo — uma entrada por aplicação, agrupada por categoria na ordem em que
/// se quer ver a grelha «Todos».
///
/// A ordem dentro de cada categoria é deliberada: o que se usa mais primeiro.
pub const APPLICATIONS: &[Application] = &[
    // ── Produtividade ────────────────────────────────────────────────────
    Application {
        screen: Screen::Notes,
        category: Category::Productivity,
        description_key: "apps.desc.notes",
        keywords: &["note", "notes", "nota", "nota", "notas", "editor"],
        can_pin: true,
        default_pin: true,
    },
    Application {
        screen: Screen::Calendar,
        category: Category::Productivity,
        description_key: "apps.desc.calendar",
        keywords: &[
            "calendar",
            "calendário",
            "calendrier",
            "evento",
            "event",
            "agenda",
        ],
        can_pin: true,
        default_pin: false,
    },
    Application {
        screen: Screen::MyWork,
        category: Category::Productivity,
        description_key: "apps.desc.work",
        keywords: &["work", "trabalho", "travail", "tarefas", "tasks"],
        can_pin: false,
        default_pin: false,
    },
    Application {
        screen: Screen::Home,
        category: Category::Productivity,
        description_key: "apps.desc.home",
        keywords: &["home", "início", "accueil", "painel", "dashboard"],
        can_pin: false,
        default_pin: false,
    },
    // ── Comunicação ──────────────────────────────────────────────────────
    Application {
        screen: Screen::Mail,
        category: Category::Communication,
        description_key: "apps.desc.mail",
        keywords: &["mail", "correio", "courrier", "email", "e-mail"],
        can_pin: true,
        default_pin: false,
    },
    Application {
        screen: Screen::Messaging,
        category: Category::Communication,
        description_key: "apps.desc.messages",
        keywords: &[
            "message",
            "messages",
            "mensagem",
            "mensagens",
            "chat",
            "equipa",
        ],
        can_pin: true,
        default_pin: false,
    },
    // ── Conhecimento ─────────────────────────────────────────────────────
    Application {
        screen: Screen::Files,
        category: Category::Knowledge,
        description_key: "apps.desc.files",
        keywords: &[
            "file",
            "files",
            "ficheiro",
            "ficheiros",
            "fichier",
            "fichiers",
            "dossier",
            "upload",
        ],
        can_pin: true,
        default_pin: true,
    },
    Application {
        screen: Screen::Knowledge,
        category: Category::Knowledge,
        description_key: "apps.desc.knowledge",
        keywords: &[
            "knowledge",
            "conhecimento",
            "connaissance",
            "acervo",
            "base",
        ],
        can_pin: true,
        default_pin: false,
    },
    Application {
        screen: Screen::Bibliography,
        category: Category::Knowledge,
        description_key: "apps.desc.bibliography",
        keywords: &[
            "bibliography",
            "bibliografia",
            "bibliographie",
            "referência",
            "reference",
            "source",
        ],
        can_pin: true,
        default_pin: false,
    },
    // ── Investigação ─────────────────────────────────────────────────────
    Application {
        screen: Screen::Units,
        category: Category::Research,
        description_key: "apps.desc.units",
        keywords: &["unit", "units", "unidade", "unidades", "unité", "unités"],
        can_pin: true,
        default_pin: false,
    },
    Application {
        screen: Screen::Ideas,
        category: Category::Research,
        description_key: "apps.desc.ideas",
        keywords: &["idea", "ideas", "ideia", "ideias", "idée", "idées"],
        can_pin: true,
        default_pin: false,
    },
    Application {
        screen: Screen::Projects,
        category: Category::Research,
        description_key: "apps.desc.projects",
        keywords: &[
            "project",
            "projects",
            "projecto",
            "projectos",
            "projet",
            "projets",
        ],
        can_pin: true,
        default_pin: true,
    },
    Application {
        screen: Screen::Datasets,
        category: Category::Research,
        description_key: "apps.desc.datasets",
        keywords: &["dataset", "datasets", "dados", "données", "data"],
        can_pin: true,
        default_pin: false,
    },
    Application {
        screen: Screen::Prompt,
        category: Category::Research,
        description_key: "apps.desc.prompt",
        keywords: &[
            "prompt",
            "ai",
            "ia",
            "assistente",
            "assistant",
            "inteligência",
        ],
        can_pin: true,
        default_pin: false,
    },
    Application {
        screen: Screen::Ai,
        category: Category::Research,
        description_key: "apps.desc.ai",
        keywords: &["ai", "ia", "inteligência", "intelligence", "hub"],
        can_pin: true,
        default_pin: false,
    },
    Application {
        screen: Screen::Agents,
        category: Category::Research,
        description_key: "apps.desc.agents",
        keywords: &["agent", "agents", "agente", "agentes"],
        can_pin: true,
        default_pin: false,
    },
    Application {
        screen: Screen::Compute,
        category: Category::Research,
        description_key: "apps.desc.compute",
        keywords: &["compute", "computação", "calcul", "gpu", "nó", "node"],
        can_pin: true,
        default_pin: false,
    },
    // ── Administração ────────────────────────────────────────────────────
    Application {
        screen: Screen::Resources,
        category: Category::Administration,
        description_key: "apps.desc.resources",
        keywords: &[
            "resource",
            "resources",
            "recurso",
            "recursos",
            "ressource",
            "quota",
        ],
        can_pin: true,
        default_pin: false,
    },
    Application {
        screen: Screen::Activity,
        category: Category::Administration,
        description_key: "apps.desc.activity",
        keywords: &["activity", "actividade", "activité", "feed"],
        can_pin: true,
        default_pin: false,
    },
    Application {
        screen: Screen::Admin,
        category: Category::Administration,
        description_key: "apps.desc.administration",
        keywords: &[
            "admin",
            "administração",
            "administration",
            "membros",
            "members",
            "consola",
        ],
        can_pin: true,
        default_pin: false,
    },
    Application {
        screen: Screen::Audit,
        category: Category::Administration,
        description_key: "apps.desc.audit",
        keywords: &["audit", "auditoria", "log", "registo"],
        can_pin: true,
        default_pin: false,
    },
    Application {
        screen: Screen::Settings,
        category: Category::Administration,
        description_key: "apps.desc.settings",
        keywords: &[
            "settings",
            "definições",
            "paramètres",
            "preferências",
            "idioma",
            "language",
        ],
        can_pin: true,
        default_pin: false,
    },
    Application {
        screen: Screen::Help,
        category: Category::Administration,
        description_key: "apps.desc.help",
        keywords: &["help", "ajuda", "aide", "suporte", "support"],
        can_pin: true,
        default_pin: false,
    },
];

/// As aplicações visíveis a este membro, na ordem do registo.
///
/// O que o lançador mostra: cada aplicação que o membro pode abrir, filtrada
/// pela mesma política da barra lateral. Não decide autorização — informa a
/// descoberta.
#[must_use]
pub fn visible_to(viewer: &Viewer, core: CoreStatus) -> Vec<&'static Application> {
    APPLICATIONS
        .iter()
        .filter(|app| app.visible_to(viewer, core))
        .collect()
}

/// A aplicação com este identificador técnico, se existir.
#[must_use]
pub fn by_id(id: &str) -> Option<&'static Application> {
    APPLICATIONS.iter().find(|app| app.id() == id)
}

/// O conjunto de aplicações fixadas por omissão, na ordem do registo.
///
/// O que um membro novo (ou um que nunca escolheu) vê na barra lateral, antes de
/// fixar ou desafixar o que quer que seja.
#[must_use]
pub fn default_pins() -> Vec<String> {
    APPLICATIONS
        .iter()
        .filter(|a| a.default_pin)
        .map(|a| a.id().to_owned())
        .collect()
}

/// Se uma aplicação existe **e** é fixável — o que o Workspace valida antes de
/// pedir ao Core para guardar a lista.
#[must_use]
pub fn is_pinnable(id: &str) -> bool {
    by_id(id).is_some_and(|a| a.can_pin)
}

/// As aplicações fixadas visíveis a este membro, na ordem em que fixou.
///
/// Resolve os identificadores fixados contra o registo, deixando cair os que já
/// não existem ou que o membro não pode abrir — a barra nunca oferece uma ficha
/// para um ecrã sem autorização, nem para um id que o registo já não conhece.
#[must_use]
pub fn pinned_visible<'a>(
    pinned: &'a [String],
    viewer: &'a Viewer,
    core: CoreStatus,
) -> Vec<&'static Application> {
    pinned
        .iter()
        .filter_map(|id| by_id(id))
        .filter(|app| app.visible_to(viewer, core))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routes::ROUTES;

    /// Uma rota corresponde a uma registada, incluindo parâmetros `{…}`.
    fn matches_route(path: &str) -> bool {
        ROUTES.iter().any(|route| {
            let r: Vec<&str> = route.split('/').collect();
            let p: Vec<&str> = path.split('/').collect();
            r.len() == p.len() && r.iter().zip(&p).all(|(r, p)| r.starts_with('{') || r == p)
        })
    }

    /// Nenhuma aplicação aponta para o vazio: cada rota do registo existe.
    #[test]
    fn cada_aplicacao_tem_rota_registada() {
        let mortas: Vec<&str> = APPLICATIONS
            .iter()
            .map(Application::route)
            .filter(|route| !matches_route(route))
            .collect();
        assert!(
            mortas.is_empty(),
            "aplicações com rota inexistente: {mortas:?}"
        );
    }

    /// Cada aplicação tem rótulo e descrição no catálogo, nas três línguas.
    ///
    /// A completude `pt`/`en`/`fr` de cada chave é garantida pelo portão de
    /// completude do catálogo; aqui prova-se que a **chave existe** — um
    /// `apps.desc.*` mal escrito mostraria a chave crua a um membro.
    #[test]
    fn cada_aplicacao_tem_rotulo_e_descricao_no_catalogo() {
        let mut faltam = Vec::new();
        for app in APPLICATIONS {
            if !crate::i18n::has(app.screen.label_key()) {
                faltam.push(format!("{}: rótulo {}", app.id(), app.screen.label_key()));
            }
            if !crate::i18n::has(app.description_key) {
                faltam.push(format!("{}: descrição {}", app.id(), app.description_key));
            }
        }
        assert!(
            faltam.is_empty(),
            "chaves i18n em falta:\n  {}",
            faltam.join("\n  ")
        );
    }

    /// Os identificadores são únicos: um registo com duas entradas do mesmo id
    /// faria a fixação e a pesquisa ambíguas.
    #[test]
    fn os_identificadores_sao_unicos() {
        let mut vistos = std::collections::BTreeSet::new();
        for app in APPLICATIONS {
            assert!(
                vistos.insert(app.id()),
                "id repetido no registo: {}",
                app.id()
            );
        }
    }

    /// A superfície de comando (`Search`/`Ask`) não é uma aplicação do lançador:
    /// pesquisar/perguntar/executar é outra superfície (§36). O lançador cobre
    /// todos os outros ecrãs.
    #[test]
    fn a_superficie_de_comando_nao_e_uma_aplicacao() {
        assert!(
            by_id("search").is_none(),
            "Search não é uma aplicação do lançador"
        );
        assert!(
            by_id("ask").is_none(),
            "Ask não é uma aplicação do lançador"
        );
        // Vinte e cinco ecrãs menos os dois da superfície de comando.
        assert_eq!(
            APPLICATIONS.len(),
            23,
            "o registo deixou de cobrir todos os ecrãs"
        );
    }

    /// A política de fixação é coerente: o que entra por omissão pode ser fixado,
    /// e as estruturais (Home, O Meu Trabalho) não se fixam — já são navegação.
    #[test]
    fn a_politica_de_fixacao_e_coerente() {
        for app in APPLICATIONS {
            if app.default_pin {
                assert!(
                    app.can_pin,
                    "{} entra por omissão mas não é fixável",
                    app.id()
                );
            }
        }
        assert!(!by_id("home").expect("home").can_pin, "Home não se fixa");
        assert!(
            !by_id("work").expect("work").can_pin,
            "O Meu Trabalho não se fixa"
        );
        // O conjunto por omissão existe e é pequeno.
        let por_omissao: Vec<&str> = APPLICATIONS
            .iter()
            .filter(|a| a.default_pin)
            .map(Application::id)
            .collect();
        assert_eq!(por_omissao, vec!["notes", "files", "projects"]);
    }

    /// Toda a aplicação pertence a uma categoria conhecida — e cada categoria
    /// tem pelo menos uma aplicação, para nenhum filtro abrir no vazio.
    #[test]
    fn cada_categoria_tem_aplicacoes() {
        for categoria in Category::all() {
            assert!(
                APPLICATIONS.iter().any(|a| a.category == categoria),
                "a categoria {} não tem nenhuma aplicação",
                categoria.id()
            );
        }
    }

    /// O registo do Workspace e o catálogo que o Core valida nomeiam as mesmas
    /// aplicações, nos dois sentidos (ADR-0014). Uma aplicação que só um dos dois
    /// conhecesse seria impossível de activar, ou activável sem ecrã.
    #[test]
    fn o_registo_e_o_catalogo_do_core_nomeiam_as_mesmas_aplicacoes() {
        use ocinye_contracts::ApplicationId;
        for app in APPLICATIONS {
            assert!(
                app.id().parse::<ApplicationId>().is_ok(),
                "{} está no registo e não no catálogo do Core",
                app.id()
            );
        }
        for id in ApplicationId::ALL {
            assert!(
                by_id(id.as_str()).is_some(),
                "{id} está no catálogo do Core e não no registo"
            );
        }
    }

    /// Uma aplicação que a Instância desactivou não aparece no lançador, nem
    /// fixada — e as outras continuam lá.
    #[test]
    fn uma_aplicacao_inactiva_nao_se_oferece() {
        let mut viewer = crate::ui::render_tests::viewer_completo();
        let antes = visible_to(&viewer, CoreStatus::Ok);
        assert!(antes.iter().any(|a| a.id() == "notes"));

        viewer.inactive_apps = vec!["notes".to_owned()];
        let depois = visible_to(&viewer, CoreStatus::Ok);
        assert!(!depois.iter().any(|a| a.id() == "notes"));
        assert_eq!(depois.len(), antes.len() - 1, "só a inactiva saiu");

        let fixadas = pinned_visible(&["notes".to_owned(), "files".to_owned()], &viewer, CoreStatus::Ok);
        assert_eq!(fixadas.iter().map(|a| a.id()).collect::<Vec<_>>(), vec!["files"]);
    }
}
