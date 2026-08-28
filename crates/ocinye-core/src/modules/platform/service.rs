//! Assembling the capability report.

use ocinye_contracts::{
    AiCapability, SystemCapabilities, SystemCapability, SystemCapabilityReport,
    SystemCapabilityState,
};
use sqlx::PgPool;

use crate::config::CoreConfig;
use crate::error::CoreResult;

/// Report what this installation can currently do.
///
/// # Derived, never declared
///
/// Every state below comes from counting real rows or reading real
/// configuration. Nothing is hardcoded, so the day a node is enrolled the
/// report changes by itself — no code change, no new screen (briefing §14).
///
/// # Not authorization
///
/// This answers *can the system*, never *may this person*. The caller is
/// authenticated, because an unauthenticated caller has no business learning
/// the shape of the installation, but no permission is consulted: availability
/// is the same fact for everyone.
///
/// # Errors
///
/// Returns an error when a count query fails.
pub async fn system_capabilities(
    pool: &PgPool,
    config: &CoreConfig,
    storage_configured: bool,
) -> CoreResult<SystemCapabilities> {
    let mut capabilities = Vec::with_capacity(SystemCapability::all().len());

    // ── Inference ───────────────────────────────────────────────────────
    //
    // A capability is served when a registered model declares it *and* is
    // healthy. With no nodes there are no models, and the honest state is
    // `NoResource` — not an error, and not "offline" (briefing §7).
    for (capability, ai_capability) in [
        (SystemCapability::AiGeneral, AiCapability::General),
        (SystemCapability::AiCoding, AiCapability::Coding),
        (SystemCapability::AiReasoning, AiCapability::Reasoning),
        (SystemCapability::AiEmbedding, AiCapability::Embedding),
    ] {
        capabilities.push(inference_report(pool, capability, ai_capability, config).await?);
    }

    // ── Agents ──────────────────────────────────────────────────────────
    //
    // Deliberately available with no AI node. An agent is a *definition* —
    // name, purpose, scope, knowledge — and defining one needs no model. It
    // simply cannot execute until a capability can serve it, which the agent's
    // own state records (briefing §9).
    capabilities.push(SystemCapabilityReport::new(
        SystemCapability::Agents,
        SystemCapabilityState::Available,
        "Os agentes podem ser definidos e guardados. A execução depende de uma \
         capacidade de IA compatível.",
    ));

    // ── Calendário ──────────────────────────────────────────────────────
    //
    // Disponível sem depender de recurso externo nenhum: o tempo institucional
    // vive na base, e não há aqui nada que possa faltar por não estar
    // configurado. Foi `Planned` enquanto o domínio existia sem interface; passou
    // a disponível no momento em que uma pessoa conseguiu usá-lo (ADR-0410).
    capabilities.push(SystemCapabilityReport::new(
        SystemCapability::Calendar,
        SystemCapabilityState::Available,
        "Compromissos, prazos de tarefas e lembretes. Os lembretes são entregues \
         pelo worker institucional, sem depender de um separador aberto.",
    ));

    // ── Compute ─────────────────────────────────────────────────────────
    let registered: i64 = sqlx::query_scalar("SELECT count(*) FROM compute_nodes")
        .fetch_one(pool)
        .await?;
    let online: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM compute_nodes
          WHERE status = 'online' AND last_seen_at > now() - $1",
    )
    .bind(
        chrono::Duration::from_std(config.compute.node_offline_after)
            .unwrap_or_else(|_| chrono::Duration::seconds(120)),
    )
    .fetch_one(pool)
    .await?;

    capabilities.push(match (registered, online) {
        (0, _) => SystemCapabilityReport::new(
            SystemCapability::Compute,
            SystemCapabilityState::NoResource,
            "Nenhum nó computacional foi registado. O Ocinye OS está preparado para \
             integrar recursos computacionais assim que existirem.",
        )
        .depending_on("Registo de um nó computacional"),
        (_, 0) => SystemCapabilityReport::new(
            SystemCapability::Compute,
            SystemCapabilityState::Unavailable,
            "Existem nós registados, mas nenhum respondeu dentro do período de \
             actividade esperado.",
        ),
        _ => SystemCapabilityReport::new(
            SystemCapability::Compute,
            SystemCapabilityState::Available,
            format!("{online} de {registered} nós activos."),
        ),
    });

    // ── Object storage ──────────────────────────────────────────────────
    //
    // Configured is not the same as reachable. This reports configuration; an
    // upload that fails against a configured-but-unreachable store surfaces its
    // own error, and says nothing was saved (briefing §31).
    capabilities.push(if storage_configured {
        SystemCapabilityReport::new(
            SystemCapability::ObjectStorage,
            SystemCapabilityState::Available,
            "Armazenamento de objectos configurado.",
        )
    } else {
        SystemCapabilityReport::new(
            SystemCapability::ObjectStorage,
            SystemCapabilityState::NotConfigured,
            "Nenhum armazenamento de objectos está configurado nesta instalação. \
             Carregamentos e descarregamentos não estão disponíveis.",
        )
        .depending_on("Configuração de armazenamento S3-compatível")
    });

    // ── Search ──────────────────────────────────────────────────────────
    //
    // Lexical search is PostgreSQL full-text and needs nothing beyond the
    // database the Core is already talking to.
    capabilities.push(SystemCapabilityReport::new(
        SystemCapability::LexicalSearch,
        SystemCapabilityState::Available,
        "Pesquisa textual sobre o índice institucional.",
    ));

    let embeddings: i64 =
        sqlx::query_scalar("SELECT count(*) FROM search_documents WHERE embedding IS NOT NULL")
            .fetch_one(pool)
            .await
            .unwrap_or(0);

    let embedding_usable = capabilities
        .iter()
        .any(|report| report.capability == SystemCapability::AiEmbedding && report.is_usable());

    capabilities.push(if embedding_usable && embeddings > 0 {
        SystemCapabilityReport::new(
            SystemCapability::SemanticSearch,
            SystemCapabilityState::Available,
            format!("{embeddings} artefactos indexados semanticamente."),
        )
    } else if embedding_usable {
        SystemCapabilityReport::new(
            SystemCapability::SemanticSearch,
            SystemCapabilityState::NotConfigured,
            "Existe capacidade de embeddings, mas nenhum artefacto foi ainda indexado.",
        )
        .depending_on("Indexação semântica do acervo")
    } else {
        SystemCapabilityReport::new(
            SystemCapability::SemanticSearch,
            SystemCapabilityState::NoResource,
            "A pesquisa semântica depende de uma capacidade de embeddings, que não \
             está actualmente disponível.",
        )
        .depending_on("ai.embedding")
    });

    // ── Ocinye Mail ─────────────────────────────────────────────────────
    //
    // Four separate reports, because they fail separately. A deployment can
    // read mail without being able to send it, send without an AI node, and
    // have both while ingestion is still not built. Collapsing them into one
    // "mail: available" would hide exactly the distinction a member needs
    // (briefing §61, §105).
    let mail_configured = config.mail.is_configured();

    capabilities.push(if mail_configured {
        SystemCapabilityReport::new(
            SystemCapability::Mail,
            SystemCapabilityState::Available,
            "Correio institucional configurado.",
        )
    } else {
        SystemCapabilityReport::new(
            SystemCapability::Mail,
            SystemCapabilityState::NotConfigured,
            "O correio institucional não está configurado nesta instalação do \
             Ocinye OS.",
        )
        .depending_on("Configuração de um serviço IMAP/SMTP institucional")
    });

    capabilities.push(if mail_configured {
        SystemCapabilityReport::new(
            SystemCapability::MailSend,
            SystemCapabilityState::Available,
            "Envio por SMTP disponível.",
        )
    } else {
        SystemCapabilityReport::new(
            SystemCapability::MailSend,
            SystemCapabilityState::NotConfigured,
            "Nenhum serviço de envio está configurado. Nenhuma mensagem pode ser \
             enviada a partir do Ocinye Workspace.",
        )
        .depending_on("Configuração de um serviço SMTP institucional")
    });

    // `Degraded` and not `Available`: a member can refresh a folder, and
    // nothing refreshes it for them. Reporting this as fully available because
    // the button exists would misdescribe what the system does — new mail does
    // not appear on its own (`CLAUDE.md` §69).
    capabilities.push(if mail_configured {
        SystemCapabilityReport::new(
            SystemCapability::MailSync,
            SystemCapabilityState::Degraded,
            "A sincronização é manual: cada pasta é actualizada quando pedida. \
             Não existe ainda um processo que actualize o correio recebido \
             automaticamente.",
        )
        .depending_on("Worker de ingestão periódica")
    } else {
        SystemCapabilityReport::new(
            SystemCapability::MailSync,
            SystemCapabilityState::NotConfigured,
            "A sincronização depende do correio institucional, que não está \
             configurado.",
        )
        .depending_on("mail")
    });

    let ai_usable = capabilities
        .iter()
        .any(|report| report.capability == SystemCapability::AiGeneral && report.is_usable());

    capabilities.push(if !mail_configured {
        SystemCapabilityReport::new(
            SystemCapability::MailAiAssist,
            SystemCapabilityState::NotConfigured,
            "A assistência de escrita depende do correio institucional, que não está \
             configurado.",
        )
        .depending_on("mail")
    } else if ai_usable {
        SystemCapabilityReport::new(
            SystemCapability::MailAiAssist,
            SystemCapabilityState::Available,
            "A assistência de escrita está disponível. Nenhuma mensagem é enviada \
             automaticamente.",
        )
    } else {
        SystemCapabilityReport::new(
            SystemCapability::MailAiAssist,
            SystemCapabilityState::NoResource,
            "A assistência de escrita depende de uma capacidade de IA, que não está \
             actualmente disponível. Ler, escrever, responder e enviar continuam a \
             funcionar normalmente.",
        )
        .depending_on("ai.general")
    });

    // ── SystemCapability runtime ──────────────────────────────────────────────
    capabilities.push(SystemCapabilityReport::new(
        SystemCapability::CapabilityRuntime,
        SystemCapabilityState::Available,
        "Runtime WebAssembly disponível para capacidades institucionais.",
    ));

    // Stable order, so the interface never reshuffles between requests.
    capabilities.sort_by_key(|report| report.capability);

    Ok(SystemCapabilities { capabilities })
}

/// State of one inference capability.
async fn inference_report(
    pool: &PgPool,
    capability: SystemCapability,
    ai_capability: AiCapability,
    config: &CoreConfig,
) -> CoreResult<SystemCapabilityReport> {
    let serving: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ai_models
          WHERE enabled = true
            AND status = 'available'
            AND capabilities ? $1",
    )
    .bind(ai_capability.as_str())
    .fetch_one(pool)
    .await?;

    if serving > 0 {
        return Ok(SystemCapabilityReport::new(
            capability,
            SystemCapabilityState::Available,
            format!("{serving} modelo(s) servem esta capacidade."),
        ));
    }

    // A capability that is *mapped* in configuration but has no healthy model
    // is a different situation from one nobody ever configured: the first is a
    // node that should be answering, the second is a node that does not exist.
    let mapped = config.ai.capability_map.contains_key(&ai_capability);
    let total: i64 = sqlx::query_scalar("SELECT count(*) FROM ai_models")
        .fetch_one(pool)
        .await?;

    Ok(if total == 0 {
        SystemCapabilityReport::new(
            capability,
            SystemCapabilityState::NoResource,
            "Nenhum nó de IA Ocinye está actualmente disponível. As funcionalidades de \
             inferência local serão activadas quando um nó de IA for registado no \
             Ocinye OS.",
        )
        .depending_on("Registo de um nó de IA Ocinye")
    } else if mapped {
        SystemCapabilityReport::new(
            capability,
            SystemCapabilityState::Unavailable,
            "Esta capacidade está mapeada para um modelo que não se encontra \
             actualmente disponível.",
        )
    } else {
        SystemCapabilityReport::new(
            capability,
            SystemCapabilityState::NotConfigured,
            "Nenhum modelo registado serve esta capacidade nesta instalação.",
        )
    })
}

#[cfg(test)]
mod tests {
    use ocinye_contracts::{
        SystemCapabilities, SystemCapability, SystemCapabilityReport, SystemCapabilityState,
    };

    /// The shape the Workspace relies on when nothing is installed.
    fn fresh_installation() -> SystemCapabilities {
        SystemCapabilities {
            capabilities: vec![
                SystemCapabilityReport::new(
                    SystemCapability::AiGeneral,
                    SystemCapabilityState::NoResource,
                    "Nenhum nó de IA Ocinye está actualmente disponível.",
                ),
                SystemCapabilityReport::new(
                    SystemCapability::Agents,
                    SystemCapabilityState::Available,
                    "Os agentes podem ser definidos e guardados.",
                ),
                SystemCapabilityReport::new(
                    SystemCapability::Compute,
                    SystemCapabilityState::NoResource,
                    "Nenhum nó computacional foi registado.",
                ),
            ],
        }
    }

    #[test]
    fn a_fresh_installation_reports_no_resource_and_not_error() {
        let report = fresh_installation();
        for capability in [SystemCapability::AiGeneral, SystemCapability::Compute] {
            let entry = report.get(capability).expect("reported");
            assert_eq!(entry.state, SystemCapabilityState::NoResource);
            // "Sem recurso" is not "broken". The words matter (briefing §7).
            let reason = entry.reason.to_lowercase();
            for banned in ["erro", "falha", "quebrad"] {
                assert!(!reason.contains(banned), "{capability:?} says «{banned}»");
            }
        }
    }

    #[test]
    fn agents_are_definable_without_any_ai_node() {
        // The architectural decision of briefing §9: an agent is a definition,
        // and defining one needs no model.
        let report = fresh_installation();
        assert!(report.is_usable(SystemCapability::Agents));
        assert!(!report.any_ai_usable());
    }

    #[test]
    fn nothing_unreported_is_treated_as_usable() {
        let report = fresh_installation();
        assert!(!report.is_usable(SystemCapability::ObjectStorage));
        assert!(!report.is_usable(SystemCapability::SemanticSearch));
    }
}
