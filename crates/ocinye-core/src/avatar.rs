//! Normalização da fotografia de perfil.
//!
//! # Entrada hostil
//!
//! O que chega aqui são bytes que alguém escolheu. Podem não ser uma imagem,
//! podem ser uma imagem que declara ser outra coisa, podem ser uma imagem
//! verdadeira com trinta mil por trinta mil pixels — quinhentos megabytes de
//! memória a partir de um ficheiro comprimido pequeno. Nada disto se descobre
//! olhando para a extensão, e por isso a extensão não é consultada.
//!
//! A ordem importa, e é a barata primeiro:
//!
//! 1. **dimensão em bytes** — antes de qualquer leitura;
//! 2. **assinatura real** — os primeiros bytes decidem o formato, não o que o
//!    cliente declarou;
//! 3. **cabeçalho** — largura, altura e total de pixels, recusados *antes* de
//!    alocar o que quer que seja;
//! 4. **descodificação**;
//! 5. **orientação EXIF** aplicada;
//! 6. **corte quadrado ao centro**;
//! 7. **redimensionamento** para a dimensão canónica;
//! 8. **re-codificação de raiz**.
//!
//! # Porque se re-codifica
//!
//! Guardar o original obrigaria a remover metadata à mão — a escrever
//! analisadores de JPEG, PNG e WebP para lhes cortar segmentos, o que é
//! exactamente o código de leitura de conteúdo hostil que se queria evitar ao
//! usar uma biblioteca especializada.
//!
//! Re-codificar resolve isto sem código nenhum de metadata: o ficheiro que sai
//! é escrito a partir dos pixels, e EXIF, XMP, GPS, perfis de cor, comentários
//! e o que mais lá viesse simplesmente não têm por onde passar. Não são
//! removidos — nunca chegam a existir.

use image::imageops::FilterType;
use image::{ImageDecoder, ImageFormat, ImageReader};
use std::io::Cursor;

use crate::error::{CoreError, CoreResult};

/// Lado do avatar guardado, em pixels.
///
/// Serve o maior sítio onde aparece com folga para ecrãs de alta densidade, e
/// mantém-se pequeno o suficiente para o ficheiro ser irrelevante ao lado de um
/// documento institucional.
pub const AVATAR_SIDE: u32 = 512;

/// Maior fotografia aceite, em bytes.
///
/// Bastante para uma fotografia de telemóvel e insuficiente para servir de
/// depósito. O limite existe antes de qualquer leitura: é a única verificação
/// que não custa nada.
pub const MAX_AVATAR_BYTES: usize = 8 * 1024 * 1024;

/// Maior número de pixels aceite na imagem de origem.
///
/// Um ficheiro de 40 KiB pode declarar 30000×30000 e pedir ao descodificador
/// mais de três gigabytes. O limite de bytes não apanha isto — só o cabeçalho
/// apanha, e por isso lê-se o cabeçalho antes de descodificar.
pub const MAX_SOURCE_PIXELS: u64 = 64_000_000;

/// O tipo de conteúdo do avatar guardado.
pub const AVATAR_CONTENT_TYPE: &str = "image/webp";

/// O resultado da normalização.
#[derive(Debug)]
pub struct NormalisedAvatar {
    /// Os bytes a guardar, já em formato canónico.
    pub data: Vec<u8>,
    /// Checksum do que se guarda, e portanto a versão do conteúdo.
    pub checksum_sha256: String,
}

/// Descobre o formato pela assinatura real dos bytes.
///
/// O `Content-Type` declarado pelo cliente não entra nesta decisão. É por isso
/// que um executável renomeado para `.jpg` e anunciado como `image/jpeg` não
/// passa: a assinatura não mente por ele.
fn formato_real(data: &[u8]) -> CoreResult<ImageFormat> {
    let formato = image::guess_format(data).map_err(|_| {
        CoreError::Validation("The file is not an image the Ocinye OS accepts.".to_owned())
    })?;

    match formato {
        ImageFormat::Jpeg | ImageFormat::Png | ImageFormat::WebP => Ok(formato),
        _ => Err(CoreError::Validation(
            "A profile photograph must be a JPEG, PNG or WebP image.".to_owned(),
        )),
    }
}

/// Normaliza uma fotografia de perfil.
///
/// # Errors
///
/// Devolve [`CoreError::Validation`] quando os bytes não são uma imagem de um
/// formato aceite, quando excedem os limites, ou quando a descodificação falha.
pub fn normalise(data: &[u8]) -> CoreResult<NormalisedAvatar> {
    if data.is_empty() {
        return Err(CoreError::Validation("The file is empty.".to_owned()));
    }
    if data.len() > MAX_AVATAR_BYTES {
        return Err(CoreError::Validation(format!(
            "A profile photograph may not exceed {} MiB.",
            MAX_AVATAR_BYTES / (1024 * 1024)
        )));
    }

    let formato = formato_real(data)?;

    let mut reader = ImageReader::new(Cursor::new(data));
    reader.set_format(formato);
    // Sem isto, o descodificador aceitaria qualquer dimensão que o cabeçalho
    // declarasse, e a alocação aconteceria antes de alguém a poder recusar.
    reader.limits({
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(AVATAR_SIDE * 64);
        limits.max_image_height = Some(AVATAR_SIDE * 64);
        limits.max_alloc = Some(MAX_SOURCE_PIXELS * 4);
        limits
    });

    let mut decoder = reader
        .into_decoder()
        .map_err(|_| CoreError::Validation("The image could not be read.".to_owned()))?;

    let (largura, altura) = decoder.dimensions();
    if u64::from(largura) * u64::from(altura) > MAX_SOURCE_PIXELS {
        return Err(CoreError::Validation(
            "The image has more pixels than the Ocinye OS accepts.".to_owned(),
        ));
    }

    // A orientação lê-se do descodificador porque é a última oportunidade: a
    // partir daqui só há pixels, e o EXIF que dizia «isto está de lado»
    // desaparece com o resto da metadata. Uma fotografia tirada na vertical com
    // um telemóvel ficaria deitada para sempre.
    let orientacao = decoder
        .orientation()
        .unwrap_or(image::metadata::Orientation::NoTransforms);

    let mut imagem = image::DynamicImage::from_decoder(decoder)
        .map_err(|_| CoreError::Validation("The image could not be decoded.".to_owned()))?;
    imagem.apply_orientation(orientacao);

    // Quadrado ao centro, e só depois redimensionar: cortar primeiro evita
    // deformar, que é o que aconteceria ao esticar uma fotografia rectangular
    // para um quadrado. O círculo da interface é apresentação; a imagem
    // guardada já é quadrada.
    let lado = imagem.width().min(imagem.height());
    let x = (imagem.width() - lado) / 2;
    let y = (imagem.height() - lado) / 2;
    let quadrada = imagem.crop_imm(x, y, lado, lado);

    let final_ = quadrada.resize_exact(AVATAR_SIDE, AVATAR_SIDE, FilterType::Lanczos3);

    let mut saida = Vec::new();
    final_
        .to_rgba8()
        .write_to(&mut Cursor::new(&mut saida), ImageFormat::WebP)
        .map_err(|error| {
            tracing::error!(error = ?error, "avatar re-encoding failed");
            CoreError::Internal("The image could not be stored.".to_owned())
        })?;

    let checksum_sha256 = crate::storage::sha256_hex(&saida);
    Ok(NormalisedAvatar {
        data: saida,
        checksum_sha256,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgb};

    /// Uma imagem sólida do tamanho pedido, no formato pedido.
    fn imagem(largura: u32, altura: u32, formato: ImageFormat) -> Vec<u8> {
        let buffer: ImageBuffer<Rgb<u8>, Vec<u8>> =
            ImageBuffer::from_fn(largura, altura, |x, y| {
                Rgb([(x % 256) as u8, (y % 256) as u8, 128])
            });
        let mut saida = Vec::new();
        image::DynamicImage::ImageRgb8(buffer)
            .write_to(&mut Cursor::new(&mut saida), formato)
            .expect("codificar");
        saida
    }

    /// Um JPEG com um segmento EXIF que diz «isto está de lado» e traz GPS.
    ///
    /// Construído à mão porque é a única forma de provar as duas coisas que
    /// importam: que a orientação é lida antes de os pixels ficarem sozinhos, e
    /// que a metadata não sobrevive à re-codificação.
    fn jpeg_com_exif(orientacao: u16) -> Vec<u8> {
        let base = imagem(120, 60, ImageFormat::Jpeg);

        // TIFF little-endian com duas entradas: Orientation e um marcador que
        // se possa procurar nos bytes de saída.
        let marcador = b"OCINYE-GPS-SECRETO";
        let mut tiff = Vec::new();
        tiff.extend_from_slice(b"II\x2a\x00");
        tiff.extend_from_slice(&8u32.to_le_bytes()); // offset da IFD
        tiff.extend_from_slice(&2u16.to_le_bytes()); // duas entradas

        // Orientation (0x0112), SHORT, 1 valor.
        tiff.extend_from_slice(&0x0112u16.to_le_bytes());
        tiff.extend_from_slice(&3u16.to_le_bytes());
        tiff.extend_from_slice(&1u32.to_le_bytes());
        tiff.extend_from_slice(&orientacao.to_le_bytes());
        tiff.extend_from_slice(&[0, 0]);

        // ImageDescription (0x010e), ASCII, com o marcador fora da entrada.
        let offset_marcador = 8 + 2 + 12 * 2 + 4;
        tiff.extend_from_slice(&0x010eu16.to_le_bytes());
        tiff.extend_from_slice(&2u16.to_le_bytes());
        tiff.extend_from_slice(&(marcador.len() as u32 + 1).to_le_bytes());
        tiff.extend_from_slice(&(offset_marcador as u32).to_le_bytes());

        tiff.extend_from_slice(&0u32.to_le_bytes()); // sem IFD seguinte
        tiff.extend_from_slice(marcador);
        tiff.push(0);

        let mut app1 = Vec::new();
        app1.extend_from_slice(b"Exif\0\0");
        app1.extend_from_slice(&tiff);

        let mut saida = Vec::new();
        saida.extend_from_slice(&base[..2]); // SOI
        saida.extend_from_slice(&[0xFF, 0xE1]);
        saida.extend_from_slice(&((app1.len() + 2) as u16).to_be_bytes());
        saida.extend_from_slice(&app1);
        saida.extend_from_slice(&base[2..]);
        saida
    }

    #[test]
    fn uma_fotografia_sai_quadrada_e_canonica() {
        for formato in [ImageFormat::Jpeg, ImageFormat::Png, ImageFormat::WebP] {
            let entrada = imagem(800, 400, formato);
            let saida = normalise(&entrada).expect("normalizar");

            let lida = image::load_from_memory(&saida.data).expect("a saída é uma imagem");
            assert_eq!(lida.width(), AVATAR_SIDE);
            assert_eq!(lida.height(), AVATAR_SIDE);
            assert_eq!(
                image::guess_format(&saida.data).expect("formato"),
                ImageFormat::WebP,
                "{formato:?} não produziu o formato canónico"
            );
            assert_ne!(
                saida.data, entrada,
                "a saída é o ficheiro de origem: não houve normalização"
            );
        }
    }

    /// A orientação é aplicada, e depois desaparece.
    ///
    /// Duas asserções que só fazem sentido juntas. Se a metadata sobrevivesse,
    /// a imagem seria rodada duas vezes — uma aqui e outra pelo browser. Se a
    /// orientação não fosse aplicada antes de a metadata desaparecer, a
    /// fotografia ficaria deitada para sempre.
    #[test]
    fn a_orientacao_e_aplicada_e_a_metadata_nao_sobrevive() {
        // Orientação 6: rodar 90° à direita. Uma imagem 120×60 passa a 60×120,
        // e o corte quadrado ao centro deixa de ser o mesmo recorte.
        let entrada = jpeg_com_exif(6);
        assert!(
            entrada
                .windows(18)
                .any(|janela| janela == b"OCINYE-GPS-SECRETO"),
            "a fixture não tem a metadata que o teste procura"
        );

        let saida = normalise(&entrada).expect("normalizar");

        assert!(
            !saida
                .data
                .windows(18)
                .any(|janela| janela == b"OCINYE-GPS-SECRETO"),
            "a metadata do ficheiro de origem sobreviveu ao armazenamento"
        );
        assert!(
            !saida.data.windows(4).any(|janela| janela == b"Exif"),
            "o segmento EXIF sobreviveu"
        );

        // E a rotação aconteceu: comparada com a mesma imagem sem EXIF, a
        // fotografia rodada não dá o mesmo resultado.
        let sem_exif = normalise(&jpeg_com_exif(1)).expect("normalizar");
        assert_ne!(
            saida.checksum_sha256, sem_exif.checksum_sha256,
            "a orientação EXIF não foi aplicada: rodada e não rodada deram o mesmo"
        );
    }

    #[test]
    fn a_versao_e_determinada_pelo_conteudo() {
        let uma = normalise(&imagem(300, 300, ImageFormat::Png)).expect("normalizar");
        let igual = normalise(&imagem(300, 300, ImageFormat::Png)).expect("normalizar");
        let outra = normalise(&imagem(300, 200, ImageFormat::Png)).expect("normalizar");

        assert_eq!(uma.checksum_sha256, igual.checksum_sha256);
        assert_ne!(uma.checksum_sha256, outra.checksum_sha256);
        assert_eq!(uma.checksum_sha256.len(), 64);
    }

    /// O que não é uma imagem aceite é recusado, e a declaração do cliente não
    /// muda nada.
    #[test]
    fn o_que_nao_e_imagem_aceite_e_recusado() {
        // Um SVG é um documento com script, não uma imagem.
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg"><script>alert(1)</script></svg>"#;
        assert!(normalise(svg).is_err(), "um SVG foi aceite como fotografia");

        // Um executável renomeado continua a não ser uma imagem.
        assert!(normalise(b"\x7fELF\x02\x01\x01\x00 e o resto").is_err());
        assert!(normalise(b"").is_err());
        assert!(normalise(b"nao sou uma imagem").is_err());

        // Um GIF é uma imagem verdadeira, reconhecida pela assinatura, e é
        // recusado à mesma — pela allow-list, e não por acaso.
        //
        // A distinção importa: o codec do GIF também não está compilado, e sem
        // olhar para a mensagem este caso passaria mesmo que a allow-list fosse
        // removida. Foi o que aconteceu quando se tentou reverti-la.
        let gif =
            b"GIF89a\x01\x00\x01\x00\x00\xff\x00,\x00\x00\x00\x00\x01\x00\x01\x00\x00\x02\x00;";
        let erro = normalise(gif).expect_err("um GIF foi aceite");
        assert!(
            matches!(erro, CoreError::Validation(ref m) if m.contains("JPEG, PNG or WebP")),
            "o GIF não foi recusado pela allow-list: {erro:?}"
        );

        // Um JPEG truncado a meio não passa por ser um começo válido.
        let truncado = &imagem(200, 200, ImageFormat::Jpeg)[..40];
        assert!(normalise(truncado).is_err(), "um JPEG truncado foi aceite");
    }

    #[test]
    fn um_ficheiro_grande_de_mais_e_recusado_antes_de_ser_lido() {
        let grande = vec![0u8; MAX_AVATAR_BYTES + 1];
        let erro = normalise(&grande).expect_err("devia recusar");
        assert!(
            matches!(erro, CoreError::Validation(ref m) if m.contains("MiB")),
            "a recusa não foi pelo tamanho: {erro:?}"
        );
    }
}
