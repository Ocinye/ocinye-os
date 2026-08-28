//! The Ocinye Inference Provider Conformance Suite.
//!
//! # What this is for
//!
//! When somebody implements an adapter — for the L40S, for a model on CAM-01,
//! for anything else — the question to ask is:
//!
//! > **Does this adapter pass the Ocinye Provider Conformance Suite?**
//!
//! and not
//!
//! > Will this provider break the architecture?
//!
//! A provider that has not passed is **not supported by the Ocinye OS**. That
//! is an engineering requirement, not a preference
//! ([ADR-0305](../../../../docs/adrs/0305-provider-conformance.md)).
//!
//! # What it tests, and what it deliberately does not
//!
//! It tests the **contract**: shapes, versions, deadlines, bounds, error
//! canonicalisation, and that the adapter does not smuggle its own semantics
//! across the boundary. It says nothing about whether a model writes well.
//!
//! It needs no GPU, no network and no database. Every check below runs against
//! a provider in memory.
//!
//! # The other half lives elsewhere
//!
//! Roughly a third of what a reader might expect here — «a hostile provider
//! cannot escalate», «a hallucinated `ResourceRef` resolves to nothing»,
//! «risk cannot be downgraded» — is not a property of the *provider*. It is a
//! property of the **Core's reaction** to one, and testing it needs the
//! registry, the executor, a principal and a database.
//!
//! Those live in `crates/ocinye-core/tests/agentic.rs`, driven by the same
//! fixture behaviours. Splitting them is honest: this module can certify an
//! adapter in isolation, and no adapter can certify the Core.

use std::time::Duration;

use ocinye_contracts::AiCapability;

use super::provider::{
    infer_within_deadline, ContractVersion, InferenceError, InferenceProvider, InferenceRequest,
    MAX_RESPONSE_BYTES,
};

/// One thing the suite checked.
#[derive(Debug, Clone)]
pub struct Check {
    /// What was being checked.
    pub name: &'static str,
    /// Whether the provider behaved.
    pub passed: bool,
    /// What happened, when it did not.
    pub detail: Option<String>,
}

impl Check {
    fn pass(name: &'static str) -> Self {
        Self {
            name,
            passed: true,
            detail: None,
        }
    }

    fn fail(name: &'static str, detail: impl Into<String>) -> Self {
        Self {
            name,
            passed: false,
            detail: Some(detail.into()),
        }
    }
}

/// What the suite found.
#[derive(Debug, Clone)]
pub struct ConformanceReport {
    /// The adapter that was examined.
    pub adapter: String,
    /// Every check, in order.
    pub checks: Vec<Check>,
}

impl ConformanceReport {
    /// Whether the adapter is conformant.
    #[must_use]
    pub fn passed(&self) -> bool {
        self.checks.iter().all(|check| check.passed)
    }

    /// The checks that failed.
    #[must_use]
    pub fn failures(&self) -> Vec<&Check> {
        self.checks.iter().filter(|check| !check.passed).collect()
    }

    /// A summary an engineer can paste into a pull request.
    #[must_use]
    pub fn summary(&self) -> String {
        let total = self.checks.len();
        let passed = self.checks.iter().filter(|check| check.passed).count();

        let mut out = format!(
            "Ocinye Provider Conformance — {}: {passed}/{total}\n",
            self.adapter
        );

        for check in self.failures() {
            out.push_str(&format!(
                "  ✗ {} — {}\n",
                check.name,
                check.detail.as_deref().unwrap_or("sem detalhe")
            ));
        }
        out
    }
}

/// What kind of provider is being certified.
///
/// # Why this exists
///
/// A provider that refuses everything is **conformant**: `NoProvider` is the
/// correct implementation for an installation with no inference, and it must
/// pass. But it cannot satisfy checks about well-formed answers, because it
/// never answers.
///
/// So the suite is told what it is looking at. It is not told what to expect
/// from each individual check — that would let an adapter grade itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    /// Serves inference. Every check applies.
    Serving,
    /// Serves nothing, and says so. Refusal checks apply; answer checks do not.
    Refusing,
}

/// How long the suite waits for any single probe.
///
/// Far below [`super::provider::DEFAULT_DEADLINE`], and deliberately: a suite
/// that waits forty-five seconds per check against a provider that never
/// answers takes minutes to tell an engineer something it knew immediately.
/// The deadline being *configurable per request* is what makes this possible,
/// which is itself a small argument for the field existing.
const PROBE_DEADLINE: Duration = Duration::from_millis(250);

/// A minimal request in the current contract.
fn probe(schema: bool) -> InferenceRequest {
    let mut request = InferenceRequest::new(
        AiCapability::General,
        "Instrução de sistema do Ocinye OS.".to_owned(),
        "um pedido de teste".to_owned(),
    );
    request.deadline = PROBE_DEADLINE;

    if schema {
        request.expecting(serde_json::json!({
            "type": "object",
            "required": ["steps"],
            "properties": { "steps": {"type": "array"} }
        }))
    } else {
        request
    }
}

/// Submit a provider to the suite.
///
/// # What a passing report means
///
/// That the adapter honours the contract boundary. It does **not** mean the
/// model behind it is safe, correct or aligned — nothing here could establish
/// that, and the architecture does not depend on it.
pub async fn certify(provider: &dyn InferenceProvider, kind: ProviderKind) -> ConformanceReport {
    let mut checks = Vec::new();

    checks.push(declares_what_it_serves(provider));
    checks.push(answers_within_the_deadline(provider, kind).await);
    checks.push(honours_the_contract_version(provider, kind).await);
    checks.push(errors_are_canonical(provider).await);
    checks.push(errors_carry_no_provider_text(provider).await);
    checks.push(identity_is_bounded(provider, kind).await);
    checks.push(oversized_answers_are_refused(provider, kind).await);
    checks.push(a_refusal_is_not_an_answer(provider, kind).await);
    checks.push(structured_requests_get_structured_answers(provider, kind).await);
    checks.push(the_provider_does_not_invent_semantics(provider, kind).await);

    ConformanceReport {
        adapter: provider.adapter_name().to_owned(),
        checks,
    }
}

/// An adapter says which capabilities it serves, and says it consistently.
fn declares_what_it_serves(provider: &dyn InferenceProvider) -> Check {
    const NAME: &str = "declares what it serves";

    if provider.adapter_name().trim().is_empty() {
        return Check::fail(NAME, "o adapter não tem nome");
    }

    // Asked twice, the same answer. A provider whose declaration varies makes
    // capability routing unpredictable.
    for capability in AiCapability::all() {
        if provider.serves(capability) != provider.serves(capability) {
            return Check::fail(NAME, format!("`serves({capability:?})` não é estável"));
        }
    }

    Check::pass(NAME)
}

/// The deadline is honoured, or the Core enforces it.
async fn answers_within_the_deadline(
    provider: &dyn InferenceProvider,
    _kind: ProviderKind,
) -> Check {
    const NAME: &str = "answers within the deadline";

    let request = probe(false);

    let started = std::time::Instant::now();
    let outcome = infer_within_deadline(provider, &request).await;
    let elapsed = started.elapsed();

    // Whatever it answered, it must not have taken materially longer than it
    // was given. A provider that ignores its deadline is caught by the guard,
    // which is the point: this check verifies the guard holds for *this*
    // provider too.
    if elapsed > PROBE_DEADLINE * 4 {
        return Check::fail(
            NAME,
            format!(
                "demorou {}ms com um prazo de {}ms",
                elapsed.as_millis(),
                PROBE_DEADLINE.as_millis()
            ),
        );
    }

    match outcome {
        Err(InferenceError::Timeout) | Ok(_) | Err(_) => Check::pass(NAME),
    }
}

/// An answer arrives in a contract the Core speaks.
async fn honours_the_contract_version(
    provider: &dyn InferenceProvider,
    kind: ProviderKind,
) -> Check {
    const NAME: &str = "honours the contract version";

    if kind == ProviderKind::Refusing {
        return Check::pass(NAME);
    }

    match infer_within_deadline(provider, &probe(false)).await {
        Ok(answer) if answer.contract == ContractVersion::CURRENT => Check::pass(NAME),
        Ok(answer) => Check::fail(NAME, format!("respondeu em «{}»", answer.contract.as_str())),
        Err(InferenceError::UnsupportedContractVersion) => {
            Check::fail(NAME, "respondeu num contrato que este Core não fala")
        }
        // Any other refusal is a different check's business.
        Err(_) => Check::pass(NAME),
    }
}

/// Every failure is one of the canonical variants.
///
/// Trivially true in Rust — the type is closed — and worth stating: an adapter
/// author reading this suite learns that inventing an error is not an option.
async fn errors_are_canonical(provider: &dyn InferenceProvider) -> Check {
    const NAME: &str = "errors are canonical";

    match infer_within_deadline(provider, &probe(false)).await {
        Ok(_) => Check::pass(NAME),
        Err(error) => {
            if error.as_str().is_empty() {
                Check::fail(NAME, "um erro sem rótulo estável")
            } else {
                Check::pass(NAME)
            }
        }
    }
}

/// No error text quotes the provider.
async fn errors_carry_no_provider_text(provider: &dyn InferenceProvider) -> Check {
    const NAME: &str = "errors carry no provider text";

    // A marker no canonical message contains. If it comes back, the adapter
    // echoed the request into an error — and the request can hold a member's
    // correspondence (briefing §18).
    let mut request = probe(false);
    request.instruction = "MARCADOR-QUE-NAO-DEVE-VOLTAR".to_owned();

    match infer_within_deadline(provider, &request).await {
        Ok(_) => Check::pass(NAME),
        Err(error) => {
            let rendered = error.to_string();
            if rendered.contains("MARCADOR-QUE-NAO-DEVE-VOLTAR") {
                Check::fail(NAME, "o erro devolveu texto do pedido")
            } else {
                Check::pass(NAME)
            }
        }
    }
}

/// The model identity is bounded and free of control characters.
async fn identity_is_bounded(provider: &dyn InferenceProvider, kind: ProviderKind) -> Check {
    const NAME: &str = "model identity is bounded";

    if kind == ProviderKind::Refusing {
        return Check::pass(NAME);
    }

    match infer_within_deadline(provider, &probe(false)).await {
        Ok(answer) => {
            for (field, value) in [
                ("provider", &answer.model.provider),
                ("model", &answer.model.model),
                ("version", &answer.model.version),
            ] {
                if value.len() > 96 {
                    return Check::fail(NAME, format!("`{field}` excede o limite"));
                }
                if value.chars().any(char::is_control) {
                    return Check::fail(NAME, format!("`{field}` tem caracteres de controlo"));
                }
                if value.is_empty() {
                    return Check::fail(NAME, format!("`{field}` está vazio"));
                }
            }
            Check::pass(NAME)
        }
        Err(_) => Check::pass(NAME),
    }
}

/// An answer larger than the bound is refused, not read.
async fn oversized_answers_are_refused(
    provider: &dyn InferenceProvider,
    kind: ProviderKind,
) -> Check {
    const NAME: &str = "oversized answers are refused";

    if kind == ProviderKind::Refusing {
        return Check::pass(NAME);
    }

    match infer_within_deadline(provider, &probe(true)).await {
        Ok(answer) => {
            let size = answer
                .value
                .as_ref()
                .and_then(|value| serde_json::to_vec(value).ok())
                .map_or(0, |bytes| bytes.len());

            if size > MAX_RESPONSE_BYTES {
                Check::fail(NAME, "uma resposta acima do limite atravessou o guarda")
            } else {
                Check::pass(NAME)
            }
        }
        Err(InferenceError::ResponseTooLarge) => Check::pass(NAME),
        Err(_) => Check::pass(NAME),
    }
}

/// A refusal is a refusal: no partial answer smuggled alongside it.
async fn a_refusal_is_not_an_answer(provider: &dyn InferenceProvider, kind: ProviderKind) -> Check {
    const NAME: &str = "a refusal is not an answer";

    if kind == ProviderKind::Serving {
        return Check::pass(NAME);
    }

    // A refusing provider must refuse every capability it declared it does not
    // serve. Answering anyway would make `serves` a suggestion.
    for capability in AiCapability::all() {
        if provider.serves(capability) {
            continue;
        }

        let mut request = probe(false);
        request.capability = capability;

        if infer_within_deadline(provider, &request).await.is_ok() {
            return Check::fail(
                NAME,
                format!("respondeu a {capability:?}, que declarou não servir"),
            );
        }
    }

    Check::pass(NAME)
}

/// A request with a schema gets a structured answer, or a canonical refusal.
///
/// Never prose where a plan was asked for: the Core would have to guess, and
/// guessing is what this whole boundary exists to avoid (briefing §12, §14).
async fn structured_requests_get_structured_answers(
    provider: &dyn InferenceProvider,
    kind: ProviderKind,
) -> Check {
    const NAME: &str = "structured requests get structured answers";

    if kind == ProviderKind::Refusing {
        return Check::pass(NAME);
    }

    match infer_within_deadline(provider, &probe(true)).await {
        Ok(answer) if answer.value.is_some() => Check::pass(NAME),
        Ok(_) => Check::fail(
            NAME,
            "devolveu prosa onde foi pedida uma resposta estruturada",
        ),
        Err(_) => Check::pass(NAME),
    }
}

/// The adapter does not decide anything the Ocinye OS decides.
///
/// # What is checked
///
/// That an answer carries no field claiming authority: no approval, no risk, no
/// permission, no execution result. Those belong to the Capability Registry,
/// the policy layer and the executor, and a provider that ships them is a
/// provider trying to be the Core (briefing §5, §36, §37, §38).
async fn the_provider_does_not_invent_semantics(
    provider: &dyn InferenceProvider,
    kind: ProviderKind,
) -> Check {
    const NAME: &str = "the provider does not invent semantics";

    if kind == ProviderKind::Refusing {
        return Check::pass(NAME);
    }

    let Ok(answer) = infer_within_deadline(provider, &probe(true)).await else {
        return Check::pass(NAME);
    };

    let Some(value) = answer.value else {
        return Check::pass(NAME);
    };

    // A model *proposing* these is expected and harmless — the Core ignores
    // them. An **adapter** that lifts them into the canonical response is
    // asserting they mean something, and they do not.
    const RESERVED: &[&str] = &[
        "approved",
        "approval",
        "risk",
        "risk_level",
        "permission",
        "permissions",
        "authorized",
        "authorised",
        "executed",
        "execution_result",
        "system_state",
    ];

    if let Some(object) = value.as_object() {
        for key in RESERVED {
            if object.contains_key(*key) {
                return Check::fail(
                    NAME,
                    format!("a resposta canónica traz «{key}», que pertence ao Ocinye Core"),
                );
            }
        }
    }

    Check::pass(NAME)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::intelligence::fixture::FixtureProvider;
    use crate::modules::intelligence::NoProvider;

    /// The correct provider for an installation with no inference passes.
    #[tokio::test]
    async fn the_no_provider_is_conformant() {
        let report = certify(&NoProvider, ProviderKind::Refusing).await;
        assert!(report.passed(), "{}", report.summary());
    }

    /// A cooperative adapter passes.
    #[tokio::test]
    async fn a_cooperative_provider_is_conformant() {
        let report = certify(&FixtureProvider::cooperative(), ProviderKind::Serving).await;
        assert!(report.passed(), "{}", report.summary());
    }

    /// A **hostile** adapter is still contract-conformant.
    ///
    /// This is not a contradiction, and it is the most important thing in this
    /// file. Conformance is about the *boundary*, not about the model's
    /// intentions. A provider that returns a plan full of invented capabilities
    /// has honoured the contract perfectly — and the Core refuses the plan,
    /// which is where containment lives.
    ///
    /// Reading this the other way round is the mistake to avoid: passing the
    /// suite does **not** mean a provider is trustworthy.
    #[tokio::test]
    async fn a_hostile_provider_is_contract_conformant_and_that_is_the_point() {
        let report = certify(&FixtureProvider::hostile(), ProviderKind::Serving).await;
        assert!(
            report.passed(),
            "a conformidade é sobre a fronteira, não sobre as intenções: {}",
            report.summary()
        );
    }

    /// A provider that answers past its deadline is caught.
    #[tokio::test]
    async fn a_slow_provider_is_caught_by_the_guard() {
        let report = certify(&FixtureProvider::timeout(), ProviderKind::Serving).await;

        // It passes the deadline check — because the guard enforced it — and
        // the answer checks pass vacuously, since it never answers.
        assert!(report.passed(), "{}", report.summary());

        // And the guard really did convert it.
        let request = probe(false);
        assert_eq!(
            infer_within_deadline(&FixtureProvider::timeout(), &request)
                .await
                .unwrap_err(),
            InferenceError::Timeout
        );
    }

    /// An oversized answer never reaches a caller.
    #[tokio::test]
    async fn an_oversized_answer_is_refused_by_the_guard() {
        assert_eq!(
            infer_within_deadline(&FixtureProvider::oversized(), &probe(true))
                .await
                .unwrap_err(),
            InferenceError::ResponseTooLarge
        );

        let report = certify(&FixtureProvider::oversized(), ProviderKind::Serving).await;
        assert!(report.passed(), "{}", report.summary());
    }

    /// A report names what failed.
    #[test]
    fn a_report_names_its_failures() {
        let report = ConformanceReport {
            adapter: "exemplo".to_owned(),
            checks: vec![Check::pass("primeira"), Check::fail("segunda", "a razão")],
        };

        assert!(!report.passed());
        assert_eq!(report.failures().len(), 1);

        let summary = report.summary();
        assert!(summary.contains("1/2"));
        assert!(summary.contains("segunda"));
        assert!(summary.contains("a razão"));
    }
}
