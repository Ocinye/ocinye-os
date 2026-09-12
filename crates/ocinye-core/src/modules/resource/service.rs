//! The resource governance service — authorization, resolution, invariants.
//!
//! # The line this module holds
//!
//! Entitlement is not access. Reading or granting a resource allocation here
//! never opens the data a scope holds, and never adds a member to a unit,
//! workspace or project. Access is decided by [`ocinye_domain::can`] elsewhere;
//! this module answers only *how much*.

use ocinye_contracts::{AllocationSource, Permission, ResourceScopeType, ResourceType};
use ocinye_domain::{can, Principal, ResourceContext, ResourceKind};
use sqlx::PgPool;
use uuid::Uuid;

use super::model::{Entitlement, EntitlementPart, ResourceProfileDetail};
use super::repository as repo;
use crate::error::{CoreError, CoreResult};

/// Authorise a resource permission, or fail closed.
fn require(principal: &Principal, permission: Permission) -> CoreResult<()> {
    let ctx = ResourceContext::organisation(ResourceKind::Person, principal.organisation_id);
    if can(principal, permission, &ctx, None).allowed {
        Ok(())
    } else {
        Err(CoreError::PermissionDenied(
            "Não possui acesso à governança de recursos.".to_owned(),
        ))
    }
}

/// Every allocation profile of the institution, with its rules.
///
/// # Errors
///
/// [`CoreError::PermissionDenied`] without `resources.view`.
pub async fn list_profiles(
    pool: &PgPool,
    principal: &Principal,
) -> CoreResult<Vec<ResourceProfileDetail>> {
    require(principal, Permission::ResourcesView)?;

    let profiles = repo::list_profiles(pool, principal.organisation_id).await?;
    let mut out = Vec::with_capacity(profiles.len());
    for profile in profiles {
        let rules = repo::profile_rules(pool, profile.id).await?;
        out.push(ResourceProfileDetail { profile, rules });
    }
    Ok(out)
}

/// Resolve the effective entitlement of a scope for one resource.
///
/// The standing base is the scope's active non-temporary allocation — an
/// explicit override if there is one, otherwise the profile-derived allocation.
/// When a member has no allocation yet, the base falls back to the institution's
/// default profile rule, so a member resolves an entitlement from day one even
/// before the lifecycle materialises their allocation. Live temporary grants add
/// on top. Each contribution is recorded so the number can be explained (§54).
///
/// This is an internal resolver: callers authorise the scope before charging or
/// displaying it. It grants nothing.
///
/// # Errors
///
/// Returns an error when a query fails.
pub async fn resolve_entitlement(
    pool: &PgPool,
    organisation_id: Uuid,
    scope_type: ResourceScopeType,
    scope_id: Uuid,
    resource_type: ResourceType,
) -> CoreResult<Entitlement> {
    let now = chrono::Utc::now();
    let allocations =
        repo::active_allocations(pool, scope_type, scope_id, resource_type, now).await?;

    let mut parts: Vec<EntitlementPart> = Vec::new();
    let unit = resource_type.canonical_unit();

    // The standing base: an override wins over a profile-derived allocation.
    let base = allocations
        .iter()
        .filter(|a| matches!(a.source, AllocationSource::Override))
        .max_by_key(|a| a.starts_at)
        .or_else(|| {
            allocations
                .iter()
                .filter(|a| matches!(a.source, AllocationSource::Profile))
                .max_by_key(|a| a.starts_at)
        });

    let mut quantity: i64 = 0;

    match base {
        Some(base) => {
            quantity += base.quantity;
            parts.push(EntitlementPart {
                source: base.source,
                quantity: base.quantity,
                expires_at: base.expires_at,
                note: base.reason.clone(),
            });
        }
        None => {
            // No allocation yet: fall back to the institution's default profile.
            // A scope that is not a member gets no implicit base (0).
            if scope_type == ResourceScopeType::Member {
                if let Some(profile) = repo::default_profile(pool, organisation_id).await? {
                    if let Some(rule) = repo::profile_rule(pool, profile.id, resource_type).await? {
                        quantity += rule.quantity;
                        parts.push(EntitlementPart {
                            source: AllocationSource::Profile,
                            quantity: rule.quantity,
                            expires_at: None,
                            note: format!("Perfil {}", profile.code),
                        });
                    }
                }
            }
        }
    }

    // Live temporary grants add to the base.
    for grant in allocations
        .iter()
        .filter(|a| matches!(a.source, AllocationSource::Temporary))
    {
        quantity += grant.quantity;
        parts.push(EntitlementPart {
            source: AllocationSource::Temporary,
            quantity: grant.quantity,
            expires_at: grant.expires_at,
            note: grant.reason.clone(),
        });
    }

    Ok(Entitlement {
        resource_type,
        unit,
        quantity,
        parts,
    })
}

/// A member's own effective entitlement for a resource.
///
/// A member always sees their own resources; seeing another member's requires
/// `resources.view`. The authority is checked here, not by the caller.
///
/// # Errors
///
/// [`CoreError::PermissionDenied`] when reading another member's entitlement
/// without `resources.view`.
pub async fn member_entitlement(
    pool: &PgPool,
    principal: &Principal,
    person_id: Uuid,
    resource_type: ResourceType,
) -> CoreResult<Entitlement> {
    if person_id != principal.person_id {
        require(principal, Permission::ResourcesView)?;
    }
    resolve_entitlement(
        pool,
        principal.organisation_id,
        ResourceScopeType::Member,
        person_id,
        resource_type,
    )
    .await
}
