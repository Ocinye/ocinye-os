//! Capabilities das Mensagens.
//!
//! # A prova que estas existem para dar
//!
//! Cada uma chama **a mesma função** que o composer do Workspace chama:
//! [`messaging::send`], [`messaging::open_direct`], [`messaging::create_group`],
//! [`messaging::add_member`]. Não há `agent_send_message`, não há endpoint para
//! IA, e não há caminho que salte a autorização (ADR-0307).
//!
//! O plano agentic não fala com o Redis nem com o socket. Passa pela operação do
//! Core, e é ela que decide o que anuncia — depois de persistir.
//!
//! # Quem se nomeia, e quem se resolve
//!
//! O modelo escreve nomes, e não identificadores. Resolver «o Fidel» é procurar
//! no universo que **quem pede** alcança, e recusar quando há mais do que um —
//! escolher por ele seria mandar uma mensagem à pessoa errada com a confiança de
//! quem acertou.

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
use crate::modules::messaging::{self, Outgoing};

/// Uma pessoa da instituição, encontrada pelo nome que o modelo escreveu.
struct Encontrada {
    id: Uuid,
    nome: String,
}

/// Resolve uma pessoa a partir de como alguém lhe chamou.
///
/// # Porque recusa em vez de escolher
///
/// Porque «manda uma mensagem ao Fidel» com dois Fidéis na instituição não tem
/// uma resposta certa. Escolher o primeiro manda a mensagem a uma pessoa que não
/// era, com a aparência de ter corrido bem — e a pessoa certa nunca fica a
/// saber que devia ter recebido nada.
async fn resolver_pessoa(ctx: &ExecutionContext<'_>, procurado: &str) -> CoreResult<Encontrada> {
    let procurado = procurado.trim();
    if procurado.is_empty() {
        return Err(CoreError::Validation("É preciso dizer a quem.".to_owned()));
    }

    let candidatos: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT id, COALESCE(display_name, full_name) AS nome
           FROM people
          WHERE organisation_id = $1
            AND deactivated_at IS NULL
            AND id <> $2
            AND (
                 full_name ILIKE '%' || $3 || '%'
              OR COALESCE(display_name, '') ILIKE '%' || $3 || '%'
              OR username ILIKE '%' || $3 || '%'
              OR email ILIKE '%' || $3 || '%'
            )
          ORDER BY nome
          LIMIT 10",
    )
    .bind(ctx.principal.organisation_id)
    .bind(ctx.principal.person_id)
    .bind(procurado)
    .fetch_all(ctx.pool)
    .await?;

    match candidatos.as_slice() {
        [] => Err(CoreError::NotFound(format!(
            "Não encontrei ninguém chamado «{procurado}» na Ocinye."
        ))),
        [(id, nome)] => Ok(Encontrada {
            id: *id,
            nome: nome.clone(),
        }),
        varios => {
            let nomes: Vec<&str> = varios.iter().map(|(_, n)| n.as_str()).collect();
            Err(CoreError::Validation(format!(
                "Há mais do que uma pessoa que corresponde a «{procurado}»: {}. \
                 Diga qual.",
                nomes.join(", ")
            )))
        }
    }
}

/// Enviar uma mensagem a alguém.
///
/// # Reversível? Não.
///
/// Uma mensagem enviada foi lida ou pode tê-lo sido. Apagá-la não a desfaz na
/// cabeça de quem a leu, e o domínio não representa um desfecho que a anule.
/// Declara-se irreversível, e é isso que faz a confirmação ser pedida.
pub struct SendMessage;

#[async_trait]
impl CapabilityHandler for SendMessage {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new("messaging.message.send"),
            operation: OperationId::new("messaging::send"),
            domain: "messaging".to_owned(),
            summary: "Enviar uma mensagem numa conversa.".to_owned(),
            permission: Permission::MessagingUse,
            scope: Scope::Institution,
            // Uma mensagem não sai da instituição — logo não é
            // `ExternalEffect`. É uma mudança material que chega a outra
            // pessoa, e que ela lê antes de haver forma de a desfazer.
            risk: RiskLevel::MaterialMutation,
            approval: ApprovalRequirement::Always,
            // `Act` e não `Workflow`: mandar mensagens em cadeia sem alguém a
            // olhar é como uma conversa se transforma noutra coisa.
            max_autonomy: AutonomyLevel::Act,
            reversibility: Reversibility::Irreversible,
            supports_dry_run: true,
            classification_ceiling: None,
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["to", "body"],
                "properties": {
                    "to": {
                        "type": "string",
                        "description": "O nome, o utilizador ou o endereço de quem recebe. \
                                        O Core resolve; não inventar identificadores."
                    },
                    "body": {"type": "string", "description": "O texto da mensagem."}
                }
            }),
        }
    }

    async fn execute(&self, ctx: &ExecutionContext<'_>) -> CoreResult<CapabilityResult> {
        let para = ctx.text("to")?;
        let corpo = ctx.text("body")?;
        let pessoa = resolver_pessoa(ctx, &para).await?;

        if ctx.dry_run {
            return Ok(CapabilityResult {
                capability: self.descriptor().id,
                status: ExecutionStatus::DryRun,
                resources: Vec::new(),
                detail: format!("Enviaria uma mensagem a {}.", pessoa.nome),
                reversibility: Reversibility::Irreversible,
                output: None,
            });
        }

        // A mesma operação que o botão «Enviar» usa. Abrir a conversa directa
        // também: sem isto, a primeira mensagem a alguém não teria onde cair.
        let conversa = messaging::open_direct(ctx.pool, ctx.principal, pessoa.id, ctx.ids).await?;
        let id = messaging::send(
            ctx.pool,
            ctx.principal,
            ctx.realtime,
            conversa,
            &Outgoing {
                body: &corpo,
                reply_to: None,
                mentions: &[],
                // Sem chave: cada pedido do plano é um pedido, e o executor já
                // não repete um passo que correu.
                idempotency_key: None,
            },
            ctx.ids,
        )
        .await?;

        Ok(CapabilityResult {
            capability: self.descriptor().id,
            status: ExecutionStatus::Succeeded,
            resources: vec![ResourceRef {
                kind: AgenticKind::Message,
                id,
                label: Some(format!("Mensagem a {}", pessoa.nome)),
            }],
            detail: format!("A mensagem a {} foi enviada.", pessoa.nome),
            reversibility: Reversibility::Irreversible,
            output: Some(serde_json::json!({
                "conversation_id": conversa,
                "message_id": id,
            })),
        })
    }
}

/// Abrir a conversa directa com alguém.
pub struct OpenDirect;

#[async_trait]
impl CapabilityHandler for OpenDirect {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new("messaging.direct.open"),
            operation: OperationId::new("messaging::open_direct"),
            domain: "messaging".to_owned(),
            summary: "Abrir a conversa directa com alguém.".to_owned(),
            permission: Permission::MessagingUse,
            scope: Scope::Institution,
            // Abrir uma conversa vazia não diz nada a ninguém: não notifica, não
            // aparece com conteúdo, e fecha-se sem rasto.
            risk: RiskLevel::LowImpact,
            approval: ApprovalRequirement::Never,
            max_autonomy: AutonomyLevel::Workflow,
            reversibility: Reversibility::Reversible,
            supports_dry_run: true,
            classification_ceiling: None,
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["with"],
                "properties": {
                    "with": {"type": "string", "description": "Com quem."}
                }
            }),
        }
    }

    async fn execute(&self, ctx: &ExecutionContext<'_>) -> CoreResult<CapabilityResult> {
        let com = ctx.text("with")?;
        let pessoa = resolver_pessoa(ctx, &com).await?;

        if ctx.dry_run {
            return Ok(CapabilityResult {
                capability: self.descriptor().id,
                status: ExecutionStatus::DryRun,
                resources: Vec::new(),
                detail: format!("Abriria a conversa com {}.", pessoa.nome),
                reversibility: Reversibility::Reversible,
                output: None,
            });
        }

        let id = messaging::open_direct(ctx.pool, ctx.principal, pessoa.id, ctx.ids).await?;

        Ok(CapabilityResult {
            capability: self.descriptor().id,
            status: ExecutionStatus::Succeeded,
            resources: vec![ResourceRef {
                kind: AgenticKind::Conversation,
                id,
                label: Some(pessoa.nome.clone()),
            }],
            detail: format!("A conversa com {} está aberta.", pessoa.nome),
            reversibility: Reversibility::Reversible,
            output: Some(serde_json::json!({ "conversation_id": id })),
        })
    }
}

/// Criar um grupo com pessoas nomeadas.
pub struct CreateGroup;

#[async_trait]
impl CapabilityHandler for CreateGroup {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new("messaging.group.create"),
            operation: OperationId::new("messaging::create_group"),
            domain: "messaging".to_owned(),
            summary: "Criar um grupo de conversa.".to_owned(),
            permission: Permission::MessagingUse,
            scope: Scope::Institution,
            // Um grupo criado aparece a toda a gente que lá foi posta. Não é
            // uma mensagem, mas também não passa despercebido.
            risk: RiskLevel::MaterialMutation,
            approval: ApprovalRequirement::Always,
            max_autonomy: AutonomyLevel::Act,
            reversibility: Reversibility::Reversible,
            supports_dry_run: true,
            classification_ceiling: None,
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["name", "with"],
                "properties": {
                    "name": {"type": "string", "description": "O nome do grupo."},
                    "with": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Os nomes de quem pertence. O Core resolve cada um."
                    }
                }
            }),
        }
    }

    async fn execute(&self, ctx: &ExecutionContext<'_>) -> CoreResult<CapabilityResult> {
        let nome = ctx.text("name")?;
        let nomes: Vec<String> = ctx.optional("with")?.unwrap_or_default();

        // Cada pessoa resolvida uma a uma, e uma ambiguidade recusa o plano
        // inteiro. Criar o grupo com quatro das cinco pessoas certas é pior do
        // que não o criar: ninguém repara em quem falta.
        let mut membros = Vec::with_capacity(nomes.len());
        let mut etiquetas = Vec::with_capacity(nomes.len());
        for candidato in &nomes {
            let pessoa = resolver_pessoa(ctx, candidato).await?;
            membros.push(pessoa.id);
            etiquetas.push(pessoa.nome);
        }

        if ctx.dry_run {
            return Ok(CapabilityResult {
                capability: self.descriptor().id,
                status: ExecutionStatus::DryRun,
                resources: Vec::new(),
                detail: format!("Criaria «{nome}» com {}.", etiquetas.join(", ")),
                reversibility: Reversibility::Reversible,
                output: None,
            });
        }

        let id = messaging::create_group(ctx.pool, ctx.principal, &nome, &membros, ctx.ids).await?;

        Ok(CapabilityResult {
            capability: self.descriptor().id,
            status: ExecutionStatus::Succeeded,
            resources: vec![ResourceRef {
                kind: AgenticKind::Conversation,
                id,
                label: Some(nome.clone()),
            }],
            detail: format!("«{nome}» ficou criado com {}.", etiquetas.join(", ")),
            reversibility: Reversibility::Reversible,
            output: Some(serde_json::json!({ "conversation_id": id })),
        })
    }
}

/// Acrescentar alguém a um grupo.
pub struct AddMember;

#[async_trait]
impl CapabilityHandler for AddMember {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new("messaging.group.add_member"),
            operation: OperationId::new("messaging::add_member"),
            domain: "messaging".to_owned(),
            summary: "Acrescentar alguém a um grupo.".to_owned(),
            permission: Permission::MessagingUse,
            scope: Scope::Resource,
            risk: RiskLevel::MaterialMutation,
            approval: ApprovalRequirement::Always,
            max_autonomy: AutonomyLevel::Act,
            reversibility: Reversibility::Reversible,
            supports_dry_run: true,
            classification_ceiling: None,
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["group", "who"],
                "properties": {
                    "group": {"type": "string", "description": "O nome do grupo."},
                    "who": {"type": "string", "description": "Quem acrescentar."}
                }
            }),
        }
    }

    async fn execute(&self, ctx: &ExecutionContext<'_>) -> CoreResult<CapabilityResult> {
        let grupo = ctx.text("group")?;
        let quem = ctx.text("who")?;
        let pessoa = resolver_pessoa(ctx, &quem).await?;

        // O grupo resolve-se no universo de quem pede, e não por identificador.
        // Um grupo a que ele não pertence não é encontrado, e a resposta é a
        // mesma que para um que não existe.
        let candidatos: Vec<(Uuid, String)> = sqlx::query_as(
            "SELECT c.id, c.name
               FROM conversations c
               JOIN conversation_participants p
                      ON p.conversation_id = c.id
                     AND p.person_id = $1
                     AND p.left_at IS NULL
              WHERE c.kind = 'group' AND c.name ILIKE '%' || $2 || '%'
              ORDER BY c.updated_at DESC
              LIMIT 10",
        )
        .bind(ctx.principal.person_id)
        .bind(&grupo)
        .fetch_all(ctx.pool)
        .await?;

        let (conversa, nome_do_grupo) = match candidatos.as_slice() {
            [] => {
                return Err(CoreError::NotFound(format!(
                    "Não encontrei nenhum grupo chamado «{grupo}»."
                )))
            }
            [(id, nome)] => (*id, nome.clone()),
            varios => {
                let nomes: Vec<&str> = varios.iter().map(|(_, n)| n.as_str()).collect();
                return Err(CoreError::Validation(format!(
                    "Há mais do que um grupo que corresponde a «{grupo}»: {}. Diga qual.",
                    nomes.join(", ")
                )));
            }
        };

        if ctx.dry_run {
            return Ok(CapabilityResult {
                capability: self.descriptor().id,
                status: ExecutionStatus::DryRun,
                resources: Vec::new(),
                detail: format!("Acrescentaria {} a «{nome_do_grupo}».", pessoa.nome),
                reversibility: Reversibility::Reversible,
                output: None,
            });
        }

        messaging::add_member(
            ctx.pool,
            ctx.principal,
            ctx.realtime,
            conversa,
            pessoa.id,
            ctx.ids,
        )
        .await?;

        Ok(CapabilityResult {
            capability: self.descriptor().id,
            status: ExecutionStatus::Succeeded,
            resources: vec![ResourceRef {
                kind: AgenticKind::Conversation,
                id: conversa,
                label: Some(nome_do_grupo.clone()),
            }],
            detail: format!("{} passou a pertencer a «{nome_do_grupo}».", pessoa.nome),
            reversibility: Reversibility::Reversible,
            output: Some(serde_json::json!({ "conversation_id": conversa })),
        })
    }
}
