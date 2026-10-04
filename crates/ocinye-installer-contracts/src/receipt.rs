//! The installation receipt (I18): machine-readable JSON plus the human view.
//!
//! Non-secret by construction: none of its types can hold a [`SecretText`], a
//! private key or a credential. The «never» list is printed on the screen and
//! enforced here by a test that looks for every secret the journey handled.
//!
//! [`SecretText`]: crate::secret::SecretText

use serde::{Deserialize, Serialize};

use crate::hardware::{HardwareCapabilities, ProviderMode};
use crate::ident::{InstallationId, PlanId};
use crate::manifest::{Artifact, BuildKind};
use crate::plan::AccessEndpoints;
use crate::verification::{LifecycleState, TlsMode, VerificationReport};

/// The schema of the receipt file.
pub const RECEIPT_SCHEMA: u32 = 1;

/// The receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstallationReceipt {
    /// [`RECEIPT_SCHEMA`].
    pub schema: u32,
    /// Installation.
    pub installation_id: InstallationId,
    /// Plan.
    pub plan_id: PlanId,
    /// Its seal.
    pub plan_sha256: String,
    /// Release id.
    pub release: String,
    /// Commit.
    pub commit: String,
    /// Build kind.
    pub build: BuildKind,
    /// Manifest seal.
    pub manifest_sha256: String,
    /// Artifacts with their sums.
    pub artifacts: Vec<Artifact>,
    /// Target host as typed.
    pub target_host: String,
    /// Host key fingerprint.
    pub host_key_sha256: String,
    /// Instance name (read from the Core).
    pub instance_name: String,
    /// Instance slug (read from the Core).
    pub instance_slug: String,
    /// Enabled Distributions (read from the Core).
    pub distributions: Vec<String>,
    /// Endpoints as planned.
    pub endpoints: AccessEndpoints,
    /// TLS mode.
    pub tls_mode: TlsMode,
    /// The certificate's `notAfter`, when supplied by the operator.
    pub tls_not_after: Option<String>,
    /// Detected hardware.
    pub hardware: HardwareCapabilities,
    /// Always `Off`.
    pub provider_mode: ProviderMode,
    /// Packages the Installer installed.
    pub installed_packages: Vec<String>,
    /// ufw rules the Installer added.
    pub firewall_rules: Vec<String>,
    /// Preflight warnings (check ids).
    pub warnings: Vec<String>,
    /// RFC 3339.
    pub started_at: String,
    /// RFC 3339.
    pub ended_at: String,
    /// Every verification item.
    pub verification: VerificationReport,
    /// The lifecycle at the time of the receipt.
    pub lifecycle: LifecycleState,
    /// First access still pending (V16).
    pub first_access_pending: bool,
}

impl InstallationReceipt {
    /// Pretty JSON for the file the operator saves.
    ///
    /// # Panics
    ///
    /// Never: the receipt is plain data.
    #[must_use]
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("plain data")
    }

    /// Read a saved receipt.
    ///
    /// # Errors
    ///
    /// Malformed JSON or unknown fields.
    pub fn parse(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// The suggested file name.
    #[must_use]
    pub fn file_name(&self) -> String {
        format!("ocinye-install-{}.json", self.installation_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::tests::sample;
    use crate::plan::tests::{config, hardware};
    use crate::verification::{ItemStatus, VerificationId, VerificationItem};

    fn receipt() -> InstallationReceipt {
        let m = sample();
        InstallationReceipt {
            schema: RECEIPT_SCHEMA,
            installation_id: InstallationId::parse("inst-7c41e20000000000").unwrap(),
            plan_id: PlanId::parse("pl-5d0a910000000000").unwrap(),
            plan_sha256: "b".repeat(64),
            release: m.release.id.clone(),
            commit: m.release.commit.clone(),
            build: m.release.build,
            manifest_sha256: m.sha256(),
            artifacts: m.artifacts.clone(),
            target_host: "192.0.2.10".into(),
            host_key_sha256: "SHA256:EXEMPLO".into(),
            instance_name: "Empresa Exemplo".into(),
            instance_slug: "empresa-exemplo".into(),
            distributions: vec!["business".into(), "research".into()],
            endpoints: config().endpoints,
            tls_mode: TlsMode::SelfSignedTest,
            tls_not_after: None,
            hardware: hardware(),
            provider_mode: ProviderMode::Off,
            installed_packages: vec!["docker-ce".into()],
            firewall_rules: vec![],
            warnings: vec!["PF_RAM".into()],
            started_at: "2026-10-04T14:16:31Z".into(),
            ended_at: "2026-10-04T14:21:33Z".into(),
            verification: VerificationReport {
                items: vec![VerificationItem {
                    id: VerificationId::V04,
                    status: ItemStatus::Pass,
                    evidence: "READY".into(),
                }],
            },
            lifecycle: LifecycleState::InstalledTestMode,
            first_access_pending: true,
        }
    }

    #[test]
    fn o_recibo_volta_igual_e_e_deterministico() {
        let r = receipt();
        let a = r.to_json();
        assert_eq!(InstallationReceipt::parse(&a).unwrap(), r);
        assert_eq!(a, InstallationReceipt::parse(&a).unwrap().to_json());
        assert_eq!(r.file_name(), "ocinye-install-inst-7c41e20000000000.json");
    }

    #[test]
    fn o_recibo_nao_tem_campos_para_segredos() {
        let json = receipt().to_json().to_lowercase();
        for k in [
            "password",
            "secret",
            "credential\"",
            "private",
            "token",
            "sealing",
        ] {
            assert!(!json.contains(k), "{k}");
        }
        // Um campo a mais é recusado: um recibo não ganha um segredo por fora.
        let mut v: serde_json::Value = serde_json::from_str(&receipt().to_json()).unwrap();
        v["admin_password"] = "x".into();
        assert!(InstallationReceipt::parse(&v.to_string()).is_err());
    }
}
