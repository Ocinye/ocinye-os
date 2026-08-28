//! O SQL do ciclo científico.
//!
//! Sem decisões de autorização: este módulo lê e escreve, e o serviço decide.
//! Cada leitura devolve `Option`, e quem chama transforma a ausência na
//! resposta que a política manda dar.

use sqlx::PgExecutor;
use uuid::Uuid;

use super::model::{
    Hypothesis, Methodology, MethodologyVersion, Result as ScientificResult, ResultValidation,
    Study, StudyExecution,
};
use crate::error::CoreResult;

const HYPOTHESIS_COLUMNS: &str = "id, unit_id, workspace_id, project_id, statement, rationale, \
     status, classification, created_by_id, created_at, updated_at";

const METHODOLOGY_COLUMNS: &str = "id, unit_id, workspace_id, project_id, title, purpose, \
     classification, created_by_id, created_at, updated_at";

const VERSION_COLUMNS: &str = "id, methodology_id, sequence, label, summary, document_id, \
     status, superseded_by_id, published_at, created_by_id, created_at";

const STUDY_COLUMNS: &str = "id, unit_id, workspace_id, project_id, hypothesis_id, title, kind, \
     objective, status, classification, created_by_id, created_at, updated_at";

const EXECUTION_COLUMNS: &str = "id, study_id, sequence, status, started_at, finished_at, \
     compute_node_id, environment, software_name, software_version, software_commit, \
     image_digest, configuration, notes, created_by_id, created_at";

const RESULT_COLUMNS: &str = "id, unit_id, workspace_id, project_id, execution_id, title, \
     summary, status, classification, superseded_by_id, created_by_id, created_at, updated_at";

const VALIDATION_COLUMNS: &str = "id, result_id, kind, outcome, execution_id, \
     methodology_version_id, note, performed_by_id, created_at";

// ── Hipóteses ───────────────────────────────────────────────────────────

/// One hypothesis, within an organisation.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn find_hypothesis<'e>(
    executor: impl PgExecutor<'e>,
    id: Uuid,
    organisation_id: Uuid,
) -> CoreResult<Option<Hypothesis>> {
    let row = sqlx::query_as::<_, Hypothesis>(&format!(
        "SELECT {HYPOTHESIS_COLUMNS} FROM hypotheses WHERE id = $1 AND organisation_id = $2"
    ))
    .bind(id)
    .bind(organisation_id)
    .fetch_optional(executor)
    .await?;
    Ok(row)
}

/// Write a hypothesis.
///
/// # Errors
///
/// Returns an error when the insert fails.
#[allow(clippy::too_many_arguments)]
pub async fn insert_hypothesis<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    unit_id: Uuid,
    workspace_id: Option<Uuid>,
    project_id: Option<Uuid>,
    statement: &str,
    rationale: Option<&str>,
    classification: &str,
    created_by: Uuid,
) -> CoreResult<Hypothesis> {
    let row = sqlx::query_as::<_, Hypothesis>(&format!(
        "INSERT INTO hypotheses
             (organisation_id, unit_id, workspace_id, project_id, statement, rationale,
              classification, created_by_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         RETURNING {HYPOTHESIS_COLUMNS}"
    ))
    .bind(organisation_id)
    .bind(unit_id)
    .bind(workspace_id)
    .bind(project_id)
    .bind(statement)
    .bind(rationale)
    .bind(classification)
    .bind(created_by)
    .fetch_one(executor)
    .await?;
    Ok(row)
}

/// The hypotheses of a workspace.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn list_hypotheses<'e>(
    executor: impl PgExecutor<'e>,
    workspace_id: Uuid,
) -> CoreResult<Vec<Hypothesis>> {
    let rows = sqlx::query_as::<_, Hypothesis>(&format!(
        "SELECT {HYPOTHESIS_COLUMNS} FROM hypotheses
          WHERE workspace_id = $1 ORDER BY created_at DESC"
    ))
    .bind(workspace_id)
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

// ── Metodologias ────────────────────────────────────────────────────────

/// The methodologies of a research environment.
///
/// # Errors
///
/// Returns [`CoreError::Database`] on failure.
pub async fn list_methodologies<'e>(
    executor: impl PgExecutor<'e>,
    workspace_id: Uuid,
) -> CoreResult<Vec<Methodology>> {
    let rows = sqlx::query_as::<_, Methodology>(&format!(
        "SELECT {METHODOLOGY_COLUMNS} FROM methodologies
          WHERE workspace_id = $1 ORDER BY created_at DESC"
    ))
    .bind(workspace_id)
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

/// One methodology.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn find_methodology<'e>(
    executor: impl PgExecutor<'e>,
    id: Uuid,
    organisation_id: Uuid,
) -> CoreResult<Option<Methodology>> {
    let row = sqlx::query_as::<_, Methodology>(&format!(
        "SELECT {METHODOLOGY_COLUMNS} FROM methodologies WHERE id = $1 AND organisation_id = $2"
    ))
    .bind(id)
    .bind(organisation_id)
    .fetch_optional(executor)
    .await?;
    Ok(row)
}

/// Write a methodology.
///
/// # Errors
///
/// Returns an error when the insert fails.
#[allow(clippy::too_many_arguments)]
pub async fn insert_methodology<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    unit_id: Uuid,
    workspace_id: Option<Uuid>,
    project_id: Option<Uuid>,
    title: &str,
    purpose: Option<&str>,
    classification: &str,
    created_by: Uuid,
) -> CoreResult<Methodology> {
    let row = sqlx::query_as::<_, Methodology>(&format!(
        "INSERT INTO methodologies
             (organisation_id, unit_id, workspace_id, project_id, title, purpose,
              classification, created_by_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         RETURNING {METHODOLOGY_COLUMNS}"
    ))
    .bind(organisation_id)
    .bind(unit_id)
    .bind(workspace_id)
    .bind(project_id)
    .bind(title)
    .bind(purpose)
    .bind(classification)
    .bind(created_by)
    .fetch_one(executor)
    .await?;
    Ok(row)
}

/// One version, and the methodology it belongs to.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn find_version<'e>(
    executor: impl PgExecutor<'e> + Copy,
    id: Uuid,
    organisation_id: Uuid,
) -> CoreResult<Option<(MethodologyVersion, Methodology)>> {
    // Duas consultas, e não um `JOIN` para um tuplo.
    //
    // `query_as` sabe decodificar **uma** estrutura por linha; um par de duas
    // exigia uma terceira estrutura só para o transporte, e essa teria de ser
    // mantida em passo com as outras duas para sempre.
    let Some(version) = sqlx::query_as::<_, MethodologyVersion>(&format!(
        "SELECT {VERSION_COLUMNS} FROM methodology_versions WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(executor)
    .await?
    else {
        return Ok(None);
    };

    let methodology = find_methodology(executor, version.methodology_id, organisation_id).await?;
    Ok(methodology.map(|m| (version, m)))
}

/// Write the next version of a methodology.
///
/// The sequence is derived here, inside the same statement that writes it: a
/// number read and then written is a number two writers can read at the same
/// time.
///
/// # Errors
///
/// Returns an error when the insert fails.
/// A versão em vigor de uma metodologia: publicada, e por substituir.
///
/// # Errors
///
/// Returns [`CoreError::Database`] on failure.
pub async fn find_version_in_force<'e>(
    executor: impl PgExecutor<'e>,
    methodology_id: Uuid,
) -> CoreResult<Option<MethodologyVersion>> {
    let row = sqlx::query_as::<_, MethodologyVersion>(&format!(
        "SELECT {VERSION_COLUMNS} FROM methodology_versions
          WHERE methodology_id = $1
            AND status = 'published'
            AND superseded_by_id IS NULL
          ORDER BY sequence DESC LIMIT 1"
    ))
    .bind(methodology_id)
    .fetch_optional(executor)
    .await?;
    Ok(row)
}

/// Marca uma versão como substituída por outra.
///
/// # Errors
///
/// Returns [`CoreError::Database`] on failure.
pub async fn supersede_version<'e>(
    executor: impl PgExecutor<'e>,
    anterior: Uuid,
    nova: Uuid,
) -> CoreResult<()> {
    sqlx::query(
        "UPDATE methodology_versions
            SET status = 'superseded', superseded_by_id = $2
          WHERE id = $1",
    )
    .bind(anterior)
    .bind(nova)
    .execute(executor)
    .await?;
    Ok(())
}

/// Escreve uma versão nova de uma metodologia.
///
/// # Errors
///
/// Returns [`CoreError::Database`] on failure.
pub async fn insert_version<'e>(
    executor: impl PgExecutor<'e>,
    methodology_id: Uuid,
    label: &str,
    summary: &str,
    document_id: Option<Uuid>,
    created_by: Uuid,
) -> CoreResult<MethodologyVersion> {
    let row = sqlx::query_as::<_, MethodologyVersion>(&format!(
        "INSERT INTO methodology_versions
             (methodology_id, sequence, label, summary, document_id, status, published_at,
              created_by_id)
         SELECT $1,
                COALESCE(MAX(sequence), 0) + 1,
                $2, $3, $4, 'published', now(), $5
           FROM methodology_versions WHERE methodology_id = $1
         RETURNING {VERSION_COLUMNS}"
    ))
    .bind(methodology_id)
    .bind(label)
    .bind(summary)
    .bind(document_id)
    .bind(created_by)
    .fetch_one(executor)
    .await?;
    Ok(row)
}

/// The versions of a methodology, newest first.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn list_versions<'e>(
    executor: impl PgExecutor<'e>,
    methodology_id: Uuid,
) -> CoreResult<Vec<MethodologyVersion>> {
    let rows = sqlx::query_as::<_, MethodologyVersion>(&format!(
        "SELECT {VERSION_COLUMNS} FROM methodology_versions
          WHERE methodology_id = $1 ORDER BY sequence DESC"
    ))
    .bind(methodology_id)
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

// ── Estudos ─────────────────────────────────────────────────────────────

/// One study.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn find_study<'e>(
    executor: impl PgExecutor<'e>,
    id: Uuid,
    organisation_id: Uuid,
) -> CoreResult<Option<Study>> {
    let row = sqlx::query_as::<_, Study>(&format!(
        "SELECT {STUDY_COLUMNS} FROM studies WHERE id = $1 AND organisation_id = $2"
    ))
    .bind(id)
    .bind(organisation_id)
    .fetch_optional(executor)
    .await?;
    Ok(row)
}

/// Write a study.
///
/// # Errors
///
/// Returns an error when the insert fails.
#[allow(clippy::too_many_arguments)]
pub async fn insert_study<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    unit_id: Uuid,
    workspace_id: Option<Uuid>,
    project_id: Option<Uuid>,
    hypothesis_id: Option<Uuid>,
    title: &str,
    kind: &str,
    objective: Option<&str>,
    classification: &str,
    created_by: Uuid,
) -> CoreResult<Study> {
    let row = sqlx::query_as::<_, Study>(&format!(
        "INSERT INTO studies
             (organisation_id, unit_id, workspace_id, project_id, hypothesis_id, title, kind,
              objective, classification, created_by_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
         RETURNING {STUDY_COLUMNS}"
    ))
    .bind(organisation_id)
    .bind(unit_id)
    .bind(workspace_id)
    .bind(project_id)
    .bind(hypothesis_id)
    .bind(title)
    .bind(kind)
    .bind(objective)
    .bind(classification)
    .bind(created_by)
    .fetch_one(executor)
    .await?;
    Ok(row)
}

/// The studies of a workspace.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn list_studies<'e>(
    executor: impl PgExecutor<'e>,
    workspace_id: Uuid,
) -> CoreResult<Vec<Study>> {
    let rows = sqlx::query_as::<_, Study>(&format!(
        "SELECT {STUDY_COLUMNS} FROM studies
          WHERE workspace_id = $1 ORDER BY created_at DESC"
    ))
    .bind(workspace_id)
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

// ── Execuções ───────────────────────────────────────────────────────────

/// One execution, and the study it runs.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn find_execution<'e>(
    executor: impl PgExecutor<'e> + Copy,
    id: Uuid,
    organisation_id: Uuid,
) -> CoreResult<Option<(StudyExecution, Study)>> {
    let Some(execution) = sqlx::query_as::<_, StudyExecution>(&format!(
        "SELECT {EXECUTION_COLUMNS} FROM study_executions WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(executor)
    .await?
    else {
        return Ok(None);
    };

    let study = find_study(executor, execution.study_id, organisation_id).await?;
    Ok(study.map(|s| (execution, s)))
}

/// Record an execution.
///
/// # Errors
///
/// Returns an error when the insert fails.
#[allow(clippy::too_many_arguments)]
pub async fn insert_execution<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    study_id: Uuid,
    status: &str,
    compute_node_id: Option<Uuid>,
    environment: Option<&str>,
    software_name: Option<&str>,
    software_version: Option<&str>,
    software_commit: Option<&str>,
    notes: Option<&str>,
    created_by: Uuid,
) -> CoreResult<StudyExecution> {
    let row = sqlx::query_as::<_, StudyExecution>(&format!(
        "INSERT INTO study_executions
             (organisation_id, study_id, sequence, status, compute_node_id, environment,
              software_name, software_version, software_commit, notes, created_by_id)
         SELECT $1, $2,
                COALESCE(MAX(sequence), 0) + 1,
                $3, $4, $5, $6, $7, $8, $9, $10
           FROM study_executions WHERE study_id = $2
         RETURNING {EXECUTION_COLUMNS}"
    ))
    .bind(organisation_id)
    .bind(study_id)
    .bind(status)
    .bind(compute_node_id)
    .bind(environment)
    .bind(software_name)
    .bind(software_version)
    .bind(software_commit)
    .bind(notes)
    .bind(created_by)
    .fetch_one(executor)
    .await?;
    Ok(row)
}

/// The executions of a study, newest first.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn list_executions<'e>(
    executor: impl PgExecutor<'e>,
    study_id: Uuid,
) -> CoreResult<Vec<StudyExecution>> {
    let rows = sqlx::query_as::<_, StudyExecution>(&format!(
        "SELECT {EXECUTION_COLUMNS} FROM study_executions
          WHERE study_id = $1 ORDER BY sequence DESC"
    ))
    .bind(study_id)
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

// ── Resultados ──────────────────────────────────────────────────────────

/// One result.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn find_result<'e>(
    executor: impl PgExecutor<'e>,
    id: Uuid,
    organisation_id: Uuid,
) -> CoreResult<Option<ScientificResult>> {
    let row = sqlx::query_as::<_, ScientificResult>(&format!(
        "SELECT {RESULT_COLUMNS} FROM results WHERE id = $1 AND organisation_id = $2"
    ))
    .bind(id)
    .bind(organisation_id)
    .fetch_optional(executor)
    .await?;
    Ok(row)
}

/// Write a result.
///
/// # Errors
///
/// Returns an error when the insert fails.
#[allow(clippy::too_many_arguments)]
pub async fn insert_result<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    unit_id: Uuid,
    workspace_id: Option<Uuid>,
    project_id: Option<Uuid>,
    execution_id: Option<Uuid>,
    title: &str,
    summary: &str,
    classification: &str,
    created_by: Uuid,
) -> CoreResult<ScientificResult> {
    let row = sqlx::query_as::<_, ScientificResult>(&format!(
        "INSERT INTO results
             (organisation_id, unit_id, workspace_id, project_id, execution_id, title,
              summary, classification, created_by_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
         RETURNING {RESULT_COLUMNS}"
    ))
    .bind(organisation_id)
    .bind(unit_id)
    .bind(workspace_id)
    .bind(project_id)
    .bind(execution_id)
    .bind(title)
    .bind(summary)
    .bind(classification)
    .bind(created_by)
    .fetch_one(executor)
    .await?;
    Ok(row)
}

/// The results of a workspace.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn list_results<'e>(
    executor: impl PgExecutor<'e>,
    workspace_id: Uuid,
) -> CoreResult<Vec<ScientificResult>> {
    let rows = sqlx::query_as::<_, ScientificResult>(&format!(
        "SELECT {RESULT_COLUMNS} FROM results
          WHERE workspace_id = $1 ORDER BY created_at DESC"
    ))
    .bind(workspace_id)
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

/// Move a result to another status.
///
/// # Errors
///
/// Returns an error when the update fails.
pub async fn set_result_status<'e>(
    executor: impl PgExecutor<'e>,
    id: Uuid,
    status: &str,
) -> CoreResult<()> {
    sqlx::query("UPDATE results SET status = $2, updated_at = now() WHERE id = $1")
        .bind(id)
        .bind(status)
        .execute(executor)
        .await?;
    Ok(())
}

// ── Validações ──────────────────────────────────────────────────────────

/// Record a validation or a reproduction.
///
/// # Errors
///
/// Returns an error when the insert fails.
#[allow(clippy::too_many_arguments)]
pub async fn insert_validation<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    result_id: Uuid,
    kind: &str,
    outcome: &str,
    execution_id: Option<Uuid>,
    methodology_version_id: Option<Uuid>,
    note: Option<&str>,
    performed_by: Uuid,
) -> CoreResult<ResultValidation> {
    let row = sqlx::query_as::<_, ResultValidation>(&format!(
        "INSERT INTO result_validations
             (organisation_id, result_id, kind, outcome, execution_id,
              methodology_version_id, note, performed_by_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         RETURNING {VALIDATION_COLUMNS}"
    ))
    .bind(organisation_id)
    .bind(result_id)
    .bind(kind)
    .bind(outcome)
    .bind(execution_id)
    .bind(methodology_version_id)
    .bind(note)
    .bind(performed_by)
    .fetch_one(executor)
    .await?;
    Ok(row)
}

/// What has been said about a result.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn list_validations<'e>(
    executor: impl PgExecutor<'e>,
    result_id: Uuid,
) -> CoreResult<Vec<ResultValidation>> {
    let rows = sqlx::query_as::<_, ResultValidation>(&format!(
        "SELECT {VALIDATION_COLUMNS} FROM result_validations
          WHERE result_id = $1 ORDER BY created_at DESC"
    ))
    .bind(result_id)
    .fetch_all(executor)
    .await?;
    Ok(rows)
}
