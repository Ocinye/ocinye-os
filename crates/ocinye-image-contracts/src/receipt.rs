//! `ImageSourceReceipt` — the non-secret extension of the D011 installation
//! receipt (E-05) [P, PD-05]. In Phase A it is a separate document; folding it
//! into `InstallationReceipt.image_source` with a receipt schema bump is a D011
//! change that waits for D011 certification.

use serde::{Deserialize, Serialize};

use crate::build::Channel;
use crate::claim::ClaimMethod;
use crate::firstboot::{BootstrapId, KeyFingerprint};
use crate::manifest::{ImageFormat, Sha256Hex};

/// Where this server came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageSourceReceipt {
    /// `ocinye-os-<id>-r<N>[-dev]-<arch>`.
    pub image: String,
    /// Format the machine was installed from.
    pub source_format: ImageFormat,
    /// Digest of the artifact as published (from the verified descriptor).
    pub artifact_sha256: Sha256Hex,
    /// Digest of the `ImageContentManifest` the server reports, matched to the descriptor.
    pub content_sha256: Sha256Hex,
    /// D011 release id.
    pub release_id: String,
    /// "24.04".
    pub ubuntu_release: String,
    /// Pinned serial.
    pub ubuntu_serial: String,
    /// Digest of the pinned base.
    pub ubuntu_base_sha256: Sha256Hex,
    /// Key that signed `SHA256SUMS`, when signed.
    pub signature_key_id: Option<String>,
    /// Channel.
    pub channel: Channel,
    /// Bootstrap id shown on the console.
    pub bootstrap_id: BootstrapId,
    /// How it was claimed.
    pub claim_method: ClaimMethod,
    /// RFC 3339.
    pub claimed_at: String,
    /// Owner key.
    pub owner_key: KeyFingerprint,
    /// Other keys authorized on `ocinye` by the platform — listed, never removed silently.
    pub other_authorized_keys: Vec<KeyFingerprint>,
}

impl ImageSourceReceipt {
    /// Canonical JSON (D011 canonical form, PD-02).
    #[must_use]
    pub fn to_canonical(&self) -> String {
        ocinye_installer_contracts::canonical::to_canonical(self).expect("integers only")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::tests::h;

    #[test]
    fn o_recibo_nao_tem_segredos_nem_campos_desconhecidos() {
        let r = ImageSourceReceipt {
            image: "ocinye-os-0123456789ab-r1-dev-amd64".into(),
            source_format: ImageFormat::Iso,
            artifact_sha256: h('a'),
            content_sha256: h('b'),
            release_id: "0123456789ab".into(),
            ubuntu_release: "24.04".into(),
            ubuntu_serial: "20260901".into(),
            ubuntu_base_sha256: h('c'),
            signature_key_id: None,
            channel: Channel::Development,
            bootstrap_id: BootstrapId::from_bytes([1; 8]),
            claim_method: ClaimMethod::PairingCode,
            claimed_at: "2026-10-05T10:00:00Z".into(),
            owner_key: KeyFingerprint(format!("SHA256:{}", "a".repeat(43))),
            other_authorized_keys: vec![],
        };
        let j = r.to_canonical();
        let v: serde_json::Value = serde_json::from_str(&j).unwrap();
        for key in v.as_object().unwrap().keys() {
            for word in ["code", "password", "secret", "private", "token"] {
                assert!(!key.contains(word), "{key}");
            }
        }
        let back: ImageSourceReceipt = serde_json::from_str(&j).unwrap();
        assert_eq!(back, r);
        assert!(serde_json::from_str::<ImageSourceReceipt>(&j.replacen(
            '{',
            "{\"pairing_code\":\"x\",",
            1
        ))
        .is_err());
    }
}
