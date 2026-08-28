//! A secret that cannot be printed by accident.
//!
//! The single most common way a password reaches a log file is a `{:?}` on a
//! struct that happens to contain one. [`Secret`] makes that impossible: its
//! `Debug` is redacted and it deliberately implements neither `Display` nor
//! `Serialize`, so it cannot be formatted into a message or a response body
//! without someone writing the words [`Secret::expose`] first.

use std::fmt;

use zeroize::{Zeroize, ZeroizeOnDrop};

/// A password, temporary credential or other short-lived secret in the clear.
///
/// Zeroized on drop. That is not a guarantee against a determined attacker with
/// memory access — Rust may still have moved the bytes — but it shortens the
/// window and costs nothing (briefing §93).
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct Secret(String);

impl Secret {
    /// Wrap a value.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Read the secret.
    ///
    /// Named so that every place a secret leaves its wrapper is greppable.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// Length in bytes of the underlying UTF-8.
    ///
    /// Available without exposing the value, for size guards.
    #[must_use]
    pub fn len_bytes(&self) -> usize {
        self.0.len()
    }

    /// Whether the secret is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // No length, either: the length of a password is information.
        f.write_str("Secret(<redacted>)")
    }
}

impl From<String> for Secret {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for Secret {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

/// Deserialised so a request body can carry a password without the field ever
/// existing as a bare `String` in a struct that might later be logged.
impl<'de> serde::Deserialize<'de> for Secret {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        String::deserialize(deserializer).map(Self::new)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_never_reveals_the_value_or_its_length() {
        let secret = Secret::new("correct horse battery staple");
        let rendered = format!("{secret:?}");
        assert_eq!(rendered, "Secret(<redacted>)");
        assert!(!rendered.contains("horse"));
        assert!(!rendered.contains("28"));
    }

    #[test]
    fn a_secret_inside_another_struct_stays_redacted() {
        #[derive(Debug)]
        #[allow(dead_code)]
        struct LoginRequest {
            username: String,
            password: Secret,
        }

        let request = LoginRequest {
            username: "fmonteiro".into(),
            password: Secret::new("a very long passphrase indeed"),
        };
        let rendered = format!("{request:?}");
        assert!(rendered.contains("fmonteiro"));
        assert!(!rendered.contains("passphrase"));
    }

    #[test]
    fn the_value_is_reachable_only_by_exposing_it() {
        let secret = Secret::new("value");
        assert_eq!(secret.expose(), "value");
        assert_eq!(secret.len_bytes(), 5);
        assert!(!secret.is_empty());
    }
}
