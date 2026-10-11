//! The wire: frames on a Unix stream socket.
//!
//! Each frame is a four-byte big-endian length followed by that many bytes of
//! JSON. A frame larger than [`MAX_FRAME`] is refused before it is read, so a
//! client cannot make the service allocate what it announces.
//!
//! The first frame a client sends is [`ClientFrame::Hello`]; anything else is
//! answered with `HANDSHAKE_REQUIRED` and the connection ends. A client whose
//! contract the service does not speak gets `INCOMPATIBLE_CONTRACT` and
//! nothing is invoked.

use std::io::{self, Read, Write};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use ts_rs::TS;

use crate::event::Event;
use crate::request::{Outcome, Request};
use crate::CONTRACT_VERSION;

/// Largest frame, either direction: 256 KiB. A diagnostics report is the
/// largest legitimate one and is bounded well below this.
pub const MAX_FRAME: u32 = 256 * 1024;

/// Client → service.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "frame", rename_all = "snake_case", deny_unknown_fields)]
pub enum ClientFrame {
    /// First frame.
    Hello {
        /// The contract the client was built against.
        contract: u16,
    },
    /// One capability call.
    Call {
        /// Chosen by the client; echoed in the reply.
        #[ts(type = "number")]
        id: u64,
        /// A fresh-authentication grant, when the capability asked for one.
        grant: Option<String>,
        /// The request.
        request: Request,
    },
    /// Start receiving events on this connection.
    Subscribe {},
}

/// Service → client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "frame", rename_all = "snake_case", deny_unknown_fields)]
pub enum ServerFrame {
    /// The answer to a compatible `hello`.
    Welcome {
        /// The contract the service speaks.
        contract: u16,
    },
    /// The answer to a call.
    Reply {
        /// The call's `id`.
        #[ts(type = "number")]
        id: u64,
        /// What happened.
        outcome: Outcome,
    },
    /// Something changed. Events inform; they never carry authority.
    Event {
        /// What.
        event: Event,
    },
    /// The connection is being closed for a protocol reason.
    Rejected {
        /// Why.
        reason: crate::reason::Reason,
    },
}

/// Why a frame could not be read or written.
#[derive(Debug)]
pub enum WireError {
    /// The peer closed the connection between frames.
    Closed,
    /// The announced length exceeds [`MAX_FRAME`].
    TooLarge(u32),
    /// The bytes are not a frame of the expected type.
    Malformed,
    /// The socket failed.
    Io(io::Error),
}

impl std::fmt::Display for WireError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Closed => f.write_str("connection closed"),
            Self::TooLarge(n) => write!(f, "frame of {n} bytes exceeds the limit"),
            Self::Malformed => f.write_str("malformed frame"),
            Self::Io(e) => write!(f, "socket error: {}", e.kind()),
        }
    }
}

impl std::error::Error for WireError {}

/// Whether the service speaks a client's contract. Exact match in v1: one
/// version exists.
#[must_use]
pub const fn compatible(client: u16) -> bool {
    client == CONTRACT_VERSION
}

/// Encode one frame.
///
/// # Errors
///
/// [`WireError::TooLarge`] when the value serialises past the limit.
pub fn encode<T: Serialize>(frame: &T) -> Result<Vec<u8>, WireError> {
    let body = serde_json::to_vec(frame).map_err(|_| WireError::Malformed)?;
    let len = u32::try_from(body.len()).map_err(|_| WireError::TooLarge(u32::MAX))?;
    if len > MAX_FRAME {
        return Err(WireError::TooLarge(len));
    }
    let mut out = Vec::with_capacity(body.len() + 4);
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(&body);
    Ok(out)
}

/// Write one frame.
///
/// # Errors
///
/// As [`encode`], or the socket's failure.
pub fn write_frame<W: Write, T: Serialize>(w: &mut W, frame: &T) -> Result<(), WireError> {
    let bytes = encode(frame)?;
    w.write_all(&bytes).map_err(WireError::Io)?;
    w.flush().map_err(WireError::Io)
}

/// Read one frame.
///
/// # Errors
///
/// [`WireError::Closed`] at a clean end of stream, [`WireError::TooLarge`]
/// before reading an oversized body, [`WireError::Malformed`] when the body is
/// not the expected type (unknown fields included).
pub fn read_frame<R: Read, T: DeserializeOwned>(r: &mut R) -> Result<T, WireError> {
    let mut len = [0u8; 4];
    match r.read_exact(&mut len) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Err(WireError::Closed),
        Err(e) => return Err(WireError::Io(e)),
    }
    let len = u32::from_be_bytes(len);
    if len > MAX_FRAME {
        return Err(WireError::TooLarge(len));
    }
    let mut body = vec![0u8; len as usize];
    r.read_exact(&mut body).map_err(WireError::Io)?;
    serde_json::from_slice(&body).map_err(|_| WireError::Malformed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn uma_trama_vai_e_volta() {
        let f = ClientFrame::Call {
            id: 7,
            grant: None,
            request: Request::PowerStatus {},
        };
        let bytes = encode(&f).unwrap();
        let back: ClientFrame = read_frame(&mut Cursor::new(bytes)).unwrap();
        assert_eq!(back, f);
    }

    #[test]
    fn um_comprimento_anunciado_acima_do_limite_e_recusado_sem_ler_o_corpo() {
        let mut bytes = (MAX_FRAME + 1).to_be_bytes().to_vec();
        bytes.extend_from_slice(b"{}");
        assert!(matches!(
            read_frame::<_, ClientFrame>(&mut Cursor::new(bytes)),
            Err(WireError::TooLarge(_))
        ));
    }

    #[test]
    fn lixo_campos_a_mais_e_tramas_desconhecidas_sao_malformados() {
        for body in [&b"not json"[..], br#"{"frame":"call","id":1,"grant":null,"request":{"capability":"power.status"},"argv":["sh"]}"#, br#"{"frame":"exec","command":"id"}"#] {
            let mut bytes = u32::try_from(body.len()).unwrap().to_be_bytes().to_vec();
            bytes.extend_from_slice(body);
            assert!(matches!(read_frame::<_, ClientFrame>(&mut Cursor::new(bytes)), Err(WireError::Malformed)));
        }
    }

    #[test]
    fn fim_limpo_do_fluxo_e_fecho_e_nao_erro() {
        assert!(matches!(
            read_frame::<_, ClientFrame>(&mut Cursor::new(Vec::new())),
            Err(WireError::Closed)
        ));
    }

    #[test]
    fn so_o_contrato_exacto_e_compativel() {
        assert!(compatible(CONTRACT_VERSION));
        assert!(!compatible(CONTRACT_VERSION + 1));
        assert!(!compatible(0));
    }
}
