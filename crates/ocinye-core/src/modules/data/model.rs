//! Data rows.

use chrono::{DateTime, NaiveDate, Utc};
use ocinye_contracts::Classification;
use serde::Serialize;
use sqlx::FromRow;
use uuid::Uuid;

/// Where a dataset came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DatasetOrigin {
    /// Collected by Ocinye.
    #[default]
    CollectedByOcinye,
    /// Derived from other datasets.
    Derived,
    /// Third-party, openly licensed.
    ThirdPartyOpen,
    /// Third-party, under licence.
    ThirdPartyLicensed,
    /// Provided by a partner.
    PartnerProvided,
    /// Produced by simulation.
    Simulated,
}

impl DatasetOrigin {
    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CollectedByOcinye => "collected_by_ocinye",
            Self::Derived => "derived",
            Self::ThirdPartyOpen => "third_party_open",
            Self::ThirdPartyLicensed => "third_party_licensed",
            Self::PartnerProvided => "partner_provided",
            Self::Simulated => "simulated",
        }
    }

    /// Parse from the stable representation.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "collected_by_ocinye" => Self::CollectedByOcinye,
            "derived" => Self::Derived,
            "third_party_open" => Self::ThirdPartyOpen,
            "third_party_licensed" => Self::ThirdPartyLicensed,
            "partner_provided" => Self::PartnerProvided,
            "simulated" => Self::Simulated,
            _ => return None,
        })
    }
}

/// Lifecycle of a dataset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DatasetState {
    /// Being catalogued.
    Draft,
    /// In use.
    Active,
    /// Superseded but retained.
    Deprecated,
    /// Closed. Retained for the record.
    Archived,
}

impl DatasetState {
    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Active => "active",
            Self::Deprecated => "deprecated",
            Self::Archived => "archived",
        }
    }
}

/// Lifecycle of a dataset version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionStatus {
    /// Being assembled. Files may still be added.
    Draft,
    /// Published and immutable.
    Published,
    /// Withdrawn. Retained, never deleted: it is provenance.
    Withdrawn,
}

impl VersionStatus {
    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Published => "published",
            Self::Withdrawn => "withdrawn",
        }
    }

    /// Parse from the stable representation.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "draft" => Self::Draft,
            "published" => Self::Published,
            "withdrawn" => Self::Withdrawn,
            _ => return None,
        })
    }
}

/// A catalogued dataset.
#[derive(Debug, Clone, FromRow)]
pub struct Dataset {
    /// Identifier.
    pub id: Uuid,
    /// Owning unit.
    pub unit_id: Uuid,
    /// Owning workspace.
    pub workspace_id: Uuid,
    /// Institutional code.
    pub code: String,
    /// Title.
    pub title: String,
    /// Description.
    pub description: Option<String>,
    /// Where it came from.
    pub origin: String,
    /// Licence.
    pub licence: Option<String>,
    /// Contractual or ethical limits on use.
    pub usage_restrictions: Option<String>,
    /// Person accountable for it.
    pub responsible_person_id: Option<Uuid>,
    /// When it was acquired.
    pub acquisition_date: Option<NaiveDate>,
    /// Keywords.
    pub keywords: Vec<String>,
    /// Classification.
    pub classification: String,
    /// Lifecycle state.
    pub state: String,
    /// Creation time.
    pub created_at: DateTime<Utc>,
}

impl Dataset {
    /// Parsed classification, defaulting to the most restrictive.
    #[must_use]
    pub fn classification(&self) -> Classification {
        Classification::parse(&self.classification).unwrap_or(Classification::Restricted)
    }
}

/// An immutable version of a dataset.
#[derive(Debug, Clone, FromRow)]
pub struct DatasetVersion {
    /// Identifier.
    pub id: Uuid,
    /// Dataset this belongs to.
    pub dataset_id: Uuid,
    /// Version label, for example `1.2`.
    pub label: String,
    /// Ordering within the dataset.
    pub sequence: i32,
    /// Status.
    pub status: String,
    /// Notes about this version.
    pub notes: Option<String>,
    /// How it was produced and from what.
    pub provenance: Option<String>,
    /// Version it was derived from.
    pub derived_from_version_id: Option<Uuid>,
    /// Total size of its files.
    pub total_size_bytes: i64,
    /// Number of files.
    pub file_count: i32,
    /// When it was published.
    pub published_at: Option<DateTime<Utc>>,
    /// Creation time.
    pub created_at: DateTime<Utc>,
}

impl DatasetVersion {
    /// Parsed status.
    #[must_use]
    pub fn status(&self) -> VersionStatus {
        VersionStatus::parse(&self.status).unwrap_or(VersionStatus::Draft)
    }
}

/// A file inside a dataset version.
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct DatasetFile {
    /// Identifier.
    pub id: Uuid,
    /// Version it belongs to.
    pub version_id: Uuid,
    /// Logical path inside the dataset.
    pub path: String,
    /// Size in bytes.
    pub size_bytes: i64,
    /// SHA-256 checksum.
    pub checksum_sha256: String,
    /// MIME type.
    pub content_type: String,
}

/// Validate a version label such as `1`, `1.2` or `1.2.3`.
///
/// # Errors
///
/// Returns an error when the label is not of that shape.
pub fn validate_version_label(raw: &str) -> Result<String, crate::error::CoreError> {
    let label = raw.trim();
    let valid = !label.is_empty()
        && label.len() <= 32
        && label.split('.').count() <= 3
        && label
            .split('.')
            .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()));

    if valid {
        Ok(label.to_owned())
    } else {
        Err(crate::error::CoreError::Validation(
            "A dataset version must look like '1', '1.2' or '1.2.3'.".to_owned(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_labels_accept_realistic_shapes() {
        for good in ["1", "1.2", "1.2.3", " 2.0 "] {
            assert!(
                validate_version_label(good).is_ok(),
                "should accept {good:?}"
            );
        }
    }

    #[test]
    fn version_labels_reject_anything_else() {
        for bad in ["", "v1", "1.", ".1", "1.2.3.4", "1-2", "latest", "1.a"] {
            assert!(
                validate_version_label(bad).is_err(),
                "should reject {bad:?}"
            );
        }
    }
}
