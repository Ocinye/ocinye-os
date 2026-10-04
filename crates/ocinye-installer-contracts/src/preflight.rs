//! Preflight (D011_PREFLIGHT_MATRIX, D011_PREREQUISITE_MATRIX).
//!
//! **Preflight discovers; installation mutates.** Every check here is read on
//! the server and judged by the functions in this module, so the bootstrap and
//! the Installer cannot disagree about what «blocked» means. A missing Docker
//! on Ubuntu Server 24.04 is not a blocker: it becomes a plan item («SERÁ
//! INSTALADO»). A conflicting or old runtime is: the Installer never replaces a
//! runtime it did not install.

use serde::{Deserialize, Serialize};

use crate::journal::JournalSummary;
use crate::manifest::Arch;

/// The checks, in the order of the screen (closed set).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PreflightCheckId {
    /// Linux, 64-bit.
    PfOs,
    /// Ubuntu Server 24.04 LTS, without a graphical environment.
    PfDistro,
    /// Server architecture = release architecture.
    PfArch,
    /// root or sudo.
    PfPriv,
    /// Clock skew against the operator.
    PfTime,
    /// vCPU.
    PfCpu,
    /// Memory.
    PfRam,
    /// Free disk under `/srv`.
    PfDisk,
    /// `/proc`, `/sys`, PCI readable.
    PfHwread,
    /// Container runtime.
    PfDocker,
    /// Ports 80/443.
    PfPorts,
    /// Other reverse proxy.
    PfProxy,
    /// System firewall.
    PfFw,
    /// Base packages.
    PfPkg,
    /// Outbound HTTPS to registries.
    PfRegistry,
    /// systemd.
    PfSystemd,
    /// Existing Ocinye OS.
    PfExist,
    /// Existing installer journal.
    PfJournal,
    /// `/srv/ocinye` origin.
    PfSrvdir,
}

impl PreflightCheckId {
    /// Every check, in display order.
    pub const ALL: [Self; 19] = [
        Self::PfOs,
        Self::PfDistro,
        Self::PfArch,
        Self::PfPriv,
        Self::PfTime,
        Self::PfCpu,
        Self::PfRam,
        Self::PfDisk,
        Self::PfHwread,
        Self::PfDocker,
        Self::PfPorts,
        Self::PfProxy,
        Self::PfFw,
        Self::PfPkg,
        Self::PfRegistry,
        Self::PfSystemd,
        Self::PfExist,
        Self::PfJournal,
        Self::PfSrvdir,
    ];
}

/// The verdict of one check. Never conveyed by colour alone: the UI writes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CheckStatus {
    /// Satisfied.
    Pass,
    /// Satisfied with a caveat (or a planned change, see [`PlannedAction`]).
    Warning,
    /// Not satisfied: the Install action is unavailable.
    Blocked,
    /// Does not apply here.
    NotApplicable,
}

/// What the plan will do about a check, shown as a badge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PlannedAction {
    /// «SERÁ INSTALADO» — a prerequisite package set (P04).
    WillInstall,
    /// «SERÁ APLICADO» — an Ocinye-owned firewall rule (P08).
    WillApply,
    /// «ACÇÃO EXTERNA» — the operator acts outside; activation waits.
    ExternalAction,
}

/// `ContainerRuntimeState` (D011_PREREQUISITE_MATRIX).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ContainerRuntimeState {
    /// Docker Engine ≥ 24 running, compose v2 present: used as is.
    DetectedSupported,
    /// No runtime on the supported target: installed in P04 after confirmation.
    MissingInstallable,
    /// Docker < 24, or compose v1 only.
    InstalledUnsupportedVersion,
    /// `podman-docker`, distro `docker.io`, the snap, rootless-only, or a
    /// foreign socket owner.
    ConflictingRuntime,
    /// No runtime on an unsupported target.
    InstallationNotSupported,
}

/// What the bootstrap saw of a container runtime.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DockerObservation {
    /// `docker` on PATH.
    pub cli_present: bool,
    /// Server version, when the daemon answered.
    pub server_version: Option<String>,
    /// `docker compose version` reports v2.
    pub compose_v2: bool,
    /// Conflicting runtimes found (closed codes).
    pub conflicts: Vec<RuntimeConflict>,
    /// Docker's apt source is already configured.
    pub docker_repo_present: bool,
}

/// A runtime the Installer refuses to work around.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RuntimeConflict {
    /// `podman-docker` installed.
    PodmanDocker,
    /// Ubuntu's own `docker.io` package.
    DistroDockerIo,
    /// The `docker` snap.
    DockerSnap,
    /// Only a rootless Docker.
    RootlessOnly,
    /// `/var/run/docker.sock` belongs to something that is not Docker.
    ForeignSocket,
}

/// Classify the container runtime.
#[must_use]
pub fn classify_runtime(
    supported_target: bool,
    docker: &DockerObservation,
) -> ContainerRuntimeState {
    if !docker.conflicts.is_empty() {
        return ContainerRuntimeState::ConflictingRuntime;
    }
    if !docker.cli_present && docker.server_version.is_none() {
        return if supported_target {
            ContainerRuntimeState::MissingInstallable
        } else {
            ContainerRuntimeState::InstallationNotSupported
        };
    }
    match &docker.server_version {
        Some(v) if crate::manifest::version_at_least(v, "24.0") && docker.compose_v2 => {
            ContainerRuntimeState::DetectedSupported
        }
        Some(v) if crate::manifest::version_at_least(v, "24.0") => {
            // Engine is fine, the compose v2 plugin is not there.
            ContainerRuntimeState::InstalledUnsupportedVersion
        }
        Some(_) => ContainerRuntimeState::InstalledUnsupportedVersion,
        // A CLI with no answering daemon is a runtime someone installed and
        // stopped: not ours to start, replace or remove.
        None => ContainerRuntimeState::ConflictingRuntime,
    }
}

/// `FirewallState` (firewall policy).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FirewallState {
    /// No active firewall manager or drop policy.
    FirewallNotPresent,
    /// ufw active (intermediate classification).
    FirewallCompatible,
    /// ufw already allows 80/tcp and 443/tcp.
    RequiredRulesAlreadyPresent,
    /// ufw active and the rules are missing: plan item P08.
    RequiredRulesCanBeApplied,
    /// firewalld, custom nftables/iptables, or unknown: never touched.
    ExternalFirewallActionRequired,
    /// ufw explicitly denies 80 or 443.
    FirewallConflict,
    /// After installation, the operator cannot reach 80/443.
    FirewallVerificationFailed,
}

/// What the bootstrap saw of the firewall.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FirewallObservation {
    /// `ufw status` says active.
    pub ufw_active: bool,
    /// ufw allows these ports (tcp) for incoming traffic.
    pub ufw_allows: Vec<u16>,
    /// ufw denies or rejects these ports explicitly.
    pub ufw_denies: Vec<u16>,
    /// firewalld is active.
    pub firewalld_active: bool,
    /// An nftables/iptables input chain with a drop/reject policy that ufw does
    /// not manage.
    pub custom_input_drop: bool,
}

/// Classify the firewall. Ports 80 and 443 are what Ocinye needs.
#[must_use]
pub fn classify_firewall(fw: &FirewallObservation) -> FirewallState {
    if fw.ufw_active {
        if fw.ufw_denies.iter().any(|p| matches!(p, 80 | 443)) {
            FirewallState::FirewallConflict
        } else if fw.ufw_allows.contains(&80) && fw.ufw_allows.contains(&443) {
            FirewallState::RequiredRulesAlreadyPresent
        } else {
            FirewallState::RequiredRulesCanBeApplied
        }
    } else if fw.firewalld_active || fw.custom_input_drop {
        FirewallState::ExternalFirewallActionRequired
    } else {
        FirewallState::FirewallNotPresent
    }
}

/// The ufw ports the plan must open (the ones not yet allowed).
#[must_use]
pub fn missing_ufw_ports(fw: &FirewallObservation) -> Vec<u16> {
    [80, 443]
        .into_iter()
        .filter(|p| !fw.ufw_allows.contains(p))
        .collect()
}

/// A foreign listener on 80/443.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortUse {
    /// The port.
    pub port: u16,
    /// Process name, when visible.
    pub process: Option<String>,
    /// It is an Ocinye container.
    pub ocinye: bool,
}

/// What a check observed: typed, so the UI writes it in the operator's
/// language and nothing free-form crosses the channel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Observation {
    /// PF-OS.
    Os {
        /// `uname -s`.
        kernel_name: String,
        /// `uname -r`.
        kernel_release: String,
    },
    /// PF-DISTRO.
    Distro {
        /// `/etc/os-release` `ID`.
        id: String,
        /// `VERSION_ID`.
        version_id: String,
        /// `PRETTY_NAME`.
        pretty: String,
        /// A graphical environment is installed.
        graphical: bool,
    },
    /// PF-ARCH.
    Arch {
        /// The server's.
        server: String,
        /// The release's.
        release: Arch,
    },
    /// PF-PRIV.
    Privilege {
        /// What the SSH user may do.
        privilege: Privilege,
    },
    /// PF-TIME.
    Clock {
        /// Server − operator, in seconds.
        skew_seconds: i64,
    },
    /// PF-CPU.
    Cpu {
        /// Online vCPUs.
        count: u32,
    },
    /// PF-RAM.
    Ram {
        /// MemTotal in MB.
        total_mb: u64,
        /// SwapTotal in MB.
        swap_mb: u64,
    },
    /// PF-DISK.
    Disk {
        /// The path measured.
        path: String,
        /// Free GB (floor).
        free_gb: u64,
    },
    /// PF-HWREAD.
    Hardware {
        /// `/proc`, `/sys` and PCI were all readable.
        readable: bool,
    },
    /// PF-DOCKER.
    Docker {
        /// Classification.
        state: ContainerRuntimeState,
        /// What was seen.
        observed: DockerObservation,
    },
    /// PF-PORTS.
    Ports {
        /// Listeners on 80/443.
        busy: Vec<PortUse>,
    },
    /// PF-PROXY.
    Proxy {
        /// Reverse-proxy units active (`nginx`, `caddy`, `traefik`, `apache2`).
        units: Vec<String>,
    },
    /// PF-FW.
    Firewall {
        /// Classification.
        state: FirewallState,
        /// What was seen.
        observed: FirewallObservation,
    },
    /// PF-PKG.
    Packages {
        /// Missing packages the plan installs (`ca-certificates`, `curl`).
        installable: Vec<String>,
        /// Missing tools that block (`bash`, `tar`, coreutils).
        missing: Vec<String>,
    },
    /// PF-REGISTRY.
    Registry {
        /// HTTPS to the registries answered.
        reachable: bool,
    },
    /// PF-SYSTEMD.
    Systemd {
        /// systemd is PID 1.
        present: bool,
    },
    /// PF-EXIST.
    Existing {
        /// The release found (`/etc/ocinye/release.env`).
        release: Option<String>,
        /// `/etc/ocinye/core.env` exists.
        config_present: bool,
    },
    /// PF-JOURNAL.
    Journal {
        /// An installer journal exists.
        found: bool,
    },
    /// PF-SRVDIR.
    SrvDir {
        /// `/srv/ocinye` exists and is non-empty.
        non_empty: bool,
        /// It is explained by a journal of this Installer.
        journalled: bool,
    },
}

/// What the SSH user may do on the server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Privilege {
    /// Logged in as root.
    Root,
    /// sudo without a password.
    SudoNoPassword,
    /// sudo with the user's password (asked in I05, kept in memory).
    SudoPassword,
    /// Neither.
    None,
}

/// One preflight result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreflightItem {
    /// Which check.
    pub id: PreflightCheckId,
    /// Verdict.
    pub status: CheckStatus,
    /// What the plan will do about it.
    pub action: Option<PlannedAction>,
    /// What was seen.
    pub observation: Observation,
}

/// The whole report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreflightReport {
    /// One per check, in [`PreflightCheckId::ALL`] order.
    pub items: Vec<PreflightItem>,
    /// An incomplete installation of this Installer, if any (I19).
    pub incomplete: Option<JournalSummary>,
}

impl PreflightReport {
    /// The item for a check.
    #[must_use]
    pub fn item(&self, id: PreflightCheckId) -> Option<&PreflightItem> {
        self.items.iter().find(|i| i.id == id)
    }

    /// Install is available only when nothing is blocked and every check was
    /// reported exactly once.
    #[must_use]
    pub fn install_allowed(&self) -> bool {
        PreflightCheckId::ALL
            .iter()
            .all(|id| self.items.iter().filter(|i| i.id == *id).count() == 1)
            && !self.items.iter().any(|i| i.status == CheckStatus::Blocked)
    }

    /// (pass, warning, blocked).
    #[must_use]
    pub fn summary(&self) -> (usize, usize, usize) {
        let count = |s| self.items.iter().filter(|i| i.status == s).count();
        (
            count(CheckStatus::Pass),
            count(CheckStatus::Warning),
            count(CheckStatus::Blocked),
        )
    }

    /// The blocking subset, as compared again in P03: what blocked or passed
    /// at review must still be so when the plan runs.
    #[must_use]
    pub fn blocking_snapshot(&self) -> Vec<(PreflightCheckId, CheckStatus)> {
        self.items
            .iter()
            .map(|i| {
                (
                    i.id,
                    if i.status == CheckStatus::Blocked {
                        CheckStatus::Blocked
                    } else {
                        CheckStatus::Pass
                    },
                )
            })
            .collect()
    }
}

// ── thresholds (from install/ocinye and docs/install; none is new) ──────────

/// install/ocinye `MIN_RAM_MB`.
pub const MIN_RAM_MB: u64 = 3500;
/// Recommended memory (docs/install).
pub const RECOMMENDED_RAM_MB: u64 = 8 * 1024;
/// install/ocinye `MIN_DISK_GB`.
pub const MIN_DISK_GB: u64 = 15;
/// Minimum vCPU.
pub const MIN_CPU: u32 = 2;
/// Recommended vCPU.
pub const RECOMMENDED_CPU: u32 = 4;

/// PF-CPU.
#[must_use]
pub const fn judge_cpu(count: u32) -> CheckStatus {
    if count < MIN_CPU {
        CheckStatus::Blocked
    } else if count < RECOMMENDED_CPU {
        CheckStatus::Warning
    } else {
        CheckStatus::Pass
    }
}

/// PF-RAM.
#[must_use]
pub const fn judge_ram(total_mb: u64) -> CheckStatus {
    if total_mb < MIN_RAM_MB {
        CheckStatus::Blocked
    } else if total_mb < RECOMMENDED_RAM_MB {
        CheckStatus::Warning
    } else {
        CheckStatus::Pass
    }
}

/// PF-DISK.
#[must_use]
pub const fn judge_disk(free_gb: u64) -> CheckStatus {
    if free_gb < MIN_DISK_GB {
        CheckStatus::Blocked
    } else {
        CheckStatus::Pass
    }
}

/// PF-TIME.
#[must_use]
pub const fn judge_clock(skew_seconds: i64) -> CheckStatus {
    let s = skew_seconds.unsigned_abs();
    if s > 300 {
        CheckStatus::Blocked
    } else if s > 60 {
        CheckStatus::Warning
    } else {
        CheckStatus::Pass
    }
}

/// PF-DISTRO: Ubuntu 24.04, no graphical environment — and nothing else.
#[must_use]
pub fn judge_distro(id: &str, version_id: &str, graphical: bool) -> CheckStatus {
    if id == "ubuntu" && version_id == "24.04" && !graphical {
        CheckStatus::Pass
    } else {
        CheckStatus::Blocked
    }
}

/// PF-DOCKER verdict and badge.
#[must_use]
pub const fn judge_runtime(state: ContainerRuntimeState) -> (CheckStatus, Option<PlannedAction>) {
    match state {
        ContainerRuntimeState::DetectedSupported => (CheckStatus::Pass, None),
        ContainerRuntimeState::MissingInstallable => {
            (CheckStatus::Warning, Some(PlannedAction::WillInstall))
        }
        _ => (CheckStatus::Blocked, None),
    }
}

/// PF-FW verdict and badge.
#[must_use]
pub const fn judge_firewall(state: FirewallState) -> (CheckStatus, Option<PlannedAction>) {
    match state {
        FirewallState::FirewallNotPresent | FirewallState::RequiredRulesAlreadyPresent => {
            (CheckStatus::Pass, None)
        }
        FirewallState::FirewallCompatible | FirewallState::RequiredRulesCanBeApplied => {
            (CheckStatus::Warning, Some(PlannedAction::WillApply))
        }
        FirewallState::ExternalFirewallActionRequired
        | FirewallState::FirewallVerificationFailed => {
            (CheckStatus::Warning, Some(PlannedAction::ExternalAction))
        }
        FirewallState::FirewallConflict => (CheckStatus::Blocked, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn docker(cli: bool, v: Option<&str>, compose: bool) -> DockerObservation {
        DockerObservation {
            cli_present: cli,
            server_version: v.map(str::to_owned),
            compose_v2: compose,
            conflicts: vec![],
            docker_repo_present: false,
        }
    }

    #[test]
    fn o_runtime_ausente_so_se_instala_no_alvo_suportado() {
        assert_eq!(
            classify_runtime(true, &docker(false, None, false)),
            ContainerRuntimeState::MissingInstallable
        );
        assert_eq!(
            classify_runtime(false, &docker(false, None, false)),
            ContainerRuntimeState::InstallationNotSupported
        );
        assert_eq!(
            judge_runtime(ContainerRuntimeState::MissingInstallable),
            (CheckStatus::Warning, Some(PlannedAction::WillInstall)),
            "Docker em falta não bloqueia: entra no plano"
        );
    }

    #[test]
    fn um_runtime_existente_e_usado_ou_recusado_nunca_substituido() {
        assert_eq!(
            classify_runtime(true, &docker(true, Some("27.3.1"), true)),
            ContainerRuntimeState::DetectedSupported
        );
        assert_eq!(
            classify_runtime(true, &docker(true, Some("20.10.24"), true)),
            ContainerRuntimeState::InstalledUnsupportedVersion
        );
        assert_eq!(
            classify_runtime(true, &docker(true, Some("27.0.0"), false)),
            ContainerRuntimeState::InstalledUnsupportedVersion,
            "compose v1 só"
        );
        assert_eq!(
            classify_runtime(true, &docker(true, None, false)),
            ContainerRuntimeState::ConflictingRuntime,
            "um Docker parado não é nosso para arrancar"
        );
        let mut c = docker(true, Some("27.3.1"), true);
        c.conflicts.push(RuntimeConflict::DistroDockerIo);
        assert_eq!(
            classify_runtime(true, &c),
            ContainerRuntimeState::ConflictingRuntime
        );
        for s in [
            ContainerRuntimeState::InstalledUnsupportedVersion,
            ContainerRuntimeState::ConflictingRuntime,
            ContainerRuntimeState::InstallationNotSupported,
        ] {
            assert_eq!(judge_runtime(s), (CheckStatus::Blocked, None));
        }
    }

    #[test]
    fn a_firewall_so_e_tocada_quando_e_ufw() {
        let none = FirewallObservation::default();
        assert_eq!(classify_firewall(&none), FirewallState::FirewallNotPresent);
        let ufw = FirewallObservation {
            ufw_active: true,
            ufw_allows: vec![22],
            ..Default::default()
        };
        assert_eq!(
            classify_firewall(&ufw),
            FirewallState::RequiredRulesCanBeApplied
        );
        assert_eq!(missing_ufw_ports(&ufw), [80, 443]);
        let done = FirewallObservation {
            ufw_active: true,
            ufw_allows: vec![22, 80, 443],
            ..Default::default()
        };
        assert_eq!(
            classify_firewall(&done),
            FirewallState::RequiredRulesAlreadyPresent
        );
        assert!(missing_ufw_ports(&done).is_empty(), "regras duplicadas");
        let deny = FirewallObservation {
            ufw_active: true,
            ufw_denies: vec![443],
            ..Default::default()
        };
        assert_eq!(classify_firewall(&deny), FirewallState::FirewallConflict);
        let fwd = FirewallObservation {
            firewalld_active: true,
            ..Default::default()
        };
        assert_eq!(
            classify_firewall(&fwd),
            FirewallState::ExternalFirewallActionRequired
        );
        assert_eq!(
            judge_firewall(FirewallState::ExternalFirewallActionRequired).0,
            CheckStatus::Warning,
            "a firewall externa não bloqueia a instalação"
        );
    }

    #[test]
    fn so_ubuntu_24_04_sem_ambiente_grafico_passa() {
        assert_eq!(judge_distro("ubuntu", "24.04", false), CheckStatus::Pass);
        for (id, v, g) in [
            ("ubuntu", "24.04", true),
            ("ubuntu", "22.04", false),
            ("ubuntu", "24.10", false),
            ("debian", "12", false),
            ("fedora", "40", false),
        ] {
            assert_eq!(judge_distro(id, v, g), CheckStatus::Blocked, "{id} {v} {g}");
        }
    }

    #[test]
    fn os_limiares_sao_os_do_instalador() {
        assert_eq!(judge_ram(3499), CheckStatus::Blocked);
        assert_eq!(judge_ram(3500), CheckStatus::Warning);
        assert_eq!(judge_ram(8192), CheckStatus::Pass);
        assert_eq!(judge_cpu(1), CheckStatus::Blocked);
        assert_eq!(judge_cpu(2), CheckStatus::Warning);
        assert_eq!(judge_disk(14), CheckStatus::Blocked);
        assert_eq!(judge_clock(-301), CheckStatus::Blocked);
        assert_eq!(judge_clock(61), CheckStatus::Warning);
    }

    #[test]
    fn instalar_exige_cada_verificacao_uma_vez_e_nenhum_bloqueio() {
        let item = |id, status| PreflightItem {
            id,
            status,
            action: None,
            observation: Observation::Journal { found: false },
        };
        let mut r = PreflightReport {
            items: PreflightCheckId::ALL
                .iter()
                .map(|id| item(*id, CheckStatus::Pass))
                .collect(),
            incomplete: None,
        };
        assert!(r.install_allowed());
        r.items.pop();
        assert!(
            !r.install_allowed(),
            "uma verificação em falta deixou instalar"
        );
        r.items
            .push(item(PreflightCheckId::PfSrvdir, CheckStatus::Blocked));
        assert!(!r.install_allowed());
    }
}
