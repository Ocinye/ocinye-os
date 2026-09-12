//! Resource governance rows and derived views.

use chrono::{DateTime, Utc};
use ocinye_contracts::{
    AllocationSource, ProfileStatus, ResourceScopeType, ResourceType, ResourceUnit,
    SchedulingPriority,
};
use uuid::Uuid;

/// An allocation profile: a named, configurable bundle of entitlements.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ResourceProfile {
    /// Identifier.
    pub id: Uuid,
    /// Stable institutional code (unique within the organisation).
    pub code: String,
    /// Human name.
    pub name: String,
    /// What the profile is for.
    pub description: String,
    /// Whether it is the profile a member receives by default.
    pub is_default: bool,
    /// The scheduling priority it confers.
    pub priority: SchedulingPriority,
    /// Whether it may currently be assigned.
    pub status: ProfileStatus,
}

/// One typed entitlement within a profile.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProfileRule {
    /// The resource governed.
    pub resource_type: ResourceType,
    /// The quantity, in the resource's canonical unit.
    pub quantity: i64,
    /// The unit the quantity is expressed in.
    pub unit: ResourceUnit,
}

/// A profile together with its rules.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ResourceProfileDetail {
    /// The profile.
    pub profile: ResourceProfile,
    /// Its typed entitlements.
    pub rules: Vec<ProfileRule>,
}

/// An allocation row: what a scope may consume, and where the authority comes
/// from.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Allocation {
    /// Identifier.
    pub id: Uuid,
    /// The resource governed.
    pub resource_type: ResourceType,
    /// The unit.
    pub unit: ResourceUnit,
    /// The scope kind.
    pub scope_type: ResourceScopeType,
    /// The scope target.
    pub scope_id: Uuid,
    /// The quantity granted.
    pub quantity: i64,
    /// Where the authority comes from.
    pub source: AllocationSource,
    /// When it takes effect.
    pub starts_at: DateTime<Utc>,
    /// When it stops, if temporary.
    pub expires_at: Option<DateTime<Utc>>,
    /// Why it was granted.
    pub reason: String,
}

/// One contribution to an effective entitlement, for explanation (§54).
#[derive(Debug, Clone, serde::Serialize)]
pub struct EntitlementPart {
    /// Whether it came from a profile, an override or a temporary grant.
    pub source: AllocationSource,
    /// The quantity this part contributes.
    pub quantity: i64,
    /// When this part expires, if it does.
    pub expires_at: Option<DateTime<Utc>>,
    /// A human note — the profile code, or the grant reason.
    pub note: String,
}

/// The resolved entitlement of a scope for one resource, with the parts that
/// explain it. A member holds this whether or not the institution currently has
/// the physical capacity to satisfy it — entitlement is not availability.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Entitlement {
    /// The resource.
    pub resource_type: ResourceType,
    /// The unit.
    pub unit: ResourceUnit,
    /// The effective quantity: the standing base plus live temporary grants.
    pub quantity: i64,
    /// The contributing parts, most fundamental first.
    pub parts: Vec<EntitlementPart>,
}
