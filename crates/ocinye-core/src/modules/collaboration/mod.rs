//! Collaboration: tasks, comments and the activity feed.
//!
//! # What belongs here
//!
//! The human coordination surface of a research workspace.
//!
//! # Activity is not audit
//!
//! Audit exists for security and evidence: append-only, access-restricted, and
//! written for a reviewer. Activity exists for collaboration: it carries only
//! what a colleague may already see, and it is shaped for reading (briefing
//! §45). Never render the audit trail as a social feed.

pub mod activity;
mod model;
mod repository;
mod service;

pub use activity::{record_activity, ActivityKind};
pub use model::{ActivityEntry, Comment, Task, TaskPriority};
pub use service::{
    add_comment, assign_task, create_task, get_task, list_activity, list_comments, list_tasks,
    transition_task, NewTask,
};
