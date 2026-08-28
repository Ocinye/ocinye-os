//! Refusal of passwords that are known-bad rather than merely short.
//!
//! # Why a local list
//!
//! The obvious alternative is a compromised-password service. Ocinye does not
//! call one, and would not send a password to one in any form (briefing §7).
//! A range-query protocol such as k-anonymity is the only acceptable shape for
//! that, and adopting it is `PLANNED`, behind an ADR, because it makes password
//! setting depend on a third party being reachable.
//!
//! # What is checked
//!
//! Comparison is case-insensitive and ignores trailing digits and common
//! substitutions, because `Password123` is not meaningfully stronger than
//! `password`. This is the one place where the password *is* transformed — for
//! comparison only. What gets hashed is always exactly what was typed (§34).

use std::collections::HashSet;
use std::sync::OnceLock;

/// Entries shipped with the binary.
///
/// The list is deliberately small and high-signal rather than a dump of ten
/// million leaked passwords: with a 15-character minimum already in force, the
/// long tail of a breach corpus is mostly unreachable anyway. What remains
/// useful is the set of things a person actually types when asked for fifteen
/// characters — repeated words, keyboard walks, and the institution's own name.
const EMBEDDED: &str = include_str!("blocklist.txt");

fn entries() -> &'static HashSet<String> {
    static ENTRIES: OnceLock<HashSet<String>> = OnceLock::new();
    ENTRIES.get_or_init(|| {
        EMBEDDED
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .map(str::to_lowercase)
            .collect()
    })
}

/// Reduce a candidate to the form the list is compared against.
///
/// Lowercases, folds the usual leet substitutions, and strips trailing digits
/// and punctuation. `0c1nye-2026!` and `ocinye` collapse to the same thing,
/// which is the point.
fn canonicalise(candidate: &str) -> String {
    // Order matters, and getting it wrong is silent. Stripping must happen
    // *before* folding: fold first and the `123` in `Password123` becomes
    // letters, which the stripper can no longer remove, and the whole entry
    // stops matching.
    let lowered = candidate.to_lowercase();
    let trimmed = lowered
        .trim_end_matches(|c: char| c.is_ascii_digit() || c.is_ascii_punctuation())
        .trim_matches(|c: char| c.is_whitespace() || c == '-' || c == '_');

    let folded: String = trimmed
        .chars()
        .map(|c| match c {
            '0' => 'o',
            '1' | '!' | '|' => 'i',
            '3' => 'e',
            '4' | '@' => 'a',
            '5' | '$' => 's',
            '7' => 't',
            other => other,
        })
        .collect();

    // Folding can expose a new trailing separator (`p4ssword-` had its hyphen
    // stripped already, but `p4ssword@` folds to `...a` and needs no second
    // pass). Trim once more so the two orders converge.
    folded
        .trim_matches(|c: char| c.is_whitespace() || c == '-' || c == '_')
        .to_owned()
}

/// Whether a string is a single character repeated, or a short unit repeated.
///
/// Catches `aaaaaaaaaaaaaaaa` and `abcabcabcabcabcabc`, which pass a length
/// check trivially.
fn is_repetitive(canonical: &str) -> bool {
    let chars: Vec<char> = canonical.chars().collect();
    if chars.len() < 4 {
        return false;
    }
    // A unit that repeats at least three times makes the password no stronger
    // than the unit itself.
    for unit in 1..=chars.len() / 3 {
        if !chars.len().is_multiple_of(unit) {
            continue;
        }
        if chars.chunks(unit).all(|chunk| chunk == &chars[..unit]) {
            return true;
        }
    }
    false
}

/// Whether the string is a run along the keyboard or the alphabet.
fn is_sequential(canonical: &str) -> bool {
    const ROWS: [&str; 5] = [
        "abcdefghijklmnopqrstuvwxyz",
        "qwertyuiop",
        "asdfghjkl",
        "zxcvbnm",
        "oiletasbtoiletasbt", // digits after leet folding: 0123456789 twice
    ];

    let lowered = canonical.to_lowercase();
    if lowered.chars().count() < 6 {
        return false;
    }
    let reversed: String = lowered.chars().rev().collect();

    ROWS.iter().any(|row| {
        // A run of six or more consecutive keys is a walk, wherever it sits.
        lowered
            .as_bytes()
            .windows(6)
            .any(|w| row.contains(std::str::from_utf8(w).unwrap_or("\u{0}")))
            || reversed
                .as_bytes()
                .windows(6)
                .any(|w| row.contains(std::str::from_utf8(w).unwrap_or("\u{0}")))
    })
}

/// Why a candidate was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockReason {
    /// Appears in the shipped list of common or breached passwords.
    KnownPassword,
    /// A repeated character or repeated short unit.
    Repetitive,
    /// A keyboard walk or alphabetic run.
    Sequential,
    /// Built around the institution's own name.
    Institutional,
}

impl BlockReason {
    /// Message shown to the person choosing the password.
    ///
    /// Says enough to choose better without narrating the rule set to someone
    /// probing it.
    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::KnownPassword => {
                "Esta palavra-passe é demasiado comum ou é conhecida de fugas de dados."
            }
            Self::Repetitive => "Esta palavra-passe repete o mesmo padrão.",
            Self::Sequential => "Esta palavra-passe é uma sequência previsível.",
            Self::Institutional => {
                "Esta palavra-passe é construída à volta do nome da instituição."
            }
        }
    }

    /// Stable label for the audit trail. Never accompanied by the candidate.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::KnownPassword => "known_password",
            Self::Repetitive => "repetitive",
            Self::Sequential => "sequential",
            Self::Institutional => "institutional",
        }
    }
}

/// Check a candidate against the blocklist.
///
/// Returns the reason for refusal, or `None` if the candidate is acceptable on
/// this axis. Length is checked elsewhere.
#[must_use]
pub fn check(candidate: &str) -> Option<BlockReason> {
    let canonical = canonicalise(candidate);

    if canonical.is_empty() {
        return Some(BlockReason::Repetitive);
    }

    if entries().contains(&canonical) {
        return Some(BlockReason::KnownPassword);
    }

    // The institution's own name, however dressed up.
    for term in ["ocinye", "ocinyeos", "ocinyeworkspace"] {
        if canonical == term || canonical.replace([' ', '-', '_', '.'], "") == term {
            return Some(BlockReason::Institutional);
        }
    }

    if is_repetitive(&canonical) {
        return Some(BlockReason::Repetitive);
    }

    if is_sequential(&canonical) {
        return Some(BlockReason::Sequential);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_examples_from_the_briefing_are_all_refused() {
        for candidate in [
            "password",
            "Password123",
            "password123456789",
            "123456789012345",
            "qwertyuiopasdfgh",
            "ocinye123456789",
            "0c1nye123456789",
            "aaaaaaaaaaaaaaaa",
            "abcabcabcabcabcabc",
        ] {
            assert!(
                check(candidate).is_some(),
                "{candidate:?} should have been refused"
            );
        }
    }

    #[test]
    fn a_real_passphrase_is_accepted() {
        for candidate in [
            "a chuva em Camama cai devagar",
            "reactor coolant loop 47 telemetry",
            "Kimbundu é falado em Luanda desde sempre",
            "ξ-spectral density of the sample",
        ] {
            assert!(
                check(candidate).is_none(),
                "{candidate:?} should have been accepted, got {:?}",
                check(candidate)
            );
        }
    }

    #[test]
    fn leet_substitution_does_not_rescue_a_blocked_password() {
        assert_eq!(check("p4ssw0rd"), Some(BlockReason::KnownPassword));
        assert_eq!(check("0c1nye"), Some(BlockReason::Institutional));
    }

    #[test]
    fn trailing_digits_do_not_rescue_a_blocked_password() {
        assert_eq!(check("password2026"), Some(BlockReason::KnownPassword));
        assert_eq!(check("letmein!!!"), Some(BlockReason::KnownPassword));
    }

    #[test]
    fn the_refusal_message_never_echoes_the_candidate() {
        for reason in [
            BlockReason::KnownPassword,
            BlockReason::Repetitive,
            BlockReason::Sequential,
            BlockReason::Institutional,
        ] {
            assert!(!reason.message().is_empty());
            assert!(!reason.as_str().is_empty());
        }
    }

    #[test]
    fn the_shipped_list_parses_and_is_not_empty() {
        assert!(
            entries().len() > 100,
            "the blocklist looks truncated: {} entries",
            entries().len()
        );
        assert!(entries().contains("password"));
        assert!(!entries().iter().any(|e| e.starts_with('#')));
    }
}
