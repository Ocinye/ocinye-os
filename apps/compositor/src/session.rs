//! What the compositor asks of the Session Service.
//!
//! Two things, once a second. Whether a session is active — because without a
//! yes only the trusted surface may be on the screen, and "no answer" is not a
//! yes. And, since this is the process that sees the keyboard and the pointer,
//! whether the person has been away for as long as the Instance's policy
//! allows — in which case it asks for the lock. It decides neither: the
//! policy's number comes from the Session Service, and so does the lock.

use std::os::unix::net::UnixStream;
use std::time::Duration;

use ocinye_system_client::SessionClient;
use ocinye_system_contracts::session::{
    SessionReply, SessionRequest, SessionState, SESSION_SOCKET_PATH,
};

use crate::State;

/// The service is on this machine: an answer that takes longer than this is
/// treated as no answer, and the screen is not held up waiting for it.
const PATIENCE: Duration = Duration::from_millis(250);

fn ask(request: &SessionRequest) -> Option<SessionReply> {
    let stream = UnixStream::connect(SESSION_SOCKET_PATH).ok()?;
    stream.set_read_timeout(Some(PATIENCE)).ok()?;
    stream.set_write_timeout(Some(PATIENCE)).ok()?;
    SessionClient::over(stream).ok()?.ask(request).ok()
}

impl State {
    /// Ask the Session Service where the session stands and apply it.
    pub fn follow_session(&mut self) {
        let (active, idle_lock_secs) = match ask(&SessionRequest::State {}) {
            Some(SessionReply::State {
                state: SessionState::Active { idle_lock_secs, .. },
            }) => (true, idle_lock_secs),
            _ => (false, 0),
        };
        self.idle_lock_secs = idle_lock_secs;
        let idle = self.last_input.elapsed().as_secs();
        if active && idle_lock_secs > 0 && idle >= u64::from(idle_lock_secs) {
            // The lock is the Session Service's to give; until it answers, the
            // screen already behaves as locked.
            let _ = ask(&SessionRequest::Lock {});
            self.set_session_active(false);
        } else {
            self.set_session_active(active);
        }
    }

    fn set_session_active(&mut self, active: bool) {
        if self.session_active != active {
            self.session_active = active;
            self.arrange();
        }
    }
}
