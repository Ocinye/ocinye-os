//! Hardware discovery (read-only). D011_HARDWARE_CAPABILITY_MATRIX.
//!
//! Everything is read from `/proc` and `/sys` under a [`Root`] — `/` on the
//! server, a fixture directory in tests — so the parsers are exercised with
//! realistic files, multi-GPU included, without pretending a test machine has
//! hardware it does not have. Nothing here is sent anywhere but back to the
//! Installer; nothing enrols anything.

use std::fs;
use std::path::PathBuf;

use ocinye_installer_contracts::hardware::{
    ComputeSecurityCapability, ConfidentialTech, CpuCapabilities, CpuFeature, DriverInfo, Evidence,
    GpuCapabilities, GpuDiscovery, GpuVendor, HardwareCapabilities, MemoryCapabilities,
    NetworkCapabilities, RuntimeKind, RuntimeStatus, StorageCapabilities,
};
use ocinye_installer_contracts::manifest::Arch;

use crate::exec;

/// Where `/proc` and `/sys` are.
#[derive(Debug, Clone)]
pub struct Root(pub PathBuf);

impl Root {
    /// The real system.
    #[must_use]
    pub fn system() -> Self {
        Self(PathBuf::from("/"))
    }

    fn path(&self, p: &str) -> PathBuf {
        self.0.join(p.trim_start_matches('/'))
    }

    fn read(&self, p: &str) -> Option<String> {
        fs::read_to_string(self.path(p)).ok()
    }

    fn exists(&self, p: &str) -> bool {
        self.path(p).exists()
    }
}

/// `0-3,5,7-8` → 7.
#[must_use]
pub fn count_cpu_list(list: &str) -> u32 {
    list.trim()
        .split(',')
        .filter(|p| !p.is_empty())
        .map(|part| match part.split_once('-') {
            Some((a, b)) => match (a.parse::<u32>(), b.parse::<u32>()) {
                (Ok(a), Ok(b)) if b >= a => b - a + 1,
                _ => 0,
            },
            None => u32::from(part.parse::<u32>().is_ok()),
        })
        .sum()
}

/// `MemTotal: 4011232 kB` → bytes.
#[must_use]
pub fn meminfo_bytes(meminfo: &str, key: &str) -> Option<u64> {
    meminfo.lines().find_map(|l| {
        let (k, v) = l.split_once(':')?;
        (k.trim() == key)
            .then(|| v.trim().trim_end_matches("kB").trim().parse::<u64>().ok())
            .flatten()
            .map(|kb| kb * 1024)
    })
}

fn cpuinfo_field<'a>(cpuinfo: &'a str, key: &str) -> Option<&'a str> {
    cpuinfo.lines().find_map(|l| {
        let (k, v) = l.split_once(':')?;
        (k.trim() == key).then(|| v.trim())
    })
}

/// The CPU, from `/proc/cpuinfo` and the online list.
#[must_use]
pub fn parse_cpu(cpuinfo: &str, online: &str, machine: &str) -> CpuCapabilities {
    let model = cpuinfo_field(cpuinfo, "model name")
        .map(str::to_owned)
        .or_else(|| {
            let implementer = cpuinfo_field(cpuinfo, "CPU implementer")?;
            let part = cpuinfo_field(cpuinfo, "CPU part")?;
            Some(format!("Arm (implementer {implementer}, part {part})"))
        });
    let flags: Vec<&str> = cpuinfo_field(cpuinfo, "flags")
        .or_else(|| cpuinfo_field(cpuinfo, "Features"))
        .map(|f| f.split_whitespace().collect())
        .unwrap_or_default();
    let mut features = Vec::new();
    for (flag, feature) in [
        ("avx2", CpuFeature::Avx2),
        ("avx512f", CpuFeature::Avx512),
        ("sev_snp", CpuFeature::SevSnp),
        ("tdx_guest", CpuFeature::Tdx),
        ("sve", CpuFeature::Sve),
    ] {
        if flags.contains(&flag) && !features.contains(&feature) {
            features.push(feature);
        }
    }
    // Physical cores: distinct (physical id, core id) pairs, when reported.
    let mut pairs = std::collections::BTreeSet::new();
    let mut phys = None;
    for line in cpuinfo.lines() {
        if let Some((k, v)) = line.split_once(':') {
            match k.trim() {
                "physical id" => phys = Some(v.trim().to_owned()),
                "core id" => {
                    pairs.insert((phys.clone(), v.trim().to_owned()));
                }
                _ => {}
            }
        }
    }
    CpuCapabilities {
        model,
        arch: Arch::from_uname(machine),
        cores: (!pairs.is_empty()).then(|| u32::try_from(pairs.len()).unwrap_or(u32::MAX)),
        threads: count_cpu_list(online),
        features,
    }
}

/// PCI classes that are accelerators: VGA, 3D controller, other display.
fn is_display_class(class: &str) -> bool {
    let c = class.trim().trim_start_matches("0x");
    c.starts_with("0300") || c.starts_with("0302") || c.starts_with("0380")
}

/// Vendors whose display functions are basic adapters, not accelerators
/// (BMC graphics, virtual consoles).
const BASIC_DISPLAY_VENDORS: [(u16, &str); 4] = [
    (0x1a03, "ASPEED"),
    (0x102b, "Matrox"),
    (0x1af4, "virtio-gpu"),
    (0x1234, "QEMU stdvga"),
];

fn hex_u16(s: &str) -> Option<u16> {
    u16::from_str_radix(s.trim().trim_start_matches("0x"), 16).ok()
}

/// One `nvidia-smi --query-gpu=pci.bus_id,name,memory.total,compute_cap
/// --format=csv,noheader,nounits` line.
#[must_use]
pub fn parse_nvidia_smi(line: &str) -> Option<(String, String, Option<u64>, Option<String>)> {
    let parts: Vec<&str> = line.split(',').map(str::trim).collect();
    let [bus, name, mem, cap] = parts.as_slice() else {
        return None;
    };
    // nvidia-smi writes `00000000:01:00.0`; sysfs uses `0000:01:00.0`.
    let bus = bus.to_lowercase();
    let bus = bus
        .rsplit_once(':')
        .and_then(|(head, func)| {
            let bus_dev = head.rsplit(':').next()?;
            Some(format!("0000:{bus_dev}:{func}"))
        })
        .unwrap_or(bus);
    Some((
        bus,
        (*name).to_owned(),
        mem.parse::<u64>().ok().map(|mib| mib * 1024 * 1024),
        Some((*cap).to_owned()).filter(|c| !c.is_empty() && *c != "[N/A]"),
    ))
}

/// Accelerators from `/sys/bus/pci/devices`.
#[must_use]
pub fn discover_gpus(root: &Root, nvidia_smi: &[String], docker_runtimes: &str) -> GpuDiscovery {
    let dir = root.path("/sys/bus/pci/devices");
    let entries = match fs::read_dir(&dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // No PCI bus exposed (some VMs, containers): not an error, and not
            // proof of absence either — but there is nothing to read.
            return GpuDiscovery::NoGpu {
                basic_display: None,
            };
        }
        Err(e) => {
            return GpuDiscovery::GpuDiscoveryError {
                code: if e.kind() == std::io::ErrorKind::PermissionDenied {
                    "EACCES".into()
                } else {
                    "SYSFS_UNREADABLE".into()
                },
            };
        }
    };
    let mut addresses: Vec<String> = entries
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    addresses.sort();
    let nvidia: Vec<_> = nvidia_smi
        .iter()
        .filter_map(|l| parse_nvidia_smi(l))
        .collect();
    let mut gpus = Vec::new();
    let mut basic = None;
    for addr in addresses {
        let base = format!("/sys/bus/pci/devices/{addr}");
        let Some(class) = root.read(&format!("{base}/class")) else {
            return GpuDiscovery::GpuDiscoveryError {
                code: "SYSFS_UNREADABLE".into(),
            };
        };
        if !is_display_class(&class) {
            continue;
        }
        let vendor_id = root
            .read(&format!("{base}/vendor"))
            .and_then(|v| hex_u16(&v))
            .unwrap_or(0);
        let device_id = root
            .read(&format!("{base}/device"))
            .and_then(|v| hex_u16(&v))
            .unwrap_or(0);
        if let Some((_, name)) = BASIC_DISPLAY_VENDORS.iter().find(|(v, _)| *v == vendor_id) {
            basic = Some((*name).to_owned());
            continue;
        }
        let vendor = GpuVendor::from_pci(vendor_id);
        let module = fs::read_link(root.path(&format!("{base}/driver")))
            .ok()
            .and_then(|l| l.file_name().map(|n| n.to_string_lossy().into_owned()));
        let driver = module.as_ref().map(|m| DriverInfo {
            module: m.clone(),
            version: root
                .read(&format!("/sys/module/{m}/version"))
                .map(|v| v.trim().to_owned()),
        });
        let nv = nvidia.iter().find(|(bus, ..)| *bus == addr);
        let (model, vram, cap) = match (vendor, nv) {
            (GpuVendor::Nvidia, Some((_, name, mem, cap))) => {
                (Some(name.clone()), *mem, cap.clone())
            }
            (GpuVendor::Amd, _) => (
                root.read(&format!("{base}/product_name"))
                    .map(|s| s.trim().to_owned())
                    .filter(|s| !s.is_empty()),
                root.read(&format!("{base}/mem_info_vram_total"))
                    .and_then(|s| s.trim().parse().ok()),
                None,
            ),
            _ => (None, None, None),
        };
        let runtime = match vendor {
            GpuVendor::Nvidia => Some((
                RuntimeKind::NvidiaContainerToolkit,
                if docker_runtimes.contains("nvidia") {
                    RuntimeStatus::Detected
                } else {
                    RuntimeStatus::Unavailable
                },
            )),
            GpuVendor::Amd => Some((
                RuntimeKind::Rocm,
                if root.exists("/dev/kfd") {
                    RuntimeStatus::Detected
                } else {
                    RuntimeStatus::Unavailable
                },
            )),
            GpuVendor::Intel => Some((
                RuntimeKind::IntelOneApi,
                if root.exists("/usr/lib/x86_64-linux-gnu/libze_loader.so.1") {
                    RuntimeStatus::Detected
                } else {
                    RuntimeStatus::Unavailable
                },
            )),
            GpuVendor::Other(_) => None,
        };
        gpus.push(GpuCapabilities {
            index: u8::try_from(gpus.len()).unwrap_or(u8::MAX),
            pci_address: addr,
            vendor,
            device_id,
            model,
            vram_bytes: vram,
            driver,
            compute_capability: cap,
            runtime,
        });
    }
    if gpus.is_empty() {
        GpuDiscovery::NoGpu {
            basic_display: basic,
        }
    } else if gpus
        .iter()
        .any(|g| matches!(g.runtime, Some((_, RuntimeStatus::Unavailable))))
    {
        GpuDiscovery::GpuRuntimeUnavailable { gpus }
    } else {
        GpuDiscovery::GpuDetected { gpus }
    }
}

fn network(root: &Root) -> NetworkCapabilities {
    let mut up = 0u8;
    let mut max = None;
    if let Ok(entries) = fs::read_dir(root.path("/sys/class/net")) {
        for e in entries.filter_map(Result::ok) {
            let name = e.file_name().to_string_lossy().into_owned();
            if name == "lo" {
                continue;
            }
            if root
                .read(&format!("/sys/class/net/{name}/operstate"))
                .is_some_and(|s| s.trim() == "up")
            {
                up = up.saturating_add(1);
                if let Some(speed) = root
                    .read(&format!("/sys/class/net/{name}/speed"))
                    .and_then(|s| s.trim().parse::<i64>().ok())
                    .filter(|s| *s > 0)
                {
                    let s = u32::try_from(speed).unwrap_or(u32::MAX);
                    max = Some(max.map_or(s, |m: u32| m.max(s)));
                }
            }
        }
    }
    // /proc/net/if_inet6: scope `00` is global.
    let ipv6 = root
        .read("/proc/net/if_inet6")
        .is_some_and(|t| t.lines().any(|l| l.split_whitespace().nth(3) == Some("00")));
    NetworkCapabilities {
        interfaces_up: up,
        max_link_mbps: max,
        ipv6,
    }
}

/// `df -P -k <path>` → (total, free) bytes.
#[must_use]
pub fn parse_df(out: &str) -> Option<(u64, u64)> {
    let line = out.lines().nth(1)?;
    let cols: Vec<&str> = line.split_whitespace().collect();
    let total = cols.get(1)?.parse::<u64>().ok()? * 1024;
    let free = cols.get(3)?.parse::<u64>().ok()? * 1024;
    Some((total, free))
}

/// The filesystem type of the mount that holds `path`.
#[must_use]
pub fn fs_type(mounts: &str, path: &str) -> Option<String> {
    mounts
        .lines()
        .filter_map(|l| {
            let mut it = l.split_whitespace();
            let _dev = it.next()?;
            let mnt = it.next()?;
            let ty = it.next()?;
            let covers = path == mnt
                || mnt == "/"
                || path.starts_with(&format!("{}/", mnt.trim_end_matches('/')));
            covers.then(|| (mnt.len(), ty.to_owned()))
        })
        .max_by_key(|(len, _)| *len)
        .map(|(_, ty)| ty)
}

/// Where Ocinye will live: `/srv` if it exists, else `/`.
#[must_use]
pub fn storage_path(root: &Root) -> &'static str {
    if root.exists("/srv") {
        "/srv"
    } else {
        "/"
    }
}

/// Discover everything. `machine` is `uname -m`.
#[must_use]
pub fn discover(root: &Root, machine: &str) -> HardwareCapabilities {
    let cpuinfo = root.read("/proc/cpuinfo").unwrap_or_default();
    let online = root
        .read("/sys/devices/system/cpu/online")
        .unwrap_or_default();
    let meminfo = root.read("/proc/meminfo").unwrap_or_default();
    let path = storage_path(root);
    let df = exec::run("/usr/bin/df", &["-P", "-k", path], exec::secs(10));
    let (total, free) = parse_df(&df.stdout).unwrap_or((0, 0));
    let mounts = root.read("/proc/mounts").unwrap_or_default();
    let nvidia_smi: Vec<String> = if exec::exists("/usr/bin/nvidia-smi") {
        exec::run(
            "/usr/bin/nvidia-smi",
            &[
                "--query-gpu=pci.bus_id,name,memory.total,compute_cap",
                "--format=csv,noheader,nounits",
            ],
            exec::secs(15),
        )
        .stdout
        .lines()
        .map(str::to_owned)
        .collect()
    } else {
        Vec::new()
    };
    let runtimes = if exec::exists(exec::program::DOCKER) {
        exec::run(
            exec::program::DOCKER,
            &["info", "--format", "{{json .Runtimes}}"],
            exec::secs(15),
        )
        .stdout
    } else {
        String::new()
    };
    let cpu = parse_cpu(&cpuinfo, &online, machine);
    let snp_enabled = root
        .read("/sys/module/kvm_amd/parameters/sev_snp")
        .is_some_and(|v| v.trim() == "Y");
    let mut confidential = Vec::new();
    if cpu.features.contains(&CpuFeature::SevSnp) || snp_enabled {
        confidential.push((ConfidentialTech::AmdSevSnp, Evidence::Detected));
    }
    if cpu.features.contains(&CpuFeature::Tdx) {
        confidential.push((ConfidentialTech::IntelTdx, Evidence::Detected));
    }
    HardwareCapabilities {
        cpu,
        memory: MemoryCapabilities {
            total_bytes: meminfo_bytes(&meminfo, "MemTotal").unwrap_or(0),
            available_bytes: meminfo_bytes(&meminfo, "MemAvailable").unwrap_or(0),
            swap_bytes: meminfo_bytes(&meminfo, "SwapTotal").unwrap_or(0),
        },
        storage: StorageCapabilities {
            path: path.to_owned(),
            free_bytes: free,
            total_bytes: total,
            filesystem: fs_type(&mounts, path),
        },
        network: network(root),
        gpu: discover_gpus(root, &nvidia_smi, &runtimes),
        security: ComputeSecurityCapability {
            confidential,
            tpm: if root.exists("/dev/tpmrm0") {
                Evidence::Detected
            } else {
                Evidence::NotDetected
            },
        },
    }
}

/// `/proc`, `/sys` and the PCI tree are readable (PF-HWREAD).
#[must_use]
pub fn readable(root: &Root) -> bool {
    root.read("/proc/cpuinfo").is_some()
        && root.read("/proc/meminfo").is_some()
        && match fs::read_dir(root.path("/sys/bus/pci/devices")) {
            Ok(_) => true,
            Err(e) => e.kind() == std::io::ErrorKind::NotFound,
        }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    struct Fixture(PathBuf);

    impl Fixture {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("ocinye-hw-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
        fn file(&self, p: &str, content: &str) -> &Self {
            let path = self.0.join(p.trim_start_matches('/'));
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, content).unwrap();
            self
        }
        fn pci(
            &self,
            addr: &str,
            class: &str,
            vendor: &str,
            device: &str,
            driver: Option<&str>,
        ) -> &Self {
            let base = format!("/sys/bus/pci/devices/{addr}");
            self.file(&format!("{base}/class"), class);
            self.file(&format!("{base}/vendor"), vendor);
            self.file(&format!("{base}/device"), device);
            if let Some(d) = driver {
                let target = self.0.join(format!("sys/bus/pci/drivers/{d}"));
                fs::create_dir_all(&target).unwrap();
                symlink(
                    &target,
                    self.0.join(format!("sys/bus/pci/devices/{addr}/driver")),
                )
                .unwrap();
            }
            self
        }
        fn root(&self) -> Root {
            Root(self.0.clone())
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn as_listas_de_cpu_contam_se() {
        assert_eq!(count_cpu_list("0-1\n"), 2);
        assert_eq!(count_cpu_list("0-3,5,7-8"), 7);
        assert_eq!(count_cpu_list(""), 0);
    }

    #[test]
    fn o_processador_le_se_em_x86_e_em_arm() {
        let x86 = "processor\t: 0\nmodel name\t: AMD EPYC 7443P\nflags\t\t: fpu avx2 sev_snp\nphysical id\t: 0\ncore id\t\t: 0\n\nprocessor\t: 1\nphysical id\t: 0\ncore id\t\t: 1\n";
        let c = parse_cpu(x86, "0-1", "x86_64");
        assert_eq!(c.model.as_deref(), Some("AMD EPYC 7443P"));
        assert_eq!(c.arch, Some(Arch::Amd64));
        assert_eq!(c.threads, 2);
        assert_eq!(c.cores, Some(2));
        assert_eq!(c.features, [CpuFeature::Avx2, CpuFeature::SevSnp]);
        let arm =
            "processor\t: 0\nFeatures\t: fp asimd\nCPU implementer\t: 0x61\nCPU part\t: 0x000\n";
        let c = parse_cpu(arm, "0-3", "aarch64");
        assert_eq!(c.arch, Some(Arch::Arm64));
        assert_eq!(c.threads, 4);
        assert!(c.model.unwrap().starts_with("Arm (implementer 0x61"));
        assert_eq!(parse_cpu("", "0", "riscv64").arch, None);
    }

    #[test]
    fn a_memoria_vem_do_meminfo() {
        let m = "MemTotal:        4011232 kB\nMemAvailable:    3500000 kB\nSwapTotal:             0 kB\n";
        assert_eq!(meminfo_bytes(m, "MemTotal"), Some(4_011_232 * 1024));
        assert_eq!(meminfo_bytes(m, "SwapTotal"), Some(0));
        assert_eq!(meminfo_bytes(m, "Nada"), None);
    }

    #[test]
    fn o_disco_e_o_sistema_de_ficheiros() {
        let df = "Filesystem     1024-blocks    Used Available Capacity Mounted on\n/dev/vda1         40000000 3000000  37000000       8% /\n";
        assert_eq!(parse_df(df), Some((40_000_000 * 1024, 37_000_000 * 1024)));
        let mounts =
            "/dev/vda1 / ext4 rw 0 0\n/dev/vdb1 /srv xfs rw 0 0\ntmpfs /run tmpfs rw 0 0\n";
        assert_eq!(fs_type(mounts, "/srv").as_deref(), Some("xfs"));
        assert_eq!(fs_type(mounts, "/").as_deref(), Some("ext4"));
    }

    #[test]
    fn sem_gpu_e_um_estado_normal_e_o_bmc_nao_e_acelerador() {
        let f = Fixture::new("nogpu");
        f.pci("0000:00:01.0", "0x060000", "0x8086", "0x1234", None)
            .pci("0000:03:00.0", "0x030000", "0x1a03", "0x2000", Some("ast"));
        assert_eq!(
            discover_gpus(&f.root(), &[], ""),
            GpuDiscovery::NoGpu {
                basic_display: Some("ASPEED".into())
            }
        );
        let empty = Fixture::new("nopci");
        assert_eq!(
            discover_gpus(&empty.root(), &[], ""),
            GpuDiscovery::NoGpu {
                basic_display: None
            }
        );
    }

    #[test]
    fn varias_gpus_de_fabricantes_diferentes_contam_se_todas() {
        let f = Fixture::new("multi");
        f.pci(
            "0000:01:00.0",
            "0x030000",
            "0x10de",
            "0x2230",
            Some("nvidia"),
        )
        .pci(
            "0000:02:00.0",
            "0x030200",
            "0x10de",
            "0x2230",
            Some("nvidia"),
        )
        .pci(
            "0000:41:00.0",
            "0x030000",
            "0x1002",
            "0x7448",
            Some("amdgpu"),
        )
        .file("/sys/module/nvidia/version", "550.54.14\n")
        .file(
            "/sys/bus/pci/devices/0000:41:00.0/mem_info_vram_total",
            "34359738368\n",
        )
        .file(
            "/sys/bus/pci/devices/0000:41:00.0/product_name",
            "AMD Radeon PRO W7800\n",
        )
        .file("/dev/kfd", "");
        let smi = vec![
            "00000000:01:00.0, NVIDIA RTX A6000, 49140, 8.6".to_owned(),
            "00000000:02:00.0, NVIDIA RTX A6000, 49140, 8.6".to_owned(),
        ];
        let GpuDiscovery::GpuDetected { gpus } = discover_gpus(
            &f.root(),
            &smi,
            r#"{"io.containerd.runc.v2":{},"nvidia":{}}"#,
        ) else {
            panic!("as GPUs não foram detectadas");
        };
        assert_eq!(gpus.len(), 3);
        assert_eq!(gpus[0].vendor, GpuVendor::Nvidia);
        assert_eq!(gpus[0].model.as_deref(), Some("NVIDIA RTX A6000"));
        assert_eq!(gpus[0].vram_bytes, Some(49140 * 1024 * 1024));
        assert_eq!(gpus[0].compute_capability.as_deref(), Some("8.6"));
        assert_eq!(
            gpus[0].driver.as_ref().unwrap().version.as_deref(),
            Some("550.54.14")
        );
        assert_eq!(gpus[2].vendor, GpuVendor::Amd);
        assert_eq!(gpus[2].vram_bytes, Some(34_359_738_368));
        assert_eq!(
            gpus[2].runtime,
            Some((RuntimeKind::Rocm, RuntimeStatus::Detected))
        );
        assert_eq!(gpus.iter().map(|g| g.index).collect::<Vec<_>>(), [0, 1, 2]);
    }

    #[test]
    fn gpu_sem_runtime_avisa_e_nao_bloqueia() {
        let f = Fixture::new("rt");
        f.pci(
            "0000:01:00.0",
            "0x030000",
            "0x10de",
            "0x2684",
            Some("nvidia"),
        );
        assert!(matches!(
            discover_gpus(&f.root(), &[], r#"{"runc":{}}"#),
            GpuDiscovery::GpuRuntimeUnavailable { .. }
        ));
    }

    #[test]
    fn uma_arvore_pci_ilegivel_e_um_erro_e_nao_sem_gpu() {
        let f = Fixture::new("err");
        // Um dispositivo sem `class` legível.
        fs::create_dir_all(f.0.join("sys/bus/pci/devices/0000:01:00.0")).unwrap();
        assert_eq!(
            discover_gpus(&f.root(), &[], ""),
            GpuDiscovery::GpuDiscoveryError {
                code: "SYSFS_UNREADABLE".into()
            }
        );
    }

    #[test]
    fn a_rede_conta_as_interfaces_activas() {
        let f = Fixture::new("net");
        f.file("/sys/class/net/lo/operstate", "unknown\n")
            .file("/sys/class/net/eth0/operstate", "up\n")
            .file("/sys/class/net/eth0/speed", "10000\n")
            .file("/sys/class/net/eth1/operstate", "down\n")
            .file(
                "/proc/net/if_inet6",
                "fe800000000000000000000000000001 02 40 20 80 eth0\n20010db8000000000000000000000010 02 40 00 80 eth0\n",
            );
        let n = network(&f.root());
        assert_eq!(n.interfaces_up, 1);
        assert_eq!(n.max_link_mbps, Some(10_000));
        assert!(n.ipv6);
    }
}
