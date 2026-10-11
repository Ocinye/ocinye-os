//! Boot modes of the Ocinye installation medium (D013 Live Mode, L0).
//!
//! One medium, four modes, chosen by one kernel parameter and fixed for the
//! life of the boot. This module is the single source of truth for what each
//! mode is called, what it may do and what it may never do; the boot menu,
//! the initramfs guard and the OIE all derive from it.
//!
//! The rule that matters: **only `install` can ever reach the code that
//! writes a disk**, and nothing turns another mode into `install` while the
//! system runs. A missing or unknown mode is not an error to recover from by
//! guessing: it resolves to the same non-destructive policy as `live`.

use serde::{Deserialize, Serialize};

/// The kernel parameter that names the mode: `ocinye.mode=<value>`.
pub const MODE_PARAM: &str = "ocinye.mode";

/// The kernel parameter that names the interface language: `ocinye.lang=<pt|en|fr>`.
pub const LANG_PARAM: &str = "ocinye.lang";

/// Fault injection for the storage-policy *verifier* (`ocinye.selftest=policy-fail`).
/// It can only make a session more restricted, never less: the initramfs guard
/// does not read it and stays armed.
pub const SELFTEST_PARAM: &str = "ocinye.selftest";
/// The one value [`SELFTEST_PARAM`] understands.
pub const SELFTEST_POLICY_FAIL: &str = "policy-fail";

/// What casper is told by `boot=casper`; the guard arms only on the medium.
pub const CASPER_PARAM: &str = "boot=casper";

/// The marker the initramfs block guard leaves when it armed, and what it says.
pub const GUARD_MARKER: &str = "/run/ocinye/guard";
/// Content of [`GUARD_MARKER`] when the guard armed.
pub const GUARD_ARMED: &str = "armed";

/// Whether the initramfs block guard must arm for a kernel command line: in
/// every boot of the medium, the installer included (D013 L0-H). In `install`
/// the OIE releases exactly one disk after its typed confirmation; in every
/// other mode nothing is ever released. The shell script in the initramfs
/// implements exactly this, and a test runs it against this function.
///
/// It never arms outside the medium: an installed system that somehow carried
/// the guard must not lose its own disks.
#[must_use]
pub fn guard_must_arm(cmdline: &str) -> bool {
    cmdline
        .split_ascii_whitespace()
        .take_while(|a| *a != "--" && *a != "---")
        .any(|a| a == CASPER_PARAM)
}

/// Where the installer records the one disk it released from the guard: a
/// file named after the disk's kernel name. The udev rule leaves that disk
/// and its partitions writable; nothing else may create a file here.
pub const GUARD_RELEASED_DIR: &str = "/run/ocinye/released";

/// A boot mode of the medium.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MediaMode {
    /// "Try Ocinye OS": a temporary session. Never an Instance, never
    /// UNCLAIMED, never CLAIMED.
    Live,
    /// The installer: the existing OIE safety contract, unchanged.
    Install,
    /// Non-destructive hardware diagnostics.
    HardwareCheck,
    /// Text-only diagnostics. Grants no installation authority.
    Recovery,
}

/// Every mode, in menu order.
pub const ALL_MODES: [MediaMode; 4] = [
    MediaMode::Live,
    MediaMode::Install,
    MediaMode::HardwareCheck,
    MediaMode::Recovery,
];

/// What a mode guarantees about block devices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StoragePolicy {
    /// Internal block devices must not be written: forced read-only below the
    /// interface, no internal filesystem mounted, no swap, no RAID assembly,
    /// no LVM activation.
    InternalReadOnly,
    /// The OIE contract (`crate::oie`), enforced below the installer: every
    /// block device is read-only from the initramfs; nothing is mounted,
    /// swapped on, assembled or activated; after the typed destructive
    /// confirmation the confirmed target, and only it, is made writable.
    OieInstallContract,
}

/// Whether the installer engine may run in a mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OieAvailability {
    /// Read-only facts only: no selection, no confirmation, no installation.
    FactsOnly,
    /// The installer's own state machine.
    Installer,
}

/// What a front-end is expected to be in a mode (the graphical clients are
/// later phases; the text client exists in every mode).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ClientExpectation {
    /// Graphical when a display is available, text otherwise.
    GraphicalOrText,
    /// Text only, on purpose.
    TextOnly,
}

/// What happens when the mode's storage policy cannot be established.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FailurePolicy {
    /// Enter the restricted state: block-device metadata only, no filesystem
    /// inspection, never a writable fallback.
    RestrictToMetadata,
    /// The OIE's own typed errors (`crate::oie::OieError`).
    OieTypedErrors,
}

/// Things a session can be asked to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MediaAction {
    /// Read hardware facts.
    ReadHardware,
    /// Read block-device metadata (model, size, partition table).
    ReadBlockMetadata,
    /// Read filesystem signatures (type, label) without mounting.
    ProbeSignatures,
    /// Test the network.
    TestNetwork,
    /// Select an installation target.
    SelectTargetDisk,
    /// Submit the typed destructive confirmation.
    ConfirmDestructive,
    /// Write the confirmed disk.
    Install,
    /// Ask for a normal restart.
    Restart,
    /// Ask for power-off.
    PowerOff,
}

impl MediaAction {
    /// Whether the action can lead to a disk write.
    #[must_use]
    pub const fn is_destructive_path(self) -> bool {
        matches!(
            self,
            Self::SelectTargetDisk | Self::ConfirmDestructive | Self::Install
        )
    }
}

/// The contract of one mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModeContract {
    /// The mode.
    pub mode: MediaMode,
    /// The value of [`MODE_PARAM`].
    pub param_value: &'static str,
    /// Block-device policy.
    pub storage: StoragePolicy,
    /// Installer availability.
    pub oie: OieAvailability,
    /// Front-end expectation.
    pub client: ClientExpectation,
    /// When the storage policy cannot be established.
    pub failure: FailurePolicy,
    /// Kernel parameters the boot entry adds for this mode, beyond the
    /// medium's base line.
    pub extra_kernel_params: &'static [&'static str],
}

/// Kernel parameters of every non-destructive entry: belt and braces under
/// the block guard (which does not depend on them).
pub const SAFE_KERNEL_PARAMS: &[&str] = &[
    "nopersistent",
    "systemd.swap=0",
    "systemd.gpt_auto=0",
    "raid=noautodetect",
    "efi_pstore.pstore_disable=1",
];

impl MediaMode {
    /// The contract of this mode.
    #[must_use]
    pub const fn contract(self) -> ModeContract {
        match self {
            Self::Live => ModeContract {
                mode: self,
                param_value: "live",
                storage: StoragePolicy::InternalReadOnly,
                oie: OieAvailability::FactsOnly,
                client: ClientExpectation::GraphicalOrText,
                failure: FailurePolicy::RestrictToMetadata,
                extra_kernel_params: SAFE_KERNEL_PARAMS,
            },
            Self::HardwareCheck => ModeContract {
                mode: self,
                param_value: "hardware-check",
                storage: StoragePolicy::InternalReadOnly,
                oie: OieAvailability::FactsOnly,
                client: ClientExpectation::GraphicalOrText,
                failure: FailurePolicy::RestrictToMetadata,
                extra_kernel_params: SAFE_KERNEL_PARAMS,
            },
            Self::Recovery => ModeContract {
                mode: self,
                param_value: "recovery",
                storage: StoragePolicy::InternalReadOnly,
                oie: OieAvailability::FactsOnly,
                client: ClientExpectation::TextOnly,
                failure: FailurePolicy::RestrictToMetadata,
                extra_kernel_params: SAFE_KERNEL_PARAMS,
            },
            Self::Install => ModeContract {
                mode: self,
                param_value: "install",
                storage: StoragePolicy::OieInstallContract,
                oie: OieAvailability::Installer,
                client: ClientExpectation::GraphicalOrText,
                failure: FailurePolicy::OieTypedErrors,
                // The verified install line gains its mode and nothing else.
                extra_kernel_params: &[],
            },
        }
    }

    /// `ocinye.mode=<value>` for this mode.
    #[must_use]
    pub fn kernel_arg(self) -> String {
        format!("{MODE_PARAM}={}", self.contract().param_value)
    }

    /// Whether the mode allows an action. Destructive-path actions exist in
    /// `install` only.
    #[must_use]
    pub const fn allows(self, action: MediaAction) -> bool {
        match self.contract().oie {
            OieAvailability::Installer => true,
            OieAvailability::FactsOnly => !action.is_destructive_path(),
        }
    }

    fn from_param(value: &str) -> Option<Self> {
        ALL_MODES
            .into_iter()
            .find(|m| m.contract().param_value == value)
    }
}

/// Why the mode is what it is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ModeSource {
    /// The command line named a known mode exactly once.
    Explicit,
    /// No `ocinye.mode=` on the command line.
    MissingFailSafe,
    /// An unknown value.
    UnknownFailSafe {
        /// What was there.
        value: String,
    },
    /// More than one `ocinye.mode=` with different values.
    ConflictingFailSafe,
}

/// The mode of this boot. There is deliberately no way to change it: a value
/// of this type is produced once, from the kernel command line, and every
/// consumer borrows it. It serialises (for reports) but does not deserialise:
/// a mode cannot be read back from a file or a socket.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolvedMode {
    mode: MediaMode,
    source: ModeSource,
    verifier_fault: bool,
}

/// The mode a missing, unknown or contradictory command line resolves to.
pub const FAIL_SAFE_MODE: MediaMode = MediaMode::Live;

impl ResolvedMode {
    /// Resolve the mode from a kernel command line (the text of
    /// `/proc/cmdline`). Only whole, whitespace-separated arguments count, and
    /// nothing after `--`/`---` does (those are for init, not for the kernel).
    #[must_use]
    pub fn from_cmdline(cmdline: &str) -> Self {
        let prefix = format!("{MODE_PARAM}=");
        let selftest = format!("{SELFTEST_PARAM}={SELFTEST_POLICY_FAIL}");
        let mut values: Vec<&str> = vec![];
        let mut verifier_fault = false;
        for arg in cmdline.split_ascii_whitespace() {
            if arg == "--" || arg == "---" {
                break;
            }
            if let Some(v) = arg.strip_prefix(&prefix) {
                values.push(v);
            } else if arg == selftest {
                verifier_fault = true;
            }
        }
        let (mode, source) = match values.as_slice() {
            [] => (FAIL_SAFE_MODE, ModeSource::MissingFailSafe),
            [first, rest @ ..] if rest.iter().all(|v| v == first) => {
                match MediaMode::from_param(first) {
                    Some(m) => (m, ModeSource::Explicit),
                    None => (
                        FAIL_SAFE_MODE,
                        ModeSource::UnknownFailSafe {
                            value: (*first).chars().take(64).collect(),
                        },
                    ),
                }
            }
            _ => (FAIL_SAFE_MODE, ModeSource::ConflictingFailSafe),
        };
        Self {
            mode,
            source,
            verifier_fault,
        }
    }

    /// The mode.
    #[must_use]
    pub const fn mode(&self) -> MediaMode {
        self.mode
    }

    /// Why.
    #[must_use]
    pub const fn source(&self) -> &ModeSource {
        &self.source
    }

    /// Whether the mode was named explicitly (as opposed to a fail-safe).
    #[must_use]
    pub fn is_explicit(&self) -> bool {
        self.source == ModeSource::Explicit
    }

    /// Whether the storage-policy verifier was told to report failure.
    #[must_use]
    pub const fn verifier_fault_injected(&self) -> bool {
        self.verifier_fault
    }

    /// Authorise an action in this boot's mode.
    pub fn authorize(&self, action: MediaAction) -> Result<(), ModeError> {
        if self.mode.allows(action) {
            Ok(())
        } else {
            Err(ModeError::NotInstallMode {
                mode: self.mode,
                action,
            })
        }
    }
}

/// Typed refusals.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "code", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ModeError {
    /// A destructive-path action outside `install`.
    NotInstallMode {
        /// The boot's mode.
        mode: MediaMode,
        /// What was asked.
        action: MediaAction,
    },
}

/// Proof that this boot is the installer. It can be obtained from a
/// [`ResolvedMode`] and from nothing else, it borrows that mode, and it is
/// neither `Clone` nor constructible elsewhere: code that needs it to write
/// cannot be reached from a Live, Hardware Check or Recovery boot.
#[derive(Debug)]
pub struct InstallAuthority<'a> {
    _mode: &'a ResolvedMode,
}

impl<'a> InstallAuthority<'a> {
    /// `Some` only when the boot's mode is `install`, named explicitly.
    #[must_use]
    pub fn from_mode(mode: &'a ResolvedMode) -> Option<Self> {
        (mode.mode == MediaMode::Install && mode.is_explicit()).then_some(Self { _mode: mode })
    }
}

/// How live storage safety may be described to a person. The strong claim is
/// a statement about evidence, not about intent, so it is a field set by a
/// certification record and never a string chosen in code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StorageSafetyEvidence {
    /// "Internal disks are intended to remain untouched."
    Intended,
    /// "Nothing is written to internal disks." Only after the Live Storage
    /// Safety Proof (L0-S) is green for the artifact.
    Certified,
}

/// What a boot menu choice does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    /// Boot the medium's kernel in a mode.
    Boot(MediaMode),
    /// Open the Advanced / Recovery submenu.
    AdvancedSubmenu,
    /// Open the language submenu.
    LanguageSubmenu,
    /// Firmware restart (a boot-loader builtin; no kernel).
    Restart,
    /// Power off (a boot-loader builtin; no kernel).
    PowerOff,
    /// Firmware settings (a boot-loader builtin; no kernel).
    FirmwareSetup,
}

/// One choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuEntry {
    /// What it does.
    pub action: MenuAction,
    /// Hotkey (ASCII, lower case).
    pub hotkey: char,
    /// Label: pt, en, fr.
    pub label: [&'static str; 3],
}

/// The boot menu. Its two safety properties are types, not numbers that a
/// refactor could change: there is no timeout to set and no default to boot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BootMenu {
    /// Top-level choices, in order.
    pub entries: &'static [MenuEntry],
    /// Choices under Advanced / Recovery.
    pub advanced: &'static [MenuEntry],
    /// `None`: the menu waits for the operator for ever.
    pub timeout_seconds: Option<u32>,
    /// `false`: nothing boots by itself.
    pub automatic_default_boot: bool,
    /// Which top-level entry is highlighted first. Highlight is not execution.
    pub initial_focus: usize,
}

/// The normative menu.
pub const BOOT_MENU: BootMenu = BootMenu {
    entries: &[
        MenuEntry {
            action: MenuAction::Boot(MediaMode::Live),
            hotkey: 't',
            label: [
                "Experimentar o Ocinye OS",
                "Try Ocinye OS",
                "Essayer Ocinye OS",
            ],
        },
        MenuEntry {
            action: MenuAction::Boot(MediaMode::Install),
            hotkey: 'i',
            label: [
                "Instalar o Ocinye OS",
                "Install Ocinye OS",
                "Installer Ocinye OS",
            ],
        },
        MenuEntry {
            action: MenuAction::Boot(MediaMode::HardwareCheck),
            hotkey: 'h',
            label: [
                "Verificar o hardware",
                "Hardware Check",
                "Vérifier le matériel",
            ],
        },
        MenuEntry {
            action: MenuAction::AdvancedSubmenu,
            hotkey: 'a',
            label: [
                "Avançado / Recuperação",
                "Advanced / Recovery",
                "Avancé / Récupération",
            ],
        },
        MenuEntry {
            action: MenuAction::LanguageSubmenu,
            hotkey: 'l',
            label: ["Idioma", "Language", "Langue"],
        },
        MenuEntry {
            action: MenuAction::Restart,
            hotkey: 'r',
            label: ["Reiniciar", "Restart", "Redémarrer"],
        },
        MenuEntry {
            action: MenuAction::PowerOff,
            hotkey: 'p',
            label: ["Desligar", "Power Off", "Éteindre"],
        },
    ],
    advanced: &[
        MenuEntry {
            action: MenuAction::Boot(MediaMode::Recovery),
            hotkey: 'd',
            label: [
                "Diagnóstico em modo de texto (não altera discos)",
                "Text-mode diagnostics (does not change disks)",
                "Diagnostic en mode texte (ne modifie aucun disque)",
            ],
        },
        MenuEntry {
            action: MenuAction::FirmwareSetup,
            hotkey: 'f',
            label: ["Firmware (UEFI)", "Firmware (UEFI)", "Micrologiciel (UEFI)"],
        },
    ],
    timeout_seconds: None,
    automatic_default_boot: false,
    initial_focus: 0,
};

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "BOOT_IMAGE=/casper/vmlinuz boot=casper noprompt fsck.mode=skip";

    fn resolve(extra: &str) -> ResolvedMode {
        ResolvedMode::from_cmdline(&format!(
            "{BASE} {extra} console=tty0 console=ttyS0,115200n8 ---"
        ))
    }

    #[test]
    fn cada_modo_le_se_do_seu_parametro() {
        for (arg, mode) in [
            ("ocinye.mode=live", MediaMode::Live),
            ("ocinye.mode=install", MediaMode::Install),
            ("ocinye.mode=hardware-check", MediaMode::HardwareCheck),
            ("ocinye.mode=recovery", MediaMode::Recovery),
        ] {
            let r = resolve(arg);
            assert_eq!(r.mode(), mode, "{arg}");
            assert!(r.is_explicit(), "{arg}");
            assert_eq!(mode.kernel_arg(), arg);
        }
    }

    #[test]
    fn modo_em_falta_resolve_sem_autoridade_de_instalacao() {
        let r = resolve("");
        assert_eq!(r.mode(), FAIL_SAFE_MODE);
        assert_eq!(r.source(), &ModeSource::MissingFailSafe);
        assert!(InstallAuthority::from_mode(&r).is_none());
        assert_eq!(r.mode().contract().storage, StoragePolicy::InternalReadOnly);
    }

    #[test]
    fn modo_desconhecido_resolve_sem_autoridade_de_instalacao() {
        for bad in [
            "ocinye.mode=",
            "ocinye.mode=INSTALL",
            "ocinye.mode=Install",
            "ocinye.mode=install,live",
            "ocinye.mode=installer",
            "ocinye.mode=instal",
            "ocinye.mode=1",
            "ocinye.mode=live;install",
        ] {
            let r = resolve(bad);
            assert_eq!(r.mode(), FAIL_SAFE_MODE, "{bad}");
            assert!(
                matches!(r.source(), ModeSource::UnknownFailSafe { .. }),
                "{bad}"
            );
            assert!(InstallAuthority::from_mode(&r).is_none(), "{bad}");
        }
    }

    #[test]
    fn modos_contraditorios_nao_escolhem_a_instalacao() {
        for bad in [
            "ocinye.mode=live ocinye.mode=install",
            "ocinye.mode=install ocinye.mode=live",
            "ocinye.mode=install ocinye.mode=bogus",
        ] {
            let r = resolve(bad);
            assert_eq!(r.mode(), FAIL_SAFE_MODE, "{bad}");
            assert_eq!(r.source(), &ModeSource::ConflictingFailSafe, "{bad}");
            assert!(InstallAuthority::from_mode(&r).is_none(), "{bad}");
        }
        // The same mode twice is not a contradiction.
        assert_eq!(
            resolve("ocinye.mode=install ocinye.mode=install").mode(),
            MediaMode::Install
        );
    }

    #[test]
    fn so_argumentos_inteiros_do_nucleo_contam() {
        // A value hidden inside another argument, or after the `---` that
        // hands the rest to init, never selects a mode.
        for line in [
            "boot=casper xocinye.mode=install",
            "boot=casper foo=ocinye.mode=install",
            "boot=casper --- ocinye.mode=install",
            "boot=casper -- ocinye.mode=install",
        ] {
            let r = ResolvedMode::from_cmdline(line);
            assert_eq!(r.mode(), FAIL_SAFE_MODE, "{line}");
            assert!(InstallAuthority::from_mode(&r).is_none(), "{line}");
        }
    }

    #[test]
    fn so_a_instalacao_autoriza_o_caminho_destrutivo() {
        let destructive = [
            MediaAction::SelectTargetDisk,
            MediaAction::ConfirmDestructive,
            MediaAction::Install,
        ];
        let harmless = [
            MediaAction::ReadHardware,
            MediaAction::ReadBlockMetadata,
            MediaAction::ProbeSignatures,
            MediaAction::TestNetwork,
            MediaAction::Restart,
            MediaAction::PowerOff,
        ];
        for mode in ALL_MODES {
            let r = resolve(&mode.kernel_arg());
            for a in harmless {
                assert_eq!(r.authorize(a), Ok(()), "{mode:?} {a:?}");
            }
            for a in destructive {
                if mode == MediaMode::Install {
                    assert_eq!(r.authorize(a), Ok(()), "{a:?}");
                } else {
                    assert_eq!(
                        r.authorize(a),
                        Err(ModeError::NotInstallMode { mode, action: a }),
                        "{mode:?} {a:?}"
                    );
                }
            }
            assert_eq!(
                InstallAuthority::from_mode(&r).is_some(),
                mode == MediaMode::Install,
                "{mode:?}"
            );
        }
    }

    #[test]
    fn a_instalacao_mantem_o_contrato_do_oie_e_os_outros_nao_escrevem() {
        let i = MediaMode::Install.contract();
        assert_eq!(i.storage, StoragePolicy::OieInstallContract);
        assert_eq!(i.oie, OieAvailability::Installer);
        assert_eq!(i.failure, FailurePolicy::OieTypedErrors);
        // The verified install line gains its mode and nothing else.
        assert!(i.extra_kernel_params.is_empty());
        for m in [
            MediaMode::Live,
            MediaMode::HardwareCheck,
            MediaMode::Recovery,
        ] {
            let c = m.contract();
            assert_eq!(c.storage, StoragePolicy::InternalReadOnly, "{m:?}");
            assert_eq!(c.oie, OieAvailability::FactsOnly, "{m:?}");
            assert_eq!(c.failure, FailurePolicy::RestrictToMetadata, "{m:?}");
            assert_eq!(c.extra_kernel_params, SAFE_KERNEL_PARAMS, "{m:?}");
        }
        assert_eq!(
            MediaMode::Recovery.contract().client,
            ClientExpectation::TextOnly
        );
    }

    #[test]
    fn os_valores_dos_parametros_sao_unicos() {
        let mut seen = std::collections::BTreeSet::new();
        for m in ALL_MODES {
            assert!(seen.insert(m.contract().param_value), "{m:?}");
        }
    }

    #[test]
    fn o_modo_nao_muda_dentro_de_um_arranque() {
        // A mode is resolved once and only borrowed afterwards. Resolving the
        // same command line again gives the same answer, and no method takes
        // the mode by `&mut` or returns a different one: a Live boot has no
        // way to obtain the installer's authority.
        let live = resolve("ocinye.mode=live");
        let again = ResolvedMode::from_cmdline(&format!(
            "{BASE} ocinye.mode=live console=tty0 console=ttyS0,115200n8 ---"
        ));
        assert_eq!(live, again);
        assert!(InstallAuthority::from_mode(&live).is_none());
        assert_eq!(
            live.authorize(MediaAction::Install),
            Err(ModeError::NotInstallMode {
                mode: MediaMode::Live,
                action: MediaAction::Install
            })
        );
        // And a mode only serialises: there is no `Deserialize`, so no file,
        // socket or environment variable can hand a session another mode.
        let json = serde_json::to_string(&live).unwrap();
        assert!(json.contains("\"mode\":\"live\""), "{json}");
    }

    #[test]
    fn a_falha_injectada_no_verificador_nunca_da_autoridade() {
        let r = resolve("ocinye.mode=live ocinye.selftest=policy-fail");
        assert!(r.verifier_fault_injected());
        assert_eq!(r.mode(), MediaMode::Live);
        assert!(InstallAuthority::from_mode(&r).is_none());
        // Unknown self-test values are ignored.
        assert!(!resolve("ocinye.mode=live ocinye.selftest=guard-off").verifier_fault_injected());
    }

    #[test]
    fn o_menu_espera_e_nada_arranca_sozinho() {
        // Through `black_box`: these are properties of the value, checked as
        // a consumer would read them, not constants folded away.
        let menu = std::hint::black_box(BOOT_MENU);
        assert_eq!(menu.timeout_seconds, None);
        assert!(!menu.automatic_default_boot);
        // Initial focus is "Try Ocinye OS": the least dangerous thing Enter
        // can do, and still only on an explicit key press.
        assert_eq!(
            BOOT_MENU.entries[BOOT_MENU.initial_focus].action,
            MenuAction::Boot(MediaMode::Live)
        );
        assert_ne!(
            BOOT_MENU.entries[BOOT_MENU.initial_focus].action,
            MenuAction::Boot(MediaMode::Install)
        );
    }

    #[test]
    fn o_menu_tem_as_sete_escolhas_normativas() {
        let actions: Vec<MenuAction> = BOOT_MENU.entries.iter().map(|e| e.action).collect();
        assert_eq!(
            actions,
            vec![
                MenuAction::Boot(MediaMode::Live),
                MenuAction::Boot(MediaMode::Install),
                MenuAction::Boot(MediaMode::HardwareCheck),
                MenuAction::AdvancedSubmenu,
                MenuAction::LanguageSubmenu,
                MenuAction::Restart,
                MenuAction::PowerOff,
            ]
        );
        assert_eq!(
            BOOT_MENU.entries[0].label,
            [
                "Experimentar o Ocinye OS",
                "Try Ocinye OS",
                "Essayer Ocinye OS"
            ]
        );
        // Recovery is reachable, text-only, and is not the installer.
        assert!(BOOT_MENU
            .advanced
            .iter()
            .any(|e| e.action == MenuAction::Boot(MediaMode::Recovery)));
        assert!(!BOOT_MENU
            .advanced
            .iter()
            .any(|e| e.action == MenuAction::Boot(MediaMode::Install)));
        // Hotkeys are unique within each level.
        for level in [BOOT_MENU.entries, BOOT_MENU.advanced] {
            let mut keys = std::collections::BTreeSet::new();
            for e in level {
                assert!(keys.insert(e.hotkey), "{:?}", e.hotkey);
            }
        }
    }
}
