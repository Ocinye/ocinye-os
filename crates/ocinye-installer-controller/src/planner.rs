//! From what the operator configured and what the server reported, the plan.
//!
//! The system changes come **only** from preflight verdicts: Docker is planned
//! when the runtime is `MISSING_INSTALLABLE` on Ubuntu Server 24.04; base
//! packages when they are missing and installable; a ufw rule per port ufw
//! does not yet allow. Nothing else changes the server outside Ocinye, and the
//! plan says so on I13 before anything happens.

use ocinye_installer_contracts::hardware::HardwareCapabilities;
use ocinye_installer_contracts::ident::{InstallationId, PlanId};
use ocinye_installer_contracts::manifest::ReleaseManifest;
use ocinye_installer_contracts::plan::{
    FirewallManager, HardwareSummary, InstallationConfiguration, InstallationPlan, PhaseId, Proto,
    ReleaseIdentity, SystemChange, TargetIdentity,
};
use ocinye_installer_contracts::preflight::{
    self as pf, ContainerRuntimeState, FirewallState, Observation, PreflightCheckId,
    PreflightReport,
};
use rand::rand_core::UnwrapErr;
use rand::rngs::SysRng;
use rand::RngExt as _;

/// The system changes the preflight implies.
#[must_use]
pub fn system_changes(report: &PreflightReport, manifest: &ReleaseManifest) -> Vec<SystemChange> {
    let mut changes = Vec::new();
    if let Some(Observation::Packages {
        installable,
        missing,
    }) = report.item(PreflightCheckId::PfPkg).map(|i| &i.observation)
    {
        if !installable.is_empty() && missing.is_empty() {
            changes.push(SystemChange::EnsurePackages {
                packages: installable.clone(),
            });
        }
    }
    if let Some(Observation::Docker {
        state: ContainerRuntimeState::MissingInstallable,
        ..
    }) = report
        .item(PreflightCheckId::PfDocker)
        .map(|i| &i.observation)
    {
        changes.push(SystemChange::InstallDockerFromOfficialRepo {
            target: ocinye_installer_contracts::SUPPORTED_TARGET.to_owned(),
            arch: manifest.target.arch,
            packages: manifest.prerequisites.docker_packages.clone(),
            repo_key_fingerprint: manifest.prerequisites.docker_repo_key_fingerprint.clone(),
        });
    }
    if let Some(Observation::Firewall {
        state: FirewallState::RequiredRulesCanBeApplied,
        observed,
    }) = report.item(PreflightCheckId::PfFw).map(|i| &i.observation)
    {
        for port in pf::missing_ufw_ports(observed) {
            changes.push(SystemChange::FirewallAllow {
                manager: FirewallManager::Ufw,
                port,
                proto: Proto::Tcp,
                comment: "ocinye".to_owned(),
            });
        }
    }
    changes
}

fn random8() -> [u8; 8] {
    UnwrapErr(SysRng).random()
}

/// A new installation identity (stable across resume).
#[must_use]
pub fn new_installation_id() -> InstallationId {
    InstallationId::from_random(random8())
}

/// Build and seal a plan. Every call mints a **new** `plan_id`: a changed
/// configuration is never the same plan.
#[must_use]
pub fn build(
    installation_id: InstallationId,
    manifest: &ReleaseManifest,
    target: TargetIdentity,
    configuration: InstallationConfiguration,
    report: &PreflightReport,
    hardware: &HardwareCapabilities,
) -> InstallationPlan {
    let system_changes = system_changes(report, manifest);
    InstallationPlan {
        plan_id: PlanId::from_random(random8()),
        installation_id,
        created_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        release: ReleaseIdentity::of(manifest),
        target,
        configuration,
        phases: InstallationPlan::phases_for(&system_changes),
        system_changes,
        preflight: report.blocking_snapshot(),
        hardware: HardwareSummary::of(hardware),
        plan_sha256: String::new(),
    }
    .seal()
}

/// The packages and firewall rules a plan actually applied: those of its
/// changes whose phase completed (P04 installs packages, P08 opens ports).
/// The receipt states these, not the plan's intentions.
#[must_use]
pub fn applied(
    changes: &[SystemChange],
    completed: impl Fn(PhaseId) -> bool,
) -> (Vec<String>, Vec<String>) {
    let packages = if completed(PhaseId::P04) {
        changes
            .iter()
            .flat_map(|c| match c {
                SystemChange::InstallDockerFromOfficialRepo { packages, .. }
                | SystemChange::EnsurePackages { packages } => packages.clone(),
                _ => Vec::new(),
            })
            .collect()
    } else {
        Vec::new()
    };
    let rules = if completed(PhaseId::P08) {
        changes
            .iter()
            .filter_map(|c| match c {
                SystemChange::FirewallAllow { port, .. } => Some(format!("{port}/tcp")),
                _ => None,
            })
            .collect()
    } else {
        Vec::new()
    };
    (packages, rules)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ocinye_installer_contracts::preflight::*;

    fn report(
        docker: ContainerRuntimeState,
        fw: FirewallObservation,
        pkgs: Vec<String>,
    ) -> PreflightReport {
        let mut items = Vec::new();
        for id in PreflightCheckId::ALL {
            let observation = match id {
                PreflightCheckId::PfDocker => Observation::Docker {
                    state: docker,
                    observed: DockerObservation::default(),
                },
                PreflightCheckId::PfFw => Observation::Firewall {
                    state: classify_firewall(&fw),
                    observed: fw.clone(),
                },
                PreflightCheckId::PfPkg => Observation::Packages {
                    installable: pkgs.clone(),
                    missing: vec![],
                },
                _ => Observation::Journal { found: false },
            };
            items.push(PreflightItem {
                id,
                status: CheckStatus::Pass,
                action: None,
                observation,
            });
        }
        PreflightReport {
            items,
            incomplete: None,
        }
    }

    fn manifest() -> ReleaseManifest {
        let dir = crate::bundle::tests::fixture("planner");
        let m = crate::bundle::verify(&dir).unwrap().manifest;
        let _ = std::fs::remove_dir_all(dir);
        m
    }

    #[test]
    fn docker_em_falta_entra_no_plano_com_a_chave_do_manifesto() {
        let m = manifest();
        let c = system_changes(
            &report(
                ContainerRuntimeState::MissingInstallable,
                FirewallObservation::default(),
                vec![],
            ),
            &m,
        );
        assert_eq!(c.len(), 1);
        assert!(c[0].is_allowed(&m));
        let SystemChange::InstallDockerFromOfficialRepo {
            repo_key_fingerprint,
            ..
        } = &c[0]
        else {
            panic!()
        };
        assert_eq!(
            *repo_key_fingerprint,
            m.prerequisites.docker_repo_key_fingerprint
        );
    }

    #[test]
    fn um_docker_compativel_nao_se_reinstala() {
        let m = manifest();
        let c = system_changes(
            &report(
                ContainerRuntimeState::DetectedSupported,
                FirewallObservation::default(),
                vec![],
            ),
            &m,
        );
        assert!(c.is_empty(), "{c:?}");
    }

    #[test]
    fn so_as_regras_ufw_em_falta_se_acrescentam() {
        let m = manifest();
        let fw = FirewallObservation {
            ufw_active: true,
            ufw_allows: vec![22, 80],
            ..Default::default()
        };
        let c = system_changes(
            &report(ContainerRuntimeState::DetectedSupported, fw, vec![]),
            &m,
        );
        assert_eq!(c.len(), 1);
        assert!(matches!(
            c[0],
            SystemChange::FirewallAllow { port: 443, .. }
        ));
        let ext = FirewallObservation {
            firewalld_active: true,
            ..Default::default()
        };
        assert!(system_changes(
            &report(ContainerRuntimeState::DetectedSupported, ext, vec![]),
            &m
        )
        .is_empty());
    }

    #[test]
    fn cada_plano_tem_identidade_propria() {
        let a = PlanId::from_random(random8());
        let b = PlanId::from_random(random8());
        assert_ne!(a, b);
    }

    #[test]
    fn o_recibo_diz_o_que_foi_aplicado_e_nao_o_que_se_pretendia() {
        let m = manifest();
        let c = system_changes(
            &report(
                ContainerRuntimeState::MissingInstallable,
                FirewallObservation::default(),
                vec![],
            ),
            &m,
        );
        let (none, _) = applied(&c, |_| false);
        assert!(
            none.is_empty(),
            "P04 did not complete: nothing was installed"
        );
        let (pkgs, rules) = applied(&c, |p| p == PhaseId::P04);
        assert_eq!(pkgs, m.prerequisites.docker_packages);
        assert!(rules.is_empty());
    }
}
