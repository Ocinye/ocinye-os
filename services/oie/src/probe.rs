//! What the OIE knows about this computer: firmware, architecture, memory,
//! and the block devices (from `lsblk --json`), with the medium it booted
//! from marked so that it can never be chosen.

use std::collections::BTreeMap;
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
                read_only: n.ro || n.kind == "rom" || !ctx.stable_paths.contains_key(&n.name),
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

#[cfg(test)]
mod tests {
    use super::*;
    use ocinye_image_contracts::oie::{classify, DiskProtection};

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
}
