//! `ocinye-convert` — o que corre **dentro** do contentor descartável.
//!
//! Não fala com a rede, não conhece segredos, não sabe onde o objecto vive. Lê
//! bytes não confiáveis de `/in/input` (montado só-leitura), executa a
//! ferramenta do perfil, e escreve o derivado em `/out/output`. Se falhar, sai
//! não-zero e não escreve nada — o runner lê o código de saída, não uma
//! afirmação do processo.
//!
//! É a fronteira que separa o conteúdo hostil do resto do sistema: aqui dentro
//! está quase vazio de propósito, para que uma vulnerabilidade de parsing
//! encontre uma caixa descartável, e não o worker da Ocinye.
//!
//! # Argumentos
//!
//! `ocinye-convert <perfil> [extensão]`. A extensão (letras e dígitos, curta)
//! serve o LibreOffice, que precisa de um ficheiro com a extensão certa para
//! detectar o formato; o `pdftoppm` e o `ffmpeg` cheiram o conteúdo e ignoram-na.

use std::path::Path;
use std::process::{Command, ExitCode};

const ENTRADA: &str = "/in/input";
const SAIDA: &str = "/out/output";
/// O lado maior da imagem produzida. O mesmo que a geração de miniatura usa.
const LADO: &str = "960";

fn main() -> ExitCode {
    let perfil = match std::env::args().nth(1) {
        Some(nome) => nome,
        None => {
            eprintln!("ocinye-convert: falta o nome do perfil");
            return ExitCode::from(2);
        }
    };
    // A extensão é opcional e higienizada: só letras e dígitos, no máximo oito.
    // Um valor fora disto é ignorado, para nunca compor um caminho inesperado.
    let extensao = std::env::args()
        .nth(2)
        .filter(|e| !e.is_empty() && e.len() <= 8 && e.chars().all(|c| c.is_ascii_alphanumeric()));

    if ocinye_conversion_runner::profile(&perfil).is_none() {
        eprintln!("ocinye-convert: perfil desconhecido: {perfil}");
        return ExitCode::from(2);
    }

    if !Path::new(ENTRADA).exists() {
        eprintln!("ocinye-convert: não há entrada em {ENTRADA}");
        return ExitCode::from(3);
    }

    let resultado = match perfil.as_str() {
        "pdf-thumbnail" => rasterizar_pdf(ENTRADA),
        "office-thumbnail" => miniatura_de_office(extensao.as_deref()),
        "video-thumbnail" => fotograma_de_video(),
        outro => {
            eprintln!("ocinye-convert: perfil sem execução: {outro}");
            return ExitCode::from(2);
        }
    };

    match resultado {
        Ok(()) => ExitCode::SUCCESS,
        Err(mensagem) => {
            eprintln!("ocinye-convert: {mensagem}");
            ExitCode::from(1)
        }
    }
}

/// Rasteriza a primeira página de um PDF numa imagem PNG.
///
/// `pdftoppm` é um **renderizador**, não um motor de scripts: não executa o
/// JavaScript embutido no documento (spec §76). Escreve para o `/tmp` (tmpfs) e
/// só depois move para `/out`, porque `-singlefile` nomeia a saída como
/// `{base}.png` e o que o runner lê é exactamente `/out/output`.
fn rasterizar_pdf(pdf: &str) -> Result<(), String> {
    let base = "/tmp/oc-page";
    let produzido = format!("{base}.png");

    let saida = Command::new("pdftoppm")
        .args([
            "-png",
            "-f",
            "1",
            "-l",
            "1",
            "-singlefile",
            "-scale-to",
            LADO,
            pdf,
            base,
        ])
        .output()
        .map_err(|erro| format!("não foi possível correr o pdftoppm: {erro}"))?;

    if !saida.status.success() {
        return Err(format!(
            "o pdftoppm terminou com {}: {}",
            saida.status,
            String::from_utf8_lossy(&saida.stderr).trim()
        ));
    }

    let bytes =
        std::fs::read(&produzido).map_err(|_| "a rasterização não produziu imagem".to_owned())?;
    std::fs::write(SAIDA, bytes)
        .map_err(|erro| format!("não foi possível escrever o derivado: {erro}"))?;
    Ok(())
}

/// Miniatura de um documento de escritório: LibreOffice converte-o a PDF, e daí
/// segue o caminho do PDF.
///
/// O LibreOffice precisa de um ficheiro com a extensão certa para detectar o
/// formato, por isso copia-se a entrada para o `/tmp` com a extensão recebida.
/// Corre `--headless`, sem rede, e com o perfil de utilizador dentro do `/tmp`
/// (tmpfs) — porque o rootfs é só-leitura e o LibreOffice quer escrever a sua
/// instalação de utilizador algalgures.
fn miniatura_de_office(extensao: Option<&str>) -> Result<(), String> {
    let ext = extensao.unwrap_or("bin");
    let origem = format!("/tmp/src.{ext}");
    std::fs::copy(ENTRADA, &origem)
        .map_err(|erro| format!("não foi possível preparar o documento: {erro}"))?;

    let saida = Command::new("soffice")
        .args([
            "--headless",
            "--nologo",
            "--nolockcheck",
            "--nodefault",
            "--norestore",
            "-env:UserInstallation=file:///tmp/lo",
            "--convert-to",
            "pdf",
            "--outdir",
            "/tmp",
            &origem,
        ])
        .output()
        .map_err(|erro| format!("não foi possível correr o soffice: {erro}"))?;

    if !saida.status.success() {
        return Err(format!(
            "o soffice terminou com {}: {}",
            saida.status,
            String::from_utf8_lossy(&saida.stderr).trim()
        ));
    }

    // `--convert-to pdf` nomeia a saída pelo nome de origem com `.pdf`.
    let pdf = "/tmp/src.pdf";
    if !Path::new(pdf).exists() {
        return Err("o soffice não produziu PDF".to_owned());
    }
    rasterizar_pdf(pdf)
}

/// Fotograma de um vídeo: extrai o primeiro, redimensiona, e escreve PNG.
///
/// `ffmpeg` sem rede, um só fotograma, sem áudio. O escalar leva o lado maior a
/// [`LADO`] preservando a proporção.
fn fotograma_de_video() -> Result<(), String> {
    let produzido = "/tmp/frame.png";

    let saida = Command::new("ffmpeg")
        .args([
            "-nostdin",
            "-y",
            "-i",
            ENTRADA,
            "-frames:v",
            "1",
            "-an",
            "-vf",
            &format!("scale='min({LADO},iw)':-2"),
            produzido,
        ])
        .output()
        .map_err(|erro| format!("não foi possível correr o ffmpeg: {erro}"))?;

    if !saida.status.success() {
        return Err(format!(
            "o ffmpeg terminou com {}: {}",
            saida.status,
            String::from_utf8_lossy(&saida.stderr).trim()
        ));
    }

    let bytes =
        std::fs::read(produzido).map_err(|_| "a extracção não produziu imagem".to_owned())?;
    std::fs::write(SAIDA, bytes)
        .map_err(|erro| format!("não foi possível escrever o derivado: {erro}"))?;
    Ok(())
}
