//! Intelligence application layer — the AI Gateway.

use ocinye_contracts::{
    AiCapability, CapabilityStatus, Classification, IntelligenceStatus, PageRequest, RagScope,
};
use ocinye_domain::policy::{authorize, Action, ResourceContext, ResourceKind};
use ocinye_domain::Principal;
use sqlx::PgPool;
use uuid::Uuid;

use super::model::{RegisteredModel, RetrievedRef};
use super::repository as repo;
use crate::config::AiConfig;
use crate::error::{CoreError, CoreResult};
use crate::modules::compute;
use crate::modules::search;
use crate::Tx;

/// Largest number of artefacts placed in a retrieval context.
const MAX_CONTEXT_ARTEFACTS: u32 = 20;

/// List the registered models.
///
/// # Errors
///
/// Returns an error when the caller may not read, or the query fails.
pub async fn list_models(pool: &PgPool, principal: &Principal) -> CoreResult<Vec<RegisteredModel>> {
    let ctx = ResourceContext::organisation(ResourceKind::AiCapability, principal.organisation_id);
    authorize(principal, Action::Read, &ctx)
        .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    repo::list_models(pool).await
}

/// Resolve a capability to a model that can serve it now.
///
/// Returns [`CoreError::CapabilityUnavailable`] when nothing can. That is not a
/// failure of the platform: it is the accurate answer while no Ocinye node is
/// enrolled, and callers are expected to degrade explicitly.
///
/// # Errors
///
/// Returns [`CoreError::CapabilityUnavailable`] when no enabled, healthy model
/// serves the capability, or when configuration maps it to a model that is not
/// currently reported by any node.
pub async fn resolve_capability(
    pool: &PgPool,
    config: &AiConfig,
    capability: AiCapability,
) -> CoreResult<RegisteredModel> {
    let models = repo::list_models(pool).await?;

    let candidates: Vec<RegisteredModel> = models
        .into_iter()
        .filter(|model| model.serves(capability))
        .filter(|model| {
            // An external provider is only ever selected when the institution
            // has explicitly enabled them (ADR-0300).
            model.provider_kind == "ocinye_node" || config.allow_external_providers
        })
        .collect();

    if candidates.is_empty() {
        return Err(CoreError::CapabilityUnavailable(
            "Nenhum nó do Ocinye OS fornece esta capacidade nesta instalação.".to_owned(),
        ));
    }

    // Configuration decides *which* model serves a capability. Code never does.
    if let Some(configured) = config.capability_map.get(&capability) {
        return candidates
            .into_iter()
            .find(|model| &model.model_name == configured)
            .ok_or_else(|| {
                CoreError::CapabilityUnavailable(
                    "O modelo configurado para esta capacidade não está disponível \
                     neste momento."
                        .to_owned(),
                )
            });
    }

    candidates.into_iter().next().ok_or_else(|| {
        CoreError::CapabilityUnavailable(
            "Nenhum nó do Ocinye OS fornece esta capacidade nesta instalação.".to_owned(),
        )
    })
}

/// Report the state of the Intelligence Plane.
///
/// # Errors
///
/// Returns an error when the caller may not read, or the query fails.
pub async fn intelligence_status(
    pool: &PgPool,
    principal: &Principal,
    config: &AiConfig,
) -> CoreResult<IntelligenceStatus> {
    let ctx = ResourceContext::organisation(ResourceKind::AiCapability, principal.organisation_id);
    authorize(principal, Action::Read, &ctx)
        .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    let models = repo::list_models(pool).await?;

    let capabilities: Vec<CapabilityStatus> = AiCapability::all()
        .into_iter()
        .map(|capability| CapabilityStatus {
            capability,
            available: models.iter().any(|model| model.serves(capability)),
            configured_model: config.capability_map.get(&capability).cloned(),
        })
        .collect();

    let providers = u32::try_from(
        models
            .iter()
            .filter(|model| model.status() == ocinye_contracts::ModelStatus::Available)
            .count(),
    )
    .unwrap_or(u32::MAX);

    let available = capabilities.iter().any(|status| status.available);

    Ok(IntelligenceStatus {
        available,
        providers,
        capabilities,
        message: if available {
            "Ocinye AI capabilities are available.".to_owned()
        } else {
            "No Ocinye AI node is currently available. The platform operates fully without \
             one, and no external provider is used in its place."
                .to_owned()
        },
    })
}

/// Mark models of silent nodes as unavailable.
///
/// Run periodically by the Worker. Availability follows a node's real
/// heartbeat rather than lingering as a stale `available`.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn refresh_availability(pool: &PgPool, offline_after_seconds: i64) -> CoreResult<u64> {
    compute::internal::mark_stale_models_unavailable(pool, offline_after_seconds).await
}

/// Assemble a retrieval context for a capability request.
///
/// # The security property
///
/// Retrieval runs through the caller's own read policy, using the same
/// permission-aware search path as any other query. A model can therefore never
/// be given an artefact the requester could not open themselves. In addition,
/// the model's own `max_classification` ceiling is applied, so a model approved
/// only for `INTERNAL` never receives `CONFIDENTIAL` material even for a
/// caller who may read it.
///
/// # Reading is not processing
///
/// A third gate sits above the other two: [`may_process_with_ai`], the
/// institution's own ceiling on what may be sent for inference at all. It is
/// **not** the same question as «may this person read it», and it is not the
/// model's declared ceiling either — a model approved for `CONFIDENTIAL` still
/// does not receive `CONFIDENTIAL` material while every model runs on hardware
/// the Ocinye does not own (`CLAUDE.md` §36, §42).
///
/// The agentic Context Engine has always applied it. This path did not, so the
/// preview it feeds over-stated what would actually reach a model. The two
/// answer the same question and must give the same answer.
///
/// Retrieved content is **data**, never instruction. Callers must keep system
/// policy, application policy, user input and retrieved content structurally
/// separate (ADR-0300).
///
/// # Errors
///
/// Returns an error when the caller may not read, or the search fails.
pub async fn assemble_context(
    pool: &PgPool,
    principal: &Principal,
    query: &str,
    scope: RagScope,
    workspace_id: Option<Uuid>,
    model_ceiling: Classification,
) -> CoreResult<Vec<RetrievedRef>> {
    // The strictest of the model's declared ceiling and the institution's own.
    //
    // `local_inference` is `false`: no Ocinye node exists, so no model runs on
    // hardware the institution controls. When one does, this becomes true in
    // one place and the ceiling rises with no other change.
    let effective_ceiling = if ocinye_domain::may_process_with_ai(model_ceiling, false) {
        model_ceiling
    } else {
        ocinye_domain::ai_processing_ceiling(false)
    };
    let model_ceiling = effective_ceiling;
    let scoped_workspace = match scope {
        RagScope::ResearchWorkspace | RagScope::Project => workspace_id,
        RagScope::Institutional | RagScope::Unit => None,
    };

    let (hits, _) = search::search(
        pool,
        principal,
        query,
        None,
        scoped_workspace,
        PageRequest {
            page: 1,
            page_size: MAX_CONTEXT_ARTEFACTS,
        },
    )
    .await?;

    Ok(hits
        .into_iter()
        .filter(|hit| {
            // The model's ceiling is applied on top of the caller's own rights.
            Classification::parse(&hit.classification)
                .is_some_and(|classification| classification.level() <= model_ceiling.level())
        })
        .map(|hit| RetrievedRef {
            entity_type: hit.entity_type,
            entity_id: hit.entity_id,
            title: hit.title,
            classification: hit.classification,
        })
        .collect())
}

/// Record a capability request that was refused.
///
/// A refused request is still institutional history: it shows what the platform
/// was asked to do and why it could not.
///
/// # Errors
///
/// Returns an error when the insert fails.
pub async fn record_rejected_job(
    tx: &mut Tx<'_>,
    principal: &Principal,
    capability: AiCapability,
    scope: RagScope,
    workspace_id: Option<Uuid>,
    reason: &str,
) -> CoreResult<Uuid> {
    repo::insert_job(
        &mut **tx,
        principal.organisation_id,
        workspace_id,
        principal.person_id,
        capability.as_str(),
        None,
        scope.as_str(),
        "rejected",
        Some(reason),
        &serde_json::Value::Array(vec![]),
    )
    .await
}
