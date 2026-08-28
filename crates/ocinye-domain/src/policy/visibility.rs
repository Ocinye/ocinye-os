//! Query-side mirror of the read policy.
//!
//! Listings, counts and search must filter *in the database*: a caller may
//! never see — or count — a row the policy would deny (`CLAUDE.md` §28). That
//! means the read rule exists twice: once as a decision over a loaded resource
//! ([`super::evaluate`]), once as a set of predicates over rows.
//!
//! Two implementations that must agree are a standing hazard. This module
//! removes the hazard by giving the filter *executable semantics*
//! ([`VisibilityFilter::permits`]) and asserting equivalence against the policy
//! over every combination of classification, role and membership. Change one
//! side alone and the equivalence test fails.
//!
//! The Core renders this structure into SQL; the rendering is mechanical and is
//! covered by integration tests against a real database.

use ocinye_contracts::{Classification, UnitRole};
use uuid::Uuid;

use crate::principal::Principal;

/// The set of rows a principal may read, described without reference to SQL.
///
/// Read as a disjunction: a row is visible if any clause admits it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VisibilityFilter {
    /// Deny everything. Set for an inactive principal.
    pub deny_all: bool,
    /// Organisation scope every clause is confined to.
    pub organisation_id: Option<Uuid>,
    /// Whether `PUBLIC` and `INTERNAL` rows are visible.
    pub allow_internal_and_below: bool,
    /// Whether `CONFIDENTIAL` is visible organisation-wide (administrative role).
    pub confidential_organisation_wide: bool,
    /// Units whose `CONFIDENTIAL` rows are visible through membership.
    pub confidential_unit_ids: Vec<Uuid>,
    /// Workspaces whose `CONFIDENTIAL` rows are visible through membership.
    pub confidential_workspace_ids: Vec<Uuid>,
    /// Workspaces whose `RESTRICTED` rows are visible through membership.
    pub restricted_workspace_ids: Vec<Uuid>,
    /// Units whose `RESTRICTED` rows are visible through management.
    pub restricted_unit_ids: Vec<Uuid>,
}

impl VisibilityFilter {
    /// Derive the filter for a principal.
    ///
    /// Note the asymmetry in the `RESTRICTED` clauses: workspace membership of
    /// any role qualifies, but only unit *management* does. Administrative
    /// roles appear nowhere in them, which is the whole point (ADR-0100).
    #[must_use]
    pub fn for_principal(principal: &Principal) -> Self {
        if !principal.is_active {
            return Self {
                deny_all: true,
                ..Self::default()
            };
        }

        Self {
            deny_all: false,
            organisation_id: Some(principal.organisation_id),
            allow_internal_and_below: true,
            confidential_organisation_wide: principal.is_organisation_admin(),
            confidential_unit_ids: principal.unit_ids(),
            confidential_workspace_ids: principal.workspace_ids(),
            restricted_workspace_ids: principal.workspace_ids(),
            restricted_unit_ids: principal.managed_unit_ids(),
        }
    }

    /// Whether this filter admits a row with the given scope and classification.
    ///
    /// This is the executable semantics the SQL rendering must reproduce.
    #[must_use]
    pub fn permits(
        &self,
        unit_id: Option<Uuid>,
        workspace_id: Option<Uuid>,
        classification: Classification,
    ) -> bool {
        if self.deny_all {
            return false;
        }

        let in_units = |ids: &[Uuid]| unit_id.is_some_and(|id| ids.contains(&id));
        let in_workspaces = |ids: &[Uuid]| workspace_id.is_some_and(|id| ids.contains(&id));

        match classification {
            Classification::Public | Classification::Internal => self.allow_internal_and_below,
            Classification::Confidential => {
                self.confidential_organisation_wide
                    || in_units(&self.confidential_unit_ids)
                    || in_workspaces(&self.confidential_workspace_ids)
            }
            Classification::Restricted => {
                in_workspaces(&self.restricted_workspace_ids) || in_units(&self.restricted_unit_ids)
            }
        }
    }

    /// Whether any row at all can match. Lets callers short-circuit a query.
    #[must_use]
    pub fn is_never_satisfiable(&self) -> bool {
        self.deny_all
    }
}

/// Whether a unit role grants `RESTRICTED` visibility over that unit.
///
/// Exposed so the Core's SQL rendering derives the same rule instead of
/// restating it.
#[must_use]
pub const fn unit_role_grants_restricted(role: UnitRole) -> bool {
    matches!(role, UnitRole::Manager)
}
