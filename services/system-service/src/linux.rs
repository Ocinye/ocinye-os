//! The Linux platform: the adapters that read and change the real system.
//!
//! Preference order, as the architecture fixes it: kernel interfaces (`/proc`,
//! `/sys`) first; then a system service's own tool through [`crate::tool`];
//! never a shell. Every answer is state read back from the system.
//!
//! Paths are rooted at [`Roots`] so the readers can be tested against a tree
//! built in a temporary directory. Mutations that go through a tool are not
//! exercised by unit tests: they are proved on a running image.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use ocinye_system_contracts::capability::{CapabilityId, HardwareDependency};
use ocinye_system_contracts::reason::{AdapterFailure, Reason, Unavailable};
use ocinye_system_contracts::request::{Payload, Request};
use ocinye_system_contracts::state::{
    AccessibilityState, Availability, Battery, CapabilityInventory, Connectivity,
    DiagnosticsReport, Disk, DisplayState, HardwareAvailability, Health, HealthItem, InterfaceKind,
    KeyboardState, Language, LocaleState, NetworkInterface, NetworkState, Percent, PowerSource,
    PowerState, PrivacyState, RecoveryState, ReleaseSigning, RemoteAccessState, SecureBoot,
    StorageState, SystemHealth, SystemInfo, TimeState, UpdatePhase, UpdateState, WifiRadioState,
};
use serde::{Deserialize, Serialize};

use crate::activation::{Activation, Installer, ReleaseInstaller};
use crate::service::Platform;
use crate::tool::{self, Tool};

/// Where the system's files are. `/` on a machine; a temporary tree in tests.
#[derive(Debug, Clone)]
pub struct Roots {
    /// `/sys`.
    pub sys: PathBuf,
    /// `/proc`.
    pub proc: PathBuf,
    /// `/etc`.
    pub etc: PathBuf,
    /// `/usr/lib/ocinye`.
    pub ocinye_lib: PathBuf,
    /// `/var/lib/ocinye/system`: what this service itself remembers.
    pub state: PathBuf,
}

impl Roots {
    /// The real system.
    #[must_use]
    pub fn system() -> Self {
        Self {
            sys: "/sys".into(),
            proc: "/proc".into(),
            etc: "/etc".into(),
            ocinye_lib: "/usr/lib/ocinye".into(),
            state: "/var/lib/ocinye/system".into(),
        }
    }
}

/// Capabilities this build carries out. Everything else in the catalogue
/// answers `NOT_IMPLEMENTED`, and the inventory says so.
pub const IMPLEMENTED: &[CapabilityId] = &[
    CapabilityId::SystemHealth,
    CapabilityId::SystemInfo,
    CapabilityId::SystemCapabilities,
    CapabilityId::NetworkStatus,
    CapabilityId::DisplaySetBrightness,
    CapabilityId::PowerStatus,
    CapabilityId::PowerShutdown,
    CapabilityId::PowerRestart,
    CapabilityId::PowerSuspend,
    CapabilityId::LocaleStatus,
    CapabilityId::LocaleConfigure,
    CapabilityId::TimeStatus,
    CapabilityId::TimeConfigure,
    CapabilityId::KeyboardStatus,
    CapabilityId::KeyboardConfigure,
    CapabilityId::AccessibilityStatus,
    CapabilityId::AccessibilityConfigure,
    CapabilityId::PrivacyStatus,
    CapabilityId::StorageStatus,
    CapabilityId::UpdateStatus,
    CapabilityId::DiagnosticsCollect,
    CapabilityId::RemoteAccessStatus,
    CapabilityId::RemoteAccessConfigure,
    CapabilityId::RecoveryStatus,
    CapabilityId::ActivationStatus,
    CapabilityId::ActivationStart,
    CapabilityId::ActivationCredential,
];

/// What the service remembers between restarts: preferences that have no
/// other home in the system.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Remembered {
    text_scale_percent: u16,
    high_contrast: bool,
    reduce_motion: bool,
    microphone_allowed: bool,
    camera_allowed: bool,
}

impl Default for Remembered {
    fn default() -> Self {
        Self {
            text_scale_percent: 100,
            high_contrast: false,
            reduce_motion: false,
            microphone_allowed: true,
            camera_allowed: true,
        }
    }
}

/// The Linux platform.
pub struct Linux {
    roots: Roots,
    remembered: Mutex<()>,
    activation: Activation,
}

fn read(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok().map(|s| s.trim().to_owned())
}

fn entries(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .map(|d| {
            d.filter_map(Result::ok)
                .filter_map(|e| e.file_name().into_string().ok())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

fn failed(cause: AdapterFailure) -> Reason {
    Reason::AdapterFailed { cause }
}

/// `KEY=value` lines, as `systemctl show`-style tools print them.
fn property<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.lines()
        .find_map(|l| l.strip_prefix(key).and_then(|r| r.strip_prefix('=')))
}

/// The usable addresses of each interface, from `ip -json address show`:
/// CIDR notation, without the link-local ones nobody can be reached at.
fn addresses(json: &str) -> std::collections::BTreeMap<String, Vec<String>> {
    let mut out = std::collections::BTreeMap::new();
    let Ok(serde_json::Value::Array(links)) = serde_json::from_str(json) else {
        return out;
    };
    for link in &links {
        let Some(name) = link["ifname"].as_str() else {
            continue;
        };
        let found = link["addr_info"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|a| {
                let local = a["local"].as_str()?;
                let prefix = a["prefixlen"].as_u64()?;
                (a["scope"].as_str() == Some("global") && local.parse::<std::net::IpAddr>().is_ok())
                    .then(|| format!("{local}/{prefix}"))
            });
        out.insert(name.to_owned(), found.collect());
    }
    out
}

impl Linux {
    /// The platform over a set of roots.
    #[must_use]
    pub fn new(roots: Roots) -> Self {
        Self::with_installer(roots, std::sync::Arc::new(ReleaseInstaller))
    }

    /// The same, with the installation carried out by `installer`.
    #[must_use]
    pub fn with_installer(roots: Roots, installer: std::sync::Arc<dyn Installer>) -> Self {
        let activation = Activation::new(
            roots.ocinye_lib.join("release"),
            roots.etc.join("ocinye/release.env"),
            roots.state.join("activation"),
            installer,
        );
        Self {
            roots,
            remembered: Mutex::new(()),
            activation,
        }
    }

    fn remembered(&self) -> Remembered {
        fs::read(self.roots.state.join("preferences.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    fn remember(&self, r: &Remembered) -> Result<(), Reason> {
        let _guard = self.remembered.lock();
        fs::create_dir_all(&self.roots.state).map_err(|_| failed(AdapterFailure::Refused))?;
        let tmp = self.roots.state.join("preferences.json.new");
        let bytes = serde_json::to_vec(r).map_err(|_| failed(AdapterFailure::UnexpectedReply))?;
        fs::write(&tmp, bytes).map_err(|_| failed(AdapterFailure::Refused))?;
        fs::rename(&tmp, self.roots.state.join("preferences.json"))
            .map_err(|_| failed(AdapterFailure::Refused))
    }

    // ---------------------------------------------------------------- hardware

    fn interfaces(&self) -> Vec<NetworkInterface> {
        let net = self.roots.sys.join("class/net");
        // The kernel does not publish addresses under /sys: `ip` is asked,
        // as JSON. Without an answer an interface is listed with none.
        let mut addressed = tool::run(Tool::Ip, &["-json", "address", "show"])
            .map(|out| addresses(&out))
            .unwrap_or_default();
        entries(&net)
            .into_iter()
            .filter(|n| n != "lo")
            // Only real devices: bridges and container veth pairs have no
            // `device` link and are not what a person means by "the network".
            .filter(|n| net.join(n).join("device").exists())
            .map(|name| {
                let d = net.join(&name);
                let kind = if d.join("wireless").exists() || d.join("phy80211").exists() {
                    InterfaceKind::Wifi
                } else if read(&d.join("type")).as_deref() == Some("1") {
                    InterfaceKind::Ethernet
                } else {
                    InterfaceKind::Other
                };
                NetworkInterface {
                    carrier: read(&d.join("carrier")).as_deref() == Some("1"),
                    mac: read(&d.join("address")).filter(|m| m != "00:00:00:00:00:00"),
                    speed_mbps: read(&d.join("speed"))
                        .and_then(|s| s.parse::<u32>().ok())
                        .filter(|s| *s > 0),
                    addresses: addressed.remove(&name).unwrap_or_default(),
                    name,
                    kind,
                }
            })
            .collect()
    }

    fn backlight(&self) -> Option<(PathBuf, u64, u64)> {
        let dir = self.roots.sys.join("class/backlight");
        entries(&dir).into_iter().find_map(|n| {
            let d = dir.join(n);
            let max = read(&d.join("max_brightness"))?
                .parse::<u64>()
                .ok()
                .filter(|m| *m > 0)?;
            let now = read(&d.join("brightness"))?.parse::<u64>().ok()?;
            Some((d, now, max))
        })
    }

    fn brightness(&self) -> Option<Percent> {
        let (_, now, max) = self.backlight()?;
        Percent::new(u8::try_from((now * 100 + max / 2) / max).ok()?.min(100))
    }

    fn battery(&self) -> Option<Battery> {
        let dir = self.roots.sys.join("class/power_supply");
        entries(&dir).into_iter().find_map(|n| {
            let d = dir.join(n);
            if read(&d.join("type")).as_deref() != Some("Battery")
                || read(&d.join("scope")).as_deref() == Some("Device")
            {
                return None;
            }
            let charge = read(&d.join("capacity"))?
                .parse::<u8>()
                .ok()
                .and_then(|c| Percent::new(c.min(100)))?;
            let full = read(&d.join("energy_full"))
                .or_else(|| read(&d.join("charge_full")))
                .and_then(|v| v.parse::<u64>().ok());
            let design = read(&d.join("energy_full_design"))
                .or_else(|| read(&d.join("charge_full_design")))
                .and_then(|v| v.parse::<u64>().ok());
            let health = match (full, design) {
                (Some(f), Some(dsg)) if dsg > 0 => {
                    Percent::new(u8::try_from((f * 100 / dsg).min(100)).unwrap_or(100))
                }
                _ => None,
            };
            Some(Battery {
                charge,
                charging: read(&d.join("status")).as_deref() == Some("Charging"),
                health,
            })
        })
    }

    fn power_source(&self) -> PowerSource {
        let dir = self.roots.sys.join("class/power_supply");
        let mut mains = None;
        for n in entries(&dir) {
            let d = dir.join(n);
            if read(&d.join("type")).as_deref() == Some("Mains") {
                mains = Some(read(&d.join("online")).as_deref() == Some("1"));
                if mains == Some(true) {
                    return PowerSource::Mains;
                }
            }
        }
        match (mains, self.battery()) {
            (Some(false), Some(_)) => PowerSource::Battery,
            (None, None) => PowerSource::Mains,
            _ => PowerSource::Unknown,
        }
    }

    fn hardware(&self) -> Vec<HardwareAvailability> {
        let ifaces = self.interfaces();
        let present = |yes: bool| {
            if yes {
                Availability::Present
            } else {
                Availability::Absent
            }
        };
        let rfkill = |kind: &str| -> Option<Availability> {
            let dir = self.roots.sys.join("class/rfkill");
            entries(&dir).into_iter().find_map(|n| {
                let d = dir.join(n);
                (read(&d.join("type")).as_deref() == Some(kind)).then(|| {
                    if read(&d.join("hard")).as_deref() == Some("1") {
                        Availability::Unavailable {
                            cause: Unavailable::HardBlocked,
                        }
                    } else {
                        Availability::Present
                    }
                })
            })
        };
        let has_wifi = ifaces.iter().any(|i| i.kind == InterfaceKind::Wifi);
        let sound = self.roots.proc.join("asound/cards");
        let has_sound = read(&sound).is_some_and(|s| !s.is_empty() && !s.contains("no soundcards"));
        let has_display = entries(&self.roots.sys.join("class/drm")).iter().any(|n| {
            n.contains('-')
                && read(&self.roots.sys.join("class/drm").join(n).join("status")).as_deref()
                    == Some("connected")
        });
        let has_camera = !entries(&self.roots.sys.join("class/video4linux")).is_empty();
        vec![
            HardwareAvailability {
                hardware: HardwareDependency::NetworkInterface,
                availability: present(!ifaces.is_empty()),
            },
            HardwareAvailability {
                hardware: HardwareDependency::WifiRadio,
                availability: if has_wifi {
                    rfkill("wlan").unwrap_or(Availability::Present)
                } else {
                    Availability::Absent
                },
            },
            HardwareAvailability {
                hardware: HardwareDependency::BluetoothController,
                availability: if entries(&self.roots.sys.join("class/bluetooth")).is_empty() {
                    Availability::Absent
                } else {
                    rfkill("bluetooth").unwrap_or(Availability::Present)
                },
            },
            HardwareAvailability {
                hardware: HardwareDependency::AudioOutput,
                availability: present(has_sound),
            },
            HardwareAvailability {
                hardware: HardwareDependency::AudioInput,
                availability: present(has_sound),
            },
            HardwareAvailability {
                hardware: HardwareDependency::Display,
                availability: present(has_display),
            },
            HardwareAvailability {
                hardware: HardwareDependency::Backlight,
                availability: present(self.backlight().is_some()),
            },
            HardwareAvailability {
                hardware: HardwareDependency::Battery,
                availability: present(self.battery().is_some()),
            },
            HardwareAvailability {
                hardware: HardwareDependency::Camera,
                availability: present(has_camera),
            },
        ]
    }

    // ---------------------------------------------------------------- readers

    fn info(&self) -> SystemInfo {
        let cpuinfo = read(&self.roots.proc.join("cpuinfo")).unwrap_or_default();
        let meminfo = read(&self.roots.proc.join("meminfo")).unwrap_or_default();
        let os = read(&self.roots.etc.join("os-release")).unwrap_or_default();
        let facts: Option<serde_json::Value> = fs::read(self.roots.etc.join("ocinye/image.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok());
        let efi = self.roots.sys.join("firmware/efi");
        // The SecureBoot variable: four attribute bytes, then one byte of state.
        let secure_boot = entries(&efi.join("efivars"))
            .into_iter()
            .find(|n| n.starts_with("SecureBoot-"))
            .and_then(|n| fs::read(efi.join("efivars").join(n)).ok())
            .map_or(SecureBoot::Unknown, |b| match b.get(4) {
                Some(1) => SecureBoot::Enabled,
                Some(0) => SecureBoot::Disabled,
                _ => SecureBoot::Unknown,
            });
        SystemInfo {
            hostname: read(&self.roots.proc.join("sys/kernel/hostname")).unwrap_or_default(),
            release_id: facts
                .as_ref()
                .and_then(|f| f.get("release_id"))
                .and_then(|v| v.as_str())
                .map(str::to_owned),
            // Until a production signing key exists, every image is a
            // development build; an image that says otherwise is believed.
            development_build: facts
                .as_ref()
                .and_then(|f| f.pointer("/image/build"))
                .and_then(|v| v.as_str())
                .is_none_or(|b| !b.eq_ignore_ascii_case("release")),
            base_system: os
                .lines()
                .find_map(|l| l.strip_prefix("PRETTY_NAME="))
                .map(|v| v.trim_matches('"').to_owned())
                .unwrap_or_default(),
            kernel: read(&self.roots.proc.join("sys/kernel/osrelease")).unwrap_or_default(),
            arch: std::env::consts::ARCH.to_owned(),
            cpu_model: cpuinfo
                .lines()
                .find_map(|l| {
                    l.strip_prefix("model name")
                        .and_then(|r| r.split_once(':'))
                        .map(|(_, v)| v.trim().to_owned())
                })
                .unwrap_or_default(),
            cpu_count: u32::try_from(
                cpuinfo
                    .lines()
                    .filter(|l| l.starts_with("processor"))
                    .count(),
            )
            .unwrap_or(0),
            memory_mib: meminfo
                .lines()
                .find_map(|l| l.strip_prefix("MemTotal:"))
                .and_then(|v| v.trim().trim_end_matches("kB").trim().parse::<u64>().ok())
                .map_or(0, |kb| kb / 1024),
            uefi: efi.exists(),
            secure_boot,
        }
    }

    fn network(&self) -> NetworkState {
        let interfaces = self.interfaces();
        // The default route, from the kernel's own table: destination 0.
        let gateway = read(&self.roots.proc.join("net/route")).and_then(|t| {
            t.lines().skip(1).find_map(|l| {
                let f: Vec<&str> = l.split_whitespace().collect();
                (f.len() > 2 && f[1] == "00000000")
                    .then(|| u32::from_str_radix(f[2], 16).ok())
                    .flatten()
            })
        });
        let gateway = gateway.map(|g| {
            let b = g.to_le_bytes();
            format!("{}.{}.{}.{}", b[0], b[1], b[2], b[3])
        });
        let dns = read(&self.roots.etc.join("resolv.conf"))
            .map(|t| {
                t.lines()
                    .filter_map(|l| l.strip_prefix("nameserver "))
                    .map(|s| s.trim().to_owned())
                    .take(4)
                    .collect()
            })
            .unwrap_or_default();
        let connectivity = if !interfaces.iter().any(|i| i.carrier) {
            Connectivity::Disconnected
        } else if gateway.is_none() {
            Connectivity::NoAddress
        } else {
            Connectivity::Local
        };
        let wifi = interfaces
            .iter()
            .any(|i| i.kind == InterfaceKind::Wifi)
            .then(|| {
                let dir = self.roots.sys.join("class/rfkill");
                let blocked = entries(&dir).into_iter().any(|n| {
                    let d = dir.join(n);
                    read(&d.join("type")).as_deref() == Some("wlan")
                        && (read(&d.join("soft")).as_deref() == Some("1")
                            || read(&d.join("hard")).as_deref() == Some("1"))
                });
                WifiRadioState {
                    enabled: !blocked,
                    connected: None,
                }
            });
        NetworkState {
            interfaces,
            gateway,
            dns,
            connectivity,
            wifi,
        }
    }

    fn storage(&self) -> StorageState {
        let block = self.roots.sys.join("block");
        let disks = entries(&block)
            .into_iter()
            .filter(|n| {
                !n.starts_with("loop")
                    && !n.starts_with("ram")
                    && !n.starts_with("dm-")
                    && !n.starts_with("zram")
            })
            .filter_map(|name| {
                let d = block.join(&name);
                let sectors = read(&d.join("size"))?
                    .parse::<u64>()
                    .ok()
                    .filter(|s| *s > 0)?;
                Some(Disk {
                    model: read(&d.join("device/model")).filter(|m| !m.is_empty()),
                    size_bytes: sectors * 512,
                    removable: read(&d.join("removable")).as_deref() == Some("1"),
                    name,
                })
            })
            .collect();
        StorageState {
            disks,
            volumes: Vec::new(),
        }
    }

    fn update(&self) -> UpdateState {
        let releases = entries(&self.roots.ocinye_lib.join("release"));
        UpdateState {
            current_release: releases.last().cloned(),
            pending_release: None,
            phase: UpdatePhase::UpToDate,
            signing: ReleaseSigning::DevOnly,
        }
    }

    fn recovery(&self) -> RecoveryState {
        let releases = entries(&self.roots.ocinye_lib.join("release"));
        let previous = (releases.len() > 1).then(|| releases[releases.len() - 2].clone());
        RecoveryState {
            rollback_available: previous.is_some(),
            previous_release: previous,
        }
    }

    fn power(&self) -> PowerState {
        let can_suspend = read(&self.roots.sys.join("power/state"))
            .is_some_and(|s| s.split_whitespace().any(|m| m == "mem" || m == "freeze"));
        PowerState {
            source: self.power_source(),
            battery: self.battery(),
            can_suspend,
            inhibitors: Vec::new(),
        }
    }

    fn inventory(&self) -> CapabilityInventory {
        CapabilityInventory {
            hardware: self.hardware(),
            implemented: IMPLEMENTED.to_vec(),
        }
    }

    fn accessibility(&self) -> AccessibilityState {
        let r = self.remembered();
        AccessibilityState {
            text_scale_percent: r.text_scale_percent,
            high_contrast: r.high_contrast,
            reduce_motion: r.reduce_motion,
        }
    }

    fn privacy(&self) -> PrivacyState {
        let r = self.remembered();
        PrivacyState {
            microphone_allowed: r.microphone_allowed,
            camera_allowed: r.camera_allowed,
            microphone_in_use: false,
            camera_in_use: false,
        }
    }

    // ------------------------------------------------- readers through a tool

    fn time(&self) -> Result<TimeState, Reason> {
        let out = tool::run(Tool::Timedatectl, &["show"]).map_err(failed)?;
        Ok(TimeState {
            timezone: property(&out, "Timezone").unwrap_or("UTC").to_owned(),
            automatic: property(&out, "NTP") == Some("yes"),
            synchronised: property(&out, "NTPSynchronized") == Some("yes"),
            unix_time: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX)),
        })
    }

    fn locale(&self) -> LocaleState {
        let conf = read(&self.roots.etc.join("locale.conf"))
            .or_else(|| read(&self.roots.etc.join("default/locale")))
            .unwrap_or_default();
        let system_locale = property(&conf, "LANG")
            .map_or_else(|| "C.UTF-8".to_owned(), |v| v.trim_matches('"').to_owned());
        let language = match system_locale.get(..2) {
            Some("en") => Language::En,
            Some("fr") => Language::Fr,
            _ => Language::Pt,
        };
        LocaleState {
            language,
            system_locale,
        }
    }

    fn keyboard(&self) -> KeyboardState {
        let conf = read(&self.roots.etc.join("default/keyboard")).unwrap_or_default();
        let get = |k: &str| {
            property(&conf, k)
                .map(|v| v.trim_matches('"').to_owned())
                .filter(|v| !v.is_empty())
        };
        KeyboardState {
            layout: get("XKBLAYOUT").unwrap_or_else(|| "pt".to_owned()),
            variant: get("XKBVARIANT"),
            offered: ["pt", "gb", "us", "fr"]
                .iter()
                .map(|s| (*s).to_owned())
                .collect(),
        }
    }

    fn remote_access(&self) -> RemoteAccessState {
        let is = |verb: &str| tool::run(Tool::Systemctl, &[verb, "--quiet", "ssh.service"]).is_ok();
        RemoteAccessState {
            ssh_enabled: is("is-enabled"),
            ssh_active: is("is-active"),
        }
    }

    fn health(&self) -> SystemHealth {
        let net = self.network();
        let item = |part: &str, health: Health| HealthItem {
            part: part.to_owned(),
            health,
        };
        SystemHealth {
            items: vec![
                item("system_service", Health::Ok),
                item(
                    "network",
                    if net.connectivity == Connectivity::Local {
                        Health::Ok
                    } else {
                        Health::Degraded
                    },
                ),
                item(
                    "clock",
                    match self.time() {
                        Ok(t) if t.synchronised => Health::Ok,
                        Ok(_) => Health::Degraded,
                        Err(_) => Health::Unknown,
                    },
                ),
                // Not checked by this service: never reported as healthy.
                item("instance", Health::Unknown),
            ],
        }
    }

    fn diagnostics(&self) -> DiagnosticsReport {
        let failed_units = tool::run(
            Tool::Systemctl,
            &[
                "list-units",
                "--failed",
                "--plain",
                "--no-legend",
                "--no-pager",
            ],
        )
        .map(|out| {
            out.lines()
                .filter_map(|l| l.split_whitespace().next())
                // Unit names only, bounded: never log text.
                .filter(|u| {
                    u.len() <= 128
                        && u.bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b"-_.@:\\".contains(&b))
                })
                .take(64)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
        DiagnosticsReport {
            info: self.info(),
            health: self.health(),
            inventory: self.inventory(),
            failed_units,
        }
    }

    // --------------------------------------------------------------- mutations

    fn set_brightness(&self, percent: Percent) -> Result<Payload, Reason> {
        let Some((dir, _, max)) = self.backlight() else {
            return Err(Reason::HardwareAbsent);
        };
        // Never fully dark from a slider: the floor is one step.
        let raw = (u64::from(percent.get()) * max / 100).max(1).min(max);
        fs::write(dir.join("brightness"), raw.to_string())
            .map_err(|_| failed(AdapterFailure::Refused))?;
        Ok(Payload::Display(DisplayState {
            outputs: Vec::new(),
            brightness: self.brightness(),
        }))
    }

    fn power_action(verb: &'static str) -> Result<Payload, Reason> {
        tool::run(Tool::Systemctl, &[verb]).map_err(failed)?;
        Ok(Payload::Done)
    }
}

impl Platform for Linux {
    fn perform(&self, request: &Request) -> Result<Payload, Reason> {
        if !IMPLEMENTED.contains(&request.capability()) {
            return Err(Reason::NotImplemented);
        }
        match request {
            Request::ActivationStatus {} => Ok(Payload::Activation(self.activation.state())),
            Request::ActivationStart { .. } => {
                self.activation.start(request).map(Payload::Activation)
            }
            Request::ActivationCredential {} => {
                self.activation.credential().map(Payload::InitialCredential)
            }
            Request::SystemHealth {} => Ok(Payload::Health(self.health())),
            Request::SystemInfo {} => Ok(Payload::Info(self.info())),
            Request::SystemCapabilities {} => Ok(Payload::Inventory(self.inventory())),
            Request::NetworkStatus {} => Ok(Payload::Network(self.network())),
            Request::DisplaySetBrightness { percent } => self.set_brightness(*percent),
            Request::PowerStatus {} => Ok(Payload::Power(self.power())),
            Request::PowerShutdown {} => Self::power_action("poweroff"),
            Request::PowerRestart {} => Self::power_action("reboot"),
            Request::PowerSuspend {} => {
                if !self.power().can_suspend {
                    return Err(Reason::HardwareAbsent);
                }
                Self::power_action("suspend")
            }
            Request::LocaleStatus {} => Ok(Payload::Locale(self.locale())),
            Request::LocaleConfigure { language } => {
                let locale = match language {
                    Language::Pt => "LANG=pt_PT.UTF-8",
                    Language::En => "LANG=en_GB.UTF-8",
                    Language::Fr => "LANG=fr_FR.UTF-8",
                };
                tool::run(Tool::Localectl, &["set-locale", locale]).map_err(failed)?;
                Ok(Payload::Locale(self.locale()))
            }
            Request::TimeStatus {} => self.time().map(Payload::Time),
            Request::TimeConfigure {
                timezone,
                automatic,
            } => {
                if let Some(tz) = timezone {
                    let tz = tool::value(tz).ok_or(Reason::ValueOutOfRange)?;
                    tool::run(Tool::Timedatectl, &["set-timezone", tz]).map_err(|e| match e {
                        // The tool refuses a zone it does not know.
                        AdapterFailure::Refused => Reason::NotFound,
                        other => failed(other),
                    })?;
                }
                if let Some(auto) = automatic {
                    tool::run(
                        Tool::Timedatectl,
                        &["set-ntp", if *auto { "true" } else { "false" }],
                    )
                    .map_err(failed)?;
                }
                self.time().map(Payload::Time)
            }
            Request::KeyboardStatus {} => Ok(Payload::Keyboard(self.keyboard())),
            Request::KeyboardConfigure { layout, variant } => {
                let layout = tool::value(layout).ok_or(Reason::ValueOutOfRange)?;
                let variant = match variant {
                    Some(v) => tool::value(v).ok_or(Reason::ValueOutOfRange)?,
                    None => "",
                };
                // model, then variant: `localectl set-x11-keymap LAYOUT MODEL VARIANT`.
                let args: &[&str] = if variant.is_empty() {
                    &["set-x11-keymap", layout]
                } else {
                    &["set-x11-keymap", layout, "pc105", variant]
                };
                tool::run(Tool::Localectl, args).map_err(|e| match e {
                    AdapterFailure::Refused => Reason::NotFound,
                    other => failed(other),
                })?;
                Ok(Payload::Keyboard(self.keyboard()))
            }
            Request::AccessibilityStatus {} => Ok(Payload::Accessibility(self.accessibility())),
            Request::AccessibilityConfigure {
                text_scale_percent,
                high_contrast,
                reduce_motion,
            } => {
                let mut r = self.remembered();
                r.text_scale_percent = *text_scale_percent;
                r.high_contrast = *high_contrast;
                r.reduce_motion = *reduce_motion;
                self.remember(&r)?;
                Ok(Payload::Accessibility(self.accessibility()))
            }
            Request::PrivacyStatus {} => Ok(Payload::Privacy(self.privacy())),
            Request::StorageStatus {} => Ok(Payload::Storage(self.storage())),
            Request::UpdateStatus {} => Ok(Payload::Update(self.update())),
            Request::DiagnosticsCollect {} => {
                Ok(Payload::Diagnostics(Box::new(self.diagnostics())))
            }
            Request::RemoteAccessStatus {} => Ok(Payload::RemoteAccess(self.remote_access())),
            Request::RemoteAccessConfigure { ssh_enabled } => {
                let verb = if *ssh_enabled { "enable" } else { "disable" };
                tool::run(Tool::Systemctl, &[verb, "--now", "ssh.service"]).map_err(failed)?;
                Ok(Payload::RemoteAccess(self.remote_access()))
            }
            Request::RecoveryStatus {} => Ok(Payload::Recovery(self.recovery())),
            _ => Err(Reason::NotImplemented),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static N: AtomicU32 = AtomicU32::new(0);

    /// A system tree in a fresh temporary directory.
    struct Tree(PathBuf);

    impl Tree {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!(
                "ocinye-system-{}-{}",
                std::process::id(),
                N.fetch_add(1, Ordering::SeqCst)
            ));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
        fn put(&self, rel: &str, content: &str) -> &Self {
            let p = self.0.join(rel);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, content).unwrap();
            self
        }
        fn linux(&self) -> Linux {
            Linux::new(Roots {
                sys: self.0.join("sys"),
                proc: self.0.join("proc"),
                etc: self.0.join("etc"),
                ocinye_lib: self.0.join("usr/lib/ocinye"),
                state: self.0.join("var/lib/ocinye/system"),
            })
        }
    }

    impl Drop for Tree {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn availability(l: &Linux, h: HardwareDependency) -> Availability {
        l.hardware()
            .into_iter()
            .find(|a| a.hardware == h)
            .unwrap()
            .availability
    }

    #[test]
    fn uma_maquina_sem_nada_diz_ausente_e_nao_inventa() {
        let t = Tree::new();
        let l = t.linux();
        for h in l.hardware() {
            assert_eq!(h.availability, Availability::Absent, "{:?}", h.hardware);
        }
        assert_eq!(l.power().battery, None);
        assert_eq!(l.brightness(), None);
        assert_eq!(l.network().connectivity, Connectivity::Disconnected);
        assert_eq!(
            l.perform(&Request::DisplaySetBrightness {
                percent: Percent::new(40).unwrap()
            }),
            Err(Reason::HardwareAbsent)
        );
    }

    #[test]
    fn um_portatil_tem_bateria_brilho_wifi_e_as_pontes_nao_contam() {
        let t = Tree::new();
        t.put("sys/class/net/enp3s0/device/vendor", "0x8086")
            .put("sys/class/net/enp3s0/type", "1")
            .put("sys/class/net/enp3s0/carrier", "1")
            .put("sys/class/net/enp3s0/address", "52:54:00:12:34:56")
            .put("sys/class/net/enp3s0/speed", "1000")
            .put("sys/class/net/wlp2s0/device/vendor", "0x8086")
            .put("sys/class/net/wlp2s0/wireless/x", "")
            .put("sys/class/net/wlp2s0/carrier", "0")
            .put("sys/class/net/docker0/type", "1")
            .put("sys/class/net/lo/type", "772")
            .put("sys/class/rfkill/rfkill0/type", "wlan")
            .put("sys/class/rfkill/rfkill0/hard", "0")
            .put("sys/class/rfkill/rfkill0/soft", "0")
            .put("sys/class/backlight/intel_backlight/max_brightness", "1000")
            .put("sys/class/backlight/intel_backlight/brightness", "700")
            .put("sys/class/power_supply/BAT0/type", "Battery")
            .put("sys/class/power_supply/BAT0/capacity", "82")
            .put("sys/class/power_supply/BAT0/status", "Discharging")
            .put("sys/class/power_supply/BAT0/energy_full", "9100")
            .put("sys/class/power_supply/BAT0/energy_full_design", "10000")
            .put("sys/class/power_supply/AC/type", "Mains")
            .put("sys/class/power_supply/AC/online", "0")
            .put(
                "proc/net/route",
                "Iface\tDestination\tGateway\nenp3s0\t00000000\t0101A8C0\t0003\n",
            )
            .put("etc/resolv.conf", "nameserver 192.168.1.1\n");
        let l = t.linux();
        let net = l.network();
        assert_eq!(
            net.interfaces
                .iter()
                .map(|i| i.name.as_str())
                .collect::<Vec<_>>(),
            ["enp3s0", "wlp2s0"]
        );
        assert_eq!(net.gateway.as_deref(), Some("192.168.1.1"));
        assert_eq!(net.connectivity, Connectivity::Local);
        assert_eq!(
            net.wifi,
            Some(WifiRadioState {
                enabled: true,
                connected: None
            })
        );
        assert_eq!(
            availability(&l, HardwareDependency::WifiRadio),
            Availability::Present
        );
        assert_eq!(l.brightness().unwrap().get(), 70);
        let p = l.power();
        assert_eq!(p.source, PowerSource::Battery);
        let b = p.battery.unwrap();
        assert_eq!(
            (b.charge.get(), b.charging, b.health.unwrap().get()),
            (82, false, 91)
        );
    }

    #[test]
    fn um_interruptor_de_hardware_e_indisponivel_e_nao_ausente() {
        let t = Tree::new();
        t.put("sys/class/net/wlp2s0/device/vendor", "0x8086")
            .put("sys/class/net/wlp2s0/wireless/x", "")
            .put("sys/class/rfkill/rfkill0/type", "wlan")
            .put("sys/class/rfkill/rfkill0/hard", "1");
        let l = t.linux();
        assert_eq!(
            availability(&l, HardwareDependency::WifiRadio),
            Availability::Unavailable {
                cause: Unavailable::HardBlocked
            }
        );
    }

    #[test]
    fn o_brilho_escreve_se_e_le_se_de_volta_e_nunca_fica_a_zero() {
        let t = Tree::new();
        t.put("sys/class/backlight/b/max_brightness", "255")
            .put("sys/class/backlight/b/brightness", "255");
        let l = t.linux();
        let Payload::Display(d) = l
            .perform(&Request::DisplaySetBrightness {
                percent: Percent::new(40).unwrap(),
            })
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(d.brightness.unwrap().get(), 40);
        assert_eq!(
            read(&t.0.join("sys/class/backlight/b/brightness")).unwrap(),
            "102"
        );
        l.perform(&Request::DisplaySetBrightness {
            percent: Percent::new(0).unwrap(),
        })
        .unwrap();
        assert_eq!(
            read(&t.0.join("sys/class/backlight/b/brightness")).unwrap(),
            "1"
        );
    }

    #[test]
    fn as_preferencias_persistem_e_um_ficheiro_estragado_volta_ao_omissao() {
        let t = Tree::new();
        let l = t.linux();
        assert_eq!(l.accessibility().text_scale_percent, 100);
        l.perform(&Request::AccessibilityConfigure {
            text_scale_percent: 130,
            high_contrast: true,
            reduce_motion: false,
        })
        .unwrap();
        assert_eq!(
            t.linux().accessibility(),
            AccessibilityState {
                text_scale_percent: 130,
                high_contrast: true,
                reduce_motion: false
            }
        );
        t.put("var/lib/ocinye/system/preferences.json", "{not json");
        assert_eq!(t.linux().accessibility().text_scale_percent, 100);
    }

    #[test]
    fn a_imagem_diz_o_release_e_sem_palavra_dela_e_desenvolvimento() {
        let t = Tree::new();
        t.put(
            "etc/ocinye/image.json",
            r#"{"release_id":"f3fc6ba0863b","image":{"build":"development"}}"#,
        )
        .put("etc/os-release", "PRETTY_NAME=\"Ubuntu 24.04.5 LTS\"\n")
        .put(
            "proc/cpuinfo",
            "processor\t: 0\nmodel name\t: Test CPU\nprocessor\t: 1\nmodel name\t: Test CPU\n",
        )
        .put("proc/meminfo", "MemTotal:       4194304 kB\n")
        .put("usr/lib/ocinye/release/f3fc6ba0863b/RELEASE", "");
        let l = t.linux();
        let i = l.info();
        assert_eq!(
            (
                i.release_id.as_deref(),
                i.development_build,
                i.cpu_count,
                i.memory_mib
            ),
            (Some("f3fc6ba0863b"), true, 2, 4096)
        );
        assert_eq!(i.base_system, "Ubuntu 24.04.5 LTS");
        assert_eq!((i.uefi, i.secure_boot), (false, SecureBoot::Unknown));
        assert_eq!(l.update().current_release.as_deref(), Some("f3fc6ba0863b"));
        assert!(!l.recovery().rollback_available);
        assert!(Tree::new().linux().info().development_build);
    }

    #[test]
    fn o_que_nao_esta_implementado_di_lo_e_o_inventario_tambem() {
        let l = Tree::new().linux();
        assert_eq!(
            l.perform(&Request::WifiScan {}),
            Err(Reason::NotImplemented)
        );
        assert_eq!(
            l.perform(&Request::UpdateApply {}),
            Err(Reason::NotImplemented)
        );
        let inv = l.inventory();
        assert!(!inv.implemented.contains(&CapabilityId::WifiScan));
        assert!(inv.implemented.contains(&CapabilityId::PowerRestart));
    }

    #[test]
    fn os_enderecos_vem_do_ip_em_json_e_so_os_que_servem() {
        let json = r#"[
          {"ifname":"lo","addr_info":[{"family":"inet","local":"127.0.0.1","prefixlen":8,"scope":"host"}]},
          {"ifname":"enp1s0","addr_info":[
            {"family":"inet","local":"192.168.1.84","prefixlen":24,"scope":"global"},
            {"family":"inet6","local":"fe80::5054:ff:fe12:3456","prefixlen":64,"scope":"link"},
            {"family":"inet6","local":"2001:db8::84","prefixlen":64,"scope":"global"},
            {"family":"inet","local":"not-an-address","prefixlen":24,"scope":"global"}]},
          {"ifname":"wlan0","addr_info":[]}
        ]"#;
        let found = addresses(json);
        assert_eq!(found["enp1s0"], ["192.168.1.84/24", "2001:db8::84/64"]);
        assert!(found["lo"].is_empty() && found["wlan0"].is_empty());
        // Anything that is not the expected JSON is no addresses, not a panic.
        assert!(addresses("ip: command failed").is_empty());
        assert!(addresses("{}").is_empty());
    }
}
