//! What the OIE knows about this computer: firmware, architecture, memory,
//! and the block devices (from `lsblk --json`), with the medium it booted
//! from marked so that it can never be chosen.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use ocinye_image_contracts::oie::ProbedDisk;
use serde::Deserialize;

/// One `lsblk -J -b -O`-style node (only the columns asked for).
#[derive(Debug, Clone, Deserialize)]
pub struct LsblkNode {
    pub name: String,
    #[serde(default, deserialize_with = "num")]
    pub size: u64,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub serial: Option<String>,
    #[serde(default)]
    pub wwn: Option<String>,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default, deserialize_with = "flag")]
    pub ro: bool,
    #[serde(default)]
    pub fstype: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub mountpoints: Vec<Option<String>>,
    #[serde(default)]
    pub children: Vec<LsblkNode>,
}

#[derive(Debug, Deserialize)]
pub struct Lsblk {
    pub blockdevices: Vec<LsblkNode>,
}

fn num<'de, D: serde::Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
    let v = serde_json::Value::deserialize(d)?;
    Ok(match v {
        serde_json::Value::Number(n) => n.as_u64().unwrap_or(0),
        serde_json::Value::String(s) => s.parse().unwrap_or(0),
        _ => 0,
    })
}

fn flag<'de, D: serde::Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    let v = serde_json::Value::deserialize(d)?;
    Ok(match v {
        serde_json::Value::Bool(b) => b,
        serde_json::Value::String(s) => s == "1",
        serde_json::Value::Number(n) => n.as_u64() == Some(1),
        _ => false,
    })
}

/// The arguments the OIE gives `lsblk`.
pub const LSBLK_ARGS: [&str; 4] = [
    "--json",
    "--bytes",
    "--output",
    "NAME,PATH,SIZE,MODEL,SERIAL,WWN,TYPE,RO,FSTYPE,LABEL,MOUNTPOINTS",
];

fn clean(s: &Option<String>) -> Option<String> {
    s.as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

fn walk_mounted(n: &LsblkNode) -> bool {
    // Any mount point, swap included.
    n.mountpoints.iter().flatten().any(|m| !m.is_empty()) || n.children.iter().any(walk_mounted)
}

fn walk_names(n: &LsblkNode, out: &mut Vec<String>) {
    out.push(n.name.clone());
    for c in &n.children {
        walk_names(c, out);
    }
}

fn walk_labels(n: &LsblkNode, out: &mut Vec<String>) {
    if let Some(l) = clean(&n.label) {
        out.push(l);
    }
    for c in &n.children {
        walk_labels(c, out);
    }
}

fn walk_fs(n: &LsblkNode, out: &mut Vec<String>) {
    if let Some(f) = clean(&n.fstype) {
        out.push(f);
    }
    for c in &n.children {
        walk_fs(c, out);
    }
}

/// Facts the classification needs that lsblk does not give.
#[derive(Debug, Clone, Default)]
pub struct Context {
    /// Kernel names of the devices the boot medium is mounted from
    /// (`findmnt /cdrom`, possibly a partition).
    pub boot_sources: Vec<String>,
    /// The ISO volume label (protects the medium even if it is not mounted).
    pub media_label: Option<String>,
    /// Stable paths by kernel name (`/dev/disk/by-path`, else `by-id`, else `by-diskseq`).
    pub stable_paths: BTreeMap<String, String>,
    /// The block guard is armed: the kernel's read-only flag is set on every
    /// disk and says nothing about the disk itself.
    pub guard_armed: bool,
    /// Disks that were read-only before the guard touched them (as it recorded).
    pub hardware_read_only: Vec<String>,
}

/// Disks the OIE considers: whole disks and optical drives, never loop, zram
/// or partitions on their own.
pub fn disks(ls: &Lsblk, ctx: &Context) -> Vec<ProbedDisk> {
    ls.blockdevices
        .iter()
        .filter(|n| matches!(n.kind.as_str(), "disk" | "rom"))
        .filter(|n| {
            !n.name.starts_with("zram") && !n.name.starts_with("loop") && !n.name.starts_with("ram")
        })
        .map(|n| {
            let mut names = vec![];
            walk_names(n, &mut names);
            let mut labels = vec![];
            walk_labels(n, &mut labels);
            let mut filesystems = vec![];
            walk_fs(n, &mut filesystems);
            filesystems.sort();
            filesystems.dedup();
            let boot_media = names.iter().any(|x| ctx.boot_sources.contains(x))
                || ctx.media_label.as_ref().is_some_and(|l| labels.contains(l));
            ProbedDisk {
                device: n.name.clone(),
                by_path: ctx.stable_paths.get(&n.name).cloned().unwrap_or_default(),
                bytes: n.size,
                model: clean(&n.model).unwrap_or_default(),
                serial: clean(&n.serial),
                wwn: clean(&n.wwn),
                partitions: u32::try_from(n.children.iter().filter(|c| c.kind == "part").count())
                    .unwrap_or(u32::MAX),
                filesystems,
                boot_media,
                mounted: walk_mounted(n),
                // An optical drive is never a target, whatever it holds.
                read_only: if ctx.guard_armed {
                    ctx.hardware_read_only.contains(&n.name)
                } else {
                    n.ro
                } || n.kind == "rom"
                    || !ctx.stable_paths.contains_key(&n.name),
            }
        })
        .collect()
}

/// Stable path links from `/dev/disk/by-path`, then `by-id`, then `by-diskseq`.
pub fn stable_paths(dev: &Path) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for sub in ["by-diskseq", "by-id", "by-path"] {
        let dir = dev.join("disk").join(sub);
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut links: Vec<_> = entries.flatten().collect();
        links.sort_by_key(std::fs::DirEntry::file_name);
        for e in links {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.contains("-part") {
                continue;
            }
            if let Ok(target) = std::fs::read_link(e.path()) {
                if let Some(k) = target.file_name().map(|k| k.to_string_lossy().into_owned()) {
                    // Later (more stable) kinds overwrite earlier ones.
                    out.insert(k, dir.join(&name).to_string_lossy().into_owned());
                }
            }
        }
    }
    out
}

/// `UEFI`, required (legacy BIOS is `NOT_SUPPORTED_V1`).
pub fn is_uefi(sys: &Path) -> bool {
    sys.join("firmware/efi").exists()
}

/// `MemTotal` in bytes.
pub fn mem_total(meminfo: &str) -> u64 {
    meminfo
        .lines()
        .find_map(|l| l.strip_prefix("MemTotal:"))
        .and_then(|v| v.split_whitespace().next())
        .and_then(|k| k.parse::<u64>().ok())
        .map_or(0, |k| k * 1024)
}

/// The processor model, from `/proc/cpuinfo`. ARM kernels often do not say.
pub fn cpu_model(cpuinfo: &str) -> Option<String> {
    cpuinfo
        .lines()
        .find_map(|l| {
            let (k, v) = l.split_once(':')?;
            matches!(k.trim(), "model name" | "Model" | "Processor" | "cpu model")
                .then(|| v.trim().to_owned())
        })
        .filter(|v| !v.is_empty())
}

fn read_trim(path: &Path) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty() && s != "To Be Filled By O.E.M." && s != "Default string")
}

/// Maker and model, from the firmware tables (`/sys/class/dmi/id`).
pub fn machine(sys: &Path) -> Option<String> {
    let id = sys.join("class/dmi/id");
    let parts: Vec<String> = ["sys_vendor", "product_name"]
        .iter()
        .filter_map(|f| read_trim(&id.join(f)))
        .collect();
    (!parts.is_empty()).then(|| parts.join(" "))
}

/// Firmware vendor and version.
pub fn firmware(sys: &Path) -> Option<String> {
    let id = sys.join("class/dmi/id");
    let parts: Vec<String> = ["bios_vendor", "bios_version"]
        .iter()
        .filter_map(|f| read_trim(&id.join(f)))
        .collect();
    (!parts.is_empty()).then(|| parts.join(" "))
}

/// Whether the firmware enforces Secure Boot: the `SecureBoot` variable is
/// four bytes of attributes and one of value. Unknown when it cannot be read.
pub fn secure_boot(sys: &Path) -> Option<bool> {
    let var = sys.join("firmware/efi/efivars/SecureBoot-8be4df61-93ca-11d2-aa0d-e98e8a39a0f6");
    let bytes = fs::read(var).ok()?;
    bytes.get(4).map(|b| *b == 1)
}

/// Entries of a class directory, or unknown when the class is not there (the
/// driver that would create it is not loaded: that is not "none").
fn class_count(dir: &Path) -> Option<u32> {
    let entries = fs::read_dir(dir).ok()?;
    Some(u32::try_from(entries.flatten().count()).unwrap_or(u32::MAX))
}

/// Wireless radios (`/sys/class/ieee80211`).
pub fn wifi_radios(sys: &Path) -> Option<u32> {
    class_count(&sys.join("class/ieee80211"))
}

/// Bluetooth controllers (`/sys/class/bluetooth`).
pub fn bluetooth_controllers(sys: &Path) -> Option<u32> {
    class_count(&sys.join("class/bluetooth"))
}

/// Connected display outputs (`/sys/class/drm/card*-*/status`), named as the
/// connector without its card (`HDMI-A-1`, `Virtual-1`).
pub fn displays(sys: &Path) -> Option<Vec<String>> {
    let entries = fs::read_dir(sys.join("class/drm")).ok()?;
    let mut out: Vec<String> = entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let (card, connector) = name.split_once('-')?;
            if !card.starts_with("card") {
                return None;
            }
            (read_trim(&e.path().join("status")).as_deref() == Some("connected"))
                .then(|| connector.to_owned())
        })
        .collect();
    out.sort();
    Some(out)
}

/// Sound cards, from `/proc/asound/cards`. Unknown without the file.
pub fn audio(cards: Option<&str>) -> Option<Vec<String>> {
    let text = cards?;
    Some(
        text.lines()
            .filter_map(|l| {
                let (head, name) = l.split_once(" - ")?;
                head.split_whitespace()
                    .next()?
                    .parse::<u32>()
                    .ok()
                    .map(|_| name.trim().to_owned())
            })
            .collect(),
    )
}

/// Addresses per interface, from `ip -brief address` (`name state addr…`).
pub fn addresses(brief: &str) -> BTreeMap<String, Vec<String>> {
    brief
        .lines()
        .filter_map(|l| {
            let mut f = l.split_whitespace();
            let name = f.next()?.split('@').next()?.to_owned();
            let _state = f.next()?;
            Some((name, f.map(str::to_owned).collect()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ocinye_image_contracts::oie::{classify, DiskProtection};

    #[test]
    fn o_que_a_sessao_le_e_o_que_nao_sabe() {
        let x86 = "processor\t: 0\nvendor_id\t: GenuineIntel\nmodel name\t: Intel(R) Core(TM) i5-8350U CPU @ 1.70GHz\n";
        assert_eq!(
            cpu_model(x86).as_deref(),
            Some("Intel(R) Core(TM) i5-8350U CPU @ 1.70GHz")
        );
        assert_eq!(cpu_model("processor\t: 0\nBogoMIPS\t: 48.00\n"), None);

        let root = std::env::temp_dir().join(format!("oie-hw-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        // Nothing there: every fact is unknown, never "none".
        assert_eq!(wifi_radios(&root), None);
        assert_eq!(bluetooth_controllers(&root), None);
        assert_eq!(displays(&root), None);
        assert_eq!(secure_boot(&root), None);
        assert_eq!(machine(&root), None);

        fs::create_dir_all(root.join("class/ieee80211")).unwrap();
        assert_eq!(wifi_radios(&root), Some(0));
        fs::create_dir_all(root.join("class/ieee80211/phy0")).unwrap();
        assert_eq!(wifi_radios(&root), Some(1));
        for (c, st) in [
            ("card0-HDMI-A-1", "connected"),
            ("card0-DP-1", "disconnected"),
            ("card0-eDP-1", "connected"),
        ] {
            fs::create_dir_all(root.join("class/drm").join(c)).unwrap();
            fs::write(
                root.join("class/drm").join(c).join("status"),
                format!("{st}\n"),
            )
            .unwrap();
        }
        fs::create_dir_all(root.join("class/drm/card0")).unwrap();
        assert_eq!(
            displays(&root),
            Some(vec!["HDMI-A-1".to_owned(), "eDP-1".to_owned()])
        );
        let id = root.join("class/dmi/id");
        fs::create_dir_all(&id).unwrap();
        fs::write(id.join("sys_vendor"), "LENOVO\n").unwrap();
        fs::write(id.join("product_name"), "20L8S02E00\n").unwrap();
        fs::write(id.join("bios_vendor"), "To Be Filled By O.E.M.\n").unwrap();
        assert_eq!(machine(&root).as_deref(), Some("LENOVO 20L8S02E00"));
        assert_eq!(firmware(&root), None);
        let vars = root.join("firmware/efi/efivars");
        fs::create_dir_all(&vars).unwrap();
        let sb = vars.join("SecureBoot-8be4df61-93ca-11d2-aa0d-e98e8a39a0f6");
        fs::write(&sb, [6, 0, 0, 0, 1]).unwrap();
        assert_eq!(secure_boot(&root), Some(true));
        fs::write(&sb, [6, 0, 0, 0, 0]).unwrap();
        assert_eq!(secure_boot(&root), Some(false));
        let _ = fs::remove_dir_all(&root);

        assert_eq!(audio(None), None);
        assert_eq!(audio(Some("--- no soundcards ---\n")), Some(vec![]));
        let cards = " 0 [PCH            ]: HDA-Intel - HDA Intel PCH\n                      HDA Intel PCH at 0xf1340000 irq 135\n";
        assert_eq!(audio(Some(cards)), Some(vec!["HDA Intel PCH".to_owned()]));

        let a = addresses("enp1s0           UP             10.0.2.15/24 fec0::5054:ff:fe12:3456/64\nwlan0            DOWN           \n");
        assert_eq!(
            a["enp1s0"],
            vec!["10.0.2.15/24", "fec0::5054:ff:fe12:3456/64"]
        );
        assert!(a["wlan0"].is_empty());
    }

    /// Three disks as QEMU shows them for the ISO-3 gate: the ISO on a virtual
    /// CD, a mounted data disk, a blank target, and two identical serial-less disks.
    const LSBLK: &str = r#"{"blockdevices":[
      {"name":"sr0","path":"/dev/sr0","size":4200000000,"model":"QEMU DVD-ROM","serial":"QM00003","wwn":null,"type":"rom","ro":false,"fstype":"iso9660","label":"OCINYE_OS","mountpoints":["/cdrom"]},
      {"name":"vda","path":"/dev/vda","size":42949672960,"model":null,"serial":"OCY-TARGET-7F3A","wwn":null,"type":"disk","ro":false,"fstype":null,"label":null,"mountpoints":[null]},
      {"name":"vdb","path":"/dev/vdb","size":42949672960,"model":null,"serial":"DATA0001","wwn":null,"type":"disk","ro":false,"fstype":null,"label":null,"mountpoints":[null],
        "children":[{"name":"vdb1","path":"/dev/vdb1","size":42948624384,"type":"part","ro":false,"fstype":"ext4","label":"data","mountpoints":["/mnt/data"]}]},
      {"name":"sda","path":"/dev/sda","size":64424509440,"model":"QEMU HARDDISK","serial":null,"wwn":null,"type":"disk","ro":"0","fstype":null,"label":null,"mountpoints":[null]},
      {"name":"sdb","path":"/dev/sdb","size":64424509440,"model":"QEMU HARDDISK","serial":null,"wwn":null,"type":"disk","ro":"0","fstype":null,"label":null,"mountpoints":[null]},
      {"name":"loop0","path":"/dev/loop0","size":1000,"type":"loop","ro":true,"mountpoints":["/rofs"]}
    ]}"#;

    fn ctx() -> Context {
        let mut stable = BTreeMap::new();
        for (k, p) in [
            ("sr0", "pci-0000:00:1f.2-ata-3"),
            ("vda", "pci-0000:00:04.0"),
            ("vdb", "pci-0000:00:05.0"),
            ("sda", "pci-0000:00:06.0-scsi-0:0:0:0"),
            ("sdb", "pci-0000:00:06.0-scsi-0:0:1:0"),
        ] {
            stable.insert(k.into(), format!("/dev/disk/by-path/{p}"));
        }
        Context {
            boot_sources: vec!["sr0".into()],
            media_label: Some("OCINYE_OS".into()),
            stable_paths: stable,
            guard_armed: false,
            hardware_read_only: vec![],
        }
    }

    #[test]
    fn suporte_montados_e_ambiguos_classificam_se_como_o_contrato_manda() {
        let ls: Lsblk = serde_json::from_str(LSBLK).unwrap();
        let d = disks(&ls, &ctx());
        assert_eq!(d.len(), 5, "loop devices are never offered");
        let c = classify(&d);
        let p = |n: &str| c.iter().find(|x| x.device == n).unwrap().protection;
        assert_eq!(p("sr0"), DiskProtection::InstallationMedia);
        assert_eq!(p("vda"), DiskProtection::Eligible);
        assert_eq!(p("vdb"), DiskProtection::Mounted);
        assert_eq!(p("sda"), DiskProtection::Ambiguous);
        assert_eq!(p("sdb"), DiskProtection::Ambiguous);
        assert_eq!(
            c.iter()
                .find(|x| x.device == "vda")
                .unwrap()
                .confirmation_token,
            "7F3A"
        );
        assert_eq!(
            d.iter().find(|x| x.device == "vdb").unwrap().filesystems,
            vec!["ext4"]
        );
    }

    #[test]
    fn suporte_identificado_pela_etiqueta_mesmo_desmontado() {
        // A USB stick with the ISO written to it, not mounted (toram): the label protects it.
        let ls: Lsblk = serde_json::from_str(r#"{"blockdevices":[
          {"name":"sdc","size":31000000000,"model":"USB DISK","serial":"USB123456","type":"disk","ro":false,"mountpoints":[null],
           "children":[{"name":"sdc1","size":4200000000,"type":"part","ro":false,"fstype":"iso9660","label":"OCINYE_OS","mountpoints":[null]},
                       {"name":"sdc2","size":5000000,"type":"part","ro":false,"fstype":"vfat","label":"ESP","mountpoints":[null]}]}]}"#).unwrap();
        let mut c = ctx();
        c.boot_sources.clear();
        c.stable_paths
            .insert("sdc".into(), "/dev/disk/by-path/usb-1".into());
        let d = disks(&ls, &c);
        assert!(d[0].boot_media);
        assert_eq!(
            classify(&d)[0].protection,
            DiskProtection::InstallationMedia
        );
    }

    #[test]
    fn sem_caminho_estavel_nao_se_escolhe() {
        let ls: Lsblk = serde_json::from_str(r#"{"blockdevices":[{"name":"vdz","size":42949672960,"serial":"Z1234","type":"disk","ro":false,"mountpoints":[null]}]}"#).unwrap();
        let d = disks(&ls, &Context::default());
        assert!(!classify(&d)[0].protection.selectable());
    }

    #[test]
    fn memoria() {
        assert_eq!(
            mem_total("MemTotal:        4028232 kB\nMemFree: 1 kB\n"),
            4_028_232 * 1024
        );
        assert_eq!(mem_total(""), 0);
    }

    #[test]
    fn sob_a_guarda_so_o_que_ja_era_so_de_leitura_fica_protegido_como_tal() {
        // With the guard armed every disk reports ro=1. A blank target must
        // stay selectable; a disk that was write-protected before the guard
        // touched it must not.
        let ls: Lsblk = serde_json::from_str(
            r#"{"blockdevices":[
              {"name":"vda","size":42949672960,"serial":"OCY-TARGET-7F3A","type":"disk","ro":true,"mountpoints":[null]},
              {"name":"vdb","size":42949672960,"serial":"LOCKED-0001","type":"disk","ro":true,"mountpoints":[null]}]}"#,
        )
        .unwrap();
        let mut c = Context {
            guard_armed: true,
            hardware_read_only: vec!["vdb".into()],
            ..Context::default()
        };
        c.stable_paths.insert("vda".into(), "p-a".into());
        c.stable_paths.insert("vdb".into(), "p-b".into());
        let d = disks(&ls, &c);
        assert!(!d[0].read_only);
        assert!(d[1].read_only);
        // Without the guard the kernel's flag is the disk's own.
        c.guard_armed = false;
        c.hardware_read_only.clear();
        let d = disks(&ls, &c);
        assert!(d[0].read_only && d[1].read_only);
    }
}
