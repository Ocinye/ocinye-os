//! Data application layer.

use chrono::NaiveDate;
use ocinye_contracts::{Classification, PageRequest};
use ocinye_domain::policy::{authorize, Action, ResourceKind, VisibilityFilter};
use ocinye_domain::Principal;
use ocinye_observability::CorrelationIds;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use super::model::{
    validate_version_label, Dataset, DatasetFile, DatasetOrigin, DatasetVersion, VersionStatus,
};
use super::repository as repo;
use crate::audit::{self, action, AuditEntry};
use crate::error::{CoreError, CoreResult};
use crate::modules::collaboration::{record_activity, ActivityKind};
use crate::modules::research::{get_workspace, readable_artefact_workspace, workspace_context};
use crate::modules::search;
use crate::outbox::{self, event};
use crate::storage::{self, ObjectStore};
use crate::Tx;

/// Details of a new dataset.
#[derive(Debug, Clone)]
pub struct NewDataset {
    /// Institutional code.
    pub code: String,
    /// Title.
    pub title: String,
    /// Description.
    pub description: Option<String>,
    /// Where it came from.
    pub origin: DatasetOrigin,
    /// Licence.
    pub licence: Option<String>,
    /// Contractual or ethical limits on use.
    pub usage_restrictions: Option<String>,
    /// Person accountable, defaulting to the cataloguer.
    pub responsible_person_id: Option<Uuid>,
    /// When it was acquired.
    pub acquisition_date: Option<NaiveDate>,
    /// Keywords.
    pub keywords: Vec<String>,
    /// Requested classification. Never widens the workspace's.
    pub classification: Option<Classification>,
}

/// Catalogue a dataset.
///
/// # Errors
///
/// Returns an error when the caller may not write, or the code is taken.
pub async fn create_dataset(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    workspace_id: Uuid,
    request: NewDataset,
) -> CoreResult<Dataset> {
    let workspace = get_workspace(&mut **tx, principal, workspace_id).await?;
    let ctx = workspace_context(&workspace, ResourceKind::Dataset);
    authorize(principal, Action::Create, &ctx)
        .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    let code = request.code.trim().to_ascii_uppercase();
    if code.is_empty() || code.len() > 64 {
        return Err(CoreError::Validation("A dataset needs a code.".to_owned()));
    }
    let title = request.title.trim();
    if title.is_empty() {
        return Err(CoreError::Validation("A dataset needs a title.".to_owned()));
    }
    if repo::code_taken(&mut **tx, principal.organisation_id, &code).await? {
        return Err(CoreError::Conflict(
            "A dataset with this code already exists.".to_owned(),
        ));
    }

    let classification = workspace
        .classification()
        .most_restrictive(request.classification.unwrap_or(Classification::DEFAULT));

    let dataset = repo::insert_dataset(
        &mut **tx,
        principal.organisation_id,
        workspace.unit_id,
        workspace.id,
        &code,
        title,
        request.description.as_deref(),
        request.origin.as_str(),
        request.licence.as_deref(),
        request.usage_restrictions.as_deref(),
        request.responsible_person_id.unwrap_or(principal.person_id),
        request.acquisition_date,
        &request.keywords,
        classification,
        principal.person_id,
    )
    .await?;

    search::index_entity(
        tx,
        search::IndexRequest {
            organisation_id: principal.organisation_id,
            unit_id: Some(workspace.unit_id),
            workspace_id: Some(workspace.id),
            entity_type: "dataset",
            entity_id: dataset.id,
            title: title.to_owned(),
            text: [
                Some(code.clone()),
                request.description.clone(),
                Some(request.keywords.join(" ")),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join("\n"),
            classification,
        },
    )
    .await?;

    outbox::emit(
        tx,
        event::DATASET_CREATED,
        "dataset",
        dataset.id,
        &ids.correlation_id,
        json!({ "workspace_id": workspace.id, "code": code }),
    )
    .await?;

    record_activity(
        tx,
        principal,
        workspace.id,
        workspace.unit_id,
        ActivityKind::Created,
        "dataset",
        Some(dataset.id),
        &format!("Dataset catalogued: {code}"),
        classification,
    )
    .await?;

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::CREATE, "dataset")
            .resource(dataset.id)
            .context(&ctx)
            .classified(classification)
            .detail("code", code.as_str())
            .detail("origin", request.origin.as_str()),
    )
    .await?;

    Ok(dataset)
}

/// Load a dataset the caller may read.
///
/// # Errors
///
/// Returns [`CoreError::NotFound`] when absent or not readable.
pub async fn get_dataset(
    pool: &PgPool,
    principal: &Principal,
    dataset_id: Uuid,
) -> CoreResult<Dataset> {
    let dataset = repo::find_dataset(pool, dataset_id, principal.organisation_id)
        .await?
        .ok_or_else(|| CoreError::NotFound("Dataset not found.".to_owned()))?;

    // Reading the dataset requires reading its workspace **and** clearing the
    // dataset's own classification, which may sit above the workspace's. A
    // dataset hidden from `list_datasets` must not be reachable by identifier.
    readable_artefact_workspace(
        pool,
        principal,
        dataset.workspace_id,
        ResourceKind::Dataset,
        dataset.classification(),
    )
    .await?;
    Ok(dataset)
}

/// List datasets the caller may read.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn list_datasets(
    pool: &PgPool,
    principal: &Principal,
    workspace_id: Option<Uuid>,
    page: PageRequest,
) -> CoreResult<(Vec<Dataset>, i64)> {
    // Mesma regra que nas tarefas: o identificador restringe, não autoriza.
    // A listagem institucional já exige o ambiente visível por linha; um pedido
    // com âmbito explícito exige que o próprio âmbito seja alcançável antes de
    // poder restringir seja o que for. São propriedades diferentes, e nenhuma
    // dispensa a outra.
    if let Some(workspace_id) = workspace_id {
        crate::modules::research::get_workspace(pool, principal, workspace_id).await?;
    }

    let filter = VisibilityFilter::for_principal(principal);
    if filter.is_never_satisfiable() {
        return Ok((Vec::new(), 0));
    }

    let datasets = repo::list_datasets(
        pool,
        principal.organisation_id,
        &filter,
        workspace_id,
        page.limit(),
        page.offset(),
    )
    .await?;
    let total =
        repo::count_datasets(pool, principal.organisation_id, &filter, workspace_id).await?;
    Ok((datasets, total))
}

/// Details of a new dataset version.
#[derive(Debug, Clone)]
pub struct NewVersion {
    /// Version label, for example `1.2`.
    pub label: String,
    /// Notes about this version.
    pub notes: Option<String>,
    /// How it was produced and from what.
    pub provenance: Option<String>,
    /// Version it was derived from.
    pub derived_from_version_id: Option<Uuid>,
}

/// Open a new draft version.
///
/// # Errors
///
/// Returns an error when the caller may not write, or the label is taken.
pub async fn create_version(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    dataset_id: Uuid,
    request: NewVersion,
) -> CoreResult<DatasetVersion> {
    let dataset = repo::find_dataset(&mut **tx, dataset_id, principal.organisation_id)
        .await?
        .ok_or_else(|| CoreError::NotFound("Dataset not found.".to_owned()))?;
    let workspace = get_workspace(&mut **tx, principal, dataset.workspace_id).await?;

    let ctx = workspace_context(&workspace, ResourceKind::Dataset)
        .with_classification(dataset.classification());
    authorize(principal, Action::Create, &ctx)
        .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    let label = validate_version_label(&request.label)?;
    if repo::version_label_taken(&mut **tx, dataset.id, &label).await? {
        return Err(CoreError::Conflict(
            "This dataset version already exists.".to_owned(),
        ));
    }

    let version = repo::insert_version(
        &mut **tx,
        dataset.id,
        &label,
        request.notes.as_deref(),
        request.provenance.as_deref(),
        request.derived_from_version_id,
        principal.person_id,
    )
    .await?;

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::CREATE, "dataset_version")
            .resource(version.id)
            .context(&ctx)
            .detail("dataset_id", dataset.id.to_string())
            .detail("label", label.as_str()),
    )
    .await?;

    Ok(version)
}

/// Add a file to a draft version.
///
/// # Errors
///
/// Returns an error when the version is not a draft, the caller may not write,
/// or validation fails.
#[expect(
    clippy::too_many_arguments,
    reason = "um parâmetro por coluna: a alternativa é uma struct que só existe para atravessar esta chamada"
)]
pub async fn add_version_file(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    store: &ObjectStore,
    organisation_slug: &str,
    dataset_id: Uuid,
    version_id: Uuid,
    path: &str,
    filename: &str,
    content_type: &str,
    data: Vec<u8>,
) -> CoreResult<Uuid> {
    let dataset = repo::find_dataset(&mut **tx, dataset_id, principal.organisation_id)
        .await?
        .ok_or_else(|| CoreError::NotFound("Dataset not found.".to_owned()))?;
    let workspace = get_workspace(&mut **tx, principal, dataset.workspace_id).await?;

    let ctx = workspace_context(&workspace, ResourceKind::Dataset)
        .with_classification(dataset.classification());
    authorize(principal, Action::Create, &ctx)
        .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    let version = repo::find_version(&mut **tx, dataset.id, version_id)
        .await?
        .ok_or_else(|| CoreError::NotFound("Dataset version not found.".to_owned()))?;

    if version.status() != VersionStatus::Draft {
        return Err(CoreError::Conflict(
            "Files can only be added to a draft version. Published versions are immutable."
                .to_owned(),
        ));
    }

    if data.is_empty() {
        return Err(CoreError::Validation(
            "The uploaded file is empty.".to_owned(),
        ));
    }
    if data.len() as u64 > store.max_upload_bytes() {
        return Err(CoreError::Validation(
            "The uploaded file exceeds the maximum permitted size.".to_owned(),
        ));
    }

    let content_type = storage::validate_content_type(content_type)?;
    // The logical path is metadata; it never reaches the object key.
    let logical_path = path
        .trim()
        .trim_start_matches('/')
        .chars()
        .take(512)
        .collect::<String>();
    if logical_path.is_empty() {
        return Err(CoreError::Validation(
            "A file needs a path inside the dataset.".to_owned(),
        ));
    }
    let safe_filename = storage::normalise_filename(filename)?;
    let checksum = storage::sha256_hex(&data);
    let size = i64::try_from(data.len())
        .map_err(|_| CoreError::Validation("The uploaded file is too large.".to_owned()))?;

    let object_id = Uuid::new_v4();
    let object_key = storage::build_object_key(organisation_slug, workspace.id, object_id);
    let classification = dataset.classification();

    let registo = sqlx::query(
        "INSERT INTO storage_objects
             (id, backend_id, organisation_id, unit_id, workspace_id, object_key,
              original_filename, content_type, size_bytes, checksum_sha256,
              classification, status, created_by_id)
         SELECT $1, b.id, $2, $3, $4, $5, $6, $7, $8, $9, $10, 'stored', $11
           FROM storage_backends b
          WHERE b.is_default AND b.is_active",
    )
    .bind(object_id)
    .bind(principal.organisation_id)
    .bind(workspace.unit_id)
    .bind(workspace.id)
    .bind(&object_key)
    .bind(&safe_filename)
    .bind(&content_type)
    .bind(size)
    .bind(&checksum)
    .bind(classification.as_str())
    .bind(principal.person_id)
    .execute(&mut **tx)
    .await?;

    // Sem backend por omissão, o `SELECT` não devolve linha nenhuma e o
    // `INSERT … SELECT` completa com zero — sem erro. O código seguia para o
    // `put`, escrevia o objecto no armazenamento institucional, e só depois
    // falhava numa chave estrangeira sobre um identificador que ninguém
    // reconhecia.
    //
    // Ficava um ficheiro no bucket que nada referenciava, e a causa real — esta
    // instalação não tem armazenamento registado — não aparecia em lado nenhum.
    //
    // A verificação vem antes do `put` porque é aí que ainda não há nada para
    // limpar.
    if registo.rows_affected() == 0 {
        return Err(CoreError::StorageUnavailable(
            "No default storage backend is registered on this deployment.".to_owned(),
        ));
    }

    store
        .put(&object_key, &content_type, &checksum, data)
        .await?;

    let file_id = repo::attach_file(
        tx,
        version.id,
        object_id,
        &logical_path,
        principal.person_id,
    )
    .await?;

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::UPLOAD, "dataset_file")
            .resource(file_id)
            .context(&ctx)
            .detail("version_id", version.id.to_string())
            .detail("size_bytes", size)
            .detail("checksum_sha256", checksum.as_str()),
    )
    .await?;

    Ok(file_id)
}

/// Publish a draft version, making it immutable.
///
/// # Errors
///
/// Returns an error when the version is not a draft, has no files, or the
/// caller may not publish it.
pub async fn publish_version(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    dataset_id: Uuid,
    version_id: Uuid,
) -> CoreResult<DatasetVersion> {
    let dataset = repo::find_dataset(&mut **tx, dataset_id, principal.organisation_id)
        .await?
        .ok_or_else(|| CoreError::NotFound("Dataset not found.".to_owned()))?;
    let workspace = get_workspace(&mut **tx, principal, dataset.workspace_id).await?;

    let ctx = workspace_context(&workspace, ResourceKind::Dataset)
        .with_classification(dataset.classification());
    authorize(principal, Action::Update, &ctx)
        .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    let version = repo::find_version(&mut **tx, dataset.id, version_id)
        .await?
        .ok_or_else(|| CoreError::NotFound("Dataset version not found.".to_owned()))?;

    if version.status() != VersionStatus::Draft {
        return Err(CoreError::Conflict(
            "Only a draft version can be published.".to_owned(),
        ));
    }
    if version.file_count == 0 {
        return Err(CoreError::Conflict(
            "A version cannot be published without files.".to_owned(),
        ));
    }

    repo::publish_version(&mut **tx, version.id, principal.person_id).await?;
    repo::activate_dataset(&mut **tx, dataset.id, principal.person_id).await?;

    outbox::emit(
        tx,
        event::DATASET_VERSIONED,
        "dataset",
        dataset.id,
        &ids.correlation_id,
        json!({ "version_id": version.id, "label": version.label }),
    )
    .await?;

    record_activity(
        tx,
        principal,
        workspace.id,
        workspace.unit_id,
        ActivityKind::Published,
        "dataset_version",
        Some(version.id),
        &format!(
            "Dataset {} version {} published",
            dataset.code, version.label
        ),
        dataset.classification(),
    )
    .await?;

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::PUBLISH, "dataset_version")
            .resource(version.id)
            .context(&ctx)
            .detail("label", version.label.as_str())
            .detail("file_count", version.file_count),
    )
    .await?;

    repo::find_version(&mut **tx, dataset.id, version.id)
        .await?
        .ok_or_else(|| CoreError::Internal("version vanished during publication".to_owned()))
}

/// List the versions of a dataset, with their files.
///
/// # Errors
///
/// Returns an error when the caller may not read the dataset.
pub async fn list_versions(
    pool: &PgPool,
    principal: &Principal,
    dataset_id: Uuid,
) -> CoreResult<Vec<(DatasetVersion, Vec<DatasetFile>)>> {
    let dataset = get_dataset(pool, principal, dataset_id).await?;
    let versions = repo::list_versions(pool, dataset.id).await?;

    let mut result = Vec::with_capacity(versions.len());
    for version in versions {
        let files = repo::list_files(pool, version.id).await?;
        result.push((version, files));
    }
    Ok(result)
}
