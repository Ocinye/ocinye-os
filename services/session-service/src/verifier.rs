//! The local verifier: what lets a session be unlocked without the Core.
//!
//! An Argon2id verifier (PHC string) of the password the member typed at
//! sign-in. It is one-way and memory-hard: it cannot be turned back into the
//! password, and it is not a Core credential — nothing but this service
//! accepts it, and only for the one session it was made for. It is destroyed
//! with the session.
//!
//! The cost is not a constant of the system. [`Cost::calibrate`] measures this
//! machine and picks the largest memory cost that still verifies within the
//! target time, never below the floor.

use std::time::{Duration, Instant};

use argon2::{Algorithm, Argon2, Params, Version};
use password_hash::rand_core::OsRng;
use password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};

/// Argon2id cost parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cost {
    /// Memory, in kibibytes.
    pub memory_kib: u32,
    /// Passes.
    pub passes: u32,
    /// Lanes.
    pub lanes: u32,
}

impl Cost {
    /// The floor: the OWASP minimum for Argon2id (19 MiB, 2 passes, 1 lane).
    /// No machine is given less.
    pub const FLOOR: Self = Self {
        memory_kib: 19 * 1024,
        passes: 2,
        lanes: 1,
    };
    /// The ceiling calibration will not exceed: an unlock must not starve the
    /// machine it unlocks.
    pub const CEILING_KIB: u32 = 256 * 1024;

    /// Measure this machine: double the memory cost from the floor while one
    /// verification stays within `target`, up to the ceiling.
    #[must_use]
    pub fn calibrate(target: Duration) -> Self {
        let mut cost = Self::FLOOR;
        loop {
            let next = Self {
                memory_kib: cost.memory_kib.saturating_mul(2),
                ..cost
            };
            if next.memory_kib > Self::CEILING_KIB {
                return cost;
            }
            let started = Instant::now();
            if create("calibration", next).is_none() || started.elapsed() > target {
                return cost;
            }
            cost = next;
        }
    }

    fn hasher(self) -> Option<Argon2<'static>> {
        let params = Params::new(self.memory_kib, self.passes, self.lanes, None).ok()?;
        Some(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
    }
}

/// Make a verifier for a password. `None` only if the parameters are invalid.
#[must_use]
pub fn create(password: &str, cost: Cost) -> Option<String> {
    let salt = SaltString::generate(&mut OsRng);
    cost.hasher()?
        .hash_password(password.as_bytes(), &salt)
        .ok()
        .map(|h| h.to_string())
}

/// Whether a password matches a stored verifier. A verifier that does not
/// parse matches nothing.
#[must_use]
pub fn matches(password: &str, stored: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(stored) else {
        return false;
    };
    // The parameters come from the PHC string itself, so a verifier made with
    // one cost still verifies after the machine is recalibrated.
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fast parameters for tests only: the property under test is the logic,
    /// not the cost.
    pub const TEST: Cost = Cost {
        memory_kib: 64,
        passes: 1,
        lanes: 1,
    };

    #[test]
    fn o_verificador_aceita_a_palavra_passe_e_so_ela() {
        let v = create("correct horse", TEST).unwrap();
        assert!(matches("correct horse", &v));
        assert!(!matches("correct horsf", &v));
        assert!(!matches("", &v));
    }

    #[test]
    fn o_verificador_nao_contem_a_palavra_passe_e_e_argon2id() {
        let v = create("correct horse", TEST).unwrap();
        assert!(v.starts_with("$argon2id$"));
        assert!(!v.contains("correct"));
    }

    #[test]
    fn dois_verificadores_da_mesma_palavra_passe_diferem() {
        assert_ne!(create("x", TEST).unwrap(), create("x", TEST).unwrap());
    }

    #[test]
    fn lixo_nao_verifica_nada() {
        assert!(!matches("x", "not a phc string"));
        assert!(!matches("x", ""));
    }

    #[test]
    fn a_calibracao_nunca_desce_do_minimo() {
        let c = Cost::calibrate(Duration::from_nanos(1));
        assert_eq!(c, Cost::FLOOR);
    }
}
