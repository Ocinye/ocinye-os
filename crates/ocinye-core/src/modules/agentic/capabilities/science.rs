//! Scientific lifecycle capabilities.
//!
//! # O que um agente pode fazer com ciência, e o que não pode
//!
//! **Descrever trabalho é endereçável.** Enunciar uma hipótese, criar uma
//! metodologia, publicar uma versão dela, desenhar um estudo, registar uma
//! execução, registar um resultado — tudo isto é registo do que se fez ou do
//! que se vai fazer, e um agente pode propô-lo. O Core autoriza antes de
//! escrever, como em qualquer outro sítio.
//!
//! **Validar um resultado não é.** Não há aqui `RecordValidation`, e a ausência
//! é deliberada: `science::record_validation` está no catálogo como
//! `non_delegable`, atrás de uma `AuthorityBoundary`. Dizer «este resultado
//! confirma-se» é uma afirmação institucional sobre o que a Ocinye sabe, e o
//! peso dela é de quem a faz. Delegá-la separaria a afirmação da pessoa que a
//! sustenta — o que não é um problema de risco, é de autoria.
//!
//! # Proveniência
//!
//! Nenhuma capability daqui escreve arestas de proveniência à mão. As arestas
//! que estas operações produzem — `produced_by`, `used_methodology`,
//! `tests` — são escritas pelo serviço, na mesma transacção, porque a operação
//! as **observou**. É a diferença que o `origin` guarda:
//!
//! > **A IA pode sugerir proveniência. A IA não inventa proveniência
//! > institucional.**
//!
//! Um agente que quisesse afirmar uma relação que nenhuma operação observou usa
//! `knowledge.link.create`, que a marca como `declared` — e nunca como se
//! tivesse sido vista acontecer.

use async_trait::async_trait;
use ocinye_contracts::agentic::{
    ApprovalRequirement, AutonomyLevel, CapabilityDescriptor, CapabilityId, CapabilityResult,
    ExecutionStatus, OperationId, ResourceKind as AgenticKind, ResourceRef, Reversibility,
    RiskLevel,
};
use ocinye_contracts::{Classification, Permission, Scope};

use crate::error::{CoreError, CoreResult};
use crate::modules::agentic::executor::ExecutionContext;
use crate::modules::agentic::registry::CapabilityHandler;
use crate::modules::science;

/// A classificação que o modelo propôs é um pedido, não uma decisão.
///
/// O serviço limita-a contra o ambiente e recusa a que não puder conceder.
fn classificacao_pedida(ctx: &ExecutionContext<'_>) -> CoreResult<Classification> {
    Ok(ctx
        .optional::<String>("classification")?
        .and_then(|raw| Classification::parse(&raw))
        .unwrap_or(Classification::Internal))
}

/// Um texto opcional que só conta quando tem conteúdo.
fn texto_opcional(ctx: &ExecutionContext<'_>, campo: &str) -> CoreResult<Option<String>> {
    Ok(ctx
        .optional::<String>(campo)?
        .map(|t| t.trim().to_owned())
        .filter(|t| !t.is_empty()))
}

/// O que se devolve quando `dry_run` está ligado.
fn ensaio(id: CapabilityId, detalhe: String) -> CapabilityResult {
    CapabilityResult {
        capability: id,
        status: ExecutionStatus::DryRun,
        detail: detalhe,
        resources: Vec::new(),
        reversibility: Reversibility::NothingToUndo,
        output: None,
    }
}

/// State a hypothesis in a research environment.
///
/// # Porque é reversível
///
/// Uma hipótese enunciada e depois abandonada é um desfecho científico
/// legítimo, e o domínio representa-o (`CLAUDE.md` §9). Não é um efeito que
/// alguém tenha de desfazer à pressa.
pub struct StateHypothesis;

#[async_trait]
impl CapabilityHandler for StateHypothesis {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new("science.hypothesis.create"),
            operation: OperationId::new("science::create_hypothesis"),
            domain: "science".to_owned(),
            summary: "Enunciar uma hipótese num ambiente de investigação.".to_owned(),
            permission: Permission::ScienceCreate,
            scope: Scope::ResearchWorkspace,
            risk: RiskLevel::LowImpact,
            approval: ApprovalRequirement::Never,
            max_autonomy: AutonomyLevel::Workflow,
            reversibility: Reversibility::Reversible,
            supports_dry_run: true,
            classification_ceiling: None,
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["statement"],
                "properties": {
                    "statement": {
                        "type": "string",
                        "description": "A afirmação que se quer testar."
                    },
                    "rationale": {
                        "type": "string",
                        "description": "Porque vale a pena testá-la."
                    },
                    "classification": {"type": "string"}
                }
            }),
        }
    }

    async fn execute(&self, ctx: &ExecutionContext<'_>) -> CoreResult<CapabilityResult> {
        let workspace_id = ctx.one(AgenticKind::Workspace)?.reference.id;
        let statement = ctx.text("statement")?;
        let rationale = texto_opcional(ctx, "rationale")?;
        let classification = classificacao_pedida(ctx)?;

        if ctx.dry_run {
            return Ok(ensaio(
                self.descriptor().id,
                format!("Seria enunciada a hipótese «{statement}»."),
            ));
        }

        let mut tx = ctx.pool.begin().await?;
        let hypothesis = science::create_hypothesis(
            &mut tx,
            ctx.principal,
            ctx.ids,
            workspace_id,
            &statement,
            rationale.as_deref(),
            classification,
        )
        .await?;
        tx.commit().await?;

        Ok(CapabilityResult {
            capability: self.descriptor().id,
            status: ExecutionStatus::Succeeded,
            detail: format!("Hipótese «{statement}» enunciada."),
            resources: vec![ResourceRef {
                kind: AgenticKind::Hypothesis,
                id: hypothesis.id,
                label: Some(statement),
            }],
            reversibility: Reversibility::Reversible,
            output: Some(serde_json::json!({ "hypothesis_id": hypothesis.id })),
        })
    }
}

/// Create a methodology.
///
/// # Porque a metodologia e a versão são duas capabilities
///
/// Porque são dois actos. Criar a metodologia dá-lhe identidade — o nome pelo
/// qual a instituição a conhece daqui a cinco anos. Publicar uma versão fixa o
/// que ela diz **agora**, e é a versão que a proveniência guarda.
///
/// Uma capability que fizesse as duas coisas escondia a segunda dentro da
/// primeira, e a segunda é a que a linhagem lê.
pub struct CreateMethodology;

#[async_trait]
impl CapabilityHandler for CreateMethodology {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new("science.methodology.create"),
            operation: OperationId::new("science::create_methodology"),
            domain: "science".to_owned(),
            summary: "Criar uma metodologia.".to_owned(),
            permission: Permission::ScienceCreate,
            scope: Scope::ResearchWorkspace,
            risk: RiskLevel::LowImpact,
            approval: ApprovalRequirement::Never,
            max_autonomy: AutonomyLevel::Workflow,
            reversibility: Reversibility::Reversible,
            supports_dry_run: true,
            classification_ceiling: None,
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["title"],
                "properties": {
                    "title": {"type": "string"},
                    "purpose": {
                        "type": "string",
                        "description": "Para que serve este método."
                    },
                    "classification": {"type": "string"}
                }
            }),
        }
    }

    async fn execute(&self, ctx: &ExecutionContext<'_>) -> CoreResult<CapabilityResult> {
        let workspace_id = ctx.one(AgenticKind::Workspace)?.reference.id;
        let title = ctx.text("title")?;
        let purpose = texto_opcional(ctx, "purpose")?;
        let classification = classificacao_pedida(ctx)?;

        if ctx.dry_run {
            return Ok(ensaio(
                self.descriptor().id,
                format!("Seria criada a metodologia «{title}»."),
            ));
        }

        let mut tx = ctx.pool.begin().await?;
        let methodology = science::create_methodology(
            &mut tx,
            ctx.principal,
            ctx.ids,
            workspace_id,
            &title,
            purpose.as_deref(),
            classification,
        )
        .await?;
        tx.commit().await?;

        Ok(CapabilityResult {
            capability: self.descriptor().id,
            status: ExecutionStatus::Succeeded,
            detail: format!("Metodologia «{title}» criada. Falta publicar uma versão."),
            resources: vec![ResourceRef {
                kind: AgenticKind::Methodology,
                id: methodology.id,
                label: Some(title),
            }],
            reversibility: Reversibility::Reversible,
            output: Some(serde_json::json!({ "methodology_id": methodology.id })),
        })
    }
}

/// Publish a methodology version.
///
/// # Porque não é reversível
///
/// Porque publicar uma versão é o acto que a proveniência passa a citar. Um
/// resultado produzido com a versão 2 continua a dizer «versão 2» depois de a
/// 5 existir, e essa é toda a razão de a versão ser um recurso.
///
/// Apagar uma versão publicada partiria arestas que já apontam para ela. O
/// domínio substitui uma versão — `superseded_by_id` —, e não a retira.
pub struct PublishMethodologyVersion;

#[async_trait]
impl CapabilityHandler for PublishMethodologyVersion {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new("science.methodology.publish_version"),
            operation: OperationId::new("science::publish_methodology_version"),
            domain: "science".to_owned(),
            summary: "Publicar uma versão de uma metodologia.".to_owned(),
            permission: Permission::ScienceCreate,
            scope: Scope::ResearchWorkspace,
            risk: RiskLevel::LowImpact,
            approval: ApprovalRequirement::Never,
            max_autonomy: AutonomyLevel::Workflow,
            // A versão anterior fica, marcada como substituída. O que não volta
            // atrás é a publicação: outras arestas passam a poder citá-la.
            reversibility: Reversibility::Irreversible,
            supports_dry_run: true,
            classification_ceiling: None,
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["label", "summary"],
                "properties": {
                    "label": {
                        "type": "string",
                        "description": "Como se chama — «v2», «2026-rev-b»."
                    },
                    "summary": {
                        "type": "string",
                        "description": "O que esta versão diz, em resumo."
                    }
                }
            }),
        }
    }

    async fn execute(&self, ctx: &ExecutionContext<'_>) -> CoreResult<CapabilityResult> {
        let methodology = ctx.one(AgenticKind::Methodology)?;
        let label = ctx.text("label")?;
        let summary = ctx.text("summary")?;

        if ctx.dry_run {
            return Ok(ensaio(
                self.descriptor().id,
                format!(
                    "Seria publicada a versão «{label}» de «{}».",
                    methodology.title
                ),
            ));
        }

        let mut tx = ctx.pool.begin().await?;
        let version = science::publish_methodology_version(
            &mut tx,
            ctx.pool,
            ctx.principal,
            ctx.ids,
            methodology.reference.id,
            &label,
            &summary,
            None,
        )
        .await?;
        tx.commit().await?;

        Ok(CapabilityResult {
            capability: self.descriptor().id,
            status: ExecutionStatus::Succeeded,
            detail: format!("«{}» está agora na versão «{label}».", methodology.title),
            resources: vec![ResourceRef {
                kind: AgenticKind::MethodologyVersion,
                id: version.id,
                label: Some(label),
            }],
            reversibility: Reversibility::Irreversible,
            output: Some(serde_json::json!({
                "methodology_version_id": version.id,
                "sequence": version.sequence,
            })),
        })
    }
}

/// Design a study.
///
/// # A hipótese entra por `resources`, e não pelo input
///
/// Porque `resources` é o campo que o executor resolve, com a política de quem
/// age. Ler um identificador do input contornaria o portão: bastaria um UUID
/// adivinhado para ligar um estudo a uma hipótese que a pessoa não alcança
/// (`CLAUDE.md` §34.2).
pub struct DesignStudy;

#[async_trait]
impl CapabilityHandler for DesignStudy {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new("science.study.create"),
            operation: OperationId::new("science::create_study"),
            domain: "science".to_owned(),
            summary: "Desenhar um estudo: experimento, simulação ou análise.".to_owned(),
            permission: Permission::ScienceCreate,
            scope: Scope::ResearchWorkspace,
            risk: RiskLevel::LowImpact,
            approval: ApprovalRequirement::Never,
            max_autonomy: AutonomyLevel::Workflow,
            reversibility: Reversibility::Reversible,
            supports_dry_run: true,
            classification_ceiling: None,
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["title", "kind"],
                "properties": {
                    "title": {"type": "string"},
                    "kind": {
                        "type": "string",
                        "description": "physical_experiment, simulation ou analysis."
                    },
                    "objective": {
                        "type": "string",
                        "description": "O que se propõe descobrir."
                    },
                    "classification": {"type": "string"}
                }
            }),
        }
    }

    async fn execute(&self, ctx: &ExecutionContext<'_>) -> CoreResult<CapabilityResult> {
        let workspace_id = ctx.one(AgenticKind::Workspace)?.reference.id;
        // A hipótese é opcional: nem todo o estudo testa uma. Quando vem, vem
        // resolvida, o que já provou que quem age a alcança.
        let hypothesis_id = ctx
            .resources
            .iter()
            .find(|r| r.reference.kind == AgenticKind::Hypothesis)
            .map(|r| r.reference.id);

        // A versão, e nunca a metodologia. Se um agente propuser a metodologia
        // mutável, ela não é um `MethodologyVersion` e não entra aqui — o
        // executor resolveu-a, e a matriz recusaria a aresta de qualquer modo.
        let methodology_version_id = ctx
            .resources
            .iter()
            .find(|r| r.reference.kind == AgenticKind::MethodologyVersion)
            .map(|r| r.reference.id);

        let title = ctx.text("title")?;
        let kind = ctx.text("kind")?;
        let objective = texto_opcional(ctx, "objective")?;
        let classification = classificacao_pedida(ctx)?;

        if ctx.dry_run {
            return Ok(ensaio(
                self.descriptor().id,
                format!("Seria desenhado o estudo «{title}»."),
            ));
        }

        let mut tx = ctx.pool.begin().await?;
        let study = science::create_study(
            &mut tx,
            ctx.pool,
            ctx.principal,
            ctx.ids,
            workspace_id,
            hypothesis_id,
            methodology_version_id,
            &title,
            &kind,
            objective.as_deref(),
            classification,
        )
        .await?;
        tx.commit().await?;

        Ok(CapabilityResult {
            capability: self.descriptor().id,
            status: ExecutionStatus::Succeeded,
            detail: format!("Estudo «{title}» desenhado."),
            resources: vec![ResourceRef {
                kind: AgenticKind::Study,
                id: study.id,
                label: Some(title),
            }],
            reversibility: Reversibility::Reversible,
            output: Some(serde_json::json!({ "study_id": study.id })),
        })
    }
}

/// Record a study execution.
///
/// # Porque é irreversível
///
/// Uma execução é um facto datado: correu, num sítio, com um software, numa
/// versão. Apagá-la não desfaz a corrida — apaga o registo de que aconteceu, e
/// é exactamente o que a reprodutibilidade precisa de conservar.
pub struct RecordExecution;

#[async_trait]
impl CapabilityHandler for RecordExecution {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new("science.execution.record"),
            operation: OperationId::new("science::record_execution"),
            domain: "science".to_owned(),
            summary: "Registar uma execução de um estudo.".to_owned(),
            permission: Permission::ScienceCreate,
            scope: Scope::ResearchWorkspace,
            risk: RiskLevel::LowImpact,
            approval: ApprovalRequirement::Never,
            max_autonomy: AutonomyLevel::Workflow,
            reversibility: Reversibility::Irreversible,
            supports_dry_run: true,
            classification_ceiling: None,
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "status": {
                        "type": "string",
                        "description":
                            "recorded, running, succeeded, failed ou aborted. \
                             Por omissão, recorded."
                    },
                    "environment": {
                        "type": "string",
                        "description": "Onde correu, quando não foi num nó que o Ocinye conhece."
                    },
                    "software_name": {"type": "string"},
                    "software_version": {"type": "string"},
                    "software_commit": {"type": "string"},
                    "notes": {"type": "string"}
                }
            }),
        }
    }

    async fn execute(&self, ctx: &ExecutionContext<'_>) -> CoreResult<CapabilityResult> {
        let study = ctx.one(AgenticKind::Study)?;
        // Sem estado, o Core escolhe o dele. Repetir aqui um valor por
        // omissão criaria uma segunda opinião sobre o que uma execução é
        // quando ninguém o diz — e as duas acabariam por discordar.
        let status = texto_opcional(ctx, "status")?.unwrap_or_default();
        let environment = texto_opcional(ctx, "environment")?;
        let software_name = texto_opcional(ctx, "software_name")?;
        let software_version = texto_opcional(ctx, "software_version")?;
        let software_commit = texto_opcional(ctx, "software_commit")?;
        let notes = texto_opcional(ctx, "notes")?;

        if ctx.dry_run {
            return Ok(ensaio(
                self.descriptor().id,
                format!("Seria registada uma execução de «{}».", study.title),
            ));
        }

        // O que a corrida seguiu e consumiu vem de `resources`, resolvido pelo
        // executor — nunca de identificadores no input.
        let methodology_version_id = ctx
            .resources
            .iter()
            .find(|r| r.reference.kind == AgenticKind::MethodologyVersion)
            .map(|r| r.reference.id);
        let dataset_version_ids: Vec<uuid::Uuid> = ctx
            .resources
            .iter()
            .filter(|r| r.reference.kind == AgenticKind::DatasetVersion)
            .map(|r| r.reference.id)
            .collect();

        let record = science::ExecutionRecord {
            status: &status,
            methodology_version_id,
            dataset_version_ids: &dataset_version_ids,
            compute_node_id: None,
            environment: environment.as_deref(),
            software_name: software_name.as_deref(),
            software_version: software_version.as_deref(),
            software_commit: software_commit.as_deref(),
            notes: notes.as_deref(),
        };

        let mut tx = ctx.pool.begin().await?;
        let execution = science::record_execution(
            &mut tx,
            ctx.pool,
            ctx.principal,
            ctx.ids,
            study.reference.id,
            &record,
        )
        .await?;
        tx.commit().await?;

        let etiqueta = format!("{} · execução {}", study.title, execution.sequence);
        Ok(CapabilityResult {
            capability: self.descriptor().id,
            status: ExecutionStatus::Succeeded,
            detail: format!(
                "Execução {} de «{}» registada.",
                execution.sequence, study.title
            ),
            resources: vec![ResourceRef {
                kind: AgenticKind::StudyExecution,
                id: execution.id,
                label: Some(etiqueta),
            }],
            reversibility: Reversibility::Irreversible,
            output: Some(serde_json::json!({
                "execution_id": execution.id,
                "sequence": execution.sequence,
            })),
        })
    }
}

/// Record a scientific result.
///
/// # A proveniência não vem do input
///
/// Quando a execução é dada, o serviço escreve `produced_by` na mesma
/// transacção — porque a operação **observou** que aquele resultado saiu
/// daquela corrida. Essa aresta nasce com `origin = operation`, e nenhuma
/// entrada deste esquema lhe toca.
///
/// É a fronteira que separa o que um agente sugere do que a instituição
/// registou.
pub struct RecordResult;

#[async_trait]
impl CapabilityHandler for RecordResult {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new("science.result.create"),
            operation: OperationId::new("science::create_result"),
            domain: "science".to_owned(),
            summary: "Registar um resultado, com a proveniência que a operação conhece.".to_owned(),
            permission: Permission::ScienceCreate,
            scope: Scope::ResearchWorkspace,
            risk: RiskLevel::LowImpact,
            approval: ApprovalRequirement::Never,
            max_autonomy: AutonomyLevel::Workflow,
            reversibility: Reversibility::Irreversible,
            supports_dry_run: true,
            classification_ceiling: None,
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["title", "summary"],
                "properties": {
                    "title": {"type": "string"},
                    "summary": {
                        "type": "string",
                        "description": "O que o resultado diz — incluindo quando diz que a hipótese não se sustenta."
                    },
                    "classification": {"type": "string"}
                }
            }),
        }
    }

    async fn execute(&self, ctx: &ExecutionContext<'_>) -> CoreResult<CapabilityResult> {
        let workspace_id = ctx.one(AgenticKind::Workspace)?.reference.id;
        let execution_id = ctx
            .resources
            .iter()
            .find(|r| r.reference.kind == AgenticKind::StudyExecution)
            .map(|r| r.reference.id);

        let title = ctx.text("title")?;
        let summary = ctx.text("summary")?;
        let classification = classificacao_pedida(ctx)?;

        if ctx.dry_run {
            return Ok(ensaio(
                self.descriptor().id,
                format!("Seria registado o resultado «{title}»."),
            ));
        }

        let mut tx = ctx.pool.begin().await?;
        let result = science::create_result(
            &mut tx,
            ctx.pool,
            ctx.principal,
            ctx.ids,
            workspace_id,
            execution_id,
            &title,
            &summary,
            classification,
        )
        .await?;
        tx.commit().await?;

        let proveniencia = if execution_id.is_some() {
            " A execução que o produziu ficou ligada."
        } else {
            ""
        };

        Ok(CapabilityResult {
            capability: self.descriptor().id,
            status: ExecutionStatus::Succeeded,
            detail: format!("Resultado «{title}» registado.{proveniencia}"),
            resources: vec![ResourceRef {
                kind: AgenticKind::Result,
                id: result.id,
                label: Some(title),
            }],
            reversibility: Reversibility::Irreversible,
            output: Some(serde_json::json!({ "result_id": result.id })),
        })
    }
}

/// Walk the lineage of a resource.
///
/// # Porque não tem tecto de classificação
///
/// Porque a travessia não precisa de um: cada nó é resolvido pelo serviço que o
/// detém, com a política de quem percorre, e um nó que essa política recuse
/// **não aparece** — nem o nó, nem a sua existência, nem uma contagem que a
/// confirme. Um tecto aqui seria uma segunda política a dizer o mesmo, e duas
/// políticas acabam por discordar.
pub struct ReadLineage;

#[async_trait]
impl CapabilityHandler for ReadLineage {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new("science.lineage.read"),
            operation: OperationId::new("science::lineage"),
            domain: "science".to_owned(),
            summary: "Percorrer a linhagem de um recurso, a montante ou a jusante.".to_owned(),
            permission: Permission::ScienceView,
            scope: Scope::ResearchWorkspace,
            risk: RiskLevel::ReadOnly,
            approval: ApprovalRequirement::Never,
            max_autonomy: AutonomyLevel::Autonomous,
            reversibility: Reversibility::NothingToUndo,
            supports_dry_run: false,
            classification_ceiling: None,
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "direction": {
                        "type": "string",
                        "description": "upstream — de onde veio; downstream — o que dependeu disto. Por omissão, upstream."
                    },
                    "depth": {
                        "type": "integer",
                        "description": "Quantos saltos, até 5."
                    }
                }
            }),
        }
    }

    async fn execute(&self, ctx: &ExecutionContext<'_>) -> CoreResult<CapabilityResult> {
        let [raiz] = ctx.resources else {
            return Err(CoreError::Validation(
                "A linhagem parte exactamente de um recurso.".to_owned(),
            ));
        };

        let sentido = match texto_opcional(ctx, "direction")?.as_deref() {
            Some("downstream") => science::Sentido::Jusante,
            Some("upstream") | None => science::Sentido::Montante,
            Some(outro) => {
                return Err(CoreError::Validation(format!(
                    "«{outro}» não é um sentido. Usa upstream ou downstream."
                )))
            }
        };

        let profundidade: u8 = ctx
            .optional::<u8>("depth")?
            .unwrap_or(science::PROFUNDIDADE_MAXIMA);

        let linhagem = science::percorrer(
            ctx.pool,
            ctx.principal,
            &raiz.reference,
            sentido,
            profundidade,
        )
        .await?;

        let detalhe = if linhagem.passos.is_empty() {
            format!("«{}» não tem linhagem {}.", raiz.title, sentido.label())
        } else {
            format!(
                "{} — {} passos a {}.",
                raiz.title,
                linhagem.passos.len(),
                sentido.label()
            )
        };

        Ok(CapabilityResult {
            capability: self.descriptor().id,
            status: ExecutionStatus::Succeeded,
            detail: detalhe,
            resources: vec![raiz.as_ref()],
            reversibility: Reversibility::NothingToUndo,
            output: Some(
                serde_json::to_value(&linhagem)
                    .map_err(|e| CoreError::Internal(format!("A linhagem não serializou: {e}")))?,
            ),
        })
    }
}
