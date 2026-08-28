//! O worker pára quando lhe pedem.
//!
//! # Porque isto é um teste de processo e não de unidade
//!
//! Porque a propriedade não vive numa função: vive na forma como o ciclo
//! principal arma o sinal. Um teste de unidade sobre `shutdown_signal()`
//! provaria que a função devolve quando há sinal — que era verdade mesmo com o
//! defeito. O que estava errado era **onde** ela era chamada.
//!
//! Este teste levanta o binário a sério, manda-lhe `SIGTERM`, e mede quanto
//! tempo demora a sair.

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Quanto tempo se dá a um worker para parar antes de o considerar surdo.
///
/// Generoso de propósito: a paragem envolve fechar um pool de ligações. O que se
/// distingue aqui é «pára» de «não pára», e não trezentos milissegundos de
/// quatrocentos.
const LIMITE: Duration = Duration::from_secs(10);

#[test]
fn o_worker_para_quando_lhe_mandam_sigterm() {
    // A mesma variável que todas as outras suites, e não a de produção.
    //
    // Este teste pedia `OCINYE_DATABASE_URL` — que é o nome que o **worker** lê,
    // e por isso parecia natural. Mas nenhuma outra suite a define: a CI define
    // `OCINYE_TEST_DATABASE_URL`, e o teste recusou-se a correr lá, como devia.
    // Localmente aconteceu o mesmo, e a primeira execução reportou `ok` sem ter
    // corrido nada.
    //
    // Uma suite que precisa de uma variável só sua é uma armadilha para quem a
    // corre. O nome de produção continua a ser o que se passa ao processo filho,
    // abaixo, que é onde ele pertence.
    let Ok(base) = std::env::var("OCINYE_TEST_DATABASE_URL") else {
        eprintln!("skipping: OCINYE_TEST_DATABASE_URL is not set");
        // Na CI isto é defeito: lá a base existe, e um teste de processo que se
        // salta a si mesmo é verde a dizer nada.
        assert!(
            std::env::var("CI").is_err(),
            "não há base de dados, e isto é a CI: o worker não pode ser saltado aqui"
        );
        return;
    };

    let mut worker = Command::new(env!("CARGO_BIN_EXE_ocinye-worker"))
        .env("OCINYE_DATABASE_URL", base)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("levantar o worker");

    // Tempo para ele chegar ao ciclo principal. Antes disso, um sinal apanha-o
    // a ligar-se à base e o teste mediria outra coisa.
    std::thread::sleep(Duration::from_secs(5));
    assert!(
        worker.try_wait().expect("estado").is_none(),
        "o worker morreu sozinho antes de alguém lhe pedir para parar"
    );

    // O sinal que um sistema de init manda. Não `SIGKILL`, que ninguém pode
    // ignorar e que por isso não provaria nada.
    //
    // Pelo `kill` do sistema e não por uma biblioteca: acrescentar uma
    // dependência à workspace por causa de uma chamada num teste é o que o
    // §54 manda evitar.
    let enviado = Command::new("kill")
        .args(["-TERM", &worker.id().to_string()])
        .status()
        .expect("mandar o sinal");
    assert!(
        enviado.success(),
        "não foi possível enviar SIGTERM ao worker"
    );

    let inicio = Instant::now();
    loop {
        if let Some(estado) = worker.try_wait().expect("estado") {
            assert!(estado.success(), "o worker parou, mas com erro: {estado:?}");
            return;
        }
        if inicio.elapsed() > LIMITE {
            let _ = worker.kill();
            let _ = worker.wait();
            panic!(
                "o worker ignorou SIGTERM durante {}s.\n\
                 O sinal está a ser armado dentro do ciclo? Um `select!` num ciclo \
                 reconstrói os seus futuros a cada iteração, e um sinal que chegue \
                 entre passagens não encontra ninguém à escuta.",
                LIMITE.as_secs()
            );
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}
