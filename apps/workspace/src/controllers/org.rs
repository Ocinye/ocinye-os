//! D006 · Organização: Unidades e Administração (Membros, Papéis, Instância).
//!
//! Só mapeamentos puros: o que o Core devolveu, na forma dos ViewModels do
//! Design. Nenhuma função daqui decide autoridade — cada acção só existe
//! quando o Core deu o sinal que a autoriza (`may_manage_account`,
//! `may_manage_roles`, `may_manage_members`…), e o Core volta a decidir na
//! operação. A vista não ordena papéis, não calcula permissões e não conhece
//! as invariantes: recebe-as já resolvidas ou recusadas.

use std::collections::HashMap;

use serde_json::Value;

use crate::api::ApiFailure;
use crate::controllers::desktop::{instant, text, Clock};
use crate::controllers::research as rs;
use crate::i18n::t;
use crate::ui::view_models::{
    OrgAccessVm, OrgAccountStatus, OrgActionKind, OrgActionVm, OrgAvatarVm, OrgConfirmVm,
    OrgGrantVm, OrgMemberRowVm, OrgNotice, OrgPermissionSource, OrgPermissionVm, OrgPosition,
    OrgReason, OrgRefusal, OrgRoleDefVm, OrgRoleGrantVm, OrgRoleHeldVm, OrgSecurityVm,
    OrgSessionVm, OrgTechRole, OrgTempCredVm, OrgUnitRefVm, OrgUnitRole, OrgUnitStatus,
    OrgWorkspaceRefVm, OrgWorkspaceRole, ResItemVm, ResOptionVm, ResTone, UnitMemberVm,
};

// ── Vocabulários do Core, um a um ────────────────────────────────────────

/// O estado da conta: os quatro do Core, sem juntar suspensa e desactivada.
#[must_use]
pub fn status(v: &str) -> Option<OrgAccountStatus> {
    Some(match v {
        "invited" => OrgAccountStatus::Invited,
        "active" => OrgAccountStatus::Active,
        "suspended" => OrgAccountStatus::Suspended,
        "disabled" => OrgAccountStatus::Disabled,
        _ => return None,
    })
}

/// Os papéis técnicos de sistema, pela ordem do catálogo do Core. A ordem não
/// é uma hierarquia: a vista dá a todos o mesmo tom.
pub const TECH_ROLES: [OrgTechRole; 8] = [
    OrgTechRole::PlatformAdmin,
    OrgTechRole::OrganisationAdmin,
    OrgTechRole::UnitManager,
    OrgTechRole::ResearchLead,
    OrgTechRole::ResearchMember,
    OrgTechRole::Collaborator,
    OrgTechRole::ExternalCollaborator,
    OrgTechRole::Auditor,
];

/// Um papel técnico pelo identificador estável.
#[must_use]
pub fn tech_role(v: &str) -> Option<OrgTechRole> {
    TECH_ROLES.into_iter().find(|r| r.as_str() == v)
}

/// O papel numa unidade.
#[must_use]
pub fn unit_role(v: &str) -> Option<OrgUnitRole> {
    Some(match v {
        "manager" => OrgUnitRole::Manager,
        "member" => OrgUnitRole::Member,
        _ => return None,
    })
}

/// O papel num ambiente de investigação.
#[must_use]
pub fn workspace_role(v: &str) -> Option<OrgWorkspaceRole> {
    Some(match v {
        "lead" => OrgWorkspaceRole::Lead,
        "member" => OrgWorkspaceRole::Member,
        "viewer" => OrgWorkspaceRole::Viewer,
        _ => return None,
    })
}

/// As nove posições institucionais, pelo código estável.
pub const POSITIONS: [(&str, OrgPosition); 9] = [
    ("founder", OrgPosition::Founder),
    ("director", OrgPosition::Director),
    ("unit_lead", OrgPosition::UnitLead),
    ("principal_investigator", OrgPosition::PrincipalInvestigator),
    ("researcher", OrgPosition::Researcher),
    ("engineer", OrgPosition::Engineer),
    ("fellow", OrgPosition::Fellow),
    ("student", OrgPosition::Student),
    ("external_collaborator", OrgPosition::ExternalCollaborator),
];

/// Uma posição institucional.
#[must_use]
pub fn position(v: &str) -> Option<OrgPosition> {
    POSITIONS.iter().find(|(c, _)| *c == v).map(|(_, p)| *p)
}

/// As posições como opções, com «Sem posição» (valor vazio) primeiro.
#[must_use]
pub fn position_options(selected: Option<&str>, none_key: &str) -> Vec<ResOptionVm> {
    let mut o = vec![ResOptionVm {
        value: String::new(),
        label: t(none_key).to_owned(),
        selected: selected.is_none_or(str::is_empty),
    }];
    o.extend(POSITIONS.iter().map(|(code, p)| ResOptionVm {
        value: (*code).to_owned(),
        label: t(p.key()).to_owned(),
        selected: selected == Some(*code),
    }));
    o
}

/// A origem de uma permissão, como o Core a explica (`source.label()`).
#[must_use]
pub fn source(v: &str) -> Option<OrgPermissionSource> {
    Some(match v {
        "technical_role" => OrgPermissionSource::TechnicalRole,
        "unit_membership" => OrgPermissionSource::UnitMembership,
        "workspace_membership" => OrgPermissionSource::WorkspaceMembership,
        "explicit_grant" => OrgPermissionSource::ExplicitGrant,
        _ => return None,
    })
}

/// O estado de uma unidade.
#[must_use]
pub fn unit_status(v: &str) -> Option<OrgUnitStatus> {
    Some(match v {
        "active" => OrgUnitStatus::Active,
        "archived" => OrgUnitStatus::Archived,
        _ => return None,
    })
}

/// As iniciais pela regra da casca (as primeiras letras das duas primeiras
/// palavras). Nenhuma imagem de outra pessoa: o Core só serve a do próprio.
#[must_use]
pub fn avatar(name: &str) -> OrgAvatarVm {
    OrgAvatarVm {
        initials: name
            .split_whitespace()
            .filter_map(|p| p.chars().next())
            .take(2)
            .flat_map(char::to_uppercase)
            .collect(),
        image_href: None,
    }
}

// ── Recusas e avisos: vocabulários fechados no endereço ──────────────────

/// A recusa pelo motivo estável do Core (`details.reason`), ou pelo que o
/// endereço devolve depois de um POST recusado. Um valor desconhecido não é
/// recusa nenhuma.
#[must_use]
pub fn refusal(v: &str) -> Option<OrgRefusal> {
    Some(match v {
        "self_lockout" => OrgRefusal::SelfLockout,
        "last_platform_admin" => OrgRefusal::LastPlatformAdmin,
        "last_unit_manager" => OrgRefusal::LastUnitManager,
        "unheld" => OrgRefusal::CannotGrantUnheld,
        "not_deletable" => OrgRefusal::NotDeletable,
        "stale" => OrgRefusal::StaleState,
        "option" => OrgRefusal::OptionUnavailable,
        _ => return None,
    })
}

/// O valor estável de uma recusa, para o endereço.
#[must_use]
pub const fn refusal_code(r: OrgRefusal) -> &'static str {
    match r {
        OrgRefusal::SelfLockout => "self_lockout",
        OrgRefusal::LastPlatformAdmin => "last_platform_admin",
        OrgRefusal::LastUnitManager => "last_unit_manager",
        OrgRefusal::CannotGrantUnheld => "unheld",
        OrgRefusal::NotDeletable => "not_deletable",
        OrgRefusal::StaleState => "stale",
        OrgRefusal::OptionUnavailable => "option",
    }
}

/// O que uma falha do Core significa para uma acção organizacional.
///
/// O motivo tipado do Core primeiro; sem ele, a classe da resposta: um `409`
/// é estado que mudou por baixo (a vista relê-se), uma transição que o
/// domínio não permite é a opção que já não existe. A prosa do Core nunca é
/// lida para adivinhar mais do que isto.
#[must_use]
pub fn refusal_of(f: &ApiFailure) -> Option<OrgRefusal> {
    match f {
        ApiFailure::Refused { reason, .. } => refusal(reason),
        ApiFailure::Conflict(_) => Some(OrgRefusal::StaleState),
        _ => None,
    }
}

/// Os avisos depois de um POST aceite, pelo valor do endereço.
pub const NOTICES: [(&str, OrgNotice); 12] = [
    ("status", OrgNotice::StatusChanged),
    ("role_granted", OrgNotice::RoleGranted),
    ("role_revoked", OrgNotice::RoleRevoked),
    ("grant_revoked", OrgNotice::GrantRevoked),
    ("session_revoked", OrgNotice::SessionRevoked),
    ("position", OrgNotice::PositionChanged),
    ("member_added", OrgNotice::MemberAdded),
    ("member_removed", OrgNotice::MemberRemoved),
    ("unit_role", OrgNotice::UnitRoleChanged),
    ("unit_saved", OrgNotice::UnitSaved),
    ("unit_archived", OrgNotice::UnitArchived),
    ("settings", OrgNotice::SettingsSaved),
];

/// O aviso pelo valor do endereço.
#[must_use]
pub fn notice(v: &str) -> Option<OrgNotice> {
    NOTICES.iter().find(|(c, _)| *c == v).map(|(_, n)| *n)
}

// ── Acções: o sinal do Core, a confirmação e o POST que já existe ────────

/// O valor de `?confirm=` de cada acção.
#[must_use]
pub const fn confirm_code(k: OrgActionKind) -> &'static str {
    match k {
        OrgActionKind::Suspend => "suspend",
        OrgActionKind::Disable => "disable",
        OrgActionKind::Reactivate => "reactivate",
        OrgActionKind::ResetPassword => "reset",
        OrgActionKind::Provision => "provision",
        OrgActionKind::Reissue => "reissue",
        OrgActionKind::DeleteInvite => "delete",
        OrgActionKind::GrantRole => "grant_role",
        OrgActionKind::RevokeRole => "revoke_role",
        OrgActionKind::RevokeGrant => "revoke_grant",
        OrgActionKind::RevokeSession => "revoke_session",
        OrgActionKind::ChangeUnitRole => "unit_role",
        OrgActionKind::RemoveUnitMember => "unit_remove",
        OrgActionKind::ArchiveUnit => "archive_unit",
        OrgActionKind::LeaveConversation => "msg_leave",
        OrgActionKind::RemoveParticipant => "msg_remove",
        // Sem inventário de serviços nem operação de paragem no Core (MON-06…
        // MON-09): o código existe para o vocabulário, e não está em
        // `ACTION_KINDS`, por isso nenhum endereço abre esta confirmação.
        OrgActionKind::StopService => "stop_service",
    }
}

const ACTION_KINDS: [OrgActionKind; 16] = [
    OrgActionKind::Suspend,
    OrgActionKind::Disable,
    OrgActionKind::Reactivate,
    OrgActionKind::ResetPassword,
    OrgActionKind::Provision,
    OrgActionKind::Reissue,
    OrgActionKind::DeleteInvite,
    OrgActionKind::GrantRole,
    OrgActionKind::RevokeRole,
    OrgActionKind::RevokeGrant,
    OrgActionKind::RevokeSession,
    OrgActionKind::ChangeUnitRole,
    OrgActionKind::RemoveUnitMember,
    OrgActionKind::ArchiveUnit,
    OrgActionKind::LeaveConversation,
    OrgActionKind::RemoveParticipant,
];

/// A acção pelo valor de `?confirm=`.
#[must_use]
pub fn confirm_kind(v: &str) -> Option<OrgActionKind> {
    ACTION_KINDS.into_iter().find(|k| confirm_code(*k) == v)
}

/// A ligação que abre a confirmação de uma acção sobre o recurso em `base`.
fn act(base: &str, kind: OrgActionKind, extra: &[(&str, &str)]) -> OrgActionVm {
    let mut href = format!("{base}?confirm={}", confirm_code(kind));
    for (k, v) in extra {
        href.push('&');
        href.push_str(k);
        href.push('=');
        href.push_str(&rs::encode(v));
    }
    OrgActionVm { kind, href }
}

/// O que a segurança da conta (`GET …/security`) diz ao actor.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AccountFlags {
    /// `may_manage_account`.
    pub manage: bool,
    /// `may_be_provisioned`.
    pub provision: bool,
    /// `temporary_credential_expired`.
    pub temporary_expired: bool,
    /// `may_be_deleted`.
    pub delete: bool,
}

impl AccountFlags {
    /// Os sinais de `/security`.
    #[must_use]
    pub fn of(security: &Value) -> Self {
        let b = |k: &str| security.get(k).and_then(Value::as_bool) == Some(true);
        Self {
            manage: b("may_manage_account"),
            provision: b("may_be_provisioned"),
            temporary_expired: b("temporary_credential_expired"),
            delete: b("may_be_deleted"),
        }
    }
}

/// As acções de conta que o Core permite ao actor sobre esta conta agora.
///
/// A autoridade é o `may_manage_account` do Core; o estado segue o ciclo de
/// vida documentado, que o Core impõe (docs/identity): activa → suspender ou
/// desactivar; suspensa → reactivar ou desactivar; desactivada → nada;
/// convidada → apagar ou (re)dar acesso. Sobre a própria conta, nenhuma: o
/// Core recusa o auto-bloqueio, e a vista não o oferece.
#[must_use]
pub fn account_actions(
    base: &str,
    st: OrgAccountStatus,
    f: AccountFlags,
    is_self: bool,
) -> Vec<OrgActionVm> {
    if !f.manage || is_self {
        return Vec::new();
    }
    let mut kinds = match st {
        OrgAccountStatus::Active => vec![OrgActionKind::Suspend, OrgActionKind::Disable],
        OrgAccountStatus::Suspended => vec![OrgActionKind::Reactivate, OrgActionKind::Disable],
        OrgAccountStatus::Disabled | OrgAccountStatus::Invited => Vec::new(),
    };
    if matches!(st, OrgAccountStatus::Active | OrgAccountStatus::Invited) {
        if f.provision {
            kinds.push(if f.temporary_expired {
                OrgActionKind::Reissue
            } else {
                OrgActionKind::Provision
            });
        } else if st == OrgAccountStatus::Active {
            kinds.push(OrgActionKind::ResetPassword);
        }
    }
    if st == OrgAccountStatus::Invited && f.delete {
        kinds.push(OrgActionKind::DeleteInvite);
    }
    kinds.into_iter().map(|k| act(base, k, &[])).collect()
}

// ── Membros ──────────────────────────────────────────────────────────────

/// As linhas do roster (`GET /administration/members`).
#[must_use]
pub(crate) fn roster_rows(
    items: &[Value],
    open: Option<&str>,
    me: &str,
    list_query: &str,
    clock: &Clock,
) -> Vec<OrgMemberRowVm> {
    items
        .iter()
        .filter_map(|m| {
            let id = text(m, "id");
            let st = status(text(m, "status"))?;
            if id.is_empty() {
                return None;
            }
            let name = text(m, "full_name").to_owned();
            Some(OrgMemberRowVm {
                avatar: avatar(&name),
                name,
                email: rs::opt(m, "email"),
                status: st,
                position: position(text(m, "institutional_position")),
                units: m
                    .get("units")
                    .and_then(Value::as_array)
                    .map(|u| u.iter().map(|u| text(u, "code").to_owned()).collect())
                    .unwrap_or_default(),
                joined: instant(m, "created_at").map(|at| rs::day(at, clock)),
                last_seen: instant(m, "last_seen_at").map(|at| clock.when(at)),
                href: format!("/admin/members/{id}{list_query}"),
                active: open == Some(id),
                is_self: id == me,
            })
        })
        .collect()
}

/// As unidades de um membro: as `{code, name}` de `/people/{id}`, com o papel
/// que `/access` dá (quando o actor o pode ler) e a ligação só quando a
/// unidade é legível para quem vê (`units` = as unidades que o actor lê).
#[must_use]
pub fn member_units(
    person: &Value,
    access: Option<&Value>,
    readable: &[Value],
) -> Vec<OrgUnitRefVm> {
    let by_code: HashMap<&str, &str> = readable
        .iter()
        .map(|u| (text(u, "code"), text(u, "id")))
        .collect();
    let roles: HashMap<String, OrgUnitRole> = access
        .and_then(|a| a.get("units"))
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|r| Some((text(r, "id").to_owned(), unit_role(text(r, "role"))?)))
                .collect()
        })
        .unwrap_or_default();
    person
        .get("units")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|u| {
                    let code = text(u, "code");
                    let id = by_code.get(code).copied();
                    OrgUnitRefVm {
                        code: code.to_owned(),
                        name: text(u, "name").to_owned(),
                        role: id.and_then(|i| roles.get(i).copied()),
                        href: id.map(|i| format!("/units/{i}")),
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Os ambientes de um membro (`/access.workspaces`: id e papel). O título só
/// quando o ambiente é legível **para quem vê** (a lista dos ambientes que o
/// actor alcança); senão, o ambiente existe e não se diz qual (M-05).
#[must_use]
pub fn member_workspaces(access: &Value, visible: &[Value]) -> Vec<OrgWorkspaceRefVm> {
    let by_id: HashMap<&str, &Value> = visible.iter().map(|w| (text(w, "id"), w)).collect();
    access
        .get("workspaces")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|r| {
                    let role = workspace_role(text(r, "role"))?;
                    let w = by_id.get(text(r, "id")).copied();
                    Some(OrgWorkspaceRefVm {
                        title: w.map(|w| text(w, "title").to_owned()),
                        role,
                        href: w.and_then(workspace_href),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// A ligação canónica de um ambiente: o projecto, ou a ideia (D005).
fn workspace_href(w: &Value) -> Option<String> {
    let s = w.get("summary")?;
    let project = text(s, "project_id");
    if !project.is_empty() {
        return Some(format!("/projects/{project}"));
    }
    let idea = text(s, "idea_id");
    (!idea.is_empty()).then(|| format!("/ideas/{idea}"))
}

/// O âmbito de um grant, traduzido e resolvido para quem vê.
fn grant_scope(g: &Value, readable_units: &[Value]) -> String {
    let scope = text(g, "scope");
    let label = t(&format!("prod.org.scope.{scope}"));
    let id = text(g, "scope_id");
    let resolved = match scope {
        "unit" => readable_units
            .iter()
            .find(|u| text(u, "id") == id)
            .map(|u| text(u, "code").to_owned()),
        _ => None,
    };
    resolved.map_or_else(|| label.to_owned(), |r| format!("{label} · {r}"))
}

/// O acesso de uma pessoa (`GET …/access`), com as acções que o actor tem.
#[must_use]
pub(crate) fn access(
    a: &Value,
    base: &str,
    is_self: bool,
    readable_units: &[Value],
    clock: &Clock,
) -> OrgAccessVm {
    let may_roles = a.get("may_manage_roles").and_then(Value::as_bool) == Some(true);
    let may_grants = a.get("may_manage_grants").and_then(Value::as_bool) == Some(true);
    let held: Vec<OrgTechRole> = a
        .get("roles")
        .and_then(Value::as_array)
        .map(|r| r.iter().filter_map(|r| tech_role(r.as_str()?)).collect())
        .unwrap_or_default();
    let roles = held
        .iter()
        .map(|r| OrgRoleHeldVm {
            role: *r,
            // Nunca o próprio papel de administração da plataforma: o Core
            // recusa o último, e a vista não oferece o auto-bloqueio.
            revoke: (may_roles && !(is_self && *r == OrgTechRole::PlatformAdmin))
                .then(|| act(base, OrgActionKind::RevokeRole, &[("role", r.as_str())])),
        })
        .collect();
    let grant = may_roles.then(|| OrgRoleGrantVm {
        action: base.to_owned(),
        options: TECH_ROLES
            .into_iter()
            .filter(|r| !held.contains(r))
            .map(|r| ResOptionVm {
                value: r.as_str().to_owned(),
                label: t(r.key()).to_owned(),
                selected: false,
            })
            .collect(),
    });
    let now = clock.now;
    let grants = a
        .get("grants")
        .and_then(Value::as_array)
        .map(|g| {
            g.iter()
                .filter(|g| g.get("revoked_at").is_none_or(Value::is_null))
                .filter(|g| instant(g, "expires_at").is_none_or(|e| e > now))
                .map(|g| OrgGrantVm {
                    permission: text(g, "permission").to_owned(),
                    scope: grant_scope(g, readable_units),
                    reason: text(g, "reason").to_owned(),
                    granted_by: rs::opt(g, "granted_by_name"),
                    expires: instant(g, "expires_at").map(|e| rs::day(e, clock)),
                    revoke: may_grants.then(|| {
                        act(
                            base,
                            OrgActionKind::RevokeGrant,
                            &[("grant", text(g, "id"))],
                        )
                    }),
                })
                .collect()
        })
        .unwrap_or_default();
    let permissions = a
        .get("institution_permissions")
        .and_then(Value::as_array)
        .map(|p| {
            p.iter()
                .filter_map(|p| {
                    Some(OrgPermissionVm {
                        permission: text(p, "permission").to_owned(),
                        source: source(text(p, "source"))?,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    OrgAccessVm {
        roles,
        grants,
        permissions,
        grant: grant.filter(|g| !g.options.is_empty()),
    }
}

/// Um resumo do agente, sem o texto cru: «Firefox · macOS».
#[must_use]
pub fn agent(ua: &str) -> Option<String> {
    let browser = [
        ("Edg/", "Edge"),
        ("Firefox/", "Firefox"),
        ("Chrome/", "Chrome"),
        ("Safari/", "Safari"),
    ]
    .into_iter()
    .find(|(k, _)| ua.contains(k))
    .map(|(_, n)| n);
    let os = [
        ("Windows", "Windows"),
        ("Mac OS X", "macOS"),
        ("Android", "Android"),
        ("iPhone", "iOS"),
        ("iPad", "iPadOS"),
        ("Linux", "Linux"),
    ]
    .into_iter()
    .find(|(k, _)| ua.contains(k))
    .map(|(_, n)| n);
    match (browser, os) {
        (Some(b), Some(o)) => Some(format!("{b} · {o}")),
        (Some(x), None) | (None, Some(x)) => Some(x.to_owned()),
        (None, None) => None,
    }
}

/// A segurança da conta (`GET …/security`): metadata, nunca a credencial nem
/// o identificador de sessão no ecrã (só no endereço da revogação).
#[must_use]
pub(crate) fn security(s: &Value, base: &str, is_self: bool, clock: &Clock) -> OrgSecurityVm {
    let manage = AccountFlags::of(s).manage;
    OrgSecurityVm {
        has_permanent_password: s.get("has_permanent_password").and_then(Value::as_bool)
            == Some(true),
        password_changed: instant(s, "password_changed_at").map(|at| rs::day(at, clock)),
        temporary: instant(s, "temporary_credential_expires_at").map(|at| OrgTempCredVm {
            expires: format!("{} {}", rs::day(at, clock), clock.hhmm(at)),
            expired: s
                .get("temporary_credential_expired")
                .and_then(Value::as_bool)
                == Some(true),
        }),
        last_sign_in: instant(s, "last_successful_sign_in").map(|at| clock.when(at)),
        recent_failures: s
            .get("recent_failed_attempts")
            .and_then(Value::as_u64)
            .and_then(|n| u32::try_from(n).ok())
            .unwrap_or(0),
        mfa_required: s.get("mfa_required").and_then(Value::as_bool) == Some(true),
        mfa_enrolled: s.get("mfa_enrolled").and_then(Value::as_bool) == Some(true),
        sessions: s
            .get("live_sessions")
            .and_then(Value::as_array)
            .map(|l| {
                l.iter()
                    .map(|x| OrgSessionVm {
                        issued: instant(x, "issued_at")
                            .map(|at| clock.when(at))
                            .unwrap_or_default(),
                        last_seen: instant(x, "last_seen_at")
                            .map(|at| clock.when(at))
                            .unwrap_or_default(),
                        expires: instant(x, "expires_at")
                            .map(|at| clock.when(at))
                            .unwrap_or_default(),
                        agent: agent(text(x, "user_agent")),
                        ip_prefix: rs::opt(x, "ip_prefix"),
                        restricted: text(x, "state") != "active",
                        // A sessão de quem vê não se revoga daqui: é a que
                        // está a usar (a saída está no menu da conta).
                        revoke: (manage && !is_self).then(|| {
                            act(
                                base,
                                OrgActionKind::RevokeSession,
                                &[("session", text(x, "id"))],
                            )
                        }),
                    })
                    .collect()
            })
            .unwrap_or_default(),
    }
}

/// O catálogo de papéis (`GET /administration/roles`), só leitura: as
/// permissões são as que o Core diz, nunca as da fixture de referência.
#[must_use]
pub fn role_catalogue(v: &Value) -> Vec<OrgRoleDefVm> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|r| {
                    Some(OrgRoleDefVm {
                        role: tech_role(text(r, "role"))?,
                        permissions: r
                            .get("permissions")
                            .and_then(Value::as_array)
                            .map(|p| {
                                p.iter()
                                    .filter_map(Value::as_str)
                                    .map(str::to_owned)
                                    .collect()
                            })
                            .unwrap_or_default(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

// ── A confirmação partilhada ─────────────────────────────────────────────

/// O que a confirmação de uma acção de conta precisa de saber do alvo.
pub struct MemberTarget<'a> {
    /// `/admin/members/{id}`.
    pub base: &'a str,
    /// O nome.
    pub name: &'a str,
    /// As acções que o Core deixa agora (de [`account_actions`]).
    pub offered: &'a [OrgActionVm],
    /// O acesso, quando o actor o lê.
    pub access: Option<&'a OrgAccessVm>,
    /// A segurança, quando o actor a lê.
    pub security: Option<&'a OrgSecurityVm>,
}

/// A confirmação pedida por `?confirm=`, **só** se a acção está oferecida
/// agora. Um `?confirm=` escrito à mão para uma acção que o Core não deu não
/// abre nada. O alvo e o destino vão fixos no formulário (caminho e campos
/// escondidos): o que se confirma é o que se viu.
#[must_use]
pub fn member_confirm(
    m: &MemberTarget<'_>,
    kind: OrgActionKind,
    param: Option<&str>,
) -> Option<OrgConfirmVm> {
    let base = m.base;
    let offered = |k: OrgActionKind| m.offered.iter().any(|a| a.kind == k);
    let (action, hidden, reason, context) = match kind {
        OrgActionKind::Suspend | OrgActionKind::Disable | OrgActionKind::Reactivate => {
            if !offered(kind) {
                return None;
            }
            let to = match kind {
                OrgActionKind::Suspend => "suspended",
                OrgActionKind::Disable => "disabled",
                _ => "active",
            };
            (
                format!("{base}/status"),
                vec![("status", to.to_owned())],
                OrgReason::Required(4),
                None,
            )
        }
        OrgActionKind::ResetPassword => {
            offered(kind).then_some(())?;
            (
                format!("{base}/reset-password"),
                vec![],
                OrgReason::None,
                None,
            )
        }
        OrgActionKind::Provision | OrgActionKind::Reissue => {
            offered(kind).then_some(())?;
            (format!("{base}/provision"), vec![], OrgReason::None, None)
        }
        OrgActionKind::DeleteInvite => {
            offered(kind).then_some(())?;
            (format!("{base}/delete"), vec![], OrgReason::None, None)
        }
        OrgActionKind::GrantRole => {
            let role = tech_role(param?)?;
            let a = m.access?;
            let ok = a
                .grant
                .as_ref()
                .is_some_and(|g| g.options.iter().any(|o| o.value == role.as_str()));
            ok.then_some(())?;
            (
                format!("{base}/roles"),
                vec![("role", role.as_str().to_owned())],
                // O Core exige a razão para conceder um papel técnico.
                OrgReason::Required(1),
                Some(t(role.key()).to_owned()),
            )
        }
        OrgActionKind::RevokeRole => {
            let role = tech_role(param?)?;
            let held = m.access?.roles.iter().find(|r| r.role == role)?;
            held.revoke.as_ref()?;
            (
                format!("{base}/roles/{}/revoke", role.as_str()),
                vec![],
                OrgReason::None,
                Some(t(role.key()).to_owned()),
            )
        }
        OrgActionKind::RevokeGrant => {
            let id = param?;
            let g = m.access?.grants.iter().find(|g| {
                g.revoke
                    .as_ref()
                    .is_some_and(|a| a.href.ends_with(&format!("grant={}", rs::encode(id))))
            })?;
            (
                format!("{base}/grants/{}/revoke", rs::encode(id)),
                vec![],
                OrgReason::Optional,
                Some(format!("{} · {}", g.permission, g.scope)),
            )
        }
        OrgActionKind::RevokeSession => {
            let id = param?;
            let s = m.security?.sessions.iter().find(|s| {
                s.revoke
                    .as_ref()
                    .is_some_and(|a| a.href.ends_with(&format!("session={}", rs::encode(id))))
            })?;
            (
                format!("{base}/sessions/{}/revoke", rs::encode(id)),
                vec![],
                OrgReason::None,
                s.agent.clone(),
            )
        }
        OrgActionKind::ChangeUnitRole
        | OrgActionKind::RemoveUnitMember
        | OrgActionKind::ArchiveUnit
        | OrgActionKind::LeaveConversation
        | OrgActionKind::RemoveParticipant
        | OrgActionKind::StopService => return None,
    };
    Some(OrgConfirmVm {
        kind,
        target: m.name.to_owned(),
        context,
        change: None,
        action,
        hidden,
        reason,
        cancel_href: base.to_owned(),
        refusal: None,
        error: None,
    })
}

// ── Unidades ─────────────────────────────────────────────────────────────

/// As linhas da lista de unidades: título = nome; código e estado no título;
/// «Áreas» é a coluna que cede primeiro.
#[must_use]
pub fn unit_items(list: &[Value], open: Option<&str>, archived: bool) -> Vec<ResItemVm> {
    list.iter()
        .filter(|u| (text(u, "status") == "archived") == archived)
        .filter_map(|u| {
            let id = text(u, "id");
            let st = unit_status(text(u, "status"))?;
            (!id.is_empty()).then(|| ResItemVm {
                title: text(u, "name").to_owned(),
                code: rs::opt(u, "code"),
                state: Some(unit_state(st)),
                cells: vec![{
                    let areas = areas(u);
                    (!areas.is_empty()).then(|| areas.join(" · "))
                }],
                overdue: false,
                priority: None,
                href: if archived {
                    format!("/units/{id}?nav=archived")
                } else {
                    format!("/units/{id}")
                },
                active: open == Some(id),
            })
        })
        .collect()
}

/// O estado de uma unidade na lista.
#[must_use]
pub const fn unit_state(s: OrgUnitStatus) -> crate::ui::view_models::ResStateVm {
    match s {
        OrgUnitStatus::Active => rs::state("units.state.active", ResTone::Done),
        OrgUnitStatus::Archived => rs::state("units.state.archived", ResTone::Closed),
    }
}

/// As áreas de investigação declaradas.
#[must_use]
pub fn areas(u: &Value) -> Vec<String> {
    u.get("research_areas")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// Os membros de uma unidade (`GET /units/{id}/members`). Mudar o papel e
/// retirar só com `may_manage_members`; a ligação ao membro na Administração
/// só para quem tem `members.manage`.
#[must_use]
pub fn unit_members(
    list: &[Value],
    unit_base: &str,
    may_manage: bool,
    admin_links: bool,
) -> Vec<UnitMemberVm> {
    list.iter()
        .filter_map(|m| {
            let id = text(m, "person_id");
            let role = unit_role(text(m, "role"))?;
            let name = text(m, "full_name").to_owned();
            Some(UnitMemberVm {
                avatar: avatar(&name),
                name,
                role,
                href: admin_links.then(|| format!("/admin/members/{id}")),
                change_role: may_manage
                    .then(|| act(unit_base, OrgActionKind::ChangeUnitRole, &[("person", id)])),
                remove: may_manage.then(|| {
                    act(
                        unit_base,
                        OrgActionKind::RemoveUnitMember,
                        &[("person", id)],
                    )
                }),
            })
        })
        .collect()
}

/// A confirmação de uma acção sobre uma unidade, só se estiver oferecida.
#[must_use]
pub fn unit_confirm(
    unit_base: &str,
    unit_label: &str,
    members: &[UnitMemberVm],
    archive: Option<&OrgActionVm>,
    kind: OrgActionKind,
    person: Option<&str>,
) -> Option<OrgConfirmVm> {
    let by_href = |a: &Option<OrgActionVm>, id: &str| {
        a.as_ref()
            .is_some_and(|a| a.href.ends_with(&format!("person={}", rs::encode(id))))
    };
    let (target, context, change, action, hidden) = match kind {
        OrgActionKind::ChangeUnitRole => {
            let id = person?;
            let m = members.iter().find(|m| by_href(&m.change_role, id))?;
            let to = match m.role {
                OrgUnitRole::Manager => OrgUnitRole::Member,
                OrgUnitRole::Member => OrgUnitRole::Manager,
            };
            (
                m.name.clone(),
                Some(unit_label.to_owned()),
                Some((t(m.role.key()).to_owned(), t(to.key()).to_owned())),
                format!("{unit_base}/members/role"),
                vec![
                    ("person_id", id.to_owned()),
                    ("role", to.as_str().to_owned()),
                ],
            )
        }
        OrgActionKind::RemoveUnitMember => {
            let id = person?;
            let m = members.iter().find(|m| by_href(&m.remove, id))?;
            (
                m.name.clone(),
                Some(unit_label.to_owned()),
                None,
                format!("{unit_base}/members/remove"),
                vec![("person_id", id.to_owned())],
            )
        }
        OrgActionKind::ArchiveUnit => {
            archive?;
            (
                unit_label.to_owned(),
                None,
                None,
                format!("{unit_base}/archive"),
                vec![],
            )
        }
        _ => return None,
    };
    Some(OrgConfirmVm {
        kind,
        target,
        context,
        change,
        action,
        hidden,
        reason: OrgReason::None,
        cancel_href: unit_base.to_owned(),
        refusal: None,
        error: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn relogio() -> Clock {
        Clock {
            now: chrono::Utc::now(),
            zone: ocinye_contracts::temporal::TimeZoneName::parse("UTC").unwrap(),
            core_ok: true,
            is_admin: true,
        }
    }

    const FULL: AccountFlags = AccountFlags {
        manage: true,
        provision: false,
        temporary_expired: false,
        delete: false,
    };

    fn kinds(v: &[OrgActionVm]) -> Vec<OrgActionKind> {
        v.iter().map(|a| a.kind).collect()
    }

    #[test]
    fn os_quatro_estados_nao_se_juntam() {
        let s: Vec<_> = ["invited", "active", "suspended", "disabled"]
            .iter()
            .map(|v| status(v).unwrap())
            .collect();
        assert_eq!(s.len(), 4);
        assert_ne!(status("suspended"), status("disabled"));
        assert_eq!(status("deleted"), None);
    }

    #[test]
    fn as_accoes_de_conta_vem_do_sinal_do_core_e_do_ciclo_de_vida() {
        let b = "/admin/members/p";
        use OrgAccountStatus as S;
        use OrgActionKind as K;
        assert_eq!(
            kinds(&account_actions(b, S::Active, FULL, false)),
            [K::Suspend, K::Disable, K::ResetPassword]
        );
        assert_eq!(
            kinds(&account_actions(b, S::Suspended, FULL, false)),
            [K::Reactivate, K::Disable]
        );
        assert!(account_actions(b, S::Disabled, FULL, false).is_empty());
        let convite = AccountFlags {
            provision: true,
            delete: true,
            ..FULL
        };
        assert_eq!(
            kinds(&account_actions(b, S::Invited, convite, false)),
            [K::Provision, K::DeleteInvite]
        );
        let expirada = AccountFlags {
            temporary_expired: true,
            ..convite
        };
        assert_eq!(
            kinds(&account_actions(b, S::Invited, expirada, false))[0],
            K::Reissue
        );
        // Sem autoridade, ou sobre a própria conta: nada.
        assert!(account_actions(b, S::Active, AccountFlags::default(), false).is_empty());
        assert!(account_actions(b, S::Active, FULL, true).is_empty());
    }

    #[test]
    fn um_confirm_escrito_a_mao_nao_abre_o_que_o_core_nao_deu() {
        let b = "/admin/members/p";
        let offered = account_actions(b, OrgAccountStatus::Suspended, FULL, false);
        let m = MemberTarget {
            base: b,
            name: "Rui",
            offered: &offered,
            access: None,
            security: None,
        };
        assert!(member_confirm(&m, OrgActionKind::Suspend, None).is_none());
        let c = member_confirm(&m, OrgActionKind::Reactivate, None).unwrap();
        assert_eq!(c.action, "/admin/members/p/status");
        assert_eq!(c.hidden, vec![("status", "active".to_owned())]);
        assert_eq!(c.reason, OrgReason::Required(4));
        assert_eq!(c.cancel_href, b);
        // Papel sem acesso lido: não há confirmação de papel.
        assert!(member_confirm(&m, OrgActionKind::GrantRole, Some("auditor")).is_none());
    }

    #[test]
    fn o_proprio_nao_revoga_o_seu_papel_de_plataforma_nem_a_sua_sessao() {
        let a = json!({
            "roles": ["platform_admin", "auditor"],
            "may_manage_roles": true, "may_manage_grants": true,
            "grants": [], "institution_permissions": [], "units": [], "workspaces": []
        });
        let v = access(&a, "/admin/members/me", true, &[], &relogio());
        assert!(v.roles[0].revoke.is_none() && v.roles[1].revoke.is_some());
        // Concedíveis: os papéis que não detém, sem inventar outros.
        let opts: Vec<_> = v
            .grant
            .unwrap()
            .options
            .into_iter()
            .map(|o| o.value)
            .collect();
        assert_eq!(opts.len(), 6);
        assert!(!opts.contains(&"platform_admin".to_owned()));
        let s = json!({
            "may_manage_account": true,
            "live_sessions": [{ "id": "s1", "state": "active", "user_agent": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15) Firefox/130.0" }]
        });
        let sec = security(&s, "/admin/members/me", true, &relogio());
        assert!(sec.sessions[0].revoke.is_none());
        assert_eq!(sec.sessions[0].agent.as_deref(), Some("Firefox · macOS"));
    }

    #[test]
    fn so_grants_vivos_e_o_ambiente_escondido_nao_diz_o_titulo() {
        let a = json!({
            "roles": [], "may_manage_roles": false, "may_manage_grants": true,
            "grants": [
                { "id": "g1", "permission": "documents.view", "scope": "institution", "reason": "r", "granted_by_name": "A" },
                { "id": "g2", "permission": "documents.view", "scope": "institution", "reason": "r", "revoked_at": "2026-01-01T00:00:00Z" }
            ],
            "workspaces": [{ "id": "w-visivel", "role": "lead" }, { "id": "w-oculto", "role": "viewer" }]
        });
        let v = access(&a, "/admin/members/p", false, &[], &relogio());
        assert_eq!(v.grants.len(), 1);
        assert!(v.grant.is_none(), "sem may_manage_roles não se concede");
        let visible =
            vec![json!({ "id": "w-visivel", "title": "Vento", "summary": { "project_id": "p1" } })];
        let w = member_workspaces(&a, &visible);
        assert_eq!(w[0].title.as_deref(), Some("Vento"));
        assert_eq!(w[0].href.as_deref(), Some("/projects/p1"));
        assert_eq!(w[1].title, None);
        assert_eq!(w[1].href, None);
    }

    #[test]
    fn a_unidade_so_oferece_accoes_de_membro_com_o_sinal_do_core() {
        let list = vec![
            json!({ "person_id": "a", "full_name": "Ana Silva", "role": "manager" }),
            json!({ "person_id": "b", "full_name": "Rui", "role": "member" }),
        ];
        let ro = unit_members(&list, "/units/u", false, false);
        assert!(ro
            .iter()
            .all(|m| m.change_role.is_none() && m.remove.is_none() && m.href.is_none()));
        let rw = unit_members(&list, "/units/u", true, true);
        let c = unit_confirm(
            "/units/u",
            "U-1 · Unidade",
            &rw,
            None,
            OrgActionKind::ChangeUnitRole,
            Some("a"),
        )
        .unwrap();
        assert_eq!(c.action, "/units/u/members/role");
        assert_eq!(
            c.hidden,
            vec![("person_id", "a".to_owned()), ("role", "member".to_owned())]
        );
        assert!(c.change.is_some());
        // Um membro que não está na lista, ou uma unidade sem arquivar oferecido:
        // nada abre.
        assert!(unit_confirm(
            "/units/u",
            "U",
            &rw,
            None,
            OrgActionKind::RemoveUnitMember,
            Some("z")
        )
        .is_none());
        assert!(
            unit_confirm("/units/u", "U", &rw, None, OrgActionKind::ArchiveUnit, None).is_none()
        );
        assert!(unit_confirm(
            "/units/u",
            "U",
            &ro,
            None,
            OrgActionKind::ChangeUnitRole,
            Some("a")
        )
        .is_none());
        assert_eq!(rw[0].avatar.initials, "AS");
    }

    #[test]
    fn as_recusas_e_os_avisos_sao_vocabularios_fechados() {
        for r in [
            OrgRefusal::SelfLockout,
            OrgRefusal::LastPlatformAdmin,
            OrgRefusal::LastUnitManager,
            OrgRefusal::StaleState,
        ] {
            assert_eq!(refusal(refusal_code(r)), Some(r));
        }
        assert_eq!(refusal("<script>"), None);
        assert_eq!(notice("status"), Some(OrgNotice::StatusChanged));
        assert_eq!(notice("x"), None);
        let f = ApiFailure::Refused {
            reason: "last_unit_manager".into(),
            message: String::new(),
        };
        assert_eq!(refusal_of(&f), Some(OrgRefusal::LastUnitManager));
        assert_eq!(refusal_of(&ApiFailure::Forbidden), None);
    }

    #[test]
    fn o_catalogo_de_papeis_e_o_do_core() {
        let v = json!([
            { "role": "auditor", "permissions": ["audit.read"], "system": true },
            { "role": "inventado", "permissions": ["x"], "system": true }
        ]);
        let c = role_catalogue(&v);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].permissions, vec!["audit.read".to_owned()]);
    }
}
