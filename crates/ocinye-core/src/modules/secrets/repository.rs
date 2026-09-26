//! Secrets persistence. Only this module reads or writes `instance_secrets`.

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgExecutor};
use uuid::Uuid;

use crate::error::CoreResult;

/// A secret's metadata — never its material.
#[derive(Debug, Clone, FromRow)]
pub struct SecretRow {
    pub id: Uuid,
    pub kind: String,
    pub label: String,
    pub scope: String,
    pub hint: Option<String>,
    pub version: i32,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub rotated_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
}

const METADATA: &str = "id, kind, label, scope, hint, version, status, created_at, rotated_at,
                        revoked_at, last_used_at";

#[expect(
    clippy::too_many_arguments,
    reason = "um parâmetro por coluna: a alternativa é uma struct que só existe para atravessar esta chamada"
)]
pub async fn insert<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    kind: &str,
    label: &str,
    scope: &str,
    sealed: (&[u8], &[u8]),
    hint: &str,
    created_by: Uuid,
) -> CoreResult<SecretRow> {
    Ok(sqlx::query_as::<_, SecretRow>(&format!(
        "INSERT INTO instance_secrets
             (organisation_id, kind, label, scope, nonce, ciphertext, hint, created_by_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         RETURNING {METADATA}"
    ))
    .bind(organisation_id)
    .bind(kind)
    .bind(label)
    .bind(scope)
    .bind(sealed.0)
    .bind(sealed.1)
    .bind(hint)
    .bind(created_by)
    .fetch_one(executor)
    .await?)
}

pub async fn list<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
) -> CoreResult<Vec<SecretRow>> {
    Ok(sqlx::query_as::<_, SecretRow>(&format!(
        "SELECT {METADATA} FROM instance_secrets
          WHERE organisation_id = $1 ORDER BY created_at"
    ))
    .bind(organisation_id)
    .fetch_all(executor)
    .await?)
}

pub async fn replace_material<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    id: Uuid,
    sealed: (&[u8], &[u8]),
    hint: &str,
) -> CoreResult<Option<SecretRow>> {
    Ok(sqlx::query_as::<_, SecretRow>(&format!(
        "UPDATE instance_secrets
            SET nonce = $3, ciphertext = $4, hint = $5, version = version + 1,
                rotated_at = now()
          WHERE organisation_id = $1 AND id = $2 AND status = 'active'
          RETURNING {METADATA}"
    ))
    .bind(organisation_id)
    .bind(id)
    .bind(sealed.0)
    .bind(sealed.1)
    .bind(hint)
    .fetch_optional(executor)
    .await?)
}

pub async fn revoke<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    id: Uuid,
) -> CoreResult<Option<SecretRow>> {
    Ok(sqlx::query_as::<_, SecretRow>(&format!(
        "UPDATE instance_secrets
            SET status = 'revoked', nonce = NULL, ciphertext = NULL, revoked_at = now()
          WHERE organisation_id = $1 AND id = $2 AND status = 'active'
          RETURNING {METADATA}"
    ))
    .bind(organisation_id)
    .bind(id)
    .fetch_optional(executor)
    .await?)
}

/// The sealed material of an active secret in a scope, marking it used.
pub async fn take_for_use<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    id: Uuid,
    scope: &str,
) -> CoreResult<Option<(Vec<u8>, Vec<u8>)>> {
    Ok(sqlx::query_as::<_, (Vec<u8>, Vec<u8>)>(
        "UPDATE instance_secrets SET last_used_at = now()
          WHERE organisation_id = $1 AND id = $2 AND scope = $3 AND status = 'active'
          RETURNING nonce, ciphertext",
    )
    .bind(organisation_id)
    .bind(id)
    .bind(scope)
    .fetch_optional(executor)
    .await?)
}
