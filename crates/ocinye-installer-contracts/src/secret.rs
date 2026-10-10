//! Secrets in memory, and the redaction layer for everything that is printed.
//!
//! An SSH password, a sudo password and the one-time administrator credential
//! live in a [`SecretText`]: zeroized on drop, printed as `[segredo omitido]`
//! by `Debug` and `Display`, and **not serialisable** — a struct that contains
//! one cannot be written to a journal, a receipt or a log by accident, because
//! it does not compile.
//!
//! The one place a secret legitimately crosses the channel — the temporary
//! credential, once, from the bootstrap to the controller — has its own message
//! type ([`crate::protocol::OneTimeSecret`]) that builds the JSON explicitly.

use std::fmt;

use serde::{Deserialize, Deserializer};
use zeroize::Zeroize;

/// What the logs say instead of a secret.
pub const REDACTED: &str = "[segredo omitido]";

/// A secret string. Zeroized on drop; never printed; not `Serialize`.
pub struct SecretText(String);

impl SecretText {
    /// Wrap a value. The caller's copy should be dropped right after.
    #[must_use]
    pub fn new(value: String) -> Self {
        Self(value)
    }

    /// The value, for the one call that needs it (`sudo -S` stdin, the
    /// one-time display). Never for formatting into a log.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// Empty?
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Drop for SecretText {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl fmt::Debug for SecretText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(REDACTED)
    }
}

impl fmt::Display for SecretText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(REDACTED)
    }
}

impl<'de> Deserialize<'de> for SecretText {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        String::deserialize(d).map(Self)
    }
}

/// Keys whose values are secrets wherever they appear as `KEY=value`.
const SECRET_KEY_PARTS: [&str; 7] = [
    "PASSWORD", "PASSWD", "SECRET", "TOKEN", "SEALING", "PRIVATE", "_KEY",
];

/// Redact a line of text before it is stored or shown as a technical detail.
///
/// - `KEY=value` where the key names a secret → `KEY=[segredo omitido]`;
/// - a line that announces a password (`Palavra-passe`, `password:`) loses
///   everything after the label;
/// - a PEM private-key block loses its body.
///
/// This is a backstop, not the mechanism: events carry closed codes, and the
/// secrets are never in them to begin with.
#[must_use]
pub fn redact_line(line: &str) -> String {
    let lower = line.to_lowercase();
    if lower.contains("private key-----") || lower.contains("begin openssh") {
        return REDACTED.to_owned();
    }
    for label in ["palavra-passe", "password:", "mot de passe", "credential:"] {
        if let Some(i) = lower.find(label) {
            let cut = i + label.len();
            // `cut` is a char boundary of `lower`; map back by char count.
            let chars = lower[..cut].chars().count();
            let head: String = line.chars().take(chars).collect();
            return format!("{head} {REDACTED}");
        }
    }
    line.split(' ')
        .map(|word| match word.split_once('=') {
            Some((k, _))
                if SECRET_KEY_PARTS
                    .iter()
                    .any(|p| k.to_uppercase().contains(p)) =>
            {
                format!("{k}={REDACTED}")
            }
            _ => word.to_owned(),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn um_segredo_nunca_se_imprime() {
        let s = SecretText::new("hunter2".to_owned());
        assert_eq!(format!("{s}"), REDACTED);
        assert_eq!(format!("{s:?}"), REDACTED);
        assert_eq!(format!("{:?}", Some(&s)), format!("Some({REDACTED})"));
        assert_eq!(s.expose(), "hunter2");
    }

    #[test]
    fn a_redaccao_tira_os_valores_e_deixa_o_resto() {
        assert_eq!(
            redact_line("core.env 0600 · OCINYE_SEALING_KEY=abc123 POSTGRES_PASSWORD=x ok=1"),
            "core.env 0600 · OCINYE_SEALING_KEY=[segredo omitido] POSTGRES_PASSWORD=[segredo omitido] ok=1"
        );
        assert_eq!(
            redact_line("  Palavra-passe        Xk3-temp-9"),
            "  Palavra-passe [segredo omitido]"
        );
        assert_eq!(redact_line("-----BEGIN PRIVATE KEY-----"), REDACTED,);
        assert_eq!(redact_line("migration 0041/0064"), "migration 0041/0064");
    }
}
