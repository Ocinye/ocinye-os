//! Capacidades sobre ficheiros institucionais.
//!
//! # A separação que este ficheiro existe para manter
//!
//! > **A leitura de metadata, a leitura de conteúdo e a execução de acções são
//! > exposições distintas. Autoridade actual é sempre reavaliada no Core.**
//!
//! `knowledge.document.read` continua a devolver `content_included: false`, e
//! não foi «melhorada» com texto. Obter conteúdo é um acto separado, com uma
//! capacidade separada — e essa capacidade não concede autoridade nenhuma: quem
//! não alcança o `File` não alcança o corpo dele, tenha o agente a capacidade
//! que tiver.

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
use crate::modules::files;

/// Quantos excertos, no máximo, saem de uma invocação.
///
/// # Porque há limites, e porque são explícitos
///
/// Porque «o ficheiro inteiro» não é uma resposta a uma pergunta: é uma
/// transferência. Um limite escrito aqui é uma decisão institucional que se
/// pode discutir; um limite implícito no tamanho do contexto de um modelo é
/// uma decisão que ninguém tomou e que muda quando o modelo muda.
pub const MAX_EXCERPTS: usize = 12;

/// O maior excerto, em caracteres.
pub const MAX_EXCERPT_CHARS: usize = 2_000;

/// O total, em caracteres, de tudo o que sai de uma invocação.
pub const MAX_TOTAL_CHARS: usize = 12_000;

/// Ler o conteúdo de um ficheiro institucional.
///
/// # O que esta capacidade **não** devolve
///
/// Não devolve o identificador do objecto guardado, nem a chave dele, nem uma
/// URL do armazenamento, nem credenciais. Um modelo que recebesse qualquer uma
/// dessas coisas teria um caminho para os bytes que não passa pelo `File` — e
/// era exactamente isso que toda esta arquitectura existe para impedir.
///
/// # Versão exacta
///
/// Indicar um `file_version` pede **aquela** versão, e resolve-se através do
/// ficheiro que a contém. Indicar um `file` pede a versão corrente. Uma citação
/// científica feita hoje não pode derivar para `latest` amanhã.
pub struct ReadFileContent;

#[async_trait]
impl CapabilityHandler for ReadFileContent {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new("files.content.read"),
            operation: OperationId::new("files::read_content"),
            domain: "files".to_owned(),
            summary: "Ler o conteúdo extraído de um ficheiro institucional.".to_owned(),
            // O mesmo direito que já governa obter o conteúdo de um documento.
            //
            // `DocumentsView` seria errado: tornaria ler o corpo tão disponível
            // como ler o nome, e a separação entre as duas exposições existe
            // precisamente para isso não acontecer. Um direito novo também
            // seria errado — o modelo já distingue ver de obter, e acrescentar
            // um terceiro nome para a mesma distinção só criava um sítio a mais
            // onde discordar.
            permission: Permission::DocumentsDownload,
            scope: Scope::ResearchWorkspace,
            // Ler é ler. Não se acrescenta confirmação humana a uma leitura só
            // porque quem a pediu foi um modelo: o que a governa é a autoridade
            // sobre o ficheiro e os limites de exposição. Uma operação que
            // **use** este conteúdo para agir — enviar, publicar, validar — tem
            // a sua própria fronteira de confirmação.
            risk: RiskLevel::ReadOnly,
            approval: ApprovalRequirement::Never,
            max_autonomy: AutonomyLevel::Workflow,
            reversibility: Reversibility::NothingToUndo,
            supports_dry_run: false,
            classification_ceiling: None,
            input_schema: serde_json::json!({"type": "object", "properties": {}}),
        }
    }

    async fn execute(&self, ctx: &ExecutionContext<'_>) -> CoreResult<CapabilityResult> {
        // Uma versão exacta, se foi pedida; senão, o ficheiro.
        //
        // As duas intenções resolvem-se pela **mesma** autoridade: o executor já
        // localizou o recurso, e localizar uma versão passa pelo ficheiro que a
        // contém. Não há aqui um segundo caminho.
        let versao_pedida = ctx
            .resources
            .iter()
            .find(|r| r.reference.kind == AgenticKind::FileVersion)
            .map(|r| r.reference.id);

        let mut conn = ctx.pool.acquire().await?;

        let (file_id, version_id, sequence, nome) = if let Some(version_id) = versao_pedida {
            let (versao, ficheiro) =
                files::get_version(&mut conn, ctx.principal, version_id).await?;
            (
                versao.file_id,
                versao.version_id,
                versao.sequence,
                ficheiro.name,
            )
        } else {
            let file_id = ctx.one(AgenticKind::File)?.reference.id;
            let (ficheiro, _) = files::get(&mut conn, ctx.principal, file_id).await?;
            let corrente = files::versions(&mut conn, ctx.principal, file_id)
                .await?
                .into_iter()
                .next();
            let Some(corrente) = corrente else {
                return Ok(CapabilityResult {
                    capability: self.descriptor().id,
                    status: ExecutionStatus::Succeeded,
                    detail: format!("«{}» não tem versões.", ficheiro.name),
                    resources: vec![ResourceRef {
                        kind: AgenticKind::File,
                        id: file_id,
                        label: Some(ficheiro.name.clone()),
                    }],
                    reversibility: Reversibility::NothingToUndo,
                    output: Some(serde_json::json!({
                        "file_id": file_id,
                        "content_included": false,
                        "reason": "O ficheiro não tem versões.",
                    })),
                });
            };
            (file_id, corrente.id, corrente.sequence, ficheiro.name)
        };

        let excertos = files::excerpts(
            &mut conn,
            ctx.principal,
            version_id,
            MAX_EXCERPTS,
            MAX_EXCERPT_CHARS,
        )
        .await?;

        // O tecto total corta onde tiver de cortar, e diz que cortou. Devolver
        // menos sem o dizer faria um modelo concluir que aquilo é o documento
        // todo — e uma resposta construída sobre metade de um relatório, a
        // afirmar-se completa, é pior do que uma resposta que falta.
        let mut saida = Vec::new();
        let mut gastos = 0_usize;
        let mut truncado = excertos.len() > MAX_EXCERPTS;

        for excerto in excertos.into_iter().take(MAX_EXCERPTS) {
            let cabe = MAX_TOTAL_CHARS.saturating_sub(gastos);
            if cabe == 0 {
                truncado = true;
                break;
            }
            let texto: String = excerto.text.chars().take(cabe).collect();
            if texto.chars().count() < excerto.text.chars().count() {
                truncado = true;
            }
            gastos += texto.chars().count();
            saida.push(serde_json::json!({
                "ordinal": excerto.ordinal,
                "text": texto,
                "locator": excerto.locator,
            }));
        }

        let vazio = saida.is_empty();

        Ok(CapabilityResult {
            capability: self.descriptor().id,
            status: ExecutionStatus::Succeeded,
            detail: if vazio {
                format!("«{nome}» não tem conteúdo extraído.")
            } else {
                format!("{} excertos de «{nome}» (v{sequence}).", saida.len())
            },
            resources: vec![
                ResourceRef {
                    kind: AgenticKind::File,
                    id: file_id,
                    label: Some(nome.clone()),
                },
                ResourceRef {
                    kind: AgenticKind::FileVersion,
                    id: version_id,
                    label: Some(format!("{nome} · v{sequence}")),
                },
            ],
            reversibility: Reversibility::NothingToUndo,
            output: Some(serde_json::json!({
                "file_id": file_id,
                "file_version_id": version_id,
                "sequence": sequence,
                "name": nome,
                "excerpts": saida,
                "truncated": truncado,
                // Dito e não deduzido da presença do campo: aqui o conteúdo
                // **está** incluído, e é a diferença desta capacidade para a
                // leitura de metadata.
                "content_included": !vazio,
            })),
        })
    }
}
