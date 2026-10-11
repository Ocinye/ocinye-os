//! `ocinye-session-service`: the daemon.
//!
//!   ocinye-session-service serve
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

use ocinye_session_service::engine::{Clock, Config, Core, CoreRefusal, Engine, Refresh};
use ocinye_session_service::server::{serve, Peer};
use ocinye_session_service::verifier::Cost;
use ocinye_system_contracts::session::{SignedSessionPolicy, SESSION_SOCKET_PATH};

/// Until the Core issues signed session policies, signing in is not possible:
/// the service says so, and never opens a session on its own word.
struct CoreNotLinked;

impl Core for CoreNotLinked {
    fn sign_in(
        &self,
        _: &str,
        _: &str,
        _: Option<&str>,
    ) -> Result<SignedSessionPolicy, CoreRefusal> {
        Err(CoreRefusal::Unreachable)
    }
    fn refresh(&self, _: &str) -> Refresh {
        Refresh::Unreachable
    }
    fn sign_out(&self, _: &str) {}
}

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

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    if args.next().as_deref() != Some("serve") || args.next().is_some() {
        eprintln!("usage: ocinye-session-service serve");
        return ExitCode::from(64);
    }
    let read = |p: &str| {
        fs::read_to_string(p)
            .map(|s| s.trim().to_owned())
            .unwrap_or_default()
    };
    // The Instance's public key is written when the machine is claimed.
    // Without it no policy can verify, so no session can open: fail closed.
    let instance_key: [u8; 32] = fs::read("/etc/ocinye/instance-session.pub")
        .ok()
        .and_then(|b| b.try_into().ok())
        .unwrap_or([0; 32]);
    let config = Config {
        path: "/var/lib/ocinye/session/session.json".into(),
        boot_id: read("/proc/sys/kernel/random/boot_id"),
        machine_id: read("/etc/machine-id"),
        instance_key,
        cost: Cost::calibrate(Duration::from_millis(250)),
    };
    let engine = Arc::new(Mutex::new(Engine::new(
        CoreNotLinked,
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
