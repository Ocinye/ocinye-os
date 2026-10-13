//! `ocinye-update-engine`: the Update Engine daemon (ADR-0032).
//!
//!   ocinye-update-engine serve     run the engine (systemd: ocinye-update.service)
//!   ocinye-update-engine journal   print the journal (root, read-only)
//!
//! Administrators use `ocinye-system update …`: the same authority as
//! Settings, through the System Service.

use ocinye_update_contracts::manifest::Arch;
use ocinye_update_contracts::{Channel, ReleaseSigning};
use ocinye_update_engine::host::LinuxHost;
use ocinye_update_engine::paths::Paths;
use ocinye_update_engine::{server, Engine, ImageFacts, Stop, SYSTEM_PROTOCOL};
use std::process::ExitCode;
use std::sync::Arc;

fn facts(paths: &Paths) -> ImageFacts {
    let image: Option<serde_json::Value> = std::fs::read(&paths.image_facts)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok());
    let development = image
        .as_ref()
        .and_then(|v| {
            v.get("build_kind")
                .or_else(|| v.pointer("/image/build_kind"))
        })
        .and_then(serde_json::Value::as_str)
        .is_none_or(|k| k == "development");
    ImageFacts {
        default_channel: if development {
            Channel::Development
        } else {
            Channel::Stable
        },
        // No production root exists yet (ADR-0026): every build is DEV_ONLY.
        signing: ReleaseSigning::DevOnly,
        arch: if std::env::consts::ARCH == "aarch64" {
            Arch::Arm64
        } else {
            Arch::Amd64
        },
        system_protocol: SYSTEM_PROTOCOL,
    }
}

fn main() -> ExitCode {
    let paths = Paths::system();
    match std::env::args().nth(1).as_deref() {
        Some("serve") | None => {
            let host = Arc::new(LinuxHost::new(paths.clone()));
            let engine =
                match Engine::open(paths.clone(), LinuxHost::new(paths.clone()), facts(&paths)) {
                    Ok(e) => Arc::new(e),
                    Err(Stop::Reason(r)) => {
                        eprintln!("ocinye-update-engine: {}", r.code());
                        return ExitCode::FAILURE;
                    }
                    Err(Stop::Crashed(_)) => return ExitCode::FAILURE,
                };
            // The installed system's recovery boot entry: return to the
            // previous release, once for this boot.
            let recovery = Arc::clone(&engine);
            std::thread::spawn(move || {
                let cmdline = std::fs::read_to_string("/proc/cmdline").unwrap_or_default();
                if recovery.reserve("rollback").is_ok() {
                    if let Some(r) = recovery.recovery_request(&cmdline) {
                        match r {
                            Ok(_) => eprintln!(
                                "ocinye-update-engine: recovery: returned to the previous release"
                            ),
                            Err(Stop::Reason(r)) => {
                                eprintln!("ocinye-update-engine: recovery: {}", r.code())
                            }
                            Err(Stop::Crashed(_)) => {
                                eprintln!("ocinye-update-engine: recovery: interrupted")
                            }
                        }
                    }
                    recovery.release();
                }
            });
            let auto = Arc::clone(&engine);
            std::thread::spawn(move || {
                let mut last: Option<u64> = None;
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(3600));
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_or(0, |d| d.as_secs());
                    if auto.auto_check_due(now, last) && !auto.busy() {
                        last = Some(now);
                        if auto.check().is_ok_and(|s| {
                            s.phase == ocinye_update_contracts::Phase::UpdateAvailable
                        }) && auto.auto_download()
                            && auto.reserve("download").is_ok()
                        {
                            let _ = auto.download("local", "boot");
                            auto.release();
                        }
                    }
                }
            });
            match server::serve(engine, paths, host) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("ocinye-update-engine: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("journal") => match std::fs::read_to_string(paths.journal()) {
            Ok(text) => {
                println!("{text}");
                ExitCode::SUCCESS
            }
            Err(_) => ExitCode::FAILURE,
        },
        _ => {
            eprintln!("usage: ocinye-update-engine serve | journal");
            ExitCode::from(64)
        }
    }
}
