//! Research lifecycle states.

use serde::{Deserialize, Serialize};

/// Whether a research workspace currently hosts an idea or a formal project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceKind {
    /// Exploratory: an idea is being developed.
    Idea,
    /// Formal: the workspace now carries a project.
    Project,
}

impl WorkspaceKind {
    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Idea => "idea",
            Self::Project => "project",
        }
    }

    /// Parse from the stable representation.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "idea" => Self::Idea,
            "project" => Self::Project,
            _ => return None,
        })
    }
}

/// Lifecycle of an exploratory idea.
///
/// An idea is not a project. Reaching a terminal state without becoming one is
/// a legitimate, recorded outcome (briefing §23, §24).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdeaState {
    /// Initial framing of the question or problem.
    Discovery,
    /// Actively explored: sources, notes, early data.
    Exploration,
    /// A concept has taken shape.
    Concept,
    /// Under institutional review.
    Review,
    /// Reviewed and eligible for promotion to a project.
    ProjectCandidate,
    /// Promoted: a project now exists. Reached only through promotion.
    Promoted,
    /// Reviewed and not taken forward. The reason is recorded.
    Rejected,
    /// Set aside without rejection. May be reopened.
    Archived,
}

impl IdeaState {
    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Discovery => "discovery",
            Self::Exploration => "exploration",
            Self::Concept => "concept",
            Self::Review => "review",
            Self::ProjectCandidate => "project_candidate",
            Self::Promoted => "promoted",
            Self::Rejected => "rejected",
            Self::Archived => "archived",
        }
    }

    /// Parse from the stable representation.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "discovery" => Self::Discovery,
            "exploration" => Self::Exploration,
            "concept" => Self::Concept,
            "review" => Self::Review,
            "project_candidate" => Self::ProjectCandidate,
            "promoted" => Self::Promoted,
            "rejected" => Self::Rejected,
            "archived" => Self::Archived,
            _ => return None,
        })
    }

    /// Whether the state closes the idea without a project.
    #[must_use]
    pub const fn is_closed_without_project(self) -> bool {
        matches!(self, Self::Rejected | Self::Archived)
    }

    /// Every idea state.
    #[must_use]
    pub const fn all() -> [Self; 8] {
        [
            Self::Discovery,
            Self::Exploration,
            Self::Concept,
            Self::Review,
            Self::ProjectCandidate,
            Self::Promoted,
            Self::Rejected,
            Self::Archived,
        ]
    }
}

/// Lifecycle of a formal project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectState {
    /// Being prepared; not yet running.
    Draft,
    /// Running.
    Active,
    /// Temporarily suspended.
    OnHold,
    /// Finished.
    Completed,
    /// Closed and retained for the record.
    Archived,
}

impl ProjectState {
    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Active => "active",
            Self::OnHold => "on_hold",
            Self::Completed => "completed",
            Self::Archived => "archived",
        }
    }

    /// Parse from the stable representation.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "draft" => Self::Draft,
            "active" => Self::Active,
            "on_hold" => Self::OnHold,
            "completed" => Self::Completed,
            "archived" => Self::Archived,
            _ => return None,
        })
    }

    /// Every project state.
    #[must_use]
    pub const fn all() -> [Self; 5] {
        [
            Self::Draft,
            Self::Active,
            Self::OnHold,
            Self::Completed,
            Self::Archived,
        ]
    }
}

/// Lifecycle of a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    /// Not started.
    Todo,
    /// Being worked on.
    InProgress,
    /// Blocked by something external.
    Blocked,
    /// Awaiting review.
    InReview,
    /// Finished.
    Done,
    /// Abandoned.
    Cancelled,
}

impl TaskState {
    /// Whether the task is closed.
    #[must_use]
    pub const fn is_closed(self) -> bool {
        matches!(self, Self::Done | Self::Cancelled)
    }

    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Todo => "todo",
            Self::InProgress => "in_progress",
            Self::Blocked => "blocked",
            Self::InReview => "in_review",
            Self::Done => "done",
            Self::Cancelled => "cancelled",
        }
    }

    /// Parse from the stable representation.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "todo" => Self::Todo,
            "in_progress" => Self::InProgress,
            "blocked" => Self::Blocked,
            "in_review" => Self::InReview,
            "done" => Self::Done,
            "cancelled" => Self::Cancelled,
            _ => return None,
        })
    }

    /// Every task state.
    #[must_use]
    pub const fn all() -> [Self; 6] {
        [
            Self::Todo,
            Self::InProgress,
            Self::Blocked,
            Self::InReview,
            Self::Done,
            Self::Cancelled,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idea_states_round_trip() {
        for state in IdeaState::all() {
            assert_eq!(IdeaState::parse(state.as_str()), Some(state));
        }
    }

    #[test]
    fn closing_without_a_project_is_representable() {
        assert!(IdeaState::Rejected.is_closed_without_project());
        assert!(IdeaState::Archived.is_closed_without_project());
        assert!(!IdeaState::Promoted.is_closed_without_project());
    }
}
