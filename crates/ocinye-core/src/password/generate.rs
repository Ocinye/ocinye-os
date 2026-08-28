//! Generation of temporary credentials.
//!
//! The administrator does not invent these (briefing §16). A human-chosen
//! "temporary" password is a permanent password in waiting, and it is chosen
//! under exactly the conditions that produce `Ocinye2026!`.

use rand::rand_core::UnwrapErr;
use rand::rngs::SysRng;
use rand::RngExt;

use super::secret::Secret;

/// Alphabet for generated credentials.
///
/// Unambiguous by construction: no `0`/`O`, no `1`/`l`/`I`. These are read off
/// a screen and typed by hand or dictated over the phone, and a credential that
/// cannot be transcribed reliably becomes a support call — or worse, gets
/// pasted somewhere durable.
const ALPHABET: &[u8] = b"abcdefghijkmnpqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789";

/// Number of groups in a generated credential.
const GROUPS: usize = 6;

/// Characters per group.
const GROUP_LENGTH: usize = 4;

/// Entropy of one generated credential, in bits.
///
/// 24 characters from a 55-symbol alphabet: `24 * log2(55)` ≈ 138.8 bits. Far
/// beyond the "20–24 random characters" the briefing asks for, and beyond any
/// offline attack that matters within the 24-hour validity window.
#[must_use]
pub fn entropy_bits() -> f64 {
    #[expect(
        clippy::cast_precision_loss,
        reason = "contagens pequenas e conhecidas: o alfabeto e o comprimento"
    )]
    let alphabet = ALPHABET.len() as f64;
    #[expect(
        clippy::cast_precision_loss,
        reason = "contagens pequenas e conhecidas: o alfabeto e o comprimento"
    )]
    let length = (GROUPS * GROUP_LENGTH) as f64;
    length * alphabet.log2()
}

/// Generate a temporary credential.
///
/// Drawn from the operating system CSPRNG. Grouped with hyphens purely so a
/// human can read it back accurately; the hyphens are part of the credential.
///
/// Nothing about the person is used as input — not the name, the username, the
/// email nor the date (briefing §16).
#[must_use]
pub fn temporary_credential() -> Secret {
    // `UnwrapErr` dá a face infalível do gerador do sistema: `random_range`
    // precisa de um `Rng` que não falhe, e a falta de entropia passa a ser um
    // pânico em vez de uma palavra-passe fraca gerada em silêncio.
    let mut rng = UnwrapErr(SysRng);
    let mut out = String::with_capacity(GROUPS * (GROUP_LENGTH + 1));

    for group in 0..GROUPS {
        if group > 0 {
            out.push('-');
        }
        for _ in 0..GROUP_LENGTH {
            // `random_range` over the slice index is uniform; modulo bias
            // would not be, and at this size it would be measurable.
            let index = rng.random_range(0..ALPHABET.len());
            out.push(char::from(ALPHABET[index]));
        }
    }

    Secret::new(out)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn a_generated_credential_has_the_documented_shape() {
        let credential = temporary_credential();
        let value = credential.expose();
        let groups: Vec<&str> = value.split('-').collect();

        assert_eq!(groups.len(), GROUPS);
        for group in groups {
            assert_eq!(group.chars().count(), GROUP_LENGTH);
        }
        assert_eq!(value.chars().count(), GROUPS * GROUP_LENGTH + GROUPS - 1);
    }

    #[test]
    fn a_generated_credential_clears_the_permanent_password_length_rule() {
        // It has to: the holder signs in with it before setting their own.
        let credential = temporary_credential();
        assert!(credential.expose().chars().count() >= super::super::policy::MIN_LENGTH);
    }

    #[test]
    fn generated_credentials_do_not_repeat() {
        let mut seen = HashSet::new();
        for _ in 0..2_000 {
            let credential = temporary_credential();
            assert!(
                seen.insert(credential.expose().to_owned()),
                "the generator repeated itself"
            );
        }
    }

    #[test]
    fn the_alphabet_excludes_characters_that_are_read_wrongly() {
        let alphabet = std::str::from_utf8(ALPHABET).unwrap();
        for ambiguous in ['0', 'O', '1', 'l', 'I'] {
            assert!(
                !alphabet.contains(ambiguous),
                "{ambiguous:?} is ambiguous when transcribed"
            );
        }
        // And no duplicates, which would skew the distribution.
        let unique: HashSet<u8> = ALPHABET.iter().copied().collect();
        assert_eq!(unique.len(), ALPHABET.len());
    }

    #[test]
    fn entropy_exceeds_the_briefing_requirement() {
        // The briefing asks for the strength of 20–24 random characters.
        assert!(
            entropy_bits() > 128.0,
            "only {:.1} bits of entropy",
            entropy_bits()
        );
    }

    #[test]
    fn every_alphabet_character_is_reachable() {
        // A generator that silently never emits part of its alphabet has less
        // entropy than advertised.
        let mut seen = HashSet::new();
        for _ in 0..5_000 {
            for c in temporary_credential().expose().chars() {
                if c != '-' {
                    seen.insert(c);
                }
            }
        }
        assert_eq!(
            seen.len(),
            ALPHABET.len(),
            "some alphabet characters are never generated"
        );
    }

    #[test]
    fn a_generated_credential_is_not_blocklisted() {
        // It must be usable at the login endpoint, which shares the blocklist.
        for _ in 0..200 {
            let credential = temporary_credential();
            assert!(
                super::super::blocklist::check(credential.expose()).is_none(),
                "generated {:?} is blocklisted",
                credential.expose()
            );
        }
    }
}
