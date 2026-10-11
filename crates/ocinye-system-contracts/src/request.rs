//! What a client may ask, and what the service answers.
//!
//! [`Request`] is a closed enum with one variant per capability. No variant
//! carries a command line, an executable, an environment or a path the service
//! would open: every argument is a bounded value the service validates again.

use std::fmt;
use std::net::Ipv4Addr;

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use zeroize::Zeroize;

use crate::capability::CapabilityId;
use crate::reason::Reason;
use crate::state::{
    AccessibilityState, ActivationState, AudioState, BluetoothState, CapabilityInventory,
    DiagnosticsReport, DisplayState, Distribution, InitialCredential, KeyboardState, Language,
    LocaleState, NetworkState, Percent, PowerState, PrivacyState, RecoveryState, RemoteAccessState,
    StorageState, SystemHealth, SystemInfo, TimeState, UpdateState, WifiScan,
};

/// A secret that crosses the socket once (a Wi-Fi passphrase). Zeroized on
/// drop and never printed. It serialises, because a client must be able to
/// send it; nothing on the service side serialises a request (audit records
/// are built from [`Request::audit_summary`], which has no secret in it).
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(type = "string")]
pub struct Secret(String);

impl Secret {
    /// Wrap a value.
    #[must_use]
    pub fn new(value: String) -> Self {
        Self(value)
    }

    /// The value, for the one call that hands it to the subsystem.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[secret]")
    }
}

/// Output or input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AudioTarget {
    /// The default output.
    Output,
    /// The default input.
    Input,
}

/// How an interface gets its address.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "mode", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
pub enum Addressing {
    /// From the network.
    Dhcp,
    /// Fixed.
    Static {
        /// Address, dotted quad.
        address: String,
        /// Prefix length, 1 to 32.
        prefix: u8,
        /// Gateway, dotted quad.
        gateway: String,
        /// Name servers, dotted quads; at most four.
        dns: Vec<String>,
    },
}

/// A request for one capability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "capability", deny_unknown_fields)]
pub enum Request {
    /// `system.health`.
    #[serde(rename = "system.health")]
    SystemHealth {},
    /// `system.info`.
    #[serde(rename = "system.info")]
    SystemInfo {},
    /// `system.capabilities`.
    #[serde(rename = "system.capabilities")]
    SystemCapabilities {},
    /// `network.status`.
    #[serde(rename = "network.status")]
    NetworkStatus {},
    /// `network.configure`.
    #[serde(rename = "network.configure")]
    NetworkConfigure {
        /// The interface, by kernel name.
        interface: String,
        /// Its addressing.
        addressing: Addressing,
    },
    /// `wifi.scan`.
    #[serde(rename = "wifi.scan")]
    WifiScan {},
    /// `wifi.connect`.
    #[serde(rename = "wifi.connect")]
    WifiConnect {
        /// The network's name.
        ssid: String,
        /// Its passphrase; absent for an open or already known network.
        passphrase: Option<Secret>,
    },
    /// `wifi.disconnect`.
    #[serde(rename = "wifi.disconnect")]
    WifiDisconnect {},
    /// `wifi.forget`.
    #[serde(rename = "wifi.forget")]
    WifiForget {
        /// The network's name.
        ssid: String,
    },
    /// `wifi.set_radio`.
    #[serde(rename = "wifi.set_radio")]
    WifiSetRadio {
        /// On or off.
        enabled: bool,
    },
    /// `bluetooth.status`.
    #[serde(rename = "bluetooth.status")]
    BluetoothStatus {},
    /// `bluetooth.set_power`.
    #[serde(rename = "bluetooth.set_power")]
    BluetoothSetPower {
        /// On or off.
        powered: bool,
    },
    /// `bluetooth.scan`.
    #[serde(rename = "bluetooth.scan")]
    BluetoothScan {},
    /// `bluetooth.pair`.
    #[serde(rename = "bluetooth.pair")]
    BluetoothPair {
        /// The device's address.
        address: String,
    },
    /// `bluetooth.connect`.
    #[serde(rename = "bluetooth.connect")]
    BluetoothConnect {
        /// The device's address.
        address: String,
    },
    /// `bluetooth.disconnect`.
    #[serde(rename = "bluetooth.disconnect")]
    BluetoothDisconnect {
        /// The device's address.
        address: String,
    },
    /// `bluetooth.remove`.
    #[serde(rename = "bluetooth.remove")]
    BluetoothRemove {
        /// The device's address.
        address: String,
    },
    /// `audio.status`.
    #[serde(rename = "audio.status")]
    AudioStatus {},
    /// `audio.set_volume`.
    #[serde(rename = "audio.set_volume")]
    AudioSetVolume {
        /// Which side.
        target: AudioTarget,
        /// The level.
        percent: Percent,
    },
    /// `audio.set_mute`.
    #[serde(rename = "audio.set_mute")]
    AudioSetMute {
        /// Which side.
        target: AudioTarget,
        /// Muted or not.
        muted: bool,
    },
    /// `audio.set_output`.
    #[serde(rename = "audio.set_output")]
    AudioSetOutput {
        /// The output, by the identifier `audio.status` gave.
        id: u32,
    },
    /// `audio.set_input`.
    #[serde(rename = "audio.set_input")]
    AudioSetInput {
        /// The input, by the identifier `audio.status` gave.
        id: u32,
    },
    /// `display.status`.
    #[serde(rename = "display.status")]
    DisplayStatus {},
    /// `display.set_brightness`.
    #[serde(rename = "display.set_brightness")]
    DisplaySetBrightness {
        /// The level.
        percent: Percent,
    },
    /// `display.configure`.
    #[serde(rename = "display.configure")]
    DisplayConfigure {
        /// The output, by connector name.
        output: String,
        /// Width in pixels.
        width: u32,
        /// Height in pixels.
        height: u32,
        /// Scale in percent: 100 to 200 in steps of 25.
        scale_percent: u16,
    },
    /// `power.status`.
    #[serde(rename = "power.status")]
    PowerStatus {},
    /// `power.shutdown`.
    #[serde(rename = "power.shutdown")]
    PowerShutdown {},
    /// `power.restart`.
    #[serde(rename = "power.restart")]
    PowerRestart {},
    /// `power.suspend`.
    #[serde(rename = "power.suspend")]
    PowerSuspend {},
    /// `locale.status`.
    #[serde(rename = "locale.status")]
    LocaleStatus {},
    /// `locale.configure`.
    #[serde(rename = "locale.configure")]
    LocaleConfigure {
        /// The machine's language.
        language: Language,
    },
    /// `time.status`.
    #[serde(rename = "time.status")]
    TimeStatus {},
    /// `time.configure`.
    #[serde(rename = "time.configure")]
    TimeConfigure {
        /// A time zone from the system's list (`Africa/Luanda`).
        timezone: Option<String>,
        /// Whether the clock follows the network.
        automatic: Option<bool>,
    },
    /// `keyboard.status`.
    #[serde(rename = "keyboard.status")]
    KeyboardStatus {},
    /// `keyboard.configure`.
    #[serde(rename = "keyboard.configure")]
    KeyboardConfigure {
        /// An xkb layout (`pt`).
        layout: String,
        /// Its variant.
        variant: Option<String>,
    },
    /// `accessibility.status`.
    #[serde(rename = "accessibility.status")]
    AccessibilityStatus {},
    /// `accessibility.configure`.
    #[serde(rename = "accessibility.configure")]
    AccessibilityConfigure {
        /// 100, 115, 130 or 150.
        text_scale_percent: u16,
        /// High contrast.
        high_contrast: bool,
        /// Reduced motion.
        reduce_motion: bool,
    },
    /// `privacy.status`.
    #[serde(rename = "privacy.status")]
    PrivacyStatus {},
    /// `privacy.set_microphone`.
    #[serde(rename = "privacy.set_microphone")]
    PrivacySetMicrophone {
        /// Whether the microphone may be used.
        allowed: bool,
    },
    /// `privacy.set_camera`.
    #[serde(rename = "privacy.set_camera")]
    PrivacySetCamera {
        /// Whether the camera may be used.
        allowed: bool,
    },
    /// `storage.status`.
    #[serde(rename = "storage.status")]
    StorageStatus {},
    /// `update.status`.
    #[serde(rename = "update.status")]
    UpdateStatus {},
    /// `update.prepare`.
    #[serde(rename = "update.prepare")]
    UpdatePrepare {},
    /// `update.apply`.
    #[serde(rename = "update.apply")]
    UpdateApply {},
    /// `diagnostics.collect`.
    #[serde(rename = "diagnostics.collect")]
    DiagnosticsCollect {},
    /// `remote_access.status`.
    #[serde(rename = "remote_access.status")]
    RemoteAccessStatus {},
    /// `remote_access.configure`.
    #[serde(rename = "remote_access.configure")]
    RemoteAccessConfigure {
        /// Whether the SSH service is enabled.
        ssh_enabled: bool,
    },
    /// `recovery.status`.
    #[serde(rename = "recovery.status")]
    RecoveryStatus {},
    /// `activation.status`.
    #[serde(rename = "activation.status")]
    ActivationStatus {},
    /// `activation.start`: create, on this machine, the Instance it will run,
    /// from the release it carries, with its first administrator named.
    #[serde(rename = "activation.start")]
    ActivationStart {
        /// The name the Instance answers to on the network.
        domain: String,
        /// The Instance's name, as people will see it.
        instance_name: String,
        /// The distributions it starts with: at least one, none twice.
        distributions: Vec<Distribution>,
        /// The first member: the person setting the machine up.
        person_name: String,
        /// Their address.
        person_email: String,
        /// The first administrator's name.
        admin_name: String,
        /// The first administrator's address: what they sign in with.
        admin_email: String,
    },
    /// `activation.credential`: the first administrator's temporary
    /// credential. Handed over once, then gone from the machine.
    #[serde(rename = "activation.credential")]
    ActivationCredential {},
}

/// Text sizes the interface offers.
pub const TEXT_SCALES: [u16; 4] = [100, 115, 130, 150];
/// Longest SSID, in bytes.
pub const SSID_MAX: usize = 32;
/// Shortest and longest WPA passphrase.
pub const PASSPHRASE_RANGE: (usize, usize) = (8, 63);

/// A DNS name: lower-case labels of letters, digits and inner hyphens, at
/// least two of them. It becomes an argument of the release's installer, a
/// server name of the proxy and a line of the host table, so nothing that is
/// not a name gets as far as any of them.
fn host_name(s: &str) -> bool {
    s.len() <= 253
        && s.split('.').count() >= 2
        && s.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        })
}

/// A name people read: something in it, bounded, one line, and not shaped
/// like an option of the program it is handed to.
fn display_name(s: &str) -> bool {
    let n = s.chars().count();
    (1..=80).contains(&n)
        && s.trim() == s
        && !s.starts_with('-')
        && !s.chars().any(char::is_control)
}

/// An address with one `@`, a local part and a host name after it.
fn email(s: &str) -> bool {
    s.len() <= 254
        && s.split_once('@').is_some_and(|(local, host)| {
            !local.is_empty()
                && local.len() <= 64
                && !local.starts_with('-')
                && local
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b".!#$%&'*+/=?^_`{|}~-".contains(&b))
                && host_name(host)
        })
}

fn bounded_name(s: &str, max: usize, extra: &[u8]) -> bool {
    !s.is_empty()
        && s.len() <= max
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || extra.contains(&b))
}

fn ipv4(s: &str) -> bool {
    s.parse::<Ipv4Addr>().is_ok()
}

fn bluetooth_address(s: &str) -> bool {
    let parts: Vec<&str> = s.split(':').collect();
    parts.len() == 6
        && parts
            .iter()
            .all(|p| p.len() == 2 && p.bytes().all(|b| b.is_ascii_hexdigit()))
}

impl Request {
    /// The capability this request is for.
    #[must_use]
    pub const fn capability(&self) -> CapabilityId {
        use CapabilityId as C;
        match self {
            Self::SystemHealth {} => C::SystemHealth,
            Self::SystemInfo {} => C::SystemInfo,
            Self::SystemCapabilities {} => C::SystemCapabilities,
            Self::NetworkStatus {} => C::NetworkStatus,
            Self::NetworkConfigure { .. } => C::NetworkConfigure,
            Self::WifiScan {} => C::WifiScan,
            Self::WifiConnect { .. } => C::WifiConnect,
            Self::WifiDisconnect {} => C::WifiDisconnect,
            Self::WifiForget { .. } => C::WifiForget,
            Self::WifiSetRadio { .. } => C::WifiSetRadio,
            Self::BluetoothStatus {} => C::BluetoothStatus,
            Self::BluetoothSetPower { .. } => C::BluetoothSetPower,
            Self::BluetoothScan {} => C::BluetoothScan,
            Self::BluetoothPair { .. } => C::BluetoothPair,
            Self::BluetoothConnect { .. } => C::BluetoothConnect,
            Self::BluetoothDisconnect { .. } => C::BluetoothDisconnect,
            Self::BluetoothRemove { .. } => C::BluetoothRemove,
            Self::AudioStatus {} => C::AudioStatus,
            Self::AudioSetVolume { .. } => C::AudioSetVolume,
            Self::AudioSetMute { .. } => C::AudioSetMute,
            Self::AudioSetOutput { .. } => C::AudioSetOutput,
            Self::AudioSetInput { .. } => C::AudioSetInput,
            Self::DisplayStatus {} => C::DisplayStatus,
            Self::DisplaySetBrightness { .. } => C::DisplaySetBrightness,
            Self::DisplayConfigure { .. } => C::DisplayConfigure,
            Self::PowerStatus {} => C::PowerStatus,
            Self::PowerShutdown {} => C::PowerShutdown,
            Self::PowerRestart {} => C::PowerRestart,
            Self::PowerSuspend {} => C::PowerSuspend,
            Self::LocaleStatus {} => C::LocaleStatus,
            Self::LocaleConfigure { .. } => C::LocaleConfigure,
            Self::TimeStatus {} => C::TimeStatus,
            Self::TimeConfigure { .. } => C::TimeConfigure,
            Self::KeyboardStatus {} => C::KeyboardStatus,
            Self::KeyboardConfigure { .. } => C::KeyboardConfigure,
            Self::AccessibilityStatus {} => C::AccessibilityStatus,
            Self::AccessibilityConfigure { .. } => C::AccessibilityConfigure,
            Self::PrivacyStatus {} => C::PrivacyStatus,
            Self::PrivacySetMicrophone { .. } => C::PrivacySetMicrophone,
            Self::PrivacySetCamera { .. } => C::PrivacySetCamera,
            Self::StorageStatus {} => C::StorageStatus,
            Self::UpdateStatus {} => C::UpdateStatus,
            Self::UpdatePrepare {} => C::UpdatePrepare,
            Self::UpdateApply {} => C::UpdateApply,
            Self::DiagnosticsCollect {} => C::DiagnosticsCollect,
            Self::RemoteAccessStatus {} => C::RemoteAccessStatus,
            Self::RemoteAccessConfigure { .. } => C::RemoteAccessConfigure,
            Self::RecoveryStatus {} => C::RecoveryStatus,
            Self::ActivationStatus {} => C::ActivationStatus,
            Self::ActivationStart { .. } => C::ActivationStart,
            Self::ActivationCredential {} => C::ActivationCredential,
        }
    }

    /// Whether every argument is within what the capability accepts. The
    /// service calls this after authorisation and before any adapter: a
    /// request that fails here never reaches the system.
    ///
    /// # Errors
    ///
    /// [`Reason::ValueOutOfRange`] naming nothing: the caller already knows
    /// what it sent.
    pub fn validate(&self) -> Result<(), Reason> {
        let ok = match self {
            Self::NetworkConfigure {
                interface,
                addressing,
            } => {
                bounded_name(interface, 15, b"-_.")
                    && match addressing {
                        Addressing::Dhcp => true,
                        Addressing::Static {
                            address,
                            prefix,
                            gateway,
                            dns,
                        } => {
                            ipv4(address)
                                && (1..=32).contains(prefix)
                                && ipv4(gateway)
                                && dns.len() <= 4
                                && dns.iter().all(|d| ipv4(d))
                        }
                    }
            }
            Self::WifiConnect { ssid, passphrase } => {
                !ssid.is_empty()
                    && ssid.len() <= SSID_MAX
                    && !ssid.chars().any(char::is_control)
                    && passphrase.as_ref().is_none_or(|p| {
                        let n = p.expose().len();
                        (PASSPHRASE_RANGE.0..=PASSPHRASE_RANGE.1).contains(&n)
                            && !p.expose().chars().any(char::is_control)
                    })
            }
            Self::WifiForget { ssid } => {
                !ssid.is_empty() && ssid.len() <= SSID_MAX && !ssid.chars().any(char::is_control)
            }
            Self::BluetoothPair { address }
            | Self::BluetoothConnect { address }
            | Self::BluetoothDisconnect { address }
            | Self::BluetoothRemove { address } => bluetooth_address(address),
            Self::DisplayConfigure {
                output,
                width,
                height,
                scale_percent,
            } => {
                bounded_name(output, 32, b"-_")
                    && (320..=16384).contains(width)
                    && (200..=16384).contains(height)
                    && (100..=200).contains(scale_percent)
                    && scale_percent % 25 == 0
            }
            Self::TimeConfigure {
                timezone,
                automatic,
            } => {
                (timezone.is_some() || automatic.is_some())
                    && timezone.as_ref().is_none_or(|t| {
                        t.len() <= 64
                            && !t.starts_with('/')
                            && !t.contains("..")
                            && t.split('/').all(|p| bounded_name(p, 32, b"-_+"))
                    })
            }
            Self::KeyboardConfigure { layout, variant } => {
                bounded_name(layout, 16, b"_")
                    && variant.as_ref().is_none_or(|v| bounded_name(v, 32, b"-_"))
            }
            Self::AccessibilityConfigure {
                text_scale_percent, ..
            } => TEXT_SCALES.contains(text_scale_percent),
            Self::ActivationStart {
                domain,
                instance_name,
                distributions,
                person_name,
                person_email,
                admin_name,
                admin_email,
            } => {
                let mut distinct = distributions.clone();
                distinct.sort_unstable();
                distinct.dedup();
                host_name(domain)
                    && display_name(instance_name)
                    && !distributions.is_empty()
                    && distinct.len() == distributions.len()
                    && display_name(person_name)
                    && display_name(admin_name)
                    && email(person_email)
                    && email(admin_email)
            }
            _ => true,
        };
        if ok {
            Ok(())
        } else {
            Err(Reason::ValueOutOfRange)
        }
    }

    /// What an audit record may say about this request: the capability and
    /// values that are not secret. A passphrase never appears.
    #[must_use]
    pub fn audit_summary(&self) -> String {
        match self {
            Self::WifiConnect { ssid, passphrase } => {
                format!(
                    "ssid={ssid:?} passphrase={}",
                    if passphrase.is_some() {
                        "given"
                    } else {
                        "none"
                    }
                )
            }
            Self::WifiForget { ssid } => format!("ssid={ssid:?}"),
            Self::NetworkConfigure {
                interface,
                addressing,
            } => {
                let mode = match addressing {
                    Addressing::Dhcp => "DHCP",
                    Addressing::Static { .. } => "STATIC",
                };
                format!("interface={interface} mode={mode}")
            }
            Self::BluetoothPair { address } | Self::BluetoothRemove { address } => {
                format!("address={address}")
            }
            Self::RemoteAccessConfigure { ssh_enabled } => format!("ssh_enabled={ssh_enabled}"),
            Self::TimeConfigure {
                timezone,
                automatic,
            } => format!("timezone={timezone:?} automatic={automatic:?}"),
            Self::LocaleConfigure { language } => format!("language={language:?}"),
            Self::PrivacySetMicrophone { allowed } | Self::PrivacySetCamera { allowed } => {
                format!("allowed={allowed}")
            }
            // Who the first administrator is belongs in the record of the
            // machine being given an Instance; the names are institutional
            // facts, not secrets.
            Self::ActivationStart {
                domain,
                distributions,
                admin_email,
                ..
            } => format!("domain={domain} distributions={distributions:?} admin={admin_email:?}"),
            _ => String::new(),
        }
    }
}

/// What a capability returned.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum Payload {
    /// Nothing to report beyond success (a power action accepted).
    Done,
    /// Health.
    Health(SystemHealth),
    /// System facts.
    Info(SystemInfo),
    /// Inventory.
    Inventory(CapabilityInventory),
    /// Network state, read back.
    Network(NetworkState),
    /// A Wi-Fi scan.
    WifiScan(WifiScan),
    /// Bluetooth state, read back.
    Bluetooth(BluetoothState),
    /// Audio state, read back.
    Audio(AudioState),
    /// Display state, read back.
    Display(DisplayState),
    /// Power state.
    Power(PowerState),
    /// Locale state, read back.
    Locale(LocaleState),
    /// Time state, read back.
    Time(TimeState),
    /// Keyboard state, read back.
    Keyboard(KeyboardState),
    /// Accessibility state, read back.
    Accessibility(AccessibilityState),
    /// Privacy state, read back.
    Privacy(PrivacyState),
    /// Storage state.
    Storage(StorageState),
    /// Update state.
    Update(UpdateState),
    /// A diagnostics report.
    Diagnostics(Box<DiagnosticsReport>),
    /// Remote access state, read back.
    RemoteAccess(RemoteAccessState),
    /// Recovery state.
    Recovery(RecoveryState),
    /// Activation state.
    Activation(ActivationState),
    /// The first administrator's temporary credential.
    InitialCredential(InitialCredential),
}

/// The result of one call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "outcome", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Outcome {
    /// Carried out; the payload is what the system now reports.
    Ok {
        /// The state read back.
        payload: Payload,
    },
    /// Not carried out.
    Refused {
        /// Why.
        reason: Reason,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::CATALOGUE;

    fn sample(id: CapabilityId) -> Request {
        let json = match id {
            CapabilityId::NetworkConfigure => r#"{"capability":"network.configure","interface":"enp3s0","addressing":{"mode":"DHCP"}}"#.to_owned(),
            CapabilityId::WifiConnect => r#"{"capability":"wifi.connect","ssid":"Lab","passphrase":"correct horse"}"#.to_owned(),
            CapabilityId::WifiForget => r#"{"capability":"wifi.forget","ssid":"Lab"}"#.to_owned(),
            CapabilityId::WifiSetRadio => r#"{"capability":"wifi.set_radio","enabled":true}"#.to_owned(),
            CapabilityId::BluetoothSetPower => r#"{"capability":"bluetooth.set_power","powered":true}"#.to_owned(),
            CapabilityId::BluetoothPair | CapabilityId::BluetoothConnect | CapabilityId::BluetoothDisconnect | CapabilityId::BluetoothRemove => {
                format!(r#"{{"capability":"{}","address":"AA:BB:CC:DD:EE:FF"}}"#, id.as_str())
            }
            CapabilityId::AudioSetVolume => r#"{"capability":"audio.set_volume","target":"OUTPUT","percent":40}"#.to_owned(),
            CapabilityId::AudioSetMute => r#"{"capability":"audio.set_mute","target":"INPUT","muted":true}"#.to_owned(),
            CapabilityId::AudioSetOutput | CapabilityId::AudioSetInput => format!(r#"{{"capability":"{}","id":51}}"#, id.as_str()),
            CapabilityId::DisplaySetBrightness => r#"{"capability":"display.set_brightness","percent":40}"#.to_owned(),
            CapabilityId::DisplayConfigure => r#"{"capability":"display.configure","output":"eDP-1","width":1920,"height":1200,"scale_percent":125}"#.to_owned(),
            CapabilityId::LocaleConfigure => r#"{"capability":"locale.configure","language":"pt"}"#.to_owned(),
            CapabilityId::TimeConfigure => r#"{"capability":"time.configure","timezone":"Africa/Luanda","automatic":true}"#.to_owned(),
            CapabilityId::KeyboardConfigure => r#"{"capability":"keyboard.configure","layout":"pt","variant":null}"#.to_owned(),
            CapabilityId::AccessibilityConfigure => r#"{"capability":"accessibility.configure","text_scale_percent":115,"high_contrast":false,"reduce_motion":true}"#.to_owned(),
            CapabilityId::PrivacySetMicrophone | CapabilityId::PrivacySetCamera => format!(r#"{{"capability":"{}","allowed":false}}"#, id.as_str()),
            CapabilityId::RemoteAccessConfigure => r#"{"capability":"remote_access.configure","ssh_enabled":false}"#.to_owned(),
            CapabilityId::ActivationStart => r#"{"capability":"activation.start","domain":"ocinye.empresa.example","instance_name":"Instituto","distributions":["research"],"person_name":"Pessoa","person_email":"pessoa@empresa.example","admin_name":"Admin","admin_email":"admin@empresa.example"}"#.to_owned(),
            other => format!(r#"{{"capability":"{}"}}"#, other.as_str()),
        };
        serde_json::from_str(&json).unwrap_or_else(|e| panic!("{}: {e}", id.as_str()))
    }

    /// The catalogue and the request enum cannot drift: every capability has a
    /// request that parses under its own name, maps back to it, and validates.
    #[test]
    fn cada_capacidade_tem_um_pedido_com_o_seu_nome() {
        for s in CATALOGUE {
            let r = sample(s.id);
            assert_eq!(r.capability(), s.id);
            assert_eq!(r.validate(), Ok(()), "{}", s.id.as_str());
            let v = serde_json::to_value(&r).unwrap();
            assert_eq!(v["capability"], s.id.as_str());
        }
    }

    #[test]
    fn um_campo_a_mais_ou_uma_capacidade_inventada_nao_entram() {
        assert!(serde_json::from_str::<Request>(
            r#"{"capability":"power.restart","command":"reboot"}"#
        )
        .is_err());
        assert!(
            serde_json::from_str::<Request>(r#"{"capability":"system.exec","argv":["sh"]}"#)
                .is_err()
        );
        assert!(serde_json::from_str::<Request>(
            r#"{"capability":"display.set_brightness","percent":140}"#
        )
        .is_err());
    }

    #[test]
    fn valores_fora_do_intervalo_sao_recusados_antes_de_qualquer_adaptador() {
        let bad = [
            r#"{"capability":"wifi.connect","ssid":"","passphrase":null}"#,
            r#"{"capability":"wifi.connect","ssid":"Lab","passphrase":"short"}"#,
            r#"{"capability":"wifi.connect","ssid":"Lab\nx","passphrase":null}"#,
            r#"{"capability":"bluetooth.pair","address":"../../etc"}"#,
            r#"{"capability":"network.configure","interface":"eth0; rm","addressing":{"mode":"DHCP"}}"#,
            r#"{"capability":"network.configure","interface":"eth0","addressing":{"mode":"STATIC","address":"10.0.0.300","prefix":24,"gateway":"10.0.0.1","dns":[]}}"#,
            r#"{"capability":"time.configure","timezone":"../../etc/shadow","automatic":null}"#,
            r#"{"capability":"time.configure","timezone":"/etc/localtime","automatic":null}"#,
            r#"{"capability":"time.configure","timezone":null,"automatic":null}"#,
            r#"{"capability":"keyboard.configure","layout":"pt;id","variant":null}"#,
            r#"{"capability":"accessibility.configure","text_scale_percent":300,"high_contrast":false,"reduce_motion":false}"#,
            r#"{"capability":"display.configure","output":"eDP-1","width":1920,"height":1200,"scale_percent":110}"#,
        ];
        for json in bad {
            let r: Request = serde_json::from_str(json).unwrap();
            assert_eq!(r.validate(), Err(Reason::ValueOutOfRange), "{json}");
        }
    }

    #[test]
    fn a_palavra_passe_nunca_aparece_em_debug_nem_no_resumo_de_auditoria() {
        let r = sample(CapabilityId::WifiConnect);
        assert!(!format!("{r:?}").contains("correct horse"));
        assert!(!r.audit_summary().contains("correct horse"));
        assert!(r.audit_summary().contains("passphrase=given"));
    }

    fn activation(change: impl FnOnce(&mut Request)) -> Result<(), Reason> {
        let mut r = Request::ActivationStart {
            domain: "ocinye.empresa.example".into(),
            instance_name: "Instituto de Prova".into(),
            distributions: vec![Distribution::Research, Distribution::Business],
            person_name: "Pessoa de Prova".into(),
            person_email: "pessoa@empresa.example".into(),
            admin_name: "Administração".into(),
            admin_email: "admin+ocinye@empresa.example".into(),
        };
        change(&mut r);
        r.validate()
    }

    #[test]
    fn activar_so_com_nomes_que_sao_nomes_e_enderecos_que_sao_enderecos() {
        assert_eq!(activation(|_| {}), Ok(()));
        let refused: [fn(&mut Request); 12] = [
            |r| {
                if let Request::ActivationStart { domain, .. } = r {
                    *domain = "localhost".into()
                }
            },
            |r| {
                if let Request::ActivationStart { domain, .. } = r {
                    *domain = "Ocinye.Example".into()
                }
            },
            |r| {
                if let Request::ActivationStart { domain, .. } = r {
                    *domain = "a.example; reboot".into()
                }
            },
            |r| {
                if let Request::ActivationStart { domain, .. } = r {
                    *domain = "--help.example".into()
                }
            },
            |r| {
                if let Request::ActivationStart { instance_name, .. } = r {
                    *instance_name = String::new()
                }
            },
            |r| {
                if let Request::ActivationStart { instance_name, .. } = r {
                    *instance_name = "--restore".into()
                }
            },
            |r| {
                if let Request::ActivationStart { instance_name, .. } = r {
                    *instance_name = "duas\nlinhas".into()
                }
            },
            |r| {
                if let Request::ActivationStart { person_name, .. } = r {
                    *person_name = "x".repeat(81)
                }
            },
            |r| {
                if let Request::ActivationStart { distributions, .. } = r {
                    distributions.clear()
                }
            },
            |r| {
                if let Request::ActivationStart { distributions, .. } = r {
                    distributions.push(Distribution::Research)
                }
            },
            |r| {
                if let Request::ActivationStart { admin_email, .. } = r {
                    *admin_email = "sem-arroba".into()
                }
            },
            |r| {
                if let Request::ActivationStart { person_email, .. } = r {
                    *person_email = "a b@empresa.example".into()
                }
            },
        ];
        for (n, change) in refused.into_iter().enumerate() {
            assert_eq!(activation(change), Err(Reason::ValueOutOfRange), "caso {n}");
        }
    }

    #[test]
    fn o_registo_de_uma_activacao_diz_o_dominio_e_o_administrador() {
        let mut said = String::new();
        let _ = activation(|r| said = r.audit_summary());
        assert!(
            said.contains("domain=ocinye.empresa.example")
                && said.contains("admin+ocinye@empresa.example"),
            "{said}"
        );
    }
}
