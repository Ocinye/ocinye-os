//! Organisation rows.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;
use uuid::Uuid;

/// Lifecycle of a unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UnitStatus {
    /// Operating.
    Active,
    /// Closed. Retained for the record, never deleted.
    Archived,
}

impl UnitStatus {
    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Archived => "archived",
        }
    }
}

/// The institution.
#[derive(Debug, Clone, FromRow)]
pub struct Organisation {
    /// Identifier.
    pub id: Uuid,
    /// Short stable slug.
    pub slug: String,
    /// Display name.
    pub name: String,
    /// Registered legal name.
    pub legal_name: Option<String>,
    /// ISO country code.
    pub country: Option<String>,
    /// Description.
    pub description: Option<String>,
    /// Creation time.
    pub created_at: DateTime<Utc>,
}

/// A scientific unit.
#[derive(Debug, Clone, FromRow)]
pub struct Unit {
    /// Identifier.
    pub id: Uuid,
    /// Organisation.
    pub organisation_id: Uuid,
    /// Short code, for example `AI`.
    pub code: String,
    /// Display name.
    pub name: String,
    /// Description.
    pub description: Option<String>,
    /// Declared areas of research.
    pub research_areas: Vec<String>,
    /// Status.
    pub status: String,
    /// When it was archived.
    pub archived_at: Option<DateTime<Utc>>,
    /// Creation time.
    pub created_at: DateTime<Utc>,
}

/// A person's membership of a unit.
#[derive(Debug, Clone, FromRow)]
pub struct UnitMember {
    /// Membership identifier.
    pub id: Uuid,
    /// Unit.
    pub unit_id: Uuid,
    /// Person.
    pub person_id: Uuid,
    /// Person's full name, joined for display.
    pub full_name: String,
    /// Role in the unit.
    pub role: String,
    /// When membership was created.
    pub created_at: DateTime<Utc>,
}
