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
