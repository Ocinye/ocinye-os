//! O que um membro faz sobre si próprio.
//!
//! # O módulo que quase teve outro nome
//!
//! Este ficheiro chamou-se `authority` e chegou a conter cinco capabilities
//! sobre papéis, acessos explícitos e estado de contas. Nenhuma delas ficou, e a
//! razão vale mais do que o código que saiu.
//!
//! O ADR-0307 diz que **risco alto não é, por si só, critério de
//! não-delegabilidade** — e continua a dizê-lo. Mas há uma segunda classe, que
//! não é sobre risco:
//!
//! > **An operation whose primary effect is to change the authorization
//! > boundary or another person's ability to access the system is non-delegable
//! > by architecture.**
//!
//! Conceder um papel, dar um acesso explícito ou suspender uma conta mudam
//! **quem poderá exercer autoridade depois da operação**. Enviar um email
//! externo não muda: é de alto impacto e continua endereçável.
//!
//! # O ataque que isto elimina
//!
//! Um documento, um email ou um dataset hostis são `UNTRUSTED DATA`, e não
//! conseguem autorizar nada — o Core impediria. Mas conseguem **induzir
//! propostas**: texto que leva o plano a sugerir, uma e outra vez, uma escalada
//! plausível, até alguém confirmar uma delas por cansaço.
//!
//! Contra essa classe, a confirmação humana é a última barreira. Não publicar a
//! capability elimina-a inteira — e é por isso que a guarda
//! `is_delegable_to_agents` continua onde estava.

use async_trait::async_trait;
use ocinye_contracts::agentic::{
    ApprovalRequirement, AutonomyLevel, CapabilityDescriptor, CapabilityId, CapabilityResult,
    ExecutionStatus, OperationId, Reversibility, RiskLevel,
};
use ocinye_contracts::{Permission, Scope};
use uuid::Uuid;

use crate::error::{CoreError, CoreResult};
use crate::modules::agentic::executor::ExecutionContext;
use crate::modules::agentic::registry::CapabilityHandler;
use crate::modules::identity;

/// Terminar uma sessão própria.
///
/// # Só as próprias
///
/// O identificador da sessão vem do pedido, e não autoriza nada: o Core resolve
/// a pessoa a quem a sessão pertence e recusa se não for esta. Conhecer o UUID
/// de uma sessão continua a não ser autoridade para a encerrar.
pub struct RevokeOwnSession;

#[async_trait]
impl CapabilityHandler for RevokeOwnSession {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new("identity.session.revoke"),
            operation: OperationId::new("identity::revoke_own_session"),
            domain: "identity".to_owned(),
            summary: "Terminar uma das suas próprias sessões.".to_owned(),
            permission: Permission::AiUse,
            scope: Scope::Institution,
            // Não é privilegiada — é sobre a própria pessoa — mas mexe em
            // autenticação activa, e pode ser a sessão que está a ser usada
            // para pedir. Confirma-se.
            risk: RiskLevel::MaterialMutation,
            approval: ApprovalRequirement::Always,
            max_autonomy: AutonomyLevel::Act,
            reversibility: Reversibility::Irreversible,
            supports_dry_run: true,
            classification_ceiling: None,
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["session_id"],
                "properties": {
                    "session_id": {"type": "string"},
                    "reason": {"type": "string"}
                }
            }),
        }
    }

    async fn execute(&self, ctx: &ExecutionContext<'_>) -> CoreResult<CapabilityResult> {
        let session_id: Uuid = ctx
            .text("session_id")?
            .parse()
            .map_err(|_| CoreError::Validation("Esse identificador não é válido.".to_owned()))?;
        let reason = ctx
            .optional::<String>("reason")?
            .unwrap_or_else(|| "Terminada pelo próprio membro.".to_owned());

        if ctx.dry_run {
            return Ok(CapabilityResult {
                capability: self.descriptor().id,
                status: ExecutionStatus::DryRun,
                detail: "A sessão seria terminada. Se for esta, o acesso acaba aqui.".to_owned(),
                resources: Vec::new(),
                reversibility: Reversibility::NothingToUndo,
                output: None,
            });
        }

        identity::revoke_own_session(ctx.pool, ctx.principal, session_id, &reason).await?;

        Ok(CapabilityResult {
            capability: self.descriptor().id,
            status: ExecutionStatus::Succeeded,
            detail: "Sessão terminada.".to_owned(),
            resources: Vec::new(),
            reversibility: Reversibility::Irreversible,
            output: None,
        })
    }
}

/// Escolher um avatar do catálogo do produto.
///
/// # A operação pequena que prova a regra
///
/// «Usa o avatar Compute 03» não muda nada de institucional. Está aqui
/// precisamente por isso: operar o Ocinye OS por intenção não é um privilégio
/// das operações graves.
///
/// Carregar uma fotografia continua fora — não por ser secreta, mas porque a
/// execução segura exige uma travessia binária mediada pela pessoa, e bytes,
/// caminhos locais e URLs não são entradas agentic (ADR-0307).
pub struct ChooseAvatarPreset;

#[async_trait]
impl CapabilityHandler for ChooseAvatarPreset {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new("identity.avatar.choose_preset"),
            operation: OperationId::new("identity::choose_preset"),
            domain: "identity".to_owned(),
            summary: "Escolher um dos avatares Ocinye como imagem de perfil.".to_owned(),
            permission: Permission::AiUse,
            scope: Scope::Institution,
            risk: RiskLevel::LowImpact,
            approval: ApprovalRequirement::Never,
            max_autonomy: AutonomyLevel::Workflow,
            reversibility: Reversibility::Reversible,
            supports_dry_run: true,
            classification_ceiling: None,
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["preset"],
                "properties": {
                    "preset": {
                        "type": "string",
                        "description": "Um identificador do catálogo Ocinye, por exemplo compute-03."
                    }
                }
            }),
        }
    }

    async fn execute(&self, ctx: &ExecutionContext<'_>) -> CoreResult<CapabilityResult> {
        let preset = ctx.text("preset")?;

        if ctx.dry_run {
            return Ok(CapabilityResult {
                capability: self.descriptor().id,
                status: ExecutionStatus::DryRun,
                detail: format!("A imagem de perfil passaria a «{preset}»."),
                resources: Vec::new(),
                reversibility: Reversibility::NothingToUndo,
                output: None,
            });
        }

        identity::choose_preset(ctx.pool, ctx.principal, None, &preset).await?;

        Ok(CapabilityResult {
            capability: self.descriptor().id,
            status: ExecutionStatus::Succeeded,
            detail: format!("Imagem de perfil alterada para «{preset}»."),
            resources: Vec::new(),
            reversibility: Reversibility::Reversible,
            output: None,
        })
    }
}
