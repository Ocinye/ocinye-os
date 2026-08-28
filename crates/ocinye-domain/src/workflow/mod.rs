//! Institutional lifecycles.
//!
//! Transitions are explicit, closed and enforced server-side. Adding a state
//! means editing a transition table here and writing a migration — never
//! scattering conditionals across services (briefing §23, §24).
//!
//! Each lifecycle is a total function over its states, so a state that nobody
//! wrote a rule for is unreachable rather than accidentally permitted.

pub mod idea;
pub mod project;
pub mod task;

pub use idea::{assert_idea_transition, idea_targets_from, PROMOTABLE_FROM};
pub use project::{assert_project_transition, project_targets_from};
pub use task::assert_task_transition;
