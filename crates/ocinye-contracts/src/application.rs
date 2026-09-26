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
