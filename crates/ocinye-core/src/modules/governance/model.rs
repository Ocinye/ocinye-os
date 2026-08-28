//! Audit rows as read back.

use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use sqlx::FromRow;
use uuid::Uuid;

/// One audit record.
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct AuditRecord {
    /// Identifier.
    pub id: Uuid,
    /// When it happened.
    pub occurred_at: DateTime<Utc>,
    /// Who acted.
    pub actor_person_id: Option<Uuid>,
    /// Their name, joined for display.
    pub actor_name: Option<String>,
    /// What happened.
    pub action: String,
    /// Kind of resource.
    pub resource_type: String,
    /// Identifier of the resource.
    pub resource_id: Option<Uuid>,
    /// Owning unit.
    pub unit_id: Option<Uuid>,
    /// Owning workspace.
    pub workspace_id: Option<Uuid>,
    /// Classification at the time.
    pub classification: Option<String>,
    /// Result.
    pub outcome: String,
    /// Identifier correlating this to logs and other services.
    pub correlation_id: Option<String>,
    /// Bounded, non-sensitive detail.
    pub metadata: Value,
}
