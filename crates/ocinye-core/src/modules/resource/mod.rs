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

pub mod model;
pub mod repository;
pub mod service;

pub use model::{
    Allocation, Entitlement, EntitlementPart, ProfileRule, ResourceProfile, ResourceProfileDetail,
};
pub use service::{list_profiles, member_entitlement, resolve_entitlement};
