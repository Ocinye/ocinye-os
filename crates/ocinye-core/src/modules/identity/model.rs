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
    /// Account status.
    pub status: String,
    /// O que esta identidade operacional **é**.
    ///
    /// `human` para uma pessoa; `privileged` para a identidade por onde alguém
    /// exerce autoridade administrativa. Não é um booleano `is_admin`: a
    /// propriedade não é sobre autorização — uma identidade privilegiada passa
    /// pela mesma política que todas as outras.
    pub identity_kind: String,
    /// A pessoa a quem esta identidade privilegiada pertence.
    ///
    /// > **Uma identidade privilegiada ligada estabelece responsabilidade, e não
    /// > herança de autoridade.**
    ///
    /// A ligação existe para a auditoria poder dizer quem está por trás de uma
    /// operação administrativa. Nada atravessa esta seta: nem papéis, nem
    /// pertenças, nem credenciais, nem sessões.
    pub belongs_to_person_id: Option<Uuid>,

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

    /// Whether this account is a still-unaccepted, never-used invitation.
    ///
    /// The one state in which a person can be deleted rather than disabled: an
    /// `invited` account becomes `active` only when its holder first signs in and
    /// sets a password, so one that is still `invited` and was never seen has
    /// authored nothing. Both facts are required — a row that is `invited` yet
    /// carries a `last_seen_at` is inconsistent, and the safe reading of an
    /// inconsistency is *not deletable*.
    #[must_use]
    pub fn never_activated(&self) -> bool {
        self.account_status() == AccountStatus::Invited && self.last_seen_at.is_none()
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

#[cfg(test)]
mod tests {
    use super::*;

    fn person_with(status: &str, last_seen_at: Option<DateTime<Utc>>) -> Person {
        Person {
            id: Uuid::nil(),
            organisation_id: Uuid::nil(),
            oidc_subject: None,
            email: "a@b.c".to_owned(),
            full_name: "A".to_owned(),
            display_name: None,
            institutional_position: None,
            orcid: None,
            biography: None,
            status: status.to_owned(),
            identity_kind: "human".to_owned(),
            belongs_to_person_id: None,
            last_seen_at,
            deactivated_at: None,
            created_at: Utc::now(),
        }
    }

    /// Deletable is the narrow case: `invited` and never seen, and nothing else.
    ///
    /// An active account is woven into the record and only ever disabled; an
    /// `invited` row that somehow carries a `last_seen_at` is inconsistent, and
    /// the safe reading of an inconsistency is *not deletable*.
    #[test]
    fn never_activated_is_invited_and_unseen_only() {
        assert!(person_with("invited", None).never_activated());
        assert!(!person_with("active", None).never_activated());
        assert!(!person_with("suspended", None).never_activated());
        assert!(!person_with("disabled", None).never_activated());
        // Invited but seen: inconsistent, so not deletable.
        assert!(!person_with("invited", Some(Utc::now())).never_activated());
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
