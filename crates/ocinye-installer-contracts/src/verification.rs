//! Product verification (D011_VERIFICATION_MATRIX) and the installation
//! lifecycle (D011_FINAL_REVIEW_CORRECTION §1).
//!
//! Running containers do not prove Ocinye OS works. Three states, kept apart
//! and never collapsed into a `success: bool`:
//!
//! - **INSTALLATION_COMPLETE** — every mandatory server item passes (V01–V09,
//!   V12b): services, database, migrations, Core readiness, Instance,
//!   Distributions, registry, storage, the login surface inside the server,
//!   and the endpoints as the Core has them.
//! - **ACTIVATION_PENDING{DNS | FIREWALL | TLS_TRUST}** — installed, but
//!   something outside the server is not there yet. «Abrir» is disabled;
//!   «Verificar novamente» reruns the operator-side items only.
//! - **OPERATIONAL** — installed **and** every operator-side item passes (V10
//!   DNS, V-FW ports, V11 HTTPS with the operator's certificate, V12 the
//!   endpoint resolves to this Instance, V13 the login page answers).
//!
//! A self-signed certificate ends in **INSTALLED_TEST_MODE**, never
//! OPERATIONAL. A failed server item ends in INSTALLATION_INCOMPLETE — a failed
//! installation is never shown as operational.

use serde::{Deserialize, Serialize};

/// The verification items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VerificationId {
    /// Services healthy (Compose healthchecks).
    V01,
    /// Database connectivity (`verify-schema`).
    V02,
    /// Migration state == manifest.
    V03,
    /// Core `/ready?contract=N`.
    V04,
    /// Instance identity == plan (`verify-instance`).
    V05,
    /// Enabled Distributions == plan.
    V06,
    /// App registry reported by `/ready`.
    V07,
    /// Object storage.
    V08,
    /// Login surface inside the server (303 → /login 200).
    V09,
    /// DNS (operator side).
    V10,
    /// HTTPS + certificate (operator side).
    V11,
    /// Endpoint → Instance (+ binding), operator side.
    V12,
    /// Endpoint list == plan (`verify-endpoints`).
    V12b,
    /// Login page from the operator.
    V13,
    /// Hardware facts recorded (informational).
    V14,
    /// First access pending (`verify-admin-bootstrap`, informational).
    V16,
    /// Ports 80/443 reachable from the operator.
    VFw,
}

impl VerificationId {
    /// Server-side, mandatory for INSTALLATION_COMPLETE.
    pub const SERVER_MANDATORY: [Self; 10] = [
        Self::V01,
        Self::V02,
        Self::V03,
        Self::V04,
        Self::V05,
        Self::V06,
        Self::V07,
        Self::V08,
        Self::V09,
        Self::V12b,
    ];
    /// Operator-side, mandatory for OPERATIONAL.
    pub const OPERATOR: [Self; 5] = [Self::V10, Self::VFw, Self::V11, Self::V12, Self::V13];
}

/// One item's status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ItemStatus {
    /// Observed and satisfied.
    Pass,
    /// Observed and violated.
    Fail,
    /// Waiting on something external (DNS not resolving yet, untrusted chain).
    Pending,
    /// Not run (an earlier item failed, or it does not apply).
    NotRun,
}

/// One item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationItem {
    /// Which.
    pub id: VerificationId,
    /// Status.
    pub status: ItemStatus,
    /// Closed evidence code (`READY`, `HTTP_303`, `DNS_UNRESOLVED`, …).
    pub evidence: String,
}

/// A set of items.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationReport {
    /// Items, in order of execution.
    pub items: Vec<VerificationItem>,
}

impl VerificationReport {
    /// The status of an item (`NotRun` when absent).
    #[must_use]
    pub fn status(&self, id: VerificationId) -> ItemStatus {
        self.items
            .iter()
            .rev()
            .find(|i| i.id == id)
            .map_or(ItemStatus::NotRun, |i| i.status)
    }

    /// Replace the items for these ids (a recheck replaces operator items).
    pub fn replace(&mut self, items: Vec<VerificationItem>) {
        let ids: Vec<VerificationId> = items.iter().map(|i| i.id).collect();
        self.items.retain(|i| !ids.contains(&i.id));
        self.items.extend(items);
        self.items.sort_by_key(|i| i.id);
    }
}

/// Why activation waits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ActivationReason {
    /// A host does not resolve to the server yet.
    Dns,
    /// 80/443 not reachable from the operator (cloud filter, external firewall).
    Firewall,
    /// The certificate chain is not trusted from the operator side.
    TlsTrust,
}

/// The lifecycle (frozen states).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LifecycleState {
    /// A mandatory server item failed (or did not run).
    InstallationIncomplete,
    /// Installed; an operator-side item failed for a reason that is not
    /// external (wrong Instance behind the name, login page not answering).
    InstallationComplete,
    /// Installed; waiting on the outside.
    ActivationPending {
        /// Sorted, distinct.
        reasons: Vec<ActivationReason>,
    },
    /// Installed and reachable with a self-signed certificate: test only.
    InstalledTestMode,
    /// Installed and reachable with the operator's certificate.
    Operational,
}

/// TLS mode, as far as the lifecycle cares.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TlsMode {
    /// The operator's certificate.
    OperatorSupplied,
    /// Self-signed (test).
    SelfSignedTest,
}

impl LifecycleState {
    /// Derive the lifecycle from a full report. Only this function decides
    /// it, and it decides OPERATIONAL only when every mandatory item passes.
    #[must_use]
    pub fn derive(report: &VerificationReport, tls: TlsMode) -> Self {
        if VerificationId::SERVER_MANDATORY
            .iter()
            .any(|id| report.status(*id) != ItemStatus::Pass)
        {
            return Self::InstallationIncomplete;
        }
        let mut reasons = Vec::new();
        if report.status(VerificationId::V10) != ItemStatus::Pass {
            reasons.push(ActivationReason::Dns);
        }
        if report.status(VerificationId::VFw) != ItemStatus::Pass {
            reasons.push(ActivationReason::Firewall);
        }
        if report.status(VerificationId::V11) == ItemStatus::Pending {
            reasons.push(ActivationReason::TlsTrust);
        }
        if !reasons.is_empty() {
            reasons.sort();
            reasons.dedup();
            return Self::ActivationPending { reasons };
        }
        if VerificationId::OPERATOR
            .iter()
            .any(|id| report.status(*id) != ItemStatus::Pass)
        {
            return Self::InstallationComplete;
        }
        match tls {
            TlsMode::OperatorSupplied => Self::Operational,
            TlsMode::SelfSignedTest => Self::InstalledTestMode,
        }
    }

    /// «Abrir Ocinye OS» is enabled.
    #[must_use]
    pub const fn may_open(&self) -> bool {
        matches!(self, Self::Operational | Self::InstalledTestMode)
    }

    /// Installation complete (the strip's first segment).
    #[must_use]
    pub const fn installed(&self) -> bool {
        !matches!(self, Self::InstallationIncomplete)
    }

    /// Operational (the strip's third segment).
    #[must_use]
    pub const fn operational(&self) -> bool {
        matches!(self, Self::Operational)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(fail: &[VerificationId], pending: &[VerificationId]) -> VerificationReport {
        let mut r = VerificationReport::default();
        r.replace(
            VerificationId::SERVER_MANDATORY
                .iter()
                .chain(VerificationId::OPERATOR.iter())
                .map(|id| VerificationItem {
                    id: *id,
                    status: if fail.contains(id) {
                        ItemStatus::Fail
                    } else if pending.contains(id) {
                        ItemStatus::Pending
                    } else {
                        ItemStatus::Pass
                    },
                    evidence: "X".into(),
                })
                .collect(),
        );
        r
    }

    #[test]
    fn so_tudo_verde_com_certificado_do_operador_e_operacional() {
        let ok = report(&[], &[]);
        assert_eq!(
            LifecycleState::derive(&ok, TlsMode::OperatorSupplied),
            LifecycleState::Operational
        );
        assert_eq!(
            LifecycleState::derive(&ok, TlsMode::SelfSignedTest),
            LifecycleState::InstalledTestMode,
            "auto-assinado nunca é operacional"
        );
        assert!(!LifecycleState::InstalledTestMode.operational());
    }

    #[test]
    fn dns_pendente_e_instalado_nao_operacional_e_sem_abrir() {
        let s = LifecycleState::derive(
            &report(&[], &[VerificationId::V10]),
            TlsMode::OperatorSupplied,
        );
        assert_eq!(
            s,
            LifecycleState::ActivationPending {
                reasons: vec![ActivationReason::Dns]
            }
        );
        assert!(s.installed() && !s.operational() && !s.may_open());
    }

    #[test]
    fn firewall_e_confianca_tls_sao_activacao_e_nao_falha() {
        let s = LifecycleState::derive(
            &report(&[VerificationId::VFw], &[VerificationId::V11]),
            TlsMode::OperatorSupplied,
        );
        assert_eq!(
            s,
            LifecycleState::ActivationPending {
                reasons: vec![ActivationReason::Firewall, ActivationReason::TlsTrust]
            }
        );
    }

    #[test]
    fn um_item_do_servidor_em_falta_nunca_e_instalado() {
        for id in VerificationId::SERVER_MANDATORY {
            let s = LifecycleState::derive(&report(&[id], &[]), TlsMode::OperatorSupplied);
            assert_eq!(s, LifecycleState::InstallationIncomplete, "{id:?}");
        }
        let mut r = report(&[], &[]);
        r.items.retain(|i| i.id != VerificationId::V04);
        assert_eq!(
            LifecycleState::derive(&r, TlsMode::OperatorSupplied),
            LifecycleState::InstallationIncomplete,
            "um item que não correu não é um item que passou"
        );
    }

    #[test]
    fn verificar_de_novo_substitui_so_os_itens_refeitos() {
        let mut r = report(&[], &[VerificationId::V10]);
        r.replace(vec![VerificationItem {
            id: VerificationId::V10,
            status: ItemStatus::Pass,
            evidence: "DNS_OK".into(),
        }]);
        assert_eq!(r.status(VerificationId::V10), ItemStatus::Pass);
        assert_eq!(
            r.items
                .iter()
                .filter(|i| i.id == VerificationId::V10)
                .count(),
            1
        );
        assert_eq!(
            LifecycleState::derive(&r, TlsMode::OperatorSupplied),
            LifecycleState::Operational
        );
    }
}
