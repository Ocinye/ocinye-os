//! `ocinye-oie` — the Ocinye Installation Environment (D013, ADR-0028).
//! PROVISIONAL_PENDING_D011_CERTIFICATION.
//!
//! ```text
//! ocinye-oie run --vt | --serial     this boot's mode on this terminal: the installer in
//!                                    `ocinye.mode=install` (one console at a time), the
//!                                    non-destructive session in every other mode
//! ocinye-oie policy                  the storage-policy verdict of this boot, JSON
//! ocinye-oie select-medium           (initramfs) which device this system booted from
//! ocinye-oie probe                   disks as the OIE classifies them, JSON (read-only)
//! ocinye-oie --version
//! ```
//!
//! It writes to exactly one disk, the one whose stable path the operator
//! chose and whose token they typed, and only after that.

#![forbid(unsafe_code)]

mod flow;
mod install;
mod origin;
mod policy;
mod probe;
mod session;
mod strings;

use std::fs;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use ocinye_image_contracts::bootmode::GUARD_RELEASED_DIR;
use ocinye_image_contracts::bootmode::{
    InstallAuthority, ResolvedMode, StorageSafetyEvidence, LANG_PARAM,
};
use ocinye_image_contracts::firstboot::PublicKeyLine;
use ocinye_image_contracts::medium::{MediumDecision, MEDIA_LABEL};
use ocinye_image_contracts::oie::{
    ConfirmedTarget, OieError, OieInstallJournal, OiePayload, OieStep, ProbedDisk,
};

struct Real {
    facts: Option<OiePayload>,
    /// The console lock, once this console has taken the installation.
    console: std::cell::RefCell<Option<fs::File>>,
    /// `ocinye.selftest=policy-fail`: the storage-policy verifier reports
    /// failure (it can only restrict a session).
    fault_injected: bool,
}

/// This build's media identity, in the live root (the initramfs has its own
/// copy at /conf/ocinye-media-id).
const MEDIA_ID_FILE: &str = "/usr/lib/ocinye/guard/media-id";
/// What `select-medium` decided in the initramfs.
const MEDIUM_FILE: &str = "/run/ocinye/medium.json";

/// Where the image says how live storage safety may be described. Absent or
/// unreadable means the weaker statement.
const STORAGE_SAFETY_FILE: &str = "/usr/lib/ocinye/oie/storage-safety";

fn storage_safety_evidence() -> StorageSafetyEvidence {
    match fs::read_to_string(STORAGE_SAFETY_FILE)
        .as_deref()
        .map(str::trim)
    {
        Ok("CERTIFIED") => StorageSafetyEvidence::Certified,
        _ => StorageSafetyEvidence::Intended,
    }
}

/// `ocinye.lang=pt|en|fr` from the boot menu; Portuguese otherwise.
fn initial_lang(cmdline: &str) -> usize {
    let prefix = format!("{LANG_PARAM}=");
    cmdline
        .split_ascii_whitespace()
        .take_while(|a| *a != "--" && *a != "---")
        .find_map(|a| a.strip_prefix(prefix.as_str()))
        .map_or(0, |v| match v {
            "en" => 1,
            "fr" => 2,
            _ => 0,
        })
}

#[derive(serde::Deserialize)]
struct LsblkDisk {
    name: String,
    #[serde(default)]
    size: Option<u64>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    tran: Option<String>,
    #[serde(rename = "type", default)]
    kind: String,
}

#[derive(serde::Deserialize)]
struct LsblkDisks {
    blockdevices: Vec<LsblkDisk>,
}

/// Whole disks from what the kernel already exposes: `lsblk -d` with no
/// filesystem column reads sysfs and opens no device.
fn disks_metadata_only() -> Vec<LsblkDisk> {
    Command::new("lsblk")
        .args([
            "--json",
            "--bytes",
            "--nodeps",
            "--output",
            "NAME,SIZE,MODEL,TRAN,TYPE",
        ])
        .output()
        .ok()
        .and_then(|o| serde_json::from_slice::<LsblkDisks>(&o.stdout).ok())
        .map(|l| l.blockdevices)
        .unwrap_or_default()
        .into_iter()
        .filter(|d| matches!(d.kind.as_str(), "disk" | "rom"))
        .filter(|d| {
            !["loop", "ram", "zram"]
                .iter()
                .any(|p| d.name.starts_with(p))
        })
        .collect()
}

fn partition_count(disk: &str) -> u32 {
    fs::read_dir(format!("/sys/block/{disk}"))
        .map(|d| {
            d.flatten()
                .filter(|e| e.path().join("partition").exists())
                .count()
        })
        .map_or(0, |n| u32::try_from(n).unwrap_or(u32::MAX))
}

impl session::SessionMachine for Real {
    fn policy(&self) -> policy::PolicyVerdict {
        policy::verify(&policy::read_system(self.fault_injected))
    }
    fn disks(&self, probe_signatures: bool) -> Vec<session::DiskLine> {
        let probed: Vec<ProbedDisk> = if probe_signatures {
            flow::Machine::disks(self)
        } else {
            vec![]
        };
        disks_metadata_only()
            .into_iter()
            .map(|d| {
                let p = probed.iter().find(|p| p.device == d.name);
                session::DiskLine {
                    partitions: p.map_or_else(|| partition_count(&d.name), |p| p.partitions),
                    filesystems: p.map(|p| p.filesystems.clone()),
                    boot_media: p.is_some_and(|p| p.boot_media),
                    bytes: d.size.unwrap_or(0),
                    model: d.model.unwrap_or_default().trim().to_owned(),
                    connection: d.tran.unwrap_or_default(),
                    device: d.name,
                }
            })
            .collect()
    }
    fn hardware(&self) -> session::HardwareFacts {
        let cpus = fs::read_to_string("/proc/cpuinfo")
            .map(|c| c.lines().filter(|l| l.starts_with("processor")).count())
            .map_or(0, |n| u32::try_from(n).unwrap_or(u32::MAX));
        let mut nics = vec![];
        if let Ok(dir) = fs::read_dir("/sys/class/net") {
            for e in dir.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                if name == "lo" {
                    continue;
                }
                let up =
                    fs::read_to_string(e.path().join("carrier")).is_ok_and(|s| s.trim() == "1");
                nics.push((name, up));
            }
        }
        nics.sort();
        session::HardwareFacts {
            arch: std::env::consts::ARCH.to_owned(),
            uefi: probe::is_uefi(Path::new("/sys")),
            memory_bytes: probe::mem_total(
                &fs::read_to_string("/proc/meminfo").unwrap_or_default(),
            ),
            cpus,
            nics,
        }
    }
    fn network(&self) -> Vec<String> {
        Command::new("ip")
            .args(["-brief", "address"])
            .output()
            .map(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .filter(|l| !l.starts_with("lo "))
                    .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
                    .collect()
            })
            .unwrap_or_default()
    }
    fn restart(&self) {
        // A normal firmware restart. Nothing is written anywhere to steer the
        // next boot: no firmware variable, no file on the medium.
        let _ = Command::new("systemctl").arg("reboot").status();
    }
    fn poweroff(&self) {
        let _ = Command::new("systemctl").arg("poweroff").status();
    }
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

#[derive(serde::Deserialize)]
struct LsblkTransport {
    name: String,
    #[serde(default)]
    tran: Option<String>,
    #[serde(default)]
    rm: Option<serde_json::Value>,
    #[serde(default)]
    hotplug: Option<serde_json::Value>,
}

#[derive(serde::Deserialize)]
struct LsblkTransports {
    blockdevices: Vec<LsblkTransport>,
}

fn truthy(v: Option<&serde_json::Value>) -> bool {
    match v {
        Some(serde_json::Value::Bool(b)) => *b,
        Some(serde_json::Value::String(s)) => s == "1",
        Some(serde_json::Value::Number(n)) => n.as_u64() == Some(1),
        _ => false,
    }
}

/// Whether a disk is the kind an operator plugs in to hand over a key: USB,
/// removable or hot-pluggable. An internal disk never is.
fn pluggable(tran: Option<&str>, rm: bool, hotplug: bool) -> bool {
    tran == Some("usb") || rm || hotplug
}

/// Kernel names of the disks an operator key may be searched on.
fn pluggable_disks() -> Vec<String> {
    Command::new("lsblk")
        .args(["--json", "--nodeps", "--output", "NAME,TRAN,RM,HOTPLUG"])
        .output()
        .ok()
        .and_then(|o| serde_json::from_slice::<LsblkTransports>(&o.stdout).ok())
        .map(|l| l.blockdevices)
        .unwrap_or_default()
        .into_iter()
        .filter(|d| {
            pluggable(
                d.tran.as_deref(),
                truthy(d.rm.as_ref()),
                truthy(d.hotplug.as_ref()),
            )
        })
        .map(|d| d.name)
        .collect()
}

/// Volumes on pluggable devices, mounted read-only under /run/ocinye-oie/keys
/// for the operator-key search (vfat, exfat, iso9660 only). Never the target,
/// and never a filesystem on an internal disk: not even its EFI partition.
fn key_roots(target: &str) -> Vec<PathBuf> {
    let mut roots = vec![];
    let Some(ls) = lsblk() else { return roots };
    let allowed = pluggable_disks();
    let mut stack: Vec<&probe::LsblkNode> = ls
        .blockdevices
        .iter()
        .filter(|d| d.name != target && allowed.contains(&d.name))
        .collect();
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

/// The key-search mounts, undone: nothing from another disk stays mounted
/// while the target is written.
fn release_key_roots() {
    if let Ok(dir) = fs::read_dir("/run/ocinye-oie/keys") {
        for e in dir.flatten() {
            let _ = Command::new("umount").arg(e.path()).status();
        }
    }
}

/// Partitions of a disk, by kernel name.
fn partitions_of(disk: &str) -> Vec<String> {
    fs::read_dir(format!("/sys/block/{disk}"))
        .map(|d| {
            d.flatten()
                .filter(|e| e.path().join("partition").exists())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default()
}

impl Real {
    /// Make the confirmed target, and only it, writable. Everything else
    /// stays under the block guard for the whole installation.
    fn release_target(&self, target: &ConfirmedTarget<'_>) -> Result<(), OieError> {
        let confirmed = target.disk();
        // The disk at that stable path must still be the disk that was
        // confirmed: a name alone grants nothing.
        let now = ocinye_image_contracts::oie::classify(&flow::Machine::disks(self));
        let same = now
            .iter()
            .find(|d| d.by_path == confirmed.by_path)
            .is_some_and(|d| ocinye_image_contracts::oie::same_target(confirmed, d));
        if !same {
            return Err(OieError::TargetChanged);
        }
        let failed = || OieError::TargetReleaseFailed {
            device: confirmed.device.clone(),
        };
        // Before the release nothing at all may be writable.
        if policy::verify(&policy::read_system(false)).restricted() {
            return Err(failed());
        }
        // The udev rule leaves this disk and its partitions alone from now on
        // (the partitions curtin is about to create included).
        fs::create_dir_all(GUARD_RELEASED_DIR).map_err(|_| failed())?;
        fs::write(Path::new(GUARD_RELEASED_DIR).join(&confirmed.device), "")
            .map_err(|_| failed())?;
        let mut devices = vec![confirmed.device.clone()];
        devices.extend(partitions_of(&confirmed.device));
        for d in &devices {
            let ok = Command::new("blockdev")
                .arg("--setrw")
                .arg(format!("/dev/{d}"))
                .status()
                .is_ok_and(|s| s.success());
            if !ok {
                return Err(failed());
            }
        }
        // And nothing but the target is writable now.
        if policy::verify_released(&policy::read_system(false), &confirmed.device).restricted() {
            return Err(failed());
        }
        Ok(())
    }
}

impl flow::Machine for Real {
    fn take_console(&self) -> bool {
        if self.console.borrow().is_some() {
            return true;
        }
        let Ok(f) = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open("/run/ocinye-oie/console.lock")
        else {
            return false;
        };
        if f.try_lock().is_ok() {
            *self.console.borrow_mut() = Some(f);
            true
        } else {
            false
        }
    }
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
            guard_armed: fs::read_to_string(policy::GUARD_MARKER)
                .is_ok_and(|s| s.trim() == policy::GUARD_ARMED),
            hardware_read_only: fs::read_dir("/run/ocinye/hardware-ro")
                .map(|d| {
                    d.flatten()
                        .map(|e| e.file_name().to_string_lossy().into_owned())
                        .collect()
                })
                .unwrap_or_default(),
        };
        probe::disks(&ls, &ctx)
    }
    fn operator_key(&self, target: &ConfirmedTarget<'_>) -> Option<PublicKeyLine> {
        let key = install::find_operator_key(&key_roots(&target.disk().device)).map(|(_, k)| k);
        release_key_roots();
        key
    }
    fn storage_protected(&self) -> Result<(), String> {
        let v = policy::verify(&policy::read_system(self.fault_injected));
        policy::first_reason(&v).map_or(Ok(()), Err)
    }
    fn install(
        &self,
        target: &ConfirmedTarget<'_>,
        key: Option<&PublicKeyLine>,
        say: &mut dyn FnMut(&str),
    ) -> Result<(), OieError> {
        let disk = target.disk();
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
        // The one transition from read-only to writable, for the one disk
        // whose typed confirmation was accepted.
        self.release_target(target)?;
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
        ["run", console @ ("--vt" | "--serial")] => {
            let _ = fs::create_dir_all("/run/ocinye-oie");
            let mut stdin = BufReader::new(std::io::stdin());
            let mut stdout = std::io::stdout();
            // The mode of this boot: read once, never changed.
            let cmdline = fs::read_to_string("/proc/cmdline").unwrap_or_default();
            let boot = ResolvedMode::from_cmdline(&cmdline);
            let machine = Real {
                facts: load_facts(),
                console: std::cell::RefCell::new(None),
                fault_injected: boot.verifier_fault_injected(),
            };
            let mut ui = flow::Ui {
                lang: initial_lang(&cmdline),
                serial: *console == "--serial",
                input: &mut stdin,
                out: &mut stdout,
            };
            // Which device this system booted from, and why it is trusted.
            if let Ok(medium) = fs::read_to_string(MEDIUM_FILE) {
                ui.say(&format!("OCINYE-MEDIUM {}", medium.trim()));
            }
            match InstallAuthority::from_mode(&boot) {
                // Only `ocinye.mode=install` reaches the installer.
                Some(authority) => match flow::run(&mut ui, &machine, &authority) {
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
                },
                None => match session::run(&mut ui, &machine, &boot, storage_safety_evidence()) {
                    session::SessionOutcome::Abandoned => 3,
                    session::SessionOutcome::Restarting | session::SessionOutcome::PoweringOff => {
                        // Leave the screen as it is until the machine goes.
                        std::thread::sleep(std::time::Duration::from_secs(600));
                        0
                    }
                },
            }
        }
        ["select-medium"] => {
            // Runs in the initramfs, before casper looks for anything: which
            // block device is the medium this system booted from. Mounts
            // nothing. Exit 0 and the kernel name on stdout when decided, 3 to
            // wait for devices, 4 when it refuses to guess.
            let id = ["/conf/ocinye-media-id", MEDIA_ID_FILE]
                .iter()
                .find_map(|p| fs::read_to_string(p).ok())
                .map(|s| s.trim().to_owned())
                .unwrap_or_default();
            let sel = origin::select(&origin::Roots::system(), MEDIA_LABEL, &id);
            let json = serde_json::to_string(&sel).unwrap_or_default();
            let _ = fs::create_dir_all("/run/ocinye");
            let _ = fs::write("/run/ocinye/medium-last.json", &json);
            match &sel.decision {
                MediumDecision::Medium { name, .. } => {
                    let _ = fs::write(MEDIUM_FILE, &json);
                    println!("{name}");
                    0
                }
                MediumDecision::Wait => 3,
                MediumDecision::RefuseAmbiguous { candidates } => {
                    eprintln!(
                        "Ocinye OS: more than one installation medium is connected ({}) and the firmware does not say which one started this computer. Remove the extra one and restart.",
                        candidates.join(", ")
                    );
                    4
                }
            }
        }
        ["policy"] => {
            let cmdline = fs::read_to_string("/proc/cmdline").unwrap_or_default();
            let boot = ResolvedMode::from_cmdline(&cmdline);
            let v = policy::verify(&policy::read_system(boot.verifier_fault_injected()));
            println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
            i32::from(v.restricted())
        }
        ["probe"] => {
            let m = Real {
                facts: load_facts(),
                console: std::cell::RefCell::new(None),
                fault_injected: false,
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
            eprintln!(
                "uso: ocinye-oie run --vt|--serial | probe | policy | select-medium | --version"
            );
            2
        }
    };
    std::process::exit(code);
}
