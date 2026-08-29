//! Leituras e escritas de ficheiros e versões.

use ocinye_contracts::Classification;
use sqlx::PgExecutor;
use uuid::Uuid;

use crate::error::CoreResult;

/// Cria a identidade do ficheiro.
///
/// # Errors
///
/// Devolve erro quando a inserção falha.
pub async fn insert_file<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    unit_id: Uuid,
    workspace_id: Uuid,
    name: &str,
    classification: Classification,
    created_by: Uuid,
) -> CoreResult<Uuid> {
    let id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO files
             (organisation_id, unit_id, workspace_id, name, classification, created_by_id)
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
    )
    .bind(organisation_id)
    .bind(unit_id)
    .bind(workspace_id)
    .bind(name)
    .bind(classification.as_str())
    .bind(created_by)
    .fetch_one(executor)
    .await?;
    Ok(id)
}

/// Acrescenta uma versão, com o número que o Core determinou.
///
/// # Porque a sequência vem calculada de fora
///
/// Porque quem a calcula tem de a calcular **dentro da transacção**, e a
/// restrição da base é a segunda linha de defesa e não a primeira. Deixar o
/// número vir de quem chama a API seria deixar o cliente decidir qual é a
/// versão corrente.
///
/// # Errors
///
/// Devolve erro quando a inserção falha — incluindo quando duas escritas
/// concorrentes chegam ao mesmo número, que a restrição única recusa.
pub async fn insert_version<'e>(
    executor: impl PgExecutor<'e>,
    file_id: Uuid,
    sequence: i32,
    storage_object_id: Uuid,
    note: Option<&str>,
    created_by: Uuid,
) -> CoreResult<Uuid> {
    let id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO file_versions (file_id, sequence, storage_object_id, note, created_by_id)
         VALUES ($1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(file_id)
    .bind(sequence)
    .bind(storage_object_id)
    .bind(note)
    .bind(created_by)
    .fetch_one(executor)
    .await?;
    Ok(id)
}

/// O número da versão seguinte, com a linha do ficheiro trancada.
///
/// Devolve `None` quando o ficheiro não existe — o que permite a quem chama
/// recusar com uma mensagem do domínio em vez de uma chave estrangeira.
///
/// # Porque tranca o ficheiro
///
/// Porque «ler o máximo e somar um» sem tranca é uma corrida: duas escritas
/// simultâneas lêem o mesmo máximo e decidem ambas que são a versão seguinte.
/// A restrição única da base recusa a segunda — o que é correcto e é uma
/// mensagem de SQL —, e a transacção que perde não tinha por onde saber que
/// devia voltar a tentar.
///
/// Com `FOR UPDATE` sobre o ficheiro, a segunda espera, lê o máximo já
/// actualizado, e recebe o número certo. As duas versões entram, por ordem.
///
/// # Errors
///
/// Devolve erro quando a consulta falha.
pub async fn next_sequence(
    executor: &mut sqlx::PgConnection,
    file_id: Uuid,
) -> CoreResult<Option<i32>> {
    // Duas consultas, e não uma: o PostgreSQL recusa `FOR UPDATE` junto de
    // `GROUP BY`, e juntá-las obrigaria a uma subconsulta que esconde qual das
    // duas coisas está a trancar o quê.
    //
    // A primeira tranca o ficheiro e diz se ele existe. A segunda lê o máximo,
    // já protegida por essa tranca.
    let existe: Option<Uuid> = sqlx::query_scalar("SELECT id FROM files WHERE id = $1 FOR UPDATE")
        .bind(file_id)
        .fetch_optional(&mut *executor)
        .await?;
    if existe.is_none() {
        return Ok(None);
    }

    let maximo: Option<i32> =
        sqlx::query_scalar("SELECT max(sequence) FROM file_versions WHERE file_id = $1")
            .bind(file_id)
            .fetch_one(&mut *executor)
            .await?;
    Ok(Some(maximo.map_or(1, |m| m + 1)))
}

/// A versão corrente de um ficheiro.
///
/// **A de maior `sequence`**, e nunca a mais recente por relógio: as datas
/// podem empatar, e as de um preenchimento histórico foram herdadas de outra
/// coisa. A ordem institucional é a sequência.
///
/// # Errors
///
/// Devolve erro quando a consulta falha.
pub async fn current_storage_object<'e>(
    executor: impl PgExecutor<'e>,
    file_id: Uuid,
) -> CoreResult<Option<(Uuid, i32)>> {
    let linha = sqlx::query_as::<_, (Uuid, i32)>(
        "SELECT storage_object_id, sequence
           FROM file_versions
          WHERE file_id = $1
          ORDER BY sequence DESC
          LIMIT 1",
    )
    .bind(file_id)
    .fetch_optional(executor)
    .await?;
    Ok(linha)
}

/// Um ficheiro institucional, tal como a base o guarda.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct FileRecord {
    /// A identidade estável.
    pub id: Uuid,
    /// A unidade. Não pode discordar da do ambiente.
    pub unit_id: Uuid,
    /// O ambiente que o governa.
    pub workspace_id: Uuid,
    /// O nome visível.
    pub name: String,
    /// A classificação do artefacto, tal como está guardada.
    ///
    /// Texto, e lida por [`FileRecord::classification`]. O padrão é o do resto
    /// do domínio: um valor que a base não reconheça cai no **mais
    /// restritivo**, e não no mais permissivo. Uma classificação ilegível não
    /// pode ser uma porta aberta.
    classification: String,
}

impl FileRecord {
    /// A classificação, interpretada.
    #[must_use]
    pub fn classification(&self) -> Classification {
        Classification::parse(&self.classification).unwrap_or(Classification::Restricted)
    }
}

/// Lê um ficheiro dentro da organização de quem pergunta.
///
/// O `organisation_id` está na consulta e não numa verificação a seguir: um
/// identificador de outra organização devolve «não existe», e não «existe e
/// não podes».
///
/// # Errors
///
/// Devolve erro quando a consulta falha.
pub async fn find_file<'e>(
    executor: impl PgExecutor<'e>,
    file_id: Uuid,
    organisation_id: Uuid,
) -> CoreResult<Option<FileRecord>> {
    let linha = sqlx::query_as::<_, FileRecord>(
        "SELECT id, unit_id, workspace_id, name, classification
           FROM files WHERE id = $1 AND organisation_id = $2",
    )
    .bind(file_id)
    .bind(organisation_id)
    .fetch_optional(executor)
    .await?;
    Ok(linha)
}

/// Lê uma versão pelo seu identificador, **sem autorizar**.
///
/// Devolve o ficheiro a que pertence para que quem chama autorize por ele. Não
/// é `pub` para o mundo por acidente: é `pub(super)` porque só o serviço a deve
/// usar, e o serviço autoriza sempre a seguir.
///
/// # Errors
///
/// Devolve erro quando a consulta falha.
pub(super) async fn find_version<'e>(
    executor: impl PgExecutor<'e>,
    version_id: Uuid,
) -> CoreResult<Option<super::service::FileVersionRecord>> {
    let linha = sqlx::query_as::<_, (Uuid, Uuid, i32, Uuid)>(
        "SELECT id, file_id, sequence, storage_object_id FROM file_versions WHERE id = $1",
    )
    .bind(version_id)
    .fetch_optional(executor)
    .await?;
    Ok(
        linha.map(|(version_id, file_id, sequence, storage_object_id)| {
            super::service::FileVersionRecord {
                file_id,
                version_id,
                sequence,
                storage_object_id,
            }
        }),
    )
}

/// Onde estão os bytes de um objecto guardado.
///
/// # Errors
///
/// Devolve erro quando a consulta falha.
pub(super) async fn object_location<'e>(
    executor: impl PgExecutor<'e>,
    object_id: Uuid,
) -> CoreResult<Option<(String, String)>> {
    let linha = sqlx::query_as::<_, (String, String)>(
        "SELECT object_key, original_filename FROM storage_objects WHERE id = $1",
    )
    .bind(object_id)
    .fetch_optional(executor)
    .await?;
    Ok(linha)
}
