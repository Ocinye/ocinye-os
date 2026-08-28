//! O avatar do próprio membro.
//!
//! # A propriedade é estrutural, não verificada
//!
//! > **Owning an asset identifier does not confer authority to attach it to
//! > another identity.**
//!
//! Nenhuma função aqui recebe `person_id`. Não é que o rejeitem: não existe
//! parâmetro por onde ele possa entrar. Todas fixam `principal.person_id`, e um
//! cliente que envie um identificador de pessoa está a enviar um campo que
//! ninguém lê. Uma verificação de propriedade pode ser esquecida numa função
//! nova; a ausência de parâmetro não se esquece.
//!
//! O mesmo vale para o objecto guardado: o membro nunca escolhe a chave, nunca
//! a vê, e nunca envia o identificador de um objecto existente. A chave é
//! gerada aqui, a partir de um UUID novo.
//!
//! # Um preset não é um upload
//!
//! Escolher um avatar do produto guarda uma palavra. Não copia ficheiros para o
//! bucket, não cria `storage_objects`, não gasta nada. Doze presets vezes os
//! membros de uma instituição seriam milhares de cópias do mesmo ficheiro para
//! representar uma escolha entre doze.

use ocinye_contracts::AvatarChoice;
use ocinye_domain::Principal;
use sqlx::PgPool;
use uuid::Uuid;

use crate::avatar::{normalise, AVATAR_CONTENT_TYPE};
use crate::error::{CoreError, CoreResult};
use crate::storage::{build_object_key, ObjectStore};

/// A chave do objecto do avatar de um membro.
///
/// `build_object_key` pede um workspace, e um avatar não pertence a nenhum:
/// pertence a uma pessoa. O `Uuid::nil()` no lugar do workspace mantém a mesma
/// forma de chave sem inventar um workspace que não existe — e o prefixo `people`
/// diz o que lá está sem depender disso.
fn object_key(organisation_slug: &str, person_id: Uuid, object_id: Uuid) -> String {
    let base = build_object_key(organisation_slug, Uuid::nil(), object_id);
    let sufixo = base.rsplit('/').take(2).collect::<Vec<_>>();
    format!(
        "{organisation_slug}/people/{person_id}/avatar/{}/{}",
        sufixo[1], sufixo[0]
    )
}

/// O avatar actualmente escolhido pelo membro.
///
/// # Errors
///
/// Devolve um erro quando a consulta falha.
pub async fn own_avatar(pool: &PgPool, principal: &Principal) -> CoreResult<AvatarChoice> {
    let row: Option<(String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT p.avatar_kind, p.avatar_preset, o.checksum_sha256
           FROM people p
           LEFT JOIN storage_objects o ON o.id = p.avatar_object_id
          WHERE p.id = $1 AND p.organisation_id = $2",
    )
    .bind(principal.person_id)
    .bind(principal.organisation_id)
    .fetch_optional(pool)
    .await?;

    Ok(
        row.map_or(AvatarChoice::Initials, |(kind, preset, version)| {
            AvatarChoice::from_columns(&kind, preset.as_deref(), version.as_deref())
        }),
    )
}

/// Volta às iniciais.
///
/// # Errors
///
/// Devolve um erro quando a escrita falha.
pub async fn use_initials(
    pool: &PgPool,
    principal: &Principal,
    store: Option<&ObjectStore>,
) -> CoreResult<AvatarChoice> {
    limpar(pool, principal, store).await?;
    Ok(AvatarChoice::Initials)
}

/// Escolhe um avatar do catálogo do produto.
///
/// # Errors
///
/// Devolve [`CoreError::Validation`] quando o identificador não pertence ao
/// catálogo.
pub async fn choose_preset(
    pool: &PgPool,
    principal: &Principal,
    store: Option<&ObjectStore>,
    preset: &str,
) -> CoreResult<AvatarChoice> {
    // O identificador vem do cliente. Só passa se estiver na lista fechada —
    // não é sanitizado, não é escapado, não é normalizado: ou é um dos doze, ou
    // não é nada.
    if !AvatarChoice::is_known_preset(preset) {
        return Err(CoreError::Validation(
            "That is not an Ocinye avatar.".to_owned(),
        ));
    }

    limpar(pool, principal, store).await?;
    sqlx::query(
        "UPDATE people
            SET avatar_kind = 'preset', avatar_preset = $3, updated_at = now()
          WHERE id = $1 AND organisation_id = $2",
    )
    .bind(principal.person_id)
    .bind(principal.organisation_id)
    .bind(preset)
    .execute(pool)
    .await?;

    Ok(AvatarChoice::Preset {
        preset: preset.to_owned(),
    })
}

/// Guarda uma fotografia carregada pelo próprio membro.
///
/// Os bytes são normalizados antes de tocarem no storage: o que fica guardado é
/// o resultado, e o ficheiro de origem não é preservado.
///
/// # Errors
///
/// Devolve [`CoreError::Validation`] quando os bytes não são uma imagem aceite,
/// e [`CoreError::StorageUnavailable`] quando o objecto não pode ser guardado.
pub async fn set_photograph(
    pool: &PgPool,
    principal: &Principal,
    store: &ObjectStore,
    organisation_slug: &str,
    data: &[u8],
) -> CoreResult<AvatarChoice> {
    // A normalização primeiro, e fora de qualquer transacção: é a parte que
    // recusa, e recusar não deve deixar nada para trás.
    let normalizado = normalise(data)?;

    let object_id = Uuid::new_v4();
    let key = object_key(organisation_slug, principal.person_id, object_id);
    let size = i64::try_from(normalizado.data.len()).unwrap_or(i64::MAX);

    store
        .put(
            &key,
            AVATAR_CONTENT_TYPE,
            &normalizado.checksum_sha256,
            normalizado.data,
        )
        .await?;

    // A partir daqui há um objecto no bucket. Se a escrita na base de dados
    // falhar, ninguém o alcança — nada o referencia — mas ele fica lá. Em vez de
    // o deixar órfão em silêncio, remove-se antes de propagar o erro.
    let resultado = async {
        let mut tx = pool.begin().await?;

        // `INSERT … SELECT` insere tantas linhas quantas o `SELECT` devolver, e
        // sem backend por omissão devolve zero — sem erro. O objecto ficava no
        // bucket, a linha não existia, e o `UPDATE` seguinte rebentava numa
        // violação de chave estrangeira sobre um UUID que ninguém reconhecia.
        //
        // A causa era «esta instalação não tem armazenamento registado», e é
        // isso que tem de ser dito.
        let escrita = sqlx::query(
            "INSERT INTO storage_objects
                 (id, backend_id, organisation_id, object_key,
                  original_filename, content_type, size_bytes, checksum_sha256,
                  classification, status, created_by_id)
             SELECT $1, b.id, $2, $3, 'avatar.webp', $4, $5, $6, 'INTERNAL', 'stored', $7
               FROM storage_backends b
              WHERE b.is_default AND b.is_active",
        )
        .bind(object_id)
        .bind(principal.organisation_id)
        .bind(&key)
        .bind(AVATAR_CONTENT_TYPE)
        .bind(size)
        .bind(&normalizado.checksum_sha256)
        .bind(principal.person_id)
        .execute(&mut *tx)
        .await?;

        if escrita.rows_affected() == 0 {
            return Err(CoreError::StorageUnavailable(
                "No default storage backend is registered on this deployment.".to_owned(),
            ));
        }

        let anterior = descartar_anterior(&mut tx, principal).await?;

        sqlx::query(
            "UPDATE people
                SET avatar_kind = 'custom', avatar_preset = NULL,
                    avatar_object_id = $3, updated_at = now()
              WHERE id = $1 AND organisation_id = $2",
        )
        .bind(principal.person_id)
        .bind(principal.organisation_id)
        .bind(object_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok::<_, CoreError>(anterior)
    }
    .await;

    match resultado {
        Ok(anterior) => {
            // A fotografia anterior deixou de ser alcançável. Apagá-la aqui, e
            // não numa recolha diferida, mantém a promessa simples: substituir
            // uma fotografia não deixa a antiga no armazenamento institucional.
            if let Some(chave) = anterior {
                store.delete(&chave).await;
            }
            Ok(AvatarChoice::Custom {
                version: normalizado.checksum_sha256,
            })
        }
        Err(error) => {
            store.delete(&key).await;
            Err(error)
        }
    }
}

/// A chave do objecto do avatar actual, quando o membro tem fotografia.
///
/// # A pergunta que esta função responde
///
/// Não é «existe um avatar com esta versão». É «a versão pedida é a do avatar
/// **deste** principal». Um identificador de conteúdo que pertença a outra
/// pessoa não encontra nada aqui, e a resposta é a mesma de uma versão que
/// nunca existiu: conhecer o identificador não concede acesso.
///
/// # Errors
///
/// Devolve [`CoreError::NotFound`] quando o membro não tem fotografia ou a
/// versão pedida não é a actual.
pub async fn own_photograph_key(
    pool: &PgPool,
    principal: &Principal,
    version: &str,
) -> CoreResult<String> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT o.object_key
           FROM people p
           JOIN storage_objects o ON o.id = p.avatar_object_id
          WHERE p.id = $1
            AND p.organisation_id = $2
            AND p.avatar_kind = 'custom'
            AND o.checksum_sha256 = $3",
    )
    .bind(principal.person_id)
    .bind(principal.organisation_id)
    .bind(version)
    .fetch_optional(pool)
    .await?;

    row.map(|(key,)| key)
        .ok_or_else(|| CoreError::NotFound("Resource not found.".to_owned()))
}

/// Desassocia o avatar actual e devolve a chave do objecto que ficou sem dono.
async fn descartar_anterior(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    principal: &Principal,
) -> CoreResult<Option<String>> {
    let anterior: Option<(Uuid, String)> = sqlx::query_as(
        "SELECT o.id, o.object_key
           FROM people p
           JOIN storage_objects o ON o.id = p.avatar_object_id
          WHERE p.id = $1 AND p.organisation_id = $2",
    )
    .bind(principal.person_id)
    .bind(principal.organisation_id)
    .fetch_optional(&mut **tx)
    .await?;

    let Some((object_id, key)) = anterior else {
        return Ok(None);
    };

    // Volta ao estado de origem inteiro, e não só ao campo do objecto.
    //
    // A restrição de coerência é por linha e é verificada a cada instrução: uma
    // pessoa com `avatar_kind = 'custom'` e `avatar_object_id = NULL` é
    // exactamente o estado que a restrição existe para impedir, e não deixa de
    // o ser por estar a meio de uma transacção.
    //
    // Quem chama decide o que vem a seguir — um preset, uma fotografia nova, ou
    // ficar assim. Aqui só se larga o que havia.
    sqlx::query(
        "UPDATE people
            SET avatar_kind = 'initials', avatar_preset = NULL, avatar_object_id = NULL
          WHERE id = $1",
    )
    .bind(principal.person_id)
    .execute(&mut **tx)
    .await?;
    sqlx::query("DELETE FROM storage_objects WHERE id = $1")
        .bind(object_id)
        .execute(&mut **tx)
        .await?;

    Ok(Some(key))
}

/// Repõe o estado de origem, largando o que houvesse — inclusive no bucket.
///
/// # A fotografia antiga não fica lá
///
/// Durante algum tempo ficava. Substituir uma fotografia apagava a anterior,
/// mas **remover** deixava-a no armazenamento: a associação desaparecia da base
/// de dados e o objecto continuava a ocupar espaço institucional sem nada que o
/// alcançasse. Escolher um preset fazia o mesmo.
///
/// Ninguém dava por isso — a interface mostrava as iniciais, e estava certa. O
/// teste contra armazenamento real é que o viu, porque foi lá buscar o objecto
/// depois de o membro o ter removido, e ele respondeu.
///
/// `store` é opcional porque uma instalação sem armazenamento configurado tem
/// de continuar a poder voltar às iniciais: não conseguir limpar o bucket não é
/// razão para prender alguém à fotografia que lá está.
async fn limpar(
    pool: &PgPool,
    principal: &Principal,
    store: Option<&ObjectStore>,
) -> CoreResult<()> {
    let mut tx = pool.begin().await?;
    let anterior = descartar_anterior(&mut tx, principal).await?;
    sqlx::query(
        "UPDATE people
            SET avatar_kind = 'initials', avatar_preset = NULL,
                avatar_object_id = NULL, updated_at = now()
          WHERE id = $1 AND organisation_id = $2",
    )
    .bind(principal.person_id)
    .bind(principal.organisation_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    // Depois do commit: enquanto a transacção não fechar, a linha ainda aponta
    // para o objecto, e apagá-lo antes deixaria a base de dados a referir um
    // ficheiro que já não existe se o commit falhasse.
    if let (Some(chave), Some(store)) = (anterior, store) {
        store.delete(&chave).await;
    }
    Ok(())
}
