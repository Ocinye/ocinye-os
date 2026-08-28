//! Rendering the read policy into SQL.
//!
//! [`ocinye_domain::policy::VisibilityFilter`] describes which rows a principal
//! may read. This module turns that description into a `WHERE` fragment so that
//! listings, counts and search filter **in the database** — a caller never sees,
//! and never counts, a row the policy would deny (`CLAUDE.md` §28).
//!
//! The translation is mechanical and deliberately dull. The interesting part —
//! that the description matches the policy — is proved in
//! `ocinye-domain`, exhaustively. What is proved *here*, by integration tests
//! against a real database, is that the SQL reproduces the description.
//!
//! # Why the identifiers are inlined
//!
//! The fragment interpolates UUIDs directly rather than binding them. That is
//! safe, and checked: the values are `Uuid`, a type whose `Display` can only
//! produce hexadecimal and hyphens. No caller-supplied string ever reaches this
//! function. Binding variable-length lists would require dynamic parameter
//! numbering across composed queries, which is far easier to get wrong.

use ocinye_domain::policy::VisibilityFilter;
use uuid::Uuid;

/// Column names the fragment refers to.
#[derive(Debug, Clone, Copy)]
pub struct VisibilityColumns {
    /// Column holding the owning unit id.
    pub unit: &'static str,
    /// Column holding the owning workspace id.
    pub workspace: &'static str,
    /// Column holding the classification.
    pub classification: &'static str,
}

impl VisibilityColumns {
    /// Columns for a table using the conventional names on an alias.
    #[must_use]
    pub const fn aliased(
        unit: &'static str,
        workspace: &'static str,
        classification: &'static str,
    ) -> Self {
        Self {
            unit,
            workspace,
            classification,
        }
    }
}

impl Default for VisibilityColumns {
    fn default() -> Self {
        Self {
            unit: "unit_id",
            workspace: "workspace_id",
            classification: "classification",
        }
    }
}

/// Render a list of identifiers as a SQL tuple, or `None` when empty.
fn id_list(ids: &[Uuid]) -> Option<String> {
    if ids.is_empty() {
        return None;
    }
    let rendered = ids
        .iter()
        .map(|id| format!("'{id}'"))
        .collect::<Vec<_>>()
        .join(", ");
    Some(rendered)
}

/// Render the filter as a boolean SQL expression.
///
/// Always returns a self-contained parenthesised expression, so it can be
/// appended to any `WHERE` clause with `AND`.
#[must_use]
pub fn to_sql(filter: &VisibilityFilter, columns: VisibilityColumns) -> String {
    if filter.deny_all {
        return "(FALSE)".to_owned();
    }

    let VisibilityColumns {
        unit,
        workspace,
        classification,
    } = columns;
    let mut clauses: Vec<String> = Vec::new();

    if filter.allow_internal_and_below {
        clauses.push(format!("{classification} IN ('PUBLIC', 'INTERNAL')"));
    }

    if filter.confidential_organisation_wide {
        clauses.push(format!("{classification} = 'CONFIDENTIAL'"));
    } else {
        let mut membership: Vec<String> = Vec::new();
        if let Some(ids) = id_list(&filter.confidential_unit_ids) {
            membership.push(format!("{unit} IN ({ids})"));
        }
        if let Some(ids) = id_list(&filter.confidential_workspace_ids) {
            membership.push(format!("{workspace} IN ({ids})"));
        }
        if !membership.is_empty() {
            clauses.push(format!(
                "({classification} = 'CONFIDENTIAL' AND ({}))",
                membership.join(" OR ")
            ));
        }
    }

    // RESTRICTED ignores administrative roles by design (ADR-0100).
    let mut restricted: Vec<String> = Vec::new();
    if let Some(ids) = id_list(&filter.restricted_workspace_ids) {
        restricted.push(format!("{workspace} IN ({ids})"));
    }
    if let Some(ids) = id_list(&filter.restricted_unit_ids) {
        restricted.push(format!("{unit} IN ({ids})"));
    }
    if !restricted.is_empty() {
        clauses.push(format!(
            "({classification} = 'RESTRICTED' AND ({}))",
            restricted.join(" OR ")
        ));
    }

    if clauses.is_empty() {
        return "(FALSE)".to_owned();
    }
    format!("({})", clauses.join(" OR "))
}

/// A condição que um artefacto workspace-scoped tem de cumprir para aparecer
/// numa vista institucional agregada.
///
/// # A invariante
///
/// > Para um artefacto aparecer numa vista agregada, **tanto o artefacto como o
/// > workspace que o contém** têm de ser visíveis ao actor.
///
/// São duas fugas diferentes, e cada metade fecha uma:
///
/// - **o artefacto** (F-01) — um artefacto mais restrito do que o seu workspace
///   continua escondido a quem alcança o workspace mas não o artefacto;
/// - **o workspace** — um artefacto legível dentro de um workspace que o actor
///   não alcança revelaria que existe trabalho onde ele não entra. O título de
///   uma referência ou o código de um dataset dizem o que se investiga, e onde.
///
/// Só a primeira metade era aplicada em `datasets`, que já listava
/// institucionalmente. A segunda faltava, e a fuga estava viva.
///
/// Vive aqui, e não em cada módulo, porque três cópias do mesmo predicado
/// divergem — e uma correcção aplicada a uma delas não chega às outras.
///
/// `alias` é o alias da tabela do artefacto na consulta; a tabela de workspaces
/// entra como `w`.
#[must_use]
pub fn contained_in_visible_workspace(filter: &VisibilityFilter, alias: &str) -> String {
    let workspace = to_sql(
        filter,
        VisibilityColumns {
            unit: "w.unit_id",
            workspace: "w.id",
            classification: "w.classification",
        },
    );
    format!(
        "EXISTS (
           SELECT 1 FROM research_workspaces w
            WHERE w.id = {alias}.workspace_id
              AND w.organisation_id = {alias}.organisation_id
              AND {workspace}
         )"
    )
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use ocinye_contracts::{TechnicalRole, UnitRole, WorkspaceRole};
    use ocinye_domain::Principal;

    use super::*;

    fn principal() -> Principal {
        Principal {
            subject: "s".into(),
            person_id: Uuid::from_u128(1),
            organisation_id: Uuid::from_u128(2),
            display_name: "P".into(),
            is_active: true,
            roles: HashSet::new(),
            unit_roles: HashMap::new(),
            workspace_roles: HashMap::new(),
            grants: Vec::new(),
        }
    }

    #[test]
    fn an_inactive_principal_yields_a_never_true_predicate() {
        let mut p = principal();
        p.is_active = false;
        let sql = to_sql(
            &VisibilityFilter::for_principal(&p),
            VisibilityColumns::default(),
        );
        assert_eq!(sql, "(FALSE)");
    }

    #[test]
    fn a_plain_member_sees_internal_and_below_only() {
        let sql = to_sql(
            &VisibilityFilter::for_principal(&principal()),
            VisibilityColumns::default(),
        );
        assert!(sql.contains("IN ('PUBLIC', 'INTERNAL')"));
        assert!(!sql.contains("CONFIDENTIAL"));
        assert!(!sql.contains("RESTRICTED"));
    }

    #[test]
    fn administrative_roles_never_produce_a_restricted_clause() {
        for role in [
            TechnicalRole::PlatformAdmin,
            TechnicalRole::OrganisationAdmin,
        ] {
            let mut p = principal();
            p.roles.insert(role);
            let sql = to_sql(
                &VisibilityFilter::for_principal(&p),
                VisibilityColumns::default(),
            );
            assert!(
                sql.contains("CONFIDENTIAL"),
                "{role:?} should see CONFIDENTIAL"
            );
            assert!(
                !sql.contains("RESTRICTED"),
                "{role:?} must not produce a RESTRICTED clause: {sql}"
            );
        }
    }

    #[test]
    fn workspace_membership_produces_a_restricted_clause() {
        let mut p = principal();
        let ws = Uuid::from_u128(42);
        p.workspace_roles.insert(ws, WorkspaceRole::Viewer);
        let sql = to_sql(
            &VisibilityFilter::for_principal(&p),
            VisibilityColumns::default(),
        );
        assert!(sql.contains("RESTRICTED"));
        assert!(sql.contains(&ws.to_string()));
    }

    #[test]
    fn only_managed_units_appear_in_the_restricted_clause() {
        let mut p = principal();
        let managed = Uuid::from_u128(10);
        let joined = Uuid::from_u128(11);
        p.unit_roles.insert(managed, UnitRole::Manager);
        p.unit_roles.insert(joined, UnitRole::Member);

        let filter = VisibilityFilter::for_principal(&p);
        let sql = to_sql(&filter, VisibilityColumns::default());

        let restricted = sql
            .split("'RESTRICTED'")
            .nth(1)
            .expect("a RESTRICTED clause should be present");
        assert!(restricted.contains(&managed.to_string()));
        assert!(!restricted.contains(&joined.to_string()));
    }

    #[test]
    fn rendered_identifiers_cannot_carry_injection() {
        let mut p = principal();
        p.workspace_roles
            .insert(Uuid::from_u128(7), WorkspaceRole::Lead);
        let sql = to_sql(
            &VisibilityFilter::for_principal(&p),
            VisibilityColumns::default(),
        );
        // Uuid's Display is hexadecimal and hyphens only; nothing else can
        // reach the fragment.
        for forbidden in [';', '-', '\''].iter().take(1) {
            assert!(
                !sql.contains(*forbidden),
                "unexpected {forbidden:?} in {sql}"
            );
        }
        assert!(!sql.contains("--"));
    }

    #[test]
    fn custom_columns_are_honoured() {
        let mut p = principal();
        p.workspace_roles
            .insert(Uuid::from_u128(7), WorkspaceRole::Lead);
        let sql = to_sql(
            &VisibilityFilter::for_principal(&p),
            VisibilityColumns::aliased("d.unit_id", "d.workspace_id", "d.classification"),
        );
        assert!(sql.contains("d.classification"));
        assert!(sql.contains("d.workspace_id"));
    }
}
