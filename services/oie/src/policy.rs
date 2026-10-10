//! Verifying the storage policy of a non-destructive boot (Live, Hardware
//! Check, Recovery): D013 Live Mode, L0-S.
//!
//! The protection itself is not here. It is below every interface: the
//! initramfs guard sets each block device read-only before udev, casper or
//! systemd look at it (`infra/image/oie-rootfs`). This module only *checks*
//! that the policy holds, from what the kernel reports, and says so in a typed
//! verdict. When it does not hold the session is restricted: block-device
//! metadata only, no filesystem inspection, never a writable fallback.
//!
//! Pure functions over text the kernel already exposes, so every rule is
//! tested without a machine.

use serde::Serialize;

pub use ocinye_image_contracts::bootmode::{GUARD_ARMED, GUARD_MARKER};

/// One block device as `/sys/class/block/<name>/ro` reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockFlag {
    /// Kernel name (`sda`, `sda1`, `nvme0n1p2`, `sr0`).
    pub name: String,
    /// The kernel's read-only flag.
    pub read_only: bool,
}

/// What the verifier looks at.
#[derive(Debug, Clone, Default)]
pub struct PolicyInputs {
    /// Content of [`GUARD_MARKER`], if it exists.
    pub guard_marker: Option<String>,
    /// Every block device and partition.
    pub block: Vec<BlockFlag>,
    /// `/proc/swaps`.
    pub swaps: String,
    /// `/proc/mdstat`.
    pub mdstat: String,
    /// `/proc/self/mountinfo`.
    pub mountinfo: String,
    /// `ocinye.selftest=policy-fail` was on the command line.
    pub fault_injected: bool,
}

/// Why the policy does not hold.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "code", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PolicyViolation {
    /// The initramfs guard did not arm.
    GuardNotArmed,
    /// A block device is writable.
    DeviceWritable {
        /// Kernel name.
        device: String,
    },
    /// Swap is active on a block device.
    SwapActive {
        /// As `/proc/swaps` names it.
        device: String,
    },
    /// A software RAID array exists.
    RaidAssembled {
        /// Array name.
        array: String,
    },
    /// A device-mapper device exists (LVM volume, crypt mapping).
    MapperActive {
        /// Kernel name.
        device: String,
    },
    /// A filesystem from a block device other than the medium is mounted.
    InternalMount {
        /// Mount source.
        source: String,
        /// Mount point.
        target: String,
        /// Filesystem type.
        fstype: String,
    },
    /// The verifier was told to fail (`ocinye.selftest=policy-fail`).
    VerifierFaultInjected,
}

/// The verdict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PolicyVerdict {
    /// Every rule holds.
    Verified {
        /// Block devices and partitions checked.
        devices: usize,
    },
    /// At least one rule does not: the session is restricted.
    Restricted {
        /// Every reason.
        violations: Vec<PolicyViolation>,
    },
}

impl PolicyVerdict {
    /// Whether the session is restricted.
    #[must_use]
    pub const fn restricted(&self) -> bool {
        matches!(self, Self::Restricted { .. })
    }
}

/// Devices that are not storage anyone owns: loop devices carry the medium's
/// own SquashFS, ram and zram are memory.
fn exempt(name: &str) -> bool {
    ["loop", "ram", "zram"].iter().any(|p| {
        name.strip_prefix(p)
            .is_some_and(|r| r.chars().all(|c| c.is_ascii_digit()))
    })
}

fn mapper(name: &str) -> bool {
    name.strip_prefix("dm-")
        .is_some_and(|r| !r.is_empty() && r.chars().all(|c| c.is_ascii_digit()))
}

/// Mounts from block devices, except the medium itself (ISO 9660 or UDF,
/// which have no journal and are what the session booted from) and the loop
/// devices over it: `(source, target, fstype)`.
#[must_use]
pub fn internal_mounts(mountinfo: &str) -> Vec<(String, String, String)> {
    let mut out = vec![];
    for line in mountinfo.lines() {
        // "36 35 98:0 /mnt1 /mnt2 rw,noatime shared:1 - ext3 /dev/root rw,errors=continue"
        let Some((left, right)) = line.split_once(" - ") else {
            continue;
        };
        let target = left.split(' ').nth(4).unwrap_or_default();
        let mut r = right.split(' ');
        let (fstype, source) = (r.next().unwrap_or_default(), r.next().unwrap_or_default());
        let Some(dev) = source.strip_prefix("/dev/") else {
            continue;
        };
        if exempt(dev.rsplit('/').next().unwrap_or(dev)) {
            continue;
        }
        if matches!(fstype, "iso9660" | "udf") {
            continue;
        }
        out.push((source.to_owned(), target.to_owned(), fstype.to_owned()));
    }
    out
}

/// Arrays `/proc/mdstat` lists, active or not.
#[must_use]
pub fn raid_arrays(mdstat: &str) -> Vec<String> {
    mdstat
        .lines()
        .filter_map(|l| {
            let (name, rest) = l.split_once(" : ")?;
            (name.starts_with("md") && (rest.starts_with("active") || rest.starts_with("inactive")))
                .then(|| name.to_owned())
        })
        .collect()
}

/// Swap areas on block devices (`/proc/swaps`, header skipped).
#[must_use]
pub fn swap_devices(swaps: &str) -> Vec<String> {
    swaps
        .lines()
        .skip(1)
        .filter_map(|l| l.split_whitespace().next())
        .filter(|d| !d.contains("zram"))
        .map(str::to_owned)
        .collect()
}

/// Check the policy. Every violation is reported, not just the first.
#[must_use]
pub fn verify(i: &PolicyInputs) -> PolicyVerdict {
    let mut v = vec![];
    if i.guard_marker.as_deref().map(str::trim) != Some(GUARD_ARMED) {
        v.push(PolicyViolation::GuardNotArmed);
    }
    let mut checked = 0;
    for b in &i.block {
        if exempt(&b.name) {
            continue;
        }
        if mapper(&b.name) {
            v.push(PolicyViolation::MapperActive {
                device: b.name.clone(),
            });
            continue;
        }
        checked += 1;
        if !b.read_only {
            v.push(PolicyViolation::DeviceWritable {
                device: b.name.clone(),
            });
        }
    }
    for d in swap_devices(&i.swaps) {
        v.push(PolicyViolation::SwapActive { device: d });
    }
    for a in raid_arrays(&i.mdstat) {
        v.push(PolicyViolation::RaidAssembled { array: a });
    }
    for (source, target, fstype) in internal_mounts(&i.mountinfo) {
        v.push(PolicyViolation::InternalMount {
            source,
            target,
            fstype,
        });
    }
    if i.fault_injected {
        v.push(PolicyViolation::VerifierFaultInjected);
    }
    if v.is_empty() {
        PolicyVerdict::Verified { devices: checked }
    } else {
        PolicyVerdict::Restricted { violations: v }
    }
}

/// Read the inputs from the running system.
#[must_use]
pub fn read_system(fault_injected: bool) -> PolicyInputs {
    let read = |p: &str| std::fs::read_to_string(p).unwrap_or_default();
    let mut block = vec![];
    if let Ok(dir) = std::fs::read_dir("/sys/class/block") {
        for e in dir.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            // Unreadable counts as writable: the verifier never assumes safety.
            let ro = std::fs::read_to_string(e.path().join("ro")).is_ok_and(|s| s.trim() == "1");
            block.push(BlockFlag {
                name,
                read_only: ro,
            });
        }
    }
    block.sort_by(|a, b| a.name.cmp(&b.name));
    PolicyInputs {
        guard_marker: std::fs::read_to_string(GUARD_MARKER).ok(),
        block,
        swaps: read("/proc/swaps"),
        mdstat: read("/proc/mdstat"),
        mountinfo: read("/proc/self/mountinfo"),
        fault_injected,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MOUNTS_OK: &str = "\
24 1 0:22 / / rw,relatime shared:1 - overlay /cow rw,lowerdir=/filesystem.squashfs,upperdir=/cow/upper,workdir=/cow/work
30 24 11:0 / /cdrom ro,noatime shared:2 - iso9660 /dev/sr0 ro
31 24 7:0 / /rofs ro,noatime shared:3 - squashfs /dev/loop0 ro
32 24 0:5 / /dev rw,nosuid shared:4 - devtmpfs udev rw
33 24 0:26 / /tmp rw,nosuid,nodev shared:5 - tmpfs tmpfs rw
";
    const SWAPS_EMPTY: &str = "Filename\t\t\t\tType\t\tSize\t\tUsed\t\tPriority\n";
    const MD_EMPTY: &str = "Personalities : [raid1] \nunused devices: <none>\n";

    fn ok() -> PolicyInputs {
        PolicyInputs {
            guard_marker: Some("armed\n".into()),
            block: ["sda", "sda1", "sdb", "sr0", "nvme0n1", "nvme0n1p1"]
                .iter()
                .map(|n| BlockFlag {
                    name: (*n).into(),
                    read_only: true,
                })
                .chain(["loop0", "ram0", "zram0"].iter().map(|n| BlockFlag {
                    name: (*n).into(),
                    // The medium's own loop devices and memory devices are
                    // not judged.
                    read_only: false,
                }))
                .collect(),
            swaps: SWAPS_EMPTY.into(),
            mdstat: MD_EMPTY.into(),
            mountinfo: MOUNTS_OK.into(),
            fault_injected: false,
        }
    }

    fn violations(i: &PolicyInputs) -> Vec<PolicyViolation> {
        match verify(i) {
            PolicyVerdict::Restricted { violations } => violations,
            PolicyVerdict::Verified { .. } => vec![],
        }
    }

    #[test]
    fn a_politica_cumprida_e_verificada() {
        assert_eq!(verify(&ok()), PolicyVerdict::Verified { devices: 6 });
    }

    #[test]
    fn sem_marca_da_guarda_a_sessao_fica_restrita() {
        for marker in [None, Some(String::new()), Some("disarmed".into())] {
            let mut i = ok();
            i.guard_marker = marker;
            assert_eq!(violations(&i), vec![PolicyViolation::GuardNotArmed]);
        }
    }

    #[test]
    fn um_so_dispositivo_gravavel_restringe() {
        for dev in ["sda", "sda1", "nvme0n1p1", "sr0"] {
            let mut i = ok();
            i.block
                .iter_mut()
                .find(|b| b.name == dev)
                .unwrap()
                .read_only = false;
            assert_eq!(
                violations(&i),
                vec![PolicyViolation::DeviceWritable { device: dev.into() }],
                "{dev}"
            );
        }
        // A name that merely starts like an exempt one is still judged.
        let mut i = ok();
        i.block.push(BlockFlag {
            name: "loopback0".into(),
            read_only: false,
        });
        assert_eq!(
            violations(&i),
            vec![PolicyViolation::DeviceWritable {
                device: "loopback0".into()
            }]
        );
    }

    #[test]
    fn swap_activa_restringe() {
        let mut i = ok();
        i.swaps = format!(
            "{SWAPS_EMPTY}/dev/sda3                               partition\t131068\t\t0\t\t-2\n"
        );
        assert_eq!(
            violations(&i),
            vec![PolicyViolation::SwapActive {
                device: "/dev/sda3".into()
            }]
        );
    }

    #[test]
    fn raid_montado_restringe_mesmo_inactivo() {
        let mut i = ok();
        i.mdstat = "Personalities : [raid1] \nmd127 : active (auto-read-only) raid1 sdd[1] sdc[0]\n      97280 blocks super 1.2 [2/2] [UU]\n\nmd126 : inactive sde[0](S)\n      1024 blocks\n\nunused devices: <none>\n".into();
        assert_eq!(
            violations(&i),
            vec![
                PolicyViolation::RaidAssembled {
                    array: "md127".into()
                },
                PolicyViolation::RaidAssembled {
                    array: "md126".into()
                },
            ]
        );
    }

    #[test]
    fn volume_lvm_activo_restringe() {
        let mut i = ok();
        i.block.push(BlockFlag {
            name: "dm-0".into(),
            read_only: true,
        });
        assert_eq!(
            violations(&i),
            vec![PolicyViolation::MapperActive {
                device: "dm-0".into()
            }]
        );
    }

    #[test]
    fn qualquer_sistema_de_ficheiros_interno_montado_restringe() {
        // Read-only does not excuse it: a read-only mount can replay a journal.
        for (line, source, target, fstype) in [
            (
                "40 24 8:1 / /mnt ro,noatime shared:9 - ext4 /dev/sda1 ro",
                "/dev/sda1",
                "/mnt",
                "ext4",
            ),
            (
                "41 24 8:17 / /run/ocinye-oie/keys/sdb1 ro,nosuid,nodev,noexec shared:9 - vfat /dev/sdb1 ro,fmask=0022",
                "/dev/sdb1",
                "/run/ocinye-oie/keys/sdb1",
                "vfat",
            ),
            (
                "42 24 253:0 / /x rw shared:9 - xfs /dev/mapper/sentvg-sentlv rw",
                "/dev/mapper/sentvg-sentlv",
                "/x",
                "xfs",
            ),
        ] {
            let mut i = ok();
            i.mountinfo = format!("{MOUNTS_OK}{line}\n");
            assert_eq!(
                violations(&i),
                vec![PolicyViolation::InternalMount {
                    source: source.into(),
                    target: target.into(),
                    fstype: fstype.into()
                }],
                "{line}"
            );
        }
    }

    #[test]
    fn a_falha_injectada_restringe_e_nunca_alivia() {
        let mut i = ok();
        i.fault_injected = true;
        assert_eq!(violations(&i), vec![PolicyViolation::VerifierFaultInjected]);
        // With a real violation as well, both are reported: the injection
        // hides nothing.
        i.block[0].read_only = false;
        assert_eq!(violations(&i).len(), 2);
    }

    #[test]
    fn todas_as_violacoes_sao_relatadas() {
        let mut i = ok();
        i.guard_marker = None;
        i.block[0].read_only = false;
        i.swaps = format!("{SWAPS_EMPTY}/dev/sda3 partition 1 0 -2\n");
        i.mountinfo = format!("{MOUNTS_OK}40 24 8:1 / /mnt ro - ext4 /dev/sda1 ro\n");
        let v = verify(&i);
        assert!(v.restricted());
        assert_eq!(violations(&i).len(), 4);
    }

    #[test]
    fn o_suporte_e_os_seus_loops_nao_contam_como_internos() {
        assert!(internal_mounts(MOUNTS_OK).is_empty());
        // The medium as a USB stick, mounted from a partition.
        assert!(internal_mounts("30 24 8:33 / /cdrom ro - iso9660 /dev/sdc1 ro\n").is_empty());
    }
}
