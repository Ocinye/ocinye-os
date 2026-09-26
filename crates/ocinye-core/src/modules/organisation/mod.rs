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

pub mod applications;

pub use applications::{
    application_of_api_path, application_states, inactive_applications, instance_applications,
    profile_of, require_active,
    set_application_active, set_profile, ApplicationState, InstanceApplications,
};
pub use model::{Organisation, Unit, UnitMember, UnitStatus};
pub use service::{
    add_unit_member, archive_unit, bootstrap_organisation, create_unit, default_instance_name,
    get_organisation, get_unit, instance_name, list_unit_members, list_units, resolve_instance,
    revoke_unit_member, seed_initial_units, suggest_unit_code, unit_context, units_for_people,
    update_unit, NewUnit, PersonUnit, UnitEdit,
};
