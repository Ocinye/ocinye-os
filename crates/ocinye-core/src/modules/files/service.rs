//! As duas operações que criam e fazem crescer um ficheiro institucional.

use ocinye_contracts::Classification;
use ocinye_domain::policy::{authorize, Action, ResourceContext, ResourceKind};
use ocinye_domain::Principal;
use ocinye_observability::CorrelationIds;
use uuid::Uuid;

use super::repository::{self as repo, FileRecord};
use crate::audit::{action, AuditEntry};
use crate::error::{CoreError, CoreResult};
use crate::storage::ObjectStore;
use crate::{audit, Tx};

/// Uma versão, tal como ficou registada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileVersionRecord {
    /// O ficheiro a que pertence.
    pub file_id: Uuid,
    /// A identidade da versão. É isto que a proveniência aponta.
    pub version_id: Uuid,
    /// O número. A corrente é a maior.
    pub sequence: i32,
    /// Os bytes desta versão.
    pub storage_object_id: Uuid,
}

/// Onde o ficheiro vive, e sob que classificação.
///
/// Agrupado porque estas quatro coisas viajam sempre juntas e sozinhas não
/// significam nada: a autorização precisa das três primeiras para resolver
/// papéis, e da quarta para saber contra que nível decidir.
#[derive(Debug, Clone, Copy)]
pub struct FileContext {
    /// A organização.
    pub organisation_id: Uuid,
    /// A unidade. Não pode discordar da do ambiente — a base impede-o.
    pub unit_id: Uuid,
    /// O ambiente de investigação que o contém.
    pub workspace_id: Uuid,
    /// A classificação do artefacto. Combina-se com a do ambiente pela mais
    /// restritiva das duas, e nunca a substitui.
    pub classification: Classification,
}

/// Cria um ficheiro e a sua primeira versão.
///
/// Chamado dentro da transacção de quem cria o recurso que interpreta o
/// ficheiro — hoje, o documento. Assim, ou nascem os quatro, ou não nasce
/// nenhum: um documento parcialmente versionado seria pior do que um documento
/// sem versões, porque pareceria completo.
///
/// # Errors
///
/// Devolve erro quando a inserção falha.
pub async fn create_with_first_version(
    tx: &mut Tx<'_>,
    contexto: FileContext,
    name: &str,
    storage_object_id: Uuid,
    created_by: Uuid,
) -> CoreResult<FileVersionRecord> {
    let file_id = repo::insert_file(
        &mut **tx,
        contexto.organisation_id,
        contexto.unit_id,
        contexto.workspace_id,
        name,
        contexto.classification,
        created_by,
    )
    .await?;
    let version_id =
        repo::insert_version(&mut **tx, file_id, 1, storage_object_id, None, created_by).await?;

    Ok(FileVersionRecord {
        file_id,
        version_id,
        sequence: 1,
        storage_object_id,
    })
}

/// Acrescenta uma versão a um ficheiro que já existe.
///
/// # A operação que dá sentido a tudo isto
///
/// Nunca substitui. A versão anterior continua a existir e a apontar
/// exactamente para os mesmos bytes, e é isso que permite que uma citação
/// feita há dois anos continue a dizer a verdade.
///
/// O número é determinado **aqui**, e não por quem chama: deixar o cliente
/// escolher a sequência seria deixá-lo decidir qual é a versão corrente.
///
/// # Errors
///
/// Devolve erro quando o ficheiro não existe ou quando a inserção falha.
pub async fn add_version(
    tx: &mut Tx<'_>,
    file_id: Uuid,
    storage_object_id: Uuid,
    note: Option<&str>,
    created_by: Uuid,
) -> CoreResult<FileVersionRecord> {
    let Some(sequence) = repo::next_sequence(tx, file_id).await? else {
        return Err(CoreError::NotFound("O ficheiro não existe.".to_owned()));
    };

    let version_id = repo::insert_version(
        &mut **tx,
        file_id,
        sequence,
        storage_object_id,
        note,
        created_by,
    )
    .await?;

    Ok(FileVersionRecord {
        file_id,
        version_id,
        sequence,
        storage_object_id,
    })
}

/// A versão corrente: a de maior sequência.
///
/// # Errors
///
/// Devolve erro quando a consulta falha.
pub async fn current_version(tx: &mut Tx<'_>, file_id: Uuid) -> CoreResult<Option<(Uuid, i32)>> {
    repo::current_storage_object(&mut **tx, file_id).await
}

// ── O ficheiro institucional genérico ───────────────────────────────────
//
// Até aqui um ficheiro só nascia por baixo de um documento. Esta é a operação
// que o faz existir por direito próprio: uma fotografia de uma montagem
// experimental é um artefacto institucional legítimo, versionável, governado e
// descarregável — e **não** é um documento de conhecimento.
//
// > **Carregar um ficheiro não é o mesmo que afirmar conhecimento
// > institucional.**
//
// O que se carrega fica guardado, classificado e ligado a um ambiente. O que
// significa institucionalmente é uma segunda decisão, de quem a tomar.

/// O que se pede para criar um ficheiro institucional.
pub struct NewFile {
    /// O nome tal como veio de quem carregou.
    pub filename: String,
    /// O tipo declarado. É validado contra a lista de permissões.
    pub content_type: String,
    /// Os bytes.
    pub data: Vec<u8>,
    /// A classificação pedida. O Core aplica a do ambiente por cima, pela mais
    /// restritiva das duas — pedir menos do que o ambiente permite não baixa
    /// a protecção.
    pub classification: Option<Classification>,
}

/// O contexto de autorização de um ficheiro.
///
/// A **mesma composição** que a leitura de um documento usa hoje: a
/// classificação efectiva é a mais restritiva entre a do ambiente e a do
/// artefacto. Trocar de representante — de `Document` para `File` — não muda a
/// política, e é essa a razão de esta função existir em vez de uma regra nova.
#[must_use]
pub fn file_context(
    workspace: &crate::modules::research::ResearchWorkspace,
    classification: Classification,
) -> ResourceContext {
    crate::modules::research::artefact_context(workspace, ResourceKind::File, classification)
}

/// Cria um ficheiro institucional, sem lhe atribuir significado nenhum.
///
/// # Errors
///
/// Devolve erro quando o ambiente não é alcançável, quando a autorização
/// recusa, quando o conteúdo é inválido, ou quando o armazenamento falha.
pub async fn create(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    store: &ObjectStore,
    organisation_slug: &str,
    workspace_id: Uuid,
    request: NewFile,
) -> CoreResult<FileVersionRecord> {
    let workspace =
        crate::modules::research::get_workspace(&mut **tx, principal, workspace_id).await?;
    let classification = workspace
        .classification()
        .most_restrictive(request.classification.unwrap_or(Classification::DEFAULT));

    // Autoriza contra a classificação **efectiva**, e não contra a pedida: um
    // pedido de `PUBLIC` dentro de um ambiente `RESTRICTED` não pode ser
    // avaliado como se fosse público.
    authorize(
        principal,
        Action::Create,
        &file_context(&workspace, classification),
    )
    .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    let objecto = guardar_bytes(
        tx,
        principal,
        store,
        organisation_slug,
        &workspace,
        classification,
        &request.filename,
        &request.content_type,
        request.data,
    )
    .await?;

    let ficheiro = create_with_first_version(
        tx,
        FileContext {
            organisation_id: principal.organisation_id,
            unit_id: workspace.unit_id,
            workspace_id: workspace.id,
            classification,
        },
        &objecto.filename,
        objecto.object_id,
        principal.person_id,
    )
    .await?;

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::CREATE, "file")
            .resource(ficheiro.file_id)
            .context(&file_context(&workspace, classification))
            .classified(classification)
            .detail("size_bytes", objecto.size.to_string()),
    )
    .await?;

    Ok(ficheiro)
}

/// Acrescenta uma versão a um ficheiro que já existe, com bytes novos.
///
/// # Errors
///
/// Devolve erro quando o ficheiro não é alcançável, quando a autorização
/// recusa, ou quando o armazenamento falha.
pub async fn upload_version(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    store: &ObjectStore,
    organisation_slug: &str,
    file_id: Uuid,
    request: NewFile,
) -> CoreResult<FileVersionRecord> {
    let (ficheiro, workspace) = get(tx, principal, file_id).await?;

    // Carregar uma versão é alterar o ficheiro, e é `Update` que o diz. Não se
    // reaproveita a autorização de leitura que `get` já fez.
    authorize(
        principal,
        Action::Update,
        &file_context(&workspace, ficheiro.classification()),
    )
    .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    // A classificação de uma versão nova é a do ficheiro. Uma versão não
    // reclassifica nada: mudar a protecção de um artefacto é outra operação,
    // com outro risco.
    let objecto = guardar_bytes(
        tx,
        principal,
        store,
        organisation_slug,
        &workspace,
        ficheiro.classification(),
        &request.filename,
        &request.content_type,
        request.data,
    )
    .await?;

    let versao = add_version(tx, file_id, objecto.object_id, None, principal.person_id).await?;

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::UPDATE, "file")
            .resource(file_id)
            .context(&file_context(&workspace, ficheiro.classification()))
            .classified(ficheiro.classification())
            .detail("sequence", versao.sequence.to_string()),
    )
    .await?;

    Ok(versao)
}

/// O que ficou guardado.
struct BytesGuardados {
    object_id: Uuid,
    filename: String,
    size: i64,
}

/// Valida, guarda e regista os bytes.
///
/// Espelha deliberadamente o caminho que os documentos já usavam: a mesma lista
/// de tipos aceites, a mesma soma, a mesma verificação de que existe
/// armazenamento registado **antes** do `put` — porque é aí que ainda não há
/// nada para limpar.
#[expect(
    clippy::too_many_arguments,
    reason = "cada argumento é uma decisão já tomada por quem chama; agrupá-los \
              num tipo esconderia que a classificação já foi normalizada"
)]
async fn guardar_bytes(
    tx: &mut Tx<'_>,
    principal: &Principal,
    store: &ObjectStore,
    organisation_slug: &str,
    workspace: &crate::modules::research::ResearchWorkspace,
    classification: Classification,
    filename: &str,
    content_type: &str,
    data: Vec<u8>,
) -> CoreResult<BytesGuardados> {
    if data.is_empty() {
        return Err(CoreError::Validation(
            "O ficheiro carregado está vazio.".to_owned(),
        ));
    }
    if data.len() as u64 > store.max_upload_bytes() {
        return Err(CoreError::Validation(
            "O ficheiro carregado excede o tamanho máximo permitido.".to_owned(),
        ));
    }

    let content_type = crate::storage::validate_content_type(content_type)?;
    let filename = crate::storage::normalise_filename(filename)?;
    let checksum = crate::storage::sha256_hex(&data);
    let size = i64::try_from(data.len())
        .map_err(|_| CoreError::Validation("O ficheiro é demasiado grande.".to_owned()))?;

    let object_id = Uuid::new_v4();
    let object_key = crate::storage::build_object_key(organisation_slug, workspace.id, object_id);

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
    .bind(&filename)
    .bind(&content_type)
    .bind(size)
    .bind(&checksum)
    .bind(classification.as_str())
    .bind(principal.person_id)
    .execute(&mut **tx)
    .await?;

    if registo.rows_affected() == 0 {
        return Err(CoreError::StorageUnavailable(
            "Esta instalação não tem armazenamento registado.".to_owned(),
        ));
    }

    store
        .put(&object_key, &content_type, &checksum, data)
        .await?;

    Ok(BytesGuardados {
        object_id,
        filename,
        size,
    })
}

/// Lê um ficheiro, com o ambiente que o governa.
///
/// # Errors
///
/// Devolve erro quando não existe ou quando a autorização recusa.
pub async fn get(
    executor: &mut sqlx::PgConnection,
    principal: &Principal,
    file_id: Uuid,
) -> CoreResult<(FileRecord, crate::modules::research::ResearchWorkspace)> {
    let ficheiro = repo::find_file(&mut *executor, file_id, principal.organisation_id)
        .await?
        .ok_or_else(|| CoreError::NotFound("Ficheiro não encontrado.".to_owned()))?;

    let workspace = crate::modules::research::readable_artefact_workspace(
        &mut *executor,
        principal,
        ficheiro.workspace_id,
        ResourceKind::File,
        ficheiro.classification(),
    )
    .await?;

    Ok((ficheiro, workspace))
}

/// Resolve uma **versão** exacta, autorizando pelo ficheiro que a governa.
///
/// # Porque isto não olha para a versão antes de autorizar
///
/// Porque a versão não tem autoridade nenhuma. Conhecer um identificador de
/// versão não pode ser uma forma de contornar o ficheiro que a contém: a
/// resolução vai da versão ao ficheiro, e é o ficheiro que decide.
///
/// É o mesmo padrão de `MethodologyVersion` e `DatasetVersion`. Uma versão
/// nunca ganha permissões mais permissivas do que o recurso pai.
///
/// # Errors
///
/// Devolve erro quando a versão não existe ou quando o ficheiro que a governa
/// recusa o acesso.
pub async fn get_version(
    executor: &mut sqlx::PgConnection,
    principal: &Principal,
    version_id: Uuid,
) -> CoreResult<(FileVersionRecord, FileRecord)> {
    let versao = repo::find_version(&mut *executor, version_id)
        .await?
        .ok_or_else(|| CoreError::NotFound("Versão não encontrada.".to_owned()))?;

    // A autorização é a do ficheiro, e corre **antes** de a versão ser
    // devolvida. Uma recusa aqui é indistinguível de a versão não existir, o
    // que é a resposta certa: quem não alcança o ficheiro não deve aprender
    // que a versão existe.
    let (ficheiro, _) = get(&mut *executor, principal, versao.file_id).await?;

    Ok((versao, ficheiro))
}

/// Uma ligação de descarga para a versão corrente de um ficheiro.
///
/// # Errors
///
/// Devolve erro quando o ficheiro não é alcançável, quando a autorização
/// recusa, ou quando o objecto não está disponível.
pub async fn download_url(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    store: &ObjectStore,
    file_id: Uuid,
) -> CoreResult<String> {
    let (ficheiro, workspace) = get(tx, principal, file_id).await?;

    // Explícito, e não herdado de `get`. A leitura e a descarga chegam à mesma
    // função de autorização com a **mesma** composição de classificação — e não
    // por coincidência de um chamador ter corrido um portão antes.
    authorize(
        principal,
        Action::Download,
        &file_context(&workspace, ficheiro.classification()),
    )
    .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    let (object_key, sequence) = repo::current_storage_object(&mut **tx, file_id)
        .await?
        .ok_or_else(|| CoreError::StorageUnavailable("O ficheiro não tem versões.".to_owned()))?;
    let _ = sequence;

    let (chave, nome) = repo::object_location(&mut **tx, object_key)
        .await?
        .ok_or_else(|| {
            CoreError::StorageUnavailable("Este objecto não está disponível.".to_owned())
        })?;

    let url = store.presigned_download(&chave, &nome).await?;

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::DOWNLOAD, "file")
            .resource(file_id)
            .context(&file_context(&workspace, ficheiro.classification()))
            .classified(ficheiro.classification()),
    )
    .await?;

    Ok(url)
}

// ── Pastas ──────────────────────────────────────────────────────────────
//
// > **Uma pasta é uma estrutura de navegação dentro de um contentor de
// > autoridade; mover um ficheiro entre contentores de autoridade não é uma
// > operação de pasta.**

/// Cria uma pasta dentro de um ambiente.
///
/// # Errors
///
/// Devolve erro quando o ambiente não é alcançável, quando a autorização
/// recusa, quando o nome é vazio, ou quando a pasta-mãe é de outro ambiente.
pub async fn create_folder(
    tx: &mut Tx<'_>,
    principal: &Principal,
    workspace_id: Uuid,
    parent_id: Option<Uuid>,
    name: &str,
) -> CoreResult<Uuid> {
    let workspace =
        crate::modules::research::get_workspace(&mut **tx, principal, workspace_id).await?;
    authorize(
        principal,
        Action::Create,
        &file_context(&workspace, workspace.classification()),
    )
    .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    let name = name.trim();
    if name.is_empty() {
        return Err(CoreError::Validation("A pasta precisa de nome.".to_owned()));
    }

    // A pasta-mãe tem de ser deste ambiente. Sem esta verificação, indicar a
    // identidade de uma pasta de outro ambiente construiria uma árvore que
    // atravessa fronteiras de autorização.
    if let Some(mae) = parent_id {
        let existente = repo::find_folder(&mut **tx, mae, principal.organisation_id)
            .await?
            .ok_or_else(|| CoreError::NotFound("Pasta não encontrada.".to_owned()))?;
        if existente.workspace_id != workspace_id {
            return Err(CoreError::Validation(
                "A pasta-mãe pertence a outro ambiente.".to_owned(),
            ));
        }
    }

    repo::insert_folder(
        &mut **tx,
        principal.organisation_id,
        workspace_id,
        parent_id,
        name,
        principal.person_id,
    )
    .await
}

/// O que uma pasta contém: pastas e ficheiros que quem pergunta alcança.
pub struct FolderContents {
    /// O caminho até à raiz, para migalhas.
    pub path: Vec<repo::FolderRecord>,
    /// As pastas imediatamente dentro.
    pub folders: Vec<repo::FolderRecord>,
    /// Os ficheiros, já filtrados pela visibilidade actual.
    pub files: Vec<repo::FileListing>,
}

/// Lista uma pasta, ou a raiz do ambiente.
///
/// # Errors
///
/// Devolve erro quando o ambiente não é alcançável ou a pasta não existe.
pub async fn browse(
    pool: &sqlx::PgPool,
    principal: &Principal,
    workspace_id: Uuid,
    folder_id: Option<Uuid>,
) -> CoreResult<FolderContents> {
    let workspace = crate::modules::research::get_workspace(pool, principal, workspace_id).await?;
    let _ = &workspace;

    let path = match folder_id {
        None => Vec::new(),
        Some(id) => {
            let pasta = repo::find_folder(pool, id, principal.organisation_id)
                .await?
                .ok_or_else(|| CoreError::NotFound("Pasta não encontrada.".to_owned()))?;
            if pasta.workspace_id != workspace_id {
                return Err(CoreError::NotFound("Pasta não encontrada.".to_owned()));
            }
            repo::folder_path(pool, id).await?
        }
    };

    let folders = repo::list_folders(pool, workspace_id, folder_id).await?;
    let filtro = ocinye_domain::policy::VisibilityFilter::for_principal(principal);
    let files = repo::list_files(pool, workspace_id, folder_id, &filtro).await?;

    Ok(FolderContents {
        path,
        folders,
        files,
    })
}

/// Move um ficheiro para outra pasta do **mesmo** ambiente.
///
/// # Porque mover não muda nada além do sítio
///
/// A pasta não tem classificação. Arrastar um artefacto `RESTRICTED` para uma
/// pasta chamada «Público» muda a navegação e mais nada — a protecção continua
/// onde estava, porque vive no ficheiro.
///
/// # Errors
///
/// Devolve erro quando o ficheiro não é alcançável, quando a autorização
/// recusa, ou quando a pasta de destino é de outro ambiente.
pub async fn move_to_folder(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    file_id: Uuid,
    folder_id: Option<Uuid>,
) -> CoreResult<()> {
    let (ficheiro, workspace) = get(tx, principal, file_id).await?;
    authorize(
        principal,
        Action::Update,
        &file_context(&workspace, ficheiro.classification()),
    )
    .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    if let Some(destino) = folder_id {
        let pasta = repo::find_folder(&mut **tx, destino, principal.organisation_id)
            .await?
            .ok_or_else(|| CoreError::NotFound("Pasta não encontrada.".to_owned()))?;
        // Atravessar ambientes não é organizar: é transferir, e transferir é
        // outra operação, com a sua própria decisão institucional.
        if pasta.workspace_id != ficheiro.workspace_id {
            return Err(CoreError::Validation(
                "Mover um ficheiro para outro ambiente não é uma operação de pasta.".to_owned(),
            ));
        }
    }

    repo::move_file(&mut **tx, file_id, folder_id).await?;

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::UPDATE, "file")
            .resource(file_id)
            .context(&file_context(&workspace, ficheiro.classification()))
            .classified(ficheiro.classification())
            .detail(
                "moved_to_folder",
                folder_id.map_or("raiz".to_owned(), |f| f.to_string()),
            ),
    )
    .await?;

    Ok(())
}
