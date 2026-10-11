//! The claim channel: `AuthorizedKeysCommand` for `ocinye-claim`, and the
//! forced command it hands out (`claim-serve`), JSON lines over SSH.
//!
//! The key to enrol is the key the SSH session authenticated with. sshd asks
//! the AKC about it, the AKC stashes it and returns a line whose forced
//! command carries its fingerprint; `claim-serve` (root, through one sudoers
//! rule) reads the stash back and checks it matches. Nothing a client sends
//! names a key.

use std::fs;
use std::io::{BufRead, Write};

use ocinye_image_contracts::claim::{
    self, ClaimChallenge, ClaimCommand, ClaimEvent, ClaimRefusal, ClaimState,
};
use ocinye_image_contracts::firstboot::{
    ImageFacts, KeyFingerprint, MachineIdentity, PublicKeyLine,
};
use serde_json::json;

use crate::machine;
use crate::store::{self, Root};
use crate::system::System;

/// The account the claim protocol runs as.
pub const CLAIM_USER: &str = "ocinye-claim";

/// The forced command for an offered key.
pub fn forced_command(fp: &KeyFingerprint) -> String {
    format!(
        "/usr/bin/sudo -n {} claim-serve --offered {}",
        ocinye_image_contracts::paths::FIRSTBOOT_BIN,
        fp.0
    )
}

/// `authorized-keys <user> <fingerprint> <type> <base64>` — the AKC.
///
/// Prints nothing (sshd refuses the key) unless the user is `ocinye-claim`,
/// the machine is claimable, and the key parses and has that fingerprint.
pub fn authorized_keys(root: &Root, args: &[&str], out: &mut dyn Write) -> std::io::Result<()> {
    let [user, fp, kind, blob] = args else {
        return Ok(());
    };
    if *user != CLAIM_USER || !root.claimable().exists() {
        return Ok(());
    }
    let Some(key) = PublicKeyLine::parse(&format!("{kind} {blob}")) else {
        return Ok(());
    };
    let fpr = key.fingerprint();
    if fpr.0 != *fp {
        return Ok(());
    }
    store::write_atomic(
        &root.offered().join(store::offered_name(&fpr)),
        key.to_line().as_bytes(),
        0o600,
    )?;
    writeln!(
        out,
        "restrict,command=\"{}\" {}",
        forced_command(&fpr),
        key.to_line()
    )
}

fn send(out: &mut dyn Write, e: &ClaimEvent) -> std::io::Result<()> {
    let line = serde_json::to_string(e).map_err(std::io::Error::other)?;
    writeln!(out, "{line}")?;
    out.flush()
}

/// Most messages a session may send; a claim needs three.
const MAX_MESSAGES: usize = 8;
/// Longest line accepted.
const MAX_LINE: usize = 4096;

fn source(conn: Option<&str>) -> String {
    conn.and_then(|c| c.split(' ').next())
        .filter(|ip| ip.parse::<std::net::IpAddr>().is_ok())
        .unwrap_or("unknown")
        .to_owned()
}

/// `claim-serve --offered <fp>` (root via sudo from `ocinye-claim`).
pub fn claim_serve(
    root: &Root,
    sys: &dyn System,
    offered: &KeyFingerprint,
    sudo_user: &str,
    ssh_connection: Option<&str>,
    input: &mut dyn BufRead,
    out: &mut dyn Write,
) -> std::io::Result<()> {
    let from = source(ssh_connection);
    if sudo_user != CLAIM_USER || !offered.is_valid() {
        return send(
            out,
            &ClaimEvent::Refused {
                reason: ClaimRefusal::WrongAccount,
            },
        );
    }
    // The stashed key must be the one the session authenticated with.
    let stashed = fs::read_to_string(root.offered().join(store::offered_name(offered))).ok();
    let Some(key) = stashed
        .as_deref()
        .and_then(|l| PublicKeyLine::parse(l.trim()))
        .filter(|k| &k.fingerprint() == offered)
    else {
        return send(
            out,
            &ClaimEvent::Refused {
                reason: ClaimRefusal::IdentityMissing,
            },
        );
    };
    let mut hello = false;
    let mut line = String::new();
    for _ in 0..MAX_MESSAGES {
        line.clear();
        let n = std::io::Read::take(&mut *input, MAX_LINE as u64 + 1).read_line(&mut line)?;
        if n == 0 {
            return Ok(());
        }
        if n > MAX_LINE {
            return Ok(());
        }
        let Ok(cmd) = serde_json::from_str::<ClaimCommand>(line.trim_end()) else {
            send(
                out,
                &ClaimEvent::Refused {
                    reason: ClaimRefusal::ProtocolUnsupported {
                        supported: ocinye_image_contracts::CLAIM_PROTOCOL,
                    },
                },
            )?;
            return Ok(());
        };
        match cmd {
            ClaimCommand::Hello { protocol } => {
                if protocol != ocinye_image_contracts::CLAIM_PROTOCOL {
                    return send(
                        out,
                        &ClaimEvent::Refused {
                            reason: ClaimRefusal::ProtocolUnsupported {
                                supported: ocinye_image_contracts::CLAIM_PROTOCOL,
                            },
                        },
                    );
                }
                hello = true;
                send(out, &welcome(root)?)?;
            }
            _ if !hello => {
                return send(
                    out,
                    &ClaimEvent::Refused {
                        reason: ClaimRefusal::ProtocolUnsupported {
                            supported: ocinye_image_contracts::CLAIM_PROTOCOL,
                        },
                    },
                );
            }
            ClaimCommand::Enroll { code } => {
                let e = enroll(root, sys, &key, &code, &from)?;
                send(out, &e)?;
            }
            ClaimCommand::Abort { claim_id } => {
                let _l = store::lock(root)?;
                let st: Option<ClaimState> = store::read_json(&root.claim_state())?;
                let Some(st) = st else { return Ok(()) };
                // Only the enrolled key may abort its own claim.
                let mine = matches!(&st, ClaimState::ClaimInProgress { owner_key, .. } if owner_key == offered);
                let gen = store::read_json::<claim::CodeRecord>(&root.code())?
                    .map_or(0, |c| c.code_generation);
                let mut rand = [0u8; 16];
                sys.random(&mut rand)?;
                let o = if mine {
                    claim::roll_back(&st, Some(&claim_id), sys.now(), rand, gen)
                } else {
                    claim::Outcome {
                        event: ClaimEvent::Refused {
                            reason: ClaimRefusal::ClaimIdMismatch,
                        },
                        state: None,
                        code: None,
                        journal: "abort_refused_owner",
                    }
                };
                machine::apply(root, sys, &st, o.clone())?;
                send(out, &o.event)?;
            }
        }
    }
    Ok(())
}

fn welcome(root: &Root) -> std::io::Result<ClaimEvent> {
    let id: MachineIdentity =
        store::read_json(&root.identity())?.ok_or_else(|| std::io::Error::other("identity"))?;
    let facts: ImageFacts =
        store::read_json(&root.sys_path(ocinye_image_contracts::paths::IMAGE_FACTS))?
            .ok_or_else(|| std::io::Error::other("facts"))?;
    let st: ClaimState =
        store::read_json(&root.claim_state())?.ok_or_else(|| std::io::Error::other("state"))?;
    let code = store::read_json(&root.code())?;
    Ok(ClaimEvent::Welcome {
        challenge: ClaimChallenge {
            protocol: ocinye_image_contracts::CLAIM_PROTOCOL,
            image: facts,
            state: claim::view(&st, code.as_ref()),
            host_keys: id.host_keys,
            bootstrap_id: id.bootstrap_id,
        },
    })
}

fn enroll(
    root: &Root,
    sys: &dyn System,
    key: &PublicKeyLine,
    code: &claim::PairingCode,
    from: &str,
) -> std::io::Result<ClaimEvent> {
    let _l = store::lock(root)?;
    if !matches!(
        store::read_json(&root.firstboot())?,
        Some(ocinye_image_contracts::firstboot::FirstBootState::Ready)
    ) {
        return Ok(ClaimEvent::Refused {
            reason: ClaimRefusal::IntegrityFailed,
        });
    }
    let st: ClaimState =
        store::read_json(&root.claim_state())?.ok_or_else(|| std::io::Error::other("state"))?;
    let rec = store::read_json(&root.code())?;
    let mut a = [0u8; 16];
    let mut b = [0u8; 16];
    sys.random(&mut a)?;
    sys.random(&mut b)?;
    let fp = key.fingerprint();
    let o = claim::enroll(&st, rec.as_ref(), code, &fp, sys.now(), a, b);
    if matches!(o.event, ClaimEvent::Enrolled { .. }) {
        // The key before the state that names it.
        let path = root.ocinye_keys();
        let mut text = fs::read_to_string(&path).unwrap_or_default();
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(&key.to_line());
        text.push('\n');
        store::write_atomic(&path, text.as_bytes(), 0o600)?;
        sys.chown_ocinye(&path)?;
    }
    machine::apply(root, sys, &st, o.clone())?;
    if matches!(o.event, ClaimEvent::Enrolled { .. }) {
        sys.arm_timeout(claim::CONFIRM_WINDOW_S)?;
    }
    store::journal(
        root,
        "claim_attempt",
        Some(json!({"from": from, "key": fp, "outcome": o.journal})),
    );
    Ok(o.event)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::machine::tests::{machine, T0};
    use crate::system::fake::Fake;
    use ocinye_image_contracts::claim::{ClaimId, CodeRecord, PairingCode};

    fn operator(sys: &Fake, r: &Root, name: &str) -> PublicKeyLine {
        let p = r.sys.join(name);
        sys.ssh_keygen("ed25519", &p).unwrap();
        PublicKeyLine::parse(fs::read_to_string(p.with_extension("pub")).unwrap().trim()).unwrap()
    }

    fn akc(r: &Root, k: &PublicKeyLine) -> String {
        let line = k.to_line();
        let (kind, blob) = line.split_once(' ').unwrap();
        let mut out = vec![];
        authorized_keys(r, &[CLAIM_USER, &k.fingerprint().0, kind, blob], &mut out).unwrap();
        String::from_utf8(out).unwrap()
    }

    fn session(r: &Root, sys: &Fake, k: &PublicKeyLine, lines: &[String]) -> Vec<ClaimEvent> {
        let input = lines.join("\n") + "\n";
        let mut out = vec![];
        claim_serve(
            r,
            sys,
            &k.fingerprint(),
            CLAIM_USER,
            Some("192.0.2.50 51000 192.0.2.10 22"),
            &mut input.as_bytes(),
            &mut out,
        )
        .unwrap();
        String::from_utf8(out)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    fn code(r: &Root) -> PairingCode {
        store::read_json::<CodeRecord>(&r.code())
            .unwrap()
            .unwrap()
            .code
    }

    fn hello() -> String {
        r#"{"cmd":"hello","protocol":1}"#.into()
    }

    fn enroll_line(c: &PairingCode) -> String {
        format!(r#"{{"cmd":"enroll","code":"{}"}}"#, c.expose())
    }

    #[test]
    fn reclamacao_por_codigo_de_ponta_a_ponta() {
        let r = machine("serve-ok");
        let sys = Fake::new(T0);
        machine::init(&r, &sys);
        let k = operator(&sys, &r, "op");
        let line = akc(&r, &k);
        assert!(
            line.starts_with(&format!(
                "restrict,command=\"{}\" ssh-ed25519 ",
                forced_command(&k.fingerprint())
            )),
            "{line}"
        );
        let ev = session(&r, &sys, &k, &[hello(), enroll_line(&code(&r))]);
        assert!(
            matches!(&ev[0], ClaimEvent::Welcome { challenge } if challenge.host_keys.len() == 2)
        );
        let ClaimEvent::Enrolled {
            claim_id,
            owner_key,
            ..
        } = ev[1].clone()
        else {
            panic!("{ev:?}")
        };
        assert_eq!(owner_key, k.fingerprint());
        assert!(fs::read_to_string(r.ocinye_keys())
            .unwrap()
            .contains(&k.to_line()));
        assert!(
            !r.claimable().exists(),
            "the AKC answers nothing during the claim"
        );
        assert_eq!(akc(&r, &k), "");
        assert!(sys.called("arm-timeout 600"));
        // Confirm from `ocinye`.
        assert!(matches!(
            machine::confirm(&r, &sys, &claim_id, "ocinye"),
            Ok(ClaimEvent::Confirmed { .. })
        ));
        assert!(sys.called("systemctl enable --now docker.service"));
        // Replays.
        assert!(matches!(
            machine::confirm(&r, &sys, &claim_id, "ocinye"),
            Err(machine::CommandError::Refused(_))
        ));
        let j = fs::read_to_string(r.journal()).unwrap();
        assert!(j.contains("\"from\":\"192.0.2.50\""));
    }

    #[test]
    fn codigo_errado_e_esgotamento() {
        let r = machine("serve-bad");
        let sys = Fake::new(T0);
        machine::init(&r, &sys);
        let k = operator(&sys, &r, "op");
        akc(&r, &k);
        let wrong = PairingCode::from_random([0xee; 16]);
        let ev = session(
            &r,
            &sys,
            &k,
            &[hello(), enroll_line(&wrong), enroll_line(&wrong)],
        );
        assert_eq!(
            ev[1],
            ClaimEvent::Refused {
                reason: ClaimRefusal::InvalidCode { remaining: 4 }
            }
        );
        assert_eq!(
            ev[2],
            ClaimEvent::Refused {
                reason: ClaimRefusal::InvalidCode { remaining: 3 }
            }
        );
        assert!(!fs::read_to_string(r.ocinye_keys())
            .unwrap_or_default()
            .contains(&k.to_line()));
        let j = fs::read_to_string(r.journal()).unwrap();
        assert!(!j.contains(wrong.expose()) && !j.contains(code(&r).expose()));
    }

    #[test]
    fn dois_reclamantes_so_um_ganha() {
        let r = machine("serve-race");
        let sys = Fake::new(T0);
        machine::init(&r, &sys);
        let a = operator(&sys, &r, "a");
        let b = operator(&sys, &r, "b");
        akc(&r, &a);
        akc(&r, &b);
        let c = code(&r);
        let ea = session(&r, &sys, &a, &[hello(), enroll_line(&c)]);
        let eb = session(&r, &sys, &b, &[hello(), enroll_line(&c)]);
        assert!(matches!(ea[1], ClaimEvent::Enrolled { .. }));
        assert!(matches!(
            eb[1],
            ClaimEvent::Refused {
                reason: ClaimRefusal::ClaimInProgress { .. }
            }
        ));
        assert!(!fs::read_to_string(r.ocinye_keys())
            .unwrap()
            .contains(&b.to_line()));
    }

    #[test]
    fn prazo_esgotado_remove_a_chave_e_reabre() {
        let r = machine("serve-timeout");
        let sys = Fake::new(T0);
        machine::init(&r, &sys);
        let k = operator(&sys, &r, "op");
        akc(&r, &k);
        let ev = session(&r, &sys, &k, &[hello(), enroll_line(&code(&r))]);
        let ClaimEvent::Enrolled { claim_id, .. } = ev[1].clone() else {
            panic!()
        };
        assert!(machine::claim_timeout(&r, &sys).is_err(), "not due yet");
        sys.clock.set(T0 + claim::CONFIRM_WINDOW_S);
        assert_eq!(machine::claim_timeout(&r, &sys), Ok(ClaimEvent::Aborted));
        assert!(!fs::read_to_string(r.ocinye_keys())
            .unwrap()
            .contains(&k.to_line()));
        assert!(r.claimable().exists() && r.code().exists());
        assert!(matches!(
            store::read_json::<ClaimState>(&r.claim_state()).unwrap(),
            Some(ClaimState::Unclaimed { generation: 2, .. })
        ));
        assert!(matches!(
            machine::confirm(&r, &sys, &claim_id, "ocinye"),
            Err(machine::CommandError::Refused(_))
        ));
    }

    #[test]
    fn sem_hello_ou_com_chave_trocada_recusa() {
        let r = machine("serve-proto");
        let sys = Fake::new(T0);
        machine::init(&r, &sys);
        let k = operator(&sys, &r, "op");
        akc(&r, &k);
        let ev = session(&r, &sys, &k, &[enroll_line(&code(&r))]);
        assert!(matches!(
            ev[0],
            ClaimEvent::Refused {
                reason: ClaimRefusal::ProtocolUnsupported { .. }
            }
        ));
        let ev = session(&r, &sys, &k, &[r#"{"cmd":"hello","protocol":2}"#.into()]);
        assert!(matches!(
            ev[0],
            ClaimEvent::Refused {
                reason: ClaimRefusal::ProtocolUnsupported { supported: 1 }
            }
        ));
        // A fingerprint with no stashed key (the AKC never saw it).
        let other = operator(&sys, &r, "other");
        let ev = session(&r, &sys, &other, &[hello()]);
        assert_eq!(
            ev[0],
            ClaimEvent::Refused {
                reason: ClaimRefusal::IdentityMissing
            }
        );
        // Not the claim account.
        let mut out = vec![];
        claim_serve(
            &r,
            &sys,
            &k.fingerprint(),
            "root",
            None,
            &mut "".as_bytes(),
            &mut out,
        )
        .unwrap();
        assert!(String::from_utf8(out).unwrap().contains("WRONG_ACCOUNT"));
        // The AKC refuses other users and mismatched fingerprints.
        let line = k.to_line();
        let (kind, blob) = line.split_once(' ').unwrap();
        let mut out = vec![];
        authorized_keys(&r, &["ocinye", &k.fingerprint().0, kind, blob], &mut out).unwrap();
        authorized_keys(
            &r,
            &[CLAIM_USER, &other.fingerprint().0, kind, blob],
            &mut out,
        )
        .unwrap();
        assert!(out.is_empty());
        let _ = ClaimId::from_bytes([0; 16]);
    }
}
