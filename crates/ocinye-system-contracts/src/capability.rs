//! The capability catalogue: every operation the Ocinye System Service offers.
//!
//! One closed table. The service, the native interface, the headless client,
//! Core proposals and the tests read the same rows; no client invents a
//! permission for a capability, and none can name one that is not here.
//!
//! Nothing in this table takes a command, a script or a path to execute, and a
//! test walks it to prove no identifier suggests otherwise.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Who may ask.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AuthorizationClass {
    /// Someone at the machine: the request comes from the local seat. Nobody
    /// needs to be signed in.
    Presence,
    /// A member with an unlocked local session.
    Member,
    /// A member whom the Core authorises for the capability's family.
    Operator,
}

/// What fresh proof the capability asks for beyond standing authority. Core
/// policy may raise it, never lower it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FreshAuthClass {
    /// Authority alone.
    None,
    /// A trusted confirmation, no credential.
    Confirm,
    /// The Session Service verifies the member again; works offline.
    LocalReauth,
    /// The Core verifies, with whatever factor its policy requires.
    CoreStepUp,
}

/// How the action is recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AuditClass {
    /// Not recorded (reads, and adjustments such as volume).
    None,
    /// A change to the machine's configuration.
    Change,
    /// Security-relevant: credentials, remote access, updates, power on
    /// someone else's behalf.
    Security,
}

/// What must exist on the machine for the capability to be offered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HardwareDependency {
    /// Nothing in particular.
    None,
    /// Any network interface.
    NetworkInterface,
    /// A Wi-Fi radio.
    WifiRadio,
    /// A Bluetooth controller.
    BluetoothController,
    /// A sound output.
    AudioOutput,
    /// A sound input.
    AudioInput,
    /// A display.
    Display,
    /// A controllable backlight.
    Backlight,
    /// A battery.
    Battery,
    /// A camera.
    Camera,
}

/// The family a capability belongs to: the unit of Core authorisation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum Family {
    /// Health, identity of the system, the inventory itself.
    System,
    /// Interfaces, addressing, Wi-Fi.
    Network,
    /// Bluetooth devices.
    Bluetooth,
    /// Sound output and input.
    Audio,
    /// Displays and brightness.
    Display,
    /// Battery, power state, restart and shutdown.
    Power,
    /// Language, region, keyboard, time.
    Locale,
    /// Accessibility preferences.
    Accessibility,
    /// Microphone, camera.
    Privacy,
    /// Disks and space.
    Storage,
    /// Updates.
    Update,
    /// Diagnostics.
    Diagnostics,
    /// Remote administration.
    RemoteAccess,
    /// Recovery information.
    Recovery,
    /// Creating, on this machine, the Instance it will run.
    Activation,
    /// The local session.
    Session,
}

/// One row of the catalogue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
pub struct CapabilitySpec {
    /// The capability.
    pub id: CapabilityId,
    /// Its family.
    pub family: Family,
    /// Who may ask.
    pub authorization: AuthorizationClass,
    /// Fresh proof required.
    pub fresh_auth: FreshAuthClass,
    /// How it is recorded.
    pub audit: AuditClass,
    /// Hardware it needs.
    pub hardware: HardwareDependency,
    /// Whether it changes anything.
    pub mutating: bool,
    /// Whether it is offered while the session is locked.
    pub while_locked: bool,
    /// Whether the headless client offers it.
    pub headless: bool,
    /// Whether Nye may propose it (through the Core; never execute it).
    pub ai_proposable: bool,
}

macro_rules! catalogue {
    ($( $variant:ident = $name:literal, $family:ident, $auth:ident, $fresh:ident, $audit:ident, $hw:ident,
        mutating $mutating:literal, locked $locked:literal, headless $headless:literal, ai $ai:literal; )+) => {
        /// A capability of the Ocinye System Service. Closed: there is no
        /// variant for "run this".
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
        pub enum CapabilityId {
            $(
                #[doc = concat!("`", $name, "`.")]
                #[serde(rename = $name)]
                $variant,
            )+
        }

        impl CapabilityId {
            /// The dotted name on the wire.
            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self { $( Self::$variant => $name, )+ }
            }
        }

        /// Every capability, in catalogue order.
        pub const CATALOGUE: &[CapabilitySpec] = &[
            $( CapabilitySpec {
                id: CapabilityId::$variant,
                family: Family::$family,
                authorization: AuthorizationClass::$auth,
                fresh_auth: FreshAuthClass::$fresh,
                audit: AuditClass::$audit,
                hardware: HardwareDependency::$hw,
                mutating: $mutating,
                while_locked: $locked,
                headless: $headless,
                ai_proposable: $ai,
            }, )+
        ];
    };
}

catalogue! {
    SystemHealth        = "system.health",          System,       Presence, None,        None,     None,                mutating false, locked true,  headless true,  ai true;
    SystemInfo          = "system.info",            System,       Presence, None,        None,     None,                mutating false, locked true,  headless true,  ai true;
    SystemCapabilities  = "system.capabilities",    System,       Presence, None,        None,     None,                mutating false, locked true,  headless true,  ai true;
    NetworkStatus       = "network.status",         Network,      Presence, None,        None,     NetworkInterface,    mutating false, locked true,  headless true,  ai true;
    NetworkConfigure    = "network.configure",      Network,      Operator, CoreStepUp,  Security, NetworkInterface,    mutating true,  locked false, headless true,  ai false;
    WifiScan            = "wifi.scan",              Network,      Presence, None,        None,     WifiRadio,           mutating false, locked false, headless true,  ai true;
    WifiConnect         = "wifi.connect",           Network,      Member,   None,        Security, WifiRadio,           mutating true,  locked false, headless true,  ai false;
    WifiDisconnect      = "wifi.disconnect",        Network,      Member,   None,        Change,   WifiRadio,           mutating true,  locked false, headless true,  ai true;
    WifiForget          = "wifi.forget",            Network,      Member,   Confirm,     Security, WifiRadio,           mutating true,  locked false, headless true,  ai false;
    WifiSetRadio        = "wifi.set_radio",         Network,      Member,   None,        Change,   WifiRadio,           mutating true,  locked false, headless true,  ai true;
    BluetoothStatus     = "bluetooth.status",       Bluetooth,    Presence, None,        None,     BluetoothController, mutating false, locked false, headless true,  ai true;
    BluetoothSetPower   = "bluetooth.set_power",    Bluetooth,    Member,   None,        Change,   BluetoothController, mutating true,  locked false, headless true,  ai true;
    BluetoothScan       = "bluetooth.scan",         Bluetooth,    Member,   None,        None,     BluetoothController, mutating false, locked false, headless true,  ai false;
    BluetoothPair       = "bluetooth.pair",         Bluetooth,    Member,   Confirm,     Security, BluetoothController, mutating true,  locked false, headless false, ai false;
    BluetoothConnect    = "bluetooth.connect",      Bluetooth,    Member,   None,        Change,   BluetoothController, mutating true,  locked false, headless true,  ai true;
    BluetoothDisconnect = "bluetooth.disconnect",   Bluetooth,    Member,   None,        Change,   BluetoothController, mutating true,  locked false, headless true,  ai true;
    BluetoothRemove     = "bluetooth.remove",       Bluetooth,    Member,   Confirm,     Security, BluetoothController, mutating true,  locked false, headless true,  ai false;
    AudioStatus         = "audio.status",           Audio,        Presence, None,        None,     AudioOutput,         mutating false, locked true,  headless false, ai true;
    AudioSetVolume      = "audio.set_volume",       Audio,        Presence, None,        None,     AudioOutput,         mutating true,  locked true,  headless false, ai true;
    AudioSetMute        = "audio.set_mute",         Audio,        Presence, None,        None,     AudioOutput,         mutating true,  locked true,  headless false, ai true;
    AudioSetOutput      = "audio.set_output",       Audio,        Member,   None,        None,     AudioOutput,         mutating true,  locked false, headless false, ai true;
    AudioSetInput       = "audio.set_input",        Audio,        Member,   None,        None,     AudioInput,          mutating true,  locked false, headless false, ai true;
    DisplayStatus       = "display.status",         Display,      Presence, None,        None,     Display,             mutating false, locked true,  headless false, ai true;
    DisplaySetBrightness = "display.set_brightness", Display,     Presence, None,        None,     Backlight,           mutating true,  locked true,  headless true,  ai true;
    DisplayConfigure    = "display.configure",      Display,      Member,   Confirm,     None,     Display,             mutating true,  locked false, headless false, ai false;
    PowerStatus         = "power.status",           Power,        Presence, None,        None,     None,                mutating false, locked true,  headless true,  ai true;
    PowerShutdown       = "power.shutdown",         Power,        Presence, Confirm,     Security, None,                mutating true,  locked true,  headless true,  ai false;
    PowerRestart        = "power.restart",          Power,        Presence, Confirm,     Security, None,                mutating true,  locked true,  headless true,  ai false;
    PowerSuspend        = "power.suspend",          Power,        Presence, None,        Change,   None,                mutating true,  locked true,  headless true,  ai false;
    LocaleStatus        = "locale.status",          Locale,       Presence, None,        None,     None,                mutating false, locked true,  headless true,  ai true;
    LocaleConfigure     = "locale.configure",       Locale,       Operator, None,        Change,   None,                mutating true,  locked false, headless true,  ai false;
    TimeStatus          = "time.status",            Locale,       Presence, None,        None,     None,                mutating false, locked true,  headless true,  ai true;
    TimeConfigure       = "time.configure",         Locale,       Operator, CoreStepUp,  Security, None,                mutating true,  locked false, headless true,  ai false;
    KeyboardStatus      = "keyboard.status",        Locale,       Presence, None,        None,     None,                mutating false, locked true,  headless true,  ai true;
    KeyboardConfigure   = "keyboard.configure",     Locale,       Presence, None,        Change,   None,                mutating true,  locked true,  headless true,  ai false;
    AccessibilityStatus = "accessibility.status",   Accessibility, Presence, None,       None,     None,                mutating false, locked true,  headless false, ai true;
    AccessibilityConfigure = "accessibility.configure", Accessibility, Presence, None,   None,     None,                mutating true,  locked true,  headless false, ai true;
    PrivacyStatus       = "privacy.status",         Privacy,      Presence, None,        None,     None,                mutating false, locked false, headless true,  ai true;
    PrivacySetMicrophone = "privacy.set_microphone", Privacy,     Member,   None,        Security, AudioInput,          mutating true,  locked false, headless true,  ai false;
    PrivacySetCamera    = "privacy.set_camera",     Privacy,      Member,   None,        Security, Camera,              mutating true,  locked false, headless true,  ai false;
    StorageStatus       = "storage.status",         Storage,      Member,   None,        None,     None,                mutating false, locked false, headless true,  ai true;
    UpdateStatus        = "update.status",          Update,       Presence, None,        None,     None,                mutating false, locked false, headless true,  ai true;
    UpdateCheck         = "update.check",           Update,       Member,   None,        None,     None,                mutating false, locked false, headless true,  ai true;
    UpdateDownload      = "update.download",        Update,       Operator, None,        Security, None,                mutating true,  locked false, headless true,  ai false;
    UpdateInstall       = "update.install",         Update,       Operator, CoreStepUp,  Security, None,                mutating true,  locked false, headless true,  ai false;
    UpdateCancel        = "update.cancel",          Update,       Operator, None,        Change,   None,                mutating true,  locked false, headless true,  ai false;
    UpdateHistory       = "update.history",         Update,       Member,   None,        None,     None,                mutating false, locked false, headless true,  ai true;
    UpdateRollback      = "update.rollback",        Update,       Operator, CoreStepUp,  Security, None,                mutating true,  locked false, headless true,  ai false;
    UpdateConfigure     = "update.configure",       Update,       Operator, CoreStepUp,  Security, None,                mutating true,  locked false, headless true,  ai false;
    UpdateImport        = "update.import",          Update,       Operator, None,        Security, None,                mutating true,  locked false, headless true,  ai false;
    BaseStatus          = "base.status",            Update,       Member,   None,        None,     None,                mutating false, locked false, headless true,  ai true;
    BaseCheck           = "base.check",             Update,       Operator, None,        Change,   None,                mutating true,  locked false, headless true,  ai false;
    BaseApply           = "base.apply",             Update,       Operator, CoreStepUp,  Security, None,                mutating true,  locked false, headless true,  ai false;
    DiagnosticsCollect  = "diagnostics.collect",    Diagnostics,  Operator, None,        Security, None,                mutating false, locked false, headless true,  ai false;
    RemoteAccessStatus  = "remote_access.status",   RemoteAccess, Member,   None,        None,     None,                mutating false, locked false, headless true,  ai true;
    RemoteAccessConfigure = "remote_access.configure", RemoteAccess, Operator, CoreStepUp, Security, None,              mutating true,  locked false, headless true,  ai false;
    RecoveryStatus      = "recovery.status",        Recovery,     Presence, None,        None,     None,                mutating false, locked true,  headless true,  ai true;
    ActivationStatus    = "activation.status",      Activation,   Presence, None,        None,     None,                mutating false, locked true,  headless true,  ai false;
    ActivationStart     = "activation.start",       Activation,   Presence, None,        Security, None,                mutating true,  locked true,  headless true,  ai false;
    ActivationCredential = "activation.credential", Activation,   Presence, None,        Security, None,                mutating true,  locked true,  headless true,  ai false;
}

/// The row for a capability.
#[must_use]
pub fn spec(id: CapabilityId) -> &'static CapabilitySpec {
    CATALOGUE
        .iter()
        .find(|s| s.id == id)
        .unwrap_or_else(|| unreachable!("the macro lists every variant in CATALOGUE"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn nomes_unicos_e_com_a_forma_familia_ponto_accao() {
        let mut seen = HashSet::new();
        for s in CATALOGUE {
            let name = s.id.as_str();
            assert!(seen.insert(name), "duplicado: {name}");
            let (family, action) = name.split_once('.').expect("família.acção");
            assert!(!family.is_empty() && !action.is_empty());
            assert!(
                name.bytes()
                    .all(|b| b.is_ascii_lowercase() || b == b'.' || b == b'_'),
                "{name}"
            );
        }
    }

    /// The constitutional rule, measured: no capability is a way to run
    /// something the caller names.
    #[test]
    fn nenhuma_capacidade_e_um_executor_generico() {
        const FORBIDDEN: &[&str] = &[
            "exec", "shell", "command", "run", "sudo", "script", "eval", "spawn", "root",
        ];
        for s in CATALOGUE {
            for word in s.id.as_str().split(['.', '_']) {
                assert!(
                    !FORBIDDEN.contains(&word),
                    "{} sugere execução genérica",
                    s.id.as_str()
                );
            }
        }
    }

    #[test]
    fn uma_leitura_nunca_pede_prova_fresca_e_uma_mudanca_de_seguranca_e_sempre_registada() {
        for s in CATALOGUE {
            if !s.mutating {
                assert_eq!(s.fresh_auth, FreshAuthClass::None, "{}", s.id.as_str());
            }
            if s.fresh_auth == FreshAuthClass::CoreStepUp {
                assert_eq!(s.audit, AuditClass::Security, "{}", s.id.as_str());
                assert_eq!(
                    s.authorization,
                    AuthorizationClass::Operator,
                    "{}",
                    s.id.as_str()
                );
            }
        }
    }

    #[test]
    fn com_a_sessao_bloqueada_so_ha_o_que_a_presenca_pode() {
        for s in CATALOGUE.iter().filter(|s| s.while_locked) {
            assert_eq!(
                s.authorization,
                AuthorizationClass::Presence,
                "{}",
                s.id.as_str()
            );
        }
    }

    #[test]
    fn o_nye_nunca_propoe_o_que_pede_prova_ou_toca_em_credenciais() {
        for s in CATALOGUE.iter().filter(|s| s.ai_proposable) {
            assert_eq!(s.fresh_auth, FreshAuthClass::None, "{}", s.id.as_str());
            assert_ne!(s.audit, AuditClass::Security, "{}", s.id.as_str());
            assert_ne!(
                s.authorization,
                AuthorizationClass::Operator,
                "{}",
                s.id.as_str()
            );
        }
    }

    #[test]
    fn o_nome_serializado_e_o_da_tabela() {
        for s in CATALOGUE {
            let json = serde_json::to_string(&s.id).unwrap();
            assert_eq!(json, format!("\"{}\"", s.id.as_str()));
            let back: CapabilityId = serde_json::from_str(&json).unwrap();
            assert_eq!(back, s.id);
        }
        assert!(serde_json::from_str::<CapabilityId>("\"system.exec\"").is_err());
    }
}
