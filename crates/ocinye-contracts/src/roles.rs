//! Institutional position and technical role — two distinct dimensions.

use serde::{Deserialize, Serialize};

/// What a person *is* in the institution.
///
/// Carries no authorization power whatsoever. It exists for organisational
/// truth and attribution. "Founder" is a fact about a person, not a key to
/// RESTRICTED material (briefing §38).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstitutionalPosition {
    /// Founder of the institution.
    Founder,
    /// Member of the direction.
    Director,
    /// Lead of a scientific unit.
    UnitLead,
    /// Principal investigator of a line of work.
    PrincipalInvestigator,
    /// Researcher.
    Researcher,
    /// Engineer.
    Engineer,
    /// Research fellow.
    Fellow,
    /// Student.
    Student,
    /// Collaborator external to the institution.
    ExternalCollaborator,
}

impl InstitutionalPosition {
    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Founder => "founder",
            Self::Director => "director",
            Self::UnitLead => "unit_lead",
            Self::PrincipalInvestigator => "principal_investigator",
            Self::Researcher => "researcher",
            Self::Engineer => "engineer",
            Self::Fellow => "fellow",
            Self::Student => "student",
            Self::ExternalCollaborator => "external_collaborator",
        }
    }

    /// Parse from the stable representation.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "founder" => Self::Founder,
            "director" => Self::Director,
            "unit_lead" => Self::UnitLead,
            "principal_investigator" => Self::PrincipalInvestigator,
            "researcher" => Self::Researcher,
            "engineer" => Self::Engineer,
            "fellow" => Self::Fellow,
            "student" => Self::Student,
            "external_collaborator" => Self::ExternalCollaborator,
            _ => return None,
        })
    }
}

/// What a person *may do* on the platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TechnicalRole {
    /// Operates the platform itself.
    PlatformAdmin,
    /// Administers the organisation: units, people, roles.
    OrganisationAdmin,
    /// Manages one or more units (granted contextually per unit).
    UnitManager,
    /// Leads a research workspace: its work, its members, its lifecycle.
    ResearchLead,
    /// Ordinary research member.
    ResearchMember,
    /// Restricted collaborator.
    Collaborator,
    /// Collaborator from outside the institution. Deny-by-default is strongest
    /// here: sees only what was explicitly granted, and nothing institution-wide.
    ExternalCollaborator,
    /// Reads the audit trail. Grants no access to institutional content.
    Auditor,
}

impl TechnicalRole {
    /// Stable representation.
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

    /// Parse from the stable representation.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "platform_admin" => Self::PlatformAdmin,
            "organisation_admin" => Self::OrganisationAdmin,
            "unit_manager" => Self::UnitManager,
            "research_lead" => Self::ResearchLead,
            "research_member" => Self::ResearchMember,
            "collaborator" => Self::Collaborator,
            "external_collaborator" => Self::ExternalCollaborator,
            "auditor" => Self::Auditor,
            _ => return None,
        })
    }

    /// Every technical role. Used by exhaustive authorization tests.
    #[must_use]
    pub const fn all() -> [Self; 8] {
        [
            Self::PlatformAdmin,
            Self::OrganisationAdmin,
            Self::UnitManager,
            Self::ResearchLead,
            Self::ResearchMember,
            Self::Collaborator,
            Self::ExternalCollaborator,
            Self::Auditor,
        ]
    }
}

/// Role held inside a scientific unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnitRole {
    /// Manages the unit and everything scoped to it.
    Manager,
    /// Belongs to the unit.
    Member,
}

impl UnitRole {
    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Manager => "manager",
            Self::Member => "member",
        }
    }

    /// Parse from the stable representation.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "manager" => Self::Manager,
            "member" => Self::Member,
            _ => return None,
        })
    }

    /// Every unit role.
    #[must_use]
    pub const fn all() -> [Self; 2] {
        [Self::Manager, Self::Member]
    }
}

/// Role held inside a research workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceRole {
    /// Leads the workspace: may classify and manage members.
    Lead,
    /// Contributes to the workspace.
    Member,
    /// Reads the workspace without contributing.
    Viewer,
}

impl WorkspaceRole {
    /// Whether this role may modify workspace content.
    #[must_use]
    pub const fn can_write(self) -> bool {
        matches!(self, Self::Lead | Self::Member)
    }

    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Lead => "lead",
            Self::Member => "member",
            Self::Viewer => "viewer",
        }
    }

    /// Parse from the stable representation.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "lead" => Self::Lead,
            "member" => Self::Member,
            "viewer" => Self::Viewer,
            _ => return None,
        })
    }

    /// Every workspace role.
    #[must_use]
    pub const fn all() -> [Self; 3] {
        [Self::Lead, Self::Member, Self::Viewer]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewers_are_read_only() {
        assert!(!WorkspaceRole::Viewer.can_write());
        assert!(WorkspaceRole::Lead.can_write());
        assert!(WorkspaceRole::Member.can_write());
    }

    #[test]
    fn technical_roles_round_trip() {
        for role in TechnicalRole::all() {
            assert_eq!(TechnicalRole::parse(role.as_str()), Some(role));
        }
    }
}
