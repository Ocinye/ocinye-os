//! Organisation capabilities: unidades e quem pertence a elas.
//!
//! # Porque estas existem
//!
//! «Cria uma unidade de Materiais e Economia Circular» é quase o exemplo
//! perfeito do Ocinye OS: uma frase que um director escreveria, sobre o acto
//! mais estrutural que a instituição tem. Até ao ADR-0307 o plano agentic não
//! lhe chegava — não por decisão, mas porque ninguém tinha decidido.
//!
//! # Mutação privilegiada
//!
//! As duas mudam o mapa institucional, e a segunda muda o universo de acesso de
//! uma pessoa. São endereçáveis na mesma, porque **risco alto não é, por si só,
//! critério de não-delegabilidade** — nenhuma delas exige que um segredo entre
//! no plano agentic.
//!
//! O que exigem é o resto: confirmação humana obrigatória, o efeito material
//! escrito no plano, e a reautorização do Core no momento da execução. O modelo
//! propõe uma operação exacta; quem decide se ela pode acontecer é o Core, e
//! decide-o outra vez depois de a pessoa confirmar.

use async_trait::async_trait;
use ocinye_contracts::agentic::{
    ApprovalRequirement, AutonomyLevel, CapabilityDescriptor, CapabilityId, CapabilityResult,
    ExecutionStatus, OperationId, ResourceKind as AgenticKind, ResourceRef, Reversibility,
    RiskLevel,
};
use ocinye_contracts::{Permission, Scope};

use crate::error::CoreResult;
use crate::modules::agentic::executor::ExecutionContext;
use crate::modules::agentic::registry::CapabilityHandler;
use crate::modules::organisation::{self, NewUnit};

/// Criar uma unidade científica.
pub struct CreateUnit;

#[async_trait]
impl CapabilityHandler for CreateUnit {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new("organisation.unit.create"),
            operation: OperationId::new("organisation::create_unit"),
            domain: "organisation".to_owned(),
            summary: "Criar uma unidade científica na instituição.".to_owned(),
            permission: Permission::UnitsCreate,
            scope: Scope::Institution,
            risk: RiskLevel::Privileged,
            // Uma unidade é o mapa da instituição. Arquivá-la desfaz, mas o
            // código fica gasto e as referências ficam — «reversível» não quer
            // dizer «sem consequência».
            approval: ApprovalRequirement::Always,
            max_autonomy: AutonomyLevel::Act,
            reversibility: Reversibility::Reversible,
            supports_dry_run: true,
            classification_ceiling: None,
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["code", "name"],
                "properties": {
                    "code": {"type": "string", "description": "Sigla institucional, por exemplo ENG."},
                    "name": {"type": "string"},
                    "description": {"type": "string"},
                    "research_areas": {"type": "array", "items": {"type": "string"}}
                }
            }),
        }
    }

    async fn execute(&self, ctx: &ExecutionContext<'_>) -> CoreResult<CapabilityResult> {
        let code = ctx.text("code")?;
        let name = ctx.text("name")?;
        let description: Option<String> = ctx.optional("description")?;
        let research_areas: Vec<String> = ctx.optional("research_areas")?.unwrap_or_default();

        if ctx.dry_run {
            return Ok(CapabilityResult {
                capability: self.descriptor().id,
                status: ExecutionStatus::DryRun,
                detail: format!("Seria criada a unidade «{name}» ({code})."),
                resources: Vec::new(),
                reversibility: Reversibility::NothingToUndo,
                output: None,
            });
        }

        let mut tx = ctx.pool.begin().await?;
        let unit = organisation::create_unit(
            &mut tx,
            ctx.principal,
            ctx.ids,
            NewUnit {
                code: code.clone(),
                name: name.clone(),
                description,
                research_areas,
            },
        )
        .await?;
        tx.commit().await?;

        Ok(CapabilityResult {
            capability: self.descriptor().id,
            status: ExecutionStatus::Succeeded,
            detail: format!("Unidade «{name}» ({code}) criada."),
            resources: vec![ResourceRef {
                kind: AgenticKind::Unit,
                id: unit.id,
                label: Some(name),
            }],
            reversibility: Reversibility::Reversible,
            output: None,
        })
    }
}

// Aqui esteve `AddUnitMember`, e a razão pela qual saiu vale mais do que o
// código.
//
// Parecia metadado organizacional: acrescentar alguém a uma unidade não concede
// papel nenhum nem escreve permissão nenhuma. Mediu-se — o teste
// `pertencer_a_uma_unidade_expande_o_acesso_efectivo` — e a mesma pessoa, sem
// lhe tocar em papel técnico, passa a poder criar ideias e ver datasets só por
// ser acrescentada.
//
// É mutação da fronteira de autoridade, e fecha-se pela mesma regra que fecha
// `grant_role` e `create_grant` (ADR-0307).
