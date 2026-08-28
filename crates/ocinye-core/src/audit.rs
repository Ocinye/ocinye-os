//! Audit recording.
//!
//! Audit is foundational, not an afterthought (`CLAUDE.md` §37). Every call
//! writes inside the caller's transaction, so an audited action and its audit
//! record commit together or not at all.
//!
//! The table is append-only, enforced by a database trigger installed in
//! migration 0001: the application cannot rewrite its own history even by
//! mistake.

use ocinye_contracts::Classification;
use ocinye_domain::policy::ResourceContext;
use ocinye_domain::Principal;
use ocinye_observability::CorrelationIds;
use serde_json::{Map, Value};
use uuid::Uuid;

use crate::error::CoreResult;
use crate::Tx;

/// Audited action names. Stable: they are queried by reviewers.
pub mod action {
    /// A resource was created.
    pub const CREATE: &str = "create";
    /// A resource was modified.
    pub const UPDATE: &str = "update";
    /// A resource was archived.
    pub const ARCHIVE: &str = "archive";
    /// A workflow transition occurred.
    pub const TRANSITION: &str = "transition";
    /// A classification was changed.
    pub const CLASSIFY: &str = "classify";
    /// A membership was granted, changed or revoked.
    pub const MEMBERSHIP_CHANGE: &str = "membership_change";
    /// A technical role was granted or revoked.
    pub const ROLE_CHANGE: &str = "role_change";
    /// An object was uploaded.
    pub const UPLOAD: &str = "upload";
    /// A stored object was downloaded.
    pub const DOWNLOAD: &str = "download";
    /// Content was exported out of the institution.
    pub const EXPORT: &str = "export";
    /// Something was approved.
    pub const APPROVE: &str = "approve";
    /// Something was published.
    pub const PUBLISH: &str = "publish";
    /// A person was invited.
    pub const INVITE: &str = "invite";
    /// A person signed in.
    pub const SIGN_IN: &str = "sign_in";
    /// A sign-in was refused. Recorded without the credential, ever.
    pub const SIGN_IN_REFUSED: &str = "sign_in_refused";
    /// A person signed out.
    pub const SIGN_OUT: &str = "sign_out";
    /// The first platform administrator was created.
    pub const BOOTSTRAP_ADMIN: &str = "bootstrap_admin";
    /// A member account was created.
    pub const MEMBER_CREATED: &str = "member_created";
    /// A temporary credential was issued.
    pub const TEMPORARY_CREDENTIAL_ISSUED: &str = "temporary_credential_issued";
    /// A person set their own permanent password.
    pub const PASSWORD_SET: &str = "password_set";
    /// An administrator reset a password, issuing a new temporary credential.
    pub const PASSWORD_RESET: &str = "password_reset";
    // Não há acção para «sessão revogada», e é deliberado.
    //
    // Uma sessão nunca é revogada por si: é sempre consequência de outra coisa
    // — a palavra-passe mudou, a conta foi suspensa, um administrador repôs a
    // credencial. Essas acções é que são registadas, e cada uma leva consigo
    // `sessions_revoked` com a contagem. Uma acção própria para a consequência
    // duplicaria o rasto e separaria-o da causa, que é a parte que um revisor
    // precisa de ler.
    /// An account was suspended.
    pub const ACCOUNT_SUSPENDED: &str = "account_suspended";
    /// An account was disabled.
    pub const ACCOUNT_DISABLED: &str = "account_disabled";
    /// An account was reinstated.
    pub const ACCOUNT_REINSTATED: &str = "account_reinstated";
    /// An explicit access grant was created.
    pub const GRANT_CREATED: &str = "grant_created";
    /// An explicit access grant was revoked.
    pub const GRANT_REVOKED: &str = "grant_revoked";
    /// A platform administration operation.
    pub const ADMIN_OPERATION: &str = "admin_operation";
    /// An authorization denial worth recording.
    pub const SECURITY_DENIAL: &str = "security_denial";
    /// A compute node was enrolled.
    pub const NODE_ENROLLMENT: &str = "node_enrollment";

    // ── Agentic Control Plane ───────────────────────────────────────────
    //
    // What is recorded is *that* an agent asked, through whom, for what, and
    // how it ended. Never the prompt, never the model's reasoning, never the
    // input — those carry a member's own words (briefing §48, §102).

    /// A capability was executed, or refused, on an agent's proposal.
    pub const CAPABILITY_EXECUTED: &str = "capability_executed";
    /// An action plan was built from a member's request.
    pub const PLAN_CREATED: &str = "plan_created";
    /// A member confirmed a plan.
    pub const PLAN_APPROVED: &str = "plan_approved";
    /// A member refused a plan, or it expired.
    pub const PLAN_REJECTED: &str = "plan_rejected";
    /// A plan was run, and settled with a factual outcome.
    ///
    /// Distinct from [`CAPABILITY_EXECUTED`], which records each step. This
    /// records the lifecycle: the plan was claimed for execution, and here is
    /// how it ended.
    pub const PLAN_EXECUTED: &str = "plan_executed";
}

/// Outcome recorded alongside an action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The action completed.
    Success,
    /// The action was refused by the policy.
    Denied,
    /// The action failed for a non-authorization reason.
    Failure,
}

impl Outcome {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Denied => "denied",
            Self::Failure => "failure",
        }
    }
}

/// Metadata keys that must never reach the audit trail.
///
/// A backstop, not a licence: callers are expected not to pass these at all.
const FORBIDDEN_METADATA_KEYS: &[&str] = &[
    "content", "body", "password", "token", "secret", "prompt", "file", "payload",
];

/// One audit record to be written.
#[derive(Debug, Clone)]
pub struct AuditEntry {
    /// What happened.
    pub action: &'static str,
    /// Kind of resource acted upon.
    pub resource_type: &'static str,
    /// Identifier of the resource, when it has one.
    pub resource_id: Option<Uuid>,
    /// Owning unit.
    pub unit_id: Option<Uuid>,
    /// Owning research workspace.
    pub workspace_id: Option<Uuid>,
    /// Classification of the resource at the time of the action.
    pub classification: Option<Classification>,
    /// Result of the action.
    pub outcome: Outcome,
    /// Bounded, non-sensitive detail.
    pub metadata: Map<String, Value>,
    /// Actor, when there is no `Principal` to take it from.
    ///
    /// Sign-in, password change and bootstrap all happen *before* a principal
    /// can be assembled, and they are precisely the events that must be
    /// attributable. Without this the identity trail would be anonymous exactly
    /// where it matters most (briefing §88).
    pub actor: Option<(Uuid, Uuid)>,
}

impl AuditEntry {
    /// Start an entry for a successful action.
    #[must_use]
    pub fn new(action: &'static str, resource_type: &'static str) -> Self {
        Self {
            action,
            resource_type,
            resource_id: None,
            unit_id: None,
            workspace_id: None,
            classification: None,
            outcome: Outcome::Success,
            metadata: Map::new(),
            actor: None,
        }
    }

    /// Attribute the entry to a person explicitly.
    ///
    /// Takes the organisation too, because an audit row without one cannot be
    /// scoped by any later query.
    #[must_use]
    pub const fn actor(mut self, person_id: Uuid, organisation_id: Uuid) -> Self {
        self.actor = Some((person_id, organisation_id));
        self
    }

    /// Attach the resource identifier.
    #[must_use]
    pub const fn resource(mut self, id: Uuid) -> Self {
        self.resource_id = Some(id);
        self
    }

    /// Attach scope and classification from an authorization context.
    #[must_use]
    pub const fn context(mut self, ctx: &ResourceContext) -> Self {
        self.unit_id = ctx.unit_id;
        self.workspace_id = ctx.workspace_id;
        self.classification = Some(ctx.classification);
        self
    }

    /// Attach scope explicitly.
    #[must_use]
    pub const fn scope(mut self, unit_id: Option<Uuid>, workspace_id: Option<Uuid>) -> Self {
        self.unit_id = unit_id;
        self.workspace_id = workspace_id;
        self
    }

    /// Attach the classification.
    #[must_use]
    pub const fn classified(mut self, classification: Classification) -> Self {
        self.classification = Some(classification);
        self
    }

    /// Mark the outcome.
    #[must_use]
    pub const fn outcome(mut self, outcome: Outcome) -> Self {
        self.outcome = outcome;
        self
    }

    /// Add a detail. Forbidden keys are dropped.
    #[must_use]
    pub fn detail(mut self, key: &str, value: impl Into<Value>) -> Self {
        if !FORBIDDEN_METADATA_KEYS.contains(&key.to_ascii_lowercase().as_str()) {
            self.metadata.insert(key.to_owned(), value.into());
        }
        self
    }
}

/// Write an audit record inside the caller's transaction.
///
/// # Errors
///
/// Returns an error when the insert fails, which aborts the whole operation:
/// an unauditable action is not performed.
pub async fn record(
    tx: &mut Tx<'_>,
    principal: Option<&Principal>,
    ids: &CorrelationIds,
    entry: AuditEntry,
) -> CoreResult<()> {
    sqlx::query(
        r"
        INSERT INTO audit_events (
            organisation_id, actor_person_id, actor_subject, action, resource_type,
            resource_id, unit_id, workspace_id, classification, outcome,
            request_id, correlation_id, metadata
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
        ",
    )
    // An explicit actor wins: it is only ever set where no principal exists.
    .bind(
        principal
            .map(|p| p.organisation_id)
            .or(entry.actor.map(|(_, organisation_id)| organisation_id)),
    )
    .bind(
        principal
            .map(|p| p.person_id)
            .or(entry.actor.map(|(person_id, _)| person_id)),
    )
    .bind(principal.map(|p| p.subject.as_str()))
    .bind(entry.action)
    .bind(entry.resource_type)
    .bind(entry.resource_id)
    .bind(entry.unit_id)
    .bind(entry.workspace_id)
    .bind(entry.classification.map(|c| c.as_str()))
    .bind(entry.outcome.as_str())
    .bind(ids.request_id.as_str())
    .bind(ids.correlation_id.as_str())
    .bind(Value::Object(entry.metadata))
    .execute(&mut **tx)
    .await?;
    Ok(())
}

// Não há aqui um `record_denial`, e a razão é a assinatura.
//
// Existiu um, que recebia um `Decision` do domínio. Nenhum dos três sítios que
// registam negações tem um `Decision` na mão: o correio recusa uma expedição
// classificada, o ciclo de vida agentic recusa um plano sem confirmação, e o
// executor recusa uma invocação — e cada um escreve o seu `AuditEntry` com os
// detalhes que só ele conhece. Era um auxiliar desenhado para uma chamada que
// nunca aconteceu, e consolidar os três numa assinatura que nenhum satisfaz
// seria forçar uma abstracção para ter menos linhas.
//
// O princípio que a sua documentação guardava — a razão da negação fica no
// registo, e não volta para quem chamou como pista (ADR-0100) — está provado
// onde se pode provar: `error.rs`, em `denial_reasons_never_reach_the_caller` e
// `a_denied_read_is_indistinguishable_from_absence`.

/// Record an event outside any surrounding transaction.
///
/// For events that are complete in themselves — signing out, for instance —
/// where there is no state change to be atomic with. Best-effort: a failure to
/// write evidence is logged loudly and does not change the operation's outcome,
/// because refusing to sign someone out because the audit table is full would
/// be the worse failure.
pub async fn record_standalone(
    pool: &sqlx::PgPool,
    ids: &CorrelationIds,
    actor_person_id: Uuid,
    organisation_id: Uuid,
    action: &'static str,
    resource_type: &'static str,
    resource_id: Uuid,
) {
    let result = sqlx::query(
        r"
        INSERT INTO audit_events (
            organisation_id, actor_person_id, action, resource_type, resource_id,
            outcome, request_id, correlation_id, metadata
        )
        VALUES ($1, $2, $3, $4, $5, 'success', $6, $7, '{}'::jsonb)
        ",
    )
    .bind(organisation_id)
    .bind(actor_person_id)
    .bind(action)
    .bind(resource_type)
    .bind(resource_id)
    .bind(ids.request_id.as_str())
    .bind(ids.correlation_id.as_str())
    .execute(pool)
    .await;

    if let Err(error) = result {
        tracing::error!(error = %error, action, "could not record an audit event");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forbidden_metadata_keys_are_dropped() {
        let entry = AuditEntry::new(action::UPLOAD, "document")
            .detail("content", "the whole document")
            .detail("password", "hunter2")
            .detail("size_bytes", 1024);

        assert!(!entry.metadata.contains_key("content"));
        assert!(!entry.metadata.contains_key("password"));
        assert_eq!(entry.metadata.get("size_bytes"), Some(&Value::from(1024)));
    }

    #[test]
    fn forbidden_keys_are_matched_case_insensitively() {
        let entry = AuditEntry::new(action::UPLOAD, "document").detail("Content", "x");
        assert!(entry.metadata.is_empty());
    }
}
