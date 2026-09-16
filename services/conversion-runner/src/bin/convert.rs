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

use std::path::Path;
use std::process::{Command, ExitCode};

const ENTRADA: &str = "/in/input";
const SAIDA: &str = "/out/output";

fn main() -> ExitCode {
    let perfil = match std::env::args().nth(1) {
        Some(nome) => nome,
        None => {
            eprintln!("ocinye-convert: falta o nome do perfil");
            return ExitCode::from(2);
        }
    };

    // Defesa em profundidade: o perfil já foi validado pelo runner, e valida-se
    // outra vez aqui. A fronteira não confia em quem a chama.
    if ocinye_conversion_runner::profile(&perfil).is_none() {
        eprintln!("ocinye-convert: perfil desconhecido: {perfil}");
        return ExitCode::from(2);
    }

    if !Path::new(ENTRADA).exists() {
        eprintln!("ocinye-convert: não há entrada em {ENTRADA}");
        return ExitCode::from(3);
    }

    let resultado = match perfil.as_str() {
        "pdf-thumbnail" => rasterizar_pdf(),
        outro => {
            // Inalcançável enquanto a lista fechada e este `match` concordarem;
            // fica explícito para o dia em que um perfil novo entrar sem ramo.
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

/// Rasteriza a primeira página do PDF numa imagem PNG.
///
/// `pdftoppm` é um **renderizador**, não um motor de scripts: não executa o
/// JavaScript embutido no documento (spec §76). Escreve para o `/tmp` (tmpfs) e
/// só depois move para `/out`, porque `-singlefile` nomeia a saída como
/// `{base}.png` e o que o runner lê é exactamente `/out/output`.
fn rasterizar_pdf() -> Result<(), String> {
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
            "960",
            ENTRADA,
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
