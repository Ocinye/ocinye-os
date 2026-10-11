//! Ocinye Installation Environment: disk eligibility, selection and the
//! typed destructive confirmation (D013_STATE_MACHINE.md §4, ISO-3, T-16).
//!
//! Nothing is written to any disk before [`confirm`] succeeds. There is no
//! default disk: a selection names a disk by its stable physical path, and the
//! confirmation is typed from what the operator reads on that disk's row.

use serde::{Deserialize, Serialize};

/// Smallest system disk the OIE offers: the base plus the free space D011
/// requires (`MIN_DISK_GB = 15`) [P, PD-21].
pub const MIN_DISK_BYTES: u64 = 20_000_000_000;

/// What the OIE probed about one block device (lsblk/udev facts).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProbedDisk {
    /// Kernel name, e.g. "nvme0n1".
    pub device: String,
    /// Stable path, e.g. "/dev/disk/by-path/pci-0000:00:17.0-ata-1".
    pub by_path: String,
    /// Size.
    pub bytes: u64,
    /// Model as reported (may be empty).
    pub model: String,
    /// Serial, when readable.
    pub serial: Option<String>,
    /// WWN, when readable.
    pub wwn: Option<String>,
    /// Partition count.
    pub partitions: u32,
    /// Filesystem types found on it.
    pub filesystems: Vec<String>,
    /// It holds the medium the OIE booted from.
    pub boot_media: bool,
    /// Some partition of it is mounted.
    pub mounted: bool,
    /// Read-only device (`ro=1`).
    pub read_only: bool,
}

/// A disk as the OIE shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstallationTargetDisk {
    /// Kernel name.
    pub device: String,
    /// Stable path; the only way a disk is selected.
    pub by_path: String,
    /// Size.
    pub bytes: u64,
    /// Model.
    pub model: String,
    /// Serial, when readable.
    pub serial: Option<String>,
    /// WWN, when readable.
    pub wwn: Option<String>,
    /// Partition count.
    pub partitions: u32,
    /// Filesystems found.
    pub filesystems: Vec<String>,
    /// Why it can or cannot be chosen.
    pub protection: DiskProtection,
    /// What the operator types to confirm erasing it.
    pub confirmation_token: String,
}

/// Why a disk can or cannot be chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DiskProtection {
    /// Selectable.
    Eligible,
    /// The medium the OIE booted from: never selectable.
    InstallationMedia,
    /// Mounted: never selectable.
    Mounted,
    /// Below [`MIN_DISK_BYTES`].
    TooSmall,
    /// Read-only device.
    ReadOnly,
    /// Selectable only through the ambiguity guard: another disk has the
    /// same model and size and neither has a serial or WWN.
    Ambiguous,
}

impl DiskProtection {
    /// Whether the operator may select it.
    #[must_use]
    pub const fn selectable(self) -> bool {
        matches!(self, Self::Eligible | Self::Ambiguous)
    }
}

/// Typed OIE errors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "code", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OieError {
    /// Not amd64/arm64.
    UnsupportedArchitecture,
    /// Legacy BIOS (`NOT_SUPPORTED_V1`).
    UnsupportedFirmware,
    /// Not enough memory.
    InsufficientMemory {
        /// Found.
        bytes: u64,
        /// Required.
        required: u64,
    },
    /// No disk can be chosen.
    NoEligibleDisk,
    /// The selection does not name exactly one disk.
    AmbiguousDisk,
    /// The selected disk is protected.
    DiskProtected {
        /// Why.
        protection: DiskProtection,
    },
    /// The typed confirmation does not match.
    ConfirmationMismatch,
    /// A payload file differs from the content manifest.
    PayloadDigestMismatch {
        /// The file.
        path: String,
    },
    /// The embedded image manifest failed validation.
    ImageManifestInvalid {
        /// The first failing field.
        field: String,
    },
    /// Writing the disk failed.
    DiskWriteFailed {
        /// Device.
        device: String,
    },
    /// The boot loader did not install.
    BootloaderInstallFailed,
    /// The installer's storage protection is not in force: some disk other
    /// than a confirmed target is writable, swap is active, or an array or
    /// volume is assembled. Nothing is offered for installation.
    StorageProtectionUnverified {
        /// The first reason, as a stable code.
        reason: String,
    },
    /// The disk at the confirmed stable path is no longer the disk that was
    /// confirmed (another serial, size or name). Nothing was made writable.
    TargetChanged,
    /// The confirmed target could not be made writable, or more than the
    /// confirmed target is.
    TargetReleaseFailed {
        /// Device.
        device: String,
    },
}

fn tail4(s: &str) -> Option<String> {
    let t: String = s
        .trim()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect();
    (t.len() >= 4).then(|| t[t.len() - 4..].to_ascii_uppercase())
}

/// The token for a disk: last 4 alphanumerics of the serial, else of the WWN,
/// else the kernel device name (the ambiguity-guard identifier).
#[must_use]
pub fn confirmation_token(d: &ProbedDisk) -> String {
    d.serial
        .as_deref()
        .and_then(tail4)
        .or_else(|| d.wwn.as_deref().and_then(tail4))
        .unwrap_or_else(|| d.device.clone())
}

fn identifiable(d: &ProbedDisk) -> bool {
    d.serial.as_deref().and_then(tail4).is_some() || d.wwn.as_deref().and_then(tail4).is_some()
}

/// Classify every probed disk. Protection wins over ambiguity: an ambiguous
/// pair where one is the boot medium leaves the other `Ambiguous` anyway,
/// because model and size alone cannot tell them apart.
#[must_use]
pub fn classify(disks: &[ProbedDisk]) -> Vec<InstallationTargetDisk> {
    disks
        .iter()
        .map(|d| {
            let protection = if d.boot_media {
                DiskProtection::InstallationMedia
            } else if d.mounted {
                DiskProtection::Mounted
            } else if d.read_only {
                DiskProtection::ReadOnly
            } else if d.bytes < MIN_DISK_BYTES {
                DiskProtection::TooSmall
            } else if !identifiable(d)
                && disks.iter().any(|o| {
                    o.by_path != d.by_path
                        && o.model == d.model
                        && o.bytes == d.bytes
                        && !identifiable(o)
                })
            {
                DiskProtection::Ambiguous
            } else {
                DiskProtection::Eligible
            };
            InstallationTargetDisk {
                device: d.device.clone(),
                by_path: d.by_path.clone(),
                bytes: d.bytes,
                model: d.model.clone(),
                serial: d.serial.clone(),
                wwn: d.wwn.clone(),
                partitions: d.partitions,
                filesystems: d.filesystems.clone(),
                protection,
                confirmation_token: confirmation_token(d),
            }
        })
        .collect()
}

/// The operator's selection, by stable path only.
///
/// # Errors
/// `NoEligibleDisk` when nothing can be chosen; `AmbiguousDisk` when the path
/// names no disk or more than one; `DiskProtected` for a protected disk.
pub fn select<'a>(
    disks: &'a [InstallationTargetDisk],
    by_path: &str,
) -> Result<&'a InstallationTargetDisk, OieError> {
    if !disks.iter().any(|d| d.protection.selectable()) {
        return Err(OieError::NoEligibleDisk);
    }
    let mut hits = disks.iter().filter(|d| d.by_path == by_path);
    let (Some(d), None) = (hits.next(), hits.next()) else {
        return Err(OieError::AmbiguousDisk);
    };
    if !d.protection.selectable() {
        return Err(OieError::DiskProtected {
            protection: d.protection,
        });
    }
    Ok(d)
}

/// Destructive confirmation: the typed text equals the disk's token
/// (case-insensitive, surrounding blanks ignored). Nothing else is accepted —
/// not "yes", not the device name when a serial exists, not an empty line.
///
/// # Errors
/// `ConfirmationMismatch`.
pub fn confirm(disk: &InstallationTargetDisk, typed: &str) -> Result<(), OieError> {
    let t = typed.trim();
    if !t.is_empty()
        && t.eq_ignore_ascii_case(&disk.confirmation_token)
        && disk.protection.selectable()
    {
        Ok(())
    } else {
        Err(OieError::ConfirmationMismatch)
    }
}

/// The authority to write one disk: this disk, in an `install` boot, after its
/// typed destructive confirmation was accepted.
///
/// It is the only thing the installation step takes, it can only be made by
/// [`authorize_installation`], and it is neither `Clone` nor serialisable. A
/// device name alone never makes a disk writable: the installer releases the
/// block guard for what this value names, and for nothing else.
#[derive(Debug)]
pub struct ConfirmedTarget<'a> {
    disk: &'a InstallationTargetDisk,
    _authority: &'a crate::bootmode::InstallAuthority<'a>,
}

impl<'a> ConfirmedTarget<'a> {
    /// The confirmed disk.
    #[must_use]
    pub const fn disk(&self) -> &'a InstallationTargetDisk {
        self.disk
    }
}

/// The destructive confirmation, as the one step that grants write authority
/// over a disk. Same rule as [`confirm`]; it additionally requires the boot to
/// be the installer.
///
/// # Errors
/// `ConfirmationMismatch`.
pub fn authorize_installation<'a>(
    authority: &'a crate::bootmode::InstallAuthority<'a>,
    disk: &'a InstallationTargetDisk,
    typed: &str,
) -> Result<ConfirmedTarget<'a>, OieError> {
    confirm(disk, typed)?;
    Ok(ConfirmedTarget {
        disk,
        _authority: authority,
    })
}

/// Whether a freshly probed disk is still the one that was confirmed: same
/// stable path, same kernel name, same size, same serial and WWN, and still
/// selectable.
#[must_use]
pub fn same_target(confirmed: &InstallationTargetDisk, now: &InstallationTargetDisk) -> bool {
    !confirmed.by_path.is_empty()
        && confirmed.by_path == now.by_path
        && confirmed.device == now.device
        && confirmed.bytes == now.bytes
        && confirmed.serial == now.serial
        && confirmed.wwn == now.wwn
        && confirmed.confirmation_token == now.confirmation_token
        && now.protection.selectable()
}

/// What the OIE was built to install, embedded in its own live root
/// (`/usr/lib/ocinye/oie/payload.json`) so a corrupted or altered medium is
/// caught before anything is written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OiePayload {
    /// Image identity.
    pub image: crate::manifest::OcinyeImageVersion,
    /// Path of the root filesystem image on the medium (curtin `fsimage`).
    pub rootfs: String,
    /// Its SHA-256.
    pub rootfs_sha256: crate::manifest::Sha256Hex,
    /// Its size.
    pub rootfs_bytes: u64,
    /// SHA-256 of `IMAGE_CONTENT.json` on the medium (= the one in the rootfs).
    pub content_sha256: crate::manifest::Sha256Hex,
    /// ISO volume label (protects the medium even when it is not mounted).
    pub media_label: String,
}

/// One step of an installation as the journal records it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OieStep {
    /// `media_check` · `install` · `verify` · `first_boot`.
    pub step: String,
    /// Seconds.
    pub seconds: u64,
    /// `OK` or the error code.
    pub result: String,
}

/// `/var/log/ocinye/oie-install.json` on the installed disk. No secret: the
/// operator key appears by fingerprint only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OieInstallJournal {
    /// Image identity.
    pub image: crate::manifest::OcinyeImageVersion,
    /// Disk chosen.
    pub disk: InstallationTargetDisk,
    /// SHA-256 of the payload written.
    pub payload_sha256: crate::manifest::Sha256Hex,
    /// Operator key placed on `ocinye`, if any.
    pub operator_key: Option<crate::firstboot::KeyFingerprint>,
    /// Steps in order.
    pub steps: Vec<OieStep>,
    /// RFC 3339.
    pub started_at: String,
    /// RFC 3339.
    pub finished_at: String,
}

/// The fixed layout the OIE writes (D013 §B: GPT · ESP 1 GiB · ext4 · no swap).
pub mod layout {
    /// EFI system partition size.
    pub const ESP_BYTES: u64 = 1 << 30;
    /// ESP filesystem label.
    pub const ESP_LABEL: &str = "OCINYE-ESP";
    /// Root filesystem label.
    pub const ROOT_LABEL: &str = "ocinye-root";
    /// No swap partition or file.
    pub const SWAP: bool = false;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn disk(dev: &str, bytes: u64, serial: Option<&str>) -> ProbedDisk {
        ProbedDisk {
            device: dev.into(),
            by_path: format!("/dev/disk/by-path/pci-0000:00:0{}.0", dev.len()),
            bytes,
            model: "QEMU HARDDISK".into(),
            serial: serial.map(Into::into),
            wwn: None,
            partitions: 0,
            filesystems: vec![],
            boot_media: false,
            mounted: false,
            read_only: false,
        }
    }

    const G40: u64 = 40_000_000_000;

    #[test]
    fn suporte_e_montados_nunca_sao_escolhiveis() {
        let mut media = disk("sr0", G40, Some("QM00003"));
        media.boot_media = true;
        let mut mounted = disk("vdb", G40, Some("MNT0001"));
        mounted.by_path = "p-mounted".into();
        mounted.mounted = true;
        let mut small = disk("vdc", 8_000_000_000, Some("SMALL01"));
        small.by_path = "p-small".into();
        let mut target = disk("vda", G40, Some("OCY-TARGET-7F3A"));
        target.by_path = "p-target".into();
        let all = classify(&[media.clone(), mounted, small, target]);
        assert_eq!(all[0].protection, DiskProtection::InstallationMedia);
        assert_eq!(all[1].protection, DiskProtection::Mounted);
        assert_eq!(all[2].protection, DiskProtection::TooSmall);
        assert_eq!(all[3].protection, DiskProtection::Eligible);
        assert_eq!(
            select(&all, &media.by_path),
            Err(OieError::DiskProtected {
                protection: DiskProtection::InstallationMedia
            })
        );
        assert_eq!(
            select(&all, "p-mounted"),
            Err(OieError::DiskProtected {
                protection: DiskProtection::Mounted
            })
        );
        assert_eq!(select(&all, "nope"), Err(OieError::AmbiguousDisk));
        let d = select(&all, "p-target").unwrap();
        assert_eq!(d.confirmation_token, "7F3A");
    }

    #[test]
    fn sem_disco_elegivel_nao_ha_escolha() {
        let mut media = disk("sr0", G40, Some("QM00003"));
        media.boot_media = true;
        let all = classify(&[media.clone()]);
        assert_eq!(select(&all, &media.by_path), Err(OieError::NoEligibleDisk));
    }

    #[test]
    fn confirmacao_errada_nao_escreve() {
        let all = classify(&[disk("vda", G40, Some("SN-ab12"))]);
        let d = &all[0];
        for wrong in ["", "yes", "sim", "vda", "AB1", "AB123", "  "] {
            assert_eq!(
                confirm(d, wrong),
                Err(OieError::ConfirmationMismatch),
                "{wrong:?}"
            );
        }
        assert_eq!(confirm(d, " ab12 "), Ok(()));
    }

    #[test]
    fn discos_identicos_sem_serie_passam_pela_guarda() {
        let mut a = disk("vdb", G40, None);
        a.by_path = "p-a".into();
        let mut b = disk("vdc", G40, None);
        b.by_path = "p-b".into();
        let all = classify(&[a, b]);
        assert!(all
            .iter()
            .all(|d| d.protection == DiskProtection::Ambiguous));
        let chosen = select(&all, "p-b").unwrap();
        // Without serial or WWN the token is the kernel name of that disk.
        assert_eq!(chosen.confirmation_token, "vdc");
        assert_eq!(confirm(chosen, "vdb"), Err(OieError::ConfirmationMismatch));
        assert_eq!(confirm(chosen, "vdc"), Ok(()));
    }

    #[test]
    fn so_a_confirmacao_escrita_num_arranque_de_instalacao_da_autoridade_sobre_o_disco() {
        use crate::bootmode::{InstallAuthority, ResolvedMode};
        let all = classify(&[disk("vda", G40, Some("SN-ab12"))]);
        let d = &all[0];
        let boot = ResolvedMode::from_cmdline("boot=casper ocinye.mode=install");
        let authority = InstallAuthority::from_mode(&boot).unwrap();
        for wrong in ["", "yes", "vda", "AB1", "AB123"] {
            assert_eq!(
                authorize_installation(&authority, d, wrong).err(),
                Some(OieError::ConfirmationMismatch),
                "{wrong:?}"
            );
        }
        let target = authorize_installation(&authority, d, "ab12").unwrap();
        assert_eq!(target.disk().device, "vda");
        // No other boot mode has an authority to pass, so none can obtain a
        // confirmed target, whatever is typed.
        for other in ["live", "hardware-check", "recovery", "bogus"] {
            let boot = ResolvedMode::from_cmdline(&format!("boot=casper ocinye.mode={other}"));
            assert!(InstallAuthority::from_mode(&boot).is_none(), "{other}");
        }
        // A protected disk is never confirmed, even with its own token.
        let mut media = disk("sr0", G40, Some("QM00003"));
        media.boot_media = true;
        let all = classify(&[media]);
        assert!(authorize_installation(&authority, &all[0], "0003").is_err());
    }

    #[test]
    fn o_disco_confirmado_tem_de_continuar_a_ser_o_mesmo() {
        let confirmed = classify(&[disk("vda", G40, Some("SN-ab12"))]).remove(0);
        assert!(same_target(&confirmed, &confirmed.clone()));
        let change = |f: &dyn Fn(&mut InstallationTargetDisk)| {
            let mut now = confirmed.clone();
            f(&mut now);
            same_target(&confirmed, &now)
        };
        assert!(!change(&|d| d.device = "vdb".into()));
        assert!(!change(&|d| d.by_path = "/dev/disk/by-path/other".into()));
        assert!(!change(&|d| d.bytes += 512));
        assert!(!change(&|d| d.serial = Some("SN-zz99".into())));
        assert!(!change(&|d| d.wwn = Some("0x5000".into())));
        assert!(!change(&|d| d.protection = DiskProtection::Mounted));
        // A disk with no stable path was never selectable and is never "the same".
        let mut nopath = confirmed.clone();
        nopath.by_path = String::new();
        assert!(!same_target(&nopath, &nopath.clone()));
    }

    #[test]
    fn disco_em_branco_unico_sem_serie_e_elegivel() {
        let all = classify(&[disk("vda", G40, None)]);
        assert_eq!(all[0].protection, DiskProtection::Eligible);
        assert_eq!(all[0].confirmation_token, "vda");
    }
}
