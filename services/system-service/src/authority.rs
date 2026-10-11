//! Who is asking, and what that lets them ask.
//!
//! The service never believes an interface's claim about authority. It derives
//! a [`Standing`] from facts it can check itself: the peer credentials of the
//! socket, the Session Service's view of the local session, and the Core's
//! answer for capabilities that need institutional permission.

use ocinye_system_contracts::capability::{
    AuthorizationClass, CapabilitySpec, Family, FreshAuthClass,
};
use ocinye_system_contracts::reason::Reason;

/// Where a connection comes from, by the kernel's word.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Peer {
    /// `root` on this machine: the recovery operator on a console or over the
    /// provisioned administration key. A Linux identity, not an Ocinye member.
    RecoveryOperator,
    /// The unprivileged account the native system interface runs as.
    SystemInterface,
    /// The Core's service account: it forwards authorised proposals.
    Core,
    /// Anyone else. Nothing is offered.
    Other,
}

/// The local session, as the Session Service reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Session {
    /// Nobody is signed in.
    SignedOut,
    /// A member is signed in and the screen is locked.
    Locked {
        /// The member.
        member: String,
    },
    /// A member is signed in and the session is unlocked.
    Active {
        /// The member.
        member: String,
    },
}

/// What a caller stands as for one request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Standing {
    /// Someone at the machine.
    Presence,
    /// A member with an unlocked session.
    Member(String),
    /// The recovery operator.
    RecoveryOperator,
}

impl Standing {
    /// Who the audit record names.
    #[must_use]
    pub fn actor(&self) -> String {
        match self {
            Self::Presence => "presence".to_owned(),
            Self::Member(m) => format!("member:{m}"),
            Self::RecoveryOperator => "recovery-operator".to_owned(),
        }
    }
}

/// The Session Service, as far as this service needs it.
pub trait SessionView: Send + Sync {
    /// The session now.
    fn session(&self) -> Session;
    /// Consume a fresh-authentication grant. True exactly once, and only when
    /// the grant was issued for this actor, this capability and these exact
    /// arguments (`binding` is the canonical request), at least at `class`.
    fn consume_grant(&self, grant: &str, actor: &str, binding: &str, class: FreshAuthClass)
        -> bool;
}

/// What the Core says about a member and a family of capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreAnswer {
    /// The member may.
    Permits,
    /// The member may not.
    Denies,
    /// The Core cannot be asked: the caller gets `AUTHORITY_UNAVAILABLE`,
    /// never a guess.
    Unreachable,
}

/// The Core, for institutional permission.
pub trait CoreAuthority: Send + Sync {
    /// Whether the member may use capabilities of this family.
    fn permits(&self, member: &str, family: Family) -> CoreAnswer;
}

/// Resolve what a peer stands as, given the session.
///
/// # Errors
///
/// [`Reason::NotAuthorized`] for a peer that is offered nothing.
pub fn standing(peer: Peer, session: &Session) -> Result<Standing, Reason> {
    match (peer, session) {
        (Peer::RecoveryOperator, _) => Ok(Standing::RecoveryOperator),
        (Peer::SystemInterface | Peer::Core, Session::Active { member }) => {
            Ok(Standing::Member(member.clone()))
        }
        (Peer::SystemInterface, Session::SignedOut | Session::Locked { .. }) => {
            Ok(Standing::Presence)
        }
        // The Core forwards a member's proposal; without an active session
        // there is no member to act for.
        (Peer::Core, Session::SignedOut | Session::Locked { .. }) | (Peer::Other, _) => {
            Err(Reason::NotAuthorized {
                needs: AuthorizationClass::Presence,
            })
        }
    }
}

/// Whether a standing satisfies a capability's authorisation, asking the Core
/// when the capability needs institutional permission.
///
/// # Errors
///
/// The refusal to return.
pub fn authorise(
    spec: &CapabilitySpec,
    who: &Standing,
    session: &Session,
    core: &dyn CoreAuthority,
) -> Result<(), Reason> {
    if matches!(who, Standing::RecoveryOperator) {
        return Ok(());
    }
    if matches!(session, Session::Locked { .. }) && !spec.while_locked {
        return Err(Reason::SessionLocked);
    }
    match (spec.authorization, who) {
        (AuthorizationClass::Presence, _) | (AuthorizationClass::Member, Standing::Member(_)) => {
            Ok(())
        }
        (AuthorizationClass::Operator, Standing::Member(member)) => {
            match core.permits(member, spec.family) {
                CoreAnswer::Permits => Ok(()),
                CoreAnswer::Denies => Err(Reason::NotAuthorized {
                    needs: AuthorizationClass::Operator,
                }),
                CoreAnswer::Unreachable => Err(Reason::AuthorityUnavailable),
            }
        }
        (needs, _) => Err(Reason::NotAuthorized { needs }),
    }
}
