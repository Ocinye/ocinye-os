//! Collaboration rows.

use chrono::{DateTime, NaiveDate, Utc};
use ocinye_contracts::{Classification, TaskState};
use serde::Serialize;
use sqlx::FromRow;
use uuid::Uuid;

/// How urgent a task is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskPriority {
    /// Can wait.
    Low,
    /// Ordinary.
    #[default]
    Normal,
    /// Should be picked up soon.
    High,
    /// Blocking something important.
    Critical,
}

impl TaskPriority {
    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }

    /// Parse from the stable representation.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "low" => Self::Low,
            "normal" => Self::Normal,
            "high" => Self::High,
            "critical" => Self::Critical,
            _ => return None,
        })
    }
}

/// A unit of work inside a research workspace.
#[derive(Debug, Clone, FromRow)]
pub struct Task {
    /// Identifier.
    pub id: Uuid,
    /// Organisation.
    pub organisation_id: Uuid,
    /// Owning unit.
    pub unit_id: Uuid,
    /// Owning workspace.
    pub workspace_id: Uuid,
    /// Title.
    pub title: String,
    /// Description.
    pub description: Option<String>,
    /// Lifecycle state.
    pub state: String,
    /// Priority.
    pub priority: String,
    /// Person responsible.
    pub assignee_id: Option<Uuid>,
    /// Due date.
    pub due_on: Option<NaiveDate>,
    /// When it closed.
    pub closed_at: Option<DateTime<Utc>>,
    /// Classification.
    pub classification: String,
    /// Creation time.
    pub created_at: DateTime<Utc>,
    /// Last change.
    pub updated_at: DateTime<Utc>,
}

impl Task {
    /// Parsed state.
    #[must_use]
    pub fn state(&self) -> TaskState {
        TaskState::parse(&self.state).unwrap_or(TaskState::Todo)
    }

    /// Parsed classification, defaulting to the most restrictive.
    #[must_use]
    pub fn classification(&self) -> Classification {
        Classification::parse(&self.classification).unwrap_or(Classification::Restricted)
    }
}

/// A comment on a research object.
#[derive(Debug, Clone, FromRow)]
pub struct Comment {
    /// Identifier.
    pub id: Uuid,
    /// Owning workspace.
    pub workspace_id: Uuid,
    /// Kind of the thing commented on.
    pub subject_type: String,
    /// Identifier of the thing commented on.
    pub subject_id: Uuid,
    /// The comment.
    pub body: String,
    /// Classification.
    pub classification: String,
    /// When it was withdrawn, if it was.
    pub withdrawn_at: Option<DateTime<Utc>>,
    /// Author.
    pub created_by_id: Option<Uuid>,
    /// Creation time.
    pub created_at: DateTime<Utc>,
}

/// An entry in the workspace activity feed.
#[derive(Debug, Clone, FromRow)]
pub struct ActivityEntry {
    /// Identifier.
    pub id: Uuid,
    /// Owning workspace.
    pub workspace_id: Uuid,
    /// Who acted.
    pub actor_person_id: Option<Uuid>,
    /// Their name, joined for display.
    pub actor_name: Option<String>,
    /// What happened.
    pub kind: String,
    /// Kind of the subject.
    pub subject_type: String,
    /// Identifier of the subject.
    pub subject_id: Option<Uuid>,
    /// Human-readable summary.
    pub summary: String,
    /// Classification.
    pub classification: String,
    /// When it happened.
    pub created_at: DateTime<Utc>,
}
