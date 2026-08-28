//! Data capabilities: datasets institucionais.
//!
//! # A separação que importa
//!
//! Criar um dataset é escrever metadados: código, título, origem, licença,
//! restrições de uso, classificação. Não passa por aqui um único byte de
//! ficheiro.
//!
//! Essa é a razão pela qual esta operação é endereçável e o carregamento de
//! ficheiros não é. Um dataset é uma declaração institucional sobre dados que
//! existem; o ficheiro é a travessia binária que o acompanha, e a travessia
//! binária não pertence ao plano agentic (ADR-0307).
//!
//! Uma capability sobre ficheiros receberia uma de três coisas — bytes, um
//! caminho local ou um URL — e as três seriam formas de o modelo escolher o que
//! entra no armazenamento institucional.

use async_trait::async_trait;
use ocinye_contracts::agentic::{
    ApprovalRequirement, AutonomyLevel, CapabilityDescriptor, CapabilityId, CapabilityResult,
    ExecutionStatus, OperationId, ResourceKind as AgenticKind, ResourceRef, Reversibility,
    RiskLevel,
};
use ocinye_contracts::{Classification, Permission, Scope};

use crate::error::CoreResult;
use crate::modules::agentic::executor::ExecutionContext;
use crate::modules::agentic::registry::CapabilityHandler;
use crate::modules::data::{self, DatasetOrigin, NewDataset};

/// Criar um dataset num ambiente de investigação.
pub struct CreateDataset;

#[async_trait]
impl CapabilityHandler for CreateDataset {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new("data.dataset.create"),
            operation: OperationId::new("data::create_dataset"),
            domain: "data".to_owned(),
            summary: "Criar um dataset institucional com os seus metadados.".to_owned(),
            permission: Permission::DatasetsCreate,
            scope: Scope::ResearchWorkspace,
            // Metadados, e reversível. O risco sai do Registry e não desta
            // decisão: um dataset sem ficheiros é uma declaração, e declarar não
            // é o mesmo que publicar.
            risk: RiskLevel::LowImpact,
            approval: ApprovalRequirement::Never,
            max_autonomy: AutonomyLevel::Workflow,
            reversibility: Reversibility::Reversible,
            supports_dry_run: true,
            classification_ceiling: None,
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["code", "title"],
                "properties": {
                    "code": {"type": "string"},
                    "title": {"type": "string"},
                    "description": {"type": "string"},
                    "origin": {
                        "type": "string",
                        "description": "Como os dados chegaram à instituição."
                    },
                    "licence": {"type": "string"},
                    "usage_restrictions": {"type": "string"},
                    "keywords": {"type": "array", "items": {"type": "string"}},
                    "classification": {"type": "string"}
                }
            }),
        }
    }

    async fn execute(&self, ctx: &ExecutionContext<'_>) -> CoreResult<CapabilityResult> {
        let workspace_id = ctx.one(AgenticKind::Workspace)?.reference.id;
        let code = ctx.text("code")?;
        let title = ctx.text("title")?;

        // A origem é declarada, e não inferida. Um dataset cuja proveniência o
        // sistema adivinhou é um dataset cuja proveniência ninguém sabe.
        let origin = ctx
            .optional::<String>("origin")?
            .and_then(|raw| DatasetOrigin::parse(&raw))
            .unwrap_or(DatasetOrigin::CollectedByOcinye);

        let classification = ctx
            .optional::<String>("classification")?
            .and_then(|raw| Classification::parse(&raw));

        if ctx.dry_run {
            return Ok(CapabilityResult {
                capability: self.descriptor().id,
                status: ExecutionStatus::DryRun,
                detail: format!("Seria criado o dataset «{title}» ({code})."),
                resources: Vec::new(),
                reversibility: Reversibility::NothingToUndo,
                output: None,
            });
        }

        let mut tx = ctx.pool.begin().await?;
        let dataset = data::create_dataset(
            &mut tx,
            ctx.principal,
            ctx.ids,
            workspace_id,
            NewDataset {
                code: code.clone(),
                title: title.clone(),
                description: ctx.optional("description")?,
                origin,
                licence: ctx.optional("licence")?,
                usage_restrictions: ctx.optional("usage_restrictions")?,
                responsible_person_id: None,
                acquisition_date: None,
                keywords: ctx.optional("keywords")?.unwrap_or_default(),
                classification,
            },
        )
        .await?;
        tx.commit().await?;

        Ok(CapabilityResult {
            capability: self.descriptor().id,
            status: ExecutionStatus::Succeeded,
            detail: format!("Dataset «{title}» ({code}) criado."),
            resources: vec![ResourceRef {
                kind: AgenticKind::Dataset,
                id: dataset.id,
                label: Some(title),
            }],
            reversibility: Reversibility::Reversible,
            output: None,
        })
    }
}
