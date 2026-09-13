//! Ocinye resource governance — the institutional resource control plane.
//!
//! # What belongs here
//!
//! How much institutional capacity a scope may consume: allocation profiles,
//! the effective entitlement derived from them, and (in later slices) usage,
//! reservations, capacity and admission. **Never** what a scope may *access* —
//! that is RBAC, decided in [`ocinye_domain`].
//!
//! # The four concepts, kept apart
//!
//! Capacity (what the institution has), entitlement (what a scope may consume),
//! reservation (capacity committed to an operation) and usage (what was
//! consumed) are distinct, and never one mutable counter (ADR-0108).

pub mod ai;
pub mod model;
pub mod repository;
pub mod service;
pub mod storage;

pub use ai::{admit_ai_access, ai_access_status, ai_access_used, record_ai_access, AiAccessStatus};
pub use model::{
    Allocation, Entitlement, EntitlementPart, ProfileRule, ResourceProfile, ResourceProfileDetail,
};
pub use service::{
    assign_member_profile, ensure_default_profile, list_profiles, member_entitlement,
    resolve_entitlement, DEFAULT_STORAGE_QUOTA_BYTES,
};
pub use storage::{
    admit_personal_bytes, personal_storage_status, personal_usage_bytes, PersonalStorageStatus,
};
