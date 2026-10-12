//! Ubuntu base maintenance (ADR-0032 §11): what is pending, from approved
//! sources only, under the maintenance lock, with an interrupted `dpkg`
//! repaired first. There is no atomic rollback of base packages and none is
//! claimed; the previous kernel stays installed by Ubuntu's own policy.

use crate::host::{AptOp, Host};
use crate::paths::Paths;
use crate::store::{write_atomic, MaintenanceLock};
use ocinye_update_contracts::{BaseOutcome, BasePhase, BaseState, UpdateReason};
use std::path::Path;

/// Hosts a package source may name. Anything else makes the sources
/// unapproved, and base maintenance refuses to run.
pub const APPROVED_HOSTS: [&str; 5] = [
    "archive.ubuntu.com",
    "security.ubuntu.com",
    "ports.ubuntu.com",
    "snapshot.ubuntu.com",
    "download.docker.com",
];

fn host_of(uri: &str) -> Option<&str> {
    let rest = uri
        .strip_prefix("https://")
        .or_else(|| uri.strip_prefix("http://"))?;
    let host = rest.split('/').next()?;
    Some(host.rsplit('@').next().unwrap_or(host))
}

fn approved(uri: &str) -> bool {
    host_of(uri).is_some_and(|h| {
        APPROVED_HOSTS
            .iter()
            .any(|a| h == *a || h.ends_with(&format!(".{a}")))
    })
}

/// Every URI in `/etc/apt/sources.list` and `sources.list.d/*` is approved.
#[must_use]
pub fn sources_approved(apt: &Path) -> bool {
    let mut uris = Vec::new();
    let mut read = |p: &Path| {
        let Ok(text) = std::fs::read_to_string(p) else {
            return;
        };
        for line in text.lines().map(str::trim) {
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            if let Some(rest) = line.strip_prefix("URIs:") {
                uris.extend(rest.split_whitespace().map(str::to_owned));
            } else if let Some(rest) = line
                .strip_prefix("deb ")
                .or_else(|| line.strip_prefix("deb-src "))
            {
                // `deb [options] URI suite components`: drop the options.
                let rest = rest.trim_start();
                let rest = match rest.strip_prefix('[') {
                    Some(r) => r.split_once(']').map_or("", |(_, after)| after),
                    None => rest,
                };
                if let Some(u) = rest.split_whitespace().next() {
                    uris.push(u.to_owned());
                }
            }
        }
    };
    read(&apt.join("sources.list"));
    if let Ok(entries) = std::fs::read_dir(apt.join("sources.list.d")) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.ends_with(".list") || name.ends_with(".sources") {
                read(&e.path());
            }
        }
    }
    !uris.is_empty() && uris.iter().all(|u| approved(u))
}

/// Count what `apt-get -s upgrade` would install, and how much of it comes
/// from a security pocket.
#[must_use]
pub fn count_pending(simulation: &str) -> (u32, u32) {
    let mut pending = 0;
    let mut security = 0;
    for line in simulation.lines() {
        if let Some(rest) = line.strip_prefix("Inst ") {
            pending += 1;
            if rest.contains("-security") {
                security += 1;
            }
        }
    }
    (pending, security)
}

fn load(paths: &Paths) -> BaseState {
    std::fs::read(paths.base())
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or(BaseState {
            phase: BasePhase::Idle,
            sources_approved: false,
            pending: 0,
            security: 0,
            last_check: None,
            reboot_required: false,
            last_outcome: None,
            reason: None,
            major_upgrade_supported: false,
        })
}

fn save(paths: &Paths, s: &BaseState) {
    if let Ok(b) = serde_json::to_vec_pretty(s) {
        let _ = write_atomic(&paths.base(), &b);
    }
}

fn interrupted(paths: &Paths) -> bool {
    std::fs::read_dir(&paths.dpkg_updates).is_ok_and(|mut d| d.next().is_some())
}

/// The base state now.
#[must_use]
pub fn status(paths: &Paths) -> BaseState {
    let mut s = load(paths);
    s.sources_approved = sources_approved(&paths.apt);
    s.reboot_required = paths.reboot_required.exists();
    if interrupted(paths) {
        s.phase = BasePhase::Interrupted;
    } else if s.phase == BasePhase::Interrupted {
        s.phase = BasePhase::Idle;
    }
    s.major_upgrade_supported = false;
    s
}

fn refuse(
    paths: &Paths,
    mut s: BaseState,
    reason: UpdateReason,
) -> Result<BaseState, UpdateReason> {
    s.reason = Some(reason);
    s.last_outcome = Some(BaseOutcome::Refused);
    s.phase = if interrupted(paths) {
        BasePhase::Interrupted
    } else {
        BasePhase::Idle
    };
    save(paths, &s);
    Err(reason)
}

/// Refresh the package lists and count what is pending.
pub fn check(paths: &Paths, host: &dyn Host) -> Result<BaseState, UpdateReason> {
    let mut s = status(paths);
    if !s.sources_approved {
        return refuse(paths, s, UpdateReason::MetadataUntrusted);
    }
    let Ok(_lock) = MaintenanceLock::try_acquire(&paths.lock) else {
        return refuse(paths, s, UpdateReason::MaintenanceBusy);
    };
    s.phase = BasePhase::Checking;
    save(paths, &s);
    if let Err(r) = host.apt(AptOp::Refresh) {
        return refuse(paths, s, r);
    }
    let simulation = match host.apt(AptOp::Simulate) {
        Ok(out) => out,
        Err(r) => return refuse(paths, s, r),
    };
    (s.pending, s.security) = count_pending(&simulation);
    s.last_check = Some(host.now());
    s.reason = None;
    s.phase = BasePhase::Idle;
    save(paths, &s);
    Ok(status(paths))
}

/// Apply pending base upgrades (never a distribution upgrade).
pub fn apply(paths: &Paths, host: &dyn Host) -> Result<BaseState, UpdateReason> {
    let mut s = status(paths);
    if !s.sources_approved {
        return refuse(paths, s, UpdateReason::MetadataUntrusted);
    }
    let Ok(_lock) = MaintenanceLock::try_acquire(&paths.lock) else {
        return refuse(paths, s, UpdateReason::MaintenanceBusy);
    };
    if interrupted(paths) {
        if let Err(r) = host.apt(AptOp::Repair) {
            s.last_outcome = Some(BaseOutcome::Failed);
            return refuse(paths, s, r);
        }
    }
    s.phase = BasePhase::Upgrading;
    save(paths, &s);
    let result = host.apt(AptOp::Upgrade);
    let after = host
        .apt(AptOp::Simulate)
        .map(|o| count_pending(&o))
        .unwrap_or((s.pending, s.security));
    (s.pending, s.security) = after;
    match result {
        Ok(_) => {
            s.last_outcome = Some(BaseOutcome::Succeeded);
            s.reason = None;
            s.phase = BasePhase::Idle;
            save(paths, &s);
            Ok(status(paths))
        }
        Err(r) => {
            s.last_outcome = Some(BaseOutcome::Failed);
            s.reason = Some(r);
            s.phase = if interrupted(paths) {
                BasePhase::Interrupted
            } else {
                BasePhase::Idle
            };
            save(paths, &s);
            Err(r)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fontes_aprovadas_so_as_da_lista() {
        let t = tempfile::tempdir().unwrap();
        let apt = t.path();
        std::fs::create_dir_all(apt.join("sources.list.d")).unwrap();
        std::fs::write(
            apt.join("sources.list.d/ubuntu.sources"),
            "Types: deb\nURIs: http://archive.ubuntu.com/ubuntu/\nSuites: noble noble-updates\n\nTypes: deb\nURIs: http://security.ubuntu.com/ubuntu/\nSuites: noble-security\n",
        )
        .unwrap();
        std::fs::write(apt.join("sources.list.d/docker.list"), "deb [arch=amd64 signed-by=/k.gpg] https://download.docker.com/linux/ubuntu noble stable\n").unwrap();
        assert!(sources_approved(apt));
        std::fs::write(
            apt.join("sources.list.d/evil.list"),
            "deb http://ppa.example.com/x noble main\n",
        )
        .unwrap();
        assert!(!sources_approved(apt));
        std::fs::remove_file(apt.join("sources.list.d/evil.list")).unwrap();
        std::fs::write(
            apt.join("sources.list"),
            "deb http://archive.ubuntu.com.evil.com/ubuntu noble main\n",
        )
        .unwrap();
        assert!(!sources_approved(apt));
    }

    #[test]
    fn conta_pendentes_e_de_seguranca() {
        let sim = "NOTE: simulation\nInst libc6 [2.39-0ubuntu8.8] (2.39-0ubuntu8.9 Ubuntu:24.04/noble-security [amd64])\nInst curl [8.5.0-2ubuntu10.5] (8.5.0-2ubuntu10.6 Ubuntu:24.04/noble-updates [amd64])\nConf libc6 (2.39-0ubuntu8.9 Ubuntu:24.04/noble-security [amd64])\n";
        assert_eq!(count_pending(sim), (2, 1));
    }
}
