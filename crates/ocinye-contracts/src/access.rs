//! Account state, credential state, session state, permissions and scopes.
//!
//! These are the types the authorization layer speaks in. They live in
//! `contracts` because the Workspace must be able to *name* a permission in
//! order to render a permission-aware interface — but note what is not here:
//! the mapping from role to permission set, and the evaluation itself, live in
//! `ocinye-domain` and never ship to a browser (`CLAUDE.md` §4).

use serde::{Deserialize, Serialize};

/// Lifecycle state of an account.
///
/// Separate from [`CredentialState`] on purpose: an account can be perfectly
/// active while its credential is a temporary one that must be replaced. The
/// two answer different questions — *may this person be here at all* versus
/// *may this person do anything yet*.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountStatus {
    /// Created by an administrator; has never completed a first sign-in.
    Invited,
    /// Normal, usable account.
    Active,
    /// Temporarily barred. Sessions are revoked; authorship is preserved.
    Suspended,
    /// Permanently barred, kept as historical identity. Never deleted.
    Disabled,
}

impl AccountStatus {
    /// Whether an account in this state may hold a session at all.
    ///
    /// Note this is about *existing*, not about *acting*: a credential that
    /// must be changed still yields a session, just a restricted one.
    #[must_use]
    pub const fn may_authenticate(self) -> bool {
        matches!(self, Self::Invited | Self::Active)
    }

    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Invited => "invited",
            Self::Active => "active",
            Self::Suspended => "suspended",
            Self::Disabled => "disabled",
        }
    }

    /// Parse from the stable representation.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "invited" => Self::Invited,
            "active" => Self::Active,
            "suspended" => Self::Suspended,
            "disabled" => Self::Disabled,
            _ => return None,
        })
    }

    /// Every account status.
    #[must_use]
    pub const fn all() -> [Self; 4] {
        [Self::Invited, Self::Active, Self::Suspended, Self::Disabled]
    }
}

/// What kind of credential a person currently holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialKind {
    /// A one-time bootstrap credential issued by an administrator.
    ///
    /// It exists only to let its holder set a permanent password. It expires,
    /// it is single-purpose, and it never becomes permanent.
    Temporary,
    /// The person's own password.
    Permanent,
}

impl CredentialKind {
    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Temporary => "temporary",
            Self::Permanent => "permanent",
        }
    }

    /// Parse from the stable representation.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "temporary" => Self::Temporary,
            "permanent" => Self::Permanent,
            _ => return None,
        })
    }
}

/// Lifecycle state of one credential.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialState {
    /// Usable.
    Active,
    /// Already used to set a permanent password. Never usable again.
    Consumed,
    /// Passed its expiry without being used.
    Expired,
    /// Superseded or explicitly revoked by an administrator.
    Revoked,
}

impl CredentialState {
    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Consumed => "consumed",
            Self::Expired => "expired",
            Self::Revoked => "revoked",
        }
    }

    /// Parse from the stable representation.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "active" => Self::Active,
            "consumed" => Self::Consumed,
            "expired" => Self::Expired,
            "revoked" => Self::Revoked,
            _ => return None,
        })
    }
}

/// What a session is currently allowed to be used for.
///
/// This is the type that makes the first-login rule enforceable server-side.
/// A session carrying [`SessionState::PasswordChangeRequired`] is *not* a
/// normal session that happens to be pointed at a different page — it is a
/// different kind of session, and the Core refuses ordinary work on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    /// The holder authenticated with a temporary credential and may do nothing
    /// except set a permanent password, read the minimum needed to do so, and
    /// sign out.
    PasswordChangeRequired,
    /// An ordinary session.
    Active,
}

impl SessionState {
    /// Whether ordinary API work is permitted on this session.
    #[must_use]
    pub const fn permits_ordinary_work(self) -> bool {
        matches!(self, Self::Active)
    }

    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PasswordChangeRequired => "password_change_required",
            Self::Active => "active",
        }
    }

    /// Parse from the stable representation.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "password_change_required" => Self::PasswordChangeRequired,
            "active" => Self::Active,
            _ => return None,
        })
    }
}

/// Where a permission is valid.
///
/// A permission without a scope is a permission over the whole institution,
/// which is almost never what is meant. Requiring the scope at the type level
/// is what stops "may manage members" from quietly meaning "may manage every
/// member of every unit".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    /// The institution as a whole.
    Institution,
    /// One scientific unit.
    Unit,
    /// One research workspace.
    ResearchWorkspace,
    /// One individual resource. Used sparingly — see [`Scope::Resource`] notes
    /// in `docs/authorization/`.
    Resource,
}

impl Scope {
    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Institution => "institution",
            Self::Unit => "unit",
            Self::ResearchWorkspace => "research_workspace",
            Self::Resource => "resource",
        }
    }

    /// Parse from the stable representation.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "institution" => Self::Institution,
            "unit" => Self::Unit,
            "research_workspace" => Self::ResearchWorkspace,
            "resource" => Self::Resource,
            _ => return None,
        })
    }

    /// Every scope.
    #[must_use]
    pub const fn all() -> [Self; 4] {
        [
            Self::Institution,
            Self::Unit,
            Self::ResearchWorkspace,
            Self::Resource,
        ]
    }
}

/// A named capability.
///
/// Every authorization question in the Ocinye OS is asked in terms of one of
/// these. The point is that `if role == admin` never appears anywhere: adding
/// a role changes one table in `ocinye-domain`, not fifty call sites.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Permission {
    // ── Organisation ────────────────────────────────────────────────────
    /// See the organisation and its structure.
    OrganisationView,
    /// Change organisation-level settings.
    OrganisationManage,
    /// See the list of members.
    MembersView,
    /// Create member accounts.
    MembersCreate,
    /// Change member accounts: suspend, disable, reset, re-role.
    MembersManage,
    /// See units.
    UnitsView,
    /// Create units.
    UnitsCreate,
    /// Manage units and their membership.
    UnitsManage,

    // ── Research ────────────────────────────────────────────────────────
    /// See ideas.
    IdeasView,
    /// Create ideas.
    IdeasCreate,
    /// Edit ideas.
    IdeasEdit,
    /// Move an idea through its lifecycle.
    IdeasTransition,
    /// See projects.
    ProjectsView,
    /// Create projects.
    ProjectsCreate,
    /// Edit projects.
    ProjectsEdit,
    /// Manage a project, including its lifecycle.
    ProjectsManage,
    /// Add, change or revoke research workspace membership.
    ResearchMembersManage,

    // ── Knowledge ───────────────────────────────────────────────────────
    /// See bibliography.
    BibliographyView,
    /// Add references.
    BibliographyCreate,
    /// Edit references.
    BibliographyEdit,
    /// See notes.
    NotesView,
    /// Create notes.
    NotesCreate,
    /// Ler o trabalho científico de um ambiente: hipóteses, metodologias,
    /// estudos, execuções, resultados e a linhagem que os liga.
    ScienceView,
    /// Descrever trabalho científico: enunciar hipóteses, criar metodologias e
    /// versões, desenhar estudos, registar execuções e resultados.
    ScienceCreate,
    /// Afirmar que um resultado se confirma, se contradiz, ou que uma execução
    /// o reproduziu.
    ///
    /// Separada de [`Permission::ScienceCreate`] porque é outra coisa:
    /// descrever trabalho é registar o que se fez; validar é afirmar o que a
    /// instituição sabe. Quem pode escrever um resultado não fica por isso
    /// habilitado a declarar que ele está certo.
    ResultsValidate,
    /// Edit notes.
    NotesEdit,
    /// Relate two research objects to each other.
    ///
    /// Its own permission rather than a borrowed one: a typed relation is a
    /// first-class research object (`CLAUDE.md` §13), and creating one asserts
    /// something institutional about both endpoints.
    LinksCreate,
    /// See tasks in a research workspace.
    TasksView,
    /// Create tasks.
    TasksCreate,
    /// Change a task: reassign, reschedule, transition.
    TasksEdit,
    /// See calendar events in reach.
    ///
    /// A agenda pessoal não depende desta permissão para ser vista pelo próprio:
    /// um evento pessoal alcança-se por ser de quem é. Isto é o que dá acesso
    /// aos eventos de unidade, workspace e instituição.
    CalendarView,
    /// Create calendar events and reminders.
    CalendarCreate,
    /// Change or cancel a calendar event.
    CalendarEdit,
    /// See document metadata.
    DocumentsView,
    /// Upload documents.
    DocumentsUpload,
    /// Obtain document bytes.
    DocumentsDownload,
    /// Manage documents, including classification.
    DocumentsManage,

    // ── Data ────────────────────────────────────────────────────────────
    /// See datasets.
    DatasetsView,
    /// Catalogue a dataset.
    DatasetsCreate,
    /// Edit dataset metadata.
    DatasetsEdit,
    /// Cut a new dataset version.
    DatasetsVersion,
    /// Obtain dataset bytes.
    DatasetsDownload,
    /// Manage datasets, including classification and residency.
    DatasetsManage,

    // ── Intelligence ────────────────────────────────────────────────────
    /// Use Ocinye AI at all.
    AiUse,
    /// See agents.
    AgentsView,
    /// Create an agent scoped to oneself.
    AgentsCreatePersonal,
    /// Create an agent scoped to a research workspace.
    AgentsCreateProject,
    /// Create an agent scoped to a unit.
    AgentsCreateUnit,
    /// Create an agent scoped to the institution.
    AgentsCreateInstitutional,
    /// Manage agents created by others.
    AgentsManage,
    /// Share an agent beyond its creator.
    AgentsShare,
    /// Administer AI infrastructure: gateway, capability mapping, models.
    AiInfrastructureManage,

    // ── Compute ─────────────────────────────────────────────────────────
    /// See compute nodes and jobs.
    ComputeView,
    /// Submit a job.
    ComputeSubmitJob,
    /// Manage jobs, including those of others.
    ComputeManageJobs,
    /// Enrol, retire and configure nodes.
    ComputeManageNodes,
    /// Administer the compute plane.
    ComputeAdmin,

    // ── Mensagens ───────────────────────────────────────────────────────
    /// Abrir as Mensagens, ler as suas conversas e escrever nelas.
    ///
    /// # Porque não há uma segunda permissão para enviar
    ///
    /// Porque no correio enviar é sair da instituição, e por isso é uma decisão
    /// à parte. Uma mensagem interna não sai de lado nenhum: quem pode ler uma
    /// conversa pode falar nela, e separá-lo daria uma pessoa que assiste sem
    /// poder responder.
    ///
    /// Quem alcança **cada** conversa não se decide aqui. Decide-se pela
    /// participação, que é um facto da base — e um `PlatformAdmin` não lê a
    /// conversa de ninguém por ter esta permissão.
    MessagingUse,
    /// Usar a assistência do Ocinye ao escrever uma mensagem.
    ///
    /// Separada, pela mesma razão que [`Permission::MailAiUse`]: pode confiar-se
    /// a alguém o assistente de investigação e não um que lhe lê as conversas.
    MessagingAiUse,

    // ── Mail ────────────────────────────────────────────────────────────
    /// Open Ocinye Mail and read one's own mailbox.
    MailUse,
    /// Send mail from one's own identity.
    MailSend,
    /// Use AI assistance while composing.
    ///
    /// Separate from [`Permission::AiUse`]: a member may be trusted with the
    /// research assistant and not with one that reads their correspondence.
    MailAiUse,
    /// Read a shared mailbox one is a member of.
    MailSharedView,
    /// Send from a shared mailbox one is a member of.
    MailSharedSend,
    /// Change shared mailbox membership.
    MailSharedManage,
    /// Configure the mail provider and see integration health.
    ///
    /// **Never** grants access to anyone's messages (briefing §26).
    MailAdminister,

    // ── Administration ──────────────────────────────────────────────────
    /// See roles and what they grant.
    RolesView,
    /// Assign and revoke roles.
    RolesManage,
    /// See the permission catalogue.
    PermissionsView,
    /// Change permission assignment. Reserved; system roles are code today.
    PermissionsManage,
    /// Read the audit trail, within an authorised scope.
    AuditView,
    /// Operate the platform itself.
    PlatformAdminister,
}

impl Permission {
    /// Stable representation, used in the API, the audit trail and grants.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OrganisationView => "organisation.view",
            Self::OrganisationManage => "organisation.manage",
            Self::MembersView => "members.view",
            Self::MembersCreate => "members.create",
            Self::MembersManage => "members.manage",
            Self::UnitsView => "units.view",
            Self::UnitsCreate => "units.create",
            Self::UnitsManage => "units.manage",

            Self::IdeasView => "ideas.view",
            Self::IdeasCreate => "ideas.create",
            Self::IdeasEdit => "ideas.edit",
            Self::IdeasTransition => "ideas.transition",
            Self::ProjectsView => "projects.view",
            Self::ProjectsCreate => "projects.create",
            Self::ProjectsEdit => "projects.edit",
            Self::ProjectsManage => "projects.manage",
            Self::ResearchMembersManage => "research.members.manage",

            Self::BibliographyView => "bibliography.view",
            Self::BibliographyCreate => "bibliography.create",
            Self::BibliographyEdit => "bibliography.edit",
            Self::NotesView => "notes.view",
            Self::NotesCreate => "notes.create",
            Self::ScienceView => "science.view",
            Self::ScienceCreate => "science.create",
            Self::ResultsValidate => "results.validate",
            Self::NotesEdit => "notes.edit",
            Self::LinksCreate => "links.create",
            Self::TasksView => "tasks.view",
            Self::TasksCreate => "tasks.create",
            Self::TasksEdit => "tasks.edit",
            Self::CalendarView => "calendar.view",
            Self::CalendarCreate => "calendar.create",
            Self::CalendarEdit => "calendar.edit",
            Self::DocumentsView => "documents.view",
            Self::DocumentsUpload => "documents.upload",
            Self::DocumentsDownload => "documents.download",
            Self::DocumentsManage => "documents.manage",

            Self::DatasetsView => "datasets.view",
            Self::DatasetsCreate => "datasets.create",
            Self::DatasetsEdit => "datasets.edit",
            Self::DatasetsVersion => "datasets.version",
            Self::DatasetsDownload => "datasets.download",
            Self::DatasetsManage => "datasets.manage",

            Self::AiUse => "ai.use",
            Self::AgentsView => "agents.view",
            Self::AgentsCreatePersonal => "agents.create.personal",
            Self::AgentsCreateProject => "agents.create.project",
            Self::AgentsCreateUnit => "agents.create.unit",
            Self::AgentsCreateInstitutional => "agents.create.institutional",
            Self::AgentsManage => "agents.manage",
            Self::AgentsShare => "agents.share",
            Self::AiInfrastructureManage => "ai.infrastructure.manage",

            Self::ComputeView => "compute.view",
            Self::ComputeSubmitJob => "compute.submit_job",
            Self::ComputeManageJobs => "compute.manage_jobs",
            Self::ComputeManageNodes => "compute.manage_nodes",
            Self::ComputeAdmin => "compute.admin",

            Self::MessagingUse => "messaging.use",
            Self::MessagingAiUse => "messaging.ai_use",
            Self::MailUse => "mail.use",
            Self::MailSend => "mail.send",
            Self::MailAiUse => "mail.ai.use",
            Self::MailSharedView => "mail.shared.view",
            Self::MailSharedSend => "mail.shared.send",
            Self::MailSharedManage => "mail.shared.manage",
            Self::MailAdminister => "mail.administer",

            Self::RolesView => "roles.view",
            Self::RolesManage => "roles.manage",
            Self::PermissionsView => "permissions.view",
            Self::PermissionsManage => "permissions.manage",
            Self::AuditView => "audit.view",
            Self::PlatformAdminister => "platform.administer",
        }
    }

    /// Parse from the stable representation.
    ///
    /// Derived from [`Permission::all`] so a new variant cannot be added
    /// without becoming parseable — the two can never drift apart.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::all().into_iter().find(|p| p.as_str() == value)
    }

    /// Every permission in the catalogue.
    #[must_use]
    pub const fn all() -> [Self; 72] {
        [
            Self::OrganisationView,
            Self::OrganisationManage,
            Self::MembersView,
            Self::MembersCreate,
            Self::MembersManage,
            Self::UnitsView,
            Self::UnitsCreate,
            Self::UnitsManage,
            Self::IdeasView,
            Self::IdeasCreate,
            Self::IdeasEdit,
            Self::IdeasTransition,
            Self::ProjectsView,
            Self::ProjectsCreate,
            Self::ProjectsEdit,
            Self::ProjectsManage,
            Self::ResearchMembersManage,
            Self::BibliographyView,
            Self::BibliographyCreate,
            Self::BibliographyEdit,
            Self::NotesView,
            Self::NotesCreate,
            Self::ScienceView,
            Self::ScienceCreate,
            Self::ResultsValidate,
            Self::NotesEdit,
            Self::LinksCreate,
            Self::TasksView,
            Self::TasksCreate,
            Self::TasksEdit,
            Self::CalendarView,
            Self::CalendarCreate,
            Self::CalendarEdit,
            Self::DocumentsView,
            Self::DocumentsUpload,
            Self::DocumentsDownload,
            Self::DocumentsManage,
            Self::DatasetsView,
            Self::DatasetsCreate,
            Self::DatasetsEdit,
            Self::DatasetsVersion,
            Self::DatasetsDownload,
            Self::DatasetsManage,
            Self::AiUse,
            Self::AgentsView,
            Self::AgentsCreatePersonal,
            Self::AgentsCreateProject,
            Self::AgentsCreateUnit,
            Self::AgentsCreateInstitutional,
            Self::AgentsManage,
            Self::AgentsShare,
            Self::AiInfrastructureManage,
            Self::ComputeView,
            Self::ComputeSubmitJob,
            Self::ComputeManageJobs,
            Self::ComputeManageNodes,
            Self::ComputeAdmin,
            Self::MessagingUse,
            Self::MessagingAiUse,
            Self::MailUse,
            Self::MailSend,
            Self::MailAiUse,
            Self::MailSharedView,
            Self::MailSharedSend,
            Self::MailSharedManage,
            Self::MailAdminister,
            Self::RolesView,
            Self::RolesManage,
            Self::PermissionsView,
            Self::PermissionsManage,
            Self::AuditView,
            Self::PlatformAdminister,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_permission_round_trips() {
        for permission in Permission::all() {
            assert_eq!(
                Permission::parse(permission.as_str()),
                Some(permission),
                "{permission:?} does not round-trip"
            );
        }
    }

    #[test]
    fn permission_names_are_unique() {
        let mut names: Vec<&str> = Permission::all().iter().map(|p| p.as_str()).collect();
        let total = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), total, "two permissions share a wire name");
    }

    #[test]
    fn only_invited_and_active_accounts_may_authenticate() {
        assert!(AccountStatus::Invited.may_authenticate());
        assert!(AccountStatus::Active.may_authenticate());
        assert!(!AccountStatus::Suspended.may_authenticate());
        assert!(!AccountStatus::Disabled.may_authenticate());
    }

    #[test]
    fn a_password_change_session_permits_no_ordinary_work() {
        assert!(!SessionState::PasswordChangeRequired.permits_ordinary_work());
        assert!(SessionState::Active.permits_ordinary_work());
    }

    #[test]
    fn account_and_scope_representations_round_trip() {
        for status in AccountStatus::all() {
            assert_eq!(AccountStatus::parse(status.as_str()), Some(status));
        }
        for scope in Scope::all() {
            assert_eq!(Scope::parse(scope.as_str()), Some(scope));
        }
    }
}
