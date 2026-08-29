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

/// Os bytes da versão corrente, com o que é preciso para os servir.
///
/// # Errors
///
/// Devolve erro quando a consulta falha.
pub(super) async fn current_object_details<'e>(
    executor: impl PgExecutor<'e>,
    file_id: Uuid,
) -> CoreResult<Option<(String, String, i64, String)>> {
    let linha = sqlx::query_as::<_, (String, String, i64, String)>(
        "SELECT o.object_key, o.content_type, o.size_bytes, o.checksum_sha256
           FROM file_versions v
           JOIN storage_objects o ON o.id = v.storage_object_id
          WHERE v.file_id = $1
          ORDER BY v.sequence DESC
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

/// Uma pasta, como a navegação a vê.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct FolderRecord {
    /// A identidade.
    pub id: Uuid,
    /// O ambiente que a contém. Uma pasta não o atravessa.
    pub workspace_id: Uuid,
    /// A pasta acima, ou `None` na raiz.
    pub parent_id: Option<Uuid>,
    /// O nome visível.
    pub name: String,
}

/// Cria uma pasta.
///
/// # Errors
///
/// Devolve erro quando a inserção falha — incluindo quando já existe uma irmã
/// com o mesmo nome.
pub async fn insert_folder<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    workspace_id: Uuid,
    parent_id: Option<Uuid>,
    name: &str,
    created_by: Uuid,
) -> CoreResult<Uuid> {
    let id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO folders (organisation_id, workspace_id, parent_id, name, created_by_id)
         VALUES ($1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(organisation_id)
    .bind(workspace_id)
    .bind(parent_id)
    .bind(name)
    .bind(created_by)
    .fetch_one(executor)
    .await?;
    Ok(id)
}

/// Lê uma pasta.
///
/// # Errors
///
/// Devolve erro quando a consulta falha.
pub async fn find_folder<'e>(
    executor: impl PgExecutor<'e>,
    folder_id: Uuid,
    organisation_id: Uuid,
) -> CoreResult<Option<FolderRecord>> {
    let linha = sqlx::query_as::<_, FolderRecord>(
        "SELECT id, workspace_id, parent_id, name
           FROM folders WHERE id = $1 AND organisation_id = $2",
    )
    .bind(folder_id)
    .bind(organisation_id)
    .fetch_optional(executor)
    .await?;
    Ok(linha)
}

/// As pastas directamente dentro de outra, ou da raiz.
///
/// # Errors
///
/// Devolve erro quando a consulta falha.
pub async fn list_folders<'e>(
    executor: impl PgExecutor<'e>,
    workspace_id: Uuid,
    parent_id: Option<Uuid>,
) -> CoreResult<Vec<FolderRecord>> {
    let linhas = sqlx::query_as::<_, FolderRecord>(
        "SELECT id, workspace_id, parent_id, name
           FROM folders
          WHERE workspace_id = $1
            AND parent_id IS NOT DISTINCT FROM $2
          ORDER BY lower(name)",
    )
    .bind(workspace_id)
    .bind(parent_id)
    .fetch_all(executor)
    .await?;
    Ok(linhas)
}

/// Um ficheiro tal como a navegação o mostra.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct FileListing {
    /// A identidade.
    pub id: Uuid,
    /// O nome.
    pub name: String,
    /// A classificação guardada.
    pub classification: String,
    /// O tipo do conteúdo da versão corrente.
    pub content_type: String,
    /// O tamanho da versão corrente.
    pub size_bytes: i64,
    /// Quantas versões existem.
    pub versions: i64,
    /// Quando mudou pela última vez.
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Os ficheiros de uma pasta, filtrados pela visibilidade de quem pergunta.
///
/// A classificação usada no filtro é a **do ficheiro composta com a do
/// ambiente**, lida agora. A pasta não entra na decisão: não tem classificação
/// e não pode ter.
///
/// # Errors
///
/// Devolve erro quando a consulta falha.
pub async fn list_files<'e>(
    executor: impl PgExecutor<'e>,
    workspace_id: Uuid,
    folder_id: Option<Uuid>,
    filter: &ocinye_domain::policy::VisibilityFilter,
) -> CoreResult<Vec<FileListing>> {
    let predicado = crate::visibility::to_sql(
        filter,
        crate::visibility::VisibilityColumns::aliased(
            "f.unit_id",
            "f.workspace_id",
            EFECTIVA_DO_FICHEIRO,
        ),
    );
    let linhas = sqlx::query_as::<_, FileListing>(&format!(
        "SELECT f.id, f.name, {EFECTIVA_DO_FICHEIRO} AS classification,
                o.content_type, o.size_bytes,
                (SELECT count(*) FROM file_versions x WHERE x.file_id = f.id) AS versions,
                f.updated_at
           FROM files f
           JOIN research_workspaces w ON w.id = f.workspace_id
           JOIN LATERAL (
               SELECT fv.storage_object_id
                 FROM file_versions fv
                WHERE fv.file_id = f.id
                ORDER BY fv.sequence DESC
                LIMIT 1
           ) v ON TRUE
           JOIN storage_objects o ON o.id = v.storage_object_id
          WHERE f.workspace_id = $1
            AND f.folder_id IS NOT DISTINCT FROM $2
            AND {predicado}
          ORDER BY lower(f.name)"
    ))
    .bind(workspace_id)
    .bind(folder_id)
    .fetch_all(executor)
    .await?;
    Ok(linhas)
}

/// A classificação efectiva de um ficheiro: a sua, composta com a do ambiente.
///
/// A mesma composição que a autorização faz, escrita em SQL para que a
/// listagem não possa discordar da leitura.
const EFECTIVA_DO_FICHEIRO: &str = "CASE
    WHEN f.classification = 'RESTRICTED' OR w.classification = 'RESTRICTED' THEN 'RESTRICTED'
    WHEN f.classification = 'CONFIDENTIAL' OR w.classification = 'CONFIDENTIAL' THEN 'CONFIDENTIAL'
    WHEN f.classification = 'INTERNAL' OR w.classification = 'INTERNAL' THEN 'INTERNAL'
    ELSE 'PUBLIC'
END";

/// Move um ficheiro para outra pasta **do mesmo ambiente**.
///
/// # Errors
///
/// Devolve erro quando a escrita falha — incluindo quando a pasta é de outro
/// ambiente, que a chave estrangeira composta recusa.
pub async fn move_file<'e>(
    executor: impl PgExecutor<'e>,
    file_id: Uuid,
    folder_id: Option<Uuid>,
) -> CoreResult<()> {
    sqlx::query("UPDATE files SET folder_id = $1, updated_at = now() WHERE id = $2")
        .bind(folder_id)
        .bind(file_id)
        .execute(executor)
        .await?;
    Ok(())
}

/// O caminho de uma pasta até à raiz, para migalhas de navegação.
///
/// # Errors
///
/// Devolve erro quando a consulta falha.
pub async fn folder_path<'e>(
    executor: impl PgExecutor<'e>,
    folder_id: Uuid,
) -> CoreResult<Vec<FolderRecord>> {
    let linhas = sqlx::query_as::<_, FolderRecord>(
        "WITH RECURSIVE subida AS (
             SELECT id, workspace_id, parent_id, name, 0 AS profundidade
               FROM folders WHERE id = $1
             UNION ALL
             SELECT p.id, p.workspace_id, p.parent_id, p.name, s.profundidade + 1
               FROM folders p JOIN subida s ON p.id = s.parent_id
              WHERE s.profundidade < 64
         )
         SELECT id, workspace_id, parent_id, name FROM subida ORDER BY profundidade DESC",
    )
    .bind(folder_id)
    .fetch_all(executor)
    .await?;
    Ok(linhas)
}

/// Uma versão, como o histórico a mostra.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct VersionListing {
    /// A identidade da versão. É por aqui que se cita e se descarrega.
    pub id: Uuid,
    /// O número. Começa em 1, e a maior é a corrente.
    pub sequence: i32,
    /// O nome com que os bytes foram carregados. Pode mudar entre versões.
    pub original_filename: String,
    /// O tipo declarado no carregamento.
    pub content_type: String,
    /// O tamanho em bytes.
    pub size_bytes: i64,
    /// A soma que distingue estes bytes de outros quaisquer.
    pub checksum_sha256: String,
    /// A nota de quem carregou, quando a escreveu.
    pub note: Option<String>,
    /// Quem carregou, pelo nome visível. `None` quando a pessoa já não existe.
    pub created_by: Option<String>,
    /// Quando.
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// O histórico completo de um ficheiro, da versão mais recente para a mais
/// antiga.
///
/// Não autoriza nada: quem chama tem de ter passado pela autoridade do
/// ficheiro primeiro.
///
/// # Errors
///
/// Devolve erro quando a consulta falha.
pub async fn list_versions<'e>(
    executor: impl PgExecutor<'e>,
    file_id: Uuid,
) -> CoreResult<Vec<VersionListing>> {
    let linhas = sqlx::query_as::<_, VersionListing>(
        "SELECT v.id,
                v.sequence,
                o.original_filename,
                o.content_type,
                o.size_bytes,
                o.checksum_sha256,
                v.note,
                p.display_name AS created_by,
                v.created_at
           FROM file_versions v
           JOIN storage_objects o ON o.id = v.storage_object_id
           LEFT JOIN people p ON p.id = v.created_by_id
          WHERE v.file_id = $1
          ORDER BY v.sequence DESC",
    )
    .bind(file_id)
    .fetch_all(executor)
    .await?;
    Ok(linhas)
}
