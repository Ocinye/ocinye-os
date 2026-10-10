//! Which block device is the installation medium this system booted from
//! (D013 L0-H, medium trust).
//!
//! casper's own answer is "the first device that mounts and has a `casper/`
//! directory", in kernel enumeration order. That let an internal disk hijack
//! the boot (D013-SF-04) and made casper mount every filesystem on the way
//! (D013-SF-01). The rules here replace it, and none of them mounts anything:
//!
//! 1. **What a medium is.** A device whose ISO 9660 primary volume descriptor
//!    carries this build's label and this build's media identity: a value
//!    generated when the image is built, stored in the initramfs and written
//!    into the volume descriptor. A filesystem with a `casper/` directory, a
//!    partition with the right label, another Ocinye build: none of them is a
//!    candidate.
//! 2. **Which candidate booted.** An exact copy of the medium on another disk
//!    is a candidate too: content cannot tell them apart. The firmware can: the
//!    UEFI `BootCurrent` entry names the hardware path it loaded the boot
//!    loader from, and that path maps to one block device.
//! 3. **When the firmware does not say.** Only a candidate on removable or
//!    optical hardware is accepted, and only if it is the only one. Two is a
//!    refusal, not a guess; an internal fixed disk is never adopted.

use serde::Serialize;

/// The ISO volume label of an Ocinye installation medium (at most 32
/// characters, upper case).
pub const MEDIA_LABEL: &str = "OCINYE_OS";

/// Where ISO 9660 keeps its primary volume descriptor.
pub const PVD_OFFSET: u64 = 16 * 2048;
/// How much of it is needed.
pub const PVD_LEN: usize = 2048;

/// Prefix of the volume set identifier of an Ocinye medium.
pub const VOLUME_SET_PREFIX: &str = "OCINYE-MEDIA-";

/// The media identity as it is written into the volume set identifier.
#[must_use]
pub fn volume_set_id(media_id: &str) -> String {
    format!("{VOLUME_SET_PREFIX}{media_id}")
}

/// A media identity: 32 lower-case hexadecimal characters, from the kernel's
/// random source at build time. Not a secret; an identity.
#[must_use]
pub fn is_media_id(s: &str) -> bool {
    s.len() == 32
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// What an ISO 9660 primary volume descriptor says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IsoVolume {
    /// Volume identifier (the label).
    pub label: String,
    /// Volume set identifier.
    pub volume_set: String,
}

/// Read the label and the volume set identifier from the 2048 bytes at
/// [`PVD_OFFSET`]. `None` when it is not an ISO 9660 primary volume descriptor.
#[must_use]
pub fn parse_pvd(sector: &[u8]) -> Option<IsoVolume> {
    if sector.len() < 318 || sector[0] != 1 || &sector[1..6] != b"CD001" {
        return None;
    }
    let text = |r: std::ops::Range<usize>| {
        String::from_utf8_lossy(&sector[r])
            .trim_end_matches([' ', '\0'])
            .to_owned()
    };
    Some(IsoVolume {
        label: text(40..72),
        volume_set: text(190..318),
    })
}

/// Whether a volume is this build's medium.
#[must_use]
pub fn is_this_medium(v: &IsoVolume, label: &str, media_id: &str) -> bool {
    is_media_id(media_id) && v.label == label && v.volume_set == volume_set_id(media_id)
}

/// One node of a UEFI device path that locates hardware.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "node", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PathNode {
    /// `PciRoot(uid)`.
    PciRoot {
        /// ACPI `_UID`.
        uid: u32,
    },
    /// `Pci(device, function)`.
    Pci {
        /// Device number on the bus.
        device: u8,
        /// Function.
        function: u8,
    },
    /// `Scsi(target, lun)`.
    Scsi {
        /// Target id.
        target: u16,
        /// Logical unit.
        lun: u16,
    },
    /// `Sata(port, multiplier, lun)`.
    Sata {
        /// HBA port, from zero.
        port: u16,
    },
    /// `USB(port, interface)`: one hop; several in a row go through hubs.
    Usb {
        /// Port on the parent hub, from zero.
        port: u8,
    },
    /// `NVMe(namespace, eui)`.
    Nvme {
        /// Namespace id.
        namespace: u32,
    },
    /// A hardware or messaging node this parser does not interpret.
    Unknown {
        /// Device path type.
        kind: u8,
        /// Sub-type.
        sub: u8,
    },
}

/// Why a boot entry does not give a hardware path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "reason", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OriginUnknown {
    /// Not booted through UEFI, or the variables are not readable.
    NoFirmwareVariables,
    /// `BootCurrent` or its `Boot####` is missing or malformed.
    MalformedBootEntry,
    /// The entry does not start at a PCI root (a short-form or vendor path).
    NoHardwarePath,
    /// The hardware path names a controller this system does not show.
    ControllerNotFound,
    /// A node this parser does not interpret, with several devices left.
    AmbiguousHardware,
}

/// `BootCurrent`: the entry the firmware booted (the variable's data, without
/// the four attribute bytes efivarfs prepends).
#[must_use]
pub fn parse_boot_current(data: &[u8]) -> Option<u16> {
    (data.len() == 2).then(|| u16::from_le_bytes([data[0], data[1]]))
}

/// The hardware nodes of a `Boot####` load option (the variable's data), up
/// to the first media node (partition, CD-ROM boot entry, file) or the end.
pub fn parse_load_option(data: &[u8]) -> Result<Vec<PathNode>, OriginUnknown> {
    use OriginUnknown::{MalformedBootEntry, NoHardwarePath};
    // u32 attributes, u16 FilePathListLength, UTF-16 description, device paths.
    if data.len() < 8 {
        return Err(MalformedBootEntry);
    }
    let list_len = usize::from(u16::from_le_bytes([data[4], data[5]]));
    let mut at = 6;
    loop {
        let unit = data.get(at..at + 2).ok_or(MalformedBootEntry)?;
        at += 2;
        if unit == [0, 0] {
            break;
        }
    }
    let list = data.get(at..at + list_len).ok_or(MalformedBootEntry)?;
    let mut nodes = vec![];
    let mut p = 0;
    while p + 4 <= list.len() {
        let (kind, sub) = (list[p], list[p + 1]);
        let len = usize::from(u16::from_le_bytes([list[p + 2], list[p + 3]]));
        if len < 4 {
            return Err(MalformedBootEntry);
        }
        let body = list.get(p + 4..p + len).ok_or(MalformedBootEntry)?;
        let u16at = |i: usize| body.get(i..i + 2).map(|b| u16::from_le_bytes([b[0], b[1]]));
        let u32at = |i: usize| {
            body.get(i..i + 4)
                .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        };
        match (kind, sub) {
            // End of the whole path, or of this instance.
            (0x7f, _) => break,
            // Media nodes: what was loaded from the device, not where it is.
            (0x04, _) => break,
            // ACPI device: a PCI root bridge (PNP0A03, PNP0A08) or something else.
            (0x02, 0x01) => {
                let hid = u32at(0).ok_or(MalformedBootEntry)?;
                let uid = u32at(4).ok_or(MalformedBootEntry)?;
                if hid == 0x0a03_41d0 || hid == 0x0a08_41d0 {
                    nodes.push(PathNode::PciRoot { uid });
                } else {
                    nodes.push(PathNode::Unknown { kind, sub });
                }
            }
            (0x01, 0x01) => {
                let b = body.get(0..2).ok_or(MalformedBootEntry)?;
                nodes.push(PathNode::Pci {
                    function: b[0],
                    device: b[1],
                });
            }
            (0x03, 0x02) => nodes.push(PathNode::Scsi {
                target: u16at(0).ok_or(MalformedBootEntry)?,
                lun: u16at(2).ok_or(MalformedBootEntry)?,
            }),
            (0x03, 0x05) => nodes.push(PathNode::Usb {
                port: *body.first().ok_or(MalformedBootEntry)?,
            }),
            (0x03, 0x12) => nodes.push(PathNode::Sata {
                port: u16at(0).ok_or(MalformedBootEntry)?,
            }),
            (0x03, 0x17) => nodes.push(PathNode::Nvme {
                namespace: u32at(0).ok_or(MalformedBootEntry)?,
            }),
            _ => nodes.push(PathNode::Unknown { kind, sub }),
        }
        p += len;
    }
    if !matches!(nodes.first(), Some(PathNode::PciRoot { .. })) {
        return Err(NoHardwarePath);
    }
    Ok(nodes)
}

/// What the firmware says about where this boot came from, after mapping its
/// hardware path onto the block devices present.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "origin", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BootOrigin {
    /// Exactly this block device (kernel name).
    Device {
        /// Kernel name.
        name: String,
    },
    /// The path is understood but no block device is there yet.
    NotPresentYet,
    /// The firmware does not tell.
    Unknown {
        /// Why.
        why: OriginUnknown,
    },
}

/// A device that carries this build's medium.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Candidate {
    /// Kernel name.
    pub name: String,
    /// Removable or optical hardware (a USB device, a CD drive, a card).
    pub removable: bool,
}

/// How the medium was bound to this boot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MediumBinding {
    /// The firmware's boot entry names this device.
    FirmwareBootEntry,
    /// The firmware does not tell; this is the only candidate on removable
    /// hardware.
    OnlyRemovableCandidate,
}

/// The decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "decision", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MediumDecision {
    /// This device is the medium.
    Medium {
        /// Kernel name.
        name: String,
        /// Why it is trusted to be the one that booted.
        binding: MediumBinding,
    },
    /// Nothing can be decided yet (devices still appearing).
    Wait,
    /// Several devices could be the medium and nothing tells them apart.
    RefuseAmbiguous {
        /// The candidates.
        candidates: Vec<String>,
    },
}

/// Decide. `candidates` are devices whose volume descriptor is this build's
/// medium; `origin` is what the firmware says.
#[must_use]
pub fn decide(candidates: &[Candidate], origin: &BootOrigin) -> MediumDecision {
    match origin {
        BootOrigin::Device { name } if candidates.iter().any(|c| &c.name == name) => {
            return MediumDecision::Medium {
                name: name.clone(),
                binding: MediumBinding::FirmwareBootEntry,
            };
        }
        // The firmware names a device that is not here yet: wait for it rather
        // than take a look-alike that appeared first.
        BootOrigin::NotPresentYet => return MediumDecision::Wait,
        // The firmware names a device that is not a medium of this build (a
        // mapping this code got wrong, or a boot chain that moved on): it is
        // not evidence for any other device, so the rule for "the firmware
        // does not say" applies.
        BootOrigin::Device { .. } | BootOrigin::Unknown { .. } => {}
    }
    let removable: Vec<&Candidate> = candidates.iter().filter(|c| c.removable).collect();
    match removable.as_slice() {
        [] => MediumDecision::Wait,
        [one] => MediumDecision::Medium {
            name: one.name.clone(),
            binding: MediumBinding::OnlyRemovableCandidate,
        },
        many => MediumDecision::RefuseAmbiguous {
            candidates: many.iter().map(|c| c.name.clone()).collect(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "0123456789abcdef0123456789abcdef";

    fn pvd(label: &str, volset: &str) -> Vec<u8> {
        let mut s = vec![0u8; PVD_LEN];
        s[0] = 1;
        s[1..6].copy_from_slice(b"CD001");
        s[6] = 1;
        let mut put = |at: usize, len: usize, text: &str| {
            let mut field = vec![b' '; len];
            field[..text.len()].copy_from_slice(text.as_bytes());
            s[at..at + len].copy_from_slice(&field);
        };
        put(40, 32, label);
        put(190, 128, volset);
        s
    }

    fn unhex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    // Boot0001 as OVMF wrote it in the L0-S bench: the medium as a virtual CD
    // on a virtio-scsi controller, and as a USB stick on an xHCI controller.
    const BOOT_CD: &str = "010000001e0055004500460049002000510045004d0055002000510045004d0055002000430044002d0052004f004d002000000002010c00d041030a00000000010106000003030208000f0000007fff04004eac0881119f594d850ee21a522c59b2";
    const BOOT_USB: &str = "010000001c0055004500460049002000510045004d0055002000510045004d00550020005500530042002000480041005200440044005200490056004500200031002d0030003000300030003a00300030003a00300034002e0030002d003100000002010c00d041030a000000000101060000040305060000007fff04004eac0881119f594d850ee21a522c59b2";
    // Boot0002 there: the firmware's own shell, a firmware-volume path.
    const BOOT_SHELL: &str = "010000002c00450046004900200049006e007400650072006e0061006c0020005300680065006c006c00000004071400c9bdb87cebf8344faaea3ee4af6516a10406140083a5047c3e9e1c4fad65e05268d0b4d17fff0400";

    #[test]
    fn um_suporte_e_a_etiqueta_e_a_identidade_desta_construcao() {
        let good = parse_pvd(&pvd("OCINYE_OS", &volume_set_id(ID))).unwrap();
        assert!(is_this_medium(&good, "OCINYE_OS", ID));
        // Same label, another build.
        let other = parse_pvd(&pvd(
            "OCINYE_OS",
            &volume_set_id("ffffffffffffffffffffffffffffffff"),
        ))
        .unwrap();
        assert!(!is_this_medium(&other, "OCINYE_OS", ID));
        // Same identity, another label.
        let relabel = parse_pvd(&pvd("UBUNTU", &volume_set_id(ID))).unwrap();
        assert!(!is_this_medium(&relabel, "OCINYE_OS", ID));
        // No identity at all (the Phase A medium had none).
        let none = parse_pvd(&pvd("OCINYE_OS", "")).unwrap();
        assert!(!is_this_medium(&none, "OCINYE_OS", ID));
        // An empty or malformed expected identity matches nothing.
        assert!(!is_this_medium(&none, "OCINYE_OS", ""));
        assert!(!is_this_medium(&good, "OCINYE_OS", "0123"));
    }

    #[test]
    fn o_que_nao_e_iso9660_nao_e_candidato() {
        // ext4, FAT, zeros, a truncated read: not a volume descriptor, whatever
        // label the filesystem has and whatever directories it holds.
        assert_eq!(parse_pvd(&vec![0u8; PVD_LEN]), None);
        assert_eq!(parse_pvd(&[1, b'C', b'D']), None);
        let mut ext4_like = vec![0u8; PVD_LEN];
        ext4_like[0x38] = 0x53;
        ext4_like[0x39] = 0xef;
        assert_eq!(parse_pvd(&ext4_like), None);
        // A supplementary or boot descriptor is not the primary one.
        let mut boot = pvd("OCINYE_OS", &volume_set_id(ID));
        boot[0] = 0;
        assert_eq!(parse_pvd(&boot), None);
    }

    #[test]
    fn identidade_do_suporte_tem_forma_fixa() {
        assert!(is_media_id(ID));
        for bad in [
            "",
            "0123",
            &ID.to_uppercase(),
            &format!("{ID}0"),
            "g123456789abcdef0123456789abcdef",
        ] {
            assert!(!is_media_id(bad), "{bad}");
        }
    }

    #[test]
    fn a_entrada_de_arranque_do_firmware_da_o_caminho_fisico() {
        assert_eq!(
            parse_load_option(&unhex(BOOT_CD)),
            Ok(vec![
                PathNode::PciRoot { uid: 0 },
                PathNode::Pci {
                    device: 3,
                    function: 0
                },
                PathNode::Scsi { target: 15, lun: 0 },
            ])
        );
        assert_eq!(
            parse_load_option(&unhex(BOOT_USB)),
            Ok(vec![
                PathNode::PciRoot { uid: 0 },
                PathNode::Pci {
                    device: 4,
                    function: 0
                },
                PathNode::Usb { port: 0 },
            ])
        );
        // A firmware-volume entry is not a hardware path.
        assert_eq!(
            parse_load_option(&unhex(BOOT_SHELL)),
            Err(OriginUnknown::NoHardwarePath)
        );
        // Truncated anywhere: malformed, never a guess.
        let cd = unhex(BOOT_CD);
        for cut in [0, 5, 7, 40, 70, 80] {
            assert!(parse_load_option(&cd[..cut]).is_err(), "{cut}");
        }
        assert_eq!(parse_boot_current(&[1, 0]), Some(1));
        assert_eq!(parse_boot_current(&[1]), None);
        assert_eq!(parse_boot_current(&[1, 0, 0]), None);
    }

    fn cand(name: &str, removable: bool) -> Candidate {
        Candidate {
            name: name.into(),
            removable,
        }
    }

    fn dev(name: &str) -> BootOrigin {
        BootOrigin::Device { name: name.into() }
    }

    const UNKNOWN: BootOrigin = BootOrigin::Unknown {
        why: OriginUnknown::NoHardwarePath,
    };

    fn medium(name: &str, binding: MediumBinding) -> MediumDecision {
        MediumDecision::Medium {
            name: name.into(),
            binding,
        }
    }

    #[test]
    fn a_copia_num_disco_interno_nunca_e_o_suporte() {
        // The real medium is the CD; an exact copy sits on an internal disk
        // that the kernel enumerates first.
        let both = [cand("sda", false), cand("sr0", true)];
        assert_eq!(
            decide(&both, &dev("sr0")),
            medium("sr0", MediumBinding::FirmwareBootEntry)
        );
        // The firmware does not say: still the removable one, never the copy.
        assert_eq!(
            decide(&both, &UNKNOWN),
            medium("sr0", MediumBinding::OnlyRemovableCandidate)
        );
        // Only the copy is visible so far: wait for the real one.
        assert_eq!(
            decide(&[cand("sda", false)], &dev("sr0")),
            MediumDecision::Wait
        );
        assert_eq!(
            decide(&[cand("sda", false)], &BootOrigin::NotPresentYet),
            MediumDecision::Wait
        );
        // Only the copy exists and the firmware does not say: not adopted.
        assert_eq!(
            decide(&[cand("sda", false)], &UNKNOWN),
            MediumDecision::Wait
        );
        assert_eq!(decide(&[], &UNKNOWN), MediumDecision::Wait);
    }

    #[test]
    fn a_ordem_de_enumeracao_nao_decide() {
        for order in [
            vec![cand("sda", false), cand("sdb", true), cand("sr0", true)],
            vec![cand("sr0", true), cand("sdb", true), cand("sda", false)],
        ] {
            // The firmware picks, whatever comes first.
            assert_eq!(
                decide(&order, &dev("sdb")),
                medium("sdb", MediumBinding::FirmwareBootEntry)
            );
            assert_eq!(
                decide(&order, &dev("sda")),
                medium("sda", MediumBinding::FirmwareBootEntry)
            );
        }
    }

    #[test]
    fn duas_copias_removiveis_sem_firmware_e_recusa() {
        let two = [cand("sdb", true), cand("sdc", true)];
        assert_eq!(
            decide(&two, &UNKNOWN),
            MediumDecision::RefuseAmbiguous {
                candidates: vec!["sdb".into(), "sdc".into()]
            }
        );
        // With the firmware's word there is no ambiguity.
        assert_eq!(
            decide(&two, &dev("sdc")),
            medium("sdc", MediumBinding::FirmwareBootEntry)
        );
    }

    #[test]
    fn o_firmware_a_apontar_para_um_nao_suporte_nao_autoriza_outro_interno() {
        // The firmware names a device that does not carry this build's medium.
        // That is no reason to trust an internal copy.
        let c = [cand("sda", false)];
        assert_eq!(decide(&c, &dev("nvme0n1")), MediumDecision::Wait);
        // A single removable candidate is still acceptable by the fallback rule.
        let c = [cand("sda", false), cand("sdb", true)];
        assert_eq!(
            decide(&c, &dev("nvme0n1")),
            medium("sdb", MediumBinding::OnlyRemovableCandidate)
        );
    }
}
