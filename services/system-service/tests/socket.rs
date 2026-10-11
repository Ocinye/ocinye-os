//! End to end over a real Unix socket: the client crate, the wire, the server
//! loop and the dispatch, with the platform replaced by a recorder.

use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use ocinye_system_client::{Client, ClientError};
use ocinye_system_contracts::capability::{AuthorizationClass, Family, FreshAuthClass};
use ocinye_system_contracts::reason::Reason;
use ocinye_system_contracts::request::{Outcome, Payload, Request};
use ocinye_system_contracts::state::Percent;
use ocinye_system_service::audit::{AuditRecord, AuditSink};
use ocinye_system_service::authority::{CoreAnswer, CoreAuthority, Peer, Session, SessionView};
use ocinye_system_service::server::{serve, Shared};
use ocinye_system_service::service::Platform;

struct Count(Arc<AtomicUsize>);
impl Platform for Count {
    fn perform(&self, _: &Request) -> Result<Payload, Reason> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(Payload::Done)
    }
}
struct Out;
impl SessionView for Out {
    fn session(&self) -> Session {
        Session::SignedOut
    }
    fn consume_grant(&self, _: &str, _: &str, _: &str, _: FreshAuthClass) -> bool {
        false
    }
}
struct Down;
impl CoreAuthority for Down {
    fn permits(&self, _: &str, _: Family) -> CoreAnswer {
        CoreAnswer::Unreachable
    }
}
struct Quiet;
impl AuditSink for Quiet {
    fn record(&self, _: &AuditRecord) {}
}

/// A service on a socket in a fresh directory, treating every peer as `peer`.
fn service(peer: Peer) -> (PathBuf, Arc<AtomicUsize>) {
    static N: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "ocinye-sock-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("system.sock");
    let listener = UnixListener::bind(&path).unwrap();
    let reached = Arc::new(AtomicUsize::new(0));
    let shared = Arc::new(Shared {
        platform: Box::new(Count(reached.clone())),
        session: Box::new(Out),
        core: Box::new(Down),
        audit: Box::new(Quiet),
    });
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let shared = Arc::clone(&shared);
            std::thread::spawn(move || serve(&mut stream, peer, &shared));
        }
    });
    (path, reached)
}

#[test]
fn a_presenca_ajusta_o_brilho_e_nao_reinicia_sem_confirmacao() {
    let (path, reached) = service(Peer::SystemInterface);
    let mut c = Client::connect(&path).unwrap();
    let ok = c
        .call(
            Request::DisplaySetBrightness {
                percent: Percent::new(40).unwrap(),
            },
            None,
        )
        .unwrap();
    assert!(matches!(ok, Outcome::Ok { .. }));
    let refused = c.call(Request::PowerRestart {}, None).unwrap();
    assert_eq!(
        refused,
        Outcome::Refused {
            reason: Reason::FreshAuthRequired {
                class: FreshAuthClass::Confirm
            }
        }
    );
    let refused = c
        .call(Request::RemoteAccessConfigure { ssh_enabled: false }, None)
        .unwrap();
    assert_eq!(
        refused,
        Outcome::Refused {
            reason: Reason::NotAuthorized {
                needs: AuthorizationClass::Operator
            }
        }
    );
    assert_eq!(reached.load(Ordering::SeqCst), 1);
}

#[test]
fn um_par_qualquer_liga_se_e_nao_obtem_nada() {
    let (path, reached) = service(Peer::Other);
    let mut c = Client::connect(&path).unwrap();
    for r in [
        Request::SystemInfo {},
        Request::PowerStatus {},
        Request::PowerRestart {},
    ] {
        assert!(matches!(
            c.call(r, None).unwrap(),
            Outcome::Refused {
                reason: Reason::NotAuthorized { .. }
            }
        ));
    }
    assert_eq!(reached.load(Ordering::SeqCst), 0);
}

#[test]
fn sem_servico_o_cliente_diz_inalcancavel_e_nao_inventa() {
    let missing = std::env::temp_dir().join("ocinye-no-such-socket.sock");
    assert!(matches!(
        Client::connect(&missing),
        Err(ClientError::Unreachable)
    ));
}
