//! A shell autenticada: sidebar, topbar, command palette e menu de criação.
//!
//! Estrutura de `design/README.md` §5. A shell é a mesma em todos os 19 ecrãs
//! autenticados; só o conteúdo muda.

use leptos::prelude::*;
use ocinye_contracts::AvatarChoice;

use crate::ui::components::AvatarSize;
use ocinye_contracts::Permission;

use crate::ui::icon::{icon, Icon};
use crate::ui::initials;

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
    /// Correio institucional.
    Mail,
    /// Mensagens entre membros.
    Messaging,
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
    /// O caminho do ecrã.
    #[must_use]
    pub const fn path(self) -> &'static str {
        match self {
            Self::Home => "/",
            Self::MyWork => "/my-work",
            Self::Mail => "/mail",
            Self::Messaging => "/messages",
            Self::Units => "/units",
            Self::Ideas => "/ideas",
            Self::Projects => "/projects",
            Self::Knowledge => "/knowledge",
            Self::Bibliography => "/bibliography",
            Self::Datasets => "/datasets",
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

    /// O rótulo do ecrã, tal como aparece na navegação e no breadcrumb.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Help => "Ajuda",
            Self::Settings => "Definições",
            Self::Home => "Home",
            Self::MyWork => "O Meu Trabalho",
            Self::Mail => "Correio",
            Self::Messaging => "Mensagens",
            Self::Units => "Unidades",
            Self::Ideas => "Ideias",
            Self::Projects => "Projectos",
            Self::Knowledge => "Conhecimento",
            Self::Bibliography => "Bibliografia",
            Self::Datasets => "Dados",
            Self::Ai => "Ocinye AI",
            Self::Agents => "Agentes",
            Self::Compute => "Computação",
            Self::Calendar => "Calendário",
            Self::Activity => "Actividade",
            Self::Admin => "Administração",
            Self::Audit => "Audit Log",
            Self::Prompt => "Prompt Ocinye",
            Self::Search => "Pesquisar",
            Self::Ask => "Pesquisar, perguntar ou executar",
        }
    }

    const fn icon(self) -> Icon {
        match self {
            Self::Help => Icon::Help,
            Self::Settings => Icon::Settings,
            Self::Home => Icon::Home,
            Self::MyWork => Icon::MyWork,
            Self::Calendar => Icon::Calendar,
            Self::Mail => Icon::Mail,
            Self::Messaging => Icon::Messaging,
            Self::Units => Icon::Units,
            Self::Ideas => Icon::Idea,
            Self::Projects => Icon::Project,
            Self::Knowledge => Icon::Knowledge,
            Self::Bibliography => Icon::Bibliography,
            Self::Datasets => Icon::Data,
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

/// Os cinco grupos da sidebar, na ordem do design.
const GROUPS: [(&str, &[Screen]); 5] = [
    // O Calendário fica em PESSOAL, ao lado do Correio: são os dois sítios onde
    // uma pessoa vê o que a espera. O relógio da barra superior continua a ser a
    // entrada rápida — mas uma entrada que só existe num canto é uma entrada que
    // metade das pessoas não encontra.
    (
        "PESSOAL",
        &[
            Screen::Home,
            Screen::MyWork,
            Screen::Calendar,
            Screen::Messaging,
            Screen::Mail,
        ],
    ),
    (
        "INVESTIGAÇÃO",
        &[Screen::Units, Screen::Ideas, Screen::Projects],
    ),
    (
        "CONHECIMENTO",
        &[Screen::Knowledge, Screen::Bibliography, Screen::Datasets],
    ),
    (
        "INTELIGÊNCIA",
        &[Screen::Ai, Screen::Agents, Screen::Compute],
    ),
    (
        "INSTITUCIONAL",
        &[Screen::Activity, Screen::Admin, Screen::Audit],
    ),
];

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
#[derive(Debug, Clone, PartialEq, Eq)]
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

/// O que a shell precisa de saber sobre quem está a usá-la.
#[derive(Debug, Clone)]
pub struct Viewer {
    /// A zona em que este membro está a olhar para o sistema.
    ///
    /// Vem do browser. Decide em que dia civil as coisas caem — e é por isso
    /// que está aqui, no que a casca inteira já recebe, em vez de ser passada a
    /// cada ecrã que dela precise.
    pub zona: ocinye_contracts::temporal::TimeZoneName,
    /// Nome do membro.
    pub name: String,
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
    /// Permissões de âmbito institucional, tal como o Core as calculou.
    ///
    /// Vêm de `GET /api/v1/me`. A shell usa-as para não mostrar o que o membro
    /// não pode usar (briefing §65, §67).
    ///
    /// **Não são autorização.** Esconder um item é cortesia; quem escrever o
    /// caminho à mão continua a bater na recusa do Core, que é onde a decisão
    /// vive (`CLAUDE.md` §4).
    pub capabilities: Vec<String>,
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
const fn screen_permission(screen: Screen) -> Option<Permission> {
    match screen {
        // Definições são do próprio membro: não exigem permissão
        // institucional nenhuma, e cada pessoa vê apenas a sua conta.
        // Ajuda e Definições são do próprio membro: sem permissão institucional.
        Screen::Home | Screen::MyWork | Screen::Settings | Screen::Help => None,
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
        Screen::Ai => Some(Permission::AiUse),
        Screen::Agents => Some(Permission::AgentsView),
        Screen::Compute => Some(Permission::ComputeView),
        Screen::Activity => Some(Permission::OrganisationView),
        Screen::Admin => Some(Permission::MembersView),
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

/// A shell completa.
pub fn shell(
    viewer: &Viewer,
    active: Screen,
    trail: Vec<Crumb>,
    current: &str,
    content: impl IntoView + 'static,
) -> impl IntoView {
    let avatar = initials(&viewer.name);
    let core_status = viewer.core_status.clone();
    let can_create = viewer.can(Permission::IdeasCreate)
        || viewer.can(Permission::NotesCreate)
        || viewer.can(Permission::DatasetsCreate)
        || viewer.can(Permission::AgentsCreatePersonal);

    view! {
        <a class="oc-skip" href="#conteudo">"Saltar para o conteúdo"</a>

        <div class="oc-shell" data-side="expanded">
            {sidebar(viewer, &avatar, active)}

            <div class="oc-main">
                {topbar(viewer, current, trail, &core_status, can_create)}
                <main class="oc-content" id="conteudo">
                    {content}
                </main>
            </div>
        </div>

        {palette(viewer)}
    }
}

fn sidebar(viewer: &Viewer, avatar: &str, active: Screen) -> impl IntoView {
    // O dossier põe no rodapé o estado do sistema, e não o nome da organização
    // (`design/README.md` §5.2). Reflecte a mesma sonda ao Core que a pílula da
    // topbar: dizer «SISTEMA OK» com o Core em baixo seria pintar o estado
    // bonito em vez do estado.
    //
    // Tem linha própria, fora do cartão do membro: quem está OK é o Core.
    let core_status = viewer.core_status.clone();
    let avatar = avatar.to_owned();

    view! {
        <aside class="oc-side">
            <div class="oc-side__head">
                <span class="oc-side__tile">
                    <img src="/static/ocinye_logo.png" alt="" />
                </span>
                <span class="oc-side__names">
                    <span class="oc-side__title">"OCINYE OS"</span>
                    <span class="oc-side__sub">"WORKSPACE"</span>
                </span>
                <button
                    type="button"
                    class="oc-side__collapse"
                    data-oc="collapse"
                    aria-expanded="true"
                    aria-label="Colapsar navegação"
                    title="Colapsar navegação"
                >
                    {icon(Icon::SidebarCollapse, 14)}
                </button>
            </div>

            <nav class="oc-side__nav" aria-label="Navegação principal">
                // A navegação mostra a instituição inteira.
                //
                // «Não tem acesso» e «não sabemos o que tem» são coisas
                // diferentes, e a barra trata-as em separado:
                //
                // - **Sabemos, e não tem.** O item aparece, esbatido, sem
                //   destino, e diz porquê. Esconder fazia a barra mudar de
                //   forma consoante quem olha, e quem não via um ecrã não
                //   ficava a saber que ele existe nem o que lhe falta para lá
                //   chegar.
                // - **Não sabemos.** Sem resposta do Core não há permissões
                //   confirmadas, e mostrar tudo seria afirmar um acesso que não
                //   se conseguiu verificar (`CLAUDE.md` §31). Aí a barra encolhe
                //   ao que não exige permissão nenhuma, e a topbar já diz
                //   «CORE OFF».
                {GROUPS
                    .iter()
                    .filter_map(|(group, screens)| {
                        let itens: Vec<(Screen, bool)> = screens
                            .iter()
                            .copied()
                            .filter_map(|screen| match screen_permission(screen) {
                                None => Some((screen, true)),
                                Some(_) if !core_status.operational() => None,
                                Some(p) => Some((screen, viewer.can(p))),
                            })
                            .collect();

                        // Um grupo sem itens nenhuns desaparece com eles: um
                        // cabeçalho «INSTITUCIONAL» sozinho não diria nada.
                        if itens.is_empty() {
                            return None;
                        }

                        Some(view! {
                            <div class="oc-side__group">{*group}</div>
                            {itens
                                .into_iter()
                                .map(|(screen, permitido)| {
                                    let on = screen == active;
                                    if permitido {
                                        view! {
                                            <a
                                                class="oc-nav"
                                                href=screen.path()
                                                title=screen.label()
                                                aria-label=screen.label()
                                                aria-current=on.then_some("page")
                                            >
                                                {icon(screen.icon(), 15)}
                                                <span>{screen.label()}</span>
                                            </a>
                                        }
                                            .into_any()
                                    } else {
                                        view! {
                                            <span
                                                class="oc-nav oc-nav--unavailable"
                                                aria-disabled="true"
                                                aria-label=format!(
                                                    "{} — não tem autorização para este ecrã.",
                                                    screen.label(),
                                                )
                                                title=format!(
                                                    "{} — não tem autorização para este ecrã.",
                                                    screen.label(),
                                                )
                                            >
                                                {icon(screen.icon(), 15)}
                                                <span>{screen.label()}</span>
                                            </span>
                                        }
                                            .into_any()
                                    }
                                })
                                .collect_view()}
                        })
                    })
                    .collect_view()}
            </nav>

            <div class="oc-side__foot">
                // O estado da plataforma vive na topbar, e só lá.
                //
                // Esteve aqui, em linha própria, depois de sair de dentro do
                // cartão do membro — onde parecia um atributo da pessoa. Mas
                // `CORE OK` na topbar diz exactamente a mesma coisa, e dizê-la
                // duas vezes no mesmo ecrã não a torna mais verdadeira.
                //
                // A hierarquia fica limpa: a topbar responde «como está o
                // sistema», e o rodapé «quem sou eu aqui».
                <a class="oc-side__foot-item" href="/settings" title="Definições" aria-label="Definições">
                    {icon(Icon::Settings, 15)}
                    <span class="oc-side__foot-label">"Definições"</span>
                </a>
                <a class="oc-side__foot-item" href="/help" title="Ajuda" aria-label="Ajuda">
                    {icon(Icon::Help, 15)}
                    <span class="oc-side__foot-label">"Ajuda"</span>
                </a>

                {account(viewer, &avatar)}
            </div>
        </aside>
    }
}

/// O controlo de conta do rodapé, e a superfície que ele abre.
///
/// # O que este menu é, e o que não pode tornar-se
///
/// > **The profile popover is a personal session surface, never an
/// > authorization surface.**
///
/// Dá atalhos para capacidades que o membro já tem — a sua conta, as suas
/// credenciais, a sua sessão — e nunca concede autoridade institucional. Não
/// há aqui papéis, permissões, concessões, administração nem troca de
/// organização: nada disso é pessoal, e um menu pessoal que os mostrasse
/// começaria a parecer o sítio onde se pedem.
///
/// # Divulgação, não menu
///
/// A semântica é a de um *disclosure*: um botão que revela uma região, e não
/// `role="menu"`. Um menu ARIA obriga a navegação por setas e faz o `Tab` sair
/// da superfície inteira; aqui as acções são ligações e um formulário normais,
/// e quem usa `Tab` espera percorrê-las. Prometer semântica de menu e entregar
/// ligações seria anunciar um teclado que não existe.
///
/// # Sem JavaScript
///
/// Sem `app.js` a região fica fechada, e com ela o botão de terminar sessão.
/// Não é um beco: `Definições → Segurança` lista as sessões do membro e
/// termina a actual com um formulário `POST` sem uma linha de script. A saída
/// existe nos dois mundos.
fn account(viewer: &Viewer, avatar: &str) -> impl IntoView {
    let name = viewer.name.clone();
    let avatar = avatar.to_owned();
    let email = viewer.email.clone();
    let trigger_title = format!("{name} — conta e sessão");

    // A linha secundária do botão fechado é a identidade, não o sistema. É o
    // endereço, que é a identidade inteira desde o ADR-0106.
    let subtitulo = email.clone();

    view! {
        <div class="oc-account" data-oc="account">
            <button
                type="button"
                class="oc-profile"
                data-oc="account-toggle"
                aria-expanded="false"
                aria-controls="oc-account-menu"
                title=trigger_title.clone()
                aria-label=trigger_title
            >
                {crate::ui::components::avatar(&viewer.avatar, &avatar, AvatarSize::Small)}
                <span class="oc-profile__text">
                    <span class="oc-profile__name">{name.clone()}</span>
                    {subtitulo
                        .clone()
                        .map(|linha| view! { <span class="oc-profile__sub">{linha}</span> })}
                </span>
                {icon(Icon::ChevronUp, 12)}
            </button>

            <div
                class="oc-account__menu"
                id="oc-account-menu"
                data-oc="account-menu"
                hidden
                aria-label="Conta e sessão"
            >
                <div class="oc-account__id">
                    {crate::ui::components::avatar(&viewer.avatar, &avatar, AvatarSize::Medium)}
                    <span class="oc-account__id-text">
                        <b>{name}</b>
                        {email.map(|e| view! {
                            <span class="oc-account__handle">{e}</span>
                        })}
                    </span>
                </div>

                <div class="oc-account__group">
                    <a class="oc-account__item" href="/settings">
                        {icon(Icon::User, 14)}
                        <span>
                            <b>"A minha conta"</b>
                            <em>"Dados da conta e identidade institucional"</em>
                        </span>
                    </a>
                    <a class="oc-account__item" href="/settings/security">
                        {icon(Icon::Shield, 14)}
                        <span>
                            <b>"Segurança"</b>
                            <em>"Palavra-passe e sessões"</em>
                        </span>
                    </a>
                </div>

                // O resumo da sessão é uma linha, não um painel. O detalhe
                // completo — todas as sessões, cada uma com a sua revogação —
                // vive em Definições, e duplicá-lo aqui faria do menu um
                // segundo Definições pior do que o primeiro.
                <div class="oc-account__session">
                    <span class="oc-account__session-label">"Sessão actual"</span>
                    <span class="oc-account__session-value">{sessao_actual(viewer)}</span>
                </div>

                // Terminar sessão fecha o menu e a hierarquia: identidade,
                // conta, segurança, sessão, saída. Separado por um divisor
                // porque é a única acção daqui que destrói alguma coisa.
                <form class="oc-account__out" method="post" action="/logout">
                    <button type="submit" class="oc-account__item oc-account__item--out">
                        {icon(Icon::Power, 14)}
                        <span><b>"Terminar sessão"</b></span>
                    </button>
                </form>
            </div>
        </div>
    }
}

/// O estado da sessão do Workspace, em palavras.
///
/// Só se afirma o que se sabe. O `Instant` guardado sabe dizer quanto falta,
/// e não sabe dizer a que horas foi emitida nem de onde: não há aqui data de
/// emissão, dispositivo nem lugar, porque nada disso está guardado — e o
/// `user-agent`, que estaria, é um indício de sessão e não um dispositivo
/// verificado.
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

fn topbar(
    viewer: &Viewer,
    current: &str,
    trail: Vec<Crumb>,
    core_status: &CoreStatus,
    can_create: bool,
) -> impl IntoView {
    // O dia de hoje onde a pessoa está, e não em Greenwich.
    let hoje = crate::ui::tempo::hoje_civil(chrono::Utc::now(), viewer.zona);
    // O último degrau é a página, e não o ecrã a que ela pertence.
    //
    // Era `active.label()`, e em todos os ecrãs com trilho isso repetia o
    // degrau anterior: `/units/{id}` lia-se «Unidades / Unidades», e
    // `/bibliography/new` lia-se «Bibliografia / Bibliografia». Um trilho que
    // repete o degrau não diz onde se está — diz duas vezes onde se entrou.
    //
    // O nome da página já viajava até aqui como título do documento, e era
    // deitado fora à porta.
    let current = current.to_owned();
    let organisation = viewer.organisation.to_uppercase();
    let has_trail = !trail.is_empty();

    view! {
        <header class="oc-top">
            <nav class="oc-crumb" aria-label="Trilho">
                // A instituição, e não a palavra «OCINYE» escrita no código. O
                // dossier mostra-a assim porque a instituição é a Ocinye; o
                // trilho deve dizer qual é, não presumir qual será.
                {organisation}
                <i aria-hidden="true">"/"</i>
                {if has_trail {
                    view! {
                        {trail
                            .into_iter()
                            .map(|crumb| {
                                view! {
                                    <a href=crumb.href>{crumb.label}</a>
                                    <i aria-hidden="true">"/"</i>
                                }
                            })
                            .collect_view()}
                        <b>{current.clone()}</b>
                    }
                        .into_any()
                } else {
                    view! { <b>{current}</b> }.into_any()
                }}
            </nav>

            // Uma ligação, e não um botão que abre a command palette. A palette
            // filtra **navegação** localmente; não procura em nada. Prometer
            // «Pesquisar no Ocinye» e abrir um filtro de menus era uma promessa
            // por cumprir sobre um endpoint que já existia (briefing §32).
            //
            // O `⌘K` continua a abrir a palette: são duas coisas distintas, e
            // agora cada uma faz o que anuncia.
            // A Universal Command Surface. Uma barra, três intenções:
            // pesquisar, perguntar, executar. É um formulário e não uma
            // ligação, porque perguntar e executar submetem — e continua a
            // funcionar sem JavaScript e sem nenhum nó de IA, porque pesquisar
            // é determinístico (briefing §29, §32).
            <form class="oc-search" method="get" action="/ask" role="search">
                {icon(Icon::Search, 14)}
                <label class="oc-sr" for="oc-command">
                    "Pesquisar, perguntar ou executar no Ocinye"
                </label>
                <input
                    class="oc-search__input"
                    id="oc-command"
                    name="q"
                    type="search"
                    placeholder="Pesquisar, perguntar ou executar no Ocinye…"
                    autocomplete="off"
                />
                <kbd class="oc-kbd" data-oc="palette-open" title="Command palette">"⌘K"</kbd>
            </form>

            <div class="oc-spacer"></div>

            // O «+ Criar» aparece sempre. Sem nenhuma das permissões que
            // abre, fica visível e declarado em vez de desaparecer: uma
            // interface que muda de forma consoante quem olha esconde a própria
            // existência da acção, e quem não a vê não fica a saber porquê.
            {if can_create {
                create_menu(viewer).into_any()
            } else {
                view! {
                    <span
                        class="oc-btn oc-btn--gold oc-unavailable"
                        aria-disabled="true"
                        title="Não pertence a nenhuma unidade, e é a filiação que dá acesso a criar."
                    >
                        {icon(Icon::Plus, 13)}
                        "Criar"
                    </span>
                }
                .into_any()
            }}

            <span class="oc-divider" aria-hidden="true"></span>

            // O sino do dossier (§5.3).
            //
            // Foi retirado numa auditoria anterior por ser um botão sem handler
            // com um ponto de «não lidas» que nada alimentava. Voltou quando
            // passou a haver o que contar: o Core tem notificações, o worker
            // entrega-as, e o ponto só se pinta quando há por ler.
            {notifications(viewer.unread)}


            {core_status_pill(core_status)}

            // O relógio, no lugar onde estava o avatar.
            //
            // O avatar ali era repetição: a identidade do membro está no rodapé
            // da barra lateral, com o nome e o menu de conta, e a topbar
            // mostrava a mesma pessoa outra vez sem acrescentar nada.
            //
            // A hierarquia fica assim:
            //
            //   topbar          → operação, estado do sistema, tempo
            //   rodapé da barra → conta, identidade, sessão
            //
            // # A hora é do computador de quem está a ver
            //
            // Não vem do Core, não precisa de API, não é persistida, e **nunca
            // decide nada**: carimbos de auditoria, expiração de sessões e
            // prazos continuam a vir do Core e da base de dados. A hora do
            // browser é escolhida por quem o usa, e usá-la para autorização
            // seria deixar decidir quem mexe no relógio.
            //
            // Chega vazio porque o servidor não sabe em que fuso está quem lê.
            // Escrever ali uma hora seria escrever a hora do *servidor* com o
            // aspecto da hora de quem está a ver.
            // O relógio deixa de ser decoração e passa a ser a entrada para o
            // Centro Temporal.
            //
            // Um `button` a sério, e não um `div` com um clique: assim tem foco,
            // responde ao teclado, e um leitor de ecrã sabe dizer o que é e se
            // está aberto. `hidden` até o JS lhe escrever a hora — mostrar um
            // relógio vazio seria mostrar uma hora que não sabemos.
            <button
                type="button"
                class="oc-clock"
                data-oc="clock"
                aria-expanded="false"
                aria-controls="oc-temporal-centre"
                aria-label="Centro Temporal"
                hidden
            >
                <b></b>
                <span></span>
            </button>

            {crate::ui::screens::calendar::system_calendar(hoje)}
        </header>
    }
}

/// O sino de notificações (`design/README.md` §5.3, item 6).
///
/// A forma é a do dossier: 29×29, ícone de 16px, e o ponto dourado de 6px com
/// anel branco no canto. O CSS do ponto já existia — foi escrito para este sino
/// e ficou sem uso quando ele saiu numa auditoria anterior.
///
/// O que o ponto significa é «tem coisas por ler», e isso é **dado**, não
/// decoração. O número vem do Core, e o ponto pinta-se apenas quando há o que
/// contar. É a mesma regra que faz os contadores da Home mostrarem `0` em vez
/// dos `86` do protótipo: o desenho traz um exemplo com dados, e nós mostramos
/// o que existe.
fn notifications(unread: usize) -> impl IntoView {
    let title = if unread == 0 {
        "Nada por ler".to_owned()
    } else {
        format!("{unread} por ler")
    };

    view! {
        <div class="oc-sino">
            // Abre um painel, e não uma página.
            //
            // Ver o que chegou é um relance, e não uma navegação: levar a
            // pessoa a outro ecrã fá-la perder o sítio onde estava para depois
            // ter de voltar.
            //
            // A página continua a existir, e é para onde o rodapé leva: um
            // painel mostra o que é recente, e um histórico é outra coisa.
            <button
                type="button"
                class="oc-icon-btn"
                data-oc="abrir-notificacoes"
                aria-haspopup="dialog"
                aria-expanded="false"
                aria-controls="oc-notificacoes"
                title=title.clone()
                aria-label=title
            >
                {icon(Icon::Bell, 16)}
                {(unread > 0).then(|| view! { <i aria-hidden="true"></i> })}
            </button>

            <div
                class="oc-pop oc-sino__painel"
                id="oc-notificacoes"
                data-oc="notificacoes"
                role="dialog"
                aria-label="Notificações"
                hidden
            >
                <header class="oc-pop__head">
                    <span class="oc-pop__title">"Notificações"</span>
                    <span class="oc-pop__meta" data-oc="notificacoes-contagem">
                        {if unread == 0 {
                            "tudo lido".to_owned()
                        } else {
                            format!("{unread} por ler")
                        }}
                    </span>
                </header>

                // O conteúdo chega quando o painel abre. Renderizá-lo em cada
                // página seria pedir ao Core a lista inteira a cada navegação,
                // para a esconder quase sempre.
                <div class="oc-sino__lista" data-oc="notificacoes-lista">
                    <p class="oc-pop__empty">"A carregar…"</p>
                </div>

                <div class="oc-pop__foot">
                    // Ícone e legenda, como as linhas do painel da conta: um
                    // rodapé com uma frase solta lê-se como um resto.
                    <a class="oc-pop__item" href="/notifications">
                        {icon(Icon::ArrowRight, 14)}
                        <span>
                            <b>"Ver todas"</b>
                            <em>"O histórico completo de avisos"</em>
                        </span>
                    </a>
                </div>
            </div>
        </div>
    }
}

/// O indicador de estado do Core.
///
/// Reflecte uma sonda real ao Core. Quando não responde, diz-o — em vez de
/// mostrar `CORE OK` porque é o estado bonito.
fn core_status_pill(estado: &CoreStatus) -> impl IntoView {
    let (rotulo, titulo, modificador) = match estado {
        CoreStatus::Ok => ("CORE OK", "O Ocinye Core está pronto", ""),
        CoreStatus::Unavailable => (
            "CORE INDISPONÍVEL",
            "O Ocinye Core respondeu que não está em condições de operar",
            " oc-core-pill--off",
        ),
        CoreStatus::Silent => (
            "CORE SEM RESPOSTA",
            "Não houve resposta do Ocinye Core",
            " oc-core-pill--off",
        ),
    };
    view! {
        <span class=format!("oc-core-pill{modificador}") title=titulo>
            <i aria-hidden="true"></i>
            <span>{rotulo}</span>
        </span>
    }
    .into_any()
}

/// As acções do menu `+ Criar`, com os atalhos do design.
///
/// O destino é `None` quando o ecrã de criação ainda não existe. O dossier
/// especifica as sete acções mas apenas dois dos ecrãs; as restantes ficam
/// visíveis e declaradas como indisponíveis, em vez de levarem a um 404.
const CREATE_ITEMS: [(&str, Option<&str>, &str, Permission); 7] = [
    (
        "Nova Ideia",
        Some("/ideas/new"),
        "I",
        Permission::IdeasCreate,
    ),
    ("Novo Projecto", None, "P", Permission::ProjectsCreate),
    ("Nova Nota", None, "N", Permission::NotesCreate),
    ("Nova Referência", None, "R", Permission::BibliographyCreate),
    ("Novo Dataset", None, "D", Permission::DatasetsCreate),
    ("Nova Tarefa", None, "T", Permission::IdeasEdit),
    (
        "Novo Agente IA",
        Some("/ai/agents/new"),
        "A",
        Permission::AgentsCreatePersonal,
    ),
];

fn create_menu(viewer: &Viewer) -> impl IntoView {
    // Todas as acções, com a marca de quais o membro pode executar. Filtrar
    // as outras deixava o menu a mudar de tamanho consoante quem o abre, e sem
    // dizer o que falta para as ter.
    let items: Vec<(&str, Option<&str>, &str, bool)> = CREATE_ITEMS
        .iter()
        .map(|(label, href, key, permission)| (*label, *href, *key, viewer.can(*permission)))
        .collect();

    view! {
        <div class="oc-create" data-oc="create">
            <button
                type="button"
                class="oc-btn oc-btn--gold"
                data-oc="create-toggle"
                aria-haspopup="menu"
                aria-expanded="false"
            >
                {icon(Icon::Plus, 12)}
                "Criar"
            </button>

            <div class="oc-create__menu" data-oc="create-menu" role="menu" hidden>
                {items
                    .into_iter()
                    .map(|(label, href, key, permitido)| {
                        // A razão é a de cada acção: o ecrã pode não existir, ou
                        // pode ser o acesso que falta. São coisas diferentes e a
                        // interface tem de as distinguir.
                        let href = if permitido { href } else { None };
                        let razao = if permitido {
                            "Ainda não disponível"
                        } else {
                            "Não tem autorização para esta acção."
                        };
                        href.map_or_else(
                            || {
                                view! {
                                    <span
                                        class="oc-create__item oc-unavailable"
                                        role="menuitem"
                                        aria-disabled="true"
                                        title=razao
                                    >
                                        {label}
                                        <kbd class="oc-kbd">{key}</kbd>
                                    </span>
                                }
                                    .into_any()
                            },
                            |href| {
                                view! {
                                    <a class="oc-create__item" role="menuitem" href=href>
                                        {label}
                                        <kbd class="oc-kbd">{key}</kbd>
                                    </a>
                                }
                                    .into_any()
                            },
                        )
                    })
                    .collect_view()}
            </div>
        </div>
    }
}

/// Todos os ecrãs, incluindo os que não estão na navegação lateral.
///
/// `PALETTE_NAV` é a lista dos dezassete destinos institucionais. Esta é maior:
/// junta-lhe os ecrãs do próprio membro, que existem no rodapé e não na
/// navegação, mas que continuam a precisar de estado activo e de posse de
/// rotas como qualquer outro.
#[cfg_attr(
    not(test),
    allow(dead_code, reason = "lida pela auditoria de estado activo")
)]
const SCREENS: [Screen; 21] = [
    Screen::Home,
    Screen::MyWork,
    Screen::Calendar,
    Screen::Messaging,
    Screen::Mail,
    Screen::Units,
    Screen::Ideas,
    Screen::Projects,
    Screen::Knowledge,
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
const PALETTE_NAV: [Screen; 17] = [
    Screen::Home,
    Screen::MyWork,
    Screen::Calendar,
    Screen::Messaging,
    Screen::Mail,
    Screen::Units,
    Screen::Ideas,
    Screen::Projects,
    Screen::Knowledge,
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

/// A command palette.
///
/// O filtro de texto é local, mas **o que entra na página não é**: a palette é
/// renderizada apenas com os ecrãs e acções que o membro pode alcançar. Mandar
/// todos e esconder alguns no browser seria enviar ao cliente informação
/// que ele não devia ter (briefing §65).
fn palette(viewer: &Viewer) -> impl IntoView {
    let screens: Vec<Screen> = PALETTE_NAV
        .iter()
        .copied()
        .filter(|screen| screen_permission(*screen).is_none_or(|p| viewer.can(p)))
        .collect();

    let actions: Vec<(&str, &str, &str)> = PALETTE_ACTIONS
        .iter()
        .filter(|(_, _, _, permission)| viewer.can(*permission))
        .map(|(label, href, shortcut, _)| (*label, *href, *shortcut))
        .collect();

    view! {
        <div
            class="oc-palette"
            data-oc="palette"
            role="dialog"
            aria-modal="true"
            aria-label="Pesquisar ou executar um comando"
            hidden
        >
            <div class="oc-palette__panel">
                <div class="oc-palette__field">
                    {icon(Icon::Search, 16)}
                    <label class="oc-sr" for="palette-input">
                        "Pesquisar ou executar um comando"
                    </label>
                    <input
                        id="palette-input"
                        type="text"
                        data-oc="palette-input"
                        autocomplete="off"
                        placeholder="Pesquisar ou executar um comando…"
                    />
                    <kbd class="oc-kbd">"ESC"</kbd>
                </div>

                <div class="oc-palette__list">
                    <div data-oc="palette-group">
                        <div class="oc-palette__group">"NAVEGAR"</div>
                        {screens
                            .into_iter()
                            .map(|screen| {
                                view! {
                                    <a
                                        class="oc-palette__item"
                                        href=screen.path()
                                        data-label=screen.label()
                                    >
                                        <i aria-hidden="true"></i>
                                        {screen.label()}
                                    </a>
                                }
                            })
                            .collect_view()}
                    </div>

                    <div data-oc="palette-group">
                        <div class="oc-palette__group">"ACÇÕES"</div>
                        {actions
                            .into_iter()
                            .map(|(label, href, shortcut)| {
                                view! {
                                    <a
                                        class="oc-palette__item oc-palette__item--action"
                                        href=href
                                        data-label=label
                                        // O atalho vai no atributo, e não só no
                                        // `<kbd>`: é daqui que o teclado o lê.
                                        // Ler do texto visível prenderia o
                                        // comportamento à forma de o escrever.
                                        data-shortcut=shortcut
                                    >
                                        <i aria-hidden="true"></i>
                                        {label}
                                        {(!shortcut.is_empty())
                                            .then(|| view! { <kbd class="oc-kbd">{shortcut}</kbd> })}
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Um membro com exactamente as permissões indicadas.
    fn viewer_with(permissions: &[Permission]) -> Viewer {
        Viewer {
            zona: "UTC".to_owned().try_into().expect("fuso conhecido"),
            avatar: ocinye_contracts::AvatarChoice::Initials,
            email: Some("jmanuel@ocinye.com".to_owned()),
            session_expires_in: Some(std::time::Duration::from_secs(8 * 3600)),
            name: "João Manuel".to_owned(),
            organisation: "Ocinye".to_owned(),
            core_status: crate::ui::shell::CoreStatus::Ok,
            temporal: Vec::new(),
            temporal_failure: None,
            unread: 0,
            capabilities: permissions.iter().map(|p| p.as_str().to_owned()).collect(),
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
        // A superfície é o último elemento do rodapé, e o rodapé fecha a barra:
        // até `</aside>` é tudo menu, e nada de menu fica de fora.
        let fim = html[inicio..]
            .find("</aside>")
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

    #[test]
    fn a_navegacao_mostra_a_instituicao_inteira_e_declara_o_que_nao_se_pode_abrir() {
        // A barra deixou de encolher consoante quem olha: mostra os ecrãs que a
        // instituição tem, e marca os que esta pessoa não pode abrir. Quem não
        // via um ecrã não ficava a saber que ele existe.
        let member = viewer_with(&[Permission::IdeasView]);
        let html = render(&member);

        for grupo in ["PESSOAL", "INVESTIGAÇÃO", "CONHECIMENTO", "INSTITUCIONAL"] {
            assert!(html.contains(grupo), "o grupo {grupo} desapareceu da barra");
        }

        // O que tem, navega.
        assert!(html.contains(r#"href="/ideas""#));
        // O que não tem, aparece sem destino e declarado.
        assert!(
            !html.contains(r#"href="/audit""#),
            "Audit Log navega sem a permissão que exige"
        );
        assert!(html.contains("Audit Log"), "Audit Log desapareceu da barra");
        assert!(html.contains("oc-nav--unavailable"));
    }

    #[test]
    fn sem_resposta_do_core_a_navegacao_encolhe_em_vez_de_afirmar_acesso() {
        // «Não tem acesso» e «não sabemos» são coisas diferentes. Sem o Core não
        // há permissões confirmadas, e mostrar a instituição inteira afirmaria
        // um acesso que não se conseguiu verificar (`CLAUDE.md` §31).
        let mut sem_core = viewer_with(&[]);
        sem_core.core_status = CoreStatus::Silent;
        let html = render(&sem_core);

        assert!(
            html.contains("PESSOAL"),
            "Home e O Meu Trabalho não dependem do Core"
        );
        for grupo in ["INVESTIGAÇÃO", "CONHECIMENTO", "INSTITUCIONAL"] {
            assert!(
                !html.contains(grupo),
                "{grupo} aparece sem que as permissões tenham sido confirmadas"
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

    #[test]
    fn o_menu_criar_desaparece_quando_nao_ha_nada_a_criar() {
        let html = render(&viewer_with(&[Permission::IdeasView]));
        assert!(!html.contains("data-oc=\"create-toggle\""));
    }

    #[test]
    fn o_menu_criar_mostra_tudo_e_so_deixa_seguir_o_permitido() {
        // As acções deixaram de ser filtradas em silêncio: o menu passou a
        // mostrar todas e a declarar as que a pessoa não pode usar. O que se
        // mantém é o que importa — só as permitidas navegam.
        let member = viewer_with(&[Permission::IdeasCreate, Permission::NotesCreate]);
        let html = render(&member);

        assert!(html.contains("data-oc=\"create-toggle\""));
        for label in ["Nova Ideia", "Nova Nota", "Novo Dataset", "Novo Agente IA"] {
            assert!(html.contains(label), "«{label}» desapareceu do menu");
        }

        // As permitidas são ligações; as outras não têm para onde ir.
        assert!(
            html.contains(r#"href="/ideas/new""#),
            "«Nova Ideia» não navega apesar da permissão"
        );
        assert!(
            !html.contains(r#"href="/datasets/new""#),
            "«Novo Dataset» navega sem a permissão que exige"
        );
        assert!(
            !html.contains(r#"href="/ai/agents/new""#),
            "«Novo Agente IA» navega sem a permissão que exige"
        );
        assert!(html.contains("Não tem autorização para esta acção."));
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

    #[test]
    fn a_sidebar_cobre_todos_os_ecras_de_navegacao() {
        let in_groups: usize = GROUPS.iter().map(|(_, screens)| screens.len()).sum();
        assert_eq!(in_groups, PALETTE_NAV.len());
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
            "«A minha conta» não leva a Definições"
        );
        assert!(
            menu.contains(r#"href="/settings/security""#),
            "«Segurança» não leva a Definições / Segurança"
        );
        assert!(menu.contains("A minha conta"));
        assert!(menu.contains("Segurança"));
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
        assert!(
            !menu.contains(r#"type="button""#),
            "o menu de conta tem um botão que não submete nada"
        );
    }

    /// A sessão actual diz o que se sabe, e só isso.
    ///
    /// Sem dispositivo, sem lugar e sem data de emissão: nada disso está
    /// guardado. O `user-agent`, que estaria, é um indício de sessão e não um
    /// dispositivo verificado — chamar-lhe dispositivo seria dar-lhe uma
    /// confiança que ele não tem.
    #[test]
    fn a_sessao_actual_nao_inventa_dispositivo_nem_lugar() {
        let html = render(&viewer_with(&[]));
        let menu = super::tests::menu(&html);

        assert!(
            menu.contains("Sessão actual"),
            "o resumo da sessão desapareceu"
        );
        assert!(
            menu.contains("expira em 8h"),
            "o resumo não usa a expiração real da sessão: {menu}"
        );

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
        let html = render(&viewer_with(&ocinye_contracts::Permission::all()));
        let barra = html
            .split(r#"class="oc-side""#)
            .nth(1)
            .and_then(|rest| rest.split("</aside>").next())
            .expect("barra lateral desapareceu");

        // Só os controlos cujo texto a container query esconde. O menu de
        // conta não entra: o seu conteúdo não colapsa — abre por cima da barra,
        // com a largura toda — e exigir-lhe `aria-label` duplicaria em atributo
        // o texto que já se lê.
        let mut sem_nome: Vec<String> = Vec::new();
        for tag in barra.split('<').skip(1) {
            let inicio = tag.split('>').next().unwrap_or_default();
            let colapsa = inicio.contains(r#"class="oc-nav""#)
                || inicio.contains(r#"class="oc-nav oc-nav--unavailable""#)
                || inicio.contains(r#"class="oc-side__foot-item""#)
                || inicio.contains(r#"class="oc-profile""#);
            if !colapsa {
                continue;
            }
            if !inicio.contains("aria-label=") {
                sem_nome.push(format!("<{inicio}>"));
            }
        }

        assert!(
            sem_nome.is_empty(),
            "controlos da barra lateral sem nome acessível quando o texto colapsa:\n  {}",
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
    #[test]
    fn cada_ecra_marca_um_e_um_so_item_activo() {
        let viewer = viewer_with(&ocinye_contracts::Permission::all());

        for screen in PALETTE_NAV {
            let html = shell(
                &viewer,
                screen,
                Vec::new(),
                screen.label(),
                view! { <p>"x"</p> },
            )
            .to_html();
            let marcados = html.matches(r#"aria-current="page""#).count();
            assert_eq!(
                marcados,
                1,
                "{} marcou {marcados} itens activos, e devia marcar um",
                screen.label(),
            );

            let fim = html
                .find(r#"aria-current="page""#)
                .expect("nenhum item marcado");
            let tag = &html[html[..fim].rfind('<').expect("etiqueta mal formada")..fim];
            assert!(
                tag.contains(&format!(r#"href="{}""#, screen.path())),
                "{} marcou o item errado: {tag}",
                screen.label(),
            );
        }
    }

    /// Um ecrã de detalhe marca o ecrã-pai, e não deixa a barra em branco.
    ///
    /// É o caso que a igualdade literal falha e que este passo existe para
    /// cobrir: `/units/{id}` não é `/units`, mas é ali que se está.
    #[test]
    fn um_ecra_filho_marca_o_pai_na_navegacao() {
        let viewer = viewer_with(&ocinye_contracts::Permission::all());

        for caminho in [
            "/units/33333333-3333-3333-3333-333333333333",
            "/projects/new",
            "/ideas/new",
            "/bibliography/new",
        ] {
            let dono = Screen::owning(caminho).expect("caminho sem dono");
            let html = shell(
                &viewer,
                dono,
                Vec::new(),
                dono.label(),
                view! { <p>"x"</p> },
            )
            .to_html();
            assert_eq!(
                html.matches(r#"aria-current="page""#).count(),
                1,
                "{caminho} não marcou exactamente um item"
            );
            let fim = html
                .find(r#"aria-current="page""#)
                .expect("nenhum item marcado");
            let tag = &html[html[..fim].rfind('<').expect("etiqueta mal formada")..fim];
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
        let viewer = viewer_with(&ocinye_contracts::Permission::all());

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
                .split(r#"class="oc-crumb""#)
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
                nav.contains("<b>Detalhe</b>"),
                "a página não fecha o trilho com o seu próprio nome: {nav}"
            );
            // E fecha-o em texto. Um degrau final que fosse ligação apontaria
            // para a página onde já se está.
            let antes_do_fim = nav.split("<b>").next().unwrap_or_default();
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

        let topbar = html
            .split(r#"class="oc-topbar""#)
            .nth(1)
            .and_then(|resto| resto.split("</header>").next())
            .or_else(|| html.split(r#"class="oc-main""#).nth(1))
            .unwrap_or(&html);
        let topbar = topbar
            .split(r#"class="oc-content""#)
            .next()
            .unwrap_or(topbar);

        assert!(
            !topbar.contains("oc-avatar"),
            "a identidade voltou à topbar, onde já estava no rodapé"
        );
        assert!(
            topbar.contains(r#"data-oc="clock""#),
            "o relógio não está na topbar"
        );

        // E o estado do sistema é dito uma vez: na topbar.
        assert_eq!(
            html.matches("CORE OK").count(),
            1,
            "o estado do sistema é dito mais do que uma vez"
        );
        assert!(
            !html.contains("SISTEMA OK"),
            "o estado do sistema voltou ao rodapé, onde a topbar já o diz"
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
            .map(|e| core_status_pill(e).to_html())
            .collect();

        assert!(rotulos[0].contains("CORE OK"));
        assert!(rotulos[1].contains("CORE INDISPONÍVEL"));
        assert!(rotulos[2].contains("CORE SEM RESPOSTA"));

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
        core_status_pill(&core).to_html()
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
            html.contains("CORE OK"),
            "com a instalação `degraded` por opcionais, a topbar diz: {html}"
        );
        assert!(
            !html.contains("CORE LIMITADO"),
            "o Core aparece limitado por falta de capacidades opcionais: {html}"
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
                html.contains(r#"class="oc-core-pill""#),
                "{prontidao:?} não usa a classe base sozinha: {html}"
            );
            for modificador in ["--limited", "--off", "--warn"] {
                assert!(
                    !html.contains(modificador),
                    "{prontidao:?} traz o modificador `{modificador}`: {html}"
                );
            }
        }
    }

    /// O que é mesmo um problema continua a dizer-se como problema.
    #[test]
    fn blocked_e_sem_resposta_nunca_sao_core_ok() {
        let bloqueado = distintivo(ReadinessOverall::Blocked);
        assert!(bloqueado.contains("CORE INDISPONÍVEL"));
        assert!(!bloqueado.contains("CORE OK"));
        assert!(bloqueado.contains("oc-core-pill--off"));

        let calado = core_status_pill(&CoreStatus::Silent).to_html();
        assert!(calado.contains("CORE SEM RESPOSTA"));
        assert!(!calado.contains("CORE OK"));
        assert!(calado.contains("oc-core-pill--off"));
    }
}
