//! A identidade semântica dos ícones.
//!
//! O sprite antigo (`static/icons.svg`) saiu com o D13: o que se desenha é
//! sempre o sprite do Claude Design (`static/ods-icons.svg`), e
//! `ui::ods::icone_do_legado` traduz cada variante para o símbolo dele. Esta
//! enumeração fica como vocabulário tipado — o ícone de um ecrã, de uma acção —
//! e não como referência a um ficheiro.

/// O ícone de um ecrã ou de uma acção, pelo seu significado.
///
/// Enumeração fechada em vez de string: um nome mal escrito passaria em
/// silêncio. O símbolo desenhado é o do sprite do Claude Design
/// (`ui::ods::icone_do_legado`).
#[allow(
    dead_code,
    reason = "vocabulário fechado; uma variante sem ecrã hoje não é código morto"
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    // Login
    User,
    Lock,
    ArrowRight,
    Power,
    Restart,
    SystemStatus,
    /// O Ocinye Browser (D008).
    Browser,
    // Shell
    SidebarCollapse,
    ChevronUp,
    Search,
    Plus,
    /// Fechar uma janela.
    ///
    /// O conjunto não tinha nenhum, e o compositor fechava com uma seta — que
    /// se lê como «seguinte». Acrescentar ao conjunto canónico é o contrário
    /// de abrir um segundo: um glifo em falta preenche-se onde os outros
    /// vivem.
    Close,
    Bell,
    /// Calendário.
    Calendar,
    Filter,
    Settings,
    Help,
    // Navegação
    Home,
    MyWork,
    /// O lançador de aplicações — uma grelha 3×3.
    Apps,
    Units,
    Idea,
    Project,
    Knowledge,
    Science,
    Bibliography,
    Data,
    Ai,
    Agent,
    Compute,
    Activity,
    Admin,
    Audit,
    // Inteligência
    AiHexLg,
    AiHexMd,
    Shield,
    Attach,
    Dataset,
    Document,
    /// Ficheiros institucionais.
    Files,
    /// Uma pasta de navegação.
    Folder,
    Tools,
    Send,
    Mail,
    /// Mensagens.
    Messaging,
    Star,
    Reply,
    Archive,
    Trash,
    // Estados vazios
    ComputeLg,
    EmptyState,
}

impl Icon {
    /// O `id` do símbolo no sprite.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::User => "oc-user",
            Self::Lock => "oc-lock",
            Self::ArrowRight => "oc-arrow-right",
            Self::Power => "oc-power",
            Self::Restart => "oc-restart",
            Self::SystemStatus => "oc-system-status",
            Self::Browser => "oc-browser",
            Self::SidebarCollapse => "oc-sidebar-collapse",
            Self::ChevronUp => "oc-chevron-up",
            Self::Search => "oc-search",
            Self::Plus => "oc-plus",
            Self::Close => "oc-close",
            Self::Bell => "oc-bell",
            Self::Calendar => "oc-calendar",
            Self::Filter => "oc-filter",
            Self::Settings => "oc-settings",
            Self::Help => "oc-help",
            Self::Home => "oc-home",
            Self::MyWork => "oc-my-work",
            Self::Apps => "oc-apps",
            Self::Units => "oc-units",
            Self::Idea => "oc-idea",
            Self::Project => "oc-project",
            Self::Knowledge => "oc-knowledge",
            Self::Science => "oc-science",
            Self::Bibliography => "oc-bibliography",
            Self::Data => "oc-data",
            Self::Ai => "oc-ai",
            Self::Agent => "oc-agent",
            Self::Compute => "oc-compute",
            Self::Activity => "oc-activity",
            Self::Admin => "oc-admin",
            Self::Audit => "oc-audit",
            Self::AiHexLg => "oc-ai-hex-lg",
            Self::AiHexMd => "oc-ai-hex-md",
            Self::Shield => "oc-shield",
            Self::Attach => "oc-attach",
            Self::Dataset => "oc-dataset",
            Self::Document => "oc-document",
            Self::Files => "oc-files",
            Self::Folder => "oc-folder",
            Self::Tools => "oc-tools",
            Self::Send => "oc-send",
            Self::Mail => "oc-mail",
            Self::Messaging => "oc-messaging",
            Self::Star => "oc-star",
            Self::Reply => "oc-reply",
            Self::Archive => "oc-archive",
            Self::Trash => "oc-trash",
            Self::ComputeLg => "oc-compute-lg",
            Self::EmptyState => "oc-empty-state",
        }
    }
}
