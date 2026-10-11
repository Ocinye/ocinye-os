//! The session state machine.
//!
//! ```text
//! SIGNED OUT ──sign-in (needs the Core)──▶ ACTIVE ──lock──▶ LOCKED
//!                                            ▲                 │ unlock: local verifier, then the Core if it answers
//!                                            └─────────────────┘
//! any ──▶ SIGNED OUT, with the reason: sign-out, revocation, offline policy, too many attempts, restart
//! ```
//!
//! Signing in establishes who the member is and needs the Core. Unlocking
//! proves the person at the seat is the one who signed in, and is local: a
//! Core that is restarting does not lock a legitimate member out. The policy
//! the Core signed decides how far that goes.

use std::path::PathBuf;

use ocinye_system_contracts::capability::FreshAuthClass;
use ocinye_system_contracts::session::{
    CoreLink, EndReason, SessionReason, SessionReply, SessionRequest, SessionState,
    SignedSessionPolicy,
};
use password_hash::rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};

use crate::policy;
use crate::verifier::{self, Cost};

/// Why the Core did not sign a member in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreRefusal {
    /// Wrong user name or password.
    InvalidCredentials,
    /// A second factor is needed.
    SecondFactorRequired,
    /// The member must first set a password.
    PasswordChangeRequired,
    /// The Core cannot be reached.
    Unreachable,
}

/// What the Core says about an existing session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refresh {
    /// Still valid, with the policy as it stands now.
    Current(SignedSessionPolicy),
    /// Revoked, expired, or the member was suspended.
    Revoked,
    /// The Core cannot be reached.
    Unreachable,
}

/// The Core, as the Session Service needs it.
pub trait Core {
    /// Sign a member in.
    ///
    /// # Errors
    ///
    /// Why not.
    fn sign_in(
        &self,
        username: &str,
        password: &str,
        second_factor: Option<&str>,
    ) -> Result<SignedSessionPolicy, CoreRefusal>;
    /// Ask about a session.
    fn refresh(&self, session_id: &str) -> Refresh;
    /// Tell the Core a session ended here. Best effort.
    fn sign_out(&self, session_id: &str);
}

/// Time, injected so the rules can be tested.
pub trait Clock {
    /// Seconds from an arbitrary origin that never goes backwards.
    fn monotonic_secs(&self) -> u64;
}

/// What survives a restart of the service (not of the machine).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    /// The boot this session belongs to.
    boot_id: String,
    policy: SignedSessionPolicy,
    /// The Argon2id verifier (PHC).
    verifier: String,
    failures: u32,
    /// Seconds the session has gone without the Core confirming it.
    offline_secs: u64,
    /// Seconds still to wait before another attempt, as of the last write.
    backoff_secs: u64,
}

/// A fresh-authentication grant: one actor, one exact request, one use.
#[derive(Debug, Clone)]
struct Grant {
    id: String,
    actor: String,
    binding: String,
    class: FreshAuthClass,
    expires_at: u64,
}

/// How long a grant lives, in seconds.
pub const GRANT_TTL_SECS: u64 = 120;
/// The longest wait between unlock attempts, in seconds.
pub const BACKOFF_CEILING_SECS: u64 = 300;
/// Grants held at once; older ones are dropped.
const GRANT_LIMIT: usize = 16;

/// The wait after `failures` consecutive failed attempts.
#[must_use]
pub fn backoff_secs(failures: u32) -> u64 {
    match failures {
        0..=2 => 0,
        n => (1u64 << (n - 2).min(16)).min(BACKOFF_CEILING_SECS),
    }
}

/// The session authority.
pub struct Engine<C: Core, K: Clock> {
    core: C,
    clock: K,
    path: PathBuf,
    boot_id: String,
    machine_id: String,
    instance_key: [u8; 32],
    cost: Cost,
    record: Option<Record>,
    locked: bool,
    link: CoreLink,
    ended: Option<EndReason>,
    /// When the backoff in the record ends, on the monotonic clock.
    retry_at: u64,
    /// When offline time was last accounted.
    accounted_at: u64,
    grants: Vec<Grant>,
}

/// What an engine is configured with.
pub struct Config {
    /// Where the record is kept.
    pub path: PathBuf,
    /// This boot's identifier.
    pub boot_id: String,
    /// This machine's identifier.
    pub machine_id: String,
    /// The Instance's public key.
    pub instance_key: [u8; 32],
    /// The verifier cost, calibrated for this machine.
    pub cost: Cost,
}

impl<C: Core, K: Clock> Engine<C, K> {
    /// Start the engine. A record from this boot means the service restarted
    /// with a session open: it comes back **locked**. A record from another
    /// boot is a session that did not survive the restart.
    pub fn new(core: C, clock: K, config: Config) -> Self {
        let now = clock.monotonic_secs();
        let stored: Option<Record> = std::fs::read(&config.path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok());
        let mut engine = Self {
            core,
            clock,
            path: config.path,
            boot_id: config.boot_id,
            machine_id: config.machine_id,
            instance_key: config.instance_key,
            cost: config.cost,
            record: None,
            locked: false,
            link: CoreLink::Unreachable,
            ended: None,
            retry_at: now,
            accounted_at: now,
            grants: Vec::new(),
        };
        match stored {
            Some(r)
                if r.boot_id == engine.boot_id
                    && policy::verify(&r.policy, &engine.instance_key, &engine.machine_id)
                        .is_some() =>
            {
                engine.retry_at = now + r.backoff_secs;
                engine.record = Some(r);
                engine.locked = true;
            }
            Some(_) => engine.end(EndReason::Restarted),
            None => {}
        }
        engine
    }

    fn persist(&mut self) {
        let now = self.clock.monotonic_secs();
        match self.record.as_mut() {
            Some(r) => {
                r.backoff_secs = self.retry_at.saturating_sub(now);
                if let Ok(bytes) = serde_json::to_vec(r) {
                    let tmp = self.path.with_extension("new");
                    if write_private(&tmp, &bytes).is_ok() {
                        let _ = std::fs::rename(&tmp, &self.path);
                    }
                }
            }
            None => {
                let _ = std::fs::remove_file(&self.path);
            }
        }
    }

    fn end(&mut self, reason: EndReason) {
        if let Some(r) = self.record.take() {
            self.core.sign_out(&r.policy.policy.session_id);
        }
        self.locked = false;
        self.grants.clear();
        self.ended = Some(reason);
        self.persist();
    }

    /// Add the time since the last account to the session's offline time.
    fn account(&mut self) {
        let now = self.clock.monotonic_secs();
        let elapsed = now.saturating_sub(self.accounted_at);
        self.accounted_at = now;
        if let Some(r) = self.record.as_mut() {
            r.offline_secs = r.offline_secs.saturating_add(elapsed);
        }
    }

    /// Who the System Service will see asking, for a grant to be bound to.
    fn actor(&self) -> String {
        match (&self.record, self.locked) {
            (Some(r), false) => format!("member:{}", r.policy.policy.member_id),
            _ => "presence".to_owned(),
        }
    }

    /// The session now.
    pub fn state(&mut self) -> SessionState {
        match &self.record {
            None => SessionState::SignedOut { ended: self.ended },
            Some(r) if self.locked => SessionState::Locked {
                member_name: r.policy.policy.member_name.clone(),
                core: self.link,
                retry_after_secs: self.retry_at.saturating_sub(self.clock.monotonic_secs()),
            },
            Some(r) => SessionState::Active {
                member_id: r.policy.policy.member_id.clone(),
                member_name: r.policy.policy.member_name.clone(),
                core: self.link,
            },
        }
    }

    fn refused(reason: SessionReason) -> SessionReply {
        SessionReply::Refused { reason }
    }

    fn sign_in(
        &mut self,
        username: &str,
        password: &str,
        second_factor: Option<&str>,
    ) -> SessionReply {
        if self.record.is_some() {
            return Self::refused(SessionReason::AlreadySignedIn);
        }
        let signed = match self.core.sign_in(username, password, second_factor) {
            Ok(p) => p,
            Err(CoreRefusal::InvalidCredentials) => {
                return Self::refused(SessionReason::InvalidCredentials)
            }
            Err(CoreRefusal::SecondFactorRequired) => {
                return Self::refused(SessionReason::SecondFactorRequired)
            }
            Err(CoreRefusal::Unreachable) => return Self::refused(SessionReason::CoreUnreachable),
            Err(CoreRefusal::PasswordChangeRequired) => {
                return Self::refused(SessionReason::PasswordChangeRequired)
            }
        };
        if policy::verify(&signed, &self.instance_key, &self.machine_id).is_none() {
            self.core.sign_out(&signed.policy.session_id);
            return Self::refused(SessionReason::PolicyInvalid);
        }
        let Some(verifier) = verifier::create(password, self.cost) else {
            self.core.sign_out(&signed.policy.session_id);
            return Self::refused(SessionReason::PolicyInvalid);
        };
        self.record = Some(Record {
            boot_id: self.boot_id.clone(),
            policy: signed,
            verifier,
            failures: 0,
            offline_secs: 0,
            backoff_secs: 0,
        });
        self.locked = false;
        self.link = CoreLink::Online;
        self.ended = None;
        self.accounted_at = self.clock.monotonic_secs();
        self.retry_at = self.accounted_at;
        self.persist();
        SessionReply::State {
            state: self.state(),
        }
    }

    /// Check a password against the session's verifier, with throttling.
    /// `Err` is the refusal to return; the session may have ended.
    fn check_password(&mut self, password: &str) -> Result<(), SessionReason> {
        let now = self.clock.monotonic_secs();
        if now < self.retry_at {
            return Err(SessionReason::Throttled {
                retry_after_secs: self.retry_at - now,
            });
        }
        let Some(r) = self.record.as_mut() else {
            return Err(SessionReason::NotSignedIn);
        };
        if verifier::matches(password, &r.verifier) {
            r.failures = 0;
            self.retry_at = now;
            return Ok(());
        }
        r.failures += 1;
        if r.failures >= r.policy.policy.max_unlock_attempts {
            self.end(EndReason::TooManyAttempts);
        } else {
            self.retry_at = now + backoff_secs(r.failures);
            self.persist();
        }
        Err(SessionReason::InvalidCredentials)
    }

    /// Ask the Core about the session and apply what it says. `Err` when the
    /// session cannot go on.
    fn resynchronise(&mut self) -> Result<(), SessionReason> {
        self.account();
        let Some(r) = self.record.as_mut() else {
            return Err(SessionReason::NotSignedIn);
        };
        match self.core.refresh(&r.policy.policy.session_id) {
            Refresh::Current(signed) => {
                let valid = policy::verify(&signed, &self.instance_key, &self.machine_id)
                    .is_some_and(|p| {
                        p.member_id == r.policy.policy.member_id
                            && p.session_id == r.policy.policy.session_id
                            && p.revocation_epoch == r.policy.policy.revocation_epoch
                    });
                if !valid {
                    // A raised epoch, another member, or a policy that does
                    // not verify: this session is over.
                    self.end(EndReason::Revoked);
                    return Err(SessionReason::SessionRevoked);
                }
                r.policy = signed;
                r.offline_secs = 0;
                self.link = CoreLink::Online;
                Ok(())
            }
            Refresh::Revoked => {
                self.end(EndReason::Revoked);
                Err(SessionReason::SessionRevoked)
            }
            Refresh::Unreachable => {
                self.link = CoreLink::Unreachable;
                let p = &r.policy.policy;
                if !p.offline_unlock {
                    return Err(SessionReason::OfflineUnlockNotPermitted);
                }
                if p.max_offline_secs.is_some_and(|max| r.offline_secs > max) {
                    self.end(EndReason::OfflinePolicyExhausted);
                    return Err(SessionReason::OfflinePolicyExpired);
                }
                Ok(())
            }
        }
    }

    fn unlock(&mut self, password: &str) -> SessionReply {
        if self.record.is_none() {
            return Self::refused(SessionReason::NotSignedIn);
        }
        if !self.locked {
            return SessionReply::State {
                state: self.state(),
            };
        }
        // The local proof first: an unauthenticated lock screen must not be a
        // way to make this machine question the Core on demand.
        if let Err(reason) = self.check_password(password) {
            return Self::refused(reason);
        }
        if let Err(reason) = self.resynchronise() {
            self.persist();
            return Self::refused(reason);
        }
        self.locked = false;
        self.persist();
        SessionReply::State {
            state: self.state(),
        }
    }

    fn grant(&mut self, binding: &str, class: FreshAuthClass) -> SessionReply {
        if binding.is_empty() || binding.len() > 8192 {
            return Self::refused(SessionReason::Malformed);
        }
        let mut raw = [0u8; 24];
        OsRng.fill_bytes(&mut raw);
        let id = hex::encode(raw);
        let now = self.clock.monotonic_secs();
        self.grants.retain(|g| g.expires_at > now);
        if self.grants.len() >= GRANT_LIMIT {
            self.grants.remove(0);
        }
        self.grants.push(Grant {
            id: id.clone(),
            actor: self.actor(),
            binding: binding.to_owned(),
            class,
            expires_at: now + GRANT_TTL_SECS,
        });
        SessionReply::Granted { grant: id }
    }

    fn consume(&mut self, grant: &str, actor: &str, binding: &str, class: FreshAuthClass) -> bool {
        let now = self.clock.monotonic_secs();
        self.grants.retain(|g| g.expires_at > now);
        let Some(i) = self.grants.iter().position(|g| g.id == grant) else {
            return false;
        };
        // Presenting a grant uses it up, whether or not it fits: a grant that
        // was tried against the wrong request is not left lying around.
        let g = self.grants.remove(i);
        g.actor == actor && g.binding == binding && g.class >= class
    }

    /// Handle one request. `privileged` is true for the System Service (root).
    pub fn handle(&mut self, request: SessionRequest, privileged: bool) -> SessionReply {
        match request {
            SessionRequest::State {} => {
                self.account();
                SessionReply::State {
                    state: self.state(),
                }
            }
            SessionRequest::SignIn {
                username,
                password,
                second_factor,
            } => self.sign_in(
                &username,
                password.expose(),
                second_factor.as_ref().map(|s| s.expose()),
            ),
            SessionRequest::Lock {} => {
                if self.record.is_none() {
                    return Self::refused(SessionReason::NotSignedIn);
                }
                self.locked = true;
                self.grants.clear();
                self.persist();
                SessionReply::State {
                    state: self.state(),
                }
            }
            SessionRequest::Unlock { password } => self.unlock(password.expose()),
            SessionRequest::SignOut {} => {
                if self.record.is_none() {
                    return Self::refused(SessionReason::NotSignedIn);
                }
                self.end(EndReason::SignedOut);
                SessionReply::State {
                    state: self.state(),
                }
            }
            SessionRequest::Confirm { binding } => self.grant(&binding, FreshAuthClass::Confirm),
            SessionRequest::Reauthenticate { binding, password } => {
                if self.record.is_none() {
                    return Self::refused(SessionReason::NotSignedIn);
                }
                if self.locked {
                    return Self::refused(SessionReason::Locked);
                }
                match self.check_password(password.expose()) {
                    Ok(()) => self.grant(&binding, FreshAuthClass::LocalReauth),
                    Err(reason) => Self::refused(reason),
                }
            }
            SessionRequest::ConsumeGrant {
                grant,
                actor,
                binding,
                class,
            } => {
                if !privileged {
                    return Self::refused(SessionReason::NotAuthorized);
                }
                SessionReply::Consumed {
                    valid: self.consume(&grant, &actor, &binding, class),
                }
            }
        }
    }
}

/// Write a file only its owner can read.
fn write_private(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    f.write_all(bytes)?;
    f.sync_all()
}
