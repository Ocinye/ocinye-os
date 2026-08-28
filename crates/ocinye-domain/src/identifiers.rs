//! Validation of human-facing institutional identifiers.
//!
//! These codes appear in citations, in file names and in conversation. They are
//! deliberately constrained so they stay unambiguous, and validated here rather
//! than at each call site.

use crate::error::{DomainError, DomainResult};

/// Longest permitted unit code.
pub(crate) const UNIT_CODE_MAX: usize = 16;
/// Longest permitted compute node identifier.
pub(crate) const NODE_IDENTIFIER_MAX: usize = 24;
/// Longest permitted project code.
pub(crate) const PROJECT_CODE_MAX: usize = 32;

fn validate_code(raw: &str, max: usize, label: &str) -> DomainResult<String> {
    let code = raw.trim().to_ascii_uppercase();

    if code.len() < 2 || code.len() > max {
        return Err(DomainError::Validation(format!(
            "A {label} must be between 2 and {max} characters."
        )));
    }
    if !code.starts_with(|c: char| c.is_ascii_alphabetic()) {
        return Err(DomainError::Validation(format!(
            "A {label} must start with a letter."
        )));
    }
    if !code.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err(DomainError::Validation(format!(
            "A {label} may contain only letters, digits and hyphens."
        )));
    }
    if code.ends_with('-') || code.contains("--") {
        return Err(DomainError::Validation(format!(
            "A {label} must not end with a hyphen or contain consecutive hyphens."
        )));
    }
    Ok(code)
}

/// Validate and normalise a unit code, for example `AI` or `ENERGY-SYS`.
///
/// # Errors
///
/// Returns [`DomainError::Validation`] when the code is malformed.
pub fn validate_unit_code(raw: &str) -> DomainResult<String> {
    validate_code(raw, UNIT_CODE_MAX, "unit code")
}

/// Validate and normalise a project code.
///
/// # Errors
///
/// Returns [`DomainError::Validation`] when the code is malformed.
pub fn validate_project_code(raw: &str) -> DomainResult<String> {
    validate_code(raw, PROJECT_CODE_MAX, "project code")
}

/// Validate and normalise a compute node identifier, for example `CAM-01`.
///
/// The identifier is supplied at registration. No node identifier is ever
/// hardcoded anywhere in the system (ADR-0500).
///
/// # Errors
///
/// Returns [`DomainError::Validation`] when the identifier is malformed.
pub fn validate_node_identifier(raw: &str) -> DomainResult<String> {
    validate_code(raw, NODE_IDENTIFIER_MAX, "node identifier")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_and_normalises_realistic_codes() {
        assert_eq!(validate_unit_code(" ai ").unwrap(), "AI");
        assert_eq!(validate_unit_code("energy-sys").unwrap(), "ENERGY-SYS");
        assert_eq!(validate_node_identifier("cam-01").unwrap(), "CAM-01");
    }

    #[test]
    fn rejects_malformed_codes() {
        for bad in [
            "",
            "A",
            "1AI",
            "AI_X",
            "AI--X",
            "AI-",
            "  ",
            &"A".repeat(64),
        ] {
            assert!(validate_unit_code(bad).is_err(), "should reject {bad:?}");
        }
    }

    #[test]
    fn node_identifiers_are_not_privileged_by_name() {
        // CAM-01 is just a value: nothing in validation treats it specially.
        assert_eq!(validate_node_identifier("CAM-01").unwrap(), "CAM-01");
        assert_eq!(validate_node_identifier("HPC-99").unwrap(), "HPC-99");
    }
}
