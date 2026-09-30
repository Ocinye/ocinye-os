//! O modelo de navegação do Workspace: ecrãs, estado do Core e quem está a ver.
//!
//! Não é apresentação. Saiu de `ui::shell` quando a UI foi apagada, porque o
//! registo de aplicações e as rotas dependem dele para decidir o que existe e a
//! quem se oferece.

use ocinye_contracts::AvatarChoice;
use ocinye_contracts::Permission;

use crate::experience::icon::Icon;

/// Um ecrã da navegação.
///
/// Enumeração fechada em vez de string: um destino mal escrito passaria em
/// silêncio e nenhum item ficaria marcado como activo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    /// Painel inicial.
    Home,
    /// O trabalho atribuído ao membro.
    MyWork,
    /// Notas pessoais.
    Notes,
    /// Correio institucional.
    Mail,
    /// Mensagens entre membros.
    Messaging,
    /// Os recursos do próprio membro — quota de armazenamento e entitlements.
    Resources,
    /// Unidades científicas.
    Units,
    /// Ideias.
    Ideas,
    /// Projectos.
    Projects,
    /// Hub de conhecimento.
    Knowledge,
    /// Bibliografia.
    Bibliography,
    /// Datasets.
    Datasets,
    /// Ficheiros institucionais.
    Files,
    /// Hub de IA.
    Ai,
    /// Agentes de IA.
    Agents,
    /// Computação.
    Compute,
    /// Calendário e Centro Temporal.
    Calendar,
    /// Feed institucional.
    Activity,
    /// Administração.
    Admin,
    /// Registo de auditoria.
    Audit,
    /// Nye (o antigo Prompt Ocinye; o `id` continua `prompt`).
    Prompt,
    /// Pesquisa institucional.
    Search,
    /// A Universal Command Surface.
    Ask,
    /// Definições do próprio membro.
    Settings,
    /// Ajuda do Workspace.
    Help,
    /// O Terminal (ocsh).
    Terminal,
    /// D008 · O Ocinye Browser.
    Browser,
    /// D007.1 · Monitor de Actividade.
    Monitor,
    /// D007.1 · Resultados.
    Results,
    /// D007.1 · Lixo pessoal.
    Trash,
}

impl Screen {
    /// O identificador técnico estável do ecrã.
    ///
    /// Nunca é o rótulo traduzido: o registo de aplicações e as preferências do
    /// membro (as fixações) referem-se a um ecrã por este `id`, que não muda com
    /// o idioma nem com o texto visível. `files`, nunca `Ficheiros`.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Home => "home",
            Self::MyWork => "work",
            Self::Notes => "notes",
            Self::Mail => "mail",
            Self::Messaging => "messages",
            Self::Resources => "resources",
            Self::Units => "units",
            Self::Ideas => "ideas",
            Self::Projects => "projects",
            Self::Knowledge => "knowledge",
            Self::Bibliography => "bibliography",
            Self::Datasets => "datasets",
            Self::Files => "files",
            Self::Ai => "ai",
            Self::Agents => "agents",
            Self::Compute => "compute",
            Self::Calendar => "calendar",
            Self::Activity => "activity",
            Self::Admin => "administration",
            Self::Audit => "audit",
            Self::Prompt => "prompt",
            Self::Search => "search",
            Self::Ask => "ask",
            Self::Settings => "settings",
            Self::Help => "help",
            Self::Terminal => "terminal",
            Self::Browser => "browser",
            Self::Monitor => "monitor",
            Self::Results => "results",
            Self::Trash => "trash",
        }
    }

    /// O caminho do ecrã.
    #[must_use]
    pub const fn path(self) -> &'static str {
        match self {
            Self::Home => "/",
            Self::MyWork => "/my-work",
            Self::Notes => "/notes",
            Self::Mail => "/mail",
            Self::Messaging => "/messages",
            Self::Resources => "/resources",
            Self::Units => "/units",
            Self::Ideas => "/ideas",
            Self::Projects => "/projects",
            Self::Knowledge => "/knowledge",
            Self::Bibliography => "/bibliography",
            Self::Datasets => "/datasets",
            Self::Files => "/files",
            Self::Ai => "/ai",
            Self::Agents => "/ai/agents",
            Self::Compute => "/compute",
            Self::Calendar => "/calendar",
            Self::Activity => "/activity",
            Self::Admin => "/admin",
            Self::Audit => "/audit",
            Self::Prompt => "/ai/prompt",
            Self::Search => "/search",
            Self::Ask => "/ask",
            Self::Settings => "/settings",
            Self::Help => "/help",
            Self::Terminal => "/terminal",
            Self::Browser => "/browser",
            Self::Monitor => "/admin/monitor",
            Self::Results => "/results",
            Self::Trash => "/trash",
        }
    }

    /// A chave i18n do rótulo do ecrã.
    ///
    /// O rótulo em si resolve-se no idioma corrente por [`Screen::label`]; a chave
    /// fica à parte para os poucos sítios que precisam dela sem o texto.
    #[must_use]
    pub const fn label_key(self) -> &'static str {
        match self {
            Self::Help => "nav.help",
            Self::Terminal => "terminal.app",
            Self::Browser => "browser.app",
            Self::Monitor => "nav.monitor",
            Self::Results => "nav.results",
            Self::Trash => "nav.trash",
            Self::Settings => "nav.settings",
            Self::Home => "nav.home",
            Self::MyWork => "nav.my_work",
            Self::Notes => "nav.notes",
            Self::Mail => "nav.mail",
            Self::Messaging => "nav.messages",
            Self::Resources => "nav.resources",
            Self::Units => "nav.units",
            Self::Ideas => "nav.ideas",
            Self::Projects => "nav.projects",
            Self::Knowledge => "nav.knowledge",
            Self::Bibliography => "nav.bibliography",
            Self::Datasets => "nav.data",
            Self::Files => "nav.files",
            Self::Ai => "nav.ai",
            Self::Agents => "nav.agents",
            Self::Compute => "nav.compute",
            Self::Calendar => "nav.calendar",
            Self::Activity => "nav.activity",
            Self::Admin => "nav.admin",
            Self::Audit => "nav.audit",
            Self::Prompt => "nav.prompt",
            Self::Search => "nav.search",
            Self::Ask => "nav.ask",
        }
    }

    /// O rótulo do ecrã no idioma corrente, para navegação, breadcrumb e título.
    #[must_use]
    pub fn label(self) -> &'static str {
        crate::i18n::t(self.label_key())
    }

    /// O ícone do ecrã, para navegação e para a ficha no lançador.
    #[must_use]
    pub const fn icon(self) -> Icon {
        self.icon_kind()
    }

    const fn icon_kind(self) -> Icon {
        match self {
            Self::Help => Icon::Help,
            Self::Terminal => Icon::SystemStatus,
            Self::Browser => Icon::Browser,
            Self::Monitor => Icon::Activity,
            Self::Results => Icon::Science,
            Self::Trash => Icon::Trash,
            Self::Settings => Icon::Settings,
            Self::Home => Icon::Home,
            Self::MyWork => Icon::MyWork,
            Self::Notes => Icon::Document,
            Self::Calendar => Icon::Calendar,
            Self::Mail => Icon::Mail,
            Self::Messaging => Icon::Messaging,
            Self::Resources => Icon::Compute,
            Self::Units => Icon::Units,
            Self::Ideas => Icon::Idea,
            Self::Projects => Icon::Project,
            Self::Knowledge => Icon::Knowledge,
            Self::Bibliography => Icon::Bibliography,
            Self::Datasets => Icon::Data,
            Self::Files => Icon::Files,
            Self::Ai | Self::Prompt => Icon::Ai,
            Self::Search => Icon::Search,
            Self::Ask => Icon::Ai,
            Self::Agents => Icon::Agent,
            Self::Compute => Icon::Compute,
            Self::Activity => Icon::Activity,
            Self::Admin => Icon::Admin,
            Self::Audit => Icon::Audit,
        }
    }
}

/// O que a topbar diz sobre o Core.
///
/// # A mesma verdade do arranque, num momento diferente
///
/// O arranque é o ciclo de entrada; isto é observação contínua. Consomem a
/// mesma fonte factual — o `/ready` — e diferem apenas em quando perguntam.
///
/// O que **não** pode acontecer é isto ser inferido de outra coisa. Houve uma
/// altura em que era: `!organisation.is_null()`, ou seja, «se o pedido de
/// organização respondeu, o Core está bem». Um pedido de domínio responde por
/// razões suas, e uma delas não é a prontidão institucional.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreStatus {
    /// Pronto.
    ///
    /// Inclui a instalação cujo `/ready` responde `degraded` por capacidades
    /// opcionais. O Core está inteiro; o que falta é correio, inferência ou
    /// computação, e isso diz-se onde se fala da instalação.
    Ok,
    /// O Core respondeu que não está em condições.
    Unavailable,
    /// Não houve resposta.
    Silent,
}

impl CoreStatus {
    /// Se o Core está em condições de servir pedidos.
    #[must_use]
    pub const fn operational(&self) -> bool {
        matches!(self, Self::Ok)
    }
}

/// Como correu o estabelecimento da identidade da sessão contra o Core.
///
/// # Porque isto existe, e porque falha fechado
///
/// A shell privilegiada é uma afirmação sobre a sessão. Se o Core respondeu
/// `200` e disse o que a sessão é, a afirmação é fiável — normal ou privilegiada.
/// Se o Core **não** respondeu — 5xx, tempo esgotado, rede, erro de base de
/// dados — não sabemos o que a sessão é, e a resposta segura é não afirmar nada:
/// não desenhar a shell autenticada normal, porque uma falha em estabelecer a
/// identidade **não é prova de que a sessão é normal**. Era esse o defeito: o
/// erro do `/me` era engolido para `Null`, e `Null` desenhava uma sessão normal.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ResolucaoSessao {
    /// O `/me` respondeu e a identidade foi estabelecida (normal ou privilegiada).
    #[default]
    Resolvida,
    /// O Core respondeu `401`: a sessão não está autenticada. Caminho de login.
    NaoAutenticada,
    /// O Core não deu uma resposta autoritária (5xx, tempo esgotado, rede, erro
    /// técnico). A identidade não pôde ser estabelecida. Falha fechado.
    Indeterminada,
}

/// O que a shell precisa de saber sobre quem está a usá-la.
#[derive(Debug, Clone)]
pub struct Viewer {
    /// Como correu o estabelecimento da identidade contra o Core.
    ///
    /// Falha fechado: uma falha técnica do `/me` **não** desenha a shell normal.
    pub resolucao: ResolucaoSessao,
    /// A zona em que este membro está a olhar para o sistema.
    ///
    /// Vem do browser. Decide em que dia civil as coisas caem — e é por isso
    /// que está aqui, no que a casca inteira já recebe, em vez de ser passada a
    /// cada ecrã que dela precise.
    pub zona: ocinye_contracts::temporal::TimeZoneName,
    /// Nome do membro.
    pub name: String,
    /// Se esta sessão foi iniciada por uma identidade privilegiada.
    ///
    /// # Porque isto vem do Core e não daqui
    ///
    /// Porque a Experience não sabe — nem deve saber — o que faz uma identidade
    /// ser privilegiada. Inferi-lo de um sufixo no endereço, do nome, ou da
    /// presença de `PlatformAdmin` seria a interface a inventar uma segunda
    /// definição, e a primeira vez que divergisse da do Core teríamos uma sessão
    /// tratada como normal quando não o é.
    ///
    /// > **A faixa representa a sessão. A faixa não autoriza a sessão.**
    pub sessao_privilegiada: bool,
    /// Se essa identidade tem **agora** autoridade de administração.
    ///
    /// Separado do anterior de propósito. Revogado o `PlatformAdmin`, a sessão
    /// continua privilegiada — e deixa de poder chamar-se «Super Admin».
    pub administra: bool,
    /// Instituição, para o wordmark.
    pub organisation: String,
    /// O endereço institucional do membro.
    ///
    /// É a identidade **e** a credencial desde o ADR-0106. Havia aqui um
    /// `username` ao lado dele, e o painel mostrava a mesma pessoa duas vezes:
    /// `@fidel` e `fidel@ocinye.com`.
    ///
    /// Vem do registo da própria pessoa, e não do principal: autorização e
    /// identidade são coisas diferentes, e o principal só carrega a primeira.
    /// `None` quando o Core não respondeu — ausente é diferente de vazio, e a
    /// interface omite a linha em vez de mostrar um espaço.
    pub email: Option<String>,
    /// Quanto falta para a sessão do Workspace expirar.
    ///
    /// É a sessão **deste** processo, não a do Core. Mostra-se como duração
    /// restante e não como instante, porque é isso que o `Instant` guardado
    /// sabe dizer sem inventar um fuso horário.
    pub session_expires_in: Option<std::time::Duration>,
    /// Como o membro escolheu ser representado.
    ///
    /// Vem do Core. Sem resposta dele ficam as iniciais — que é o estado certo:
    /// não saber qual é a escolha não é razão para inventar uma.
    pub avatar: AvatarChoice,
    /// Se o Ocinye Core está a responder.
    /// O que o Core respondeu, quando esta página foi construída.
    ///
    /// Quatro estados, e não um booleano. Um booleano obrigava a escolher entre
    /// «pronto» e «não pronto» para quatro situações que não são duas: pronto,
    /// pronto com menos, o Core disse que não, e o Core não disse nada.
    ///
    /// As duas últimas são as que mais custam a distinguir e as que mais
    /// importam — uma sabe-se, a outra não.
    pub core_status: CoreStatus,
    /// Quantas notificações estão por ler.
    pub unread: usize,
    /// Os módulos que o Core diz pertencerem ao espaço de trabalho desta pessoa.
    ///
    /// # Porque isto não é uma permissão
    ///
    /// Porque responde a outra pergunta. Uma permissão diz «pode fazer isto
    /// aqui»; a relevância diz «isto pertence ao trabalho desta pessoa». Quatro
    /// módulos — Conhecimento, Ficheiros, Bibliografia e Dados — governam-se
    /// dentro de um contentor de autoridade, e por isso a navegação **não pode**
    /// responder por eles: perguntá-lo em âmbito institucional dava sempre não,
    /// e era isso que os deixava esbatidos para toda a gente.
    ///
    /// Apresentar não é autorizar. Quem entra sem alcançar nada vê zero
    /// recursos, que é um estado honesto — e cada operação lá dentro volta a
    /// decidir contra o ambiente concreto.
    pub modules: Vec<String>,
    /// Permissões de âmbito institucional, tal como o Core as calculou.
    ///
    /// Vêm de `GET /api/v1/me`. A shell usa-as para não mostrar o que o membro
    /// não pode usar (briefing §65, §67).
    ///
    /// **Não são autorização.** Esconder um item é cortesia; quem escrever o
    /// caminho à mão continua a bater na recusa do Core, que é onde a decisão
    /// vive (`CLAUDE.md` §4).
    pub capabilities: Vec<String>,
    /// As aplicações que o membro fixou na barra lateral, pela sua ordem.
    ///
    /// Os identificadores técnicos do registo, vindos do Core (`/me/apps/pins`).
    /// Quando o membro nunca escolheu, o Workspace resolve isto para o conjunto
    /// por omissão do registo antes de construir a barra — por isso aqui já é a
    /// lista efectiva, e a barra desenha-a filtrada pela visibilidade.
    pub pinned: Vec<String>,
    /// As aplicações que **esta Instância** tem inactivas (ADR-0014), como o
    /// Core as disse. Escondem-se do lançador, da barra, da paleta e do «+
    /// Criar»; a rota diz que a aplicação não está activa. Vazia quando todas
    /// estão activas — ou quando o Core não respondeu, e aí as outras regras já
    /// encolhem a navegação.
    pub inactive_apps: Vec<String>,
    /// O perfil da Instância (`research`, `business`, `personal`, `education`),
    /// como o Core o disse em `/organisation` (ADR-0014). `None` sem resposta: o
    /// distintivo do perfil não afirma o que não se sabe.
    pub perfil: Option<String>,
}

impl Viewer {
    /// Se o membro possui a permissão indicada, à escala institucional.
    #[must_use]
    pub fn can(&self, permission: Permission) -> bool {
        self.capabilities
            .iter()
            .any(|held| held == permission.as_str())
    }
}

/// A permissão que faz um ecrã aparecer na navegação.
///
/// `None` para os ecrãs que qualquer membro autenticado vê — a Home e O Meu
/// Trabalho mostram o que é do próprio, e filtram-se sozinhos.
/// O módulo cuja relevância governa a presença deste ecrã na navegação.
///
/// `None` para os ecrãs cuja presença não é uma questão de relevância — os que
/// são de toda a gente, e os que um direito institucional já resolve.
pub(crate) const fn screen_module(screen: Screen) -> Option<&'static str> {
    match screen {
        // Os quatro que se governam dentro de um contentor. A navegação
        // apresenta-os por relevância; a autorização acontece lá dentro.
        Screen::Knowledge => Some("knowledge"),
        Screen::Files => Some("files"),
        Screen::Bibliography => Some("bibliography"),
        Screen::Datasets => Some("datasets"),
        _ => None,
    }
}

pub(crate) const fn screen_permission(screen: Screen) -> Option<Permission> {
    match screen {
        // Definições são do próprio membro: não exigem permissão
        // institucional nenhuma, e cada pessoa vê apenas a sua conta.
        // Ajuda e Definições são do próprio membro: sem permissão institucional.
        // As notas pessoais são do próprio membro: qualquer pessoa autenticada
        // tem as suas, e o Core resolve o dono pela sessão. Não há direito
        // institucional a exigir para ver a entrada.
        // «Meus Recursos» é do próprio membro: cada pessoa vê a sua quota e os
        // seus entitlements, e o Core resolve o dono pela sessão. Não há direito
        // institucional a exigir para ver a entrada — ver os recursos de outro
        // membro é outra coisa, governada por `resources.view` na Administração.
        Screen::Home
        | Screen::MyWork
        | Screen::Notes
        | Screen::Resources
        | Screen::Settings
        | Screen::Help
        // O Lixo é do próprio membro: o Core lê os ficheiros e as notas que
        // o principal apagou, e nada mais.
        | Screen::Trash
        // O Terminal não precisa de direito próprio: cada comando é autorizado
        // pelo Core como a capability que invoca (ADR-0312).
        | Screen::Terminal
        // O Browser também não: o conteúdo externo não recebe nada do Ocinye, e o
        // que toca no Ocinye são capabilities já governadas (ADR-0623). A
        // Instância pode desactivá-lo (ADR-0014).
        | Screen::Browser => None,
        // O Calendário: a agenda pessoal é do próprio, e `CalendarView` é o que
        // dá acesso aos eventos de unidade, workspace e instituição.
        Screen::Calendar => Some(Permission::CalendarView),
        Screen::Mail => Some(Permission::MailUse),
        Screen::Messaging => Some(Permission::MessagingUse),
        Screen::Units => Some(Permission::UnitsView),
        Screen::Ideas => Some(Permission::IdeasView),
        Screen::Projects => Some(Permission::ProjectsView),
        Screen::Knowledge | Screen::Bibliography => Some(Permission::BibliographyView),
        Screen::Datasets => Some(Permission::DatasetsView),
        // Ficheiros institucionais. `DocumentsView` é o mesmo direito que já
        // governa o acervo documental: um ficheiro não é um substantivo novo
        // com um direito novo, é o objecto sobre o qual o Document se apoia.
        //
        // Ver a entrada na navegação não é ver ficheiro nenhum: quem entra sem
        // alcançar nada vê um ambiente vazio, porque é o Core que decide o que
        // existe para cada pessoa.
        Screen::Files => Some(Permission::DocumentsView),
        Screen::Ai => Some(Permission::AiUse),
        Screen::Agents => Some(Permission::AgentsView),
        Screen::Compute => Some(Permission::ComputeView),
        Screen::Activity => Some(Permission::OrganisationView),
        // `MembersManage`, e não `MembersView`: ver colegas para lhes escrever é
        // um direito de qualquer membro (o directório `/people`), mas a consola
        // de Administração — o roster com o estado de cada conta — é de quem
        // administra pessoas. Um investigador com `MembersView` deixa de a abrir.
        Screen::Admin => Some(Permission::MembersManage),
        Screen::Audit => Some(Permission::AuditView),
        // O Monitor segue a regra do Core para `/system/operations`
        // (administrar a plataforma), e não a da consola de pessoas.
        Screen::Monitor => Some(Permission::PlatformAdminister),
        // Os resultados vivem nos ambientes de projecto: quem não vê
        // projectos não tem onde os ler. Cada resultado é autorizado pelo Core.
        Screen::Results => Some(Permission::ProjectsView),
        // O Prompt não está na navegação lateral; o `AiUse` que o guarda é
        // verificado no ecrã que lá chega.
        Screen::Prompt => Some(Permission::AiUse),
        // A pesquisa está aberta a qualquer membro autenticado: o Core aplica a
        // autorização dentro da consulta, e um membro sem acesso a nada recebe
        // zero resultados — não uma recusa (briefing §28).
        Screen::Search => None,
        // A superfície de comando está aberta a qualquer membro autenticado:
        // pesquisar não exige permissão, e perguntar ou executar declaram-se
        // indisponíveis a quem não tiver `ai.use` — que é informação diferente
        // de não ver a barra (briefing §68).
        Screen::Ask => None,
    }
}
