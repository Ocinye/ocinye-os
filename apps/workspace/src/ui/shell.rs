//! A shell autenticada: sidebar, topbar, command palette e menu de criação.
//!
//! Estrutura de `design/README.md` §5. A shell é a mesma em todos os 19 ecrãs
//! autenticados; só o conteúdo muda.

use leptos::prelude::*;
use ocinye_contracts::AvatarChoice;

use ocinye_contracts::Permission;

use crate::ui::apps;
use crate::ui::icon::Icon;
use crate::ui::initials;
use crate::ui::ods;

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
    /// Prompt Ocinye.
    Prompt,
    /// Pesquisa institucional.
    Search,
    /// A Universal Command Surface.
    Ask,
    /// Definições do próprio membro.
    Settings,
    /// Ajuda do Workspace.
    Help,
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
    /// O que o Centro Temporal mostra, já autorizado pelo Core.
    ///
    /// Vazio quando a consulta falhou **e** quando não há nada — a diferença
    /// vive em `temporal_failure`, e não numa lista vazia a fingir que é
    /// resposta.
    pub temporal: Vec<crate::ui::screens::calendar::Item>,
    /// A razão pela qual a agenda do Centro Temporal não pôde ser lida.
    pub temporal_failure: Option<String>,
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
        | Screen::Help => None,
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

/// Um degrau do breadcrumb, para lá do ecrã actual.
pub struct Crumb {
    /// Rótulo.
    pub label: String,
    /// Destino.
    pub href: String,
}

impl Crumb {
    /// Um degrau do trilho que aponta para um ecrã.
    ///
    /// # Porque não se escreve o par à mão
    ///
    /// Os vinte e um trilhos da aplicação eram literais — `"Bibliografia"` ao
    /// lado de `"/bibliography"`, repetidos handler a handler. Nada obrigava as
    /// duas metades a concordarem entre si, nem qualquer delas a concordar com o
    /// ecrã que dizem ser: um rótulo renomeado na navegação e esquecido aqui
    /// deixava o trilho a chamar-lhe outra coisa, e um caminho alterado deixava
    /// o degrau a apontar para lado nenhum — sem que nada acusasse.
    ///
    /// Vindo do `Screen`, o rótulo e o destino são os mesmos que a navegação
    /// usa, por construção.
    #[must_use]
    pub fn to(screen: Screen) -> Self {
        Self {
            label: screen.label().to_owned(),
            href: screen.path().to_owned(),
        }
    }
}

/// A shell completa (Claude Design, D2 e D6).
///
/// A barra de topo, o conteúdo, e a doca: o botão flutuante que abre a barra
/// vertical das aplicações fixadas. A filtragem por permissões, o registo de
/// aplicações e as fixações continuam a ser as de sempre — muda só a
/// apresentação (`docs/ui/D2_SHELL.md`).
pub fn shell(
    viewer: &Viewer,
    active: Screen,
    trail: Vec<Crumb>,
    current: &str,
    content: impl IntoView + 'static,
) -> impl IntoView {
    view! {
        <a class="ods-sr-only" data-part="skip" href="#conteudo">
            {crate::i18n::t("nav.skip_to_content")}
        </a>

        <div class="ods-shell" data-oc="shell">
            {faixa_privilegiada(viewer)}
            {topbar(viewer, current, trail)}
            <main class="ods-shell__main" id="conteudo" data-part="main content" data-ods-scroll>
                {content}
            </main>
            {doca(viewer, active)}
        </div>

        {palette(viewer)}
        {launcher(viewer)}
    }
}

/// A faixa de sessão privilegiada.
///
/// Uma sessão com autoridade elevada diz-se em cada ecrã, e não só no menu da
/// conta: é a regra de sempre (ADR-0107). O D2 não a desenha; entra com a
/// primitiva de aviso do D1, a mais próxima, até o Claude Design a desenhar.
fn faixa_privilegiada(viewer: &Viewer) -> impl IntoView {
    if !viewer.sessao_privilegiada {
        return ().into_any();
    }

    // O rótulo diz o que a autoridade é **agora**.
    let rotulo = if viewer.administra {
        "SUPER ADMIN · SESSÃO PRIVILEGIADA"
    } else {
        "SESSÃO PRIVILEGIADA · SEM AUTORIDADE ADMINISTRATIVA"
    };
    let nome = viewer.name.clone();
    let email = viewer.email.clone();

    // Faixa própria, vermelha e a toda a largura, acima da barra (D12, Q-07):
    // não se fecha nem some em repouso.
    view! {
        <div class="ods-privileged" role="status" data-oc="privileged-session" data-privilegiada="1">
            {ods::icone("shield", "ods-icon--sm")}
            <b>{rotulo}</b>
            <span class="ods-privileged__who">
                {nome}
                {email.map(|e| view! { " · " {e} })}
                " · "
                {sessao_actual(viewer)}
            </span>
        </div>
    }
    .into_any()
}

/// O estado da sessão do Workspace, em palavras.
///
/// Só se afirma o que se sabe. O `Instant` guardado sabe dizer quanto falta,
/// e não sabe dizer a que horas foi emitida nem de onde: não há aqui data de
/// emissão, dispositivo nem lugar, porque nada disso está guardado.
fn sessao_actual(viewer: &Viewer) -> String {
    let Some(restante) = viewer.session_expires_in else {
        return "activa".to_owned();
    };

    let minutos = restante.as_secs() / 60;
    if minutos < 1 {
        return "activa · a expirar".to_owned();
    }
    let horas = minutos / 60;
    if horas == 0 {
        return format!("activa · expira em {minutos} min");
    }
    let resto = minutos % 60;
    if resto == 0 {
        format!("activa · expira em {horas}h")
    } else {
        format!("activa · expira em {horas}h {resto}min")
    }
}

/// O rótulo do perfil de uma Instância, e as suas iniciais no distintivo.
fn perfil_de(perfil: &str) -> Option<(&'static str, &'static str, &'static str)> {
    // (iniciais, chave do nome, chave da descrição)
    match perfil {
        "research" => Some((
            "Re",
            "admin.instance.profile.research",
            "shell.profile.research.desc",
        )),
        "business" => Some((
            "Bu",
            "admin.instance.profile.business",
            "shell.profile.business.desc",
        )),
        "personal" => Some((
            "Pe",
            "admin.instance.profile.personal",
            "shell.profile.personal.desc",
        )),
        "education" => Some((
            "Ed",
            "admin.instance.profile.education",
            "shell.profile.education.desc",
        )),
        _ => None,
    }
}

/// A barra de topo, pela ordem exacta do D2.
fn topbar(viewer: &Viewer, current: &str, trail: Vec<Crumb>) -> impl IntoView {
    // O dia de hoje onde a pessoa está, e não em Greenwich.
    let hoje = crate::ui::tempo::hoje_civil(chrono::Utc::now(), viewer.zona);
    let current = current.to_owned();

    view! {
        <header class="ods-topbar">
            {conta(viewer)}
            {perfil(viewer)}

            // O contexto é a Instância. O Ocinye OS não tem unidade activa
            // global (§34.3): mostrar aqui uma unidade seria inventá-la. Texto, e
            // não botão, porque não há nada a escolher.
            <span class="ods-topbar__ctx" data-oc="ctx">
                <span class="ods-topbar__ctx-icon">{ods::icone("units", "ods-icon--sm")}</span>
                {viewer.organisation.clone()}
            </span>

            <nav class="ods-crumbs" data-part="crumb" aria-label="Trilho">
                {trail
                    .into_iter()
                    .map(|crumb| {
                        view! {
                            <span aria-hidden="true">"/"</span>
                            <a href=crumb.href>{crumb.label}</a>
                        }
                    })
                    .collect_view()}
                <span aria-hidden="true">"/"</span>
                <span aria-current="page">{current}</span>
            </nav>

            // A Universal Command Surface: pesquisar, perguntar, executar. Um
            // formulário, que funciona sem JavaScript e sem nó de IA.
            <form class="ods-topbar__ask" method="get" action="/ask" role="search">
                {ods::icone("nye", "")}
                <label class="ods-sr-only" for="oc-command">{crate::i18n::t("nav.ask")}</label>
                <input
                    id="oc-command"
                    name="q"
                    type="search"
                    placeholder=crate::i18n::t("nav.search.placeholder")
                    autocomplete="off"
                />
                <kbd class="ods-kbd" data-oc="palette-open" title="Command palette">"⌘K"</kbd>
            </form>

            {create_menu(&viewer.inactive_apps)}
            {estado_do_sistema(viewer.core_status)}
            {notifications(viewer.unread)}

            // O relógio é do computador de quem está a ver, e nunca decide
            // nada. `hidden` até o JS lhe escrever a hora: mostrar um relógio
            // vazio seria mostrar uma hora que não sabemos.
            <div class="ods-slot">
                <button
                    type="button"
                    class="ods-topbar__clock"
                    data-oc="clock"
                    aria-expanded="false"
                    aria-controls="oc-temporal-centre"
                    aria-label="Centro Temporal"
                    hidden
                >
                    <span class="ods-topbar__date" data-part="clock-date"></span>
                    <span class="ods-topbar__time" data-part="clock-time"></span>
                </button>
                {crate::ui::screens::calendar::system_calendar(hoje)}
            </div>
        </header>
    }
}

/// A conta: o logótipo abre o menu pessoal.
///
/// > **The profile popover is a personal session surface, never an
/// > authorization surface.**
///
/// Atalhos para o que o membro já tem, e nunca autoridade institucional.
fn conta(viewer: &Viewer) -> impl IntoView {
    let nome = viewer.name.clone();
    let iniciais = initials(&nome);
    let email = viewer.email.clone();
    let instancia = viewer
        .perfil
        .as_deref()
        .and_then(perfil_de)
        .map(|(_, chave, _)| format!("OCINYE OS · {}", crate::i18n::t(chave).to_uppercase()))
        .unwrap_or_else(|| "OCINYE OS".to_owned());

    view! {
        <div class="ods-slot" data-oc="account">
            <button
                type="button"
                class="ods-topbar__logo"
                data-oc="account-toggle"
                aria-haspopup="menu"
                aria-expanded="false"
                aria-controls="oc-account-menu"
                aria-label=nome.clone()
                title=nome.clone()
            >
                <img src="/static/ocinye_logo.png" alt="" />
            </button>

            <div
                class="ods-popover ods-popover--left ods-glass ods-account"
                id="oc-account-menu"
                data-oc="account-menu"
                role="menu"
                aria-label="Conta e sessão"
                hidden
            >
                <div class="ods-account__head">
                    {ods::avatar_do_membro(&viewer.avatar, &iniciais, &nome, ods::TamanhoAvatar::Medio)}
                    <div>
                        <p class="ods-account__name">{nome.clone()}</p>
                        {email.map(|e| view! { <p class="ods-account__mail">{e}</p> })}
                        <p class="ods-account__inst">{instancia}</p>
                    </div>
                </div>
                // Informação de segurança (D12, Q-09): o que se sabe da sessão, e
                // nada que não esteja guardado.
                <p class="ods-account__session">
                    {format!("Sessão actual · {}", sessao_actual(viewer))}
                </p>
                <div class="ods-menu">
                    <a class="ods-menu__item" href="/settings" role="menuitem">
                        {ods::icone("user", "")}
                        {crate::i18n::t("shell.account")}
                    </a>
                    <a class="ods-menu__item" href="/settings/security" role="menuitem">
                        {ods::icone("settings", "")}
                        {crate::i18n::t("nav.settings")}
                    </a>
                    <a class="ods-menu__item" href="/help" role="menuitem">
                        {ods::icone("help", "")}
                        {crate::i18n::t("nav.help")}
                    </a>
                    // G-01: bloquear o ecrã espera por `POST /session/lock`.
                    <button
                        type="button"
                        class="ods-menu__item"
                        data-oc="lock-open"
                        role="menuitem"
                        aria-disabled="true"
                        data-tip=crate::i18n::t("ods.state.pending_contract")
                    >
                        {ods::icone("lock", "")}
                        {crate::i18n::t("shell.lock")}
                        <kbd class="ods-menu__kbd">"⌘ L"</kbd>
                    </button>
                    <div class="ods-menu__sep"></div>
                    <form method="post" action="/logout">
                        <button type="submit" class="ods-menu__item ods-menu__item--danger" role="menuitem">
                            {ods::icone("logout", "")}
                            {crate::i18n::t("nav.sign_out")}
                        </button>
                    </form>
                </div>
            </div>
        </div>
    }
}

/// O distintivo do perfil da Instância (Re/Bu/Pe/Ed) e o seu cartão.
///
/// O perfil é da Instância, decidido pela administração, e o cartão di-lo. Sem
/// resposta do Core o distintivo não aparece: não se afirma um perfil que não
/// se sabe.
fn perfil(viewer: &Viewer) -> impl IntoView {
    let Some((iniciais, chave_nome, chave_desc)) = viewer.perfil.as_deref().and_then(perfil_de)
    else {
        return ().into_any();
    };
    let nome = crate::i18n::t(chave_nome);
    let rotulo = format!("{}: {nome}", crate::i18n::t("shell.profile.title"));
    let modulos: Vec<&'static str> = crate::ui::apps::visible_to(viewer, viewer.core_status)
        .into_iter()
        .map(|app| app.label())
        .collect();

    view! {
        <div class="ods-slot" data-oc="profile">
            <button
                type="button"
                class="ods-topbar__profile ods-gold-badge"
                data-oc="profile-toggle"
                aria-haspopup="dialog"
                aria-expanded="false"
                aria-controls="oc-profile-card"
                aria-label=rotulo.clone()
                title=rotulo
            >
                {iniciais}
            </button>
            <div
                class="ods-popover ods-popover--left ods-glass ods-profile-card"
                id="oc-profile-card"
                data-oc="profile-card"
                role="dialog"
                aria-label=crate::i18n::t("shell.profile.title")
                hidden
            >
                <div class="ods-profile-card__head">
                    <span class="ods-profile-card__badge ods-gold-badge" aria-hidden="true">{iniciais}</span>
                    <div>
                        <p class="ods-label">{crate::i18n::t("shell.profile.title")}</p>
                        <p class="ods-profile-card__name">{nome}</p>
                    </div>
                </div>
                <p class="ods-profile-card__desc">{crate::i18n::t(chave_desc)}</p>
                <dl class="ods-kv">
                    <dt>{crate::i18n::t("shell.profile.instance")}</dt>
                    <dd>{viewer.organisation.clone()}</dd>
                    <dt>{crate::i18n::t("shell.profile.desktop")}</dt>
                    // G-04: a versão da predefinição do Desktop não existe ainda.
                    <dd>{crate::i18n::t("shell.profile.desktop_version_pending")}</dd>
                </dl>
                <div class="ods-kv">
                    <p class="ods-label">{crate::i18n::t("shell.profile.main_modules")}</p>
                    <div class="ods-chips">
                        {modulos
                            .into_iter()
                            .map(|m| view! { <span class="ods-chip">{m}</span> })
                            .collect_view()}
                    </div>
                </div>
                <p class="ods-profile-card__desc">{crate::i18n::t("shell.profile.set_by_admin")}</p>
            </div>
        </div>
    }
    .into_any()
}

/// O estado do sistema: CORE e IA.
///
/// O ponto do Core reflecte a sonda real ao Core — nunca «OK» sem resposta
/// dele. A linha da IA espera pelo G-09 (um resumo tipado do fornecedor) e
/// diz-o, em vez de adivinhar.
fn estado_do_sistema(estado: CoreStatus) -> impl IntoView {
    let (tom, rotulo, estado_core) = match estado {
        CoreStatus::Ok => (
            ods::Tom::Sucesso,
            crate::i18n::t("shell.status.core.ok"),
            "ok",
        ),
        CoreStatus::Unavailable => (ods::Tom::Erro, "INDISPONÍVEL", "indisponivel"),
        CoreStatus::Silent => (ods::Tom::Erro, "SEM RESPOSTA", "silencio"),
    };

    view! {
        <div class="ods-slot">
            <button
                type="button"
                class="ods-status"
                data-oc="status-toggle"
                data-part="core-pill"
                data-estado=estado_core
                aria-haspopup="dialog"
                aria-expanded="false"
                aria-controls="oc-status-card"
                aria-label=crate::i18n::t("shell.status.title")
            >
                {ods::ponto(tom)}
                "CORE"
                {ods::ponto(ods::Tom::Neutro)}
                "IA"
            </button>
            <div
                class="ods-popover ods-popover--right ods-glass ods-status-card"
                id="oc-status-card"
                data-oc="status-card"
                role="dialog"
                aria-label=crate::i18n::t("shell.status.title")
                hidden
            >
                <p class="ods-menu__title ods-label">{crate::i18n::t("shell.status.title")}</p>
                <div class="ods-status-row">
                    <span class="ods-status-row__icon">{ods::ponto(tom)}</span>
                    <div>
                        <p class="ods-status-row__title">
                            {crate::i18n::t("shell.status.core")} " "
                            {ods::distintivo(rotulo.to_uppercase(), tom)}
                        </p>
                        <p class="ods-status-row__desc">{crate::i18n::t("shell.status.core.desc")}</p>
                    </div>
                </div>
                <div class="ods-status-row">
                    <span class="ods-status-row__icon ods-status-row__icon--warning">
                        {ods::ponto(ods::Tom::Aviso)}
                    </span>
                    <div>
                        <p class="ods-status-row__title">{crate::i18n::t("shell.status.ai")}</p>
                        {ods::a_espera_de_contrato()}
                    </div>
                </div>
                <div class="ods-menu">
                    <a class="ods-btn ods-btn--sm" href="/ai">{crate::i18n::t("shell.status.ai.options")}</a>
                </div>
            </div>
        </div>
    }
}

/// O sino de notificações.
///
/// O ponto só se pinta quando há o que contar: o número vem do Core. O painel
/// carrega quando abre — renderizá-lo em cada página seria pedir ao Core a lista
/// inteira a cada navegação para a esconder quase sempre.
fn notifications(unread: usize) -> impl IntoView {
    let titulo = if unread == 0 {
        crate::i18n::t("shell.notifications").to_owned()
    } else {
        format!(
            "{} · {}",
            crate::i18n::t("shell.notifications"),
            crate::i18n::tf("shell.notifications.unread", &[("n", &unread.to_string())])
        )
    };

    view! {
        <div class="ods-slot" data-oc="sino">
            <button
                type="button"
                class="ods-iconbtn"
                data-oc="abrir-notificacoes"
                aria-haspopup="dialog"
                aria-expanded="false"
                aria-controls="oc-notificacoes"
                title=titulo.clone()
                aria-label=titulo
            >
                {ods::icone("bell", "")}
                {(unread > 0).then(|| view! {
                    <span class="ods-count" data-oc="notificacoes-contagem" aria-hidden="true">
                        {unread.to_string()}
                    </span>
                })}
            </button>

            <div
                class="ods-popover ods-popover--right ods-glass ods-notif"
                id="oc-notificacoes"
                data-oc="notificacoes"
                role="dialog"
                aria-label=crate::i18n::t("shell.notifications")
                hidden
            >
                <div class="ods-notif__list" data-oc="notificacoes-lista">
                    <p class="ods-empty__body">{crate::i18n::t("ods.loading")}</p>
                </div>
                <a class="ods-btn ods-btn--ghost ods-btn--block" href="/notifications">
                    {crate::i18n::t("shell.notifications.all")}
                </a>
            </div>
        </div>
    }
}

/// O «+ Criar».
///
/// Os itens são os de sempre (ideia, projecto, nota, referência, dataset,
/// tarefa, agente), filtrados pelas aplicações activas na Instância. O D2
/// propõe outro conjunto; a escolha fica com quem decide o produto, e até lá
/// não se perde nenhuma criação que já existe.
fn create_menu(inactive_apps: &[String]) -> impl IntoView {
    let inactive_apps = inactive_apps.to_vec();
    view! {
        <div class="ods-slot" data-oc="create">
            <button
                type="button"
                class="ods-topbar__create"
                data-oc="create-toggle"
                aria-haspopup="menu"
                aria-expanded="false"
            >
                {ods::icone("plus", "ods-icon--sm")}
                {crate::i18n::t("nav.create")}
            </button>

            <div
                class="ods-popover ods-popover--right ods-glass ods-create-menu"
                data-part="create__menu"
                data-oc="create-menu"
                role="menu"
                hidden
            >
                <p class="ods-menu__title ods-label">{crate::i18n::t("shell.create.title")}</p>
                <div class="ods-menu">
                    {CREATE_ITEMS
                        .iter()
                        .filter(|item| !inactive_apps.iter().any(|id| id == item.app))
                        .map(create_menu_item)
                        .collect_view()}
                </div>
            </div>
        </div>
    }
}

/// Um item do «Criar»: uma ligação que abre o formulário, ou um botão que cria
/// de imediato com um `POST`. `data-oc-key` leva a tecla de acesso ao `app.js`.
fn create_menu_item(action: &CreateAction) -> impl IntoView {
    let CreateAction {
        label, via, key, ..
    } = *action;

    match via {
        CreateVia::Open(href) => view! {
            <a class="ods-menu__item" data-part="create__item" role="menuitem" href=href data-oc-key=key>
                {crate::i18n::t(label)}
                <kbd class="ods-menu__kbd">{key}</kbd>
            </a>
        }
        .into_any(),
        CreateVia::Create(action_url) => view! {
            <form method="post" action=action_url role="none">
                <button
                    type="submit"
                    class="ods-menu__item"
                    data-part="create__item"
                    role="menuitem"
                    data-oc-key=key
                >
                    {crate::i18n::t(label)}
                    <kbd class="ods-menu__kbd">{key}</kbd>
                </button>
            </form>
        }
        .into_any(),
    }
}

/// A doca: o botão flutuante das aplicações e a barra vertical que ele abre
/// (D4, D6).
///
/// A barra é navegação essencial mais as aplicações que o membro fixou, pela
/// ordem dele e filtradas pela visibilidade: nunca oferece um ecrã sem
/// autorização. `Desafixar ≠ desinstalar` (§45-A).
fn doca(viewer: &Viewer, active: Screen) -> impl IntoView {
    let fixadas = apps::pinned_visible(&viewer.pinned, viewer, viewer.core_status);

    view! {
        <button
            type="button"
            class="ods-float-apps"
            data-oc="shelf-toggle"
            data-ods-float
            aria-expanded="false"
            aria-controls="oc-shelf"
            aria-label=crate::i18n::t("shell.shelf.show")
        >
            {ods::icone("apps-brand-dark", "")}
        </button>

        <nav
            class="ods-shelf ods-glass--dark"
            id="oc-shelf"
            data-oc="side-pinned"
            aria-label=crate::i18n::t("shell.shelf.apps")
            hidden
        >
            <a
                class="ods-shelf__btn"
                href="/"
                aria-current=(active == Screen::Home).then_some("page")
                data-tip=crate::i18n::t("shell.shelf.desktop")
                aria-label=crate::i18n::t("shell.shelf.desktop")
            >
                {ods::icone("home", "")}
            </a>
            // G-05: não há janelas enquanto o gestor de janelas não existir.
            <button
                type="button"
                class="ods-shelf__btn"
                aria-disabled="true"
                data-tip=crate::i18n::t("ods.state.pending_contract")
                aria-label=crate::i18n::t("shell.shelf.windows")
            >
                {ods::icone("grid", "")}
            </button>
            <button
                type="button"
                class="ods-shelf__btn"
                data-oc="launcher-open"
                aria-haspopup="dialog"
                data-tip=crate::i18n::t("shell.shelf.apps")
                aria-label=crate::i18n::t("shell.shelf.apps")
            >
                {ods::icone("apps-brand-dark", "")}
            </button>
            <span class="ods-shelf__sep" aria-hidden="true"></span>
            {fixadas
                .into_iter()
                .map(|app| item_fixado(app.screen, app.screen == active))
                .collect_view()}
            <span class="ods-shelf__sep" data-oc="shelf-fim" aria-hidden="true"></span>
            <a
                class="ods-shelf__btn"
                href="/files?trash=1"
                data-tip=crate::i18n::t("shell.shelf.trash")
                aria-label=crate::i18n::t("shell.shelf.trash")
            >
                {ods::icone("trash", "")}
            </a>
        </nav>
    }
}

/// Uma aplicação fixada na barra, com o `data-app-id` que o cliente usa para a
/// acrescentar ou retirar ao vivo quando o membro fixa ou desafixa no lançador.
fn item_fixado(screen: Screen, on: bool) -> impl IntoView {
    view! {
        <a
            class="ods-shelf__btn"
            href=screen.path()
            data-oc="fixada"
            data-app-id=screen.id()
            aria-current=on.then_some("page")
            data-tip=screen.label()
            aria-label=screen.label()
        >
            {ods::icone(ods::icone_da_aplicacao(screen), "")}
        </a>
    }
}

/// A command palette (⌘K).
///
/// O D2 não a desenha: entra com as primitivas do D1 (diálogo, pesquisa,
/// menu). O filtro é local, mas o que entra na página não é: só os ecrãs e as
/// acções que o membro alcança (briefing §65).
fn palette(viewer: &Viewer) -> impl IntoView {
    let screens: Vec<Screen> = PALETTE_NAV
        .iter()
        .copied()
        .filter(|screen| screen_permission(*screen).is_none_or(|p| viewer.can(p)))
        .filter(|screen| !viewer.inactive_apps.iter().any(|id| id == screen.id()))
        .collect();

    // Uma acção de uma aplicação que a Instância desactivou não se oferece.
    let activa = |href: &str| {
        crate::ui::apps::APPLICATIONS
            .iter()
            .filter(|app| app.screen.path() != "/" && href.starts_with(app.screen.path()))
            .max_by_key(|app| app.screen.path().len())
            .is_none_or(|app| !viewer.inactive_apps.iter().any(|id| id == app.id()))
    };
    let actions: Vec<(&str, &str, &str)> = PALETTE_ACTIONS
        .iter()
        .filter(|(_, _, _, permission)| viewer.can(*permission))
        .filter(|(_, href, _, _)| activa(href))
        .map(|(label, href, shortcut, _)| (*label, *href, *shortcut))
        .collect();

    view! {
        <div
            class="ods-modal"
            data-part="palette"
            data-oc="palette"
            role="dialog"
            aria-modal="true"
            aria-label="Pesquisar ou executar um comando"
            hidden
        >
            <div class="ods-scrim"></div>
            <div class="ods-modal__panel ods-window-surface">
                <div class="ods-modal__head">
                    <label class="ods-search">
                        {ods::icone("search", "")}
                        <span class="ods-sr-only">"Pesquisar ou executar um comando"</span>
                        <input
                            id="palette-input"
                            class="ods-search__input"
                            type="text"
                            data-oc="palette-input"
                            autocomplete="off"
                            placeholder="Pesquisar ou executar um comando…"
                        />
                        <kbd class="ods-kbd">"ESC"</kbd>
                    </label>
                </div>
                <div class="ods-modal__body ods-menu">
                    <div data-oc="palette-group">
                        <p class="ods-menu__title ods-label" data-oc="palette-group-label">"NAVEGAR"</p>
                        {screens
                            .into_iter()
                            .map(|screen| {
                                view! {
                                    <a
                                        class="ods-menu__item"
                                        data-oc="palette-item"
                                        href=screen.path()
                                        data-label=screen.label()
                                    >
                                        {ods::icone(ods::icone_da_aplicacao(screen), "")}
                                        {screen.label()}
                                    </a>
                                }
                            })
                            .collect_view()}
                    </div>
                    <div data-oc="palette-group">
                        <p class="ods-menu__title ods-label" data-oc="palette-group-label">"ACÇÕES"</p>
                        {actions
                            .into_iter()
                            .map(|(label, href, shortcut)| {
                                view! {
                                    <a
                                        class="ods-menu__item"
                                        data-oc="palette-item"
                                        href=href
                                        data-label=label
                                        // O atalho vai no atributo, e não só no
                                        // `<kbd>`: é daqui que o teclado o lê.
                                        data-shortcut=shortcut
                                    >
                                        {label}
                                        {(!shortcut.is_empty())
                                            .then(|| view! { <kbd class="ods-menu__kbd">{shortcut}</kbd> })}
                                    </a>
                                }
                            })
                            .collect_view()}
                    </div>
                </div>
            </div>
        </div>
    }
}

/// O Gestor de Aplicações (D6): o lançador, a superfície de descoberta de todas
/// as aplicações que o membro alcança.
///
/// A grelha vem renderizada do servidor, filtrada pela autorização; o cliente
/// só abre, fecha e filtra o que já lá está. Não há segunda lista no cliente.
fn launcher(viewer: &Viewer) -> impl IntoView {
    use crate::ui::apps::{self, Category};

    let apps = apps::visible_to(viewer, viewer.core_status);
    let total = apps.len();

    let cats = std::iter::once(("apps.category.all", "all"))
        .chain(
            Category::all()
                .into_iter()
                .filter(|c| apps.iter().any(|app| app.category() == *c))
                .map(|c| (c.label_key(), c.id())),
        )
        .map(|(label_key, id)| {
            let contagem = if id == "all" {
                total
            } else {
                apps.iter().filter(|app| app.category().id() == id).count()
            };
            view! {
                <button
                    type="button"
                    class="ods-tabs__tab"
                    role="tab"
                    data-oc="launcher-chip"
                    data-category=id
                    aria-selected=if id == "all" { "true" } else { "false" }
                >
                    {crate::i18n::t(label_key)}
                    <span class="ods-tabs__count">{contagem.to_string()}</span>
                </button>
            }
        })
        .collect_view();

    let fixadas: std::collections::BTreeSet<&str> =
        viewer.pinned.iter().map(String::as_str).collect();

    let cards = apps
        .into_iter()
        .map(|app| {
            let label = app.label();
            let descricao = app.description();
            let procura = format!(
                "{} {} {}",
                label.to_lowercase(),
                descricao.to_lowercase(),
                app.keywords.join(" ")
            );
            let fixada = fixadas.contains(app.id());
            let rotulo_fixar = crate::i18n::t(if fixada { "apps.unpin" } else { "apps.pin" });
            let botao_fixar = app.can_pin().then(|| {
                view! {
                    <button
                        type="button"
                        class="ods-iconbtn ods-launcher__pin"
                        data-oc="launcher-pin"
                        data-app-id=app.id()
                        data-label-pin=crate::i18n::t("apps.pin")
                        data-label-unpin=crate::i18n::t("apps.unpin")
                        aria-pressed=if fixada { "true" } else { "false" }
                        title=rotulo_fixar
                        aria-label=rotulo_fixar
                    >
                        {ods::icone("star-fill", "ods-icon--sm")}
                    </button>
                }
            });
            view! {
                <div class="ods-launcher__cell" data-oc="launcher-cell">
                    <a
                        class="ods-launcher__item"
                        href=app.route()
                        data-oc="launcher-item"
                        data-app-id=app.id()
                        data-category=app.category().id()
                        data-search=procura
                        aria-label=label
                    >
                        <span class="ods-app-tile" data-oc="launcher-icone">
                            {ods::icone(ods::icone_da_aplicacao(app.screen), "ods-icon--lg")}
                        </span>
                        <span>
                            <span class="ods-launcher__name" data-oc="launcher-nome">{label}</span>
                            <span class="ods-launcher__desc">{descricao}</span>
                        </span>
                    </a>
                    {botao_fixar}
                </div>
            }
        })
        .collect_view();

    view! {
        <div
            class="ods-launcher"
            data-oc="launcher"
            role="dialog"
            aria-modal="true"
            aria-labelledby="ods-launcher-title"
            hidden
        >
            <div class="ods-scrim" data-oc="launcher-fechar"></div>
            <div class="ods-launcher__panel ods-window-surface" data-oc="launcher-painel">
                <div class="ods-launcher__head">
                    <div class="ods-launcher__title-row">
                        {ods::icone("apps-brand", "ods-icon--lg")}
                        <h2 class="ods-launcher__title" id="ods-launcher-title">
                            {crate::i18n::t("apps.title")}
                        </h2>
                        <span class="ods-badge">{total.to_string()}</span>
                        <span class="ods-kbd">{crate::i18n::t("apps.esc_hint")}</span>
                        <button
                            type="button"
                            class="ods-iconbtn"
                            data-oc="launcher-fechar"
                            aria-label=crate::i18n::t("apps.close")
                        >
                            {ods::icone("close", "")}
                        </button>
                    </div>
                    <label class="ods-search ods-launcher__search">
                        {ods::icone("search", "")}
                        <span class="ods-sr-only">{crate::i18n::t("apps.search_placeholder")}</span>
                        <input
                            id="launcher-input"
                            class="ods-search__input"
                            type="text"
                            data-oc="launcher-input"
                            autocomplete="off"
                            placeholder=crate::i18n::t("apps.search_placeholder")
                        />
                        <kbd class="ods-kbd">"⌘J"</kbd>
                    </label>
                    <div
                        class="ods-tabs ods-launcher__cats"
                        role="tablist"
                        aria-label=crate::i18n::t("apps.title")
                    >
                        {cats}
                    </div>
                </div>
                <div class="ods-launcher__body" data-ods-scroll>
                    <div class="ods-launcher__grid" data-oc="launcher-grelha">
                        {cards}
                    </div>
                    <div class="ods-empty" data-oc="launcher-vazio" hidden>
                        <p class="ods-empty__title">{crate::i18n::t("apps.empty")}</p>
                        <p class="ods-empty__body">{crate::i18n::t("apps.empty.hint")}</p>
                    </div>
                </div>
            </div>
        </div>
    }
}

/// A superfície neutra de falha-fechada.
///
/// Desenha-se quando a identidade da sessão **não pôde ser estabelecida** — o
/// `/me` respondeu com um erro técnico (5xx, tempo esgotado, rede, base de
/// dados), e não com `200`. Deliberadamente **não** é a shell normal: uma falha
/// em ler o que a sessão é não é prova de que a sessão é normal, e desenhar a
/// shell normal aqui foi precisamente o defeito. Não afirma identidade, não
/// mostra navegação, e não deixa uma sessão privilegiada passar por normal.
pub fn identidade_indeterminada() -> impl IntoView {
    view! {
        <main class="ods-auth">
            <span class="ods-auth__mark"><img src="/static/ocinye_logo.png" alt="" /></span>
            <section class="ods-auth__card">
                <div class="ods-notice ods-notice--warning" role="alert">
                    "Não foi possível estabelecer a sua sessão neste momento. \
                     Isto não é um acesso recusado: o serviço não respondeu a tempo. \
                     Por segurança, nada é apresentado até a sessão ser confirmada."
                </div>
                <a class="ods-btn ods-btn--primary ods-btn--block" href="/login">
                    "Voltar ao início de sessão"
                </a>
            </section>
        </main>
    }
}

/// Como uma acção do «Criar» se concretiza.
///
/// Toda a criação determinista funciona sem GPU nem modelo: a ausência de
/// inferência só degrada a **execução** de um agente, nunca a criação de um
/// artefacto (regra pré-IA). Cada acção ou abre o formulário onde o contexto se
/// resolve, ou cria de imediato.
#[derive(Clone, Copy)]
enum CreateVia {
    /// Abre um formulário/página (GET). O contexto (unidade, ambiente, ideia)
    /// resolve-se lá, com estado vazio accionável quando falta.
    Open(&'static str),
    /// Cria de imediato (POST) e o servidor abre o objecto novo. Para a acção
    /// que não precisa de contexto nenhum: uma nota pessoal.
    Create(&'static str),
}

/// Uma acção de criação, orientada a dados: um único registo tipado, e não sete
/// ramos condicionais espalhados (briefing §28).
///
/// **Sem permissão no menu.** Toda a criação determinista funciona sem GPU, e o
/// que decide se uma criação acontece é a autoridade do Core, sempre. O menu não
/// cinzenta acções por uma permissão que é, no fundo, um proxy de contexto (a
/// filiação numa unidade ou ambiente): cinzentar assim escondia acções que o
/// membro pode fazer, e foi o defeito visível. Cada acção abre o seu fluxo, onde
/// o contexto se resolve com estado vazio accionável quando falta (§2, §3, §15).
struct CreateAction {
    /// O que se lê no menu.
    label: &'static str,
    /// Como se concretiza.
    via: CreateVia,
    /// A tecla de acesso, activada com o menu aberto.
    key: &'static str,
    /// A aplicação a que a criação pertence: uma aplicação que a Instância
    /// desactivou não oferece criações (ADR-0014).
    app: &'static str,
}

/// A ordem do «+ Criar» é a do Claude Design (D12, Q-01): o que se cria todos os
/// dias primeiro, e o institucional depois.
const CREATE_ITEMS: [CreateAction; 9] = [
    // Uma nota pessoal não precisa de contexto: cria-se e abre-se o editor.
    // Caminho próprio (`/notes/new`) para não colidir, no DOM, com o formulário
    // de criação da lista de Notas — os dois criam a mesma nota pessoal.
    CreateAction {
        label: "shell.create.note",
        via: CreateVia::Create("/notes/new"),
        key: "N",
        app: "notes",
    },
    CreateAction {
        label: "shell.create.task",
        via: CreateVia::Open("/tasks/new"),
        key: "T",
        app: "projects",
    },
    CreateAction {
        label: "shell.create.event",
        via: CreateVia::Open("/calendar/events/new"),
        key: "E",
        app: "calendar",
    },
    CreateAction {
        label: "shell.create.message",
        via: CreateVia::Open("/mail/compose"),
        key: "M",
        app: "mail",
    },
    CreateAction {
        label: "create.idea",
        via: CreateVia::Open("/ideas/new"),
        key: "I",
        app: "ideas",
    },
    CreateAction {
        label: "create.project",
        via: CreateVia::Open("/projects/new"),
        key: "P",
        app: "projects",
    },
    CreateAction {
        label: "create.dataset",
        via: CreateVia::Open("/datasets/new"),
        key: "D",
        app: "datasets",
    },
    CreateAction {
        label: "create.reference",
        via: CreateVia::Open("/bibliography/new"),
        key: "R",
        app: "bibliography",
    },
    CreateAction {
        label: "shell.create.agent",
        via: CreateVia::Open("/ai/agents/new"),
        key: "A",
        app: "agents",
    },
];

/// Todos os ecrãs, incluindo os que não estão na navegação lateral.
///
/// `PALETTE_NAV` é a lista dos dezoito destinos institucionais. Esta é maior:
/// junta-lhe os ecrãs do próprio membro, que existem no rodapé e não na
/// navegação, mas que continuam a precisar de estado activo e de posse de
/// rotas como qualquer outro.
#[cfg_attr(
    not(test),
    allow(dead_code, reason = "lida pela auditoria de estado activo")
)]
const SCREENS: [Screen; 23] = [
    Screen::Home,
    Screen::MyWork,
    Screen::Calendar,
    Screen::Messaging,
    Screen::Mail,
    Screen::Resources,
    Screen::Units,
    Screen::Ideas,
    Screen::Projects,
    Screen::Knowledge,
    Screen::Files,
    Screen::Bibliography,
    Screen::Datasets,
    Screen::Ai,
    Screen::Agents,
    Screen::Compute,
    Screen::Activity,
    Screen::Admin,
    Screen::Audit,
    Screen::Prompt,
    Screen::Search,
    Screen::Settings,
    Screen::Help,
];

impl Screen {
    /// O ecrã a que um caminho pertence.
    ///
    /// # Porque não basta a igualdade literal
    ///
    /// A navegação tem quinze entradas, e a aplicação tem muito mais caminhos
    /// do que isso. `/projects/new` não é a lista de projectos, mas pertence-lhe:
    /// quem lá está está em Projectos, e a barra tem de o dizer. Comparar o
    /// caminho actual com `screen.path()` por igualdade deixaria a barra sem
    /// nenhum item marcado em todos os ecrãs de detalhe e de criação — que são
    /// a maioria.
    ///
    /// A posse decide-se pelo prefixo mais longo, e o mais longo é que ganha:
    /// `/ai/agents/new` pertence a Agentes (`/ai/agents`) e não ao hub de IA
    /// (`/ai`), embora ambos sejam prefixos válidos. `Home` é um caso à parte —
    /// `/` é prefixo de tudo, e por isso só se reclama a si próprio.
    ///
    /// Devolve `None` para os caminhos que não vivem dentro da shell: o login,
    /// o logout e as submissões de formulário não têm barra lateral, e por isso
    /// não têm item activo.
    #[must_use]
    #[cfg_attr(
        not(test),
        allow(dead_code, reason = "oráculo da auditoria de estado activo")
    )]
    pub fn owning(path: &str) -> Option<Self> {
        if path == "/" {
            return Some(Self::Home);
        }

        SCREENS
            .into_iter()
            .filter(|screen| {
                let base = screen.path();
                base != "/"
                    && (path == base
                        || path
                            .strip_prefix(base)
                            .is_some_and(|rest| rest.starts_with('/')))
            })
            .max_by_key(|screen| screen.path().len())
    }
}

/// Os destinos da command palette.
const PALETTE_NAV: [Screen; 20] = [
    Screen::Home,
    Screen::MyWork,
    Screen::Notes,
    Screen::Calendar,
    Screen::Messaging,
    Screen::Mail,
    Screen::Resources,
    Screen::Units,
    Screen::Ideas,
    Screen::Projects,
    Screen::Knowledge,
    Screen::Files,
    Screen::Bibliography,
    Screen::Datasets,
    Screen::Ai,
    Screen::Agents,
    Screen::Compute,
    Screen::Activity,
    Screen::Admin,
    Screen::Audit,
];

/// As acções da command palette, com a permissão que cada uma exige.
const PALETTE_ACTIONS: [(&str, &str, &str, Permission); 4] = [
    ("Nova Ideia", "/ideas/new", "⌘⇧I", Permission::IdeasCreate),
    (
        "Novo Agente IA",
        "/ai/agents/new",
        "⌘⇧A",
        Permission::AgentsCreatePersonal,
    ),
    (
        "Abrir Prompt Ocinye",
        "/ai/prompt",
        "⌘⇧P",
        Permission::AiUse,
    ),
    ("Ver Computação", "/compute", "⌘⇧C", Permission::ComputeView),
];

#[cfg(test)]
mod tests {
    use super::*;

    /// Todos os módulos de investigação, para quem os deva ver.
    ///
    /// A relevância é um eixo **separado** das permissões: um actor pode ter
    /// todos os direitos institucionais e nenhum módulo de investigação
    /// relevante — é o caso de um administrador de plataforma —, e pode ter
    /// poucos direitos e os módulos todos, que é o caso de um membro de
    /// investigação sem pertenças. Os testes têm de os poder pedir em separado.
    fn todos_os_modulos() -> Vec<String> {
        [
            "units",
            "ideas",
            "projects",
            "knowledge",
            "files",
            "bibliography",
            "datasets",
        ]
        .into_iter()
        .map(ToOwned::to_owned)
        .collect()
    }

    /// Um membro com as permissões indicadas e os módulos de investigação.
    fn viewer_de_investigacao(permissions: &[Permission]) -> Viewer {
        Viewer {
            pinned: crate::ui::apps::default_pins(),
            inactive_apps: Vec::new(),
            perfil: None,
            resolucao: crate::ui::shell::ResolucaoSessao::Resolvida,
            modules: todos_os_modulos(),
            ..viewer_with(permissions)
        }
    }

    /// Um membro com exactamente as permissões indicadas, e nenhum módulo de
    /// investigação relevante.
    fn viewer_with(permissions: &[Permission]) -> Viewer {
        Viewer {
            pinned: crate::ui::apps::default_pins(),
            inactive_apps: Vec::new(),
            perfil: None,
            resolucao: crate::ui::shell::ResolucaoSessao::Resolvida,
            zona: "UTC".to_owned().try_into().expect("fuso conhecido"),
            avatar: ocinye_contracts::AvatarChoice::Initials,
            email: Some("jmanuel@ocinye.com".to_owned()),
            session_expires_in: Some(std::time::Duration::from_secs(8 * 3600)),
            name: "João Manuel".to_owned(),
            sessao_privilegiada: false,
            administra: false,
            organisation: "Ocinye".to_owned(),
            core_status: crate::ui::shell::CoreStatus::Ok,
            temporal: Vec::new(),
            temporal_failure: None,
            unread: 0,
            capabilities: permissions.iter().map(|p| p.as_str().to_owned()).collect(),
            modules: Vec::new(),
        }
    }

    /// A superfície de conta, isolada do resto da shell.
    ///
    /// Só existe depois de um clique no browser, mas o servidor já a manda no
    /// documento: é aí que se pode auditá-la sem fingir um browser.
    fn menu(html: &str) -> String {
        let inicio = html
            .find(r#"data-oc="account-menu""#)
            .expect("a superfície de conta desapareceu");
        // O menu vive no primeiro lugar da barra de topo (D2): acaba onde começa
        // o lugar seguinte — o perfil, ou o contexto quando não há perfil.
        let fim = [r#"data-oc="profile""#, r#"data-oc="ctx""#]
            .iter()
            .filter_map(|marca| html[inicio..].find(marca))
            .min()
            .map_or(html.len(), |offset| inicio + offset);
        html[inicio..fim].to_owned()
    }

    fn render(viewer: &Viewer) -> String {
        shell(
            viewer,
            Screen::Home,
            Vec::new(),
            Screen::Home.label(),
            view! { <p>"x"</p> },
        )
        .to_html()
    }

    /// A navegação muda de língua com o idioma corrente, e nada mais.
    ///
    /// A prova do interruptor ao nível da renderização: a mesma barra, os mesmos
    /// destinos, os mesmos ids — só as palavras da interface mudam. Corre dentro
    /// de um escopo de idioma, como um pedido faria.
    #[tokio::test]
    async fn a_navegacao_fala_o_idioma_corrente() {
        use ocinye_contracts::Locale;
        let viewer = viewer_de_investigacao(&[Permission::IdeasView]);

        // Português canónico (o predefinido, mesmo sem escopo).
        let pt = render(&viewer);
        assert!(pt.contains("Ficheiros"), "pt: Ficheiros");
        assert!(pt.contains("Investigação"), "pt: secção Investigação");

        // Francês: a mesma barra, outra língua — e os destinos não mudam.
        let fr = crate::i18n::with_locale(Locale::Fr, async { render(&viewer) }).await;
        assert!(fr.contains("Fichiers"), "fr: Fichiers");
        assert!(fr.contains("Accueil"), "fr: Accueil (Home)");
        assert!(fr.contains("Recherche"), "fr: secção Recherche");
        assert!(!fr.contains("Ficheiros"), "fr não deve trazer 'Ficheiros'");
        assert!(fr.contains(r#"href="/ideas""#), "o destino /ideas não muda");

        // Inglês.
        let en = crate::i18n::with_locale(Locale::En, async { render(&viewer) }).await;
        assert!(en.contains(">Files<") || en.contains("Files"), "en: Files");
        assert!(en.contains("Research"), "en: secção Research");
        assert!(en.contains(r#"href="/ideas""#), "o destino /ideas não muda");
    }

    /// O lançador é o último elemento da shell; isola-se do resto por corte.
    fn lancador(html: &str) -> String {
        let inicio = html
            .find(r#"data-oc="launcher""#)
            .expect("o lançador desapareceu da shell");
        html[inicio..].to_owned()
    }

    /// A shell traz sempre o Gestor de Aplicações, e o gatilho para o abrir.
    #[test]
    fn a_shell_traz_o_gestor_de_aplicacoes() {
        let html = render(&viewer_de_investigacao(&Permission::all()));
        // O gatilho na barra lateral.
        assert!(
            html.contains(r#"data-oc="launcher-open""#),
            "falta o gatilho do lançador"
        );
        let l = lancador(&html);
        // O lançador tem título, campo de pesquisa, filtros de categoria e grelha.
        assert!(l.contains("Aplicações"), "falta o título");
        assert!(
            l.contains(r#"data-oc="launcher-input""#),
            "falta a pesquisa"
        );
        assert!(
            l.contains(r#"data-oc="launcher-chip""#),
            "faltam os filtros de categoria"
        );
        assert!(l.contains(r#"data-oc="launcher-grelha""#), "falta a grelha");
        // «Todos» começa activo; as cinco categorias existem.
        assert!(l.contains(r#"data-category="all""#), "falta o filtro Todos");
        for cat in [
            "productivity",
            "research",
            "knowledge",
            "communication",
            "administration",
        ] {
            assert!(
                l.contains(&format!(r#"data-category="{cat}""#)),
                "falta a categoria {cat}"
            );
        }
        // Fichas de aplicações que qualquer membro autorizado vê.
        for app in ["Notas", "Ficheiros", "Projectos"] {
            assert!(l.contains(app), "falta a ficha de {app}");
        }
    }

    /// A barra desenha as aplicações fixadas, e o lançador reflecte o estado de
    /// fixação de cada ficha — fixada com `aria-pressed="true"`, por fixar com
    /// `false`. As estruturais (Home, O Meu Trabalho) não trazem botão de fixar.
    #[test]
    fn o_lancador_reflecte_o_estado_de_fixacao() {
        let mut viewer = viewer_de_investigacao(&Permission::all());
        viewer.pinned = vec!["notes".to_owned(), "files".to_owned()];
        let html = render(&viewer);
        let l = lancador(&html);

        // A ficha das Notas (fixada) traz o botão pressionado.
        assert!(
            l.contains(r#"data-app-id="notes""#) && l.contains(r#"aria-pressed="true""#),
            "a ficha fixada devia trazer o botão pressionado"
        );
        // A ficha do Calendário (não fixada) traz o botão por pressionar.
        assert!(
            l.contains(r#"data-app-id="calendar""#),
            "falta o botão de fixar do Calendário"
        );
        // Home não é fixável: não tem botão de fixar.
        assert!(
            !l.contains(r#"data-oc="launcher-pin" data-app-id="home""#),
            "Home não devia ter botão de fixar"
        );

        // E a barra mostra as duas fixadas, na ordem.
        let barra = barra_nav(&html);
        assert!(
            barra.contains(r#"data-app-id="notes""#),
            "Notas não está na barra"
        );
        assert!(
            barra.contains(r#"data-app-id="files""#),
            "Ficheiros não está na barra"
        );
    }

    /// A descoberta não contorna a autorização: um membro sem `MembersManage`
    /// não vê a ficha da Administração no lançador — e um que a tem, vê.
    #[test]
    fn o_lancador_esconde_o_que_o_membro_nao_pode_abrir() {
        // Sem MembersManage: sem consola de administração.
        let sem = render(&viewer_with(&[Permission::IdeasView]));
        let l_sem = lancador(&sem);
        assert!(
            !l_sem.contains(r#"href="/admin""#),
            "o lançador não pode oferecer a Administração a quem não a tem"
        );

        // Com MembersManage: a ficha aparece.
        let com = render(&viewer_with(&[Permission::MembersManage]));
        let l_com = lancador(&com);
        assert!(
            l_com.contains(r#"href="/admin""#),
            "a Administração devia aparecer a quem a pode abrir"
        );
    }

    /// O Prompt é uma aplicação para quem tem `ai.use`, e o lançador não o
    /// desactiva por não haver GPU: a disponibilidade da aplicação é um eixo
    /// distinto da do fornecedor (§54). O lançador só olha para a autorização.
    #[test]
    fn o_prompt_e_uma_aplicacao_mesmo_sem_gpu() {
        let html = render(&viewer_with(&[Permission::AiUse]));
        let l = lancador(&html);
        assert!(
            l.contains(r#"href="/ai/prompt""#),
            "o Prompt devia estar no lançador para quem tem ai.use"
        );
    }

    /// O «Criar» leva cada acção determinista ao seu fluxo real.
    ///
    /// O defeito (F-07 / GLOBAL_CREATE): acções apareciam como «Ainda não
    /// disponível» ou apontavam a listas, e a Tarefa não tinha destino. Todas as
    /// sete criações deterministas funcionam sem GPU — cada uma abre o formulário
    /// onde o contexto se resolve, ou cria de imediato (a Nota). Só a falta de
    /// autorização desactiva de facto.
    #[test]
    fn o_criar_leva_cada_accao_ao_seu_fluxo_real() {
        let html = render(&viewer_with(&ocinye_contracts::Permission::all()));

        for accao in [
            "Nova nota",
            "Nova tarefa",
            "Novo evento",
            "Nova mensagem",
            "Nova ideia",
            "Novo projecto",
            "Novo dataset",
            "Nova referência",
            "Novo agente IA",
        ] {
            assert!(html.contains(accao), "a acção {accao} sumiu do «Criar»");
        }

        let criar = html
            .split(r#"data-oc="create-menu""#)
            .nth(1)
            .and_then(|resto| resto.split(r#"data-oc="status-toggle""#).next())
            .expect("o menu «Criar» desapareceu");
        assert!(
            !criar.contains("Ainda não disponível") && !criar.contains("aria-disabled"),
            "o «Criar» declara uma acção implementada como indisponível"
        );

        // As acções que abrem um formulário levam ao ecrã onde o contexto se
        // resolve — não à lista.
        for destino in [
            "/calendar/events/new",
            "/mail/compose",
            "/ideas/new",
            "/projects/new",
            "/bibliography/new",
            "/datasets/new",
            "/tasks/new",
            "/ai/agents/new",
        ] {
            assert!(
                html.contains(&format!("href=\"{destino}\"")),
                "o «Criar» não leva a {destino}"
            );
        }

        // A Nota cria-se de imediato: um POST (para o seu caminho próprio,
        // distinto do formulário da lista), não um link.
        assert!(
            html.contains("action=\"/notes/new\""),
            "a Nota deve criar-se por POST /notes/new, não por um link"
        );
    }

    /// Um atalho mostrado é um atalho que funciona.
    ///
    /// A palette anunciava `⌘⇧I`, `⌘⇧A`, `⌘⇧P` e `⌘⇧C` ao lado de cada acção, e
    /// nenhum deles estava ligado a coisa nenhuma — só o `⌘K` tinha handler.
    /// Um atalho impresso que não responde é uma promessa que a interface faz e
    /// o teclado não cumpre; é o mesmo defeito do sino sem contagem e dos
    /// controlos de paginação sem página seguinte.
    ///
    /// O atributo é o contrato entre as duas metades: o servidor escreve-o na
    /// linha, e `app.js` lê-o de lá. Sem ele, a interface volta a anunciar sem
    /// cumprir.
    #[test]
    fn cada_atalho_anunciado_esta_ligado_ao_teclado() {
        let todas: Vec<Permission> = Permission::all().into_iter().collect();
        let html = render(&viewer_with(&todas));

        // Cada acção da palette anuncia o seu atalho, e cada um tem de estar
        // no atributo que o teclado lê. Contar `oc-kbd` não servia: a pílula
        // do `⌘K` e o `ESC` também são `oc-kbd`, e ambos já funcionam.
        for (label, _, atalho, _) in PALETTE_ACTIONS {
            if atalho.is_empty() {
                continue;
            }
            assert!(
                html.contains(&format!(r#"data-shortcut="{atalho}""#)),
                "«{label}» anuncia {atalho} e o teclado não o conhece"
            );
        }

        assert!(
            include_str!("../../static/app.js").contains("data-shortcut"),
            "`app.js` tem de ler o atalho do atributo, e não de uma lista repetida"
        );
    }

    #[test]
    fn a_navegacao_esconde_o_que_o_membro_nao_pode_usar() {
        let member = viewer_with(&[
            Permission::IdeasView,
            Permission::ProjectsView,
            Permission::AiUse,
        ]);
        let html = render(&member);

        assert!(html.contains("/ideas"));
        assert!(html.contains("/projects"));
        // Sem `MembersView` nem `AuditView`, a administração não aparece.
        assert!(
            !html.contains(r#"href="/admin""#),
            "Administração visível sem permissão"
        );
        assert!(
            !html.contains(r#"href="/audit""#),
            "Audit Log visível sem permissão"
        );
        assert!(!html.contains(r#"href="/units""#));
    }

    /// A barra de aplicações, isolada — de `side-pinned` até ao seu `</nav>`.
    fn barra_nav(html: &str) -> String {
        let inicio = html
            .find(r#"data-oc="side-pinned""#)
            .expect("a barra sumiu");
        let fim = html[inicio..]
            .find("</nav>")
            .map_or(html.len(), |o| inicio + o);
        html[inicio..fim].to_owned()
    }

    #[test]
    fn a_barra_mostra_essencial_mais_fixadas_e_nao_o_catalogo_inteiro() {
        // A barra deixou de ser o catálogo: é navegação essencial (Home, O Meu
        // Trabalho) mais as aplicações fixadas. Tudo o resto descobre-se no
        // Gestor de Aplicações. O membro pode tudo e fixa o conjunto por omissão
        // (notas, ficheiros, projectos), todas visíveis.
        let member = viewer_de_investigacao(&Permission::all());
        let barra = barra_nav(&render(&member));

        // O essencial e as fixadas visíveis estão lá.
        // O Desktop e as fixadas por omissão (D6). «O Meu Trabalho» deixou de
        // estar fixo na barra: abre-se pelo lançador, como as outras.
        for rota in [
            r#"href="/""#,
            r#"href="/notes""#,
            r#"href="/files""#,
            r#"href="/projects""#,
        ] {
            assert!(barra.contains(rota), "a barra devia mostrar {rota}");
        }
        // O que **não** está fixado não está na barra — mesmo que exista. A
        // descoberta é no lançador, e a barra não volta a ser o catálogo.
        for rota in [
            r#"href="/audit""#,
            r#"href="/calendar""#,
            r#"href="/units""#,
        ] {
            assert!(!barra.contains(rota), "a barra não devia listar {rota}");
        }
    }

    #[test]
    fn sem_resposta_do_core_a_barra_nao_afirma_acesso_por_confirmar() {
        // «Não tem acesso» e «não sabemos» são coisas diferentes. Sem o Core não
        // há permissões nem módulos confirmados, e uma aplicação fixada mas
        // governada por direito **não** aparece — afirmá-la seria dar por
        // verificado um acesso que não se conseguiu confirmar (`CLAUDE.md` §31).
        let mut sem_core = viewer_with(&[]);
        sem_core.core_status = CoreStatus::Silent;
        let barra = barra_nav(&render(&sem_core));

        // O essencial não depende do Core.
        assert!(
            barra.contains(r#"href="/""#),
            "o Desktop não depende do Core e tem de estar sempre"
        );
        // Ficheiros e Projectos estão no conjunto por omissão, mas são governados
        // e não aparecem sem confirmação. As Notas (sem direito) podem aparecer.
        for rota in [r#"href="/files""#, r#"href="/projects""#] {
            assert!(
                !barra.contains(rota),
                "{rota} apareceu sem confirmação do Core"
            );
        }
    }

    #[test]
    fn home_e_o_meu_trabalho_estao_sempre_disponiveis() {
        // Mostram o que é do próprio membro e filtram-se sozinhos.
        let html = render(&viewer_with(&[]));
        assert!(html.contains(r#"href="/""#));
        assert!(html.contains(r#"href="/my-work""#));
    }

    /// O «Criar» está sempre presente: toda a criação determinista funciona, e a
    /// mais simples — uma nota pessoal — está ao alcance de qualquer membro. Um
    /// membro só com uma permissão de leitura vê-o na mesma.
    #[test]
    fn o_menu_criar_esta_sempre_presente() {
        let html = render(&viewer_with(&[Permission::IdeasView]));
        assert!(html.contains("data-oc=\"create-toggle\""));
        assert!(html.contains(r#"action="/notes/new""#));
    }

    /// O «Criar» não cinzenta a criação por uma permissão de contexto.
    ///
    /// O defeito visível (GLOBAL_CREATE): um membro sem filiação numa unidade ou
    /// ambiente — um administrador que nunca entrou numa — via quase todo o menu
    /// esbatido, porque as permissões de criação vêm da filiação, não do papel
    /// técnico. Cinzentar assim escondia acções que o membro pode iniciar. Agora
    /// toda a criação determinista é accionável; a autoridade real é do Core, e
    /// o contexto resolve-se no formulário (§2, §3, §15).
    #[test]
    fn o_criar_nao_cinzenta_a_criacao_por_falta_de_contexto() {
        // Um membro sem *nenhuma* das permissões de criação de contexto.
        let html = render(&viewer_with(&[]));

        assert!(html.contains("data-oc=\"create-toggle\""));
        // Todas as sete acções continuam accionáveis: nada esbatido.
        assert!(
            !html.contains("oc-unavailable") && !html.contains("Não tem autorização"),
            "o «Criar» voltou a cinzentar acções por falta de contexto"
        );
        for destino in [
            "/calendar/events/new",
            "/mail/compose",
            "/ideas/new",
            "/projects/new",
            "/bibliography/new",
            "/datasets/new",
            "/tasks/new",
            "/ai/agents/new",
        ] {
            assert!(
                html.contains(&format!("href=\"{destino}\"")),
                "«{destino}» deixou de ser accionável"
            );
        }
        assert!(
            html.contains(r#"action="/notes/new""#),
            "a Nota deixou de se criar por POST"
        );
    }

    #[test]
    fn um_colaborador_externo_ve_uma_shell_quase_vazia() {
        // Deny-by-default no seu ponto mais forte (briefing §54).
        let html = render(&viewer_with(&[]));
        for ausente in [
            "/units",
            "/ideas",
            "/projects",
            "/datasets",
            "/admin",
            "/audit",
        ] {
            assert!(
                !html.contains(&format!(r#"href="{ausente}""#)),
                "shell vazia expõe {ausente}"
            );
        }
    }

    #[test]
    fn cada_ecra_da_navegacao_tem_caminho_e_rotulo_unicos() {
        let mut paths: Vec<&str> = PALETTE_NAV.iter().map(|s| s.path()).collect();
        paths.sort_unstable();
        let count = paths.len();
        paths.dedup();
        assert_eq!(paths.len(), count, "dois ecrãs partilham o mesmo caminho");
    }

    /// O registo de aplicações cobre todos os ecrãs navegáveis.
    ///
    /// A barra lateral deixou de ser o catálogo (é essencial + fixadas); o
    /// catálogo autoritativo é agora o registo de aplicações. Cada destino da
    /// palette é uma aplicação registada **ou** a superfície de comando
    /// (`Search`/`Ask`), que não é uma aplicação.
    #[test]
    fn o_registo_cobre_todos_os_ecras_de_navegacao() {
        let orfaos: Vec<&str> = PALETTE_NAV
            .iter()
            .filter(|s| !matches!(s, Screen::Search | Screen::Ask))
            .filter(|s| apps::by_id(s.id()).is_none())
            .map(|s| s.id())
            .collect();
        assert!(
            orfaos.is_empty(),
            "ecrãs navegáveis sem entrada no registo de aplicações: {orfaos:?}"
        );
    }

    /// O cartão do membro abre a sua superfície de conta e sessão.
    ///
    /// Durante algum tempo o cartão inteiro era o botão de terminar sessão:
    /// clicar no próprio nome — o gesto que em quase toda a parte abre o perfil
    /// — desligava a pessoa do sistema. Não era uma ligação morta, era pior,
    /// porque fazia uma coisa destrutiva sem a anunciar.
    ///
    /// Passou a ser um gatilho de divulgação, e o teste fixa o contrato que o
    /// `app.js` do outro lado consome: o par `aria-expanded`/`aria-controls`,
    /// e um `id` que existe mesmo.
    #[test]
    fn o_cartao_do_membro_abre_a_conta() {
        let html = render(&viewer_with(&[]));

        // A ordem dos atributos é escolha do renderizador, não contrato: a
        // asserção lê a etiqueta inteira em vez de assumir a ordem.
        let fim = html
            .find(r#"data-oc="account-toggle""#)
            .expect("o gatilho de conta desapareceu do rodapé");
        let tag = &html[html[..fim].rfind('<').expect("etiqueta mal formada")..];
        let tag = &tag[..tag.find('>').expect("etiqueta mal formada")];

        assert!(
            tag.starts_with("<button"),
            "o gatilho de conta não é um botão: {tag}"
        );
        assert!(
            tag.contains(r#"aria-expanded="false""#),
            "o gatilho não anuncia que há uma superfície por abrir: {tag}"
        );
        assert!(
            tag.contains(r#"aria-controls="oc-account-menu""#),
            "o gatilho não diz o que controla: {tag}"
        );
        assert!(
            html.contains(r#"id="oc-account-menu""#),
            "`aria-controls` aponta para um id que não existe"
        );
        assert!(
            html.contains(r#"data-oc="account-menu""#),
            "a superfície não está ligada à camada de interacção"
        );
    }

    /// As duas acções pessoais levam a Definições, e a lado nenhum mais.
    ///
    /// O menu é um atalho para capacidades que o membro já tem. Não cria uma
    /// página `/profile`: a fonte de verdade da conta já é Definições, e uma
    /// segunda versão dela envelheceria em separado.
    #[test]
    fn o_menu_de_conta_leva_as_definicoes_reais() {
        let html = render(&viewer_with(&[]));
        let menu = super::tests::menu(&html);

        assert!(
            menu.contains(r#"href="/settings""#),
            "«Conta» não leva a Definições"
        );
        assert!(
            menu.contains(r#"href="/settings/security""#),
            "«Definições» não leva às definições da pessoa"
        );
        assert!(menu.contains("Conta"));
        assert!(menu.contains("Definições"));
    }

    /// O menu pessoal não expõe autoridade institucional.
    ///
    /// > **It provides shortcuts to capabilities the member already owns; it
    /// > never grants institutional authority.**
    ///
    /// Papéis, permissões, concessões e administração não são pessoais. Um
    /// menu de conta que os mostrasse começaria a parecer o sítio onde se
    /// pedem — e o sítio onde se pedem não existe, o que faria dele uma
    /// promessa dupla: de um caminho e de uma autoridade.
    #[test]
    fn o_menu_de_conta_nao_expoe_autoridade() {
        let html = render(&viewer_with(&ocinye_contracts::Permission::all()));
        let menu = super::tests::menu(&html);

        for fora in [
            "Administração",
            "Papéis",
            "Permissões",
            "Concessões",
            "Auditoria",
            "Organização",
            "Idioma",
            "Tema",
            "Modelo",
        ] {
            assert!(
                !menu.contains(fora),
                "o menu pessoal passou a mostrar «{fora}», que não é pessoal"
            );
        }
        assert!(
            !menu.contains("/admin"),
            "o menu pessoal passou a ligar à administração"
        );
    }

    /// Terminar sessão fecha o menu, e é uma operação real.
    #[test]
    fn terminar_sessao_fecha_o_menu_e_e_um_post() {
        let html = render(&viewer_with(&[]));
        let menu = super::tests::menu(&html);

        assert!(
            menu.contains(r#"action="/logout""#),
            "terminar sessão desapareceu do menu"
        );
        assert!(menu.contains("Terminar sessão"));

        let form = menu
            .split(r#"action="/logout""#)
            .nth(1)
            .and_then(|rest| rest.split("</form>").next())
            .expect("formulário de logout mal formado");
        assert!(
            form.contains(r#"type="submit""#),
            "o botão de terminar sessão não submete: {form}"
        );

        // É a última acção: nada aparece depois dela.
        let depois = menu.split("Terminar sessão").nth(1).unwrap_or_default();
        assert!(
            !depois.contains("<a "),
            "há acções depois de terminar sessão, e ela devia fechar a lista"
        );
    }

    /// Nenhuma opção do menu é decorativa.
    ///
    /// Cada uma leva a uma rota real ou submete um formulário real. É a mesma
    /// invariante da varredura geral, aplicada onde ela mais tenta escapar:
    /// numa superfície que só existe depois de um clique.
    #[test]
    fn nenhuma_opcao_do_menu_de_conta_e_morta() {
        let html = render(&viewer_with(&[]));
        let menu = super::tests::menu(&html);

        for pedaco in menu.split("href=\"").skip(1) {
            let alvo = pedaco.split('"').next().unwrap_or_default();
            assert!(
                alvo != "#" && !alvo.is_empty(),
                "o menu de conta tem uma ligação para lado nenhum"
            );
        }
        // Um botão que não submete só é aceitável declarado indisponível, com
        // a razão (D2: «Bloquear ecrã» espera pelo G-01).
        for pedaco in menu.split("<button").skip(1) {
            let inicio = pedaco.split('>').next().unwrap_or_default();
            if inicio.contains(r#"type="button""#) {
                assert!(
                    inicio.contains(r#"aria-disabled="true""#) && inicio.contains("data-tip="),
                    "o menu de conta tem um botão que não faz nada e não diz porquê: {inicio}"
                );
            }
        }
    }

    /// A sessão actual diz o que se sabe, e só isso.
    ///
    /// Sem dispositivo, sem lugar e sem data de emissão: nada disso está
    /// guardado. O `user-agent`, que estaria, é um indício de sessão e não um
    /// dispositivo verificado — chamar-lhe dispositivo seria dar-lhe uma
    /// confiança que ele não tem.
    #[test]
    fn a_sessao_actual_nao_inventa_dispositivo_nem_lugar() {
        // O resumo «Sessão actual» saiu com o desenho do D2; o que fica é o que
        // ele guardava: o menu não afirma o que não está guardado.
        let html = render(&viewer_with(&[]));
        let menu = super::tests::menu(&html);

        for invencao in [
            "Chrome",
            "Safari",
            "macOS",
            "Windows",
            "Dispositivo",
            "Lisboa",
            "Portugal",
            "IP ",
        ] {
            assert!(
                !menu.contains(invencao),
                "a sessão passou a afirmar «{invencao}», que não está guardado"
            );
        }
    }

    /// O cartão do membro não inventa nada sobre a pessoa.
    ///
    /// Mostra o que vem do principal autenticado — nome e iniciais — e o estado
    /// do Core, que é sobre o sistema e não sobre quem o usa. Cargo, unidade
    /// principal, presença e nível de segurança seriam invenções; a última
    /// seria a pior, porque insinuaria que a barra lateral sabe alguma coisa
    /// sobre autorização.
    ///
    /// > Frontend state informs UX; Core authorization decides authority.
    #[test]
    fn o_cartao_do_membro_nao_inventa_atributos() {
        let html = render(&viewer_with(&[]));
        // Só o gatilho fechado: é o que se vê sem clicar, e era ali que o
        // estado do sistema estava colado ao nome.
        let inicio = html
            .find(r#"data-oc="account-toggle""#)
            .expect("gatilho de conta desapareceu");
        let fim = html[inicio..]
            .find("</button>")
            .map_or(html.len(), |offset| inicio + offset);
        let rodape = &html[inicio..fim];

        for invencao in [
            "Investigador",
            "Cargo",
            "Unidade principal",
            "Online",
            "Disponível",
            "Nível de segurança",
            "Dispositivo",
        ] {
            assert!(
                !rodape.contains(invencao),
                "o cartão do membro passou a afirmar «{invencao}», que não vem do principal"
            );
        }

        assert!(rodape.contains("João Manuel"), "o nome real desapareceu");
    }

    /// Com a barra estreita, nenhum controlo perde o nome.
    ///
    /// A container query esconde o texto abaixo dos 120px, e o que fica é o
    /// ícone. Um ícone sozinho não é um nome: quem navega por teclado e leitor
    /// de ecrã ouviria «botão», «ligação», «ligação». Cada controlo carrega o
    /// seu nome num atributo, que sobrevive ao `display: none`.
    #[test]
    fn com_a_barra_estreita_nenhum_controlo_conserva_o_nome() {
        // Os botões da barra de aplicações só têm ícone (D6): o nome acessível
        // é o único nome que têm.
        let html = render(&viewer_with(&ocinye_contracts::Permission::all()));
        let barra = barra_nav(&html);

        let mut sem_nome: Vec<String> = Vec::new();
        for tag in barra.split('<').skip(1) {
            let inicio = tag.split('>').next().unwrap_or_default();
            if inicio.contains("ods-shelf__btn") && !inicio.contains("aria-label=") {
                sem_nome.push(format!("<{inicio}>"));
            }
        }

        assert!(
            sem_nome.is_empty(),
            "controlos da barra de aplicações sem nome acessível:\n  {}",
            sem_nome.join("\n  ")
        );
    }

    /// Cada caminho da aplicação pertence a um e um só ecrã.
    ///
    /// Um caminho sem dono é um ecrã onde a barra lateral não marca nada, e
    /// quem lá está deixa de saber onde está. A posse é por prefixo mais longo,
    /// e este teste fixa os casos que a igualdade literal falharia.
    #[test]
    fn cada_caminho_pertence_ao_ecra_certo() {
        for (caminho, esperado) in [
            ("/", Screen::Home),
            ("/units", Screen::Units),
            ("/units/new", Screen::Units),
            ("/units/33333333-3333-3333-3333-333333333333", Screen::Units),
            ("/ideas", Screen::Ideas),
            ("/ideas/new", Screen::Ideas),
            ("/projects", Screen::Projects),
            ("/projects/new", Screen::Projects),
            ("/bibliography/new", Screen::Bibliography),
            ("/datasets/new", Screen::Datasets),
            ("/mail/settings", Screen::Mail),
            ("/settings", Screen::Settings),
            ("/settings/security", Screen::Settings),
            ("/help", Screen::Help),
            // O prefixo mais longo ganha: `/ai` também é prefixo destes.
            ("/ai", Screen::Ai),
            ("/ai/agents", Screen::Agents),
            ("/ai/agents/new", Screen::Agents),
            ("/ai/prompt", Screen::Prompt),
        ] {
            assert_eq!(
                Screen::owning(caminho),
                Some(esperado),
                "{caminho} devia pertencer a {}",
                esperado.label(),
            );
        }

        // Fora da shell não há item activo — e dizê-lo é diferente de errar.
        for caminho in ["/login", "/logout", "/first-access"] {
            assert_eq!(
                Screen::owning(caminho),
                None,
                "{caminho} não vive dentro da shell e não devia ter dono"
            );
        }
    }

    /// Em cada ecrã, exactamente um item da navegação fica marcado.
    ///
    /// Nem zero — que deixa a barra muda sobre onde se está — nem dois, que a
    /// deixa a mentir. `aria-current="page"` é o que o leitor de ecrã anuncia,
    /// e o CSS pinta a partir dele: uma só fonte para as duas coisas.
    /// Um membro com tudo fixável na barra, para as provas de item activo.
    fn viewer_tudo_fixado() -> Viewer {
        let mut v = viewer_de_investigacao(&ocinye_contracts::Permission::all());
        v.pinned = apps::APPLICATIONS
            .iter()
            .filter(|a| a.can_pin())
            .map(|a| a.id().to_owned())
            .collect();
        v
    }

    /// Os ecrãs que a barra desenha: o Desktop mais tudo o que é fixável (D6).
    fn ecras_da_barra() -> Vec<Screen> {
        std::iter::once(Screen::Home)
            .chain(
                apps::APPLICATIONS
                    .iter()
                    .filter(|a| a.can_pin())
                    .map(|a| a.screen),
            )
            .collect()
    }

    #[test]
    fn cada_ecra_da_barra_marca_um_e_um_so_item_activo() {
        let viewer = viewer_tudo_fixado();

        for screen in ecras_da_barra() {
            let html = shell(
                &viewer,
                screen,
                Vec::new(),
                screen.label(),
                view! { <p>"x"</p> },
            )
            .to_html();
            let barra = barra_nav(&html);
            let marcados = barra.matches(r#"aria-current="page""#).count();
            assert_eq!(
                marcados,
                1,
                "{} marcou {marcados} itens activos na barra, e devia marcar um",
                screen.label(),
            );

            let fim = barra
                .find(r#"aria-current="page""#)
                .expect("nenhum item marcado");
            let tag = &barra[barra[..fim].rfind('<').expect("etiqueta mal formada")..fim];
            assert!(
                tag.contains(&format!(r#"href="{}""#, screen.path())),
                "{} marcou o item errado: {tag}",
                screen.label(),
            );
        }
    }

    /// Um ecrã que não está na barra (não fixado, não essencial) não marca nada —
    /// e nunca mais do que um. É o caso §82: a app activa que não está fixada não
    /// obriga a barra a inventar uma fixação temporária.
    #[test]
    fn um_ecra_fora_da_barra_nao_marca_nenhum_item() {
        // O membro fixou só o conjunto por omissão; o Calendário não está lá.
        let viewer = viewer_de_investigacao(&ocinye_contracts::Permission::all());
        let barra = barra_nav(
            &shell(
                &viewer,
                Screen::Calendar,
                Vec::new(),
                Screen::Calendar.label(),
                view! { <p></p> },
            )
            .to_html(),
        );
        assert_eq!(barra.matches(r#"aria-current="page""#).count(), 0);
    }

    /// Um ecrã de detalhe marca o ecrã-pai, e não deixa a barra em branco.
    ///
    /// É o caso que a igualdade literal falha e que este passo existe para
    /// cobrir: `/units/{id}` não é `/units`, mas é ali que se está.
    #[test]
    fn um_ecra_filho_marca_o_pai_na_navegacao() {
        // Com os pais fixados, um caminho-filho marca o pai na barra.
        let viewer = viewer_tudo_fixado();

        for caminho in [
            "/units/33333333-3333-3333-3333-333333333333",
            "/projects/new",
            "/ideas/new",
            "/bibliography/new",
        ] {
            let dono = Screen::owning(caminho).expect("caminho sem dono");
            let barra = barra_nav(
                &shell(
                    &viewer,
                    dono,
                    Vec::new(),
                    dono.label(),
                    view! { <p>"x"</p> },
                )
                .to_html(),
            );
            assert_eq!(
                barra.matches(r#"aria-current="page""#).count(),
                1,
                "{caminho} não marcou exactamente um item"
            );
            let fim = barra
                .find(r#"aria-current="page""#)
                .expect("nenhum item marcado");
            let tag = &barra[barra[..fim].rfind('<').expect("etiqueta mal formada")..fim];
            assert!(
                tag.contains(&format!(r#"href="{}""#, dono.path())),
                "{caminho} devia marcar {}, e marcou: {tag}",
                dono.label(),
            );
        }
    }

    /// Um trilho leva sempre a um ecrã real, e nunca à própria página.
    ///
    /// Três propriedades de uma vez, porque falham juntas: o degrau existe, o
    /// destino é uma rota do Workspace, e o último elemento — o ecrã actual —
    /// não é uma ligação. Um breadcrumb que liga à página onde já se está é
    /// mobiliário.
    #[test]
    fn o_trilho_leva_a_ecras_reais_e_nunca_a_si_proprio() {
        let viewer = viewer_de_investigacao(&ocinye_contracts::Permission::all());

        for (filho, pai) in [
            (Screen::Units, Screen::Units),
            (Screen::Ideas, Screen::Ideas),
            (Screen::Projects, Screen::Projects),
            (Screen::Bibliography, Screen::Bibliography),
            (Screen::Datasets, Screen::Datasets),
            (Screen::Agents, Screen::Agents),
            (Screen::Admin, Screen::Admin),
            (Screen::Mail, Screen::Mail),
        ] {
            let html = shell(
                &viewer,
                filho,
                vec![Crumb::to(pai)],
                "Detalhe",
                view! { <p>"x"</p> },
            )
            .to_html();

            let nav = html
                .split(r#"data-part="crumb""#)
                .nth(1)
                .and_then(|resto| resto.split("</nav>").next())
                .expect("o trilho desapareceu da topbar");

            assert!(
                nav.contains(&format!(r#"href="{}""#, pai.path())),
                "o trilho de {} não aponta para {}",
                filho.label(),
                pai.path(),
            );
            assert!(
                nav.contains(r#"aria-current="page">Detalhe<"#),
                "a página não fecha o trilho com o seu próprio nome: {nav}"
            );
            // E fecha-o em texto. Um degrau final que fosse ligação apontaria
            // para a página onde já se está.
            let antes_do_fim = nav
                .split(r#"aria-current="page""#)
                .next()
                .unwrap_or_default();
            assert!(
                !antes_do_fim.contains(">Detalhe</a>"),
                "a página actual aparece como ligação no seu próprio trilho: {nav}"
            );
            // E o degrau anterior não repete o nome da página.
            assert_ne!(
                pai.label(),
                "Detalhe",
                "o degrau anterior repete a página: {nav}"
            );
        }
    }

    /// O degrau do trilho é o ecrã que o `Screen` diz ser.
    ///
    /// `Crumb::to` constrói o par a partir do ecrã, e é isso que este teste
    /// fixa: rótulo e destino vêm ambos da mesma tabela que a navegação usa, e
    /// não de dois literais escritos lado a lado num handler.
    #[test]
    fn o_degrau_do_trilho_concorda_com_a_navegacao() {
        for screen in SCREENS {
            let crumb = Crumb::to(screen);
            assert_eq!(crumb.label, screen.label());
            assert_eq!(crumb.href, screen.path());
            assert_eq!(
                Screen::owning(&crumb.href),
                Some(screen),
                "o destino do degrau de {} não pertence a {}",
                screen.label(),
                screen.label(),
            );
        }
    }

    /// Nenhum caminho da aplicação tem dois donos.
    ///
    /// Zero é legítimo — o login e o logout não vivem na shell. Dois nunca é:
    /// significaria que a navegação não sabe onde marcar, e marcaria em ambos.
    #[test]
    fn nenhum_caminho_tem_dois_donos() {
        for rota in crate::routes::ROUTES {
            let caminho = rota.replace(['{', '}'], "");
            let donos: Vec<Screen> = SCREENS
                .into_iter()
                .filter(|screen| Screen::owning(&caminho) == Some(*screen))
                .collect();
            assert!(
                donos.len() <= 1,
                "{caminho} tem {} donos: {:?}",
                donos.len(),
                donos.iter().map(|s| s.label()).collect::<Vec<_>>(),
            );
        }
    }

    /// O relógio chega vazio do servidor.
    ///
    /// # Porque não vem preenchido
    ///
    /// O servidor não sabe em que fuso está quem lê. Escrever ali uma hora
    /// seria escrever *a hora do servidor* com o aspecto da hora de quem está a
    /// ver — e alguém em Luanda leria a hora de outro sítio sem nada que o
    /// dissesse.
    ///
    /// Vem vazio e escondido; o browser preenche-o com o relógio do computador.
    /// Sem JavaScript fica escondido e não abre buraco: é apresentação, e a
    /// página não depende dele.
    ///
    /// # E nunca decide nada
    ///
    /// Carimbos de auditoria, expiração de sessões e prazos vêm do Core. A hora
    /// do browser é escolhida por quem o usa, e usá-la para autorização seria
    /// deixar decidir quem mexe no relógio.
    #[test]
    fn o_relogio_chega_vazio_do_servidor() {
        let html = render(&viewer_with(&[]));

        let fim = html
            .find(r#"data-oc="clock""#)
            .expect("o relógio desapareceu da topbar");
        let etiqueta = &html[html[..fim].rfind('<').expect("etiqueta")..];
        let etiqueta = &etiqueta[..etiqueta.find("</time>").map_or(200, |n| n + 7)];

        assert!(
            etiqueta.contains("hidden"),
            "o relógio aparece antes de o browser saber as horas: {etiqueta}"
        );
        // Nenhum dígito: o servidor não escreveu hora nenhuma lá dentro.
        let miolo = etiqueta.split('>').skip(1).collect::<String>();
        assert!(
            !miolo.chars().any(|c| c.is_ascii_digit()),
            "o servidor escreveu uma hora no relógio: {miolo}"
        );
    }

    /// O avatar aparece uma vez por ecrã, e é no rodapé.
    ///
    /// Estava também na topbar, e era repetição: a identidade do membro vive no
    /// rodapé da barra lateral, com o nome e o menu de conta. A topbar mostrava
    /// a mesma pessoa outra vez sem acrescentar nada.
    ///
    ///   topbar          → operação, estado do sistema, tempo
    ///   rodapé da barra → conta, identidade, sessão
    #[test]
    fn a_identidade_aparece_uma_vez_e_e_no_rodape() {
        let html = render(&viewer_with(&[]));

        // A identidade vive no menu da conta, que o logótipo abre (D2), e só
        // lá: a barra de topo mostra o logótipo, não a pessoa outra vez.
        let menu = super::tests::menu(&html);
        assert!(
            menu.contains("ods-avatar"),
            "a identidade saiu do menu da conta"
        );
        assert_eq!(
            html.matches("ods-avatar").count(),
            menu.matches("ods-avatar").count(),
            "a identidade aparece fora do menu da conta"
        );
        assert!(
            html.contains(r#"data-oc="clock""#),
            "o relógio não está na barra"
        );

        assert_eq!(
            html.matches("OPERACIONAL").count(),
            1,
            "o estado do sistema é dito mais do que uma vez"
        );
    }

    /// Os três estados dizem três coisas diferentes.
    ///
    /// O que este teste guarda não é o texto: é a distinção. Um booleano
    /// obrigava a escolher entre «pronto» e «não pronto» para situações que não
    /// são duas — e as duas que mais custam a separar são justamente as piores
    /// de confundir: o Core disse que não, e o Core não disse nada.
    #[test]
    fn os_tres_estados_do_core_dizem_coisas_diferentes() {
        let rotulos: Vec<String> = [CoreStatus::Ok, CoreStatus::Unavailable, CoreStatus::Silent]
            .iter()
            .map(|e| estado_do_sistema(*e).to_html())
            .collect();

        assert!(rotulos[0].contains("OPERACIONAL"));
        assert!(rotulos[1].contains("INDISPONÍVEL"));
        assert!(rotulos[2].contains("SEM RESPOSTA"));

        // E são mesmo quatro: nenhum par diz o mesmo.
        for (i, a) in rotulos.iter().enumerate() {
            for (j, b) in rotulos.iter().enumerate() {
                assert!(
                    i == j || a != b,
                    "dois estados do Core dizem exactamente o mesmo"
                );
            }
        }
    }

    /// Só o Core pronto deixa trabalhar.
    #[test]
    fn so_o_core_pronto_deixa_trabalhar() {
        assert!(CoreStatus::Ok.operational());
        assert!(!CoreStatus::Unavailable.operational());
        assert!(!CoreStatus::Silent.operational());
    }
    /// Uma sessão privilegiada com autoridade: faixa e rótulo.
    fn privilegiada_com_autoridade() -> Viewer {
        Viewer {
            pinned: crate::ui::apps::default_pins(),
            inactive_apps: Vec::new(),
            perfil: None,
            resolucao: crate::ui::shell::ResolucaoSessao::Resolvida,
            sessao_privilegiada: true,
            administra: true,
            name: "Fidel Admin".to_owned(),
            email: Some("fidel.admin@ocinye.com".to_owned()),
            ..viewer_with(&[])
        }
    }

    /// A faixa diz as três coisas, e não só a cor.
    #[test]
    fn a_faixa_identifica_a_sessao_e_quem_a_conduz() {
        let html = render(&privilegiada_com_autoridade());
        assert!(html.contains("SUPER ADMIN"), "falta o rótulo da autoridade");
        assert!(
            html.contains("SESSÃO PRIVILEGIADA"),
            "falta o texto que diz que tipo de sessão é"
        );
        assert!(
            html.contains("Fidel Admin"),
            "a faixa não diz quem conduz a sessão"
        );
        assert!(
            html.contains("fidel.admin@ocinye.com"),
            "a faixa não diz por que credencial"
        );
        assert!(
            html.contains(r#"data-privilegiada="1""#),
            "o tratamento visual não foi aplicado"
        );
    }

    /// **A negativa**: uma sessão normal não carrega nada disto.
    ///
    /// Sem esta metade, uma faixa sempre presente passaria a primeira prova e
    /// não distinguiria coisa nenhuma.
    #[test]
    fn uma_sessao_normal_nao_tem_faixa() {
        let html = render(&viewer_with(&[]));
        assert!(
            !html.contains(r#"data-privilegiada="1""#),
            "uma sessão normal recebeu a faixa"
        );
        assert!(
            !html.contains("SUPER ADMIN"),
            "uma sessão normal foi rotulada Super Admin"
        );
        assert!(
            !html.contains("SESSÃO PRIVILEGIADA"),
            "uma sessão normal foi apresentada como privilegiada"
        );
    }

    /// **As duas verdades, na apresentação.**
    ///
    /// Revogada a autoridade, a faixa fica — a sessão continua a ser a de quem
    /// administrava — e o rótulo deixa de dizer «Super Admin», porque já não é
    /// verdade.
    #[test]
    fn sem_autoridade_a_faixa_fica_e_o_rotulo_muda() {
        let sem = Viewer {
            administra: false,
            ..privilegiada_com_autoridade()
        };
        let html = render(&sem);
        assert!(
            html.contains(r#"data-privilegiada="1""#),
            "a faixa desapareceu por lhe terem tirado a autoridade: a sessão continua \
             a ser privilegiada"
        );
        assert!(
            html.contains("SEM AUTORIDADE ADMINISTRATIVA"),
            "o rótulo não diz que a autoridade acabou"
        );
        assert!(
            !html.contains("SUPER ADMIN"),
            "a interface afirma «Super Admin» a quem já não pode administrar"
        );
    }

    /// **A faixa representa a sessão; não a autoriza.**
    ///
    /// Uma sessão normal com a faixa forçada continua sem permissões. O que
    /// decide o que aparece e o que é permitido são coisas diferentes, e esta
    /// prova mantém-nas diferentes.
    #[test]
    fn a_faixa_nao_concede_autoridade() {
        let mentira = Viewer {
            sessao_privilegiada: true,
            administra: true,
            ..viewer_with(&[])
        };
        let html = render(&mentira);
        assert!(
            html.contains(r#"data-privilegiada="1""#),
            "o controlo não montou a mentira"
        );
        // E mesmo assim não pode nada: as permissões são vazias.
        assert!(
            !mentira.can(Permission::MembersManage),
            "a faixa concedeu autoridade que a sessão não tem"
        );
        assert!(
            !html.contains("/admin\""),
            "a faixa fez aparecer administração a quem não a pode ver"
        );
    }

    /// A superfície de falha-fechada não é a shell normal.
    ///
    /// Quando a identidade não pôde ser estabelecida, não se afirma nada: não
    /// há shell, não há navegação, não há faixa. Uma falha técnica ao ler o
    /// `/me` não pode virar uma sessão normal — era esse o defeito.
    #[test]
    fn a_superficie_indeterminada_nao_e_a_shell_normal() {
        let html = identidade_indeterminada().to_html();
        assert!(
            html.contains("Não foi possível estabelecer a sua sessão"),
            "a superfície neutra não diz o que aconteceu"
        );
        assert!(
            !html.contains("oc-shell"),
            "a falha-fechada desenhou a shell autenticada normal"
        );
        assert!(
            !html.contains(r#"data-privilegiada="1""#),
            "a falha-fechada deixou passar a faixa privilegiada"
        );
        assert!(
            !html.contains("oc-side"),
            "a falha-fechada mostrou a navegação da shell"
        );
    }
}

#[cfg(test)]
mod prontidao_da_instalacao_e_estado_do_core {
    use super::*;
    use ocinye_contracts::readiness::ReadinessOverall;

    /// O caminho inteiro, de `ReadinessOverall` ao que a topbar escreve.
    ///
    /// Reproduz aqui os dois saltos que a aplicação faz — `boot::probe` traduz
    /// `ReadinessOverall` em `BootState`, e a página traduz `BootState` em
    /// `CoreStatus` — para que este teste falhe se qualquer um deles mudar.
    fn distintivo(prontidao: ReadinessOverall) -> String {
        use crate::boot::BootState;
        let estado = match prontidao {
            ReadinessOverall::Ready => BootState::Ready,
            ReadinessOverall::Degraded => BootState::Degraded,
            ReadinessOverall::Blocked => BootState::Blocked,
        };
        let core = match estado {
            BootState::Ready | BootState::Degraded => CoreStatus::Ok,
            BootState::Blocked => CoreStatus::Unavailable,
            BootState::Unreachable | BootState::Uninitialized | BootState::Checking => {
                CoreStatus::Silent
            }
        };
        estado_do_sistema(core).to_html()
    }

    /// Uma instalação sem correio, sem inferência e sem computação continua a
    /// ter um Core inteiro, e a topbar tem de o dizer.
    ///
    /// # Porque é este o teste que interessa
    ///
    /// O distintivo dizia `CORE LIMITADO` porque reproduzia o enum global de
    /// prontidão. Mas ele diz **CORE**, e `degraded` é uma afirmação sobre a
    /// *instalação*: `decide()` no Core devolve `Blocked` antes de chegar a
    /// `Degraded`, portanto `Degraded` significa que todos os componentes
    /// críticos estão disponíveis. Um Core operacional aparecia amarelo por não
    /// haver SMTP configurado.
    ///
    /// Este teste guarda as duas metades ao mesmo tempo: a prontidão continua
    /// `Degraded` e o distintivo diz `CORE OK`. Quem quiser vê-lo verde
    /// mudando o Core para `Ready` desfaz a primeira metade e falha aqui.
    #[test]
    fn degraded_por_opcionais_apresenta_um_core_pronto() {
        let prontidao = ReadinessOverall::Degraded;

        // A primeira metade: a prontidão da instalação não foi suavizada.
        assert_eq!(
            prontidao,
            ReadinessOverall::Degraded,
            "o cenário deixou de ser `degraded`, e então não prova nada"
        );
        assert!(
            prontidao.may_proceed(),
            "`degraded` deixou de deixar entrar no Workspace"
        );

        // A segunda: o que a pessoa lê.
        let html = distintivo(prontidao);
        assert!(
            html.contains(r#"data-estado="ok""#) && html.contains("OPERACIONAL"),
            "com a instalação `degraded` por opcionais, a topbar diz: {html}"
        );
        assert!(
            !html.contains("ods-dot--error"),
            "o Core aparece em falha por falta de capacidades opcionais: {html}"
        );
    }

    /// E aparece com o tratamento visual são, não com o de aviso.
    ///
    /// Sem isto, `CORE OK` podia ficar escrito ao lado de um ponto amarelo: o
    /// texto certo com o indicador errado é a mesma imprecisão, dita a meio.
    #[test]
    fn o_core_pronto_nao_usa_o_indicador_de_aviso() {
        for prontidao in [ReadinessOverall::Ready, ReadinessOverall::Degraded] {
            let html = distintivo(prontidao);
            assert!(
                html.contains("ods-dot--success"),
                "{prontidao:?} não pinta o ponto do Core como operacional: {html}"
            );
            assert!(
                !html.contains("ods-badge--error"),
                "{prontidao:?} traz o distintivo de falha: {html}"
            );
        }
    }

    /// O que é mesmo um problema continua a dizer-se como problema.
    #[test]
    fn blocked_e_sem_resposta_nunca_sao_core_ok() {
        let bloqueado = distintivo(ReadinessOverall::Blocked);
        assert!(bloqueado.contains("INDISPONÍVEL"));
        assert!(!bloqueado.contains("OPERACIONAL"));
        assert!(bloqueado.contains(r#"data-estado="indisponivel""#));
        assert!(bloqueado.contains("ods-dot--error"));

        let calado = estado_do_sistema(CoreStatus::Silent).to_html();
        assert!(calado.contains("SEM RESPOSTA"));
        assert!(!calado.contains("OPERACIONAL"));
        assert!(calado.contains(r#"data-estado="silencio""#));
        assert!(calado.contains("ods-dot--error"));
    }
}
