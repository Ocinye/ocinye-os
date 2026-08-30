//! Research: workspaces, ideas and projects.
//!
//! # What belongs here
//!
//! The Research Workspace — the contextual container that holds everything an
//! idea or project accumulates — together with the two lifecycles that run
//! inside it and the promotion that links them.
//!
//! # What does not belong here
//!
//! The artefacts themselves. Sources, notes, documents, datasets and tasks live
//! in their own modules; they merely borrow this module's workspace as their
//! authorization context.
//!
//! # Why the workspace carries over on promotion
//!
//! Promotion changes the workspace's `kind` rather than creating a second
//! workspace. Everything gathered while exploring stays attached, and the
//! lineage idea → project is recorded on both sides (briefing §24).

mod model;
mod repository;
mod service;

pub use model::{Idea, Project, ResearchWorkspace, WorkspaceMember};
pub use repository::WorkspaceQuery;
pub use service::{
    add_workspace_member, artefact_context, create_idea, get_idea, get_project, get_workspace,
    get_workspace_overview, list_workspaces, promote_idea, readable_artefact_workspace,
    reclassify_workspace, remove_workspace_member, transition_idea, transition_project,
    update_idea, workspace_context, IdeaRevision, NewIdea, Promotion, WorkspaceOverview,
};
