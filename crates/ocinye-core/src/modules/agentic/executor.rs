//! The Capability Executor.
//!
//! # The one place agentic action becomes institutional state
//!
//! Every agentic mutation in the Ocinye OS passes through [`execute`]. That is
//! the point: one function to read when asking «what could an agent do to this
//! system», one place where the order of the checks is visible, and one place
//! to add a gate that then applies everywhere.
//!
//! # The order
//!
//! ```text
//! resolve capability  →  a name a model invented resolves to nothing
//! resolve resources   →  each named resource, through the service that owns it
//! authorise           →  against each resource's own context, or the request's
//! validate input      →  against the published schema, before anything runs
//! approval gate       →  external and privileged always need a person
//! execute             →  the domain service, which owns the invariant
//! audit               →  what was asked, by whom, through which agent
//! ```
//!
//! **Authorisation comes before validation, and that order is deliberate.** A
//! validation error describes the shape of a capability's input. Returning one
//! to somebody who may not use the capability hands them a map of an interface
//! they have no business seeing.
//!
//! # Where the authorization context comes from
//!
//! **From the resource, when the step names one.** This is the correction at
//! the centre of ADR-0306, and it is worth stating plainly because the earlier
//! arrangement was wrong in a way that was easy to miss.
//!
//! A plan step arrives with no context of its own — the request that carried it
//! is about the institution, not about any unit or workspace. Authorising
//! against *that* asks «does this person hold this permission across the whole
//! institution», which for a workspace-scoped operation is the wrong question:
//! it refuses a workspace lead acting inside their own workspace, and it names
//! no unit for a foreign reference to be outside of. It failed closed, so
//! nothing was ever wrongly permitted — but it also meant most of Research and
//! Knowledge could not be reached through this plane at all.
//!
//! So when a step names resources, each one is resolved first and the decision
//! is taken against **its** context: real unit, real workspace, real
//! classification. That is stricter where it matters — a reference into
//! another unit is now refused by name rather than by accident — and correct
//! where it did not work at all.
//!
//! A step that names no resource keeps the request's own context, which for
//! institution-wide capabilities such as search is exactly right.
//!
//! Authorisation is **before** execution and verification is **after**
//! (briefing §46, §55). Nothing here takes the model's word for anything.

use ocinye_contracts::agentic::{
    CapabilityDescriptor, CapabilityRequest, CapabilityResult, ExecutionStatus, ResourceKind,
    Reversibility,
};
use ocinye_domain::{
    approval_needed, effective_risk, may_invoke, AgentBoundary, AgenticRefusal, Principal,
    ResourceContext,
};
use ocinye_observability::CorrelationIds;
use sqlx::PgPool;
use uuid::Uuid;

use super::registry::registry;
use super::resolver::{self, ResolvedResource};
use crate::audit::{self, action, AuditEntry};
use crate::error::{CoreError, CoreResult};

/// Everything a handler is given.
///
/// # What is deliberately absent
///
/// No shell, no filesystem, no HTTP client, no secrets. A handler has a pool —
/// because domain services need one — and the acting principal. It reaches the
/// institution the same way an HTTP route does: through a domain service that
/// owns the invariant (briefing §6).
pub struct ExecutionContext<'a> {
    /// Database pool, for the domain service to use.
    pub pool: &'a PgPool,
    /// Who is acting. **Not the agent** — the person.
    pub principal: &'a Principal,
    /// The input, already validated against the descriptor's schema.
    pub input: &'a serde_json::Value,
    /// What it acts on: looked up in the Core, and readable by this person.
    ///
    /// A handler may use these without checking again. It may **not** use
    /// `CapabilityRequest::resources`, which is whatever the model wrote.
    pub resources: &'a [ResolvedResource],
    /// O Capability Runtime, para uma capacidade que precise de computação
    /// isolada.
    ///
    /// Está aqui e não dentro do handler porque a escolha do componente é do
    /// Core: um handler recebe a porta, e nunca a abre por conta própria.
    pub capabilities: &'a crate::capabilities::Capabilities,
    /// Describe the effect instead of causing it.
    pub dry_run: bool,
    /// Correlation, carried end to end.
    pub ids: &'a CorrelationIds,
}

impl ExecutionContext<'_> {
    /// Read a typed field from the input.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Validation`] when the field is absent or the wrong
    /// shape. Handlers use this rather than indexing, so a model that proposed
    /// a number where a string belongs produces a validation error and not a
    /// panic (briefing §173).
    pub fn field<T: serde::de::DeserializeOwned>(&self, name: &str) -> CoreResult<T> {
        let value = self
            .input
            .get(name)
            .ok_or_else(|| CoreError::Validation(format!("Falta o campo «{name}».")))?;

        serde_json::from_value(value.clone())
            .map_err(|_| CoreError::Validation(format!("O campo «{name}» tem o tipo errado.")))
    }

    /// Read an optional typed field.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Validation`] when present and the wrong shape.
    pub fn optional<T: serde::de::DeserializeOwned>(&self, name: &str) -> CoreResult<Option<T>> {
        match self.input.get(name) {
            None | Some(serde_json::Value::Null) => Ok(None),
            Some(value) => serde_json::from_value(value.clone())
                .map(Some)
                .map_err(|_| CoreError::Validation(format!("O campo «{name}» tem o tipo errado."))),
        }
    }

    /// The one resource of a given kind this step names.
    ///
    /// # Why handlers address by reference and not by identifier
    ///
    /// An identifier in `input` is a string a model wrote, and reading it would
    /// route around the gate that resolved it. `resources` is the channel the
    /// executor checks, so it is the only channel a handler may use to say
    /// *which* thing (ADR-0306).
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Validation`] when the step names no resource of
    /// that kind, or more than one.
    pub fn one(&self, kind: ResourceKind) -> CoreResult<&ResolvedResource> {
        let mut matching = self
            .resources
            .iter()
            .filter(|resource| resource.reference.kind == kind);

        let first = matching.next().ok_or_else(|| {
            CoreError::Validation(format!(
                "Esta operação precisa de indicar {}.",
                article(kind)
            ))
        })?;

        if matching.next().is_some() {
            return Err(CoreError::Validation(format!(
                "Esta operação aceita apenas {}.",
                article(kind)
            )));
        }
        Ok(first)
    }

    /// A trimmed, non-empty string field.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Validation`] when absent, the wrong type, or blank.
    pub fn text(&self, name: &str) -> CoreResult<String> {
        let raw: String = self.field(name)?;
        let trimmed = raw.trim();

        if trimmed.is_empty() {
            return Err(CoreError::Validation(format!(
                "O campo «{name}» não pode estar vazio."
            )));
        }
        Ok(trimmed.to_owned())
    }
}

/// How to name a kind of resource in a refusal, in Portuguese.
fn article(kind: ResourceKind) -> &'static str {
    match kind {
        ResourceKind::Idea => "uma Ideia",
        ResourceKind::Project => "um Projecto",
        ResourceKind::Workspace => "um Research Workspace",
        ResourceKind::Note => "uma Nota",
        ResourceKind::Source => "uma fonte bibliográfica",
        ResourceKind::Document => "um Documento",
        ResourceKind::Task => "uma tarefa",
        other => other.as_str(),
    }
}

/// Validate a proposed input against a published schema.
///
/// # A deliberately small validator
///
/// It checks the things a model actually gets wrong: absent required fields,
/// wrong JSON types, and a non-object where an object belongs. It is not a
/// complete JSON Schema implementation, and pulling one in would be a large
/// dependency for a job the handlers finish anyway — every handler types its
/// own input, and a value that survives this and fails there produces the same
/// validation error (`CLAUDE.md` §54).
fn validate_against_schema(
    input: &serde_json::Value,
    schema: &serde_json::Value,
) -> Result<(), String> {
    let Some(properties) = schema.get("properties").and_then(|p| p.as_object()) else {
        return Ok(());
    };

    let Some(object) = input.as_object() else {
        return Err("O pedido deve ser um objecto.".to_owned());
    };

    // Required fields must be present *and* carry a value. An explicit `null`
    // is absence written out, and treating it as presence lets a model satisfy
    // a required field with nothing.
    if let Some(required) = schema.get("required").and_then(|r| r.as_array()) {
        for name in required.iter().filter_map(|n| n.as_str()) {
            match object.get(name) {
                None | Some(serde_json::Value::Null) => {
                    return Err(format!("Falta o campo «{name}»."))
                }
                Some(_) => {}
            }
        }
    }

    // Present fields must have the declared type.
    for (name, value) in object {
        let Some(expected) = properties
            .get(name)
            .and_then(|p| p.get("type"))
            .and_then(|t| t.as_str())
        else {
            // A field the schema does not describe is ignored rather than
            // refused: handlers read what they need, and an extra key a model
            // added is harmless noise.
            continue;
        };

        let matches = match expected {
            "string" => value.is_string(),
            "boolean" => value.is_boolean(),
            "integer" => value.is_i64() || value.is_u64(),
            "number" => value.is_number(),
            "array" => value.is_array(),
            "object" => value.is_object(),
            _ => true,
        };

        if !matches && !value.is_null() {
            return Err(format!("O campo «{name}» deve ser do tipo {expected}."));
        }
    }

    Ok(())
}

/// A result that says nothing happened, and why.
fn refused(
    descriptor: &CapabilityDescriptor,
    status: ExecutionStatus,
    detail: impl Into<String>,
) -> CapabilityResult {
    CapabilityResult {
        capability: descriptor.id.clone(),
        status,
        resources: Vec::new(),
        detail: detail.into(),
        // Nothing ran, so there is nothing to undo. Saying otherwise would put
        // an Undo affordance on an action that never happened.
        reversibility: Reversibility::NothingToUndo,
        output: None,
    }
}

/// Run one capability, or refuse it.
///
/// # Never returns an error for a refusal
///
/// A refusal is an outcome, not a failure: a plan with a denied step still has
/// steps that ran, and the member needs to see both. `Err` is reserved for the
/// executor itself being unable to reach a conclusion.
///
/// # Errors
///
/// Returns an error when audit cannot be written — because an unaudited
/// agentic mutation is worse than a refused one.
#[expect(
    clippy::too_many_arguments,
    reason = "each argument is a distinct gate"
)]
pub async fn execute(
    pool: &PgPool,
    capabilities: &crate::capabilities::Capabilities,
    principal: &Principal,
    agent: &AgentBoundary,
    agent_id: Option<Uuid>,
    request: &CapabilityRequest,
    ctx: &ResourceContext,
    approved: bool,
    ids: &CorrelationIds,
) -> CoreResult<CapabilityResult> {
    // ── 0. Autoridade ───────────────────────────────────────────────────
    //
    // A fronteira, no ponto em que o efeito acontece.
    //
    // O `principal` que chegou aqui identifica quem age; a partir daqui não
    // autoriza nada. A autoridade volta a estabelecer-se à fonte canónica,
    // imediatamente antes de qualquer capability correr (ADR-0411).
    //
    // Estava no ciclo de vida do plano, que é o único chamador em produção — e
    // por isso não havia fuga. Mas a garantia dependia de quem chamasse se
    // lembrar, e um segundo chamador não herdaria esse cuidado. Aqui não há como
    // não passar por ela.
    //
    // Fecha em caso de dúvida: se não conseguirmos saber quem a pessoa é agora,
    // nada corre.
    let autoridade =
        crate::authority::resolve(pool, crate::authority::ActorRef::of(principal)).await?;
    let principal = autoridade.principal();

    // ── 1. Resolve ──────────────────────────────────────────────────────
    //
    // A name a model invented resolves to nothing. This is the whole defence
    // against hallucinated capabilities (briefing §161).
    let Some(handler) = registry().get(&request.capability) else {
        return Ok(CapabilityResult {
            capability: request.capability.clone(),
            status: ExecutionStatus::ValidationFailed,
            resources: Vec::new(),
            detail: "Esta operação não existe no Ocinye OS.".to_owned(),
            reversibility: Reversibility::NothingToUndo,
            output: None,
        });
    };

    let descriptor = handler.descriptor();

    // ── 2. Resolve the resources it names ───────────────────────────────
    //
    // Before any decision, because the decision depends on what these are.
    // Until here a `ResourceRef` is a claim a model made; resolution turns it
    // into a row this person may read, or into nothing at all. A hallucinated
    // identifier and one belonging to another unit produce the same answer, and
    // that answer arrives before the input schema is ever described.
    let resolved = match resolver::resolve_all(pool, principal, &request.resources).await {
        Ok(resolved) => resolved,
        Err(CoreError::NotFound(reason)) => {
            return Ok(refused(
                &descriptor,
                ExecutionStatus::ResourceNotFound,
                reason,
            ))
        }
        Err(error) => return Err(error),
    };

    // ── 3. Authorise ────────────────────────────────────────────────────
    //
    // Against each resource's own context when the step names any, and against
    // the request's when it does not. Actor first; every later gate narrows.
    //
    // And **before** validation: a validation error describes the shape of a
    // capability's input, and handing that to somebody who may not use the
    // capability is a map of an interface they have no business seeing.
    let decisions: Vec<(&ResourceContext, Option<Uuid>)> = if resolved.is_empty() {
        vec![(ctx, None)]
    } else {
        resolved
            .iter()
            .map(|resource| (&resource.context, Some(resource.reference.id)))
            .collect()
    };

    for (context, resource_id) in decisions {
        if let Err(refusal) = may_invoke(principal, agent, &descriptor, context, resource_id) {
            record(
                pool,
                principal,
                agent_id,
                &descriptor,
                ExecutionStatus::PermissionDenied,
                Some(refusal),
                ids,
            )
            .await?;

            // Every refusal reads as `PermissionDenied` outward. The *reason*
            // is in the message and in the audit row; the status does not
            // distinguish them, because telling a caller which gate stopped
            // them draws a map of the boundary for whoever is probing it.
            return Ok(refused(
                &descriptor,
                ExecutionStatus::PermissionDenied,
                refusal.message(),
            ));
        }
    }

    // ── 4. Validate ─────────────────────────────────────────────────────
    //
    // Only now, when we already know this person may do this.
    if let Err(reason) = validate_against_schema(&request.input, &descriptor.input_schema) {
        return Ok(refused(
            &descriptor,
            ExecutionStatus::ValidationFailed,
            reason,
        ));
    }

    // A dry run against something that cannot simulate is refused rather than
    // quietly executed for real.
    if request.dry_run && !descriptor.supports_dry_run {
        return Ok(refused(
            &descriptor,
            ExecutionStatus::ValidationFailed,
            "Esta operação não pode ser simulada.",
        ));
    }

    // ── 5. Approval ─────────────────────────────────────────────────────
    //
    // A dry run needs none: it describes rather than does.
    let risk = effective_risk(&descriptor, request.dry_run);
    if risk.mutates() && approval_needed(&descriptor, agent) && !approved {
        return Ok(refused(
            &descriptor,
            ExecutionStatus::ApprovalRequired,
            "Esta acção precisa da sua confirmação antes de ser executada.",
        ));
    }

    // ── 6. Execute ──────────────────────────────────────────────────────
    let execution = ExecutionContext {
        pool,
        principal,
        input: &request.input,
        resources: &resolved,
        capabilities,
        dry_run: request.dry_run,
        ids,
    };

    let outcome = handler.execute(&execution).await;

    // ── 7. Verify and audit ─────────────────────────────────────────────
    //
    // The result is the Core's, not the model's.
    let result = match outcome {
        Ok(mut result) => {
            // A handler that forgot to mark a dry run cannot be allowed to
            // report a simulation as a change.
            if request.dry_run {
                result.status = ExecutionStatus::DryRun;
                result.reversibility = Reversibility::NothingToUndo;
            }
            result
        }
        Err(CoreError::PermissionDenied(reason)) => {
            refused(&descriptor, ExecutionStatus::PermissionDenied, reason)
        }
        Err(CoreError::NotFound(reason)) => {
            refused(&descriptor, ExecutionStatus::ResourceNotFound, reason)
        }
        Err(CoreError::Validation(reason)) => {
            refused(&descriptor, ExecutionStatus::ValidationFailed, reason)
        }
        Err(CoreError::CapabilityUnavailable(reason)) => {
            refused(&descriptor, ExecutionStatus::CapabilityUnavailable, reason)
        }
        Err(error) => {
            // The internal detail is logged and does not travel outward.
            tracing::error!(
                correlation_id = %ids.correlation_id,
                capability = %descriptor.id,
                cause = %error,
                "capability execution failed"
            );
            refused(
                &descriptor,
                ExecutionStatus::Failed,
                "A operação não foi concluída. Nada foi alterado.",
            )
        }
    };

    record(
        pool,
        principal,
        agent_id,
        &descriptor,
        result.status,
        None,
        ids,
    )
    .await?;

    Ok(result)
}

/// Write the audit row for one attempt.
///
/// # What is recorded, and what is not
///
/// Who asked, through which agent, which capability, and how it ended. **Not**
/// the input, which can carry a member's own words, a mail body or a document
/// excerpt. The correlation identifier is what ties a row to the request that
/// produced it (briefing §48, §102).
async fn record(
    pool: &PgPool,
    principal: &Principal,
    agent_id: Option<Uuid>,
    descriptor: &CapabilityDescriptor,
    status: ExecutionStatus,
    refusal: Option<AgenticRefusal>,
    ids: &CorrelationIds,
) -> CoreResult<()> {
    // A refusal on authorisation grounds is a security event and reads as one
    // in the audit log; anything else is an ordinary execution row.
    let auditable = if status == ExecutionStatus::PermissionDenied {
        action::SECURITY_DENIAL
    } else {
        action::CAPABILITY_EXECUTED
    };

    let mut entry = AuditEntry::new(auditable, "capability")
        .detail("capability", descriptor.id.as_str())
        .detail("risk", descriptor.risk.as_str())
        .detail("status", status.as_str());

    if let Some(agent_id) = agent_id {
        entry = entry.detail("agent_id", agent_id.to_string());
    }
    if let Some(refusal) = refusal {
        entry = entry.detail("refusal", refusal.as_str());
    }
    if status != ExecutionStatus::Succeeded && status != ExecutionStatus::DryRun {
        entry = entry.outcome(audit::Outcome::Denied);
    }

    let mut tx = pool.begin().await?;
    audit::record(&mut tx, Some(principal), ids, entry).await?;
    tx.commit().await?;

    Ok(())
}

// Não há aqui um `risk_of(request)`, e ainda bem.
//
// Existiu um, para mostrar o risco de um pedido antes de correr, e tinha um
// valor por omissão perigoso: uma capacidade desconhecida era reportada como
// `ReadOnly`. Ninguém o chamava, portanto nunca fez mal — mas era a forma
// exacta de um defeito que se abre para o lado errado.
//
// Quem decide o risco é `effective_risk(&descriptor, dry_run)`, e só é chamado
// depois de o descritor existir. Não há aí um caminho em que «não sei que
// capacidade é esta» se traduza em «então é inofensiva».

#[cfg(test)]
mod tests {
    use super::*;

    fn schema() -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "required": ["title"],
            "properties": {
                "title": {"type": "string"},
                "count": {"type": "integer"},
                "flag": {"type": "boolean"},
                "tags": {"type": "array"}
            }
        })
    }

    #[test]
    fn a_missing_required_field_is_refused_before_anything_runs() {
        let result = validate_against_schema(&serde_json::json!({}), &schema());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("title"));
    }

    #[test]
    fn a_field_of_the_wrong_type_is_refused() {
        // O erro que um modelo comete mais: um número onde vai texto.
        let wrong = serde_json::json!({"title": 42});
        assert!(validate_against_schema(&wrong, &schema()).is_err());

        let also_wrong = serde_json::json!({"title": "ok", "count": "sete"});
        assert!(validate_against_schema(&also_wrong, &schema()).is_err());

        let right = serde_json::json!({"title": "ok", "count": 7, "flag": true, "tags": []});
        assert!(validate_against_schema(&right, &schema()).is_ok());
    }

    #[test]
    fn a_non_object_request_is_refused() {
        for value in [
            serde_json::json!("uma string"),
            serde_json::json!(42),
            serde_json::json!([1, 2, 3]),
        ] {
            assert!(
                validate_against_schema(&value, &schema()).is_err(),
                "{value} foi aceite como pedido"
            );
        }
    }

    #[test]
    fn an_extra_field_is_noise_and_not_a_refusal() {
        // Um modelo acrescenta chaves. Um handler lê o que precisa; recusar
        // por ruído tornaria o planeamento frágil sem tornar nada mais seguro.
        let noisy = serde_json::json!({"title": "ok", "inventado": "seja o que for"});
        assert!(validate_against_schema(&noisy, &schema()).is_ok());
    }

    #[test]
    fn a_schema_without_properties_accepts_anything() {
        let open = serde_json::json!({"type": "object"});
        assert!(validate_against_schema(&serde_json::json!({"x": 1}), &open).is_ok());
    }

    #[test]
    fn a_null_optional_field_is_treated_as_absent_rather_than_wrong() {
        let nulled = serde_json::json!({"title": "ok", "count": null});
        assert!(validate_against_schema(&nulled, &schema()).is_ok());
    }

    #[test]
    fn a_required_field_set_to_null_is_absent_and_not_present() {
        // `null` é ausência por extenso. Tratá-lo como presença deixaria um
        // modelo satisfazer um campo obrigatório com nada.
        let nulled = serde_json::json!({"title": null});
        assert!(validate_against_schema(&nulled, &schema()).is_err());
    }
}
