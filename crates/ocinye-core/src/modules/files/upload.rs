//! Carregamento em partes: como meio gigabyte atravessa um edge que recusa cem.
//!
//! # A propriedade
//!
//! > **Um ficheiro institucional pode ser carregado em pedaços, através de uma
//! > sessão autorizada antes do primeiro byte, sem que exista `FileVersion`
//! > enquanto o conjunto não estiver completo e verificado — e sem que o
//! > browser receba credenciais do armazenamento.**
//!
//! # Porque não se reduziu o limite
//!
//! Porque o limite de um fornecedor de edge não é uma decisão institucional. O
//! Ocinye Files aceita 512 MiB porque foi isso que a instituição decidiu que
//! precisa de guardar; a Cloudflare recusa pedidos proxied acima de ~100 MB
//! porque é o plano em que a instituição está. Fazer o segundo número apagar o
//! primeiro seria deixar a infraestrutura redefinir o produto.
//!
//! # Porque não se tirou a API de trás do edge
//!
//! Porque um hostname DNS-only para carregamentos publicaria o origin, e um
//! origin publicado é um origin sem WAF, sem rate limiting e com o seu endereço
//! a circular. A alternativa — o browser falar directamente com o object store
//! — obrigaria a entregar-lhe credenciais ou URLs assinados de escrita, e nesse
//! momento a autorização deixaria de estar no Core.
//!
//! O que atravessa o edge são pedaços pequenos. O que monta o ficheiro é o
//! armazenamento, do lado de lá do Core.

use uuid::Uuid;

use ocinye_contracts::Classification;
use ocinye_observability::CorrelationIds;

use crate::audit::{self, action, AuditEntry};
use crate::Tx;
use crate::error::{CoreError, CoreResult};
use crate::storage::ObjectStore;
use ocinye_domain::policy::{authorize, Action};
use ocinye_domain::Principal;

/// O tamanho de cada pedaço.
///
/// 32 MiB, e não 90: o limite da Cloudflare é do **pedido**, e um pedido leva
/// mais do que o pedaço — envelope multipart, cabeçalhos, e a margem que um
/// proxy intermédio possa consumir. Escolher um número perto do limite faz o
/// carregamento falhar em condições que ninguém consegue reproduzir.
///
/// O mínimo de uma parte de S3 é 5 MiB (excepto a última). 32 MiB está
/// confortavelmente acima, e mantém o número de partes baixo: 512 MiB são 16
/// pedaços, não centenas.
pub const CHUNK_SIZE_BYTES: i32 = 32 * 1024 * 1024;

/// Quanto tempo uma sessão sobrevive sem ser finalizada.
///
/// Generoso, porque um carregamento grande numa ligação má é legítimo. Não
/// infinito, porque uma sessão aberta segura partes no armazenamento, e partes
/// que nada refere são espaço que ninguém consegue explicar.
pub const SESSION_TTL_HOURS: i64 = 12;

/// O que se pede para abrir uma sessão.
pub struct NewUpload {
    /// O nome com que o ficheiro entra na instituição.
    pub filename: String,
    /// O tipo declarado, validado na abertura e não no fim.
    pub content_type: String,
    /// O tamanho total declarado. Fixa o número de partes.
    pub size_bytes: i64,
    /// A classificação pedida; a efectiva é a mais restritiva com o ambiente.
    pub classification: Option<Classification>,
    /// A pasta de destino, quando há uma.
    pub folder_id: Option<Uuid>,
    /// Presente quando o carregamento é uma nova versão de um ficheiro que já
    /// existe.
    pub file_id: Option<Uuid>,
}

/// Uma sessão aberta, tal como quem carrega precisa de a conhecer.
#[derive(Debug, Clone)]
pub struct UploadSession {
    /// O identificador com que quem carrega envia cada parte.
    pub id: Uuid,
    /// O tamanho de cada parte, escolhido pelo Core.
    pub chunk_size_bytes: i32,
    /// Quantas partes compõem este ficheiro.
    pub total_parts: i32,
    /// Quando a sessão deixa de aceitar partes.
    pub expires_at: chrono::DateTime<chrono::Utc>,
    /// As partes que já chegaram. Numa sessão nova está vazio; ao retomar, diz
    /// a quem carrega o que **não** precisa de reenviar.
    pub received_parts: Vec<i32>,
}

/// A linha da sessão, como vive na base.
#[derive(sqlx::FromRow)]
struct SessionRow {
    workspace_id: Uuid,
    file_id: Option<Uuid>,
    folder_id: Option<Uuid>,
    filename: String,
    content_type: String,
    classification: Option<String>,
    declared_size_bytes: i64,
    chunk_size_bytes: i32,
    total_parts: i32,
    storage_object_id: Uuid,
    storage_key: String,
    storage_upload_id: String,
    created_by_id: Uuid,
    expires_at: chrono::DateTime<chrono::Utc>,
    state: String,
}

/// Abre uma sessão de carregamento.
///
/// # A autorização acontece **aqui**, antes do primeiro byte
///
/// Não no fim. Uma sessão que aceitasse quinhentos megabytes e só depois
/// perguntasse se aquela pessoa podia escrever naquele ambiente teria gasto a
/// rede e o disco de quem não podia — e teria dito «não» tarde de mais para
/// isso ter significado.
///
/// # Errors
///
/// Recusa quando o destino não é autorizável, quando o tamanho excede o limite
/// da instalação, ou quando o nome ou o tipo não são aceitáveis.
pub async fn begin(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    store: &ObjectStore,
    organisation_slug: &str,
    workspace_id: Uuid,
    request: NewUpload,
) -> CoreResult<UploadSession> {
    let workspace = crate::modules::research::get_workspace(&mut **tx, principal, workspace_id).await?;
    let classification = workspace
        .classification()
        .most_restrictive(request.classification.unwrap_or(Classification::DEFAULT));

    authorize(
        principal,
        Action::Create,
        &super::service::file_context(&workspace, classification),
    )
    .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    if request.size_bytes <= 0 {
        return Err(CoreError::Validation(
            "Um ficheiro vazio não é um carregamento.".to_owned(),
        ));
    }
    let tamanho = u64::try_from(request.size_bytes)
        .map_err(|_| CoreError::Validation("Tamanho inválido.".to_owned()))?;
    if tamanho > store.max_upload_bytes() {
        return Err(CoreError::Validation(
            "O ficheiro excede o tamanho máximo permitido.".to_owned(),
        ));
    }

    // O nome e o tipo validam-se na abertura, e não no fim: recusar um formato
    // depois de ele ter atravessado a rede é fazer alguém pagar o transporte de
    // uma coisa que nunca ia ser aceite.
    let content_type = crate::storage::validate_content_type(&request.content_type)?;
    let filename = crate::storage::normalise_filename(&request.filename)?;

    let chunk = CHUNK_SIZE_BYTES;
    let total_parts = i32::try_from((request.size_bytes + i64::from(chunk) - 1) / i64::from(chunk))
        .map_err(|_| CoreError::Validation("Ficheiro grande de mais.".to_owned()))?;

    let object_id = Uuid::new_v4();
    let object_key = crate::storage::build_object_key(organisation_slug, workspace.id, object_id);
    let upload_id = store.begin_multipart(&object_key, &content_type).await?;

    let expires_at = chrono::Utc::now() + chrono::Duration::hours(SESSION_TTL_HOURS);

    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO upload_sessions
             (organisation_id, workspace_id, file_id, folder_id, filename, content_type,
              classification, declared_size_bytes, chunk_size_bytes, total_parts,
              storage_object_id, storage_key, storage_upload_id, created_by_id, expires_at)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15)
         RETURNING id",
    )
    .bind(principal.organisation_id)
    .bind(workspace.id)
    .bind(request.file_id)
    .bind(request.folder_id)
    .bind(&filename)
    .bind(&content_type)
    .bind(classification.as_str())
    .bind(request.size_bytes)
    .bind(chunk)
    .bind(total_parts)
    .bind(object_id)
    .bind(&object_key)
    .bind(&upload_id)
    .bind(principal.person_id)
    .bind(expires_at)
    .fetch_one(&mut **tx)
    .await?;

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::CREATE, "upload_session")
            .resource(id)
            .context(&super::service::file_context(&workspace, classification))
            .classified(classification)
            .detail("declared_size_bytes", request.size_bytes.to_string())
            .detail("total_parts", total_parts.to_string()),
    )
    .await?;

    Ok(UploadSession {
        id,
        chunk_size_bytes: chunk,
        total_parts,
        expires_at,
        received_parts: Vec::new(),
    })
}

/// Lê a sessão e exige que quem pede seja quem a abriu.
///
/// # Porque a sessão é de quem a abriu, e de mais ninguém
///
/// Porque um identificador de sessão que qualquer pessoa autenticada pudesse
/// usar transformaria o carregamento numa forma de escrever no ambiente de
/// outra pessoa: a autorização foi avaliada contra **aquele** principal, e é a
/// ele que ficou ligada.
async fn sessao_de(
    executor: &mut Tx<'_>,
    principal: &Principal,
    session_id: Uuid,
) -> CoreResult<SessionRow> {
    let sessao: Option<SessionRow> = sqlx::query_as(
        "SELECT workspace_id, file_id, folder_id, filename, content_type, classification,
                declared_size_bytes, chunk_size_bytes, total_parts, storage_object_id,
                storage_key, storage_upload_id, created_by_id, expires_at, state
           FROM upload_sessions
          WHERE id = $1 AND organisation_id = $2",
    )
    .bind(session_id)
    .bind(principal.organisation_id)
    .fetch_optional(&mut **executor)
    .await?;

    // A mesma resposta para «não existe» e «não é sua». Distingui-las diria a
    // quem tenta que a sessão existe — que é informação que não lhe pertence.
    let sessao = sessao
        .filter(|s| s.created_by_id == principal.person_id)
        .ok_or_else(|| CoreError::NotFound("Sessão de carregamento não encontrada.".to_owned()))?;

    if sessao.state != "open" {
        return Err(CoreError::Conflict(
            "Esta sessão de carregamento já foi fechada.".to_owned(),
        ));
    }
    if sessao.expires_at < chrono::Utc::now() {
        return Err(CoreError::Conflict(
            "Esta sessão de carregamento expirou.".to_owned(),
        ));
    }
    Ok(sessao)
}

/// O que a aceitação de uma parte devolve.
pub struct PartAccepted {
    /// A parte que acabou de ser aceite.
    pub part_number: i32,
    /// Verdadeiro quando a parte já lá estava e nada foi reescrito.
    pub already_present: bool,
    /// Quantas partes já chegaram, para quem mostra progresso.
    pub received_parts: i64,
    /// Quantas faltam ao todo.
    pub total_parts: i32,
}

/// Aceita uma parte.
///
/// # Idempotente por construção
///
/// Uma parte que chega duas vezes — porque a rede caiu depois de o
/// armazenamento a ter escrito mas antes de a resposta voltar — encontra a
/// linha que já lá está e é aceite sem se reescrever. Repetir é seguro, e por
/// isso quem carrega pode repetir sem perguntar.
///
/// A soma é verificada **à chegada**. Um pedaço corrompido é recusado agora, e
/// quem carrega repete trinta e dois megabytes — não meio gigabyte.
///
/// # Errors
///
/// Recusa uma parte fora do intervalo, maior do que o pedaço acordado, ou cuja
/// soma não corresponde aos bytes recebidos.
pub async fn accept_part(
    tx: &mut Tx<'_>,
    principal: &Principal,
    store: &ObjectStore,
    session_id: Uuid,
    part_number: i32,
    sha256: &str,
    data: Vec<u8>,
) -> CoreResult<PartAccepted> {
    let sessao = sessao_de(tx, principal, session_id).await?;

    if part_number < 1 || part_number > sessao.total_parts {
        return Err(CoreError::Validation(format!(
            "Parte {part_number} fora do carregamento, que tem {} partes.",
            sessao.total_parts
        )));
    }
    if data.is_empty() {
        return Err(CoreError::Validation("Uma parte vazia não é uma parte.".to_owned()));
    }

    // A última parte é mais pequena; as outras são exactamente o pedaço
    // acordado. Aceitar uma parte maior deixaria contornar o tamanho que foi
    // autorizado na abertura.
    let esperado = if part_number == sessao.total_parts {
        let restante = sessao.declared_size_bytes
            - i64::from(sessao.chunk_size_bytes) * i64::from(part_number - 1);
        usize::try_from(restante).unwrap_or(0)
    } else {
        usize::try_from(sessao.chunk_size_bytes).unwrap_or(0)
    };
    if data.len() != esperado {
        return Err(CoreError::Validation(format!(
            "A parte {part_number} tem {} bytes e devia ter {esperado}.",
            data.len()
        )));
    }

    let soma = crate::storage::sha256_hex(&data);
    if !soma.eq_ignore_ascii_case(sha256) {
        return Err(CoreError::Validation(
            "A soma da parte não corresponde aos bytes recebidos.".to_owned(),
        ));
    }

    // Já cá está? Então não se reescreve. É isto que torna repetir seguro.
    let existente: Option<String> = sqlx::query_scalar(
        "SELECT sha256 FROM upload_parts WHERE session_id = $1 AND part_number = $2",
    )
    .bind(session_id)
    .bind(part_number)
    .fetch_optional(&mut **tx)
    .await?;

    let already_present = if let Some(anterior) = existente {
        if !anterior.eq_ignore_ascii_case(&soma) {
            return Err(CoreError::Conflict(
                "Esta parte já chegou com outro conteúdo.".to_owned(),
            ));
        }
        true
    } else {
        let etag = store
            .put_part(&sessao.storage_key, &sessao.storage_upload_id, part_number, data)
            .await?;
        sqlx::query(
            "INSERT INTO upload_parts (session_id, part_number, size_bytes, sha256, etag)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (session_id, part_number) DO NOTHING",
        )
        .bind(session_id)
        .bind(part_number)
        .bind(i32::try_from(esperado).unwrap_or(i32::MAX))
        .bind(&soma)
        .bind(&etag)
        .execute(&mut **tx)
        .await?;
        false
    };

    let recebidas: i64 =
        sqlx::query_scalar("SELECT count(*) FROM upload_parts WHERE session_id = $1")
            .bind(session_id)
            .fetch_one(&mut **tx)
            .await?;

    Ok(PartAccepted {
        part_number,
        already_present,
        received_parts: recebidas,
        total_parts: sessao.total_parts,
    })
}

/// Fecha o carregamento e produz a versão.
///
/// # A autoridade é reavaliada **agora**
///
/// Entre abrir a sessão e finalizá-la podem passar horas. Nesse intervalo a
/// pertença ao ambiente pode ter sido revogada, a conta pode ter sido suspensa,
/// a classificação do ambiente pode ter mudado. Autorizar só na abertura seria
/// dar a quem já não pode uma janela que fica aberta enquanto o carregamento
/// durar.
///
/// # Nada existe antes daqui
///
/// Não há `File` nem `FileVersion` enquanto esta função não devolve. Um
/// carregamento interrompido não deixa meio ficheiro na instituição.
///
/// # Errors
///
/// Recusa quando faltam partes, quando a autoridade deixou de existir, quando o
/// tamanho montado não é o declarado, ou quando a soma final não corresponde.
pub async fn finalise(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    store: &ObjectStore,
    session_id: Uuid,
    sha256: &str,
) -> CoreResult<super::service::FileVersionRecord> {
    let sessao = sessao_de(tx, principal, session_id).await?;

    // ── A autoridade, outra vez ─────────────────────────────────────────
    let workspace =
        crate::modules::research::get_workspace(&mut **tx, principal, sessao.workspace_id).await?;
    let classification = sessao
        .classification
        .as_deref()
        .and_then(Classification::parse)
        .unwrap_or(Classification::DEFAULT);
    let efectiva = workspace.classification().most_restrictive(classification);
    authorize(
        principal,
        Action::Create,
        &super::service::file_context(&workspace, efectiva),
    )
    .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    // ── As partes, todas e por ordem ────────────────────────────────────
    let partes: Vec<(i32, String, i64)> = sqlx::query_as(
        "SELECT part_number, etag, size_bytes::bigint
           FROM upload_parts WHERE session_id = $1 ORDER BY part_number",
    )
    .bind(session_id)
    .fetch_all(&mut **tx)
    .await?;

    if partes.len() != usize::try_from(sessao.total_parts).unwrap_or(usize::MAX) {
        return Err(CoreError::Validation(format!(
            "O carregamento tem {} de {} partes. Um conjunto incompleto não produz versão.",
            partes.len(),
            sessao.total_parts
        )));
    }
    // Contíguas, e não apenas na quantidade certa: contar partes deixaria passar
    // 1,2,2,4 se a base o permitisse, e o ficheiro sairia com um buraco.
    for (indice, (numero, _, _)) in partes.iter().enumerate() {
        let esperado = i32::try_from(indice + 1).unwrap_or(i32::MAX);
        if *numero != esperado {
            return Err(CoreError::Validation(format!(
                "Falta a parte {esperado}."
            )));
        }
    }

    let montado: i64 = partes.iter().map(|(_, _, tamanho)| tamanho).sum();
    if montado != sessao.declared_size_bytes {
        return Err(CoreError::Validation(format!(
            "O conjunto tem {montado} bytes e foi declarado com {}.",
            sessao.declared_size_bytes
        )));
    }

    // ── Montar ──────────────────────────────────────────────────────────
    let etiquetas: Vec<(i32, String)> = partes
        .iter()
        .map(|(numero, etag, _)| (*numero, etag.clone()))
        .collect();
    store
        .complete_multipart(&sessao.storage_key, &sessao.storage_upload_id, &etiquetas)
        .await?;

    // ── A soma do que ficou lá, e não a do que dizemos ter mandado ──────
    //
    // Lê-se de volta. As somas das partes provam que cada pedaço chegou
    // inteiro; não provam que o objecto montado é o ficheiro. Só a leitura do
    // que o armazenamento tem prova isso — e é ela que faz «hash final errado»
    // ser uma recusa em vez de uma versão silenciosamente errada.
    let bytes = store.get(&sessao.storage_key).await?;
    let soma = crate::storage::sha256_hex(&bytes);
    if !soma.eq_ignore_ascii_case(sha256) {
        store
            .abort_multipart(&sessao.storage_key, &sessao.storage_upload_id)
            .await;
        store.delete(&sessao.storage_key).await;
        marcar(tx, session_id, "abandoned").await?;
        return Err(CoreError::Validation(
            "A soma do ficheiro montado não corresponde à declarada.".to_owned(),
        ));
    }

    // ── O objecto institucional ─────────────────────────────────────────
    sqlx::query(
        "INSERT INTO storage_objects
             (id, backend_id, organisation_id, unit_id, workspace_id, object_key,
              original_filename, content_type, size_bytes, checksum_sha256,
              classification, status, created_by_id)
         SELECT $1, b.id, $2, $3, $4, $5, $6, $7, $8, $9, $10, 'stored', $11
           FROM storage_backends b
          WHERE b.is_default AND b.is_active",
    )
    .bind(sessao.storage_object_id)
    .bind(principal.organisation_id)
    .bind(workspace.unit_id)
    .bind(workspace.id)
    .bind(&sessao.storage_key)
    .bind(&sessao.filename)
    .bind(&sessao.content_type)
    .bind(montado)
    .bind(&soma)
    .bind(efectiva.as_str())
    .bind(principal.person_id)
    .execute(&mut **tx)
    .await?;

    let versao = match sessao.file_id {
        // Nova versão de um ficheiro que já existe.
        Some(file_id) => {
            super::service::add_version(
                tx,
                ids,
                file_id,
                sessao.storage_object_id,
                None,
                principal.person_id,
            )
            .await?
        }
        None => {
            let ficheiro = super::service::create_with_first_version(
                tx,
                ids,
                super::service::FileContext {
                    organisation_id: principal.organisation_id,
                    unit_id: workspace.unit_id,
                    workspace_id: workspace.id,
                    classification: efectiva,
                },
                &sessao.filename,
                sessao.storage_object_id,
                principal.person_id,
            )
            .await?;
            if let Some(folder_id) = sessao.folder_id {
                super::service::move_to_folder(
                    tx,
                    principal,
                    ids,
                    ficheiro.file_id,
                    Some(folder_id),
                )
                .await?;
            }
            ficheiro
        }
    };

    marcar(tx, session_id, "finalised").await?;

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::CREATE, "file")
            .resource(versao.file_id)
            .context(&super::service::file_context(&workspace, efectiva))
            .classified(efectiva)
            .detail("size_bytes", montado.to_string())
            .detail("parts", sessao.total_parts.to_string())
            .detail("upload_session_id", session_id.to_string()),
    )
    .await?;

    Ok(versao)
}

async fn marcar(tx: &mut Tx<'_>, session_id: Uuid, estado: &str) -> CoreResult<()> {
    sqlx::query(
        "UPDATE upload_sessions
            SET state = $2,
                finalised_at = CASE WHEN $2 = 'finalised' THEN now() ELSE NULL END
          WHERE id = $1",
    )
    .bind(session_id)
    .bind(estado)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Desiste de uma sessão e manda apagar as partes.
///
/// # Errors
///
/// Recusa quando a sessão não é de quem pede.
pub async fn abandon(
    tx: &mut Tx<'_>,
    principal: &Principal,
    store: &ObjectStore,
    session_id: Uuid,
) -> CoreResult<()> {
    let sessao = sessao_de(tx, principal, session_id).await?;
    store
        .abort_multipart(&sessao.storage_key, &sessao.storage_upload_id)
        .await;
    marcar(tx, session_id, "abandoned").await
}

/// Fecha as sessões que expiraram e liberta o que elas seguravam.
///
/// # Porque isto tem de existir
///
/// Porque uma sessão aberta segura partes no armazenamento. Sem limpeza, o
/// espaço cresce, nada o refere, e nenhuma listagem o explica — até ao dia em
/// que o disco enche e o sintoma aparece noutro sítio qualquer.
///
/// # Errors
///
/// Returns a database error.
pub async fn sweep_expired(pool: &sqlx::PgPool, store: Option<&ObjectStore>) -> CoreResult<usize> {
    let expiradas: Vec<(Uuid, String, String)> = sqlx::query_as(
        "SELECT id, storage_key, storage_upload_id
           FROM upload_sessions
          WHERE state = 'open' AND expires_at < now()",
    )
    .fetch_all(pool)
    .await?;

    for (id, key, upload_id) in &expiradas {
        if let Some(store) = store {
            store.abort_multipart(key, upload_id).await;
        }
        sqlx::query("UPDATE upload_sessions SET state = 'abandoned' WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
    }
    Ok(expiradas.len())
}
