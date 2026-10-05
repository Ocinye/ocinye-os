//! `ocinye-console@tty1` and `@ttyS0`: the machine's own screen.
//!
//! Text only, the Design's wording (pt, en, fr), state never carried by colour
//! alone. On a serial line the text is folded to 7-bit ASCII. The pairing code
//! appears only after a key press, for at most 60 s, and only on this
//! terminal — stdout here is the terminal, never the journal.

use std::io::{Read, Write};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use ocinye_image_contracts::claim::{self, ClaimState, CodeRecord};
use ocinye_image_contracts::firstboot::{
    FirstBootError, FirstBootState, ImageFacts, MachineIdentity,
};
use ocinye_image_contracts::manifest::BuildKind;

use crate::machine;
use crate::store::{self, Root};
use crate::strings::STRINGS;
use crate::system::System;

const W: usize = 78;

fn t(key: &str, lang: usize) -> String {
    STRINGS
        .iter()
        .find(|(k, _)| *k == key)
        .map_or_else(|| key.to_owned(), |(_, v)| v[lang].to_owned())
}

fn tf(key: &str, lang: usize, args: &[(&str, String)]) -> String {
    args.iter()
        .fold(t(key, lang), |s, (k, v)| s.replace(&format!("{{{k}}}"), v))
}

/// 7-bit fold for serial consoles.
pub fn ascii(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' => "a".into(),
            'Á' | 'À' | 'Â' | 'Ã' => "A".into(),
            'é' | 'ê' | 'è' => "e".into(),
            'É' | 'Ê' | 'È' => "E".into(),
            'í' | 'î' => "i".into(),
            'Í' => "I".into(),
            'ó' | 'ô' | 'õ' => "o".into(),
            'Ó' | 'Ô' | 'Õ' => "O".into(),
            'ú' | 'û' | 'ù' => "u".into(),
            'Ú' => "U".into(),
            'ç' => "c".into(),
            'Ç' => "C".into(),
            '«' | '»' | '“' | '”' => "\"".into(),
            '’' | '‘' => "'".into(),
            '·' => "-".into(),
            '…' => "...".into(),
            '—' | '–' => "-".into(),
            c if c.is_ascii() => c.to_string(),
            _ => "?".into(),
        })
        .collect()
}

fn wrap(s: &str, width: usize) -> Vec<String> {
    let mut lines = vec![];
    let mut cur = String::new();
    for w in s.split_whitespace() {
        if !cur.is_empty() && cur.chars().count() + 1 + w.chars().count() > width {
            lines.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(w);
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

/// Everything the console shows, read from the state files.
#[derive(Debug, Clone)]
pub struct View {
    pub firstboot: Option<FirstBootState>,
    pub claim: Option<ClaimState>,
    pub code: Option<CodeRecord>,
    pub identity: Option<MachineIdentity>,
    pub facts: Option<ImageFacts>,
    pub addresses: Vec<String>,
    pub now: i64,
}

pub fn load(root: &Root, now: i64, addresses: Vec<String>) -> View {
    View {
        firstboot: store::read_json(&root.firstboot()).ok().flatten(),
        claim: store::read_json(&root.claim_state()).ok().flatten(),
        code: store::read_json(&root.code()).ok().flatten(),
        identity: store::read_json(&root.identity()).ok().flatten(),
        facts: store::read_json(&root.sys_path(ocinye_image_contracts::paths::IMAGE_FACTS))
            .ok()
            .flatten(),
        addresses,
        now,
    }
}

fn hhmm(unix: i64) -> String {
    chrono::DateTime::from_timestamp(unix, 0)
        .map_or_else(String::new, |d| d.format("%H:%M UTC").to_string())
}

fn kv(k: &str, v: &str) -> String {
    format!("  {:<22}{v}", k)
}

/// The screen, as lines. Pure: tested without a terminal.
pub fn render(v: &View, lang: usize, revealed: bool) -> Vec<String> {
    let mut out = vec![];
    let title = v.facts.as_ref().map_or_else(
        || "Ocinye OS".to_owned(),
        |f| format!("Ocinye OS · {}", f.image.name()),
    );
    out.push(title);
    if v.facts
        .as_ref()
        .is_some_and(|f| f.image.build_kind == BuildKind::Development)
    {
        out.push(format!(" ! {}", t("c13.dev", lang)));
    }
    out.push("-".repeat(W));
    let id_block = |out: &mut Vec<String>| {
        out.push(kv(
            &t("c13.network", lang),
            &if v.addresses.is_empty() {
                "—".into()
            } else {
                v.addresses.join(" · ")
            },
        ));
        if let Some(id) = &v.identity {
            if let Some(k) = id.host_keys.first() {
                out.push(kv(&t("c13.hostKey", lang), &k.algorithm));
                out.push(kv("", &k.grouped()));
            }
            out.push(kv(&t("c13.bootstrapId", lang), &id.bootstrap_id.0));
        }
    };
    let failed =
        |out: &mut Vec<String>, title: String, body: String, next: String, code: String| {
            out.push(format!("[{}] {title}", t("c13.fail", lang)));
            out.extend(wrap(&body, W));
            out.push(String::new());
            out.push(kv(&t("c13.err.code", lang), &code));
            out.extend(wrap(&next, W));
        };
    match (&v.firstboot, &v.claim) {
        (None | Some(FirstBootState::InProgress { .. }), _) => {
            out.push(t("c13.fb.t", lang));
            out.extend(wrap(&t("c13.fb.note", lang), W));
        }
        (Some(FirstBootState::Failed { error }), _) => match error {
            FirstBootError::IdentityGenerationFailed { part } => failed(
                &mut out,
                t("c13.e.identity", lang),
                tf(
                    "c13.e.identityB",
                    lang,
                    &[("s", machine::RNG_WAIT.as_secs().to_string())],
                ),
                t("c13.e.identityN", lang),
                format!(
                    "IDENTITY_GENERATION_FAILED · {}",
                    serde_json::to_string(part)
                        .unwrap_or_default()
                        .trim_matches('"')
                        .to_uppercase()
                ),
            ),
            FirstBootError::ReleasePayloadTampered { path } => failed(
                &mut out,
                t("c13.int.t", lang),
                tf("c13.int.b", lang, &[("f", path.clone())]),
                t("c13.int.n", lang),
                "RELEASE_PAYLOAD_TAMPERED".into(),
            ),
            other => failed(
                &mut out,
                t("c13.err.t", lang),
                String::new(),
                t("c13.int.n", lang),
                serde_json::to_value(other)
                    .ok()
                    .and_then(|j| j["code"].as_str().map(str::to_owned))
                    .unwrap_or_default(),
            ),
        },
        (Some(FirstBootState::Ready), Some(ClaimState::Unclaimed { locked_until, .. })) => {
            let locked = locked_until
                .as_deref()
                .and_then(claim::unix)
                .filter(|u| *u > v.now);
            if let Some(until) = locked {
                out.push(t("c13.lock.t", lang));
                out.push(kv(
                    &t("c13.state", lang),
                    &format!("{} · {}", t("st.unclaimed", lang), t("st.locked", lang)),
                ));
                out.push(kv("", &hhmm(until)));
                id_block(&mut out);
                out.push(String::new());
                out.extend(wrap(&t("c13.lock.local", lang), W));
                out.push(format!(
                    "[U] {}  [L] {}",
                    t("c13.k.unlock", lang),
                    t("c13.k.lang", lang)
                ));
            } else {
                out.push(t("c13.un.t", lang));
                if !revealed {
                    out.extend(wrap(&t("c13.un.b", lang), W));
                }
                out.push(String::new());
                out.push(kv(&t("c13.state", lang), &t("st.unclaimed", lang)));
                id_block(&mut out);
                out.extend(wrap(&t("c13.un.compare", lang), W));
                out.push(String::new());
                match (&v.code, revealed) {
                    (Some(c), true) => {
                        out.push(format!("  +{}+", "-".repeat(W - 6)));
                        out.push(format!("    {}", t("c13.un.code", lang)));
                        out.push(format!("        {}", c.code.expose()));
                        out.push(format!(
                            "    {}",
                            tf(
                                "c13.un.expires",
                                lang,
                                &[
                                    ("t", hhmm(c.expires_at)),
                                    ("n", c.attempts_left.to_string())
                                ]
                            )
                        ));
                        out.push(format!("  +{}+", "-".repeat(W - 6)));
                        out.extend(wrap(&t("c13.un.shown", lang), W));
                        out.extend(wrap(&t("c13.un.serial", lang), W));
                        out.push(format!(
                            "[P] {}  [L] {}",
                            t("c13.k.hide", lang),
                            t("c13.k.lang", lang)
                        ));
                    }
                    (Some(c), false) => {
                        out.push(kv(&t("c13.un.code", lang), &t("c13.un.hidden", lang)));
                        out.push(kv(
                            "",
                            &tf(
                                "c13.un.expires",
                                lang,
                                &[
                                    ("t", hhmm(c.expires_at)),
                                    ("n", c.attempts_left.to_string()),
                                ],
                            ),
                        ));
                        out.push(format!(
                            "[P] {}  [L] {}",
                            t("c13.k.reveal", lang),
                            t("c13.k.lang", lang)
                        ));
                    }
                    (None, _) => out.push(format!("[L] {}", t("c13.k.lang", lang))),
                }
            }
        }
        (
            Some(FirstBootState::Ready),
            Some(ClaimState::ClaimInProgress {
                owner_key,
                deadline,
                ..
            }),
        ) => {
            out.push(t("c13.cip.t", lang));
            out.extend(wrap(&t("c13.cip.b", lang), W));
            out.push(String::new());
            out.push(kv(&t("c13.state", lang), &t("st.claiming", lang)));
            out.push(kv(&t("c13.cip.key", lang), &owner_key.0));
            id_block(&mut out);
            out.push(String::new());
            out.extend(wrap(
                &tf(
                    "c13.cip.deadline",
                    lang,
                    &[("t", claim::unix(deadline).map(hhmm).unwrap_or_default())],
                ),
                W,
            ));
            out.push(format!("[L] {}", t("c13.k.lang", lang)));
        }
        (
            Some(FirstBootState::Ready),
            Some(ClaimState::Claimed {
                owner_key,
                claimed_at,
                ..
            }),
        ) => {
            out.push(t("c13.cl.t", lang));
            out.extend(wrap(&t("c13.cl.b", lang), W));
            out.push(String::new());
            out.push(kv(&t("c13.state", lang), &t("st.claimed", lang)));
            out.push(kv(&t("c13.cl.owner", lang), &owner_key.0));
            out.push(kv(&t("c13.cl.when", lang), claimed_at));
            id_block(&mut out);
            out.push(String::new());
            out.extend(wrap(&t("c13.cl.release", lang), W));
            out.push(format!(
                "[R] {}  [L] {}",
                t("c13.k.release", lang),
                t("c13.k.lang", lang)
            ));
        }
        (Some(FirstBootState::Ready), None) => out.push(t("c13.fb.t", lang)),
    }
    out
}

fn addresses() -> Vec<String> {
    std::process::Command::new("/usr/bin/hostname")
        .arg("-I")
        .output()
        .ok()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .split_whitespace()
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// The interactive loop on the service's terminal.
pub fn run(root: &Root, sys: &dyn System, serial: bool) -> std::io::Result<()> {
    // Single key presses, no echo (the code is never typed here anyway).
    let _ = std::process::Command::new("/usr/bin/stty")
        .args(["-icanon", "-echo", "min", "1"])
        .status();
    let (tx, rx) = mpsc::channel::<u8>();
    std::thread::spawn(move || {
        let mut b = [0u8; 1];
        let mut stdin = std::io::stdin();
        while stdin.read_exact(&mut b).is_ok() {
            if tx.send(b[0]).is_err() {
                break;
            }
        }
    });
    let mut lang = 0usize;
    let mut revealed_until: Option<Instant> = None;
    let mut release_armed = false;
    let mut stdout = std::io::stdout();
    loop {
        let revealed = revealed_until.is_some_and(|u| Instant::now() < u);
        let v = load(root, sys.now(), addresses());
        let mut screen = String::from("\x1b[2J\x1b[H");
        for l in render(&v, lang, revealed) {
            screen.push_str(&if serial { ascii(&l) } else { l });
            screen.push_str("\r\n");
        }
        if release_armed {
            screen.push_str("[R] ?\r\n");
        }
        stdout.write_all(screen.as_bytes())?;
        stdout.flush()?;
        match rx.recv_timeout(Duration::from_secs(if revealed { 1 } else { 5 })) {
            Ok(b'p' | b'P') => {
                revealed_until = if revealed {
                    None
                } else {
                    Some(Instant::now() + Duration::from_secs(60))
                }
            }
            Ok(b'l' | b'L') => lang = (lang + 1) % 3,
            Ok(b'u' | b'U') => {
                let _ = machine::unlock(root, sys);
            }
            Ok(b'r' | b'R') if release_armed => {
                release_armed = false;
                let _ = machine::release(root, sys, machine::d011_past_p06(root));
            }
            Ok(b'r' | b'R') => release_armed = matches!(v.claim, Some(ClaimState::Claimed { .. })),
            Ok(_) => release_armed = false,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
        }
        if !revealed {
            revealed_until = revealed_until.filter(|u| Instant::now() < *u);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::machine::tests::{machine, T0};
    use crate::system::fake::Fake;

    #[test]
    fn o_codigo_so_aparece_a_pedido() {
        let r = machine("console");
        let sys = Fake::new(T0);
        machine::init(&r, &sys);
        let v = load(&r, T0, vec!["192.0.2.10".into()]);
        let code = v.code.as_ref().unwrap().code.expose().to_owned();
        let hidden = render(&v, 0, false).join("\n");
        assert!(!hidden.contains(&code));
        assert!(
            hidden.contains("NÃO RECLAMADO")
                && hidden.contains("192.0.2.10")
                && hidden.contains("ocb-")
        );
        assert!(
            hidden.contains("COMPILAÇÃO DE DESENVOLVIMENTO"),
            "a development image says so"
        );
        let shown = render(&v, 1, true).join("\n");
        assert!(shown.contains(&code) && shown.contains("UNCLAIMED"));
        // Serial fold is 7-bit.
        assert!(render(&v, 0, false)
            .iter()
            .map(|l| ascii(l))
            .all(|l| l.is_ascii()));
        assert!(render(&v, 2, false)
            .iter()
            .map(|l| ascii(l))
            .all(|l| l.is_ascii()));
    }

    #[test]
    fn falha_de_integridade_aparece_com_o_ficheiro() {
        let r = machine("console-fail");
        let v = View {
            firstboot: Some(FirstBootState::Failed {
                error: FirstBootError::ReleasePayloadTampered {
                    path: "install/ocinye".into(),
                },
            }),
            claim: None,
            code: None,
            identity: None,
            facts: None,
            addresses: vec![],
            now: T0,
        };
        let s = render(&v, 0, false).join("\n");
        assert!(s.contains("install/ocinye") && s.contains("RELEASE_PAYLOAD_TAMPERED"));
        let _ = r;
    }
}
