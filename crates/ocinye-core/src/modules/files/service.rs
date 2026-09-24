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
    ids: &CorrelationIds,
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

    // A fila de extracção nasce **aqui**, e não em quem chama.
    //
    // Estava a ser posta por quem carregava, e isso torna a invariante uma
    // coisa de que alguém se tem de lembrar: um caminho novo que criasse uma
    // versão sem pedir extracção produziria um ficheiro silenciosamente não
    // pesquisável, e nada acusaria. Dentro da mesma transacção, ou nascem as
    // duas ou não nasce nenhuma.
    super::extraction::queue(tx, version_id, ids).await?;

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
    ids: &CorrelationIds,
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

    // Pela mesma razão da primeira versão: a versão nova é outro corpo, e tem
    // de ser lida. A da v1 fica exactamente como estava.
    super::extraction::queue(tx, version_id, ids).await?;

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
        ids,
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

/// Cria um ficheiro **de uma pessoa** — para uma imagem ou anexo de uma nota.
///
/// Sem ambiente: o dono é a autoridade, e a classificação é a sua própria
/// (`INTERNAL`, como a nota), sem composição com um ambiente que não existe. Os
/// bytes atravessam a mesma validação de sempre — tipo permitido, tamanho, soma,
/// chave opaca —, e a extracção para pesquisa **não** é enfileirada aqui: a
/// pesquisa de conteúdo das notas é de uma fatia posterior, e indexar agora
/// arriscaria uma fuga de visibilidade antes de o filtro existir.
///
/// # Errors
///
/// Devolve erro quando o conteúdo é inválido ou quando o armazenamento falha.
pub async fn create_personal(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    store: &ObjectStore,
    organisation_slug: &str,
    request: NewFile,
) -> CoreResult<FileVersionRecord> {
    // Um ficheiro pessoal nasce INTERNAL. Não se pede menos — não há ambiente
    // por cima para o restringir, e um ficheiro pessoal PUBLIC por descuido seria
    // uma porta aberta sem ninguém a decidir abri-la.
    let classification = Classification::Internal;

    let objecto = guardar_bytes_personal(
        tx,
        principal,
        store,
        organisation_slug,
        &request.filename,
        &request.content_type,
        request.data,
    )
    .await?;

    let file_id = repo::insert_personal_file(
        &mut **tx,
        principal.organisation_id,
        principal.person_id,
        &objecto.filename,
        classification,
    )
    .await?;
    let version_id = repo::insert_version(
        &mut **tx,
        file_id,
        1,
        objecto.object_id,
        None,
        principal.person_id,
    )
    .await?;

    // Uma imagem pessoal pede a sua miniatura, na mesma transacção que a cria: a
    // grelha de Ficheiros mostra o conteúdo em vez de um ícone. O worker gera-a
    // depois; até lá, a grelha cai no ícone. A extracção de corpo continua a
    // **não** ser enfileirada aqui (ver acima) — a miniatura é visual, não texto
    // pesquisável, e não tem a mesma questão de visibilidade.
    if super::thumbnail::is_thumbnailable(&objecto.content_type) {
        super::thumbnail::queue(tx, version_id, ids).await?;
    }

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::CREATE, "file")
            .resource(file_id)
            .classified(classification)
            .detail("owner_id", principal.person_id.to_string())
            .detail("size_bytes", objecto.size.to_string()),
    )
    .await?;

    Ok(FileVersionRecord {
        file_id,
        version_id,
        sequence: 1,
        storage_object_id: objecto.object_id,
    })
}

/// Regista um objecto **já montado** no armazenamento como um ficheiro pessoal.
///
/// O gémeo de [`create_personal`] para o caminho por partes ([`super::upload`]):
/// os bytes já foram montados e verificados, por isso aqui não se transporta byte
/// nenhum — regista-se o objecto, admite-se a quota (a mesma admissão serializada
/// por membro que `guardar_bytes_personal` usa, e que cobre o `INSERT` que se
/// segue), e cria-se o ficheiro e a sua primeira versão.
///
/// # Errors
///
/// Recusa quando a quota não admite os bytes, quando a instalação não tem
/// armazenamento registado, ou quando a base falha.
#[allow(clippy::too_many_arguments)]
pub async fn finalise_personal_object(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    storage_object_id: Uuid,
    object_key: &str,
    filename: &str,
    content_type: &str,
    size_bytes: i64,
    checksum_sha256: &str,
) -> CoreResult<FileVersionRecord> {
    // Um ficheiro pessoal é sempre INTERNAL — como em `create_personal`.
    let classification = Classification::Internal;

    crate::modules::resource::admit_personal_bytes(tx, principal.person_id, size_bytes).await?;

    let registo = sqlx::query(
        "INSERT INTO storage_objects
             (id, backend_id, organisation_id, owner_id, object_key,
              original_filename, content_type, size_bytes, checksum_sha256,
              classification, status, created_by_id)
         SELECT $1, b.id, $2, $3, $4, $5, $6, $7, $8, $9, 'stored', $3
           FROM storage_backends b
          WHERE b.is_default AND b.is_active",
    )
    .bind(storage_object_id)
    .bind(principal.organisation_id)
    .bind(principal.person_id)
    .bind(object_key)
    .bind(filename)
    .bind(content_type)
    .bind(size_bytes)
    .bind(checksum_sha256)
    .bind(classification.as_str())
    .execute(&mut **tx)
    .await?;
    if registo.rows_affected() == 0 {
        return Err(CoreError::StorageUnavailable(
            "Esta instalação não tem armazenamento registado.".to_owned(),
        ));
    }

    let file_id = repo::insert_personal_file(
        &mut **tx,
        principal.organisation_id,
        principal.person_id,
        filename,
        classification,
    )
    .await?;
    let version_id = repo::insert_version(
        &mut **tx,
        file_id,
        1,
        storage_object_id,
        None,
        principal.person_id,
    )
    .await?;

    if super::thumbnail::is_thumbnailable(content_type) {
        super::thumbnail::queue(tx, version_id, ids).await?;
    }

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::CREATE, "file")
            .resource(file_id)
            .classified(classification)
            .detail("owner_id", principal.person_id.to_string())
            .detail("size_bytes", size_bytes.to_string())
            .detail("via", "chunked_upload"),
    )
    .await?;

    Ok(FileVersionRecord {
        file_id,
        version_id,
        sequence: 1,
        storage_object_id,
    })
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

    let versao = add_version(
        tx,
        ids,
        file_id,
        objecto.object_id,
        None,
        principal.person_id,
    )
    .await?;

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
pub struct BytesGuardados {
    /// O identificador do objecto guardado.
    pub object_id: Uuid,
    /// O nome de ficheiro normalizado (sem travessias nem controlo).
    pub filename: String,
    /// O tamanho em bytes.
    pub size: i64,
    /// O tipo validado (da lista de permissões), com `;charset` retirado.
    pub content_type: String,
    /// A soma SHA-256 dos bytes.
    pub checksum: String,
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
        content_type,
        checksum,
    })
}

/// Guarda bytes de um ficheiro **de uma pessoa** — ambiente nulo, dono presente.
///
/// A mesma validação de [`guardar_bytes`], com a chave do objecto keyed no dono
/// e o `owner_id` guardado na linha, para a governação saber de quem são os
/// bytes e o caminho não os misturar com os de um ambiente.
/// Guarda bytes pessoais de um membro e devolve o objecto guardado.
///
/// Exposta para além dos ficheiros pessoais — um anexo de correio é um objecto
/// pessoal do autor — para que quem guarda passe pela mesma fronteira: validação
/// de tipo, soma, chave opaca, dono, e **admissão de quota** (`admit_personal_bytes`,
/// ADR-0108). Não cria uma linha em `files`: devolve só o objecto, para o
/// chamador o referenciar onde precisar (§40).
pub async fn guardar_bytes_personal(
    tx: &mut Tx<'_>,
    principal: &Principal,
    store: &ObjectStore,
    organisation_slug: &str,
    filename: &str,
    content_type: &str,
    data: Vec<u8>,
) -> CoreResult<BytesGuardados> {
    // Um ficheiro pessoal é sempre INTERNAL — não há ambiente por cima, e a nota
    // que o contém também o é. Não é um argumento porque não há escolha a fazer.
    let classification = Classification::Internal;

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

    // Personal storage is governed: new personal bytes are admitted against the
    // member's quota, serialised per member so parallel uploads cannot both slip
    // past a full quota (ADR-0108). The advisory lock is held to the end of this
    // transaction, so the INSERT below is covered by the same admission.
    crate::modules::resource::admit_personal_bytes(tx, principal.person_id, size).await?;

    let object_id = Uuid::new_v4();
    let object_key = crate::storage::build_object_key_personal(
        organisation_slug,
        principal.person_id,
        object_id,
    );

    let registo = sqlx::query(
        "INSERT INTO storage_objects
             (id, backend_id, organisation_id, owner_id, object_key,
              original_filename, content_type, size_bytes, checksum_sha256,
              classification, status, created_by_id)
         SELECT $1, b.id, $2, $3, $4, $5, $6, $7, $8, $9, 'stored', $3
           FROM storage_backends b
          WHERE b.is_default AND b.is_active",
    )
    .bind(object_id)
    .bind(principal.organisation_id)
    .bind(principal.person_id)
    .bind(&object_key)
    .bind(&filename)
    .bind(&content_type)
    .bind(size)
    .bind(&checksum)
    .bind(classification.as_str())
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
        content_type,
        checksum,
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

/// Os bytes da versão corrente de um ficheiro, para **descarregar** same-origin.
///
/// A autorização é exactamente a da ligação assinada — `get` compõe a
/// classificação efectiva e `Action::Download` corre contra ela —, mas os bytes
/// saem pela origem do Workspace em vez de uma URL assinada. O armazenamento não
/// tem endpoint público, pelo que a URL assinada apontaria para o host interno,
/// que o browser não alcança; o Core transporta os bytes, e a localização do
/// armazenamento nunca chega à página (§26, §40, ADR-0608).
///
/// # Errors
///
/// Devolve erro quando o ficheiro não é alcançável, quando a autorização recusa,
/// ou quando o objecto não está disponível.
pub async fn read_download(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    store: &ObjectStore,
    file_id: Uuid,
) -> CoreResult<FileDownload> {
    let (ficheiro, workspace) = get(tx, principal, file_id).await?;

    authorize(
        principal,
        Action::Download,
        &file_context(&workspace, ficheiro.classification()),
    )
    .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    let linha: Option<(String, String, String, String)> = sqlx::query_as(
        "SELECT o.object_key, o.content_type, o.original_filename, o.checksum_sha256
           FROM file_versions v
           JOIN storage_objects o ON o.id = v.storage_object_id
          WHERE v.file_id = $1
          ORDER BY v.sequence DESC
          LIMIT 1",
    )
    .bind(file_id)
    .fetch_optional(&mut **tx)
    .await?;

    let (chave, tipo, nome, soma) = linha
        .ok_or_else(|| CoreError::StorageUnavailable("O ficheiro não tem versões.".to_owned()))?;

    let bytes = store.get(&chave).await?;

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

    Ok(FileDownload {
        content_type: tipo,
        filename: nome,
        bytes,
        checksum_sha256: soma,
    })
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
    /// Se quem está a navegar pode criar aqui.
    ///
    /// # Porque vem do Core, e não da lista de capacidades
    ///
    /// Porque a lista que o `/me` devolve é de âmbito institucional, e o
    /// direito de carregar um ficheiro é do ambiente: quem gere uma unidade
    /// carrega nos ambientes dela e em mais nenhum. Perguntar à lista
    /// institucional escondia o botão a quem o Core teria aceitado — um
    /// controlo ausente é tão enganador como um que não funciona.
    ///
    /// Continua a ser cortesia e não segurança: `create` volta a decidir.
    pub may_create: bool,
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

    // A mesma pergunta que `create` faz, contra a classificação do ambiente:
    // um ficheiro criado sem declarar classificação herda-a, e é essa a decisão
    // que o ecrã precisa de antecipar.
    let may_create = authorize(
        principal,
        Action::Create,
        &file_context(&workspace, workspace.classification()),
    )
    .is_ok();

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
        may_create,
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

/// Se este principal pode acrescentar uma versão a este ficheiro.
///
/// A mesma pergunta que `upload_version` faz, e feita aqui para que um ecrã
/// possa antecipá-la sem repetir a política fora do Core. Continua a ser
/// cortesia de renderização: `upload_version` volta a decidir, e é a decisão
/// dela que vale.
#[must_use]
pub fn may_write(
    principal: &Principal,
    workspace: &crate::modules::research::ResearchWorkspace,
    classification: Classification,
) -> bool {
    authorize(
        principal,
        Action::Update,
        &file_context(workspace, classification),
    )
    .is_ok()
}

/// O histórico de versões de um ficheiro.
///
/// A autorização é a do ficheiro, e corre antes de o histórico existir. Quem
/// não alcança o ficheiro não aprende quantas versões tem, nem quem as
/// carregou, nem quando — o histórico é informação sobre o ficheiro, e segue a
/// mesma autoridade.
///
/// # Errors
///
/// Devolve erro quando o ficheiro não é alcançável ou quando a autorização
/// recusa.
pub async fn versions(
    executor: &mut sqlx::PgConnection,
    principal: &Principal,
    file_id: Uuid,
) -> CoreResult<Vec<repo::VersionListing>> {
    let (ficheiro, _) = get(&mut *executor, principal, file_id).await?;
    repo::list_versions(&mut *executor, ficheiro.id).await
}

/// Uma ligação de descarga para **uma versão determinada**.
///
/// A versão corrente muda quando alguém carrega outra. Uma citação que aponte
/// para «o ficheiro» aponta, no dia seguinte, para bytes diferentes; é por isso
/// que descarregar uma versão exacta é uma operação própria e não um parâmetro
/// da outra.
///
/// A autoridade continua a ser a do ficheiro: a versão não tem classificação
/// própria e não abre nada que o ficheiro feche.
///
/// # Errors
///
/// Devolve erro quando a versão não é alcançável, quando a autorização recusa,
/// ou quando o objecto não está disponível.
pub async fn version_download_url(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    store: &ObjectStore,
    version_id: Uuid,
) -> CoreResult<String> {
    let (versao, ficheiro) = get_version(tx, principal, version_id).await?;

    // O ambiente relê-se aqui, e não se reaproveita o que `get_version`
    // possa ter visto: a composição da classificação decide-se contra o
    // estado corrente, no momento da descarga.
    let (_, workspace) = get(tx, principal, versao.file_id).await?;

    // Nota honesta sobre o que segue: hoje a política define `Download` como
    // «segue a autorização de leitura», por isso esta chamada **não pode**
    // recusar onde `get` já deixou passar — foi retirada numa reversão e o
    // teste continuou verde. Quem recusa é o `get` acima.
    //
    // Fica na mesma, e não por decoração: é aqui que a decisão é registada sob
    // a acção que realmente aconteceu, e é este o sítio já correcto no dia em
    // que `Download` divergir de `Read` — como `Export` já divergiu.

    authorize(
        principal,
        Action::Download,
        &file_context(&workspace, ficheiro.classification()),
    )
    .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    let (chave, nome) = repo::object_location(&mut **tx, versao.storage_object_id)
        .await?
        .ok_or_else(|| {
            CoreError::StorageUnavailable("Este objecto não está disponível.".to_owned())
        })?;

    let url = store.presigned_download(&chave, &nome).await?;

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::DOWNLOAD, "file_version")
            .resource(version_id)
            .context(&file_context(&workspace, ficheiro.classification()))
            .classified(ficheiro.classification()),
    )
    .await?;

    Ok(url)
}

/// Os bytes de uma **versão exacta** de um ficheiro, para descarregar
/// same-origin.
///
/// A autoridade é a do ficheiro que governa a versão (via `get_version`), com a
/// mesma composição de classificação de [`version_download_url`]; os bytes saem
/// pela origem do Workspace pela razão de sempre — o armazenamento não tem
/// endpoint público (§26, §40, ADR-0608). Abrir uma citação abre os bytes que
/// foram lidos, e não o que o ficheiro diz hoje.
///
/// # Errors
///
/// Devolve erro quando a versão não é alcançável, quando a autorização recusa,
/// ou quando o objecto não está disponível.
pub async fn read_version_download(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    store: &ObjectStore,
    version_id: Uuid,
) -> CoreResult<FileDownload> {
    let (versao, ficheiro) = get_version(tx, principal, version_id).await?;
    let (_, workspace) = get(tx, principal, versao.file_id).await?;

    authorize(
        principal,
        Action::Download,
        &file_context(&workspace, ficheiro.classification()),
    )
    .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    let linha: Option<(String, String, String, String)> = sqlx::query_as(
        "SELECT o.object_key, o.content_type, o.original_filename, o.checksum_sha256
           FROM file_versions v
           JOIN storage_objects o ON o.id = v.storage_object_id
          WHERE v.id = $1",
    )
    .bind(version_id)
    .fetch_optional(&mut **tx)
    .await?;

    let (chave, tipo, nome, soma) = linha.ok_or_else(|| {
        CoreError::StorageUnavailable("Este objecto não está disponível.".to_owned())
    })?;

    let bytes = store.get(&chave).await?;

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::DOWNLOAD, "file_version")
            .resource(version_id)
            .context(&file_context(&workspace, ficheiro.classification()))
            .classified(ficheiro.classification()),
    )
    .await?;

    Ok(FileDownload {
        content_type: tipo,
        filename: nome,
        bytes,
        checksum_sha256: soma,
    })
}

// ── Pré-visualização ────────────────────────────────────────────────────
//
// > **A Experience não precisa de conhecer nem confiar no endpoint físico onde
// > os bytes institucionais estão guardados.**
//
// A alternativa era pôr a URL do armazenamento dentro de um `<img>` e alargar a
// `Content-Security-Policy` do Workspace ao host de object storage. Isso faria a
// camada de experiência conhecer topologia de armazenamento, tornaria a CSP
// dependente do deployment, e acrescentaria uma origem externa à página.
//
// O Core transporta os bytes. É um custo real de largura de banda, e é a troca
// certa nesta fase: se um dia o débito justificar outra arquitectura,
// introduz-se um mecanismo dedicado e prova-se a fronteira outra vez.

/// Os tipos que se mostram inline.
///
/// # Porque não é `image/*`
///
/// Porque um SVG é um documento com script, e servi-lo inline na origem do
/// Workspace seria executá-lo lá. A lista é de formatos raster, e cresce por
/// decisão — não por alguém ter carregado um ficheiro novo.
pub const PREVIEWABLE_TYPES: [&str; 3] = ["image/png", "image/jpeg", "image/webp"];

/// O maior objecto que se transporta para mostrar inline.
///
/// Não é o limite do ficheiro: é o limite do que faz sentido atravessar o Core
/// para caber num ecrã.
pub const PREVIEW_MAX_BYTES: i64 = 16 * 1024 * 1024;

/// Uma representação inline autorizada de um ficheiro.
pub struct InlinePreview {
    /// O tipo, validado contra [`PREVIEWABLE_TYPES`] — nunca o que o cliente disse.
    pub content_type: String,
    /// Os bytes.
    pub bytes: Vec<u8>,
    /// A soma dos bytes guardados, para quem quiser derivar um `ETag`.
    pub checksum_sha256: String,
}

/// O maior objecto textual que se transporta para ler inline.
///
/// Um visualizador de texto ou de código não é um editor: mostra o suficiente
/// para se reconhecer o ficheiro, não o ficheiro inteiro a qualquer tamanho.
pub const TEXT_PREVIEW_MAX_BYTES: i64 = 512 * 1024;

/// Os tipos que se lêem inline como texto simples.
///
/// `text/*` cobre o código carregado como texto (um `.rs`, um `.py`, um
/// `.csv`). A lista fechada acrescenta os formatos estruturados de base
/// textual. **Nunca** se interpreta: o cliente escapa e mostra num `<pre>`,
/// pelo que um `text/html` é lido como o seu código-fonte, não renderizado.
pub fn is_textual_type(content_type: &str) -> bool {
    content_type.starts_with("text/")
        || matches!(
            content_type,
            "application/json"
                | "application/xml"
                | "application/javascript"
                | "application/x-yaml"
                | "application/toml"
        )
}

/// Uma representação textual autorizada de um ficheiro, para ler inline.
pub struct InlineText {
    /// O tipo guardado — nunca o que o cliente disse.
    pub content_type: String,
    /// O texto, já validado como UTF-8.
    pub text: String,
    /// A soma dos bytes guardados, para quem quiser derivar um `ETag`.
    pub checksum_sha256: String,
}

/// Os bytes de **uma versão determinada**, para mostrar inline.
///
/// A autoridade é a do ficheiro, reavaliada aqui: `get_version` resolve através
/// dele. Uma citação que aponte para a v2 mostra a imagem da v2.
///
/// # Errors
///
/// Devolve erro quando a versão não é alcançável, quando a autorização recusa,
/// quando o tipo não se mostra inline, ou quando o objecto não está disponível.
pub async fn preview_version(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    store: &ObjectStore,
    version_id: Uuid,
) -> CoreResult<InlinePreview> {
    let (versao, ficheiro) = get_version(tx, principal, version_id).await?;
    let (_, workspace) = get(tx, principal, versao.file_id).await?;

    authorize(
        principal,
        Action::Read,
        &file_context(&workspace, ficheiro.classification()),
    )
    .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    let linha: Option<(String, String, i64, String)> = sqlx::query_as(
        "SELECT o.object_key, o.content_type, o.size_bytes, o.checksum_sha256
           FROM file_versions v
           JOIN storage_objects o ON o.id = v.storage_object_id
          WHERE v.id = $1",
    )
    .bind(versao.version_id)
    .fetch_optional(&mut **tx)
    .await?;

    let (chave, tipo, tamanho, soma) = linha
        .ok_or_else(|| CoreError::StorageUnavailable("Esta versão não tem objecto.".to_owned()))?;

    if !PREVIEWABLE_TYPES.contains(&tipo.as_str()) {
        return Err(CoreError::Validation(
            "Este tipo não se mostra inline.".to_owned(),
        ));
    }
    if tamanho > PREVIEW_MAX_BYTES {
        return Err(CoreError::Validation(
            "Este ficheiro é grande de mais para mostrar inline.".to_owned(),
        ));
    }

    let bytes = store.get(&chave).await?;

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::PREVIEW, "file_version")
            .resource(version_id)
            .context(&file_context(&workspace, ficheiro.classification()))
            .classified(ficheiro.classification()),
    )
    .await?;

    Ok(InlinePreview {
        content_type: tipo,
        bytes,
        checksum_sha256: soma,
    })
}

/// Os bytes de uma versão de um ficheiro **de uma pessoa**, para mostrar inline.
///
/// A autoridade é a posse, reavaliada aqui: só o dono vê os bytes. Uma recusa é
/// indistinguível de a versão não existir — conhecer o identificador de uma
/// versão da imagem de outra pessoa não a abre (ADR-0413 §8). É por aqui que a
/// imagem de uma nota se serve, same-origin, sem nunca expor a chave do objecto.
///
/// # Errors
///
/// [`CoreError::NotFound`] quando a versão não é do dono, [`CoreError::Validation`]
/// quando o tipo não se mostra inline ou é grande de mais, e erro de
/// armazenamento quando o objecto não está disponível.
pub async fn read_version_preview(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    store: &ObjectStore,
    version_id: Uuid,
) -> CoreResult<InlinePreview> {
    // No ownership check here — the caller authorises (the file's owner, or a
    // note shared with them referencing it), because a shared note's image is
    // owned by the note's owner, not by the viewer. See the route.
    let versao = repo::find_version(&mut **tx, version_id)
        .await?
        .ok_or_else(|| CoreError::NotFound("Versão não encontrada.".to_owned()))?;

    let linha: Option<(String, String, i64, String)> = sqlx::query_as(
        "SELECT o.object_key, o.content_type, o.size_bytes, o.checksum_sha256
           FROM file_versions v
           JOIN storage_objects o ON o.id = v.storage_object_id
          WHERE v.id = $1",
    )
    .bind(versao.version_id)
    .fetch_optional(&mut **tx)
    .await?;

    let (chave, tipo, tamanho, soma) = linha
        .ok_or_else(|| CoreError::StorageUnavailable("Esta versão não tem objecto.".to_owned()))?;

    if !PREVIEWABLE_TYPES.contains(&tipo.as_str()) {
        return Err(CoreError::Validation(
            "Este tipo não se mostra inline.".to_owned(),
        ));
    }
    if tamanho > PREVIEW_MAX_BYTES {
        return Err(CoreError::Validation(
            "Este ficheiro é grande de mais para mostrar inline.".to_owned(),
        ));
    }

    let bytes = store.get(&chave).await?;

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::PREVIEW, "file_version")
            .resource(version_id)
            .classified(Classification::Internal),
    )
    .await?;

    Ok(InlinePreview {
        content_type: tipo,
        bytes,
        checksum_sha256: soma,
    })
}

/// Uma versão de ficheiro é de um ficheiro pessoal **deste** principal?
///
/// A fronteira que impede uma nota de referenciar a imagem de outra pessoa:
/// guardar o documento resolve cada `file_version_id` por aqui, e recusa o que
/// não for do dono. Devolve `false` quando a versão não existe, ou o ficheiro
/// tem ambiente (não é pessoal), ou é de outra pessoa.
///
/// # Errors
///
/// Devolve erro quando a consulta falha.
pub async fn owns_personal_file_version(
    executor: &mut sqlx::PgConnection,
    principal: &Principal,
    version_id: Uuid,
) -> CoreResult<bool> {
    let Some(versao) = repo::find_version(&mut *executor, version_id).await? else {
        return Ok(false);
    };
    let owner =
        repo::personal_file_owner(&mut *executor, versao.file_id, principal.organisation_id)
            .await?;
    Ok(owner == Some(principal.person_id))
}

// ── Meus ficheiros ──────────────────────────────────────────────────────
//
// > **Todo o membro activo tem um espaço de ficheiros pessoal.** Não exige
// > unidade, projecto nem ambiente de investigação — nem IA. A autoridade é o
// > dono; a quota é a do armazenamento pessoal já governado (ADR-0207).

pub use repo::PersonalFileListing;

/// A vista de «Meus ficheiros»: os ficheiros do dono, as suas pastas e o estado
/// do armazenamento pessoal.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PersonalFiles {
    /// Os ficheiros do dono, mais recentes primeiro.
    pub files: Vec<repo::PersonalFileListing>,
    /// As pastas do dono.
    pub folders: Vec<repo::PersonalFolder>,
    /// Usado, limite e disponível — a mesma medida de «Meus Recursos».
    pub storage: crate::modules::resource::PersonalStorageStatus,
}

/// Lista o espaço pessoal de quem pergunta: ficheiros, pastas e quota.
///
/// Não há permissão de ficheiros a exigir — um ficheiro pessoal é do dono, e a
/// consulta fecha-se sobre `person_id`. Um membro sem ambiente nenhum tem este
/// espaço na mesma: é o que faz «Meus ficheiros» existir sempre.
///
/// # Errors
///
/// Devolve erro quando uma das consultas falha.
pub async fn list_personal(
    pool: &sqlx::PgPool,
    principal: &Principal,
    folder: Option<Uuid>,
    limit: i64,
) -> CoreResult<PersonalFiles> {
    // Uma pasta pedida tem de ser do dono: pedir a pasta de outra pessoa não
    // devolve os ficheiros dela — devolve a raiz, como se a pasta não existisse.
    let folder = match folder {
        Some(id) => {
            let mut conn = pool.acquire().await?;
            if owns_personal_folder(&mut conn, principal, id).await? {
                Some(id)
            } else {
                None
            }
        }
        None => None,
    };
    let files = repo::list_personal_files(pool, principal.person_id, folder, limit).await?;
    let folders = repo::list_personal_folders(pool, principal.person_id).await?;
    let storage =
        crate::modules::resource::personal_storage_status(pool, principal, principal.person_id)
            .await?;
    Ok(PersonalFiles {
        files,
        folders,
        storage,
    })
}

/// Os ficheiros favoritos do membro, atravessando pastas — a vista «Favoritos».
///
/// # Errors
///
/// Devolve erro quando a consulta falha.
pub async fn list_personal_favourites(
    pool: &sqlx::PgPool,
    principal: &Principal,
    limit: i64,
) -> CoreResult<PersonalFiles> {
    let files = repo::list_personal_favourites(pool, principal.person_id, limit).await?;
    let folders = repo::list_personal_folders(pool, principal.person_id).await?;
    let storage =
        crate::modules::resource::personal_storage_status(pool, principal, principal.person_id)
            .await?;
    Ok(PersonalFiles {
        files,
        folders,
        storage,
    })
}

/// Os ficheiros recentes do membro, atravessando pastas — a vista «Recentes».
///
/// # Errors
///
/// Devolve erro quando a consulta falha.
pub async fn list_personal_recent(
    pool: &sqlx::PgPool,
    principal: &Principal,
    limit: i64,
) -> CoreResult<PersonalFiles> {
    let files = repo::list_personal_recent(pool, principal.person_id, limit).await?;
    let folders = repo::list_personal_folders(pool, principal.person_id).await?;
    let storage =
        crate::modules::resource::personal_storage_status(pool, principal, principal.person_id)
            .await?;
    Ok(PersonalFiles {
        files,
        folders,
        storage,
    })
}

/// Alterna a marca de favorito de um ficheiro do próprio, e devolve o estado
/// novo (`None` quando o ficheiro não é do dono ou está no Lixo).
///
/// # Errors
///
/// Devolve erro quando a escrita falha.
pub async fn toggle_personal_favourite(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    file_id: Uuid,
) -> CoreResult<Option<bool>> {
    let estado = repo::toggle_personal_favourite(tx, principal.person_id, file_id).await?;
    if let Some(marcado) = estado {
        audit::record(
            tx,
            Some(principal),
            ids,
            AuditEntry::new(action::UPDATE, "file")
                .resource(file_id)
                .detail("owner_id", principal.person_id.to_string())
                .detail("favourite", marcado.to_string()),
        )
        .await?;
    }
    Ok(estado)
}

/// Muda o nome de um ficheiro pessoal do próprio.
///
/// # Errors
///
/// [`CoreError::Validation`] quando o nome está vazio; [`CoreError::NotFound`]
/// quando o ficheiro não é do dono.
pub async fn rename_personal_file(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    file_id: Uuid,
    name: &str,
) -> CoreResult<()> {
    let name = name.trim();
    if name.is_empty() {
        return Err(CoreError::Validation(
            "O ficheiro precisa de um nome.".to_owned(),
        ));
    }
    let mudou = repo::rename_personal_file(&mut **tx, principal.person_id, file_id, name).await?;
    if !mudou {
        return Err(CoreError::NotFound("Ficheiro não encontrado.".to_owned()));
    }
    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::UPDATE, "file")
            .resource(file_id)
            .detail("owner_id", principal.person_id.to_string()),
    )
    .await?;
    Ok(())
}

/// Move um ficheiro pessoal para uma pasta do próprio, ou para a raiz.
///
/// A posse decide as duas pontas: o ficheiro e a pasta de destino têm de ser do
/// mesmo dono. Uma pasta que não seja sua responde «não encontrada» — não se
/// arruma um ficheiro na pasta de outra pessoa.
///
/// # Errors
///
/// [`CoreError::NotFound`] quando o ficheiro ou a pasta de destino não são do
/// dono.
pub async fn move_personal_file(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    file_id: Uuid,
    folder_id: Option<Uuid>,
) -> CoreResult<()> {
    if let Some(destino) = folder_id {
        if !owns_personal_folder(&mut *tx, principal, destino).await? {
            return Err(CoreError::NotFound("Pasta não encontrada.".to_owned()));
        }
    }
    let mudou =
        repo::move_personal_file(&mut **tx, principal.person_id, file_id, folder_id).await?;
    if !mudou {
        return Err(CoreError::NotFound("Ficheiro não encontrado.".to_owned()));
    }
    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::UPDATE, "file")
            .resource(file_id)
            .detail("owner_id", principal.person_id.to_string())
            .detail(
                "folder_id",
                folder_id.map_or_else(|| "raiz".to_owned(), |f| f.to_string()),
            ),
    )
    .await?;
    Ok(())
}

/// Os ficheiros no Lixo do próprio.
///
/// # Errors
///
/// Devolve erro quando a consulta falha.
pub async fn list_personal_trash(
    pool: &sqlx::PgPool,
    principal: &Principal,
    limit: i64,
) -> CoreResult<Vec<repo::PersonalFileListing>> {
    repo::list_personal_trash(pool, principal.person_id, limit).await
}

/// Põe um ficheiro pessoal no Lixo. Reversível: os bytes ficam, e a quota
/// continua a contá-los até ao apagar definitivo.
///
/// # Errors
///
/// [`CoreError::NotFound`] quando o ficheiro não é do dono.
pub async fn trash_personal_file(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    file_id: Uuid,
) -> CoreResult<()> {
    let mudou =
        repo::set_personal_file_deleted(&mut **tx, principal.person_id, file_id, true).await?;
    if !mudou {
        return Err(CoreError::NotFound("Ficheiro não encontrado.".to_owned()));
    }
    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::DELETE, "file")
            .resource(file_id)
            .detail("owner_id", principal.person_id.to_string())
            .detail("event", "trashed"),
    )
    .await?;
    Ok(())
}

/// Tira um ficheiro pessoal do Lixo.
///
/// # Errors
///
/// [`CoreError::NotFound`] quando o ficheiro não é do dono.
pub async fn restore_personal_file(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    file_id: Uuid,
) -> CoreResult<()> {
    let mudou =
        repo::set_personal_file_deleted(&mut **tx, principal.person_id, file_id, false).await?;
    if !mudou {
        return Err(CoreError::NotFound("Ficheiro não encontrado.".to_owned()));
    }
    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::UPDATE, "file")
            .resource(file_id)
            .detail("owner_id", principal.person_id.to_string())
            .detail("event", "restored"),
    )
    .await?;
    Ok(())
}

/// Apaga definitivamente um ficheiro pessoal: a linha e os bytes.
///
/// A metadata é apagada numa transacção; os bytes são removidos do
/// armazenamento **depois** de a transacção fechar. Se o armazenamento falhar,
/// fica um objecto órfão — nunca um ficheiro sem bytes —, e o registo de
/// auditoria diz o que aconteceu. Só se pode apagar definitivamente o que já
/// está no Lixo: um passo de cada vez.
///
/// # Errors
///
/// [`CoreError::NotFound`] quando o ficheiro não é do dono ou não está no Lixo.
pub async fn purge_personal_file(
    pool: &sqlx::PgPool,
    principal: &Principal,
    ids: &CorrelationIds,
    store: &ObjectStore,
    file_id: Uuid,
) -> CoreResult<()> {
    let mut tx = pool.begin().await?;

    // Só do Lixo: apagar definitivamente exige que o ficheiro já lá esteja.
    let no_lixo: Option<bool> = sqlx::query_scalar(
        "SELECT deleted_at IS NOT NULL FROM files WHERE id = $1 AND owner_id = $2",
    )
    .bind(file_id)
    .bind(principal.person_id)
    .fetch_optional(&mut *tx)
    .await?;
    if no_lixo != Some(true) {
        return Err(CoreError::NotFound(
            "Ficheiro não encontrado no Lixo.".to_owned(),
        ));
    }

    let mut chaves =
        repo::personal_file_object_keys(&mut *tx, principal.person_id, file_id).await?;
    // Os derivados (miniaturas) não saem com o objecto de origem — apagam-se
    // aqui, e as suas chaves juntam-se às que os bytes vão seguir.
    let miniaturas = super::thumbnail::purge_personal_thumbnail_objects(&mut tx, file_id).await?;
    chaves.extend(miniaturas);
    let apagou = repo::purge_personal_file(&mut tx, principal.person_id, file_id).await?;
    if !apagou {
        return Err(CoreError::NotFound("Ficheiro não encontrado.".to_owned()));
    }

    audit::record(
        &mut tx,
        Some(principal),
        ids,
        AuditEntry::new(action::DELETE, "file")
            .resource(file_id)
            .detail("owner_id", principal.person_id.to_string())
            .detail("event", "purged")
            .detail("objects", chaves.len().to_string()),
    )
    .await?;

    tx.commit().await?;

    // Os bytes, agora que a metadata caiu. `delete` é best-effort e não falha:
    // um objecto que fique para trás é órfão, não um ficheiro partido.
    for chave in &chaves {
        store.delete(chave).await;
    }

    Ok(())
}

/// Esvazia o Lixo pessoal: apaga definitivamente **tudo** o que lá está.
///
/// Cada ficheiro passa pelo mesmo [`purge_personal_file`] — mesma autoridade,
/// mesma ordem (metadados na transacção, bytes depois), mesma linha de auditoria
/// por ficheiro. Não há um caminho paralelo que apague em massa por baixo das
/// invariantes; há só este, a repetir a operação de um, para que esvaziar nunca
/// signifique menos garantias do que apagar um.
///
/// Devolve quantos ficheiros foram apagados. Um Lixo vazio devolve zero sem
/// tocar em nada. A operação é best-effort no seu todo: se um ficheiro falhar a
/// meio, os que já saíram ficaram saídos e o erro sobe — o Lixo não fica num
/// estado que ninguém pediu, só menos cheio.
///
/// # Errors
///
/// Propaga o erro de listar o Lixo, ou o de apagar um dos ficheiros.
pub async fn purge_all_personal_trash(
    pool: &sqlx::PgPool,
    principal: &Principal,
    ids: &CorrelationIds,
    store: &ObjectStore,
) -> CoreResult<u64> {
    // Sem teto artificial: quem esvazia o Lixo quer o Lixo todo, e uma página a
    // menos deixaria ficheiros para trás sem o dizer. O Lixo pessoal é do próprio
    // e é limitado pela sua quota, não por este número.
    let no_lixo = repo::list_personal_trash(pool, principal.person_id, i64::MAX).await?;

    let mut apagados: u64 = 0;
    for ficheiro in &no_lixo {
        purge_personal_file(pool, principal, ids, store, ficheiro.id).await?;
        apagados += 1;
    }

    Ok(apagados)
}

/// Uma ligação assinada de curta duração para a versão de um ficheiro pessoal.
///
/// A autoridade é a posse: `owns_personal_file_version` recusa a versão que não
/// for do dono — conhecer um identificador de versão não abre o ficheiro de
/// outra pessoa. Como em todo o lado, o Core assina e não transporta os bytes.
///
/// # Errors
///
/// [`CoreError::NotFound`] quando a versão não é do dono; erro quando o objecto
/// não está disponível.
pub async fn download_url_personal(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    store: &ObjectStore,
    version_id: Uuid,
) -> CoreResult<String> {
    if !owns_personal_file_version(&mut *tx, principal, version_id).await? {
        return Err(CoreError::NotFound("Ficheiro não encontrado.".to_owned()));
    }
    let versao = repo::find_version(&mut **tx, version_id)
        .await?
        .ok_or_else(|| CoreError::NotFound("Ficheiro não encontrado.".to_owned()))?;
    let (chave, nome) = repo::object_location(&mut **tx, versao.storage_object_id)
        .await?
        .ok_or_else(|| {
            CoreError::StorageUnavailable("Este objecto não está disponível.".to_owned())
        })?;
    let url = store.presigned_download(&chave, &nome).await?;

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::DOWNLOAD, "file_version")
            .resource(version_id)
            .detail("owner_id", principal.person_id.to_string()),
    )
    .await?;

    Ok(url)
}

/// O texto de uma versão de um ficheiro **de uma pessoa**, para ler inline.
///
/// A autoridade é a posse, reavaliada aqui: só o dono lê os bytes, e um
/// identificador de versão de outra pessoa responde «não encontrado». Serve
/// same-origin — o Workspace mostra o texto sem nunca aprender a chave do
/// objecto — e só tipos textuais ([`is_textual_type`]), com um tecto de
/// tamanho ([`TEXT_PREVIEW_MAX_BYTES`]).
///
/// # Errors
///
/// [`CoreError::NotFound`] quando a versão não é do dono; [`CoreError::Validation`]
/// quando o tipo não é textual, é grande de mais, ou os bytes não são UTF-8; e
/// erro de armazenamento quando o objecto não está disponível.
pub async fn read_version_text_personal(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    store: &ObjectStore,
    version_id: Uuid,
) -> CoreResult<InlineText> {
    if !owns_personal_file_version(&mut *tx, principal, version_id).await? {
        return Err(CoreError::NotFound("Ficheiro não encontrado.".to_owned()));
    }

    let linha: Option<(String, String, i64, String)> = sqlx::query_as(
        "SELECT o.object_key, o.content_type, o.size_bytes, o.checksum_sha256
           FROM file_versions v
           JOIN storage_objects o ON o.id = v.storage_object_id
          WHERE v.id = $1",
    )
    .bind(version_id)
    .fetch_optional(&mut **tx)
    .await?;

    let (chave, tipo, tamanho, soma) = linha
        .ok_or_else(|| CoreError::StorageUnavailable("Esta versão não tem objecto.".to_owned()))?;

    if !is_textual_type(&tipo) {
        return Err(CoreError::Validation(
            "Este tipo não se lê como texto.".to_owned(),
        ));
    }
    if tamanho > TEXT_PREVIEW_MAX_BYTES {
        return Err(CoreError::Validation(
            "Este ficheiro é grande de mais para ler inline.".to_owned(),
        ));
    }

    let bytes = store.get(&chave).await?;
    let text = String::from_utf8(bytes)
        .map_err(|_| CoreError::Validation("Este ficheiro não é texto legível.".to_owned()))?;

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::PREVIEW, "file_version")
            .resource(version_id)
            .detail("owner_id", principal.person_id.to_string()),
    )
    .await?;

    Ok(InlineText {
        content_type: tipo,
        text,
        checksum_sha256: soma,
    })
}

/// Uma descarga autorizada de um ficheiro pessoal, servida same-origin.
pub struct FileDownload {
    /// O tipo guardado.
    pub content_type: String,
    /// O nome com que o ficheiro foi carregado.
    pub filename: String,
    /// Os bytes.
    pub bytes: Vec<u8>,
    /// A soma dos bytes guardados.
    pub checksum_sha256: String,
}

/// Os bytes de uma versão pessoal, para **descarregar** same-origin.
///
/// A posse é a autoridade, reavaliada aqui: uma versão que não seja do dono
/// responde «não encontrado». Serve pela origem do Workspace em vez de uma URL
/// assinada porque o armazenamento não tem endpoint público — uma URL assinada
/// apontaria para o host interno, que o browser não alcança. Assim o Core
/// transporta os bytes desta descarga, e a localização do armazenamento nunca
/// chega à página (§26, §40).
///
/// # Errors
///
/// [`CoreError::NotFound`] quando a versão não é do dono; erro de armazenamento
/// quando o objecto não está disponível.
pub async fn read_version_download_personal(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    store: &ObjectStore,
    version_id: Uuid,
) -> CoreResult<FileDownload> {
    if !owns_personal_file_version(&mut *tx, principal, version_id).await? {
        return Err(CoreError::NotFound("Ficheiro não encontrado.".to_owned()));
    }

    let linha: Option<(String, String, String, String)> = sqlx::query_as(
        "SELECT o.object_key, o.content_type, o.original_filename, o.checksum_sha256
           FROM file_versions v
           JOIN storage_objects o ON o.id = v.storage_object_id
          WHERE v.id = $1",
    )
    .bind(version_id)
    .fetch_optional(&mut **tx)
    .await?;

    let (chave, tipo, nome, soma) = linha
        .ok_or_else(|| CoreError::StorageUnavailable("Esta versão não tem objecto.".to_owned()))?;

    let bytes = store.get(&chave).await?;

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::DOWNLOAD, "file_version")
            .resource(version_id)
            .detail("owner_id", principal.person_id.to_string()),
    )
    .await?;

    Ok(FileDownload {
        content_type: tipo,
        filename: nome,
        bytes,
        checksum_sha256: soma,
    })
}

/// Os tipos que o Quick Look mostra **inline** na origem do Workspace.
///
/// Os rasters já se serviam ([`PREVIEWABLE_TYPES`]); junta-se-lhes o PDF, que o
/// browser desenha no seu visualizador próprio, **fora do processo da página** —
/// pelo que o JavaScript embutido no documento não alcança a origem do
/// Workspace (o DOM, os cookies, os pedidos same-origin). O SVG continua de
/// fora — é um documento com script, e servi-lo inline seria executá-lo na
/// nossa origem. A rasterização por trabalhador isolado, mais estrita, fica
/// para o endurecimento do P3 (spec §76).
pub const INLINE_VIEWER_TYPES: [&str; 4] =
    ["image/png", "image/jpeg", "image/webp", "application/pdf"];

/// Os bytes de uma versão pessoal, para **ver inline** no Quick Look.
///
/// A posse é a autoridade, reavaliada aqui: uma versão que não seja do dono
/// responde «não encontrado». Serve same-origin, só a lista fechada
/// [`INLINE_VIEWER_TYPES`], até ao tecto de [`PREVIEW_MAX_BYTES`].
///
/// # Errors
///
/// [`CoreError::NotFound`] quando a versão não é do dono; [`CoreError::Validation`]
/// quando o tipo não se vê inline ou é grande de mais; erro de armazenamento
/// quando o objecto não está disponível.
pub async fn read_version_inline_personal(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    store: &ObjectStore,
    version_id: Uuid,
) -> CoreResult<InlinePreview> {
    if !owns_personal_file_version(&mut *tx, principal, version_id).await? {
        return Err(CoreError::NotFound("Ficheiro não encontrado.".to_owned()));
    }

    let linha: Option<(String, String, i64, String)> = sqlx::query_as(
        "SELECT o.object_key, o.content_type, o.size_bytes, o.checksum_sha256
           FROM file_versions v
           JOIN storage_objects o ON o.id = v.storage_object_id
          WHERE v.id = $1",
    )
    .bind(version_id)
    .fetch_optional(&mut **tx)
    .await?;

    let (chave, tipo, tamanho, soma) = linha
        .ok_or_else(|| CoreError::StorageUnavailable("Esta versão não tem objecto.".to_owned()))?;

    if !INLINE_VIEWER_TYPES.contains(&tipo.as_str()) {
        return Err(CoreError::Validation(
            "Este tipo não se mostra inline.".to_owned(),
        ));
    }
    if tamanho > PREVIEW_MAX_BYTES {
        return Err(CoreError::Validation(
            "Este ficheiro é grande de mais para mostrar inline.".to_owned(),
        ));
    }

    let bytes = store.get(&chave).await?;

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::PREVIEW, "file_version")
            .resource(version_id)
            .detail("owner_id", principal.person_id.to_string()),
    )
    .await?;

    Ok(InlinePreview {
        content_type: tipo,
        bytes,
        checksum_sha256: soma,
    })
}

// ── Pastas pessoais ─────────────────────────────────────────────────────

pub use repo::PersonalFolder;

/// Cria uma pasta de uma pessoa, para arrumar as suas notas.
///
/// # Errors
///
/// [`CoreError::Validation`] quando o nome está vazio ou já existe uma pasta do
/// dono com esse nome; erro quando a inserção falha.
pub async fn create_personal_folder(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    name: &str,
) -> CoreResult<PersonalFolder> {
    let name = name.trim();
    if name.is_empty() {
        return Err(CoreError::Validation(
            "A pasta precisa de um nome.".to_owned(),
        ));
    }
    // O índice único recusa um nome repetido do dono; traduz-se a recusa numa
    // mensagem do domínio em vez de um erro de base.
    if repo::list_personal_folders(&mut **tx, principal.person_id)
        .await?
        .iter()
        .any(|f| f.name.eq_ignore_ascii_case(name))
    {
        return Err(CoreError::Validation(
            "Já tem uma pasta com esse nome.".to_owned(),
        ));
    }

    let folder = repo::insert_personal_folder(
        &mut **tx,
        principal.organisation_id,
        principal.person_id,
        name,
    )
    .await?;

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::CREATE, "folder")
            .resource(folder.id)
            .detail("owner_id", principal.person_id.to_string()),
    )
    .await?;

    Ok(folder)
}

/// As pastas de uma pessoa.
///
/// # Errors
///
/// Devolve erro quando a consulta falha.
pub async fn list_personal_folders(
    pool: &sqlx::PgPool,
    principal: &Principal,
) -> CoreResult<Vec<PersonalFolder>> {
    repo::list_personal_folders(pool, principal.person_id).await
}

/// Uma pasta é de uma pessoa **deste** principal?
///
/// A fronteira que impede uma nota de referenciar a pasta de outra pessoa.
///
/// # Errors
///
/// Devolve erro quando a consulta falha.
pub async fn owns_personal_folder(
    executor: &mut sqlx::PgConnection,
    principal: &Principal,
    folder_id: Uuid,
) -> CoreResult<bool> {
    let owner =
        repo::personal_folder_owner(&mut *executor, folder_id, principal.organisation_id).await?;
    Ok(owner == Some(principal.person_id))
}

/// Apaga uma pasta pessoal do dono. As notas que lá estavam ficam sem pasta.
///
/// # Errors
///
/// [`CoreError::NotFound`] quando a pasta não é do dono; erro quando a consulta
/// falha.
pub async fn delete_personal_folder(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    folder_id: Uuid,
) -> CoreResult<()> {
    // Os ficheiros que lá estavam ficam na raiz — a pasta desaparece, o que
    // estava dentro não. Sem isto, a chave estrangeira `RESTRICT` recusaria
    // apagar uma pasta com ficheiros.
    repo::detach_personal_folder_files(&mut **tx, principal.person_id, folder_id).await?;
    let apagou = repo::delete_personal_folder(&mut **tx, principal.person_id, folder_id).await?;
    if !apagou {
        return Err(CoreError::NotFound("Pasta não encontrada.".to_owned()));
    }
    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::DELETE, "folder").resource(folder_id),
    )
    .await?;
    Ok(())
}

/// Muda o nome de uma pasta do dono.
///
/// # Errors
///
/// [`CoreError::Validation`] quando o nome está vazio ou já existe uma pasta do
/// dono com esse nome; [`CoreError::NotFound`] quando a pasta não é do dono.
pub async fn rename_personal_folder(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    folder_id: Uuid,
    name: &str,
) -> CoreResult<()> {
    let name = name.trim();
    if name.is_empty() {
        return Err(CoreError::Validation(
            "A pasta precisa de um nome.".to_owned(),
        ));
    }
    // Um nome repetido do próprio recusa-se com uma mensagem do domínio, e não
    // com um erro de base — exceptuando a própria pasta, que pode manter o nome.
    if repo::list_personal_folders(&mut **tx, principal.person_id)
        .await?
        .iter()
        .any(|f| f.id != folder_id && f.name.eq_ignore_ascii_case(name))
    {
        return Err(CoreError::Validation(
            "Já tem uma pasta com esse nome.".to_owned(),
        ));
    }
    let mudou =
        repo::rename_personal_folder(&mut **tx, principal.person_id, folder_id, name).await?;
    if !mudou {
        return Err(CoreError::NotFound("Pasta não encontrada.".to_owned()));
    }
    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::UPDATE, "folder").resource(folder_id),
    )
    .await?;
    Ok(())
}

/// Os bytes da versão corrente, para mostrar inline.
///
/// Não é uma descarga: é uma representação. A descarga continua a sair por
/// ligação assinada, e esta função não emite nenhuma — quem chama recebe bytes,
/// e não uma URL que sobreviva ao pedido.
///
/// # Errors
///
/// Devolve erro quando o ficheiro não é alcançável, quando a autorização
/// recusa, quando o tipo não se mostra inline, quando é grande de mais, ou
/// quando o objecto não está disponível.
pub async fn preview(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    store: &ObjectStore,
    file_id: Uuid,
) -> CoreResult<InlinePreview> {
    let (ficheiro, workspace) = get(tx, principal, file_id).await?;

    authorize(
        principal,
        Action::Read,
        &file_context(&workspace, ficheiro.classification()),
    )
    .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    let (chave, tipo, tamanho, soma) = repo::current_object_details(&mut **tx, file_id)
        .await?
        .ok_or_else(|| CoreError::StorageUnavailable("O ficheiro não tem versões.".to_owned()))?;

    // O tipo que decide é o que está guardado, e tem de estar na lista. Um
    // `content_type` que o Core não reconheça não se serve inline — servir-se-ia
    // na origem do Workspace, e é aí que um SVG passaria a ser script.
    if !PREVIEWABLE_TYPES.contains(&tipo.as_str()) {
        return Err(CoreError::Validation(
            "Este tipo não se mostra inline.".to_owned(),
        ));
    }
    if tamanho > PREVIEW_MAX_BYTES {
        return Err(CoreError::Validation(
            "Este ficheiro é grande de mais para mostrar inline.".to_owned(),
        ));
    }

    let bytes = store.get(&chave).await?;

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::PREVIEW, "file")
            .resource(file_id)
            .context(&file_context(&workspace, ficheiro.classification()))
            .classified(ficheiro.classification()),
    )
    .await?;

    Ok(InlinePreview {
        content_type: tipo,
        bytes,
        checksum_sha256: soma,
    })
}

/// O conteúdo textual de um ficheiro, para quem o pode ler.
///
/// # Porque isto existe em vez de um segundo parser
///
/// Porque a pré-visualização e a pesquisa liam o mesmo ficheiro por caminhos
/// diferentes: a pesquisa pela extracção, a pré-visualização descarregando os
/// bytes e descodificando-os outra vez. Dois caminhos para o mesmo texto
/// divergem — e o dia em que divergissem seria o dia em que alguém veria no
/// ecrã uma coisa diferente daquela que a pesquisa encontrou.
///
/// Devolve `None` quando não há extracção disponível: um ficheiro por processar
/// e um ficheiro sem leitor não têm texto, e isso diz-se em vez de se inventar.
///
/// # Errors
///
/// Devolve erro quando o ficheiro não é alcançável ou quando a autorização
/// recusa.
pub async fn content(
    executor: &mut sqlx::PgConnection,
    principal: &Principal,
    file_id: Uuid,
    max_chars: usize,
) -> CoreResult<Option<String>> {
    // A mesma autoridade de tudo o resto. O conteúdo não tem porta própria.
    let (_, _) = get(&mut *executor, principal, file_id).await?;
    super::extraction::text_of_current(&mut *executor, file_id, max_chars).await
}

/// Excertos do corpo de uma versão determinada, para quem a pode ler.
///
/// # A autoridade é a do ficheiro, e é reavaliada aqui
///
/// Não se confia em o chamador ter autorizado antes. `get_version` resolve a
/// versão **através** do ficheiro, e é o ficheiro que decide — pelo que
/// conhecer o identificador de uma versão não abre um caminho paralelo para o
/// conteúdo dela.
///
/// # Errors
///
/// Devolve erro quando a versão não é alcançável ou quando a autorização
/// recusa.
pub async fn excerpts(
    executor: &mut sqlx::PgConnection,
    principal: &Principal,
    file_version_id: Uuid,
    max_excerpts: usize,
    max_chars: usize,
) -> CoreResult<Vec<super::extraction::Excerpt>> {
    let (versao, _) = get_version(&mut *executor, principal, file_version_id).await?;
    super::extraction::excerpts_of_version(
        &mut *executor,
        versao.version_id,
        max_excerpts,
        max_chars,
    )
    .await
}

/// O conteúdo textual de **uma versão determinada**, para quem a pode ler.
///
/// # Porque isto existe ao lado de [`content`]
///
/// Porque uma citação aponta para a versão 2, e abrir a versão 2 tem de mostrar
/// a versão 2. `content` responde «o que este ficheiro diz agora»; isto responde
/// «o que este ficheiro dizia quando alguém o citou», e são perguntas
/// diferentes.
///
/// A autoridade é a mesma: `get_version` resolve através do ficheiro.
///
/// # Errors
///
/// Devolve erro quando a versão não é alcançável ou quando a autorização
/// recusa.
pub async fn content_of_version(
    executor: &mut sqlx::PgConnection,
    principal: &Principal,
    file_version_id: Uuid,
    max_chars: usize,
) -> CoreResult<Option<String>> {
    let (versao, _) = get_version(&mut *executor, principal, file_version_id).await?;
    super::extraction::text_of_version(&mut *executor, versao.version_id, max_chars).await
}

/// A vista agregada dos ficheiros: tudo o que este principal alcança.
pub struct AllFiles {
    /// Os ficheiros, do mais recentemente alterado para trás.
    pub files: Vec<repo::FileAcrossWorkspaces>,
    /// Quantos existem ao todo, pelo **mesmo** predicado da lista.
    pub total: i64,
    /// Os ambientes onde este principal pode criar ficheiros.
    ///
    /// Carregar exige destino. Zero destinos é um estado honesto e diz-se;
    /// um pode pré-seleccionar-se; vários obrigam a escolher — e nunca se
    /// escolhe um por alguém.
    pub destinos: Vec<(Uuid, String)>,
}

/// Todos os ficheiros que este principal alcança, em todos os ambientes.
///
/// # Errors
///
/// Devolve erro quando a consulta falha.
pub async fn all(pool: &sqlx::PgPool, principal: &Principal, limit: i64) -> CoreResult<AllFiles> {
    let filtro = ocinye_domain::policy::VisibilityFilter::for_principal(principal);
    if filtro.is_never_satisfiable() {
        return Ok(AllFiles {
            files: Vec::new(),
            total: 0,
            destinos: Vec::new(),
        });
    }

    let files =
        repo::list_files_across_workspaces(pool, principal.organisation_id, &filtro, limit).await?;
    let total =
        repo::count_files_across_workspaces(pool, principal.organisation_id, &filtro).await?;

    // Os destinos não se derivam da lista: quem não tem ficheiro nenhum pode ter
    // onde os pôr, e quem vê muitos pode não poder escrever em nenhum. São
    // perguntas diferentes, e esta faz-se ao `authorize` que `create` faz.
    let (ambientes, _) = crate::modules::research::list_workspaces(
        pool,
        principal,
        Default::default(),
        ocinye_contracts::PageRequest {
            page: 1,
            page_size: 100,
        },
    )
    .await?;
    let mut destinos = Vec::new();
    for ambiente in ambientes {
        let pode = ocinye_domain::policy::authorize(
            principal,
            Action::Create,
            &file_context(&ambiente, ambiente.classification()),
        )
        .is_ok();
        if pode {
            destinos.push((
                ambiente.id,
                format!("{} · {}", ambiente.code, ambiente.title),
            ));
        }
    }

    Ok(AllFiles {
        files,
        total,
        destinos,
    })
}

#[cfg(test)]
mod tests {
    use super::is_textual_type;

    #[test]
    fn is_textual_type_aceita_texto_e_codigo_e_recusa_binario() {
        // Texto e código lêem-se inline; um binário nunca.
        assert!(is_textual_type("text/plain"));
        assert!(is_textual_type("text/csv"));
        assert!(is_textual_type("text/markdown"));
        assert!(is_textual_type("application/json"));
        assert!(is_textual_type("application/xml"));
        assert!(!is_textual_type("image/png"));
        assert!(!is_textual_type("application/pdf"));
        assert!(!is_textual_type(
            "application/vnd.openxmlformats-officedocument.presentationml.presentation"
        ));
    }
}
