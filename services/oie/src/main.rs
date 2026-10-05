//! `ocinye-oie` — the Ocinye Installation Environment (D013, ADR-0028).
//! PROVISIONAL_PENDING_D011_CERTIFICATION.
//!
//! ```text
//! ocinye-oie run --vt | --serial     the installer on this terminal (one console at a time)
//! ocinye-oie probe                   disks as the OIE classifies them, JSON (read-only)
//! ocinye-oie --version
//! ```
//!
//! It writes to exactly one disk, the one whose stable path the operator
//! chose and whose token they typed, and only after that.

#![forbid(unsafe_code)]

mod flow;
mod install;
mod probe;
mod strings;

use std::fs;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use ocinye_image_contracts::firstboot::PublicKeyLine;
use ocinye_image_contracts::oie::{
    InstallationTargetDisk, OieError, OieInstallJournal, OiePayload, OieStep, ProbedDisk,
};

struct Real {
    facts: Option<OiePayload>,
}

fn findmnt_source(target: &str) -> Option<String> {
    let o = Command::new("findmnt")
        .args(["-n", "-o", "SOURCE", target])
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&o.stdout).trim().to_owned();
    s.strip_prefix("/dev/").map(str::to_owned)
}

fn lsblk() -> Option<probe::Lsblk> {
    let o = Command::new("lsblk")
        .args(probe::LSBLK_ARGS)
        .output()
        .ok()?;
    serde_json::from_slice(&o.stdout).ok()
}

/// Removable volumes mounted read-only under /run/ocinye-oie/keys for the
/// operator-key search (vfat, exfat, iso9660 only; never the target).
fn key_roots() -> Vec<PathBuf> {
    let mut roots = vec![];
    let Some(ls) = lsblk() else { return roots };
    let mut stack: Vec<&probe::LsblkNode> = ls.blockdevices.iter().collect();
    while let Some(n) = stack.pop() {
        stack.extend(n.children.iter());
        let fs = n.fstype.as_deref().unwrap_or_default();
        if !matches!(fs, "vfat" | "exfat" | "iso9660") {
            continue;
        }
        if let Some(Some(mp)) = n.mountpoints.iter().find(|m| m.is_some()) {
            roots.push(PathBuf::from(mp));
            continue;
        }
        let dir = PathBuf::from("/run/ocinye-oie/keys").join(&n.name);
        if fs::create_dir_all(&dir).is_ok()
            && Command::new("mount")
                .args(["-o", "ro,nosuid,nodev,noexec"])
                .arg(format!("/dev/{}", n.name))
                .arg(&dir)
                .status()
                .is_ok_and(|s| s.success())
        {
            roots.push(dir);
        }
    }
    roots
}

impl flow::Machine for Real {
    fn uefi(&self) -> bool {
        probe::is_uefi(Path::new("/sys"))
    }
    fn arch_supported(&self) -> bool {
        matches!(std::env::consts::ARCH, "x86_64" | "aarch64")
    }
    fn memory(&self) -> u64 {
        probe::mem_total(&fs::read_to_string("/proc/meminfo").unwrap_or_default())
    }
    fn disks(&self) -> Vec<ProbedDisk> {
        let Some(ls) = lsblk() else { return vec![] };
        let mut boot_sources = vec![];
        for m in [install::MEDIA, "/run/live/medium", "/rofs"] {
            if let Some(src) = findmnt_source(m) {
                boot_sources.push(src);
            }
        }
        let ctx = probe::Context {
            boot_sources,
            media_label: self.facts.as_ref().map(|f| f.media_label.clone()),
            stable_paths: probe::stable_paths(Path::new("/dev")),
        };
        probe::disks(&ls, &ctx)
    }
    fn operator_key(&self) -> Option<PublicKeyLine> {
        install::find_operator_key(&key_roots()).map(|(_, k)| k)
    }
    fn install(
        &self,
        disk: &InstallationTargetDisk,
        key: Option<&PublicKeyLine>,
        say: &mut dyn FnMut(&str),
    ) -> Result<(), OieError> {
        let facts = self.facts.as_ref().ok_or(OieError::ImageManifestInvalid {
            field: "payload.json".into(),
        })?;
        let started = chrono::Utc::now();
        let mut steps = vec![];
        let mut step =
            |name: &str, t: Instant, r: &Result<(), OieError>, say: &mut dyn FnMut(&str)| {
                let result = match r {
                    Ok(()) => "OK".to_owned(),
                    Err(e) => serde_json::to_value(e)
                        .ok()
                        .and_then(|v| v["code"].as_str().map(str::to_owned))
                        .unwrap_or_default(),
                };
                say(&format!("  [{result}] {name}"));
                steps.push(OieStep {
                    step: name.into(),
                    seconds: t.elapsed().as_secs(),
                    result,
                });
            };
        let media = Path::new(install::MEDIA);
        let t = Instant::now();
        let checked = install::check_media(media, facts);
        step(
            "media_check",
            t,
            &checked.as_ref().map(|_| ()).map_err(Clone::clone),
            say,
        );
        let content = checked?;
        fs::create_dir_all("/run/ocinye-oie").map_err(|_| OieError::DiskWriteFailed {
            device: "run".into(),
        })?;
        let cfg = PathBuf::from("/run/ocinye-oie/curtin.yaml");
        fs::write(
            &cfg,
            install::curtin_config(disk, &media.join(&facts.rootfs)),
        )
        .map_err(|_| OieError::DiskWriteFailed {
            device: "run".into(),
        })?;
        let t = Instant::now();
        let r = install::run_curtin(&cfg);
        step("install", t, &r, say);
        r?;
        let target = Path::new(install::TARGET);
        let t = Instant::now();
        let r = install::verify_target(target, &content);
        step("verify", t, &r, say);
        r?;
        let t = Instant::now();
        let mut journal = OieInstallJournal {
            image: facts.image.clone(),
            disk: disk.clone(),
            payload_sha256: facts.rootfs_sha256.clone(),
            operator_key: key.map(PublicKeyLine::fingerprint),
            steps: steps.clone(),
            started_at: started.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            finished_at: String::new(),
        };
        journal.steps.push(OieStep {
            step: "first_boot".into(),
            seconds: t.elapsed().as_secs(),
            result: "OK".into(),
        });
        journal.finished_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let text = ocinye_installer_contracts::canonical::to_canonical(&journal).map_err(|_| {
            OieError::DiskWriteFailed {
                device: "journal".into(),
            }
        })?;
        let r = install::prepare_first_boot(target, key, &text).map(|_| ());
        say(&format!(
            "  [{}] first_boot",
            if r.is_ok() { "OK" } else { "DISK_WRITE_FAILED" }
        ));
        r?;
        let _ = Command::new("sync").status();
        let _ = Command::new("umount")
            .args(["-R", install::TARGET])
            .status();
        Ok(())
    }
    fn reboot(&self) {
        let _ = Command::new("eject").arg("/dev/sr0").status();
        let _ = Command::new("systemctl").arg("reboot").status();
    }
}

fn load_facts() -> Option<OiePayload> {
    let b = fs::read(install::PAYLOAD_FACTS).ok()?;
    let f: OiePayload = serde_json::from_slice(&b).ok()?;
    f.rootfs_sha256.is_valid().then_some(f)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["run", mode @ ("--vt" | "--serial")] => {
            // One console drives the installation; the other waits.
            let _ = fs::create_dir_all("/run/ocinye-oie");
            let lock = fs::OpenOptions::new()
                .create(true)
                .truncate(false)
                .write(true)
                .open("/run/ocinye-oie/console.lock");
            let mut stdin = BufReader::new(std::io::stdin());
            let mut stdout = std::io::stdout();
            let held = lock.as_ref().is_ok_and(|f| f.try_lock().is_ok());
            if !held {
                let _ = std::io::Write::write_all(
                    &mut stdout,
                    b"Ocinye OS: installation in progress on another console.\r\n",
                );
                if let Ok(f) = &lock {
                    let _ = f.lock();
                }
            }
            let machine = Real {
                facts: load_facts(),
            };
            let mut ui = flow::Ui {
                lang: 0,
                serial: *mode == "--serial",
                input: &mut stdin,
                out: &mut stdout,
            };
            match flow::run(&mut ui, &machine) {
                flow::Outcome::Installed => 0,
                flow::Outcome::Abandoned => 3,
                flow::Outcome::Failed(e) => {
                    let _ = fs::write(
                        "/run/ocinye-oie/failed.json",
                        serde_json::to_string(&e).unwrap_or_default(),
                    );
                    // Keep the failure on screen until someone acts.
                    let mut line = String::new();
                    let _ = std::io::BufRead::read_line(&mut stdin, &mut line);
                    1
                }
            }
        }
        ["probe"] => {
            let m = Real {
                facts: load_facts(),
            };
            let d = ocinye_image_contracts::oie::classify(&flow::Machine::disks(&m));
            println!("{}", serde_json::to_string_pretty(&d).unwrap_or_default());
            0
        }
        ["--version"] => {
            println!("ocinye-oie {}", env!("CARGO_PKG_VERSION"));
            0
        }
        _ => {
            eprintln!("uso: ocinye-oie run --vt|--serial | probe | --version");
            2
        }
    };
    std::process::exit(code);
}
