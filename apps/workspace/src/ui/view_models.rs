//! TODOS os ViewModels da interface, num só sítio.
//!
//! O servidor preenche-os a partir do Core; as vistas só os lêem. Um campo
//! `Option<…>` a `None` quer dizer «o Core não respondeu» ou «não se sabe», e a
//! vista mostra o estado honesto — nunca um `0` inventado.
//!
//! Cada parte da entrega acrescenta aqui os seus tipos. Mudanças de contrato
//! ficam registadas em `docs/ui/HANDOFF.md` (secção «Contratos alterados»).

/// Superfície do documento: decide que CSS/JS de área carregam.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    /// Arranque, autenticação e fim de sessão (fundo navy).
    Auth,
    /// A casca autenticada (barra de topo, Desktop, aplicações).
    Shell,
}

impl Surface {
    /// O identificador em `data-surface`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Auth => "auth",
            Self::Shell => "shell",
        }
    }
}

/// Tema visual. As superfícies de autenticação são sempre escuras.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Theme {
    /// Claro (predefinido na casca).
    #[default]
    Light,
    /// Escuro.
    Dark,
}

impl Theme {
    /// O identificador em `data-theme`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }
}

/// O documento base.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocumentVm {
    /// O título da página, já traduzido (ex.: «Iniciar sessão»).
    pub title: String,
    /// A superfície.
    pub surface: Surface,
    /// O tema.
    pub theme: Theme,
}

/// Estado de uma dependência que a vista mostra (Core, nó de IA, correio…).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Health {
    /// A funcionar.
    Operational,
    /// A funcionar com limitações.
    Degraded,
    /// Sem resposta ou por configurar.
    Unavailable,
}

/// Uma resposta de erro do Core, sem detalhe técnico.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoreError {
    /// A referência que a pessoa pode dar ao administrador (ex.: `OC-7F3A`).
    pub reference: String,
}

/// O estado de uma página ou de um bloco de dados.
///
/// Todas as páginas desenham todos os ramos (HANDOFF · «Estados»).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Load<T> {
    /// Há dados.
    Ready(T),
    /// Ainda à espera do Core (resposta em streaming ou bloco carregado depois).
    Loading,
    /// A lista existe e está vazia.
    Empty,
    /// O Core não respondeu; mostra a referência.
    Failed(CoreError),
    /// Sem permissão — sem revelar se o recurso existe.
    Denied,
    /// Uma dependência em falta (IA sem nó, correio por configurar).
    Unavailable,
    /// A aplicação está inactiva nesta Instância.
    Inactive,
}

// ── Autenticação e arranque (P1) ────────────────────────────────────────────

/// A distribuição da Instância. O Core ainda lhe chama `profile`
/// (`InstanceProfile`): a conversão faz-se na rota
/// (BACKEND_TERMINOLOGY_MIGRATION_REQUIRED).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Distribution {
    /// Centros de investigação e I&D.
    Research,
    /// Empresas e organizações operacionais.
    Business,
    /// Ambientes individuais.
    Personal,
    /// Escolas e universidades.
    Education,
}

impl Distribution {
    /// O identificador estável (`research`…), usado em `data-distribution`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Research => "research",
            Self::Business => "business",
            Self::Personal => "personal",
            Self::Education => "education",
        }
    }
}

/// O que se sabe à porta, antes de haver sessão.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct DoorVm {
    /// A distribuição da Instância, se o Core respondeu a
    /// `GET /api/v1/instance/branding`.
    pub distribution: Option<Distribution>,
    /// O estado da Instância, se foi sondado neste pedido. `None` não afirma nada.
    pub core: Option<Health>,
}

/// `GET /login` e a resposta a um `POST /login` recusado.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct LoginVm {
    /// A porta.
    pub door: DoorVm,
    /// A mensagem do Core quando a entrada falhou (a mesma para todas as falhas).
    pub error: Option<String>,
    /// O endereço que a pessoa escreveu, para não o perder depois de um erro.
    pub email: String,
}

/// `GET /password/recover` (G-26).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct RecoverVm {
    /// A porta.
    pub door: DoorVm,
    /// `true` quando existir `POST /password/recover`. Hoje: `false`.
    pub available: bool,
    /// Mostra a confirmação neutra (depois do envio, exista ou não a conta).
    pub sent: bool,
}

/// Porque terminou a sessão (`/login?reason=…`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionEndReason {
    /// O cookie de sessão já não é conhecido.
    Expired,
    /// O acesso foi retirado por um administrador (G-27: o Core ainda não dá o motivo).
    Revoked,
}

/// Fim de sessão.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionEndVm {
    /// A porta.
    pub door: DoorVm,
    /// O motivo.
    pub reason: SessionEndReason,
}

/// `GET /first-access`: definir a palavra-passe definitiva.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct FirstAccessVm {
    /// A porta.
    pub door: DoorVm,
    /// O nome do membro (da sessão restrita).
    pub display_name: String,
    /// O endereço do membro.
    pub email: String,
    /// O comprimento mínimo exigido pelo Core.
    pub min_length: u32,
    /// A mensagem do Core quando a palavra-passe foi recusada.
    pub error: Option<String>,
}

/// `GET /mfa` quando o segundo factor ainda não está configurado (D8a).
/// `manual_key` só vem preenchida com `/mfa?show_key=1` (ADR-0107).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct MfaSetupVm {
    /// A porta.
    pub door: DoorVm,
    /// O URI `otpauth://` que o QR codifica. Nunca vai para a URL nem para logs.
    pub otpauth_uri: String,
    /// A mesma chave, para escrever à mão (mostrada só a pedido).
    pub manual_key: String,
    /// A mensagem do Core quando o código não confirmou.
    pub error: Option<String>,
}

/// A resposta a `POST /mfa/confirm`: os códigos de recuperação (D8b).
/// Mostrados uma única vez, nesta resposta; nunca guardados pela vista.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct MfaCodesVm {
    /// A porta.
    pub door: DoorVm,
    /// Os códigos, pela ordem do Core.
    pub codes: Vec<String>,
}

/// `GET /mfa` quando o segundo factor já está configurado (D8).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct MfaChallengeVm {
    /// A porta.
    pub door: DoorVm,
    /// A mensagem do Core quando o código falhou.
    pub error: Option<String>,
    /// `true` para abrir já a secção «usar um código de recuperação».
    pub recovery_open: bool,
}

/// O arranque, como uma pessoa o vê (`GET /boot`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BootState {
    /// A Instância ainda está a arrancar.
    Starting,
    /// Pronta: segue-se o início de sessão.
    Ready,
    /// Parou num componente indispensável.
    Blocked,
}

/// Um componente do arranque.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BootComponent {
    /// O nome, já traduzido (ex.: «Base de dados»).
    pub name: String,
    /// O estado.
    pub health: Health,
    /// Uma nota curta, já traduzida, quando o estado não é operacional.
    pub note: Option<String>,
}

/// `GET /boot`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BootVm {
    /// A porta.
    pub door: DoorVm,
    /// O estado.
    pub state: BootState,
    /// Os componentes, pela ordem do Core.
    pub components: Vec<BootComponent>,
    /// A referência para o administrador quando parou.
    pub reference: Option<String>,
}

// ── Casca e Home (P2) ───────────────────────────────────────────────────────

/// Uma aplicação do registo, já filtrada pelo que o membro pode ver.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppTile {
    /// O identificador (`ApplicationId`), enviado em `PUT /apps/pins`.
    pub id: &'static str,
    /// A rota da aplicação (`ApplicationManifest::route`). Também escolhe o ícone.
    pub href: &'static str,
    /// O nome, já traduzido (`name_key`).
    pub label: String,
    /// A descrição curta, já traduzida (`description_key`).
    pub description: String,
    /// Está fixada na barra de aplicações.
    pub pinned: bool,
    /// Pode ser fixada (`Application::can_pin`).
    pub pinnable: bool,
    /// É a aplicação da página actual.
    pub active: bool,
}

/// O que a casca precisa em todas as páginas autenticadas.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ShellVm {
    /// O nome do membro.
    pub display_name: String,
    /// O endereço do membro.
    pub email: String,
    /// As aplicações visíveis, pela ordem do registo.
    pub apps: Vec<AppTile>,
    /// Notificações por ler; `None` quando o Core não respondeu (não se mostra 0).
    pub unread: Option<u32>,
    /// Estado da Instância (G-09).
    pub core: Option<Health>,
    /// Estado do nó de IA (G-09). `Unavailable` = sem nó.
    pub ai: Option<Health>,
    /// O texto da pesquisa actual, para o preservar.
    pub query: String,
    /// A distribuição da Instância (distintivo na barra de cima).
    pub distribution: Option<Distribution>,
    /// O trilho: o nome da aplicação actual («Home»), já traduzido.
    pub crumb: String,
    /// O fundo do Desktop do membro (vale em todas as páginas da casca).
    pub wallpaper: Wallpaper,
    /// Escurecimento do fundo, 0–60 (%), em passos de 5.
    pub dim: u8,
    /// D002 · As janelas abertas. `None` = sem gestor de janelas (comportamento D001).
    pub wm: Option<WmVm>,
    /// D002 · Painéis da barra de cima. Cada `None` mantém o controlo D001 (ligação).
    pub panels: TopPanels,
    /// D003 · A superfície universal da Nye (Pesquisar · Perguntar · Executar).
    /// `None` = a paleta D001, sem alterações (contrato de regressão D003).
    pub nye: Option<NyeSurfaceVm>,
}

/// Um elemento de um widget da Home.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WidgetItem {
    /// O título.
    pub title: String,
    /// A linha secundária (prazo, origem, tamanho…), já traduzida.
    pub meta: String,
    /// Para onde leva.
    pub href: String,
}

// ── Desktop (P2.3 · G-02/03/04) ─────────────────────────────────────────────

/// Os fundos do Desktop (lista fechada). `photo` fica de fora até haver
/// contrato de carregamento de imagem.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Wallpaper {
    /// Gradiente Ocinye (predefinido).
    #[default]
    Ocinye,
    /// Crepúsculo.
    Dusk,
    /// Institucional (riscado dourado).
    Institutional,
    /// Névoa (claro).
    Mist,
    /// Ardósia.
    Slate,
    /// Areia (claro).
    Sand,
}

impl Wallpaper {
    /// Todos, pela ordem da escolha.
    pub const ALL: [Self; 6] = [
        Self::Ocinye,
        Self::Dusk,
        Self::Institutional,
        Self::Mist,
        Self::Slate,
        Self::Sand,
    ];

    /// O identificador no contrato (`wallpaper`) e em `data-wall`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ocinye => "ocinye",
            Self::Dusk => "dusk",
            Self::Institutional => "org",
            Self::Mist => "mist",
            Self::Slate => "slate",
            Self::Sand => "sand",
        }
    }

    /// Lê o identificador do contrato.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|w| w.as_str() == s)
    }
}

/// Os tipos de widget (identificador estável no contrato: `kind`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WidgetKind {
    /// Indicadores (números do contexto).
    Kpis,
    /// Avisos institucionais (obrigatório).
    Notice,
    /// Continuar trabalho.
    Continue,
    /// Tarefas.
    Tasks,
    /// Calendário (hoje).
    Calendar,
    /// Notas recentes.
    Notes,
    /// Ficheiros recentes.
    Files,
    /// Correio por ler.
    Mail,
    /// Actividade.
    Activity,
    /// Projectos.
    Projects,
    /// Ideias.
    Ideas,
    /// Datasets.
    Datasets,
    /// Armazenamento.
    Storage,
    /// Estado do sistema (todos os membros; o administrador abre o Monitor).
    Health,
}

impl WidgetKind {
    /// Todos.
    pub const ALL: [Self; 14] = [
        Self::Kpis,
        Self::Notice,
        Self::Continue,
        Self::Tasks,
        Self::Calendar,
        Self::Notes,
        Self::Files,
        Self::Mail,
        Self::Activity,
        Self::Projects,
        Self::Ideas,
        Self::Datasets,
        Self::Storage,
        Self::Health,
    ];

    /// O identificador no contrato.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Kpis => "kpis",
            Self::Notice => "notice",
            Self::Continue => "continue",
            Self::Tasks => "tasks",
            Self::Calendar => "calendar",
            Self::Notes => "notes",
            Self::Files => "files",
            Self::Mail => "mail",
            Self::Activity => "activity",
            Self::Projects => "projects",
            Self::Ideas => "ideas",
            Self::Datasets => "datasets",
            Self::Storage => "storage",
            Self::Health => "health",
        }
    }

    /// Lê o identificador do contrato.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.as_str() == s)
    }
}

/// Um widget colocado: a ordem no vector é a posição na grelha de 4 colunas.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlacedWidget {
    /// Identificador estável (hoje igual a `kind`: um de cada tipo).
    pub id: String,
    /// O tipo.
    pub kind: WidgetKind,
    /// Colunas (1–4), de entre os tamanhos permitidos do tipo.
    pub w: u8,
    /// Linhas (1–2).
    pub h: u8,
    /// Recolhido: só o cabeçalho (2 linhas finas da grelha).
    pub minimized: bool,
}

/// Um indicador (widget Indicadores): ícone, título, seta, número e qualificativo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Metric {
    /// O ícone do sprite (`registry::KPIS`).
    pub icon: &'static str,
    /// O título, já traduzido («Unidades»).
    pub label: String,
    /// O número, já formatado. Nunca `0` inventado: sem resposta, o widget é `Failed`.
    pub value: String,
    /// O qualificativo, já traduzido e concordante com o número («activas»).
    pub qualifier: String,
    /// Para onde leva.
    pub href: String,
}

/// Um contador (Ideias, Datasets, Projectos a 1 coluna, Estado do sistema).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Count {
    /// O valor, já formatado («12», «OK»).
    pub value: String,
    /// O qualificativo, já traduzido («em investigação»).
    pub qualifier: String,
}

/// O uso de armazenamento.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StorageUse {
    /// Usado, já formatado («12,4 GB»).
    pub used: String,
    /// Quota, já formatada.
    pub total: String,
    /// Percentagem 0–100.
    pub percent: u8,
}

/// O conteúdo de um widget, pela forma que o tipo desenha.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WidgetContent {
    /// Uma lista (tarefas, notas, ficheiros, correio, actividade, avisos,
    /// calendário, projectos a 2 colunas).
    List(Load<Vec<WidgetItem>>),
    /// Continuar trabalho (11a): a meta compõe-se na vista a partir de dados tipados.
    Continue(Load<Vec<ContinueItem>>),
    /// Estado do sistema (11a).
    Health(Load<HealthVm>),
    /// Os indicadores.
    Metrics(Load<Vec<Metric>>),
    /// Um contador.
    Count(Load<Count>),
    /// Armazenamento.
    Storage(Load<StorageUse>),
}

/// Um widget no Desktop, com os dados.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeskWidget {
    /// Onde e com que tamanho.
    pub placed: PlacedWidget,
    /// O que mostra.
    pub content: WidgetContent,
}

/// De onde vem a predefinição que «Repor» aplica (D001.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DefaultSource {
    /// A predefinição do sistema para a Distribuição (`registry::system_default`).
    /// Não houve publicação: `name`, `version` e `published` não se mostram.
    #[default]
    System,
    /// Publicada pela administração da Instância (FG-014).
    Instance,
}

/// A predefinição que «Repor» aplica: a da Instância, se a administração a
/// publicou; senão, a do sistema para a Distribuição.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DesktopDefault {
    /// A origem. Com `System` a folha não mostra versão nem data.
    pub source: DefaultSource,
    /// O nome dado pela administração (só `Instance`).
    pub name: String,
    /// A versão publicada (só `Instance`; com `System`, a versão do registo).
    pub version: u32,
    /// A data de publicação, já formatada (só `Instance`; com `System`, vazia).
    pub published: String,
    /// O fundo.
    pub wallpaper: Wallpaper,
    /// O escurecimento.
    pub dim: u8,
    /// Os widgets, por ordem.
    pub widgets: Vec<PlacedWidget>,
}

// ── Desktop · Continuar trabalho e Estado do sistema (11a) ──────────────────

/// O tipo de um item de «Continuar trabalho» (contrato: `kind`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContinueKind {
    /// Ideia.
    Idea,
    /// Projecto.
    Project,
    /// Ficheiro.
    File,
    /// Nota.
    Note,
    /// Dataset.
    Dataset,
}

impl ContinueKind {
    /// Todos.
    pub const ALL: [Self; 5] = [
        Self::Idea,
        Self::Project,
        Self::File,
        Self::Note,
        Self::Dataset,
    ];

    /// O identificador no contrato.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Idea => "idea",
            Self::Project => "project",
            Self::File => "file",
            Self::Note => "note",
            Self::Dataset => "dataset",
        }
    }

    /// A chave do rótulo em maiúsculas («IDEIA»).
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Idea => "desk.cont.type.idea",
            Self::Project => "desk.cont.type.project",
            Self::File => "desk.cont.type.file",
            Self::Note => "desk.cont.type.note",
            Self::Dataset => "desk.cont.type.dataset",
        }
    }

    /// Lê o identificador do contrato.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.as_str() == s)
    }
}

/// Há quanto tempo, na forma curta dos widgets («agora», «5 min», «2 h», «3 d», «22/09»).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ago {
    /// Menos de 1 minuto.
    Now,
    /// 1–59 minutos.
    Minutes(u32),
    /// 1–23 horas.
    Hours(u32),
    /// 1–6 dias.
    Days(u32),
    /// 7 dias ou mais: a data curta, já formatada no idioma do membro (`dd/mm`).
    Date(String),
}

impl Ago {
    /// Escolhe a forma pelos segundos decorridos; `date` só é chamada a partir de 7 dias.
    #[must_use]
    pub fn from_secs(secs: u64, date: impl FnOnce() -> String) -> Self {
        let n = |d: u64| u32::try_from(secs / d).unwrap_or(u32::MAX);
        match secs {
            0..60 => Self::Now,
            60..3_600 => Self::Minutes(n(60)),
            3_600..86_400 => Self::Hours(n(3_600)),
            86_400..604_800 => Self::Days(n(86_400)),
            _ => Self::Date(date()),
        }
    }
}

/// Um item de «Continuar trabalho»: um objecto que o próprio membro abriu ou editou.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContinueItem {
    /// O tipo.
    pub kind: ContinueKind,
    /// O nome do objecto.
    pub title: String,
    /// Para onde leva (ficha/editor na aplicação, ou a pré-visualização em janela).
    pub href: String,
    /// Só projectos: % de tarefas concluídas (sem canceladas). `None` sem tarefas.
    pub progress: Option<u8>,
    /// O último toque do membro.
    pub when: Ago,
}

/// A última cópia de segurança, como o widget a diz.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Backup {
    /// Hoje, com êxito: a hora (`hh:mm`, na zona do membro).
    Today(String),
    /// Ontem, com êxito: a hora.
    Yesterday(String),
    /// Antes de ontem, com êxito: a data curta (`dd/mm`).
    Date(String),
    /// A última tentativa falhou.
    Failed,
    /// Nunca houve cópia.
    Never,
    /// O Core não tem registo de cópias (D001.1). Não afirma êxito nem falha.
    /// Para `derive_state`, passe `backup_fresh = true`: a ausência de registo
    /// não degrada o estado.
    Unknown,
}

/// O widget «Estado do sistema».
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HealthVm {
    /// O estado agregado ([`HealthVm::derive_state`]).
    pub state: Health,
    /// Nós de computação (GPU/CPU) activos, pelo batimento.
    pub nodes_up: u16,
    /// Nós de computação registados. `0`: o segmento dos nós não aparece.
    pub nodes_total: u16,
    /// A última cópia.
    pub backup: Backup,
    /// Só administradores: a rota do Monitor. `None`: o widget não é clicável.
    pub admin_href: Option<String>,
}

impl HealthVm {
    /// O pior entre Core, nós e cópia. `core_ok`: o Core responde e a base de dados
    /// está acessível. `backup_fresh`: última cópia com êxito há menos de 24 h e a
    /// última tentativa não falhou. Sem nós registados, os nós não degradam.
    #[must_use]
    pub const fn derive_state(
        core_ok: bool,
        nodes_up: u16,
        nodes_total: u16,
        backup_fresh: bool,
    ) -> Health {
        if !core_ok {
            Health::Unavailable
        } else if (nodes_total > 0 && nodes_up < nodes_total) || !backup_fresh {
            Health::Degraded
        } else {
            Health::Operational
        }
    }
}

/// `GET /` — o Desktop.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DesktopVm {
    /// A casca (inclui `wallpaper` e `dim` do membro).
    pub shell: ShellVm,
    /// A versão da disposição do membro (concorrência optimista no `PUT`).
    pub version: u32,
    /// Os widgets, por ordem, com os dados.
    pub widgets: Vec<DeskWidget>,
    /// A predefinição da distribuição, se o Core a publicou.
    pub default: Option<DesktopDefault>,
    /// A versão da predefinição de onde veio a disposição do membro.
    pub base_version: Option<u32>,
    /// Pode ver widgets só de administração (Estado do sistema).
    pub is_admin: bool,
    /// A política permite personalizar. `false`: «Personalizar» desactivado com a razão.
    pub can_customise: bool,
}

// ── Erros e identidade por confirmar (D001.1) ──────────────────────────────

/// O tipo de erro que a página mostra. O estado HTTP é do servidor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    /// 404: não existe, ou está escondido de propósito.
    NotFound,
    /// 403: autenticado, mas sem permissão.
    Forbidden,
    /// 502: o Core ou outra dependência falhou.
    Upstream,
}

impl ErrorKind {
    /// O código mostrado («404»).
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::NotFound => "404",
            Self::Forbidden => "403",
            Self::Upstream => "502",
        }
    }

    /// O identificador em `data-kind`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotFound => "not-found",
            Self::Forbidden => "forbidden",
            Self::Upstream => "upstream",
        }
    }
}

/// Uma página de erro.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ErrorVm {
    /// O tipo.
    pub kind: ErrorKind,
    /// A referência para o administrador (`OC-…`), nunca detalhe técnico.
    pub reference: Option<String>,
    /// Para «Tentar de novo» (só `Upstream`): a rota pedida.
    pub retry_href: Option<String>,
}

/// Sessão existe, mas o Core não confirmou a identidade do membro: falha fechado.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct IdentityFailVm {
    /// A porta.
    pub door: DoorVm,
    /// A referência para o administrador.
    pub reference: Option<String>,
    /// A rota pedida, para «Tentar de novo».
    pub retry_href: String,
}

// ── D002 · Janelas e painéis da casca ───────────────────────────────────────
//
// Só apresentação. O motor (ordem, geometria, persistência, política de
// lançamento, RBAC) é do Claude Code; estes tipos são o que a vista desenha.

/// Como uma janela está apresentada.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum WindowState {
    /// Livre, com a geometria dada.
    #[default]
    Normal,
    /// Ocupa a área de trabalho do Ocinye (nunca o ecrã do sistema anfitrião).
    Maximized,
    /// Fora da vista; continua na prateleira.
    Minimized,
    /// Encaixada na metade esquerda.
    SnappedLeft,
    /// Encaixada na metade direita.
    SnappedRight,
}

impl WindowState {
    /// O valor de `data-state`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Maximized => "maximized",
            Self::Minimized => "minimized",
            Self::SnappedLeft => "snap-left",
            Self::SnappedRight => "snap-right",
        }
    }
}

/// O que a janela mostra no corpo.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum WindowContent {
    /// O corpo vem na resposta (a janela da rota pedida).
    Ready,
    /// O corpo é carregado depois (`?frame=1`), ou a aplicação está a abrir.
    #[default]
    Loading,
    /// O ecrã da aplicação ainda não foi entregue (D001 `app_pending`).
    Pending,
    /// A aplicação falhou; mostra só a referência.
    Failed(CoreError),
    /// Sem permissão (sem revelar se o recurso existe).
    Denied,
    /// A aplicação está indisponível nesta Instância.
    Unavailable,
}

/// Posição e tamanho em px, relativos à área de trabalho.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowGeometry {
    /// Esquerda.
    pub x: i32,
    /// Cima.
    pub y: i32,
    /// Largura (≥ `WINDOW_MIN.0`).
    pub w: u32,
    /// Altura (≥ `WINDOW_MIN.1`).
    pub h: u32,
}

/// O tamanho mínimo de uma janela livre (px). Abaixo disto a vista não encolhe.
pub const WINDOW_MIN: (u32, u32) = (360, 240);

impl Default for WindowGeometry {
    fn default() -> Self {
        Self {
            x: 48,
            y: 32,
            w: 760,
            h: 480,
        }
    }
}

/// Uma janela gerida pelo Ocinye.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct WindowVm {
    /// Identificador estável da janela (vários por aplicação são possíveis).
    pub id: String,
    /// A aplicação (`ApplicationId`).
    pub app_id: &'static str,
    /// A rota da aplicação (escolhe o ícone).
    pub app_href: &'static str,
    /// A rota actual da janela (ligação profunda: `/files/abc`).
    pub href: String,
    /// O nome da aplicação, já traduzido.
    pub title: String,
    /// O recurso aberto («Relatório Q3.pdf»), se a aplicação o disser.
    pub subtitle: Option<String>,
    /// Apresentação.
    pub state: WindowState,
    /// Tem o foco.
    pub active: bool,
    /// Ordem de empilhamento (maior = à frente). Só para `data-z`.
    pub z: u16,
    /// Geometria em `Normal` (e a de regresso ao restaurar).
    pub geometry: WindowGeometry,
    /// Há trabalho por guardar (fechar pede confirmação).
    pub dirty: bool,
    /// O corpo.
    pub content: WindowContent,
}

/// O gestor de janelas desta página.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct WmVm {
    /// As janelas, por qualquer ordem (a vista usa `z`).
    pub windows: Vec<WindowVm>,
    /// A dica do atalho do alternador, já no formato da plataforma («⌥ Tab»);
    /// `None` = não mostrar dica.
    pub switcher_hint: Option<String>,
    /// Aplicações que aceitam mais de uma janela (mostra «Nova janela»).
    pub multi_window_apps: Vec<&'static str>,
}

/// Confirmação ao fechar uma janela com trabalho por guardar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirtyCloseVm {
    /// A janela.
    pub window_id: String,
    /// O nome do documento ou da aplicação.
    pub title: String,
    /// É possível guardar agora (senão «Guardar» fica indisponível com a razão).
    pub can_save: bool,
    /// D004 · O rótulo de «Guardar» quando a aplicação o nomeia («Guardar rascunho»).
    /// `None` = `wm.dirty.save`.
    pub save_label: Option<&'static str>,
    /// D004 · O formulário da aplicação que «Guardar» submete (`form=`), para
    /// guardar o texto que ainda só existe no editor. `None` = POST ao gestor.
    pub save_form: Option<String>,
}

/// Uma capacidade no painel de estado.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Capability {
    /// O Core do Ocinye OS.
    Core,
    /// Nós de computação.
    Compute,
    /// Cópias de segurança.
    Backup,
    /// Nós de IA (opcional: a falta não torna o Ocinye OS indisponível).
    Ai,
}

impl Capability {
    /// A chave do nome.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Core => "wm.status.cap.core",
            Self::Compute => "wm.status.cap.compute",
            Self::Backup => "wm.status.cap.backup",
            Self::Ai => "wm.status.cap.ai",
        }
    }
}

/// O estado de uma capacidade.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapabilityVm {
    /// Qual.
    pub kind: Capability,
    /// Obrigatória para o Ocinye OS funcionar.
    pub required: bool,
    /// `None` = sem registo (não afirma êxito nem falha).
    pub state: Option<Health>,
    /// Uma linha já traduzida («3/4 nós», «cópia 03:00»), sem detalhe técnico.
    pub detail: Option<String>,
}

/// O painel que abre da pastilha CORE·IA.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusPanelVm {
    /// O estado do Ocinye OS: só as obrigatórias contam.
    pub overall: Health,
    /// As capacidades, obrigatórias primeiro.
    pub capabilities: Vec<CapabilityVm>,
    /// Armazenamento pessoal (usado, limite) em bytes, se conhecido.
    pub storage: Option<(u64, u64)>,
    /// «Estado detalhado» (só administração: o Monitor).
    pub detail_href: Option<&'static str>,
}

/// Uma notificação no painel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotificationItem {
    /// Identificador (para marcar como lida).
    pub id: String,
    /// Título.
    pub title: String,
    /// Uma linha, já traduzida.
    pub body: String,
    /// Há quanto tempo.
    pub when: Ago,
    /// Já lida.
    pub read: bool,
    /// Para onde leva.
    pub href: String,
}

/// O painel das notificações (até 6).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotificationsPanelVm {
    /// As últimas.
    pub items: Load<Vec<NotificationItem>>,
}

/// O painel do relógio: o mês de hoje e a agenda de hoje.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClockPanelVm {
    /// Ano, mês (1–12) e dia de hoje no fuso do membro.
    pub today: (i32, u8, u8),
    /// Dia da semana do dia 1 (0 = segunda … 6 = domingo).
    pub first_weekday: u8,
    /// Dias do mês.
    pub days_in_month: u8,
    /// Até 3 eventos de hoje (`/calendar/agenda`).
    pub agenda: Load<Vec<WidgetItem>>,
}

/// Os painéis da barra de cima.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct TopPanels {
    /// Estado do sistema.
    pub status: Option<StatusPanelVm>,
    /// Notificações.
    pub notifications: Option<NotificationsPanelVm>,
    /// Relógio.
    pub clock: Option<ClockPanelVm>,
}

// ── D003 · Nye: Search · Ask · Act ───────────────────────────────────────
//
// Só forma de apresentação. Autorização, risco, necessidade de confirmação,
// estado de execução e disponibilidade chegam do Core (ou do AI Fabric através
// do Core) já decididos; a vista nunca os infere. Nenhum destes tipos transporta
// autoridade executável: um `plan_id` e um `digest` identificam uma proposta,
// não a autorizam. Nenhum nome de modelo ou fornecedor aparece aqui.

/// O que o membro pediu (`ocinye_contracts::agentic::Intent`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NyeIntent {
    /// Encontrar. Determinístico; não precisa de modelo.
    Search,
    /// Uma pergunta respondida por inferência, com fontes.
    Ask,
    /// Uma acção proposta através de capacidades tipadas.
    Act,
}

impl NyeIntent {
    /// Valor do formulário (`intent` em `POST /agentic/invoke`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Search => "search",
            Self::Ask => "ask",
            Self::Act => "act",
        }
    }
}

/// Porque uma parte da Nye não está disponível, ou porque um pedido parou.
/// Tipado: nunca «algo correu mal» quando há uma razão mais precisa.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NyeReason {
    /// Nenhum modelo registado serve a capacidade (`capability_unavailable`).
    NoCompatibleModel,
    /// Nenhum recurso de inferência nesta Instância (`NO_RESOURCE`).
    NoInference,
    /// Há recurso, mas não responde agora.
    InferenceUnavailable,
    /// O fornecedor escolhido pelo Router falhou (`AI_PROVIDER_UNHEALTHY`).
    ProviderUnavailable,
    /// A política exclui todos os candidatos (`AI_POLICY_BLOCKED`).
    PolicyBlocked,
    /// Excedeu o prazo do Core.
    Timeout,
    /// O membro parou.
    Cancelled,
    /// O Core recusou (403).
    PermissionDenied,
    /// O contexto deixou de existir ou de estar acessível.
    ContextUnavailable,
    /// A capacidade não existe ou não está publicada no registry.
    CapabilityUnavailable,
    /// O Planner rejeitou a saída do modelo.
    MalformedProposal,
    /// O executor devolveu erro.
    ExecutionFailed,
    /// Alguns passos concluíram e outros não.
    PartialExecution,
    /// O Core não responde.
    CoreUnavailable,
    /// A ligação caiu e está a ser restabelecida.
    Reconnecting,
    /// O dispositivo ou o runtime não tem microfone.
    MicUnavailable,
    /// O membro, ou o browser, recusou o microfone.
    MicDenied,
    /// Sem STT/TTS nesta Instância.
    VoiceUnavailable,
    /// Uma fonte deixou de estar autorizada.
    SourceRevoked,
}

impl NyeReason {
    /// A chave de catálogo da frase que a explica.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::NoCompatibleModel => "nye.reason.no_compatible_model",
            Self::NoInference => "nye.reason.no_inference",
            Self::InferenceUnavailable => "nye.reason.inference_unavailable",
            Self::ProviderUnavailable => "nye.reason.provider_unavailable",
            Self::PolicyBlocked => "nye.reason.policy_blocked",
            Self::Timeout => "nye.reason.timeout",
            Self::Cancelled => "nye.reason.cancelled",
            Self::PermissionDenied => "nye.reason.permission_denied",
            Self::ContextUnavailable => "nye.reason.context_unavailable",
            Self::CapabilityUnavailable => "nye.reason.capability_unavailable",
            Self::MalformedProposal => "nye.reason.malformed_proposal",
            Self::ExecutionFailed => "nye.reason.execution_failed",
            Self::PartialExecution => "nye.reason.partial_execution",
            Self::CoreUnavailable => "nye.reason.core_unavailable",
            Self::Reconnecting => "nye.reason.reconnecting",
            Self::MicUnavailable => "nye.reason.mic_unavailable",
            Self::MicDenied => "nye.reason.mic_denied",
            Self::VoiceUnavailable => "nye.reason.voice_unavailable",
            Self::SourceRevoked => "nye.reason.source_revoked",
        }
    }
}

/// Disponibilidade de uma capacidade da Nye. Sem `Default`: quem preenche
/// tem de saber (a ausência de resposta do Core é `Unavailable(CoreUnavailable)`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NyeAvail {
    /// Pode ser usada agora.
    Available,
    /// Não pode, por esta razão.
    Unavailable(NyeReason),
}

impl NyeAvail {
    /// Se está disponível.
    #[must_use]
    pub const fn is_available(self) -> bool {
        matches!(self, Self::Available)
    }
}

/// O estado da ligação da Nye ao Core.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NyeLink {
    /// Ligada.
    Connected,
    /// A restabelecer (o Workspace perdeu o Core há pouco).
    Reconnecting,
    /// O Core não responde.
    CoreUnavailable,
}

/// NYE-01 · O que a Nye pode fazer agora. Nunca «IA ligada/desligada».
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NyeAvailability {
    /// Pesquisa determinística, navegação e comandos determinísticos.
    pub search: NyeAvail,
    /// Respostas por inferência.
    pub ask: NyeAvail,
    /// Propostas de acção (o Planner precisa de inferência hoje).
    pub act: NyeAvail,
    /// Entrada de voz (STT + microfone do runtime).
    pub voice_input: NyeAvail,
    /// Saída de voz (TTS).
    pub voice_output: NyeAvail,
    /// Anexar ficheiros do Ocinye.
    pub attachments: NyeAvail,
    /// A ligação ao Core.
    pub link: NyeLink,
}

/// O tipo de contexto (`CLAUDE.md` · o Espaço Pessoal é um contexto, não uma Distribuição).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NyeContextKind {
    /// A organização.
    Organization,
    /// Uma unidade.
    Unit,
    /// Uma equipa.
    Team,
    /// Um projecto.
    Project,
    /// O Espaço Pessoal.
    Personal,
}

/// O estado do contexto de trabalho.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NyeContextState {
    /// Em uso.
    Active,
    /// Mudou nesta conversa (as permissões não mudam com ele).
    Changed,
    /// Deixou de existir ou de estar acessível. `label` vem vazio: nunca se revela o nome.
    Unavailable,
}

/// NYE-09 · O envelope de contexto, pelo nome humano (nunca um identificador).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NyeContextVm {
    /// O tipo.
    pub kind: NyeContextKind,
    /// «Projeto Solander». Vazio quando `Unavailable`.
    pub label: String,
    /// «Unidade de Investigação», quando ajuda a distinguir.
    pub parent: Option<String>,
    /// O estado.
    pub state: NyeContextState,
    /// Onde mudar de contexto, quando o Workspace o permite.
    pub change_href: Option<String>,
}

/// O tipo de um resultado ou de uma fonte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NyeKind {
    /// Aplicação.
    App,
    /// Ficheiro.
    File,
    /// Pasta.
    Folder,
    /// Nota.
    Note,
    /// Projecto.
    Project,
    /// Tarefa.
    Task,
    /// Pessoa (quando autorizado).
    Member,
    /// Definição.
    Setting,
    /// Comando determinístico.
    Action,
    /// Dataset.
    Dataset,
    /// Mensagem de correio.
    Mail,
    /// Evento do calendário.
    Event,
    /// Conversa da Nye.
    Conversation,
    /// Conteúdo web externo (não confiável).
    Web,
    /// Outro registo do Ocinye.
    Other,
}

/// NYE-04 · Um resultado da pesquisa determinística.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NyeHitVm {
    /// O tipo.
    pub kind: NyeKind,
    /// O nome.
    pub title: String,
    /// Onde vive («Projetos / Solander»), para distinguir nomes iguais.
    pub context: String,
    /// «Alterado há 2 h», «2,4 MB».
    pub meta: Option<String>,
    /// A aplicação dona («Ficheiros»).
    pub app: Option<String>,
    /// Ligação profunda canónica do Ocinye.
    pub href: String,
}

/// Um grupo de resultados, pela ordem do Core.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NyeHitGroupVm {
    /// O tipo do grupo.
    pub kind: NyeKind,
    /// Os resultados.
    pub hits: Vec<NyeHitVm>,
}

/// Confiança de uma fonte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NyeTrust {
    /// Um registo do Ocinye, lido com a política do membro.
    Ocinye,
    /// Conteúdo externo: dados, nunca instrução.
    External,
}

/// NYE-05 · Uma fonte de uma resposta. Nunca inventada.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NyeSourceVm {
    /// O número da referência no texto (`[1]`).
    pub n: u16,
    /// O tipo.
    pub kind: NyeKind,
    /// O título. Vazio quando `available == false` (não se mostra conteúdo protegido).
    pub title: String,
    /// Onde vive. Vazio quando indisponível.
    pub context: String,
    /// Secção ou página, quando o Core a tem.
    pub locator: Option<String>,
    /// Quando, se for significativo.
    pub at: Option<String>,
    /// Ligação profunda do Ocinye. Nunca um caminho do anfitrião.
    pub href: Option<String>,
    /// `false` quando a autorização foi revogada depois da resposta.
    pub available: bool,
    /// Confiança.
    pub trust: NyeTrust,
}

/// O estado de um passo de actividade.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NyeStepState {
    /// Em fila.
    Queued,
    /// A correr.
    Running,
    /// À espera de uma dependência externa.
    Waiting,
    /// Concluído.
    Done,
    /// Falhou.
    Failed,
    /// Não chegou a correr.
    Skipped,
    /// À espera de confirmação do membro.
    AwaitingConfirmation,
}

/// NYE-14 · Um passo do que a Nye fez (acções, capacidades, resultados).
/// Nunca raciocínio, prompts ou mensagens de sistema.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NyeStepVm {
    /// O domínio que o serviu («Projetos»), quando houve delegação.
    pub domain: Option<String>,
    /// «Encontrou 4 notas», «Criou 3 tarefas».
    pub label: String,
    /// O estado.
    pub state: NyeStepState,
    /// Um detalhe factual. Nunca um segredo.
    pub detail: Option<String>,
}

/// A classe de risco, para explicar a acção. Não decide a confirmação.
/// Mapeamento do Core (`RiskLevel`): `read_only` → `ReadOnly`; `low_impact` →
/// `ReversibleWrite`; `material_mutation` → `InstitutionalChange`;
/// `external_effect` → `ExternalCommunication`; `privileged` → `Privileged`.
/// `Navigation` é abrir por ligação profunda (sem plano). `Destructive` não tem
/// origem no Core hoje (nenhuma eliminação definitiva é capacidade).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NyeRisk {
    /// Lê.
    ReadOnly,
    /// Abre.
    Navigation,
    /// Alteração pequena e reversível.
    ReversibleWrite,
    /// Alteração institucional material.
    InstitutionalChange,
    /// Sai da instituição ou chega a alguém fora dela.
    ExternalCommunication,
    /// Privilegiada ou sensível.
    Privileged,
    /// Irreversível.
    Destructive,
}

impl NyeRisk {
    /// Valor de `data-risk`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ReadOnly => "read_only",
            Self::Navigation => "navigation",
            Self::ReversibleWrite => "reversible_write",
            Self::InstitutionalChange => "institutional",
            Self::ExternalCommunication => "external",
            Self::Privileged => "privileged",
            Self::Destructive => "destructive",
        }
    }

    /// Se a apresentação de confirmação é a forte (diálogo). A exigência de
    /// confirmação continua a ser do Core (`NyeAuth::ConfirmationRequired`).
    #[must_use]
    pub const fn high_impact(self) -> bool {
        matches!(
            self,
            Self::ExternalCommunication | Self::Privileged | Self::Destructive
        )
    }
}

/// A decisão do Core sobre a proposta.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NyeAuth {
    /// Autorizada; não precisa de confirmação.
    Authorized,
    /// Autorizada se o membro confirmar (`requires_approval`).
    ConfirmationRequired,
    /// Recusada.
    Denied,
    /// A capacidade não está disponível.
    Unavailable,
    /// Bloqueada pela política da Instância.
    PolicyBlocked,
}

/// O estado de execução (`ocinye_contracts::agentic::PlanState`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NyeExecState {
    /// Proposta.
    Proposed,
    /// Aguarda confirmação.
    AwaitingConfirmation,
    /// Confirmada, ainda não corre.
    Authorized,
    /// A correr.
    Running,
    /// Concluída.
    Completed,
    /// Parcialmente concluída.
    Partial,
    /// Falhou.
    Failed,
    /// Cancelada.
    Cancelled,
    /// Bloqueada (recusa ou política).
    Blocked,
    /// Recusada pelo membro.
    Rejected,
    /// A janela de aprovação fechou.
    Expired,
}

impl NyeExecState {
    /// Valor de `data-state`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Proposed => "proposed",
            Self::AwaitingConfirmation => "awaiting",
            Self::Authorized => "authorized",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Partial => "partial",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Blocked => "blocked",
            Self::Rejected => "rejected",
            Self::Expired => "expired",
        }
    }
}

/// Um parâmetro legível da proposta («Prazo» · «10 out»).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NyeField {
    /// O nome.
    pub label: String,
    /// O valor.
    pub value: String,
    /// Texto longo (corpo de mensagem): desenhado em bloco.
    pub long: bool,
}

/// Um item afectado (uma tarefa, um destinatário, um resultado).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NyeLine {
    /// O nome.
    pub title: String,
    /// A linha secundária.
    pub meta: Option<String>,
    /// Depois da execução: `Some(true)` concluído, `Some(false)` falhou.
    pub ok: Option<bool>,
    /// Ligação profunda para o resultado.
    pub href: Option<String>,
}

/// NYE-08 · O resultado da execução. Sem percentagens inventadas.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NyeExecutionVm {
    /// A frase factual do Core.
    pub summary: Option<String>,
    /// A razão, quando falhou.
    pub error: Option<NyeReason>,
    /// O detalhe do erro, sem dados técnicos.
    pub error_detail: Option<String>,
    /// `retry_allowed` do executor. Sem isto não há «Tentar de novo».
    pub retry_allowed: bool,
    /// Onde repetir (POST), quando permitido.
    pub retry_action: Option<String>,
    /// A referência de auditoria.
    pub audit_ref: Option<String>,
    /// Quando.
    pub at: Option<String>,
    /// Progresso real (`feitos`, `total`), só se o executor o fornece.
    pub progress: Option<(u32, u32)>,
}

/// NYE-06 · Uma proposta de capacidade, legível. Imutável depois de mostrada.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NyeProposalVm {
    /// O plano (`ActionPlan.id`).
    pub plan_id: String,
    /// `ActionPlan.digest`: a confirmação liga-se a este valor.
    pub digest: String,
    /// «Criar 3 tarefas no Projeto Solander».
    pub title: String,
    /// O identificador da capacidade (só em «Detalhes»).
    pub capability: String,
    /// O alvo exacto.
    pub target: String,
    /// O âmbito (contexto).
    pub scope: Option<String>,
    /// Parâmetros importantes.
    pub fields: Vec<NyeField>,
    /// Itens afectados.
    pub lines: Vec<NyeLine>,
    /// Consequências relevantes, em frases.
    pub consequences: Vec<String>,
    /// Classe de risco (do registry, nunca do modelo).
    pub risk: NyeRisk,
    /// A decisão do Core.
    pub auth: NyeAuth,
    /// O estado.
    pub state: NyeExecState,
    /// Se a proposta aceita alterações (que criam uma NOVA proposta).
    pub edit_href: Option<String>,
    /// Substituída por outra proposta: já não se confirma.
    pub superseded: bool,
    /// A proposta refere conteúdo externo.
    pub cites_external: bool,
    /// Até quando a confirmação vale (aprovação de 15 minutos).
    pub expires_at: Option<String>,
    /// O resultado, quando há.
    pub execution: Option<NyeExecutionVm>,
}

/// Quem fala.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NyeRole {
    /// O membro.
    Member,
    /// A Nye.
    Nye,
}

/// Um bloco de conteúdo seguro (nunca HTML do modelo). O Workspace converte o
/// texto da resposta nestes blocos (ADAPTER_REQUIRED).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NyeBlock {
    /// Parágrafo, com as referências a fontes.
    Para {
        /// O texto.
        text: String,
        /// Números de fonte citados.
        cites: Vec<u16>,
    },
    /// Título.
    Heading(String),
    /// Lista.
    List(Vec<String>),
    /// Lista numerada.
    Steps(Vec<String>),
    /// Código.
    Code {
        /// A linguagem, se declarada.
        lang: Option<String>,
        /// O código.
        text: String,
    },
    /// Citação de uma fonte.
    Quote {
        /// O excerto.
        text: String,
        /// A fonte.
        cite: Option<u16>,
    },
    /// Tabela.
    Table {
        /// Cabeçalho.
        head: Vec<String>,
        /// Linhas.
        rows: Vec<Vec<String>>,
    },
}

/// O estado de uma mensagem.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NyeMsgState {
    /// Completa.
    Complete,
    /// A chegar (NYE-03).
    Streaming,
    /// Parada pelo membro ou pela ligação.
    Interrupted,
    /// Falhou, por esta razão.
    Failed(NyeReason),
}

/// O fundamento de uma resposta.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NyeGrounding {
    /// Apoia-se nas fontes listadas.
    Sourced,
    /// Sem fontes do Ocinye: fica dito.
    Ungrounded,
    /// Fontes revogadas depois da resposta: o conteúdo dependente foi retirado.
    Revoked,
    /// Mensagem do membro, ou resultados determinísticos.
    NotApplicable,
}

/// Onde o pedido foi processado, quando a política o torna relevante.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NyeProcessing {
    /// Nesta Instância.
    Local,
    /// Por um fornecedor externo permitido pela política.
    External,
    /// Bloqueado pela política (os dados não podem sair).
    Blocked,
    /// A política exige aprovação do membro para sair.
    ApprovalRequired,
}

/// Um anexo do Ocinye (NYE-10).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NyeAttachmentVm {
    /// O nome.
    pub name: String,
    /// O tipo.
    pub kind: NyeKind,
    /// O tamanho, formatado.
    pub size: Option<String>,
    /// Onde vive no Ocinye.
    pub context: String,
    /// O valor que o composer reenvia (`attachment`), e o de «retirar».
    pub value: String,
}

/// Uma mensagem de uma conversa.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NyeMessageVm {
    /// Identificador.
    pub id: String,
    /// Quem fala.
    pub role: NyeRole,
    /// Quando, formatado.
    pub at: String,
    /// O conteúdo.
    pub blocks: Vec<NyeBlock>,
    /// O estado.
    pub state: NyeMsgState,
    /// O fundamento.
    pub grounding: NyeGrounding,
    /// Onde foi processada (só quando a política o torna relevante).
    pub processing: Option<NyeProcessing>,
    /// As fontes.
    pub sources: Vec<NyeSourceVm>,
    /// O que a Nye fez.
    pub steps: Vec<NyeStepVm>,
    /// Resultados determinísticos.
    pub hits: Vec<NyeHitGroupVm>,
    /// Propostas de acção.
    pub proposals: Vec<NyeProposalVm>,
    /// Anexos (mensagens do membro).
    pub attachments: Vec<NyeAttachmentVm>,
    /// NYE-03 · A fonte do fluxo (SSE, mesma origem) quando `Streaming`.
    pub stream_src: Option<String>,
    /// NYE-13 · Onde parar (POST).
    pub stop_action: Option<String>,
    /// Onde repetir a resposta (POST). Só inferência; nunca uma acção.
    pub retry_action: Option<String>,
    /// Onde aprovar a saída para um fornecedor externo, quando a política o pede.
    pub egress_action: Option<String>,
}

/// Uma conversa na lista (NYE-02).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NyeConvItemVm {
    /// O título.
    pub title: String,
    /// Quando, formatado.
    pub at: String,
    /// Onde abre.
    pub href: String,
    /// A actual.
    pub active: bool,
}

/// A conversa aberta.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NyeConversationVm {
    /// O título.
    pub title: String,
    /// As mensagens, por ordem.
    pub messages: Vec<NyeMessageVm>,
}

/// O estado do composer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NyeComposerState {
    /// Pronto.
    Idle,
    /// A enviar.
    Submitting,
    /// Uma resposta está a chegar.
    Streaming,
    /// Há uma proposta à espera de decisão.
    ProposalPending,
    /// Indisponível (só pesquisa e comandos determinísticos).
    Unavailable(NyeReason),
}

/// O composer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NyeComposerVm {
    /// Para onde envia (POST).
    pub action: String,
    /// O texto por enviar (preservado depois de um erro).
    pub text: String,
    /// Os anexos.
    pub attachments: Vec<NyeAttachmentVm>,
    /// O estado.
    pub state: NyeComposerState,
    /// Onde parar a resposta em curso (POST).
    pub stop_action: Option<String>,
    /// Onde escolher um ficheiro do Ocinye para anexar.
    pub attach_href: Option<String>,
}

/// O estado da voz (NYE-11/12). Premir para falar; nunca escuta contínua.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NyeVoiceState {
    /// Pronta; microfone desligado.
    Idle,
    /// A pedir permissão ao runtime.
    RequestingPermission,
    /// A gravar (só enquanto o membro prime, ou até parar).
    Listening,
    /// A transcrever.
    Transcribing,
    /// A preparar a resposta.
    Processing,
    /// A falar.
    Speaking,
    /// Parada.
    Stopped,
    /// Indisponível, por esta razão.
    Unavailable(NyeReason),
    /// Erro, por esta razão.
    Error(NyeReason),
}

/// A língua da voz.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NyeLang {
    /// Português.
    Pt,
    /// Inglês.
    En,
    /// Francês.
    Fr,
}

impl NyeLang {
    /// Código BCP 47 curto.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pt => "pt",
            Self::En => "en",
            Self::Fr => "fr",
        }
    }
}

/// O modo de voz.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NyeVoiceVm {
    /// O estado.
    pub state: NyeVoiceState,
    /// A língua de entrada.
    pub lang: NyeLang,
    /// `true` quando a língua foi detectada pelo backend (não pela interface).
    pub lang_detected: bool,
    /// A transcrição parcial ou final.
    pub transcript: Option<String>,
    /// Se há resposta para repetir.
    pub replay: bool,
}

/// O painel lateral da aplicação.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum NyePanel {
    /// Nenhum.
    #[default]
    None,
    /// Fontes («o que apoia isto»).
    Sources,
    /// Actividade («o que a Nye fez»).
    Activity,
}

/// A aplicação Nye, dentro de uma janela gerida (D002).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NyeAppVm {
    /// NYE-01.
    pub availability: NyeAvailability,
    /// O contexto de trabalho.
    pub context: Option<NyeContextVm>,
    /// NYE-02 · As conversas. `None` = sem persistência no Core (lacuna honesta).
    pub conversations: Option<Vec<NyeConvItemVm>>,
    /// A pesquisa na lista de conversas.
    pub conv_query: String,
    /// A conversa aberta. `None` = conversa nova.
    pub current: Option<NyeConversationVm>,
    /// O composer.
    pub composer: NyeComposerVm,
    /// O modo de voz, quando aberto.
    pub voice: Option<NyeVoiceVm>,
    /// O painel aberto.
    pub panel: NyePanel,
    /// Todas as fontes da conversa.
    pub panel_sources: Vec<NyeSourceVm>,
    /// Toda a actividade da conversa.
    pub panel_steps: Vec<NyeStepVm>,
    /// Sugestões da Distribuição para a conversa nova (texto do pedido).
    pub suggestions: Vec<String>,
}

/// A superfície universal (D001_COMPONENT_EXTENSION da paleta de comandos).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NyeSurfaceVm {
    /// NYE-01.
    pub availability: NyeAvailability,
    /// A escolha explícita do membro. `None` = automático.
    pub intent: Option<NyeIntent>,
    /// Como o Core leu o pedido (`Intent::detect`), quando houve pedido.
    pub detected: Option<NyeIntent>,
    /// O pedido.
    pub query: String,
    /// Resultados determinísticos (NYE-04).
    pub hits: Vec<NyeHitGroupVm>,
    /// Uma resposta ou proposta curta.
    pub answer: Option<NyeMessageVm>,
    /// O contexto, quando importa.
    pub context: Option<NyeContextVm>,
    /// «Continuar na Nye» (a aplicação completa, com o pedido).
    pub continue_href: String,
    /// O atalho, como o runtime o declara («⌘K», «Ctrl K»). `None` = não se mostra.
    pub shortcut: Option<String>,
    /// Abre já (resposta a `GET /ask?q=…`).
    pub open: bool,
}

// ── D004 · Aplicações de produtividade: Ficheiros · Notas · Calendário · Correio ──
//
// Só forma. Os dados chegam do Core já autorizados; a vista não filtra por
// permissões, não guarda nada e não conhece o armazenamento (sem bucket, chave
// de objecto ou caminho do anfitrião). Datas, horas, números e tamanhos chegam
// já formatados na língua e no fuso do membro (`String`), para que a vista
// nunca formate por si.

/// O estado do conteúdo de uma vista de aplicação.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppLoad {
    /// Pronto (a lista pode estar vazia: ver o estado vazio de cada aplicação).
    Ready,
    /// A carregar (esqueleto parcial, nunca um spinner da aplicação inteira).
    Loading,
    /// Falhou, por esta razão.
    Failed(AppError),
}

/// Erros tipados, partilhados pelas quatro aplicações (e pelas seguintes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppError {
    /// O serviço não responde.
    Unavailable,
    /// O Core recusou.
    PermissionDenied,
    /// Não existe (ou não existe para si: nunca se distingue).
    NotFound,
    /// Alguém alterou entretanto (revisão mudou).
    Conflict,
    /// A sincronização com o serviço externo falhou.
    SyncFailed,
    /// Não foi possível guardar.
    SaveFailed,
    /// O envio de um ficheiro falhou.
    UploadFailed,
    /// O servidor de correio não aceitou ou não foi alcançado.
    TransportFailed,
    /// O acesso foi retirado depois de a vista abrir.
    Revoked,
    /// A ligação ao Core caiu e está a ser restabelecida.
    Reconnecting,
}

impl AppError {
    /// A chave de catálogo do título e do texto (`…​.title` / `…​.body`).
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Unavailable => "app.err.unavailable",
            Self::PermissionDenied => "app.err.denied",
            Self::NotFound => "app.err.not_found",
            Self::Conflict => "app.err.conflict",
            Self::SyncFailed => "app.err.sync",
            Self::SaveFailed => "app.err.save",
            Self::UploadFailed => "app.err.upload",
            Self::TransportFailed => "app.err.transport",
            Self::Revoked => "app.err.revoked",
            Self::Reconnecting => "app.err.reconnecting",
        }
    }
}

/// Uma entrada da navegação lateral de uma aplicação.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppNavVm {
    /// O rótulo, já traduzido.
    pub label: String,
    /// O ícone (`icons.svg`).
    pub icon: &'static str,
    /// Onde leva.
    pub href: String,
    /// Uma contagem real do Core (por ler, rascunhos). `None` = não se mostra.
    pub count: Option<u32>,
    /// A secção actual.
    pub active: bool,
}

/// Paginação por cursor (listas grandes: nunca 10 000 linhas no DOM).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct AppPageVm {
    /// «Mostrar mais» (o cursor seguinte), quando há.
    pub more_href: Option<String>,
    /// «N de M», quando o Core o sabe.
    pub summary: Option<String>,
}

/// O estado de gravação de um documento (Notas, rascunho de Correio).
/// A aplicação reporta-o; o fecho com alterações é o do gestor de janelas (D002).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AppSaveState {
    /// Nada por guardar.
    Clean,
    /// Há alterações por guardar.
    Dirty,
    /// A guardar.
    Saving,
    /// Guardado (quando, formatado).
    Saved(String),
    /// Falhou; o texto continua no editor.
    Failed(AppError),
}

/// A ligação contextual à Nye: abre a Nye canónica com uma referência tipada.
/// A referência não concede autoridade; o Core decide o que a Nye pode ler.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppNyeVm {
    /// `/ai/prompt?ref=…​&q=…` (ou `/ask?…`), construído pelo Code.
    pub href: String,
    /// A chave do rótulo («Perguntar à Nye sobre este ficheiro»).
    pub label_key: &'static str,
}

// ── Ficheiros ──

/// A secção de Ficheiros (as que o Core serve: `/me/files`, favoritos, lixo).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilesSection {
    /// Os meus ficheiros (pastas pessoais).
    Mine,
    /// Recentes.
    Recent,
    /// Favoritos.
    Favourites,
    /// Partilhados comigo.
    Shared,
    /// Ficheiros de projectos e unidades.
    Workspaces,
    /// Lixo (restaurar, eliminar definitivamente).
    Trash,
}

/// Lista ou grelha. A preferência é guardada pelo Code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FilesView {
    /// Lista densa com colunas.
    #[default]
    List,
    /// Grelha com miniaturas.
    Grid,
}

/// A coluna de ordenação.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilesSortKey {
    /// Nome.
    Name,
    /// Alterado.
    Modified,
    /// Tamanho.
    Size,
    /// Tipo.
    Kind,
}

/// O tipo de um item de Ficheiros (escolhe o ícone e a pré-visualização).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileKind {
    /// Pasta.
    Folder,
    /// Imagem.
    Image,
    /// PDF.
    Pdf,
    /// Texto.
    Text,
    /// Código.
    Code,
    /// Folha de cálculo ou dados tabulares.
    Data,
    /// Documento de escritório.
    Document,
    /// Arquivo comprimido.
    Archive,
    /// Áudio ou vídeo.
    Media,
    /// Outro binário.
    Other,
}

/// Um ficheiro ou uma pasta, como o Core o autoriza a ver.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileItemVm {
    /// Identificador opaco para os formulários (nunca uma chave de objecto).
    pub id: String,
    /// O nome.
    pub name: String,
    /// O tipo.
    pub kind: FileKind,
    /// O tipo legível («PDF», «Imagem PNG»).
    pub kind_label: String,
    /// O tamanho formatado. `None` para pastas.
    pub size: Option<String>,
    /// Alterado, formatado.
    pub modified: String,
    /// Dono ou contexto («Projeto Solander»), quando não é o próprio.
    pub context: Option<String>,
    /// Abrir (pasta ou detalhe).
    pub href: String,
    /// Miniatura (mesma origem), quando o Core a tem.
    pub thumb: Option<String>,
    /// Favorito.
    pub favourite: bool,
    /// Partilhado.
    pub shared: bool,
    /// Seleccionado (não é o mesmo que ter o foco nem estar aberto).
    pub selected: bool,
    /// Aberto no inspector.
    pub open: bool,
}

/// Um passo do caminho (nunca um caminho do anfitrião).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CrumbVm {
    /// O nome.
    pub label: String,
    /// Onde leva.
    pub href: String,
}

/// A pré-visualização, segura e só de leitura.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FilePreviewVm {
    /// Imagem (`/me/files/{v}/inline`).
    Image {
        /// A origem.
        src: String,
    },
    /// PDF num leitor do browser, com sandbox.
    Pdf {
        /// A origem.
        src: String,
    },
    /// Texto ou código, truncado pelo Core (`/text`).
    Text {
        /// O texto (nunca interpretado).
        text: String,
        /// A linguagem, quando se sabe.
        lang: Option<String>,
        /// O Core cortou o conteúdo.
        truncated: bool,
    },
    /// Sem pré-visualização para este tipo.
    Unsupported,
}

/// Uma versão de um ficheiro.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileVersionVm {
    /// «Versão 3».
    pub label: String,
    /// Quando.
    pub at: String,
    /// Quem.
    pub author: Option<String>,
    /// O tamanho.
    pub size: String,
    /// Descarregar esta versão.
    pub download_href: String,
    /// Tornar actual (POST), quando o Core o permite.
    pub restore_action: Option<String>,
    /// A actual.
    pub current: bool,
}

/// O inspector de um item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileDetailsVm {
    /// O item.
    pub item: FileItemVm,
    /// Criado.
    pub created: Option<String>,
    /// O dono.
    pub owner: Option<String>,
    /// As versões. `None` = o Core não as serve para este item.
    pub versions: Option<Vec<FileVersionVm>>,
    /// A pré-visualização.
    pub preview: Option<FilePreviewVm>,
    /// Descarregar.
    pub download_href: Option<String>,
    /// Com quem está partilhado (nomes), quando autorizado.
    pub shared_with: Vec<String>,
    /// A Nye sobre este item.
    pub nye: Option<AppNyeVm>,
}

/// O estado de um envio. O progresso só existe quando o envio multipart o mede.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UploadState {
    /// Em fila.
    Queued,
    /// A verificar (hash incremental), sem fim conhecido.
    Checking,
    /// A enviar: bytes enviados e total, formatados, e a fracção real.
    Sending {
        /// «1,2 GB de 4,8 GB».
        text: String,
        /// Partes concluídas (`done`, `total`).
        parts: (u32, u32),
    },
    /// Concluído.
    Done,
    /// Já existe um ficheiro com este nome.
    Conflict,
    /// Falhou.
    Failed(AppError),
    /// Cancelado pelo membro.
    Cancelled,
}

/// Um envio na bandeja.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UploadVm {
    /// Identificador da sessão (para o JS actualizar a linha).
    pub id: String,
    /// O nome.
    pub name: String,
    /// O tamanho, formatado.
    pub size: String,
    /// O estado.
    pub state: UploadState,
}

/// A aplicação Ficheiros.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilesVm {
    /// A secção.
    pub section: FilesSection,
    /// A navegação lateral (só as secções que o Core serve).
    pub nav: Vec<AppNavVm>,
    /// O caminho, desde a raiz da secção.
    pub crumbs: Vec<CrumbVm>,
    /// Lista ou grelha.
    pub view: FilesView,
    /// Ordenação e sentido (`true` = descendente).
    pub sort: (FilesSortKey, bool),
    /// A pesquisa desta pasta (âmbito: Ficheiros).
    pub query: String,
    /// Os itens desta página.
    pub items: Vec<FileItemVm>,
    /// O estado.
    pub load: AppLoad,
    /// Paginação.
    pub page: AppPageVm,
    /// O inspector aberto.
    pub details: Option<FileDetailsVm>,
    /// A bandeja de envios.
    pub uploads: Vec<UploadVm>,
    /// A pasta actual (para enviar e criar pasta), quando se pode escrever.
    pub folder_id: Option<String>,
    /// O Core permite escrever aqui.
    pub can_write: bool,
    /// Pré-visualização em ecrã inteiro (móvel e `?preview=1`).
    pub preview_open: bool,
}

// ── Notas ──

/// Uma nota na lista.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteItemVm {
    /// O título (ou «Sem título», traduzido pelo Code).
    pub title: String,
    /// Alterada, formatado.
    pub modified: String,
    /// A pasta ou o contexto, quando ajuda.
    pub context: Option<String>,
    /// Abrir.
    pub href: String,
    /// A aberta.
    pub active: bool,
    /// Partilhada.
    pub shared: bool,
}

/// O editor de uma nota. Modelo: texto estruturado (Markdown restrito) num
/// `textarea`; a barra insere sintaxe. Nunca HTML guardado ou interpretado.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteEditorVm {
    /// Identificador (para o formulário).
    pub id: String,
    /// O título.
    pub title: String,
    /// O texto.
    pub body: String,
    /// A revisão em que o editor abriu (`base_revision`).
    pub revision: i32,
    /// Onde guardar (POST).
    pub save_action: String,
    /// O estado de gravação.
    pub save: AppSaveState,
    /// Só leitura (partilhada sem escrita, ou no lixo).
    pub read_only: bool,
    /// Mover para o lixo (POST), quando o Core o permite.
    pub trash_action: Option<String>,
    /// Revisões, quando o Core as serve.
    pub revisions_href: Option<String>,
    /// A Nye sobre esta nota.
    pub nye: Option<AppNyeVm>,
    /// O rodapé factual («1 240 palavras»), formatado pelo Code.
    pub stats: Option<String>,
}

/// A aplicação Notas.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotesVm {
    /// A navegação (As minhas, Partilhadas, Lixo).
    pub nav: Vec<AppNavVm>,
    /// A pesquisa (âmbito: Notas).
    pub query: String,
    /// A lista.
    pub items: Vec<NoteItemVm>,
    /// O estado da lista.
    pub load: AppLoad,
    /// Paginação.
    pub page: AppPageVm,
    /// A nota aberta.
    pub editor: Option<NoteEditorVm>,
    /// O erro da nota aberta (revogada, não existe).
    pub editor_error: Option<AppError>,
    /// Criar (POST `/notes/new`), quando se pode.
    pub create_action: Option<String>,
}

// ── Calendário ──

/// A vista do calendário.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CalView {
    /// Mês.
    Month,
    /// Semana.
    Week,
    /// Dia.
    Day,
    /// Agenda (lista; a vista por omissão no móvel).
    Agenda,
}

/// O âmbito de um evento (`ocinye_contracts::calendar::EventScope`): dá o tom.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CalScope {
    /// Pessoal.
    Personal,
    /// Unidade.
    Unit,
    /// Espaço de investigação.
    Workspace,
    /// Instituição.
    Institution,
}

/// Um evento posicionado. As posições são calculadas pelo Code no fuso do
/// membro (a vista não converte fusos).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CalEventVm {
    /// Identificador.
    pub id: String,
    /// O título.
    pub title: String,
    /// «09:30–10:30» ou «Todo o dia».
    pub time: String,
    /// O local.
    pub location: Option<String>,
    /// O âmbito.
    pub scope: CalScope,
    /// Cancelado (fica riscado; não desaparece).
    pub cancelled: bool,
    /// Todo o dia.
    pub all_day: bool,
    /// Semana/Dia: minuto de início (0–1439) e duração em minutos.
    pub span: Option<(u16, u16)>,
    /// Semana/Dia: coluna de sobreposição (`lane`, `lanes`).
    pub lane: (u8, u8),
    /// Abrir.
    pub href: String,
    /// O fuso original, quando difere do do membro («Europe/Lisbon · 10:30»).
    pub origin_tz: Option<String>,
}

/// Um dia de uma grelha (mês) ou uma coluna (semana/dia).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CalDayVm {
    /// «29», ou «Ter 29» nas colunas.
    pub label: String,
    /// A data para o leitor de ecrã («terça-feira, 29 de setembro»).
    pub full: String,
    /// Hoje.
    pub today: bool,
    /// Fora do mês mostrado.
    pub outside: bool,
    /// Seleccionado.
    pub selected: bool,
    /// Os eventos (o mês mostra até 3 e «+N»).
    pub events: Vec<CalEventVm>,
    /// Quantos não cabem («+2»), no mês.
    pub more: u16,
    /// Abrir o dia.
    pub href: String,
    /// Criar neste dia.
    pub new_href: Option<String>,
}

/// O detalhe de um evento.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CalEventDetailsVm {
    /// O evento.
    pub event: CalEventVm,
    /// A data longa.
    pub date: String,
    /// A descrição (texto simples).
    pub description: Option<String>,
    /// Participantes (nomes).
    pub participants: Vec<String>,
    /// O contexto (projecto, unidade).
    pub context: Option<String>,
    /// Editar, quando se pode.
    pub edit_href: Option<String>,
    /// Cancelar o evento (POST), quando se pode.
    pub cancel_action: Option<String>,
    /// A Nye sobre este evento.
    pub nye: Option<AppNyeVm>,
}

/// O formulário de criação/edição.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CalFormVm {
    /// Para onde envia (POST).
    pub action: String,
    /// Valores (`datetime-local` / `date`), no fuso do membro.
    pub title: String,
    /// Início.
    pub start: String,
    /// Fim.
    pub end: String,
    /// Todo o dia.
    pub all_day: bool,
    /// O local.
    pub location: String,
    /// A descrição.
    pub description: String,
    /// Os âmbitos em que o membro pode criar (valor, rótulo).
    pub scopes: Vec<(String, String)>,
    /// O âmbito escolhido.
    pub scope: String,
    /// Erro de validação (chave de catálogo).
    pub error: Option<&'static str>,
}

/// A aplicação Calendário.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CalendarVm {
    /// A vista.
    pub view: CalView,
    /// O título do intervalo («setembro de 2026», «28 set – 4 out»).
    pub range_label: String,
    /// Anterior / seguinte / hoje.
    pub prev_href: String,
    /// Seguinte.
    pub next_href: String,
    /// Hoje.
    pub today_href: String,
    /// Os cabeçalhos dos dias da semana (mês), já na língua e no primeiro dia local.
    pub weekdays: Vec<String>,
    /// Os dias (mês: 35 ou 42; semana: 7; dia: 1).
    pub days: Vec<CalDayVm>,
    /// Agenda: grupos (data, eventos).
    pub agenda: Vec<(String, Vec<CalEventVm>)>,
    /// Semana/Dia: minuto actual, quando hoje está visível.
    pub now_minute: Option<u16>,
    /// Semana/Dia: as horas mostradas (primeira, última).
    pub hours: (u8, u8),
    /// O fuso do membro («Africa/Luanda · WAT»), mostrado quando é relevante.
    pub timezone: Option<String>,
    /// O estado.
    pub load: AppLoad,
    /// O evento aberto.
    pub details: Option<CalEventDetailsVm>,
    /// O formulário aberto.
    pub form: Option<CalFormVm>,
    /// Criar, quando se pode.
    pub new_href: Option<String>,
}

// ── Correio ──

/// Uma pasta de correio (`ocinye_contracts::mail::MailFolder`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MailFolderVm {
    /// Entrada.
    Inbox,
    /// Favoritos.
    Starred,
    /// Rascunhos.
    Drafts,
    /// Enviados.
    Sent,
    /// Arquivados.
    Archive,
    /// Spam.
    Spam,
    /// Lixo.
    Trash,
}

/// Uma caixa de correio ligada (`MailboxKind`: pessoal ou partilhada).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MailboxVm {
    /// O nome ou endereço.
    pub label: String,
    /// As pastas, com contagens reais.
    pub folders: Vec<(MailFolderVm, AppNavVm)>,
    /// Sincronizar (POST), quando se pode.
    pub sync_action: Option<String>,
    /// A última sincronização, formatada.
    pub synced: Option<String>,
}

/// Uma mensagem na lista.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MailItemVm {
    /// Identificador.
    pub id: String,
    /// O remetente (ou os destinatários, em Enviados).
    pub from: String,
    /// O assunto.
    pub subject: String,
    /// O início do texto, em texto simples.
    pub snippet: String,
    /// Quando, formatado.
    pub at: String,
    /// Por ler.
    pub unread: bool,
    /// Favorito.
    pub starred: bool,
    /// Tem anexos.
    pub attachments: bool,
    /// Seleccionada (caixa).
    pub selected: bool,
    /// Aberta no painel de leitura.
    pub open: bool,
    /// Abrir.
    pub href: String,
    /// Destinatário externo à instituição (`RecipientScope`).
    pub external: bool,
}

/// Um anexo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MailAttachmentVm {
    /// O nome.
    pub name: String,
    /// O tamanho.
    pub size: String,
    /// O tipo.
    pub kind: FileKind,
    /// Descarregar (mensagem) ou retirar (POST, rascunho).
    pub href: Option<String>,
    /// Retirar (POST), no rascunho.
    pub remove_action: Option<String>,
    /// Guardar em Ficheiros (POST), na mensagem.
    pub save_action: Option<String>,
}

/// Uma mensagem aberta. O corpo é conteúdo externo não confiável: chega
/// já sanitizado pelo Code como blocos de texto; imagens remotas bloqueadas por
/// omissão (`RemoteContentPolicy`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MailMessageVm {
    /// O assunto.
    pub subject: String,
    /// O remetente («Nome <endereço>»).
    pub from: String,
    /// Para.
    pub to: Vec<String>,
    /// Cc.
    pub cc: Vec<String>,
    /// A data longa.
    pub date: String,
    /// Parágrafos em texto simples (nunca HTML).
    pub body: Vec<String>,
    /// Há conteúdo remoto bloqueado.
    pub remote_blocked: bool,
    /// Mostrar o conteúdo remoto (GET), quando a política o permite.
    pub remote_href: Option<String>,
    /// Anexos.
    pub attachments: Vec<MailAttachmentVm>,
    /// Responder, Responder a todos, Reencaminhar (`ComposeAction`).
    pub reply_href: Option<String>,
    /// Responder a todos.
    pub reply_all_href: Option<String>,
    /// Reencaminhar.
    pub forward_href: Option<String>,
    /// Marcar por ler / favorito / arquivar / lixo (POST `/flags`).
    pub flags_action: Option<String>,
    /// A Nye sobre esta mensagem.
    pub nye: Option<AppNyeVm>,
}

/// O estado de envio (`OutboxState`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MailSendState {
    /// Nada enviado.
    Idle,
    /// Na fila do Core.
    Queued,
    /// A entregar ao servidor.
    Sending,
    /// Enviado.
    Sent,
    /// Não enviado (o rascunho fica).
    Failed,
}

/// O compositor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MailComposeVm {
    /// O rascunho, quando já existe.
    pub draft_id: Option<String>,
    /// Guardar rascunho (POST).
    pub save_action: String,
    /// Enviar (POST): passa pela capacidade de comunicação externa do Core.
    pub send_action: String,
    /// Para, Cc, Bcc, Assunto, Texto.
    pub to: String,
    /// Cc.
    pub cc: String,
    /// Bcc.
    pub bcc: String,
    /// Assunto.
    pub subject: String,
    /// O texto (Markdown restrito, como Notas).
    pub body: String,
    /// Anexos.
    pub attachments: Vec<MailAttachmentVm>,
    /// Escolher de Ficheiros (superfície interna).
    pub attach_href: Option<String>,
    /// O estado do rascunho.
    pub save: AppSaveState,
    /// O estado de envio.
    pub send: MailSendState,
    /// Destinatários externos à instituição (aviso factual).
    pub external_count: u32,
    /// A Nye pode preparar o texto (abre a Nye; nunca envia).
    pub nye: Option<AppNyeVm>,
    /// O erro de envio, quando falhou.
    pub error: Option<AppError>,
}

/// A aplicação Correio.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MailVm {
    /// As caixas ligadas. Vazio = nenhuma caixa (ligar em Definições).
    pub mailboxes: Vec<MailboxVm>,
    /// Ligar uma caixa (quando não há nenhuma).
    pub connect_href: Option<String>,
    /// A pasta actual.
    pub folder: MailFolderVm,
    /// O nome da pasta actual, já traduzido.
    pub folder_label: String,
    /// A pesquisa (âmbito: esta caixa).
    pub query: String,
    /// A lista.
    pub items: Vec<MailItemVm>,
    /// O estado da lista.
    pub load: AppLoad,
    /// Paginação.
    pub page: AppPageVm,
    /// A mensagem aberta.
    pub message: Option<MailMessageVm>,
    /// O erro da mensagem aberta.
    pub message_error: Option<AppError>,
    /// O compositor aberto.
    pub compose: Option<MailComposeVm>,
    /// Escrever.
    pub compose_href: String,
}
