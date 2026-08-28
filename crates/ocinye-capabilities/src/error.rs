//! SystemCapability runtime errors.

use thiserror::Error;

/// Result of a capability operation.
pub type CapabilityResult<T> = Result<T, CapabilityError>;

/// What can go wrong running a capability.
#[derive(Debug, Error)]
pub enum CapabilityError {
    /// The manifest is not valid.
    #[error("invalid capability manifest: {0}")]
    InvalidManifest(String),

    /// The component could not be loaded.
    #[error("capability could not be loaded: {0}")]
    Load(String),

    /// The capability asked for something policy does not grant.
    #[error("capability requested a permission that is not granted: {0}")]
    PermissionDenied(String),

    /// The capability exceeded its resource limits.
    ///
    /// Distinguished from a crash on purpose: hitting a limit is the sandbox
    /// working, and an operator should be able to tell the two apart.
    #[error("capability exceeded its resource limits: {0}")]
    ResourceExhausted(String),

    /// The capability failed while running.
    #[error("capability failed: {0}")]
    Execution(String),

    /// Input or output did not match the declared contract.
    #[error("capability contract violated: {0}")]
    Contract(String),
}
