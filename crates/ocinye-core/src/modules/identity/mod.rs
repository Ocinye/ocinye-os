//! Identity: people, their link to the identity provider, technical roles and
//! invitations.
//!
//! # What belongs here
//!
//! The mapping between a verified OIDC subject and a person of the institution,
//! the technical roles granted to that person, and the invitation flow that
//! creates them.
//!
//! # Credentials
//!
//! Under ADR-0103 the Core *is* the authentication authority: it stores
//! Argon2id verifiers and owns the password lifecycle. It still stores no
//! password, in any form, at any moment — only verifiers ([`credentials`]).
//!
//! # What does not belong here
//!
//! Institutional position lives on a person but grants nothing — it is never
//! consulted by the policy (ADR-0100).

mod accounts;
mod authentication;
mod avatar;
mod credentials;
mod model;
mod repository;
mod service;

pub use accounts::{
    bootstrap_platform_admin, change_own_password, create_member, reset_password,
    set_account_status, set_permanent_password, validate_username, NewMember, TemporaryCredential,
    DEFAULT_TEMPORARY_CREDENTIAL_HOURS,
};
pub use authentication::{
    AttemptContext, Authenticator, IssuedSession, Throttle, PASSWORD_CHANGE_SESSION_MINUTES,
    SESSION_LIFETIME_HOURS,
};
pub use avatar::{choose_preset, own_avatar, own_photograph_key, set_photograph, use_initials};
pub use credentials::{
    find_session, list_own_sessions, list_sessions, live_grants, revoke_all_sessions,
    revoke_own_session, revoke_session, session_digest, sweep_expired, touch_session, Credential,
    StoredSession,
};
pub use model::{Invitation, InvitationStatus, Person};
pub use service::{
    accept_invitation, attempt_summary, create_invitation, get_own_person, get_person, grant_role,
    list_people, live_credentials_for, load_principal, person_by_id, principal_for_person,
    principal_within, revoke_role, IssuedInvitation, NewInvitation,
};
