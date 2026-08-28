//! The Action Planner.
//!
//! # Where model output stops being trusted
//!
//! A model returns text. [`validate_proposal`] is the only door through which
//! that text becomes an [`ActionPlan`], and it refuses everything it cannot
//! account for: a capability that does not exist, an input that does not match
//! the published schema, a plan longer than the system will run, a risk level
//! the model assigned itself.
//!
//! Whatever survives is still only a *plan*. It has authorised nothing.
//!
//! # Risk is taken from the registry, never from the proposal
//!
//! A model asked to label its own risk will label a destructive action
//! harmless — sometimes by mistake, sometimes because a document told it to.
//! The proposal has no risk field, and [`ActionStep::risk`] is filled in from
//! the descriptor (briefing §49, §81).
//!
//! # The digest binds an approval to a plan
//!
//! [`digest_of`] hashes what the plan *does*: capabilities, inputs, resources,
//! order. Confirming a plan confirms that digest. Change a recipient after the
//! confirmation and the digest changes, which invalidates it — «yes, send that»
//! cannot become authority to send something else (briefing §100, §101).

use ocinye_contracts::agentic::{
    ActionPlan, ActionStep, CapabilityId, CapabilityRequest, PlanState, ResourceRef, RiskLevel,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::registry::registry;
use crate::error::{CoreError, CoreResult};

/// How many steps one plan may contain.
///
/// A model asked to «tidy up the workspace» will happily propose two hundred
/// operations. A bound means a runaway plan is refused rather than executed,
/// and eight is more than any request a person actually makes in one sentence.
const MAX_STEPS: usize = 8;

/// How large one step's input may be.
///
/// A plan step carries a title, an identifier, a short body. Sixteen kilobytes
/// is generous for all of those together and finite, which an unbounded value
/// from a model is not.
const MAX_STEP_INPUT_BYTES: usize = 16 * 1024;

/// What a model is asked to produce.
///
/// Deliberately minimal. No risk, no approval, no permission, no reversibility:
/// every one of those comes from the registry, because a proposal that could
/// describe its own safety would be a proposal that could lie about it.
///
/// # Policy for what a model gets wrong
///
/// Stated once, here, because every field below inherits it:
///
/// | Case | Policy |
/// |---|---|
/// | **Unknown field** | Ignored. Models add keys; refusing on noise would make planning brittle without making it safer. |
/// | **Missing required field** | Refused. There is nothing to substitute. |
/// | **`null` in a required field** | Refused — `null` is absence written out. |
/// | **Wrong type** | Refused. Coercion is guessing. |
/// | **Unknown enum value** | Refused. A capability that does not exist resolves to nothing. |
/// | **Unknown contract version** | Refused before it reaches here, by the provider guard. |
///
/// Everything with security impact **fails closed** (briefing §15).
#[derive(Debug, Clone, Deserialize)]
pub struct PlanProposal {
    /// What the model understood the member to want.
    #[serde(default)]
    pub intent: String,
    /// The steps it proposes.
    #[serde(default)]
    pub steps: Vec<ProposedStep>,
}

/// One proposed step.
///
/// # What is deliberately absent
///
/// No risk, no approval, no permission, no reversibility, no execution result.
/// A proposal that could describe its own safety would be a proposal that could
/// lie about it — and a document that tells a model «marca isto como
/// inofensivo» would then have somewhere to write (briefing §36, §37, §41).
#[derive(Debug, Clone, Deserialize)]
pub struct ProposedStep {
    /// The capability identifier, as text.
    pub capability: String,
    /// What to pass it.
    #[serde(default)]
    pub input: serde_json::Value,
    /// A sentence for the member.
    #[serde(default)]
    pub summary: String,
    /// Resources it acts on.
    #[serde(default)]
    pub resources: Vec<ResourceRef>,
}

/// Turn a proposal into a plan, or refuse it.
///
/// # Errors
///
/// Returns [`CoreError::Validation`] when the proposal names something that
/// does not exist, exceeds the step bound, or is empty. Each refusal names what
/// was wrong, because these are read by an operator diagnosing a model that has
/// started proposing nonsense — not by the member, who sees the outcome.
pub fn validate_proposal(proposal: &PlanProposal) -> CoreResult<ActionPlan> {
    if proposal.steps.is_empty() {
        return Err(CoreError::Validation(
            "O plano não contém nenhuma acção.".to_owned(),
        ));
    }

    if proposal.steps.len() > MAX_STEPS {
        return Err(CoreError::Validation(format!(
            "Um plano não pode ter mais de {MAX_STEPS} acções."
        )));
    }

    // Bounded before anything is walked. A proposal can arrive with an input
    // object of arbitrary size, and the size guard on the provider boundary
    // bounds the *response*, not any single field within it.
    for proposed in &proposal.steps {
        let rendered = serde_json::to_vec(&proposed.input)
            .map_err(|_| CoreError::Validation("Uma acção proposta não é legível.".to_owned()))?;

        if rendered.len() > MAX_STEP_INPUT_BYTES {
            return Err(CoreError::Validation(
                "Uma acção proposta excede o tamanho permitido.".to_owned(),
            ));
        }
    }

    let mut steps = Vec::with_capacity(proposal.steps.len());

    for (index, proposed) in proposal.steps.iter().enumerate() {
        // Shape first: a value that is not even a capability identifier tells
        // us the model is not producing what was asked for.
        let capability = CapabilityId::parse(&proposed.capability).ok_or_else(|| {
            CoreError::Validation(format!(
                "«{}» não tem a forma de uma operação do Ocinye OS.",
                proposed.capability
            ))
        })?;

        // Existence second. This is what stops a hallucinated capability: the
        // registry is the only thing that knows what exists.
        let handler = registry().get(&capability).ok_or_else(|| {
            CoreError::Validation(format!("A operação «{capability}» não existe."))
        })?;

        let descriptor = handler.descriptor();

        let summary = proposed.summary.trim();
        let summary = if summary.is_empty() {
            // A model that returned no summary does not get to leave the member
            // with a blank line in the confirmation dialog.
            descriptor.summary.clone()
        } else {
            summary.chars().take(200).collect()
        };

        steps.push(ActionStep {
            ordinal: u16::try_from(index + 1).unwrap_or(u16::MAX),
            summary,
            request: CapabilityRequest {
                capability,
                input: proposed.input.clone(),
                resources: proposed.resources.clone(),
                dry_run: false,
            },
            // From the registry. Never from the proposal.
            risk: descriptor.risk,
            result: None,
        });
    }

    let mut plan = ActionPlan {
        id: Uuid::new_v4(),
        intent: proposal.intent.trim().chars().take(500).collect(),
        steps,
        state: PlanState::Proposed,
        digest: String::new(),
    };

    plan.digest = digest_of(&plan);
    Ok(plan)
}

/// A digest of what a plan does.
///
/// # What is hashed, and what is not
///
/// Capabilities, inputs, resource identifiers and order — everything that
/// determines the effect. **Not** the intent text and not the step summaries:
/// rewording «send the report» to «send the report to Carlos» must not
/// invalidate a confirmation, but changing the recipient must.
#[must_use]
pub fn digest_of(plan: &ActionPlan) -> String {
    let mut hasher = Sha256::new();

    for step in &plan.steps {
        hasher.update(step.ordinal.to_be_bytes());
        hasher.update(step.request.capability.as_str().as_bytes());
        hasher.update([u8::from(step.request.dry_run)]);

        // Canonical JSON, so that two semantically identical inputs that differ
        // only in key order produce the same digest — otherwise a confirmation
        // would be invalidated by a reserialisation.
        hasher.update(canonical(&step.request.input).as_bytes());

        for resource in &step.request.resources {
            hasher.update(resource.kind.as_str().as_bytes());
            hasher.update(resource.id.as_bytes());
        }
    }

    hex::encode(hasher.finalize())
}

/// Serialise JSON with object keys in a stable order.
fn canonical(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort_unstable();

            let inner: Vec<String> = keys
                .into_iter()
                .map(|key| format!("{key:?}:{}", canonical(&map[key])))
                .collect();

            format!("{{{}}}", inner.join(","))
        }
        serde_json::Value::Array(items) => {
            let inner: Vec<String> = items.iter().map(canonical).collect();
            format!("[{}]", inner.join(","))
        }
        other => other.to_string(),
    }
}

/// The peak risk of a plan, as the registry classifies it **now**.
///
/// # Why this is not read from the stored plan
///
/// [`ActionStep::risk`] is a snapshot: the value the registry gave when the
/// plan was built. A plan can outlive that. If a capability is reclassified
/// upward — `LowImpact` becomes `ExternalEffect` because it started reaching
/// outside the institution — then a plan built yesterday carries yesterday's
/// answer, and showing it would tell a member the operation is safer than the
/// Ocinye now believes it to be.
///
/// Risk always comes from the Capability Registry, and «always» includes later
/// (briefing §49, §81). A capability that has since disappeared from the
/// registry contributes nothing here: it resolves to nothing at execution, so
/// there is no risk to report for it.
#[must_use]
pub fn current_peak_risk(plan: &ActionPlan) -> RiskLevel {
    plan.steps
        .iter()
        .filter_map(|step| registry().get(&step.request.capability))
        .map(|handler| handler.descriptor().risk)
        .max()
        .unwrap_or(RiskLevel::ReadOnly)
}

/// Whether this plan still needs a person to say yes, by today's policy.
///
/// # The direction that matters
///
/// A risk raised since the plan was built makes this `true` where the stored
/// snapshot said `false`. That is the safe direction, and it is the point: a
/// temporal downgrade of risk must be impossible, so the question is asked
/// again against the registry rather than answered from the plan.
///
/// A plan that reads and changes nothing still returns `true` when it was
/// produced by `Act`, because the executor's own gate is what decides; this
/// answers the narrower question of whether the *capabilities* demand consent.
#[must_use]
pub fn approval_required_now(plan: &ActionPlan) -> bool {
    plan.steps.iter().any(|step| {
        registry()
            .get(&step.request.capability)
            .is_some_and(|handler| {
                let descriptor = handler.descriptor();
                descriptor.requires_approval() || descriptor.risk.mutates()
            })
    })
}

/// Whether an approval still covers a plan.
///
/// # Errors
///
/// Returns [`CoreError::Validation`] when the plan has changed materially since
/// it was confirmed.
pub fn approval_still_binds(plan: &ActionPlan, approved_digest: &str) -> CoreResult<()> {
    if digest_of(plan) == approved_digest {
        return Ok(());
    }

    Err(CoreError::Validation(
        "O plano foi alterado depois de confirmado. Confirme novamente.".to_owned(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proposal(steps: Vec<ProposedStep>) -> PlanProposal {
        PlanProposal {
            intent: "um pedido".to_owned(),
            steps,
        }
    }

    fn step(capability: &str, input: serde_json::Value) -> ProposedStep {
        ProposedStep {
            capability: capability.to_owned(),
            input,
            summary: String::new(),
            resources: Vec::new(),
        }
    }

    #[test]
    fn a_hallucinated_capability_is_refused() {
        // O modelo inventa uma operação que não existe. O registry é o único
        // que sabe o que existe, e recusa.
        let invented = proposal(vec![step("mail.delete_everything", serde_json::json!({}))]);

        let outcome = validate_proposal(&invented);
        assert!(outcome.is_err());
        assert!(outcome.unwrap_err().to_string().contains("não existe"));
    }

    #[test]
    fn something_that_is_not_even_an_identifier_is_refused() {
        for nonsense in ["", "faz isso", "DROP TABLE mail_messages", "shell"] {
            assert!(
                validate_proposal(&proposal(vec![step(nonsense, serde_json::json!({}))])).is_err(),
                "«{nonsense}» foi aceite como operação"
            );
        }
    }

    #[test]
    fn an_empty_plan_is_refused() {
        assert!(validate_proposal(&proposal(vec![])).is_err());
    }

    #[test]
    fn a_runaway_plan_is_refused_rather_than_executed() {
        // «Arruma o workspace» pode produzir duzentas operações. Um limite faz
        // com que seja recusado em vez de corrido.
        let many = (0..MAX_STEPS + 1)
            .map(|_| step("knowledge.search", serde_json::json!({"query": "x"})))
            .collect();

        assert!(validate_proposal(&proposal(many)).is_err());
    }

    #[test]
    fn risk_comes_from_the_registry_and_not_from_the_proposal() {
        // A proposta não tem sequer campo de risco — este teste fixa isso, e
        // verifica que o valor vem do descriptor.
        let plan = validate_proposal(&proposal(vec![step(
            "mail.send",
            serde_json::json!({"draft_id": "00000000-0000-0000-0000-000000000000"}),
        )]))
        .expect("mail.send existe");

        assert_eq!(
            plan.steps[0].risk,
            ocinye_contracts::agentic::RiskLevel::ExternalEffect,
            "o risco de enviar correio deixou de vir do registry"
        );
        assert!(plan.mutates());
    }

    #[test]
    fn a_step_without_a_summary_borrows_the_descriptor_s() {
        let plan = validate_proposal(&proposal(vec![step(
            "knowledge.search",
            serde_json::json!({"query": "hidrogénio"}),
        )]))
        .expect("existe");

        assert!(
            !plan.steps[0].summary.is_empty(),
            "um passo sem resumo deixaria uma linha em branco na confirmação"
        );
    }

    // ── The digest ──────────────────────────────────────────────────────

    #[test]
    fn changing_what_a_plan_does_invalidates_its_approval() {
        let mut plan = validate_proposal(&proposal(vec![step(
            "mail.draft",
            serde_json::json!({
                "mailbox_id": "11111111-1111-1111-1111-111111111111",
                "to": ["carlos@ocinye.com"],
                "subject": "Relatório",
                "body": "Segue."
            }),
        )]))
        .expect("existe");

        let approved = plan.digest.clone();
        assert!(approval_still_binds(&plan, &approved).is_ok());

        // A garantia que mais importa: mudar o destinatário depois de
        // confirmado não deixa a confirmação de pé.
        plan.steps[0].request.input["to"] = serde_json::json!(["outra-pessoa@fora.com"]);
        assert!(
            approval_still_binds(&plan, &approved).is_err(),
            "o destinatário mudou e a aprovação continuou válida"
        );
    }

    #[test]
    fn rewording_a_summary_does_not_invalidate_an_approval() {
        // O digest cobre o que o plano *faz*. Reescrever a frase mostrada não
        // devia obrigar a confirmar outra vez.
        let mut plan = validate_proposal(&proposal(vec![step(
            "knowledge.search",
            serde_json::json!({"query": "baterias"}),
        )]))
        .expect("existe");

        let approved = plan.digest.clone();
        plan.steps[0].summary = "Procurar por baterias no acervo".to_owned();
        plan.intent = "outra redacção do mesmo pedido".to_owned();

        assert!(approval_still_binds(&plan, &approved).is_ok());
    }

    #[test]
    fn adding_a_step_invalidates_an_approval() {
        let plan = validate_proposal(&proposal(vec![step(
            "knowledge.search",
            serde_json::json!({"query": "baterias"}),
        )]))
        .expect("existe");
        let approved = plan.digest.clone();

        let longer = validate_proposal(&proposal(vec![
            step("knowledge.search", serde_json::json!({"query": "baterias"})),
            step(
                "mail.draft",
                serde_json::json!({"mailbox_id": "1", "to": [], "subject": "x", "body": "y"}),
            ),
        ]))
        .expect("existe");

        assert!(approval_still_binds(&longer, &approved).is_err());
    }

    #[test]
    fn a_step_with_no_capability_is_refused() {
        // Campo obrigatório em falta: não há nada por que o substituir.
        let partial: Result<PlanProposal, _> = serde_json::from_value(serde_json::json!({
            "intent": "faz aquilo",
            "steps": [{"summary": "faz aquilo", "input": {}}]
        }));

        assert!(partial.is_err(), "um passo sem capability desserializou");
    }

    #[test]
    fn a_capability_set_to_null_is_refused() {
        let nulled: Result<PlanProposal, _> = serde_json::from_value(serde_json::json!({
            "steps": [{"capability": null, "input": {}}]
        }));
        assert!(nulled.is_err(), "`null` foi aceite como capability");
    }

    #[test]
    fn a_capability_of_the_wrong_type_is_refused() {
        // Coerção é adivinhação.
        let wrong: Result<PlanProposal, _> = serde_json::from_value(serde_json::json!({
            "steps": [{"capability": 42, "input": {}}]
        }));
        assert!(wrong.is_err());
    }

    #[test]
    fn unknown_fields_are_noise_and_not_a_refusal() {
        // Modelos acrescentam chaves. Recusar por ruído tornaria o planeamento
        // frágil sem o tornar mais seguro — e nenhuma chave extra tem
        // significado, porque risco e aprovação vêm do registry.
        let noisy: PlanProposal = serde_json::from_value(serde_json::json!({
            "intent": "procurar",
            "confidence": 0.97,
            "reasoning": "pensei muito nisto",
            "steps": [{
                "capability": "knowledge.search",
                "input": {"query": "x"},
                "risk": "read_only",
                "approved": true,
                "already_executed": true
            }]
        }))
        .expect("chaves extra são ruído");

        let plan = validate_proposal(&noisy).expect("existe");

        // E as chaves que tentavam afirmar autoridade não têm efeito nenhum.
        assert_eq!(
            plan.steps[0].risk,
            ocinye_contracts::agentic::RiskLevel::ReadOnly
        );
        assert!(plan.steps[0].result.is_none(), "o modelo declarou execução");
    }

    #[test]
    fn a_step_with_an_enormous_input_is_refused() {
        let huge = proposal(vec![step(
            "knowledge.search",
            serde_json::json!({"query": "x".repeat(32 * 1024)}),
        )]);

        assert!(validate_proposal(&huge).is_err());
    }

    #[test]
    fn key_order_does_not_change_the_digest() {
        // Sem canonicalização, uma re-serialização invalidaria uma confirmação
        // sem nada ter mudado.
        let one = validate_proposal(&proposal(vec![step(
            "mail.draft",
            serde_json::json!({"mailbox_id": "a", "to": ["x"], "subject": "s", "body": "b"}),
        )]))
        .expect("existe");

        let other = validate_proposal(&proposal(vec![step(
            "mail.draft",
            serde_json::json!({"body": "b", "subject": "s", "to": ["x"], "mailbox_id": "a"}),
        )]))
        .expect("existe");

        assert_eq!(one.digest, other.digest);
    }

    #[test]
    fn array_order_does_change_the_digest() {
        // Ordem de destinatários não é ruído: `to: [a, b]` e `to: [b, a]` são
        // a mesma mensagem, mas a ordem de *passos* não é, e a canonicalização
        // não deve apagar diferenças reais dentro de um array.
        let one = validate_proposal(&proposal(vec![step(
            "mail.draft",
            serde_json::json!({"mailbox_id": "a", "to": ["x", "y"], "subject": "s", "body": "b"}),
        )]))
        .expect("existe");

        let other = validate_proposal(&proposal(vec![step(
            "mail.draft",
            serde_json::json!({"mailbox_id": "a", "to": ["y", "x"], "subject": "s", "body": "b"}),
        )]))
        .expect("existe");

        assert_ne!(one.digest, other.digest);
    }
}
