//! A fronteira de capacidades de runtime (ADR-0611).
//!
//! Duas propriedades, sem browser nem base de dados:
//!
//! 1. **Uma só origem da detecção.** Só `static/runtime.js` pergunta ao
//!    ambiente em que corre. Um `if (window.__TAURI__)` ou uma leitura do agente
//!    de utilizador noutro ficheiro é o começo de dois produtos — e um global que
//!    uma página externa podia imitar.
//! 2. **O cliente diz o mesmo que o contrato.** A tabela da Web em `runtime.js`
//!    é igual a `RuntimeCapability::target(RuntimeMode::Web)`, com os mesmos
//!    ids e pela mesma ordem.

use std::path::Path;

use ocinye_contracts::runtime::{RuntimeCapability, RuntimeMode, CAPABILITY_VERSION};

/// O que só `runtime.js` pode escrever.
const DETECCAO: &[&str] = &[
    "__TAURI__",
    "__TAURI_INTERNALS__",
    "navigator.userAgent",
    "userAgentData",
    "navigator.platform",
    "display-mode",
    "window.ipc",
];

/// Código de terceiros vendorizado, que não é do Workspace e não se edita: o
/// editor de notas (ProseMirror) usa o agente de utilizador para contornar
/// peculiaridades dos motores, e isso é dele.
const VENDORIZADO: &[&str] = &["notes-editor.js"];

fn estaticos() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("static")
}

#[test]
fn so_o_runtime_js_pergunta_ao_ambiente() {
    let mut examinados = 0;
    let mut violacoes = Vec::new();
    for entrada in std::fs::read_dir(estaticos()).expect("static/") {
        let caminho = entrada.expect("entrada").path();
        let nome = caminho.file_name().unwrap().to_string_lossy().into_owned();
        if !nome.ends_with(".js") || nome == "runtime.js" || VENDORIZADO.contains(&nome.as_str()) {
            continue;
        }
        examinados += 1;
        let texto = std::fs::read_to_string(&caminho).expect("ler");
        for proibido in DETECCAO {
            if texto.contains(proibido) {
                violacoes.push(format!("{nome}: «{proibido}»"));
            }
        }
    }
    // Zero examinados seria um guarda que não viu nada.
    assert!(examinados >= 1, "nenhum JavaScript do Workspace examinado");

    let fontes = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut pilha = vec![fontes];
    while let Some(dir) = pilha.pop() {
        for entrada in std::fs::read_dir(&dir).expect("src/") {
            let p = entrada.expect("entrada").path();
            if p.is_dir() {
                pilha.push(p);
            } else if p.extension().is_some_and(|e| e == "rs") {
                let texto = std::fs::read_to_string(&p).expect("ler");
                for proibido in ["__TAURI__", "__TAURI_INTERNALS__"] {
                    if texto.contains(proibido) {
                        violacoes.push(format!("{}: «{proibido}»", p.display()));
                    }
                }
            }
        }
    }

    assert!(
        violacoes.is_empty(),
        "detecção de runtime fora de static/runtime.js (ADR-0611):\n  {}",
        violacoes.join("\n  ")
    );
}

/// Os pares `nome: 'valor'` da tabela `WEB_TARGET`, pela ordem do ficheiro.
fn tabela_web(js: &str) -> Vec<(String, String)> {
    let inicio = js
        .find("const WEB_TARGET")
        .expect("WEB_TARGET em runtime.js");
    let corpo = &js[inicio..];
    let corpo = &corpo[corpo.find('{').expect("{") + 1..corpo.find("});").expect("});")];
    corpo
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(|l| {
            let (nome, valor) = l.split_once(':').expect("nome: valor");
            (
                nome.trim().to_owned(),
                valor
                    .trim()
                    .trim_end_matches(',')
                    .trim_matches('\'')
                    .to_owned(),
            )
        })
        .collect()
}

#[test]
fn a_tabela_da_web_e_a_do_contrato() {
    let js = std::fs::read_to_string(estaticos().join("runtime.js")).expect("runtime.js");
    let cliente = tabela_web(&js);
    let contrato: Vec<(String, String)> = RuntimeCapability::ALL
        .iter()
        .map(|c| {
            (
                c.as_str().to_owned(),
                c.target(RuntimeMode::Web).as_str().to_owned(),
            )
        })
        .collect();
    assert_eq!(
        cliente, contrato,
        "runtime.js e ocinye_contracts::runtime divergem"
    );
    assert!(
        js.contains(&format!("const CAPABILITY_VERSION = {CAPABILITY_VERSION};")),
        "a versão da declaração diverge"
    );
}

/// O `runtime.js` carrega antes do `app.js` em todas as páginas.
#[test]
fn o_runtime_carrega_antes_da_camada_de_interaccao() {
    let casca = include_str!("../src/ui/mod.rs");
    let runtime = casca
        .find("/static/runtime.js")
        .expect("runtime.js na casca");
    let app = casca.find("/static/app.js").expect("app.js na casca");
    assert!(runtime < app, "runtime.js tem de carregar antes de app.js");
}
