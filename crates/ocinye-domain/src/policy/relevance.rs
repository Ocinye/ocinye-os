//! Relevância de módulo: o que pertence ao espaço de trabalho de uma pessoa.
//!
//! # O contrato
//!
//! > **A relevância de um módulo responde se uma capacidade pertence ao espaço
//! > de trabalho institucional da pessoa. A autorização de um recurso responde
//! > ao que ela pode de facto ver ou fazer. A relevância nunca concede
//! > autoridade.**
//!
//! # Porque isto teve de nascer
//!
//! Quatro módulos — Conhecimento, Ficheiros, Bibliografia e Dados — apareciam
//! esbatidos na navegação **para toda a gente**. Os direitos que os governam
//! (`BibliographyView`, `DatasetsView`, `DocumentsView`) só existem como
//! concessão contextual: pertença a uma unidade ou a um ambiente. A navegação
//! perguntava-os no contexto da organização, onde `unit_id` e `workspace_id`
//! são `None` — e ali nenhuma concessão contextual se aplica. A resposta era
//! sempre não, para todos, incluindo para quem pertencia a um ambiente cheio de
//! ficheiros.
//!
//! As duas saídas óbvias eram ambas erradas. Conceder esses direitos em âmbito
//! institucional teria relaxado a política de segurança: passariam a ser
//! verdadeiros em contexto de organização, e todo o código que os verifique ali
//! passaria a permitir. Abrir os módulos a qualquer membro autenticado teria
//! quebrado duas propriedades já provadas — a navegação encolhe quando o Core
//! não confirma, e um colaborador externo não vê Dados.
//!
//! O que faltava era um terceiro conceito, e não uma resposta diferente à
//! mesma pergunta.
//!
//! # O que isto **não** é
//!
//! Não é uma segunda política de autorização. Não devolve `allowed`. Um módulo
//! relevante com zero recursos acessíveis é um estado normal e honesto — e é
//! precisamente o que uma conta de investigação sem pertenças deve ver.
//!
//! Dizer `allowed = true` sem um `workspace_id` seria a mesma mentira com outro
//! nome.

use ocinye_contracts::TechnicalRole;

use crate::principal::Principal;

/// Onde a autorização de um módulo se decide.
///
/// É esta a distinção que impede que um mapa de navegação seja usado como ACL
/// daqui a seis meses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthorizationScope {
    /// Decide-se em âmbito institucional. A capacidade global basta.
    Institutional,
    /// Decide-se dentro de um contentor de autoridade — unidade ou ambiente.
    ///
    /// A navegação **não** pode responder por ela: só o recurso concreto pode.
    Contextual,
}

/// Um módulo do Workspace, para efeitos de relevância.
///
/// Deliberadamente não é o `Screen` da Experience: a relevância é uma decisão
/// do domínio, e o domínio não conhece ecrãs. Um ecrã novo que mostre ficheiros
/// continua a perguntar por `Files`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Module {
    /// Unidades da instituição.
    Units,
    /// Ideias.
    Ideas,
    /// Projectos.
    Projects,
    /// O acervo de conhecimento.
    Knowledge,
    /// Ficheiros institucionais.
    Files,
    /// Bibliografia.
    Bibliography,
    /// Datasets.
    Datasets,
}

impl Module {
    /// Representação estável, para atravessar o contrato HTTP.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Units => "units",
            Self::Ideas => "ideas",
            Self::Projects => "projects",
            Self::Knowledge => "knowledge",
            Self::Files => "files",
            Self::Bibliography => "bibliography",
            Self::Datasets => "datasets",
        }
    }

    /// Onde a autorização deste módulo se decide.
    #[must_use]
    pub const fn scope(self) -> AuthorizationScope {
        match self {
            // Estes quatro governam-se dentro de um contentor: pertença,
            // classificação e política do ambiente. A navegação apresenta-os;
            // não os autoriza.
            Self::Knowledge | Self::Files | Self::Bibliography | Self::Datasets => {
                AuthorizationScope::Contextual
            }
            // Estes três já têm direito institucional, e continuam a tê-lo.
            Self::Units | Self::Ideas | Self::Projects => AuthorizationScope::Institutional,
        }
    }

    /// Todos os módulos, para quem precise de enumerar.
    #[must_use]
    pub const fn all() -> [Self; 7] {
        [
            Self::Units,
            Self::Ideas,
            Self::Projects,
            Self::Knowledge,
            Self::Files,
            Self::Bibliography,
            Self::Datasets,
        ]
    }
}

/// Se este módulo pertence ao espaço de trabalho desta pessoa.
///
/// # A regra, e o que ela deliberadamente não faz
///
/// Deriva do **papel institucional**, e de mais nada. Não olha para pertenças,
/// porque a ausência de pertença é exactamente a situação que este conceito
/// existe para tratar com honestidade: uma conta de investigação sem ambiente
/// atribuído não é uma conta partida, é uma conta que ainda não tem trabalho.
///
/// Um colaborador externo não ganha módulos de investigação por aqui. Um
/// administrador de plataforma também não: administrar a plataforma não é fazer
/// investigação, e é o mesmo princípio que impede um papel administrativo de
/// ler material `RESTRICTED`.
#[must_use]
pub fn is_relevant(principal: &Principal, module: Module) -> bool {
    let faz_investigacao = principal.roles.iter().any(|role| {
        matches!(
            role,
            TechnicalRole::ResearchLead
                | TechnicalRole::ResearchMember
                | TechnicalRole::UnitManager
                | TechnicalRole::OrganisationAdmin
        )
    });

    match module {
        // Quem faz investigação conhece o espaço onde ela acontece, tenha ou
        // não trabalho atribuído hoje.
        Module::Knowledge
        | Module::Files
        | Module::Bibliography
        | Module::Datasets
        | Module::Ideas
        | Module::Projects => faz_investigacao,

        // As unidades são a estrutura da instituição, e quem a administra
        // precisa de as ver mesmo sem fazer investigação.
        Module::Units => {
            faz_investigacao
                || principal
                    .roles
                    .iter()
                    .any(|role| matches!(role, TechnicalRole::PlatformAdmin))
        }
    }
}

/// Os módulos relevantes para esta pessoa, em ordem estável.
#[must_use]
pub fn relevant_modules(principal: &Principal) -> Vec<Module> {
    Module::all()
        .into_iter()
        .filter(|module| is_relevant(principal, *module))
        .collect()
}
