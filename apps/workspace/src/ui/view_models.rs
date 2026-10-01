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
    /// D010: a Distribuição **fixa** do ponto de acesso deste pedido (S08). Num
    /// ponto genérico, nenhuma antes da entrada (S07).
    pub distribution: Option<Distribution>,
    /// O estado da Instância, se foi sondado neste pedido. `None` não afirma nada.
    pub core: Option<Health>,
    /// D010: o nome da Instância servida por este ponto (S07/S08). `None` fora
    /// de uma Instância (S13, S14, S36).
    pub instance: Option<String>,
    /// D010: o endereço deste ponto de acesso.
    pub host: Option<String>,
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
    /// Code (D009) · Os ids fixados, pela ordem em que estão fixados (as do
    /// membro, ou as da Distribuição). A barra desenha as fixações por esta
    /// ordem; `apps` continua pela ordem do registo (o lançador). Vazio = a
    /// ordem de `apps` (comportamento anterior).
    pub pin_order: Vec<&'static str>,
    /// D010 · S15: mudar de Distribuição, no painel do distintivo — só com
    /// mais de uma acessível. `None`: só uma (nada a mudar).
    pub dist_switch: Option<DistSwitchVm>,
    /// D010 · S19–S21: o chip de contexto, dentro da Distribuição activa.
    pub context: Option<ContextVm>,
}

/// D010 · A mudança de Distribuição (S15, S17).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct DistSwitchVm {
    /// As acessíveis, a activa incluída, pela ordem do produto.
    pub choices: Vec<Distribution>,
    /// Num ponto fixo não se muda aqui: os endereços **configurados** por onde
    /// se entra nas outras (S17-bound). `None` num ponto genérico.
    pub bound: Option<BoundSwitchVm>,
}

/// D010 · S17-bound: num ponto fixo, onde entrar nas outras Distribuições.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct BoundSwitchVm {
    /// O anfitrião genérico configurado, se houver (o texto do aviso).
    pub generic_host: Option<String>,
    /// `(Distribuição, anfitrião, URL)` dos pontos configurados das outras —
    /// e o genérico, se não houver ponto fixo da outra.
    pub targets: Vec<(Distribution, String, String)>,
}

/// D010 · O contexto activo e os que o membro pode usar (ADR-0625).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ContextVm {
    /// O activo, ou `None` (o chip pede para escolher).
    pub active: Option<ContextItemVm>,
    /// Os que o membro pode usar.
    pub items: Vec<ContextItemVm>,
    /// S21: o contexto activo deixou de ser do membro (o nome dele).
    pub revoked: Option<String>,
}

/// Um contexto: `organisation` · `unit` · `project` · `personal`.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ContextItemVm {
    /// O tipo.
    pub kind: String,
    /// A unidade ou o projecto.
    pub id: Option<String>,
    /// Como se mostra.
    pub name: String,
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
    /// D009 · Campo — predefinido de Research.
    Field,
    /// D009 · Módulo — predefinido de Business.
    Module,
    /// D009 · Calma — predefinido de Personal.
    Calm,
    /// D009 · Trama — predefinido de Education.
    Lattice,
}

impl Wallpaper {
    /// Todos, pela ordem da escolha.
    pub const ALL: [Self; 10] = [
        Self::Ocinye,
        Self::Dusk,
        Self::Institutional,
        Self::Mist,
        Self::Slate,
        Self::Sand,
        Self::Field,
        Self::Module,
        Self::Calm,
        Self::Lattice,
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
            Self::Field => "field",
            Self::Module => "module",
            Self::Calm => "calm",
            Self::Lattice => "lattice",
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
    /// D009 · A predefinição mínima do sistema: só quando a Distribuição não
    /// se conhece (`distribution::SystemFallback`). Sem widgets.
    #[default]
    System,
    /// D009 · A predefinição da Distribuição, versionada
    /// (`distribution::DISTRIBUTION_DEFAULTS_VERSION`). Ninguém a publicou:
    /// `name` e `published` ficam vazios; `version` mostra-se.
    Distribution,
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
    /// D010 · S16: a decisão desta janela faz parte de uma mudança de
    /// Distribuição (`switch:<d>`); «Cancelar» aborta a mudança.
    pub after: Option<String>,
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
    /// D004.1 · A conta existe no Core mas não está ligada (`connected = false`):
    /// há dados guardados, mas não se lê nem envia. Não é uma falha passageira.
    NotConnected,
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
            Self::NotConnected => "app.err.not_connected",
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

// ═════════════════════════════════════════════════════════════════════════
// D005 · Investigação e trabalho: Projectos · O Meu Trabalho (tarefas) ·
// Ideias · Dados · Conhecimento (documentos e bibliografia). Aditivo.
//
// A vista mostra o que o Core já autorizou e só as acções que ele devolve
// (`available_transitions`, `may_create`). Estar num ambiente não dá
// autoridade; um identificador nunca aparece ao membro.
// ═════════════════════════════════════════════════════════════════════════

/// A classificação de um recurso (`ocinye_contracts::Classification`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResClassification {
    /// Público.
    Public,
    /// Interno.
    Internal,
    /// Confidencial.
    Confidential,
    /// Restrito.
    Restricted,
}

impl ResClassification {
    /// A chave de catálogo.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Public => "res.class.public",
            Self::Internal => "res.class.internal",
            Self::Confidential => "res.class.confidential",
            Self::Restricted => "res.class.restricted",
        }
    }
}

/// O tom de um estado: dá a cor, nunca o significado (o texto vai sempre).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResTone {
    /// Ainda não começou (rascunho, por fazer, descoberta).
    Neutral,
    /// Em curso.
    Progress,
    /// Precisa de atenção (bloqueada, em espera, em revisão).
    Attention,
    /// Concluído ou publicado.
    Done,
    /// Fechado sem desfecho positivo (cancelada, rejeitada, arquivada, retirada).
    Closed,
}

/// Um estado, já com rótulo e tom.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResStateVm {
    /// A chave do rótulo (`res.project.state.active`…).
    pub key: &'static str,
    /// O tom.
    pub tone: ResTone,
}

/// O tipo de um recurso ligado. Escolhe o ícone e o rótulo do tipo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResKind {
    /// Projecto.
    Project,
    /// Ideia.
    Idea,
    /// Tarefa.
    Task,
    /// Dataset.
    Dataset,
    /// Versão de um dataset.
    DatasetVersion,
    /// Entrada bibliográfica / fonte.
    Source,
    /// Documento.
    Document,
    /// Nota.
    Note,
    /// Ficheiro.
    File,
    /// Qualquer outro objecto científico (resultado, estudo…), pelo rótulo do Code.
    Other,
}

/// A relação tipada (`research_links.relation`, vocabulário fechado).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResRelation {
    /// cites
    Cites,
    /// supports
    Supports,
    /// refutes
    Refutes,
    /// derived_from
    DerivedFrom,
    /// uses
    Uses,
    /// produces
    Produces,
    /// relates_to
    RelatesTo,
    /// tests
    Tests,
    /// follows
    Follows,
    /// input_to
    InputTo,
    /// produced_by
    ProducedBy,
    /// executed_on
    ExecutedOn,
    /// validates
    Validates,
    /// reproduces
    Reproduces,
    /// supersedes
    Supersedes,
}

impl ResRelation {
    /// A chave do rótulo, no sentido de quem lê (`res.rel.cites`).
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Cites => "res.rel.cites",
            Self::Supports => "res.rel.supports",
            Self::Refutes => "res.rel.refutes",
            Self::DerivedFrom => "res.rel.derived_from",
            Self::Uses => "res.rel.uses",
            Self::Produces => "res.rel.produces",
            Self::RelatesTo => "res.rel.relates_to",
            Self::Tests => "res.rel.tests",
            Self::Follows => "res.rel.follows",
            Self::InputTo => "res.rel.input_to",
            Self::ProducedBy => "res.rel.produced_by",
            Self::ExecutedOn => "res.rel.executed_on",
            Self::Validates => "res.rel.validates",
            Self::Reproduces => "res.rel.reproduces",
            Self::Supersedes => "res.rel.supersedes",
        }
    }
}

/// Uma ligação a um recurso canónico de outra aplicação (ou da mesma).
///
/// Só existe se o Core resolveu **as duas pontas** para este membro
/// (ADR-0306): uma aresta cujo extremo não se alcança não chega à vista.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResLinkVm {
    /// O tipo.
    pub kind: ResKind,
    /// O rótulo do tipo, quando `kind` é `Other` («Resultado»), já traduzido.
    pub kind_label: Option<String>,
    /// O título.
    pub title: String,
    /// Uma linha de metadata curta («v3 · publicada», «Em curso»).
    pub meta: Option<String>,
    /// A relação, quando vem de `research_links`.
    pub relation: Option<ResRelation>,
    /// A relação foi declarada por uma pessoa (`declared`) ou pela operação.
    pub by_operation: bool,
    /// A ligação profunda canónica. `None` = o recurso existe mas não tem ecrã.
    pub href: Option<String>,
}

/// Uma transição permitida agora (`available_transitions` do Core).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResTransitionVm {
    /// O valor enviado (`state=done`).
    pub value: &'static str,
    /// O rótulo do botão («Concluir», «Reabrir», «Pôr em espera»).
    pub label_key: &'static str,
    /// Exige um motivo registado (fechar uma ideia).
    pub requires_note: bool,
    /// A acção principal (uma só por recurso).
    pub primary: bool,
}

/// O bloco de transições de um recurso: um formulário, POST ao Core.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResTransitionsVm {
    /// Para onde envia (`/tasks/{id}/transitions`, pela BFF).
    pub action: String,
    /// As transições.
    pub options: Vec<ResTransitionVm>,
}

/// Uma pessoa, pelo nome (nunca pelo identificador).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResPersonVm {
    /// O nome.
    pub name: String,
    /// O papel no ambiente, já traduzido («Responsável», «Membro», «Leitura»).
    pub role: Option<String>,
}

/// Uma opção de um selector governado (ambiente, unidade, pessoa candidata).
/// A lista vem do Core já filtrada: o browser nunca enumera sozinho.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResOptionVm {
    /// O valor opaco do formulário.
    pub value: String,
    /// O rótulo.
    pub label: String,
    /// Escolhida.
    pub selected: bool,
}

/// A vista em duas partes das cinco aplicações: lista e detalhe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResPane {
    /// A lista (em janela estreita, é o ecrã).
    List,
    /// O detalhe ou o formulário (em janela estreita, é o ecrã).
    Detail,
}

/// Uma linha de uma lista das cinco aplicações.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResItemVm {
    /// O título.
    pub title: String,
    /// O código institucional (`PRJ-2026-014`), quando o domínio o tem.
    pub code: Option<String>,
    /// O estado.
    pub state: Option<ResStateVm>,
    /// Colunas curtas, já formatadas, pela ordem de `ResListVm.columns`.
    pub cells: Vec<Option<String>>,
    /// Uma data vencida (tarefas): o texto já o diz; isto dá o tom.
    pub overdue: bool,
    /// A prioridade (tarefas), com texto e ícone, nunca só cor.
    pub priority: Option<TaskPriorityLevel>,
    /// Onde abre (a mesma janela; ADR-0620).
    pub href: String,
    /// É o que está aberto.
    pub active: bool,
}

/// Uma lista densa: colunas com prioridade. A primeira é sempre o título.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResListVm {
    /// As chaves dos cabeçalhos das colunas extra, por prioridade decrescente.
    pub columns: Vec<&'static str>,
    /// As linhas.
    pub items: Vec<ResItemVm>,
    /// O estado da lista.
    pub load: AppLoad,
    /// Paginação (o Core pagina por página; o Code faz de `page+1` o cursor).
    pub page: AppPageVm,
}

// ── Projectos ──

/// `ocinye_contracts::ProjectState`, na vista.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectStatus {
    /// draft
    Draft,
    /// active
    Active,
    /// on_hold
    OnHold,
    /// completed
    Completed,
    /// archived
    Archived,
}

/// O projecto aberto.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectVm {
    /// Código.
    pub code: String,
    /// Título.
    pub title: String,
    /// Estado.
    pub state: ProjectStatus,
    /// Classificação efectiva.
    pub classification: ResClassification,
    /// Resumo (texto simples; nunca HTML).
    pub summary: Option<String>,
    /// Objectivos (texto simples).
    pub objectives: Option<String>,
    /// A unidade.
    pub unit: Option<String>,
    /// O responsável.
    pub responsible: Option<String>,
    /// Início, já formatado.
    pub started: Option<String>,
    /// Conclusão, já formatada.
    pub completed: Option<String>,
    /// A ideia de origem (linhagem; nunca reescrita).
    pub origin_idea: Option<ResLinkVm>,
    /// Os membros do ambiente (só leitura aqui).
    pub members: Vec<ResPersonVm>,
    /// As tarefas em aberto do ambiente (as primeiras; o resto em «O Meu Trabalho»).
    pub tasks: Vec<ResLinkVm>,
    /// Todas as tarefas do projecto (`/my-work?workspace=…`).
    pub tasks_href: Option<String>,
    /// Criar uma tarefa neste projecto (quando `may_create`).
    pub new_task_href: Option<String>,
    /// Os datasets do ambiente.
    pub datasets: Vec<ResLinkVm>,
    /// Bibliografia e documentos do ambiente.
    pub knowledge: Vec<ResLinkVm>,
    /// Relações tipadas (`research_links`).
    pub links: Vec<ResLinkVm>,
    /// As transições permitidas.
    pub transitions: Option<ResTransitionsVm>,
    /// A Nye contextual.
    pub nye: Option<AppNyeVm>,
}

/// A aplicação Projectos.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectsVm {
    /// Em curso · Os meus · Todos (filtros reais de `/workspaces?kind=project`).
    pub nav: Vec<AppNavVm>,
    /// A lista.
    pub list: ResListVm,
    /// O que se mostra em janela estreita.
    pub pane: ResPane,
    /// O projecto aberto.
    pub project: Option<ProjectVm>,
    /// O erro do projecto aberto (revogado, não encontrado).
    pub project_error: Option<AppError>,
    /// As Ideias, onde um projecto nasce (promoção), quando o membro as vê.
    pub ideas_href: Option<String>,
    /// A lista, com os filtros correntes (o «voltar» da janela estreita).
    pub list_href: String,
}

// ── O Meu Trabalho (tarefas) ──

/// `ocinye_contracts::TaskState`, na vista.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskStatus {
    /// todo
    Todo,
    /// in_progress
    InProgress,
    /// blocked
    Blocked,
    /// in_review
    InReview,
    /// done
    Done,
    /// cancelled
    Cancelled,
}

/// A prioridade de uma tarefa.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskPriorityLevel {
    /// low
    Low,
    /// normal
    Normal,
    /// high
    High,
    /// critical
    Critical,
}

impl TaskPriorityLevel {
    /// A chave do rótulo.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Low => "work.prio.low",
            Self::Normal => "work.prio.normal",
            Self::High => "work.prio.high",
            Self::Critical => "work.prio.critical",
        }
    }
}

/// Atribuir: só entre candidatos que o Core devolveu (membros do ambiente).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskAssignVm {
    /// Para onde envia.
    pub action: String,
    /// Os candidatos (e «Ninguém», com valor vazio, se retirar for permitido).
    pub candidates: Vec<ResOptionVm>,
}

/// A tarefa aberta.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskVm {
    /// Título.
    pub title: String,
    /// Estado.
    pub state: TaskStatus,
    /// Prioridade.
    pub priority: TaskPriorityLevel,
    /// Classificação.
    pub classification: ResClassification,
    /// Descrição (texto simples).
    pub description: Option<String>,
    /// O prazo (uma data, no fuso do membro), já formatado.
    pub due: Option<String>,
    /// Vencida (o Code decide no fuso do membro).
    pub overdue: bool,
    /// A pessoa atribuída.
    pub assignee: Option<String>,
    /// O ambiente (projecto ou ideia) a que pertence.
    pub workspace: Option<ResLinkVm>,
    /// Fechada em, já formatado.
    pub closed: Option<String>,
    /// Transições.
    pub transitions: Option<ResTransitionsVm>,
    /// Atribuir.
    pub assign: Option<TaskAssignVm>,
    /// Relações.
    pub links: Vec<ResLinkVm>,
    /// Nye.
    pub nye: Option<AppNyeVm>,
}

/// Criar uma tarefa (`POST /workspaces/{id}/tasks`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskFormVm {
    /// Para onde envia (a BFF escolhe o ambiente a partir de `workspace`).
    pub action: String,
    /// Os ambientes onde o membro pode criar (`may_create`).
    pub workspaces: Vec<ResOptionVm>,
    /// Título (devolvido em caso de erro).
    pub title: String,
    /// Descrição.
    pub description: String,
    /// Prioridade escolhida.
    pub priority: TaskPriorityLevel,
    /// Prazo (`AAAA-MM-DD`).
    pub due: String,
    /// O erro de validação/gravação.
    pub error: Option<AppError>,
    /// Cancelar (volta à lista).
    pub cancel_href: String,
}

/// A aplicação O Meu Trabalho.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkVm {
    /// Atribuídas a mim · Em aberto · Todas as que vejo.
    pub nav: Vec<AppNavVm>,
    /// Filtrar por ambiente (`workspace_id`), só ambientes autorizados.
    pub workspace_filter: Vec<ResOptionVm>,
    /// A lista.
    pub list: ResListVm,
    /// O painel em janela estreita.
    pub pane: ResPane,
    /// A tarefa aberta.
    pub task: Option<TaskVm>,
    /// O erro da tarefa aberta.
    pub task_error: Option<AppError>,
    /// O formulário de criação aberto.
    pub form: Option<TaskFormVm>,
    /// «Nova tarefa», quando há pelo menos um ambiente `may_create`.
    pub new_href: Option<String>,
    /// A lista, com os filtros correntes (o «voltar» da janela estreita).
    pub list_href: String,
}

// ── Ideias ──

/// `ocinye_contracts::IdeaState`, na vista.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdeaStage {
    /// discovery
    Discovery,
    /// exploration
    Exploration,
    /// concept
    Concept,
    /// review
    Review,
    /// project_candidate
    ProjectCandidate,
    /// promoted
    Promoted,
    /// rejected
    Rejected,
    /// archived
    Archived,
}

/// Promover a projecto: cria o projecto no **mesmo** ambiente; a ideia fica.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdeaPromotionVm {
    /// Para onde envia.
    pub action: String,
    /// Responsáveis possíveis (membros do ambiente).
    pub responsible: Vec<ResOptionVm>,
}

/// A ideia aberta.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdeaVm {
    /// Título.
    pub title: String,
    /// Estado.
    pub state: IdeaStage,
    /// Classificação.
    pub classification: ResClassification,
    /// A unidade.
    pub unit: Option<String>,
    /// Resumo.
    pub summary: Option<String>,
    /// Pergunta de investigação.
    pub research_question: Option<String>,
    /// Hipótese.
    pub hypothesis: Option<String>,
    /// Motivação.
    pub motivation: Option<String>,
    /// Palavras-chave.
    pub keywords: Vec<String>,
    /// O motivo registado ao rejeitar/arquivar.
    pub outcome_note: Option<String>,
    /// O projecto em que foi promovida.
    pub promoted_project: Option<ResLinkVm>,
    /// Transições.
    pub transitions: Option<ResTransitionsVm>,
    /// Promover (só `promotable`).
    pub promotion: Option<IdeaPromotionVm>,
    /// Membros do ambiente.
    pub members: Vec<ResPersonVm>,
    /// Relações.
    pub links: Vec<ResLinkVm>,
    /// Nye.
    pub nye: Option<AppNyeVm>,
    /// Criada por / quando, já formatado.
    pub created: Option<String>,
}

/// Registar uma ideia (`POST /ideas`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdeaFormVm {
    /// Para onde envia.
    pub action: String,
    /// As unidades onde pode registar.
    pub units: Vec<ResOptionVm>,
    /// As classificações que pode escolher.
    pub classifications: Vec<ResOptionVm>,
    /// Título.
    pub title: String,
    /// Resumo.
    pub summary: String,
    /// Pergunta.
    pub research_question: String,
    /// Hipótese.
    pub hypothesis: String,
    /// Motivação.
    pub motivation: String,
    /// Palavras-chave, separadas por vírgulas.
    pub keywords: String,
    /// Erro.
    pub error: Option<AppError>,
    /// Cancelar.
    pub cancel_href: String,
}

/// A aplicação Ideias.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdeasVm {
    /// Em desenvolvimento · Candidatas · Promovidas · Encerradas · As minhas.
    pub nav: Vec<AppNavVm>,
    /// A lista.
    pub list: ResListVm,
    /// O painel.
    pub pane: ResPane,
    /// A ideia aberta.
    pub idea: Option<IdeaVm>,
    /// Erro.
    pub idea_error: Option<AppError>,
    /// Formulário.
    pub form: Option<IdeaFormVm>,
    /// «Nova ideia».
    pub new_href: Option<String>,
    /// A lista, com os filtros correntes (o «voltar» da janela estreita).
    pub list_href: String,
}

// ── Dados ──

/// `datasets.state`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DatasetStatus {
    /// draft
    Draft,
    /// active
    Active,
    /// deprecated
    Deprecated,
    /// archived
    Archived,
}

/// `dataset_versions.status`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DatasetVersionStatus {
    /// draft
    Draft,
    /// published
    Published,
    /// withdrawn
    Withdrawn,
}

/// Um ficheiro de uma versão: caminho lógico, nunca a chave do objecto.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DatasetFileVm {
    /// O caminho lógico dentro do dataset (`medicoes/2026-09.csv`).
    pub path: String,
    /// O tamanho, já formatado na língua.
    pub size: Option<String>,
    /// O recurso canónico de Ficheiros, quando existe (`None` hoje: FG).
    pub href: Option<String>,
}

/// Uma versão do dataset (não é a versão de um ficheiro).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DatasetVersionVm {
    /// «v3».
    pub label: String,
    /// Estado.
    pub status: DatasetVersionStatus,
    /// Publicada em, já formatado.
    pub published: Option<String>,
    /// Notas da versão.
    pub notes: Option<String>,
    /// Como foi produzida (texto de proveniência).
    pub provenance: Option<String>,
    /// De que versão deriva.
    pub derived_from: Option<String>,
    /// Motivo da retirada.
    pub withdrawn_reason: Option<String>,
    /// «4 ficheiros · 1,2 GB».
    pub totals: Option<String>,
    /// Os ficheiros (os primeiros; paginação no Code).
    pub files: Vec<DatasetFileVm>,
    /// Acrescentar ficheiros (só versão em rascunho e `may_create`).
    pub add_file_action: Option<String>,
    /// Publicar (só rascunho).
    pub publish_action: Option<String>,
    /// É a versão mostrada.
    pub open: bool,
    /// Onde abre.
    pub href: String,
}

/// O dataset aberto.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DatasetVm {
    /// Código.
    pub code: String,
    /// Título.
    pub title: String,
    /// Estado.
    pub state: DatasetStatus,
    /// Classificação.
    pub classification: ResClassification,
    /// Descrição.
    pub description: Option<String>,
    /// A origem, já traduzida («Recolhido pelo Ocinye», «Derivado»…).
    pub origin: String,
    /// Licença.
    pub licence: Option<String>,
    /// Restrições de uso (texto humano).
    pub usage_restrictions: Option<String>,
    /// Palavras-chave.
    pub keywords: Vec<String>,
    /// Responsável.
    pub responsible: Option<String>,
    /// Data de aquisição.
    pub acquired: Option<String>,
    /// O ambiente.
    pub workspace: Option<ResLinkVm>,
    /// As versões, da mais recente para a mais antiga.
    pub versions: Vec<DatasetVersionVm>,
    /// Nova versão (`POST …/versions`).
    pub new_version_action: Option<String>,
    /// Relações.
    pub links: Vec<ResLinkVm>,
    /// Nye.
    pub nye: Option<AppNyeVm>,
}

/// Registar um dataset (`POST /workspaces/{id}/datasets`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DatasetFormVm {
    /// Para onde envia.
    pub action: String,
    /// Ambientes `may_create`.
    pub workspaces: Vec<ResOptionVm>,
    /// As origens (vocabulário fechado, traduzido).
    pub origins: Vec<ResOptionVm>,
    /// Código.
    pub code: String,
    /// Título.
    pub title: String,
    /// Descrição.
    pub description: String,
    /// Licença.
    pub licence: String,
    /// Restrições de uso.
    pub usage_restrictions: String,
    /// Palavras-chave.
    pub keywords: String,
    /// Erro.
    pub error: Option<AppError>,
    /// Cancelar.
    pub cancel_href: String,
}

/// A aplicação Dados.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DatasetsVm {
    /// Filtrar por ambiente.
    pub workspace_filter: Vec<ResOptionVm>,
    /// A lista.
    pub list: ResListVm,
    /// O painel.
    pub pane: ResPane,
    /// O dataset aberto.
    pub dataset: Option<DatasetVm>,
    /// Erro.
    pub dataset_error: Option<AppError>,
    /// Formulário.
    pub form: Option<DatasetFormVm>,
    /// «Novo dataset».
    pub new_href: Option<String>,
    /// A lista, com os filtros correntes (o «voltar» da janela estreita).
    pub list_href: String,
}

// ── Conhecimento ──

/// A secção de Conhecimento. A aplicação Bibliografia abre a mesma vista em `Sources`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KnowledgeSection {
    /// Documentos (afirmações sobre um ficheiro).
    Documents,
    /// Bibliografia (`sources`).
    Sources,
}

/// `sources.content_right`: a base legal para guardar conteúdo integral.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContentRight {
    /// metadata_only
    MetadataOnly,
    /// open_licence
    OpenLicence,
    /// institutional_licence
    InstitutionalLicence,
    /// authored_by_ocinye
    AuthoredByOcinye,
    /// public_domain
    PublicDomain,
    /// permission_granted
    PermissionGranted,
}

/// Uma entrada bibliográfica aberta.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceVm {
    /// O tipo, já traduzido («Artigo»).
    pub source_type: String,
    /// Título (texto externo: dados).
    pub title: String,
    /// Autores.
    pub authors: Vec<String>,
    /// Ano.
    pub year: Option<String>,
    /// Publicação.
    pub container_title: Option<String>,
    /// Editora.
    pub publisher: Option<String>,
    /// DOI.
    pub doi: Option<String>,
    /// ISBN.
    pub isbn: Option<String>,
    /// URL externo (mostrado como texto; abre fora do Ocinye).
    pub url: Option<String>,
    /// Resumo (texto externo não confiável; nunca HTML).
    pub abstract_text: Option<String>,
    /// Palavras-chave.
    pub keywords: Vec<String>,
    /// Licença.
    pub licence: Option<String>,
    /// Base legal.
    pub content_right: ContentRight,
    /// O documento com o texto integral, quando há base legal.
    pub full_text: Option<ResLinkVm>,
    /// Chave de citação.
    pub citation_key: Option<String>,
    /// Origem do registo («Importado de BibTeX»).
    pub origin: Option<String>,
    /// Classificação.
    pub classification: ResClassification,
    /// O ambiente.
    pub workspace: Option<ResLinkVm>,
    /// O que cita, apoia ou refuta esta fonte, e o que ela apoia.
    pub links: Vec<ResLinkVm>,
    /// Nye.
    pub nye: Option<AppNyeVm>,
}

/// Um documento aberto. O conteúdo não viaja: metadata e transferência.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KnowledgeDocumentVm {
    /// Título.
    pub title: String,
    /// O tipo, já traduzido.
    pub kind: String,
    /// Descrição.
    pub description: Option<String>,
    /// Data do documento.
    pub date: Option<String>,
    /// O nome original do ficheiro.
    pub filename: String,
    /// O tipo de conteúdo, legível («PDF»).
    pub content_type: String,
    /// O tamanho, já formatado.
    pub size: Option<String>,
    /// SHA-256 abreviado (verificável, não secreto).
    pub checksum: Option<String>,
    /// Classificação (a do ficheiro).
    pub classification: ResClassification,
    /// O ficheiro canónico (Ficheiros), quando tem ecrã.
    pub file: Option<ResLinkVm>,
    /// Transferir (`/documents/{id}/download`, pela BFF).
    pub download_href: Option<String>,
    /// O ambiente.
    pub workspace: Option<ResLinkVm>,
    /// Relações.
    pub links: Vec<ResLinkVm>,
    /// Nye.
    pub nye: Option<AppNyeVm>,
}

/// Registar uma referência (`POST /workspaces/{id}/sources`), sempre `metadata_only`
/// salvo decisão de uma pessoa com base legal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceFormVm {
    /// Para onde envia.
    pub action: String,
    /// Ambientes `may_create`.
    pub workspaces: Vec<ResOptionVm>,
    /// Tipos (vocabulário do Core, traduzido).
    pub types: Vec<ResOptionVm>,
    /// Bases legais que esta pessoa pode registar.
    pub rights: Vec<ResOptionVm>,
    /// Título.
    pub title: String,
    /// Autores, um por linha.
    pub authors: String,
    /// Ano.
    pub year: String,
    /// Publicação.
    pub container_title: String,
    /// DOI.
    pub doi: String,
    /// URL.
    pub url: String,
    /// Erro.
    pub error: Option<AppError>,
    /// Cancelar.
    pub cancel_href: String,
}

/// A aplicação Conhecimento (e Bibliografia).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KnowledgeVm {
    /// A secção.
    pub section: KnowledgeSection,
    /// Documentos · Bibliografia.
    pub nav: Vec<AppNavVm>,
    /// A pesquisa de âmbito, quando o Code a serve (`None` = sem campo).
    pub query: Option<String>,
    /// A lista.
    pub list: ResListVm,
    /// O painel.
    pub pane: ResPane,
    /// A fonte aberta.
    pub source: Option<SourceVm>,
    /// O documento aberto.
    pub document: Option<KnowledgeDocumentVm>,
    /// O erro do que está aberto.
    pub open_error: Option<AppError>,
    /// Formulário de referência.
    pub form: Option<SourceFormVm>,
    /// «Nova referência» (secção Bibliografia).
    pub new_href: Option<String>,
    /// A lista, com os filtros correntes (o «voltar» da janela estreita).
    pub list_href: String,
}

// ═════════════════════════════════════════════════════════════════════════
// D006 · Organização · pertença · administração (Unidades, Administração →
// Membros, Papéis, Instância). Aditivo.
//
// O Core governa; a vista apresenta. Nenhum tipo desta secção transporta um
// segredo, um verificador, um token ou um identificador de sessão, com UMA
// excepção deliberada e estreita: `OrgCredentialOnceVm`, a credencial
// temporária que o Core devolve uma única vez na resposta de criar, repor ou
// dar acesso (docs/identity). Nunca se guarda, nunca aparece numa lista.
// As acções só existem quando o VM as traz; ter um papel ou pertencer a uma
// unidade não dá autoridade na vista.
// ═════════════════════════════════════════════════════════════════════════

/// Estado da conta (`ocinye_contracts::AccountStatus`): os quatro do Core.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrgAccountStatus {
    /// Criada por um administrador; só existe credencial temporária.
    Invited,
    /// Normal.
    Active,
    /// Não autentica; sessões revogadas; reversível.
    Suspended,
    /// Identidade histórica permanente; nunca apagada.
    Disabled,
}

impl OrgAccountStatus {
    /// A chave do rótulo.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Invited => "org.status.invited",
            Self::Active => "org.status.active",
            Self::Suspended => "org.status.suspended",
            Self::Disabled => "org.status.disabled",
        }
    }
    /// O valor estável enviado ao Core.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Invited => "invited",
            Self::Active => "active",
            Self::Suspended => "suspended",
            Self::Disabled => "disabled",
        }
    }
}

/// Papel técnico (`ocinye_contracts::TechnicalRole`). Papéis de sistema,
/// definidos no código; a vista não cria nem edita nenhum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrgTechRole {
    /// platform_admin
    PlatformAdmin,
    /// organisation_admin
    OrganisationAdmin,
    /// unit_manager
    UnitManager,
    /// research_lead
    ResearchLead,
    /// research_member
    ResearchMember,
    /// collaborator
    Collaborator,
    /// external_collaborator
    ExternalCollaborator,
    /// auditor
    Auditor,
}

impl OrgTechRole {
    /// O identificador estável (mostrado em mono, ao lado do rótulo).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PlatformAdmin => "platform_admin",
            Self::OrganisationAdmin => "organisation_admin",
            Self::UnitManager => "unit_manager",
            Self::ResearchLead => "research_lead",
            Self::ResearchMember => "research_member",
            Self::Collaborator => "collaborator",
            Self::ExternalCollaborator => "external_collaborator",
            Self::Auditor => "auditor",
        }
    }
    /// A chave do rótulo.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::PlatformAdmin => "org.role.platform_admin",
            Self::OrganisationAdmin => "org.role.organisation_admin",
            Self::UnitManager => "org.role.unit_manager",
            Self::ResearchLead => "org.role.research_lead",
            Self::ResearchMember => "org.role.research_member",
            Self::Collaborator => "org.role.collaborator",
            Self::ExternalCollaborator => "org.role.external_collaborator",
            Self::Auditor => "org.role.auditor",
        }
    }
}

/// Papel numa unidade (`UnitRole`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrgUnitRole {
    /// Gere a unidade.
    Manager,
    /// Pertence à unidade.
    Member,
}

impl OrgUnitRole {
    /// A chave do rótulo.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Manager => "org.unit_role.manager",
            Self::Member => "org.unit_role.member",
        }
    }
    /// O valor estável enviado ao Core.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Manager => "manager",
            Self::Member => "member",
        }
    }
}

/// Papel num ambiente de investigação (`WorkspaceRole`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrgWorkspaceRole {
    /// lead
    Lead,
    /// member
    Member,
    /// viewer
    Viewer,
}

impl OrgWorkspaceRole {
    /// A chave do rótulo.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Lead => "org.ws_role.lead",
            Self::Member => "org.ws_role.member",
            Self::Viewer => "org.ws_role.viewer",
        }
    }
}

/// Posição institucional (`InstitutionalPosition`). Registo; não concede nada.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrgPosition {
    /// founder
    Founder,
    /// director
    Director,
    /// unit_lead
    UnitLead,
    /// principal_investigator
    PrincipalInvestigator,
    /// researcher
    Researcher,
    /// engineer
    Engineer,
    /// fellow
    Fellow,
    /// student
    Student,
    /// external_collaborator
    ExternalCollaborator,
}

impl OrgPosition {
    /// A chave do rótulo.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Founder => "org.pos.founder",
            Self::Director => "org.pos.director",
            Self::UnitLead => "org.pos.unit_lead",
            Self::PrincipalInvestigator => "org.pos.principal_investigator",
            Self::Researcher => "org.pos.researcher",
            Self::Engineer => "org.pos.engineer",
            Self::Fellow => "org.pos.fellow",
            Self::Student => "org.pos.student",
            Self::ExternalCollaborator => "org.pos.external_collaborator",
        }
    }
}

/// A origem de uma permissão (`GET /administration/members/{id}/access`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrgPermissionSource {
    /// technical_role
    TechnicalRole,
    /// unit_membership
    UnitMembership,
    /// workspace_membership
    WorkspaceMembership,
    /// explicit_grant
    ExplicitGrant,
}

impl OrgPermissionSource {
    /// A chave do rótulo.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::TechnicalRole => "org.source.technical_role",
            Self::UnitMembership => "org.source.unit_membership",
            Self::WorkspaceMembership => "org.source.workspace_membership",
            Self::ExplicitGrant => "org.source.explicit_grant",
        }
    }
}

/// A representação de uma pessoa. Iniciais calculadas pelo Code a partir do
/// nome que o Core devolve (o mesmo cálculo do avatar `initials`); imagem só
/// servida pelo Core (preset do produto ou objecto governado), nunca um URL
/// externo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrgAvatarVm {
    /// Uma ou duas letras.
    pub initials: String,
    /// `/me/avatar/{v}` ou o asset do preset; `None` = iniciais.
    pub image_href: Option<String>,
}

/// Uma unidade a que alguém pertence (`PersonUnit` + papel quando se sabe).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrgUnitRefVm {
    /// O código institucional.
    pub code: String,
    /// O nome.
    pub name: String,
    /// O papel nessa unidade, quando o contrato o traz.
    pub role: Option<OrgUnitRole>,
    /// `/units/{id}` — reautorizado ao abrir.
    pub href: Option<String>,
}

/// Um ambiente de investigação a que alguém pertence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrgWorkspaceRefVm {
    /// O título que o Core resolve para quem vê; `None` = o ambiente existe e
    /// não é legível por quem vê (mostra-se «Ambiente sem acesso»).
    pub title: Option<String>,
    /// O papel.
    pub role: OrgWorkspaceRole,
    /// A ligação canónica (Projectos/Ideias), quando legível.
    pub href: Option<String>,
}

/// O tipo de uma acção organizacional: dá o rótulo, o tom e o texto da
/// confirmação partilhada. A semântica é a do Core (docs/identity,
/// docs/authorization, organisation::service).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrgActionKind {
    /// status → suspended
    Suspend,
    /// status → disabled
    Disable,
    /// status → active
    Reactivate,
    /// password-reset (emite credencial temporária)
    ResetPassword,
    /// provision (dá acesso a quem existe sem credencial)
    Provision,
    /// provision, quando a temporária expirou
    Reissue,
    /// DELETE de um convite nunca aceite
    DeleteInvite,
    /// conceder um papel técnico
    GrantRole,
    /// revogar um papel técnico
    RevokeRole,
    /// revogar um grant explícito
    RevokeGrant,
    /// revogar uma sessão
    RevokeSession,
    /// mudar o papel numa unidade (gestor ↔ membro)
    ChangeUnitRole,
    /// retirar a pertença a uma unidade
    RemoveUnitMember,
    /// arquivar uma unidade
    ArchiveUnit,
    /// D007.1 · parar um serviço do runtime (só com inventário tipado e `may_stop`)
    StopService,
    /// D007 · Sair de uma conversa de grupo (Mensagens).
    LeaveConversation,
    /// D007 · Retirar alguém de uma conversa de grupo (Mensagens).
    RemoveParticipant,
}

impl OrgActionKind {
    /// O prefixo das chaves (`org.act.suspend` → `.label`, `.title`, `.body`, `.do`).
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Suspend => "org.act.suspend",
            Self::Disable => "org.act.disable",
            Self::Reactivate => "org.act.reactivate",
            Self::ResetPassword => "org.act.reset",
            Self::Provision => "org.act.provision",
            Self::Reissue => "org.act.reissue",
            Self::DeleteInvite => "org.act.delete",
            Self::GrantRole => "org.act.grant_role",
            Self::RevokeRole => "org.act.revoke_role",
            Self::RevokeGrant => "org.act.revoke_grant",
            Self::RevokeSession => "org.act.revoke_session",
            Self::ChangeUnitRole => "org.act.unit_role",
            Self::RemoveUnitMember => "org.act.unit_remove",
            Self::ArchiveUnit => "org.act.archive_unit",
            Self::StopService => "org.act.stop_service",
            Self::LeaveConversation => "org.act.msg_leave",
            Self::RemoveParticipant => "org.act.msg_remove",
        }
    }
    /// Retira acesso ou apaga: o botão final é de perigo e nunca tem o foco.
    #[must_use]
    pub const fn reduces_access(self) -> bool {
        matches!(
            self,
            Self::Suspend
                | Self::Disable
                | Self::DeleteInvite
                | Self::RevokeRole
                | Self::RevokeGrant
                | Self::RevokeSession
                | Self::RemoveUnitMember
                | Self::ArchiveUnit
                | Self::StopService
                | Self::LeaveConversation
                | Self::RemoveParticipant
        )
    }
    /// A operação devolve uma credencial temporária, mostrada uma vez.
    #[must_use]
    pub const fn issues_credential(self) -> bool {
        matches!(self, Self::ResetPassword | Self::Provision | Self::Reissue)
    }
}

/// Uma acção disponível agora: abre a confirmação partilhada (`?confirm=…`,
/// GET). A operação só corre no POST da confirmação, e o Core reautoriza.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrgActionVm {
    /// O tipo.
    pub kind: OrgActionKind,
    /// Onde abre a confirmação.
    pub href: String,
}

/// O motivo que a operação exige (só quando o Core o exige ou aceita).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrgReason {
    /// Sem motivo.
    None,
    /// Aceite, não obrigatório (conceder papel).
    Optional,
    /// Obrigatório, com o mínimo de caracteres que o Core aplica.
    Required(u8),
}

/// A confirmação partilhada das acções privilegiadas (SHARED_CONFIRMATION_EXTENSION).
/// Desenhada pela rota depois da casca, como `wm::dirty_close` e
/// `nye::confirm_dialog`. Confirmar não autoriza: o Core volta a decidir.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrgConfirmVm {
    /// A acção.
    pub kind: OrgActionKind,
    /// O alvo, pelo nome («Marta Quintas», «UENR-001 · Unidade de Energia»).
    pub target: String,
    /// O contexto, quando existe («na Unidade de Energia», o papel).
    pub context: Option<String>,
    /// Actual → proposto (mudança de papel), já traduzidos.
    pub change: Option<(String, String)>,
    /// POST (a rota BFF que já existe).
    pub action: String,
    /// Campos escondidos (papel, estado de destino).
    pub hidden: Vec<(&'static str, String)>,
    /// O motivo.
    pub reason: OrgReason,
    /// Voltar sem fazer nada.
    pub cancel_href: String,
    /// O Core recusou esta tentativa (fica aberta com a razão).
    pub refusal: Option<OrgRefusal>,
    /// Falhou por outra razão.
    pub error: Option<AppError>,
}

/// Recusas tipadas das invariantes do Core. O Code mapeia a resposta; a vista
/// nunca as calcula. Nenhuma revela o que o membro não pode ver.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrgRefusal {
    /// Suspender ou desactivar a própria conta.
    SelfLockout,
    /// Deixaria a instituição sem Platform Admin capaz de entrar.
    LastPlatformAdmin,
    /// Deixaria a unidade sem gestor.
    LastUnitManager,
    /// Conceder o que não detém.
    CannotGrantUnheld,
    /// Só um convite nunca aceite se apaga.
    NotDeletable,
    /// O estado mudou entretanto; a vista foi relida.
    StaleState,
    /// A opção escolhida já não está disponível.
    OptionUnavailable,
}

impl OrgRefusal {
    /// A chave do texto.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::SelfLockout => "org.refusal.self",
            Self::LastPlatformAdmin => "org.refusal.last_admin",
            Self::LastUnitManager => "org.refusal.last_manager",
            Self::CannotGrantUnheld => "org.refusal.unheld",
            Self::NotDeletable => "org.refusal.not_deletable",
            Self::StaleState => "org.refusal.stale",
            Self::OptionUnavailable => "org.refusal.option",
        }
    }
}

/// O que acabou de acontecer (uma linha discreta, não um toast).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrgNotice {
    /// Estado da conta alterado.
    StatusChanged,
    /// Papel concedido.
    RoleGranted,
    /// Papel revogado.
    RoleRevoked,
    /// Grant revogado.
    GrantRevoked,
    /// Sessão revogada.
    SessionRevoked,
    /// Posição alterada.
    PositionChanged,
    /// Membro acrescentado à unidade.
    MemberAdded,
    /// Pertença retirada.
    MemberRemoved,
    /// Papel na unidade alterado.
    UnitRoleChanged,
    /// Unidade guardada.
    UnitSaved,
    /// Unidade arquivada.
    UnitArchived,
    /// Configuração da Instância guardada.
    SettingsSaved,
}

impl OrgNotice {
    /// A chave do texto.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::StatusChanged => "org.done.status",
            Self::RoleGranted => "org.done.role_granted",
            Self::RoleRevoked => "org.done.role_revoked",
            Self::GrantRevoked => "org.done.grant_revoked",
            Self::SessionRevoked => "org.done.session_revoked",
            Self::PositionChanged => "org.done.position",
            Self::MemberAdded => "org.done.member_added",
            Self::MemberRemoved => "org.done.member_removed",
            Self::UnitRoleChanged => "org.done.unit_role",
            Self::UnitSaved => "org.done.unit_saved",
            Self::UnitArchived => "org.done.unit_archived",
            Self::SettingsSaved => "org.done.settings",
        }
    }
}

/// A credencial temporária, devolvida UMA vez pelo Core (criar, repor, dar
/// acesso). O único VM com um segredo. Nunca numa lista, nunca guardado; a
/// resposta que o traz vai com `Cache-Control: no-store`. Nada foi enviado por
/// correio: o administrador entrega-a por canal seguro.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrgCredentialOnceVm {
    /// O que a emitiu.
    pub origin: OrgActionKind,
    /// A pessoa.
    pub name: String,
    /// O endereço institucional com que entra.
    pub email: String,
    /// O segredo em claro (`temporary_password`).
    pub secret: String,
    /// Válida até (formatado).
    pub expires: String,
    /// Concluir (para o detalhe do membro).
    pub done_href: String,
}

/// Uma linha do roster administrativo (`GET /administration/members`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrgMemberRowVm {
    /// O nome (`full_name`).
    pub name: String,
    /// O endereço institucional (o roster administrativo trá-lo).
    pub email: Option<String>,
    /// Avatar.
    pub avatar: OrgAvatarVm,
    /// Estado da conta.
    pub status: OrgAccountStatus,
    /// Posição institucional.
    pub position: Option<OrgPosition>,
    /// Códigos das unidades (sem unidade «principal»: o Core não a infere).
    pub units: Vec<String>,
    /// Registo (`created_at`), formatado.
    pub joined: Option<String>,
    /// Última actividade (`last_seen_at`), formatada.
    pub last_seen: Option<String>,
    /// `/admin/members/{id}`.
    pub href: String,
    /// É o que está aberto.
    pub active: bool,
    /// É a conta de quem vê.
    pub is_self: bool,
}

/// O roster.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrgMembersListVm {
    /// As linhas.
    pub rows: Vec<OrgMemberRowVm>,
    /// Estado.
    pub load: AppLoad,
    /// Paginação (página → cursor, pelo Code).
    pub page: AppPageVm,
}

/// Um papel técnico detido, com a revogação quando o actor a pode fazer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrgRoleHeldVm {
    /// O papel.
    pub role: OrgTechRole,
    /// Revogar (abre a confirmação).
    pub revoke: Option<OrgActionVm>,
}

/// Um grant explícito vivo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrgGrantVm {
    /// A permissão (identificador estável, `documents.view`).
    pub permission: String,
    /// O âmbito, já traduzido e resolvido («Unidade · UENR-001»).
    pub scope: String,
    /// O motivo registado.
    pub reason: String,
    /// Quem concedeu.
    pub granted_by: Option<String>,
    /// Caduca (formatado); `None` = sem data.
    pub expires: Option<String>,
    /// Revogar.
    pub revoke: Option<OrgActionVm>,
}

/// Uma permissão efectiva à escala institucional, com a origem.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrgPermissionVm {
    /// Identificador estável.
    pub permission: String,
    /// Origem.
    pub source: OrgPermissionSource,
}

/// O acesso de uma pessoa (`/access`, exige `RolesView` para outrem).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrgAccessVm {
    /// Papéis técnicos.
    pub roles: Vec<OrgRoleHeldVm>,
    /// Grants explícitos.
    pub grants: Vec<OrgGrantVm>,
    /// Permissões institucionais e a sua origem.
    pub permissions: Vec<OrgPermissionVm>,
    /// Conceder um papel: as opções vêm do Core (papéis ainda não detidos que o
    /// actor pode conceder). `None` = o actor não gere papéis.
    pub grant: Option<OrgRoleGrantVm>,
}

/// O formulário de conceder papel (abre a confirmação com o papel escolhido).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrgRoleGrantVm {
    /// GET que abre a confirmação (`?confirm=grant_role`).
    pub action: String,
    /// Papéis concedíveis (valor estável + rótulo).
    pub options: Vec<ResOptionVm>,
}

/// A credencial temporária existente (sem o segredo: só a validade).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrgTempCredVm {
    /// A data, formatada.
    pub expires: String,
    /// Já passou.
    pub expired: bool,
}

/// Uma sessão viva (metadata; nunca o identificador opaco).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrgSessionVm {
    /// Emitida (formatado).
    pub issued: String,
    /// Última actividade (formatado).
    pub last_seen: String,
    /// Expira (formatado).
    pub expires: String,
    /// O agente, resumido pelo Code («Firefox · macOS»).
    pub agent: Option<String>,
    /// O prefixo de rede que o Core guarda.
    pub ip_prefix: Option<String>,
    /// Sessão restrita (mudança de palavra-passe obrigatória).
    pub restricted: bool,
    /// Revogar.
    pub revoke: Option<OrgActionVm>,
}

/// A segurança da conta (`/security`): metadata segura, nunca a credencial.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrgSecurityVm {
    /// Já definiu palavra-passe própria.
    pub has_permanent_password: bool,
    /// Quando (formatado).
    pub password_changed: Option<String>,
    /// A temporária que existe.
    pub temporary: Option<OrgTempCredVm>,
    /// Última entrada com sucesso.
    pub last_sign_in: Option<String>,
    /// Tentativas falhadas recentes.
    pub recent_failures: u32,
    /// O segundo factor é exigido a esta pessoa (regra do Core).
    pub mfa_required: bool,
    /// Tem TOTP confirmado.
    pub mfa_enrolled: bool,
    /// Sessões vivas.
    pub sessions: Vec<OrgSessionVm>,
}

/// A posição institucional, quando o actor a pode mudar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrgPositionFormVm {
    /// POST `/admin/members/{id}/position`.
    pub action: String,
    /// As nove posições + «Sem posição» (valor vazio).
    pub options: Vec<ResOptionVm>,
}

/// Um membro aberto na Administração.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrgMemberVm {
    /// O nome.
    pub name: String,
    /// O endereço institucional.
    pub email: Option<String>,
    /// Avatar.
    pub avatar: OrgAvatarVm,
    /// Estado.
    pub status: OrgAccountStatus,
    /// Posição.
    pub position: Option<OrgPosition>,
    /// É a própria conta de quem vê.
    pub is_self: bool,
    /// Registo.
    pub joined: Option<String>,
    /// Última actividade.
    pub last_seen: Option<String>,
    /// Unidades.
    pub units: Vec<OrgUnitRefVm>,
    /// Ambientes de investigação.
    pub workspaces: Vec<OrgWorkspaceRefVm>,
    /// Acesso (`None` = o actor não tem `RolesView`: a secção não aparece).
    pub access: Option<OrgAccessVm>,
    /// Segurança (`None` = sem `MembersManage`).
    pub security: Option<OrgSecurityVm>,
    /// Acções de conta que o Core permite ao actor, já filtradas pelo estado.
    pub account_actions: Vec<OrgActionVm>,
    /// Mudar a posição.
    pub position_form: Option<OrgPositionFormVm>,
    /// Uma recusa da última tentativa.
    pub refusal: Option<OrgRefusal>,
    /// O que acabou de acontecer.
    pub notice: Option<OrgNotice>,
}

/// Criar um membro (`POST /administration/members`): conta `invited` com
/// credencial temporária. Não é um convite por correio.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrgNewMemberVm {
    /// POST `/admin/members/new`.
    pub action: String,
    /// Valores já escritos (depois de uma recusa).
    pub full_name: String,
    /// Endereço institucional.
    pub email: String,
    /// Posições (opcional).
    pub positions: Vec<ResOptionVm>,
    /// Papéis que o actor pode conceder (o Core recusa `platform_admin` a quem
    /// não o detém; o Code só o oferece a quem o detém).
    pub roles: Vec<ResOptionVm>,
    /// Unidades legíveis (opcional).
    pub units: Vec<ResOptionVm>,
    /// Falha.
    pub error: Option<AppError>,
    /// Recusa.
    pub refusal: Option<OrgRefusal>,
}

/// Um papel de sistema no catálogo (`GET /administration/roles`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrgRoleDefVm {
    /// O papel.
    pub role: OrgTechRole,
    /// As permissões que concede, identificadores estáveis.
    pub permissions: Vec<String>,
}

/// Uma aplicação da Instância (`GET /instance/applications`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrgAppStateVm {
    /// Identificador técnico (`app:<id>` no formulário).
    pub id: String,
    /// O rótulo, já traduzido.
    pub label: String,
    /// O ícone.
    pub icon: &'static str,
    /// Essencial (não se desactiva).
    pub essential: bool,
    /// Activa agora.
    pub active: bool,
    /// Decisão explícita da Instância (senão, o perfil).
    pub explicit: bool,
    /// O que o perfil diria.
    pub profile_default: bool,
}

/// A Instância (`/admin/instance`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrgInstanceVm {
    /// O nome da Instância.
    pub name: String,
    /// A distribuição (perfil). Mostrada, não editada na D006.
    pub distribution: Distribution,
    /// Língua por omissão (rótulo).
    pub default_locale: String,
    /// Fuso IANA configurado.
    pub timezone: String,
    /// Última alteração da configuração.
    pub updated: Option<String>,
    /// As aplicações.
    pub apps: Vec<OrgAppStateVm>,
    /// POST `/admin/instance` (as decisões por aplicação); `None` = só leitura.
    pub apps_action: Option<String>,
    /// Estado.
    pub load: AppLoad,
    /// Acabou de guardar.
    pub notice: Option<OrgNotice>,
}

/// A secção da Administração.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdminSection {
    /// Membros (roster, detalhe, criar).
    Members,
    /// Papéis (catálogo, só leitura).
    Roles,
    /// Instância (informação, aplicações).
    Instance,
    /// D010 · S26/S38 — as Distribuições da Instância.
    Distributions,
    /// D010 · S37 — quem pode abrir cada Distribuição.
    DistributionAccess,
    /// D010 · S27–S32 — os pontos de acesso.
    Endpoints,
    /// D010 · S33 — as predefinições (a camada da Instância adiada).
    Defaults,
}

/// A aplicação Administração (`/admin`, `members.manage`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdminVm {
    /// A secção.
    pub section: AdminSection,
    /// Membros · Papéis · Instância (só as que o Core abre).
    pub nav: Vec<AppNavVm>,
    /// A aplicação inteira recusada (ligação directa, privilégio perdido).
    pub error: Option<AppError>,
    /// O roster.
    pub members: OrgMembersListVm,
    /// Lista | detalhe.
    pub pane: ResPane,
    /// O membro aberto.
    pub member: Option<OrgMemberVm>,
    /// O erro do membro pedido.
    pub member_error: Option<AppError>,
    /// Criar membro.
    pub new_member: Option<OrgNewMemberVm>,
    /// A credencial acabada de emitir.
    pub credential: Option<OrgCredentialOnceVm>,
    /// «Novo membro» (`members.create`).
    pub new_href: Option<String>,
    /// O catálogo de papéis.
    pub roles: Vec<OrgRoleDefVm>,
    /// A Instância.
    pub instance: Option<OrgInstanceVm>,
    /// O roster, com a página corrente (o «voltar»).
    pub list_href: String,
    /// O corpo de uma secção D010 (`None` nas da D006).
    pub d010: Option<AdminD010Vm>,
}

/// O corpo de uma secção D010 da Administração.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AdminD010Vm {
    /// S26 · as quatro Distribuições.
    Distributions {
        /// Uma linha por Distribuição, pela ordem canónica.
        rows: Vec<AdmDistRowVm>,
        /// Quem vê tem `organisation.manage` (senão, só leitura).
        manage: bool,
    },
    /// S37 · a matriz de acesso.
    Access {
        /// As Distribuições activadas (as colunas).
        dists: Vec<Distribution>,
        /// Um membro por linha.
        rows: Vec<AdmAccessRowVm>,
    },
    /// S27 · os pontos de acesso, e o aberto (S29/S30).
    Endpoints {
        /// Todos.
        rows: Vec<AdmEndpointRowVm>,
        /// O aberto.
        open: Option<AdmEndpointRowVm>,
        /// As Distribuições activadas (os destinos possíveis).
        enabled: Vec<Distribution>,
        /// Quem vê pode mudar.
        manage: bool,
    },
    /// S33 · as predefinições.
    Defaults {
        /// As Distribuições activadas.
        dists: Vec<Distribution>,
    },
}

/// Uma Distribuição da Instância (S26).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdmDistRowVm {
    /// Qual.
    pub distribution: Distribution,
    /// `enabled` · `disabled` · `available`.
    pub state: String,
    /// Membros com acesso.
    pub members: i64,
    /// Sessões vivas nela (facto de S38).
    pub sessions: i64,
    /// Os pontos fixos nela.
    pub endpoints: Vec<String>,
    /// É a única activada: desactivar é recusado (o Core recusa de qualquer modo).
    pub last: bool,
}

/// Um membro na matriz de acesso (S37).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdmAccessRowVm {
    /// Quem.
    pub person_id: String,
    /// O nome.
    pub name: String,
    /// Uma célula por Distribuição activada.
    pub cells: Vec<AdmAccessCellVm>,
}

/// Uma célula da matriz.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdmAccessCellVm {
    /// A coluna.
    pub distribution: Distribution,
    /// Tem acesso.
    pub has: bool,
    /// Retirar deixaria a Instância sem administrador que entre.
    pub protected: bool,
    /// As sessões vivas do membro nesta Distribuição.
    pub sessions: i64,
}

/// Um ponto de acesso (S27).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdmEndpointRowVm {
    /// Identidade.
    pub id: String,
    /// O anfitrião.
    pub host: String,
    /// A Distribuição fixa (`None` = genérico).
    pub binding: Option<Distribution>,
    /// `active` · `disabled` · `unverified`.
    pub state: String,
    /// A última observação da ligação segura: `valid` · `invalid` · `pending`.
    pub tls: String,
    /// A última observação do nome: `resolves_here` · `resolves_elsewhere` · `not_observed`.
    pub dns: String,
    /// O canónico da Instância.
    pub canonical: bool,
}

/// Um facto imutável de uma confirmação.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AdmFact {
    /// Texto.
    Text(String),
    /// Em destaque.
    Strong(String),
    /// Um anfitrião.
    Code(String),
}

/// A confirmação governada da D010 (S31, S32, S37, S38).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdmConfirmVm {
    /// O ícone.
    pub icon: &'static str,
    /// O título.
    pub title: String,
    /// O que acontece.
    pub body: String,
    /// Os factos.
    pub facts: Vec<(String, AdmFact)>,
    /// Para onde vai o `POST`.
    pub action: String,
    /// Campos escondidos do formulário.
    pub hidden: Vec<(String, String)>,
    /// A recusa (sem botão de confirmar).
    pub refusal: Option<String>,
    /// A chave do botão de confirmar.
    pub ok_key: &'static str,
    /// Cancelar volta aqui.
    pub cancel: String,
}

/// S28 · acrescentar um ponto de acesso.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdmAddEndpointVm {
    /// O que foi escrito.
    pub host: String,
    /// O destino escolhido (`generic` ou uma Distribuição).
    pub binding: String,
    /// A chave do erro, quando o Core recusou.
    pub error: Option<&'static str>,
    /// As Distribuições activadas.
    pub dists: Vec<Distribution>,
}

/// O estado de uma unidade (`active` | `archived`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrgUnitStatus {
    /// Activa.
    Active,
    /// Arquivada (história; nunca apagada).
    Archived,
}

/// Um membro de uma unidade (`GET /units/{id}/members`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnitMemberVm {
    /// O nome.
    pub name: String,
    /// Avatar.
    pub avatar: OrgAvatarVm,
    /// Papel na unidade.
    pub role: OrgUnitRole,
    /// O membro na Administração — só quando quem vê tem `members.manage`.
    pub href: Option<String>,
    /// Passar a gestor / a membro (abre a confirmação).
    pub change_role: Option<OrgActionVm>,
    /// Retirar da unidade (abre a confirmação).
    pub remove: Option<OrgActionVm>,
}

/// Acrescentar um membro à unidade: selector governado.
///
/// Os candidatos vêm do Core (CORE_CONTRACT_REQUIRED: candidatos elegíveis com
/// pesquisa). O browser nunca enumera; forjar um identificador falha no Core.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnitAddMemberVm {
    /// GET da pesquisa de candidatos.
    pub search_action: String,
    /// O texto pesquisado.
    pub query: String,
    /// `None` = ainda não pesquisou; `Some(vec![])` = sem resultados.
    pub candidates: Option<Vec<ResOptionVm>>,
    /// POST `/units/{id}/members`.
    pub add_action: String,
    /// `manager` | `member`.
    pub roles: Vec<ResOptionVm>,
    /// Sem contrato de candidatos: o bloco aparece desactivado, com a razão.
    pub unavailable: bool,
}

/// Uma unidade aberta.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnitVm {
    /// Código institucional (imutável).
    pub code: String,
    /// Nome.
    pub name: String,
    /// Estado.
    pub status: OrgUnitStatus,
    /// Descrição (texto simples).
    pub description: Option<String>,
    /// Áreas de investigação declaradas.
    pub areas: Vec<String>,
    /// Membros.
    pub members: Vec<UnitMemberVm>,
    /// Estado da lista de membros.
    pub members_load: AppLoad,
    /// `may_manage_members` do Core.
    pub may_manage: bool,
    /// Editar (nome, descrição, áreas).
    pub edit_href: Option<String>,
    /// Arquivar.
    pub archive: Option<OrgActionVm>,
    /// Acrescentar membro.
    pub add: Option<UnitAddMemberVm>,
    /// Nye contextual (a referência `unit` já existe na D003).
    pub nye: Option<AppNyeVm>,
    /// Recusa.
    pub refusal: Option<OrgRefusal>,
    /// O que acabou de acontecer.
    pub notice: Option<OrgNotice>,
}

/// Criar ou editar uma unidade.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnitFormVm {
    /// POST `/units/new` ou `/units/{id}/edit`.
    pub action: String,
    /// Criar (senão, editar).
    pub is_new: bool,
    /// Editar: o código, só leitura.
    pub code: Option<String>,
    /// Criar: a sugestão indicativa (`/units/code-suggestion`).
    pub code_suggestion: Option<String>,
    /// Nome.
    pub name: String,
    /// Descrição.
    pub description: String,
    /// Áreas, separadas por vírgulas.
    pub areas: String,
    /// Falha.
    pub error: Option<AppError>,
}

/// A secção de Unidades.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnitsSection {
    /// Activas.
    Active,
    /// Arquivadas (`include_archived`).
    Archived,
}

/// A aplicação Unidades (`/units`, `units.view`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnitsVm {
    /// A secção.
    pub section: UnitsSection,
    /// Activas · Arquivadas.
    pub nav: Vec<AppNavVm>,
    /// A lista (título = nome; código e estado no título).
    pub list: ResListVm,
    /// Lista | detalhe.
    pub pane: ResPane,
    /// A unidade aberta.
    pub unit: Option<UnitVm>,
    /// O erro do que foi pedido.
    pub unit_error: Option<AppError>,
    /// Criar/editar.
    pub form: Option<UnitFormVm>,
    /// «Nova unidade» (quando o Core deixa criar).
    pub new_href: Option<String>,
    /// A lista corrente.
    pub list_href: String,
}

// ═════════════════════════════════════════════════════════════════════════
// D007 · Conclusão das aplicações: Mensagens · IA · Agentes · Computação ·
// Meus Recursos · Actividade · Auditoria · Definições · Ajuda. Aditivo.
//
// Só estado que o Core/runtime já autorizou. Nenhum VM traz segredo, chave de
// fornecedor, instruções de sistema, raciocínio de modelo, conteúdo de
// mensagem fora das Mensagens, nem metadata de auditoria não saneada.
// ═════════════════════════════════════════════════════════════════════════

// ── Mensagens ──

/// `direct` | `group` (`conversations.kind`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MsgKind {
    /// Directa, com uma pessoa.
    Direct,
    /// Grupo com nome.
    Group,
}

/// A presença resolvida pelo tempo real. `None` no VM = tempo real em baixo
/// (não é «offline»).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MsgPresence {
    /// online
    Online,
    /// away
    Away,
    /// offline
    Offline,
}

/// Uma conversa na lista.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MsgConvRowVm {
    /// Nome do grupo ou da pessoa.
    pub title: String,
    /// Tipo.
    pub kind: MsgKind,
    /// Avatar (pessoa) ou iniciais do grupo.
    pub avatar: OrgAvatarVm,
    /// Por ler (`unread`).
    pub unread: u32,
    /// Menções por ler.
    pub mentions: u32,
    /// Excerto da última mensagem (texto; escapado).
    pub last: Option<String>,
    /// Quando (formatado).
    pub last_at: Option<String>,
    /// Presença da outra pessoa (directa).
    pub presence: Option<MsgPresence>,
    /// `/messages/{id}`.
    pub href: String,
    /// Aberta.
    pub active: bool,
}

/// Uma reacção agregada.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MsgReactionVm {
    /// O emoji (texto do membro).
    pub emoji: String,
    /// Quantas.
    pub count: u32,
    /// Já reagi.
    pub mine: bool,
}

/// Uma mensagem.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MsgVm {
    /// Âncora (`m-…`), para responder e voltar.
    pub anchor: String,
    /// Autor.
    pub author: String,
    /// Avatar.
    pub avatar: OrgAvatarVm,
    /// É minha.
    pub mine: bool,
    /// Corpo (texto; `None` = retirada).
    pub body: Option<String>,
    /// Quando.
    pub at: String,
    /// Editada.
    pub edited: bool,
    /// Resposta a (autor, excerto).
    pub reply: Option<(String, String)>,
    /// Menciona-me.
    pub mentions_me: bool,
    /// Reacções.
    pub reactions: Vec<MsgReactionVm>,
    /// POST reagir (`emoji=`), quando se pode.
    pub react_action: Option<String>,
    /// GET que prepara a resposta (`?reply=`).
    pub reply_href: Option<String>,
    /// Separador de dia antes desta mensagem.
    pub day: Option<String>,
}

/// O compositor. O texto volta intacto se o envio falhar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MsgComposerVm {
    /// POST `/messages/{id}/send`.
    pub action: String,
    /// Texto por enviar.
    pub body: String,
    /// A responder a (autor, excerto) + o id opaco.
    pub reply: Option<(String, String, String)>,
    /// Cancelar a resposta.
    pub reply_cancel_href: Option<String>,
    /// Chave de idempotência (uma por rascunho).
    pub idempotency_key: String,
    /// Falhou.
    pub error: Option<AppError>,
}

/// Um participante.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MsgMemberVm {
    /// Nome.
    pub name: String,
    /// Papel na conversa, traduzido («Dono», «Administrador», «Membro»).
    pub role: String,
    /// Retirar (abre a confirmação D006).
    pub remove: Option<OrgActionVm>,
}

/// A conversa aberta.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MsgThreadVm {
    /// Título.
    pub title: String,
    /// Tipo.
    pub kind: MsgKind,
    /// Presença (directa).
    pub presence: Option<MsgPresence>,
    /// Mensagens, da mais antiga para a mais recente.
    pub messages: Vec<MsgVm>,
    /// Mensagens anteriores (`?before=`).
    pub older_href: Option<String>,
    /// Estado.
    pub load: AppLoad,
    /// Compositor.
    pub composer: MsgComposerVm,
    /// Participantes (grupo).
    pub members: Vec<MsgMemberVm>,
    /// Sair do grupo (confirmação).
    pub leave: Option<OrgActionVm>,
}

/// Nova conversa: directa com pessoa elegível, ou grupo. Candidatos do Core.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MsgNewVm {
    /// GET pesquisa.
    pub search_action: String,
    /// Texto.
    pub query: String,
    /// `None` = ainda não pesquisou.
    pub candidates: Option<Vec<ResOptionVm>>,
    /// POST `/messages/start`.
    pub action: String,
    /// Sem contrato de candidatos: explicado, desactivado.
    pub unavailable: bool,
    /// Falha.
    pub error: Option<AppError>,
}

/// A aplicação Mensagens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MessagesVm {
    /// Conversas.
    pub conversations: Vec<MsgConvRowVm>,
    /// Estado da lista.
    pub load: AppLoad,
    /// Lista | conversa.
    pub pane: ResPane,
    /// Aberta.
    pub thread: Option<MsgThreadVm>,
    /// Erro do que foi pedido (não revela nada).
    pub thread_error: Option<AppError>,
    /// Nova conversa.
    pub new: Option<MsgNewVm>,
    /// «Nova conversa».
    pub new_href: Option<String>,
    /// A lista.
    pub list_href: String,
}

// ── IA ──

/// Capacidade de inferência (`AiCapability`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AiCap {
    /// GENERAL
    General,
    /// CODING
    Coding,
    /// REASONING
    Reasoning,
    /// EMBEDDING
    Embedding,
}

/// Porque a inferência não serve (`AiReasonCode`, mapeamento explícito).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AiReason {
    /// AI_NO_PROVIDER_AVAILABLE
    NoProvider,
    /// AI_NO_COMPATIBLE_MODEL
    NoCompatibleModel,
    /// AI_CAPACITY_UNAVAILABLE
    CapacityUnavailable,
    /// AI_MODEL_HARDWARE_NOT_SATISFIED
    HardwareNotSatisfied,
    /// AI_PROVIDER_UNHEALTHY
    ProviderUnhealthy,
    /// AI_MODEL_LOADING
    ModelLoading,
    /// AI_DISABLED_BY_POLICY
    DisabledByPolicy,
    /// AI_POLICY_BLOCKED
    PolicyBlocked,
    /// Qualquer código que esta versão não conheça: dito como desconhecido.
    Unknown,
}

/// Uma capacidade, com o que a serve.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AiCapVm {
    /// Capacidade.
    pub cap: AiCap,
    /// Servida agora.
    pub available: bool,
    /// Modelo mapeado.
    pub model: Option<String>,
    /// Porque não.
    pub reason: Option<AiReason>,
    /// Fornecedor preferido (rótulo) e se há alternativa.
    pub preferred: Option<String>,
    /// Pode recorrer a outro candidato.
    pub fallback: Option<bool>,
}

/// Um modelo registado (`GET /ai/models`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AiModelVm {
    /// Nome.
    pub name: String,
    /// Versão.
    pub version: String,
    /// Fornecedor (rótulo).
    pub provider: String,
    /// `local` | `external`.
    pub local: bool,
    /// Estado declarado pelo registo (`status`, texto já traduzido).
    pub status: String,
    /// Tom do estado.
    pub tone: ResTone,
    /// Activo para encaminhamento.
    pub enabled: bool,
    /// Capacidades que declara.
    pub caps: Vec<AiCap>,
    /// Classificação máxima que pode receber.
    pub max_class: ResClassification,
}

/// Um fornecedor, sem credencial.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AiProviderVm {
    /// Rótulo (texto da administração; escapado).
    pub label: String,
    /// Protocolo.
    pub kind: String,
    /// Local.
    pub local: bool,
    /// Activo.
    pub enabled: bool,
    /// Última verificação: saudável.
    pub healthy: Option<bool>,
    /// Quando.
    pub checked: Option<String>,
    /// Tem credencial por referência (nunca o valor).
    pub has_credential: bool,
    /// Activar/desactivar (confirmação), só com autoridade de plataforma.
    pub toggle: Option<OrgActionVm>,
}

/// A secção.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AiSection {
    /// Estado.
    Overview,
    /// Modelos.
    Models,
    /// Fornecedores e encaminhamento.
    Providers,
}

/// A aplicação IA (Ocinye AI Fabric).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AiVm {
    /// Secção.
    pub section: AiSection,
    /// Navegação.
    pub nav: Vec<AppNavVm>,
    /// Alguma capacidade servida.
    pub available: bool,
    /// Fornecedores saudáveis.
    pub healthy_providers: u32,
    /// Mensagem do Core quando nada serve.
    pub message: Option<String>,
    /// Por capacidade.
    pub caps: Vec<AiCapVm>,
    /// Modelos.
    pub models: Vec<AiModelVm>,
    /// Fornecedores (`None` = sem autoridade para os ver).
    pub providers: Option<Vec<AiProviderVm>>,
    /// Política: máximo externo (`None` = sem IA externa).
    pub external_max: Option<Option<ResClassification>>,
    /// A instalação permite externos.
    pub installation_external: Option<bool>,
    /// Estado.
    pub load: AppLoad,
    /// Abrir a Nye.
    pub nye_href: Option<String>,
}

// ── Agentes ──

/// `AgentState`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentStatus {
    /// Pronto: uma capacidade serve-o agora.
    Ready,
    /// Configurado, nada o serve.
    Configured,
    /// Desactivado pelo dono.
    Disabled,
    /// Arquivado.
    Archived,
}

/// `AgentScope`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentScopeVm {
    /// Pessoal.
    Personal,
    /// Ambiente.
    Workspace,
    /// Unidade.
    Unit,
    /// Instituição.
    Institutional,
}

/// Um agente aberto.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentVm {
    /// Nome.
    pub name: String,
    /// Para que serve.
    pub purpose: Option<String>,
    /// Estado.
    pub status: AgentStatus,
    /// Capacidade que pede.
    pub cap: AiCap,
    /// Âmbito.
    pub scope: AgentScopeVm,
    /// Unidade/ambiente, resolvido para quem vê.
    pub scope_target: Option<ResLinkVm>,
    /// Tecto de classificação.
    pub max_class: ResClassification,
    /// Fontes: bibliografia, documentos, dados.
    pub sources: (bool, bool, bool),
    /// As instruções — só para quem o criou.
    pub instructions: Option<String>,
    /// Criado por · quando.
    pub created: String,
    /// Abrir na Nye (a execução é da Nye/Core).
    pub nye: Option<AppNyeVm>,
}

/// Criar um agente (`POST /ai/agents`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentFormVm {
    /// POST.
    pub action: String,
    /// Âmbitos que o actor pode criar (permissão por âmbito).
    pub scopes: Vec<ResOptionVm>,
    /// Alvos (unidades/ambientes) legíveis.
    pub targets: Vec<ResOptionVm>,
    /// Capacidades.
    pub caps: Vec<ResOptionVm>,
    /// Classificações.
    pub classes: Vec<ResOptionVm>,
    /// Falha.
    pub error: Option<AppError>,
}

/// A aplicação Agentes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentsVm {
    /// Lista (título = nome; estado; colunas capacidade, âmbito, criado).
    pub list: ResListVm,
    /// Há execução disponível agora.
    pub execution_available: bool,
    /// Lista | detalhe.
    pub pane: ResPane,
    /// Aberto.
    pub agent: Option<AgentVm>,
    /// Erro.
    pub agent_error: Option<AppError>,
    /// Criar.
    pub form: Option<AgentFormVm>,
    /// «Novo agente».
    pub new_href: Option<String>,
    /// A lista.
    pub list_href: String,
}

// ── Computação ──

/// `ComputeNodeStatus`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeStatus {
    /// pending_enrollment
    Pending,
    /// online
    Online,
    /// offline
    Offline,
    /// draining
    Draining,
    /// retired
    Retired,
}

/// Uma linha de capacidade (`CapacityLine`), já formatada. `None` = não reportado.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapLineVm {
    /// Rótulo (CPU, Memória, Armazenamento).
    pub label_key: &'static str,
    /// Físico.
    pub physical: Option<String>,
    /// Reservado para o anfitrião.
    pub reserved: String,
    /// Atribuível.
    pub allocatable: Option<String>,
    /// Em uso reportado.
    pub consumed: Option<String>,
    /// Fracção em uso 0–100, só se o nó reporta físico e uso.
    pub consumed_pct: Option<u8>,
}

/// Um nó aberto.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeVm {
    /// Nome.
    pub name: String,
    /// Identificador estável (não é segredo).
    pub identifier: String,
    /// Estado.
    pub status: NodeStatus,
    /// Tipo, traduzido.
    pub kind: String,
    /// Local.
    pub location: Option<String>,
    /// Controlo institucional, traduzido.
    pub control: String,
    /// Residência física, traduzida.
    pub residency: String,
    /// Capacidade.
    pub lines: Vec<CapLineVm>,
    /// GPUs: contagem · memória.
    pub gpus: Option<String>,
    /// Versão do agente do nó.
    pub agent_version: Option<String>,
    /// Última vez visto.
    pub last_seen: Option<String>,
}

/// A aplicação Computação.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComputeVm {
    /// Nós registados.
    pub registered: u32,
    /// Em linha.
    pub online: u32,
    /// Mensagem do Core com o plano vazio.
    pub message: Option<String>,
    /// Nós (título = nome; colunas estado, tipo, GPUs, visto).
    pub list: ResListVm,
    /// Lista | detalhe.
    pub pane: ResPane,
    /// Aberto.
    pub node: Option<NodeVm>,
    /// Erro.
    pub node_error: Option<AppError>,
    /// A lista.
    pub list_href: String,
}

// ── Meus Recursos ──

/// `StorageState`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StorageStateVm {
    /// normal
    Normal,
    /// warning
    Warning,
    /// critical
    Critical,
    /// over_quota
    OverQuota,
}

/// Uma parte do direito (`EntitlementPart`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntPartVm {
    /// `profile` | `override` | `temporary`, traduzido.
    pub source: String,
    /// Quantidade formatada.
    pub quantity: String,
    /// Caduca.
    pub expires: Option<String>,
    /// Nota (código do perfil ou motivo).
    pub note: String,
}

/// A aplicação Meus Recursos: o que posso consumir e o que consumo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourcesVm {
    /// Estado.
    pub load: AppLoad,
    /// Usado.
    pub used: String,
    /// Reservado por envios em curso.
    pub reserved: String,
    /// Limite (`None` = ainda sem limite resolvido).
    pub limit: Option<String>,
    /// Disponível.
    pub available: String,
    /// 0–100 do limite.
    pub used_pct: Option<u8>,
    /// 0–100 reservado.
    pub reserved_pct: Option<u8>,
    /// Estado.
    pub state: StorageStateVm,
    /// Direito efectivo.
    pub entitlement: String,
    /// As partes que o explicam.
    pub parts: Vec<EntPartVm>,
    /// Abrir Ficheiros.
    pub files_href: Option<String>,
}

// ── Actividade ──

/// Um evento. `target: None` + `redacted` = o alvo já não é visível.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActivityItemVm {
    /// Quem (nome histórico).
    pub actor: Option<String>,
    /// O que (resumo do Core; texto).
    pub summary: String,
    /// O alvo, se o Core o resolve agora para quem vê.
    pub target: Option<ResLinkVm>,
    /// Alvo já sem acesso.
    pub redacted: bool,
    /// Ambiente (resolvido agora).
    pub context: Option<String>,
    /// Classificação.
    pub class: ResClassification,
    /// Quando.
    pub at: String,
    /// Dia (separador).
    pub day: Option<String>,
}

/// A aplicação Actividade.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActivityVm {
    /// Filtro de ambiente (opções legíveis).
    pub workspaces: Vec<ResOptionVm>,
    /// Eventos.
    pub items: Vec<ActivityItemVm>,
    /// Estado.
    pub load: AppLoad,
    /// Paginação.
    pub page: AppPageVm,
    /// Recusa da aplicação.
    pub error: Option<AppError>,
}

// ── Auditoria ──

/// `outcome`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuditOutcome {
    /// success
    Success,
    /// denied
    Denied,
    /// failure
    Failure,
}

/// Um registo de auditoria.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuditRowVm {
    /// Data e hora precisas no fuso do membro.
    pub at: String,
    /// A mesma em UTC ISO-8601 (`datetime`).
    pub at_utc: String,
    /// Actor (nome histórico) · `None` = sistema.
    pub actor: Option<String>,
    /// Identidade privilegiada, em nome de.
    pub on_behalf: Option<String>,
    /// Acção (código estável).
    pub action: String,
    /// Tipo de recurso (código estável).
    pub resource_type: String,
    /// Resultado.
    pub outcome: AuditOutcome,
    /// Classificação no momento.
    pub class: Option<ResClassification>,
    /// Abrir o detalhe.
    pub href: String,
    /// Aberto.
    pub active: bool,
    /// Filtrar por este actor (`?actor=`).
    pub actor_filter_href: Option<String>,
}

/// O detalhe: só campos seguros.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuditDetailVm {
    /// A linha.
    pub row: AuditRowVm,
    /// Correlação (logs).
    pub correlation: Option<String>,
    /// Metadata por lista branca de chaves (chave, valor).
    pub metadata: Vec<(String, String)>,
    /// Campos omitidos por não estarem na lista branca.
    pub omitted: u32,
    /// O recurso, se o Core o resolve para quem vê.
    pub target: Option<ResLinkVm>,
}

/// A aplicação Auditoria.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuditVm {
    /// Recusa (sem audit.view): nada mais.
    pub error: Option<AppError>,
    /// GET dos filtros.
    pub filter_action: String,
    /// Tipos de recurso conhecidos.
    pub types: Vec<ResOptionVm>,
    /// Desde (data).
    pub since: String,
    /// Filtro de actor activo (nome), com limpar.
    pub actor: Option<(String, String)>,
    /// Linhas (servidor pagina).
    pub rows: Vec<AuditRowVm>,
    /// Estado.
    pub load: AppLoad,
    /// Paginação.
    pub page: AppPageVm,
    /// Lista | detalhe.
    pub pane: ResPane,
    /// Aberto.
    pub detail: Option<AuditDetailVm>,
    /// A lista.
    pub list_href: String,
}

// ── Definições ──

/// A secção.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsSection {
    /// Conta e avatar.
    Account,
    /// Idioma e fuso.
    Language,
    /// Palavra-passe, segundo factor, sessões.
    Security,
    /// Aplicações fixadas.
    Apps,
}

/// Uma sessão própria.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnSessionVm {
    /// Agente resumido.
    pub agent: String,
    /// Última actividade.
    pub seen: String,
    /// Esta.
    pub current: bool,
    /// Terminar.
    pub revoke_action: Option<String>,
}

/// Uma aplicação fixável.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PinVm {
    /// id.
    pub id: String,
    /// Rótulo.
    pub label: String,
    /// Ícone.
    pub icon: &'static str,
    /// Fixada.
    pub pinned: bool,
}

/// A aplicação Definições (só a camada do membro).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SettingsVm {
    /// Secção.
    pub section: SettingsSection,
    /// Navegação.
    pub nav: Vec<AppNavVm>,
    /// O que acabou de guardar.
    pub saved: bool,
    /// Falha.
    pub error: Option<AppError>,
    /// Conta: nome, endereço, estado, avatar.
    pub name: String,
    /// Endereço.
    pub email: String,
    /// Avatar.
    pub avatar: OrgAvatarVm,
    /// Presets do produto (id, rótulo, selecionado).
    pub presets: Vec<ResOptionVm>,
    /// Idioma: pt/en/fr (valor, rótulo nativo, actual).
    pub locales: Vec<ResOptionVm>,
    /// Fuso da Instância (herdado, só leitura).
    pub timezone: String,
    /// Segundo factor: configurado / exigido.
    pub mfa: (bool, bool),
    /// Palavra-passe mudada em.
    pub password_changed: Option<String>,
    /// Sessões próprias.
    pub sessions: Vec<OwnSessionVm>,
    /// Aplicações fixáveis.
    pub pins: Vec<PinVm>,
    /// Origem das fixadas: `member` | `instance` | `product`, traduzida.
    pub pins_source: String,
}

// ── Ajuda ──

/// Um tópico: gerado do registo de aplicações ou do conjunto de atalhos.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HelpTopicVm {
    /// Título.
    pub title: String,
    /// Ícone.
    pub icon: &'static str,
    /// Categoria (traduzida).
    pub category: String,
    /// Descrição do registo.
    pub body: String,
    /// Abrir a aplicação (se o membro a vê).
    pub open_href: Option<String>,
    /// Âncora.
    pub anchor: String,
}

/// Um atalho declarado pelo runtime.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HelpShortcutVm {
    /// Teclas («⌘K»).
    pub keys: String,
    /// O que faz.
    pub what: String,
}

/// A secção.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HelpSection {
    /// As aplicações.
    Apps,
    /// Atalhos.
    Shortcuts,
}

/// A aplicação Ajuda (conteúdo de primeira parte, versionado com o código).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HelpVm {
    /// Secção.
    pub section: HelpSection,
    /// Navegação.
    pub nav: Vec<AppNavVm>,
    /// Pesquisa (`None` = sem campo).
    pub query: Option<String>,
    /// Tópicos (filtrados no servidor).
    pub topics: Vec<HelpTopicVm>,
    /// Atalhos.
    pub shortcuts: Vec<HelpShortcutVm>,
    /// Abrir a Nye com a pergunta.
    pub nye: Option<AppNyeVm>,
}

// ══ D007.1 · Monitor de Actividade · Resultados · Lixo ══════════════════════

/// Um plano de métrica. A lista dos disponíveis vem do Core/runtime
/// (`available_metric_planes`); a vista nunca a deduz de nomes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetricPlane {
    /// Utilização de CPU.
    Cpu,
    /// Memória em uso.
    Memory,
    /// Disco em uso.
    Storage,
    /// Rede.
    Network,
    /// Utilização de GPU.
    Gpu,
}

impl MetricPlane {
    /// A chave do rótulo.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Cpu => "mon.plane.cpu",
            Self::Memory => "mon.plane.memory",
            Self::Storage => "mon.plane.storage",
            Self::Network => "mon.plane.network",
            Self::Gpu => "mon.plane.gpu",
        }
    }
    /// O identificador estável (`?plane=`).
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Memory => "memory",
            Self::Storage => "storage",
            Self::Network => "network",
            Self::Gpu => "gpu",
        }
    }
}

/// A frescura de uma leitura. O limiar é do Core (o mesmo do heartbeat).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Freshness {
    /// Reportado dentro do intervalo do heartbeat.
    Live,
    /// O último valor é antigo: mostra-se com a hora, nunca como actual.
    Stale,
    /// O nó não reporta este valor.
    Unreported,
}

/// O consumo de uma fonte (nó) no plano escolhido. Sem identificadores internos
/// de processo; a ficha do nó é da Computação.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MonitorSampleVm {
    /// Identificador estável do nó (não é segredo).
    pub source: String,
    /// Nome do nó.
    pub name: String,
    /// Estado do nó.
    pub status: NodeStatus,
    /// Em uso, formatado (`12,4 GB`). `None` = não reportado.
    pub used: Option<String>,
    /// Total físico, formatado.
    pub total: Option<String>,
    /// 0–100, só com uso e total reportados.
    pub pct: Option<u8>,
    /// Hora da leitura (fuso do membro).
    pub seen: Option<String>,
    /// Frescura.
    pub freshness: Freshness,
    /// A ficha canónica (`/compute/{id}`), se o membro a alcança.
    pub compute_href: Option<String>,
}

/// O resumo de `GET /system/operations`: contagens, sem nomes de membros.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MonitorSummaryVm {
    /// Nós em linha.
    pub nodes_online: u32,
    /// Nós registados.
    pub nodes_total: u32,
    /// Fornecedores de IA saudáveis.
    pub providers_healthy: u32,
    /// Fornecedores de IA registados.
    pub providers_total: u32,
    /// Aplicações activas.
    pub apps_active: u32,
    /// Aplicações do catálogo.
    pub apps_total: u32,
    /// Membros em aviso.
    pub storage_warning: u32,
    /// Membros em estado crítico.
    pub storage_critical: u32,
    /// Membros acima da quota.
    pub storage_over: u32,
    /// Armazenamento pessoal em uso, formatado.
    pub personal_used: String,
}

/// Um serviço do runtime (MONITOR-06; ainda sem contrato). Só campos seguros:
/// nunca linha de comando, variáveis de ambiente, credenciais ou segredos.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MonitorServiceVm {
    /// Rótulo.
    pub label: String,
    /// Estado (`mon.svc.state.*`).
    pub state: ResStateVm,
    /// O nó onde corre.
    pub source: Option<String>,
    /// O consumo no plano actual, formatado.
    pub value: Option<String>,
    /// Protegido (do Core: `may_stop = false`).
    pub protected: bool,
    /// O motivo da protecção, quando o Core o dá.
    pub protection_key: Option<&'static str>,
    /// «Parar» (`OrgActionKind::StopService`) só com `may_stop = true`.
    pub stop: Option<OrgActionVm>,
}

/// O recibo de um pedido de paragem (MONITOR-10). Assíncrono: diz «a parar»
/// até o Core dizer «parado».
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MonitorReceiptVm {
    /// Identificador da operação.
    pub action_id: String,
    /// O serviço.
    pub target: String,
    /// `mon.receipt.stopping` / `mon.receipt.stopped`.
    pub state_key: &'static str,
    /// Hora do pedido.
    pub at: String,
    /// O registo de auditoria, para quem o pode ver.
    pub audit_href: Option<String>,
}

/// Recusas tipadas de uma paragem.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MonitorStopRefusal {
    /// Serviço protegido (invariante do Core).
    Protected,
    /// Deixou de existir antes da confirmação.
    Gone,
    /// Mudou de estado.
    Changed,
    /// Sem administração da plataforma.
    Denied,
}

/// A aplicação Monitor de Actividade.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MonitorVm {
    /// Carregamento.
    pub load: AppLoad,
    /// Recusa ou falha da leitura inteira (sem administração: `PermissionDenied`).
    pub error: Option<AppError>,
    /// Hora da leitura.
    pub read_at: Option<String>,
    /// Voltar a ler (GET).
    pub refresh_href: String,
    /// Resumo.
    pub summary: Option<MonitorSummaryVm>,
    /// Os planos suportados, com a ligação de cada um.
    pub planes: Vec<(MetricPlane, String)>,
    /// Os planos que os nós não reportam (nomeados, nunca zero).
    pub unsupported: Vec<MetricPlane>,
    /// O plano actual (`None` = nenhum suportado).
    pub plane: Option<MetricPlane>,
    /// Consumo por nó.
    pub samples: Vec<MonitorSampleVm>,
    /// Inventário de serviços (`None` = o runtime não o publica).
    pub services: Option<Vec<MonitorServiceVm>>,
    /// Recibo do último pedido.
    pub receipt: Option<MonitorReceiptVm>,
    /// Recusa do último pedido.
    pub refusal: Option<MonitorStopRefusal>,
    /// Ocinye AI (fornecedores), para quem a abre.
    pub ai_href: Option<String>,
    /// Administração › Instância (aplicações), para quem a abre.
    pub apps_href: Option<String>,
}

/// `results.status` (0019).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResultStatus {
    /// draft
    Draft,
    /// under_review
    UnderReview,
    /// validated
    Validated,
    /// superseded
    Superseded,
    /// invalidated
    Invalidated,
}

/// `result_validations.kind`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValidationKind {
    /// validation
    Validation,
    /// reproduction
    Reproduction,
}

/// `result_validations.outcome` (sem valor por omissão de sucesso).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValidationOutcome {
    /// confirmed
    Confirmed,
    /// contradicted
    Contradicted,
    /// inconclusive
    Inconclusive,
}

/// Uma validação ou reprodução registada.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResultValidationVm {
    /// Tipo.
    pub kind: ValidationKind,
    /// Desfecho.
    pub outcome: ValidationOutcome,
    /// Quem registou (nome), se ainda resolvível.
    pub by: Option<String>,
    /// Quando.
    pub at: String,
    /// Nota (texto simples, escapado).
    pub note: Option<String>,
    /// A execução que serviu de prova, se alcançável.
    pub execution: Option<ResLinkVm>,
}

/// Um resultado aberto.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResultVm {
    /// Título.
    pub title: String,
    /// Estado.
    pub status: ResultStatus,
    /// Classificação.
    pub class: ResClassification,
    /// Conclusão (`summary`).
    pub summary: String,
    /// Ambiente.
    pub workspace: Option<ResLinkVm>,
    /// Projecto.
    pub project: Option<ResLinkVm>,
    /// Execução de origem.
    pub execution: Option<ResLinkVm>,
    /// Substituído por.
    pub superseded_by: Option<ResLinkVm>,
    /// Registado por.
    pub created_by: Option<String>,
    /// Registado.
    pub created: String,
    /// Actualizado.
    pub updated: String,
    /// Validações.
    pub validations: Vec<ResultValidationVm>,
    /// Linhagem (`/lineage/result/{id}`), já reautorizada ponta a ponta.
    pub lineage: Vec<ResLinkVm>,
    /// `/results/{id}/validate`, quando o Core a oferece a esta pessoa.
    pub validate_href: Option<String>,
    /// Perguntar à Nye sobre este resultado.
    pub nye: Option<AppNyeVm>,
}

/// A aplicação Resultados.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResultsVm {
    /// Estados (filtro).
    pub nav: Vec<AppNavVm>,
    /// Ambientes do membro (filtro).
    pub workspaces: Vec<ResOptionVm>,
    /// Para onde o filtro envia.
    pub filter_action: String,
    /// A lista.
    pub list: ResListVm,
    /// Painel.
    pub pane: ResPane,
    /// Voltar à lista.
    pub list_href: String,
    /// O resultado aberto.
    pub result: Option<ResultVm>,
    /// Erro do pedido directo.
    pub result_error: Option<AppError>,
}

/// O tipo de um item no Lixo (só os que o Core tem com `deleted_at` pessoal).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrashKind {
    /// Ficheiro pessoal.
    File,
    /// Nota pessoal.
    Note,
}

/// Um item no Lixo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrashItemVm {
    /// Tipo.
    pub kind: TrashKind,
    /// Identificador opaco do formulário.
    pub id: String,
    /// Nome (escapado).
    pub name: String,
    /// Quando foi apagado.
    pub deleted: String,
    /// Quem apagou (notas: `deleted_by_id`), se diferente do próprio.
    pub deleted_by: Option<String>,
    /// Onde estava (pasta), se ainda existe.
    pub origin: Option<String>,
    /// Tamanho (ficheiros).
    pub size: Option<String>,
    /// Continua a contar para a quota (ficheiros).
    pub counts_storage: bool,
    /// POST de restauro (a rota BFF existente), quando o Core o permite.
    pub restore_action: Option<String>,
}

/// O que acabou de acontecer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrashNotice {
    /// Restaurado.
    Restored,
}

/// Recusas tipadas do restauro.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrashRefusal {
    /// Já não está no Lixo.
    Gone,
    /// O Core recusou.
    Denied,
    /// O lugar de origem mudou.
    Conflict,
}

/// A aplicação Lixo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrashVm {
    /// Tudo · Ficheiros · Notas (com contagens do Core).
    pub nav: Vec<AppNavVm>,
    /// A lista.
    pub list: ResListVm,
    /// Painel.
    pub pane: ResPane,
    /// Voltar à lista.
    pub list_href: String,
    /// O item aberto.
    pub item: Option<TrashItemVm>,
    /// Erro do pedido directo.
    pub item_error: Option<AppError>,
    /// Aviso.
    pub notice: Option<TrashNotice>,
    /// O nome no aviso.
    pub notice_name: Option<String>,
    /// Recusa.
    pub refusal: Option<TrashRefusal>,
}

// ═══ D008 · Interfaces de sistema — modelos de vista (anexar a ui/view_models.rs) ═══
// ADITIVO. Nenhum campo de D001–D007.1 muda. Sem handles de processo, sem ponteiros
// de webview, sem tokens: só o que se desenha.

// ── D008-A · Terminal (ocsh) ─────────────────────────────────────────────

/// O Terminal: uma sessão por janela (SingleInstance; separadores DEFERRED, HANDOFF §T-10).
#[derive(Debug, Clone)]
pub struct TerminalVm {
    /// O contexto activo, como o Core o resolveu (`ocsh::wire::ContextView`).
    pub context: TermContextVm,
    /// `false` → prompt desactivado, «Sem ligação ao Core» (nada é enviado).
    pub core_online: bool,
    /// A versão do ocsh (`crate::terminal::OCSH_VERSION`).
    pub version: &'static str,
    /// Descoberta: os comandos que o registo mostra a esta pessoa (`Audience`),
    /// para ajuda imediata e autocompletar. **Não é autorização**: o Core decide.
    pub registry: Vec<TermRegistryEntryVm>,
    /// SSR desenha a sessão nova (boas-vindas); as respostas desenham-se no cliente.
    /// Preenchido só num percurso sem JS (HANDOFF §T-06).
    pub scrollback: Vec<TermEntryVm>,
}

#[derive(Debug, Clone)]
pub struct TermContextVm {
    /// `None` = pessoal.
    pub id: Option<String>,
    /// `pessoal` / código do ambiente (`WSUENR01`). Texto de dados.
    pub label: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TermGroup {
    Shell,
    Workspace,
    Work,
    Ai,
    System,
    Admin,
}

impl TermGroup {
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Shell => "ocsh.group.shell",
            Self::Workspace => "ocsh.group.workspace",
            Self::Work => "ocsh.group.work",
            Self::Ai => "ocsh.group.ai",
            Self::System => "ocsh.group.system",
            Self::Admin => "ocsh.group.admin",
        }
    }
}

/// Uma linha da ajuda/autocompletar, derivada de `ocsh::registry::COMMANDS`.
#[derive(Debug, Clone)]
pub struct TermRegistryEntryVm {
    /// `context use <target>` — sintaxe canónica, nunca traduzida.
    pub usage: String,
    /// `context use` — o que o autocompletar escreve.
    pub completion: String,
    /// `ocsh.cmd.context.use`.
    pub help_key: &'static str,
    pub group: TermGroup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TermTone {
    Ok,
    Info,
    Warn,
    Err,
    Deny,
}

impl TermTone {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Err => "err",
            Self::Deny => "deny",
        }
    }
    #[must_use]
    pub const fn icon(self) -> &'static str {
        match self {
            Self::Ok => "check",
            Self::Info => "status",
            Self::Warn | Self::Err => "warning",
            Self::Deny => "lock",
        }
    }
    /// O tom de um código de saída do ocsh (`ocsh::ExitCode`).
    #[must_use]
    pub const fn of_exit(code: u8) -> Self {
        match code {
            0 => Self::Ok,
            2 | 69 | 127 => Self::Warn,
            77 | 126 => Self::Deny,
            130 => Self::Info,
            _ => Self::Err,
        }
    }
}

/// Uma linha executada e o que o Core devolveu.
#[derive(Debug, Clone)]
pub struct TermEntryVm {
    /// A linha **redigida** (`ocsh::redact`). Nunca a linha crua.
    pub echo: String,
    pub context_label: String,
    /// `None` = à espera de confirmação.
    pub exit: Option<u8>,
    pub ms: Option<u64>,
    /// A capability que correu (transparência; não é controlo).
    pub capability: Option<String>,
    pub blocks: Vec<TermBlockVm>,
}

/// Blocos de saída, já localizados pelo Workspace (`crate::terminal::localize`).
#[derive(Debug, Clone)]
pub enum TermBlockVm {
    Note {
        tone: TermTone,
        title: String,
        body: Option<String>,
        suggestions: Vec<String>,
    },
    Table {
        columns: Vec<String>,
        rows: Vec<Vec<String>>,
        pipeline: Option<String>,
        empty: Option<String>,
    },
    Facts {
        rows: Vec<(String, String)>,
    },
    Help {
        groups: Vec<(String, Vec<(String, String)>)>,
        footer: Option<String>,
    },
    /// TERMINAL-13 · contrato futuro: `ocsh::wire::Block::Link` ainda não existe.
    Links {
        items: Vec<TermLinkVm>,
        note: String,
    },
    /// A ponte explícita `nye ask` / `? …`. Só resposta e fontes — nunca raciocínio.
    Nye {
        paragraphs: Vec<String>,
        sources: Vec<String>,
    },
    /// TERMINAL-11 · contrato futuro.
    Receipt(TermReceiptVm),
}

#[derive(Debug, Clone)]
pub struct TermLinkVm {
    pub title: String,
    /// A rota canónica da aplicação dona (que reautoriza). Nunca um token.
    pub href: String,
    pub icon: &'static str,
    pub app_label: String,
}

#[derive(Debug, Clone)]
pub struct TermReceiptVm {
    pub id: String,
    pub at: String,
    pub capability: String,
    pub audit_href: Option<String>,
}

/// TERMINAL-11 · o plano congelado que a confirmação mostra (`ActionPlan` de um passo).
#[derive(Debug, Clone)]
pub struct TermPlanVm {
    pub id: String,
    pub id_short: String,
    pub action_label: String,
    /// A linha redigida que originou o plano — só para mostrar; o Core não a relê.
    pub echo: String,
    pub capability: String,
    /// `ocinye_contracts::agentic::RiskLevel` (os cinco níveis do Core; nenhum novo).
    pub risk: ocinye_contracts::agentic::RiskLevel,
    pub context_label: String,
    pub target_label: String,
    pub valid_until: String,
}

/// A chave i18n de um nível de risco do Core.
#[must_use]
pub const fn risk_key(r: ocinye_contracts::agentic::RiskLevel) -> &'static str {
    use ocinye_contracts::agentic::RiskLevel as R;
    match r {
        R::ReadOnly => "term.risk.read_only",
        R::LowImpact => "term.risk.low_impact",
        R::MaterialMutation => "term.risk.material_mutation",
        R::ExternalEffect => "term.risk.external_effect",
        R::Privileged => "term.risk.privileged",
    }
}

// ── D008-B · Browser ─────────────────────────────────────────────────────

/// O runtime declarado ao Workspace (`ocinye_contracts::runtime::RuntimeMode`).
/// Sem aperto de mão da casca (ADR-0704), é sempre `Web`.
pub type BrowserRuntime = ocinye_contracts::runtime::RuntimeMode;

#[derive(Debug, Clone)]
pub struct BrowserVm {
    pub runtime: BrowserRuntime,
    /// Abas por id interno (nunca o URL).
    pub tabs: Vec<BrowserTabVm>,
    pub active: usize,
    pub navigation: BrowserNavigationVm,
    pub page: BrowserPageStateVm,
    /// Linhas de cromado entre a barra e o conteúdo (sempre acima da fronteira).
    pub notices: Vec<BrowserNoticeVm>,
    pub side: Option<BrowserSideVm>,
    /// `true` só quando o runtime entrega transferências (Desktop/Dedicado).
    pub downloads_supported: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserTabState {
    New,
    Ready,
    Loading,
    Failed,
}

#[derive(Debug, Clone)]
pub struct BrowserTabVm {
    pub id: String,
    /// Texto de dados (título da página ou host). Escapado; nunca HTML.
    pub title: String,
    pub origin: Option<String>,
    pub state: BrowserTabState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserSecurity {
    Https,
    Http,
    Blocked,
}

#[derive(Debug, Clone)]
pub struct BrowserNavigationVm {
    /// `None` na Web: o histórico do site não é observável, os botões não se desenham.
    pub can_back: Option<bool>,
    pub can_forward: Option<bool>,
    pub loading: bool,
    /// Web: o endereço **pedido**. Desktop: o endereço actual do webview.
    pub url: String,
    /// Host normalizado pelo parser WHATWG/runtime (punycode quando é o caso).
    pub host: Option<String>,
    pub security: Option<BrowserSecurity>,
    /// O texto escrito, quando a pessoa escreveu algo que não navegou.
    pub typed: Option<String>,
}

#[derive(Debug, Clone)]
pub enum BrowserPageStateVm {
    NewTab,
    /// Há uma página externa. `origin` = `https://host[:porta]`.
    External {
        origin: String,
    },
    WebMayBeBlocked {
        url: String,
    },
    Invalid {
        text: String,
    },
    BlockedScheme {
        scheme: String,
    },
    Failed {
        host: String,
    },
    Certificate {
        host: String,
    },
    Crashed,
    Loading {
        host: String,
    },
    /// Desktop/Dedicado declarado, mas sem ponte `browser.*` — nunca sucesso simulado.
    NoRuntime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserPermissionKind {
    Microphone,
    Camera,
    Location,
    Notifications,
    Clipboard,
}

impl BrowserPermissionKind {
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Microphone => "brw.perm.mic",
            Self::Camera => "brw.perm.cam",
            Self::Location => "brw.perm.geo",
            Self::Notifications => "brw.perm.notif",
            Self::Clipboard => "brw.perm.clip",
        }
    }
    #[must_use]
    pub const fn icon(self) -> &'static str {
        match self {
            Self::Microphone => "ob-mic",
            Self::Camera => "ob-cam",
            Self::Location => "ob-geo",
            Self::Notifications => "ob-bell",
            Self::Clipboard => "copy",
        }
    }
}

/// Um pedido de permissão de um site. Permissão do site, nunca do Ocinye.
#[derive(Debug, Clone)]
pub struct BrowserPermissionVm {
    pub request_id: String,
    pub origin: String,
    pub kind: BrowserPermissionKind,
}

#[derive(Debug, Clone)]
pub enum BrowserNoticeVm {
    WebLimits { url: String },
    WebOpened { origin: String },
    WebPermissions,
    WebDownloads,
    Permission(BrowserPermissionVm),
    PopupTab { origin: String },
    PopupBlocked { origin: String },
    FullscreenDenied { origin: String },
    Idn { host: String },
}

#[derive(Debug, Clone)]
pub enum BrowserSideVm {
    Nye(BrowserNyeVm),
    Downloads(Vec<BrowserDownloadVm>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserExtractionKind {
    Selection,
    Main,
}

/// BROWSER-17 · o que seria enviado à Nye, antes de enviar. Limitado e com proveniência.
#[derive(Debug, Clone)]
pub struct BrowserExtractionVm {
    pub url: String,
    pub title: String,
    pub kind: BrowserExtractionKind,
    pub chars: usize,
    /// O limite da política (o do Context Engine; não um número inventado aqui).
    pub max_chars: usize,
    pub read_at: String,
    /// Um excerto do texto extraído, para a pessoa ver o que vai.
    pub excerpt: String,
    /// Chave i18n da descrição da política de egresso que o roteamento de IA aplicou.
    pub egress_key: &'static str,
}

#[derive(Debug, Clone)]
pub enum BrowserNyeVm {
    WebUnavailable,
    NoAi,
    Preview {
        extraction: BrowserExtractionVm,
        question: String,
    },
    /// `mentions_instructions` vem do envelope da resposta; é aviso, não garantia.
    Answer {
        paragraphs: Vec<String>,
        mentions_instructions: bool,
        extraction: BrowserExtractionVm,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserDownloadState {
    Pending,
    Downloading,
    Complete,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone)]
pub enum BrowserDownloadDest {
    Host,
    Files { folder: String },
}

#[derive(Debug, Clone)]
pub struct BrowserDownloadVm {
    pub id: String,
    /// O nome **saneado** (sem separadores, `..`, reservados, controlo, bidi; NFC).
    pub file_name: String,
    /// O nome que o site sugeriu, quando foi alterado (mostrado com caracteres visíveis).
    pub suggested: Option<String>,
    pub origin: String,
    pub state: BrowserDownloadState,
    pub done: Option<String>,
    pub total: Option<String>,
    /// Só quando o runtime dá bytes totais. Nunca inventado.
    pub pct: Option<u8>,
    pub destination: Option<BrowserDownloadDest>,
}
