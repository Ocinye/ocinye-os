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
use ocinye_image_contracts::oie::{
    authorize_installation, classify, confirm, select, InstallationTargetDisk, OieError,
    MIN_DISK_BYTES,
};
use ocinye_image_contracts::window::{WindowAction, WindowEvent, WindowStep, WINDOW_PROTOCOL};

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
    // STORAGE → REVIEW → the typed confirmation. Disks are probed again on
    // every pass; a choice is a stable path the engine itself listed.
    let mut refused = None;
    loop {
        let disks = classify(&m.disks());
        link.at(WindowStep::Storage {
            disks: disks.clone(),
            minimum_bytes: MIN_DISK_BYTES,
            refused: refused.take(),
        });
        let chosen = loop {
            match link.next() {
                None => return Outcome::Abandoned,
                Some(WindowAction::Rescan) => break None,
                Some(WindowAction::SelectDisk { by_path }) => break Some(by_path),
                // Giving up before anything was confirmed: nothing was written.
                Some(WindowAction::Restart) => {
                    m.reboot();
                    return Outcome::Abandoned;
                }
                Some(_) => {}
            }
        };
        let Some(by_path) = chosen else { continue };
        let disk = match select(&disks, &by_path) {
            Ok(d) => d,
            Err(e) => {
                refused = Some(e);
                continue;
            }
        };
        eprintln!("OCINYE-WINDOW selected={}", disk.device);
        link.at(WindowStep::Review {
            disk: disk.clone(),
            confirmation_hint: hint(disk),
            confirmation_length: u32::try_from(disk.confirmation_token.chars().count())
                .unwrap_or(0),
        });
        let target = loop {
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
        let Some(target) = target else { continue };
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
                        Some(WindowAction::AnswerKey { authorise }) => {
                            break authorise.then_some(k)
                        }
                        Some(_) => {}
                    }
                }
            }
            None => None,
        };
        // INSTALLING: everything that writes.
        link.at(WindowStep::Installing);
        let mut touched = false;
        let r = m.install(&target, key.as_ref(), &mut |line| {
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
        return match r {
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
        };
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

    use super::*;

    struct Fake {
        busy: bool,
        uefi: bool,
        unprotected: Option<&'static str>,
        fail: Option<OieError>,
        installed: RefCell<Vec<String>>,
        rebooted: Cell<u32>,
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
        fn operator_key(&self, _: &ConfirmedTarget<'_>) -> Option<PublicKeyLine> {
            None
        }
        fn install(
            &self,
            target: &ConfirmedTarget<'_>,
            _: Option<&PublicKeyLine>,
            say: &mut dyn FnMut(&str),
        ) -> Result<(), OieError> {
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

    #[test]
    fn instala_no_disco_escolhido_e_confirmado_e_em_mais_nenhum() {
        let m = Fake::new();
        let (outcome, events) = drive(
            &m,
            &[
                BEGIN,
                CONTINUE,
                VDA,
                r#"{"action":"check_confirmation","typed":"7F3"}"#,
                r#"{"action":"check_confirmation","typed":"7f3a"}"#,
                r#"{"action":"confirm","typed":"7F3A"}"#,
                RESTART,
            ],
        );
        assert_eq!(outcome, Outcome::Installed);
        assert_eq!(*m.installed.borrow(), ["/dev/disk/by-path/vda"]);
        assert_eq!(
            steps(&events),
            [
                "WELCOME",
                "COMPATIBILITY",
                "STORAGE",
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
    }

    #[test]
    fn nada_e_escrito_sem_a_confirmacao_certa_por_mais_que_a_janela_insista() {
        let m = Fake::new();
        let (outcome, events) = drive(
            &m,
            &[
                BEGIN,
                CONTINUE,
                // Actions out of turn, a made-up one, and the other disk's token.
                r#"{"action":"confirm","typed":"7F3A"}"#,
                r#"{"action":"install","by_path":"/dev/disk/by-path/vda"}"#,
                VDA,
                r#"{"action":"confirm","typed":""}"#,
                r#"{"action":"confirm","typed":"9K2M"}"#,
                r#"{"action":"confirm","typed":"OCY-TARGET-7F3A"}"#,
                CONTINUE,
                // Giving up restarts the machine; still nothing was written.
                RESTART,
            ],
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
            &[
                BEGIN,
                CONTINUE,
                r#"{"action":"select_disk","by_path":"/dev/disk/by-path/sr0"}"#,
                r#"{"action":"select_disk","by_path":"/dev/disk/by-path/vdc"}"#,
                r#"{"action":"select_disk","by_path":"/dev/sda"}"#,
            ],
        );
        assert_eq!(
            steps(&events),
            [
                "WELCOME",
                "COMPATIBILITY",
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
            &[
                BEGIN,
                CONTINUE,
                VDA,
                r#"{"action":"back"}"#,
                r#"{"action":"select_disk","by_path":"/dev/disk/by-path/vdb"}"#,
                // The first disk's confirmation does not confirm the second.
                r#"{"action":"confirm","typed":"7F3A"}"#,
                r#"{"action":"confirm","typed":"9K2M"}"#,
                RESTART,
            ],
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
            &[
                BEGIN,
                CONTINUE,
                VDA,
                r#"{"action":"confirm","typed":"7F3A"}"#,
                RESTART,
            ],
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
}
