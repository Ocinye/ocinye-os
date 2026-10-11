//! `ocinye-session-service`: the daemon.
//!
//!   ocinye-session-service serve
//!   ocinye-session-service link-instance
//!
//! Started by systemd under its own service account. It listens on
//! `/run/ocinye/session.sock`, keeps its record in
//! `/var/lib/ocinye/session/`, and reads nothing else from its arguments.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ocinye_session_service::core_link::HttpCore;
use ocinye_session_service::engine::{Clock, Config, Engine, InstanceKey};
use ocinye_session_service::link;
use ocinye_session_service::server::{serve, Peer};
use ocinye_session_service::verifier::Cost;
use ocinye_system_contracts::session::SESSION_SOCKET_PATH;

struct Monotonic(Instant);

impl Clock for Monotonic {
    fn monotonic_secs(&self) -> u64 {
        self.0.elapsed().as_secs()
    }
}

#[cfg(target_os = "linux")]
fn peer_uid(stream: &UnixStream) -> Option<u32> {
    rustix::net::sockopt::socket_peercred(stream)
        .ok()
        .map(|c| c.uid.as_raw())
}

#[cfg(not(target_os = "linux"))]
fn peer_uid(_: &UnixStream) -> Option<u32> {
    None
}

fn interface_uid() -> Option<u32> {
    fs::read_to_string("/etc/passwd")
        .ok()?
        .lines()
        .find_map(|l| {
            let mut f = l.split(':');
            (f.next() == Some("ocinye-ui"))
                .then(|| f.nth(1)?.parse().ok())
                .flatten()
        })
}

/// Where the Core's API is. On an Ocinye OS machine the Core is local; the
/// unit file says where, and nothing else is read from the environment.
fn core_api() -> String {
    std::env::var("OCINYE_CORE_API").unwrap_or_else(|_| "http://127.0.0.1:8480/api/v1".to_owned())
}

/// `link-instance`: see [`ocinye_session_service::link`].
fn link_instance() -> ExitCode {
    let Ok(core) = HttpCore::new(&core_api(), "", "/nonexistent".into()) else {
        eprintln!("the Core address is not a loopback address");
        return ExitCode::FAILURE;
    };
    match link::link(&link::Paths::system(), || core.instance_key()) {
        Ok(link::Linked::Fresh) => {
            println!("linked: the Core answered");
            ExitCode::SUCCESS
        }
        Ok(link::Linked::Kept) => {
            println!("linked: the Core did not answer, the key learned before stays");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("not linked: {e:?}");
            ExitCode::FAILURE
        }
    }
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    match (args.next().as_deref(), args.next()) {
        (Some("serve"), None) => {}
        (Some("link-instance"), None) => return link_instance(),
        _ => {
            eprintln!("usage: ocinye-session-service serve | link-instance");
            return ExitCode::from(64);
        }
    }
    let read = |p: &str| {
        fs::read_to_string(p)
            .map(|s| s.trim().to_owned())
            .unwrap_or_default()
    };
    // The Instance's public key is written when the machine is claimed.
    // Without it no policy can verify, so no session can open: fail closed.
    let config = Config {
        path: "/var/lib/ocinye/session/session.json".into(),
        boot_id: read("/proc/sys/kernel/random/boot_id"),
        machine_id: read("/etc/machine-id"),
        instance_key: InstanceKey::File(format!("{}/instance-session.pub", link::SEAT_DIR).into()),
        cost: Cost::calibrate(Duration::from_millis(250)),
    };
    let base = core_api();
    let link = "/var/lib/ocinye/session/core-link.json".into();
    let Ok(core) = HttpCore::new(&base, &config.machine_id, link) else {
        eprintln!("cannot build the Core link");
        return ExitCode::FAILURE;
    };
    let engine = Arc::new(Mutex::new(Engine::new(
        core,
        Monotonic(Instant::now()),
        config,
    )));

    let _ = fs::remove_file(SESSION_SOCKET_PATH);
    let listener = match UnixListener::bind(SESSION_SOCKET_PATH) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("cannot listen on {SESSION_SOCKET_PATH}: {}", e.kind());
            return ExitCode::FAILURE;
        }
    };
    if fs::set_permissions(SESSION_SOCKET_PATH, fs::Permissions::from_mode(0o666)).is_err() {
        return ExitCode::FAILURE;
    }
    let interface = interface_uid();
    for stream in listener.incoming() {
        let Ok(mut stream) = stream else { continue };
        let peer = match peer_uid(&stream) {
            Some(0) => Peer::Privileged,
            Some(uid) if Some(uid) == interface => Peer::SystemInterface,
            _ => Peer::Other,
        };
        let engine = Arc::clone(&engine);
        std::thread::spawn(move || {
            let _ = stream.set_read_timeout(Some(Duration::from_secs(300)));
            serve(&mut stream, peer, &engine);
        });
    }
    ExitCode::SUCCESS
}
