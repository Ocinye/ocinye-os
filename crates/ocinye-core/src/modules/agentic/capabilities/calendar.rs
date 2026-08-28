//! Capabilities do Calendário.
//!
//! # Nada de novo por baixo
//!
//! Cada uma destas chama exactamente a operação do Core que a interface chama.
//! Não há endpoint para IA, não há caminho paralelo, não há política própria: o
//! agente é outra forma de entrar no mesmo sítio (ADR-0307).

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use ocinye_contracts::agentic::{
    ApprovalRequirement, AutonomyLevel, CapabilityDescriptor, CapabilityId, CapabilityResult,
    ExecutionStatus, OperationId, ResourceKind as AgenticKind, ResourceRef, Reversibility,
    RiskLevel,
};
use ocinye_contracts::calendar::EventScope;
use ocinye_contracts::temporal::{resolve_local, Occurrence, TimeZoneName};
use ocinye_contracts::{Classification, Permission, Scope};

use crate::error::{CoreError, CoreResult};
use crate::modules::agentic::executor::ExecutionContext;
use crate::modules::agentic::registry::CapabilityHandler;
use crate::modules::calendar::{self, EventEdit, NewEvent, NewReminder};

/// Lê uma ocorrência a partir do que o modelo propôs.
///
/// # O modelo interpreta; o Core converte
///
/// A hora local e a zona chegam separadas, e é aqui que se juntam — pela mesma
/// função que a interface usa. Um modelo que enviasse um instante já convertido
/// estaria a decidir o que significa «14:00 em Paris», e a resposta mudaria com
/// quem perguntasse.
///
/// Uma hora que não existe na zona indicada é recusada com a frase certa, e não
/// escolhida em silêncio.
fn occurrence_from(ctx: &ExecutionContext<'_>) -> CoreResult<Occurrence> {
    if ctx.optional::<bool>("all_day")?.unwrap_or(false) {
        let inicio = ctx.text("starts_on")?;
        let inicio = chrono::NaiveDate::parse_from_str(&inicio, "%Y-%m-%d")
            .map_err(|_| CoreError::Validation("A data de início não é válida.".to_owned()))?;
        // O modelo dá o último dia como uma pessoa o diria: inclusivo.
        let fim = match ctx.optional::<String>("ends_on")? {
            Some(valor) => chrono::NaiveDate::parse_from_str(&valor, "%Y-%m-%d")
                .map_err(|_| CoreError::Validation("A data de fim não é válida.".to_owned()))?,
            None => inicio,
        };
        return Ok(Occurrence::AllDay {
            starts_on: inicio,
            ends_before: fim
                .succ_opt()
                .ok_or_else(|| CoreError::Validation("A data de fim não é válida.".to_owned()))?,
        });
    }

    let zona = TimeZoneName::parse(&ctx.text("timezone")?).map_err(CoreError::Validation)?;
    let ler = |campo: &str| -> CoreResult<chrono::NaiveDateTime> {
        let bruto = ctx.text(campo)?;
        chrono::NaiveDateTime::parse_from_str(&bruto, "%Y-%m-%dT%H:%M:%S")
            .or_else(|_| chrono::NaiveDateTime::parse_from_str(&bruto, "%Y-%m-%dT%H:%M"))
            .map_err(|_| CoreError::Validation(format!("«{bruto}» não é uma data e hora.")))
    };

    let inicio = resolve_local(ler("starts_at")?, zona)
        .map_err(|erro| CoreError::Validation(erro.to_string()))?;
    let fim = resolve_local(ler("ends_at")?, zona)
        .map_err(|erro| CoreError::Validation(erro.to_string()))?;

    Ok(Occurrence::Timed {
        starts_at: inicio,
        ends_at: fim,
        timezone: zona,
    })
}

/// Marcar um compromisso.
///
/// # Reversível de propósito
///
/// Um evento marcado a mais cancela-se, e cancelar é um desfecho que o domínio
/// representa. O pior caso é um compromisso a mais na agenda de quem o pediu —
/// não uma perda.
pub struct CreateEvent;

#[async_trait]
impl CapabilityHandler for CreateEvent {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new("calendar.event.create"),
            operation: OperationId::new("calendar::create_event"),
            domain: "calendar".to_owned(),
            summary: "Marcar um compromisso.".to_owned(),
            permission: Permission::CalendarCreate,
            scope: Scope::Institution,
            risk: RiskLevel::LowImpact,
            approval: ApprovalRequirement::Never,
            max_autonomy: AutonomyLevel::Workflow,
            reversibility: Reversibility::Reversible,
            supports_dry_run: true,
            classification_ceiling: None,
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["title", "timezone", "starts_at", "ends_at"],
                "properties": {
                    "title": {"type": "string"},
                    "description": {"type": "string"},
                    "location": {"type": "string"},
                    "timezone": {
                        "type": "string",
                        "description": "Zona IANA, por exemplo Europe/Lisbon. \
                                        O Core converte; não enviar um instante já convertido."
                    },
                    "starts_at": {"type": "string", "description": "Hora local: AAAA-MM-DDTHH:MM."},
                    "ends_at": {"type": "string", "description": "Hora local: AAAA-MM-DDTHH:MM."},
                    "all_day": {"type": "boolean"},
                    "starts_on": {"type": "string", "description": "AAAA-MM-DD, para dia inteiro."},
                    "ends_on": {"type": "string", "description": "Último dia, inclusive."},
                    "scope": {
                        "type": "string",
                        "description": "personal, unit, research_workspace ou institution. \
                                        Por omissão, pessoal."
                    }
                }
            }),
        }
    }

    async fn execute(&self, ctx: &ExecutionContext<'_>) -> CoreResult<CapabilityResult> {
        let title = ctx.text("title")?;
        let occurrence = occurrence_from(ctx)?;

        // O âmbito por omissão é o pessoal. Um modelo que não diga a quem o
        // evento pertence não deve estar a marcar na agenda de uma unidade.
        let scope = ctx
            .optional::<String>("scope")?
            .and_then(|valor| EventScope::parse(&valor))
            .unwrap_or(EventScope::Personal);

        // O contentor vem dos recursos resolvidos, e não da entrada: um
        // identificador que o modelo escreveu é uma alegação até o executor o
        // resolver contra o que o actor alcança (ADR-0306).
        let unit_id = ctx.one(AgenticKind::Unit).ok().map(|r| r.reference.id);
        let workspace_id = ctx.one(AgenticKind::Workspace).ok().map(|r| r.reference.id);

        if ctx.dry_run {
            return Ok(CapabilityResult {
                capability: self.descriptor().id,
                status: ExecutionStatus::DryRun,
                resources: Vec::new(),
                detail: format!("Marcaria «{title}»."),
                reversibility: Reversibility::Reversible,
                output: None,
            });
        }

        let mut tx = ctx.pool.begin().await?;
        let evento = calendar::create_event(
            &mut tx,
            ctx.principal,
            ctx.ids,
            NewEvent {
                scope,
                unit_id,
                workspace_id,
                title: title.clone(),
                description: ctx.optional("description")?,
                location: ctx.optional("location")?,
                occurrence,
                classification: ctx
                    .optional::<String>("classification")?
                    .and_then(|raw| Classification::parse(&raw)),
                // O plano agentic não adiciona participantes.
                //
                // Não é limitação técnica: é fronteira. Associar alguém a uma
                // actividade é um efeito sobre a agenda de outra pessoa, e a
                // capability é `ReadOnly` sobre um pedido de quem a invoca. Se um
                // dia for para existir, é uma capability própria, com o seu risco
                // e a sua aprovação declarados.
                participants: Vec::new(),
            },
        )
        .await?;
        tx.commit().await?;

        Ok(CapabilityResult {
            capability: self.descriptor().id,
            status: ExecutionStatus::Succeeded,
            resources: vec![ResourceRef {
                kind: AgenticKind::CalendarEvent,
                id: evento.id,
                label: Some(evento.title.clone()),
            }],
            detail: format!("«{title}» ficou marcado."),
            reversibility: Reversibility::Reversible,
            output: Some(serde_json::json!({ "event_id": evento.id })),
        })
    }
}

/// Alterar um compromisso.
pub struct UpdateEvent;

#[async_trait]
impl CapabilityHandler for UpdateEvent {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new("calendar.event.update"),
            operation: OperationId::new("calendar::update_event"),
            domain: "calendar".to_owned(),
            summary: "Alterar um compromisso já marcado.".to_owned(),
            permission: Permission::CalendarEdit,
            scope: Scope::Resource,
            risk: RiskLevel::LowImpact,
            approval: ApprovalRequirement::Never,
            max_autonomy: AutonomyLevel::Workflow,
            reversibility: Reversibility::Reversible,
            supports_dry_run: true,
            classification_ceiling: None,
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "title": {"type": "string"},
                    "description": {"type": "string"},
                    "location": {"type": "string"},
                    "timezone": {"type": "string"},
                    "starts_at": {"type": "string"},
                    "ends_at": {"type": "string"}
                }
            }),
        }
    }

    async fn execute(&self, ctx: &ExecutionContext<'_>) -> CoreResult<CapabilityResult> {
        let event_id = ctx.one(AgenticKind::CalendarEvent)?.reference.id;

        // A ocorrência só muda se o pedido a trouxer inteira. Metade de uma hora
        // nova seria uma reunião a uma hora que ninguém escolheu.
        let occurrence = if ctx.optional::<String>("starts_at")?.is_some() {
            Some(occurrence_from(ctx)?)
        } else {
            None
        };

        if ctx.dry_run {
            return Ok(CapabilityResult {
                capability: self.descriptor().id,
                status: ExecutionStatus::DryRun,
                resources: Vec::new(),
                detail: "Alteraria o compromisso.".to_owned(),
                reversibility: Reversibility::Reversible,
                output: None,
            });
        }

        let mut tx = ctx.pool.begin().await?;
        let evento = calendar::update_event(
            &mut tx,
            ctx.principal,
            ctx.ids,
            event_id,
            EventEdit {
                title: ctx.optional("title")?,
                description: ctx.optional::<String>("description")?.map(Some),
                location: ctx.optional::<String>("location")?.map(Some),
                occurrence,
            },
        )
        .await?;
        tx.commit().await?;

        Ok(CapabilityResult {
            capability: self.descriptor().id,
            status: ExecutionStatus::Succeeded,
            resources: vec![ResourceRef {
                kind: AgenticKind::CalendarEvent,
                id: evento.id,
                label: Some(evento.title.clone()),
            }],
            detail: format!("«{}» foi alterado.", evento.title),
            reversibility: Reversibility::Reversible,
            output: None,
        })
    }
}

/// Cancelar um compromisso.
///
/// # Porque isto pede confirmação
///
/// Porque cancelar tem efeito sobre quem esperava a reunião, e não só sobre quem
/// a cancela. Marcar a mais é um incómodo; cancelar sem querer faz alguém não
/// aparecer.
pub struct CancelEvent;

#[async_trait]
impl CapabilityHandler for CancelEvent {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new("calendar.event.cancel"),
            operation: OperationId::new("calendar::cancel_event"),
            domain: "calendar".to_owned(),
            summary: "Cancelar um compromisso.".to_owned(),
            permission: Permission::CalendarEdit,
            scope: Scope::Resource,
            risk: RiskLevel::MaterialMutation,
            approval: ApprovalRequirement::Always,
            max_autonomy: AutonomyLevel::Act,
            reversibility: Reversibility::Reversible,
            supports_dry_run: true,
            classification_ceiling: None,
            input_schema: serde_json::json!({ "type": "object", "properties": {} }),
        }
    }

    async fn execute(&self, ctx: &ExecutionContext<'_>) -> CoreResult<CapabilityResult> {
        let event_id = ctx.one(AgenticKind::CalendarEvent)?.reference.id;

        if ctx.dry_run {
            return Ok(CapabilityResult {
                capability: self.descriptor().id,
                status: ExecutionStatus::DryRun,
                resources: Vec::new(),
                detail: "Cancelaria o compromisso.".to_owned(),
                reversibility: Reversibility::Reversible,
                output: None,
            });
        }

        let mut tx = ctx.pool.begin().await?;
        let evento = calendar::cancel_event(&mut tx, ctx.principal, ctx.ids, event_id).await?;
        tx.commit().await?;

        Ok(CapabilityResult {
            capability: self.descriptor().id,
            status: ExecutionStatus::Succeeded,
            resources: vec![ResourceRef {
                kind: AgenticKind::CalendarEvent,
                id: evento.id,
                label: Some(evento.title.clone()),
            }],
            detail: format!("«{}» foi cancelado.", evento.title),
            reversibility: Reversibility::Reversible,
            output: None,
        })
    }
}

/// Pedir um lembrete.
///
/// # Não é um evento
///
/// «Lembra-me sexta às nove de rever o relatório» não é um compromisso: é uma
/// intenção de ser avisado. Criar um evento para isso encheria a agenda de coisas
/// que ninguém marcou.
pub struct CreateReminder;

#[async_trait]
impl CapabilityHandler for CreateReminder {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new("calendar.reminder.create"),
            operation: OperationId::new("calendar::create_reminder"),
            domain: "calendar".to_owned(),
            summary: "Pedir para ser lembrado de alguma coisa.".to_owned(),
            permission: Permission::CalendarCreate,
            scope: Scope::Institution,
            risk: RiskLevel::LowImpact,
            approval: ApprovalRequirement::Never,
            max_autonomy: AutonomyLevel::Workflow,
            reversibility: Reversibility::Reversible,
            supports_dry_run: true,
            classification_ceiling: None,
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["trigger_at"],
                "properties": {
                    "note": {"type": "string"},
                    "trigger_at": {
                        "type": "string",
                        "description": "Instante em UTC, AAAA-MM-DDTHH:MM:SSZ."
                    }
                }
            }),
        }
    }

    async fn execute(&self, ctx: &ExecutionContext<'_>) -> CoreResult<CapabilityResult> {
        let bruto = ctx.text("trigger_at")?;
        let trigger_at: DateTime<Utc> = bruto
            .parse::<DateTime<Utc>>()
            .map_err(|_| CoreError::Validation(format!("«{bruto}» não é um instante.")))?;

        // O recurso, quando o modelo nomeou um. Resolvido, e não confiado.
        let event_id = ctx
            .one(AgenticKind::CalendarEvent)
            .ok()
            .map(|r| r.reference.id);
        let task_id = ctx.one(AgenticKind::Task).ok().map(|r| r.reference.id);

        if ctx.dry_run {
            return Ok(CapabilityResult {
                capability: self.descriptor().id,
                status: ExecutionStatus::DryRun,
                resources: Vec::new(),
                detail: "Criaria o lembrete.".to_owned(),
                reversibility: Reversibility::Reversible,
                output: None,
            });
        }

        let mut tx = ctx.pool.begin().await?;
        let lembrete = calendar::create_reminder(
            &mut tx,
            ctx.principal,
            ctx.ids,
            NewReminder {
                event_id,
                task_id,
                note: ctx.optional("note")?,
                trigger_at,
            },
        )
        .await?;
        tx.commit().await?;

        Ok(CapabilityResult {
            capability: self.descriptor().id,
            status: ExecutionStatus::Succeeded,
            resources: vec![ResourceRef {
                kind: AgenticKind::Reminder,
                id: lembrete.id,
                label: lembrete.note.clone(),
            }],
            detail: "O lembrete ficou agendado.".to_owned(),
            reversibility: Reversibility::Reversible,
            output: Some(serde_json::json!({ "reminder_id": lembrete.id })),
        })
    }
}
