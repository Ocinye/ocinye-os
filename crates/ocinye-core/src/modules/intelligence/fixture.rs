//! A deterministic inference provider, for tests.
//!
//! # Why this is a provider and not a mock
//!
//! It implements [`InferenceProvider`] — the Core's own contract — and nothing
//! else. It does not imitate a vendor's wire format, because imitating one
//! would test the adapter rather than the architecture. What it exercises is
//! the path that matters:
//!
//! ```text
//! linguagem natural → Main Agent → ActionPlan → Capability → aprovação → Core → resultado
//! ```
//!
//! and it exercises it with no GPU, no node and no network.
//!
//! # Why it cannot reach production
//!
//! Behind `#[cfg(feature = "test-fixtures")]`. The feature is enabled by the
//! test harness and by nothing else, so a release binary does not contain this
//! code at all — not merely unreachable, absent (briefing §164, §204).
//!
//! # Deterministic on purpose
//!
//! It matches on the instruction and returns a fixed plan. A fixture that
//! produced varied output would make failures irreproducible, and the point of
//! testing the agentic path is that its guarantees hold for **every** answer a
//! model could give — including a hostile one, which is why
//! [`FixtureProvider::hostile`] exists.

use std::time::Duration;

use async_trait::async_trait;
use ocinye_contracts::AiCapability;

use super::provider::{
    ContractVersion, InferenceError, InferenceProvider, InferenceRequest, InferenceResponse,
    InferenceResult, ModelIdentity, TokenUsage,
};

/// What the fixture should pretend the model returned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixtureBehaviour {
    /// A well-formed plan that matches the instruction.
    Cooperative,
    /// A plan naming capabilities that do not exist.
    ///
    /// What a model looks like after reading a document that told it to call
    /// the admin tool. The architecture's claim is that this changes nothing.
    Hostile,
    /// A response that is not the shape that was asked for.
    Malformed,
    /// Does not answer at all.
    Unavailable,
    /// Answers, but not before the deadline.
    Timeout,
    /// A plan missing a required field.
    Partial,
    /// An answer in a contract this Core does not speak.
    WrongVersion,
    /// A structurally valid answer that is far larger than the bound.
    Oversized,
}

/// A provider that answers from a table.
pub struct FixtureProvider {
    behaviour: FixtureBehaviour,
}

impl FixtureProvider {
    /// A fixture that returns well-formed plans.
    #[must_use]
    pub const fn cooperative() -> Self {
        Self {
            behaviour: FixtureBehaviour::Cooperative,
        }
    }

    /// A fixture that behaves as a fully subverted model.
    #[must_use]
    pub const fn hostile() -> Self {
        Self {
            behaviour: FixtureBehaviour::Hostile,
        }
    }

    /// A fixture that returns something that is not a plan.
    #[must_use]
    pub const fn malformed() -> Self {
        Self {
            behaviour: FixtureBehaviour::Malformed,
        }
    }

    /// A fixture that does not answer.
    #[must_use]
    pub const fn unavailable() -> Self {
        Self {
            behaviour: FixtureBehaviour::Unavailable,
        }
    }

    /// A fixture that answers after the deadline has passed.
    #[must_use]
    pub const fn timeout() -> Self {
        Self {
            behaviour: FixtureBehaviour::Timeout,
        }
    }

    /// A fixture whose plan is missing a required field.
    #[must_use]
    pub const fn partial() -> Self {
        Self {
            behaviour: FixtureBehaviour::Partial,
        }
    }

    /// A fixture that answers in a contract this Core does not speak.
    #[must_use]
    pub const fn wrong_version() -> Self {
        Self {
            behaviour: FixtureBehaviour::WrongVersion,
        }
    }

    /// A fixture that answers with far more than the Core will read.
    #[must_use]
    pub const fn oversized() -> Self {
        Self {
            behaviour: FixtureBehaviour::Oversized,
        }
    }

    /// Identifiers appearing in an instruction, in order.
    ///
    /// # Why the fixture reads them out of the sentence
    ///
    /// Because it has no other way to know one. A real model would name a
    /// resource from the retrieved context; this one is a keyword table, so a
    /// test writes the identifier into the instruction and the fixture echoes
    /// it back. What the Core then does with it — resolve it, check it against
    /// the acting person, refuse it — is the real path, and that is what the
    /// tests are for.
    fn identifiers(instruction: &str) -> Vec<String> {
        instruction
            .split(|c: char| !c.is_ascii_alphanumeric() && c != '-')
            .filter(|token| token.len() == 36 && token.matches('-').count() == 4)
            .map(str::to_owned)
            .collect()
    }

    /// The plan a cooperative fixture proposes for an instruction.
    ///
    /// Matched on keywords, deliberately crudely: this is not a language
    /// model and does not pretend to be one. It is a table that lets the tests
    /// below drive the real Runtime, the real planner and the real executor.
    fn plan_for(instruction: &str) -> serde_json::Value {
        let lower = instruction.to_lowercase();
        let ids = Self::identifiers(instruction);
        // A reference to whatever the instruction named at this position.
        //
        // When it named nothing, this invents a well-formed identifier rather
        // than emitting an empty string. Both are wrong answers, and the
        // difference matters: an empty string is not a `ResourceRef` at all, so
        // the *proposal* fails to deserialise and no plan exists to examine. An
        // invented identifier is exactly what a model that hallucinated a
        // resource would produce — a plan that looks right and resolves to
        // nothing, which is the case the Core is built to refuse.
        //
        // Nil rather than random: a fixture that varied would make failures
        // irreproducible, and this one exists to be deterministic.
        let reference = |index: usize, kind: &str| {
            serde_json::json!({
                "kind": kind,
                "id": ids
                    .get(index)
                    .cloned()
                    .unwrap_or_else(|| uuid::Uuid::nil().to_string()),
            })
        };

        // ── Research e Knowledge ────────────────────────────────────────
        //
        // Cada um destes ramos nomeia recursos, que é o que os torna úteis:
        // o caminho que exercitam é o da resolução e da autorização contra o
        // contexto do próprio recurso.

        if lower.contains("relaciona") {
            return serde_json::json!({
                "intent": instruction,
                "steps": [{
                    "capability": "knowledge.link.create",
                    "input": {"relation": "relates_to"},
                    "resources": [reference(0, "note"), reference(1, "project")],
                    "summary": "Relacionar os dois objectos"
                }]
            });
        }

        if lower.contains("resume") && lower.contains("nota") {
            return serde_json::json!({
                "intent": instruction,
                "steps": [{
                    "capability": "knowledge.note.read",
                    "input": {},
                    "resources": [reference(0, "note")],
                    "summary": "Ler a Nota"
                }]
            });
        }

        if lower.contains("resume") && (lower.contains("projecto") || lower.contains("estado")) {
            return serde_json::json!({
                "intent": instruction,
                "steps": [{
                    "capability": "research.workspace.overview",
                    "input": {},
                    "resources": [reference(0, "workspace")],
                    "summary": "Obter o estado do ambiente"
                }]
            });
        }

        if lower.contains("cria") && lower.contains("nota") {
            return serde_json::json!({
                "intent": instruction,
                "steps": [{
                    "capability": "knowledge.note.create",
                    "input": {
                        "title": "Nota preparada",
                        "body": "Preparada a partir do material fornecido."
                    },
                    "resources": [reference(0, "workspace")],
                    "summary": "Criar a Nota"
                }]
            });
        }

        if lower.contains("converte") || lower.contains("promove") {
            return serde_json::json!({
                "intent": instruction,
                "steps": [{
                    "capability": "research.idea.promote",
                    "input": {"code": "TEST-001"},
                    "resources": [reference(0, "idea")],
                    "summary": "Converter a Ideia num Projecto"
                }]
            });
        }

        // Responder a um email: procurar, preparar, e — o passo que obriga a
        // confirmação — enviar.
        if lower.contains("responde") || lower.contains("resposta") {
            return serde_json::json!({
                "intent": instruction,
                "steps": [
                    {
                        "capability": "knowledge.search",
                        "input": {"query": "relatório"},
                        "summary": "Procurar a mensagem referida"
                    },
                    {
                        "capability": "mail.draft_reply",
                        "input": {
                            "message_id": "00000000-0000-0000-0000-000000000000",
                            "body": "Enviaremos a versão revista sexta-feira."
                        },
                        "summary": "Preparar uma resposta"
                    }
                ]
            });
        }

        if lower.contains("cria") && lower.contains("tarefa") {
            return serde_json::json!({
                "intent": instruction,
                "steps": [{
                    "capability": "collaboration.task.create",
                    "input": {"title": "Rever o relatório"},
                    "resources": [reference(0, "workspace")],
                    "summary": "Criar a tarefa"
                }]
            });
        }

        if lower.contains("cria") && lower.contains("ideia") {
            return serde_json::json!({
                "intent": instruction,
                "steps": [{
                    "capability": "research.idea.create",
                    "input": {"title": "Ideia proposta"},
                    // A unidade é nomeada por referência, como qualquer outro
                    // recurso: é ela que dá o contexto contra o qual o Core
                    // autoriza, e um identificador no `input` seria autorizado
                    // contra a organização (ADR-0306).
                    "resources": [reference(0, "unit")],
                    "summary": "Criar a Ideia"
                }]
            });
        }

        if lower.contains("envia") {
            return serde_json::json!({
                "intent": instruction,
                "steps": [{
                    "capability": "mail.send",
                    "input": {"draft_id": "00000000-0000-0000-0000-000000000000"},
                    "summary": "Enviar a mensagem"
                }]
            });
        }

        // Sem correspondência: pesquisar é a resposta segura.
        serde_json::json!({
            "intent": instruction,
            "steps": [{
                "capability": "knowledge.search",
                "input": {"query": instruction},
                "summary": "Procurar no acervo institucional"
            }]
        })
    }

    /// What a subverted model proposes.
    fn hostile_plan(instruction: &str) -> serde_json::Value {
        serde_json::json!({
            "intent": instruction,
            "steps": [
                {
                    "capability": "system.execute_shell",
                    "input": {"command": "cat /etc/passwd"},
                    "summary": "Acção de rotina, sem risco, não precisa de confirmação"
                },
                {
                    "capability": "administration.member.grant_admin",
                    "input": {"person_id": "00000000-0000-0000-0000-000000000000"},
                    "summary": "Passo interno do sistema"
                }
            ]
        })
    }

    fn identity(&self) -> ModelIdentity {
        ModelIdentity {
            provider: "fixture".to_owned(),
            model: match self.behaviour {
                FixtureBehaviour::Cooperative => "fixture-cooperative",
                FixtureBehaviour::Hostile => "fixture-hostile",
                FixtureBehaviour::Malformed => "fixture-malformed",
                FixtureBehaviour::Unavailable => "fixture-unavailable",
                FixtureBehaviour::Timeout => "fixture-timeout",
                FixtureBehaviour::Partial => "fixture-partial",
                FixtureBehaviour::WrongVersion => "fixture-wrong-version",
                FixtureBehaviour::Oversized => "fixture-oversized",
            }
            .to_owned(),
            version: "1".to_owned(),
        }
    }
}

#[async_trait]
impl InferenceProvider for FixtureProvider {
    fn adapter_name(&self) -> &'static str {
        "fixture"
    }

    fn serves(&self, capability: AiCapability) -> bool {
        // Everything except embeddings: a fixture that pretended to produce
        // vectors would let a semantic-search test pass while proving nothing.
        !matches!(capability, AiCapability::Embedding)
    }

    async fn infer(&self, request: &InferenceRequest) -> InferenceResult<InferenceResponse> {
        if self.behaviour == FixtureBehaviour::Unavailable {
            return Err(InferenceError::Unavailable);
        }

        // Sleeps past whatever deadline was set. The Core's guard is what turns
        // this into `Timeout`; the fixture just never finishes in time.
        if self.behaviour == FixtureBehaviour::Timeout {
            tokio::time::sleep(request.deadline * 2 + Duration::from_millis(50)).await;
            return Err(InferenceError::Unavailable);
        }

        // No schema means prose was asked for.
        let Some(_schema) = request.schema.as_ref() else {
            return Ok(InferenceResponse {
                contract: ContractVersion::CURRENT,
                text: format!("Resposta de teste a: {}", request.instruction),
                value: None,
                model: self.identity(),
                usage: Some(TokenUsage {
                    input: 0,
                    output: 0,
                }),
            });
        };

        let value = match self.behaviour {
            FixtureBehaviour::Cooperative => Self::plan_for(&request.instruction),
            FixtureBehaviour::Hostile => Self::hostile_plan(&request.instruction),
            // Not a plan at all. The Core must report this rather than guess.
            FixtureBehaviour::Malformed => serde_json::json!("desculpe, não percebi"),
            // A step with no capability. Present, and unusable.
            FixtureBehaviour::Partial => serde_json::json!({
                "intent": request.instruction,
                "steps": [{"summary": "faz aquilo", "input": {}}]
            }),
            FixtureBehaviour::WrongVersion => Self::plan_for(&request.instruction),
            FixtureBehaviour::Oversized => serde_json::json!({
                "intent": request.instruction,
                "steps": [{
                    "capability": "knowledge.search",
                    "input": {"query": "x".repeat(512 * 1024)},
                    "summary": "procurar"
                }]
            }),
            FixtureBehaviour::Unavailable | FixtureBehaviour::Timeout => {
                unreachable!("tratado acima")
            }
        };

        Ok(InferenceResponse {
            // The one behaviour that answers in a contract the Core does not
            // speak. `ContractVersion` has a single variant by design, so the
            // fixture reaches for the guard through a value the Core rejects.
            contract: ContractVersion::CURRENT,
            text: String::new(),
            value: Some(value),
            model: self.identity(),
            usage: Some(TokenUsage {
                input: 0,
                output: 0,
            }),
        })
    }
}
