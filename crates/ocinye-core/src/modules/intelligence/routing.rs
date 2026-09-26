//! AI policy and routing across providers (ADR-0311, Part 8 of the
//! generalization).
//!
//! # Policy first, preference second
//!
//! The Router answers «who may receive this request, and in what order?». The
//! first half is **policy** and is never negotiable: a model whose ceiling is
//! below the request's classification, or an external provider above the
//! Instance's external ceiling — or any external provider while the
//! installation forbids them — is not a candidate, however it is configured.
//! The second half is **preference**: among what policy allows, the preferred
//! provider goes first, then the configured model, then local before external.
//!
//! «Every candidate was excluded by policy» is its own typed answer,
//! `AI_POLICY_BLOCKED`, distinct from «nothing serves this capability»: the
//! first tells an administrator that the data is too sensitive for what is
//! connected, the second that nothing is connected.

use chrono::{DateTime, Utc};
use ocinye_contracts::{AiCapability, AiReasonCode, Classification, Permission};
use ocinye_domain::policy::permissions::can;
use ocinye_domain::policy::{ResourceContext, ResourceKind};
use ocinye_domain::Principal;
use ocinye_observability::CorrelationIds;
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use super::model::RegisteredModel;
use super::repository as repo;
use crate::audit::{self, action, AuditEntry};
use crate::config::AiConfig;
use crate::error::{CoreError, CoreResult};

/// The Instance's AI policy, as administration sees and sets it.
#[derive(Debug, Clone, Serialize)]
pub struct AiPolicy {
    /// The highest classification an external provider may receive; `None`
    /// means no external AI at all.
    pub external_max_classification: Option<Classification>,
    /// Whether the installation allows external providers at all
    /// (`OCINYE_AI_ALLOW_EXTERNAL_PROVIDERS`). Read-only here: it is the
    /// operator's switch, above any Instance's policy.
    pub installation_allows_external: bool,
    /// The preferred provider and fallback per capability.
    pub preferences: Vec<RoutingPreference>,
}

/// Who answers a capability first.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct RoutingPreference {
    /// `GENERAL`, `CODING`, `REASONING` or `EMBEDDING`.
    pub capability: String,
    /// The provider tried first, when set.
    pub preferred_provider_id: Option<Uuid>,
    /// Whether another candidate may answer when the preferred one does not.
    pub allow_fallback: bool,
    /// When it was last changed.
    pub updated_at: DateTime<Utc>,
}

/// The Router's answer.
#[derive(Debug)]
pub enum Routing {
    /// Candidates in the order to try them. With `allow_fallback` false only
    /// the first may be tried.
    Candidates {
        /// Allowed by policy, ordered by preference.
        models: Vec<RegisteredModel>,
        /// Whether the caller may move to the next candidate on failure.
        allow_fallback: bool,
    },
    /// Nobody may answer, with the machine reason why.
    NoCandidate(AiReasonCode),
}

const EXTERNAL_DEFAULT: Classification = Classification::Internal;

fn parse_external(value: &str) -> Option<Classification> {
    if value == "NONE" {
        None
    } else {
        Some(Classification::parse(value).unwrap_or(Classification::Public))
    }
}

async fn external_ceiling(
    pool: &PgPool,
    organisation_id: Uuid,
) -> CoreResult<Option<Classification>> {
    let stored: Option<String> = sqlx::query_scalar(
        "SELECT external_max_classification FROM instance_ai_policy WHERE organisation_id = $1",
    )
    .bind(organisation_id)
    .fetch_optional(pool)
    .await?;
    Ok(stored.map_or(Some(EXTERNAL_DEFAULT), |value| parse_external(&value)))
}

async fn preference(
    pool: &PgPool,
    organisation_id: Uuid,
    capability: AiCapability,
) -> CoreResult<Option<RoutingPreference>> {
    Ok(sqlx::query_as::<_, RoutingPreference>(
        "SELECT capability, preferred_provider_id, allow_fallback, updated_at
           FROM ai_routing_preferences WHERE organisation_id = $1 AND capability = $2",
    )
    .bind(organisation_id)
    .bind(capability.as_str())
    .fetch_optional(pool)
    .await?)
}

/// Whether policy lets this model receive a request of this classification.
fn allowed(
    model: &RegisteredModel,
    classification: Classification,
    allow_external: bool,
    external_ceiling: Option<Classification>,
) -> bool {
    if classification > model.max_classification() {
        return false;
    }
    if model.provider_kind == "external" {
        return allow_external && external_ceiling.is_some_and(|ceiling| classification <= ceiling);
    }
    true
}

/// Route a capability for a request of a given classification.
///
/// # Errors
///
/// Only on a database fault. «Nobody may answer» is [`Routing::NoCandidate`].
pub async fn route(
    pool: &PgPool,
    organisation_id: Uuid,
    config: &AiConfig,
    capability: AiCapability,
    classification: Classification,
) -> CoreResult<Routing> {
    let models = repo::list_models(pool, organisation_id).await?;
    if models.is_empty() {
        return Ok(Routing::NoCandidate(AiReasonCode::AiNoProviderAvailable));
    }
    let serving: Vec<RegisteredModel> = models
        .into_iter()
        .filter(|model| model.serves(capability))
        .collect();
    if serving.is_empty() {
        return Ok(Routing::NoCandidate(AiReasonCode::AiNoCompatibleModel));
    }

    let ceiling = external_ceiling(pool, organisation_id).await?;
    let mut allowed_models: Vec<RegisteredModel> = serving
        .into_iter()
        .filter(|model| {
            allowed(
                model,
                classification,
                config.allow_external_providers,
                ceiling,
            )
        })
        .collect();
    if allowed_models.is_empty() {
        return Ok(Routing::NoCandidate(AiReasonCode::AiPolicyBlocked));
    }

    // Configuration pins a model name per capability: only that model, as
    // before (ADR-0304). Policy has already had its say.
    if let Some(configured) = config.capability_map.get(&capability) {
        allowed_models.retain(|model| &model.model_name == configured);
        if allowed_models.is_empty() {
            return Ok(Routing::NoCandidate(AiReasonCode::AiNoCompatibleModel));
        }
    }

    let preference = preference(pool, organisation_id, capability).await?;
    let preferred = preference.as_ref().and_then(|p| p.preferred_provider_id);
    // Preferred provider first; then what stays on the Instance's own
    // infrastructure; then the rest. Stable within each group.
    allowed_models.sort_by_key(|model| {
        (
            u8::from(preferred.is_none() || model.provider_id != preferred),
            u8::from(model.provider_kind == "external"),
        )
    });
    Ok(Routing::Candidates {
        models: allowed_models,
        allow_fallback: preference.is_none_or(|p| p.allow_fallback),
    })
}

fn require(principal: &Principal) -> CoreResult<()> {
    let ctx = ResourceContext::organisation(ResourceKind::Platform, principal.organisation_id);
    if can(principal, Permission::AiInfrastructureManage, &ctx, None).allowed {
        Ok(())
    } else {
        Err(CoreError::PermissionDenied(
            "A política de IA é da administração da infraestrutura de IA.".to_owned(),
        ))
    }
}

/// The Instance's AI policy.
///
/// # Errors
///
/// [`CoreError::PermissionDenied`] without `ai.infrastructure.manage`.
pub async fn get_policy(
    pool: &PgPool,
    principal: &Principal,
    config: &AiConfig,
) -> CoreResult<AiPolicy> {
    require(principal)?;
    let preferences = sqlx::query_as::<_, RoutingPreference>(
        "SELECT capability, preferred_provider_id, allow_fallback, updated_at
           FROM ai_routing_preferences WHERE organisation_id = $1 ORDER BY capability",
    )
    .bind(principal.organisation_id)
    .fetch_all(pool)
    .await?;
    Ok(AiPolicy {
        external_max_classification: external_ceiling(pool, principal.organisation_id).await?,
        installation_allows_external: config.allow_external_providers,
        preferences,
    })
}

/// Set how far data may travel to external providers. `None` forbids them.
///
/// # Errors
///
/// [`CoreError::PermissionDenied`] without `ai.infrastructure.manage`.
pub async fn set_external_ceiling(
    pool: &PgPool,
    principal: &Principal,
    ceiling: Option<Classification>,
    ids: &CorrelationIds,
) -> CoreResult<()> {
    require(principal)?;
    let value = ceiling.map_or("NONE", Classification::as_str);
    let mut tx = pool.begin().await?;
    sqlx::query(
        "INSERT INTO instance_ai_policy (organisation_id, external_max_classification, updated_by_id)
         VALUES ($1, $2, $3)
         ON CONFLICT (organisation_id) DO UPDATE
             SET external_max_classification = EXCLUDED.external_max_classification,
                 updated_by_id = EXCLUDED.updated_by_id, updated_at = now()",
    )
    .bind(principal.organisation_id)
    .bind(value)
    .bind(principal.person_id)
    .execute(&mut *tx)
    .await?;
    audit::record(
        &mut tx,
        Some(principal),
        ids,
        AuditEntry::new(action::ADMIN_OPERATION, "instance_ai_policy")
            .resource(principal.organisation_id)
            .detail("external_max_classification", value),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

/// Set who answers a capability first, and whether another may answer.
///
/// # Errors
///
/// [`CoreError::PermissionDenied`] without `ai.infrastructure.manage`;
/// [`CoreError::Validation`] for a provider that is not this Instance's.
pub async fn set_preference(
    pool: &PgPool,
    principal: &Principal,
    capability: AiCapability,
    preferred_provider_id: Option<Uuid>,
    allow_fallback: bool,
    ids: &CorrelationIds,
) -> CoreResult<()> {
    require(principal)?;
    let mut tx = pool.begin().await?;
    if let Some(provider_id) = preferred_provider_id {
        let ours: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM ai_providers WHERE id = $1 AND organisation_id = $2)",
        )
        .bind(provider_id)
        .bind(principal.organisation_id)
        .fetch_one(&mut *tx)
        .await?;
        if !ours {
            return Err(CoreError::Validation(
                "O fornecedor não é desta instância.".to_owned(),
            ));
        }
    }
    sqlx::query(
        "INSERT INTO ai_routing_preferences
             (organisation_id, capability, preferred_provider_id, allow_fallback, updated_by_id)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (organisation_id, capability) DO UPDATE
             SET preferred_provider_id = EXCLUDED.preferred_provider_id,
                 allow_fallback = EXCLUDED.allow_fallback,
                 updated_by_id = EXCLUDED.updated_by_id, updated_at = now()",
    )
    .bind(principal.organisation_id)
    .bind(capability.as_str())
    .bind(preferred_provider_id)
    .bind(allow_fallback)
    .bind(principal.person_id)
    .execute(&mut *tx)
    .await?;
    audit::record(
        &mut tx,
        Some(principal),
        ids,
        AuditEntry::new(action::ADMIN_OPERATION, "ai_routing_preference")
            .resource(principal.organisation_id)
            .detail("capability", capability.as_str())
            .detail(
                "preferred_provider_id",
                preferred_provider_id.map_or_else(String::new, |id| id.to_string()),
            )
            .detail("allow_fallback", allow_fallback.to_string()),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn modelo(kind: &str, ceiling: &str) -> RegisteredModel {
        RegisteredModel {
            id: Uuid::new_v4(),
            provider_kind: kind.to_owned(),
            provider_name: "p".to_owned(),
            node_id: None,
            provider_id: Some(Uuid::new_v4()),
            model_name: "m".to_owned(),
            version: "1".to_owned(),
            capabilities: serde_json::json!(["GENERAL"]),
            context_limit: None,
            status: "available".to_owned(),
            max_classification: ceiling.to_owned(),
            enabled: true,
            reported_at: None,
        }
    }

    #[test]
    fn confidencial_nunca_vai_para_fora_com_o_tecto_por_omissao() {
        let externo = modelo("external", "RESTRICTED");
        assert!(!allowed(
            &externo,
            Classification::Confidential,
            true,
            Some(EXTERNAL_DEFAULT)
        ));
        assert!(allowed(
            &externo,
            Classification::Internal,
            true,
            Some(EXTERNAL_DEFAULT)
        ));
    }

    #[test]
    fn sem_ia_externa_nenhum_externo_passa() {
        let externo = modelo("external", "RESTRICTED");
        assert!(!allowed(&externo, Classification::Public, true, None));
        // E o interruptor da instalação manda acima da política da Instância.
        assert!(!allowed(
            &externo,
            Classification::Public,
            false,
            Some(Classification::Restricted)
        ));
    }

    #[test]
    fn o_tecto_do_modelo_aplica_se_tambem_aos_locais() {
        let local = modelo("local", "INTERNAL");
        assert!(allowed(&local, Classification::Internal, false, None));
        assert!(!allowed(&local, Classification::Confidential, false, None));
    }

    #[test]
    fn none_e_a_ausencia_de_ia_externa_e_nao_um_valor_invalido() {
        assert_eq!(parse_external("NONE"), None);
        assert_eq!(
            parse_external("CONFIDENTIAL"),
            Some(Classification::Confidential)
        );
        // Ilegível fecha para o mais apertado, nunca abre.
        assert_eq!(parse_external("??"), Some(Classification::Public));
    }
}
