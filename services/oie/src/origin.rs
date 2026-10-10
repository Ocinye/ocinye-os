//! Finding the installation medium this system booted from, without mounting
//! anything (D013 L0-H). The rules are `ocinye_image_contracts::medium`; this
//! module reads what they need: the firmware's boot entry, the hardware tree
//! in sysfs, and the first volume descriptor of each disk.
//!
//! It runs in the initramfs (`ocinye-oie select-medium`), before casper looks
//! for anything.

use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use ocinye_image_contracts::medium::{
    decide, is_this_medium, parse_boot_current, parse_load_option, parse_pvd, BootOrigin,
    Candidate, MediumDecision, OriginUnknown, PathNode, PVD_LEN, PVD_OFFSET,
};

/// The EFI global variable namespace.
const EFI_GLOBAL: &str = "8be4df61-93ca-11d2-aa0d-00e098032b8c";

/// Where to read from (the real system, or a tree built by a test).
#[derive(Debug, Clone)]
pub struct Roots {
    /// `/sys`.
    pub sys: PathBuf,
    /// `/dev`.
    pub dev: PathBuf,
}

impl Roots {
    /// The running system.
    #[must_use]
    pub fn system() -> Self {
        Self {
            sys: PathBuf::from("/sys"),
            dev: PathBuf::from("/dev"),
        }
    }
}

fn efi_var(sys: &Path, name: &str) -> Option<Vec<u8>> {
    let p = sys.join(format!("firmware/efi/efivars/{name}-{EFI_GLOBAL}"));
    let data = fs::read(p).ok()?;
    // efivarfs prefixes the value with four attribute bytes.
    (data.len() >= 4).then(|| data[4..].to_vec())
}

/// The hardware path of the entry the firmware booted.
pub fn firmware_path(sys: &Path) -> Result<Vec<PathNode>, OriginUnknown> {
    if !sys.join("firmware/efi/efivars").is_dir() {
        return Err(OriginUnknown::NoFirmwareVariables);
    }
    let current = efi_var(sys, "BootCurrent")
        .and_then(|d| parse_boot_current(&d))
        .ok_or(OriginUnknown::MalformedBootEntry)?;
    let entry =
        efi_var(sys, &format!("Boot{current:04X}")).ok_or(OriginUnknown::MalformedBootEntry)?;
    parse_load_option(&entry)
}

fn child_named(dir: &Path, matches: impl Fn(&str) -> bool) -> Option<PathBuf> {
    let mut found: Vec<PathBuf> = fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter(|e| matches(&e.file_name().to_string_lossy()))
        .map(|e| e.path())
        .collect();
    found.sort();
    (found.len() == 1).then(|| found.remove(0))
}

fn pci_root(sys: &Path, uid: u32) -> Option<PathBuf> {
    let devices = sys.join("devices");
    let mut roots: Vec<PathBuf> = fs::read_dir(&devices)
        .ok()?
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("pci"))
        .map(|e| e.path())
        .collect();
    roots.sort();
    let by_uid: Vec<&PathBuf> = roots
        .iter()
        .filter(|r| {
            fs::read_to_string(r.join("firmware_node/uid"))
                .is_ok_and(|s| s.trim().parse::<u32>() == Ok(uid))
        })
        .collect();
    match by_uid.as_slice() {
        [one] => Some((*one).clone()),
        [] if roots.len() == 1 && uid == 0 => Some(roots.remove(0)),
        _ => None,
    }
}

fn is_pci_function(name: &str, device: u8, function: u8) -> bool {
    // "0000:00:03.0": domain, bus, device.function.
    let Some((rest, func)) = name.rsplit_once('.') else {
        return false;
    };
    let Some((_, dev)) = rest.rsplit_once(':') else {
        return false;
    };
    name.len() >= 12
        && u8::from_str_radix(dev, 16) == Ok(device)
        && u8::from_str_radix(func, 16) == Ok(function)
}

fn skip(name: &str) -> bool {
    ["loop", "ram", "zram", "dm-", "md", "nbd"]
        .iter()
        .any(|p| name.starts_with(p))
}

/// Whole disks and optical drives, with where each sits in the hardware tree.
fn disks(sys: &Path) -> Vec<(String, PathBuf)> {
    let mut out = vec![];
    if let Ok(dir) = fs::read_dir(sys.join("block")) {
        for e in dir.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if skip(&name) {
                continue;
            }
            if let Ok(real) = fs::canonicalize(e.path()) {
                out.push((name, real));
            }
        }
    }
    out.sort();
    out
}

fn components(p: &Path) -> Vec<String> {
    p.components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect()
}

fn scsi_address(component: &str, target: u16, lun: u16) -> bool {
    // "0:0:15:0": host, channel, target, lun.
    let parts: Vec<&str> = component.split(':').collect();
    parts.len() == 4
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
        && parts[2].parse::<u16>() == Ok(target)
        && parts[3].parse::<u16>() == Ok(lun)
}

fn usb_port_chain(component: &str, chain: &[u8]) -> bool {
    // "1-1" or "1-1.2": bus, then one-based ports through each hub.
    let Some((bus, ports)) = component.split_once('-') else {
        return false;
    };
    if bus.is_empty() || !bus.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    let want: Vec<String> = chain
        .iter()
        .map(|p| (u16::from(*p) + 1).to_string())
        .collect();
    ports == want.join(".")
}

fn sata_port(controller: &Path, comps: &[String], port: u16) -> bool {
    comps.iter().any(|c| {
        c.strip_prefix("ata")
            .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
            && fs::read_to_string(controller.join(c).join("ata_port").join(c).join("port_no"))
                .is_ok_and(|s| s.trim().parse::<u32>() == Ok(u32::from(port) + 1))
    })
}

/// Map a firmware hardware path onto the block devices present.
#[must_use]
pub fn resolve(sys: &Path, nodes: &[PathNode]) -> BootOrigin {
    let unknown = |why| BootOrigin::Unknown { why };
    let mut it = nodes.iter().peekable();
    let Some(PathNode::PciRoot { uid }) = it.next() else {
        return unknown(OriginUnknown::NoHardwarePath);
    };
    let Some(mut at) = pci_root(sys, *uid) else {
        return unknown(OriginUnknown::ControllerNotFound);
    };
    let mut any_pci = false;
    while let Some(PathNode::Pci { device, function }) = it.peek() {
        let Some(next) = child_named(&at, |n| is_pci_function(n, *device, *function)) else {
            return unknown(OriginUnknown::ControllerNotFound);
        };
        at = next;
        any_pci = true;
        it.next();
    }
    if !any_pci {
        return unknown(OriginUnknown::NoHardwarePath);
    }
    let Ok(controller) = fs::canonicalize(&at) else {
        return unknown(OriginUnknown::ControllerNotFound);
    };
    let mut left: Vec<(String, PathBuf)> = disks(sys)
        .into_iter()
        .filter(|(_, real)| real.starts_with(&controller))
        .collect();
    let rest: Vec<&PathNode> = it.collect();
    let usb: Vec<u8> = rest
        .iter()
        .filter_map(|n| match n {
            PathNode::Usb { port } => Some(*port),
            _ => None,
        })
        .collect();
    let mut uninterpreted = false;
    if !usb.is_empty() {
        left.retain(|(_, real)| components(real).iter().any(|c| usb_port_chain(c, &usb)));
    }
    for n in rest {
        match n {
            PathNode::Scsi { target, lun } => {
                left.retain(|(_, real)| {
                    components(real)
                        .iter()
                        .any(|c| scsi_address(c, *target, *lun))
                });
            }
            PathNode::Sata { port } => {
                left.retain(|(_, real)| sata_port(&controller, &components(real), *port));
            }
            PathNode::Nvme { namespace } => {
                left.retain(|(name, _)| {
                    fs::read_to_string(sys.join("block").join(name).join("nsid"))
                        .is_ok_and(|s| s.trim().parse::<u32>() == Ok(*namespace))
                });
            }
            PathNode::Usb { .. } => {}
            PathNode::PciRoot { .. } | PathNode::Pci { .. } | PathNode::Unknown { .. } => {
                uninterpreted = true;
            }
        }
    }
    match left.as_slice() {
        [] => BootOrigin::NotPresentYet,
        [(name, _)] if !uninterpreted => BootOrigin::Device { name: name.clone() },
        // A node this code does not interpret may have been the one that told
        // two devices apart: one survivor is then only probably right.
        [_] | [_, _, ..] => unknown(OriginUnknown::AmbiguousHardware),
    }
}

/// What the firmware says, mapped.
#[must_use]
pub fn boot_origin(sys: &Path) -> BootOrigin {
    match firmware_path(sys) {
        Ok(nodes) => resolve(sys, &nodes),
        Err(why) => BootOrigin::Unknown { why },
    }
}

fn removable(sys: &Path, name: &str, real: &Path) -> bool {
    let read = |f: &str| fs::read_to_string(sys.join("block").join(name).join(f));
    read("removable").is_ok_and(|s| s.trim() == "1")
        // SCSI peripheral type 5: a CD or DVD drive.
        || read("device/type").is_ok_and(|s| s.trim() == "5")
        || components(real).iter().any(|c| {
            c.strip_prefix("usb")
                .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
        })
}

fn volume_descriptor(dev: &Path) -> Option<Vec<u8>> {
    // Opened for reading only; nothing is mounted.
    let mut f = fs::File::open(dev).ok()?;
    f.seek(SeekFrom::Start(PVD_OFFSET)).ok()?;
    let mut buf = vec![0u8; PVD_LEN];
    f.read_exact(&mut buf).ok()?;
    Some(buf)
}

/// Devices that carry this build's medium.
#[must_use]
pub fn candidates(roots: &Roots, label: &str, media_id: &str) -> Vec<Candidate> {
    disks(&roots.sys)
        .into_iter()
        .filter(|(name, _)| {
            volume_descriptor(&roots.dev.join(name))
                .and_then(|s| parse_pvd(&s))
                .is_some_and(|v| is_this_medium(&v, label, media_id))
        })
        .map(|(name, real)| Candidate {
            removable: removable(&roots.sys, &name, &real),
            name,
        })
        .collect()
}

/// Everything `select-medium` found and decided.
#[derive(Debug, serde::Serialize)]
pub struct Selection {
    /// The decision.
    pub decision: MediumDecision,
    /// What the firmware said.
    pub origin: BootOrigin,
    /// Devices carrying this build's medium.
    pub candidates: Vec<Candidate>,
}

/// Find the medium.
#[must_use]
pub fn select(roots: &Roots, label: &str, media_id: &str) -> Selection {
    let candidates = candidates(roots, label, media_id);
    let origin = boot_origin(&roots.sys);
    Selection {
        decision: decide(&candidates, &origin),
        origin,
        candidates,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ocinye_image_contracts::medium::{volume_set_id, MediumBinding, MEDIA_LABEL};
    use std::os::unix::fs::symlink;

    const ID: &str = "0123456789abcdef0123456789abcdef";

    struct Tree {
        root: PathBuf,
    }

    impl Tree {
        fn new(name: &str) -> Self {
            let root =
                std::env::temp_dir().join(format!("ocinye-origin-{}-{name}", std::process::id()));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(root.join("sys/block")).unwrap();
            fs::create_dir_all(root.join("sys/devices/pci0000:00/firmware_node")).unwrap();
            fs::write(root.join("sys/devices/pci0000:00/firmware_node/uid"), "0\n").unwrap();
            fs::create_dir_all(root.join("dev")).unwrap();
            Self { root }
        }
        fn roots(&self) -> Roots {
            Roots {
                sys: self.root.join("sys"),
                dev: self.root.join("dev"),
            }
        }
        /// A disk at a hardware path under the PCI root, with what its first
        /// volume descriptor holds (`None`: not ISO 9660).
        fn disk(
            &self,
            name: &str,
            hw: &str,
            medium: Option<(&str, &str)>,
            attrs: &[(&str, &str)],
        ) -> &Self {
            let real = self
                .root
                .join("sys/devices/pci0000:00")
                .join(hw)
                .join("block")
                .join(name);
            fs::create_dir_all(&real).unwrap();
            for (f, v) in attrs {
                let p = real.join(f);
                fs::create_dir_all(p.parent().unwrap()).unwrap();
                fs::write(p, format!("{v}\n")).unwrap();
            }
            symlink(&real, self.root.join("sys/block").join(name)).unwrap();
            let mut image = vec![0u8; usize::try_from(PVD_OFFSET).unwrap() + PVD_LEN];
            if let Some((label, volset)) = medium {
                let s = &mut image[usize::try_from(PVD_OFFSET).unwrap()..];
                s[0] = 1;
                s[1..6].copy_from_slice(b"CD001");
                s[40..72].fill(b' ');
                s[40..40 + label.len()].copy_from_slice(label.as_bytes());
                s[190..318].fill(b' ');
                s[190..190 + volset.len()].copy_from_slice(volset.as_bytes());
            }
            fs::write(self.root.join("dev").join(name), image).unwrap();
            self
        }
        fn boot_entry(&self, hex: &str) -> &Self {
            let dir = self.root.join("sys/firmware/efi/efivars");
            fs::create_dir_all(&dir).unwrap();
            let mut current = vec![6, 0, 0, 0];
            current.extend([1, 0]);
            fs::write(dir.join(format!("BootCurrent-{EFI_GLOBAL}")), current).unwrap();
            let mut entry = vec![7, 0, 0, 0];
            entry.extend(
                (0..hex.len())
                    .step_by(2)
                    .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap()),
            );
            fs::write(dir.join(format!("Boot0001-{EFI_GLOBAL}")), entry).unwrap();
            self
        }
    }

    impl Drop for Tree {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    // Boot0001 as OVMF wrote it in the L0-S bench.
    const BOOT_CD: &str = "010000001e0055004500460049002000510045004d0055002000510045004d0055002000430044002d0052004f004d002000000002010c00d041030a00000000010106000003030208000f0000007fff04004eac0881119f594d850ee21a522c59b2";
    const BOOT_USB: &str = "010000001c0055004500460049002000510045004d0055002000510045004d00550020005500530042002000480041005200440044005200490056004500200031002d0030003000300030003a00300030003a00300034002e0030002d003100000002010c00d041030a000000000101060000040305060000007fff04004eac0881119f594d850ee21a522c59b2";

    fn ours() -> Option<(&'static str, &'static str)> {
        Some((MEDIA_LABEL, Box::leak(volume_set_id(ID).into_boxed_str())))
    }

    const CD: &str = "0000:00:03.0/virtio1/host0/target0:0:15/0:0:15:0";
    const COPY: &str = "0000:00:03.0/virtio1/host0/target0:0:0/0:0:0:0";
    const USB: &str = "0000:00:04.0/usb1/1-1/1-1:1.0/host1/target1:0:0/1:0:0:0";

    fn medium(name: &str, binding: MediumBinding) -> MediumDecision {
        MediumDecision::Medium {
            name: name.into(),
            binding,
        }
    }

    #[test]
    fn o_cd_de_onde_o_firmware_arrancou_ganha_a_copia_que_aparece_primeiro() {
        let t = Tree::new("cd");
        // `sda` is an exact copy of the medium on an internal disk of the SAME
        // controller, enumerated first; `sr0` is what the firmware booted.
        t.disk(
            "sda",
            COPY,
            ours(),
            &[("removable", "0"), ("device/type", "0")],
        )
        .disk(
            "sr0",
            CD,
            ours(),
            &[("removable", "1"), ("device/type", "5")],
        )
        .boot_entry(BOOT_CD);
        let s = select(&t.roots(), MEDIA_LABEL, ID);
        assert_eq!(s.origin, BootOrigin::Device { name: "sr0".into() });
        assert_eq!(s.candidates.len(), 2);
        assert_eq!(s.decision, medium("sr0", MediumBinding::FirmwareBootEntry));
    }

    #[test]
    fn a_pen_de_onde_o_firmware_arrancou_ganha_a_copia_interna() {
        let t = Tree::new("usb");
        t.disk(
            "sda",
            COPY,
            ours(),
            &[("removable", "0"), ("device/type", "0")],
        )
        .disk(
            "sdp",
            USB,
            ours(),
            &[("removable", "1"), ("device/type", "0")],
        )
        .boot_entry(BOOT_USB);
        let s = select(&t.roots(), MEDIA_LABEL, ID);
        assert_eq!(s.origin, BootOrigin::Device { name: "sdp".into() });
        assert_eq!(s.decision, medium("sdp", MediumBinding::FirmwareBootEntry));
    }

    #[test]
    fn sem_palavra_do_firmware_a_copia_interna_nunca_e_adoptada() {
        // No EFI variables at all.
        let t = Tree::new("nofw");
        t.disk(
            "sda",
            COPY,
            ours(),
            &[("removable", "0"), ("device/type", "0")],
        );
        let s = select(&t.roots(), MEDIA_LABEL, ID);
        assert_eq!(
            s.origin,
            BootOrigin::Unknown {
                why: OriginUnknown::NoFirmwareVariables
            }
        );
        assert_eq!(s.decision, MediumDecision::Wait);
        // The real medium appears: it is removable, and the only one.
        t.disk(
            "sr0",
            CD,
            ours(),
            &[("removable", "1"), ("device/type", "5")],
        );
        let s = select(&t.roots(), MEDIA_LABEL, ID);
        assert_eq!(
            s.decision,
            medium("sr0", MediumBinding::OnlyRemovableCandidate)
        );
    }

    #[test]
    fn o_firmware_nomeia_um_dispositivo_que_ainda_nao_apareceu() {
        let t = Tree::new("late");
        fs::create_dir_all(t.root.join("sys/devices/pci0000:00/0000:00:04.0")).unwrap();
        t.disk(
            "sda",
            COPY,
            ours(),
            &[("removable", "0"), ("device/type", "0")],
        )
        .boot_entry(BOOT_USB);
        let s = select(&t.roots(), MEDIA_LABEL, ID);
        assert_eq!(s.origin, BootOrigin::NotPresentYet);
        // Wait for it; the copy that is already there is not taken meanwhile.
        assert_eq!(s.decision, MediumDecision::Wait);
    }

    #[test]
    fn falsos_suportes_nao_sao_candidatos() {
        let t = Tree::new("fakes");
        t.disk(
            "sda",
            "0000:00:03.0/virtio1/host0/target0:0:1/0:0:1:0",
            None,
            &[("removable", "0")],
        )
        // The right label on another build's medium.
        .disk(
            "sdb",
            "0000:00:03.0/virtio1/host0/target0:0:2/0:0:2:0",
            Some((MEDIA_LABEL, "OCINYE-MEDIA-ffffffffffffffffffffffffffffffff")),
            &[("removable", "1")],
        )
        // The Phase A medium: the label, no identity.
        .disk(
            "sdc",
            "0000:00:03.0/virtio1/host0/target0:0:3/0:0:3:0",
            Some((MEDIA_LABEL, "")),
            &[("removable", "1")],
        )
        // The identity under another label.
        .disk(
            "sdd",
            "0000:00:03.0/virtio1/host0/target0:0:4/0:0:4:0",
            Some(("UBUNTU", Box::leak(volume_set_id(ID).into_boxed_str()))),
            &[("removable", "1")],
        );
        assert!(candidates(&t.roots(), MEDIA_LABEL, ID).is_empty());
        assert_eq!(
            select(&t.roots(), MEDIA_LABEL, ID).decision,
            MediumDecision::Wait
        );
    }

    #[test]
    fn duas_pens_iguais_sem_firmware_sao_recusadas() {
        let t = Tree::new("two");
        t.disk("sdb", USB, ours(), &[("removable", "1")]).disk(
            "sdc",
            "0000:00:04.0/usb1/1-2/1-2:1.0/host2/target2:0:0/2:0:0:0",
            ours(),
            &[("removable", "1")],
        );
        assert_eq!(
            select(&t.roots(), MEDIA_LABEL, ID).decision,
            MediumDecision::RefuseAmbiguous {
                candidates: vec!["sdb".into(), "sdc".into()]
            }
        );
        // With the firmware's entry (port 0 of the root hub, sysfs "1-1"):
        t.boot_entry(BOOT_USB);
        assert_eq!(
            select(&t.roots(), MEDIA_LABEL, ID).decision,
            medium("sdb", MediumBinding::FirmwareBootEntry)
        );
    }

    #[test]
    fn enderecos_de_hardware() {
        assert!(scsi_address("0:0:15:0", 15, 0));
        assert!(!scsi_address("0:0:1:0", 15, 0));
        assert!(!scsi_address("target0:0:15", 15, 0));
        assert!(usb_port_chain("1-1", &[0]));
        assert!(usb_port_chain("2-1.3", &[0, 2]));
        assert!(!usb_port_chain("1-2", &[0]));
        assert!(!usb_port_chain("1-1:1.0", &[0]));
        assert!(!usb_port_chain("usb1", &[0]));
        assert!(is_pci_function("0000:00:03.0", 3, 0));
        assert!(is_pci_function("0000:02:1f.2", 0x1f, 2));
        assert!(!is_pci_function("0000:00:13.0", 3, 0));
        assert!(!is_pci_function("virtio1", 3, 0));
    }
}
