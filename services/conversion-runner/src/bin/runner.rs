//! `ocinye-conversion-runner` — a fronteira de isolamento.
//!
//! É o único componente com autoridade para criar contentores (é o único onde o
//! socket do Docker é montado). O worker **não** tem o socket: pede a este
//! serviço, por HTTP na rede interna, um perfil de uma lista fechada, e recebe o
//! derivado. Assim uma vulnerabilidade num parser de conteúdo hostil encontra um
//! contentor descartável quase vazio, e não o worker da Ocinye — e comprometer o
//! worker não dá root sobre o host, porque o worker nunca teve o socket.
//!
//! O que este serviço faz é deliberadamente pequeno: recebe bytes, escreve-os
//! num directório efémero, corre `docker run` **endurecido** (sem rede, rootfs
//! só-leitura, sem capacidades, sem novos privilégios, com tectos de CPU,
//! memória, processos e tempo), lê o derivado, e apaga tudo. Não faz parsing de
//! conteúdo nenhum — isso é dentro do contentor descartável.
//!
//! # Porque o socket aqui, e não no worker
//!
//! Criar contentores dinamicamente precisa do socket do Docker, e montar o
//! socket é dar autoridade equivalente a root sobre o host. A decisão é conter
//! essa autoridade **nesta** caixa mínima e auditada — que não abre bytes
//! hostis e só sabe correr uma lista fechada de perfis — em vez de a espalhar
//! pelo worker, que processa a fila inteira da instituição.

use std::path::PathBuf;
use std::time::Duration;

use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use ocinye_conversion_runner::Profile;
use ocinye_observability::LogFormat;
use tokio::process::Command;

/// A configuração, lida do ambiente. Falha no arranque se faltar o essencial —
/// é preferível a assumir um valor por omissão para a imagem do conversor.
#[derive(Clone)]
struct Config {
    /// Onde escutar, na rede interna. Sem porta pública.
    addr: String,
    /// A imagem do conversor descartável a correr.
    converter_image: String,
    /// O directório de spool. Tem de estar montado no **mesmo caminho** no host
    /// e neste contentor, para que o caminho que passamos ao `docker run` irmão
    /// se resolva no host.
    spool: PathBuf,
    /// O tecto de bytes de entrada.
    max_bytes: usize,
}

impl Config {
    fn from_env() -> anyhow::Result<Self> {
        let converter_image = std::env::var("OCINYE_CONVERTER_IMAGE").map_err(|_| {
            anyhow::anyhow!("OCINYE_CONVERTER_IMAGE em falta: a imagem do conversor descartável")
        })?;
        Ok(Self {
            addr: std::env::var("OCINYE_CONVERSION_RUNNER_ADDR")
                .unwrap_or_else(|_| "0.0.0.0:8730".to_owned()),
            converter_image,
            spool: std::env::var("OCINYE_CONVERSION_SPOOL")
                .unwrap_or_else(|_| "/srv/ocinye/conversion-spool".to_owned())
                .into(),
            max_bytes: std::env::var("OCINYE_CONVERSION_MAX_BYTES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(64 * 1024 * 1024),
        })
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let log_level = std::env::var("OCINYE_LOG_LEVEL").unwrap_or_else(|_| "info".to_owned());
    let log_format = std::env::var("OCINYE_LOG_FORMAT").unwrap_or_else(|_| "json".to_owned());
    ocinye_observability::init(
        "ocinye-conversion-runner",
        &log_level,
        LogFormat::parse(&log_format),
    );

    let config = Config::from_env()?;
    tokio::fs::create_dir_all(&config.spool).await.ok();

    let addr = config.addr.clone();
    let app = Router::new()
        .route("/health", get(|| async { StatusCode::OK }))
        .route("/convert/{profile}", post(convert))
        .layer(DefaultBodyLimit::max(config.max_bytes))
        .with_state(config);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!(%addr, "Ocinye Conversion Runner started");
    axum::serve(listener, app).await?;
    Ok(())
}

/// `POST /convert/{profile}` — o corpo são os bytes de entrada; a resposta é o
/// derivado.
///
/// O perfil é validado contra a lista fechada antes de tudo. Os bytes nunca vão
/// para a linha de comando: viajam por um ficheiro montado só-leitura. O
/// directório efémero é sempre apagado, mesmo quando a conversão falha.
async fn convert(
    State(config): State<Config>,
    Path(profile): Path<String>,
    Query(pedido): Query<ConvertQuery>,
    corpo: Bytes,
) -> Response {
    let Some(perfil) = ocinye_conversion_runner::profile(&profile) else {
        return (StatusCode::NOT_FOUND, "perfil desconhecido").into_response();
    };

    // A extensão é uma pista para o conversor (o LibreOffice precisa dela), e
    // higieniza-se à porta: só letras e dígitos, curta. Um valor fora disto
    // descarta-se em vez de viajar para a linha de comando do contentor.
    let extensao = pedido
        .ext
        .filter(|e| !e.is_empty() && e.len() <= 8 && e.chars().all(|c| c.is_ascii_alphanumeric()));

    if corpo.is_empty() {
        return (StatusCode::BAD_REQUEST, "entrada vazia").into_response();
    }

    let job = uuid::Uuid::new_v4().simple().to_string();
    let dir = config.spool.join(&job);
    let entrada = dir.join("in");
    let saida = dir.join("out");

    let preparado = preparar(&entrada, &saida, &corpo).await;
    if let Err(erro) = preparado {
        tracing::error!(error = %erro, job, "não foi possível preparar o job");
        let _ = tokio::fs::remove_dir_all(&dir).await;
        return (StatusCode::INTERNAL_SERVER_ERROR, "falha a preparar").into_response();
    }

    let resultado = correr(&config, perfil, &job, &entrada, &saida, extensao.as_deref()).await;
    let _ = tokio::fs::remove_dir_all(&dir).await;

    match resultado {
        Ok(bytes) => (
            [(axum::http::header::CONTENT_TYPE, "application/octet-stream")],
            bytes,
        )
            .into_response(),
        Err(Falha::Conversao(razao)) => {
            tracing::warn!(job, profile = perfil.name, razao, "conversão falhou");
            (StatusCode::UNPROCESSABLE_ENTITY, razao).into_response()
        }
        Err(Falha::Interna(razao)) => {
            tracing::error!(job, profile = perfil.name, razao, "runner falhou");
            (StatusCode::INTERNAL_SERVER_ERROR, "falha interna").into_response()
        }
    }
}

/// Prepara o directório efémero: `in/input` legível (0644) para o conteúdo, e
/// `out` gravável (0777) para o contentor não-privilegiado poder escrever o
/// derivado.
async fn preparar(
    entrada: &std::path::Path,
    saida: &std::path::Path,
    corpo: &[u8],
) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    tokio::fs::create_dir_all(entrada).await?;
    tokio::fs::create_dir_all(saida).await?;

    let ficheiro = entrada.join("input");
    tokio::fs::write(&ficheiro, corpo).await?;
    tokio::fs::set_permissions(&ficheiro, std::fs::Permissions::from_mode(0o644)).await?;
    // O contentor corre como 65534 (nobody) e escreve aqui; o directório
    // efémero é destruído a seguir, sem segredos e sem partilha.
    tokio::fs::set_permissions(saida, std::fs::Permissions::from_mode(0o777)).await?;
    Ok(())
}

/// A extensão do ficheiro de origem, pista para o conversor. Opcional.
#[derive(serde::Deserialize)]
struct ConvertQuery {
    #[serde(default)]
    ext: Option<String>,
}

/// Uma falha de conversão (conteúdo que não converte) é diferente de uma falha
/// do runner (o Docker não atendeu). A primeira é um estado do ficheiro; a
/// segunda, um erro que o worker deve voltar a tentar.
enum Falha {
    Conversao(String),
    Interna(String),
}

/// Os argumentos do `docker run` que endurecem o contentor de conversão.
///
/// Extraída de [`correr`] para ser testável: o endurecimento é a fronteira de
/// segurança (ADR-0609), e uma flag que caia sem que nada o diga é o contentor a
/// deixar de ser uma caixa fechada. Todos os argumentos são fixos ou vêm de
/// valores validados (o nome do perfil, da lista fechada; um UUID; uma extensão
/// higienizada). Os bytes hostis **não** estão aqui — estão no ficheiro montado
/// só-leitura.
fn docker_run_args(
    perfil: &Profile,
    nome: &str,
    entrada: &str,
    saida: &str,
    imagem: &str,
    extensao: Option<&str>,
) -> Vec<String> {
    let mut args: Vec<String> = [
        "run",
        "--rm",
        "--name",
        nome,
        "--network=none",
        "--read-only",
        "--cap-drop=ALL",
        "--security-opt=no-new-privileges",
        "--pids-limit",
        &perfil.pids.to_string(),
        "--memory",
        perfil.memory,
        "--cpus",
        perfil.cpus,
        "--user",
        "65534:65534",
        "--tmpfs",
        "/tmp:rw,nosuid,nodev,size=256m",
        "-v",
        &format!("{entrada}:/in:ro"),
        "-v",
        &format!("{saida}:/out"),
        imagem,
        perfil.name,
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    // A extensão, quando há, é o argumento seguinte do `ocinye-convert`.
    if let Some(ext) = extensao {
        args.push(ext.to_owned());
    }
    args
}

/// Corre a conversão num contentor descartável e endurecido, e devolve os bytes
/// do derivado.
async fn correr(
    config: &Config,
    perfil: &Profile,
    job: &str,
    entrada: &std::path::Path,
    saida: &std::path::Path,
    extensao: Option<&str>,
) -> Result<Vec<u8>, Falha> {
    let nome = format!("oc-conv-{job}");
    let entrada_s = entrada.to_string_lossy().into_owned();
    let saida_s = saida.to_string_lossy().into_owned();

    let mut comando = Command::new("docker");
    comando.args(docker_run_args(
        perfil,
        &nome,
        &entrada_s,
        &saida_s,
        &config.converter_image,
        extensao,
    ));
    comando.kill_on_drop(true);

    let prazo = Duration::from_secs(perfil.timeout_secs);
    let execucao = tokio::time::timeout(prazo, comando.output()).await;

    let saida_processo = match execucao {
        Ok(Ok(s)) => s,
        Ok(Err(erro)) => {
            return Err(Falha::Interna(format!("docker não correu: {erro}")));
        }
        Err(_) => {
            // O prazo esgotou. Matar o `docker run` local não garante que o
            // contentor morre — força-se a remoção pelo nome.
            let _ = Command::new("docker")
                .args(["rm", "-f", &nome])
                .output()
                .await;
            return Err(Falha::Conversao(format!(
                "a conversão excedeu {}s",
                perfil.timeout_secs
            )));
        }
    };

    if !saida_processo.status.success() {
        let stderr = String::from_utf8_lossy(&saida_processo.stderr);
        return Err(Falha::Conversao(format!(
            "o conversor terminou com {}: {}",
            saida_processo.status,
            stderr.trim()
        )));
    }

    let ficheiro = saida.join("output");
    match tokio::fs::read(&ficheiro).await {
        Ok(bytes) if !bytes.is_empty() => Ok(bytes),
        _ => Err(Falha::Conversao(
            "a conversão não produziu derivado".to_owned(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// O contentor de conversão corre endurecido, e o argv nunca é um comando
    /// arbitrário — só a imagem e o perfil da lista fechada.
    ///
    /// A fronteira de segurança do ADR-0609 vive nestas flags. Uma que caísse
    /// sem que nada o dissesse abriria a caixa; este teste recusa a queda.
    #[test]
    fn o_contentor_de_conversao_corre_endurecido() {
        let perfil = ocinye_conversion_runner::profile("pdf-thumbnail").expect("perfil");
        let args = docker_run_args(
            perfil,
            "oc-conv-x",
            "/spool/j/in",
            "/spool/j/out",
            "img:sha",
            Some("pdf"),
        );

        for flag in [
            "--network=none",
            "--read-only",
            "--cap-drop=ALL",
            "--security-opt=no-new-privileges",
            "--pids-limit",
            "--memory",
            "--cpus",
        ] {
            assert!(
                args.iter().any(|a| a == flag),
                "falta a flag de endurecimento {flag}"
            );
        }
        assert!(
            args.windows(2)
                .any(|w| w[0] == "--user" && w[1] == "65534:65534"),
            "o conversor não corre como não-root"
        );
        assert!(
            args.iter().any(|a| a.ends_with(":/in:ro")),
            "a entrada não é montada só-leitura"
        );
        // Termina na imagem + perfil (+ extensão), nunca num comando arbitrário.
        assert!(
            args.contains(&"img:sha".to_owned()),
            "falta a imagem do conversor"
        );
        assert!(
            args.contains(&"pdf-thumbnail".to_owned()),
            "falta o nome do perfil"
        );
        assert!(args.contains(&"pdf".to_owned()), "falta a extensão passada");
    }
}
