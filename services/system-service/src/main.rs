//! `ocinye-system-service`: the daemon.
//!
//!   ocinye-system-service serve [SOCKET]
//!
//! Started by systemd as root on an installed Ocinye OS. It listens on a Unix
//! socket, asks the kernel who each peer is, and serves the typed contract.
//! It takes no other argument and reads no command from anywhere.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use ocinye_system_client::SessionClient;
use ocinye_system_contracts::capability::{Family, FreshAuthClass};
use ocinye_system_contracts::session::{
    SessionReply, SessionRequest, SessionState, SESSION_SOCKET_PATH,
};
use ocinye_system_contracts::SOCKET_PATH;
use ocinye_system_service::audit::FileAudit;
use ocinye_system_service::authority::{CoreAnswer, CoreAuthority, Peer, Session, SessionView};
use ocinye_system_service::linux::{Linux, Roots};
use ocinye_system_service::server::{serve, Accounts, Shared};

/// The Session Service, asked over its socket for every request. When it
/// cannot be reached nobody is signed in as far as this service can prove,
/// and no grant can be honoured: refused, never assumed.
struct SessionOverSocket;

impl SessionOverSocket {
    fn ask(request: &SessionRequest) -> Option<SessionReply> {
        SessionClient::connect(Path::new(SESSION_SOCKET_PATH))
            .ok()?
            .ask(request)
            .ok()
    }
}

impl SessionView for SessionOverSocket {
    fn session(&self) -> Session {
        match Self::ask(&SessionRequest::State {}) {
            Some(SessionReply::State {
                state: SessionState::Active { member_id, .. },
            }) => Session::Active { member: member_id },
            Some(SessionReply::State {
                state: SessionState::Locked { member_name, .. },
            }) => Session::Locked {
                member: member_name,
            },
            _ => Session::SignedOut,
        }
    }
    fn consume_grant(
        &self,
        grant: &str,
        actor: &str,
        binding: &str,
        class: FreshAuthClass,
    ) -> bool {
        let request = SessionRequest::ConsumeGrant {
            grant: grant.to_owned(),
            actor: actor.to_owned(),
            binding: binding.to_owned(),
            class,
        };
        matches!(
            Self::ask(&request),
            Some(SessionReply::Consumed { valid: true })
        )
    }
}

/// Until the Core link exists, institutional permission cannot be asked:
/// operator capabilities answer `AUTHORITY_UNAVAILABLE`.
struct CoreUnreachable;

impl CoreAuthority for CoreUnreachable {
    fn permits(&self, _: &str, _: Family) -> CoreAnswer {
        CoreAnswer::Unreachable
    }
}

#[cfg(target_os = "linux")]
fn peer_uid(stream: &UnixStream) -> Option<u32> {
    rustix::net::sockopt::socket_peercred(stream)
        .ok()
        .map(|c| c.uid.as_raw())
}

/// Off Linux there is no kernel word to take, so nobody is anybody: the
/// daemon builds for development and serves nothing.
#[cfg(not(target_os = "linux"))]
fn peer_uid(_: &UnixStream) -> Option<u32> {
    None
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    if args.next().as_deref() != Some("serve") {
        eprintln!("usage: ocinye-system-service serve [SOCKET]");
        return ExitCode::from(64);
    }
    let socket = PathBuf::from(args.next().unwrap_or_else(|| SOCKET_PATH.to_owned()));
    if args.next().is_some() {
        eprintln!("usage: ocinye-system-service serve [SOCKET]");
        return ExitCode::from(64);
    }
    // A socket left by a previous run is not a running service.
    let _ = fs::remove_file(&socket);
    let listener = match UnixListener::bind(&socket) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("cannot listen on {}: {}", socket.display(), e.kind());
            return ExitCode::FAILURE;
        }
    };
    // Anyone may connect; what they may ask is decided per request from the
    // peer's credentials, not from who could open the file.
    if fs::set_permissions(&socket, fs::Permissions::from_mode(0o666)).is_err() {
        eprintln!("cannot set the socket's mode");
        return ExitCode::FAILURE;
    }
    let accounts = Accounts::from_passwd(&fs::read_to_string("/etc/passwd").unwrap_or_default());
    let shared = Arc::new(Shared {
        platform: Box::new(Linux::new(Roots::system())),
        session: Box::new(SessionOverSocket),
        core: Box::new(CoreUnreachable),
        audit: Box::new(FileAudit::new("/var/log/ocinye/system-audit.jsonl".into())),
    });
    for stream in listener.incoming() {
        let Ok(mut stream) = stream else { continue };
        let peer = peer_uid(&stream).map_or(Peer::Other, |uid| accounts.peer(uid));
        let shared = Arc::clone(&shared);
        std::thread::spawn(move || {
            let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(300)));
            serve(&mut stream, peer, &shared);
        });
    }
    ExitCode::SUCCESS
}
