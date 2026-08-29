//! As duas operações que criam e fazem crescer um ficheiro institucional.

use ocinye_contracts::Classification;
use uuid::Uuid;

use super::repository as repo;
use crate::error::{CoreError, CoreResult};
use crate::Tx;

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
