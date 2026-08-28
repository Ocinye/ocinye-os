//! Mail capabilities.
//!
//! # Preparing is not sending
//!
//! Two capabilities produce drafts and one sends. They are separate entries in
//! the registry with different risk levels, so an agent that may compose is not
//! thereby an agent that may send — the distinction is in the data, not in a
//! convention somebody could refactor away (ADR-0406, briefing §51, §148).

use async_trait::async_trait;
use ocinye_contracts::agentic::{
    ApprovalRequirement, AutonomyLevel, CapabilityDescriptor, CapabilityId, CapabilityResult,
    ExecutionStatus, OperationId, ResourceKind as AgenticKind, ResourceRef, Reversibility,
    RiskLevel,
};
use ocinye_contracts::{Permission, Scope};
use uuid::Uuid;

use crate::error::{CoreError, CoreResult};
use crate::modules::agentic::executor::ExecutionContext;
use crate::modules::agentic::registry::CapabilityHandler;
use crate::modules::mail;
use crate::modules::mail::policy::{SendDecision, SendPolicy};

/// Prepare a new message.
///
/// Produces a draft. Nothing leaves the institution.
pub struct DraftMessage;

#[async_trait]
impl CapabilityHandler for DraftMessage {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new("mail.draft"),
            operation: OperationId::new("mail::draft"),
            domain: "mail".to_owned(),
            summary: "Preparar uma mensagem. Não envia.".to_owned(),
            permission: Permission::MailUse,
            scope: Scope::Resource,
            // A draft is low impact and reversible: it is deleted by
            // discarding it, and nobody outside sees it.
            risk: RiskLevel::LowImpact,
            approval: ApprovalRequirement::Never,
            max_autonomy: AutonomyLevel::Workflow,
            reversibility: Reversibility::Reversible,
            supports_dry_run: false,
            classification_ceiling: None,
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["mailbox_id", "to", "subject", "body"],
                "properties": {
                    "mailbox_id": {"type": "string"},
                    "to": {"type": "array"},
                    "subject": {"type": "string"},
                    "body": {"type": "string"}
                }
            }),
        }
    }

    async fn execute(&self, ctx: &ExecutionContext<'_>) -> CoreResult<CapabilityResult> {
        let mailbox_id: Uuid = ctx.field("mailbox_id")?;
        let to: Vec<String> = ctx.field("to")?;
        let subject = ctx.text("subject")?;
        let body = ctx.text("body")?;

        if to.is_empty() {
            return Err(CoreError::Validation(
                "Indique pelo menos um destinatário.".to_owned(),
            ));
        }

        // Resolving the mailbox is also the access check: a mailbox that is not
        // the caller's reads as not found (ADR-0404).
        let mailbox = mail::mailbox(ctx.pool, ctx.principal, mailbox_id).await?;

        if !mailbox.may_send() {
            return Err(CoreError::PermissionDenied(
                "Não pode enviar a partir desta caixa de correio.".to_owned(),
            ));
        }

        let draft_id = mail::repository::create_draft(
            ctx.pool,
            mailbox_id,
            ctx.principal.person_id,
            &subject,
            &body,
            &to,
            None,
        )
        .await?;

        Ok(CapabilityResult {
            capability: self.descriptor().id,
            status: ExecutionStatus::Succeeded,
            detail: format!(
                "Rascunho preparado para {}. Não foi enviado.",
                to.join(", ")
            ),
            resources: vec![ResourceRef {
                kind: AgenticKind::MailDraft,
                id: draft_id,
                label: Some(subject),
            }],
            reversibility: Reversibility::Reversible,
            output: None,
        })
    }
}

/// Prepare a reply to a message.
pub struct DraftReply;

#[async_trait]
impl CapabilityHandler for DraftReply {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new("mail.draft_reply"),
            operation: OperationId::new("mail::draft_reply"),
            domain: "mail".to_owned(),
            summary: "Preparar uma resposta a uma mensagem. Não envia.".to_owned(),
            permission: Permission::MailUse,
            scope: Scope::Resource,
            risk: RiskLevel::LowImpact,
            approval: ApprovalRequirement::Never,
            max_autonomy: AutonomyLevel::Workflow,
            reversibility: Reversibility::Reversible,
            supports_dry_run: false,
            classification_ceiling: None,
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["message_id", "body"],
                "properties": {
                    "message_id": {"type": "string"},
                    "body": {"type": "string"}
                }
            }),
        }
    }

    async fn execute(&self, ctx: &ExecutionContext<'_>) -> CoreResult<CapabilityResult> {
        let message_id: Uuid = ctx.field("message_id")?;
        let body = ctx.text("body")?;

        // Reaching the original is the access check.
        let (original, mailbox_id, _address) =
            mail::repository::accessible_message(ctx.pool, ctx.principal.person_id, message_id)
                .await?
                .ok_or_else(|| CoreError::NotFound("Mensagem não encontrada.".to_owned()))?;

        let subject = original.subject.as_deref().unwrap_or("(sem assunto)");
        let subject = if subject.to_lowercase().starts_with("re:") {
            subject.to_owned()
        } else {
            format!("Re: {subject}")
        };

        let draft_id = mail::repository::create_draft(
            ctx.pool,
            mailbox_id,
            ctx.principal.person_id,
            &subject,
            &body,
            std::slice::from_ref(&original.from_address),
            Some(message_id),
        )
        .await?;

        Ok(CapabilityResult {
            capability: self.descriptor().id,
            status: ExecutionStatus::Succeeded,
            detail: format!(
                "Resposta preparada para {}. Não foi enviada.",
                original.from_address
            ),
            resources: vec![ResourceRef {
                kind: AgenticKind::MailDraft,
                id: draft_id,
                label: Some(subject),
            }],
            reversibility: Reversibility::Reversible,
            output: None,
        })
    }
}

/// Send a prepared draft.
///
/// # The only external effect in the registry
///
/// `RiskLevel::ExternalEffect` and `ApprovalRequirement::Always`, which
/// together mean no configuration and no autonomy level lets this run without a
/// person confirming. Once sent, no ACL of the Ocinye reaches inside somebody
/// else's mailbox — hence `Irreversible`, and hence no Undo is offered
/// (briefing §51, §137).
pub struct SendDraft;

#[async_trait]
impl CapabilityHandler for SendDraft {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new("mail.send"),
            operation: OperationId::new("mail::send_message"),
            domain: "mail".to_owned(),
            summary: "Enviar um rascunho já preparado.".to_owned(),
            permission: Permission::MailSend,
            scope: Scope::Resource,
            risk: RiskLevel::ExternalEffect,
            approval: ApprovalRequirement::Always,
            // Even at the highest autonomy this installation permits, the
            // approval requirement above still applies.
            max_autonomy: AutonomyLevel::Workflow,
            reversibility: Reversibility::Irreversible,
            supports_dry_run: true,
            classification_ceiling: None,
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["draft_id"],
                "properties": {
                    "draft_id": {"type": "string"}
                }
            }),
        }
    }

    async fn execute(&self, ctx: &ExecutionContext<'_>) -> CoreResult<CapabilityResult> {
        let draft_id: Uuid = ctx.field("draft_id")?;

        let draft = mail::repository::accessible_draft(ctx.pool, ctx.principal.person_id, draft_id)
            .await?
            .ok_or_else(|| CoreError::NotFound("Rascunho não encontrado.".to_owned()))?;

        if ctx.dry_run {
            return Ok(CapabilityResult {
                capability: self.descriptor().id,
                status: ExecutionStatus::DryRun,
                detail: format!(
                    "Seria enviada a mensagem «{}» para {} destinatário(s).",
                    draft.subject.as_deref().unwrap_or("(sem assunto)"),
                    draft.to_addresses.len()
                ),
                resources: Vec::new(),
                reversibility: Reversibility::NothingToUndo,
                output: None,
            });
        }

        // Reached only after a person confirmed: the executor's approval gate
        // is ahead of this, and `ApprovalRequirement::Always` means it cannot
        // be satisfied by a previous confirmation of something else.
        //
        // Sending itself is `PLANNED` from here: the agentic path deliberately
        // does not duplicate the send pipeline that `POST /mail/send` owns,
        // because two ways to send is two places for the classification policy
        // to be applied differently.
        Err(CoreError::CapabilityUnavailable(
            "O envio a partir de um plano ainda não está ligado. Abra o rascunho no \
             Correio e envie a partir daí."
                .to_owned(),
        ))
    }
}

/// Search a mailbox the acting person may reach.
///
/// # Why mail search is its own capability
///
/// `knowledge.search` reads the institutional index. Mail is deliberately not
/// in it: personal correspondence never enters a shared index
/// ([ADR-0407](../../../../../docs/adrs/0407-mail-index-not-archive.md)). So
/// finding «o último email do Carlos» needs a capability that searches inside
/// one mailbox, through the ownership filter.
pub struct SearchMail;

#[async_trait]
impl CapabilityHandler for SearchMail {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new("mail.search"),
            operation: OperationId::new("mail::search_messages"),
            domain: "mail".to_owned(),
            summary: "Procurar mensagens numa caixa de correio.".to_owned(),
            permission: Permission::MailUse,
            scope: Scope::Resource,
            risk: RiskLevel::ReadOnly,
            approval: ApprovalRequirement::Never,
            max_autonomy: AutonomyLevel::Workflow,
            reversibility: Reversibility::NothingToUndo,
            supports_dry_run: false,
            classification_ceiling: None,
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["query"],
                "properties": {
                    "query": {"type": "string"},
                    "mailbox_id": {"type": "string", "description": "Opcional; por omissão, a primeira caixa."}
                }
            }),
        }
    }

    async fn execute(&self, ctx: &ExecutionContext<'_>) -> CoreResult<CapabilityResult> {
        let query = ctx.text("query")?;

        // Absent a named mailbox, the caller's own. `mailboxes` already applies
        // the ownership filter, so this reaches nothing it should not.
        let mailbox_id: Uuid = match ctx.optional("mailbox_id")? {
            Some(id) => id,
            None => {
                let boxes = mail::mailboxes(ctx.pool, ctx.principal).await?;
                boxes
                    .first()
                    .ok_or_else(|| {
                        CoreError::NotFound("Não possui nenhuma caixa de correio.".to_owned())
                    })?
                    .id
            }
        };

        let mailbox = mail::mailbox(ctx.pool, ctx.principal, mailbox_id).await?;
        let hits = mail::repository::search_messages(ctx.pool, mailbox.id, &query, 10).await?;

        Ok(CapabilityResult {
            capability: self.descriptor().id,
            status: ExecutionStatus::Succeeded,
            detail: match hits.len() {
                0 => "Nenhuma mensagem corresponde.".to_owned(),
                1 => "1 mensagem encontrada.".to_owned(),
                other => format!("{other} mensagens encontradas."),
            },
            resources: hits
                .iter()
                .map(|hit| ResourceRef {
                    kind: AgenticKind::MailMessage,
                    id: hit.id,
                    label: hit.subject.clone(),
                })
                .collect(),
            reversibility: Reversibility::NothingToUndo,
            output: Some(serde_json::json!({
                "items": hits.iter().map(|hit| serde_json::json!({
                    "id": hit.id,
                    "from": hit.from_address,
                    "subject": hit.subject,
                    "snippet": hit.snippet,
                    "sent_at": hit.sent_at,
                })).collect::<Vec<_>>(),
            })),
        })
    }
}

/// Read one message, sanitised.
///
/// # The body is the most hostile input in the system
///
/// It arrives here already cleaned by the Core, which is the same path the
/// interface uses. An agent never sees raw email HTML, because nothing does
/// ([ADR-0402](../../../../../docs/adrs/0402-mail-html-sanitisation.md)).
///
/// Remote content stays blocked: an agent reading a message must not be the
/// thing that tells a sender it was opened.
pub struct ReadMessage;

#[async_trait]
impl CapabilityHandler for ReadMessage {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new("mail.read"),
            operation: OperationId::new("mail::read_message"),
            domain: "mail".to_owned(),
            summary: "Ler uma mensagem, já higienizada.".to_owned(),
            permission: Permission::MailUse,
            scope: Scope::Resource,
            risk: RiskLevel::ReadOnly,
            approval: ApprovalRequirement::Never,
            max_autonomy: AutonomyLevel::Workflow,
            reversibility: Reversibility::NothingToUndo,
            supports_dry_run: false,
            classification_ceiling: None,
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["message_id"],
                "properties": { "message_id": {"type": "string"} }
            }),
        }
    }

    async fn execute(&self, ctx: &ExecutionContext<'_>) -> CoreResult<CapabilityResult> {
        let message_id: Uuid = ctx.field("message_id")?;

        let (indexed, _mailbox_id, _address) =
            mail::repository::accessible_message(ctx.pool, ctx.principal.person_id, message_id)
                .await?
                .ok_or_else(|| CoreError::NotFound("Mensagem não encontrada.".to_owned()))?;

        // The indexed metadata and excerpt, not the body: fetching the body
        // needs the provider, and this installation has none. The excerpt is
        // what the index keeps, and it is enough to summarise from.
        Ok(CapabilityResult {
            capability: self.descriptor().id,
            status: ExecutionStatus::Succeeded,
            detail: format!(
                "Mensagem de {}: {}",
                indexed.from_address,
                indexed.subject.as_deref().unwrap_or("(sem assunto)")
            ),
            resources: vec![ResourceRef {
                kind: AgenticKind::MailMessage,
                id: indexed.id,
                label: indexed.subject.clone(),
            }],
            reversibility: Reversibility::NothingToUndo,
            output: Some(serde_json::json!({
                "from": indexed.from_address,
                "subject": indexed.subject,
                "snippet": indexed.snippet,
                "sent_at": indexed.sent_at,
                "has_attachments": indexed.has_attachments,
            })),
        })
    }
}

/// Rewrite a draft: shorter, more formal, clearer.
///
/// # Why transforming is a capability and not a second assist endpoint
///
/// «Torna mais curto e mais formal» arrives through the same command surface as
/// everything else, and has to be planned, authorised and audited the same way.
/// A second path to the same effect is a second place for the rules to drift.
pub struct TransformDraft;

#[async_trait]
impl CapabilityHandler for TransformDraft {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new("mail.draft_transform"),
            operation: OperationId::new("mail::draft_transform"),
            domain: "mail".to_owned(),
            summary: "Reescrever um rascunho: mais curto, mais formal, mais claro.".to_owned(),
            permission: Permission::MailAiUse,
            scope: Scope::Resource,
            risk: RiskLevel::LowImpact,
            approval: ApprovalRequirement::Never,
            max_autonomy: AutonomyLevel::Workflow,
            reversibility: Reversibility::Reversible,
            supports_dry_run: false,
            classification_ceiling: None,
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["draft_id", "action"],
                "properties": {
                    "draft_id": {"type": "string"},
                    "action": {
                        "type": "string",
                        "description": "shorter, more_formal, more_cordial, more_direct, clarify, proofread, translate"
                    }
                }
            }),
        }
    }

    async fn execute(&self, ctx: &ExecutionContext<'_>) -> CoreResult<CapabilityResult> {
        let draft_id: Uuid = ctx.field("draft_id")?;
        let action = ctx.text("action")?;

        let action = ocinye_contracts::ComposeAction::parse(&action)
            .ok_or_else(|| CoreError::Validation("Transformação desconhecida.".to_owned()))?;

        let draft = mail::repository::accessible_draft(ctx.pool, ctx.principal.person_id, draft_id)
            .await?
            .ok_or_else(|| CoreError::NotFound("Rascunho não encontrado.".to_owned()))?;

        // The transformation itself needs a model. Without one, this refuses
        // with a reason — and the draft is untouched, which is the point:
        // `LowImpact` and `Reversible` describe what happens on success.
        Err(CoreError::CapabilityUnavailable(format!(
            "A transformação «{}» depende de uma capacidade de IA do Ocinye OS, que \
             não está disponível. O rascunho «{}» não foi alterado.",
            action.label(),
            draft.subject.as_deref().unwrap_or("(sem assunto)")
        )))
    }
}

/// Decide whether a message may leave the institution.
///
/// # A read that answers the question a send would raise
///
/// `ReadOnly`: it consults the classification policy and reports. Nothing
/// leaves. It exists so a plan can show «este envio precisa de confirmação
/// porque contém material INTERNAL» *before* the member confirms anything
/// ([ADR-0403](../../../../../docs/adrs/0403-mail-send-policy.md)).
pub struct EvaluateSend;

#[async_trait]
impl CapabilityHandler for EvaluateSend {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new("mail.evaluate_send"),
            operation: OperationId::new("mail::evaluate_send"),
            domain: "mail".to_owned(),
            summary: "Verificar se uma mensagem pode sair da instituição.".to_owned(),
            permission: Permission::MailSend,
            scope: Scope::Resource,
            risk: RiskLevel::ReadOnly,
            approval: ApprovalRequirement::Never,
            max_autonomy: AutonomyLevel::Workflow,
            reversibility: Reversibility::NothingToUndo,
            supports_dry_run: false,
            classification_ceiling: None,
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["draft_id"],
                "properties": { "draft_id": {"type": "string"} }
            }),
        }
    }

    async fn execute(&self, ctx: &ExecutionContext<'_>) -> CoreResult<CapabilityResult> {
        let draft_id: Uuid = ctx.field("draft_id")?;

        let draft = mail::repository::accessible_draft(ctx.pool, ctx.principal.person_id, draft_id)
            .await?
            .ok_or_else(|| CoreError::NotFound("Rascunho não encontrado.".to_owned()))?;

        let domains = mail::repository::institutional_domains(ctx.pool).await?;
        let recipients: Vec<ocinye_contracts::MailAddress> = draft
            .to_addresses
            .iter()
            .map(|address| ocinye_contracts::MailAddress::new(address, None, &domains))
            .collect();

        // Attachments are `PLANNED`, so nothing classified travels yet. When
        // they arrive, their classifications come from
        // `mail_draft_attachments` and this call changes in one place.
        let decision = SendPolicy::evaluate(&recipients, &[], false);
        let external = recipients.iter().filter(|r| r.is_external()).count();

        Ok(CapabilityResult {
            capability: self.descriptor().id,
            status: ExecutionStatus::Succeeded,
            detail: match &decision {
                SendDecision::Allowed => {
                    format!("Pode ser enviada. {external} destinatário(s) externo(s).")
                }
                SendDecision::NeedsConfirmation { reason, .. }
                | SendDecision::Refused { reason } => reason.clone(),
            },
            resources: vec![ResourceRef {
                kind: AgenticKind::MailDraft,
                id: draft.id,
                label: draft.subject.clone(),
            }],
            reversibility: Reversibility::NothingToUndo,
            output: Some(serde_json::json!({
                "decision": decision.as_str(),
                "external_recipients": external,
                "may_send": decision.is_allowed(),
            })),
        })
    }
}
