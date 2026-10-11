//! The one path every request takes.
//!
//! ```text
//! request ─▶ standing ─▶ authorise ─▶ fresh proof ─▶ validate ─▶ platform ─▶ audit ─▶ outcome
//! ```
//!
//! Authorisation comes before validation: a validation error describes the
//! shape a request should have had, and that is not owed to someone who may
//! not make it. Nothing reaches the platform before all four gates pass.

use ocinye_system_contracts::capability::{spec, AuditClass, FreshAuthClass};
use ocinye_system_contracts::reason::Reason;
use ocinye_system_contracts::request::{Outcome, Payload, Request};

use crate::audit::{AuditRecord, AuditSink};
use crate::authority::{authorise, standing, CoreAuthority, Peer, SessionView, Standing};

/// The system underneath: the adapters. One implementation talks to Linux;
/// tests use another.
pub trait Platform: Send + Sync {
    /// Carry out a validated, authorised request and report the state read
    /// back.
    ///
    /// # Errors
    ///
    /// A typed reason. Never a tool's text.
    fn perform(&self, request: &Request) -> Result<Payload, Reason>;
}

/// What the service is built from.
pub struct Service<'a> {
    /// The adapters.
    pub platform: &'a dyn Platform,
    /// The local session.
    pub session: &'a dyn SessionView,
    /// The Core.
    pub core: &'a dyn CoreAuthority,
    /// Where audit records go.
    pub audit: &'a dyn AuditSink,
}

impl Service<'_> {
    /// Handle one call.
    pub fn call(&self, peer: Peer, grant: Option<&str>, request: &Request) -> Outcome {
        let spec = spec(request.capability());
        let session = self.session.session();
        let who = match standing(peer, &session) {
            Ok(who) => who,
            Err(reason) => return self.refuse(spec.audit, "unknown", request, reason),
        };
        let actor = who.actor();
        if let Err(reason) = authorise(spec, &who, &session, self.core) {
            return self.refuse(spec.audit, &actor, request, reason);
        }
        // The recovery operator is already root on this machine: a prompt
        // would prove nothing. Everyone else presents a grant bound to exactly
        // this request.
        if spec.fresh_auth != FreshAuthClass::None && !matches!(who, Standing::RecoveryOperator) {
            let binding = serde_json::to_string(request).unwrap_or_default();
            let held = grant.is_some_and(|g| {
                self.session
                    .consume_grant(g, &actor, &binding, spec.fresh_auth)
            });
            if !held {
                let reason = if grant.is_some() {
                    Reason::GrantInvalid
                } else {
                    Reason::FreshAuthRequired {
                        class: spec.fresh_auth,
                    }
                };
                return self.refuse(spec.audit, &actor, request, reason);
            }
        }
        if let Err(reason) = request.validate() {
            return self.refuse(spec.audit, &actor, request, reason);
        }
        match self.platform.perform(request) {
            Ok(payload) => {
                if spec.audit != AuditClass::None {
                    self.audit.record(&AuditRecord::new(&actor, request, "OK"));
                }
                Outcome::Ok { payload }
            }
            Err(reason) => self.refuse(spec.audit, &actor, request, reason),
        }
    }

    fn refuse(&self, class: AuditClass, actor: &str, request: &Request, reason: Reason) -> Outcome {
        // A refused security-relevant request is itself worth a record.
        if class == AuditClass::Security {
            self.audit
                .record(&AuditRecord::new(actor, request, reason.code()));
        }
        Outcome::Refused { reason }
    }
}
