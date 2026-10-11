//! Contracts of the Ocinye Session Service: the authority of the local
//! interactive session.
//!
//! The Core says who a member is and what they may do. The Session Service
//! says whether that member's session on **this** machine is active or locked,
//! and unlocks it — without the Core, when the policy the Core signed allows.
//! It is not an identity provider, and a Linux account is not a member.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::capability::FreshAuthClass;
use crate::request::Secret;

/// Where the Session Service listens.
pub const SESSION_SOCKET_PATH: &str = "/run/ocinye/session.sock";

/// What the Core lets a local session do while the Core cannot be asked. The
/// Core signs it at sign-in and on every resynchronisation; the values are
/// Instance policy, not constants of this system.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct SessionPolicy {
    /// The member, by the Core's identifier.
    pub member_id: String,
    /// The name to show on the lock screen.
    pub member_name: String,
    /// The Core session this local session belongs to.
    pub session_id: String,
    /// The machine the policy was issued for.
    pub machine_id: String,
    /// Whether the member administers the platform this machine runs: the
    /// Core's answer, signed, to "may this member operate the machine". The
    /// System Service asks for nothing else before an operator capability.
    pub operator: bool,
    /// Raised by the Core when sessions of this member must end.
    #[ts(type = "number")]
    pub revocation_epoch: u64,
    /// Whether the session may be unlocked while the Core is unreachable.
    pub offline_unlock: bool,
    /// How long, in seconds, the session may go without the Core confirming
    /// it. `None`: the Instance sets no limit.
    #[ts(type = "number | null")]
    pub max_offline_secs: Option<u64>,
    /// Failed unlock attempts after which the local verifier is destroyed.
    pub max_unlock_attempts: u32,
    /// Idle seconds before the session locks itself.
    pub idle_lock_secs: u32,
    /// When the Core issued it, Unix seconds.
    #[ts(type = "number")]
    pub issued_at: i64,
}

impl SessionPolicy {
    /// The bytes the Core's signature covers.
    ///
    /// A fixed, versioned encoding of this exact structure — a domain label,
    /// then every field in declaration order, strings length-prefixed and
    /// integers big-endian — so that the signer and the verifier cannot
    /// disagree through a serialiser's settings, and no two different
    /// policies share an encoding.
    #[must_use]
    pub fn signed_bytes(&self) -> Vec<u8> {
        let mut out = b"ocinye/session-policy/v1\0".to_vec();
        for text in [
            &self.member_id,
            &self.member_name,
            &self.session_id,
            &self.machine_id,
        ] {
            out.extend_from_slice(&(text.len() as u64).to_be_bytes());
            out.extend_from_slice(text.as_bytes());
        }
        out.push(u8::from(self.operator));
        out.extend_from_slice(&self.revocation_epoch.to_be_bytes());
        out.push(u8::from(self.offline_unlock));
        match self.max_offline_secs {
            None => out.push(0),
            Some(secs) => {
                out.push(1);
                out.extend_from_slice(&secs.to_be_bytes());
            }
        }
        out.extend_from_slice(&self.max_unlock_attempts.to_be_bytes());
        out.extend_from_slice(&self.idle_lock_secs.to_be_bytes());
        out.extend_from_slice(&self.issued_at.to_be_bytes());
        out
    }
}

/// A policy with the Core's signature over [`SessionPolicy::signed_bytes`]
/// (Ed25519, hex).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct SignedSessionPolicy {
    /// The policy.
    pub policy: SessionPolicy,
    /// The signature.
    pub signature: String,
}

/// Whether the Core can be asked right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CoreLink {
    /// Reachable, and the policy was confirmed.
    Online,
    /// Not reachable: the last signed policy governs.
    Unreachable,
}

/// Why a session ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EndReason {
    /// The member signed out.
    SignedOut,
    /// The Core revoked the session or raised the revocation epoch.
    Revoked,
    /// The offline policy no longer allows the session to continue.
    OfflinePolicyExhausted,
    /// Too many failed unlock attempts: the verifier was destroyed.
    TooManyAttempts,
    /// The machine restarted: a session does not survive that.
    Restarted,
}

/// The local session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "state", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SessionState {
    /// Nobody is signed in.
    SignedOut {
        /// Why the previous session ended, shown once.
        ended: Option<EndReason>,
    },
    /// A member is signed in and the session is unlocked.
    Active {
        /// The member's identifier.
        member_id: String,
        /// The member's name.
        member_name: String,
        /// The Core link.
        core: CoreLink,
        /// Whether the Core says this member may operate the machine.
        operator: bool,
        /// Idle seconds after which the session must lock itself; zero when
        /// the Instance sets no idle lock. The compositor, which sees the
        /// input, asks for the lock.
        idle_lock_secs: u32,
    },
    /// A member is signed in and the screen is locked.
    Locked {
        /// The member's name: the lock screen shows whose session it is.
        member_name: String,
        /// The Core link.
        core: CoreLink,
        /// Seconds until another attempt is accepted; zero when one is.
        #[ts(type = "number")]
        retry_after_secs: u64,
    },
}

/// What a client asks of the Session Service.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "request", rename_all = "snake_case", deny_unknown_fields)]
pub enum SessionRequest {
    /// The session now.
    State {},
    /// Initial sign-in. Needs the Core.
    SignIn {
        /// The member's user name.
        username: String,
        /// The password.
        password: Secret,
        /// A second factor, when the Core asked for one.
        second_factor: Option<Secret>,
        /// The password the member chooses, when the Core said the one above
        /// is temporary and must be replaced before any session can open.
        new_password: Option<Secret>,
    },
    /// Start enrolling a second factor for a member whose account requires
    /// one and has none: the Core generates the authenticator key, and it is
    /// handed to the trusted screen to be typed into an authenticator. The
    /// enrolment is closed by a [`SessionRequest::SignIn`] carrying a code
    /// from that authenticator.
    BeginSecondFactorEnrollment {
        /// The member's user name.
        username: String,
        /// The password.
        password: Secret,
    },
    /// Lock the session.
    Lock {},
    /// Unlock the session. Does not need the Core.
    Unlock {
        /// The password.
        password: Secret,
    },
    /// End the session.
    SignOut {},
    /// A trusted confirmation was given for one exact request: issue a grant.
    Confirm {
        /// The canonical request the grant is for.
        binding: String,
    },
    /// The member proved who they are again, for one exact request.
    Reauthenticate {
        /// The canonical request the grant is for.
        binding: String,
        /// The password.
        password: Secret,
    },
    /// Consume a grant. Only the System Service may ask.
    ConsumeGrant {
        /// The grant.
        grant: String,
        /// Who presented it.
        actor: String,
        /// The canonical request it is presented for.
        binding: String,
        /// The class the capability needs.
        class: FreshAuthClass,
    },
}

/// Why the Session Service refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "code", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SessionReason {
    /// The request did not parse.
    Malformed,
    /// This peer may not ask that.
    NotAuthorized,
    /// The credentials were not accepted.
    InvalidCredentials,
    /// The Core asks for a second factor.
    SecondFactorRequired,
    /// The account requires a second factor and has none enrolled yet.
    SecondFactorEnrollmentRequired,
    /// The member must set a password in the Workspace before a session can
    /// open here.
    PasswordChangeRequired,
    /// The Core did not accept the new password: it does not meet the
    /// Instance's password policy.
    PasswordNotAccepted,
    /// Too many attempts: wait.
    Throttled {
        /// Seconds to wait.
        #[ts(type = "number")]
        retry_after_secs: u64,
    },
    /// Signing in needs the Core, and it cannot be reached.
    CoreUnreachable,
    /// Nobody is signed in.
    NotSignedIn,
    /// Someone is already signed in.
    AlreadySignedIn,
    /// The session is locked.
    Locked,
    /// The Instance does not allow unlocking without the Core.
    OfflineUnlockNotPermitted,
    /// The session went too long without the Core confirming it.
    OfflinePolicyExpired,
    /// The Core ended the session.
    SessionRevoked,
    /// The policy the Core sent is not validly signed for this machine.
    PolicyInvalid,
}

/// The Session Service's answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "reply", rename_all = "snake_case")]
pub enum SessionReply {
    /// The session, after the request.
    State {
        /// The state.
        state: SessionState,
    },
    /// A grant for one exact request.
    Granted {
        /// The grant: opaque, single use, short lived.
        grant: String,
    },
    /// Whether a grant was valid and is now consumed.
    Consumed {
        /// True exactly once.
        valid: bool,
    },
    /// The authenticator key of an enrolment that has begun. Shown once, on
    /// the trusted screen.
    SecondFactorSecret {
        /// The key, base32.
        secret_base32: Secret,
    },
    /// Signed in, having just enrolled a second factor: the session, and the
    /// recovery codes the Core issued. They are shown once.
    Enrolled {
        /// The session.
        state: SessionState,
        /// The recovery codes.
        recovery_codes: Vec<Secret>,
    },
    /// Refused.
    Refused {
        /// Why.
        reason: SessionReason,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_palavra_passe_nao_aparece_em_debug() {
        let r: SessionRequest =
            serde_json::from_str(r#"{"request":"unlock","password":"correct horse"}"#).unwrap();
        assert!(!format!("{r:?}").contains("correct horse"));
    }

    #[test]
    fn um_campo_a_mais_nao_entra() {
        assert!(serde_json::from_str::<SessionRequest>(
            r#"{"request":"unlock","password":"x","skip":true}"#
        )
        .is_err());
        assert!(serde_json::from_str::<SessionRequest>(r#"{"request":"force_unlock"}"#).is_err());
        assert!(serde_json::from_str::<SessionPolicy>(
            r#"{"member_id":"m","offline_unlock":true}"#
        )
        .is_err());
    }
}
