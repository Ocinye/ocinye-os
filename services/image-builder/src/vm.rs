//! B04–B10: the build VM. The verified base is never modified: each pass
//! boots a qcow2 overlay, runs one `provision.sh` stage over SSH with an
//! ephemeral key made for this build, and powers off.
//!
//! KVM when the image's architecture is the builder's and `/dev/kvm` exists;
//! TCG otherwise (slow, but the same bytes).

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use ocinye_image_contracts::build::ImageBuildError;
use ocinye_image_contracts::manifest::Arch;

use crate::cmd::{self, p};

pub const BUILD_USER: &str = "ocinye-builder";

fn fail(step: &str) -> impl Fn() -> ImageBuildError + '_ {
    move || ImageBuildError::ImageAssemblyFailed { step: step.into() }
}

pub struct BuildVm {
    pub arch: Arch,
    pub work: PathBuf,
    pub key: PathBuf,
    pub seed: PathBuf,
    pub port: u16,
    pub memory_mib: u32,
    pub cpus: u32,
}

/// Firmware for UEFI guests (Ubuntu's ovmf / qemu-efi-aarch64 packages).
pub fn firmware(arch: Arch) -> (&'static str, &'static str) {
    match arch {
        Arch::Amd64 => (
            "/usr/share/OVMF/OVMF_CODE_4M.fd",
            "/usr/share/OVMF/OVMF_VARS_4M.fd",
        ),
        Arch::Arm64 => (
            "/usr/share/AAVMF/AAVMF_CODE.fd",
            "/usr/share/AAVMF/AAVMF_VARS.fd",
        ),
    }
}

fn native(arch: Arch) -> bool {
    matches!(
        (arch, std::env::consts::ARCH),
        (Arch::Amd64, "x86_64") | (Arch::Arm64, "aarch64")
    ) && Path::new("/dev/kvm").exists()
}

/// QEMU arguments for a UEFI guest. Shared with the certification harness.
pub fn qemu_args(arch: Arch, vars: &Path, memory_mib: u32, cpus: u32) -> (String, Vec<String>) {
    let (code, _) = firmware(arch);
    let kvm = native(arch);
    let (bin, machine, cpu) = match arch {
        Arch::Amd64 => (
            "qemu-system-x86_64",
            "q35",
            if kvm { "host" } else { "max" },
        ),
        Arch::Arm64 => (
            "qemu-system-aarch64",
            "virt",
            if kvm { "host" } else { "max" },
        ),
    };
    let accel = if kvm { "kvm" } else { "tcg,thread=multi" };
    let args = vec![
        "-machine".into(),
        machine.into(),
        "-accel".into(),
        accel.into(),
        "-cpu".into(),
        cpu.into(),
        "-m".into(),
        memory_mib.to_string(),
        "-smp".into(),
        cpus.to_string(),
        "-drive".into(),
        format!("if=pflash,format=raw,unit=0,readonly=on,file={code}"),
        "-drive".into(),
        format!("if=pflash,format=raw,unit=1,file={}", vars.display()),
        "-device".into(),
        "virtio-rng-pci".into(),
        "-display".into(),
        "none".into(),
        "-monitor".into(),
        "none".into(),
    ];
    (bin.into(), args)
}

impl BuildVm {
    /// B04: the ephemeral builder key and the NoCloud seed.
    pub fn prepare(
        arch: Arch,
        work: &Path,
        port: u16,
        memory_mib: u32,
        cpus: u32,
    ) -> Result<Self, ImageBuildError> {
        let step = "B04";
        fs::create_dir_all(work).map_err(|_| fail(step)())?;
        let key = work.join("builder_ed25519");
        let _ = fs::remove_file(&key);
        let _ = fs::remove_file(key.with_extension("pub"));
        cmd::run(
            step,
            "ssh-keygen",
            &[
                "-q",
                "-t",
                "ed25519",
                "-N",
                "",
                "-C",
                "ocinye-image-build",
                "-f",
                p(&key),
            ],
            fail(step),
        )?;
        let public = fs::read_to_string(key.with_extension("pub")).map_err(|_| fail(step)())?;
        let mut rnd = [0u8; 8];
        fs::File::open("/dev/urandom")
            .and_then(|mut f| std::io::Read::read_exact(&mut f, &mut rnd))
            .map_err(|_| fail(step)())?;
        let user_data = format!(
            "#cloud-config\nusers:\n  - name: {BUILD_USER}\n    sudo: \"ALL=(ALL) NOPASSWD:ALL\"\n    shell: /bin/bash\n    lock_passwd: true\n    ssh_authorized_keys:\n      - {}\nssh_pwauth: false\n",
            public.trim()
        );
        fs::write(work.join("user-data"), user_data).map_err(|_| fail(step)())?;
        fs::write(
            work.join("meta-data"),
            format!(
                "instance-id: ocinye-build-{}\nlocal-hostname: ocinye-build\n",
                hex::encode(rnd)
            ),
        )
        .map_err(|_| fail(step)())?;
        let seed = work.join("seed.img");
        cmd::run(
            step,
            "cloud-localds",
            &[
                p(&seed),
                p(&work.join("user-data")),
                p(&work.join("meta-data")),
            ],
            fail(step),
        )?;
        Ok(Self {
            arch,
            work: work.to_path_buf(),
            key,
            seed,
            port,
            memory_mib,
            cpus,
        })
    }

    /// A qcow2 overlay on `backing` (never modifies it).
    pub fn overlay(&self, backing: &Path, disk: &Path, size: &str) -> Result<(), ImageBuildError> {
        let _ = fs::remove_file(disk);
        cmd::run(
            "B04",
            "qemu-img",
            &[
                "create",
                "-q",
                "-f",
                "qcow2",
                "-F",
                "qcow2",
                "-b",
                p(backing),
                p(disk),
                size,
            ],
            fail("B04"),
        )
    }

    fn ssh_base(&self) -> Vec<String> {
        vec![
            "-i".into(),
            p(&self.key).into(),
            "-p".into(),
            self.port.to_string(),
            "-o".into(),
            "StrictHostKeyChecking=no".into(),
            "-o".into(),
            "UserKnownHostsFile=/dev/null".into(),
            "-o".into(),
            "LogLevel=ERROR".into(),
            "-o".into(),
            "ConnectTimeout=10".into(),
            "-o".into(),
            "ServerAliveInterval=30".into(),
            format!("{BUILD_USER}@127.0.0.1"),
        ]
    }

    /// Boot `disk` and wait for SSH.
    pub fn boot(&self, disk: &Path, name: &str) -> Result<Running<'_>, ImageBuildError> {
        let step = "B05";
        let vars = self.work.join(format!("{name}-vars.fd"));
        fs::copy(firmware(self.arch).1, &vars).map_err(|_| fail(step)())?;
        let (bin, mut args) = qemu_args(self.arch, &vars, self.memory_mib, self.cpus);
        args.extend([
            "-drive".into(),
            format!("file={},if=virtio,format=qcow2", disk.display()),
            "-drive".into(),
            format!(
                "file={},if=virtio,format=raw,readonly=on",
                self.seed.display()
            ),
            "-netdev".into(),
            format!("user,id=n0,hostfwd=tcp:127.0.0.1:{}-:22", self.port),
            "-device".into(),
            "virtio-net-pci,netdev=n0".into(),
            "-serial".into(),
            format!(
                "file:{}",
                self.work.join(format!("{name}-console.log")).display()
            ),
        ]);
        cmd::log(step, &format!("{bin} … {}", disk.display()));
        let child = Command::new(&bin)
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|_| fail(step)())?;
        let mut running = Running { child, vm: self };
        let limit = if native(self.arch) {
            Duration::from_secs(600)
        } else {
            Duration::from_secs(3600)
        };
        let start = Instant::now();
        loop {
            if running.ssh_status(&["true"]).is_ok_and(|ok| ok) {
                cmd::log(
                    step,
                    &format!("ssh up after {}s", start.elapsed().as_secs()),
                );
                return Ok(running);
            }
            if start.elapsed() > limit || running.child.try_wait().ok().flatten().is_some() {
                running.kill();
                return Err(fail(step)());
            }
            std::thread::sleep(Duration::from_secs(10));
        }
    }
}

pub struct Running<'a> {
    child: Child,
    vm: &'a BuildVm,
}

impl Running<'_> {
    fn ssh_status(&self, remote: &[&str]) -> std::io::Result<bool> {
        Command::new("ssh")
            .args(self.vm.ssh_base())
            .args(remote)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
    }

    /// Run a remote command; stdout captured.
    pub fn ssh(&self, step: &str, remote: &str) -> Result<Vec<u8>, ImageBuildError> {
        cmd::log(step, &format!("vm$ {remote}"));
        let o = Command::new("ssh")
            .args(self.vm.ssh_base())
            .arg(remote)
            .stdin(Stdio::null())
            .stderr(Stdio::inherit())
            .output()
            .map_err(|_| fail(step)())?;
        if o.status.success() {
            Ok(o.stdout)
        } else {
            Err(fail(step)())
        }
    }

    /// Stream a local directory into `/root/ocinye-build` on the VM.
    pub fn upload(&self, step: &str, dir: &Path) -> Result<(), ImageBuildError> {
        cmd::log(step, &format!("upload {}", dir.display()));
        let mut tar = Command::new("tar")
            .args(["-C", p(dir), "-cf", "-", "."])
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|_| fail(step)())?;
        let out = tar.stdout.take().ok_or_else(fail(step))?;
        let st = Command::new("ssh")
            .args(self.vm.ssh_base())
            .arg("sudo install -d -m 0700 /root/ocinye-build && sudo tar -C /root/ocinye-build --no-same-owner -xf -")
            .stdin(out)
            .status()
            .map_err(|_| fail(step)())?;
        let t = tar.wait().map_err(|_| fail(step)())?;
        if st.success() && t.success() {
            Ok(())
        } else {
            Err(fail(step)())
        }
    }

    /// Power off from inside and wait for QEMU to exit.
    pub fn poweroff(mut self, already_requested: bool) -> Result<(), ImageBuildError> {
        if !already_requested {
            let _ = self.ssh("B10", "sudo systemctl poweroff --no-block");
        }
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(900) {
            if self.child.try_wait().ok().flatten().is_some() {
                return Ok(());
            }
            std::thread::sleep(Duration::from_secs(5));
        }
        self.kill();
        Err(fail("B10")())
    }

    fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Write a file into a staging directory.
pub fn stage_file(dir: &Path, rel: &str, bytes: &[u8], mode: u32) -> Result<(), ImageBuildError> {
    use std::os::unix::fs::PermissionsExt as _;
    let path = dir.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|_| fail("stage")())?;
    }
    let mut f = fs::File::create(&path).map_err(|_| fail("stage")())?;
    f.write_all(bytes).map_err(|_| fail("stage")())?;
    fs::set_permissions(&path, fs::Permissions::from_mode(mode)).map_err(|_| fail("stage")())
}
