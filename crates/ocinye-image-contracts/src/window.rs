//! What the Installation Environment and its native window say to each other.
//!
//! The window is a client. It shows the typed state the engine sends and
//! returns typed actions; it holds no installation logic and no authority.
//! Which disks exist, which may be chosen, whether a typed confirmation is the
//! right one, and everything that writes are the engine's — the same functions
//! of [`crate::oie`] the text installer goes through. This module is only the
//! shape of the exchange: one JSON object per line, each way.

use serde::{Deserialize, Serialize};

use crate::oie::{InstallationTargetDisk, OieError};

/// Version of this exchange.
pub const WINDOW_PROTOCOL: u16 = 1;

/// What the window asks of the engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum WindowAction {
    /// Take the installation for this console and go on from the welcome.
    Begin,
    /// Go on from the compatibility summary.
    Continue,
    /// Look for disks again (one may have been attached).
    Rescan,
    /// Choose a disk, by its stable path — the only way a disk is named.
    SelectDisk {
        /// The stable path, as the engine listed it.
        by_path: String,
    },
    /// Go back from the review to the list of disks. Nothing was written.
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
    /// The disks, as the engine classifies them. None is chosen.
    Storage {
        /// Every disk found, eligible or not.
        disks: Vec<InstallationTargetDisk>,
        /// The least a target must hold.
        minimum_bytes: u64,
        /// Why the last choice was refused, when one was.
        refused: Option<OieError>,
    },
    /// A disk is chosen and nothing is written: the last look before the
    /// typed confirmation.
    Review {
        /// The chosen disk.
        disk: InstallationTargetDisk,
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
