//! Identity rows.

use chrono::{DateTime, Utc};
use ocinye_contracts::{AccountStatus, InstitutionalPosition};
use serde::Serialize;
use sqlx::FromRow;
use uuid::Uuid;

/// A member of the institution.
#[derive(Debug, Clone, FromRow)]
pub struct Person {
    /// Identifier.
    pub id: Uuid,
    /// Organisation.
    pub organisation_id: Uuid,
    /// Verified OIDC subject; `None` until first sign-in.
    pub oidc_subject: Option<String>,
    /// Email address.
    pub email: String,
    /// Full name.
    pub full_name: String,
    /// Preferred display name.
    pub display_name: Option<String>,
    /// Institutional position. Grants nothing.
    pub institutional_position: Option<String>,
    /// ORCID identifier.
    pub orcid: Option<String>,
    /// Short biography.
    pub biography: Option<String>,
    /// Sign-in name. `None` only for rows predating ADR-0103.
    pub username: Option<String>,
    /// Account status.
    pub status: String,
    /// Last activity.
    pub last_seen_at: Option<DateTime<Utc>>,
    /// When membership ended.
    pub deactivated_at: Option<DateTime<Utc>>,
    /// Creation time.
    pub created_at: DateTime<Utc>,
}

impl Person {
    /// Parsed account status.
    ///
    /// An unrecognised value reads as `Suspended`, never as `Active`: a row
    /// this build cannot interpret must not authenticate.
    #[must_use]
    pub fn account_status(&self) -> AccountStatus {
        AccountStatus::parse(&self.status).unwrap_or(AccountStatus::Suspended)
    }

    /// Whether the person may act on the platform.
    #[must_use]
    pub fn can_act(&self) -> bool {
        self.account_status() == AccountStatus::Active
    }

    /// Parsed institutional position.
    #[must_use]
    pub fn position(&self) -> Option<InstitutionalPosition> {
        self.institutional_position
            .as_deref()
            .and_then(InstitutionalPosition::parse)
    }

    /// Name to show.
    #[must_use]
    pub fn preferred_name(&self) -> &str {
        self.display_name.as_deref().unwrap_or(&self.full_name)
    }
}

/// Lifecycle of an invitation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InvitationStatus {
    /// Awaiting acceptance.
    Pending,
    /// Accepted.
    Accepted,
    /// Withdrawn by an administrator.
    Revoked,
    /// Expired.
    Expired,
}

impl InvitationStatus {
    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Accepted => "accepted",
            Self::Revoked => "revoked",
            Self::Expired => "expired",
        }
    }
}

/// An invitation to join the institution.
#[derive(Debug, Clone, FromRow)]
pub struct Invitation {
    /// Identifier.
    pub id: Uuid,
    /// Organisation.
    pub organisation_id: Uuid,
    /// Invited email.
    pub email: String,
    /// Invited person's full name.
    pub full_name: String,
    /// Institutional position offered.
    pub institutional_position: Option<String>,
    /// Status.
    pub status: String,
    /// Expiry.
    pub expires_at: DateTime<Utc>,
    /// Acceptance time.
    pub accepted_at: Option<DateTime<Utc>>,
    /// Person created on acceptance.
    pub accepted_person_id: Option<Uuid>,
    /// Creation time.
    pub created_at: DateTime<Utc>,
}
