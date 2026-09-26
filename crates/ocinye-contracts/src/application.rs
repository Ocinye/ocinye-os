//! As aplicações do Ocinye OS, e os perfis de Instância que decidem quais
//! começam activas (ADR-0014).
//!
//! # Porque vive nos contratos
//!
//! Porque o Core e o Workspace têm de concordar sobre o que é uma aplicação. O
//! registo do Workspace (§45-A) diz como cada uma se apresenta; o Core guarda que
//! aplicações uma Instância tem activas, e só pode validar o que guarda se
//! conhecer os identificadores. Sem I/O, compilável para `wasm32`, como o resto
//! deste crate.
//!
//! # O que isto não é
//!
//! Não é autorização. Uma aplicação activa não concede nada a ninguém: a
//! visibilidade de uma aplicação é `activa ∧ autorizada ∧ relevante`, e as duas
//! últimas continuam a ser da política de sempre.

use serde::{Deserialize, Serialize};

/// Uma aplicação do Ocinye OS, pelo seu identificador técnico estável.
///
/// O identificador é o que se persiste (`member_app_pins`, `instance_applications`)
/// e nunca muda. O rótulo traduzido é do Workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum ApplicationId {
    /// A página inicial.
    Home,
    /// O Meu Trabalho.
    Work,
    /// Notas.
    Notes,
    /// Calendário.
    Calendar,
    /// Correio.
    Mail,
    /// Mensagens.
    Messages,
    /// Ficheiros.
    Files,
    /// Conhecimento.
    Knowledge,
    /// Bibliografia.
    Bibliography,
    /// Unidades.
    Units,
    /// Ideias.
    Ideas,
    /// Projectos (e as suas tarefas).
    Projects,
    /// Datasets.
    Datasets,
    /// O Prompt.
    Prompt,
    /// O estado da IA.
    Ai,
    /// Agentes.
    Agents,
    /// Computação.
    Compute,
    /// Meus Recursos.
    Resources,
    /// Actividade.
    Activity,
    /// Administração.
    Administration,
    /// Audit Log.
    Audit,
    /// Definições.
    Settings,
    /// Ajuda.
    Help,
}

/// Se uma aplicação pode ser desactivada numa Instância.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplicationClass {
    /// Sem ela não há sistema operativo: não se desactiva.
    Essential,
    /// Activa ou não por Instância; o perfil dá a predefinição.
    Optional,
}

impl ApplicationId {
    /// Todas, na ordem do registo.
    pub const ALL: [ApplicationId; 23] = [
        Self::Notes,
        Self::Calendar,
        Self::Work,
        Self::Home,
        Self::Mail,
        Self::Messages,
        Self::Files,
        Self::Knowledge,
        Self::Bibliography,
        Self::Units,
        Self::Ideas,
        Self::Projects,
        Self::Datasets,
        Self::Prompt,
        Self::Ai,
        Self::Agents,
        Self::Compute,
        Self::Resources,
        Self::Activity,
        Self::Administration,
        Self::Audit,
        Self::Settings,
        Self::Help,
    ];

    /// O identificador técnico — o que se persiste.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Home => "home",
            Self::Work => "work",
            Self::Notes => "notes",
            Self::Calendar => "calendar",
            Self::Mail => "mail",
            Self::Messages => "messages",
            Self::Files => "files",
            Self::Knowledge => "knowledge",
            Self::Bibliography => "bibliography",
            Self::Units => "units",
            Self::Ideas => "ideas",
            Self::Projects => "projects",
            Self::Datasets => "datasets",
            Self::Prompt => "prompt",
            Self::Ai => "ai",
            Self::Agents => "agents",
            Self::Compute => "compute",
            Self::Resources => "resources",
            Self::Activity => "activity",
            Self::Administration => "administration",
            Self::Audit => "audit",
            Self::Settings => "settings",
            Self::Help => "help",
        }
    }

    /// A classe da aplicação (ADR-0014 §3).
    ///
    /// Essenciais: entrar, trabalhar, guardar, ver os próprios recursos,
    /// administrar, configurar e pedir ajuda. Tudo o resto é opcional.
    #[must_use]
    pub const fn class(self) -> ApplicationClass {
        match self {
            Self::Home
            | Self::Work
            | Self::Files
            | Self::Resources
            | Self::Administration
            | Self::Settings
            | Self::Help => ApplicationClass::Essential,
            _ => ApplicationClass::Optional,
        }
    }

    /// Se a aplicação pode ser desactivada.
    #[must_use]
    pub const fn is_optional(self) -> bool {
        matches!(self.class(), ApplicationClass::Optional)
    }
}

impl TryFrom<String> for ApplicationId {
    type Error = UnknownApplication;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl std::str::FromStr for ApplicationId {
    type Err = UnknownApplication;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|app| app.as_str() == value)
            .ok_or_else(|| UnknownApplication(value.to_owned()))
    }
}

impl From<ApplicationId> for String {
    fn from(value: ApplicationId) -> Self {
        value.as_str().to_owned()
    }
}

impl std::fmt::Display for ApplicationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Um identificador que não nomeia nenhuma aplicação.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownApplication(pub String);

impl std::fmt::Display for UnknownApplication {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "aplicação desconhecida: «{}»", self.0)
    }
}

impl std::error::Error for UnknownApplication {}

/// O perfil de uma Instância (ADR-0014 §1).
///
/// Decide predefinições — aplicações activas, fixações iniciais, estrutura
/// inicial — e nunca autoridade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum InstanceProfile {
    /// Investigação: o Ocinye como a Ocinye o usa — todas as aplicações.
    Research,
    /// Empresa: comunicação, projectos e unidades; sem os módulos científicos.
    Business,
    /// Educação: como a empresa, mais conhecimento e bibliografia.
    Education,
    /// Pessoal: uma pessoa e as suas coisas.
    Personal,
}

impl InstanceProfile {
    /// Os quatro, na ordem em que se apresentam.
    pub const ALL: [InstanceProfile; 4] = [
        Self::Research,
        Self::Business,
        Self::Education,
        Self::Personal,
    ];

    /// O identificador persistido.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Research => "research",
            Self::Business => "business",
            Self::Education => "education",
            Self::Personal => "personal",
        }
    }

    /// Se esta aplicação começa activa neste perfil (ADR-0014 §4).
    ///
    /// Uma aplicação essencial está sempre activa, em qualquer perfil.
    #[must_use]
    pub const fn activates(self, app: ApplicationId) -> bool {
        use ApplicationId as A;
        if !app.is_optional() {
            return true;
        }
        match self {
            Self::Research => true,
            Self::Business => matches!(
                app,
                A::Notes
                    | A::Calendar
                    | A::Mail
                    | A::Prompt
                    | A::Messages
                    | A::Activity
                    | A::Audit
                    | A::Units
                    | A::Projects
            ),
            Self::Education => matches!(
                app,
                A::Notes
                    | A::Calendar
                    | A::Mail
                    | A::Prompt
                    | A::Messages
                    | A::Activity
                    | A::Audit
                    | A::Units
                    | A::Projects
                    | A::Knowledge
                    | A::Bibliography
            ),
            Self::Personal => matches!(app, A::Notes | A::Calendar | A::Mail | A::Prompt),
        }
    }

    /// Se a estrutura inicial de unidades é semeada (ADR-0014 §7).
    #[must_use]
    pub const fn seeds_initial_units(self) -> bool {
        matches!(self, Self::Research)
    }
}

impl TryFrom<String> for InstanceProfile {
    type Error = UnknownProfile;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl std::str::FromStr for InstanceProfile {
    type Err = UnknownProfile;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|profile| profile.as_str() == value.trim())
            .ok_or_else(|| UnknownProfile(value.to_owned()))
    }
}

impl From<InstanceProfile> for String {
    fn from(value: InstanceProfile) -> Self {
        value.as_str().to_owned()
    }
}

/// Um valor que não nomeia nenhum perfil.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownProfile(pub String);

impl std::fmt::Display for UnknownProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "perfil desconhecido: «{}» (research, business, education ou personal)",
            self.0
        )
    }
}

impl std::error::Error for UnknownProfile {}

// ── O manifesto (ADR-0016) ──────────────────────────────────────────────

/// A versão do contrato de manifesto. Muda quando um campo muda de
/// significado, e não quando uma aplicação muda.
pub const MANIFEST_VERSION: u32 = 1;

/// A família em que uma aplicação se apresenta no lançador.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplicationCategory {
    /// Notas, calendário, trabalho.
    Productivity,
    /// Correio e mensagens.
    Communication,
    /// Ficheiros, conhecimento, bibliografia.
    Knowledge,
    /// Unidades, ideias, projectos, dados, IA.
    Research,
    /// Recursos, actividade, administração, definições, ajuda.
    Administration,
}

impl ApplicationCategory {
    /// As cinco, na ordem do lançador.
    pub const ALL: [ApplicationCategory; 5] = [
        Self::Productivity,
        Self::Research,
        Self::Knowledge,
        Self::Communication,
        Self::Administration,
    ];

    /// O identificador técnico.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Productivity => "productivity",
            Self::Communication => "communication",
            Self::Knowledge => "knowledge",
            Self::Research => "research",
            Self::Administration => "administration",
        }
    }
}

/// Que armazenamento a aplicação usa — sempre pelas fronteiras governadas do
/// Core, nunca por caminhos próprios (§40, ADR-0108).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageUse {
    /// Nenhum.
    None,
    /// Bytes do membro, contra a sua quota.
    Personal,
    /// Bytes de um contentor institucional (ambiente, unidade).
    Institutional,
}

/// Que rede a aplicação precisa de alcançar fora da Instância.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkUse {
    /// Nenhuma: tudo acontece dentro da Instância.
    None,
    /// Servidores de correio (IMAP/SMTP) configurados pela Instância.
    ExternalMail,
}

/// De onde vem o estado de disponibilidade da aplicação.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthSource {
    /// A saúde do Core basta.
    Core,
    /// O estado do correio (`/mail/status`).
    Mail,
    /// O estado da IA (`/ai/status`).
    Intelligence,
    /// O estado da computação (`/compute/status`).
    Compute,
    /// A disponibilidade do armazenamento de objectos.
    Storage,
}

/// Como a aplicação chegou à Instância.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lifecycle {
    /// Nativa: compilada no Ocinye OS e confiável. A única que existe hoje;
    /// aplicações da organização, conectores e pacotes externos são classes
    /// futuras que o manifesto já não impede (ADR-0016).
    Native,
}

/// O manifesto de uma aplicação: o contrato do Ocinye OS com ela.
///
/// Diz quem é, como se apresenta, que rotas da API são suas, o que pede ao
/// Core — armazenamento, rede, capacidades de IA, recursos — e de onde vem a
/// sua saúde. **Pedir não é receber**: uma aplicação declara o que precisa, e o
/// Core governa o que lhe dá.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ApplicationManifest {
    /// Identidade estável.
    pub id: ApplicationId,
    /// Família no lançador.
    pub category: ApplicationCategory,
    /// A rota do ecrã no Workspace.
    pub route: &'static str,
    /// Chave i18n do nome.
    pub name_key: &'static str,
    /// Chave i18n da descrição.
    pub description_key: &'static str,
    /// Os prefixos da API (sem `/api/v1`) que pertencem **só** a esta aplicação.
    pub api_prefixes: &'static [&'static str],
    /// Que armazenamento usa.
    pub storage: StorageUse,
    /// Que rede externa precisa.
    pub network: NetworkUse,
    /// Que capacidades de IA pede — capacidades, nunca modelos.
    pub ai_capabilities: &'static [crate::AiCapability],
    /// Que recursos governados consome.
    pub requested_resources: &'static [crate::ResourceType],
    /// De onde vem a sua disponibilidade.
    pub health: HealthSource,
    /// Se o membro a pode fixar na barra.
    pub can_pin: bool,
    /// Se começa fixada para quem nunca escolheu.
    pub default_pin: bool,
}

impl ApplicationManifest {
    /// A classe (essencial ou opcional).
    #[must_use]
    pub const fn class(&self) -> ApplicationClass {
        self.id.class()
    }

    /// Como chegou à Instância.
    #[must_use]
    pub const fn lifecycle(&self) -> Lifecycle {
        Lifecycle::Native
    }

    /// A versão do contrato em que está escrito.
    #[must_use]
    pub const fn version(&self) -> u32 {
        MANIFEST_VERSION
    }
}

use crate::intelligence::AiCapability;
use crate::resource::ResourceType;

/// Os manifestos de todas as aplicações nativas, na ordem do registo.
pub const MANIFESTS: [ApplicationManifest; 23] = [
    ApplicationManifest {
        id: ApplicationId::Notes,
        category: ApplicationCategory::Productivity,
        route: "/notes",
        name_key: "nav.notes",
        description_key: "apps.desc.notes",
        api_prefixes: &[
            "/notes",
            "/me/notes",
            "/me/deleted-notes",
            "/me/shared-notes",
        ],
        storage: StorageUse::Personal,
        network: NetworkUse::None,
        ai_capabilities: &[],
        requested_resources: &[ResourceType::PersistentStorage],
        health: HealthSource::Core,
        can_pin: true,
        default_pin: true,
    },
    ApplicationManifest {
        id: ApplicationId::Calendar,
        category: ApplicationCategory::Productivity,
        route: "/calendar",
        name_key: "nav.calendar",
        description_key: "apps.desc.calendar",
        api_prefixes: &["/calendar"],
        storage: StorageUse::None,
        network: NetworkUse::None,
        ai_capabilities: &[],
        requested_resources: &[],
        health: HealthSource::Core,
        can_pin: true,
        default_pin: false,
    },
    ApplicationManifest {
        id: ApplicationId::Work,
        category: ApplicationCategory::Productivity,
        route: "/my-work",
        name_key: "nav.my_work",
        description_key: "apps.desc.work",
        api_prefixes: &[],
        storage: StorageUse::None,
        network: NetworkUse::None,
        ai_capabilities: &[],
        requested_resources: &[],
        health: HealthSource::Core,
        can_pin: false,
        default_pin: false,
    },
    ApplicationManifest {
        id: ApplicationId::Home,
        category: ApplicationCategory::Productivity,
        route: "/",
        name_key: "nav.home",
        description_key: "apps.desc.home",
        api_prefixes: &[],
        storage: StorageUse::None,
        network: NetworkUse::None,
        ai_capabilities: &[],
        requested_resources: &[],
        health: HealthSource::Core,
        can_pin: false,
        default_pin: false,
    },
    ApplicationManifest {
        id: ApplicationId::Mail,
        category: ApplicationCategory::Communication,
        route: "/mail",
        name_key: "nav.mail",
        description_key: "apps.desc.mail",
        api_prefixes: &["/mail"],
        storage: StorageUse::Personal,
        network: NetworkUse::ExternalMail,
        ai_capabilities: &[],
        requested_resources: &[ResourceType::PersistentStorage],
        health: HealthSource::Mail,
        can_pin: true,
        default_pin: false,
    },
    ApplicationManifest {
        id: ApplicationId::Messages,
        category: ApplicationCategory::Communication,
        route: "/messages",
        name_key: "nav.messages",
        description_key: "apps.desc.messages",
        api_prefixes: &["/messaging"],
        storage: StorageUse::None,
        network: NetworkUse::None,
        ai_capabilities: &[],
        requested_resources: &[],
        health: HealthSource::Core,
        can_pin: true,
        default_pin: false,
    },
    ApplicationManifest {
        id: ApplicationId::Files,
        category: ApplicationCategory::Knowledge,
        route: "/files",
        name_key: "nav.files",
        description_key: "apps.desc.files",
        api_prefixes: &[],
        storage: StorageUse::Personal,
        network: NetworkUse::None,
        ai_capabilities: &[],
        requested_resources: &[ResourceType::PersistentStorage],
        health: HealthSource::Storage,
        can_pin: true,
        default_pin: true,
    },
    ApplicationManifest {
        id: ApplicationId::Knowledge,
        category: ApplicationCategory::Knowledge,
        route: "/knowledge",
        name_key: "nav.knowledge",
        description_key: "apps.desc.knowledge",
        api_prefixes: &["/documents"],
        storage: StorageUse::Institutional,
        network: NetworkUse::None,
        ai_capabilities: &[],
        requested_resources: &[],
        health: HealthSource::Core,
        can_pin: true,
        default_pin: false,
    },
    ApplicationManifest {
        id: ApplicationId::Bibliography,
        category: ApplicationCategory::Knowledge,
        route: "/bibliography",
        name_key: "nav.bibliography",
        description_key: "apps.desc.bibliography",
        api_prefixes: &["/sources"],
        storage: StorageUse::Institutional,
        network: NetworkUse::None,
        ai_capabilities: &[],
        requested_resources: &[],
        health: HealthSource::Core,
        can_pin: true,
        default_pin: false,
    },
    ApplicationManifest {
        id: ApplicationId::Units,
        category: ApplicationCategory::Research,
        route: "/units",
        name_key: "nav.units",
        description_key: "apps.desc.units",
        api_prefixes: &["/units"],
        storage: StorageUse::None,
        network: NetworkUse::None,
        ai_capabilities: &[],
        requested_resources: &[],
        health: HealthSource::Core,
        can_pin: true,
        default_pin: false,
    },
    ApplicationManifest {
        id: ApplicationId::Ideas,
        category: ApplicationCategory::Research,
        route: "/ideas",
        name_key: "nav.ideas",
        description_key: "apps.desc.ideas",
        api_prefixes: &["/ideas"],
        storage: StorageUse::Institutional,
        network: NetworkUse::None,
        ai_capabilities: &[],
        requested_resources: &[],
        health: HealthSource::Core,
        can_pin: true,
        default_pin: false,
    },
    ApplicationManifest {
        id: ApplicationId::Projects,
        category: ApplicationCategory::Research,
        route: "/projects",
        name_key: "nav.projects",
        description_key: "apps.desc.projects",
        api_prefixes: &["/projects"],
        storage: StorageUse::Institutional,
        network: NetworkUse::None,
        ai_capabilities: &[],
        requested_resources: &[],
        health: HealthSource::Core,
        can_pin: true,
        default_pin: true,
    },
    ApplicationManifest {
        id: ApplicationId::Datasets,
        category: ApplicationCategory::Research,
        route: "/datasets",
        name_key: "nav.data",
        description_key: "apps.desc.datasets",
        api_prefixes: &["/datasets"],
        storage: StorageUse::Institutional,
        network: NetworkUse::None,
        ai_capabilities: &[],
        requested_resources: &[],
        health: HealthSource::Storage,
        can_pin: true,
        default_pin: false,
    },
    ApplicationManifest {
        id: ApplicationId::Prompt,
        category: ApplicationCategory::Research,
        route: "/ai/prompt",
        name_key: "nav.prompt",
        description_key: "apps.desc.prompt",
        api_prefixes: &["/ai/prompt", "/ai/conversations"],
        storage: StorageUse::None,
        network: NetworkUse::None,
        ai_capabilities: &[
            AiCapability::General,
            AiCapability::Coding,
            AiCapability::Reasoning,
        ],
        requested_resources: &[ResourceType::ModelAccess],
        health: HealthSource::Intelligence,
        can_pin: true,
        default_pin: false,
    },
    ApplicationManifest {
        id: ApplicationId::Ai,
        category: ApplicationCategory::Research,
        route: "/ai",
        name_key: "nav.ai",
        description_key: "apps.desc.ai",
        api_prefixes: &[],
        storage: StorageUse::None,
        network: NetworkUse::None,
        ai_capabilities: &[],
        requested_resources: &[],
        health: HealthSource::Intelligence,
        can_pin: true,
        default_pin: false,
    },
    ApplicationManifest {
        id: ApplicationId::Agents,
        category: ApplicationCategory::Research,
        route: "/ai/agents",
        name_key: "nav.agents",
        description_key: "apps.desc.agents",
        api_prefixes: &["/ai/agents"],
        storage: StorageUse::None,
        network: NetworkUse::None,
        ai_capabilities: &[AiCapability::General],
        requested_resources: &[ResourceType::ModelAccess],
        health: HealthSource::Intelligence,
        can_pin: true,
        default_pin: false,
    },
    ApplicationManifest {
        id: ApplicationId::Compute,
        category: ApplicationCategory::Research,
        route: "/compute",
        name_key: "nav.compute",
        description_key: "apps.desc.compute",
        api_prefixes: &["/compute/nodes", "/compute/status"],
        storage: StorageUse::None,
        network: NetworkUse::None,
        ai_capabilities: &[],
        requested_resources: &[],
        health: HealthSource::Compute,
        can_pin: true,
        default_pin: false,
    },
    ApplicationManifest {
        id: ApplicationId::Resources,
        category: ApplicationCategory::Administration,
        route: "/resources",
        name_key: "nav.resources",
        description_key: "apps.desc.resources",
        api_prefixes: &[],
        storage: StorageUse::None,
        network: NetworkUse::None,
        ai_capabilities: &[],
        requested_resources: &[],
        health: HealthSource::Core,
        can_pin: true,
        default_pin: false,
    },
    ApplicationManifest {
        id: ApplicationId::Activity,
        category: ApplicationCategory::Administration,
        route: "/activity",
        name_key: "nav.activity",
        description_key: "apps.desc.activity",
        api_prefixes: &[],
        storage: StorageUse::None,
        network: NetworkUse::None,
        ai_capabilities: &[],
        requested_resources: &[],
        health: HealthSource::Core,
        can_pin: true,
        default_pin: false,
    },
    ApplicationManifest {
        id: ApplicationId::Administration,
        category: ApplicationCategory::Administration,
        route: "/admin",
        name_key: "nav.admin",
        description_key: "apps.desc.administration",
        api_prefixes: &[],
        storage: StorageUse::None,
        network: NetworkUse::None,
        ai_capabilities: &[],
        requested_resources: &[],
        health: HealthSource::Core,
        can_pin: true,
        default_pin: false,
    },
    ApplicationManifest {
        id: ApplicationId::Audit,
        category: ApplicationCategory::Administration,
        route: "/audit",
        name_key: "nav.audit",
        description_key: "apps.desc.audit",
        api_prefixes: &[],
        storage: StorageUse::None,
        network: NetworkUse::None,
        ai_capabilities: &[],
        requested_resources: &[],
        health: HealthSource::Core,
        can_pin: true,
        default_pin: false,
    },
    ApplicationManifest {
        id: ApplicationId::Settings,
        category: ApplicationCategory::Administration,
        route: "/settings",
        name_key: "nav.settings",
        description_key: "apps.desc.settings",
        api_prefixes: &[],
        storage: StorageUse::None,
        network: NetworkUse::None,
        ai_capabilities: &[],
        requested_resources: &[],
        health: HealthSource::Core,
        can_pin: true,
        default_pin: false,
    },
    ApplicationManifest {
        id: ApplicationId::Help,
        category: ApplicationCategory::Administration,
        route: "/help",
        name_key: "nav.help",
        description_key: "apps.desc.help",
        api_prefixes: &[],
        storage: StorageUse::None,
        network: NetworkUse::None,
        ai_capabilities: &[],
        requested_resources: &[],
        health: HealthSource::Core,
        can_pin: true,
        default_pin: false,
    },
];

impl ApplicationId {
    /// O manifesto desta aplicação.
    #[must_use]
    pub fn manifest(self) -> &'static ApplicationManifest {
        MANIFESTS
            .iter()
            .find(|manifest| manifest.id == self)
            .unwrap_or(&MANIFESTS[0])
    }
}

/// A que aplicação pertence um caminho da API (sem o prefixo `/api/v1`),
/// segundo os manifestos: o prefixo declarado mais longo que coincida por
/// segmentos inteiros. Os caminhos que nenhuma aplicação declara — identidade,
/// autenticação, saúde, Instância, contentores partilhados, autoridade sobre
/// nós — não são de nenhuma (ADR-0015).
#[must_use]
pub fn application_of_api_path(path: &str) -> Option<ApplicationId> {
    MANIFESTS
        .iter()
        .flat_map(|manifest| manifest.api_prefixes.iter().map(move |p| (manifest.id, *p)))
        .filter(|(_, prefixo)| {
            path == *prefixo
                || path
                    .strip_prefix(prefixo)
                    .is_some_and(|resto| resto.starts_with('/') || resto.starts_with('?'))
        })
        .max_by_key(|(_, prefixo)| prefixo.len())
        .map(|(id, _)| id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_identificadores_sao_unicos_e_voltam_a_si_mesmos() {
        let mut vistos = std::collections::BTreeSet::new();
        for app in ApplicationId::ALL {
            assert!(vistos.insert(app.as_str()), "{app} repetido");
            assert_eq!(app.as_str().parse::<ApplicationId>(), Ok(app));
        }
        assert!("search".parse::<ApplicationId>().is_err());
    }

    #[test]
    fn uma_essencial_esta_activa_em_todos_os_perfis() {
        for profile in InstanceProfile::ALL {
            for app in ApplicationId::ALL.into_iter().filter(|a| !a.is_optional()) {
                assert!(
                    profile.activates(app),
                    "{app} essencial inactiva em {profile:?}"
                );
            }
        }
    }

    /// O perfil de investigação é o comportamento de antes dos perfis: tudo.
    #[test]
    fn investigacao_activa_tudo() {
        assert!(ApplicationId::ALL
            .into_iter()
            .all(|app| InstanceProfile::Research.activates(app)));
    }

    #[test]
    fn os_outros_perfis_nao_trazem_os_modulos_cientificos() {
        for profile in [
            InstanceProfile::Business,
            InstanceProfile::Education,
            InstanceProfile::Personal,
        ] {
            for app in [
                ApplicationId::Ideas,
                ApplicationId::Datasets,
                ApplicationId::Compute,
            ] {
                assert!(!profile.activates(app), "{app} activa em {profile:?}");
            }
        }
        assert!(InstanceProfile::Education.activates(ApplicationId::Bibliography));
        assert!(!InstanceProfile::Business.activates(ApplicationId::Bibliography));
        assert!(!InstanceProfile::Personal.activates(ApplicationId::Units));
    }

    #[test]
    fn so_a_investigacao_semeia_unidades() {
        assert!(InstanceProfile::Research.seeds_initial_units());
        assert!(!InstanceProfile::Business.seeds_initial_units());
    }
}
