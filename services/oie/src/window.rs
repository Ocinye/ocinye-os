//! The installer, for the native window.
//!
//! A second host of the same engine, beside the text one (`flow`). Both go
//! through the same [`Machine`], the same classification of disks, the same
//! [`authorize_installation`] — the only producer of write authority — and
//! the same `install`. What differs is the exchange: here it is typed
//! ([`ocinye_image_contracts::window`]), so that the window can draw an
//! interface instead of printing a conversation. Nothing the window sends is
//! trusted to be true: a disk is whatever `classify` says it is now, and a
//! confirmation is right only when the engine's own comparison says so.

use std::io::{BufRead, Write};

use ocinye_image_contracts::bootmode::InstallAuthority;
use ocinye_image_contracts::bootmode::MediaMode;
use ocinye_image_contracts::oie::{
    authorize_installation, classify, confirm, select, InstallationTargetDisk, OieError,
    MIN_DISK_BYTES,
};
use ocinye_image_contracts::window::{
    FixedLayout, HardwareReport, KeyboardLayout, MediaCompatibility, SystemSettings, WindowAction,
    WindowEvent, WindowStep, KEYBOARD_LAYOUTS, WINDOW_PROTOCOL,
};

use crate::flow::{Machine, Outcome, MIN_OIE_RAM};

/// The window's connection: events out, actions in, one JSON object per line.
pub struct Link<'a> {
    pub input: &'a mut dyn BufRead,
    pub out: &'a mut dyn Write,
}

impl Link<'_> {
    fn send(&mut self, event: &WindowEvent) {
        if let Ok(line) = serde_json::to_string(event) {
            let _ = writeln!(self.out, "{line}");
            let _ = self.out.flush();
        }
    }

    fn at(&mut self, state: WindowStep) {
        // What the machine's own console log says of the window: which step
        // it is at. Never an argument, never what was typed.
        let name = serde_json::to_value(&state)
            .ok()
            .and_then(|v| v["step"].as_str().map(str::to_owned))
            .unwrap_or_default();
        eprintln!("OCINYE-WINDOW step={name}");
        self.send(&WindowEvent::Step { state });
    }

    /// The next action; `None` when the window is gone. A line that is not an
    /// action is not answered and not acted on.
    fn next(&mut self) -> Option<WindowAction> {
        loop {
            let mut line = String::new();
            match self.input.read_line(&mut line) {
                Ok(0) | Err(_) => return None,
                Ok(_) => {}
            }
            if let Ok(action) = serde_json::from_str::<WindowAction>(line.trim()) {
                return Some(action);
            }
        }
    }
}

/// What the confirmation is the last characters of.
fn hint(disk: &InstallationTargetDisk) -> String {
    disk.serial
        .clone()
        .unwrap_or_else(|| disk.confirmation_token.clone())
}

/// What a non-destructive session of the window needs from the machine.
/// There is nothing here that selects, writes or mounts a disk.
pub trait MediaMachine {
    /// Read the computer again.
    fn report(&self) -> HardwareReport;
    /// The storage policy could not be proven.
    fn restricted(&self) -> bool;
    /// A supported architecture.
    fn architecture_supported(&self) -> bool;
    /// A normal firmware restart.
    fn restart(&self);
    /// Power off.
    fn poweroff(&self);
}

/// How a non-destructive window session ended.
#[derive(Debug, PartialEq, Eq)]
pub enum MediaOutcome {
    /// The window went away.
    Abandoned,
    /// A restart was asked for and requested.
    Restarting,
    /// A power-off was asked for and requested.
    PoweringOff,
}

/// Live, Hardware Check or Recovery, for the window: the report, read again
/// on request, and the two ways out. Every other action is ignored.
pub fn media(
    link: &mut Link<'_>,
    m: &dyn MediaMachine,
    mode: MediaMode,
    release_id: &str,
    development: bool,
    language: &str,
) -> MediaOutcome {
    link.send(&WindowEvent::Hello {
        protocol: WINDOW_PROTOCOL,
    });
    loop {
        let report = m.report();
        let compatibility =
            MediaCompatibility::of(&report, m.architecture_supported(), MIN_OIE_RAM);
        link.at(WindowStep::Media {
            mode,
            release_id: release_id.to_owned(),
            development,
            language: language.to_owned(),
            restricted: m.restricted(),
            report: Box::new(report),
            compatibility,
        });
        loop {
            match link.next() {
                None => return MediaOutcome::Abandoned,
                Some(WindowAction::Rescan) => break,
                Some(WindowAction::Restart) => {
                    m.restart();
                    return MediaOutcome::Restarting;
                }
                Some(WindowAction::PowerOff) => {
                    m.poweroff();
                    return MediaOutcome::PoweringOff;
                }
                Some(_) => {}
            }
        }
    }
}

/// The installer. As in the text host, it can only be entered with an
/// [`InstallAuthority`].
pub fn run(
    link: &mut Link<'_>,
    m: &dyn Machine,
    authority: &InstallAuthority<'_>,
    release_id: &str,
    development: bool,
    language: &str,
) -> Outcome {
    link.send(&WindowEvent::Hello {
        protocol: WINDOW_PROTOCOL,
    });
    // WELCOME: the first console to go on drives the installation.
    link.at(WindowStep::Welcome {
        release_id: release_id.to_owned(),
        development,
        language: language.to_owned(),
    });
    loop {
        match link.next() {
            None => return Outcome::Abandoned,
            Some(WindowAction::Begin) if m.take_console() => break,
            Some(WindowAction::Begin) => link.at(WindowStep::DrivenElsewhere),
            Some(_) => {}
        }
    }
    // COMPATIBILITY: the same four questions, in the same order.
    let memory = m.memory();
    let protected = m.storage_protected();
    let blocked = if !m.uefi() {
        Some(OieError::UnsupportedFirmware)
    } else if !m.arch_supported() {
        Some(OieError::UnsupportedArchitecture)
    } else if memory < MIN_OIE_RAM {
        Some(OieError::InsufficientMemory {
            bytes: memory,
            required: MIN_OIE_RAM,
        })
    } else {
        protected
            .as_ref()
            .err()
            .map(|reason| OieError::StorageProtectionUnverified {
                reason: reason.clone(),
            })
    };
    link.at(WindowStep::Compatibility {
        uefi: m.uefi(),
        architecture_supported: m.arch_supported(),
        memory_bytes: memory,
        memory_required: MIN_OIE_RAM,
        storage_protected: protected.is_ok(),
        blocked: blocked.clone(),
    });
    loop {
        match (link.next(), &blocked) {
            (None, _) => return Outcome::Abandoned,
            // Nothing is offered for installation on a machine that cannot
            // take it, or whose disks are not protected.
            (Some(WindowAction::Restart), Some(e)) => {
                m.reboot();
                return Outcome::Failed(e.clone());
            }
            (Some(WindowAction::Continue), None) => break,
            _ => {}
        }
    }
    // KEYBOARD → NETWORK → STORAGE → ADVANCED → REVIEW. Settings are the
    // engine's to validate; nothing is written until the confirmation, and
    // Back from any step changes nothing on any disk.
    let layouts: Vec<KeyboardLayout> = KEYBOARD_LAYOUTS
        .iter()
        .map(|(code, name)| KeyboardLayout {
            code: (*code).to_owned(),
            name: (*name).to_owned(),
        })
        .collect();
    let interfaces = m.interfaces();
    let mut settings = SystemSettings::defaults(language);
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum At {
        Keyboard,
        Network,
        Storage,
        Advanced,
        Review,
    }
    let mut at = At::Keyboard;
    let mut refused = None;
    let mut settings_refused = None;
    let mut chosen_path: Option<String> = None;
    // The disks the confirmation is checked against; the write authority
    // borrows from them, so they outlive the conversation.
    let mut reviewed: Vec<InstallationTargetDisk>;
    let target = loop {
        match at {
            At::Keyboard => {
                link.at(WindowStep::Keyboard {
                    layouts: layouts.clone(),
                    chosen: settings.keyboard.clone(),
                });
                match link.next() {
                    None => return Outcome::Abandoned,
                    Some(WindowAction::ChooseKeyboard { layout }) => {
                        let mut s = settings.clone();
                        s.keyboard = layout;
                        if s.validate(&interfaces).is_ok() {
                            settings = s;
                        }
                    }
                    Some(WindowAction::Continue) => at = At::Network,
                    Some(WindowAction::Restart) => {
                        m.reboot();
                        return Outcome::Abandoned;
                    }
                    Some(_) => {}
                }
            }
            At::Network => {
                link.at(WindowStep::Network {
                    interfaces: interfaces.clone(),
                    settings: Box::new(settings.network.clone()),
                    refused: settings_refused.take(),
                });
                match link.next() {
                    None => return Outcome::Abandoned,
                    Some(WindowAction::SetNetwork { settings: network }) => {
                        let mut s = settings.clone();
                        s.network = network;
                        match s.validate(&interfaces) {
                            Ok(()) => settings = s,
                            Err(e) => settings_refused = Some(e),
                        }
                    }
                    Some(WindowAction::Continue) => at = At::Storage,
                    Some(WindowAction::Back) => at = At::Keyboard,
                    Some(WindowAction::Restart) => {
                        m.reboot();
                        return Outcome::Abandoned;
                    }
                    Some(_) => {}
                }
            }
            At::Storage => {
                // Disks are probed again on every pass; a choice is a stable
                // path the engine itself listed.
                let disks = classify(&m.disks());
                link.at(WindowStep::Storage {
                    disks: disks.clone(),
                    minimum_bytes: MIN_DISK_BYTES,
                    refused: refused.take(),
                });
                match link.next() {
                    None => return Outcome::Abandoned,
                    Some(WindowAction::Rescan) => {}
                    Some(WindowAction::Back) => at = At::Network,
                    Some(WindowAction::SelectDisk { by_path }) => match select(&disks, &by_path) {
                        Ok(d) => {
                            eprintln!("OCINYE-WINDOW selected={}", d.device);
                            chosen_path = Some(by_path);
                            at = At::Advanced;
                        }
                        Err(e) => refused = Some(e),
                    },
                    // Giving up before anything was confirmed: nothing was written.
                    Some(WindowAction::Restart) => {
                        m.reboot();
                        return Outcome::Abandoned;
                    }
                    Some(_) => {}
                }
            }
            At::Advanced => {
                link.at(WindowStep::Advanced {
                    hostname: settings.hostname.clone(),
                    layout: FixedLayout {
                        efi_bytes: 1 << 30,
                        filesystem: "ext4".into(),
                        swap: false,
                    },
                    refused: settings_refused.take(),
                });
                match link.next() {
                    None => return Outcome::Abandoned,
                    Some(WindowAction::SetHostname { hostname }) => {
                        let mut s = settings.clone();
                        s.hostname = hostname.filter(|h| !h.is_empty());
                        match s.validate(&interfaces) {
                            Ok(()) => settings = s,
                            Err(e) => settings_refused = Some(e),
                        }
                    }
                    Some(WindowAction::Continue) => at = At::Review,
                    Some(WindowAction::Back) => at = At::Storage,
                    Some(WindowAction::Restart) => {
                        m.reboot();
                        return Outcome::Abandoned;
                    }
                    Some(_) => {}
                }
            }
            At::Review => {
                // The disk is chosen again from a fresh probe: what was
                // listed a moment ago must still be there, and the same.
                reviewed = classify(&m.disks());
                let Some(disk) = chosen_path
                    .as_deref()
                    .and_then(|p| select(&reviewed, p).ok())
                else {
                    refused = None;
                    at = At::Storage;
                    continue;
                };
                link.at(WindowStep::Review {
                    disk: Box::new(disk.clone()),
                    settings: Box::new(settings.clone()),
                    confirmation_hint: hint(disk),
                    confirmation_length: u32::try_from(disk.confirmation_token.chars().count())
                        .unwrap_or(0),
                });
                let confirmed = loop {
                    match link.next() {
                        None => return Outcome::Abandoned,
                        Some(WindowAction::Back) => break None,
                        Some(WindowAction::Restart) => {
                            m.reboot();
                            return Outcome::Abandoned;
                        }
                        // The window may ask as often as it likes whether what was
                        // typed is right; the answer grants nothing.
                        Some(WindowAction::CheckConfirmation { typed }) => {
                            link.send(&WindowEvent::ConfirmationChecked {
                                valid: confirm(disk, &typed).is_ok(),
                            });
                        }
                        // The typed confirmation is what grants write authority over
                        // this disk, and over no other.
                        Some(WindowAction::Confirm { typed }) => {
                            match authorize_installation(authority, disk, &typed) {
                                Ok(target) => break Some(target),
                                Err(_) => {
                                    eprintln!("OCINYE-WINDOW confirmation=refused");
                                    link.send(&WindowEvent::ConfirmationChecked { valid: false });
                                }
                            }
                        }
                        Some(_) => {}
                    }
                };
                match confirmed {
                    Some(t) => break t,
                    None => at = At::Advanced,
                }
            }
        }
    };
    eprintln!("OCINYE-WINDOW confirmation=accepted");
    // OPERATOR KEY: looked for on removable media, never on the target.
    let key = match m.operator_key(&target) {
        Some(k) => {
            link.at(WindowStep::KeyOffer {
                fingerprint: k.fingerprint().0,
            });
            loop {
                match link.next() {
                    None => return Outcome::Abandoned,
                    Some(WindowAction::AnswerKey { authorise }) => break authorise.then_some(k),
                    Some(_) => {}
                }
            }
        }
        None => None,
    };
    // INSTALLING: everything that writes.
    link.at(WindowStep::Installing);
    let mut touched = false;
    let r = m.install(&target, key.as_ref(), &settings, &mut |line| {
        if let Some((name, ok)) = stage(line) {
            // `install` is the stage that partitions and writes: once it
            // is reported, in either sense, the disk is no longer as it was.
            touched |= name == "install";
            link.send(&WindowEvent::Stage {
                name: name.to_owned(),
                ok,
            });
        }
        link.send(&WindowEvent::Detail {
            line: line.trim().to_owned(),
        });
    });
    match r {
        Ok(()) => {
            link.at(WindowStep::Installed);
            loop {
                match link.next() {
                    None | Some(WindowAction::Restart) => break,
                    Some(_) => {}
                }
            }
            m.reboot();
            Outcome::Installed
        }
        Err(error) => {
            link.at(WindowStep::Failed {
                error: error.clone(),
                disk_touched: touched,
            });
            loop {
                match link.next() {
                    None => break,
                    Some(WindowAction::Restart) => {
                        m.reboot();
                        break;
                    }
                    Some(_) => {}
                }
            }
            Outcome::Failed(error)
        }
    }
}

/// A stage report of the installation — `  [OK] verify`, `  [DISK_WRITE_FAILED]
/// install` — as its name and whether it succeeded.
fn stage(line: &str) -> Option<(&str, bool)> {
    let rest = line.trim().strip_prefix('[')?;
    let (result, name) = rest.split_once("] ")?;
    let plain = !name.is_empty() && name.bytes().all(|b| b.is_ascii_lowercase() || b == b'_');
    plain.then_some((name, result == "OK"))
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::io::BufReader;

    use ocinye_image_contracts::bootmode::ResolvedMode;
    use ocinye_image_contracts::firstboot::PublicKeyLine;
    use ocinye_image_contracts::oie::{ConfirmedTarget, ProbedDisk};
    use ocinye_image_contracts::window::{NetworkInterface, NetworkSettings};

    use super::*;

    struct Fake {
        busy: bool,
        uefi: bool,
        unprotected: Option<&'static str>,
        fail: Option<OieError>,
        installed: RefCell<Vec<String>>,
        rebooted: Cell<u32>,
        settings: RefCell<Option<SystemSettings>>,
    }

    impl Fake {
        fn new() -> Self {
            Self {
                busy: false,
                uefi: true,
                unprotected: None,
                fail: None,
                installed: RefCell::new(vec![]),
                rebooted: Cell::new(0),
                settings: RefCell::new(None),
            }
        }
    }

    impl Machine for Fake {
        fn take_console(&self) -> bool {
            !self.busy
        }
        fn uefi(&self) -> bool {
            self.uefi
        }
        fn arch_supported(&self) -> bool {
            true
        }
        fn memory(&self) -> u64 {
            8 << 30
        }
        fn disks(&self) -> Vec<ProbedDisk> {
            let d = |dev: &str, serial: Option<&str>, media: bool, bytes: u64| ProbedDisk {
                device: dev.into(),
                by_path: format!("/dev/disk/by-path/{dev}"),
                bytes,
                model: "QEMU".into(),
                serial: serial.map(Into::into),
                wwn: None,
                partitions: 0,
                filesystems: vec![],
                boot_media: media,
                mounted: false,
                read_only: false,
            };
            vec![
                d("sr0", Some("cd"), true, 5_000_000_000),
                d("vda", Some("OCY-TARGET-7F3A"), false, 40_000_000_000),
                d("vdb", Some("OCY-OTHER-9K2M"), false, 40_000_000_000),
                d("vdc", Some("SMALL-0001"), false, 8_000_000_000),
            ]
        }
        fn interfaces(&self) -> Vec<NetworkInterface> {
            vec![NetworkInterface {
                name: "enp1s0".into(),
                wired: true,
                carrier: true,
                mac: "52:54:00:12:34:56".into(),
            }]
        }
        fn operator_key(&self, _: &ConfirmedTarget<'_>) -> Option<PublicKeyLine> {
            None
        }
        fn install(
            &self,
            target: &ConfirmedTarget<'_>,
            _: Option<&PublicKeyLine>,
            settings: &SystemSettings,
            say: &mut dyn FnMut(&str),
        ) -> Result<(), OieError> {
            self.settings.replace(Some(settings.clone()));
            say("  [OK] media_check");
            if let Some(e) = &self.fail {
                say("  [DISK_WRITE_FAILED] install");
                return Err(e.clone());
            }
            self.installed
                .borrow_mut()
                .push(target.disk().by_path.clone());
            say("  [OK] install");
            say("  [OK] verify");
            Ok(())
        }
        fn reboot(&self) {
            self.rebooted.set(self.rebooted.get() + 1);
        }
        fn storage_protected(&self) -> Result<(), String> {
            self.unprotected.map_or(Ok(()), |r| Err(r.to_owned()))
        }
    }

    /// Run the installer against these actions; the outcome and every event.
    fn drive(m: &Fake, actions: &[&str]) -> (Outcome, Vec<serde_json::Value>) {
        let boot = ResolvedMode::from_cmdline("ocinye.mode=install");
        let authority = InstallAuthority::from_mode(&boot).expect("install mode");
        let text: String = actions.iter().map(|a| format!("{a}\n")).collect();
        let mut input = BufReader::new(text.as_bytes());
        let mut out = Vec::new();
        let outcome = run(
            &mut Link {
                input: &mut input,
                out: &mut out,
            },
            m,
            &authority,
            "f3fc6ba0863b",
            true,
            "pt",
        );
        let events = String::from_utf8(out)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        (outcome, events)
    }

    fn steps(events: &[serde_json::Value]) -> Vec<String> {
        events
            .iter()
            .filter(|e| e["event"] == "step")
            .map(|e| e["state"]["step"].as_str().unwrap().to_owned())
            .collect()
    }

    const BEGIN: &str = r#"{"action":"begin"}"#;
    const CONTINUE: &str = r#"{"action":"continue"}"#;
    const VDA: &str = r#"{"action":"select_disk","by_path":"/dev/disk/by-path/vda"}"#;
    const RESTART: &str = r#"{"action":"restart"}"#;

    /// From the welcome to the disks with the default settings.
    const TO_STORAGE: [&str; 4] = [BEGIN, CONTINUE, CONTINUE, CONTINUE];

    fn seq(parts: &[&[&'static str]]) -> Vec<&'static str> {
        parts.iter().flat_map(|p| p.iter().copied()).collect()
    }

    #[test]
    fn instala_no_disco_escolhido_e_confirmado_e_em_mais_nenhum() {
        let m = Fake::new();
        let (outcome, events) = drive(
            &m,
            &seq(&[
                &TO_STORAGE,
                &[
                    VDA,
                    CONTINUE,
                    r#"{"action":"check_confirmation","typed":"7F3"}"#,
                    r#"{"action":"check_confirmation","typed":"7f3a"}"#,
                    r#"{"action":"confirm","typed":"7F3A"}"#,
                    RESTART,
                ],
            ]),
        );
        assert_eq!(outcome, Outcome::Installed);
        assert_eq!(*m.installed.borrow(), ["/dev/disk/by-path/vda"]);
        assert_eq!(
            steps(&events),
            [
                "WELCOME",
                "COMPATIBILITY",
                "KEYBOARD",
                "NETWORK",
                "STORAGE",
                "ADVANCED",
                "REVIEW",
                "INSTALLING",
                "INSTALLED"
            ]
        );
        let checks: Vec<bool> = events
            .iter()
            .filter(|e| e["event"] == "confirmation_checked")
            .map(|e| e["valid"].as_bool().unwrap())
            .collect();
        assert_eq!(checks, [false, true]);
        assert_eq!(m.rebooted.get(), 1);
        // The defaults of a Portuguese boot: pt keyboard, DHCP, the image's name.
        assert_eq!(
            m.settings.borrow().clone().unwrap(),
            SystemSettings::defaults("pt")
        );
    }

    #[test]
    fn as_escolhas_da_janela_chegam_a_instalacao_e_as_invalidas_nao() {
        let m = Fake::new();
        let (outcome, events) = drive(
            &m,
            &[
                BEGIN,
                CONTINUE,
                r#"{"action":"choose_keyboard","layout":"klingon"}"#,
                r#"{"action":"choose_keyboard","layout":"fr"}"#,
                CONTINUE,
                // An interface this machine does not have, then a gateway off
                // the network: refused, and the step says so.
                r#"{"action":"set_network","settings":{"mode":"STATIC","interface":"eth9","address":"192.168.1.20","prefix":24,"gateway":"192.168.1.1","dns":[]}}"#,
                r#"{"action":"set_network","settings":{"mode":"STATIC","interface":"enp1s0","address":"192.168.1.20","prefix":24,"gateway":"10.0.0.1","dns":[]}}"#,
                r#"{"action":"set_network","settings":{"mode":"STATIC","interface":"enp1s0","address":"192.168.1.20","prefix":24,"gateway":"192.168.1.1","dns":["192.168.1.1"]}}"#,
                CONTINUE,
                VDA,
                r#"{"action":"set_hostname","hostname":"Posto 1"}"#,
                r#"{"action":"set_hostname","hostname":"posto-1"}"#,
                CONTINUE,
                r#"{"action":"confirm","typed":"7F3A"}"#,
                RESTART,
            ],
        );
        assert_eq!(outcome, Outcome::Installed);
        let got = m.settings.borrow().clone().unwrap();
        assert_eq!(got.keyboard, "fr");
        assert_eq!(got.hostname.as_deref(), Some("posto-1"));
        assert!(
            matches!(got.network, NetworkSettings::Static { ref interface, .. } if interface == "enp1s0")
        );
        let refusals: Vec<&str> = events
            .iter()
            .filter(|e| !e["state"]["refused"].is_null() && e["state"]["refused"].is_string())
            .map(|e| e["state"]["refused"].as_str().unwrap())
            .collect();
        assert_eq!(
            refusals,
            ["UNKNOWN_INTERFACE", "INVALID_ADDRESS", "INVALID_HOSTNAME"]
        );
        let review = events
            .iter()
            .find(|e| e["state"]["step"] == "REVIEW")
            .unwrap();
        assert_eq!(review["state"]["settings"]["keyboard"], "fr");
    }

    #[test]
    fn voltar_atras_percorre_os_passos_sem_escrever_nada() {
        let m = Fake::new();
        let back = r#"{"action":"back"}"#;
        let (outcome, events) = drive(
            &m,
            &seq(&[
                &TO_STORAGE,
                &[
                    back, back, CONTINUE, CONTINUE, VDA, back, VDA, CONTINUE, back, CONTINUE,
                    RESTART,
                ],
            ]),
        );
        assert_eq!(outcome, Outcome::Abandoned);
        assert!(m.installed.borrow().is_empty());
        assert_eq!(
            steps(&events),
            [
                "WELCOME",
                "COMPATIBILITY",
                "KEYBOARD",
                "NETWORK",
                "STORAGE",
                "NETWORK",
                "KEYBOARD",
                "NETWORK",
                "STORAGE",
                "ADVANCED",
                "STORAGE",
                "ADVANCED",
                "REVIEW",
                "ADVANCED",
                "REVIEW"
            ]
        );
    }

    #[test]
    fn nada_e_escrito_sem_a_confirmacao_certa_por_mais_que_a_janela_insista() {
        let m = Fake::new();
        let (outcome, events) = drive(
            &m,
            &seq(&[
                &[BEGIN, CONTINUE],
                // Actions out of turn and a made-up one, at the settings steps.
                &[
                    r#"{"action":"confirm","typed":"7F3A"}"#,
                    r#"{"action":"install","by_path":"/dev/disk/by-path/vda"}"#,
                    CONTINUE,
                    r#"{"action":"confirm","typed":"7F3A"}"#,
                    CONTINUE,
                    VDA,
                    CONTINUE,
                    r#"{"action":"confirm","typed":""}"#,
                    r#"{"action":"confirm","typed":"9K2M"}"#,
                    r#"{"action":"confirm","typed":"OCY-TARGET-7F3A"}"#,
                    CONTINUE,
                    // Giving up restarts the machine; still nothing was written.
                    RESTART,
                ],
            ]),
        );
        assert_eq!(outcome, Outcome::Abandoned);
        assert!(m.installed.borrow().is_empty());
        assert!(!steps(&events).contains(&"INSTALLING".to_owned()));
        assert_eq!(m.rebooted.get(), 1);
    }

    #[test]
    fn o_suporte_um_disco_pequeno_e_um_caminho_inventado_nao_se_escolhem() {
        let m = Fake::new();
        let (_, events) = drive(
            &m,
            &seq(&[
                &TO_STORAGE,
                &[
                    r#"{"action":"select_disk","by_path":"/dev/disk/by-path/sr0"}"#,
                    r#"{"action":"select_disk","by_path":"/dev/disk/by-path/vdc"}"#,
                    r#"{"action":"select_disk","by_path":"/dev/sda"}"#,
                ],
            ]),
        );
        assert_eq!(
            steps(&events),
            [
                "WELCOME",
                "COMPATIBILITY",
                "KEYBOARD",
                "NETWORK",
                "STORAGE",
                "STORAGE",
                "STORAGE",
                "STORAGE"
            ]
        );
        let refusals: Vec<&str> = events
            .iter()
            .filter(|e| e["state"]["step"] == "STORAGE" && !e["state"]["refused"].is_null())
            .map(|e| e["state"]["refused"]["code"].as_str().unwrap())
            .collect();
        assert_eq!(
            refusals,
            ["DISK_PROTECTED", "DISK_PROTECTED", "AMBIGUOUS_DISK"]
        );
        assert!(m.installed.borrow().is_empty());
    }

    #[test]
    fn voltar_atras_larga_o_disco_e_outro_pode_ser_escolhido() {
        let m = Fake::new();
        let (outcome, _) = drive(
            &m,
            &seq(&[
                &TO_STORAGE,
                &[
                    VDA,
                    CONTINUE,
                    r#"{"action":"back"}"#,
                    r#"{"action":"back"}"#,
                    r#"{"action":"select_disk","by_path":"/dev/disk/by-path/vdb"}"#,
                    CONTINUE,
                    // The first disk's confirmation does not confirm the second.
                    r#"{"action":"confirm","typed":"7F3A"}"#,
                    r#"{"action":"confirm","typed":"9K2M"}"#,
                    RESTART,
                ],
            ]),
        );
        assert_eq!(outcome, Outcome::Installed);
        assert_eq!(*m.installed.borrow(), ["/dev/disk/by-path/vdb"]);
    }

    #[test]
    fn sem_proteccao_dos_discos_ou_sem_uefi_nao_se_chega_aos_discos() {
        for (uefi, unprotected, code) in [
            (true, Some("swap_active"), "STORAGE_PROTECTION_UNVERIFIED"),
            (false, None, "UNSUPPORTED_FIRMWARE"),
        ] {
            let mut m = Fake::new();
            m.uefi = uefi;
            m.unprotected = unprotected;
            let (outcome, events) = drive(&m, &[BEGIN, CONTINUE, VDA, RESTART]);
            assert!(matches!(outcome, Outcome::Failed(_)), "{code}");
            assert_eq!(steps(&events), ["WELCOME", "COMPATIBILITY"]);
            assert_eq!(events.last().unwrap()["state"]["blocked"]["code"], code);
            assert!(m.installed.borrow().is_empty());
        }
    }

    #[test]
    fn a_instalacao_pertence_a_primeira_consola_que_continua() {
        let mut m = Fake::new();
        m.busy = true;
        let (outcome, events) = drive(&m, &[BEGIN, BEGIN]);
        assert_eq!(outcome, Outcome::Abandoned);
        assert_eq!(
            steps(&events),
            ["WELCOME", "DRIVEN_ELSEWHERE", "DRIVEN_ELSEWHERE"]
        );
    }

    #[test]
    fn uma_falha_diz_o_codigo_e_se_o_disco_ja_foi_tocado() {
        let mut m = Fake::new();
        m.fail = Some(OieError::DiskWriteFailed {
            device: "vda".into(),
        });
        let (outcome, events) = drive(
            &m,
            &seq(&[
                &TO_STORAGE,
                &[
                    VDA,
                    CONTINUE,
                    r#"{"action":"confirm","typed":"7F3A"}"#,
                    RESTART,
                ],
            ]),
        );
        assert!(matches!(
            outcome,
            Outcome::Failed(OieError::DiskWriteFailed { .. })
        ));
        let failed = events
            .iter()
            .find(|e| e["state"]["step"] == "FAILED")
            .unwrap();
        assert_eq!(failed["state"]["error"]["code"], "DISK_WRITE_FAILED");
        assert_eq!(failed["state"]["disk_touched"], true);
        let stages: Vec<(&str, bool)> = events
            .iter()
            .filter(|e| e["event"] == "stage")
            .map(|e| (e["name"].as_str().unwrap(), e["ok"].as_bool().unwrap()))
            .collect();
        assert_eq!(stages, [("media_check", true), ("install", false)]);
    }

    #[test]
    fn um_relato_de_etapa_le_se_e_o_resto_e_so_detalhe() {
        assert_eq!(stage("  [OK] verify"), Some(("verify", true)));
        assert_eq!(
            stage("  [DISK_WRITE_FAILED] first_boot"),
            Some(("first_boot", false))
        );
        assert_eq!(stage("curtin: [OK] something else"), None);
        assert_eq!(stage("  [OK] Not A Stage"), None);
    }

    struct Media {
        reads: Cell<u32>,
        restarted: Cell<bool>,
        off: Cell<bool>,
    }

    impl MediaMachine for Media {
        fn report(&self) -> HardwareReport {
            self.reads.set(self.reads.get() + 1);
            HardwareReport {
                arch: "x86_64".into(),
                machine: None,
                cpu_model: Some("CPU".into()),
                cpus: 4,
                memory_bytes: 8 << 30,
                uefi: true,
                secure_boot: None,
                firmware: None,
                disks: vec![
                    ocinye_image_contracts::window::MediaDisk {
                        device: "sr0".into(),
                        bytes: 3_000_000_000,
                        model: "QEMU DVD-ROM".into(),
                        connection: "sata".into(),
                        boot_media: true,
                    },
                    ocinye_image_contracts::window::MediaDisk {
                        device: "vda".into(),
                        bytes: 64_000_000_000,
                        model: String::new(),
                        connection: String::new(),
                        boot_media: false,
                    },
                ],
                network: vec![],
                wifi_radios: None,
                bluetooth_controllers: Some(0),
                displays: Some(vec!["Virtual-1".into()]),
                audio: None,
            }
        }
        fn restricted(&self) -> bool {
            false
        }
        fn architecture_supported(&self) -> bool {
            true
        }
        fn restart(&self) {
            self.restarted.set(true);
        }
        fn poweroff(&self) {
            self.off.set(true);
        }
    }

    fn media_drive(m: &Media, actions: &[&str]) -> (MediaOutcome, Vec<serde_json::Value>) {
        let script: String = actions.iter().map(|a| format!("{a}\n")).collect();
        let mut input = BufReader::new(script.as_bytes());
        let mut out = Vec::new();
        let r = media(
            &mut Link {
                input: &mut input,
                out: &mut out,
            },
            m,
            MediaMode::HardwareCheck,
            "f3fc6ba0863b-r1",
            true,
            "pt",
        );
        let events = String::from_utf8(out)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        (r, events)
    }

    #[test]
    fn a_sessao_de_hardware_le_de_novo_e_so_sai_por_reiniciar_ou_desligar() {
        let m = Media {
            reads: Cell::new(0),
            restarted: Cell::new(false),
            off: Cell::new(false),
        };
        // Nothing an installation would send does anything here.
        let (r, ev) = media_drive(
            &m,
            &[
                r#"{"action":"begin"}"#,
                r#"{"action":"select_disk","by_path":"pci-0000:00:04.0"}"#,
                r#"{"action":"confirm","typed":"vda"}"#,
                r#"{"action":"rescan"}"#,
                r#"{"action":"restart"}"#,
            ],
        );
        assert_eq!(r, MediaOutcome::Restarting);
        assert!(m.restarted.get() && !m.off.get());
        assert_eq!(m.reads.get(), 2, "read once, and again on rescan");
        let steps: Vec<_> = ev.iter().filter(|e| e["event"] == "step").collect();
        assert_eq!(steps.len(), 2);
        let st = &steps[0]["state"];
        assert_eq!(st["step"], "MEDIA");
        assert_eq!(st["mode"], "hardware-check");
        // Unknown stays unknown on the wire.
        assert!(st["report"]["wifi_radios"].is_null());
        assert_eq!(st["report"]["bluetooth_controllers"], 0);
        // The medium is not counted as a place to install.
        assert_eq!(st["compatibility"]["large_enough_disks"], 1);

        let m = Media {
            reads: Cell::new(0),
            restarted: Cell::new(false),
            off: Cell::new(false),
        };
        let (r, _) = media_drive(&m, &[r#"{"action":"power_off"}"#]);
        assert_eq!(r, MediaOutcome::PoweringOff);
        assert!(m.off.get() && !m.restarted.get());
        let (r, _) = media_drive(&m, &[]);
        assert_eq!(r, MediaOutcome::Abandoned);
    }
}
