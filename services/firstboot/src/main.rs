//! `ocinye-firstboot` — first boot, claim and console of an Ocinye OS image
//! (D013, ADR-0027/0028). PROVISIONAL_PENDING_D011_CERTIFICATION.
//!
//! ```text
//! ocinye-firstboot init                                   F1–F6 (ocinye-firstboot.service)
//! ocinye-firstboot authorized-keys %u %f %t %k            sshd AuthorizedKeysCommand (ocinye-claim)
//! ocinye-firstboot claim-serve --offered SHA256:…         forced command (root via sudo)
//! ocinye-firstboot confirm --claim <id>                   from `ocinye`, via sudo -n
//! ocinye-firstboot claim --provisioned --key SHA256:…     from `ocinye`, via sudo -n
//! ocinye-firstboot claim-timeout                          the confirmation timer
//! ocinye-firstboot console --vt | --serial                ocinye-console@.service
//! ocinye-firstboot status                                 non-secret state as JSON
//! ocinye-firstboot --version
//! ```
//!
//! It never creates an Instance, contacts anything, installs packages or
//! runs a command it was sent: the claim channel is a closed set of typed
//! messages.

#![forbid(unsafe_code)]

mod console;
mod machine;
mod serve;
mod store;
mod strings;
mod system;

use std::io::BufReader;

use ocinye_image_contracts::claim::{ClaimEvent, ClaimId};
use ocinye_image_contracts::firstboot::KeyFingerprint;

use crate::machine::CommandError;
use crate::store::Root;
use crate::system::Real;

fn report(r: Result<ClaimEvent, CommandError>) -> i32 {
    match r {
        Ok(e) => {
            println!("{}", serde_json::to_string(&e).unwrap_or_default());
            0
        }
        Err(CommandError::Refused(reason)) => {
            println!(
                "{}",
                serde_json::to_string(&ClaimEvent::Refused { reason }).unwrap_or_default()
            );
            10
        }
        Err(CommandError::NotReady) => {
            eprintln!("ocinye-firstboot: estado indisponível");
            11
        }
    }
}

fn sudo_user() -> String {
    std::env::var("SUDO_USER").unwrap_or_default()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = Root::system();
    let sys = Real;
    let code = match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["init"] => match machine::init(&root, &sys) {
            ocinye_image_contracts::firstboot::FirstBootState::Ready => 0,
            _ => 1,
        },
        ["authorized-keys", rest @ ..] => {
            let mut out = std::io::stdout();
            // Any error: print nothing, sshd refuses the key.
            let _ = serve::authorized_keys(&root, rest, &mut out);
            0
        }
        ["claim-serve", "--offered", fp] => {
            let conn = std::env::var("SSH_CONNECTION").ok();
            let mut stdin = BufReader::new(std::io::stdin());
            let mut stdout = std::io::stdout();
            match serve::claim_serve(
                &root,
                &sys,
                &KeyFingerprint((*fp).to_owned()),
                &sudo_user(),
                conn.as_deref(),
                &mut stdin,
                &mut stdout,
            ) {
                Ok(()) => 0,
                Err(_) => 1,
            }
        }
        ["confirm", "--claim", id] => {
            let id = ClaimId((*id).to_owned());
            if !id.is_valid() {
                eprintln!("--claim inválido");
                std::process::exit(2);
            }
            report(machine::confirm(&root, &sys, &id, &sudo_user()))
        }
        ["claim", "--provisioned", "--key", fp] => {
            let fp = KeyFingerprint((*fp).to_owned());
            if !fp.is_valid() {
                eprintln!("--key inválida");
                std::process::exit(2);
            }
            report(machine::claim_provisioned(&root, &sys, &fp, &sudo_user()))
        }
        ["claim-timeout"] => report(machine::claim_timeout(&root, &sys)),
        ["console", mode @ ("--vt" | "--serial")] => {
            match console::run(&root, &sys, *mode == "--serial") {
                Ok(()) => 0,
                Err(_) => 1,
            }
        }
        ["status"] => {
            let v = console::load(&root, chrono::Utc::now().timestamp(), vec![]);
            let state = serde_json::json!({
                "firstboot": v.firstboot,
                "claim": v.claim.as_ref().map(|s| ocinye_image_contracts::claim::view(s, v.code.as_ref())),
                "bootstrap_id": v.identity.as_ref().map(|i| &i.bootstrap_id),
                "host_keys": v.identity.as_ref().map(|i| &i.host_keys),
            });
            println!("{state}");
            0
        }
        ["--version"] => {
            println!(
                "ocinye-firstboot {} (claim protocol {})",
                env!("CARGO_PKG_VERSION"),
                ocinye_image_contracts::CLAIM_PROTOCOL
            );
            0
        }
        _ => {
            eprintln!("uso: ocinye-firstboot init | authorized-keys U F T K | claim-serve --offered FP | confirm --claim ID | claim --provisioned --key FP | claim-timeout | console --vt|--serial | status | --version");
            2
        }
    };
    std::process::exit(code);
}
