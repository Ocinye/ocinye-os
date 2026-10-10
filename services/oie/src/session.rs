//! The non-destructive session of the medium: Live, Hardware Check and
//! Recovery, in text (D013 Live Mode, L0 / L0-S).
//!
//! PROVISIONAL bench surface. It exists so that the boot-mode contract and the
//! storage policy can be exercised and proven; it is not the product
//! interface (that is the Design's, in a later phase) and its wording is not
//! from the Design package.
//!
//! What this module cannot do matters more than what it does: its machine
//! trait has no way to select a disk, confirm anything or install. "Install
//! Ocinye OS" here only explains a restart and, if asked, performs one. No
//! state is handed to the next boot.

use std::fmt::Write as _;

use ocinye_image_contracts::bootmode::{
    MediaAction, MediaMode, ModeSource, ResolvedMode, StorageSafetyEvidence,
};
use serde::Serialize;

use crate::flow::Ui;
use crate::policy::{PolicyVerdict, PolicyViolation};

/// One disk as the session lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiskLine {
    /// Kernel name.
    pub device: String,
    /// Size.
    pub bytes: u64,
    /// Model as reported.
    pub model: String,
    /// Connection (`nvme`, `sata`, `usb`, `scsi`, ...), when the kernel says.
    pub connection: String,
    /// Partition count.
    pub partitions: u32,
    /// Filesystem signatures found, without mounting. `None` in a restricted
    /// session: nothing was probed.
    pub filesystems: Option<Vec<String>>,
    /// It holds the medium the session booted from.
    pub boot_media: bool,
}

/// Basic hardware facts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HardwareFacts {
    /// `x86_64`, `aarch64`.
    pub arch: String,
    /// Booted through UEFI.
    pub uefi: bool,
    /// Memory.
    pub memory_bytes: u64,
    /// Online processors.
    pub cpus: u32,
    /// Network interfaces: name and whether the link is up.
    pub nics: Vec<(String, bool)>,
}

/// What the session needs from the machine. There is deliberately nothing
/// here that writes a disk.
pub trait SessionMachine {
    /// Check the storage policy now.
    fn policy(&self) -> PolicyVerdict;
    /// Disks. With `probe_signatures` false only what the kernel already
    /// exposes is read: no device is opened.
    fn disks(&self, probe_signatures: bool) -> Vec<DiskLine>;
    /// Hardware facts.
    fn hardware(&self) -> HardwareFacts;
    /// Network state, one line per address.
    fn network(&self) -> Vec<String>;
    /// A normal firmware restart.
    fn restart(&self);
    /// Power off.
    fn poweroff(&self);
}

/// How the session ended.
#[derive(Debug, PartialEq, Eq)]
pub enum SessionOutcome {
    /// Restart requested.
    Restarting,
    /// Power-off requested.
    PoweringOff,
    /// Input ended.
    Abandoned,
}

const T: &[(&str, [&str; 3])] = &[
    ("live", ["Experimentar o Ocinye OS", "Try Ocinye OS", "Essayer Ocinye OS"]),
    ("check", ["Verificar o hardware", "Hardware Check", "Vérifier le matériel"]),
    ("recovery", ["Diagnóstico em modo de texto", "Text-mode diagnostics", "Diagnostic en mode texte"]),
    ("temp", ["Sessão temporária", "Temporary session", "Session temporaire"]),
    ("intended", [
        "Os discos internos devem permanecer intactos.",
        "Internal disks are intended to remain untouched.",
        "Les disques internes sont censés rester intacts.",
    ]),
    ("certified", [
        "Nada é escrito nos discos internos.",
        "Nothing is written to internal disks.",
        "Rien n’est écrit sur les disques internes.",
    ]),
    ("restricted", [
        "A protecção de armazenamento não pôde ser totalmente verificada. Os discos não são inspeccionados.",
        "Live storage protection could not be fully verified. Disks are not inspected.",
        "La protection du stockage n’a pas pu être entièrement vérifiée. Les disques ne sont pas inspectés.",
    ]),
    ("failsafe", [
        "Modo de arranque em falta ou desconhecido: sessão sem instalação.",
        "Boot mode missing or unknown: session without installation.",
        "Mode de démarrage absent ou inconnu : session sans installation.",
    ]),
    ("notinst", [
        "Esta sessão não é uma instalação, não é uma Instância e não fica reclamada.",
        "This session is not an installation, not an Instance, and is never claimed.",
        "Cette session n’est pas une installation, pas une Instance, et n’est jamais revendiquée.",
    ]),
    ("m.disks", ["Discos", "Disks", "Disques"]),
    ("m.hw", ["Relatório de hardware", "Hardware report", "Rapport matériel"]),
    ("m.net", ["Rede", "Network", "Réseau"]),
    ("m.install", ["Instalar o Ocinye OS (reinicia)", "Install Ocinye OS (restarts)", "Installer Ocinye OS (redémarre)"]),
    ("m.restart", ["Reiniciar", "Restart", "Redémarrer"]),
    ("m.off", ["Desligar", "Power Off", "Éteindre"]),
    ("m.lang", ["Língua", "Language", "Langue"]),
    ("choose", ["Escolher", "Choose", "Choisir"]),
    ("d.none", ["sem partições", "no partitions", "aucune partition"]),
    ("d.notprobed", ["não inspeccionado", "not inspected", "non inspecté"]),
    ("d.media", ["SUPORTE DE INSTALAÇÃO", "INSTALLATION MEDIA", "SUPPORT D’INSTALLATION"]),
    ("i.t", ["Reiniciar para instalar o Ocinye OS?", "Restart to install Ocinye OS?", "Redémarrer pour installer Ocinye OS ?"]),
    ("i.b", [
        "Esta sessão temporária não instala nada. A instalação é um modo de arranque separado: o computador reinicia, o menu de arranque aparece e espera, e escolhe-se «Instalar o Ocinye OS». Nada desta sessão é guardado.",
        "This temporary session cannot install anything. Installing is a separate boot mode: the computer restarts, the boot menu appears and waits, and you choose “Install Ocinye OS”. Nothing from this session is kept.",
        "Cette session temporaire n’installe rien. L’installation est un mode de démarrage distinct : l’ordinateur redémarre, le menu de démarrage apparaît et attend, et vous choisissez « Installer Ocinye OS ». Rien de cette session n’est conservé.",
    ]),
    ("i.restricted", [
        "A protecção de armazenamento desta sessão não pôde ser totalmente verificada. Pode, ainda assim, reiniciar para o ambiente de instalação independente do Ocinye. Nenhum disco é alterado até escolher explicitamente um destino e confirmar a instalação destrutiva.",
        "Live storage protection could not be fully verified. You can still restart into the independent Ocinye installation environment. No disk will be modified until you explicitly select a target and confirm the destructive installation.",
        "La protection du stockage de cette session n’a pas pu être entièrement vérifiée. Vous pouvez tout de même redémarrer dans l’environnement d’installation indépendant d’Ocinye. Aucun disque n’est modifié tant que vous n’avez pas explicitement choisi une cible et confirmé l’installation destructive.",
    ]),
    ("i.ask", ["[S] Reiniciar agora · [Enter] Ficar nesta sessão", "[S] Restart now · [Enter] Stay in this session", "[S] Redémarrer maintenant · [Entrée] Rester dans cette session"]),
    ("off", ["A desligar.", "Powering off.", "Extinction."]),
    ("rst", ["A reiniciar.", "Restarting.", "Redémarrage."]),
];

fn t(ui: &Ui<'_>, key: &str) -> String {
    T.iter()
        .find(|(k, _)| *k == key)
        .map_or_else(|| key.to_owned(), |(_, v)| v[ui.lang].to_owned())
}

fn gb(bytes: u64) -> String {
    format!("{:.1} GB", bytes as f64 / 1e9)
}

fn mode_title(ui: &Ui<'_>, mode: MediaMode) -> String {
    t(
        ui,
        match mode {
            MediaMode::HardwareCheck => "check",
            MediaMode::Recovery => "recovery",
            // `Install` never reaches the session; it is named for totality.
            MediaMode::Live | MediaMode::Install => "live",
        },
    )
}

/// The storage sentence a session may show: the strong claim needs both the
/// artifact's certification and this boot's verified policy.
#[must_use]
pub fn storage_sentence_key(
    evidence: StorageSafetyEvidence,
    verdict: &PolicyVerdict,
) -> &'static str {
    match (verdict.restricted(), evidence) {
        (true, _) => "restricted",
        (false, StorageSafetyEvidence::Certified) => "certified",
        (false, StorageSafetyEvidence::Intended) => "intended",
    }
}

#[derive(Serialize)]
struct PolicyLine<'a> {
    mode: &'a ResolvedMode,
    evidence: StorageSafetyEvidence,
    verdict: &'a PolicyVerdict,
}

fn header(
    ui: &mut Ui<'_>,
    mode: &ResolvedMode,
    verdict: &PolicyVerdict,
    evidence: StorageSafetyEvidence,
) {
    let title = mode_title(ui, mode.mode());
    let temp = t(ui, "temp");
    ui.say("");
    ui.say(&format!("== Ocinye OS · {title} · {temp}"));
    let sentence = t(ui, storage_sentence_key(evidence, verdict));
    ui.say(&sentence);
    let notinst = t(ui, "notinst");
    ui.say(&notinst);
    if !mode.is_explicit() {
        let f = t(ui, "failsafe");
        let why = match mode.source() {
            ModeSource::UnknownFailSafe { value } => format!(" ({value})"),
            _ => String::new(),
        };
        ui.say(&format!("{f}{why}"));
    }
    if let PolicyVerdict::Restricted { violations } = verdict {
        for v in violations {
            let code = match v {
                PolicyViolation::GuardNotArmed => "GUARD_NOT_ARMED".to_owned(),
                PolicyViolation::DeviceWritable { device } => format!("DEVICE_WRITABLE {device}"),
                PolicyViolation::SwapActive { device } => format!("SWAP_ACTIVE {device}"),
                PolicyViolation::RaidAssembled { array } => format!("RAID_ASSEMBLED {array}"),
                PolicyViolation::MapperActive { device } => format!("MAPPER_ACTIVE {device}"),
                PolicyViolation::InternalMount {
                    source,
                    target,
                    fstype,
                } => {
                    format!("INTERNAL_MOUNT {source} {target} {fstype}")
                }
                PolicyViolation::VerifierFaultInjected => "VERIFIER_FAULT_INJECTED".to_owned(),
            };
            ui.say(&format!("  ! {code}"));
        }
    }
    let line = serde_json::to_string(&PolicyLine {
        mode,
        evidence,
        verdict,
    })
    .unwrap_or_default();
    ui.say(&format!("OCINYE-POLICY {line}"));
}

fn inventory(ui: &mut Ui<'_>, m: &dyn SessionMachine, verdict: &PolicyVerdict) {
    // A restricted session reads block-device metadata only.
    let probe = !verdict.restricted();
    ui.say("OCINYE-INVENTORY-BEGIN");
    for d in m.disks(probe) {
        let contents = match &d.filesystems {
            None => t(ui, "d.notprobed"),
            Some(_) if d.partitions == 0 && d.filesystems.as_ref().is_some_and(Vec::is_empty) => {
                t(ui, "d.none")
            }
            Some(fs) => format!("{} · {}", d.partitions, fs.join(", ")),
        };
        let media = if d.boot_media {
            format!("  [{}]", t(ui, "d.media"))
        } else {
            String::new()
        };
        ui.say(&format!(
            "  {:<10}{:<10}{:<24}{:<8}{contents}{media}",
            d.device,
            gb(d.bytes),
            d.model.chars().take(23).collect::<String>(),
            d.connection
        ));
    }
    ui.say("OCINYE-INVENTORY-END");
}

fn report(ui: &mut Ui<'_>, m: &dyn SessionMachine, mode: &ResolvedMode, verdict: &PolicyVerdict) {
    #[derive(Serialize)]
    struct Report<'a> {
        kind: &'static str,
        schema: u32,
        source: &'static str,
        mode: MediaMode,
        storage_policy: &'a PolicyVerdict,
        hardware: HardwareFacts,
        disks: Vec<DiskLine>,
    }
    let r = Report {
        kind: "ocinye-hardware-report",
        schema: 0,
        source: "MEDIA_SESSION",
        mode: mode.mode(),
        storage_policy: verdict,
        hardware: m.hardware(),
        disks: m.disks(!verdict.restricted()),
    };
    ui.say("OCINYE-REPORT-BEGIN");
    let mut text = String::new();
    let _ = write!(text, "{}", serde_json::to_string(&r).unwrap_or_default());
    ui.say(&text);
    ui.say("OCINYE-REPORT-END");
}

fn network(ui: &mut Ui<'_>, m: &dyn SessionMachine) {
    ui.say("OCINYE-NETWORK-BEGIN");
    for l in m.network() {
        ui.say(&format!("  {l}"));
    }
    ui.say("OCINYE-NETWORK-END");
}

/// The session. Returns when a restart or power-off was requested or the
/// input ended.
pub fn run(
    ui: &mut Ui<'_>,
    m: &dyn SessionMachine,
    mode: &ResolvedMode,
    evidence: StorageSafetyEvidence,
) -> SessionOutcome {
    let mut first = true;
    loop {
        let verdict = m.policy();
        header(ui, mode, &verdict, evidence);
        if first && mode.mode() == MediaMode::HardwareCheck {
            report(ui, m, mode, &verdict);
        }
        first = false;
        for (key, label) in [
            ("D", "m.disks"),
            ("H", "m.hw"),
            ("N", "m.net"),
            ("I", "m.install"),
            ("R", "m.restart"),
            ("P", "m.off"),
            ("L", "m.lang"),
        ] {
            let l = t(ui, label);
            ui.say(&format!("  [{key}] {l}"));
        }
        ui.say("OCINYE-SESSION-READY");
        let choose = t(ui, "choose");
        let Some(answer) = ui.ask(&choose) else {
            return SessionOutcome::Abandoned;
        };
        match answer.to_ascii_lowercase().as_str() {
            "d" => inventory(ui, m, &verdict),
            "h" => report(ui, m, mode, &verdict),
            "n" => network(ui, m),
            "l" => ui.lang = (ui.lang + 1) % 3,
            "i" => {
                // The contract refuses the destructive path here; the only
                // thing this can become is a restart.
                debug_assert!(mode.authorize(MediaAction::Install).is_err());
                let title = t(ui, "i.t");
                ui.say("");
                ui.say(&format!("== {title}"));
                if verdict.restricted() {
                    let w = t(ui, "i.restricted");
                    ui.say(&w);
                }
                let b = t(ui, "i.b");
                ui.say(&b);
                ui.say("OCINYE-INSTALL-NOTICE");
                let ask = t(ui, "i.ask");
                match ui.ask(&ask).as_deref() {
                    None => return SessionOutcome::Abandoned,
                    Some("s" | "S") => {
                        let r = t(ui, "rst");
                        ui.say(&r);
                        ui.say("OCINYE-RESTART");
                        m.restart();
                        return SessionOutcome::Restarting;
                    }
                    Some(_) => {}
                }
            }
            "r" => {
                let r = t(ui, "rst");
                ui.say(&r);
                ui.say("OCINYE-RESTART");
                m.restart();
                return SessionOutcome::Restarting;
            }
            "p" => {
                let o = t(ui, "off");
                ui.say(&o);
                ui.say("OCINYE-POWEROFF");
                m.poweroff();
                return SessionOutcome::PoweringOff;
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    struct Fake {
        verdict: PolicyVerdict,
        probes: RefCell<Vec<bool>>,
        restarts: Cell<u32>,
        poweroffs: Cell<u32>,
    }

    impl Fake {
        fn new(verdict: PolicyVerdict) -> Self {
            Self {
                verdict,
                probes: RefCell::new(vec![]),
                restarts: Cell::new(0),
                poweroffs: Cell::new(0),
            }
        }
    }

    impl SessionMachine for Fake {
        fn policy(&self) -> PolicyVerdict {
            self.verdict.clone()
        }
        fn disks(&self, probe_signatures: bool) -> Vec<DiskLine> {
            self.probes.borrow_mut().push(probe_signatures);
            vec![DiskLine {
                device: "nvme0n1".into(),
                bytes: 512_000_000_000,
                model: "SENTINEL".into(),
                connection: "nvme".into(),
                partitions: 2,
                filesystems: probe_signatures.then(|| vec!["vfat".into(), "ext4".into()]),
                boot_media: false,
            }]
        }
        fn hardware(&self) -> HardwareFacts {
            HardwareFacts {
                arch: "x86_64".into(),
                uefi: true,
                memory_bytes: 8 << 30,
                cpus: 4,
                nics: vec![("enp2s0".into(), true)],
            }
        }
        fn network(&self) -> Vec<String> {
            vec!["enp2s0 UP 192.0.2.10/24".into()]
        }
        fn restart(&self) {
            self.restarts.set(self.restarts.get() + 1);
        }
        fn poweroff(&self) {
            self.poweroffs.set(self.poweroffs.get() + 1);
        }
    }

    fn verified() -> PolicyVerdict {
        PolicyVerdict::Verified { devices: 3 }
    }

    fn restricted() -> PolicyVerdict {
        PolicyVerdict::Restricted {
            violations: vec![PolicyViolation::DeviceWritable {
                device: "sda".into(),
            }],
        }
    }

    fn drive(
        m: &Fake,
        cmdline: &str,
        evidence: StorageSafetyEvidence,
        script: &str,
    ) -> (SessionOutcome, String) {
        let mode = ResolvedMode::from_cmdline(cmdline);
        let mut input = script.as_bytes();
        let mut out = Vec::new();
        let mut ui = Ui {
            lang: 1,
            serial: true,
            input: &mut input,
            out: &mut out,
        };
        let o = run(&mut ui, m, &mode, evidence);
        (o, String::from_utf8(out).unwrap())
    }

    const LIVE: &str = "boot=casper ocinye.mode=live";

    #[test]
    fn instalar_na_sessao_so_explica_e_reinicia() {
        // Asking to install, and declining: nothing happens at all.
        let m = Fake::new(verified());
        let (o, text) = drive(&m, LIVE, StorageSafetyEvidence::Intended, "i\n\n");
        assert_eq!(o, SessionOutcome::Abandoned);
        assert!(text.contains("OCINYE-INSTALL-NOTICE"));
        assert!(text.contains("separate boot mode"));
        assert_eq!((m.restarts.get(), m.poweroffs.get()), (0, 0));
        // Asking and accepting: a restart, and only a restart.
        let m = Fake::new(verified());
        let (o, text) = drive(&m, LIVE, StorageSafetyEvidence::Intended, "i\ns\n");
        assert_eq!(o, SessionOutcome::Restarting);
        assert!(text.contains("OCINYE-RESTART"));
        assert_eq!((m.restarts.get(), m.poweroffs.get()), (1, 0));
    }

    #[test]
    fn nenhum_modo_da_sessao_autoriza_o_caminho_destrutivo() {
        for cmdline in [
            "boot=casper ocinye.mode=live",
            "boot=casper ocinye.mode=hardware-check",
            "boot=casper ocinye.mode=recovery",
            "boot=casper",
            "boot=casper ocinye.mode=nonsense",
        ] {
            let mode = ResolvedMode::from_cmdline(cmdline);
            for a in [
                MediaAction::SelectTargetDisk,
                MediaAction::ConfirmDestructive,
                MediaAction::Install,
            ] {
                assert!(mode.authorize(a).is_err(), "{cmdline} {a:?}");
            }
            assert!(
                ocinye_image_contracts::bootmode::InstallAuthority::from_mode(&mode).is_none(),
                "{cmdline}"
            );
        }
    }

    #[test]
    fn a_frase_forte_exige_certificacao_e_politica_verificada() {
        use StorageSafetyEvidence::{Certified, Intended};
        assert_eq!(storage_sentence_key(Intended, &verified()), "intended");
        assert_eq!(storage_sentence_key(Certified, &verified()), "certified");
        // A certified artifact whose policy does not hold in THIS boot does
        // not get to say it.
        assert_eq!(storage_sentence_key(Certified, &restricted()), "restricted");
        assert_eq!(storage_sentence_key(Intended, &restricted()), "restricted");
        let m = Fake::new(verified());
        let (_, text) = drive(&m, LIVE, Intended, "");
        assert!(text.contains("intended to remain untouched"));
        assert!(!text.contains("Nothing is written to internal disks"));
        let (_, text) = drive(&m, LIVE, Certified, "");
        assert!(text.contains("Nothing is written to internal disks"));
    }

    #[test]
    fn sessao_restrita_nao_inspecciona_e_avisa_ao_instalar() {
        let m = Fake::new(restricted());
        let (o, text) = drive(&m, LIVE, StorageSafetyEvidence::Certified, "d\nh\ni\n\np\n");
        assert_eq!(o, SessionOutcome::PoweringOff);
        // Nothing was probed: block-device metadata only, in the inventory
        // and in the report.
        assert_eq!(*m.probes.borrow(), vec![false, false]);
        assert!(text.contains("not inspected"));
        assert!(!text.contains("ext4"));
        assert!(text.contains("DEVICE_WRITABLE sda"));
        assert!(!text.contains("Nothing is written to internal disks"));
        // Install stays available (it is only a restart) with the warning.
        assert!(text.contains("could not be fully verified"));
        assert!(text.contains("No disk will be modified until you explicitly select a target"));
        assert!(text.contains("OCINYE-INSTALL-NOTICE"));
        assert_eq!(m.restarts.get(), 0);
        assert_eq!(m.poweroffs.get(), 1);
    }

    #[test]
    fn sessao_verificada_mostra_assinaturas_sem_montar() {
        let m = Fake::new(verified());
        let (_, text) = drive(&m, LIVE, StorageSafetyEvidence::Intended, "d\n");
        assert_eq!(*m.probes.borrow(), vec![true]);
        assert!(text.contains("vfat, ext4"));
    }

    #[test]
    fn verificar_o_hardware_imprime_o_relatorio_sem_tecla() {
        let m = Fake::new(verified());
        let (o, text) = drive(
            &m,
            "boot=casper ocinye.mode=hardware-check",
            StorageSafetyEvidence::Intended,
            "",
        );
        assert_eq!(o, SessionOutcome::Abandoned);
        assert!(text.contains("OCINYE-REPORT-BEGIN"));
        assert!(text.contains("\"kind\":\"ocinye-hardware-report\""));
        assert!(text.contains("\"source\":\"MEDIA_SESSION\""));
        assert!(text.contains("Hardware Check"));
    }

    #[test]
    fn modo_em_falta_ou_desconhecido_diz_que_nao_instala() {
        let m = Fake::new(verified());
        let (_, text) = drive(&m, "boot=casper", StorageSafetyEvidence::Intended, "");
        assert!(text.contains("Boot mode missing or unknown"));
        let (_, text) = drive(
            &m,
            "boot=casper ocinye.mode=wipe",
            StorageSafetyEvidence::Intended,
            "",
        );
        assert!(text.contains("(wipe)"));
        assert!(text.contains("not an installation, not an Instance"));
    }

    #[test]
    fn a_linha_de_politica_e_legivel_por_maquina() {
        let m = Fake::new(restricted());
        let (_, text) = drive(&m, LIVE, StorageSafetyEvidence::Intended, "");
        let line = text
            .lines()
            .find_map(|l| l.strip_prefix("OCINYE-POLICY "))
            .unwrap();
        let v: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
        assert_eq!(v["mode"]["mode"], "live");
        assert_eq!(v["evidence"], "INTENDED");
        assert_eq!(v["verdict"]["state"], "RESTRICTED");
        assert_eq!(v["verdict"]["violations"][0]["code"], "DEVICE_WRITABLE");
    }
}
