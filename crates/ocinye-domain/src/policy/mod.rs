//! Authorization policy: RBAC plus contextual rules, fail closed.
//!
//! Two invariants govern this module (ADR-0100).
//!
//! 1. **Fail closed.** Every path ends in an explicit allow. Anything the
//!    policy cannot positively justify is denied. There is no `_ => allow`.
//! 2. **Institutional position grants nothing.** Only technical roles and
//!    contextual memberships grant capability. An organisation admin is *not*
//!    automatically able to read `RESTRICTED` material — that single rule is
//!    what keeps "Founder" from meaning "reads everything".
//!
//! The policy is pure, which is what makes [`mod@tests`] able to enumerate
//! every combination of classification, role and membership rather than
//! sampling a few.

pub mod agentic;
pub mod permissions;
pub mod visibility;

use ocinye_contracts::{Classification, TechnicalRole, UnitRole, WorkspaceRole};
use uuid::Uuid;

pub use agentic::{
    ai_processing_ceiling, approval_needed, effective_risk, is_delegable_to_agents, may_invoke,
    may_process_with_ai, AgentBoundary, AgenticRefusal,
};
pub use permissions::{can, explain, AccessSource, ExplicitGrant};
pub use visibility::VisibilityFilter;

use crate::principal::Principal;

/// An operation subject to authorization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    /// Read a resource.
    Read,
    /// Create a resource in the given context.
    Create,
    /// Modify a resource.
    Update,
    /// Archive a resource. Archival, not deletion (briefing §72).
    Archive,
    /// Move a resource through its workflow.
    Transition,
    /// Change a resource's classification.
    Classify,
    /// Add, change or revoke memberships.
    ManageMembers,
    /// Obtain the bytes of a stored object.
    Download,
    /// Take content out of the institution.
    Export,
    /// Operate the platform itself.
    Administer,
    /// Read the audit trail.
    ReadAudit,
}

impl Action {
    /// Whether a denial of this action must be indistinguishable from absence.
    ///
    /// Read-shaped denials are reported as "not found" so that an unauthorised
    /// caller cannot infer that a resource exists.
    #[must_use]
    pub const fn hides_existence_on_denial(self) -> bool {
        matches!(self, Self::Read | Self::Download | Self::ReadAudit)
    }
}

/// The kind of resource being acted upon. Carried for auditing and clarity;
/// the policy branches on context and classification, not on kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceKind {
    /// The organisation itself.
    Organisation,
    /// A person.
    Person,
    /// A scientific unit.
    Unit,
    /// A research workspace.
    ResearchWorkspace,
    /// An exploratory idea.
    Idea,
    /// A formal project.
    Project,
    /// A bibliographic source.
    Source,
    /// A conceptual note.
    Note,
    /// A document backed by a stored object.
    Document,
    /// A dataset.
    Dataset,
    /// A task.
    Task,
    /// A calendar event.
    CalendarEvent,
    /// A reminder somebody set for themselves.
    Reminder,
    /// A comment.
    Comment,
    /// An audit record.
    AuditEvent,
    /// A compute node.
    ComputeNode,
    /// An AI capability request.
    AiCapability,
    /// Uma hipótese científica.
    Hypothesis,
    /// Uma metodologia.
    Methodology,
    /// Uma versão de metodologia — um recurso, e não um campo.
    MethodologyVersion,
    /// Uma versão de dataset.
    DatasetVersion,
    /// Um estudo: experimento, simulação ou análise.
    Study,
    /// Uma execução de um estudo.
    StudyExecution,
    /// Um resultado científico.
    Result,
    /// The platform itself.
    Platform,
}

/// The authorization-relevant facts about a resource.
///
/// Assembled server-side from persisted state, never from a request body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceContext {
    /// Kind of resource.
    pub kind: ResourceKind,
    /// Classification of the resource.
    pub classification: Classification,
    /// Owning unit, when the resource is unit-scoped.
    pub unit_id: Option<Uuid>,
    /// Owning research workspace, when the resource is workspace-scoped.
    pub workspace_id: Option<Uuid>,
    /// Owning organisation, when known.
    pub organisation_id: Option<Uuid>,
}

impl ResourceContext {
    /// Context for an organisation-scoped resource.
    #[must_use]
    pub const fn organisation(kind: ResourceKind, organisation_id: Uuid) -> Self {
        Self {
            kind,
            classification: Classification::Internal,
            unit_id: None,
            workspace_id: None,
            organisation_id: Some(organisation_id),
        }
    }

    /// Context for a unit-scoped resource.
    #[must_use]
    pub const fn unit(kind: ResourceKind, organisation_id: Uuid, unit_id: Uuid) -> Self {
        Self {
            kind,
            classification: Classification::Internal,
            unit_id: Some(unit_id),
            workspace_id: None,
            organisation_id: Some(organisation_id),
        }
    }

    /// Context for a workspace-scoped resource.
    #[must_use]
    pub const fn workspace(
        kind: ResourceKind,
        organisation_id: Uuid,
        unit_id: Uuid,
        workspace_id: Uuid,
        classification: Classification,
    ) -> Self {
        Self {
            kind,
            classification,
            unit_id: Some(unit_id),
            workspace_id: Some(workspace_id),
            organisation_id: Some(organisation_id),
        }
    }

    /// Return a copy with a different classification.
    #[must_use]
    pub const fn with_classification(mut self, classification: Classification) -> Self {
        self.classification = classification;
        self
    }
}

/// The outcome of an authorization evaluation, with its justification.
///
/// The reason is recorded in the audit trail on denial, which is what makes an
/// access decision reviewable months later.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    /// Whether the action is permitted.
    pub allowed: bool,
    /// Why. Stable enough to be logged and read by a human.
    pub reason: &'static str,
}

impl Decision {
    pub(crate) const fn allow(reason: &'static str) -> Self {
        Self {
            allowed: true,
            reason,
        }
    }

    pub(crate) const fn deny(reason: &'static str) -> Self {
        Self {
            allowed: false,
            reason,
        }
    }
}

/// Read gate driven by classification and contextual membership.
///
/// `RESTRICTED` deliberately ignores administrative roles.
pub(crate) fn classification_allows_read(principal: &Principal, ctx: &ResourceContext) -> Decision {
    let workspace_role = principal.workspace_role(ctx.workspace_id);
    let unit_role = principal.unit_role(ctx.unit_id);

    match ctx.classification {
        Classification::Public | Classification::Internal => {
            Decision::allow("active organisation member may read INTERNAL and below")
        }
        Classification::Confidential => {
            if workspace_role.is_some() {
                Decision::allow("workspace member may read CONFIDENTIAL")
            } else if unit_role.is_some() {
                Decision::allow("unit member may read CONFIDENTIAL of own unit")
            } else if principal.is_organisation_admin() {
                Decision::allow("organisation admin may read CONFIDENTIAL")
            } else {
                Decision::deny("CONFIDENTIAL requires unit or workspace membership")
            }
        }
        Classification::Restricted => {
            if workspace_role.is_some() {
                Decision::allow("explicit workspace membership grants RESTRICTED read")
            } else if unit_role == Some(UnitRole::Manager) {
                Decision::allow("unit manager may read RESTRICTED of own unit")
            } else {
                Decision::deny(
                    "RESTRICTED requires explicit workspace membership or unit management; \
                     administrative roles alone are insufficient",
                )
            }
        }
    }
}

/// Write gate, evaluated only after the read gate has already allowed.
fn may_write_in_context(principal: &Principal, ctx: &ResourceContext) -> Decision {
    let workspace_role = principal.workspace_role(ctx.workspace_id);
    let unit_role = principal.unit_role(ctx.unit_id);

    if ctx.workspace_id.is_some() {
        return match workspace_role {
            Some(role) if role.can_write() => Decision::allow("workspace lead or member may write"),
            Some(WorkspaceRole::Viewer) => Decision::deny("workspace viewers are read-only"),
            _ if unit_role == Some(UnitRole::Manager) => {
                Decision::allow("unit manager may write in workspaces of own unit")
            }
            _ => Decision::deny("write requires workspace membership or unit management"),
        };
    }

    if ctx.unit_id.is_some() {
        return if unit_role == Some(UnitRole::Manager) {
            Decision::allow("unit manager may write unit-scoped resources")
        } else if principal.is_organisation_admin() {
            Decision::allow("organisation admin may write unit-scoped resources")
        } else {
            Decision::deny("write requires unit management or organisation administration")
        };
    }

    if principal.is_organisation_admin() {
        Decision::allow("organisation admin may write organisation-scoped resources")
    } else {
        Decision::deny("write on organisation scope requires an administrative role")
    }
}

/// Evaluate an authorization decision.
///
/// Never panics and never allows by default.
#[must_use]
pub fn evaluate(principal: &Principal, action: Action, ctx: &ResourceContext) -> Decision {
    if !principal.is_active {
        return Decision::deny("principal is not an active member");
    }

    if let Some(organisation_id) = ctx.organisation_id {
        if organisation_id != principal.organisation_id {
            return Decision::deny("cross-organisation access is not permitted");
        }
    }

    match action {
        Action::Administer => {
            if principal.has_role(&[TechnicalRole::PlatformAdmin]) {
                Decision::allow("platform admin may administer the platform")
            } else {
                Decision::deny("administration requires platform_admin")
            }
        }

        Action::ReadAudit => {
            if principal.has_role(&[
                TechnicalRole::Auditor,
                TechnicalRole::PlatformAdmin,
                TechnicalRole::OrganisationAdmin,
            ]) {
                Decision::allow("auditor or admin may read the audit trail")
            } else {
                Decision::deny("reading the audit trail requires auditor or admin role")
            }
        }

        Action::Read => classification_allows_read(principal, ctx),

        Action::Download => {
            let read = classification_allows_read(principal, ctx);
            if !read.allowed {
                return read;
            }
            Decision::allow("download follows read authorization")
        }

        Action::Export => {
            let read = classification_allows_read(principal, ctx);
            if !read.allowed {
                return read;
            }
            if ctx.classification == Classification::Restricted {
                // Taking RESTRICTED material out of the institution is a
                // deliberately narrower right than reading it.
                return if principal.workspace_role(ctx.workspace_id) == Some(WorkspaceRole::Lead) {
                    Decision::allow("workspace lead may export RESTRICTED material")
                } else if principal.unit_role(ctx.unit_id) == Some(UnitRole::Manager) {
                    Decision::allow("unit manager may export RESTRICTED material")
                } else {
                    Decision::deny(
                        "exporting RESTRICTED material requires workspace lead or unit manager",
                    )
                };
            }
            Decision::allow("export follows read authorization")
        }

        Action::Create | Action::Update | Action::Archive | Action::Transition => {
            let read = classification_allows_read(principal, ctx);
            if !read.allowed {
                return read;
            }
            may_write_in_context(principal, ctx)
        }

        Action::Classify | Action::ManageMembers => {
            let read = classification_allows_read(principal, ctx);
            if !read.allowed {
                return read;
            }
            if principal.workspace_role(ctx.workspace_id) == Some(WorkspaceRole::Lead) {
                Decision::allow("workspace lead may classify and manage members")
            } else if principal.unit_role(ctx.unit_id) == Some(UnitRole::Manager) {
                Decision::allow("unit manager may classify and manage members")
            } else if principal.is_organisation_admin() {
                Decision::allow("organisation admin may classify and manage members")
            } else {
                Decision::deny(
                    "classification and membership changes require lead, manager or admin",
                )
            }
        }
    }
}

/// How a denial should be reported to the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Denial {
    /// Report as absent, so existence is not disclosed.
    NotFound,
    /// Report as forbidden. Safe: readability is already established.
    Forbidden,
}

/// Evaluate and classify the denial, if any.
///
/// A denied write on a *readable* resource is reported as forbidden; anything
/// else is reported as absent.
pub fn authorize(
    principal: &Principal,
    action: Action,
    ctx: &ResourceContext,
) -> Result<Decision, (Denial, Decision)> {
    let decision = evaluate(principal, action, ctx);
    if decision.allowed {
        return Ok(decision);
    }
    if action.hides_existence_on_denial() {
        return Err((Denial::NotFound, decision));
    }
    if evaluate(principal, Action::Read, ctx).allowed {
        Err((Denial::Forbidden, decision))
    } else {
        Err((Denial::NotFound, decision))
    }
}

#[cfg(test)]
mod permission_tests;
#[cfg(test)]
mod tests;
