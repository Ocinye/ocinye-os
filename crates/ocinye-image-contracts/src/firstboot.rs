//! Machine identity and first-boot state (D013_FIRST_BOOT_MODEL.md).

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::manifest::{ImageFormat, ImageProfile, OcinyeImageVersion};

/// `SHA256:<base64 without padding>` of an SSH public key — OpenSSH's form.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct KeyFingerprint(pub String);

impl KeyFingerprint {
    /// Whether it has OpenSSH's SHA-256 shape (43 base64 characters).
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.0.strip_prefix("SHA256:").is_some_and(|b| {
            b.len() == 43
                && b.bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'+' || c == b'/')
        })
    }
}

/// Key types accepted for an SSH public key: login keys and host keys.
const KEY_TYPES: [&str; 4] = [
    "ssh-ed25519",
    "ecdsa-sha2-nistp256",
    "ecdsa-sha2-nistp384",
    "ssh-rsa",
];

/// A parsed OpenSSH public key line (`type base64 [comment]`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicKeyLine {
    /// `ssh-ed25519`, `ecdsa-sha2-nistp256`, …
    pub algorithm: String,
    /// The key blob.
    pub blob: Vec<u8>,
}

impl PublicKeyLine {
    /// Parse one line; the comment is dropped (it is operator text, not identity).
    /// Options before the type (`restrict,…`) are refused: this parses keys,
    /// not `authorized_keys` entries.
    #[must_use]
    pub fn parse(line: &str) -> Option<Self> {
        let mut it = line.split_ascii_whitespace();
        let algorithm = it.next()?;
        if !KEY_TYPES.contains(&algorithm) {
            return None;
        }
        let blob = base64::engine::general_purpose::STANDARD
            .decode(it.next()?)
            .ok()?;
        // The blob starts with the length-prefixed algorithm name.
        let n = u32::from_be_bytes(blob.get(..4)?.try_into().ok()?) as usize;
        if blob.get(4..4 + n)? != algorithm.as_bytes() {
            return None;
        }
        if algorithm == "ssh-rsa" && blob.len() < 384 {
            // RSA below 3072 bits: refused for login (D013_FIRST_BOOT_MODEL.md).
            return None;
        }
        Some(Self {
            algorithm: algorithm.to_owned(),
            blob,
        })
    }

    /// The OpenSSH SHA-256 fingerprint.
    #[must_use]
    pub fn fingerprint(&self) -> KeyFingerprint {
        let d = Sha256::digest(&self.blob);
        KeyFingerprint(format!(
            "SHA256:{}",
            base64::engine::general_purpose::STANDARD_NO_PAD.encode(d)
        ))
    }

    /// The line again, normalised (no comment, no options).
    #[must_use]
    pub fn to_line(&self) -> String {
        format!(
            "{} {}",
            self.algorithm,
            base64::engine::general_purpose::STANDARD.encode(&self.blob)
        )
    }
}

/// Human-verifiable machine fingerprint: a host key, shown in groups of 4.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MachineFingerprint {
    /// `ssh-ed25519` | `ecdsa-sha2-nistp256` (D011 accepts only these) [P, PD-17].
    pub algorithm: String,
    /// `SHA256:…`.
    pub fingerprint: KeyFingerprint,
}

impl MachineFingerprint {
    /// `SHA256: EXEM PLO0 …` — the exact grouping console and Installer render.
    #[must_use]
    pub fn grouped(&self) -> String {
        let b64 = self.fingerprint.0.trim_start_matches("SHA256:");
        let groups: Vec<String> = b64
            .as_bytes()
            .chunks(4)
            .map(|c| String::from_utf8_lossy(c).into_owned())
            .collect();
        format!("SHA256: {}", groups.join(" "))
    }
}

/// `ocb-xxxx-xxxx-xxxx-xxxx` — random 64-bit, non-secret correlation id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BootstrapId(pub String);

impl BootstrapId {
    /// From 8 random bytes.
    #[must_use]
    pub fn from_bytes(b: [u8; 8]) -> Self {
        let h = hex::encode(b);
        Self(format!(
            "ocb-{}-{}-{}-{}",
            &h[0..4],
            &h[4..8],
            &h[8..12],
            &h[12..16]
        ))
    }

    /// Whether it matches `ocb-[0-9a-f]{4}(-[0-9a-f]{4}){3}`.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        let Some(rest) = self.0.strip_prefix("ocb-") else {
            return false;
        };
        let parts: Vec<&str> = rest.split('-').collect();
        parts.len() == 4 && parts.iter().all(|p| crate::is_lower_hex(p, 4))
    }

    /// The four characters that suffix the default hostname.
    #[must_use]
    pub fn short(&self) -> &str {
        self.0.get(4..8).unwrap_or("0000")
    }
}

/// Per-machine identity created at first boot (never in an image).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MachineIdentity {
    /// systemd machine-id (32 hex). Not shown to operators.
    pub machine_id: String,
    /// Host keys present.
    pub host_keys: Vec<MachineFingerprint>,
    /// Bootstrap id.
    pub bootstrap_id: BootstrapId,
    /// RFC 3339.
    pub created_at: String,
}

/// First-boot stage results.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FirstBootState {
    /// F1–F5 running.
    InProgress {
        /// Stage.
        stage: FirstBootStage,
    },
    /// F6 done.
    Ready,
    /// Claim disabled; the console shows the reason.
    Failed {
        /// Why.
        error: FirstBootError,
    },
}

/// Stages F1–F6.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FirstBootStage {
    /// Machine id, host keys, bootstrap id.
    F1Identity,
    /// Grow the root filesystem (virt).
    F2GrowFs,
    /// Release payload and image ids against the embedded manifest.
    F3Integrity,
    /// cloud-init: hostname, keys, network (virt).
    F4Platform,
    /// DHCP / static.
    F5Network,
    /// `UNCLAIMED`, pairing code, claim access.
    F6Unclaimed,
}

/// Typed first-boot failures.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "code", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FirstBootError {
    /// F1.
    IdentityGenerationFailed {
        /// Which part.
        part: IdentityPart,
    },
    /// F2 (non-fatal for identity).
    RootfsExpansionFailed,
    /// F3: claim permanently disabled.
    ReleasePayloadTampered {
        /// Path.
        path: String,
    },
    /// F3.
    ImageContentManifestInvalid {
        /// Field.
        field: String,
    },
    /// F4: nothing applied from user data.
    CloudInitUserdataRejected {
        /// Field.
        field: String,
    },
    /// F6.
    ClaimServiceFailed,
}

/// Which identity part failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityPart {
    /// `/etc/machine-id`.
    MachineId,
    /// SSH host keys.
    SshHostKeys,
    /// `ocb-…`.
    BootstrapId,
}

/// Orthogonal network state (not a claim state).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NetworkState {
    /// No address yet.
    Waiting,
    /// DHCP lease.
    Dhcp,
    /// Static IPv4 from the console.
    Static,
}

/// Non-secret facts: `/etc/ocinye/image.json`, and what the claim protocol reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageFacts {
    /// Image identity.
    pub image: OcinyeImageVersion,
    /// Profile.
    pub profile: ImageProfile,
    /// Release id.
    pub release_id: String,
    /// Digest of the embedded `ImageContentManifest`.
    pub content_sha256: String,
    /// Ubuntu base serial.
    pub ubuntu_serial: String,
    /// QCOW2/RAW for the virt payload; ISO when OIE installed it.
    pub source_format: ImageFormat,
}

/// A cloud-init hostname: RFC 1123 labels, ≤ 63 per label, lowercase ASCII.
#[must_use]
pub fn is_valid_hostname(h: &str) -> bool {
    !h.is_empty()
        && h.len() <= 253
        && h.split('.').all(|l| {
            !l.is_empty()
                && l.len() <= 63
                && !l.starts_with('-')
                && !l.ends_with('-')
                && l.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        })
}

/// A static IPv4 the console accepts: address/prefix, gateway in the subnet,
/// DNS servers as plain addresses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StaticIpv4 {
    /// Interface name (as `ip link` names it).
    pub interface: String,
    /// `a.b.c.d`.
    pub address: std::net::Ipv4Addr,
    /// 1..=32.
    pub prefix: u8,
    /// In the subnet.
    pub gateway: std::net::Ipv4Addr,
    /// At most three.
    pub dns: Vec<std::net::Ipv4Addr>,
}

impl StaticIpv4 {
    /// Whether it is coherent.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        let iface_ok = !self.interface.is_empty()
            && self.interface.len() <= 15
            && self
                .interface
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b));
        let mask = if self.prefix == 0 {
            0
        } else {
            u32::MAX << (32 - u32::from(self.prefix))
        };
        iface_ok
            && (8..=32).contains(&self.prefix)
            && u32::from(self.address) & mask == u32::from(self.gateway) & mask
            && self.address != self.gateway
            && self.dns.len() <= 3
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ED: &str = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOMqqnkVzrm0SdG6UOoqKLsabgH5C9okWi0dh2l9GKJl operador@exemplo";

    #[test]
    fn a_impressao_digital_e_a_do_openssh_e_agrupa_de_quatro_em_quatro() {
        let k = PublicKeyLine::parse(ED).unwrap();
        let fp = k.fingerprint();
        assert!(fp.is_valid(), "{}", fp.0);
        let m = MachineFingerprint {
            algorithm: k.algorithm.clone(),
            fingerprint: fp.clone(),
        };
        let g = m.grouped();
        assert!(g.starts_with("SHA256: "));
        assert_eq!(
            g.trim_start_matches("SHA256: ").replace(' ', ""),
            fp.0.trim_start_matches("SHA256:")
        );
        assert_eq!(k.to_line().split(' ').count(), 2, "comment dropped");
    }

    #[test]
    fn chaves_invalidas_sao_recusadas() {
        assert!(PublicKeyLine::parse("restrict,command=\"x\" ssh-ed25519 AAAA").is_none());
        assert!(PublicKeyLine::parse("ssh-dss AAAAB3NzaC1kc3M=").is_none());
        // The blob names another algorithm than the line.
        assert!(PublicKeyLine::parse("ecdsa-sha2-nistp256 AAAAC3NzaC1lZDI1NTE5AAAAIOMqqnkVzrm0SdG6UOoqKLsabgH5C9okWi0dh2l9GKJl").is_none());
        assert!(PublicKeyLine::parse("ssh-ed25519 !!!").is_none());
    }

    #[test]
    fn identificador_de_arranque() {
        let b = BootstrapId::from_bytes([0x7f, 0x3a, 0x9c, 0x0e, 0x2b, 0x4d, 0x6a, 0x18]);
        assert_eq!(b.0, "ocb-7f3a-9c0e-2b4d-6a18");
        assert!(b.is_valid());
        assert_eq!(b.short(), "7f3a");
        assert!(!BootstrapId("ocb-7f3a-9c0e-2b4d".into()).is_valid());
        assert!(!BootstrapId("ocb-7F3A-9c0e-2b4d-6a18".into()).is_valid());
    }

    #[test]
    fn nomes_de_anfitriao_hostis_sao_recusados() {
        assert!(is_valid_hostname("srv-01.exemplo"));
        for bad in [
            "",
            "Srv",
            "a;rm -rf /",
            "-a",
            "a..b",
            "$(id)",
            &"a".repeat(64),
        ] {
            assert!(!is_valid_hostname(bad), "{bad}");
        }
    }

    #[test]
    fn ipv4_estatico_coerente() {
        let mut s = StaticIpv4 {
            interface: "enp1s0".into(),
            address: "10.0.0.5".parse().unwrap(),
            prefix: 24,
            gateway: "10.0.0.1".parse().unwrap(),
            dns: vec!["10.0.0.1".parse().unwrap()],
        };
        assert!(s.is_valid());
        s.gateway = "10.0.1.1".parse().unwrap();
        assert!(!s.is_valid(), "gateway outside the subnet");
        s.gateway = "10.0.0.1".parse().unwrap();
        s.interface = "eth0;reboot".into();
        assert!(!s.is_valid());
    }
}
