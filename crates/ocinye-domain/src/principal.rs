//! The authenticated caller and its institutional context.

use std::collections::{HashMap, HashSet};

use ocinye_contracts::{TechnicalRole, UnitRole, WorkspaceRole};

use crate::policy::ExplicitGrant;
use uuid::Uuid;

/// O tipo de identidade operacional por trás de uma sessão.
///
/// > **Uma identidade privilegiada ligada estabelece responsabilidade, e não
/// > herança de autoridade.**
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IdentityKind {
    /// Uma pessoa. A identidade normal com que se trabalha.
    #[default]
    Human,
    /// A identidade por onde alguém exerce autoridade administrativa.
    ///
    /// Nunca é uma pessoa: pertence sempre a uma, e é essa que responde pelo
    /// que ela faz.
    Privileged,
}

impl IdentityKind {
    /// Lê o que a base guarda. Um valor desconhecido lê-se como `Human`.
    ///
    /// Conservador de propósito: um tipo que ninguém reconhece não deve ganhar
    /// a apresentação privilegiada por acidente.
    #[must_use]
    pub fn parse(raw: &str) -> Self {
        match raw {
            "privileged" => Self::Privileged,
            _ => Self::Human,
        }
    }

    /// A forma que a base guarda.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Human => "human",
            Self::Privileged => "privileged",
        }
    }

    /// Se esta sessão tem de ser inequivocamente distinguível de uma normal.
    #[must_use]
    pub const fn is_privileged(self) -> bool {
        matches!(self, Self::Privileged)
    }
}

/// Who is acting, and in what institutional context.
///
/// A `Principal` is assembled server-side from the verified OIDC subject plus
/// the person's memberships in the database. Roles and memberships are never
/// read from token claims: they are institutional facts, not assertions a
/// client could influence (ADR-0102).
///
/// Institutional position is deliberately absent from this type. It grants no
/// capability, so the policy has no business seeing it (ADR-0100).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Principal {
    /// Verified OIDC subject.
    pub subject: String,
    /// Identity of the person inside the organisation.
    pub person_id: Uuid,
    /// Organisation the person belongs to.
    pub organisation_id: Uuid,
    /// Name for display and attribution.
    pub display_name: String,
    /// Whether membership is currently active.
    pub is_active: bool,
    /// Que **tipo** de identidade iniciou esta sessão.
    ///
    /// # Porque isto não é a mesma coisa que ter `PlatformAdmin`
    ///
    /// Porque respondem a perguntas diferentes:
    ///
    /// - `identity_kind` responde «que tipo de identidade iniciou esta sessão?»
    /// - os papéis respondem «que autoridade é que ela tem **agora**?»
    ///
    /// Uma identidade privilegiada a quem revoguem o `PlatformAdmin` continua a
    /// ser uma identidade privilegiada — a sessão não deixa de ser o que é — mas
    /// deixa de ter autoridade administrativa. Colapsar as duas faria a
    /// Experience afirmar «Super Admin» a quem já não pode administrar, ou tratar
    /// como sessão normal uma sessão que não o é.
    ///
    /// E na direcção inversa: uma pessoa a quem alguém desse `PlatformAdmin` não
    /// passa a ser uma identidade privilegiada. Continua a ser uma pessoa, com
    /// autoridade a mais.
    pub identity_kind: IdentityKind,
    /// Technical roles currently granted.
    pub roles: HashSet<TechnicalRole>,
    /// Role held in each unit.
    pub unit_roles: HashMap<Uuid, UnitRole>,
    /// Role held in each research workspace.
    pub workspace_roles: HashMap<Uuid, WorkspaceRole>,
    /// Live explicit grants. Already filtered for revocation and expiry by the
    /// repository, so the policy never has to ask what time it is.
    pub grants: Vec<ExplicitGrant>,
}

impl Principal {
    /// Whether any of the given technical roles is held.
    #[must_use]
    pub fn has_role(&self, roles: &[TechnicalRole]) -> bool {
        roles.iter().any(|role| self.roles.contains(role))
    }

    /// Whether the caller administers the organisation or the platform.
    ///
    /// Note what this does *not* imply: administrative roles never grant
    /// `RESTRICTED` reads (ADR-0100).
    #[must_use]
    pub fn is_organisation_admin(&self) -> bool {
        self.has_role(&[
            TechnicalRole::PlatformAdmin,
            TechnicalRole::OrganisationAdmin,
        ])
    }

    /// Role held in the given unit, if any.
    #[must_use]
    pub(crate) fn unit_role(&self, unit_id: Option<Uuid>) -> Option<UnitRole> {
        unit_id.and_then(|id| self.unit_roles.get(&id).copied())
    }

    /// Role held in the given research workspace, if any.
    #[must_use]
    pub(crate) fn workspace_role(&self, workspace_id: Option<Uuid>) -> Option<WorkspaceRole> {
        workspace_id.and_then(|id| self.workspace_roles.get(&id).copied())
    }

    /// Units the caller belongs to, in any role.
    #[must_use]
    pub fn unit_ids(&self) -> Vec<Uuid> {
        self.unit_roles.keys().copied().collect()
    }

    /// Units the caller manages.
    #[must_use]
    pub(crate) fn managed_unit_ids(&self) -> Vec<Uuid> {
        self.unit_roles
            .iter()
            .filter(|(_, role)| **role == UnitRole::Manager)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Research workspaces the caller belongs to.
    #[must_use]
    pub fn workspace_ids(&self) -> Vec<Uuid> {
        self.workspace_roles.keys().copied().collect()
    }
}
