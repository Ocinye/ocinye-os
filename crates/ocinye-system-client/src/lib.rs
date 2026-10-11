//! The client of the Ocinye System Service.
//!
//! Every client — the native system interface's Tauri host, the headless
//! `ocinye-system` command, a test — reaches the service through this crate and
//! therefore through the same contract. It decides nothing: it sends a typed
//! request and returns the typed outcome.
//!
//! The handshake happens in [`Client::connect`]. A service that speaks another
//! contract is an error there, before any capability can be called.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

use ocinye_system_contracts::reason::Reason;
use ocinye_system_contracts::request::{Outcome, Request};
use ocinye_system_contracts::session::{SessionReply, SessionRequest};
use ocinye_system_contracts::wire::{read_frame, write_frame, ClientFrame, ServerFrame, WireError};
use ocinye_system_contracts::CONTRACT_VERSION;

/// Why a call did not produce an outcome.
#[derive(Debug)]
pub enum ClientError {
    /// The service is not running or the socket cannot be reached.
    Unreachable,
    /// The service refused the connection, with its reason (an incompatible
    /// contract, most likely).
    Rejected(Reason),
    /// The service answered something that is not the contract.
    Protocol,
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unreachable => f.write_str("SYSTEM_SERVICE_UNREACHABLE"),
            Self::Rejected(r) => f.write_str(r.code()),
            Self::Protocol => f.write_str("SYSTEM_SERVICE_PROTOCOL"),
        }
    }
}

impl std::error::Error for ClientError {}

impl From<WireError> for ClientError {
    fn from(e: WireError) -> Self {
        match e {
            WireError::Closed | WireError::Io(_) => Self::Unreachable,
            WireError::TooLarge(_) | WireError::Malformed => Self::Protocol,
        }
    }
}

/// A connection that has completed the handshake.
pub struct Client<S = UnixStream> {
    stream: S,
    next_id: u64,
}

impl Client<UnixStream> {
    /// Connect to the service's socket and shake hands.
    ///
    /// # Errors
    ///
    /// [`ClientError::Unreachable`] when nothing listens,
    /// [`ClientError::Rejected`] when the service refuses this contract.
    pub fn connect(socket: &Path) -> Result<Self, ClientError> {
        let stream = UnixStream::connect(socket).map_err(|_| ClientError::Unreachable)?;
        // A capability may wait on a subsystem; none waits this long.
        let _ = stream.set_read_timeout(Some(Duration::from_secs(60)));
        let _ = stream.set_write_timeout(Some(Duration::from_secs(10)));
        Self::over(stream)
    }
}

impl<S: Read + Write> Client<S> {
    /// Shake hands over an established stream.
    ///
    /// # Errors
    ///
    /// As [`Client::connect`].
    pub fn over(mut stream: S) -> Result<Self, ClientError> {
        write_frame(
            &mut stream,
            &ClientFrame::Hello {
                contract: CONTRACT_VERSION,
            },
        )?;
        match read_frame::<_, ServerFrame>(&mut stream)? {
            ServerFrame::Welcome { contract } if contract == CONTRACT_VERSION => {
                Ok(Self { stream, next_id: 1 })
            }
            ServerFrame::Rejected { reason } => Err(ClientError::Rejected(reason)),
            _ => Err(ClientError::Protocol),
        }
    }

    /// Call one capability.
    ///
    /// # Errors
    ///
    /// A [`ClientError`] when no outcome came back. A refusal by the service
    /// is not an error: it is [`Outcome::Refused`].
    pub fn call(
        &mut self,
        request: Request,
        grant: Option<String>,
    ) -> Result<Outcome, ClientError> {
        let id = self.next_id;
        self.next_id += 1;
        write_frame(&mut self.stream, &ClientFrame::Call { id, grant, request })?;
        loop {
            match read_frame::<_, ServerFrame>(&mut self.stream)? {
                ServerFrame::Reply { id: got, outcome } if got == id => return Ok(outcome),
                // An event may arrive between a call and its reply.
                ServerFrame::Event { .. } => {}
                ServerFrame::Rejected { reason } => return Err(ClientError::Rejected(reason)),
                _ => return Err(ClientError::Protocol),
            }
        }
    }
}

/// A connection to the Session Service that has completed the handshake.
///
/// Same wire and same handshake as [`Client`]; after it, one
/// [`SessionRequest`] per frame and one [`SessionReply`] back.
pub struct SessionClient<S = UnixStream> {
    stream: S,
}

impl SessionClient<UnixStream> {
    /// Connect to the Session Service's socket and shake hands.
    ///
    /// # Errors
    ///
    /// As [`Client::connect`].
    pub fn connect(socket: &Path) -> Result<Self, ClientError> {
        let stream = UnixStream::connect(socket).map_err(|_| ClientError::Unreachable)?;
        // Signing in asks the Core and derives a key; nothing waits longer.
        let _ = stream.set_read_timeout(Some(Duration::from_secs(60)));
        let _ = stream.set_write_timeout(Some(Duration::from_secs(10)));
        Self::over(stream)
    }
}

impl<S: Read + Write> SessionClient<S> {
    /// Shake hands over an established stream.
    ///
    /// # Errors
    ///
    /// As [`Client::connect`].
    pub fn over(stream: S) -> Result<Self, ClientError> {
        Client::over(stream).map(|c| Self { stream: c.stream })
    }

    /// Ask one thing of the Session Service.
    ///
    /// # Errors
    ///
    /// A [`ClientError`] when no reply came back. A refusal is a reply.
    pub fn ask(&mut self, request: &SessionRequest) -> Result<SessionReply, ClientError> {
        write_frame(&mut self.stream, request)?;
        Ok(read_frame::<_, SessionReply>(&mut self.stream)?)
    }
}
