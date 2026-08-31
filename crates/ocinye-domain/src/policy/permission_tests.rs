//! Exhaustive tests for the permission layer.
//!
//! The policy is pure, so these enumerate rather than sample. Every DENY here
//! is a boundary someone could otherwise cross by accident (briefing §105).

use std::collections::{HashMap, HashSet};

use ocinye_contracts::{Classification, Permission, Scope, TechnicalRole, UnitRole, WorkspaceRole};
use uuid::Uuid;

use super::permissions::{can, effective_permissions, explain, AccessSource, ExplicitGrant};
use super::{ResourceContext, ResourceKind};
use crate::principal::Principal;

const ORG: Uuid = Uuid::from_u128(1);
const OTHER_ORG: Uuid = Uuid::from_u128(2);
const UNIT_A: Uuid = Uuid::from_u128(10);
const UNIT_B: Uuid = Uuid::from_u128(11);
const WS_A: Uuid = Uuid::from_u128(20);
const WS_B: Uuid = Uuid::from_u128(21);
const DOC: Uuid = Uuid::from_u128(30);

fn person() -> Principal {
    Principal {
        subject: "sub-1".into(),
        person_id: Uuid::from_u128(100),
        organisation_id: ORG,
        display_name: "Test Person".into(),
        is_active: true,
        identity_kind: crate::IdentityKind::Human,
        roles: HashSet::new(),
        unit_roles: HashMap::new(),
        workspace_roles: HashMap::new(),
        grants: Vec::new(),
    }
}

fn with_role(role: TechnicalRole) -> Principal {
    let mut p = person();
    p.roles.insert(role);
    p
}

fn workspace_ctx(classification: Classification) -> ResourceContext {
    ResourceContext::workspace(ResourceKind::Document, ORG, UNIT_A, WS_A, classification)
}

fn org_ctx() -> ResourceContext {
    ResourceContext::organisation(ResourceKind::Organisation, ORG)
}

// ── Deny by default ─────────────────────────────────────────────────────

#[test]
fn a_principal_with_no_roles_holds_no_permissions_anywhere() {
    let subject = person();
    for permission in Permission::all() {
        assert!(
            !can(&subject, permission, &org_ctx(), None).allowed,
            "{permission:?} was allowed with no roles at all"
        );
    }
}

#[test]
fn an_inactive_principal_holds_nothing_regardless_of_role() {
    for role in TechnicalRole::all() {
        let mut subject = with_role(role);
        subject.is_active = false;
        for permission in Permission::all() {
            assert!(
                !can(&subject, permission, &org_ctx(), None).allowed,
                "{role:?} kept {permission:?} while inactive"
            );
        }
    }
}

#[test]
fn cross_organisation_access_is_refused_for_every_role_and_permission() {
    let foreign = ResourceContext::organisation(ResourceKind::Organisation, OTHER_ORG);
    for role in TechnicalRole::all() {
        let subject = with_role(role);
        for permission in Permission::all() {
            assert!(
                !can(&subject, permission, &foreign, None).allowed,
                "{role:?} reached {permission:?} across organisations"
            );
        }
    }
}

// ── Technical administration is not scientific access ───────────────────

#[test]
fn a_platform_admin_administers_but_reads_no_restricted_science() {
    let admin = with_role(TechnicalRole::PlatformAdmin);

    assert!(can(&admin, Permission::PlatformAdminister, &org_ctx(), None).allowed);
    assert!(can(&admin, Permission::MembersCreate, &org_ctx(), None).allowed);

    let restricted = workspace_ctx(Classification::Restricted);
    for permission in [
        Permission::DocumentsView,
        Permission::DocumentsDownload,
        Permission::DatasetsDownload,
        Permission::NotesView,
    ] {
        assert!(
            !can(&admin, permission, &restricted, Some(DOC)).allowed,
            "platform_admin reached {permission:?} on RESTRICTED material"
        );
    }
}

#[test]
fn an_organisation_admin_manages_people_but_not_restricted_material() {
    let admin = with_role(TechnicalRole::OrganisationAdmin);

    assert!(can(&admin, Permission::MembersManage, &org_ctx(), None).allowed);
    assert!(can(&admin, Permission::UnitsCreate, &org_ctx(), None).allowed);

    assert!(
        !can(
            &admin,
            Permission::DocumentsView,
            &workspace_ctx(Classification::Restricted),
            Some(DOC),
        )
        .allowed
    );
    assert!(
        !can(&admin, Permission::PlatformAdminister, &org_ctx(), None).allowed,
        "organisation_admin must not administer the platform"
    );
}

#[test]
fn no_technical_role_alone_confers_restricted_content_access() {
    let restricted = workspace_ctx(Classification::Restricted);
    for role in TechnicalRole::all() {
        let subject = with_role(role);
        assert!(
            !can(
                &subject,
                Permission::DocumentsDownload,
                &restricted,
                Some(DOC)
            )
            .allowed,
            "{role:?} downloaded RESTRICTED material on role alone"
        );
    }
}

// ── Scoping ─────────────────────────────────────────────────────────────

#[test]
fn a_unit_manager_of_a_does_not_manage_unit_b() {
    let mut manager = with_role(TechnicalRole::UnitManager);
    manager.unit_roles.insert(UNIT_A, UnitRole::Manager);

    let in_a = ResourceContext::unit(ResourceKind::Unit, ORG, UNIT_A);
    let in_b = ResourceContext::unit(ResourceKind::Unit, ORG, UNIT_B);

    assert!(can(&manager, Permission::UnitsManage, &in_a, None).allowed);
    assert!(
        !can(&manager, Permission::UnitsManage, &in_b, None).allowed,
        "unit manager of A managed unit B"
    );
}

#[test]
fn a_research_lead_of_one_workspace_does_not_manage_another() {
    let mut lead = with_role(TechnicalRole::ResearchLead);
    lead.workspace_roles.insert(WS_A, WorkspaceRole::Lead);

    let in_a = ResourceContext::workspace(
        ResourceKind::Project,
        ORG,
        UNIT_A,
        WS_A,
        Classification::Internal,
    );
    let in_b = ResourceContext::workspace(
        ResourceKind::Project,
        ORG,
        UNIT_A,
        WS_B,
        Classification::Internal,
    );

    assert!(can(&lead, Permission::ResearchMembersManage, &in_a, None).allowed);
    assert!(
        !can(&lead, Permission::ResearchMembersManage, &in_b, None).allowed,
        "research lead of A managed workspace B"
    );
}

#[test]
fn a_workspace_viewer_reads_but_never_downloads() {
    let mut viewer = with_role(TechnicalRole::ResearchMember);
    viewer.workspace_roles.insert(WS_A, WorkspaceRole::Viewer);
    let ctx = workspace_ctx(Classification::Confidential);

    assert!(can(&viewer, Permission::DocumentsView, &ctx, Some(DOC)).allowed);
    assert!(
        !can(&viewer, Permission::DocumentsDownload, &ctx, Some(DOC)).allowed,
        "a viewer took a copy"
    );
    assert!(!can(&viewer, Permission::NotesCreate, &ctx, None).allowed);
}

#[test]
fn an_external_collaborator_starts_from_nothing() {
    let outsider = with_role(TechnicalRole::ExternalCollaborator);
    for permission in Permission::all() {
        assert!(
            !can(&outsider, permission, &org_ctx(), None).allowed,
            "external collaborator held {permission:?} institution-wide"
        );
    }
}

#[test]
fn an_external_collaborator_reaches_only_the_workspace_they_were_placed_in() {
    let mut outsider = with_role(TechnicalRole::ExternalCollaborator);
    outsider.workspace_roles.insert(WS_A, WorkspaceRole::Member);

    let granted = ResourceContext::workspace(
        ResourceKind::Note,
        ORG,
        UNIT_A,
        WS_A,
        Classification::Confidential,
    );
    let other = ResourceContext::workspace(
        ResourceKind::Note,
        ORG,
        UNIT_A,
        WS_B,
        Classification::Confidential,
    );

    assert!(can(&outsider, Permission::NotesCreate, &granted, None).allowed);
    assert!(!can(&outsider, Permission::NotesView, &other, None).allowed);
    assert!(!can(&outsider, Permission::MembersView, &org_ctx(), None).allowed);
}

// ── Auditor ─────────────────────────────────────────────────────────────

#[test]
fn an_auditor_reads_evidence_and_never_content() {
    let auditor = with_role(TechnicalRole::Auditor);
    assert!(can(&auditor, Permission::AuditView, &org_ctx(), None).allowed);

    for permission in [
        Permission::DocumentsView,
        Permission::DatasetsView,
        Permission::NotesView,
        Permission::DocumentsDownload,
    ] {
        assert!(
            !can(
                &auditor,
                permission,
                &workspace_ctx(Classification::Internal),
                Some(DOC)
            )
            .allowed,
            "auditor reached {permission:?}"
        );
    }
    assert!(!can(&auditor, Permission::MembersManage, &org_ctx(), None).allowed);
}

// ── Explicit grants ─────────────────────────────────────────────────────

#[test]
fn an_explicit_grant_opens_restricted_material_only_in_its_scope() {
    let mut subject = with_role(TechnicalRole::ResearchMember);
    subject.grants.push(ExplicitGrant {
        permission: Permission::DocumentsDownload,
        scope: Scope::ResearchWorkspace,
        scope_id: Some(WS_A),
    });

    let inside = workspace_ctx(Classification::Restricted);
    let elsewhere = ResourceContext::workspace(
        ResourceKind::Document,
        ORG,
        UNIT_A,
        WS_B,
        Classification::Restricted,
    );

    assert!(can(&subject, Permission::DocumentsDownload, &inside, Some(DOC)).allowed);
    assert!(
        !can(
            &subject,
            Permission::DocumentsDownload,
            &elsewhere,
            Some(DOC)
        )
        .allowed,
        "a workspace-scoped grant leaked into another workspace"
    );
}

#[test]
fn a_grant_confers_only_the_permission_it_names() {
    let mut subject = with_role(TechnicalRole::ResearchMember);
    subject.grants.push(ExplicitGrant {
        permission: Permission::DocumentsView,
        scope: Scope::ResearchWorkspace,
        scope_id: Some(WS_A),
    });
    let ctx = workspace_ctx(Classification::Restricted);

    assert!(can(&subject, Permission::DocumentsView, &ctx, Some(DOC)).allowed);
    assert!(
        !can(&subject, Permission::DocumentsDownload, &ctx, Some(DOC)).allowed,
        "a view grant became a download grant"
    );
    assert!(!can(&subject, Permission::DocumentsManage, &ctx, Some(DOC)).allowed);
}

#[test]
fn a_resource_scoped_grant_applies_to_that_resource_alone() {
    let mut subject = with_role(TechnicalRole::ResearchMember);
    subject.grants.push(ExplicitGrant {
        permission: Permission::DocumentsDownload,
        scope: Scope::Resource,
        scope_id: Some(DOC),
    });
    let ctx = workspace_ctx(Classification::Restricted);
    let other_doc = Uuid::from_u128(31);

    assert!(can(&subject, Permission::DocumentsDownload, &ctx, Some(DOC)).allowed);
    assert!(
        !can(
            &subject,
            Permission::DocumentsDownload,
            &ctx,
            Some(other_doc)
        )
        .allowed
    );
}

#[test]
fn a_scoped_grant_that_names_no_target_confers_nothing() {
    for scope in [Scope::Unit, Scope::ResearchWorkspace, Scope::Resource] {
        let mut subject = with_role(TechnicalRole::ResearchMember);
        subject.grants.push(ExplicitGrant {
            permission: Permission::DocumentsDownload,
            scope,
            scope_id: None,
        });
        assert!(
            !can(
                &subject,
                Permission::DocumentsDownload,
                &workspace_ctx(Classification::Restricted),
                Some(DOC),
            )
            .allowed,
            "{scope:?} grant with no identifier was honoured"
        );
    }
}

#[test]
fn revoking_a_grant_removes_the_access_it_conferred() {
    // Revocation is expressed by the grant no longer reaching the policy: the
    // repository filters revoked and expired grants out.
    let mut subject = with_role(TechnicalRole::ResearchMember);
    let grant = ExplicitGrant {
        permission: Permission::DocumentsDownload,
        scope: Scope::ResearchWorkspace,
        scope_id: Some(WS_A),
    };
    subject.grants.push(grant);
    let ctx = workspace_ctx(Classification::Restricted);
    assert!(can(&subject, Permission::DocumentsDownload, &ctx, Some(DOC)).allowed);

    subject.grants.clear();
    assert!(!can(&subject, Permission::DocumentsDownload, &ctx, Some(DOC)).allowed);
}

// ── Compute is its own dimension ────────────────────────────────────────

#[test]
fn compute_permissions_are_independent_of_research_access() {
    let mut member = with_role(TechnicalRole::ResearchMember);
    member.workspace_roles.insert(WS_A, WorkspaceRole::Member);
    let ctx = workspace_ctx(Classification::Internal);

    assert!(can(&member, Permission::ComputeSubmitJob, &ctx, None).allowed);
    assert!(
        !can(&member, Permission::ComputeManageNodes, &ctx, None).allowed,
        "submitting a job became administering a node"
    );
    assert!(!can(&member, Permission::ComputeAdmin, &ctx, None).allowed);

    // And the reverse: a platform admin operates nodes without research access.
    let admin = with_role(TechnicalRole::PlatformAdmin);
    assert!(can(&admin, Permission::ComputeManageNodes, &org_ctx(), None).allowed);
    assert!(!can(&admin, Permission::ComputeSubmitJob, &ctx, None).allowed);
}

// ── Agent creation is graded ────────────────────────────────────────────

#[test]
fn agent_creation_rights_do_not_escalate_by_themselves() {
    let mut member = with_role(TechnicalRole::ResearchMember);
    member.workspace_roles.insert(WS_A, WorkspaceRole::Member);
    let ctx = workspace_ctx(Classification::Internal);

    assert!(can(&member, Permission::AgentsCreatePersonal, &ctx, None).allowed);
    for higher in [
        Permission::AgentsCreateProject,
        Permission::AgentsCreateUnit,
        Permission::AgentsCreateInstitutional,
        Permission::AgentsManage,
        Permission::AiInfrastructureManage,
    ] {
        assert!(
            !can(&member, higher, &ctx, None).allowed,
            "research member reached {higher:?}"
        );
    }

    let mut lead = with_role(TechnicalRole::ResearchLead);
    lead.workspace_roles.insert(WS_A, WorkspaceRole::Lead);
    assert!(can(&lead, Permission::AgentsCreateProject, &ctx, None).allowed);
    assert!(!can(&lead, Permission::AgentsCreateInstitutional, &ctx, None).allowed);
}

// ── Explainability ──────────────────────────────────────────────────────

#[test]
fn access_is_explainable_by_its_actual_source() {
    let ctx = workspace_ctx(Classification::Confidential);

    let mut by_workspace = with_role(TechnicalRole::ResearchMember);
    by_workspace
        .workspace_roles
        .insert(WS_A, WorkspaceRole::Member);
    assert!(matches!(
        explain(&by_workspace, Permission::NotesCreate, &ctx, None),
        Some(AccessSource::WorkspaceMembership { .. })
    ));

    let mut by_unit = with_role(TechnicalRole::UnitManager);
    by_unit.unit_roles.insert(UNIT_A, UnitRole::Manager);
    assert!(matches!(
        explain(&by_unit, Permission::UnitsManage, &ctx, None),
        Some(AccessSource::UnitMembership { .. })
    ));

    let mut by_grant = with_role(TechnicalRole::ResearchMember);
    by_grant.grants.push(ExplicitGrant {
        permission: Permission::DocumentsDownload,
        scope: Scope::ResearchWorkspace,
        scope_id: Some(WS_A),
    });
    assert!(matches!(
        explain(&by_grant, Permission::DocumentsDownload, &ctx, Some(DOC)),
        Some(AccessSource::ExplicitGrant { .. })
    ));

    let admin = with_role(TechnicalRole::OrganisationAdmin);
    assert!(matches!(
        explain(&admin, Permission::MembersCreate, &org_ctx(), None),
        Some(AccessSource::TechnicalRole { .. })
    ));

    let nobody = person();
    assert!(explain(&nobody, Permission::MembersCreate, &org_ctx(), None).is_none());
}

#[test]
fn what_can_allows_is_exactly_what_explain_can_source() {
    // If `can` says yes, an administrator must be able to be told why.
    let mut subject = with_role(TechnicalRole::UnitManager);
    subject.unit_roles.insert(UNIT_A, UnitRole::Manager);
    subject.workspace_roles.insert(WS_A, WorkspaceRole::Lead);

    let ctx = workspace_ctx(Classification::Internal);
    for permission in effective_permissions(&subject, &ctx, Some(DOC)) {
        if can(&subject, permission, &ctx, Some(DOC)).allowed {
            assert!(
                explain(&subject, permission, &ctx, Some(DOC)).is_some(),
                "{permission:?} is allowed but has no explainable source"
            );
        }
    }
}
