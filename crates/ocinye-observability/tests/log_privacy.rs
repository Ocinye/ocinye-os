//! Nenhum sítio de log escreve um campo sensível.
//!
//! # Porque isto existe
//!
//! `CLAUDE.md` §62 diz que palavras-passe, tokens, cookies, documentos
//! inteiros, conteúdo de datasets e prompts nunca aparecem num log. Era uma
//! regra escrita e mais nada: `SENSITIVE_FIELDS` estava declarada, `redact`
//! estava implementada e testada — e nenhum crate do sistema chamava qualquer
//! uma das duas.
//!
//! Uma protecção que ninguém invoca não é uma protecção. Ou o repositório
//! cumpria a regra por disciplina — e cumpria, os sítios de log são poucos e
//! todos benignos — ou não cumpria, e ninguém saberia. As duas hipóteses são
//! indistinguíveis sem alguém a medir.
//!
//! Este teste mede. Percorre todos os sítios de log de todos os crates e
//! recusa um campo cujo nome esteja em `SENSITIVE_FIELDS`, a não ser que o
//! valor passe por `redact` — que passa assim a ser a única saída sancionada,
//! em vez de uma função à espera de um dia ser útil.
//!
//! # O que isto não apanha
//!
//! Um segredo escrito dentro da mensagem em vez de num campo — `info!("token
//! {t}")` — ou um campo com nome inocente a carregar material sensível. Nenhuma
//! busca de texto apanha a segunda; a primeira é apanhada mais abaixo, por
//! interpolação directa na mensagem. O que este teste garante é que o caminho
//! **nomeado** está fechado.

use std::path::{Path, PathBuf};

use ocinye_observability::{is_sensitive_field, SENSITIVE_FIELDS};

/// A raiz do repositório, a partir deste crate.
fn raiz() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("raiz do repositório")
}

/// Todos os ficheiros de código de produção do sistema.
fn fontes() -> Vec<PathBuf> {
    let base = raiz();
    let mut ficheiros = Vec::new();
    for area in ["crates", "services", "apps"] {
        recolher(&base.join(area), &mut ficheiros);
    }
    assert!(
        ficheiros.len() > 100,
        "encontrei {} ficheiros: o caminho está errado e este teste não observaria nada",
        ficheiros.len()
    );
    ficheiros
}

fn recolher(pasta: &Path, destino: &mut Vec<PathBuf>) {
    let Ok(entradas) = std::fs::read_dir(pasta) else {
        return;
    };
    for entrada in entradas.flatten() {
        let caminho = entrada.path();
        if caminho.is_dir() {
            // `tests/` fica de fora: um teste pode legitimamente imprimir o que
            // está a exercitar, e não é isso que sai para um agregador de logs.
            if caminho
                .file_name()
                .is_some_and(|n| n == "tests" || n == "target")
            {
                continue;
            }
            recolher(&caminho, destino);
        } else if caminho.extension().is_some_and(|e| e == "rs") {
            destino.push(caminho);
        }
    }
}

/// As invocações de macro de log de um ficheiro, com o texto dos argumentos.
fn sitios_de_log(fonte: &str) -> Vec<(usize, String)> {
    const MACROS: [&str; 5] = ["trace!", "debug!", "info!", "warn!", "error!"];
    let mut sitios = Vec::new();

    for (numero, linha) in fonte.lines().enumerate() {
        let sem_comentario = linha.split("//").next().unwrap_or(linha);
        for macro_ in MACROS {
            let mut procura = sem_comentario;
            while let Some(posicao) = procura.find(macro_) {
                let resto = &procura[posicao + macro_.len()..];
                procura = resto;
                if !resto.starts_with('(') {
                    continue;
                }
                // Até ao parêntesis que fecha, contando aninhamento.
                let mut profundidade = 0i32;
                let mut fim = None;
                for (i, c) in resto.char_indices() {
                    match c {
                        '(' => profundidade += 1,
                        ')' => {
                            profundidade -= 1;
                            if profundidade == 0 {
                                fim = Some(i);
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                let argumentos = match fim {
                    Some(i) => resto[1..i].to_owned(),
                    // A invocação continua na linha seguinte; leva-se o que há.
                    None => resto[1..].to_owned(),
                };
                sitios.push((numero + 1, argumentos));
            }
        }
    }
    sitios
}

/// Os nomes de campo estruturado de uma invocação.
fn campos(argumentos: &str) -> Vec<String> {
    let mut nomes = Vec::new();
    for pedaco in argumentos.split(',') {
        let Some((esquerda, _)) = pedaco.split_once('=') else {
            continue;
        };
        let nome = esquerda.trim().trim_start_matches(['%', '?']);
        if nome.is_empty()
            || !nome
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
        {
            continue;
        }
        // O nome do campo é o último segmento: `self.password` é `password`.
        let nome = nome.rsplit('.').next().unwrap_or(nome);
        nomes.push(nome.to_owned());
    }
    nomes
}

/// Nenhum campo de log tem um nome que sabemos carregar material sensível.
///
/// A saída sancionada é `redact`, e é a única: um campo sensível cujo valor
/// passe por lá é aceite, porque o que sai é a marca e não o valor.
#[test]
fn nenhum_sitio_de_log_escreve_um_campo_sensivel() {
    let base = raiz();
    let mut infractores = Vec::new();

    for caminho in fontes() {
        let fonte = std::fs::read_to_string(&caminho).expect("ficheiro");
        for (linha, argumentos) in sitios_de_log(&fonte) {
            if argumentos.contains("redact(") {
                continue;
            }
            for nome in campos(&argumentos) {
                if is_sensitive_field(&nome) {
                    let relativo = caminho.strip_prefix(&base).unwrap_or(&caminho);
                    infractores.push(format!("{}:{linha} escreve `{nome}`", relativo.display()));
                }
            }
        }
    }

    assert!(
        infractores.is_empty(),
        "um sítio de log escreve um campo que sabemos carregar material sensível.\n\
         Se o valor tiver mesmo de ser registado, passe-o por \
         `ocinye_observability::redact`, que devolve a marca em vez do valor:\n{infractores:#?}"
    );
}

/// Nenhuma mensagem de log interpola uma variável com nome sensível.
///
/// O caminho nomeado fecha-se acima. Este fecha o outro: `warn!("token {token}
/// recusado")` não tem campo nenhum, e mandava o valor para o log na mesma.
#[test]
fn nenhuma_mensagem_de_log_interpola_um_valor_sensivel() {
    let base = raiz();
    let mut infractores = Vec::new();

    for caminho in fontes() {
        let fonte = std::fs::read_to_string(&caminho).expect("ficheiro");
        for (linha, argumentos) in sitios_de_log(&fonte) {
            let mut resto = argumentos.as_str();
            while let Some(inicio) = resto.find('{') {
                let depois = &resto[inicio + 1..];
                let Some(fim) = depois.find('}') else { break };
                let dentro = &depois[..fim];
                resto = &depois[fim + 1..];
                let nome = dentro.split(':').next().unwrap_or(dentro).trim();
                let nome = nome.rsplit('.').next().unwrap_or(nome);
                if is_sensitive_field(nome) {
                    let relativo = caminho.strip_prefix(&base).unwrap_or(&caminho);
                    infractores.push(format!("{}:{linha} interpola `{nome}`", relativo.display()));
                }
            }
        }
    }

    assert!(
        infractores.is_empty(),
        "uma mensagem de log interpola uma variável com nome sensível: {infractores:#?}"
    );
}

/// O inventário de nomes sensíveis não encolhe sem que alguém repare.
///
/// Controlo positivo do par acima: se a lista ficasse vazia, os dois testes
/// passariam sem terem observado nada.
#[test]
fn o_inventario_de_nomes_sensiveis_esta_povoado() {
    assert!(
        SENSITIVE_FIELDS.len() >= 15,
        "a lista de nomes sensíveis encolheu para {}: os guardas que dependem \
         dela passariam a não recusar quase nada",
        SENSITIVE_FIELDS.len()
    );
    for obrigatorio in ["password", "token", "cookie", "authorization", "prompt"] {
        assert!(
            is_sensitive_field(obrigatorio),
            "`{obrigatorio}` saiu da lista de nomes sensíveis"
        );
    }
}
