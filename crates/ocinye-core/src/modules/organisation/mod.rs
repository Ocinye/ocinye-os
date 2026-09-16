//! Organisation and scientific units.
//!
//! # What belongs here
//!
//! The institution itself, its scientific units, and who belongs to each unit
//! in what role. Units are rows, never hardcoded: creating one needs no code
//! change (briefing §23).
//!
//! # What does not belong here
//!
//! Research content. A unit owns workspaces; it does not own ideas, sources or
//! datasets directly — those belong to a research workspace, which supplies the
//! authorization context.

mod model;
mod repository;
mod service;

pub use model::{Organisation, Unit, UnitMember, UnitStatus};
pub use service::{
    add_unit_member, archive_unit, bootstrap_organisation, create_unit, get_organisation, get_unit,
    list_unit_members, list_units, revoke_unit_member, seed_initial_units, suggest_unit_code,
    unit_context, units_for_people, update_unit, NewUnit, PersonUnit, UnitEdit,
};
