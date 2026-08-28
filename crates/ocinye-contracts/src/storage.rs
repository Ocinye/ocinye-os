//! Storage residency contracts.
//!
//! Institutional control and physical residency are different things. The Core
//! governs every object it records; where the bytes physically live is a
//! separate, explicitly declared attribute (briefing §34).

use serde::{Deserialize, Serialize};

/// Where the bytes of an object physically reside.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Residency {
    /// Not declared. The honest default while Ocinye owns no infrastructure.
    #[default]
    Undeclared,
    /// Infrastructure operated by a third party.
    ThirdPartyCloud,
    /// Ocinye infrastructure in Camama, Angola. Only once it exists.
    OcinyeCamama,
    /// Ocinye equipment in a colocation facility.
    OcinyeColocation,
}

impl Residency {
    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Undeclared => "UNDECLARED",
            Self::ThirdPartyCloud => "THIRD_PARTY_CLOUD",
            Self::OcinyeCamama => "OCINYE_CAMAMA",
            Self::OcinyeColocation => "OCINYE_COLOCATION",
        }
    }

    /// Parse from the stable representation.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "UNDECLARED" => Self::Undeclared,
            "THIRD_PARTY_CLOUD" => Self::ThirdPartyCloud,
            "OCINYE_CAMAMA" => Self::OcinyeCamama,
            "OCINYE_COLOCATION" => Self::OcinyeColocation,
            _ => return None,
        })
    }

    /// Whether this residency is on infrastructure the institution owns.
    ///
    /// Used by reporting so the system never claims institutional residency it
    /// does not have.
    #[must_use]
    pub const fn is_ocinye_owned(self) -> bool {
        matches!(self, Self::OcinyeCamama | Self::OcinyeColocation)
    }
}

/// Whether a backend is stable or part of a migration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationState {
    /// Not migrating.
    Stable,
    /// A migration has been decided but not started.
    MigrationPlanned,
    /// Objects are being moved.
    Migrating,
}

impl MigrationState {
    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Stable => "stable",
            Self::MigrationPlanned => "migration_planned",
            Self::Migrating => "migrating",
        }
    }

    /// Parse from the stable representation.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "stable" => Self::Stable,
            "migration_planned" => Self::MigrationPlanned,
            "migrating" => Self::Migrating,
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_residency_claims_nothing() {
        assert_eq!(Residency::default(), Residency::Undeclared);
        assert!(!Residency::default().is_ocinye_owned());
    }
}
