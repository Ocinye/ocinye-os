//! Ícones.
//!
//! O conjunto vem exclusivamente de `static/icons.svg` — os 44 símbolos do
//! design, em traço fino e `currentColor`. Não se misturam bibliotecas com
//! pesos de traço diferentes: a coerência do conjunto depende disso
//! (`design/icons/ICONS.md`).

use leptos::prelude::*;

/// Os ícones do design, pelo `id` do sprite.
///
/// Enumeração fechada em vez de string: um `id` mal escrito passaria em silêncio
/// e renderizaria um espaço vazio.
///
/// # Porque é um catálogo e não uma lista de utilização
///
/// `Icon` espelha `static/icons.svg`, que é o dossier de design. Dois testes
/// garantem que os dois se cobrem **mutuamente**: nenhum símbolo no sprite sem
/// variante, nenhuma variante sem símbolo.
///
/// Daí o `allow`: uma variante que nenhum ecrã usa hoje não é código morto, é o
/// catálogo a estar completo. Apagá-la porque o compilador a assinala
/// desalinharia o catálogo do dossier, e o teste inverso passaria a falhar por
/// uma razão que ninguém entenderia meses depois.
#[allow(
    dead_code,
    reason = "o catálogo espelha o sprite; uma variante sem ecrã é o catálogo completo, não código morto"
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
    // Shell
    SidebarCollapse,
    ChevronUp,
    Search,
    Plus,
    Bell,
    /// Calendário.
    Calendar,
    Filter,
    Settings,
    Help,
    // Navegação
    Home,
    MyWork,
    Units,
    Idea,
    Project,
    Knowledge,
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
    Tools,
    Send,
    Mail,
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
            Self::SidebarCollapse => "oc-sidebar-collapse",
            Self::ChevronUp => "oc-chevron-up",
            Self::Search => "oc-search",
            Self::Plus => "oc-plus",
            Self::Bell => "oc-bell",
            Self::Calendar => "oc-calendar",
            Self::Filter => "oc-filter",
            Self::Settings => "oc-settings",
            Self::Help => "oc-help",
            Self::Home => "oc-home",
            Self::MyWork => "oc-my-work",
            Self::Units => "oc-units",
            Self::Idea => "oc-idea",
            Self::Project => "oc-project",
            Self::Knowledge => "oc-knowledge",
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
            Self::Tools => "oc-tools",
            Self::Send => "oc-send",
            Self::Mail => "oc-mail",
            Self::Star => "oc-star",
            Self::Reply => "oc-reply",
            Self::Archive => "oc-archive",
            Self::Trash => "oc-trash",
            Self::ComputeLg => "oc-compute-lg",
            Self::EmptyState => "oc-empty-state",
        }
    }

    /// O `viewBox` original do símbolo.
    ///
    /// Necessário porque o sprite mistura três grelhas (14, 16 e 32) e um
    /// `viewBox` errado deforma o ícone.
    #[must_use]
    pub const fn view_box(self) -> &'static str {
        match self {
            Self::Plus => "0 0 12 12",
            Self::User
            | Self::Lock
            | Self::ArrowRight
            | Self::Power
            | Self::Restart
            | Self::SystemStatus
            | Self::SidebarCollapse
            | Self::Filter
            | Self::Attach
            | Self::Dataset
            | Self::Document
            | Self::Tools => "0 0 14 14",
            Self::AiHexLg | Self::AiHexMd | Self::ComputeLg => "0 0 32 32",
            _ => "0 0 16 16",
        }
    }
}

/// Renderiza um ícone.
///
/// Decorativo por omissão (`aria-hidden`): quase todos acompanham texto, e
/// anunciá-los duplicaria o rótulo. Quando o ícone é a única etiqueta de um
/// controlo, o rótulo acessível pertence ao botão, não a este elemento.
pub fn icon(kind: Icon, size: u16) -> impl IntoView {
    view! {
        <svg
            width=size
            height=size
            viewBox=kind.view_box()
            fill="none"
            stroke="currentColor"
            stroke-width="1.4"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
            focusable="false"
        >
            <use href=format!("/static/icons.svg#{}", kind.id())></use>
        </svg>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// O catálogo completo do dossier, para verificação cruzada com
    /// `static/icons.svg`.
    ///
    /// O sprite é a fonte de verdade. Se um símbolo for removido ou renomeado
    /// lá, os dois testes abaixo falham em vez de a interface renderizar um
    /// espaço vazio.
    const ALL: [Icon; 44] = [
        Icon::User,
        Icon::Lock,
        Icon::ArrowRight,
        Icon::Power,
        Icon::Restart,
        Icon::SystemStatus,
        Icon::SidebarCollapse,
        Icon::ChevronUp,
        Icon::Search,
        Icon::Plus,
        Icon::Bell,
        Icon::Calendar,
        Icon::Filter,
        Icon::Settings,
        Icon::Help,
        Icon::Home,
        Icon::MyWork,
        Icon::Units,
        Icon::Idea,
        Icon::Project,
        Icon::Knowledge,
        Icon::Bibliography,
        Icon::Data,
        Icon::Ai,
        Icon::Agent,
        Icon::Compute,
        Icon::Activity,
        Icon::Admin,
        Icon::Audit,
        Icon::AiHexLg,
        Icon::AiHexMd,
        Icon::Shield,
        Icon::Attach,
        Icon::Dataset,
        Icon::Document,
        Icon::Tools,
        Icon::Send,
        Icon::Mail,
        Icon::Star,
        Icon::Reply,
        Icon::Archive,
        Icon::Trash,
        Icon::ComputeLg,
        Icon::EmptyState,
    ];

    #[test]
    fn todos_os_icones_existem_no_sprite() {
        let sprite = include_str!("../../static/icons.svg");
        for kind in ALL {
            assert!(
                sprite.contains(&format!("id=\"{}\"", kind.id())),
                "o símbolo {} não existe em static/icons.svg",
                kind.id()
            );
        }
    }

    #[test]
    fn o_sprite_nao_tem_simbolos_por_declarar() {
        let sprite = include_str!("../../static/icons.svg");
        let declared: Vec<&str> = ALL.iter().map(|kind| kind.id()).collect();

        for line in sprite.lines() {
            if let Some(rest) = line.split("id=\"oc-").nth(1) {
                if let Some(name) = rest.split('"').next() {
                    let full = format!("oc-{name}");
                    assert!(
                        declared.contains(&full.as_str()),
                        "o símbolo {full} existe no sprite mas não em Icon"
                    );
                }
            }
        }
    }

    #[test]
    fn os_viewboxes_seguem_as_tres_grelhas_do_conjunto() {
        assert_eq!(Icon::Plus.view_box(), "0 0 12 12");
        assert_eq!(Icon::Attach.view_box(), "0 0 14 14");
        assert_eq!(Icon::Home.view_box(), "0 0 16 16");
        assert_eq!(Icon::AiHexLg.view_box(), "0 0 32 32");
    }
}
