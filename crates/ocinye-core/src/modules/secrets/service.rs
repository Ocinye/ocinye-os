//! The Secrets Authority's operations. Every one is audited, and no audit
//! entry, log line or response carries a secret's value.

use chrono::{DateTime, Utc};
use ocinye_contracts::Permission;
use ocinye_domain::policy::permissions::can;
use ocinye_domain::policy::{ResourceContext, ResourceKind};
use ocinye_domain::Principal;
use ocinye_observability::CorrelationIds;
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use super::repository::{self as repo, SecretRow};
use crate::audit::{self, action, AuditEntry};
use crate::error::{CoreError, CoreResult};
use crate::password::sealed::{self, Sealed, SealingDomain, SealingKey};
use crate::password::Secret;

/// The Core service that may open a secret. A secret names one, at creation,
/// and only that service's call reaches the plaintext.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretScope {
    /// The AI Gateway, to call a provider (Parts 7–8).
    AiGateway,
    /// The mail integration.
    Mail,
    /// A future connector.
    Connector,
}

impl SecretScope {
    /// The stored identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AiGateway => "ai_gateway",
            Self::Mail => "mail",
            Self::Connector => "connector",
        }
    }

    /// Parse the stored identifier.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "ai_gateway" => Some(Self::AiGateway),
            "mail" => Some(Self::Mail),
            "connector" => Some(Self::Connector),
            _ => None,
        }
    }
}

/// What an administrator gives to create a secret.
pub struct NewSecret {
    /// What it is for, e.g. `ai_provider`.
    pub kind: String,
    /// A human label.
    pub label: String,
    /// The Core service that may use it.
    pub scope: SecretScope,
    /// The value. Wrapped so it cannot be printed by accident.
    pub value: Secret,
}

/// What anyone may learn about a secret: never the value.
#[derive(Debug, Clone, Serialize)]
pub struct SecretSummary {
    /// Identity.
    pub id: Uuid,
    /// What it is for.
    pub kind: String,
    /// Its label.
    pub label: String,
    /// The Core service that may use it.
    pub scope: String,
    /// The last four characters, for recognition.
    pub hint: Option<String>,
    /// Incremented by every rotation.
    pub version: i32,
    /// `active` or `revoked`.
    pub status: String,
    /// When created.
    pub created_at: DateTime<Utc>,
    /// When last rotated.
    pub rotated_at: Option<DateTime<Utc>>,
    /// When revoked.
    pub revoked_at: Option<DateTime<Utc>>,
    /// When a Core service last used it.
    pub last_used_at: Option<DateTime<Utc>>,
}

impl From<SecretRow> for SecretSummary {
    fn from(row: SecretRow) -> Self {
        Self {
            id: row.id,
            kind: row.kind,
            label: row.label,
            scope: row.scope,
            hint: row.hint,
            version: row.version,
            status: row.status,
            created_at: row.created_at,
            rotated_at: row.rotated_at,
            revoked_at: row.revoked_at,
            last_used_at: row.last_used_at,
        }
    }
}

/// The Secrets Authority is platform administration: privileged, and behind
/// the second factor (ADR-0107).
fn require(principal: &Principal) -> CoreResult<()> {
    let ctx = ResourceContext::organisation(ResourceKind::Platform, principal.organisation_id);
    if can(principal, Permission::PlatformAdminister, &ctx, None).allowed {
        Ok(())
    } else {
        Err(CoreError::PermissionDenied(
            "Os segredos da instância são da administração da plataforma.".to_owned(),
        ))
    }
}

fn root(key: Option<&SealingKey>) -> CoreResult<&SealingKey> {
    key.ok_or_else(|| {
        CoreError::CapabilityUnavailable(
            "Esta instalação não tem raiz de selagem (OCINYE_SEALING_KEY): não pode guardar \
             segredos."
                .to_owned(),
        )
    })
}

/// A value fit to be sealed, and its recognition hint.
fn validate(value: &Secret) -> CoreResult<String> {
    let claro = value.expose();
    if claro.trim().is_empty() {
        return Err(CoreError::Validation("O segredo está vazio.".to_owned()));
    }
    if claro.len() > 4096 {
        return Err(CoreError::Validation(
            "O segredo é demasiado longo.".to_owned(),
        ));
    }
    if claro.chars().any(char::is_control) {
        return Err(CoreError::Validation(
            "O segredo tem caracteres de controlo.".to_owned(),
        ));
    }
    // Quatro caracteres só quando o segredo é longo o bastante para que
    // mostrá-los não o entregue: abaixo de dezasseis, nenhum.
    let chars: Vec<char> = claro.chars().collect();
    Ok(if chars.len() >= 16 {
        chars[chars.len() - 4..].iter().collect()
    } else {
        String::new()
    })
}

fn clean_text(value: &str, what: &str, max: usize) -> CoreResult<String> {
    let value = value.trim();
    if value.is_empty() || value.len() > max || value.chars().any(char::is_control) {
        return Err(CoreError::Validation(format!("{what} inválido.")));
    }
    Ok(value.to_owned())
}

/// Seal and store a new secret.
///
/// # Errors
///
/// [`CoreError::PermissionDenied`] without platform administration;
/// [`CoreError::CapabilityUnavailable`] without a sealing root;
/// [`CoreError::Validation`] for an unfit value; database errors.
pub async fn create_secret(
    pool: &PgPool,
    key: Option<&SealingKey>,
    principal: &Principal,
    new: NewSecret,
    ids: &CorrelationIds,
) -> CoreResult<SecretSummary> {
    require(principal)?;
    let raiz = root(key)?;
    let kind = clean_text(&new.kind, "Tipo", 48)?;
    let label = clean_text(&new.label, "Nome", 128)?;
    let hint = validate(&new.value)?;
    let Sealed { nonce, ciphertext } =
        sealed::seal(raiz, SealingDomain::InstanceSecrets, new.value.expose())?;

    let mut tx = pool.begin().await?;
    let row = repo::insert(
        &mut *tx,
        principal.organisation_id,
        &kind,
        &label,
        new.scope.as_str(),
        (&nonce, &ciphertext),
        &hint,
        principal.person_id,
    )
    .await?;
    audit::record(
        &mut tx,
        Some(principal),
        ids,
        AuditEntry::new(action::ADMIN_OPERATION, "instance_secret")
            .resource(row.id)
            .detail("event", "created")
            .detail("kind", kind.as_str())
            .detail("scope", new.scope.as_str()),
    )
    .await?;
    tx.commit().await?;
    Ok(row.into())
}

/// Every secret's metadata.
///
/// # Errors
///
/// [`CoreError::PermissionDenied`] without platform administration.
pub async fn list_secrets(pool: &PgPool, principal: &Principal) -> CoreResult<Vec<SecretSummary>> {
    require(principal)?;
    Ok(repo::list(pool, principal.organisation_id)
        .await?
        .into_iter()
        .map(Into::into)
        .collect())
}

/// Replace a secret's value. The version increments, and the old value stops
/// existing: it is overwritten, not kept.
///
/// # Errors
///
/// As [`create_secret`]; [`CoreError::NotFound`] for a revoked or unknown one.
pub async fn rotate_secret(
    pool: &PgPool,
    key: Option<&SealingKey>,
    principal: &Principal,
    id: Uuid,
    value: Secret,
    ids: &CorrelationIds,
) -> CoreResult<SecretSummary> {
    require(principal)?;
    let raiz = root(key)?;
    let hint = validate(&value)?;
    let Sealed { nonce, ciphertext } =
        sealed::seal(raiz, SealingDomain::InstanceSecrets, value.expose())?;
    let mut tx = pool.begin().await?;
    let row = repo::replace_material(
        &mut *tx,
        principal.organisation_id,
        id,
        (&nonce, &ciphertext),
        &hint,
    )
    .await?
    .ok_or_else(|| CoreError::NotFound("Segredo não encontrado.".to_owned()))?;
    audit::record(
        &mut tx,
        Some(principal),
        ids,
        AuditEntry::new(action::ADMIN_OPERATION, "instance_secret")
            .resource(id)
            .detail("event", "rotated")
            .detail("version", row.version),
    )
    .await?;
    tx.commit().await?;
    Ok(row.into())
}

/// Revoke a secret. The ciphertext is wiped; nothing can open it afterwards.
///
/// # Errors
///
/// [`CoreError::PermissionDenied`]; [`CoreError::NotFound`] for an unknown or
/// already revoked one.
pub async fn revoke_secret(
    pool: &PgPool,
    principal: &Principal,
    id: Uuid,
    ids: &CorrelationIds,
) -> CoreResult<SecretSummary> {
    require(principal)?;
    let mut tx = pool.begin().await?;
    let row = repo::revoke(&mut *tx, principal.organisation_id, id)
        .await?
        .ok_or_else(|| CoreError::NotFound("Segredo não encontrado.".to_owned()))?;
    audit::record(
        &mut tx,
        Some(principal),
        ids,
        AuditEntry::new(action::ADMIN_OPERATION, "instance_secret")
            .resource(id)
            .detail("event", "revoked"),
    )
    .await?;
    tx.commit().await?;
    Ok(row.into())
}

/// Open a secret for a Core service in its scope, for the moment of use.
///
/// **Not an HTTP route, and never one.** The caller is Core code — the AI
/// Gateway calling a provider — and the value it gets is a [`Secret`] that does
/// not print. A service outside the secret's scope gets `NotFound`, the same as
/// a secret that does not exist.
///
/// # Errors
///
/// [`CoreError::NotFound`] for an unknown, revoked or out-of-scope secret;
/// [`CoreError::CapabilityUnavailable`] without a sealing root.
pub async fn use_secret(
    pool: &PgPool,
    key: Option<&SealingKey>,
    organisation_id: Uuid,
    id: Uuid,
    scope: SecretScope,
    ids: &CorrelationIds,
) -> CoreResult<Secret> {
    let raiz = root(key)?;
    let mut tx = pool.begin().await?;
    let (nonce, ciphertext) = repo::take_for_use(&mut *tx, organisation_id, id, scope.as_str())
        .await?
        .ok_or_else(|| CoreError::NotFound("Segredo não encontrado.".to_owned()))?;
    let claro = sealed::open(
        raiz,
        SealingDomain::InstanceSecrets,
        &Sealed { nonce, ciphertext },
    )?;
    audit::record(
        &mut tx,
        None,
        ids,
        AuditEntry::new(action::ADMIN_OPERATION, "instance_secret")
            .resource(id)
            .detail("event", "used")
            .detail("scope", scope.as_str()),
    )
    .await?;
    tx.commit().await?;
    Ok(Secret::new(claro))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pista_so_aparece_quando_nao_entrega_o_segredo() {
        assert_eq!(
            validate(&Secret::new("sk-0123456789abcdef7f2a")).expect("ok"),
            "7f2a"
        );
        assert_eq!(validate(&Secret::new("curta")).expect("ok"), "");
        assert!(validate(&Secret::new("   ")).is_err());
        assert!(validate(&Secret::new("com\nquebra")).is_err());
    }
}
