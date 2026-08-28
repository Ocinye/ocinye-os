//! Research rows.

use chrono::{DateTime, Utc};
use ocinye_contracts::{Classification, IdeaState, ProjectState, WorkspaceKind};
use sqlx::FromRow;
use uuid::Uuid;

/// The contextual container for an idea or a project.
#[derive(Debug, Clone, FromRow)]
pub struct ResearchWorkspace {
    /// Identifier.
    pub id: Uuid,
    /// Organisation.
    pub organisation_id: Uuid,
    /// Owning unit.
    pub unit_id: Uuid,
    /// Human-readable code, for example `AI-IDEA-004`.
    pub code: String,
    /// Title.
    pub title: String,
    /// Whether it currently hosts an idea or a project.
    pub kind: String,
    /// Classification governing everything inside it.
    pub classification: String,
    /// When it was archived.
    pub archived_at: Option<DateTime<Utc>>,
    /// Creation time.
    pub created_at: DateTime<Utc>,
    /// Last change.
    pub updated_at: DateTime<Utc>,
}

impl ResearchWorkspace {
    /// Parsed classification.
    ///
    /// Falls back to the most restrictive value if the stored string is ever
    /// unrecognised: an unreadable classification must never widen access.
    #[must_use]
    pub fn classification(&self) -> Classification {
        Classification::parse(&self.classification).unwrap_or(Classification::Restricted)
    }

    /// Parsed kind.
    #[must_use]
    pub fn kind(&self) -> WorkspaceKind {
        WorkspaceKind::parse(&self.kind).unwrap_or(WorkspaceKind::Idea)
    }
}

/// A person's membership of a research workspace.
#[derive(Debug, Clone, FromRow)]
pub struct WorkspaceMember {
    /// Membership identifier.
    pub id: Uuid,
    /// Workspace.
    pub workspace_id: Uuid,
    /// Person.
    pub person_id: Uuid,
    /// Person's full name, joined for display.
    pub full_name: String,
    /// Role in the workspace.
    pub role: String,
    /// When membership was created.
    pub created_at: DateTime<Utc>,
}

/// An exploratory idea.
#[derive(Debug, Clone, FromRow)]
pub struct Idea {
    /// Identifier.
    pub id: Uuid,
    /// Workspace holding it.
    pub workspace_id: Uuid,
    /// Title.
    pub title: String,
    /// Summary.
    pub summary: Option<String>,
    /// The question being asked.
    pub research_question: Option<String>,
    /// The hypothesis, when one has formed.
    pub hypothesis: Option<String>,
    /// Why it matters.
    pub motivation: Option<String>,
    /// Keywords.
    pub keywords: Vec<String>,
    /// Lifecycle state.
    pub state: String,
    /// Why it was closed, when it was.
    pub outcome_note: Option<String>,
    /// Project it became, when promoted.
    pub promoted_project_id: Option<Uuid>,
    /// Creation time.
    pub created_at: DateTime<Utc>,
    /// Last change.
    pub updated_at: DateTime<Utc>,
}

impl Idea {
    /// Parsed state.
    #[must_use]
    pub fn state(&self) -> IdeaState {
        IdeaState::parse(&self.state).unwrap_or(IdeaState::Discovery)
    }
}

/// A formal project.
#[derive(Debug, Clone, FromRow)]
pub struct Project {
    /// Identifier.
    pub id: Uuid,
    /// Organisation.
    pub organisation_id: Uuid,
    /// Workspace holding it.
    pub workspace_id: Uuid,
    /// Institutional code.
    pub code: String,
    /// Title.
    pub title: String,
    /// Summary.
    pub summary: Option<String>,
    /// Objectives.
    pub objectives: Option<String>,
    /// Lifecycle state.
    pub state: String,
    /// The idea it came from, when it came from one.
    pub origin_idea_id: Option<Uuid>,
    /// Person accountable for it.
    pub responsible_person_id: Option<Uuid>,
    /// When work started.
    pub started_at: Option<DateTime<Utc>>,
    /// When work finished.
    pub completed_at: Option<DateTime<Utc>>,
    /// Creation time.
    pub created_at: DateTime<Utc>,
    /// Last change.
    pub updated_at: DateTime<Utc>,
}

impl Project {
    /// Parsed state.
    #[must_use]
    pub fn state(&self) -> ProjectState {
        ProjectState::parse(&self.state).unwrap_or(ProjectState::Draft)
    }
}
