//! The socket: who connects, the handshake, and one thread per connection.
//!
//! The kernel says who the peer is ([`PeerId`]); nothing a client sends can
//! change that. A connection that does not open with a compatible `hello` is
//! told why and closed, and no capability is invoked across a contract
//! mismatch.

use std::io::{Read, Write};
use std::sync::Arc;

use ocinye_system_contracts::reason::Reason;
use ocinye_system_contracts::request::Outcome;
use ocinye_system_contracts::wire::{
    compatible, read_frame, write_frame, ClientFrame, ServerFrame, WireError,
};
use ocinye_system_contracts::CONTRACT_VERSION;

use crate::audit::AuditSink;
use crate::authority::{CoreAuthority, Peer, SessionView};
use crate::service::{Platform, Service};

/// The accounts the service recognises, by numeric user id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Accounts {
    /// The native system interface's account (`ocinye-ui`), when it exists.
    pub interface: Option<u32>,
    /// The Core's account (`ocinye-core`), when it exists.
    pub core: Option<u32>,
}

impl Accounts {
    /// Read the two accounts from a `passwd` file.
    #[must_use]
    pub fn from_passwd(text: &str) -> Self {
        let uid = |name: &str| {
            text.lines().find_map(|l| {
                let mut f = l.split(':');
                (f.next() == Some(name))
                    .then(|| f.nth(1)?.parse::<u32>().ok())
                    .flatten()
            })
        };
        Self {
            interface: uid("ocinye-ui"),
            core: uid("ocinye-core"),
        }
    }

    /// What a user id stands for.
    #[must_use]
    pub fn peer(&self, uid: u32) -> Peer {
        match uid {
            0 => Peer::RecoveryOperator,
            u if Some(u) == self.interface => Peer::SystemInterface,
            u if Some(u) == self.core => Peer::Core,
            _ => Peer::Other,
        }
    }
}

/// Everything a connection needs.
pub struct Shared {
    /// The adapters.
    pub platform: Box<dyn Platform>,
    /// The local session.
    pub session: Box<dyn SessionView>,
    /// The Core.
    pub core: Box<dyn CoreAuthority>,
    /// The audit trail.
    pub audit: Box<dyn AuditSink>,
}

/// Serve one connection until the peer leaves or breaks the protocol.
pub fn serve<S: Read + Write>(stream: &mut S, peer: Peer, shared: &Arc<Shared>) {
    let reject = |stream: &mut S, reason: Reason| {
        let _ = write_frame(stream, &ServerFrame::Rejected { reason });
    };
    match read_frame::<_, ClientFrame>(stream) {
        Ok(ClientFrame::Hello { contract }) if compatible(contract) => {
            if write_frame(
                stream,
                &ServerFrame::Welcome {
                    contract: CONTRACT_VERSION,
                },
            )
            .is_err()
            {
                return;
            }
        }
        Ok(ClientFrame::Hello { contract }) => {
            return reject(
                stream,
                Reason::IncompatibleContract {
                    service: CONTRACT_VERSION,
                    client: contract,
                },
            );
        }
        Ok(_) => return reject(stream, Reason::HandshakeRequired),
        Err(WireError::TooLarge(_)) => return reject(stream, Reason::RequestTooLarge),
        Err(WireError::Malformed) => return reject(stream, Reason::MalformedRequest),
        Err(_) => return,
    }
    let service = Service {
        platform: shared.platform.as_ref(),
        session: shared.session.as_ref(),
        core: shared.core.as_ref(),
        audit: shared.audit.as_ref(),
    };
    loop {
        let outcome_for = match read_frame::<_, ClientFrame>(stream) {
            Ok(ClientFrame::Call { id, grant, request }) => {
                (id, service.call(peer, grant.as_deref(), &request))
            }
            // Events are delivered by the subscription thread of a later
            // slice; accepting the frame keeps the contract stable.
            Ok(ClientFrame::Subscribe {}) => continue,
            Ok(ClientFrame::Hello { .. }) => return reject(stream, Reason::MalformedRequest),
            Err(WireError::Closed | WireError::Io(_)) => return,
            Err(WireError::TooLarge(_)) => return reject(stream, Reason::RequestTooLarge),
            // The frame boundary held, so the connection can go on: answer and
            // wait for the next one. The call id is unknown; zero says so.
            Err(WireError::Malformed) => (
                0,
                Outcome::Refused {
                    reason: Reason::MalformedRequest,
                },
            ),
        };
        let (id, outcome) = outcome_for;
        if write_frame(stream, &ServerFrame::Reply { id, outcome }).is_err() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::AuditRecord;
    use crate::authority::{CoreAnswer, Session};
    use ocinye_system_contracts::capability::{Family, FreshAuthClass};
    use ocinye_system_contracts::request::{Payload, Request};
    use ocinye_system_contracts::wire::encode;
    use std::io::Cursor;
    use std::sync::atomic::{AtomicUsize, Ordering};

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

    /// A stream that reads scripted input and collects output.
    struct Duplex {
        input: Cursor<Vec<u8>>,
        output: Vec<u8>,
    }
    impl Read for Duplex {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            self.input.read(buf)
        }
    }
    impl Write for Duplex {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.output.extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn run(frames: Vec<Vec<u8>>, peer: Peer) -> (Vec<ServerFrame>, usize) {
        let reached = Arc::new(AtomicUsize::new(0));
        let shared = Arc::new(Shared {
            platform: Box::new(Count(reached.clone())),
            session: Box::new(Out),
            core: Box::new(Down),
            audit: Box::new(Quiet),
        });
        let mut d = Duplex {
            input: Cursor::new(frames.concat()),
            output: Vec::new(),
        };
        serve(&mut d, peer, &shared);
        let mut out = Cursor::new(d.output);
        let mut replies = Vec::new();
        while let Ok(f) = read_frame::<_, ServerFrame>(&mut out) {
            replies.push(f);
        }
        (replies, reached.load(Ordering::SeqCst))
    }

    fn hello(v: u16) -> Vec<u8> {
        encode(&ClientFrame::Hello { contract: v }).unwrap()
    }
    fn call(id: u64, request: Request) -> Vec<u8> {
        encode(&ClientFrame::Call {
            id,
            grant: None,
            request,
        })
        .unwrap()
    }
    fn raw(body: &[u8]) -> Vec<u8> {
        let mut b = u32::try_from(body.len()).unwrap().to_be_bytes().to_vec();
        b.extend_from_slice(body);
        b
    }

    #[test]
    fn sem_aperto_de_mao_nada_e_invocado() {
        let (replies, reached) = run(
            vec![call(1, Request::PowerStatus {})],
            Peer::SystemInterface,
        );
        assert_eq!(
            replies,
            [ServerFrame::Rejected {
                reason: Reason::HandshakeRequired
            }]
        );
        assert_eq!(reached, 0);
    }

    #[test]
    fn um_contrato_diferente_e_recusado_e_nada_e_invocado() {
        let (replies, reached) = run(
            vec![
                hello(CONTRACT_VERSION + 1),
                call(1, Request::PowerStatus {}),
            ],
            Peer::SystemInterface,
        );
        assert_eq!(
            replies,
            [ServerFrame::Rejected {
                reason: Reason::IncompatibleContract {
                    service: CONTRACT_VERSION,
                    client: CONTRACT_VERSION + 1
                }
            }]
        );
        assert_eq!(reached, 0);
    }

    #[test]
    fn um_pedido_malformado_e_respondido_e_a_ligacao_continua() {
        let (replies, reached) = run(
            vec![hello(CONTRACT_VERSION), raw(br#"{"frame":"call","id":4,"grant":null,"request":{"capability":"system.exec","argv":["sh"]}}"#), call(5, Request::PowerStatus {})],
            Peer::SystemInterface,
        );
        assert_eq!(replies.len(), 3);
        assert_eq!(
            replies[1],
            ServerFrame::Reply {
                id: 0,
                outcome: Outcome::Refused {
                    reason: Reason::MalformedRequest
                }
            }
        );
        assert!(matches!(
            &replies[2],
            ServerFrame::Reply {
                id: 5,
                outcome: Outcome::Ok { .. }
            }
        ));
        assert_eq!(reached, 1);
    }

    #[test]
    fn uma_trama_gigante_fecha_a_ligacao_sem_ser_lida() {
        let mut big = (ocinye_system_contracts::wire::MAX_FRAME + 1)
            .to_be_bytes()
            .to_vec();
        big.extend_from_slice(&[b'x'; 16]);
        let (replies, reached) = run(
            vec![
                hello(CONTRACT_VERSION),
                big,
                call(9, Request::PowerStatus {}),
            ],
            Peer::SystemInterface,
        );
        assert_eq!(
            replies.last(),
            Some(&ServerFrame::Rejected {
                reason: Reason::RequestTooLarge
            })
        );
        assert_eq!(reached, 0);
    }

    #[test]
    fn as_contas_vem_do_passwd_e_um_uid_qualquer_nao_e_ninguem() {
        let a = Accounts::from_passwd("root:x:0:0:root:/root:/bin/bash\nocinye-ui:x:990:990::/var/lib/ocinye-ui:/usr/sbin/nologin\nfm:x:1000:1000::/home/fm:/bin/bash\n");
        assert_eq!(
            a,
            Accounts {
                interface: Some(990),
                core: None
            }
        );
        assert_eq!(a.peer(0), Peer::RecoveryOperator);
        assert_eq!(a.peer(990), Peer::SystemInterface);
        assert_eq!(a.peer(1000), Peer::Other);
        assert_eq!(Accounts::from_passwd("").peer(990), Peer::Other);
    }
}
