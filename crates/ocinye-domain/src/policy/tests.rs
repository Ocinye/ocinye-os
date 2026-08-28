//! Exhaustive authorization tests.
//!
//! These enumerate rather than sample. Authorization is the one place where a
//! missed combination is a breach, so every test that can iterate over
//! `Classification::all()`, `TechnicalRole::all()` and the membership shapes
//! does so.

use std::collections::{HashMap, HashSet};

use ocinye_contracts::{Classification, TechnicalRole, UnitRole, WorkspaceRole};
use uuid::Uuid;

use super::*;
use crate::principal::Principal;

const ORG: Uuid = Uuid::from_u128(1);
const UNIT_A: Uuid = Uuid::from_u128(10);
const UNIT_B: Uuid = Uuid::from_u128(11);
const WS_A: Uuid = Uuid::from_u128(20);

fn principal() -> Principal {
    Principal {
        subject: "sub-1".into(),
        person_id: Uuid::from_u128(100),
        organisation_id: ORG,
        display_name: "Test Person".into(),
        is_active: true,
        roles: HashSet::new(),
        unit_roles: HashMap::new(),
        workspace_roles: HashMap::new(),
        grants: Vec::new(),
    }
}

fn with_role(mut p: Principal, role: TechnicalRole) -> Principal {
    p.roles.insert(role);
    p
}

fn in_unit(mut p: Principal, unit: Uuid, role: UnitRole) -> Principal {
    p.unit_roles.insert(unit, role);
    p
}

fn in_workspace(mut p: Principal, ws: Uuid, role: WorkspaceRole) -> Principal {
    p.workspace_roles.insert(ws, role);
    p
}

fn ctx(classification: Classification) -> ResourceContext {
    ResourceContext::workspace(ResourceKind::Note, ORG, UNIT_A, WS_A, classification)
}

fn allowed(p: &Principal, action: Action, c: &ResourceContext) -> bool {
    evaluate(p, action, c).allowed
}

// --- The rule the whole model exists to protect ---------------------------

#[test]
fn no_administrative_role_alone_grants_restricted_read() {
    let restricted = ctx(Classification::Restricted);
    for role in TechnicalRole::all() {
        let p = with_role(principal(), role);
        assert!(
            !allowed(&p, Action::Read, &restricted),
            "technical role {role:?} must not grant RESTRICTED read without membership"
        );
    }
}

#[test]
fn founder_title_is_not_represented_in_the_policy() {
    // Institutional position is absent from Principal by construction. This
    // test documents that absence: a "Founder" with no technical role and no
    // membership is an ordinary member.
    let p = principal();
    assert!(allowed(&p, Action::Read, &ctx(Classification::Internal)));
    assert!(!allowed(
        &p,
        Action::Read,
        &ctx(Classification::Confidential)
    ));
    assert!(!allowed(&p, Action::Read, &ctx(Classification::Restricted)));
}

#[test]
fn explicit_workspace_membership_grants_restricted_read_in_every_role() {
    let restricted = ctx(Classification::Restricted);
    for role in WorkspaceRole::all() {
        let p = in_workspace(principal(), WS_A, role);
        assert!(
            allowed(&p, Action::Read, &restricted),
            "workspace role {role:?} should grant RESTRICTED read"
        );
    }
}

#[test]
fn unit_manager_reads_restricted_but_plain_unit_member_does_not() {
    let restricted = ctx(Classification::Restricted);
    assert!(allowed(
        &in_unit(principal(), UNIT_A, UnitRole::Manager),
        Action::Read,
        &restricted
    ));
    assert!(!allowed(
        &in_unit(principal(), UNIT_A, UnitRole::Member),
        Action::Read,
        &restricted
    ));
}

// --- Denial shapes ---------------------------------------------------------

#[test]
fn inactive_principal_is_denied_every_action_at_every_classification() {
    let mut p = principal();
    p.is_active = false;
    p.roles.insert(TechnicalRole::PlatformAdmin);
    p.workspace_roles.insert(WS_A, WorkspaceRole::Lead);

    for classification in Classification::all() {
        let context = ctx(classification);
        for action in every_action() {
            assert!(
                !allowed(&p, action, &context),
                "inactive principal must be denied {action:?} at {classification}"
            );
        }
    }
}

#[test]
fn cross_organisation_access_is_denied_even_for_platform_admin() {
    let p = with_role(principal(), TechnicalRole::PlatformAdmin);
    let foreign = ResourceContext {
        organisation_id: Some(Uuid::from_u128(999)),
        ..ctx(Classification::Public)
    };
    for action in every_action() {
        assert!(
            !allowed(&p, action, &foreign),
            "{action:?} must not cross organisations"
        );
    }
}

#[test]
fn denied_reads_hide_existence_and_denied_writes_do_not() {
    let restricted = ctx(Classification::Restricted);
    let outsider = principal();
    assert_eq!(
        authorize(&outsider, Action::Read, &restricted)
            .unwrap_err()
            .0,
        Denial::NotFound
    );

    // A viewer can read but not write: existence is already known to them, so
    // the denial may be honest about being a permission problem.
    let viewer = in_workspace(principal(), WS_A, WorkspaceRole::Viewer);
    assert_eq!(
        authorize(&viewer, Action::Update, &restricted)
            .unwrap_err()
            .0,
        Denial::Forbidden
    );

    // A non-member's write denial must not reveal that the resource exists.
    assert_eq!(
        authorize(&outsider, Action::Update, &restricted)
            .unwrap_err()
            .0,
        Denial::NotFound
    );
}

// --- Write and membership --------------------------------------------------

#[test]
fn viewers_never_write_at_any_classification() {
    for classification in Classification::all() {
        let p = in_workspace(principal(), WS_A, WorkspaceRole::Viewer);
        let context = ctx(classification);
        for action in [
            Action::Create,
            Action::Update,
            Action::Archive,
            Action::Transition,
        ] {
            assert!(
                !allowed(&p, action, &context),
                "viewer must not {action:?} at {classification}"
            );
        }
    }
}

#[test]
fn membership_in_another_unit_grants_nothing_here() {
    let p = in_unit(principal(), UNIT_B, UnitRole::Manager);
    for classification in [Classification::Confidential, Classification::Restricted] {
        assert!(
            !allowed(&p, Action::Read, &ctx(classification)),
            "membership of another unit must not grant {classification} access"
        );
    }
}

#[test]
fn only_lead_manager_or_admin_may_classify_or_manage_members() {
    let context = ctx(Classification::Internal);
    for action in [Action::Classify, Action::ManageMembers] {
        assert!(allowed(
            &in_workspace(principal(), WS_A, WorkspaceRole::Lead),
            action,
            &context
        ));
        assert!(allowed(
            &in_unit(principal(), UNIT_A, UnitRole::Manager),
            action,
            &context
        ));
        assert!(allowed(
            &with_role(principal(), TechnicalRole::OrganisationAdmin),
            action,
            &context
        ));
        assert!(!allowed(
            &in_workspace(principal(), WS_A, WorkspaceRole::Member),
            action,
            &context
        ));
    }
}

// --- Export ----------------------------------------------------------------

#[test]
fn exporting_restricted_is_narrower_than_reading_it() {
    let restricted = ctx(Classification::Restricted);

    let member = in_workspace(principal(), WS_A, WorkspaceRole::Member);
    assert!(allowed(&member, Action::Read, &restricted));
    assert!(!allowed(&member, Action::Export, &restricted));

    assert!(allowed(
        &in_workspace(principal(), WS_A, WorkspaceRole::Lead),
        Action::Export,
        &restricted
    ));
    assert!(allowed(
        &in_unit(principal(), UNIT_A, UnitRole::Manager),
        Action::Export,
        &restricted
    ));
}

#[test]
fn download_never_exceeds_read() {
    for classification in Classification::all() {
        for shape in membership_shapes() {
            let context = ctx(classification);
            if allowed(&shape, Action::Download, &context) {
                assert!(
                    allowed(&shape, Action::Read, &context),
                    "download allowed where read is not, at {classification}"
                );
            }
        }
    }
}

// --- Audit -----------------------------------------------------------------

#[test]
fn audit_reading_requires_an_explicit_role() {
    let context = ResourceContext::organisation(ResourceKind::AuditEvent, ORG);
    assert!(allowed(
        &with_role(principal(), TechnicalRole::Auditor),
        Action::ReadAudit,
        &context
    ));
    assert!(!allowed(
        &in_workspace(principal(), WS_A, WorkspaceRole::Lead),
        Action::ReadAudit,
        &context
    ));
}

#[test]
fn auditor_role_grants_no_access_to_institutional_content() {
    let p = with_role(principal(), TechnicalRole::Auditor);
    for classification in [Classification::Confidential, Classification::Restricted] {
        assert!(
            !allowed(&p, Action::Read, &ctx(classification)),
            "auditor must not read {classification} content"
        );
    }
}

// --- Administration --------------------------------------------------------

#[test]
fn only_platform_admin_administers() {
    let context = ResourceContext::organisation(ResourceKind::Platform, ORG);
    for role in TechnicalRole::all() {
        let expected = role == TechnicalRole::PlatformAdmin;
        assert_eq!(
            allowed(&with_role(principal(), role), Action::Administer, &context),
            expected,
            "administration by {role:?}"
        );
    }
}

// --- Equivalence between the policy and the SQL-side filter -----------------

#[test]
fn visibility_filter_agrees_with_the_read_policy_exhaustively() {
    let mut checked = 0_usize;
    for p in membership_shapes() {
        let filter = VisibilityFilter::for_principal(&p);
        for classification in Classification::all() {
            for (unit_id, workspace_id) in scope_shapes() {
                let context = ResourceContext {
                    kind: ResourceKind::Note,
                    classification,
                    unit_id,
                    workspace_id,
                    organisation_id: Some(ORG),
                };
                let by_policy = evaluate(&p, Action::Read, &context).allowed;
                let by_filter = filter.permits(unit_id, workspace_id, classification);
                assert_eq!(
                    by_policy, by_filter,
                    "policy and visibility filter disagree: roles={:?} units={:?} \
                     workspaces={:?} classification={classification} unit_id={unit_id:?} \
                     workspace_id={workspace_id:?}",
                    p.roles, p.unit_roles, p.workspace_roles
                );
                checked += 1;
            }
        }
    }
    // Guards against the enumeration silently collapsing to a trivial case.
    assert!(
        checked > 500,
        "expected a broad enumeration, checked only {checked}"
    );
}

#[test]
fn inactive_principal_filter_matches_inactive_policy() {
    let mut p = principal();
    p.is_active = false;
    let filter = VisibilityFilter::for_principal(&p);
    assert!(filter.is_never_satisfiable());
    for classification in Classification::all() {
        assert!(!filter.permits(Some(UNIT_A), Some(WS_A), classification));
    }
}

// --- Fixtures --------------------------------------------------------------

fn every_action() -> [Action; 11] {
    [
        Action::Read,
        Action::Create,
        Action::Update,
        Action::Archive,
        Action::Transition,
        Action::Classify,
        Action::ManageMembers,
        Action::Download,
        Action::Export,
        Action::Administer,
        Action::ReadAudit,
    ]
}

fn scope_shapes() -> Vec<(Option<Uuid>, Option<Uuid>)> {
    vec![
        (None, None),
        (Some(UNIT_A), None),
        (Some(UNIT_B), None),
        (Some(UNIT_A), Some(WS_A)),
        (Some(UNIT_B), Some(WS_A)),
        (Some(UNIT_A), Some(Uuid::from_u128(21))),
    ]
}

/// Every membership shape worth distinguishing: no membership, each technical
/// role alone, each unit role, each workspace role, and the combinations that
/// have historically hidden authorization bugs.
fn membership_shapes() -> Vec<Principal> {
    let mut shapes = vec![principal()];

    for role in TechnicalRole::all() {
        shapes.push(with_role(principal(), role));
    }
    for role in UnitRole::all() {
        shapes.push(in_unit(principal(), UNIT_A, role));
        shapes.push(in_unit(principal(), UNIT_B, role));
    }
    for role in WorkspaceRole::all() {
        shapes.push(in_workspace(principal(), WS_A, role));
    }
    for technical in TechnicalRole::all() {
        for workspace in WorkspaceRole::all() {
            shapes.push(in_workspace(
                with_role(principal(), technical),
                WS_A,
                workspace,
            ));
        }
        for unit in UnitRole::all() {
            shapes.push(in_unit(with_role(principal(), technical), UNIT_A, unit));
        }
    }
    shapes
}

/// Criar não depende do tipo do recurso.
///
/// # Porque isto tem de ser um teste
///
/// A interface serve um único booleano — `WorkspaceView.may_create` — para
/// alimentar dois selectores diferentes: «em que workspace crio esta
/// referência?» e «em que workspace crio este dataset?».
///
/// Esse atalho só é honesto enquanto a política decidir `Create` da mesma
/// maneira para ambos. Hoje decide: o ramo `Create` consulta classificação e
/// filiação, e **não** olha para `ctx.kind`.
///
/// Se alguém tornar a criação sensível ao tipo — uma permissão própria para
/// fontes, digamos — este teste falha, e falha **antes** de o selector começar
/// a oferecer o workspace errado. É essa a razão de ele existir: o booleano
/// único é uma aproximação, e isto é o que a mantém verdadeira.
#[test]
fn criar_nao_depende_do_tipo_de_recurso() {
    const TIPOS: [ResourceKind; 8] = [
        ResourceKind::Source,
        ResourceKind::Dataset,
        ResourceKind::Note,
        ResourceKind::Document,
        ResourceKind::Idea,
        ResourceKind::Project,
        ResourceKind::Task,
        ResourceKind::Comment,
    ];

    // Vários actores, para que a igualdade não seja verdadeira por acidente de
    // um deles poder — ou não poder — tudo.
    let actores = [
        principal(),
        in_workspace(principal(), WS_A, WorkspaceRole::Member),
        in_workspace(principal(), WS_A, WorkspaceRole::Viewer),
        in_unit(principal(), UNIT_A, UnitRole::Manager),
        with_role(principal(), TechnicalRole::PlatformAdmin),
    ];

    for actor in &actores {
        let referencia = authorize(
            actor,
            Action::Create,
            &ResourceContext::workspace(
                ResourceKind::Source,
                ORG,
                UNIT_A,
                WS_A,
                Classification::Internal,
            ),
        )
        .is_ok();

        for tipo in TIPOS {
            let decisao = authorize(
                actor,
                Action::Create,
                &ResourceContext::workspace(tipo, ORG, UNIT_A, WS_A, Classification::Internal),
            )
            .is_ok();
            assert_eq!(
                decisao, referencia,
                "criar {tipo:?} decide-se de forma diferente de criar Source; \
                 um `may_create` único deixou de ser suficiente para os selectores"
            );
        }
    }
}
