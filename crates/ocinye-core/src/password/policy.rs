//! What makes a password acceptable.
//!
//! Length and unpredictability, and nothing else. There is deliberately no
//! "must contain a symbol" rule (briefing §6): composition rules push people
//! towards `Password1!` and away from the long passphrases that actually
//! resist an offline attack.
//!
//! # Normalisation
//!
//! Exactly one transformation is applied before hashing: Unicode **NFC**. It is
//! documented rather than silent (briefing §34), and it exists so that a
//! passphrase typed on a keyboard that emits `e` + combining acute verifies
//! against one typed on a keyboard that emits `é`. Nothing is trimmed,
//! case-folded or truncated — those would change what the person typed.

use unicode_normalization::UnicodeNormalization;

use super::blocklist::{self, BlockReason};
use super::secret::Secret;

/// Minimum length of a permanent password, in characters.
///
/// Fifteen because password is currently the only factor (briefing §5). If a
/// second factor is ever introduced this number is revisitable by ADR — but
/// downwards only with a documented reason.
pub const MIN_LENGTH: usize = 15;

/// Maximum length accepted, in characters.
///
/// Well past the 64 the briefing requires. The cap exists only so that an
/// unauthenticated caller cannot make the server run Argon2 over a megabyte.
pub const MAX_LENGTH: usize = 256;

/// Hard byte ceiling, checked before any other work.
///
/// A single character can occupy four bytes, so this sits above
/// `MAX_LENGTH * 4` and only ever rejects input that is not a password at all.
pub const MAX_BYTES: usize = 4096;

/// Why a candidate password was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rejection {
    /// Shorter than [`MIN_LENGTH`].
    TooShort,
    /// Longer than [`MAX_LENGTH`], or beyond [`MAX_BYTES`].
    TooLong,
    /// Refused by the blocklist.
    Blocked(BlockReason),
    /// Identical to the temporary credential it is meant to replace.
    SameAsTemporary,
    /// Identical to the password already in use.
    SameAsCurrent,
}

impl Rejection {
    /// Message shown to the person choosing the password.
    #[must_use]
    pub fn message(self) -> String {
        match self {
            Self::TooShort => {
                format!("A palavra-passe deve ter pelo menos {MIN_LENGTH} caracteres.")
            }
            Self::TooLong => format!("A palavra-passe não pode exceder {MAX_LENGTH} caracteres."),
            Self::Blocked(reason) => reason.message().to_owned(),
            Self::SameAsTemporary => {
                "A nova palavra-passe não pode ser a palavra-passe temporária.".to_owned()
            }
            Self::SameAsCurrent => "A nova palavra-passe não pode ser igual à actual.".to_owned(),
        }
    }

    /// Stable label for the audit trail. Never accompanied by the candidate.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TooShort => "too_short",
            Self::TooLong => "too_long",
            Self::Blocked(reason) => reason.as_str(),
            Self::SameAsTemporary => "same_as_temporary",
            Self::SameAsCurrent => "same_as_current",
        }
    }
}

/// How a candidate reads to the person typing it.
///
/// Three honest states, not a percentage (briefing §26). Nothing here claims a
/// password is "secure" — the system cannot know that.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Strength {
    /// Below the minimum length.
    TooShort,
    /// Long enough but refused.
    Blocked,
    /// Meets the policy.
    Acceptable,
}

/// Normalise a password for hashing and comparison.
///
/// NFC only. See the module documentation for why this is the sole
/// transformation.
#[must_use]
pub fn normalise(raw: &Secret) -> Secret {
    Secret::new(raw.expose().nfc().collect::<String>())
}

/// Validate a candidate permanent password.
///
/// `current_matches` and `temporary_matches` are closures rather than values
/// because comparison is against a *hash*, and this module must never receive
/// one password in order to check another.
///
/// # Errors
///
/// Returns the first [`Rejection`] that applies.
pub fn validate(
    candidate: &Secret,
    temporary_matches: impl FnOnce(&Secret) -> bool,
    current_matches: impl FnOnce(&Secret) -> bool,
) -> Result<Secret, Rejection> {
    if candidate.len_bytes() > MAX_BYTES {
        return Err(Rejection::TooLong);
    }

    let normalised = normalise(candidate);
    let length = normalised.expose().chars().count();

    if length < MIN_LENGTH {
        return Err(Rejection::TooShort);
    }
    if length > MAX_LENGTH {
        return Err(Rejection::TooLong);
    }

    if let Some(reason) = blocklist::check(normalised.expose()) {
        return Err(Rejection::Blocked(reason));
    }

    // Checked after the cheap rules so that a hopeless candidate never costs an
    // Argon2 verification.
    if temporary_matches(&normalised) {
        return Err(Rejection::SameAsTemporary);
    }
    if current_matches(&normalised) {
        return Err(Rejection::SameAsCurrent);
    }

    Ok(normalised)
}

/// Assess a candidate for the interface, without the hash comparisons.
///
/// Used by the strength indicator. The authoritative check is always
/// [`validate`], server-side (briefing §27).
#[must_use]
pub fn assess(candidate: &Secret) -> Strength {
    if candidate.len_bytes() > MAX_BYTES {
        return Strength::Blocked;
    }
    let normalised = normalise(candidate);
    if normalised.expose().chars().count() < MIN_LENGTH {
        return Strength::TooShort;
    }
    if normalised.expose().chars().count() > MAX_LENGTH {
        return Strength::Blocked;
    }
    if blocklist::check(normalised.expose()).is_some() {
        return Strength::Blocked;
    }
    Strength::Acceptable
}

#[cfg(test)]
mod tests {
    use super::*;

    fn never(_: &Secret) -> bool {
        false
    }

    fn ok(candidate: &str) -> Result<Secret, Rejection> {
        validate(&Secret::new(candidate), never, never)
    }

    #[test]
    fn fourteen_characters_is_refused_and_fifteen_is_accepted() {
        assert_eq!(ok("chuva-de-abril").unwrap_err(), Rejection::TooShort);
        assert_eq!("chuva-de-abril".chars().count(), 14);

        let fifteen = "chuva-de-abrils";
        assert_eq!(fifteen.chars().count(), 15);
        assert!(ok(fifteen).is_ok());
    }

    #[test]
    fn a_sixty_four_character_passphrase_is_accepted() {
        let passphrase = "a chuva em Camama cai devagar sobre o telhado de zinco velho hoje";
        assert!(
            passphrase.chars().count() >= 64,
            "{}",
            passphrase.chars().count()
        );
        assert!(ok(passphrase).is_ok());
    }

    #[test]
    fn spaces_and_unicode_survive_unchanged_apart_from_nfc() {
        let raw = "  espaços à frente e atrás  ";
        let accepted = ok(raw).expect("should be accepted");
        // Not trimmed: the leading and trailing spaces are part of the password.
        assert!(accepted.expose().starts_with("  "));
        assert!(accepted.expose().ends_with("  "));
        // Not case-folded.
        assert!(ok("MAIÚSCULAS e minúsculas ok").is_ok());
    }

    #[test]
    fn decomposed_and_composed_forms_normalise_to_the_same_bytes() {
        // "é" as one code point, and as "e" + combining acute.
        let composed = Secret::new("investigação aplicada é boa");
        let decomposed = Secret::new("investigac\u{327}a\u{303}o aplicada e\u{301} boa");
        assert_ne!(composed.expose(), decomposed.expose());
        assert_eq!(
            normalise(&composed).expose(),
            normalise(&decomposed).expose(),
            "NFC must make the two input methods agree"
        );
    }

    #[test]
    fn nothing_is_truncated() {
        let long = "x".repeat(MAX_LENGTH);
        let accepted = validate(&Secret::new(long.clone()), never, never);
        // Refused for being repetitive, not silently cut down.
        assert!(matches!(accepted, Err(Rejection::Blocked(_))));

        let varied: String = (0..MAX_LENGTH)
            .map(|i| char::from(b'a' + u8::try_from(i % 26).unwrap()))
            .collect();
        // A 256-character non-repeating string is a keyboard/alphabet run.
        assert!(validate(&Secret::new(varied), never, never).is_err());
    }

    #[test]
    fn beyond_the_maximum_is_refused_before_any_hashing() {
        let enormous = "a".repeat(MAX_BYTES + 1);
        assert_eq!(
            validate(
                &Secret::new(enormous),
                |_| panic!("must not hash"),
                |_| { panic!("must not hash") }
            )
            .unwrap_err(),
            Rejection::TooLong
        );
    }

    #[test]
    fn no_composition_rule_is_imposed() {
        // All lowercase, no digits, no symbols — and perfectly acceptable.
        assert!(ok("uma frase inteiramente em letras minusculas").is_ok());
    }

    #[test]
    fn the_temporary_credential_cannot_be_reused_as_the_permanent_one() {
        let result = validate(
            &Secret::new("a chuva em Camama cai devagar"),
            |_| true,
            never,
        );
        assert_eq!(result.unwrap_err(), Rejection::SameAsTemporary);
    }

    #[test]
    fn the_current_password_cannot_be_set_again() {
        let result = validate(&Secret::new("a chuva em Camama cai devagar"), never, |_| {
            true
        });
        assert_eq!(result.unwrap_err(), Rejection::SameAsCurrent);
    }

    #[test]
    fn a_hopeless_candidate_never_reaches_the_hash_comparisons() {
        // Both closures panic; a short password must be refused before them.
        let result = validate(
            &Secret::new("short"),
            |_| panic!("must not verify"),
            |_| panic!("must not verify"),
        );
        assert_eq!(result.unwrap_err(), Rejection::TooShort);
    }

    #[test]
    fn the_indicator_reports_three_honest_states() {
        assert_eq!(assess(&Secret::new("short")), Strength::TooShort);
        assert_eq!(assess(&Secret::new("password123456789")), Strength::Blocked);
        assert_eq!(
            assess(&Secret::new("a chuva em Camama cai devagar")),
            Strength::Acceptable
        );
    }

    #[test]
    fn rejection_messages_never_echo_the_candidate() {
        for rejection in [
            Rejection::TooShort,
            Rejection::TooLong,
            Rejection::Blocked(BlockReason::KnownPassword),
            Rejection::SameAsTemporary,
            Rejection::SameAsCurrent,
        ] {
            let message = rejection.message();
            assert!(!message.is_empty());
            assert!(!message.contains("password123"));
        }
    }
}
