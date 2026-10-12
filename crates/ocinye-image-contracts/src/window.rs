//! What the Installation Environment and its native window say to each other.
//!
//! The window is a client. It shows the typed state the engine sends and
//! returns typed actions; it holds no installation logic and no authority.
//! Which disks exist, which may be chosen, whether a typed confirmation is the
//! right one, and everything that writes are the engine's — the same functions
//! of [`crate::oie`] the text installer goes through. This module is only the
//! shape of the exchange: one JSON object per line, each way.

use std::net::Ipv4Addr;

use serde::{Deserialize, Serialize};

use crate::oie::{InstallationTargetDisk, OieError};

/// Version of this exchange. 2: keyboard, network and advanced settings.
pub const WINDOW_PROTOCOL: u16 = 2;

/// The keyboard layouts the installer offers: an XKB layout code and its
/// name in its own language. A closed list: the window cannot name another.
pub const KEYBOARD_LAYOUTS: &[(&str, &str)] = &[
    ("pt", "Português (Portugal)"),
    ("br", "Português (Brasil)"),
    ("us", "English (US)"),
    ("gb", "English (UK)"),
    ("fr", "Français"),
    ("es", "Español"),
    ("de", "Deutsch"),
    ("it", "Italiano"),
    ("ch", "Suisse / Schweiz"),
    ("be", "Belgique"),
];

/// The layout that goes with a boot-menu language when nobody chooses.
#[must_use]
pub fn default_layout(language: &str) -> &'static str {
    match language {
        "en" => "us",
        "fr" => "fr",
        _ => "pt",
    }
}

/// A keyboard layout, as offered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyboardLayout {
    /// XKB code.
    pub code: String,
    /// Its name.
    pub name: String,
}

/// A network interface of this machine, as the engine found it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkInterface {
    /// Kernel name (`enp1s0`).
    pub name: String,
    /// Ethernet rather than wireless.
    pub wired: bool,
    /// A cable or link is present.
    pub carrier: bool,
    /// Hardware address.
    pub mac: String,
}

/// How the installed system configures its network.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
pub enum NetworkSettings {
    /// DHCP on every wired interface (the image's default).
    Automatic,
    /// A fixed IPv4 address on one interface; the others stay unconfigured.
    Static {
        /// The interface, one the engine listed.
        interface: String,
        /// Address.
        address: Ipv4Addr,
        /// Prefix length, 1–32.
        prefix: u8,
        /// Default gateway.
        gateway: Ipv4Addr,
        /// Name servers, at most four.
        dns: Vec<Ipv4Addr>,
    },
}

/// What the installation sets on the installed system besides the disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SystemSettings {
    /// A code of [`KEYBOARD_LAYOUTS`].
    pub keyboard: String,
    /// Network.
    pub network: NetworkSettings,
    /// The machine's name; `None` keeps the image's.
    pub hostname: Option<String>,
}

/// Why settings were refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SettingsRefusal {
    /// Not a layout of the list.
    UnknownLayout,
    /// The interface is not one this machine has.
    UnknownInterface,
    /// Address, prefix, gateway or name servers are not usable.
    InvalidAddress,
    /// The name is not a valid host name.
    InvalidHostname,
}

impl SystemSettings {
    /// The settings a text installation uses: the language's layout, DHCP,
    /// the image's name.
    #[must_use]
    pub fn defaults(language: &str) -> Self {
        Self {
            keyboard: default_layout(language).to_owned(),
            network: NetworkSettings::Automatic,
            hostname: None,
        }
    }

    /// Every rule, against the interfaces this machine has.
    pub fn validate(&self, interfaces: &[NetworkInterface]) -> Result<(), SettingsRefusal> {
        if !KEYBOARD_LAYOUTS.iter().any(|(c, _)| *c == self.keyboard) {
            return Err(SettingsRefusal::UnknownLayout);
        }
        if let NetworkSettings::Static {
            interface,
            address,
            prefix,
            gateway,
            dns,
        } = &self.network
        {
            if !interfaces.iter().any(|i| &i.name == interface) {
                return Err(SettingsRefusal::UnknownInterface);
            }
            let usable = |a: &Ipv4Addr| {
                !(a.is_unspecified() || a.is_broadcast() || a.is_multicast() || a.is_loopback())
            };
            if !(1..=32).contains(prefix)
                || !usable(address)
                || !usable(gateway)
                || dns.len() > 4
                || !dns.iter().all(usable)
                || address == gateway
            {
                return Err(SettingsRefusal::InvalidAddress);
            }
            // The gateway must be on the address's own network.
            let mask = if *prefix == 32 {
                u32::MAX
            } else {
                !(u32::MAX >> prefix)
            };
            if *prefix < 31 && (u32::from(*address) & mask) != (u32::from(*gateway) & mask) {
                return Err(SettingsRefusal::InvalidAddress);
            }
        }
        if let Some(h) = &self.hostname {
            if !is_hostname(h) {
                return Err(SettingsRefusal::InvalidHostname);
            }
        }
        Ok(())
    }
}

/// RFC 1123 host name label: 1–63 of `a-z0-9-`, not starting or ending in `-`.
#[must_use]
pub fn is_hostname(h: &str) -> bool {
    (1..=63).contains(&h.len())
        && !h.starts_with('-')
        && !h.ends_with('-')
        && h.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// The fixed disk layout, shown in the advanced step: not chosen, said.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixedLayout {
    /// EFI system partition size.
    pub efi_bytes: u64,
    /// Root file system.
    pub filesystem: String,
    /// Swap.
    pub swap: bool,
}

/// What the window asks of the engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum WindowAction {
    /// Take the installation for this console and go on from the welcome.
    Begin,
    /// Go on from the compatibility summary, or from a settings step
    /// without changing it.
    Continue,
    /// Choose the installed system's keyboard layout (one the engine listed).
    ChooseKeyboard {
        /// XKB code.
        layout: String,
    },
    /// Set how the installed system configures its network.
    SetNetwork {
        /// The settings.
        settings: NetworkSettings,
    },
    /// Set the machine's name, or keep the image's (`None`).
    SetHostname {
        /// The name.
        hostname: Option<String>,
    },
    /// Look for disks again (one may have been attached).
    Rescan,
    /// Choose a disk, by its stable path — the only way a disk is named.
    SelectDisk {
        /// The stable path, as the engine listed it.
        by_path: String,
    },
    /// Go back one step (keyboard, network, storage, advanced, review).
    /// Nothing was written.
    Back,
    /// Ask whether a typed confirmation is the right one. Grants nothing.
    CheckConfirmation {
        /// What the person typed.
        typed: String,
    },
    /// Confirm the erasure of the chosen disk with the typed text. The only
    /// action that can lead to a write, and only when the engine accepts it.
    Confirm {
        /// What the person typed.
        typed: String,
    },
    /// Answer the offer of an operator key found on removable media.
    AnswerKey {
        /// Whether to authorise it.
        authorise: bool,
    },
    /// Restart the machine (after an installation, or after a failure).
    Restart,
}

/// Where the installation stands, as the engine tells the window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "step", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
pub enum WindowStep {
    /// Nothing has started. Nothing is written until a disk is confirmed.
    Welcome {
        /// The release this medium installs.
        release_id: String,
        /// Whether it is a development build.
        development: bool,
        /// The language chosen in the boot menu (`pt`, `en` or `fr`).
        language: String,
    },
    /// Another console of this machine is driving the installation.
    DrivenElsewhere,
    /// This boot is not an installation: the window has nothing to do.
    NotInstalling,
    /// What the engine found about this computer.
    Compatibility {
        /// UEFI firmware (required).
        uefi: bool,
        /// A supported architecture.
        architecture_supported: bool,
        /// Memory found.
        memory_bytes: u64,
        /// Memory the installation needs.
        memory_required: u64,
        /// Whether every disk is protected from the medium's own system.
        storage_protected: bool,
        /// Why the installation cannot go on, when it cannot.
        blocked: Option<OieError>,
    },
    /// The installed system's keyboard.
    Keyboard {
        /// What is offered.
        layouts: Vec<KeyboardLayout>,
        /// What is chosen now.
        chosen: String,
    },
    /// The installed system's network.
    Network {
        /// This machine's interfaces.
        interfaces: Vec<NetworkInterface>,
        /// What is chosen now.
        settings: Box<NetworkSettings>,
        /// Why the last change was refused, when one was.
        refused: Option<SettingsRefusal>,
    },
    /// The disks, as the engine classifies them. None is chosen.
    Storage {
        /// Every disk found, eligible or not.
        disks: Vec<InstallationTargetDisk>,
        /// The least a target must hold.
        minimum_bytes: u64,
        /// Why the last choice was refused, when one was.
        refused: Option<OieError>,
    },
    /// Optional settings after the disk: the machine's name, and the disk
    /// layout, which is fixed and said.
    Advanced {
        /// The name chosen, if any.
        hostname: Option<String>,
        /// The layout the disk will get.
        layout: FixedLayout,
        /// Why the last name was refused, when one was.
        refused: Option<SettingsRefusal>,
    },
    /// A disk is chosen and nothing is written: the last look before the
    /// typed confirmation.
    Review {
        /// The chosen disk.
        disk: Box<InstallationTargetDisk>,
        /// What the installed system will be set to.
        settings: Box<SystemSettings>,
        /// What the confirmation is the last characters of: the serial, when
        /// the disk has one, otherwise the token itself.
        confirmation_hint: String,
        /// How many characters the confirmation has.
        confirmation_length: u32,
    },
    /// An operator key was found on removable media: authorise it?
    KeyOffer {
        /// Its fingerprint.
        fingerprint: String,
    },
    /// The installation is running.
    Installing,
    /// Installed.
    Installed,
    /// The installation did not happen, or did not finish.
    Failed {
        /// Why.
        error: OieError,
        /// Whether the chosen disk may have been written to. False means the
        /// engine stopped before its first write.
        disk_touched: bool,
    },
}

/// What the engine sends.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case", deny_unknown_fields)]
pub enum WindowEvent {
    /// The exchange begins.
    Hello {
        /// [`WINDOW_PROTOCOL`].
        protocol: u16,
    },
    /// Where the installation stands now.
    Step {
        /// The step.
        state: WindowStep,
    },
    /// The answer to [`WindowAction::CheckConfirmation`], and to a
    /// [`WindowAction::Confirm`] that was not accepted.
    ConfirmationChecked {
        /// Whether the typed text is the confirmation of the chosen disk.
        valid: bool,
    },
    /// One stage of the installation ended.
    Stage {
        /// Its name (`media_check`, `partition`, `install`, `verify`, …).
        name: String,
        /// Whether it succeeded.
        ok: bool,
    },
    /// A line of the installation's own account of itself, for whoever asks
    /// to see the details.
    Detail {
        /// The line.
        line: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wired() -> Vec<NetworkInterface> {
        vec![NetworkInterface {
            name: "enp1s0".into(),
            wired: true,
            carrier: true,
            mac: "52:54:00:12:34:56".into(),
        }]
    }

    #[test]
    fn as_definicoes_so_aceitam_o_que_a_maquina_tem() {
        let ok = SystemSettings {
            keyboard: "pt".into(),
            network: NetworkSettings::Static {
                interface: "enp1s0".into(),
                address: "192.168.1.20".parse().unwrap(),
                prefix: 24,
                gateway: "192.168.1.1".parse().unwrap(),
                dns: vec!["192.168.1.1".parse().unwrap()],
            },
            hostname: Some("posto-01".into()),
        };
        assert_eq!(ok.validate(&wired()), Ok(()));
        let mut bad = ok.clone();
        bad.keyboard = "dvorak-evil".into();
        assert_eq!(bad.validate(&wired()), Err(SettingsRefusal::UnknownLayout));
        let mut bad = ok.clone();
        if let NetworkSettings::Static { interface, .. } = &mut bad.network {
            *interface = "wlan9".into();
        }
        assert_eq!(
            bad.validate(&wired()),
            Err(SettingsRefusal::UnknownInterface)
        );
        let mut bad = ok.clone();
        if let NetworkSettings::Static { gateway, .. } = &mut bad.network {
            *gateway = "10.0.0.1".parse().unwrap();
        }
        assert_eq!(bad.validate(&wired()), Err(SettingsRefusal::InvalidAddress));
        let mut bad = ok.clone();
        if let NetworkSettings::Static { prefix, .. } = &mut bad.network {
            *prefix = 0;
        }
        assert_eq!(bad.validate(&wired()), Err(SettingsRefusal::InvalidAddress));
        for name in ["", "-a", "a-", "Posto", "posto.lab", "a b", &"x".repeat(64)] {
            let mut bad = ok.clone();
            bad.hostname = Some(name.into());
            assert_eq!(
                bad.validate(&wired()),
                Err(SettingsRefusal::InvalidHostname),
                "{name}"
            );
        }
        assert_eq!(SystemSettings::defaults("fr").keyboard, "fr");
        assert_eq!(
            SystemSettings::defaults("xx").network,
            NetworkSettings::Automatic
        );
    }

    #[test]
    fn uma_accao_desconhecida_ou_com_campos_a_mais_nao_e_uma_accao() {
        assert_eq!(
            serde_json::from_str::<WindowAction>(r#"{"action":"confirm","typed":"7F3A"}"#).unwrap(),
            WindowAction::Confirm {
                typed: "7F3A".into()
            }
        );
        for refused in [
            r#"{"action":"format","device":"/dev/sda"}"#,
            r#"{"action":"confirm","typed":"7F3A","force":true}"#,
            r#"{"action":"select_disk","device":"sda"}"#,
            r#"{"action":"run","command":"dd"}"#,
            r#"{"action":"choose_keyboard","layout":"pt","command":"loadkeys"}"#,
            r#"{"action":"set_network","settings":{"mode":"SCRIPT","text":"ip a"}}"#,
            r#"{}"#,
        ] {
            assert!(
                serde_json::from_str::<WindowAction>(refused).is_err(),
                "{refused}"
            );
        }
    }

    #[test]
    fn nenhuma_accao_nomeia_um_disco_senao_pelo_caminho_estavel() {
        // The window can say which disk only one way; there is no action that
        // carries a kernel name, a size or a command.
        let json = serde_json::to_string(&WindowAction::SelectDisk {
            by_path: "/dev/disk/by-path/x".into(),
        })
        .unwrap();
        assert_eq!(
            json,
            r#"{"action":"select_disk","by_path":"/dev/disk/by-path/x"}"#
        );
    }

    #[test]
    fn um_passo_viaja_com_o_seu_nome_e_os_seus_factos() {
        let e = WindowEvent::Step {
            state: WindowStep::Welcome {
                release_id: "abc".into(),
                development: true,
                language: "pt".into(),
            },
        };
        let json = serde_json::to_string(&e).unwrap();
        assert_eq!(
            json,
            r#"{"event":"step","state":{"step":"WELCOME","release_id":"abc","development":true,"language":"pt"}}"#
        );
        assert_eq!(serde_json::from_str::<WindowEvent>(&json).unwrap(), e);
    }
}
