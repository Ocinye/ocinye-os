//! Resource identification.

use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A typed reference to any institutional resource.
///
/// Identifiers are UUIDv4: unguessable, but never a substitute for
/// authorization. Every read still passes the policy (briefing §71).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ResourceIdentifier {
    /// Kind of resource, e.g. `idea`, `dataset`, `document`.
    pub kind: String,
    /// Identifier of the resource.
    pub id: Uuid,
}

impl ResourceIdentifier {
    /// Build a reference.
    #[must_use]
    pub fn new(kind: impl Into<String>, id: Uuid) -> Self {
        Self {
            kind: kind.into(),
            id,
        }
    }
}

impl fmt::Display for ResourceIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ocinye:{}:{}", self.kind, self.id)
    }
}
