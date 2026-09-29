//! D005 · Contratos permanentes das aplicações de investigação que se medem
//! sobre os ficheiros entregues, sem base de dados.

use std::fs;

fn ler(caminho: &str) -> String {
    fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/").to_owned() + caminho)
        .unwrap_or_else(|e| panic!("{caminho}: {e}"))
}

/// Os selectores de uma folha de estilo, sem comentários e sem cabeças @.
fn selectores(css: &str) -> Vec<String> {
    let mut sem = String::new();
    let mut resto = css;
    while let Some(i) = resto.find("/*") {
        sem.push_str(&resto[..i]);
        resto = resto[i..].find("*/").map_or("", |j| &resto[i + j + 2..]);
    }
    sem.push_str(resto);
    let mut out = Vec::new();
    let mut cabeca = String::new();
    for c in sem.chars() {
        match c {
            '{' => {
                let h = cabeca.trim().to_owned();
                if !h.starts_with('@') {
                    out.extend(
                        h.split(',')
                            .map(|s| s.trim().to_owned())
                            .filter(|s| !s.is_empty()),
                    );
                }
                cabeca.clear();
            }
            '}' | ';' => cabeca.clear(),
            _ => cabeca.push(c),
        }
    }
    out.retain(|s| !matches!(s.as_str(), "from" | "to") && !s.ends_with('%'));
    out
}

/// Nenhuma regra das aplicações (D004, D005) pinta fora delas: cada selector
/// ancora-se no espaço `.oc-` — num antepassado (`.oc-app *:focus-visible`) ou
/// no sujeito (`[data-js] .oc-cal-now`). Um `button`, `a`, `table`,
/// `[data-status]` ou `[data-scope]` soltos casariam na casca, no Desktop ou
/// na Nye.
#[test]
fn nenhuma_regra_das_aplicacoes_sai_do_espaco_oc() {
    let css = ler("static/oc-apps.css");
    let todos = selectores(&css);
    assert!(todos.len() > 200, "observou {} selectores", todos.len());
    for s in &todos {
        assert!(s.contains(".oc-"), "selector fora do espaço .oc-: {s}");
    }
}

/// `oc-apps.js` só tem o ajudante `$$` (lista). Um `$(` solto é um
/// `ReferenceError` a cada aplicação ligada: o teclado do `reslist` não
/// funcionava e a excepção cortava o `init` a meio (D005, corrigido pela Code).
#[test]
fn o_script_das_aplicacoes_nao_chama_um_ajudante_que_nao_existe() {
    let js = ler("static/oc-apps.js");
    let definido = js.contains("const $ =") || js.contains("function $(");
    let bytes = js.as_bytes();
    let soltos: Vec<usize> = js
        .match_indices("$(")
        .map(|(i, _)| i)
        .filter(|&i| {
            i == 0
                || !(bytes[i - 1] == b'$'
                    || bytes[i - 1].is_ascii_alphanumeric()
                    || bytes[i - 1] == b'_')
        })
        .collect();
    assert!(
        definido || soltos.is_empty(),
        "`$(` sem `$` definido em {} sítio(s)",
        soltos.len()
    );
}
