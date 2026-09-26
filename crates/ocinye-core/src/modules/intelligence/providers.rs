//! The AI Fabric's provider registry (ADR-0310, Part 7 of the generalization).
//!
//! # What a provider is
//!
//! Identity, endpoint, residency, and a **reference** to its credential in the
//! Secrets Authority (ADR-0110). Never the credential: this module cannot read
//! one, and the Gateway opens it only for the call that needs it, in the
//! `ai_gateway` scope.
//!
//! # What a provider is not
//!
//! A model. The models a provider serves are registered in the same registry
//! as the ones nodes report (`ai_models`), and the Model Router chooses among
//! all of them by capability. A provider disabled here takes its models out of
//! routing at once, with no restart; an `external` one is only ever routed to
//! when the Instance has allowed external providers.

use chrono::{DateTime, Utc};
use ocinye_contracts::{AiCapability, Permission};
use ocinye_domain::policy::permissions::can;
use ocinye_domain::policy::{ResourceContext, ResourceKind};
use ocinye_domain::Principal;
use ocinye_observability::CorrelationIds;
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use super::adapters::{HttpAdapter, ProviderKind};
use super::model::RegisteredModel;
use crate::audit::{self, action, AuditEntry};
use crate::error::{CoreError, CoreResult};
use crate::modules::secrets::{use_secret, SecretScope};
use crate::password::sealed::SealingKey;

/// Where a provider runs, which decides whether data leaves the Instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Residency {
    /// Data leaves the Instance: a cloud API.
    External,
    /// Data stays on infrastructure the Instance controls.
    Local,
}

impl Residency {
    /// The stored representation, which is also the model's `provider_kind`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::External => "external",
            Self::Local => "local",
        }
    }

    /// Parse the stored representation.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "external" => Some(Self::External),
            "local" => Some(Self::Local),
            _ => None,
        }
    }
}

/// A registered provider, as administration sees it.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct AiProvider {
    /// Identifier.
    pub id: Uuid,
    /// Which protocol it speaks.
    pub kind: String,
    /// The name administration gave it.
    pub label: String,
    /// Where it is reached.
    pub endpoint_url: String,
    /// `external` or `local`.
    pub residency: String,
    /// The credential it uses, by reference.
    pub secret_id: Option<Uuid>,
    /// Whether its models may be routed to.
    pub enabled: bool,
    /// The outcome of the last call made to it.
    pub health: String,
    /// When that call was made.
    pub last_checked_at: Option<DateTime<Utc>>,
    /// When it was registered.
    pub created_at: DateTime<Utc>,
}

/// What administration submits to register a provider.
#[derive(Debug, Clone)]
pub struct NewProvider {
    /// One of [`ProviderKind`].
    pub kind: String,
    /// A name for people.
    pub label: String,
    /// The base URL.
    pub endpoint_url: String,
    /// `external` or `local`.
    pub residency: String,
    /// A secret in the `ai_gateway` scope, when the provider needs one.
    pub secret_id: Option<Uuid>,
}

/// What administration submits to register one of a provider's models.
#[derive(Debug, Clone)]
pub struct NewProviderModel {
    /// The model's name as the provider calls it.
    pub model_name: String,
    /// Its version, when the provider has one.
    pub version: Option<String>,
    /// What it serves.
    pub capabilities: Vec<AiCapability>,
    /// Its context window, when known.
    pub context_limit: Option<i32>,
    /// The ceiling on what may be sent to it. Defaults to `PUBLIC` for an
    /// external provider and `INTERNAL` for a local one.
    pub max_classification: Option<String>,
}

const PROVIDER_COLUMNS: &str = "id, kind, label, endpoint_url, residency, secret_id, enabled,
                                health, last_checked_at, created_at";

fn require(principal: &Principal) -> CoreResult<()> {
    let ctx = ResourceContext::organisation(ResourceKind::Platform, principal.organisation_id);
    if can(principal, Permission::AiInfrastructureManage, &ctx, None).allowed {
        Ok(())
    } else {
        Err(CoreError::PermissionDenied(
            "Os fornecedores de IA são da administração da infraestrutura de IA.".to_owned(),
        ))
    }
}

fn clean(value: &str, field: &str, max: usize) -> CoreResult<String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(CoreError::Validation(format!("{field} é obrigatório.")));
    }
    if value.chars().count() > max || value.chars().any(char::is_control) {
        return Err(CoreError::Validation(format!("{field} não é válido.")));
    }
    Ok(value.to_owned())
}

/// An endpoint the Gateway may send a credential to.
///
/// No user-info (a credential in a URL ends up in logs; it belongs in the
/// Secrets Authority), no query or fragment, and `https` whenever data leaves
/// the Instance. Plain `http` is accepted only for a local provider — Ollama on
/// the same host is the ordinary case.
fn validate_endpoint(url: &str, residency: Residency) -> CoreResult<String> {
    let url = clean(url, "O endereço", 512)?;
    let parsed = reqwest::Url::parse(&url)
        .map_err(|_| CoreError::Validation("O endereço não é um URL.".to_owned()))?;
    let scheme_ok = match residency {
        Residency::External => parsed.scheme() == "https",
        Residency::Local => matches!(parsed.scheme(), "http" | "https"),
    };
    if !scheme_ok {
        return Err(CoreError::Validation(
            "Um fornecedor externo exige https.".to_owned(),
        ));
    }
    if parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(CoreError::Validation(
            "O endereço não pode levar credenciais, parâmetros nem fragmento.".to_owned(),
        ));
    }
    Ok(url.trim_end_matches('/').to_owned())
}

/// Register a provider.
///
/// # Errors
///
/// [`CoreError::PermissionDenied`] without `ai.infrastructure.manage`;
/// [`CoreError::Validation`] for an unknown kind or residency, an unsafe
/// endpoint, or a secret that is not this Instance's, active, and in the
/// `ai_gateway` scope.
pub async fn create_provider(
    pool: &PgPool,
    principal: &Principal,
    new: NewProvider,
    ids: &CorrelationIds,
) -> CoreResult<AiProvider> {
    require(principal)?;
    let kind = ProviderKind::parse(&new.kind)
        .ok_or_else(|| CoreError::Validation("Tipo de fornecedor desconhecido.".to_owned()))?;
    let residency = Residency::parse(&new.residency)
        .ok_or_else(|| CoreError::Validation("Residência desconhecida.".to_owned()))?;
    let label = clean(&new.label, "O nome", 128)?;
    let endpoint = validate_endpoint(&new.endpoint_url, residency)?;

    let mut tx = pool.begin().await?;
    if let Some(secret_id) = new.secret_id {
        let usable: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM instance_secrets
                             WHERE id = $1 AND organisation_id = $2
                               AND scope = $3 AND status = 'active')",
        )
        .bind(secret_id)
        .bind(principal.organisation_id)
        .bind(SecretScope::AiGateway.as_str())
        .fetch_one(&mut *tx)
        .await?;
        if !usable {
            return Err(CoreError::Validation(
                "O segredo não existe, não está activo ou não é do Gateway de IA.".to_owned(),
            ));
        }
    }
    let provider = sqlx::query_as::<_, AiProvider>(&format!(
        "INSERT INTO ai_providers
             (organisation_id, kind, label, endpoint_url, residency, secret_id, created_by_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         RETURNING {PROVIDER_COLUMNS}"
    ))
    .bind(principal.organisation_id)
    .bind(kind.as_str())
    .bind(&label)
    .bind(&endpoint)
    .bind(residency.as_str())
    .bind(new.secret_id)
    .bind(principal.person_id)
    .fetch_one(&mut *tx)
    .await?;
    audit::record(
        &mut tx,
        Some(principal),
        ids,
        AuditEntry::new(action::ADMIN_OPERATION, "ai_provider")
            .resource(provider.id)
            .detail("event", "created")
            .detail("kind", kind.as_str())
            .detail("residency", residency.as_str()),
    )
    .await?;
    tx.commit().await?;
    Ok(provider)
}

/// The Instance's providers.
///
/// # Errors
///
/// [`CoreError::PermissionDenied`] without `ai.infrastructure.manage`.
pub async fn list_providers(pool: &PgPool, principal: &Principal) -> CoreResult<Vec<AiProvider>> {
    require(principal)?;
    Ok(sqlx::query_as::<_, AiProvider>(&format!(
        "SELECT {PROVIDER_COLUMNS} FROM ai_providers
          WHERE organisation_id = $1 ORDER BY label, created_at"
    ))
    .bind(principal.organisation_id)
    .fetch_all(pool)
    .await?)
}

/// Enable or disable a provider. A disabled provider's models leave routing
/// on the next request.
///
/// # Errors
///
/// [`CoreError::PermissionDenied`] without `ai.infrastructure.manage`;
/// [`CoreError::NotFound`] for another Instance's provider or none.
pub async fn set_provider_enabled(
    pool: &PgPool,
    principal: &Principal,
    provider_id: Uuid,
    enabled: bool,
    ids: &CorrelationIds,
) -> CoreResult<AiProvider> {
    require(principal)?;
    let mut tx = pool.begin().await?;
    let provider = sqlx::query_as::<_, AiProvider>(&format!(
        "UPDATE ai_providers SET enabled = $3, updated_at = now()
          WHERE id = $1 AND organisation_id = $2
          RETURNING {PROVIDER_COLUMNS}"
    ))
    .bind(provider_id)
    .bind(principal.organisation_id)
    .bind(enabled)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| CoreError::NotFound("Fornecedor não encontrado.".to_owned()))?;
    audit::record(
        &mut tx,
        Some(principal),
        ids,
        AuditEntry::new(action::ADMIN_OPERATION, "ai_provider")
            .resource(provider.id)
            .detail("event", if enabled { "enabled" } else { "disabled" }),
    )
    .await?;
    tx.commit().await?;
    Ok(provider)
}

/// Remove a provider and, with it, its models.
///
/// # Errors
///
/// [`CoreError::PermissionDenied`] without `ai.infrastructure.manage`;
/// [`CoreError::NotFound`] for another Instance's provider or none.
pub async fn delete_provider(
    pool: &PgPool,
    principal: &Principal,
    provider_id: Uuid,
    ids: &CorrelationIds,
) -> CoreResult<()> {
    require(principal)?;
    let mut tx = pool.begin().await?;
    let removed = sqlx::query("DELETE FROM ai_providers WHERE id = $1 AND organisation_id = $2")
        .bind(provider_id)
        .bind(principal.organisation_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    if removed == 0 {
        return Err(CoreError::NotFound("Fornecedor não encontrado.".to_owned()));
    }
    audit::record(
        &mut tx,
        Some(principal),
        ids,
        AuditEntry::new(action::ADMIN_OPERATION, "ai_provider")
            .resource(provider_id)
            .detail("event", "deleted"),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

/// Register a model a provider serves.
///
/// # Errors
///
/// [`CoreError::PermissionDenied`] without `ai.infrastructure.manage`;
/// [`CoreError::NotFound`] for another Instance's provider or none;
/// [`CoreError::Validation`] for an empty capability list or an unknown
/// classification.
pub async fn register_provider_model(
    pool: &PgPool,
    principal: &Principal,
    provider_id: Uuid,
    new: NewProviderModel,
    ids: &CorrelationIds,
) -> CoreResult<Uuid> {
    require(principal)?;
    let model_name = clean(&new.model_name, "O modelo", 128)?;
    let version = match new.version.as_deref() {
        Some(version) if !version.trim().is_empty() => clean(version, "A versão", 64)?,
        _ => "unknown".to_owned(),
    };
    if new.capabilities.is_empty() {
        return Err(CoreError::Validation(
            "Um modelo serve pelo menos uma capacidade.".to_owned(),
        ));
    }
    let capabilities: Vec<&str> = new.capabilities.iter().map(|c| c.as_str()).collect();

    let mut tx = pool.begin().await?;
    let residency: String = sqlx::query_scalar(
        "SELECT residency FROM ai_providers WHERE id = $1 AND organisation_id = $2",
    )
    .bind(provider_id)
    .bind(principal.organisation_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| CoreError::NotFound("Fornecedor não encontrado.".to_owned()))?;
    let ceiling = match new.max_classification.as_deref() {
        Some(value @ ("PUBLIC" | "INTERNAL" | "CONFIDENTIAL" | "RESTRICTED")) => value.to_owned(),
        Some(_) => {
            return Err(CoreError::Validation(
                "Classificação desconhecida.".to_owned(),
            ))
        }
        // External providers close at PUBLIC unless administration says
        // otherwise, as embeddings do (ADR-0206).
        None if residency == "external" => "PUBLIC".to_owned(),
        None => "INTERNAL".to_owned(),
    };
    let model_id: Uuid = sqlx::query_scalar(
        "INSERT INTO ai_models
             (provider_kind, provider_name, provider_id, model_name, version, capabilities,
              context_limit, status, max_classification, created_by_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, 'available', $8, $9)
         RETURNING id",
    )
    .bind(&residency)
    .bind(provider_id.to_string())
    .bind(provider_id)
    .bind(&model_name)
    .bind(&version)
    .bind(serde_json::json!(capabilities))
    .bind(new.context_limit)
    .bind(&ceiling)
    .bind(principal.person_id)
    .fetch_one(&mut *tx)
    .await?;
    audit::record(
        &mut tx,
        Some(principal),
        ids,
        AuditEntry::new(action::ADMIN_OPERATION, "ai_model")
            .resource(model_id)
            .detail("event", "registered")
            .detail("provider_id", provider_id.to_string())
            .detail("model", model_name.as_str()),
    )
    .await?;
    tx.commit().await?;
    Ok(model_id)
}

/// The adapter that runs a provider model, with its credential opened for
/// this call only.
///
/// # Errors
///
/// [`CoreError::NotFound`] when the provider is gone or disabled, or its
/// secret was revoked; the Secrets Authority's errors when the sealing key is
/// absent.
pub async fn adapter_for(
    pool: &PgPool,
    key: Option<&SealingKey>,
    organisation_id: Uuid,
    model: &RegisteredModel,
    ids: &CorrelationIds,
) -> CoreResult<HttpAdapter> {
    let provider_id = model
        .provider_id
        .ok_or_else(|| CoreError::NotFound("O modelo não é de um fornecedor.".to_owned()))?;
    let provider = sqlx::query_as::<_, AiProvider>(&format!(
        "SELECT {PROVIDER_COLUMNS} FROM ai_providers
          WHERE id = $1 AND organisation_id = $2 AND enabled"
    ))
    .bind(provider_id)
    .bind(organisation_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| CoreError::NotFound("Fornecedor não encontrado.".to_owned()))?;
    let kind = ProviderKind::parse(&provider.kind).ok_or_else(|| {
        CoreError::Internal("tipo de fornecedor guardado desconhecido".to_owned())
    })?;
    let credential = match provider.secret_id {
        Some(secret_id) => Some(
            use_secret(
                pool,
                key,
                organisation_id,
                secret_id,
                SecretScope::AiGateway,
                ids,
            )
            .await?,
        ),
        None => None,
    };
    let capabilities = model
        .capabilities
        .as_array()
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str().and_then(AiCapability::parse))
                .collect()
        })
        .unwrap_or_default();
    HttpAdapter::new(
        kind,
        &provider.label,
        &provider.endpoint_url,
        (&model.model_name, &model.version),
        capabilities,
        credential,
    )
    .map_err(|_| CoreError::Internal("cliente HTTP do fornecedor".to_owned()))
}

/// Record the outcome of the last call to a provider. Best-effort: health is
/// an observation for administration, never a routing input.
pub async fn record_health(pool: &PgPool, provider_id: Uuid, health: &str) {
    let _ =
        sqlx::query("UPDATE ai_providers SET health = $2, last_checked_at = now() WHERE id = $1")
            .bind(provider_id)
            .bind(health)
            .execute(pool)
            .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn um_fornecedor_externo_so_por_https() {
        assert!(validate_endpoint("http://api.exemplo.com/v1", Residency::External).is_err());
        assert!(validate_endpoint("https://api.exemplo.com/v1/", Residency::External).is_ok());
        assert!(validate_endpoint("http://127.0.0.1:11434/v1", Residency::Local).is_ok());
    }

    #[test]
    fn o_endereco_nao_leva_credenciais_nem_parametros() {
        for url in [
            "https://chave:segredo@api.exemplo.com/v1",
            "https://api.exemplo.com/v1?key=segredo",
            "https://api.exemplo.com/v1#x",
            "ftp://api.exemplo.com",
            "não é url",
        ] {
            assert!(
                validate_endpoint(url, Residency::External).is_err(),
                "{url}"
            );
        }
    }

    #[test]
    fn a_barra_final_nao_duplica_caminhos() {
        assert_eq!(
            validate_endpoint("https://api.exemplo.com/v1/", Residency::External).expect("url"),
            "https://api.exemplo.com/v1"
        );
    }
}
