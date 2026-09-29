//! D004.1 · Contratos permanentes das aplicações de produtividade que se medem
//! sobre os ficheiros entregues, sem base de dados.

use std::fs;

fn ler(caminho: &str) -> String {
    fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/").to_owned() + caminho)
        .unwrap_or_else(|e| panic!("{caminho}: {e}"))
}

/// Os selectores de uma folha de estilo, sem comentários nem blocos @.
fn selectores(css: &str) -> Vec<String> {
    let mut sem = String::new();
    let mut resto = css;
    while let Some(i) = resto.find("/*") {
        sem.push_str(&resto[..i]);
        resto = resto[i..].find("*/").map_or("", |j| &resto[i + j + 2..]);
    }
    sem.push_str(resto);
    let mut out = Vec::new();
    for bloco in sem.split('}') {
        if let Some(i) = bloco.rfind('{') {
            let cabeca = bloco[..i].rsplit('{').next().unwrap_or("");
            for s in cabeca.split(',') {
                let s = s.trim();
                if !s.is_empty() && !s.starts_with('@') {
                    out.push(s.to_owned());
                }
            }
        }
    }
    out
}

/// As cores do Calendário vivem sob `.oc-app`: um `[data-scope]` global
/// casaria na casca, no Desktop ou na Nye (D4-S1, fechado pela D004.1).
#[test]
fn as_regras_de_ambito_do_calendario_nao_saem_das_aplicacoes() {
    let css = ler("static/oc-apps.css");
    let todos = selectores(&css);
    let ambito: Vec<_> = todos.iter().filter(|s| s.contains("[data-scope")).collect();
    assert!(
        !ambito.is_empty(),
        "nenhuma regra [data-scope]: o guarda não observou nada"
    );
    for s in ambito {
        assert!(s.starts_with(".oc-app"), "selector global: {s}");
    }
}

/// O motor das janelas liga o corpo que chega por `?frame=1` com o `init`
/// por raiz do Design, e esse `init` não liga a mesma aplicação duas vezes
/// (D4-J1, fechado pela D004.1). A prova de comportamento está no browser
/// (registo da integração); isto impede que o contrato se desfaça em silêncio.
#[test]
fn o_corpo_de_uma_janela_liga_se_pela_raiz_e_uma_so_vez() {
    let engine = ler("static/wm-engine.js");
    assert!(engine.contains("window.OcApps.init(body)"));
    assert!(!engine.contains("app-wired"), "o contorno da D004 voltou");
    let apps = ler("static/oc-apps.js");
    assert!(
        apps.contains("const bound = new WeakSet()")
            && apps.contains("if (bound.has(app)) return;")
    );
    assert!(apps.contains("const init = (root) =>"));
}
