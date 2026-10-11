//! `ocinye-system`: the headless client of the Ocinye System Service.
//!
//! One explicit subcommand per capability, typed arguments, the same reason
//! codes as the interface. It is not a shell: it has no way to pass a command
//! through, and what it asks is authorised by the service exactly as a click
//! would be.
//!
//!   ocinye-system [--json] [--socket PATH] FAMILY ACTION [ARGS]
//!
//! Exit codes: 0 carried out · 1 refused (the reason code is printed) ·
//! 2 the service could not be reached · 64 usage.

use std::path::PathBuf;
use std::process::ExitCode;

use ocinye_system_client::Client;
use ocinye_system_contracts::request::{Outcome, Request};
use ocinye_system_contracts::state::{Language, Percent};
use ocinye_system_contracts::SOCKET_PATH;

const USAGE: &str = "usage: ocinye-system [--json] [--socket PATH] FAMILY ACTION [ARGS]

  system health | info | capabilities
  network status
  power status | restart | shutdown | suspend
  display brightness PERCENT
  time status | timezone ZONE | automatic on|off
  locale status | language pt|en|fr
  keyboard status | layout LAYOUT [VARIANT]
  storage status
  update status
  remote-access status | ssh on|off
  recovery status
  activation status
  diagnostics collect";

fn switch(word: Option<&str>) -> Option<bool> {
    match word {
        Some("on") => Some(true),
        Some("off") => Some(false),
        _ => None,
    }
}

/// The request a command line names, or nothing when it names none.
fn parse(words: &[&str]) -> Option<Request> {
    let arg = |i: usize| words.get(i).copied();
    let exact = |n: usize, r: Request| (words.len() == n).then_some(r);
    match (arg(0)?, arg(1)?) {
        ("system", "health") => exact(2, Request::SystemHealth {}),
        ("system", "info") => exact(2, Request::SystemInfo {}),
        ("system", "capabilities") => exact(2, Request::SystemCapabilities {}),
        ("network", "status") => exact(2, Request::NetworkStatus {}),
        ("power", "status") => exact(2, Request::PowerStatus {}),
        ("power", "restart") => exact(2, Request::PowerRestart {}),
        ("power", "shutdown") => exact(2, Request::PowerShutdown {}),
        ("power", "suspend") => exact(2, Request::PowerSuspend {}),
        ("display", "brightness") => {
            let percent = Percent::new(arg(2)?.parse().ok()?)?;
            exact(3, Request::DisplaySetBrightness { percent })
        }
        ("time", "status") => exact(2, Request::TimeStatus {}),
        ("time", "timezone") => exact(
            3,
            Request::TimeConfigure {
                timezone: Some(arg(2)?.to_owned()),
                automatic: None,
            },
        ),
        ("time", "automatic") => exact(
            3,
            Request::TimeConfigure {
                timezone: None,
                automatic: Some(switch(arg(2))?),
            },
        ),
        ("locale", "status") => exact(2, Request::LocaleStatus {}),
        ("locale", "language") => {
            let language = match arg(2)? {
                "pt" => Language::Pt,
                "en" => Language::En,
                "fr" => Language::Fr,
                _ => return None,
            };
            exact(3, Request::LocaleConfigure { language })
        }
        ("keyboard", "status") => exact(2, Request::KeyboardStatus {}),
        ("keyboard", "layout") if words.len() == 3 || words.len() == 4 => {
            Some(Request::KeyboardConfigure {
                layout: arg(2)?.to_owned(),
                variant: arg(3).map(str::to_owned),
            })
        }
        ("storage", "status") => exact(2, Request::StorageStatus {}),
        ("update", "status") => exact(2, Request::UpdateStatus {}),
        ("remote-access", "status") => exact(2, Request::RemoteAccessStatus {}),
        ("remote-access", "ssh") => exact(
            3,
            Request::RemoteAccessConfigure {
                ssh_enabled: switch(arg(2))?,
            },
        ),
        ("recovery", "status") => exact(2, Request::RecoveryStatus {}),
        ("activation", "status") => exact(2, Request::ActivationStatus {}),
        ("diagnostics", "collect") => exact(2, Request::DiagnosticsCollect {}),
        _ => None,
    }
}

fn main() -> ExitCode {
    let mut json = false;
    let mut socket = PathBuf::from(SOCKET_PATH);
    let mut words: Vec<String> = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--json" => json = true,
            "--socket" => match args.next() {
                Some(p) => socket = PathBuf::from(p),
                None => {
                    eprintln!("{USAGE}");
                    return ExitCode::from(64);
                }
            },
            _ => words.push(a),
        }
    }
    let refs: Vec<&str> = words.iter().map(String::as_str).collect();
    let Some(request) = parse(&refs) else {
        eprintln!("{USAGE}");
        return ExitCode::from(64);
    };
    let outcome = match Client::connect(&socket).and_then(|mut c| c.call(request, None)) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    match &outcome {
        Outcome::Ok { payload } => {
            let text = if json {
                serde_json::to_string(payload)
            } else {
                serde_json::to_string_pretty(payload)
            };
            println!("{}", text.unwrap_or_default());
            ExitCode::SUCCESS
        }
        Outcome::Refused { reason } => {
            if json {
                println!("{}", serde_json::to_string(reason).unwrap_or_default());
            } else {
                eprintln!("{}", reason.code());
            }
            ExitCode::from(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ocinye_system_contracts::capability::CapabilityId;

    #[test]
    fn cada_subcomando_e_uma_capacidade_e_nada_mais() {
        let cases: &[(&[&str], CapabilityId)] = &[
            (&["system", "health"], CapabilityId::SystemHealth),
            (&["network", "status"], CapabilityId::NetworkStatus),
            (&["power", "restart"], CapabilityId::PowerRestart),
            (
                &["display", "brightness", "40"],
                CapabilityId::DisplaySetBrightness,
            ),
            (
                &["time", "timezone", "Africa/Luanda"],
                CapabilityId::TimeConfigure,
            ),
            (&["time", "automatic", "on"], CapabilityId::TimeConfigure),
            (&["locale", "language", "fr"], CapabilityId::LocaleConfigure),
            (
                &["keyboard", "layout", "pt"],
                CapabilityId::KeyboardConfigure,
            ),
            (
                &["remote-access", "ssh", "off"],
                CapabilityId::RemoteAccessConfigure,
            ),
            (
                &["diagnostics", "collect"],
                CapabilityId::DiagnosticsCollect,
            ),
        ];
        for (words, id) in cases {
            assert_eq!(parse(words).map(|r| r.capability()), Some(*id), "{words:?}");
        }
    }

    #[test]
    fn nao_ha_forma_de_passar_um_comando() {
        for words in [
            &["exec", "id"][..],
            &["system", "exec", "id"],
            &["power", "restart", "--force"],
            &["power", "restart", ";", "id"],
            &["display", "brightness", "140"],
            &["display", "brightness", "40", "extra"],
            &["locale", "language", "es"],
            &["time", "automatic", "maybe"],
            &["system"],
            &[],
        ] {
            assert!(parse(words).is_none(), "{words:?}");
        }
    }
}
