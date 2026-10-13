//! What the service reports: the state an interface renders. Every value here
//! was read from the system; none is what a client asked for.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The update state and signing are the Update Engine's own types (ADR-0032).
pub use ocinye_update_contracts::{ReleaseSigning, UpdateState};

use crate::capability::{CapabilityId, HardwareDependency};
use crate::reason::Unavailable;

/// A percentage, 0 to 100. Out-of-range values do not deserialise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, TS)]
#[ts(type = "number")]
pub struct Percent(u8);

impl Percent {
    /// A percentage, or nothing when above 100.
    #[must_use]
    pub const fn new(value: u8) -> Option<Self> {
        if value <= 100 {
            Some(Self(value))
        } else {
            None
        }
    }

    /// A percentage, values above 100 taken as 100. For readings that are
    /// already bounded by their source (a mixer, a signal scale).
    #[must_use]
    pub const fn saturating(value: u8) -> Self {
        Self(if value > 100 { 100 } else { value })
    }

    /// The number.
    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }
}

impl<'de> Deserialize<'de> for Percent {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = u8::deserialize(d)?;
        Self::new(v).ok_or_else(|| serde::de::Error::custom("a percentage is 0 to 100"))
    }
}

/// Whether a class of hardware or a capability can be used on this machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "state", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Availability {
    /// Usable.
    Present,
    /// Exists, cannot be used.
    Unavailable {
        /// Why.
        cause: Unavailable,
    },
    /// The machine has none.
    Absent,
}

/// One class of hardware in the inventory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct HardwareAvailability {
    /// The class.
    pub hardware: HardwareDependency,
    /// Its state.
    pub availability: Availability,
}

/// What this machine can do: the answer to `system.capabilities`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct CapabilityInventory {
    /// Each hardware class, once.
    pub hardware: Vec<HardwareAvailability>,
    /// Capabilities this build implements. A capability absent from this list
    /// answers `NOT_IMPLEMENTED`.
    pub implemented: Vec<CapabilityId>,
}

/// How healthy one part is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Health {
    /// Working.
    Ok,
    /// Working with a problem.
    Degraded,
    /// Not working.
    Failed,
    /// Not checked: never reported as healthy.
    Unknown,
}

/// One part of the system in a health report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct HealthItem {
    /// A stable identifier (`system_service`, `network`, `clock`, `storage`,
    /// `instance`).
    pub part: String,
    /// Its health.
    pub health: Health,
}

/// The answer to `system.health`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct SystemHealth {
    /// Each part checked.
    pub items: Vec<HealthItem>,
}

/// The answer to `system.info`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct SystemInfo {
    /// The machine's name.
    pub hostname: String,
    /// Ocinye OS release identifier, when one is installed.
    pub release_id: Option<String>,
    /// Whether this is a development build.
    pub development_build: bool,
    /// The base system (`Ubuntu 24.04.x LTS`).
    pub base_system: String,
    /// The kernel release.
    pub kernel: String,
    /// The architecture (`x86_64`, `aarch64`).
    pub arch: String,
    /// Processor model.
    pub cpu_model: String,
    /// Logical processors.
    pub cpu_count: u32,
    /// Memory, in mebibytes.
    pub memory_mib: u64,
    /// Whether the machine started through UEFI.
    pub uefi: bool,
    /// Secure Boot, as the firmware reports it.
    pub secure_boot: SecureBoot,
}

/// Secure Boot state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SecureBoot {
    /// Enabled.
    Enabled,
    /// Disabled.
    Disabled,
    /// The firmware does not say (or the machine is not UEFI).
    Unknown,
}

/// The kind of a network interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InterfaceKind {
    /// Wired.
    Ethernet,
    /// Wi-Fi.
    Wifi,
    /// Anything else.
    Other,
}

/// One network interface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct NetworkInterface {
    /// Kernel name.
    pub name: String,
    /// Kind.
    pub kind: InterfaceKind,
    /// Whether the link has carrier.
    pub carrier: bool,
    /// Hardware address.
    pub mac: Option<String>,
    /// Addresses, in CIDR notation.
    pub addresses: Vec<String>,
    /// Link speed in megabits per second, when the driver reports one.
    pub speed_mbps: Option<u32>,
}

/// Whether the machine reaches anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Connectivity {
    /// No interface has carrier.
    Disconnected,
    /// Carrier, no address.
    NoAddress,
    /// An address and a default route. Whether the internet answers is not
    /// probed by this capability.
    Local,
}

/// The answer to `network.status`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct NetworkState {
    /// Interfaces, loopback excluded.
    pub interfaces: Vec<NetworkInterface>,
    /// Default gateway, when there is one.
    pub gateway: Option<String>,
    /// Name servers in use.
    pub dns: Vec<String>,
    /// Summary.
    pub connectivity: Connectivity,
    /// Wi-Fi radio state, when a radio exists.
    pub wifi: Option<WifiRadioState>,
}

/// The Wi-Fi radio.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct WifiRadioState {
    /// Whether the radio is on.
    pub enabled: bool,
    /// The network it is connected to.
    pub connected: Option<String>,
}

/// How a Wi-Fi network is protected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WifiSecurity {
    /// Open.
    Open,
    /// A passphrase (WPA2 or WPA3 Personal).
    Passphrase,
    /// Enterprise authentication: not supported in v1.
    Enterprise,
}

/// One network seen in a scan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct WifiNetwork {
    /// Its name.
    pub ssid: String,
    /// Its protection.
    pub security: WifiSecurity,
    /// Signal, 0 to 100.
    pub signal: Percent,
    /// Whether credentials for it are stored.
    pub known: bool,
    /// Whether it is the one in use.
    pub connected: bool,
}

/// The answer to `wifi.scan`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct WifiScan {
    /// Networks in range, strongest first.
    pub networks: Vec<WifiNetwork>,
}

/// The class of a Bluetooth device, as far as v1 distinguishes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BluetoothClass {
    /// Headset or speaker.
    Audio,
    /// Keyboard.
    Keyboard,
    /// Mouse or trackpad.
    Pointer,
    /// Anything else: listed, not supported.
    Other,
}

/// One Bluetooth device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct BluetoothDevice {
    /// Its address.
    pub address: String,
    /// Its name.
    pub name: String,
    /// Its class.
    pub class: BluetoothClass,
    /// Paired.
    pub paired: bool,
    /// Connected.
    pub connected: bool,
}

/// The answer to `bluetooth.status` and `bluetooth.scan`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct BluetoothState {
    /// Whether the controller is powered.
    pub powered: bool,
    /// Devices known or seen.
    pub devices: Vec<BluetoothDevice>,
}

/// A sound output or input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct AudioDevice {
    /// The identifier to select it by.
    pub id: u32,
    /// Its name.
    pub name: String,
    /// Whether it is the default.
    pub default: bool,
    /// Volume.
    pub volume: Percent,
    /// Muted.
    pub muted: bool,
}

/// The answer to the audio capabilities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct AudioState {
    /// Outputs.
    pub outputs: Vec<AudioDevice>,
    /// Inputs.
    pub inputs: Vec<AudioDevice>,
}

/// One display, as the kernel's DRM connectors report it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct DisplayOutput {
    /// Connector name (`eDP-1`).
    pub name: String,
    /// Whether it is the built-in panel (eDP, LVDS, DSI).
    pub internal: bool,
    /// Whether the connector is driving the display now.
    pub enabled: bool,
    /// Width of the display's preferred mode, in pixels; 0 when it reports
    /// no mode.
    pub width: u32,
    /// Height of the display's preferred mode, in pixels; 0 when it reports
    /// no mode.
    pub height: u32,
    /// Refresh rate in millihertz, when known. The kernel's connector list
    /// does not carry it; the compositor, which sets modes, would.
    pub refresh_mhz: Option<u32>,
    /// Scale in percent (100, 125, …), when known. Scale is the
    /// compositor's, not the kernel's.
    pub scale_percent: Option<u16>,
}

/// The answer to the display capabilities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct DisplayState {
    /// Connected displays. Empty when none is connected.
    pub outputs: Vec<DisplayOutput>,
    /// Backlight level, when a controllable panel exists.
    pub brightness: Option<Percent>,
}

/// Where power comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PowerSource {
    /// Mains.
    Mains,
    /// Battery.
    Battery,
    /// Not reported.
    Unknown,
}

/// A battery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct Battery {
    /// Charge.
    pub charge: Percent,
    /// Charging.
    pub charging: bool,
    /// Present capacity against design capacity, when the firmware gives both.
    pub health: Option<Percent>,
}

/// The answer to `power.status`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct PowerState {
    /// Source.
    pub source: PowerSource,
    /// The battery, when there is one.
    pub battery: Option<Battery>,
    /// Whether the machine can suspend.
    pub can_suspend: bool,
    /// What currently forbids a power action, by stable identifier.
    pub inhibitors: Vec<String>,
}

/// A product language: exactly these three.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    /// Portuguese (Portugal): canonical.
    Pt,
    /// English.
    En,
    /// French.
    Fr,
}

/// The answer to `locale.status`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct LocaleState {
    /// The machine's language: sign-in, lock and first-start screens.
    pub language: Language,
    /// The system locale (`pt_PT.UTF-8`).
    pub system_locale: String,
}

/// The answer to `time.status`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct TimeState {
    /// The time zone (`Africa/Luanda`).
    pub timezone: String,
    /// Whether the clock is set from the network.
    pub automatic: bool,
    /// Whether it is currently synchronised.
    pub synchronised: bool,
    /// Now, Unix seconds.
    pub unix_time: i64,
}

/// The answer to `keyboard.status`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct KeyboardState {
    /// The xkb layout in use (`pt`).
    pub layout: String,
    /// Its variant, when one is set.
    pub variant: Option<String>,
    /// Layouts offered first by the interface.
    pub offered: Vec<String>,
}

/// The answer to `accessibility.status`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct AccessibilityState {
    /// Text size in percent: 100, 115, 130 or 150.
    pub text_scale_percent: u16,
    /// High contrast.
    pub high_contrast: bool,
    /// Reduced motion.
    pub reduce_motion: bool,
}

/// The answer to `privacy.status`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct PrivacyState {
    /// Whether the machine has a sound input.
    pub microphone_present: bool,
    /// Whether the machine has a camera.
    pub camera_present: bool,
    /// Whether the microphone may be used at all.
    pub microphone_allowed: bool,
    /// Whether the camera may be used at all.
    pub camera_allowed: bool,
    /// Whether a microphone stream is capturing now.
    pub microphone_in_use: bool,
    /// Whether a camera is open now. Not cheaply knowable from the
    /// kernel's files: always `false` on this build.
    pub camera_in_use: bool,
}

/// One mounted volume.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct Volume {
    /// Where it is mounted.
    pub mount_point: String,
    /// Filesystem type.
    pub filesystem: String,
    /// Size in bytes.
    #[ts(type = "number")]
    pub total_bytes: u64,
    /// Free bytes.
    #[ts(type = "number")]
    pub free_bytes: u64,
}

/// One disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct Disk {
    /// Kernel name.
    pub name: String,
    /// Model, when reported.
    pub model: Option<String>,
    /// Size in bytes.
    #[ts(type = "number")]
    pub size_bytes: u64,
    /// Removable.
    pub removable: bool,
}

/// The answer to `storage.status`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct StorageState {
    /// Disks.
    pub disks: Vec<Disk>,
    /// Mounted volumes that hold the system or its data.
    pub volumes: Vec<Volume>,
}

/// The answer to the remote-access capabilities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct RemoteAccessState {
    /// Whether the SSH service is enabled.
    pub ssh_enabled: bool,
    /// Whether it is running.
    pub ssh_active: bool,
}

/// The answer to `recovery.status`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct RecoveryState {
    /// Whether a previous release is kept and can be returned to.
    pub rollback_available: bool,
    /// The release that would be returned to.
    pub previous_release: Option<String>,
    /// The running release.
    pub current_release: Option<String>,
    /// A release staged or in flight.
    pub pending_release: Option<String>,
    /// How the last update ended.
    pub last_update: Option<ocinye_update_contracts::UpdateOutcome>,
}

/// The answer to `diagnostics.collect`: a bounded, redacted report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct DiagnosticsReport {
    /// System facts.
    pub info: SystemInfo,
    /// Health.
    pub health: SystemHealth,
    /// Inventory.
    pub inventory: CapabilityInventory,
    /// Failed units, by name. Names only, never log text.
    pub failed_units: Vec<String>,
}

/// A distribution an Instance can be created with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum Distribution {
    /// Research.
    Research,
    /// Business.
    Business,
    /// Education.
    Education,
    /// Personal.
    Personal,
}

impl Distribution {
    /// The name the release's installer knows it by.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Research => "research",
            Self::Business => "business",
            Self::Education => "education",
            Self::Personal => "personal",
        }
    }
}

/// Where local activation stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ActivationPhase {
    /// This machine cannot create an Instance by itself: it carries no
    /// release to create one from.
    NotOffered,
    /// No Instance yet, and one can be created here.
    Ready,
    /// The Instance is being created.
    Running,
    /// Creating it failed. The machine is as the failed step left it; the log
    /// is on the machine for whoever operates it.
    Failed,
    /// An Instance runs on this machine.
    Activated,
}

/// The answer to the activation capabilities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ActivationState {
    /// Phase.
    pub phase: ActivationPhase,
    /// The release an Instance is, or would be, created from.
    pub release_id: Option<String>,
    /// While running: how many steps of the installation have begun.
    pub steps_begun: u32,
    /// Whether the first administrator's temporary credential is still
    /// waiting to be shown. It is shown once.
    pub credential_waiting: bool,
}

/// The first administrator's temporary credential, handed over once.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct InitialCredential {
    /// The administrator's sign-in address.
    pub username: String,
    /// The temporary password. It opens nothing but the step that replaces it.
    pub password: crate::request::Secret,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uma_percentagem_fora_do_intervalo_nao_entra() {
        assert_eq!(serde_json::from_str::<Percent>("100").unwrap().get(), 100);
        assert!(serde_json::from_str::<Percent>("101").is_err());
        assert!(serde_json::from_str::<Percent>("-1").is_err());
        assert!(Percent::new(101).is_none());
    }

    #[test]
    fn as_linguas_sao_exactamente_tres() {
        assert_eq!(serde_json::to_string(&Language::Pt).unwrap(), "\"pt\"");
        assert!(serde_json::from_str::<Language>("\"pt-PT\"").is_err());
        assert!(serde_json::from_str::<Language>("\"es\"").is_err());
    }
}
