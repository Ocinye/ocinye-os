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
    let find = |pat: &[u8]| bytes.windows(pat.len()).position(|w| w == pat);
    if find(b"PRIVATE KEY-----").is_some() {
        return Some("private-key");
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

/// B12: the inspection the image must pass, written as `inspection.json`.
pub fn inspect(m: &Mounted, profile: &str, release_id: &str) -> Inspection {
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
        .any(|w| r.join("etc/systemd/system").join(w).join(unit).exists())
    };
    c(
        "runtime_disabled_until_claimed",
        !wants("docker.service") && !wants("docker.socket") && !wants("containerd.service"),
        None,
    );
    match profile {
        "oie" => c(
            "oie_enabled",
            wants("ocinye-oie@tty1.service")
                && wants("ocinye-oie@ttyS0.service")
                && r.join("usr/lib/ocinye/ocinye-oie").exists(),
            None,
        ),
        _ => c(
            "firstboot_enabled",
            wants("ocinye-firstboot.service") && r.join("usr/lib/ocinye/ocinye-firstboot").exists(),
            None,
        ),
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
