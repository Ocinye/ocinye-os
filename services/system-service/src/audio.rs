//! Sound, through ALSA: no sound server.
//!
//! The cards are the kernel's (`/proc/asound/cards`). Levels and switches
//! are each card's mixer, read and set with `amixer -M` (the mapped scale a
//! person hears, as alsamixer shows it). The default card is ALSA's own
//! setting in `/etc/asound.conf`, which this service writes only when it
//! wrote the file before or there is none.
//!
//! Output and input of one card are one ALSA default: `defaults.pcm.card`
//! moves both. Choosing an input on another card than the output needs a
//! per-direction route, which is a sound server's job; `audio.set_input` is
//! therefore not offered on this build.

use std::path::Path;

use ocinye_system_contracts::reason::{AdapterFailure, Reason};
use ocinye_system_contracts::request::AudioTarget;
use ocinye_system_contracts::state::{AudioDevice, AudioState, Percent};

use crate::files;
use crate::tool::{Runner, Tool};

/// Mixer controls that are the output level, in the order they are tried.
pub const OUTPUT_CONTROLS: &[&str] = &["Master", "PCM", "Speaker", "Headphone"];
/// Mixer controls that are the input level, in the order they are tried.
pub const INPUT_CONTROLS: &[&str] = &["Capture", "Mic"];
/// The first line of an `/etc/asound.conf` this service wrote.
pub const MARKER: &str = "# Ocinye OS: written by the System Service (audio.set_output).";

/// One sound card, as the kernel lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    /// Its index: what `amixer -c` takes and `defaults.pcm.card` names.
    pub index: u32,
    /// Its long name.
    pub name: String,
}

/// The cards in `/proc/asound/cards`:
///
/// ```text
///  0 [PCH            ]: HDA-Intel - HDA Intel PCH
///                       HDA Intel PCH at 0xf7f10000 irq 32
/// ```
#[must_use]
pub fn cards(text: &str) -> Vec<Card> {
    text.lines()
        .filter_map(|l| {
            let l = l.trim_start();
            let (index, rest) = l.split_once(' ')?;
            let index = index.parse::<u32>().ok()?;
            let rest = rest.trim_start().strip_prefix('[')?;
            let (_, rest) = rest.split_once("]:")?;
            let name = rest.split_once(" - ").map_or(rest, |(_, long)| long).trim();
            (!name.is_empty() && name.len() <= 80).then(|| Card {
                index,
                name: name.chars().filter(|c| !c.is_control()).collect(),
            })
        })
        .collect()
}

/// A level and a switch, read from one mixer control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Level {
    /// The level, averaged over the channels.
    pub percent: Percent,
    /// Whether every channel that has a switch has it off.
    pub muted: bool,
}

/// The level in `amixer -M get CONTROL` output, for one direction:
///
/// ```text
///   Front Left: Playback 52 [60%] [-21.00dB] [on]
/// ```
///
/// Strict: a channel line without a well-formed percentage makes the whole
/// answer unreadable, and a control with no level at all is not a level.
#[must_use]
pub fn level(text: &str, target: AudioTarget) -> Option<Level> {
    let word = match target {
        AudioTarget::Output => ": Playback ",
        AudioTarget::Input => ": Capture ",
    };
    let mut percents = Vec::new();
    let mut switches = Vec::new();
    // Channel lines carry bracketed values; `Limits: Playback 0 - 87` does not.
    for line in text
        .lines()
        .filter(|l| l.starts_with("  ") && l.contains(word) && l.contains('['))
    {
        let (_, values) = line.split_once(word)?;
        let mut pct = None;
        for field in values.split('[').skip(1) {
            let field = field.split_once(']')?.0;
            if let Some(n) = field.strip_suffix('%') {
                let n = n.parse::<u8>().ok().filter(|n| *n <= 100)?;
                pct = Some(n);
            } else if field == "on" || field == "off" {
                switches.push(field == "on");
            }
        }
        percents.push(pct?);
    }
    if percents.is_empty() {
        return None;
    }
    let sum: u32 = percents.iter().map(|p| u32::from(*p)).sum();
    let n = u32::try_from(percents.len()).ok()?;
    let avg = u8::try_from((sum + n / 2) / n).ok()?;
    Some(Level {
        percent: Percent::saturating(avg),
        muted: !switches.is_empty() && switches.iter().all(|on| !on),
    })
}

/// The default card in an `asound.conf`: `defaults.pcm.card N`; 0, ALSA's
/// own default, when the file says nothing.
#[must_use]
pub fn default_card(conf: Option<&str>) -> u32 {
    conf.and_then(|c| {
        c.lines().find_map(|l| {
            l.trim()
                .strip_prefix("defaults.pcm.card")
                .and_then(|v| v.trim().parse::<u32>().ok())
        })
    })
    .unwrap_or(0)
}

/// The `asound.conf` that makes `card` the default.
#[must_use]
pub fn asound_conf(card: u32) -> String {
    format!(
        "{MARKER}\n# Replaced on the next change; edit nothing here.\ndefaults.pcm.card {card}\ndefaults.ctl.card {card}\n"
    )
}

/// One card's control for one direction, and its level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Control {
    /// The card.
    pub card: u32,
    /// The control's name.
    pub name: &'static str,
    /// What it reads.
    pub level: Level,
}

/// The first control of `card` that reads as a level for `target`.
pub fn control(runner: &dyn Runner, card: u32, target: AudioTarget) -> Option<Control> {
    let names = match target {
        AudioTarget::Output => OUTPUT_CONTROLS,
        AudioTarget::Input => INPUT_CONTROLS,
    };
    let c = card.to_string();
    names.iter().find_map(|name| {
        let out = runner
            .run(Tool::Amixer, &["-c", &c, "-M", "get", name])
            .ok()?;
        level(&out, target).map(|level| Control { card, name, level })
    })
}

/// The sound adapter.
pub struct Audio<'a> {
    /// Who runs `amixer`.
    pub runner: &'a dyn Runner,
    /// `/proc`.
    pub proc: &'a Path,
    /// `/etc/asound.conf`.
    pub conf: &'a Path,
}

fn failed(cause: AdapterFailure) -> Reason {
    Reason::AdapterFailed { cause }
}

impl Audio<'_> {
    /// The kernel's cards.
    #[must_use]
    pub fn cards(&self) -> Vec<Card> {
        std::fs::read_to_string(self.proc.join("asound/cards"))
            .map(|t| cards(&t))
            .unwrap_or_default()
    }

    fn default(&self) -> u32 {
        default_card(std::fs::read_to_string(self.conf).ok().as_deref())
    }

    /// Every card's control for `target`.
    #[must_use]
    pub fn controls(&self, target: AudioTarget) -> Vec<(Card, Control)> {
        self.cards()
            .into_iter()
            .filter_map(|card| control(self.runner, card.index, target).map(|c| (card, c)))
            .collect()
    }

    /// `audio.status`.
    ///
    /// # Errors
    ///
    /// `HARDWARE_ABSENT` when the machine has no sound card.
    pub fn status(&self) -> Result<AudioState, Reason> {
        if self.cards().is_empty() {
            return Err(Reason::HardwareAbsent);
        }
        let default = self.default();
        let list = |target| {
            self.controls(target)
                .into_iter()
                .map(|(card, c)| AudioDevice {
                    id: card.index,
                    name: card.name,
                    default: card.index == default,
                    volume: c.level.percent,
                    muted: c.level.muted,
                })
                .collect()
        };
        Ok(AudioState {
            outputs: list(AudioTarget::Output),
            inputs: list(AudioTarget::Input),
        })
    }

    /// The control a change to `target` goes to: the default card's, or the
    /// first card's when the default has none for that direction.
    fn target(&self, target: AudioTarget) -> Result<Control, Reason> {
        let all = self.controls(target);
        if all.is_empty() {
            return Err(Reason::HardwareAbsent);
        }
        let default = self.default();
        let pick = all
            .iter()
            .position(|(card, _)| card.index == default)
            .unwrap_or(0);
        all.into_iter()
            .nth(pick)
            .map(|(_, c)| c)
            .ok_or(Reason::HardwareAbsent)
    }

    /// `audio.set_volume`.
    ///
    /// # Errors
    ///
    /// A typed reason.
    pub fn set_volume(&self, target: AudioTarget, percent: Percent) -> Result<(), Reason> {
        let c = self.target(target)?;
        let card = c.card.to_string();
        let level = format!("{}%", percent.get());
        self.runner
            .run(Tool::Amixer, &["-c", &card, "-M", "sset", c.name, &level])
            .map(drop)
            .map_err(failed)
    }

    /// `audio.set_mute`. For the input, "muted" is the capture switch off.
    ///
    /// # Errors
    ///
    /// A typed reason.
    pub fn set_mute(&self, target: AudioTarget, muted: bool) -> Result<(), Reason> {
        let c = self.target(target)?;
        let card = c.card.to_string();
        let word = match (target, muted) {
            (AudioTarget::Output, true) => "mute",
            (AudioTarget::Output, false) => "unmute",
            (AudioTarget::Input, true) => "nocap",
            (AudioTarget::Input, false) => "cap",
        };
        self.runner
            .run(Tool::Amixer, &["-c", &card, "sset", c.name, word])
            .map(drop)
            .map_err(failed)
    }

    /// Capture on or off on every card that has an input: the microphone
    /// switch of `privacy.set_microphone`. Every card is tried; the first
    /// failure is returned after the others were.
    ///
    /// # Errors
    ///
    /// A typed reason.
    pub fn set_capture_everywhere(&self, on: bool) -> Result<(), Reason> {
        let mut first = None;
        for (card, c) in self.controls(AudioTarget::Input) {
            let index = card.index.to_string();
            if let Err(e) = self.runner.run(
                Tool::Amixer,
                &[
                    "-c",
                    &index,
                    "sset",
                    c.name,
                    if on { "cap" } else { "nocap" },
                ],
            ) {
                first.get_or_insert(failed(e));
            }
        }
        first.map_or(Ok(()), Err)
    }

    /// `audio.set_output`: make `card` ALSA's default.
    ///
    /// # Errors
    ///
    /// `NOT_FOUND` for a card without an output; `ADAPTER_FAILED { REFUSED }`
    /// when `/etc/asound.conf` is someone else's file.
    pub fn set_output(&self, card: u32) -> Result<(), Reason> {
        if !self
            .controls(AudioTarget::Output)
            .iter()
            .any(|(c, _)| c.index == card)
        {
            return Err(Reason::NotFound);
        }
        if let Ok(existing) = std::fs::read_to_string(self.conf) {
            if existing.lines().next() != Some(MARKER) {
                return Err(failed(AdapterFailure::Refused));
            }
        }
        files::write_atomic(self.conf, asound_conf(card).as_bytes(), 0o644)
    }

    /// Whether any capture stream is running now, from the kernel's PCM
    /// status files (`/proc/asound/cardN/pcmMc/subK/status`).
    #[must_use]
    pub fn capturing(&self) -> bool {
        let root = self.proc.join("asound");
        let dirs = |p: &Path, pick: &dyn Fn(&str) -> bool| -> Vec<std::path::PathBuf> {
            std::fs::read_dir(p)
                .map(|d| {
                    d.filter_map(Result::ok)
                        .filter(|e| e.file_name().to_str().is_some_and(pick))
                        .map(|e| e.path())
                        .collect()
                })
                .unwrap_or_default()
        };
        dirs(&root, &|n| n.starts_with("card"))
            .iter()
            .flat_map(|c| dirs(c, &|n| n.starts_with("pcm") && n.ends_with('c')))
            .flat_map(|p| dirs(&p, &|n| n.starts_with("sub")))
            .any(|s| {
                std::fs::read_to_string(s.join("status"))
                    .is_ok_and(|t| t.lines().any(|l| l.trim() == "state: RUNNING"))
            })
    }

    /// Whether any card has a capture device, from the kernel's files.
    #[must_use]
    pub fn has_input(&self) -> bool {
        let root = self.proc.join("asound");
        std::fs::read_dir(&root)
            .map(|d| {
                d.filter_map(Result::ok)
                    .filter(|e| e.file_name().to_string_lossy().starts_with("card"))
                    .any(|e| {
                        std::fs::read_dir(e.path()).is_ok_and(|mut d| {
                            d.any(|x| {
                                x.is_ok_and(|x| {
                                    let n = x.file_name();
                                    let n = n.to_string_lossy();
                                    n.starts_with("pcm") && n.ends_with('c')
                                })
                            })
                        })
                    })
            })
            .unwrap_or(false)
    }
}

#[cfg(test)]
pub mod tests {
    //! Parsers against recorded output; the adapter against a script.

    use super::*;
    use crate::tool::fake::Script;

    /// `/proc/asound/cards` with two cards.
    pub const CARDS: &str = " 0 [PCH            ]: HDA-Intel - HDA Intel PCH
                      HDA Intel PCH at 0xf7f10000 irq 32
 1 [Device         ]: USB-Audio - USB Audio Device
                      Generic USB Audio Device at usb-0000:00:14.0-2, full speed
";

    /// `amixer -M get Master` of a mono output.
    pub const MASTER: &str = "Simple mixer control 'Master',0
  Capabilities: pvolume pvolume-joined pswitch pswitch-joined
  Playback channels: Mono
  Limits: Playback 0 - 87
  Mono: Playback 52 [60%] [-21.00dB] [on]
";

    /// `amixer -M get Capture` of a stereo input, switched off.
    pub const CAPTURE: &str = "Simple mixer control 'Capture',0
  Capabilities: cvolume cswitch
  Capture channels: Front Left - Front Right
  Limits: Capture 0 - 63
  Front Left: Capture 39 [62%] [12.00dB] [off]
  Front Right: Capture 40 [63%] [12.75dB] [off]
";

    #[test]
    fn as_placas_sao_as_do_nucleo() {
        let c = cards(CARDS);
        assert_eq!(
            c,
            [
                Card {
                    index: 0,
                    name: "HDA Intel PCH".into()
                },
                Card {
                    index: 1,
                    name: "USB Audio Device".into()
                }
            ]
        );
        assert!(cards("--- no soundcards ---").is_empty());
        assert!(cards("").is_empty());
    }

    #[test]
    fn o_nivel_le_se_estritamente() {
        assert_eq!(
            level(MASTER, AudioTarget::Output),
            Some(Level {
                percent: Percent::new(60).unwrap(),
                muted: false
            })
        );
        assert_eq!(
            level(CAPTURE, AudioTarget::Input),
            Some(Level {
                percent: Percent::new(63).unwrap(),
                muted: true
            })
        );
        // The wrong direction is no level.
        assert_eq!(level(MASTER, AudioTarget::Input), None);
        // A switch-only control is not a level.
        assert_eq!(level("  Mono: Playback [on]\n", AudioTarget::Output), None);
        // A malformed percentage spoils the answer.
        assert_eq!(
            level("  Mono: Playback 5 [600%] [on]\n", AudioTarget::Output),
            None
        );
        assert_eq!(
            level("  Mono: Playback 5 [6x%] [on]\n", AudioTarget::Output),
            None
        );
        assert_eq!(
            level(
                "amixer: Unable to find simple control 'Master',0\n",
                AudioTarget::Output
            ),
            None
        );
    }

    #[test]
    fn o_cartao_por_omissao_e_o_ficheiro_que_o_diz() {
        assert_eq!(default_card(None), 0);
        assert_eq!(
            default_card(Some("defaults.pcm.card 1\ndefaults.ctl.card 1\n")),
            1
        );
        assert_eq!(default_card(Some(&asound_conf(2))), 2);
        assert!(asound_conf(2).starts_with(MARKER));
    }

    fn tree(cards_text: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!(
            "ocinye-audio-{}-{}",
            std::process::id(),
            N.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("proc/asound")).unwrap();
        std::fs::write(d.join("proc/asound/cards"), cards_text).unwrap();
        d
    }
    static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

    fn script() -> Script {
        let s = Script::default();
        s.answer(Tool::Amixer, &["-c", "0", "-M", "get", "Master"], MASTER);
        s.answer(Tool::Amixer, &["-c", "0", "-M", "get", "Capture"], CAPTURE);
        // Card 1 has no Master: its level is on PCM.
        s.answer(Tool::Amixer, &["-c", "1", "-M", "get", "PCM"], MASTER);
        s
    }

    #[test]
    fn o_estado_e_as_mudancas_vao_ao_cartao_certo() {
        let d = tree(CARDS);
        let s = script();
        let conf = d.join("etc/asound.conf");
        let a = Audio {
            runner: &s,
            proc: &d.join("proc"),
            conf: &conf,
        };
        let st = a.status().unwrap();
        assert_eq!(
            st.outputs
                .iter()
                .map(|o| (o.id, o.default))
                .collect::<Vec<_>>(),
            [(0, true), (1, false)]
        );
        assert_eq!(st.inputs.len(), 1);
        assert!(st.inputs[0].muted);
        a.set_volume(AudioTarget::Output, Percent::new(40).unwrap())
            .unwrap();
        a.set_mute(AudioTarget::Input, false).unwrap();
        assert!(s.ran(&["-c", "0", "-M", "sset", "Master", "40%"]));
        assert!(s.ran(&["-c", "0", "sset", "Capture", "cap"]));
        // The default moves to card 1, and changes follow it.
        a.set_output(1).unwrap();
        assert_eq!(std::fs::read_to_string(&conf).unwrap(), asound_conf(1));
        a.set_mute(AudioTarget::Output, true).unwrap();
        assert!(s.ran(&["-c", "1", "sset", "PCM", "mute"]));
        assert_eq!(a.set_output(7), Err(Reason::NotFound));
        // Somebody else's file is not overwritten.
        std::fs::write(&conf, "pcm.!default { type hw card 0 }\n").unwrap();
        assert_eq!(
            a.set_output(0),
            Err(Reason::AdapterFailed {
                cause: AdapterFailure::Refused
            })
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn sem_placas_e_ausente() {
        let d = tree("--- no soundcards ---\n");
        let s = Script::default();
        let conf = d.join("etc/asound.conf");
        let a = Audio {
            runner: &s,
            proc: &d.join("proc"),
            conf: &conf,
        };
        assert_eq!(a.status(), Err(Reason::HardwareAbsent));
        assert_eq!(
            a.set_volume(AudioTarget::Output, Percent::new(5).unwrap()),
            Err(Reason::HardwareAbsent)
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_captura_em_curso_le_se_do_nucleo() {
        let d = tree(CARDS);
        let s = Script::default();
        let conf = d.join("etc/asound.conf");
        let a = Audio {
            runner: &s,
            proc: &d.join("proc"),
            conf: &conf,
        };
        assert!(!a.has_input() && !a.capturing());
        let sub = d.join("proc/asound/card0/pcm0c/sub0");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("status"), "closed\n").unwrap();
        assert!(a.has_input() && !a.capturing());
        std::fs::write(sub.join("status"), "state: RUNNING\nowner_pid   : 4242\n").unwrap();
        assert!(a.capturing());
        let _ = std::fs::remove_dir_all(&d);
    }
}
