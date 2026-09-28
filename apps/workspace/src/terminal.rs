//! O lado do Workspace do Ocinye Terminal: traduzir a resposta do Core.
//!
//! # O Core devolve chaves; o Workspace devolve frases
//!
//! `POST /api/v1/commands/exec` responde com blocos tipados e chaves i18n
//! (ADR-0312 §7). Aqui cada chave vira texto no idioma do membro, e o resultado
//! segue para o `terminal.js`, que o desenha **só com nós de texto**. Nenhum
//! valor que venha de dados — o que o membro escreveu, um título, uma célula —
//! passa por HTML em lado nenhum.
//!
//! # O que isto não decide
//!
//! Nada. Não há parse que conte aqui, nem autorização: o que o Core disse é o
//! que se mostra. Um comando desconhecido não passa ao Nye; um erro do Core não
//! vira sucesso.

use ocinye_contracts::ocsh::registry::POSIX_HINTS;
use ocinye_contracts::ocsh::wire::{Block, ExecResponse, Tone};
use serde_json::{json, Value};

use crate::i18n::{has, t, tf};

/// A versão do ocsh mostrada nas boas-vindas e na barra de estado.
pub const OCSH_VERSION: &str = "1.0";

/// A resposta do Core, pronta para desenhar.
#[must_use]
pub fn localize(response: &ExecResponse) -> Value {
    let context = response
        .context
        .code
        .clone()
        .unwrap_or_else(|| t("terminal.context.personal").to_owned());
    json!({
        "exit": response.exit,
        "ms": response.ms,
        "capability": response.capability,
        "context": {
            "id": response.context.workspace_id,
            "label": context,
            "title": response.context.title,
        },
        "blocks": response.blocks.iter().map(block).collect::<Vec<_>>(),
    })
}

/// Uma falha a caminho do Core, dita como um bloco do Terminal.
///
/// A linha não chegou a ser um comando: não há exit do Core a mostrar, e o
/// Terminal não finge que houve.
#[must_use]
pub fn transport_failure(key: &str, exit: u8) -> Value {
    json!({
        "exit": exit,
        "ms": 0,
        "capability": Value::Null,
        "context": Value::Null,
        "blocks": [note_value(Tone::Err, key, &[], &[])],
    })
}

fn block(block: &Block) -> Value {
    match block {
        Block::Note {
            tone,
            key,
            params,
            suggestions,
        } => note_value(*tone, key, params, suggestions),
        Block::Table {
            columns,
            rows,
            pipeline,
        } => json!({
            "kind": "table",
            "columns": columns.iter().map(|c| column(c)).collect::<Vec<_>>(),
            "rows": rows,
            "pipeline": pipeline,
            "empty": rows.is_empty().then(|| t("ocsh.no_results")),
        }),
        Block::Facts { rows } => json!({
            "kind": "facts",
            "rows": rows
                .iter()
                .map(|(id, value)| json!([column(id), value]))
                .collect::<Vec<_>>(),
        }),
        Block::Json { value } => json!({
            "kind": "json",
            "value": value,
            "copy": t("terminal.copy"),
            "copied": t("terminal.copied"),
        }),
        Block::Help { topic, entries } => json!({
            "kind": "help",
            "topic": topic,
            "tagline": topic.is_empty().then(|| t("ocsh.help.tagline")),
            "footer": t("ocsh.help.footer"),
            "entries": entries
                .iter()
                .map(|(usage, key)| json!([usage, t(key)]))
                .collect::<Vec<_>>(),
        }),
        Block::Client { action } => json!({ "kind": "client", "action": action }),
    }
}

/// O rótulo de uma coluna: `ocsh.col.<id>`, ou o próprio id se faltar.
fn column(id: &str) -> String {
    let key = format!("ocsh.col.{id}");
    if has(&key) {
        t(&key).to_owned()
    } else {
        id.to_owned()
    }
}

fn note_value(tone: Tone, key: &str, params: &[(String, String)], suggestions: &[String]) -> Value {
    let args: Vec<(&str, &str)> = params
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    let title = tf(key, &args);
    // O desenho escreve `ocsh.host.title` / `ocsh.host.detail` e
    // `ocsh.denied` / `ocsh.denied.detail`: as duas formas.
    let detail_key = key
        .strip_suffix(".title")
        .map_or_else(|| format!("{key}.detail"), |base| format!("{base}.detail"));
    let hint_key = format!("{key}.hint");
    // Uma palavra POSIX (`ls`, `rm`…) não é um erro de dedo: é outro modelo.
    // A frase diz qual é o do ocsh, e a sugestão aponta a família.
    let posix = key == "ocsh.err.not_found"
        && params
            .iter()
            .any(|(k, v)| k == "cmd" && POSIX_HINTS.iter().any(|(w, _)| w == v));
    let detail = if posix {
        Some(t("ocsh.err.posix").to_owned())
    } else {
        has(&detail_key).then(|| tf(&detail_key, &args))
    };
    let hint = has(&hint_key).then(|| t(&hint_key));
    let did_you_mean = (key == "ocsh.err.not_found")
        .then(|| suggestions.first())
        .flatten()
        .map(|cmd| tf("ocsh.err.did_you_mean", &[("cmd", cmd)]));
    json!({
        "kind": "note",
        "tone": tone,
        "title": title,
        "detail": detail,
        "hint": hint,
        "did_you_mean": did_you_mean,
        "suggestions": suggestions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ocinye_contracts::ocsh::wire::ContextView;

    fn resposta(blocks: Vec<Block>) -> ExecResponse {
        ExecResponse {
            exit: 0,
            blocks,
            capability: None,
            context: ContextView::personal(),
            ms: 3,
        }
    }

    #[test]
    fn a_nota_leva_o_texto_e_nunca_a_chave() {
        let r = localize(&resposta(vec![Block::Note {
            tone: Tone::Err,
            key: "ocsh.err.not_found".into(),
            params: vec![("cmd".into(), "prjects".into())],
            suggestions: vec!["projects".into()],
        }]));
        let nota = &r["blocks"][0];
        let titulo = nota["title"].as_str().unwrap();
        assert!(titulo.contains("prjects"), "{titulo}");
        assert!(!titulo.contains("ocsh."), "chave crua: {titulo}");
        assert!(nota["did_you_mean"].as_str().unwrap().contains("projects"));
    }

    #[test]
    fn palavras_posix_explicam_o_modelo_do_ocsh() {
        let r = localize(&resposta(vec![Block::Note {
            tone: Tone::Err,
            key: "ocsh.err.not_found".into(),
            params: vec![("cmd".into(), "ls".into())],
            suggestions: vec![],
        }]));
        assert_eq!(r["blocks"][0]["detail"], t("ocsh.err.posix"));
    }

    #[test]
    fn host_e_sudo_trazem_o_porque() {
        for key in ["ocsh.host.title", "ocsh.sudo.title", "ocsh.denied"] {
            let r = localize(&resposta(vec![Block::Note {
                tone: Tone::Deny,
                key: key.into(),
                params: vec![],
                suggestions: vec![],
            }]));
            assert!(r["blocks"][0]["detail"].is_string(), "{key} sem detalhe");
        }
    }

    #[test]
    fn as_colunas_sao_rotuladas_e_as_celulas_ficam_como_vieram() {
        let r = localize(&resposta(vec![Block::Table {
            columns: vec!["title".into()],
            rows: vec![vec!["<b>x</b>".into()]],
            pipeline: None,
        }]));
        assert_eq!(r["blocks"][0]["columns"][0], t("ocsh.col.title"));
        assert_eq!(r["blocks"][0]["rows"][0][0], "<b>x</b>");
    }

    /// Cada chave que o Core pode emitir tem texto no catálogo.
    #[test]
    fn todas_as_chaves_do_core_tem_texto() {
        let fonte = include_str!("../../../crates/ocinye-core/src/modules/terminal/mod.rs");
        let mut em_falta = Vec::new();
        for pedaço in fonte.split('"').skip(1).step_by(2) {
            if pedaço.starts_with("ocsh.")
                && pedaço
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c == '.' || c == '_')
                && !has(pedaço)
            {
                em_falta.push(pedaço.to_owned());
            }
        }
        for c in ocinye_contracts::ocsh::registry::COMMANDS {
            if !has(c.help_key) {
                em_falta.push(c.help_key.to_owned());
            }
            if let ocinye_contracts::ocsh::registry::OutputShape::Table(cols)
            | ocinye_contracts::ocsh::registry::OutputShape::Facts(cols) = c.output
            {
                for col in cols {
                    let k = format!("ocsh.col.{col}");
                    if !has(&k) {
                        em_falta.push(k);
                    }
                }
            }
        }
        em_falta.sort();
        em_falta.dedup();
        assert!(em_falta.is_empty(), "sem texto: {em_falta:?}");
    }
}
