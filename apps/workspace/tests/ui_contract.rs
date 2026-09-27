//! O contrato técnico da UI, que sobrevive a qualquer desenho.
//!
//! Estes testes viviam em `design_fidelity.rs`, ao lado das medições da UI
//! legada. As medições saíram com ela (UI Reset); estes ficam, porque guardam
//! fronteiras que a UI nova do Claude Design também tem de respeitar: a CSP
//! sem estilos inline, o arranque do `app.js` sem chamadas partidas, o catálogo
//! de avatares completo, e nenhum ecrã a pedir um nome de utilizador.
//! Ver `docs/ui/CLAUDE_DESIGN_CONTRACT.md`.

/// Nenhum ecrã pode depender de um atributo `style`.
///
/// A Content-Security-Policy do Workspace declara `style-src 'self'` sem
/// `'unsafe-inline'` — decisão registada no threat model e no baseline de
/// segurança, onde um botão de correio chegou a ser retirado por causa dela em
/// vez de a política ser alargada.
///
/// O browser descarta esses atributos antes de pintar. Um `style` inline chega
/// portanto **correcto no HTML** e sem efeito nenhum no ecrã, que é a razão de
/// os testes de marcação nunca terem apanhado isto: as tabelas tinham as suas
/// colunas no atributo, `display: grid` caía para uma coluna só, e o cabeçalho
/// empilhava-se por cima das linhas em todas as listas da aplicação.
///
/// A regra é procurada no código-fonte e não no HTML renderizado: assim cobre
/// também os ecrãs que nenhum teste chega a renderizar.
#[test]
fn nenhum_ecra_depende_de_um_atributo_style_que_a_csp_descarta() {
    let mut culpados = Vec::new();
    let raiz = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");

    fn varrer(dir: &std::path::Path, culpados: &mut Vec<String>) {
        for entrada in std::fs::read_dir(dir).expect("ler directório") {
            let caminho = entrada.expect("entrada").path();
            if caminho.is_dir() {
                varrer(&caminho, culpados);
            } else if caminho.extension().is_some_and(|e| e == "rs") {
                let fonte = std::fs::read_to_string(&caminho).expect("ler ficheiro");
                for (n, linha) in fonte.lines().enumerate() {
                    let corte = linha.trim_start();
                    if corte.starts_with("//") {
                        continue;
                    }
                    if corte.contains("style=") {
                        culpados.push(format!("{}:{}", caminho.display(), n + 1));
                    }
                }
            }
        }
    }

    varrer(&raiz, &mut culpados);

    assert!(
        culpados.is_empty(),
        "a CSP descarta `style` inline; estes seriam ignorados pelo browser:\n  {}",
        culpados.join("\n  ")
    );
}

/// A política que torna o teste acima necessário continua a existir.
///
/// Se `'unsafe-inline'` alguma vez voltar, isto tem de ser uma decisão visível
/// e não um efeito colateral de alguém a tentar fazer uma tabela funcionar.
#[test]
fn a_csp_do_workspace_nao_admite_estilos_inline() {
    let rotas = include_str!("../src/routes.rs");
    assert!(rotas.contains("style-src 'self'"));
    assert!(
        !rotas.contains("unsafe-inline"),
        "a CSP foi alargada a estilos inline"
    );
}

/// Cada avatar do catálogo tem um ficheiro que o Workspace serve.
///
/// # A classe de defeito
///
/// O catálogo vive em `ocinye-contracts` e o ficheiro em `static/avatars/`.
/// São dois sítios, e nada os obrigava a concordar: acrescentar um
/// identificador sem entregar a imagem dá uma grelha com um buraco, e entregar
/// uma imagem sem a registar dá um ficheiro que ninguém alcança.
///
/// Nenhuma das duas falha a compilar, e nenhuma falha em tempo de execução —
/// o `<img>` simplesmente não pinta, e o componente cai nas iniciais, que é o
/// comportamento certo para uma imagem partida e o comportamento errado para
/// uma imagem que devia existir.
#[test]
fn cada_avatar_do_catalogo_tem_ficheiro() {
    let pasta = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("static/avatars");

    let mut declarados = std::collections::BTreeSet::new();
    for (id, file) in ocinye_contracts::AVATAR_PRESETS {
        let caminho = pasta.join(file);
        assert!(
            caminho.is_file(),
            "o avatar «{id}» está no catálogo e o ficheiro {file} não existe"
        );
        declarados.insert((*file).to_owned());
    }

    // E o inverso: um ficheiro na pasta que ninguém registou nunca aparece.
    for entrada in std::fs::read_dir(&pasta).expect("a pasta dos avatares") {
        let nome = entrada.expect("entrada").file_name();
        let nome = nome.to_string_lossy().to_string();
        assert!(
            declarados.contains(&nome),
            "o ficheiro {nome} está na pasta e não no catálogo: ninguém o alcança"
        );
    }
}

/// Nenhuma função chamada no arranque deixa de existir.
///
/// # O defeito que isto guarda
///
/// `start()` chama uma função por ecrã. Uma chamada a uma função que não foi
/// escrita é um `ReferenceError` — e o que ele parte não é só essa: parte
/// **tudo o que vem depois** dela na mesma sequência.
///
/// Aconteceu: uma edição inseriu `initSino()` na lista e falhou a escrever a
/// função, porque a âncora de texto não coincidiu. O JavaScript continuou
/// sintacticamente válido, a página carregou, e a barra lateral, a paleta e o
/// centro temporal deixaram de responder — sem uma linha vermelha em lado
/// nenhum.
#[test]
fn tudo_o_que_o_arranque_chama_existe() {
    let js = include_str!("../static/app.js");

    let inicio = js
        .find("const start = () => {")
        .expect("o arranque tem de existir");
    let fim = inicio
        + js[inicio..]
            .find("\n  };")
            .expect("o arranque tem de fechar");
    let corpo = &js[inicio..fim];

    let chamadas: Vec<&str> = corpo
        .lines()
        .filter_map(|linha| linha.trim().strip_suffix("();"))
        .filter(|nome| nome.starts_with("init"))
        .collect();

    assert!(
        chamadas.len() > 5,
        "o arranque devia chamar vários inicializadores, e encontrou {}",
        chamadas.len()
    );

    let em_falta: Vec<&str> = chamadas
        .iter()
        .filter(|nome| !js.contains(&format!("function {nome}(")))
        .copied()
        .collect();

    assert!(
        em_falta.is_empty(),
        "o arranque chama funções que não existem: {em_falta:?}\n\nUm `ReferenceError` \
         aqui não parte só esta — parte tudo o que vem depois dela."
    );
}

/// Nenhum ecrã corrente pede um nome de utilizador.
///
/// # Porque isto não é um grep sobre o repositório
///
/// Porque `username` continua a ser legítimo em dois sítios, e um portão que
/// os apanhasse seria desligado na primeira vez que incomodasse:
///
/// - `autocomplete="username"` é o nome que os gestores de palavras-passe do
///   browser esperam no campo da conta. É convenção do HTML, não conceito do
///   Ocinye, e o ecrã de entrada usa-o **no campo do endereço**.
/// - `preferred_username` é um *claim* do OIDC, guardado para uma federação
///   futura ([ADR-0103](../../docs/adrs/0103-core-owned-authentication.md)).
///
/// Por isso o que se mede é a **superfície**: o texto que uma pessoa lê e os
/// campos que ela preenche. É aí que o conceito reaparece, e foi aí que ele
/// esteve escondido — um campo com a etiqueta «Nome de utilizador», o `pattern`
/// do username e o `name="email"`, ao lado do campo do endereço verdadeiro.
///
/// # O que esse campo fazia
///
/// O `pattern` não admitia `@`. Nenhum endereço válido passava a validação do
/// browser, e **ninguém conseguia criar um membro** por aquele ecrã. Não deu
/// erro em lado nenhum: o botão simplesmente não submetia.
#[test]
fn nenhum_ecra_pede_um_nome_de_utilizador() {
    let mut problemas = Vec::new();

    for caminho in std::fs::read_dir("src/ui/screens").expect("ecrãs") {
        let caminho = caminho.expect("entrada").path();
        if caminho.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let fonte = std::fs::read_to_string(&caminho).expect("ler");
        let ecra = caminho
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("?")
            .to_owned();

        for (numero, linha) in fonte.lines().enumerate() {
            let limpa = linha.trim();
            // Comentários e documentação falam do passado, e devem poder
            // continuar a falar dele.
            if limpa.starts_with("//") {
                continue;
            }
            let baixa = limpa.to_lowercase();

            if baixa.contains("nome de utilizador") {
                problemas.push(format!(
                    "{ecra}:{} pede «nome de utilizador» a quem lê",
                    numero + 1
                ));
            }
            // Um campo chamado `username` num formulário. O atributo
            // `autocomplete` tem o mesmo texto e é outra coisa.
            if baixa.contains("name=\"username\"") {
                problemas.push(format!(
                    "{ecra}:{} tem um campo `username` no formulário",
                    numero + 1
                ));
            }
        }
    }

    assert!(
        problemas.is_empty(),
        "o nome de utilizador voltou à superfície:\n  {}",
        problemas.join("\n  ")
    );
}
