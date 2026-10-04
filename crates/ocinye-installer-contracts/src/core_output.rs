//! What the Core's host-side subcommands print (one JSON line each), as the
//! bootstrap reads it: `endpoint-seed`, `verify-schema`, `verify-instance`,
//! `verify-endpoints`, `verify-admin-bootstrap` (ADR-0022).
//!
//! A test in the Core server serialises the Core's own types and parses them
//! here, so the two sides cannot drift apart without a red test.

use serde::{Deserialize, Serialize};

/// `endpoint-seed` refusal codes (exit 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EndpointSeedRefusal {
    /// The Instance was not created by this installation.
    NotANewInstance,
    /// A member session exists.
    SessionsExist,
    /// Not a host name.
    HostInvalid,
    /// The canonical host.
    HostIsCanonical,
    /// The host serves another destination.
    HostTaken,
    /// The Distribution is not enabled.
    DistributionNotEnabled,
    /// Not one of the four.
    DistributionUnknown,
}

/// `endpoint-seed` output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EndpointSeedOutput {
    /// Created or unchanged.
    Seeded {
        /// `created` or `unchanged`.
        result: String,
        /// Endpoint id.
        endpoint_id: String,
        /// Host.
        host: String,
        /// Distribution.
        distribution: String,
        /// `active`.
        state: String,
    },
    /// Refused.
    Refused {
        /// Code.
        refused: EndpointSeedRefusal,
    },
}

/// `verify-schema`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerifySchema {
    /// Latest applied, four digits.
    pub latest: String,
    /// Applied count.
    pub count: u32,
    /// Known to the binary, not applied.
    pub pending: u32,
}

/// Applications in `verify-instance`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerifyApplications {
    /// Registered in the release.
    pub registered: u32,
    /// Active in the Instance.
    pub active: u32,
    /// Essential applications inactive.
    pub essential_inactive: u32,
}

/// `verify-instance`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerifyInstance {
    /// Name.
    pub name: String,
    /// Slug.
    pub slug: String,
    /// Enabled, birth first.
    pub distributions: Vec<String>,
    /// The registry.
    pub applications: VerifyApplications,
}

/// One `verify-endpoints` entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerifyEndpoint {
    /// Host.
    pub host: String,
    /// Canonical.
    pub canonical: bool,
    /// Bound Distribution.
    pub distribution: Option<String>,
    /// State.
    pub state: String,
}

/// `verify-admin-bootstrap`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerifyAdminBootstrap {
    /// A privileged identity exists.
    pub privileged_identity_exists: bool,
    /// Its temporary credential is still unused.
    pub temporary_credential_pending: bool,
}
