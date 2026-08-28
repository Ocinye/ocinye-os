//! Data persistence.

use chrono::NaiveDate;
use ocinye_contracts::Classification;
use ocinye_domain::policy::VisibilityFilter;
use sqlx::PgExecutor;
use uuid::Uuid;

use super::model::{Dataset, DatasetFile, DatasetVersion};
use crate::error::CoreResult;
use crate::visibility::{contained_in_visible_workspace, to_sql, VisibilityColumns};

const DATASET_COLUMNS: &str = "id, unit_id, workspace_id, code, title, description, origin,
                               licence, usage_restrictions, responsible_person_id,
                               acquisition_date, keywords, classification, state, created_at";

const VERSION_COLUMNS: &str = "id, dataset_id, label, sequence, status, notes, provenance,
                               derived_from_version_id, total_size_bytes, file_count,
                               published_at, created_at";

/// Whether a dataset code is taken.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn code_taken<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    code: &str,
) -> CoreResult<bool> {
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM datasets WHERE organisation_id = $1 AND code = $2)",
    )
    .bind(organisation_id)
    .bind(code)
    .fetch_one(executor)
    .await?;
    Ok(exists)
}

/// Insert a dataset.
///
/// # Errors
///
/// Returns an error when the insert fails.
#[expect(
    clippy::too_many_arguments,
    reason = "um parâmetro por coluna: a alternativa é uma struct que só existe para atravessar esta chamada"
)]
pub async fn insert_dataset<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    unit_id: Uuid,
    workspace_id: Uuid,
    code: &str,
    title: &str,
    description: Option<&str>,
    origin: &str,
    licence: Option<&str>,
    usage_restrictions: Option<&str>,
    responsible_person_id: Uuid,
    acquisition_date: Option<NaiveDate>,
    keywords: &[String],
    classification: Classification,
    created_by: Uuid,
) -> CoreResult<Dataset> {
    let dataset = sqlx::query_as::<_, Dataset>(&format!(
        "INSERT INTO datasets
             (organisation_id, unit_id, workspace_id, code, title, description, origin,
              licence, usage_restrictions, responsible_person_id, acquisition_date,
              keywords, classification, state, created_by_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, 'draft', $14)
         RETURNING {DATASET_COLUMNS}"
    ))
    .bind(organisation_id)
    .bind(unit_id)
    .bind(workspace_id)
    .bind(code)
    .bind(title)
    .bind(description)
    .bind(origin)
    .bind(licence)
    .bind(usage_restrictions)
    .bind(responsible_person_id)
    .bind(acquisition_date)
    .bind(keywords)
    .bind(classification.as_str())
    .bind(created_by)
    .fetch_one(executor)
    .await?;
    Ok(dataset)
}

/// Load a dataset.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn find_dataset<'e>(
    executor: impl PgExecutor<'e>,
    id: Uuid,
    organisation_id: Uuid,
) -> CoreResult<Option<Dataset>> {
    let dataset = sqlx::query_as::<_, Dataset>(&format!(
        "SELECT {DATASET_COLUMNS} FROM datasets WHERE id = $1 AND organisation_id = $2"
    ))
    .bind(id)
    .bind(organisation_id)
    .fetch_optional(executor)
    .await?;
    Ok(dataset)
}

/// List datasets the principal may read.
///
/// # Errors
///
/// Returns an error when the query fails.
/// O predicado partilhado pela listagem de datasets e pela sua contagem.
///
/// Quando `workspace_id` é dado, a pergunta é «o que há neste ambiente», e o
/// próprio ambiente já foi autorizado por quem chamou. Quando é omitido, a
/// pergunta passa a ser institucional — e aí o ambiente que contém cada dataset
/// tem de ser visível também, senão a lista revela onde há trabalho a que o
/// membro não chega.
fn dataset_predicate(filter: &VisibilityFilter, institutional: bool) -> String {
    let artefacto = to_sql(filter, VisibilityColumns::default());
    if institutional {
        let contido = contained_in_visible_workspace(filter, "datasets");
        format!("{artefacto} AND {contido}")
    } else {
        artefacto
    }
}

pub async fn list_datasets<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    filter: &VisibilityFilter,
    workspace_id: Option<Uuid>,
    limit: i64,
    offset: i64,
) -> CoreResult<Vec<Dataset>> {
    let predicate = dataset_predicate(filter, workspace_id.is_none());
    let datasets = sqlx::query_as::<_, Dataset>(&format!(
        "SELECT {DATASET_COLUMNS} FROM datasets
          WHERE organisation_id = $1
            AND ($2::uuid IS NULL OR workspace_id = $2)
            AND {predicate}
          ORDER BY code
          LIMIT $3 OFFSET $4"
    ))
    .bind(organisation_id)
    .bind(workspace_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(executor)
    .await?;
    Ok(datasets)
}

/// Count datasets the principal may read.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn count_datasets<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    filter: &VisibilityFilter,
    workspace_id: Option<Uuid>,
) -> CoreResult<i64> {
    let predicate = dataset_predicate(filter, workspace_id.is_none());
    let total = sqlx::query_scalar::<_, i64>(&format!(
        "SELECT COUNT(*) FROM datasets
          WHERE organisation_id = $1
            AND ($2::uuid IS NULL OR workspace_id = $2)
            AND {predicate}"
    ))
    .bind(organisation_id)
    .bind(workspace_id)
    .fetch_one(executor)
    .await?;
    Ok(total)
}

/// Activate a dataset that is still in draft.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn activate_dataset<'e>(
    executor: impl PgExecutor<'e>,
    dataset_id: Uuid,
    updated_by: Uuid,
) -> CoreResult<()> {
    sqlx::query(
        "UPDATE datasets SET state = 'active', updated_by_id = $2, updated_at = now()
          WHERE id = $1 AND state = 'draft'",
    )
    .bind(dataset_id)
    .bind(updated_by)
    .execute(executor)
    .await?;
    Ok(())
}

/// Whether a version label is taken within a dataset.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn version_label_taken<'e>(
    executor: impl PgExecutor<'e>,
    dataset_id: Uuid,
    label: &str,
) -> CoreResult<bool> {
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM dataset_versions WHERE dataset_id = $1 AND label = $2)",
    )
    .bind(dataset_id)
    .bind(label)
    .fetch_one(executor)
    .await?;
    Ok(exists)
}

/// Insert a draft version.
///
/// # Errors
///
/// Returns an error when the insert fails.
pub async fn insert_version<'e>(
    executor: impl PgExecutor<'e>,
    dataset_id: Uuid,
    label: &str,
    notes: Option<&str>,
    provenance: Option<&str>,
    derived_from_version_id: Option<Uuid>,
    created_by: Uuid,
) -> CoreResult<DatasetVersion> {
    let version = sqlx::query_as::<_, DatasetVersion>(&format!(
        "INSERT INTO dataset_versions
             (dataset_id, label, sequence, status, notes, provenance,
              derived_from_version_id, created_by_id)
         VALUES ($1, $2,
                 COALESCE((SELECT MAX(sequence) FROM dataset_versions WHERE dataset_id = $1), 0) + 1,
                 'draft', $3, $4, $5, $6)
         RETURNING {VERSION_COLUMNS}"
    ))
    .bind(dataset_id)
    .bind(label)
    .bind(notes)
    .bind(provenance)
    .bind(derived_from_version_id)
    .bind(created_by)
    .fetch_one(executor)
    .await?;
    Ok(version)
}

/// Load a version of a dataset.
///
/// # Errors
///
/// Returns an error when the query fails.
/// One version, by its own identifier.
///
/// Existe além de [`find_version`] porque a proveniência conhece a versão e
/// não o dataset: uma aresta aponta para `dataset_version`, e exigir também o
/// dataset obrigaria quem a percorre a saber de antemão a resposta que está a
/// procurar.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn find_version_by_id<'e>(
    executor: impl PgExecutor<'e>,
    version_id: Uuid,
) -> CoreResult<Option<DatasetVersion>> {
    let version = sqlx::query_as::<_, DatasetVersion>(&format!(
        "SELECT {VERSION_COLUMNS} FROM dataset_versions WHERE id = $1"
    ))
    .bind(version_id)
    .fetch_optional(executor)
    .await?;
    Ok(version)
}

pub async fn find_version<'e>(
    executor: impl PgExecutor<'e>,
    dataset_id: Uuid,
    version_id: Uuid,
) -> CoreResult<Option<DatasetVersion>> {
    let version = sqlx::query_as::<_, DatasetVersion>(&format!(
        "SELECT {VERSION_COLUMNS} FROM dataset_versions WHERE id = $1 AND dataset_id = $2"
    ))
    .bind(version_id)
    .bind(dataset_id)
    .fetch_optional(executor)
    .await?;
    Ok(version)
}

/// List the versions of a dataset, newest first.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn list_versions<'e>(
    executor: impl PgExecutor<'e>,
    dataset_id: Uuid,
) -> CoreResult<Vec<DatasetVersion>> {
    let versions = sqlx::query_as::<_, DatasetVersion>(&format!(
        "SELECT {VERSION_COLUMNS} FROM dataset_versions
          WHERE dataset_id = $1 ORDER BY sequence DESC"
    ))
    .bind(dataset_id)
    .fetch_all(executor)
    .await?;
    Ok(versions)
}

/// Attach a stored object to a draft version and update its counters.
///
/// # Errors
///
/// Returns an error when the insert or update fails.
pub async fn attach_file(
    tx: &mut crate::Tx<'_>,
    version_id: Uuid,
    storage_object_id: Uuid,
    path: &str,
    created_by: Uuid,
) -> CoreResult<Uuid> {
    let file_id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO dataset_files (version_id, storage_object_id, path, created_by_id)
         VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(version_id)
    .bind(storage_object_id)
    .bind(path)
    .bind(created_by)
    .fetch_one(&mut **tx)
    .await?;

    // Counters are recomputed from the files rather than incremented, so they
    // cannot drift if an insert is ever retried.
    sqlx::query(
        "UPDATE dataset_versions v
            SET file_count = agg.count, total_size_bytes = agg.total, updated_at = now()
           FROM (SELECT COUNT(*)::int AS count, COALESCE(SUM(o.size_bytes), 0) AS total
                   FROM dataset_files f
                   JOIN storage_objects o ON o.id = f.storage_object_id
                  WHERE f.version_id = $1) agg
          WHERE v.id = $1",
    )
    .bind(version_id)
    .execute(&mut **tx)
    .await?;

    Ok(file_id)
}

/// List the files of a version.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn list_files<'e>(
    executor: impl PgExecutor<'e>,
    version_id: Uuid,
) -> CoreResult<Vec<DatasetFile>> {
    let files = sqlx::query_as::<_, DatasetFile>(
        "SELECT f.id, f.version_id, f.path, o.size_bytes, o.checksum_sha256, o.content_type
           FROM dataset_files f
           JOIN storage_objects o ON o.id = f.storage_object_id
          WHERE f.version_id = $1
          ORDER BY f.path",
    )
    .bind(version_id)
    .fetch_all(executor)
    .await?;
    Ok(files)
}

/// Publish a draft version.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn publish_version<'e>(
    executor: impl PgExecutor<'e>,
    version_id: Uuid,
    updated_by: Uuid,
) -> CoreResult<()> {
    sqlx::query(
        "UPDATE dataset_versions
            SET status = 'published', published_at = now(),
                updated_by_id = $2, updated_at = now()
          WHERE id = $1 AND status = 'draft'",
    )
    .bind(version_id)
    .bind(updated_by)
    .execute(executor)
    .await?;
    Ok(())
}
