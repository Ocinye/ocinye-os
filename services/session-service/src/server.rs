//! The Session Service's socket.
//!
//! Same wire as the System Service: bounded frames, a handshake carrying the
//! contract version, then one [`SessionRequest`] per frame and one
//! [`SessionReply`] back. Who may ask what is decided from the peer's
//! credentials: the trusted system interface asks for sign-in, lock and
//! unlock; only the System Service (root) may consume a grant; nobody else is
//! answered.

use std::io::{Read, Write};
use std::sync::Mutex;

use ocinye_system_contracts::reason::Reason;
use ocinye_system_contracts::session::{SessionReason, SessionReply, SessionRequest};
use ocinye_system_contracts::wire::{
    compatible, read_frame, write_frame, ClientFrame, ServerFrame, WireError,
};
use ocinye_system_contracts::CONTRACT_VERSION;

use crate::engine::{Clock, Core, Engine};

/// Who is on the other end, by the kernel's word.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Peer {
    /// `root`: the System Service, or the recovery operator.
    Privileged,
    /// The account the native system interface runs as.
    SystemInterface,
    /// Anyone else.
    Other,
}

/// Serve one connection.
pub fn serve<S: Read + Write, C: Core, K: Clock>(
    stream: &mut S,
    peer: Peer,
    engine: &Mutex<Engine<C, K>>,
) {
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
            let reason = Reason::IncompatibleContract {
                service: CONTRACT_VERSION,
                client: contract,
            };
            let _ = write_frame(stream, &ServerFrame::Rejected { reason });
            return;
        }
        Ok(_) => {
            let _ = write_frame(
                stream,
                &ServerFrame::Rejected {
                    reason: Reason::HandshakeRequired,
                },
            );
            return;
        }
        Err(_) => return,
    }
    loop {
        let reply = match read_frame::<_, SessionRequest>(stream) {
            Ok(_) if peer == Peer::Other => SessionReply::Refused {
                reason: SessionReason::NotAuthorized,
            },
            Ok(request) => match engine.lock() {
                Ok(mut engine) => engine.handle(request, peer == Peer::Privileged),
                // A poisoned engine answers nothing rather than something wrong.
                Err(_) => return,
            },
            Err(WireError::Malformed) => SessionReply::Refused {
                reason: SessionReason::Malformed,
            },
            Err(_) => return,
        };
        if write_frame(stream, &reply).is_err() {
            return;
        }
    }
}
