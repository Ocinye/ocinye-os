//! Argon2id password verifiers.
//!
//! # What is stored
//!
//! A PHC string: `$argon2id$v=19$m=...,t=...,p=...$<salt>$<hash>`. It carries
//! the algorithm, the version, every cost parameter and a unique random salt
//! alongside the digest. That format is what makes [`needs_rehash`] possible —
//! the parameters a hash was made with are readable from the hash itself, so
//! raising them later costs one transparent rehash at next sign-in rather than
//! a forced reset for everybody (briefing §32).
//!
//! # Parameters
//!
//! The defaults follow OWASP's Argon2id guidance: 19 MiB of memory, two
//! passes, one lane. They are configurable because the right cost depends on
//! the machine, and `docs/security/` carries the benchmark procedure. They are
//! not hardcoded silently.
//!
//! # No pepper, for now
//!
//! A server-side pepper is not used. It would defend against a database-only
//! leak, which is a real threat, but only while the pepper itself stays out of
//! the same backup — and Ocinye has one host, one backup procedure and no
//! secrets manager yet (`CLAUDE.md` §1). Introducing one now would add a
//! rotation problem without the isolation that makes it pay. The PHC format
//! plus [`needs_rehash`] leaves the door open: adding a pepper later is a
//! parameter change plus transparent rehashing. Recorded in ADR-0104.

use argon2::{Algorithm, Argon2, Params, Version};
// O gerador do sal vem do `password_hash`, e não do crate `rand` autónomo.
//
// Não é preferência: são duas linhagens de `rand_core` diferentes. O `argon2`
// fala a versão que o `password-hash` traz consigo, e um `OsRng` do `rand`
// autónomo — que segue a sua própria cadência de versões — deixa de a
// satisfazer assim que as duas divergem. Tirá-lo daqui faz com que o sal
// acompanhe sempre o algoritmo que o consome.
use password_hash::rand_core::OsRng;
use password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};

use super::secret::Secret;
use crate::error::{CoreError, CoreResult};

/// Cost parameters for Argon2id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HashingParams {
    /// Memory cost in kibibytes.
    pub memory_kib: u32,
    /// Number of passes.
    pub iterations: u32,
    /// Degree of parallelism.
    pub parallelism: u32,
}

impl Default for HashingParams {
    /// OWASP's recommended Argon2id baseline: m=19456 (19 MiB), t=2, p=1.
    fn default() -> Self {
        Self {
            memory_kib: 19 * 1024,
            iterations: 2,
            parallelism: 1,
        }
    }
}

impl HashingParams {
    /// Reject parameters that would make hashing pointless.
    ///
    /// A misconfigured environment variable must fail at startup, not quietly
    /// produce weak verifiers for years (`CLAUDE.md` §55).
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Configuration`] when a parameter is below the floor.
    pub fn validate(self) -> CoreResult<Self> {
        // Floors, not recommendations: below these the hash is not doing its job.
        const MIN_MEMORY_KIB: u32 = 8 * 1024;
        const MIN_ITERATIONS: u32 = 2;

        if self.memory_kib < MIN_MEMORY_KIB {
            return Err(CoreError::Configuration(format!(
                "OCINYE_ARGON2_MEMORY_KIB must be at least {MIN_MEMORY_KIB}"
            )));
        }
        if self.iterations < MIN_ITERATIONS {
            return Err(CoreError::Configuration(format!(
                "OCINYE_ARGON2_ITERATIONS must be at least {MIN_ITERATIONS}"
            )));
        }
        if self.parallelism == 0 {
            return Err(CoreError::Configuration(
                "OCINYE_ARGON2_PARALLELISM must be at least 1".to_owned(),
            ));
        }
        Ok(self)
    }
}

/// Hashes and verifies password verifiers.
#[derive(Debug, Clone)]
pub struct Hasher {
    params: HashingParams,
}

impl Default for Hasher {
    fn default() -> Self {
        Self::new(HashingParams::default())
    }
}

impl Hasher {
    /// Build a hasher with the given parameters.
    #[must_use]
    pub const fn new(params: HashingParams) -> Self {
        Self { params }
    }

    /// The parameters in force.
    #[must_use]
    pub const fn params(&self) -> HashingParams {
        self.params
    }

    fn argon2(&self) -> CoreResult<Argon2<'static>> {
        let params = Params::new(
            self.params.memory_kib,
            self.params.iterations,
            self.params.parallelism,
            None,
        )
        .map_err(|error| CoreError::Configuration(format!("argon2 parameters: {error}")))?;

        Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
    }

    /// Produce a PHC verifier string for a password.
    ///
    /// The salt is drawn from the operating system CSPRNG, per hash.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Internal`] if hashing fails, which in practice
    /// means the parameters were rejected.
    pub fn hash(&self, password: &Secret) -> CoreResult<String> {
        let salt = SaltString::generate(&mut OsRng);
        self.argon2()?
            .hash_password(password.expose().as_bytes(), &salt)
            .map(|hash| hash.to_string())
            .map_err(|error| CoreError::Internal(format!("password hashing failed: {error}")))
    }

    /// Verify a password against a stored verifier.
    ///
    /// Returns `false` for a malformed stored hash rather than an error: a row
    /// that cannot be parsed must behave exactly like a wrong password, or the
    /// endpoint becomes an oracle for which accounts have broken records.
    ///
    /// The comparison itself is constant-time inside `argon2`.
    #[must_use]
    pub fn verify(&self, password: &Secret, stored: &str) -> bool {
        let Ok(parsed) = PasswordHash::new(stored) else {
            return false;
        };
        // Verification reads the parameters from the stored hash, not from
        // `self` — that is what lets an old hash keep verifying after the
        // configured cost has been raised.
        Argon2::default()
            .verify_password(password.expose().as_bytes(), &parsed)
            .is_ok()
    }

    /// Whether a stored verifier was produced with weaker parameters than the
    /// ones now in force.
    ///
    /// Called after a *successful* verification, when the plaintext is briefly
    /// available and can be rehashed without troubling the person.
    #[must_use]
    pub fn needs_rehash(&self, stored: &str) -> bool {
        let Ok(parsed) = PasswordHash::new(stored) else {
            // Unparseable: replace it at the first opportunity.
            return true;
        };

        if parsed.algorithm.as_str() != "argon2id" {
            return true;
        }

        let Ok(params) = Params::try_from(&parsed) else {
            return true;
        };

        params.m_cost() < self.params.memory_kib
            || params.t_cost() < self.params.iterations
            || params.p_cost() < self.params.parallelism
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cheap parameters so the suite stays fast. Never used outside tests.
    fn fast() -> Hasher {
        Hasher::new(HashingParams {
            memory_kib: 8 * 1024,
            iterations: 2,
            parallelism: 1,
        })
    }

    #[test]
    fn a_password_verifies_against_its_own_hash() {
        let hasher = fast();
        let password = Secret::new("a chuva em Camama cai devagar");
        let stored = hasher.hash(&password).unwrap();
        assert!(hasher.verify(&password, &stored));
    }

    #[test]
    fn a_different_password_does_not_verify() {
        let hasher = fast();
        let stored = hasher
            .hash(&Secret::new("a chuva em Camama cai devagar"))
            .unwrap();
        assert!(!hasher.verify(&Secret::new("a chuva em Camama cai depressa"), &stored));
        assert!(!hasher.verify(&Secret::new(""), &stored));
    }

    #[test]
    fn the_same_password_hashes_differently_every_time() {
        let hasher = fast();
        let password = Secret::new("a chuva em Camama cai devagar");
        let first = hasher.hash(&password).unwrap();
        let second = hasher.hash(&password).unwrap();
        assert_ne!(first, second, "the salt is not unique per hash");
        assert!(hasher.verify(&password, &first));
        assert!(hasher.verify(&password, &second));
    }

    #[test]
    fn the_stored_form_is_a_phc_string_that_names_argon2id() {
        let stored = fast()
            .hash(&Secret::new("a chuva em Camama cai devagar"))
            .unwrap();
        assert!(stored.starts_with("$argon2id$"), "{stored}");
        assert!(stored.contains("v=19"));
        assert!(stored.contains("m=8192"));
        assert!(stored.contains("t=2"));
    }

    #[test]
    fn the_plaintext_never_appears_in_the_stored_form() {
        let password = "a chuva em Camama cai devagar";
        let stored = fast().hash(&Secret::new(password)).unwrap();
        assert!(!stored.contains(password));
        assert!(!stored.contains("chuva"));
    }

    #[test]
    fn a_malformed_stored_hash_behaves_like_a_wrong_password() {
        let hasher = fast();
        let password = Secret::new("a chuva em Camama cai devagar");
        for stored in ["", "not-a-hash", "$argon2id$broken", "$2y$10$abcdef"] {
            assert!(
                !hasher.verify(&password, stored),
                "{stored:?} should verify as false, not panic"
            );
        }
    }

    #[test]
    fn raising_the_cost_marks_older_hashes_for_rehashing() {
        let weak = Hasher::new(HashingParams {
            memory_kib: 8 * 1024,
            iterations: 2,
            parallelism: 1,
        });
        let stronger = Hasher::new(HashingParams {
            memory_kib: 19 * 1024,
            iterations: 3,
            parallelism: 1,
        });

        let password = Secret::new("a chuva em Camama cai devagar");
        let old = weak.hash(&password).unwrap();

        assert!(
            !weak.needs_rehash(&old),
            "current parameters need no rehash"
        );
        assert!(stronger.needs_rehash(&old), "weaker hash must be flagged");

        // And the old hash still verifies, so nobody is locked out by the change.
        assert!(stronger.verify(&password, &old));
    }

    #[test]
    fn an_unparseable_or_foreign_hash_is_always_flagged_for_rehash() {
        let hasher = fast();
        assert!(hasher.needs_rehash("garbage"));
        assert!(hasher.needs_rehash("$2y$10$abcdefghijklmnopqrstuv"));
    }

    #[test]
    fn parameters_below_the_floor_are_refused_at_configuration_time() {
        assert!(HashingParams {
            memory_kib: 1024,
            iterations: 2,
            parallelism: 1
        }
        .validate()
        .is_err());
        assert!(HashingParams {
            memory_kib: 19 * 1024,
            iterations: 1,
            parallelism: 1
        }
        .validate()
        .is_err());
        assert!(HashingParams {
            memory_kib: 19 * 1024,
            iterations: 2,
            parallelism: 0
        }
        .validate()
        .is_err());
        assert!(HashingParams::default().validate().is_ok());
    }

    #[test]
    fn long_passphrases_and_unicode_hash_and_verify() {
        let hasher = fast();
        for password in [
            "ξ-spectral density of the sample at 4.2 kelvin, measured twice",
            "  espaços à frente e atrás fazem parte da palavra-passe  ",
            &"漢字とひらがなとカタカナ".repeat(4),
        ] {
            let secret = Secret::new(password);
            let stored = hasher.hash(&secret).unwrap();
            assert!(hasher.verify(&secret, &stored), "failed for {password:?}");
        }
    }
}
