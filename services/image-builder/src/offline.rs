//! B11–B13 on the powered-off disk, never booted again: attach it with
//! `qemu-nbd`, mount the root (and `/boot`, `/boot/efi`), clean what a clone
//! must not share, inspect, and write the embedded manifests.
//!
//! The builder runs as root inside its disposable builder VM; nothing here
//! runs on the operator's machine.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use ocinye_image_contracts::build::ImageBuildError;
use serde::Serialize;

use crate::cmd::{self, p};
use crate::vm::BUILD_USER;

fn asm(step: &str) -> ImageBuildError {
    ImageBuildError::ImageAssemblyFailed { step: step.into() }
}

/// A disk attached to an nbd device and mounted at `mnt`; detached on drop.
pub struct Mounted {
    dev: String,
    pub mnt: PathBuf,
}

fn free_nbd() -> Option<String> {
    (0..16)
        .map(|i| format!("nbd{i}"))
        .find(|n| {
            fs::read_to_string(format!("/sys/block/{n}/size"))
                .map(|s| s.trim() == "0")
                .unwrap_or(false)
        })
        .map(|n| format!("/dev/{n}"))
}

#[derive(serde::Deserialize)]
struct Ls {
    blockdevices: Vec<LsNode>,
}
#[derive(serde::Deserialize)]
struct LsNode {
    name: String,
    label: Option<String>,
    #[serde(default)]
    children: Vec<LsNode>,
}

/// Partitions by label (`cloudimg-rootfs`, `BOOT`, `UEFI` in Ubuntu cloud images).
fn by_label(dev: &str) -> Vec<(String, String)> {
    let Ok(o) = std::process::Command::new("lsblk")
        .args(["-J", "-o", "NAME,LABEL", dev])
        .output()
    else {
        return vec![];
    };
    let Ok(ls) = serde_json::from_slice::<Ls>(&o.stdout) else {
        return vec![];
    };
    ls.blockdevices
        .iter()
        .flat_map(|d| d.children.iter())
        .filter_map(|c| c.label.clone().map(|l| (l, format!("/dev/{}", c.name))))
        .collect()
}

impl Mounted {
    pub fn attach(
        step: &str,
        disk: &Path,
        mnt: &Path,
        readonly: bool,
    ) -> Result<Self, ImageBuildError> {
        let _ = cmd::run(step, "modprobe", &["nbd", "max_part=16"], || asm(step));
        let dev = free_nbd().ok_or_else(|| asm(step))?;
        let mut args = vec!["--fork", "-c", dev.as_str()];
        if readonly {
            args.insert(0, "-r");
        }
        args.push(p(disk));
        cmd::run(step, "qemu-nbd", &args, || asm(step))?;
        let m = Self {
            dev: dev.clone(),
            mnt: mnt.to_path_buf(),
        };
        let _ = cmd::run(step, "partprobe", &[&dev], || asm(step));
        let mut parts = vec![];
        for _ in 0..30 {
            parts = by_label(&dev);
            if parts.iter().any(|(l, _)| l == "cloudimg-rootfs") {
                break;
            }
            std::thread::sleep(Duration::from_millis(500));
        }
        let find = |l: &str| parts.iter().find(|(x, _)| x == l).map(|(_, d)| d.clone());
        let root = find("cloudimg-rootfs").ok_or_else(|| asm(step))?;
        fs::create_dir_all(mnt).map_err(|_| asm(step))?;
        let ro = if readonly { "ro" } else { "rw" };
        cmd::run(step, "mount", &["-o", ro, &root, p(mnt)], || asm(step))?;
        if let Some(b) = find("BOOT") {
            cmd::run(step, "mount", &["-o", ro, &b, p(&mnt.join("boot"))], || {
                asm(step)
            })?;
        }
        if let Some(e) = find("UEFI") {
            cmd::run(
                step,
                "mount",
                &["-o", ro, &e, p(&mnt.join("boot/efi"))],
                || asm(step),
            )?;
        }
        Ok(m)
    }

    pub fn path(&self, abs: &str) -> PathBuf {
        self.mnt.join(abs.trim_start_matches('/'))
    }
}

impl Drop for Mounted {
    fn drop(&mut self) {
        let _ = std::process::Command::new("sync").status();
        let _ = std::process::Command::new("umount")
            .args(["-R", p(&self.mnt)])
            .status();
        let _ = std::process::Command::new("qemu-nbd")
            .args(["-d", &self.dev])
            .status();
    }
}

fn rm(path: &Path) {
    if path.is_dir() && !path.is_symlink() {
        let _ = fs::remove_dir_all(path);
    } else {
        let _ = fs::remove_file(path);
    }
}

fn empty_dir(path: &Path) {
    if let Ok(entries) = fs::read_dir(path) {
        for e in entries.flatten() {
            rm(&e.path());
        }
    }
}

fn truncate_tree(path: &Path) {
    if let Ok(entries) = fs::read_dir(path) {
        for e in entries.flatten() {
            let pth = e.path();
            match e.file_type() {
                Ok(t) if t.is_dir() => truncate_tree(&pth),
                Ok(t) if t.is_file() => {
                    let _ = fs::OpenOptions::new().write(true).truncate(true).open(&pth);
                }
                _ => {}
            }
        }
    }
}

/// Remove `user` from passwd/shadow/group/gshadow/subuid/subgid, including
/// supplementary group member lists.
fn drop_user(root: &Path, user: &str) -> std::io::Result<()> {
    for f in ["etc/passwd", "etc/shadow", "etc/subuid", "etc/subgid"] {
        let path = root.join(f);
        if let Ok(text) = fs::read_to_string(&path) {
            let kept: String = text
                .lines()
                .filter(|l| l.split(':').next() != Some(user))
                .map(|l| format!("{l}\n"))
                .collect();
            fs::write(&path, kept)?;
        }
    }
    for f in ["etc/group", "etc/gshadow"] {
        let path = root.join(f);
        if let Ok(text) = fs::read_to_string(&path) {
            let kept: String = text
                .lines()
                .filter(|l| l.split(':').next() != Some(user))
                .map(|l| {
                    let mut fields: Vec<String> = l.split(':').map(str::to_owned).collect();
                    if let Some(last) = fields.last_mut() {
                        *last = last
                            .split(',')
                            .filter(|m| *m != user && !m.is_empty())
                            .collect::<Vec<_>>()
                            .join(",");
                    }
                    format!("{}\n", fields.join(":"))
                })
                .collect();
            fs::write(&path, kept)?;
        }
    }
    Ok(())
}

/// B11: every row of D013_CLONE_SAFETY_MATRIX.md that sanitization owns.
pub fn sanitize(m: &Mounted) -> Result<Vec<String>, ImageBuildError> {
    let r = &m.mnt;
    let mut done = vec![];
    let mut item = |name: &str, f: &mut dyn FnMut()| {
        f();
        done.push(name.to_owned());
    };
    item("build user", &mut || {
        let _ = drop_user(r, BUILD_USER);
        rm(&r.join("home").join(BUILD_USER));
        rm(&r.join("etc/sudoers.d/90-cloud-init-users"));
        rm(&r.join("var/spool/mail").join(BUILD_USER));
    });
    item("build inputs", &mut || rm(&r.join("root/ocinye-build")));
    item("root dotfiles", &mut || {
        for f in [
            "root/.ssh",
            "root/.bash_history",
            "root/.cache",
            "root/.wget-hsts",
            "root/.lesshst",
            "root/snap",
        ] {
            rm(&r.join(f));
        }
    });
    item("machine-id", &mut || {
        let _ = fs::write(r.join("etc/machine-id"), "");
        rm(&r.join("var/lib/dbus/machine-id"));
    });
    item("ssh host keys", &mut || {
        if let Ok(entries) = fs::read_dir(r.join("etc/ssh")) {
            for e in entries.flatten() {
                if e.file_name().to_string_lossy().starts_with("ssh_host_") {
                    rm(&e.path());
                }
            }
        }
        rm(&r.join("etc/ssh/sshd_config.d/00-ocinye-build.conf"));
    });
    item("cloud-init state", &mut || {
        empty_dir(&r.join("var/lib/cloud"));
        if let Ok(entries) = fs::read_dir(r.join("var/log")) {
            for e in entries.flatten() {
                if e.file_name().to_string_lossy().starts_with("cloud-init") {
                    rm(&e.path());
                }
            }
        }
        rm(&r.join("etc/netplan/50-cloud-init.yaml"));
    });
    item("leases and network state", &mut || {
        empty_dir(&r.join("var/lib/systemd/network"));
        empty_dir(&r.join("var/lib/dhcp"));
    });
    item("random seed and credentials", &mut || {
        rm(&r.join("var/lib/systemd/random-seed"));
        rm(&r.join("var/lib/systemd/credential.secret"));
        rm(&r.join("var/lib/systemd/timesync/clock"));
    });
    item("logs and journal", &mut || {
        empty_dir(&r.join("var/log/journal"));
        truncate_tree(&r.join("var/log"));
    });
    item("tmp", &mut || {
        empty_dir(&r.join("tmp"));
        empty_dir(&r.join("var/tmp"));
    });
    item("apt lists and caches", &mut || {
        empty_dir(&r.join("var/lib/apt/lists"));
        for f in [
            "var/cache/apt/pkgcache.bin",
            "var/cache/apt/srcpkgcache.bin",
        ] {
            rm(&r.join(f));
        }
        empty_dir(&r.join("var/cache/apt/archives/partial"));
    });
    item("docker engine state", &mut || {
        rm(&r.join("var/lib/docker/engine-id"));
        empty_dir(&r.join("var/lib/docker/network/files"));
        empty_dir(&r.join("var/lib/docker/containers"));
        empty_dir(&r.join("var/lib/docker/volumes"));
        rm(&r.join("var/lib/docker/buildkit"));
    });
    item("first-boot state", &mut || {
        rm(&r.join("var/lib/ocinye-firstboot"))
    });
    item("hostname", &mut || {
        let _ = fs::write(r.join("etc/hostname"), "localhost\n");
    });
    Ok(done)
}

#[derive(Debug, Serialize)]
pub struct Check {
    pub check: &'static str,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Inspection {
    pub schema: u32,
    pub profile: String,
    pub checks: Vec<Check>,
}

impl Inspection {
    pub fn first_failure(&self) -> Option<&'static str> {
        self.checks.iter().find(|c| !c.ok).map(|c| c.check)
    }
}

fn dir_has_files(p: &Path) -> bool {
    fs::read_dir(p)
        .map(|mut d| d.next().is_some())
        .unwrap_or(false)
}

fn any_nonempty_file(p: &Path) -> Option<String> {
    let entries = fs::read_dir(p).ok()?;
    for e in entries.flatten() {
        let pth = e.path();
        match e.file_type() {
            Ok(t) if t.is_dir() => {
                if let Some(x) = any_nonempty_file(&pth) {
                    return Some(x);
                }
            }
            Ok(t) if t.is_file() && e.metadata().map(|m| m.len()).unwrap_or(0) > 0 => {
                return Some(pth.display().to_string())
            }
            _ => {}
        }
    }
    None
}

/// Secret patterns (Ocinye's own, plus private key armour and cloud keys).
fn secret_in(bytes: &[u8]) -> Option<&'static str> {
    // Text only: executables and libraries carry these strings as format
    // text (ssh, gnutls), not as secrets.
    if bytes.iter().take(8192).any(|b| *b == 0) {
        return None;
    }
    let find = |pat: &[u8]| bytes.windows(pat.len()).position(|w| w == pat);
    if let Some(i) = find(b"PRIVATE KEY-----") {
        let line_start = bytes[..i]
            .iter()
            .rposition(|b| *b == b'\n')
            .map_or(0, |p| p + 1);
        if bytes[line_start..i].windows(10).any(|w| w == b"-----BEGIN") {
            return Some("private-key");
        }
    }
    if let Some(i) = find(b"AKIA") {
        if bytes.get(i + 4..i + 20).is_some_and(|w| {
            w.iter()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        }) {
            return Some("aws-access-key");
        }
    }
    let mut from = 0;
    while let Some(off) = bytes[from..].windows(2).position(|w| w == b"GK") {
        let i = from + off;
        if bytes
            .get(i + 2..i + 26)
            .is_some_and(|w| w.iter().all(u8::is_ascii_hexdigit))
            && !bytes.get(i + 26).is_some_and(u8::is_ascii_hexdigit)
        {
            return Some("garage-key");
        }
        from = i + 2;
    }
    None
}

fn scan(root: &Path, dir: &Path, skip: &[&str], out: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let pth = e.path();
        let rel = pth
            .strip_prefix(root)
            .map(|x| x.display().to_string())
            .unwrap_or_default();
        if skip.iter().any(|s| rel.starts_with(s)) {
            continue;
        }
        match e.file_type() {
            Ok(t) if t.is_dir() => scan(root, &pth, skip, out),
            Ok(t) if t.is_file() => {
                if e.metadata().map(|m| m.len()).unwrap_or(0) > 64 << 20 {
                    continue;
                }
                if let Ok(b) = fs::read(&pth) {
                    if let Some(kind) = secret_in(&b) {
                        out.push(format!("{kind}:{rel}"));
                    }
                }
            }
            _ => {}
        }
    }
}

/// The in-target UEFI boot stack curtin's `install_missing_packages` requests
/// for a debian UEFI install (curtin 24.0.0 `curthooks.py`): efibootmgr, the
/// real `grub-efi-<arch>` (requested unconditionally), its `-bin`, and the
/// `-signed` flavour, plus `shim-signed`. curtin installs only what is missing,
/// so the metal rootfs must already carry every one of these or the offline ISO
/// install fetches over a network it does not have. Returns `(required_present,
/// forbidden_present)`; v1 is UEFI-only, so the BIOS flavour is forbidden.
pub fn boot_packages(arch: &str) -> (Vec<&'static str>, Vec<&'static str>) {
    match arch {
        "amd64" => (
            vec![
                "grub-efi-amd64",
                "grub-efi-amd64-bin",
                "grub-efi-amd64-signed",
                "shim-signed",
                "efibootmgr",
            ],
            vec!["grub-pc", "grub-pc-bin"],
        ),
        "arm64" => (
            vec![
                "grub-efi-arm64",
                "grub-efi-arm64-bin",
                "grub-efi-arm64-signed",
                "shim-signed",
                "efibootmgr",
            ],
            // grub-pc is an amd64/i386 (BIOS) package; it cannot be present here.
            vec![],
        ),
        _ => (vec![], vec![]),
    }
}

/// Fragments under `etc/default/grub.d` that pin `GRUB_FORCE_PARTUUID`. Ubuntu
/// cloud images ship `40-force-partuuid.cfg` with the image's own root PARTUUID;
/// if it survives into the metal rootfs the installed kernel command line keeps
/// that stale build-time identity instead of the fresh target's, and the target
/// cannot find its root. The metal rootfs must carry none.
pub fn forced_partuuid_fragments(root: &Path) -> Vec<String> {
    let dir = root.join("etc/default/grub.d");
    let Ok(entries) = fs::read_dir(&dir) else {
        return vec![];
    };
    let mut hits = vec![];
    for e in entries.flatten() {
        let p = e.path();
        if let Ok(text) = fs::read_to_string(&p) {
            if text
                .lines()
                .any(|l| l.trim_start().starts_with("GRUB_FORCE_PARTUUID="))
            {
                hits.push(e.file_name().to_string_lossy().into_owned());
            }
        }
    }
    hits.sort();
    hits
}

/// Packages marked `install ok installed` in a target's dpkg status file.
fn installed_debs(status_path: &Path) -> std::collections::BTreeSet<String> {
    let mut set = std::collections::BTreeSet::new();
    let Ok(text) = fs::read_to_string(status_path) else {
        return set;
    };
    let mut pkg: Option<String> = None;
    for line in text.lines() {
        if let Some(n) = line.strip_prefix("Package: ") {
            pkg = Some(n.trim().to_string());
        } else if let Some(st) = line.strip_prefix("Status: ") {
            if st.trim() == "install ok installed" {
                if let Some(p) = pkg.take() {
                    set.insert(p);
                }
            }
        } else if line.is_empty() {
            pkg = None;
        }
    }
    set
}

/// `(missing_required, present_forbidden)` for the metal rootfs's boot stack.
pub fn boot_package_report(
    arch: &str,
    installed: &std::collections::BTreeSet<String>,
) -> (Vec<&'static str>, Vec<&'static str>) {
    let (required, forbidden) = boot_packages(arch);
    let missing = required
        .into_iter()
        .filter(|p| !installed.contains(*p))
        .collect();
    let present = forbidden
        .into_iter()
        .filter(|p| installed.contains(*p))
        .collect();
    (missing, present)
}

/// B12: the inspection the image must pass, written as `inspection.json`.
pub fn inspect(m: &Mounted, profile: &str, arch: &str, release_id: &str) -> Inspection {
    let r = &m.mnt;
    let mut checks = vec![];
    let mut c = |check: &'static str, ok: bool, detail: Option<String>| {
        checks.push(Check { check, ok, detail })
    };
    let mid = fs::metadata(r.join("etc/machine-id")).map(|x| x.len());
    c(
        "machine_id_empty",
        mid.as_ref().is_ok_and(|n| *n == 0),
        Some(format!("{mid:?}")),
    );
    let keys: Vec<String> = fs::read_dir(r.join("etc/ssh"))
        .map(|d| {
            d.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| n.starts_with("ssh_host_"))
                .collect()
        })
        .unwrap_or_default();
    c(
        "no_ssh_host_keys",
        keys.is_empty(),
        (!keys.is_empty()).then(|| keys.join(",")),
    );
    c(
        "no_cloud_init_state",
        !dir_has_files(&r.join("var/lib/cloud")),
        None,
    );
    let passwd = fs::read_to_string(r.join("etc/passwd")).unwrap_or_default();
    let humans: Vec<&str> = passwd
        .lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.split(':').collect();
            let uid: u32 = f.get(2)?.parse().ok()?;
            (uid >= 1000 && uid != 65534).then_some(f[0])
        })
        .collect();
    let humans_ok = humans
        .iter()
        .all(|u| ["ocinye", "ocinye-claim"].contains(u))
        && humans.contains(&"ocinye");
    c(
        "accounts",
        humans_ok
            && !passwd
                .lines()
                .any(|l| l.starts_with(&format!("{BUILD_USER}:"))),
        Some(humans.join(",")),
    );
    let shadow = fs::read_to_string(r.join("etc/shadow")).unwrap_or_default();
    let pw = |u: &str| {
        shadow
            .lines()
            .find(|l| l.starts_with(&format!("{u}:")))
            .and_then(|l| l.split(':').nth(1))
            .unwrap_or("")
            .to_owned()
    };
    let no_password = |h: &str| h.starts_with('!') || h.starts_with('*');
    c(
        "no_passwords",
        no_password(&pw("root")) && no_password(&pw("ocinye")) && no_password(&pw("ocinye-claim")),
        None,
    );
    let ok_keys = fs::metadata(r.join("home/ocinye/.ssh/authorized_keys"))
        .map(|x| x.len() == 0)
        .unwrap_or(true);
    c("no_operator_keys", ok_keys, None);
    c(
        "no_build_inputs",
        !r.join("root/ocinye-build").exists() && !r.join("home").join(BUILD_USER).exists(),
        None,
    );
    c(
        "logs_empty",
        any_nonempty_file(&r.join("var/log")).is_none(),
        any_nonempty_file(&r.join("var/log")),
    );
    c(
        "tmp_empty",
        !dir_has_files(&r.join("tmp")) && !dir_has_files(&r.join("var/tmp")),
        None,
    );
    c(
        "no_docker_engine_state",
        !r.join("var/lib/docker/engine-id").exists()
            && !dir_has_files(&r.join("var/lib/docker/containers"))
            && !dir_has_files(&r.join("var/lib/docker/volumes")),
        None,
    );
    let wants = |unit: &str| {
        [
            "multi-user.target.wants",
            "sockets.target.wants",
            "getty.target.wants",
        ]
        .iter()
        .any(|w| {
            // Enable links are absolute (`/etc/systemd/system/…`): resolve
            // them inside the image, never on the builder.
            let link = r.join("etc/systemd/system").join(w).join(unit);
            match fs::read_link(&link) {
                Ok(t) if t.is_absolute() => r.join(t.strip_prefix("/").unwrap_or(&t)).exists(),
                Ok(t) => link.parent().is_some_and(|d| d.join(t).exists()),
                Err(_) => link.exists(),
            }
        })
    };
    c(
        "runtime_disabled_until_claimed",
        !wants("docker.service") && !wants("docker.socket") && !wants("containerd.service"),
        None,
    );
    // The block guard of the medium arms in every boot that is not, exactly,
    // the installer. It belongs in the OIE root and nowhere else: in an
    // installed system's initramfs it would be a liability.
    const GUARD_FILES: [&str; 6] = [
        "usr/lib/ocinye/guard/blockguard-apply",
        "usr/share/initramfs-tools/hooks/ocinye-blockguard",
        "usr/share/initramfs-tools/scripts/init-top/ocinye-blockguard",
        "usr/share/initramfs-tools/scripts/casper-premount/05ocinye_blockguard",
        "usr/lib/udev/rules.d/01-ocinye-blockguard.rules",
        "usr/lib/ocinye/guard/casper-guard-functions",
    ];
    let guard_present: Vec<&str> = GUARD_FILES
        .iter()
        .copied()
        .filter(|f| r.join(f).exists())
        .collect();
    if profile == "oie" {
        c(
            "block_guard_installed",
            guard_present.len() == GUARD_FILES.len(),
            (guard_present.len() != GUARD_FILES.len()).then(|| guard_present.join(",")),
        );
        c(
            "storage_safety_not_overclaimed",
            // CERTIFIED is set by a certification record, never by a build.
            fs::read_to_string(r.join("usr/lib/ocinye/oie/storage-safety"))
                .is_ok_and(|s| s.trim() == "INTENDED"),
            None,
        );
        c(
            "no_automounter",
            !r.join("usr/lib/udisks2").exists() && !r.join("usr/sbin/automount").exists(),
            None,
        );
    } else {
        c(
            "block_guard_absent_from_installed_system",
            guard_present.is_empty(),
            (!guard_present.is_empty()).then(|| guard_present.join(",")),
        );
    }
    match profile {
        "oie" => c(
            "oie_enabled",
            wants("ocinye-oie@tty1.service")
                && (wants("ocinye-oie@ttyS0.service") || wants("ocinye-oie@ttyAMA0.service"))
                && r.join("usr/lib/ocinye/ocinye-oie").exists(),
            None,
        ),
        _ => c(
            "firstboot_enabled",
            wants("ocinye-firstboot.service") && r.join("usr/lib/ocinye/ocinye-firstboot").exists(),
            None,
        ),
    }
    // The metal rootfs is the one the ISO's OIE installs onto a blank disk with
    // curtin and no network. If its UEFI boot stack is incomplete the install
    // cannot populate the ESP — fail the image build here, not at OIE runtime.
    if profile == "metal" {
        let installed = installed_debs(&r.join("var/lib/dpkg/status"));
        let (missing, forbidden) = boot_package_report(arch, &installed);
        c(
            "uefi_boot_stack_complete",
            missing.is_empty(),
            (!missing.is_empty()).then(|| missing.join(",")),
        );
        c(
            "no_bios_grub",
            forbidden.is_empty(),
            (!forbidden.is_empty()).then(|| forbidden.join(",")),
        );
        // A forced PARTUUID in the metal rootfs becomes the installed kernel's
        // stale root= on every fresh disk. The installed root identity must be
        // derived from the actual target, so no fragment may pin it.
        let forced = forced_partuuid_fragments(r);
        c(
            "no_forced_partuuid",
            forced.is_empty(),
            (!forced.is_empty()).then(|| forced.join(",")),
        );
    }
    c(
        "no_instance_state",
        !r.join("srv/ocinye").exists()
            && !r.join("var/lib/ocinye-installer").exists()
            && !r.join("etc/ocinye/ocinye.env").exists(),
        None,
    );
    c(
        "release_payload",
        r.join(format!("usr/lib/ocinye/release/{release_id}/MANIFEST.json"))
            .exists(),
        None,
    );
    let sudoers_mode = fs::metadata(r.join("etc/sudoers.d/60-ocinye-image"))
        .map(|x| std::os::unix::fs::PermissionsExt::mode(&x.permissions()) & 0o777);
    c(
        "sudoers",
        sudoers_mode.as_ref().is_ok_and(|m| *m == 0o440),
        Some(format!("{sudoers_mode:?}")),
    );
    let mut found = vec![];
    // Image layers (third-party content) and package docs are outside this
    // scan: D013_SECURITY_MATRIX records the scope.
    scan(
        r,
        r,
        &[
            "proc",
            "sys",
            "dev",
            "run",
            "usr/share",
            "usr/lib/python3",
            "var/lib/docker",
            "var/lib/containerd",
            "snap",
            "boot",
            "lib/firmware",
            "usr/lib/firmware",
            "usr/src",
        ],
        &mut found,
    );
    c(
        "secret_scan",
        found.is_empty(),
        (!found.is_empty()).then(|| found.join(",")),
    );
    Inspection {
        schema: 1,
        profile: profile.into(),
        checks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn padroes_de_segredo() {
        assert_eq!(
            secret_in(b"x -----BEGIN OPENSSH PRIVATE KEY-----\n"),
            Some("private-key")
        );
        assert_eq!(
            secret_in(b"key=AKIAABCDEFGHIJKLMNOP"),
            Some("aws-access-key")
        );
        assert_eq!(secret_in(b"GK0123456789abcdef01234567"), Some("garage-key"));
        assert_eq!(
            secret_in(b"GK0123456789abcdef0123456789"),
            None,
            "longer hex runs are not garage key ids"
        );
        assert_eq!(secret_in(b"nothing to see"), None);
        assert_eq!(
            secret_in(b"\x7fELF\0\0 -----BEGIN OPENSSH PRIVATE KEY-----"),
            None,
            "binaries are not scanned"
        );
        assert_eq!(
            secret_in(b"printf(\"%s PRIVATE KEY-----\")"),
            None,
            "format text is not armour"
        );
    }

    fn pkgset(names: &[&str]) -> std::collections::BTreeSet<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn amd64_metal_exige_a_pilha_uefi_completa() {
        // A: the complete amd64 UEFI boot stack leaves nothing missing.
        let full = pkgset(&[
            "grub-efi-amd64",
            "grub-efi-amd64-bin",
            "grub-efi-amd64-signed",
            "shim-signed",
            "efibootmgr",
            "linux-generic",
        ]);
        let (missing, forbidden) = boot_package_report("amd64", &full);
        assert!(missing.is_empty(), "{missing:?}");
        assert!(forbidden.is_empty());
        // The exact failure that shipped: grub-efi-amd64 absent, grub-pc present.
        let broken = pkgset(&[
            "grub-efi-amd64-bin",
            "grub-efi-amd64-signed",
            "shim-signed",
            "efibootmgr",
            "grub-pc",
            "grub-pc-bin",
        ]);
        let (missing, forbidden) = boot_package_report("amd64", &broken);
        assert_eq!(missing, vec!["grub-efi-amd64"]);
        assert_eq!(forbidden, vec!["grub-pc", "grub-pc-bin"]);
    }

    #[test]
    fn arm64_metal_exige_a_pilha_uefi_arm64() {
        // B: arm64 needs the arm64 flavour, and has no BIOS grub to forbid.
        let full = pkgset(&[
            "grub-efi-arm64",
            "grub-efi-arm64-bin",
            "grub-efi-arm64-signed",
            "shim-signed",
            "efibootmgr",
        ]);
        let (missing, forbidden) = boot_package_report("arm64", &full);
        assert!(missing.is_empty(), "{missing:?}");
        assert!(forbidden.is_empty());
    }

    #[test]
    fn nomes_de_arquitectura_nao_se_misturam() {
        // F: the amd64 stack never satisfies arm64 and vice versa; no leak.
        let (req_amd, forb_amd) = boot_packages("amd64");
        assert!(req_amd.iter().all(|p| !p.contains("arm64")));
        assert!(forb_amd.contains(&"grub-pc"));
        let (req_arm, forb_arm) = boot_packages("arm64");
        assert!(req_arm.iter().all(|p| !p.contains("amd64")));
        assert!(forb_arm.is_empty(), "arm64 has no grub-pc to forbid");
        // C: an amd64 image that kept grub-pc fails the forbidden check.
        let amd_with_bios = pkgset(&["grub-efi-amd64", "grub-pc"]);
        let (_m, forbidden) = boot_package_report("amd64", &amd_with_bios);
        assert_eq!(forbidden, vec!["grub-pc"]);
    }

    #[test]
    fn force_partuuid_no_rootfs_e_detectado() {
        let d = std::env::temp_dir().join(format!("ocinye-forcepu-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(d.join("etc/default/grub.d")).unwrap();
        // A clean rootfs: nothing pins the identity.
        fs::write(
            d.join("etc/default/grub.d/90-ocinye.cfg"),
            "GRUB_TIMEOUT=3\n",
        )
        .unwrap();
        assert!(forced_partuuid_fragments(&d).is_empty());
        // The cloud image's fragment is caught.
        fs::write(
            d.join("etc/default/grub.d/40-force-partuuid.cfg"),
            "# comment\nGRUB_FORCE_PARTUUID=bd907978-e121-479a-9598-288628fcfd21\n",
        )
        .unwrap();
        assert_eq!(
            forced_partuuid_fragments(&d),
            vec!["40-force-partuuid.cfg".to_string()]
        );
    }

    #[test]
    fn dpkg_status_le_so_os_instalados() {
        let d = std::env::temp_dir().join(format!("ocinye-dpkg-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        fs::write(
            d.join("status"),
            "Package: grub-efi-amd64\nStatus: install ok installed\nVersion: 1\n\n\
             Package: grub-pc\nStatus: deinstall ok config-files\nVersion: 2\n\n\
             Package: shim-signed\nStatus: install ok installed\nVersion: 3\n\n",
        )
        .unwrap();
        let set = installed_debs(&d.join("status"));
        assert!(set.contains("grub-efi-amd64") && set.contains("shim-signed"));
        assert!(
            !set.contains("grub-pc"),
            "a removed package left as config-files is not installed"
        );
    }

    #[test]
    fn utilizador_de_construcao_sai_de_todos_os_ficheiros() {
        let d = std::env::temp_dir().join(format!("ocinye-builder-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(d.join("etc")).unwrap();
        fs::write(d.join("etc/passwd"), format!("root:x:0:0::/root:/bin/bash\n{BUILD_USER}:x:1001:1001::/home/{BUILD_USER}:/bin/bash\nocinye:x:1000:1000::/home/ocinye:/bin/bash\n")).unwrap();
        fs::write(
            d.join("etc/group"),
            format!("sudo:x:27:{BUILD_USER}\nadm:x:4:syslog,{BUILD_USER}\n{BUILD_USER}:x:1001:\n"),
        )
        .unwrap();
        drop_user(&d, BUILD_USER).unwrap();
        assert!(!fs::read_to_string(d.join("etc/passwd"))
            .unwrap()
            .contains(BUILD_USER));
        assert_eq!(
            fs::read_to_string(d.join("etc/group")).unwrap(),
            "sudo:x:27:\nadm:x:4:syslog\n"
        );
    }
}
