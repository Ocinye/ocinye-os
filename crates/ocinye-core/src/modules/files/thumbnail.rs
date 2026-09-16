//! Miniaturas: dar à grelha de Ficheiros o conteúdo em vez de um ícone.
//!
//! # A propriedade
//!
//! > **Um `FileVersion` de imagem pode produzir uma miniatura derivada,
//! > reconstruível e ligada à versão exacta; a grelha mostra-a em vez de um
//! > ícone genérico, sem que ela se torne autoridade e sem alterar a validade
//! > do ficheiro se a geração falhar.**
//!
//! # O que isto não faz
//!
//! Não é uma versão. Uma miniatura é um objecto derivado — não citável, fora do
//! histórico, e nunca onde uma versão aparece. Liga-se por `file_thumbnails`, e
//! não reutilizando `file_versions`.
//!
//! Não autoriza. Quem a vê decide-se pela posse da versão de origem, ao servir.
//!
//! Não substitui o ficheiro. É uma leitura visual, feita por um gerador
//! concreto, sobre uma versão concreta — e é por isso que essa identidade fica
//! guardada. A geração re-codifica de raiz: os pixels entram, uma WebP sai, e a
//! metadata da origem (EXIF, GPS, perfis) não tem por onde passar.

use image::imageops::FilterType;
use image::{ImageDecoder, ImageFormat, ImageReader};
use serde_json::json;
use std::io::Cursor;
use uuid::Uuid;

use crate::error::{CoreError, CoreResult};
use crate::storage::{self, ObjectStore};
use crate::Tx;
use ocinye_contracts::Classification;
use ocinye_domain::Principal;
use ocinye_observability::CorrelationIds;

use super::service::InlinePreview;

/// O evento que põe uma versão na fila da geração de miniatura.
pub const EVENT_THUMBNAIL: &str = "file_version.thumbnail_requested";

/// O maior lado da miniatura. A imagem cabe dentro de um quadrado deste lado,
/// preservando a proporção — não se corta nem se deforma.
pub const THUMB_MAX_SIDE: u32 = 480;

/// O tipo do derivado guardado. Re-codifica-se sempre para este.
pub const THUMB_CONTENT_TYPE: &str = "image/webp";

/// Quem gerou, para a linha o guardar.
pub const GENERATOR_NAME: &str = "ocinye-image-thumbnailer";
/// A versão do gerador. Muda quando a forma da saída mudar.
pub const GENERATOR_VERSION: &str = "1";

/// O maior objecto de origem que se lê para gerar. Um limite explícito, e não a
/// memória da máquina a decidir por acidente.
pub const MAX_SOURCE_BYTES: i64 = 32 * 1024 * 1024;

/// O tecto de pixels da origem, recusado a partir do cabeçalho, antes de alocar.
const MAX_SOURCE_PIXELS: u64 = 64_000_000;

/// Os rasters, que se descodificam **no processo** pela biblioteca `image` — o
/// risco é o dela, e não há parser externo a correr. Um SVG fica de fora: é um
/// documento com script.
pub const RASTER_TYPES: [&str; 3] = ["image/png", "image/jpeg", "image/webp"];

/// Os documentos, rasterizados a uma imagem **na fronteira isolada**: o PDF
/// directo, e os formatos de escritório que o LibreOffice converte a PDF antes.
/// Nenhum executa scripts nem macros (spec §76): o PDF por um renderizador, o
/// Office num LibreOffice `--headless` sem rede, ambos num contentor descartável.
pub const DOCUMENT_TYPES: [&str; 11] = [
    "application/pdf",
    "application/msword",
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    "application/vnd.ms-excel",
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    "application/vnd.ms-powerpoint",
    "application/vnd.openxmlformats-officedocument.presentationml.presentation",
    "application/vnd.oasis.opendocument.text",
    "application/vnd.oasis.opendocument.spreadsheet",
    "application/vnd.oasis.opendocument.presentation",
    "application/rtf",
];

/// Os vídeos, de que se extrai um fotograma **na fronteira isolada**, com o
/// `ffmpeg`.
pub const VIDEO_TYPES: [&str; 5] = [
    "video/mp4",
    "video/webm",
    "video/quicktime",
    "video/x-matroska",
    "video/x-msvideo",
];

/// Um raster descodifica-se no processo; tudo o resto vai à fronteira.
#[must_use]
pub fn is_raster(content_type: &str) -> bool {
    RASTER_TYPES.contains(&content_type)
}

/// De que tipos de origem se gera uma miniatura de todo.
#[must_use]
pub fn is_thumbnailable(content_type: &str) -> bool {
    RASTER_TYPES.contains(&content_type)
        || DOCUMENT_TYPES.contains(&content_type)
        || VIDEO_TYPES.contains(&content_type)
}

/// A fronteira que converte conteúdo não confiável fora do processo do worker.
///
/// Um raster descodifica-se no processo ([`is_raster`]) — é a biblioteca `image`,
/// e o risco é o dela. Um documento ou um vídeo é outra coisa: convertê-lo é
/// correr um parser grande (poppler, LibreOffice, ffmpeg) sobre bytes
/// potencialmente hostis, e isso **não** deve acontecer dentro do worker. Esta
/// trait é o ponto onde a conversão sai para uma fronteira isolada — em produção,
/// um contentor descartável e endurecido gerido pelo Conversion Runner
/// (ADR-0609). O domínio não sabe como a fronteira isola nem que ferramenta
/// corre; sabe que dá um tipo e bytes e recebe uma imagem, e trata o resultado
/// como ainda não confiável (re-codifica-o de raiz em [`gerar`]).
///
/// Injecta-se em [`process`] para que o `ocinye-core` não conheça nem Docker nem
/// subprocessos: quem os conhece é o worker, que constrói a implementação e
/// mapeia o tipo para o perfil de conversão.
#[async_trait::async_trait]
pub trait ConversionBoundary: Send + Sync {
    /// Converte bytes não confiáveis do tipo dado numa imagem (PNG), numa
    /// fronteira isolada. Só se chama para tipos que não são raster.
    ///
    /// # Errors
    ///
    /// [`CoreError::Validation`] quando o conteúdo não converte — um estado de
    /// miniatura, não uma avaria, e o ficheiro fica com o ícone.
    /// [`CoreError::Internal`] quando a própria fronteira não respondeu, que é
    /// retentável pelo outbox.
    async fn to_thumbnail_image(&self, content_type: &str, bytes: &[u8]) -> CoreResult<Vec<u8>>;
}

/// O estado da geração — que não é o estado do armazenamento.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Estado {
    /// Pedida, à espera do worker.
    Queued,
    /// Gerada, com objecto derivado.
    Ready,
    /// O tipo não produz miniatura. Estado, não erro.
    Unsupported,
    /// A geração falhou sobre bytes que deviam servir.
    Failed,
}

impl Estado {
    fn as_str(self) -> &'static str {
        match self {
            Estado::Queued => "QUEUED",
            Estado::Ready => "READY",
            Estado::Unsupported => "UNSUPPORTED",
            Estado::Failed => "FAILED",
        }
    }

    fn parse(valor: &str) -> Self {
        match valor {
            "READY" => Estado::Ready,
            "UNSUPPORTED" => Estado::Unsupported,
            "FAILED" => Estado::Failed,
            _ => Estado::Queued,
        }
    }
}

/// Põe uma versão na fila da geração de miniatura.
///
/// Idempotente: pedir duas vezes deixa a linha como está, e um evento
/// reentregue é o caso normal. A inserção vive na transacção que cria a versão,
/// pelo que a fila e a versão nascem ou falham juntas.
///
/// # Errors
///
/// Devolve erro quando a inserção falha.
pub async fn queue(tx: &mut Tx<'_>, file_version_id: Uuid, ids: &CorrelationIds) -> CoreResult<()> {
    sqlx::query(
        "INSERT INTO file_thumbnails (file_version_id, status)
         VALUES ($1, 'QUEUED')
         ON CONFLICT (file_version_id) DO NOTHING",
    )
    .bind(file_version_id)
    .execute(&mut **tx)
    .await?;

    crate::outbox::emit(
        tx,
        EVENT_THUMBNAIL,
        "file_version",
        file_version_id,
        &ids.correlation_id,
        json!({ "file_version_id": file_version_id }),
    )
    .await?;

    Ok(())
}

/// Garante que uma versão pessoal de imagem está na fila — usado ao servir uma
/// miniatura que ainda não existe, para os ficheiros anteriores à
/// funcionalidade a ganharem sem um preenchimento em massa.
///
/// A posse é a autoridade: uma versão que não seja do dono não faz nada e não
/// revela que existe. Um tipo que não gera miniatura também não entra na fila.
///
/// # Errors
///
/// Devolve erro quando a base não responde.
pub async fn ensure_queued_personal(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    file_version_id: Uuid,
) -> CoreResult<()> {
    if !super::service::owns_personal_file_version(&mut *tx, principal, file_version_id).await? {
        return Ok(());
    }

    let tipo: Option<String> = sqlx::query_scalar(
        "SELECT o.content_type
           FROM file_versions v
           JOIN storage_objects o ON o.id = v.storage_object_id
          WHERE v.id = $1",
    )
    .bind(file_version_id)
    .fetch_optional(&mut **tx)
    .await?;

    if tipo.as_deref().is_some_and(is_thumbnailable) {
        queue(tx, file_version_id, ids).await?;
    }

    Ok(())
}

/// O que o worker precisa para gerar a miniatura de uma versão.
struct Trabalho {
    object_key: String,
    content_type: String,
    size_bytes: i64,
    owner_id: Option<Uuid>,
    organisation_id: Uuid,
}

/// Reclama a linha e devolve o que é preciso, ou `None` quando já não há nada a
/// fazer — porque já está `READY`/`UNSUPPORTED`, ou a versão desapareceu.
///
/// # Errors
///
/// Devolve erro quando a consulta falha.
async fn claim(tx: &mut Tx<'_>, file_version_id: Uuid) -> CoreResult<Option<Trabalho>> {
    // `FOR UPDATE` na linha da miniatura: dois workers com o mesmo evento
    // esperam um pelo outro, e o segundo vê o estado que o primeiro deixou.
    let estado: Option<String> = sqlx::query_scalar(
        "SELECT status FROM file_thumbnails WHERE file_version_id = $1 FOR UPDATE",
    )
    .bind(file_version_id)
    .fetch_optional(&mut **tx)
    .await?;

    let Some(estado) = estado else {
        return Ok(None);
    };

    // Assente. Regenerar é uma operação deliberada, e não o que acontece porque
    // um evento chegou outra vez. `FAILED` também fica — reprocessar bytes que
    // já falharam sem mudar nada só repetiria a falha.
    if matches!(
        Estado::parse(&estado),
        Estado::Ready | Estado::Unsupported | Estado::Failed
    ) {
        return Ok(None);
    }

    let linha: Option<(String, String, i64, Option<Uuid>, Uuid)> = sqlx::query_as(
        "SELECT o.object_key, o.content_type, o.size_bytes, o.owner_id, o.organisation_id
           FROM file_versions v
           JOIN storage_objects o ON o.id = v.storage_object_id
          WHERE v.id = $1",
    )
    .bind(file_version_id)
    .fetch_optional(&mut **tx)
    .await?;

    let Some((object_key, content_type, size_bytes, owner_id, organisation_id)) = linha else {
        return Ok(None);
    };

    Ok(Some(Trabalho {
        object_key,
        content_type,
        size_bytes,
        owner_id,
        organisation_id,
    }))
}

/// Marca o estado assente de uma geração que não produziu objecto.
async fn record_estado(
    tx: &mut Tx<'_>,
    file_version_id: Uuid,
    estado: Estado,
    razao: Option<&str>,
) -> CoreResult<Estado> {
    sqlx::query(
        "UPDATE file_thumbnails
            SET status = $2, failure_reason = $3, updated_at = now()
          WHERE file_version_id = $1",
    )
    .bind(file_version_id)
    .bind(estado.as_str())
    .bind(razao)
    .execute(&mut **tx)
    .await?;
    Ok(estado)
}

/// Uma miniatura gerada, pronta a guardar.
struct Miniatura {
    data: Vec<u8>,
    width: u32,
    height: u32,
    checksum: String,
}

/// Gera a miniatura a partir dos bytes de origem — a barata primeiro.
///
/// Assinatura real (não a extensão), cabeçalho recusado antes de alocar,
/// orientação EXIF aplicada, redimensionar preservando a proporção, e
/// re-codificar para WebP a partir dos pixels.
fn gerar(source: &[u8]) -> CoreResult<Miniatura> {
    let formato = image::guess_format(source)
        .map_err(|_| CoreError::Validation("Os bytes não são uma imagem legível.".to_owned()))?;
    if !matches!(
        formato,
        ImageFormat::Jpeg | ImageFormat::Png | ImageFormat::WebP
    ) {
        return Err(CoreError::Validation(
            "Este formato de imagem não gera miniatura.".to_owned(),
        ));
    }

    let mut reader = ImageReader::new(Cursor::new(source));
    reader.set_format(formato);
    reader.limits({
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(THUMB_MAX_SIDE * 64);
        limits.max_image_height = Some(THUMB_MAX_SIDE * 64);
        limits.max_alloc = Some(MAX_SOURCE_PIXELS * 4);
        limits
    });

    let mut decoder = reader
        .into_decoder()
        .map_err(|_| CoreError::Validation("A imagem não pôde ser lida.".to_owned()))?;

    let (largura, altura) = decoder.dimensions();
    if u64::from(largura) * u64::from(altura) > MAX_SOURCE_PIXELS {
        return Err(CoreError::Validation(
            "A imagem tem mais pixels do que o Ocinye OS aceita.".to_owned(),
        ));
    }

    let orientacao = decoder
        .orientation()
        .unwrap_or(image::metadata::Orientation::NoTransforms);

    let mut imagem = image::DynamicImage::from_decoder(decoder)
        .map_err(|_| CoreError::Validation("A imagem não pôde ser descodificada.".to_owned()))?;
    imagem.apply_orientation(orientacao);

    // `resize` preserva a proporção e cabe dentro do quadrado. Uma imagem já
    // mais pequena não se amplia — não há detalhe para inventar.
    let mini = if imagem.width() > THUMB_MAX_SIDE || imagem.height() > THUMB_MAX_SIDE {
        imagem.resize(THUMB_MAX_SIDE, THUMB_MAX_SIDE, FilterType::Lanczos3)
    } else {
        imagem
    };
    let (width, height) = (mini.width(), mini.height());

    let mut saida = Vec::new();
    mini.to_rgba8()
        .write_to(&mut Cursor::new(&mut saida), ImageFormat::WebP)
        .map_err(|error| {
            tracing::error!(error = ?error, "thumbnail re-encoding failed");
            CoreError::Internal("A miniatura não pôde ser guardada.".to_owned())
        })?;

    let checksum = storage::sha256_hex(&saida);
    Ok(Miniatura {
        data: saida,
        width,
        height,
        checksum,
    })
}

/// Guarda o objecto derivado da miniatura e devolve o seu id.
///
/// Não passa pela admissão de quota: uma miniatura não é um carregamento do
/// membro, é um derivado do sistema, e recusá-la a quem está no limite deixaria
/// a grelha sem miniaturas exactamente a quem mais ficheiros tem. Herda o dono
/// da origem, para desaparecer com a pessoa como o resto do que é seu.
async fn guardar_objecto(
    tx: &mut Tx<'_>,
    store: &ObjectStore,
    organisation_id: Uuid,
    owner_id: Option<Uuid>,
    mini: &Miniatura,
) -> CoreResult<Uuid> {
    let slug: String = sqlx::query_scalar("SELECT slug FROM organisations WHERE id = $1")
        .bind(organisation_id)
        .fetch_one(&mut **tx)
        .await?;

    let object_id = Uuid::new_v4();
    // A chave é opaca. Sem dono (um derivado institucional de uma imagem de
    // nota), cai numa gaveta de derivados da organização.
    let object_key = match owner_id {
        Some(dono) => storage::build_object_key_personal(&slug, dono, object_id),
        None => format!("{slug}/thumbnails/{object_id}"),
    };
    let size = i64::try_from(mini.data.len())
        .map_err(|_| CoreError::Internal("A miniatura é demasiado grande.".to_owned()))?;

    let registo = sqlx::query(
        "INSERT INTO storage_objects
             (id, backend_id, organisation_id, owner_id, object_key,
              original_filename, content_type, size_bytes, checksum_sha256,
              classification, status, created_by_id)
         SELECT $1, b.id, $2, $3, $4, 'thumbnail.webp', $5, $6, $7, $8, 'stored', $3
           FROM storage_backends b
          WHERE b.is_default AND b.is_active",
    )
    .bind(object_id)
    .bind(organisation_id)
    .bind(owner_id)
    .bind(&object_key)
    .bind(THUMB_CONTENT_TYPE)
    .bind(size)
    .bind(&mini.checksum)
    .bind(Classification::Internal.as_str())
    .execute(&mut **tx)
    .await?;

    if registo.rows_affected() == 0 {
        return Err(CoreError::StorageUnavailable(
            "Esta instalação não tem armazenamento registado.".to_owned(),
        ));
    }

    store
        .put(
            &object_key,
            THUMB_CONTENT_TYPE,
            &mini.checksum,
            mini.data.clone(),
        )
        .await?;

    Ok(object_id)
}

/// Gera a miniatura de uma versão de ponta a ponta: reclama, lê, gera, guarda.
///
/// É esta a função que o worker chama. Vive aqui, e não no worker, porque o que
/// fazer com um formato sem miniatura, ou com um descodificador que falha, é
/// decisão do domínio.
///
/// # Errors
///
/// Devolve erro quando a base ou o armazenamento não respondem — o outbox volta
/// a tentar. Um formato sem miniatura e uma descodificação que falha **não** são
/// erros: são estados, e ficam registados como tal.
pub async fn process(
    tx: &mut Tx<'_>,
    store: &ObjectStore,
    converter: &dyn ConversionBoundary,
    file_version_id: Uuid,
) -> CoreResult<Option<Estado>> {
    let Some(trabalho) = claim(tx, file_version_id).await? else {
        return Ok(None);
    };

    if !is_thumbnailable(&trabalho.content_type) {
        return Ok(Some(
            record_estado(tx, file_version_id, Estado::Unsupported, None).await?,
        ));
    }

    if trabalho.size_bytes > MAX_SOURCE_BYTES {
        return Ok(Some(
            record_estado(
                tx,
                file_version_id,
                Estado::Failed,
                Some("a imagem excede o limite de geração de miniatura"),
            )
            .await?,
        ));
    }

    // Se o armazenamento não responde, **é** um erro: o outbox volta a tentar.
    // Não se marca `FAILED`, que afirmaria que a imagem não gera miniatura
    // quando o que aconteceu foi o disco não atender.
    let bytes = store.get(&trabalho.object_key).await?;

    // Um raster segue directo — descodifica-se no processo pela `image`. Um
    // documento ou um vídeo sai para a fronteira de conversão: em produção, um
    // contentor descartável e endurecido que não executa scripts nem macros e
    // não alcança o resto do sistema (spec §76, ADR-0609).
    let pixels = if is_raster(&trabalho.content_type) {
        bytes.clone()
    } else {
        match converter
            .to_thumbnail_image(&trabalho.content_type, &bytes)
            .await
        {
            Ok(imagem) => imagem,
            Err(CoreError::Validation(razao)) => {
                return Ok(Some(
                    record_estado(tx, file_version_id, Estado::Failed, Some(&razao)).await?,
                ));
            }
            Err(outro) => return Err(outro),
        }
    };

    let mini = match gerar(&pixels) {
        Ok(mini) => mini,
        Err(CoreError::Validation(razao)) => {
            return Ok(Some(
                record_estado(tx, file_version_id, Estado::Failed, Some(&razao)).await?,
            ));
        }
        Err(outro) => return Err(outro),
    };

    let (width, height) = (mini.width, mini.height);
    let source_checksum = storage::sha256_hex(&bytes);
    let object_id = guardar_objecto(
        tx,
        store,
        trabalho.organisation_id,
        trabalho.owner_id,
        &mini,
    )
    .await?;

    sqlx::query(
        "UPDATE file_thumbnails
            SET status = 'READY',
                thumbnail_object_id = $2,
                width = $3, height = $4,
                source_checksum_sha256 = $5,
                generator_name = $6, generator_version = $7,
                failure_reason = NULL, updated_at = now()
          WHERE file_version_id = $1",
    )
    .bind(file_version_id)
    .bind(object_id)
    .bind(i32::try_from(width).unwrap_or(0))
    .bind(i32::try_from(height).unwrap_or(0))
    .bind(&source_checksum)
    .bind(GENERATOR_NAME)
    .bind(GENERATOR_VERSION)
    .execute(&mut **tx)
    .await?;

    Ok(Some(Estado::Ready))
}

/// Os bytes da miniatura de uma versão pessoal, para a grelha os mostrar inline.
///
/// A posse é a autoridade, reavaliada aqui: uma versão que não seja do dono
/// responde «não encontrado». Devolve `None` quando ainda não há miniatura
/// pronta — a grelha cai no ícone do tipo, sem erro.
///
/// # Errors
///
/// [`CoreError::NotFound`] quando a versão não é do dono; erro de armazenamento
/// quando o objecto não responde.
pub async fn read_thumbnail_personal(
    tx: &mut Tx<'_>,
    principal: &Principal,
    store: &ObjectStore,
    file_version_id: Uuid,
) -> CoreResult<Option<InlinePreview>> {
    if !super::service::owns_personal_file_version(&mut *tx, principal, file_version_id).await? {
        return Err(CoreError::NotFound("Ficheiro não encontrado.".to_owned()));
    }

    let linha: Option<(String, String, String)> = sqlx::query_as(
        "SELECT o.object_key, o.content_type, o.checksum_sha256
           FROM file_thumbnails t
           JOIN storage_objects o ON o.id = t.thumbnail_object_id
          WHERE t.file_version_id = $1 AND t.status = 'READY'",
    )
    .bind(file_version_id)
    .fetch_optional(&mut **tx)
    .await?;

    let Some((object_key, content_type, checksum)) = linha else {
        return Ok(None);
    };

    let bytes = store.get(&object_key).await?;
    Ok(Some(InlinePreview {
        content_type,
        bytes,
        checksum_sha256: checksum,
    }))
}

/// Apaga os objectos de miniatura das versões de um ficheiro pessoal e devolve
/// as suas chaves, para o chamador remover os bytes depois de a transacção
/// fechar.
///
/// Usada ao apagar definitivamente: uma miniatura é um objecto derivado que não
/// sai com o objecto de origem (não é uma versão), pelo que sem isto ficariam a
/// linha em `storage_objects` e os bytes órfãos no armazenamento.
///
/// # Errors
///
/// Devolve erro quando a escrita falha.
pub async fn purge_personal_thumbnail_objects(
    tx: &mut Tx<'_>,
    file_id: Uuid,
) -> CoreResult<Vec<String>> {
    // `DELETE ... USING` apaga o objecto derivado e devolve a chave. A ligação
    // em `file_thumbnails` fica a `NULL` (a FK é `SET NULL`) e desaparece a
    // seguir, quando o ficheiro é apagado em cascata.
    let chaves: Vec<String> = sqlx::query_scalar(
        "DELETE FROM storage_objects o
           USING file_thumbnails t, file_versions v
          WHERE o.id = t.thumbnail_object_id
            AND t.file_version_id = v.id
            AND v.file_id = $1
        RETURNING o.object_key",
    )
    .bind(file_id)
    .fetch_all(&mut **tx)
    .await?;
    Ok(chaves)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(largura: u32, altura: u32) -> Vec<u8> {
        let imagem = image::RgbaImage::from_pixel(largura, altura, image::Rgba([12, 40, 74, 255]));
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgba8(imagem)
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();
        bytes
    }

    #[test]
    fn gera_miniatura_preservando_a_proporcao() {
        // 800×600 cabe em 480 preservando a proporção: 480×360.
        let mini = gerar(&png(800, 600)).expect("miniatura");
        assert_eq!((mini.width, mini.height), (480, 360));
        assert!(!mini.data.is_empty(), "a miniatura não tem bytes");
        assert_eq!(
            image::guess_format(&mini.data).ok(),
            Some(ImageFormat::WebP),
            "a miniatura devia sair em WebP"
        );
    }

    #[test]
    fn uma_imagem_pequena_nao_se_amplia() {
        // 100×80 já cabe: fica como está, não se inventa detalhe.
        let mini = gerar(&png(100, 80)).expect("miniatura");
        assert_eq!((mini.width, mini.height), (100, 80));
    }

    #[test]
    fn lixo_nao_e_imagem() {
        assert!(gerar(b"isto nao e uma imagem nenhuma").is_err());
    }

    /// CRC-32/ISO-HDLC, o que o PNG usa nos seus chunks. Escrito à mão para o
    /// teste não depender de uma crate só para forjar um cabeçalho.
    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc: u32 = 0xFFFF_FFFF;
        for &b in bytes {
            crc ^= u32::from(b);
            for _ in 0..8 {
                let mask = (crc & 1).wrapping_neg();
                crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
            }
        }
        !crc
    }

    /// Um PNG que **declara** `largura`×`altura` no IHDR mas não traz os pixels —
    /// o cabeçalho de uma bomba de descompressão. O ficheiro fica minúsculo; a
    /// imagem que ele afirma ter é gigantesca.
    fn png_declarando(largura: u32, altura: u32) -> Vec<u8> {
        fn chunk(tipo: &[u8; 4], dados: &[u8]) -> Vec<u8> {
            let mut c = Vec::new();
            c.extend_from_slice(&(dados.len() as u32).to_be_bytes());
            c.extend_from_slice(tipo);
            c.extend_from_slice(dados);
            let mut crc_in = tipo.to_vec();
            crc_in.extend_from_slice(dados);
            c.extend_from_slice(&crc32(&crc_in).to_be_bytes());
            c
        }
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&largura.to_be_bytes());
        ihdr.extend_from_slice(&altura.to_be_bytes());
        ihdr.extend_from_slice(&[8, 2, 0, 0, 0]); // 8 bits, RGB, sem compressão/filtro/entrelaçado exóticos
        let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        png.extend_from_slice(&chunk(b"IHDR", &ihdr));
        // Um IDAT com um fluxo zlib vazio válido, e o IEND — o suficiente para o
        // descodificador ler o cabeçalho e devolver as dimensões.
        png.extend_from_slice(&chunk(
            b"IDAT",
            &[
                0x78, 0x9C, 0x01, 0x00, 0x00, 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x01,
            ],
        ));
        png.extend_from_slice(&chunk(b"IEND", &[]));
        png
    }

    /// Uma imagem que declara mais pixels do que o Ocinye aceita é recusada
    /// **antes** de se alocar — o cabeçalho chega, os pixels não precisam de vir.
    ///
    /// É a defesa contra a bomba de descompressão (#14): 20000×20000 = 400M
    /// pixels afirmados num ficheiro de dezenas de bytes. `gerar` lê as dimensões
    /// do IHDR e recusa acima de [`MAX_SOURCE_PIXELS`], sem descodificar o IDAT.
    #[test]
    fn uma_imagem_com_dimensoes_absurdas_e_recusada_antes_de_alocar() {
        let bomba = png_declarando(20_000, 20_000);
        assert!(
            bomba.len() < 4096,
            "o cabeçalho da bomba devia ser minúsculo, tinha {} bytes",
            bomba.len()
        );
        assert!(
            u64::from(20_000u32) * u64::from(20_000u32) > MAX_SOURCE_PIXELS,
            "o teste tem de afirmar mais pixels do que o tecto"
        );
        let resultado = gerar(&bomba);
        assert!(
            resultado.is_err(),
            "uma imagem de 400M pixels foi aceite — a bomba passou"
        );
    }
}
