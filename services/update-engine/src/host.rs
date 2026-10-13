//! What the engine does to the machine, behind one interface: the real Linux
//! host in production, an in-memory one in the tests (which is how every
//! power-cut and failure case is exercised without a machine).

use crate::paths::Paths;
use crate::runner::{run, run_installer, Tool};
use ocinye_update_contracts::manifest::ImageRef;
use ocinye_update_contracts::UpdateReason;
use std::path::Path;
use std::time::Duration;

/// The units the engine may restart, in the order it restarts them.
pub const RESTART_ORDER: [&str; 3] = [
    "ocinye-session.service",
    "ocinye-system.service",
    "ocinye-display.service",
];

/// Ubuntu base operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AptOp {
    /// `apt-get update`.
    Refresh,
    /// `apt-get -s upgrade`: what would change, nothing changed.
    Simulate,
    /// `apt-get -y upgrade` with the configuration files kept.
    Upgrade,
    /// `dpkg --configure -a`: finish an interrupted run.
    Repair,
}

/// The machine.
pub trait Host: Send + Sync {
    /// Now, `YYYY-MM-DDTHH:MM:SSZ`.
    fn now(&self) -> String;
    /// This boot's identifier: a different one means the machine restarted.
    fn boot_id(&self) -> String;
    /// Free bytes on the filesystem holding `path`.
    fn free_bytes(&self, path: &Path) -> u64;
    /// The System and Session services are running.
    fn system_healthy(&self) -> Result<(), UpdateReason>;
    /// The Instance's containers are healthy (only asked when one exists).
    fn instance_healthy(&self) -> Result<(), UpdateReason>;
    /// Load an image archive into the container engine.
    fn load_image(&self, archive: &Path) -> Result<(), UpdateReason>;
    /// Whether the engine has an image with this id.
    fn image_present(&self, image_id: &str) -> bool;
    /// Move the Instance with the release's own installer
    /// (`<bundle>/install/ocinye upgrade`): checkpoint, switch, health gate,
    /// and its own return to the previous release when that gate fails.
    fn instance_upgrade(&self, bundle: &Path) -> Result<(), UpdateReason>;
    /// Return the Instance to the release before the last upgrade
    /// (`ocinye rollback --confirm`, with its checkpoint).
    fn instance_rollback(&self) -> Result<(), UpdateReason>;
    /// `systemctl daemon-reload`.
    fn reload_units(&self) -> Result<(), UpdateReason>;
    /// Restart one of [`RESTART_ORDER`].
    fn restart_unit(&self, unit: &str) -> Result<(), UpdateReason>;
    /// Whether a unit is active.
    fn unit_active(&self, unit: &str) -> bool;
    /// Restart this engine once the reply is sent, so the new release's
    /// engine takes over.
    fn restart_engine_later(&self);
    /// A base operation; returns the tool's (bounded) output.
    fn apt(&self, op: AptOp) -> Result<String, UpdateReason>;
}

/// The installed Linux system.
#[derive(Debug, Clone)]
pub struct LinuxHost {
    paths: Paths,
}

impl LinuxHost {
    /// The host whose files live at `paths`.
    #[must_use]
    pub fn new(paths: Paths) -> Self {
        Self { paths }
    }

    /// What the release's installer said, for whoever has to find out why an
    /// Instance did not move: the last run of each kind, root-only. The
    /// installer never prints a secret (it hands the credential over in a
    /// file), and the record is bounded by the runner's limit.
    fn keep_installer_output(&self, what: &str, r: &crate::runner::Ran) {
        use std::os::unix::fs::PermissionsExt;
        let path = self.paths.state.join(format!("instance-{what}.log"));
        let text = format!(
            "exit: {:?}\n--- stdout\n{}\n--- stderr\n{}\n",
            r.code, r.stdout, r.stderr
        );
        if crate::store::write_atomic(&path, text.as_bytes()).is_ok() {
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
        }
    }
}

const MINUTE: Duration = Duration::from_secs(60);

fn ok(code: Option<i32>) -> bool {
    code == Some(0)
}

impl Host for LinuxHost {
    fn now(&self) -> String {
        rfc3339(std::time::SystemTime::now())
    }

    fn boot_id(&self) -> String {
        std::fs::read_to_string("/proc/sys/kernel/random/boot_id")
            .unwrap_or_default()
            .trim()
            .to_owned()
    }

    fn free_bytes(&self, path: &Path) -> u64 {
        let mut p = path.to_path_buf();
        while !p.exists() {
            if !p.pop() {
                return 0;
            }
        }
        rustix::fs::statvfs(&p).map_or(0, |s| s.f_bavail.saturating_mul(s.f_frsize))
    }

    fn system_healthy(&self) -> Result<(), UpdateReason> {
        for u in ["ocinye-system.service", "ocinye-session.service"] {
            if !self.unit_active(u) {
                return Err(UpdateReason::SystemUnhealthy);
            }
        }
        Ok(())
    }

    fn instance_healthy(&self) -> Result<(), UpdateReason> {
        for c in ["ocinye-core-1", "ocinye-workspace-1", "ocinye-proxy-1"] {
            let r = run(
                Tool::Docker,
                &[
                    "inspect",
                    "-f",
                    "{{if .State.Health}}{{.State.Health.Status}}{{else}}none{{end}}",
                    c,
                ],
                MINUTE,
            )
            .map_err(|_| UpdateReason::SystemUnhealthy)?;
            if !ok(r.code) || r.stdout.trim() != "healthy" {
                return Err(UpdateReason::SystemUnhealthy);
            }
        }
        Ok(())
    }

    fn load_image(&self, archive: &Path) -> Result<(), UpdateReason> {
        let a = archive.to_str().ok_or(UpdateReason::ArchiveUnsafe)?;
        let r = run(Tool::Docker, &["load", "-q", "-i", a], 30 * MINUTE)
            .map_err(|_| UpdateReason::InstanceUpgradeFailed)?;
        if ok(r.code) {
            Ok(())
        } else {
            Err(UpdateReason::InstanceUpgradeFailed)
        }
    }

    fn image_present(&self, image_id: &str) -> bool {
        run(
            Tool::Docker,
            &["image", "inspect", "--format", "{{.Id}}", image_id],
            MINUTE,
        )
        .is_ok_and(|r| ok(r.code) && r.stdout.trim() == image_id)
    }

    fn instance_upgrade(&self, bundle: &Path) -> Result<(), UpdateReason> {
        let installer = bundle.join("install/ocinye");
        let r = run_installer(&installer, &["upgrade"], bundle, 90 * MINUTE)
            .map_err(|_| UpdateReason::InstanceUpgradeFailed)?;
        self.keep_installer_output("upgrade", &r);
        if ok(r.code) {
            Ok(())
        } else {
            Err(UpdateReason::InstanceUpgradeFailed)
        }
    }

    fn instance_rollback(&self) -> Result<(), UpdateReason> {
        let current = Path::new("/srv/ocinye/current");
        let installer = current.join("install/ocinye");
        let r = run_installer(&installer, &["rollback", "--confirm"], current, 60 * MINUTE)
            .map_err(|_| UpdateReason::RollbackUnsafe)?;
        self.keep_installer_output("rollback", &r);
        if ok(r.code) {
            Ok(())
        } else {
            Err(UpdateReason::RollbackUnsafe)
        }
    }

    fn reload_units(&self) -> Result<(), UpdateReason> {
        let r = run(Tool::Systemctl, &["daemon-reload"], MINUTE)
            .map_err(|_| UpdateReason::HealthGateFailed)?;
        if ok(r.code) {
            Ok(())
        } else {
            Err(UpdateReason::HealthGateFailed)
        }
    }

    fn restart_unit(&self, unit: &str) -> Result<(), UpdateReason> {
        if !RESTART_ORDER.contains(&unit) {
            return Err(UpdateReason::HealthGateFailed);
        }
        let r = run(Tool::Systemctl, &["restart", unit], 2 * MINUTE)
            .map_err(|_| UpdateReason::HealthGateFailed)?;
        if ok(r.code) {
            Ok(())
        } else {
            Err(UpdateReason::HealthGateFailed)
        }
    }

    fn unit_active(&self, unit: &str) -> bool {
        run(Tool::Systemctl, &["is-active", "--quiet", unit], MINUTE).is_ok_and(|r| ok(r.code))
    }

    fn restart_engine_later(&self) {
        let _ = run(
            Tool::Systemctl,
            &["--no-block", "restart", "ocinye-update.service"],
            MINUTE,
        );
    }

    fn apt(&self, op: AptOp) -> Result<String, UpdateReason> {
        let _ = &self.paths;
        let (tool, args, deadline): (Tool, &[&str], Duration) = match op {
            AptOp::Refresh => (Tool::AptGet, &["update"], 10 * MINUTE),
            AptOp::Simulate => (Tool::AptGet, &["-s", "upgrade"], 5 * MINUTE),
            AptOp::Upgrade => (
                Tool::AptGet,
                &[
                    "-y",
                    "-o",
                    "Dpkg::Options::=--force-confold",
                    "-o",
                    "Dpkg::Options::=--force-confdef",
                    "upgrade",
                ],
                90 * MINUTE,
            ),
            AptOp::Repair => (Tool::Dpkg, &["--configure", "-a"], 60 * MINUTE),
        };
        let r = run(tool, args, deadline).map_err(|_| UpdateReason::RepositoryUnreachable)?;
        if ok(r.code) {
            Ok(r.stdout)
        } else if op == AptOp::Refresh {
            Err(UpdateReason::RepositoryUnreachable)
        } else {
            Err(UpdateReason::InstanceUpgradeFailed)
        }
    }
}

/// `YYYY-MM-DDTHH:MM:SSZ` for a moment.
#[must_use]
pub fn rfc3339(t: std::time::SystemTime) -> String {
    let secs = t
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    // Howard Hinnant's civil-from-days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

/// The images a release names that are all present.
#[must_use]
pub fn images_present(host: &dyn Host, images: &[ImageRef]) -> bool {
    images.iter().all(|i| host.image_present(&i.image_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn datas_rfc3339() {
        let t = std::time::UNIX_EPOCH + Duration::from_secs(1_791_753_376);
        assert_eq!(rfc3339(t), "2026-10-11T21:16:16Z");
        assert_eq!(rfc3339(std::time::UNIX_EPOCH), "1970-01-01T00:00:00Z");
        let leap = std::time::UNIX_EPOCH + Duration::from_secs(951_782_400); // 2000-02-29
        assert_eq!(rfc3339(leap), "2000-02-29T00:00:00Z");
    }
}
