//! O worker leva ao outbox o fornecedor de embeddings que a configuração diz.
//!
//! # Porque isto é um teste de processo e não de unidade
//!
//! Porque a função que escolhe o fornecedor — `embeddings::from_config` — sempre
//! esteve certa. Nunca era chamada. O ciclo principal passava `None` literal ao
//! `drain`, e por isso **nenhuma** instalação produzia embeddings: configurada
//! ou não, a recuperação semântica ficava vazia e nada o dizia.
//!
//! Um teste de unidade sobre `from_config` passaria com o defeito no sítio. O
//! que estava errado era **onde** ela não era chamada — e isso só se vê
//! levantando o binário.
//!
//! O que se mede é a declaração de arranque: o worker diz qual o fornecedor que
//! tem, ou diz que não tem nenhum. Os dois estados são honestos; o que não era
//! honesto era não haver estado nenhum.

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const LIMITE: Duration = Duration::from_secs(30);

/// Levanta o worker com a configuração dada e devolve o que ele declarou.
///
/// Por ficheiro e com prazo, e não a ler o *pipe* linha a linha: `lines()`
/// bloqueia à espera da linha seguinte, e se a declaração nunca aparecer o
/// teste fica pendurado em vez de falhar. A primeira versão deste teste lia
/// `stderr` — e o `tracing_subscriber` escreve em **stdout**, pelo que esperou
/// para sempre por uma linha que estava no sítio ao lado.
fn declaracao_de_arranque(provider: Option<&str>) -> Option<String> {
    let Ok(base) = std::env::var("OCINYE_TEST_DATABASE_URL") else {
        eprintln!("skipping: OCINYE_TEST_DATABASE_URL is not set");
        assert!(
            std::env::var("CI").is_err(),
            "não há base de dados, e isto é a CI: a configuração do worker não \
             pode ficar por verificar aqui"
        );
        return None;
    };

    let registo = std::env::temp_dir().join(format!(
        "ocinye-worker-{}-{}.log",
        std::process::id(),
        provider.unwrap_or("nenhum")
    ));
    let saida = std::fs::File::create(&registo).expect("ficheiro de registo");
    let erros = saida.try_clone().expect("duplicar");

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ocinye-worker"));
    cmd.env("OCINYE_DATABASE_URL", base)
        .stdout(Stdio::from(saida))
        .stderr(Stdio::from(erros));

    match provider {
        Some(nome) => cmd.env("OCINYE_AI_EMBEDDING_PROVIDER", nome),
        // Explicitamente removida, e não apenas ausente: o ambiente de quem
        // corre os testes pode tê-la definida, e então este caso mediria o
        // outro.
        None => cmd.env_remove("OCINYE_AI_EMBEDDING_PROVIDER"),
    };

    let mut worker = cmd.spawn().expect("levantar o worker");

    let inicio = Instant::now();
    let mut declaracao = String::new();
    while inicio.elapsed() < LIMITE {
        let texto = std::fs::read_to_string(&registo).unwrap_or_default();
        if let Some(linha) = texto.lines().find(|l| {
            l.contains("embedding provider configured") || l.contains("no embedding provider")
        }) {
            declaracao = linha.to_owned();
            break;
        }
        if worker.try_wait().expect("estado").is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }

    let _ = Command::new("kill")
        .args(["-TERM", &worker.id().to_string()])
        .status();
    let _ = worker.wait();
    let _ = std::fs::remove_file(&registo);

    assert!(
        !declaracao.is_empty(),
        "o worker não declarou nada sobre embeddings em {LIMITE:?}: o arranque \
         voltou a ficar em silêncio sobre uma capacidade que tem ou não tem"
    );
    Some(declaracao)
}

/// Configurado, o worker diz qual o modelo — e o vector nunca entra no registo.
#[test]
fn um_fornecedor_configurado_chega_ao_worker() {
    let Some(declaracao) = declaracao_de_arranque(Some("deterministic")) else {
        return;
    };

    assert!(
        declaracao.contains("embedding provider configured"),
        "o worker não declarou o fornecedor que a configuração lhe deu: {declaracao}"
    );
    // A identidade do modelo entra no registo, para que a instalação seja
    // auditável. O `not-a-model` do fornecedor determinista é deliberado: diz
    // que não é um modelo a sério em vez de fingir que é.
    assert!(
        declaracao.contains("not-a-model"),
        "a identidade do modelo não chegou ao registo de arranque: {declaracao}"
    );
}

/// Sem fornecedor, o worker diz que não tem — e continua a correr.
///
/// `NOT_CONFIGURED` é um estado honesto. O que não era honesto era o silêncio.
#[test]
fn sem_fornecedor_o_worker_diz_que_nao_tem() {
    let Some(declaracao) = declaracao_de_arranque(None) else {
        return;
    };

    assert!(
        declaracao.contains("no embedding provider configured"),
        "o worker não declarou a ausência de fornecedor: {declaracao}"
    );
    assert!(
        declaracao.contains("lexical retrieval continues to work"),
        "a declaração não diz que a pesquisa lexical não cai com isto: {declaracao}"
    );
}
