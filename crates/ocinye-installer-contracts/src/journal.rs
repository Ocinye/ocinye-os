//! The installation journal (`/var/lib/ocinye-installer/<id>/journal.json`,
//! 0600 root) and the resume rules (D011_STATE_MACHINE).
//!
//! The journal is what makes an installation understandable after the
//! Installer closes, the network drops, or the server reboots: which plan,
//! which release, which phases started and finished, what failed with which
//! code, and what the Installer created and may therefore remove. It holds **no
//! secret**: no credential, no environment value, no key — `config_written_at`
//! records *that* the configuration was written, never what it says.

use serde::{Deserialize, Serialize};

use crate::ident::{InstallationId, PlanId};
use crate::plan::{PhaseId, SafetyClass};

/// Overall state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum JournalState {
    /// A phase is running, or the executor was cut while one was.
    Running,
    /// Stopped at a safe point at the operator's request.
    Stopped,
    /// A phase failed.
    Failed,
    /// Every phase completed.
    Completed,
}

/// One phase, as it happened.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhaseRecord {
    /// Which.
    pub id: PhaseId,
    /// RFC 3339, when it started.
    pub started_at: Option<String>,
    /// RFC 3339, when it completed.
    pub completed_at: Option<String>,
    /// Skipped (nothing planned).
    pub skipped: bool,
    /// Failure code, when it failed.
    pub failed_code: Option<String>,
    /// Whether the failure may be retried.
    pub retryable: Option<bool>,
}

/// The journal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstallationJournal {
    /// The installation.
    pub installation_id: InstallationId,
    /// The plan executed.
    pub plan_id: PlanId,
    /// Its seal: a resume must present the same plan.
    pub plan_sha256: String,
    /// Release id.
    pub release: String,
    /// The host key the plan was confirmed against.
    pub host_key_sha256: String,
    /// Overall state.
    pub state: JournalState,
    /// One record per phase, in order.
    pub phases: Vec<PhaseRecord>,
    /// Paths the Installer created (removable by `RemoveIncomplete` only).
    pub created_paths: Vec<String>,
    /// Packages the Installer installed.
    pub installed_packages: Vec<String>,
    /// ufw rules the Installer added (`80/tcp`, `443/tcp`).
    pub firewall_rules: Vec<String>,
    /// When the configuration was written (never its content).
    pub config_written_at: Option<String>,
    /// P11 completed: an Instance exists.
    pub instance_created: bool,
    /// The self-signed certificate's SHA-256, when one was generated (V11).
    pub self_signed_cert_sha256: Option<String>,
    /// RFC 3339.
    pub started_at: String,
    /// RFC 3339, last write.
    pub updated_at: String,
}

/// What the Installer shows on I19 / I36.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JournalSummary {
    /// The installation.
    pub installation_id: InstallationId,
    /// The plan.
    pub plan_id: PlanId,
    /// Release id.
    pub release: String,
    /// State.
    pub state: JournalState,
    /// Phases completed (or skipped).
    pub completed: Vec<PhaseId>,
    /// The phase in progress or that failed.
    pub current: Option<PhaseId>,
    /// Failure code, if any.
    pub failed_code: Option<String>,
    /// An Instance exists.
    pub instance_created: bool,
    /// Last write.
    pub updated_at: String,
}

/// What may happen next, decided from the journal **and** the server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "decision",
    content = "phase",
    rename_all = "SCREAMING_SNAKE_CASE"
)]
pub enum ResumeDecision {
    /// Nothing left: every phase completed.
    Completed,
    /// Continue at this phase (it never started, or it is safe to redo).
    ContinueAt(PhaseId),
    /// P11 started and did not finish: read the Core's state before deciding
    /// (an Instance may or may not exist). Never a blind replay.
    InspectInstance,
    /// A non-retryable failure: no resume (report; remove when allowed).
    NotResumable(PhaseId),
}

impl InstallationJournal {
    /// A fresh journal for a plan.
    #[must_use]
    pub fn new(
        installation_id: InstallationId,
        plan_id: PlanId,
        plan_sha256: String,
        release: String,
        host_key_sha256: String,
        skipped: &[PhaseId],
        now: String,
    ) -> Self {
        Self {
            installation_id,
            plan_id,
            plan_sha256,
            release,
            host_key_sha256,
            state: JournalState::Running,
            phases: PhaseId::ALL
                .iter()
                .map(|p| PhaseRecord {
                    id: *p,
                    started_at: None,
                    completed_at: None,
                    skipped: skipped.contains(p),
                    failed_code: None,
                    retryable: None,
                })
                .collect(),
            created_paths: Vec::new(),
            installed_packages: Vec::new(),
            firewall_rules: Vec::new(),
            config_written_at: None,
            instance_created: false,
            self_signed_cert_sha256: None,
            started_at: now.clone(),
            updated_at: now,
        }
    }

    fn record(&mut self, id: PhaseId) -> &mut PhaseRecord {
        let i = id.number() - 1;
        &mut self.phases[i]
    }

    /// A phase started.
    pub fn started(&mut self, id: PhaseId, now: &str) {
        let r = self.record(id);
        r.started_at = Some(now.to_owned());
        r.failed_code = None;
        r.retryable = None;
        self.state = JournalState::Running;
        now.clone_into(&mut self.updated_at);
    }

    /// A phase completed.
    pub fn completed(&mut self, id: PhaseId, now: &str) {
        self.record(id).completed_at = Some(now.to_owned());
        if id == PhaseId::P11 {
            self.instance_created = true;
        }
        if PhaseId::ALL.iter().all(|p| {
            self.phases[p.number() - 1].completed_at.is_some()
                || self.phases[p.number() - 1].skipped
        }) {
            self.state = JournalState::Completed;
        }
        now.clone_into(&mut self.updated_at);
    }

    /// A phase failed.
    pub fn failed(&mut self, id: PhaseId, code: &str, retryable: bool, now: &str) {
        let r = self.record(id);
        r.failed_code = Some(code.to_owned());
        r.retryable = Some(retryable);
        self.state = JournalState::Failed;
        now.clone_into(&mut self.updated_at);
    }

    /// The first phase neither completed nor skipped.
    #[must_use]
    pub fn first_incomplete(&self) -> Option<&PhaseRecord> {
        self.phases
            .iter()
            .find(|r| r.completed_at.is_none() && !r.skipped)
    }

    /// The resume rule of the safety classes.
    #[must_use]
    pub fn resume_decision(&self) -> ResumeDecision {
        let Some(r) = self.first_incomplete() else {
            return ResumeDecision::Completed;
        };
        if r.retryable == Some(false) {
            return ResumeDecision::NotResumable(r.id);
        }
        if r.id.class() == SafetyClass::D && r.started_at.is_some() {
            return ResumeDecision::InspectInstance;
        }
        ResumeDecision::ContinueAt(r.id)
    }

    /// `RemoveIncomplete` is allowed only while no Instance exists.
    #[must_use]
    pub const fn removal_allowed(&self) -> bool {
        !self.instance_created
    }

    /// The summary shown to the operator.
    #[must_use]
    pub fn summary(&self) -> JournalSummary {
        let current = self.first_incomplete();
        JournalSummary {
            installation_id: self.installation_id.clone(),
            plan_id: self.plan_id.clone(),
            release: self.release.clone(),
            state: self.state,
            completed: self
                .phases
                .iter()
                .filter(|r| r.completed_at.is_some() || r.skipped)
                .map(|r| r.id)
                .collect(),
            current: current.map(|r| r.id),
            failed_code: current.and_then(|r| r.failed_code.clone()),
            instance_created: self.instance_created,
            updated_at: self.updated_at.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn journal() -> InstallationJournal {
        InstallationJournal::new(
            InstallationId::parse("inst-7c41e20000000000").unwrap(),
            PlanId::parse("pl-5d0a910000000000").unwrap(),
            "a".repeat(64),
            "3f9c2a7d1e04".into(),
            "SHA256:EXEMPLO".into(),
            &[PhaseId::P08],
            "2026-10-04T14:00:00Z".into(),
        )
    }

    #[test]
    fn retoma_na_primeira_fase_por_acabar_e_salta_as_saltadas() {
        let mut j = journal();
        assert_eq!(
            j.resume_decision(),
            ResumeDecision::ContinueAt(PhaseId::P01)
        );
        for p in &PhaseId::ALL[..7] {
            j.started(*p, "t");
            j.completed(*p, "t");
        }
        assert_eq!(
            j.resume_decision(),
            ResumeDecision::ContinueAt(PhaseId::P09)
        );
        assert!(j.removal_allowed());
    }

    #[test]
    fn dentro_da_p11_nunca_se_repete_as_cegas() {
        let mut j = journal();
        for p in &PhaseId::ALL[..10] {
            j.started(*p, "t");
            j.completed(*p, "t");
        }
        j.started(PhaseId::P11, "t");
        assert_eq!(j.resume_decision(), ResumeDecision::InspectInstance);
        j.failed(PhaseId::P11, "MIGRATION_FAILED", false, "t");
        assert_eq!(
            j.resume_decision(),
            ResumeDecision::NotResumable(PhaseId::P11)
        );
        assert!(j.removal_allowed(), "sem Instância, remover é permitido");
    }

    #[test]
    fn depois_da_instancia_nao_ha_remocao() {
        let mut j = journal();
        for p in PhaseId::ALL {
            j.started(p, "t");
            j.completed(p, "t");
        }
        assert!(!j.removal_allowed());
        assert_eq!(j.state, JournalState::Completed);
        assert_eq!(j.resume_decision(), ResumeDecision::Completed);
    }

    #[test]
    fn o_diario_nao_tem_onde_guardar_um_segredo() {
        let json = serde_json::to_string(&journal()).unwrap();
        for k in ["password", "credential", "secret", "token", "key\":", "env"] {
            assert!(!json.to_lowercase().contains(k), "{k} em {json}");
        }
    }
}
