//! Preflight on the server. **Read-only**: nothing here writes outside the
//! bootstrap's own temporary directory — no package, no repository, no rule,
//! no directory, no service (D011_PREFLIGHT_MATRIX). The verdicts come from
//! the contracts crate, so the Installer and the bootstrap judge alike.

use std::fs;
use std::net::{TcpStream, ToSocketAddrs};
use std::path::Path;
use std::time::Duration;

use ocinye_installer_contracts::manifest::Arch;
use ocinye_installer_contracts::preflight::{
    self as pf, CheckStatus, DockerObservation, FirewallObservation, Observation, PlannedAction,
    PortUse, PreflightCheckId, PreflightItem, PreflightReport, Privilege, RuntimeConflict,
};
use ocinye_installer_contracts::protocol::ServerFacts;
use ocinye_installer_contracts::{paths, SUPPORTED_TARGET};

use crate::exec::{self, program};
use crate::hardware::{self, Root};
use crate::state::StateRoot;

/// `ss -H -ltnp` → listeners on 80/443.
#[must_use]
pub fn parse_ss(out: &str) -> Vec<PortUse> {
    let mut busy = Vec::new();
    for line in out.lines() {
        let cols: Vec<&str> = line.split_whitespace().collect();
        let Some(local) = cols.get(3) else { continue };
        let Some(port) = local.rsplit(':').next().and_then(|p| p.parse::<u16>().ok()) else {
            continue;
        };
        if !matches!(port, 80 | 443) {
            continue;
        }
        let process = line
            .split("((\"")
            .nth(1)
            .and_then(|r| r.split('"').next())
            .map(str::to_owned);
        if busy.iter().any(|b: &PortUse| b.port == port) {
            continue;
        }
        busy.push(PortUse {
            port,
            ocinye: false,
            process,
        });
    }
    busy
}

fn port_in(spec: &str, port: u16) -> bool {
    // `80`, `80/tcp`, `80,443/tcp`, `8000:9000/tcp`; never `/udp`.
    let (ports, proto) = spec.split_once('/').unwrap_or((spec, "tcp"));
    if proto != "tcp" {
        return false;
    }
    ports.split(',').any(|p| match p.split_once(':') {
        Some((a, b)) => match (a.parse::<u16>(), b.parse::<u16>()) {
            (Ok(a), Ok(b)) => (a..=b).contains(&port),
            _ => false,
        },
        None => p.parse::<u16>() == Ok(port),
    })
}

/// `ufw status` → (active, allowed ports of 80/443, denied ports of 80/443).
#[must_use]
pub fn parse_ufw_status(out: &str) -> (bool, Vec<u16>, Vec<u16>) {
    let active = out.lines().any(|l| l.trim() == "Status: active");
    let mut allows = Vec::new();
    let mut denies = Vec::new();
    for line in out.lines() {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() < 3 {
            continue;
        }
        let to = cols[0];
        // The action is the first column that is an action word.
        let Some(action) = cols
            .iter()
            .skip(1)
            .find(|c| matches!(**c, "ALLOW" | "DENY" | "REJECT" | "LIMIT"))
        else {
            continue;
        };
        // Outgoing rules (`ALLOW OUT`) do not open a port to the world.
        if line.contains(" OUT ") {
            continue;
        }
        for port in [80u16, 443] {
            if port_in(to, port) {
                match *action {
                    "ALLOW" | "LIMIT" if !allows.contains(&port) => allows.push(port),
                    "DENY" | "REJECT" if !denies.contains(&port) => denies.push(port),
                    _ => {}
                }
            }
        }
    }
    (active, allows, denies)
}

/// `nft list ruleset` → an input chain with a drop/reject policy.
///
/// Only consulted when ufw is not active: an active ufw owns the input policy,
/// and its rules are judged by [`parse_ufw_status`].
#[must_use]
pub fn nft_custom_input_drop(ruleset: &str) -> bool {
    ruleset.lines().any(|l| {
        l.contains("hook input") && (l.contains("policy drop") || l.contains("policy reject"))
    })
}

fn dpkg_installed(package: &str) -> bool {
    let o = exec::run(
        program::DPKG_QUERY,
        &["-W", "-f=${Status}", package],
        exec::secs(10),
    );
    o.ok() && o.stdout.contains("install ok installed")
}

fn systemd_active(unit: &str) -> bool {
    exec::run(
        program::SYSTEMCTL,
        &["is-active", "--quiet", unit],
        exec::secs(10),
    )
    .ok()
}

/// What the bootstrap sees of a container runtime.
#[must_use]
pub fn observe_docker() -> DockerObservation {
    let cli = exec::exists(program::DOCKER);
    let (server_version, compose_v2) = if cli {
        let v = exec::run(
            program::DOCKER,
            &["version", "-f", "{{.Server.Version}}"],
            exec::secs(15),
        );
        let c = exec::run(
            program::DOCKER,
            &["compose", "version", "--short"],
            exec::secs(15),
        );
        (
            v.ok()
                .then(|| v.stdout.trim().to_owned())
                .filter(|s| !s.is_empty()),
            c.ok() && pf::compose_plugin_supported(&c.stdout),
        )
    } else {
        (None, false)
    };
    let mut conflicts = Vec::new();
    if dpkg_installed("podman-docker") {
        conflicts.push(RuntimeConflict::PodmanDocker);
    }
    if dpkg_installed("docker.io") {
        conflicts.push(RuntimeConflict::DistroDockerIo);
    }
    if exec::exists(program::SNAP)
        && exec::run(program::SNAP, &["list", "docker"], exec::secs(15)).ok()
    {
        conflicts.push(RuntimeConflict::DockerSnap);
    }
    // Something answers on the Docker socket without a Docker CLI: a runtime
    // that is not ours. A socket file nobody listens on (left by a purged
    // package until the next boot) is not a runtime.
    if !cli
        && server_version.is_none()
        && std::os::unix::net::UnixStream::connect("/var/run/docker.sock").is_ok()
    {
        conflicts.push(RuntimeConflict::ForeignSocket);
    }
    DockerObservation {
        cli_present: cli,
        server_version,
        compose_v2,
        conflicts,
        docker_repo_present: Path::new("/etc/apt/sources.list.d/docker.sources").exists()
            || Path::new("/etc/apt/sources.list.d/docker.list").exists(),
    }
}

/// What the bootstrap sees of the firewall.
#[must_use]
pub fn observe_firewall() -> FirewallObservation {
    let (ufw_active, ufw_allows, ufw_denies) = if exec::exists(program::UFW) {
        parse_ufw_status(&exec::run(program::UFW, &["status"], exec::secs(15)).stdout)
    } else {
        (false, Vec::new(), Vec::new())
    };
    let custom_input_drop = !ufw_active
        && exec::exists(program::NFT)
        && nft_custom_input_drop(
            &exec::run(program::NFT, &["list", "ruleset"], exec::secs(15)).stdout,
        );
    FirewallObservation {
        ufw_active,
        ufw_allows,
        ufw_denies,
        firewalld_active: systemd_active("firewalld"),
        custom_input_drop,
    }
}

/// A graphical environment is installed (Ubuntu Desktop and friends).
///
/// Not `systemctl get-default`: Ubuntu's minimal **server** cloud image
/// defaults to `graphical.target` with no graphical stack at all — observed on
/// the certification image, where that heuristic blocked the one supported
/// target. What makes a desktop is a session or a display manager.
fn graphical() -> bool {
    let has_entries = |p: &str| {
        fs::read_dir(p)
            .map(|mut d| d.next().is_some())
            .unwrap_or(false)
    };
    has_entries("/usr/share/xsessions")
        || has_entries("/usr/share/wayland-sessions")
        || Path::new("/etc/systemd/system/display-manager.service").exists()
}

fn registry_reachable() -> bool {
    ["registry-1.docker.io:443", "download.docker.com:443"]
        .iter()
        .all(|host| {
            host.to_socket_addrs()
                .ok()
                .and_then(|mut a| a.next())
                .is_some_and(|addr| {
                    TcpStream::connect_timeout(&addr, Duration::from_secs(5)).is_ok()
                })
        })
}

fn item(
    id: PreflightCheckId,
    status: CheckStatus,
    action: Option<PlannedAction>,
    observation: Observation,
) -> PreflightItem {
    PreflightItem {
        id,
        status,
        action,
        observation,
    }
}

/// Run every check. `emit` receives each item as it is decided.
#[allow(clippy::too_many_lines)]
pub fn run(
    facts: &ServerFacts,
    release_arch: Arch,
    operator_unix_time: i64,
    root: &Root,
    state: &StateRoot,
    emit: &mut dyn FnMut(&PreflightItem),
) -> PreflightReport {
    let mut items = Vec::new();
    let mut push = |i: PreflightItem, items: &mut Vec<PreflightItem>| {
        emit(&i);
        items.push(i);
    };

    let linux64 = facts.kernel_name == "Linux" && facts.arch.is_some();
    push(
        item(
            PreflightCheckId::PfOs,
            if linux64 {
                CheckStatus::Pass
            } else {
                CheckStatus::Blocked
            },
            None,
            Observation::Os {
                kernel_name: facts.kernel_name.clone(),
                kernel_release: facts.kernel_release.clone(),
            },
        ),
        &mut items,
    );
    let gui = graphical();
    push(
        item(
            PreflightCheckId::PfDistro,
            pf::judge_distro(&facts.os.id, &facts.os.version_id, gui),
            None,
            Observation::Distro {
                id: facts.os.id.clone(),
                version_id: facts.os.version_id.clone(),
                pretty: facts.os.pretty.clone(),
                graphical: gui,
            },
        ),
        &mut items,
    );
    let supported_target =
        format!("{}-{}", facts.os.id, facts.os.version_id) == SUPPORTED_TARGET && !gui;
    push(
        item(
            PreflightCheckId::PfArch,
            if facts.arch == Some(release_arch) {
                CheckStatus::Pass
            } else {
                CheckStatus::Blocked
            },
            None,
            Observation::Arch {
                server: facts.machine.clone(),
                release: release_arch,
            },
        ),
        &mut items,
    );
    push(
        item(
            PreflightCheckId::PfPriv,
            if facts.privilege == Privilege::None {
                CheckStatus::Blocked
            } else {
                CheckStatus::Pass
            },
            None,
            Observation::Privilege {
                privilege: facts.privilege,
            },
        ),
        &mut items,
    );
    let skew = chrono::Utc::now().timestamp() - operator_unix_time;
    push(
        item(
            PreflightCheckId::PfTime,
            pf::judge_clock(skew),
            None,
            Observation::Clock { skew_seconds: skew },
        ),
        &mut items,
    );

    let hw = hardware::discover(root, &facts.machine);
    push(
        item(
            PreflightCheckId::PfCpu,
            pf::judge_cpu(hw.cpu.threads),
            None,
            Observation::Cpu {
                count: hw.cpu.threads,
            },
        ),
        &mut items,
    );
    let ram_mb = hw.memory.total_bytes / (1024 * 1024);
    push(
        item(
            PreflightCheckId::PfRam,
            pf::judge_ram(ram_mb),
            None,
            Observation::Ram {
                total_mb: ram_mb,
                swap_mb: hw.memory.swap_bytes / (1024 * 1024),
            },
        ),
        &mut items,
    );
    let free_gb = hw.storage.free_bytes / (1024 * 1024 * 1024);
    push(
        item(
            PreflightCheckId::PfDisk,
            pf::judge_disk(free_gb),
            None,
            Observation::Disk {
                path: hw.storage.path.clone(),
                free_gb,
            },
        ),
        &mut items,
    );
    let readable = hardware::readable(root);
    push(
        item(
            PreflightCheckId::PfHwread,
            if readable {
                CheckStatus::Pass
            } else {
                CheckStatus::Warning
            },
            None,
            Observation::Hardware { readable },
        ),
        &mut items,
    );

    let docker = observe_docker();
    let runtime = pf::classify_runtime(supported_target, &docker);
    let (st, action) = pf::judge_runtime(runtime);
    push(
        item(
            PreflightCheckId::PfDocker,
            st,
            action,
            Observation::Docker {
                state: runtime,
                observed: docker,
            },
        ),
        &mut items,
    );

    let mut busy = parse_ss(&exec::run(program::SS, &["-H", "-ltnp"], exec::secs(10)).stdout);
    for b in &mut busy {
        b.ocinye = b.process.as_deref() == Some("docker-proxy")
            && Path::new(&format!("{}/core.env", paths::CONFIG)).exists();
    }
    push(
        item(
            PreflightCheckId::PfPorts,
            if busy.is_empty() {
                CheckStatus::Pass
            } else {
                CheckStatus::Blocked
            },
            None,
            Observation::Ports { busy },
        ),
        &mut items,
    );
    let units: Vec<String> = ["nginx", "caddy", "traefik", "apache2", "haproxy"]
        .iter()
        .filter(|u| systemd_active(u))
        .map(|u| (*u).to_owned())
        .collect();
    push(
        item(
            PreflightCheckId::PfProxy,
            if units.is_empty() {
                CheckStatus::Pass
            } else {
                CheckStatus::Warning
            },
            None,
            Observation::Proxy { units },
        ),
        &mut items,
    );
    let fw = observe_firewall();
    let fw_state = pf::classify_firewall(&fw);
    let (st, action) = pf::judge_firewall(fw_state);
    push(
        item(
            PreflightCheckId::PfFw,
            st,
            action,
            Observation::Firewall {
                state: fw_state,
                observed: fw,
            },
        ),
        &mut items,
    );
    let installable: Vec<String> = ["ca-certificates", "curl"]
        .iter()
        .filter(|p| !dpkg_installed(p))
        .map(|p| (*p).to_owned())
        .collect();
    let missing: Vec<String> = [
        ("bash", "/usr/bin/bash"),
        ("tar", "/usr/bin/tar"),
        ("coreutils", "/usr/bin/sha256sum"),
    ]
    .iter()
    .filter(|(_, path)| !Path::new(path).exists())
    .map(|(n, _)| (*n).to_owned())
    .collect();
    push(
        item(
            PreflightCheckId::PfPkg,
            if !missing.is_empty() || (!installable.is_empty() && !supported_target) {
                CheckStatus::Blocked
            } else if installable.is_empty() {
                CheckStatus::Pass
            } else {
                CheckStatus::Warning
            },
            (!installable.is_empty() && missing.is_empty() && supported_target)
                .then_some(PlannedAction::WillInstall),
            Observation::Packages {
                installable,
                missing,
            },
        ),
        &mut items,
    );
    let reachable = registry_reachable();
    push(
        item(
            PreflightCheckId::PfRegistry,
            if reachable {
                CheckStatus::Pass
            } else {
                CheckStatus::Blocked
            },
            None,
            Observation::Registry { reachable },
        ),
        &mut items,
    );
    push(
        item(
            PreflightCheckId::PfSystemd,
            if facts.systemd {
                CheckStatus::Pass
            } else {
                CheckStatus::Warning
            },
            None,
            Observation::Systemd {
                present: facts.systemd,
            },
        ),
        &mut items,
    );

    let journals = state.journals();
    let incomplete = journals
        .iter()
        .find(|j| j.state != ocinye_installer_contracts::journal::JournalState::Completed);
    let config_present = Path::new(&format!("{}/core.env", paths::CONFIG)).exists();
    let release = fs::read_to_string(format!("{}/release.env", paths::CONFIG))
        .ok()
        .and_then(|t| {
            t.lines()
                .find_map(|l| l.strip_prefix("OCINYE_RELEASE_SHA="))
                .map(str::to_owned)
        });
    let explained = incomplete.is_some();
    push(
        item(
            PreflightCheckId::PfExist,
            if config_present && !explained {
                CheckStatus::Blocked
            } else if config_present {
                CheckStatus::NotApplicable
            } else {
                CheckStatus::Pass
            },
            None,
            Observation::Existing {
                release,
                config_present,
            },
        ),
        &mut items,
    );
    push(
        item(
            PreflightCheckId::PfJournal,
            if incomplete.is_some() {
                CheckStatus::Warning
            } else {
                CheckStatus::Pass
            },
            None,
            Observation::Journal {
                found: !journals.is_empty(),
            },
        ),
        &mut items,
    );
    let srv = Path::new(paths::ROOT);
    let non_empty = fs::read_dir(srv)
        .map(|mut d| d.next().is_some())
        .unwrap_or(false);
    let journalled = incomplete.is_some_and(|j| j.created_paths.iter().any(|p| p == paths::ROOT));
    push(
        item(
            PreflightCheckId::PfSrvdir,
            if non_empty && !journalled && !config_present {
                CheckStatus::Blocked
            } else {
                CheckStatus::Pass
            },
            None,
            Observation::SrvDir {
                non_empty,
                journalled,
            },
        ),
        &mut items,
    );

    PreflightReport {
        items,
        incomplete: incomplete
            .map(ocinye_installer_contracts::journal::InstallationJournal::summary),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_portas_ocupadas_le_se_do_ss() {
        let out = "LISTEN 0 511 0.0.0.0:80 0.0.0.0:* users:((\"caddy\",pid=812,fd=6))\nLISTEN 0 4096 [::]:443 [::]:* users:((\"docker-proxy\",pid=9,fd=4))\nLISTEN 0 128 0.0.0.0:22 0.0.0.0:* users:((\"sshd\",pid=1,fd=3))\n";
        let busy = parse_ss(out);
        assert_eq!(busy.len(), 2);
        assert_eq!(busy[0].port, 80);
        assert_eq!(busy[0].process.as_deref(), Some("caddy"));
        assert_eq!(busy[1].process.as_deref(), Some("docker-proxy"));
        assert!(parse_ss("").is_empty());
    }

    #[test]
    fn o_estado_do_ufw_le_se_regra_a_regra() {
        let out = "Status: active\n\nTo                         Action      From\n--                         ------      ----\n22/tcp                     ALLOW       Anywhere\n80/tcp                     ALLOW       Anywhere                   # ocinye\n443                        DENY        Anywhere\n53/udp                     ALLOW       Anywhere\n22/tcp (v6)                ALLOW       Anywhere (v6)\n";
        let (active, allows, denies) = parse_ufw_status(out);
        assert!(active);
        assert_eq!(allows, [80]);
        assert_eq!(denies, [443]);
        let (active, allows, _) = parse_ufw_status("Status: inactive\n");
        assert!(!active && allows.is_empty());
        let (_, allows, _) = parse_ufw_status(
            "Status: active\n80,443/tcp ALLOW Anywhere\n8000:9000/tcp ALLOW Anywhere\n",
        );
        assert_eq!(allows, [80, 443]);
        let (_, allows, _) = parse_ufw_status(
            "Status: active\n443/tcp ALLOW OUT Anywhere\n443/udp ALLOW Anywhere\n",
        );
        assert!(allows.is_empty(), "nem a saída nem o udp abrem o 443");
    }

    #[test]
    fn uma_politica_de_entrada_propria_e_uma_firewall_externa() {
        let custom = "table inet filter {\n\tchain input {\n\t\ttype filter hook input priority filter; policy drop;\n\t}\n}\n";
        assert!(nft_custom_input_drop(custom));
        let open = "table inet filter {\n\tchain input {\n\t\ttype filter hook input priority filter; policy accept;\n\t}\n}\n";
        assert!(!nft_custom_input_drop(open));
        assert!(!nft_custom_input_drop(""));
    }
}
