//! What each technical role grants, and how a permission is evaluated.
//!
//! # System roles live in code
//!
//! The role → permission mapping is a `match`, not a table. That is deliberate
//! (`CLAUDE.md` §71, briefing §75): a permission set that can be edited at
//! runtime is a permission set no test can pin down, and this is the layer
//! where an exhaustive test is worth most. Custom roles remain `PLANNED`.
//!
//! # Two independent gates
//!
//! A permission answers *may this actor perform this kind of operation here*.
//! Classification answers *may this actor see this particular material*. Both
//! must allow. Keeping them separate is what lets `PlatformAdmin` administer
//! the platform without thereby reading `RESTRICTED` science (briefing §49).

use std::collections::BTreeSet;

use ocinye_contracts::{Classification, Permission, Scope, TechnicalRole, UnitRole, WorkspaceRole};
use uuid::Uuid;

use super::{classification_allows_read, Decision, ResourceContext};
use crate::principal::Principal;

/// A permission granted to one subject on one scope, outside the role model.
///
/// Grants are how `RESTRICTED` material is reached by someone who is not a
/// member of the owning workspace — deliberately explicit, attributable and
/// expiring (briefing §63).
///
/// # Liveness is the repository's job
///
/// A grant that reaches this type is **live**: not revoked, not expired. The
/// domain stays pure by never asking what time it is; the repository filters on
/// `revoked_at IS NULL AND (expires_at IS NULL OR expires_at > now())`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExplicitGrant {
    /// What is granted.
    pub permission: Permission,
    /// Where it is valid.
    pub scope: Scope,
    /// Which unit, workspace or resource. `None` means institution-wide.
    pub scope_id: Option<Uuid>,
}

impl ExplicitGrant {
    /// Whether this grant applies in the given resource context.
    ///
    /// An institution-wide grant applies everywhere in the organisation; a
    /// scoped grant applies only to its own unit, workspace or resource. A
    /// scoped grant with no identifier applies to nothing — a grant that cannot
    /// name its target is not a grant.
    #[must_use]
    pub(crate) fn applies_to(&self, ctx: &ResourceContext, resource_id: Option<Uuid>) -> bool {
        match self.scope {
            Scope::Institution => self.scope_id.is_none(),
            Scope::Unit => self.scope_id.is_some() && self.scope_id == ctx.unit_id,
            Scope::ResearchWorkspace => {
                self.scope_id.is_some() && self.scope_id == ctx.workspace_id
            }
            Scope::Resource => self.scope_id.is_some() && self.scope_id == resource_id,
        }
    }
}

/// Permissions granted by an institution-wide technical role.
///
/// Read this table as the answer to "what does this role mean". Note what is
/// absent as much as what is present.
#[must_use]
pub const fn role_permissions(role: TechnicalRole) -> &'static [Permission] {
    use Permission as P;
    match role {
        // Operates the platform. Note the absence of every research-content
        // permission: technical administration is not scientific access
        // (briefing §49).
        TechnicalRole::PlatformAdmin => &[
            P::PlatformAdminister,
            P::OrganisationView,
            P::OrganisationManage,
            P::MembersView,
            P::MembersCreate,
            P::MembersManage,
            P::UnitsView,
            P::UnitsCreate,
            P::UnitsManage,
            P::RolesView,
            P::RolesManage,
            P::PermissionsView,
            P::AuditView,
            P::AiInfrastructureManage,
            P::AgentsView,
            P::AgentsManage,
            // Coerência com `AgentsManage`: administrar todos os agentes e não
            // poder criar um institucional é uma lacuna, não uma restrição.
            // Um agente é uma **definição** — nome, propósito, instruções,
            // capacidade — e o tecto de classificação do próprio agente
            // continua a governar o que ele alcança. Liderar um workspace não
            // chega para isto: um agente institucional é usado por toda a
            // instituição, e o `permission_tests` guarda essa distinção.
            P::AgentsCreateInstitutional,
            P::ComputeView,
            P::ComputeManageNodes,
            P::ComputeManageJobs,
            P::ComputeAdmin,
            // Administra o **serviço** de correio: configuração, diagnóstico,
            // caixas partilhadas. Não é uma chave para correspondência alheia,
            // e não pode ser: a pertença à caixa é decidida em SQL, contra o
            // próprio actor (ADR-0404).
            P::MailAdminister,
            // A sua própria caixa. `MailUse` nunca alcança mais do que isso —
            // por isso concedê-la a quem administra a plataforma não abre nada.
            P::MessagingUse,
            P::MailUse,
            P::CalendarView,
            P::CalendarCreate,
            P::CalendarEdit,
            P::MailSend,
        ],

        // Administers the institution's people and structure. Does not thereby
        // obtain the institution's research material (briefing §50).
        TechnicalRole::OrganisationAdmin => &[
            P::OrganisationView,
            P::OrganisationManage,
            P::MembersView,
            P::MembersCreate,
            P::MembersManage,
            P::UnitsView,
            P::UnitsCreate,
            P::UnitsManage,
            P::RolesView,
            P::RolesManage,
            P::PermissionsView,
            P::AuditView,
            P::IdeasView,
            P::ProjectsView,
            P::AgentsView,
            P::ComputeView,
            P::MessagingUse,
            P::MailUse,
            P::CalendarView,
            P::CalendarCreate,
            P::CalendarEdit,
            P::MailSend,
            P::MailSharedView,
        ],

        // A unit manager's power is contextual: this set only ever applies
        // inside a unit they actually manage. See `contextual_permissions`.
        TechnicalRole::UnitManager => &[
            P::OrganisationView,
            P::UnitsView,
            P::MembersView,
            P::RolesView,
        ],

        TechnicalRole::ResearchLead | TechnicalRole::ResearchMember => &[
            P::OrganisationView,
            P::UnitsView,
            P::MembersView,
            P::IdeasView,
            P::ProjectsView,
            P::AiUse,
            P::AgentsView,
            P::AgentsCreatePersonal,
            P::ComputeView,
            // Correio institucional: o membro tem uma caixa e escreve a partir
            // dela. `MailUse` e `MailSend` só alcançam caixas que sejam suas ou
            // partilhadas de que faça parte.
            P::MessagingUse,
            P::MailUse,
            // A agenda. Todo o membro tem a sua: um evento pessoal alcança-se
            // por ser de quem é, e a instituição não o vê. `CalendarView` é o
            // que dá acesso aos eventos de unidade, workspace e instituição
            // (ADR-0410).
            P::CalendarView,
            P::CalendarCreate,
            P::CalendarEdit,
            P::MailSend,
            P::MessagingAiUse,
            P::MailAiUse,
            P::MailSharedView,
            P::MailSharedSend,
        ],

        // A collaborator inside the institution: sees that the institution
        // exists and works where placed, nothing more.
        // Correio sim, assistência de IA não: escrever em nome da instituição
        // com apoio de um modelo é uma capacidade de quem a integra.
        TechnicalRole::Collaborator => &[
            P::OrganisationView,
            P::AgentsView,
            P::MessagingUse,
            P::MailUse,
            P::MailSend,
            P::CalendarView,
            P::CalendarCreate,
            P::CalendarEdit,
        ],

        // Deny-by-default at its strongest (briefing §54). Not even the member
        // list, not even the unit list. Everything comes from membership in a
        // specific workspace or from an explicit grant.
        TechnicalRole::ExternalCollaborator => &[],

        // Evidence and history, never content. `AuditView` is still scoped:
        // holding it does not mean platform-wide audit (briefing §86).
        TechnicalRole::Auditor => &[P::OrganisationView, P::AuditView, P::PermissionsView],
    }
}

/// Permissions granted by a role held inside one unit.
#[must_use]
pub const fn unit_role_permissions(role: UnitRole) -> &'static [Permission] {
    use Permission as P;
    match role {
        UnitRole::Manager => &[
            P::UnitsView,
            P::UnitsManage,
            P::MembersView,
            P::IdeasView,
            P::IdeasCreate,
            P::IdeasEdit,
            P::IdeasTransition,
            P::ProjectsView,
            P::ProjectsCreate,
            P::ProjectsEdit,
            P::ProjectsManage,
            P::ResultsValidate,
            P::ResearchMembersManage,
            P::BibliographyView,
            P::BibliographyCreate,
            P::BibliographyEdit,
            P::NotesView,
            P::ScienceView,
            P::NotesCreate,
            P::ScienceCreate,
            P::LinksCreate,
            P::NotesEdit,
            P::DocumentsView,
            P::DocumentsUpload,
            P::DocumentsDownload,
            P::DocumentsManage,
            P::DatasetsView,
            P::DatasetsCreate,
            P::DatasetsEdit,
            P::DatasetsVersion,
            P::DatasetsDownload,
            P::DatasetsManage,
            P::AiUse,
            P::AgentsView,
            P::AgentsCreatePersonal,
            P::AgentsCreateProject,
            P::AgentsCreateUnit,
            P::AgentsShare,
            P::CalendarView,
            P::CalendarCreate,
            P::CalendarEdit,
            P::AuditView,
            P::ComputeView,
            P::ComputeSubmitJob,
        ],
        UnitRole::Member => &[
            P::UnitsView,
            P::CalendarView,
            P::IdeasView,
            P::IdeasCreate,
            P::ProjectsView,
            P::BibliographyView,
            P::NotesView,
            P::ScienceView,
            P::DocumentsView,
            P::DatasetsView,
            P::AiUse,
            P::AgentsView,
            P::ComputeView,
        ],
    }
}

/// Permissions granted by a role held inside one research workspace.
#[must_use]
pub const fn workspace_role_permissions(role: WorkspaceRole) -> &'static [Permission] {
    use Permission as P;
    match role {
        WorkspaceRole::Lead => &[
            P::IdeasView,
            P::IdeasEdit,
            P::IdeasTransition,
            P::ProjectsView,
            P::ProjectsEdit,
            P::ProjectsManage,
            P::ResultsValidate,
            P::ResearchMembersManage,
            P::BibliographyView,
            P::BibliographyCreate,
            P::BibliographyEdit,
            P::NotesView,
            P::ScienceView,
            P::NotesCreate,
            P::ScienceCreate,
            P::LinksCreate,
            P::NotesEdit,
            P::DocumentsView,
            P::DocumentsUpload,
            P::DocumentsDownload,
            P::DocumentsManage,
            P::DatasetsView,
            P::DatasetsCreate,
            P::DatasetsEdit,
            P::DatasetsVersion,
            P::DatasetsDownload,
            P::DatasetsManage,
            P::AiUse,
            P::AgentsView,
            P::AgentsCreatePersonal,
            P::AgentsCreateProject,
            P::AgentsShare,
            P::AuditView,
            P::ComputeView,
            P::ComputeSubmitJob,
            P::TasksView,
            P::TasksCreate,
            P::TasksEdit,
            P::CalendarView,
            P::CalendarCreate,
            P::CalendarEdit,
        ],
        WorkspaceRole::Member => &[
            P::IdeasView,
            P::IdeasEdit,
            P::ProjectsView,
            P::ProjectsEdit,
            P::BibliographyView,
            P::BibliographyCreate,
            P::BibliographyEdit,
            P::NotesView,
            P::ScienceView,
            P::NotesCreate,
            P::ScienceCreate,
            P::LinksCreate,
            P::NotesEdit,
            P::TasksView,
            P::TasksCreate,
            P::TasksEdit,
            P::CalendarView,
            P::CalendarCreate,
            P::CalendarEdit,
            P::DocumentsView,
            P::DocumentsUpload,
            P::DocumentsDownload,
            P::DatasetsView,
            P::DatasetsCreate,
            P::DatasetsEdit,
            P::DatasetsVersion,
            P::DatasetsDownload,
            P::AiUse,
            P::AgentsView,
            P::AgentsCreatePersonal,
            P::ComputeView,
            P::ComputeSubmitJob,
        ],
        // A viewer reads. Note the absence of `DocumentsDownload` and
        // `DatasetsDownload`: seeing that something exists and taking a copy of
        // it are different rights.
        WorkspaceRole::Viewer => &[
            P::IdeasView,
            P::ProjectsView,
            P::BibliographyView,
            P::NotesView,
            P::ScienceView,
            P::TasksView,
            P::CalendarView,
            P::DocumentsView,
            P::DatasetsView,
        ],
    }
}

/// Whether exercising this permission puts institutional material in front of
/// the actor, and therefore has to clear the classification gate as well.
///
/// Administrative permissions are absent on purpose: managing a member list is
/// not reading science, and subjecting it to a classification gate would
/// conflate the two dimensions this module exists to keep apart.
#[must_use]
const fn touches_content(permission: Permission) -> bool {
    use Permission as P;
    matches!(
        permission,
        P::IdeasView
            | P::IdeasEdit
            | P::IdeasTransition
            | P::ProjectsView
            | P::ProjectsEdit
            | P::ProjectsManage
            | P::BibliographyView
            | P::BibliographyCreate
            | P::BibliographyEdit
            | P::NotesView
            | P::NotesCreate
            | P::NotesEdit
            | P::ScienceView
            | P::ScienceCreate
            | P::ResultsValidate
            | P::DocumentsView
            | P::DocumentsUpload
            | P::DocumentsDownload
            | P::DocumentsManage
            | P::DatasetsView
            | P::DatasetsCreate
            | P::DatasetsEdit
            | P::DatasetsVersion
            | P::DatasetsDownload
            | P::DatasetsManage
    )
}

/// Every permission the principal holds in the given context.
///
/// The union of: institution-wide technical roles, the role held in this
/// context's unit, the role held in this context's workspace, and any live
/// explicit grant that applies here.
///
/// A union is correct *here* because this answers "what operations are open to
/// you". It is emphatically not how classification is decided — that gate is
/// applied separately, and never widened by holding more roles.
#[must_use]
pub(crate) fn effective_permissions(
    principal: &Principal,
    ctx: &ResourceContext,
    resource_id: Option<Uuid>,
) -> BTreeSet<Permission> {
    let mut permissions = BTreeSet::new();

    if !principal.is_active {
        return permissions;
    }

    for role in &principal.roles {
        permissions.extend(role_permissions(*role).iter().copied());
    }

    if let Some(role) = principal.unit_role(ctx.unit_id) {
        permissions.extend(unit_role_permissions(role).iter().copied());
    }

    if let Some(role) = principal.workspace_role(ctx.workspace_id) {
        permissions.extend(workspace_role_permissions(role).iter().copied());
    }

    for grant in &principal.grants {
        if grant.applies_to(ctx, resource_id) {
            permissions.insert(grant.permission);
        }
    }

    permissions
}

/// Evaluate a named permission in a resource context.
///
/// Fails closed at every step. A permission that is not positively present in
/// the effective set is denied, and a content permission is denied again if the
/// classification gate does not also allow.
#[must_use]
pub fn can(
    principal: &Principal,
    permission: Permission,
    ctx: &ResourceContext,
    resource_id: Option<Uuid>,
) -> Decision {
    if !principal.is_active {
        return Decision::deny("principal is not an active member");
    }

    if let Some(organisation_id) = ctx.organisation_id {
        if organisation_id != principal.organisation_id {
            return Decision::deny("cross-organisation access is not permitted");
        }
    }

    if !effective_permissions(principal, ctx, resource_id).contains(&permission) {
        return Decision::deny("no role, membership or grant confers this permission here");
    }

    // A grant naming this exact permission is itself the explicit authorisation
    // that RESTRICTED demands, so it clears the classification gate for that
    // permission and no other (briefing §63).
    let granted_explicitly = principal
        .grants
        .iter()
        .any(|grant| grant.permission == permission && grant.applies_to(ctx, resource_id));

    if touches_content(permission) && !granted_explicitly {
        let read = classification_allows_read(principal, ctx);
        if !read.allowed {
            return read;
        }
        if ctx.classification == Classification::Restricted {
            return Decision::allow("explicit membership clears the RESTRICTED gate");
        }
    }

    Decision::allow("permission is conferred in this context")
}

/// Explain why a subject can exercise a permission here.
///
/// Answers the administrative question "why does this person have this access?"
/// (briefing §64) with the concrete source, not a yes/no.
#[must_use]
pub fn explain(
    principal: &Principal,
    permission: Permission,
    ctx: &ResourceContext,
    resource_id: Option<Uuid>,
) -> Option<AccessSource> {
    if !principal.is_active {
        return None;
    }

    if let Some(grant) = principal
        .grants
        .iter()
        .find(|grant| grant.permission == permission && grant.applies_to(ctx, resource_id))
    {
        return Some(AccessSource::ExplicitGrant {
            scope: grant.scope,
            scope_id: grant.scope_id,
        });
    }

    if let Some(role) = principal.workspace_role(ctx.workspace_id) {
        if workspace_role_permissions(role).contains(&permission) {
            return Some(AccessSource::WorkspaceMembership {
                workspace_id: ctx.workspace_id?,
                role,
            });
        }
    }

    if let Some(role) = principal.unit_role(ctx.unit_id) {
        if unit_role_permissions(role).contains(&permission) {
            return Some(AccessSource::UnitMembership {
                unit_id: ctx.unit_id?,
                role,
            });
        }
    }

    principal
        .roles
        .iter()
        .find(|role| role_permissions(**role).contains(&permission))
        .map(|role| AccessSource::TechnicalRole { role: *role })
}

/// Where a permission came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessSource {
    /// An institution-wide technical role.
    TechnicalRole {
        /// The role.
        role: TechnicalRole,
    },
    /// Membership of the unit that owns the resource.
    UnitMembership {
        /// The unit.
        unit_id: Uuid,
        /// The role held there.
        role: UnitRole,
    },
    /// Membership of the research workspace that owns the resource.
    WorkspaceMembership {
        /// The workspace.
        workspace_id: Uuid,
        /// The role held there.
        role: WorkspaceRole,
    },
    /// An explicit, attributable grant.
    ExplicitGrant {
        /// Where the grant applies.
        scope: Scope,
        /// Which unit, workspace or resource.
        scope_id: Option<Uuid>,
    },
}

impl AccessSource {
    /// Short stable label, for the administration interface and the audit trail.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::TechnicalRole { .. } => "technical_role",
            Self::UnitMembership { .. } => "unit_membership",
            Self::WorkspaceMembership { .. } => "workspace_membership",
            Self::ExplicitGrant { .. } => "explicit_grant",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ocinye_contracts::{Permission, TechnicalRole, UnitRole, WorkspaceRole};

    /// Permissões que existem no catálogo e que **nenhum papel de sistema
    /// concede**, deliberadamente.
    ///
    /// Cada uma tem de ter uma razão escrita aqui. Uma permissão que caia nesta
    /// lista por esquecimento é uma funcionalidade inteira que ninguém
    /// alcança — foi exactamente o que aconteceu com as sete permissões de
    /// correio, e o que o teste abaixo passa a impedir.
    const DELIBERATELY_UNGRANTED: &[(Permission, &str)] = &[
        (
            Permission::PermissionsManage,
            "Papéis personalizados são PLANNED (ADR-0101). Conceder isto sem \
             eles daria a alguém o poder de reescrever o modelo de autorização \
             em tempo de execução.",
        ),
        (
            Permission::MailSharedManage,
            "A administração de caixas partilhadas é PLANNED: o modelo e as \
             consultas existem, o ecrã não. Concedê-la agora ofereceria uma \
             capacidade sem forma de a exercer.",
        ),
    ];

    /// Todas as permissões alcançáveis por algum papel do sistema.
    fn reachable() -> Vec<Permission> {
        let mut found: Vec<Permission> = Vec::new();

        for role in TechnicalRole::all() {
            found.extend_from_slice(role_permissions(role));
        }
        for role in UnitRole::all() {
            found.extend_from_slice(unit_role_permissions(role));
        }
        for role in WorkspaceRole::all() {
            found.extend_from_slice(workspace_role_permissions(role));
        }

        found.sort_unstable_by_key(|p| p.as_str());
        found.dedup();
        found
    }

    /// Nenhuma permissão fica órfã por esquecimento.
    ///
    /// # Porque este teste existe
    ///
    /// As sete permissões de correio foram definidas no catálogo, verificadas
    /// em cada rota e cada consulta, documentadas em ADR — e **não foram
    /// concedidas a papel nenhum**. Tudo compilava, o clippy estava limpo, e
    /// os testes unitários passavam, porque cada um deles constrói o seu
    /// próprio principal. O correio inteiro estava inalcançável.
    ///
    /// Um teste de integração apanhou-o. Este apanha a classe.
    #[test]
    fn nenhuma_permissao_fica_sem_papel_que_a_conceda() {
        let reachable = reachable();
        let excused: Vec<Permission> = DELIBERATELY_UNGRANTED
            .iter()
            .map(|(permission, _)| *permission)
            .collect();

        let orphaned: Vec<&str> = Permission::all()
            .into_iter()
            .filter(|permission| !reachable.contains(permission) && !excused.contains(permission))
            .map(Permission::as_str)
            .collect();

        assert!(
            orphaned.is_empty(),
            "permissões que nenhum papel concede:\n  {}\n\
             Ou um papel a concede, ou entra em DELIBERATELY_UNGRANTED com a \
             razão escrita.",
            orphaned.join("\n  ")
        );
    }

    /// A lista de excepções não guarda permissões que já são concedidas.
    ///
    /// Sem isto, a lista acumularia entradas obsoletas e deixaria de ser lida.
    #[test]
    fn a_lista_de_excepcoes_nao_tem_entradas_obsoletas() {
        let reachable = reachable();

        for (permission, reason) in DELIBERATELY_UNGRANTED {
            assert!(
                !reachable.contains(permission),
                "`{}` está em DELIBERATELY_UNGRANTED mas já é concedida por um \
                 papel. Retire-a da lista.",
                permission.as_str()
            );
            assert!(
                reason.len() > 40,
                "`{}` não tem razão escrita que valha a pena ler",
                permission.as_str()
            );
        }
    }

    /// Correio é uma capacidade de quem integra a instituição.
    #[test]
    fn um_membro_de_investigacao_alcanca_o_correio() {
        let member = role_permissions(TechnicalRole::ResearchMember);

        for permission in [
            Permission::MailUse,
            Permission::MailSend,
            Permission::MailAiUse,
        ] {
            assert!(
                member.contains(&permission),
                "um membro de investigação não alcança `{}`",
                permission.as_str()
            );
        }
    }

    /// Administrar a plataforma não é administrar o correio de ninguém.
    ///
    /// O `PlatformAdmin` tem `MailUse` — para a **sua própria** caixa, que é
    /// tudo o que essa permissão alcança — e `MailAdminister`, que cobre o
    /// serviço. Não tem, e não deve ter, forma de ler a caixa de um colega:
    /// isso não é decidido aqui, mas na cláusula `WHERE` de cada consulta
    /// (ADR-0404).
    #[test]
    fn administrar_a_plataforma_nao_e_ler_correio_alheio() {
        let admin = role_permissions(TechnicalRole::PlatformAdmin);

        assert!(admin.contains(&Permission::MailAdminister));
        assert!(
            !admin.contains(&Permission::MailSharedView),
            "administrar a plataforma passou a dar acesso a caixas partilhadas \
             de que não se faz parte"
        );
    }

    /// Um colaborador externo continua a não alcançar nada.
    #[test]
    fn um_colaborador_externo_nao_ganhou_correio() {
        assert!(
            role_permissions(TechnicalRole::ExternalCollaborator).is_empty(),
            "o conjunto vazio do colaborador externo deixou de estar vazio"
        );
    }
}
