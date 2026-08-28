//! Intelligence persistence.

use sqlx::PgExecutor;
use uuid::Uuid;

use super::model::RegisteredModel;
use crate::error::CoreResult;

const MODEL_COLUMNS: &str = "id, provider_kind, provider_name, node_id, model_name, version,
                             capabilities, context_limit, status, max_classification,
                             enabled, reported_at";

/// List registered models.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn list_models<'e>(executor: impl PgExecutor<'e>) -> CoreResult<Vec<RegisteredModel>> {
    let models = sqlx::query_as::<_, RegisteredModel>(&format!(
        "SELECT {MODEL_COLUMNS} FROM ai_models ORDER BY provider_name, model_name, version"
    ))
    .fetch_all(executor)
    .await?;
    Ok(models)
}

/// Record an AI job.
///
/// # Errors
///
/// Returns an error when the insert fails.
#[expect(
    clippy::too_many_arguments,
    reason = "um parâmetro por coluna: a alternativa é uma struct que só existe para atravessar esta chamada"
)]
pub async fn insert_job<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    workspace_id: Option<Uuid>,
    requested_by: Uuid,
    capability: &str,
    model_id: Option<Uuid>,
    scope: &str,
    status: &str,
    rejection_reason: Option<&str>,
    retrieved_refs: &serde_json::Value,
) -> CoreResult<Uuid> {
    let id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO ai_jobs
             (organisation_id, workspace_id, requested_by_id, capability, model_id,
              scope, status, rejection_reason, retrieved_refs)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
         RETURNING id",
    )
    .bind(organisation_id)
    .bind(workspace_id)
    .bind(requested_by)
    .bind(capability)
    .bind(model_id)
    .bind(scope)
    .bind(status)
    .bind(rejection_reason)
    .bind(retrieved_refs)
    .fetch_one(executor)
    .await?;
    Ok(id)
}
