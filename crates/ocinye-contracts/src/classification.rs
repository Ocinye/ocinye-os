//! Information classification.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Institutional information classification.
///
/// Classification is ordered and travels with the artefact through copies,
/// versions, derivatives and exports. It constrains reading, writing,
/// downloading, exporting, search indexing and AI retrieval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Classification {
    /// Publishable outside the institution.
    Public,
    /// Readable by any active member of the organisation.
    Internal,
    /// Requires unit or research-workspace membership.
    Confidential,
    /// Requires explicit research-workspace membership or unit management.
    /// Administrative roles alone never suffice.
    Restricted,
}

impl Classification {
    /// The safe default for a newly created artefact.
    ///
    /// Deliberately not [`Classification::Public`]: secure defaults mean the
    /// more restrictive choice wins when nobody stated one.
    pub const DEFAULT: Self = Self::Internal;

    // Não existe aqui uma constante para o tecto de indexação por IA, e é
    // deliberado.
    //
    // Existiu: `AI_INDEXABLE_MAX = Confidential`, com a nota «a classificação
    // mais alta que pode entrar num índice de recuperação sem uma decisão
    // explícita à parte». Nunca foi lida por ninguém, e era mais permissiva do
    // que a política que o sistema aplica de facto.
    //
    // Quem decide é `ocinye_domain::ai_processing_ceiling`, e decide com o
    // parâmetro que importa: sem inferência local, o tecto é `INTERNAL`, porque
    // todo o modelo está algures que a instituição não controla. Uma constante
    // que dizia `CONFIDENTIAL` sem esse parâmetro afirmava, no crate de aspecto
    // mais autoritativo, exactamente aquilo que o domínio recusa hoje.
    //
    // Uma segunda fonte de verdade sobre uma decisão de segurança é pior do que
    // nenhuma: as duas parecem oficiais, e a mais permissiva é a que alguém cita.

    /// Ordinal used for comparisons and storage.
    #[must_use]
    pub const fn level(self) -> u8 {
        match self {
            Self::Public => 0,
            Self::Internal => 1,
            Self::Confidential => 2,
            Self::Restricted => 3,
        }
    }

    /// Stable wire and database representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Public => "PUBLIC",
            Self::Internal => "INTERNAL",
            Self::Confidential => "CONFIDENTIAL",
            Self::Restricted => "RESTRICTED",
        }
    }

    /// Parse from the stable representation.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "PUBLIC" => Some(Self::Public),
            "INTERNAL" => Some(Self::Internal),
            "CONFIDENTIAL" => Some(Self::Confidential),
            "RESTRICTED" => Some(Self::Restricted),
            _ => None,
        }
    }

    /// The more restrictive of two classifications.
    ///
    /// A derived artefact never becomes more open than what it was derived
    /// from, which is why every creation path runs its inputs through this.
    #[must_use]
    pub fn most_restrictive(self, other: Self) -> Self {
        if self.level() >= other.level() {
            self
        } else {
            other
        }
    }

    /// Every classification, ascending. Useful for exhaustive policy tests.
    #[must_use]
    pub const fn all() -> [Self; 4] {
        [
            Self::Public,
            Self::Internal,
            Self::Confidential,
            Self::Restricted,
        ]
    }
}

impl fmt::Display for Classification {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Default for Classification {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordering_matches_levels() {
        assert!(Classification::Restricted > Classification::Confidential);
        assert!(Classification::Confidential > Classification::Internal);
        assert!(Classification::Internal > Classification::Public);
    }

    #[test]
    fn round_trips_through_stable_representation() {
        for value in Classification::all() {
            assert_eq!(Classification::parse(value.as_str()), Some(value));
        }
    }

    #[test]
    fn default_is_not_public() {
        assert_ne!(Classification::default(), Classification::Public);
    }

    #[test]
    fn derivation_never_becomes_more_open() {
        for a in Classification::all() {
            for b in Classification::all() {
                let derived = a.most_restrictive(b);
                assert!(derived.level() >= a.level());
                assert!(derived.level() >= b.level());
            }
        }
    }
}
