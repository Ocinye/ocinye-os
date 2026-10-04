//! The Installer's own known-hosts store (D011_SECURITY_MATRIX #1–#3).
//!
//! Not `~/.ssh/known_hosts`: the Installer pins the fingerprints it was told to
//! trust, in its own file, and never mutates the operator's SSH configuration.
//!
//! - **first contact**: no session is authenticated with an unknown key. The
//!   connection stops after the key exchange, the operator sees host,
//!   algorithm and fingerprint, and only an explicit «Confiar neste servidor»
//!   pins it;
//! - **match**: proceed;
//! - **mismatch**: hard stop. There is no «continue anyway». The only exit is
//!   [`KnownHosts::forget`], which the UI exposes as a governed action that
//!   shows both fingerprints and asks again.

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// A pinned host key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pinned {
    /// `ssh-ed25519`, `ecdsa-sha2-nistp256`, …
    pub algorithm: String,
    /// `SHA256:<base64>`.
    pub fingerprint: String,
    /// RFC 3339.
    pub first_seen: String,
}

/// What the store says about a presented key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostKeyDecision {
    /// Pinned and equal.
    Known,
    /// Never seen: the operator decides.
    FirstContact {
        /// Algorithm.
        algorithm: String,
        /// Fingerprint.
        fingerprint: String,
    },
    /// Pinned and different: hard stop.
    Mismatch {
        /// What was pinned.
        known: Pinned,
        /// What was presented.
        presented_algorithm: String,
        /// What was presented.
        presented_fingerprint: String,
    },
}

/// `host:port` → pinned key.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnownHosts {
    #[serde(skip)]
    path: Option<PathBuf>,
    hosts: BTreeMap<String, Pinned>,
}

/// The store key of a target.
#[must_use]
pub fn key_of(host: &str, port: u16) -> String {
    format!("{host}:{port}")
}

impl KnownHosts {
    /// Load from a file (missing → empty).
    #[must_use]
    pub fn load(path: PathBuf) -> Self {
        let mut s: Self = fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        s.path = Some(path);
        s
    }

    /// In memory only (tests).
    #[must_use]
    pub fn in_memory() -> Self {
        Self::default()
    }

    /// Decide about a presented key.
    #[must_use]
    pub fn decide(&self, target: &str, algorithm: &str, fingerprint: &str) -> HostKeyDecision {
        match self.hosts.get(target) {
            Some(p) if p.fingerprint == fingerprint && p.algorithm == algorithm => {
                HostKeyDecision::Known
            }
            Some(p) => HostKeyDecision::Mismatch {
                known: p.clone(),
                presented_algorithm: algorithm.to_owned(),
                presented_fingerprint: fingerprint.to_owned(),
            },
            None => HostKeyDecision::FirstContact {
                algorithm: algorithm.to_owned(),
                fingerprint: fingerprint.to_owned(),
            },
        }
    }

    /// Pin after the operator's explicit trust. Refuses to overwrite a
    /// different pin: that path is [`Self::forget`] first.
    ///
    /// # Errors
    ///
    /// When a different key is already pinned, or the file cannot be written.
    pub fn trust(
        &mut self,
        target: &str,
        algorithm: &str,
        fingerprint: &str,
    ) -> std::io::Result<()> {
        if let Some(p) = self.hosts.get(target) {
            if p.fingerprint != fingerprint {
                return Err(std::io::Error::other("HOST_KEY_MISMATCH"));
            }
            return Ok(());
        }
        self.hosts.insert(
            target.to_owned(),
            Pinned {
                algorithm: algorithm.to_owned(),
                fingerprint: fingerprint.to_owned(),
                first_seen: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            },
        );
        self.save()
    }

    /// The governed «Esquecer servidor»: removes the pin so the next connection
    /// is a first contact again, with the new fingerprint on screen.
    ///
    /// # Errors
    ///
    /// When the file cannot be written.
    pub fn forget(&mut self, target: &str) -> std::io::Result<()> {
        self.hosts.remove(target);
        self.save()
    }

    /// The pin, if any.
    #[must_use]
    pub fn pinned(&self, target: &str) -> Option<&Pinned> {
        self.hosts.get(target)
    }

    fn save(&self) -> std::io::Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("tmp");
        fs::write(
            &tmp,
            serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?,
        )?;
        fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600))?;
        fs::rename(tmp, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primeiro_contacto_confianca_e_depois_reconhecido() {
        let mut k = KnownHosts::in_memory();
        let t = key_of("192.0.2.10", 22);
        assert!(matches!(
            k.decide(&t, "ssh-ed25519", "SHA256:A"),
            HostKeyDecision::FirstContact { .. }
        ));
        k.trust(&t, "ssh-ed25519", "SHA256:A").unwrap();
        assert_eq!(
            k.decide(&t, "ssh-ed25519", "SHA256:A"),
            HostKeyDecision::Known
        );
    }

    #[test]
    fn uma_chave_diferente_e_paragem_e_nao_se_sobrepoe() {
        let mut k = KnownHosts::in_memory();
        let t = key_of("192.0.2.10", 22);
        k.trust(&t, "ssh-ed25519", "SHA256:A").unwrap();
        assert!(matches!(
            k.decide(&t, "ssh-ed25519", "SHA256:B"),
            HostKeyDecision::Mismatch { .. }
        ));
        // Confiar outra vez não substitui a chave fixada.
        assert!(k.trust(&t, "ssh-ed25519", "SHA256:B").is_err());
        assert!(matches!(
            k.decide(&t, "ssh-ed25519", "SHA256:B"),
            HostKeyDecision::Mismatch { .. }
        ));
        // Só esquecendo (acção governada) volta a ser um primeiro contacto.
        k.forget(&t).unwrap();
        assert!(matches!(
            k.decide(&t, "ssh-ed25519", "SHA256:B"),
            HostKeyDecision::FirstContact { .. }
        ));
    }

    #[test]
    fn o_algoritmo_tambem_conta() {
        let mut k = KnownHosts::in_memory();
        let t = key_of("h.test", 22);
        k.trust(&t, "ssh-ed25519", "SHA256:A").unwrap();
        assert!(matches!(
            k.decide(&t, "ecdsa-sha2-nistp256", "SHA256:A"),
            HostKeyDecision::Mismatch { .. }
        ));
    }

    #[test]
    fn o_ficheiro_e_so_do_operador() {
        let dir = std::env::temp_dir().join(format!("ocinye-kh-{}", std::process::id()));
        let path = dir.join("known_hosts.json");
        let mut k = KnownHosts::load(path.clone());
        k.trust("h.test:22", "ssh-ed25519", "SHA256:A").unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let again = KnownHosts::load(path);
        assert_eq!(
            again.decide("h.test:22", "ssh-ed25519", "SHA256:A"),
            HostKeyDecision::Known
        );
        let _ = fs::remove_dir_all(dir);
    }
}
