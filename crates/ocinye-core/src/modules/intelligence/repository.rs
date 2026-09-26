//! Intelligence persistence.

use sqlx::PgExecutor;
use uuid::Uuid;

use super::model::RegisteredModel;
use crate::error::CoreResult;

const MODEL_COLUMNS: &str = "m.id, m.provider_kind, m.provider_name, m.node_id, m.model_name,
                             m.version, m.capabilities, m.context_limit, m.status,
                             m.max_classification, m.enabled, m.reported_at";

/// List the models registered **in one instance**.
///
/// A model belongs to the instance of the node that reports it: `ai_models`
/// has no organisation of its own, so the scope comes through
/// `compute_nodes.organisation_id`. Reading the table whole let a node enrolled
/// in one instance serve another's prompts in a shared database (F-08 of the
/// pre-generalization baseline). A model with no node belongs to no instance and
/// is not listed; no code path writes one today.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn list_models<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
) -> CoreResult<Vec<RegisteredModel>> {
    let models = sqlx::query_as::<_, RegisteredModel>(&format!(
        "SELECT {MODEL_COLUMNS}
           FROM ai_models m
           JOIN compute_nodes n ON n.id = m.node_id
          WHERE n.organisation_id = $1
          ORDER BY m.provider_name, m.model_name, m.version"
    ))
    .bind(organisation_id)
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
