//! The OIE screens, as a line-driven flow over any terminal (VT or serial).
//!
//! `WELCOME → COMPAT → DISK_SELECT → (DISK_AMBIGUOUS) → DISK_CONFIRM →
//! OPERATOR_KEY → INSTALLING → COMPLETE`. The machine is behind [`Machine`],
//! so the guard that matters — nothing is written before the typed
//! confirmation — is tested with scripted input.

use std::io::{BufRead, Write};

use ocinye_image_contracts::bootmode::InstallAuthority;
use ocinye_image_contracts::firstboot::PublicKeyLine;
use ocinye_image_contracts::oie::{
    authorize_installation, classify, select, ConfirmedTarget, DiskProtection, OieError,
    ProbedDisk, MIN_DISK_BYTES,
};

use crate::strings::STRINGS;

/// Memory the installation environment needs.
pub const MIN_OIE_RAM: u64 = 2 << 30;

pub trait Machine {
    /// Take the installation for this console (the screen and the serial
    /// line both show the welcome; the first to continue drives it).
    fn take_console(&self) -> bool;
    fn uefi(&self) -> bool;
    fn arch_supported(&self) -> bool;
    fn memory(&self) -> u64;
    fn disks(&self) -> Vec<ProbedDisk>;
    /// Look for an operator key on pluggable volumes, never on the target.
    fn operator_key(&self, target: &ConfirmedTarget<'_>) -> Option<PublicKeyLine>;
    /// Everything that writes: only called after confirmation.
    /// It takes a [`ConfirmedTarget`]: the write authority over one disk,
    /// which only the typed confirmation produces.
    fn install(
        &self,
        target: &ConfirmedTarget<'_>,
        key: Option<&PublicKeyLine>,
        say: &mut dyn FnMut(&str),
    ) -> Result<(), OieError>;
    fn reboot(&self);
    /// Whether the installer's storage protection is in force: every block
    /// device read-only, no swap, no array, no volume, nothing mounted from a
    /// disk. `Err` carries a stable reason code.
    fn storage_protected(&self) -> Result<(), String>;
}

pub struct Ui<'a> {
    pub lang: usize,
    pub serial: bool,
    pub input: &'a mut dyn BufRead,
    pub out: &'a mut dyn Write,
}

fn fold(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' => 'a',
            'Á' | 'À' | 'Â' | 'Ã' => 'A',
            'é' | 'ê' | 'è' => 'e',
            'É' | 'Ê' => 'E',
            'í' => 'i',
            'ó' | 'ô' | 'õ' => 'o',
            'Ó' => 'O',
            'ú' => 'u',
            'ç' => 'c',
            'Ç' => 'C',
            '«' | '»' | '“' | '”' => '"',
            '’' => '\'',
            '·' | '—' | '–' => '-',
            c if c.is_ascii() => c,
            _ => '?',
        })
        .collect()
}

impl Ui<'_> {
    pub fn t(&self, key: &str) -> String {
        STRINGS
            .iter()
            .find(|(k, _)| *k == key)
            .map_or_else(|| key.to_owned(), |(_, v)| v[self.lang].to_owned())
    }

    fn tf(&self, key: &str, args: &[(&str, String)]) -> String {
        args.iter()
            .fold(self.t(key), |s, (k, v)| s.replace(&format!("{{{k}}}"), v))
    }

    pub fn say(&mut self, s: &str) {
        let s = if self.serial { fold(s) } else { s.to_owned() };
        let _ = write!(self.out, "{s}\r\n");
        let _ = self.out.flush();
    }

    fn title(&mut self, key: &str) {
        let t = self.t(key);
        let oie = self.t("c13.oie");
        self.say("");
        self.say(&format!("== Ocinye OS · {oie} · {t}"));
    }

    /// One line of input; `None` at end of input.
    pub fn ask(&mut self, prompt: &str) -> Option<String> {
        let p = if self.serial {
            fold(prompt)
        } else {
            prompt.to_owned()
        };
        let _ = write!(self.out, "{p} > ");
        let _ = self.out.flush();
        let mut line = String::new();
        match self.input.read_line(&mut line) {
            Ok(0) | Err(_) => None,
            Ok(_) => Some(line.trim().to_owned()),
        }
    }
}

fn gb(bytes: u64) -> String {
    format!("{:.1} GB", bytes as f64 / 1e9)
}

/// How the flow ended.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Installed,
    Failed(OieError),
    /// Input ended (console closed) before anything was written.
    Abandoned,
}

/// The installer. It can only be entered with an [`InstallAuthority`], which
/// exists only in a boot whose mode is `install`: a Live, Hardware Check or
/// Recovery session has no value to pass here.
pub fn run(ui: &mut Ui<'_>, m: &dyn Machine, authority: &InstallAuthority<'_>) -> Outcome {
    // WELCOME
    ui.title("c13.welcome.t");
    let b = ui.t("c13.welcome.b");
    ui.say(&b);
    let n = ui.t("c13.welcome.nothing");
    ui.say(&n);
    loop {
        let k = ui.t("c13.k.continue");
        let l = ui.t("c13.k.lang");
        match ui.ask(&format!("[Enter] {k} · [L] {l}")).as_deref() {
            None => return Outcome::Abandoned,
            Some("l" | "L") => ui.lang = (ui.lang + 1) % 3,
            Some(_) if m.take_console() => break,
            Some(_) => ui.say("Ocinye OS: instalação em curso noutra consola · installation in progress on another console"),
        }
    }
    // COMPAT
    ui.title("c13.compat.t");
    if !m.uefi() {
        return Outcome::Failed(OieError::UnsupportedFirmware);
    }
    if !m.arch_supported() {
        return Outcome::Failed(OieError::UnsupportedArchitecture);
    }
    let mem = m.memory();
    let ram = ui.t("c13.compat.ram");
    ui.say(&format!("  {ram}: {}", gb(mem)));
    if mem < MIN_OIE_RAM {
        let msg = ui.tf("c13.e.hwB", &[("v", gb(mem)), ("m", gb(MIN_OIE_RAM))]);
        ui.say(&msg);
        return Outcome::Failed(OieError::InsufficientMemory {
            bytes: mem,
            required: MIN_OIE_RAM,
        });
    }
    let gpu = format!("  {}: {}", ui.t("c13.compat.gpu"), ui.t("c13.compat.gpuV"));
    ui.say(&gpu);
    // Nothing is offered for installation unless every disk is protected.
    if let Err(reason) = m.storage_protected() {
        ui.say(&format!(
            "Ocinye OS: a protecção dos discos não está em vigor ({reason}); nada é instalado · disk protection is not in force ({reason}); nothing is installed"
        ));
        return Outcome::Failed(OieError::StorageProtectionUnverified { reason });
    }
    // DISK_SELECT (re-probed on every pass: a disk may be attached)
    let disk = loop {
        let disks = classify(&m.disks());
        ui.title("c13.disk.t");
        let b = ui.t("c13.disk.b");
        ui.say(&b);
        let h = ui.t("c13.disk.h");
        ui.say(&format!("   {h}"));
        for (i, d) in disks.iter().enumerate() {
            let state = match d.protection {
                DiskProtection::Eligible => format!("[{}]", i + 1),
                DiskProtection::Ambiguous => format!("[{}]?", i + 1),
                _ => " - ".into(),
            };
            let contents = if d.partitions == 0 {
                ui.t("c13.disk.empty")
            } else {
                ui.tf(
                    "c13.disk.parts",
                    &[
                        ("n", d.partitions.to_string()),
                        ("fs", d.filesystems.join(", ")),
                    ],
                )
            };
            let note = match d.protection {
                DiskProtection::InstallationMedia => ui.t("c13.disk.media"),
                DiskProtection::Mounted => ui.t("c13.disk.mounted"),
                DiskProtection::TooSmall => ui.tf("c13.disk.small", &[("m", gb(MIN_DISK_BYTES))]),
                DiskProtection::ReadOnly => "read-only".into(),
                _ => contents,
            };
            ui.say(&format!(
                "{state:<5}{:<12}{:<9}{:<22}{:<10}{note}",
                d.device,
                gb(d.bytes),
                d.model.chars().take(21).collect::<String>(),
                d.serial.as_deref().unwrap_or("—")
            ));
            ui.say(&format!(
                "       {}: {}",
                ui.t("c13.disk.path"),
                if d.by_path.is_empty() {
                    "—"
                } else {
                    &d.by_path
                }
            ));
        }
        let legend = ui.tf("c13.disk.legend", &[("m", gb(MIN_DISK_BYTES))]);
        ui.say(&legend);
        if !disks.iter().any(|d| d.protection.selectable()) {
            let t = ui.t("c13.e.nodisk");
            ui.say(&t);
            let r = ui.t("c13.k.retry");
            match ui.ask(&format!("[Enter] {r}")) {
                None => return Outcome::Abandoned,
                Some(_) => continue,
            }
        }
        let s = ui.t("c13.k.select");
        let Some(answer) = ui.ask(&format!("{s} (1–{})", disks.len())) else {
            return Outcome::Abandoned;
        };
        let Some(chosen) = answer
            .parse::<usize>()
            .ok()
            .and_then(|n| n.checked_sub(1))
            .and_then(|i| disks.get(i))
        else {
            continue;
        };
        match select(&disks, &chosen.by_path) {
            Ok(d) => {
                if d.protection == DiskProtection::Ambiguous {
                    let t = ui.t("c13.disk.ambT");
                    ui.say(&t);
                    let b = ui.t("c13.disk.ambB");
                    ui.say(&b);
                }
                break d.clone();
            }
            Err(_) => continue,
        }
    };
    // DISK_CONFIRM — the only gate before writing.
    ui.title("c13.confirm.t");
    let t = ui.tf(
        "c13.confirm.t",
        &[(
            "d",
            format!("{} ({}, {})", disk.device, gb(disk.bytes), disk.by_path),
        )],
    );
    ui.say(&t);
    let b = ui.t("c13.confirm.b");
    ui.say(&b);
    ui.say(&format!(
        "  {}",
        if disk.partitions == 0 {
            ui.t("c13.disk.empty")
        } else {
            disk.filesystems.join(", ")
        }
    ));
    let layout = ui.t("c13.confirm.layout");
    ui.say(&layout);
    let hint = disk
        .serial
        .as_deref()
        .map_or_else(|| disk.confirmation_token.clone(), |s| s.to_owned());
    let prompt = ui.tf("c13.confirm.type", &[("s", hint)]);
    let Some(typed) = ui.ask(&prompt) else {
        return Outcome::Abandoned;
    };
    // The typed confirmation is what grants write authority over this disk,
    // and over no other.
    let Ok(target) = authorize_installation(authority, &disk, &typed) else {
        let e = ui.t("c13.err.disk");
        ui.say(&e);
        return Outcome::Failed(OieError::ConfirmationMismatch);
    };
    // OPERATOR_KEY
    ui.title("c13.key.t");
    let b = ui.t("c13.key.b");
    ui.say(&b);
    let only = ui.t("c13.key.only");
    ui.say(&only);
    let key = match m.operator_key(&target) {
        Some(k) => {
            let f = ui.tf("c13.key.found", &[("fp", k.fingerprint().0)]);
            ui.say(&f);
            let u = ui.t("c13.k.useKey");
            let sk = ui.t("c13.k.skip");
            match ui.ask(&format!("[S] {u} · [Enter] {sk}")).as_deref() {
                None => return Outcome::Abandoned,
                Some("s" | "S") => Some(k),
                Some(_) => None,
            }
        }
        None => None,
    };
    // INSTALLING
    ui.title("c13.inst.t");
    let note = ui.t("c13.inst.note");
    ui.say(&note);
    let mut lines = vec![];
    let r = m.install(&target, key.as_ref(), &mut |s| lines.push(s.to_owned()));
    for l in lines {
        ui.say(&l);
    }
    match r {
        Ok(()) => {
            ui.title("c13.done.t");
            let b = ui.t("c13.done.b");
            ui.say(&b);
            let rb = ui.t("c13.k.reboot");
            let _ = ui.ask(&format!("[Enter] {rb}"));
            m.reboot();
            Outcome::Installed
        }
        Err(e) => {
            let t = ui.t("c13.err.t");
            ui.say(&format!("[{}] {t}", ui.t("c13.fail")));
            let code = serde_json::to_value(&e)
                .ok()
                .and_then(|v| v["code"].as_str().map(str::to_owned))
                .unwrap_or_default();
            ui.say(&format!("  {}: {code}", ui.t("c13.err.code")));
            let partial = ui.t("c13.err.diskPartial");
            ui.say(&partial);
            Outcome::Failed(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    struct Fake {
        disks: Vec<ProbedDisk>,
        installed: Cell<Option<String>>,
        mem: u64,
        uefi: bool,
        busy: bool,
        unprotected: Option<&'static str>,
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
            self.mem
        }
        fn disks(&self) -> Vec<ProbedDisk> {
            self.disks.clone()
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
            self.installed.set(Some(target.disk().by_path.clone()));
            say("ok");
            Ok(())
        }
        fn reboot(&self) {}
        fn storage_protected(&self) -> Result<(), String> {
            self.unprotected.map_or(Ok(()), |r| Err(r.to_owned()))
        }
    }

    fn d(dev: &str, path: &str, serial: Option<&str>, media: bool) -> ProbedDisk {
        ProbedDisk {
            device: dev.into(),
            by_path: path.into(),
            bytes: 40_000_000_000,
            model: "QEMU".into(),
            serial: serial.map(Into::into),
            wwn: None,
            partitions: 0,
            filesystems: vec![],
            boot_media: media,
            mounted: false,
            read_only: false,
        }
    }

    fn fake() -> Fake {
        Fake {
            disks: vec![
                d("sr0", "p-cd", Some("QM00003"), true),
                d("vda", "p-a", Some("SER-AAAA"), false),
                d("vdb", "p-b", Some("SER-BBBB"), false),
            ],
            installed: Cell::new(None),
            mem: 4 << 30,
            uefi: true,
            busy: false,
            unprotected: None,
        }
    }

    fn drive(m: &Fake, script: &str) -> (Outcome, String) {
        let mut input = script.as_bytes();
        let mut out = vec![];
        let mut ui = Ui {
            lang: 0,
            serial: true,
            input: &mut input,
            out: &mut out,
        };
        let mode = ocinye_image_contracts::bootmode::ResolvedMode::from_cmdline(
            "boot=casper ocinye.mode=install",
        );
        let authority = InstallAuthority::from_mode(&mode).unwrap();
        let o = run(&mut ui, m, &authority);
        (o, String::from_utf8(out).unwrap())
    }

    #[test]
    fn confirmacao_errada_nao_escreve_nada() {
        for wrong in ["", "yes", "vdb", "AAAA", "BBB", "sim"] {
            let m = fake();
            let (o, _) = drive(&m, &format!("\n3\n{wrong}\n"));
            assert_eq!(
                o,
                Outcome::Failed(OieError::ConfirmationMismatch),
                "{wrong:?}"
            );
            assert_eq!(m.installed.take(), None, "{wrong:?}");
        }
    }

    #[test]
    fn o_suporte_nao_se_escolhe_e_o_disco_certo_e_escrito() {
        let m = fake();
        // "1" is the medium: refused, the list comes back; then disk 3 + its token.
        let (o, text) = drive(&m, "\n1\n3\nbbbb\n\n\n");
        assert_eq!(o, Outcome::Installed);
        assert_eq!(m.installed.take().as_deref(), Some("p-b"));
        assert!(text.is_ascii(), "serial output is 7-bit");
        assert!(text.contains("ESTE SUPORTE - protegido"));
    }

    #[test]
    fn sem_proteccao_dos_discos_nao_ha_lista_nem_instalacao() {
        // A swap partition is active, a disk is writable, the guard did not
        // arm: whatever the reason, no disk is offered and nothing is written,
        // even with the right answers typed.
        for reason in ["GUARD_NOT_ARMED", "DEVICE_WRITABLE", "SWAP_ACTIVE"] {
            let mut m = fake();
            m.unprotected = Some(reason);
            let (o, text) = drive(&m, "\n2\nAAAA\n\n");
            assert_eq!(
                o,
                Outcome::Failed(OieError::StorageProtectionUnverified {
                    reason: reason.into()
                })
            );
            assert!(text.contains(reason));
            assert!(!text.contains("SER-AAAA"), "no disk list was shown");
            assert_eq!(m.installed.take(), None);
        }
    }

    #[test]
    fn sem_uefi_ou_memoria_nao_ha_instalacao() {
        let mut m = fake();
        m.uefi = false;
        assert_eq!(
            drive(&m, "\n").0,
            Outcome::Failed(OieError::UnsupportedFirmware)
        );
        let mut m = fake();
        m.mem = 1 << 30;
        assert!(matches!(
            drive(&m, "\n").0,
            Outcome::Failed(OieError::InsufficientMemory { .. })
        ));
        assert_eq!(m.installed.take(), None);
    }

    #[test]
    fn outra_consola_com_a_instalacao_nao_escreve() {
        let mut m = fake();
        m.busy = true;
        let (o, text) = drive(&m, "\n\n");
        assert_eq!(o, Outcome::Abandoned);
        assert!(text.contains("noutra consola"));
        assert_eq!(m.installed.take(), None);
    }

    #[test]
    fn entrada_que_acaba_abandona_sem_escrever() {
        let m = fake();
        assert_eq!(drive(&m, "\n3\n").0, Outcome::Abandoned);
        assert_eq!(m.installed.take(), None);
    }
}
