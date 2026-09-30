//! O lado do Workspace do Ocinye Terminal: traduzir a resposta do Core.
//!
//! # O Core devolve chaves; o Workspace devolve frases
//!
//! `POST /api/v1/commands/exec` responde com blocos tipados e chaves i18n
//! (ADR-0312 §7). Aqui cada chave vira texto no idioma do membro, na forma que
//! o `oc-terminal.js` do Design desenha (D008 · T-05) **só com nós de texto**,
//! ou, sem JavaScript, na entrada tipada que o ecrã desenha no servidor. Nenhum
//! valor que venha de dados — o que o membro escreveu, um título, uma célula —
//! passa por HTML em lado nenhum.
//!
//! # O que isto não decide
//!
//! Nada. Não há parse que conte aqui, nem autorização: o que o Core disse é o
//! que se mostra. Um comando desconhecido não passa à Nye; um erro do Core não
//! vira sucesso. A única coisa que o Workspace acrescenta é o `echo` — a linha
//! **redigida** por `ocsh::redact` (T-06) —, que é tudo o que o cliente guarda.

use ocinye_contracts::ocsh::registry::{Audience, Binding, CommandSpec, COMMANDS, POSIX_WORDS};
use ocinye_contracts::ocsh::wire::{Block, ExecResponse, Tone};
use serde_json::{json, Value};

use crate::i18n::{has, t, tf};
use crate::ui::view_models::{TermBlockVm, TermEntryVm, TermGroup, TermRegistryEntryVm, TermTone};

/// A versão do ocsh mostrada nas boas-vindas.
pub const OCSH_VERSION: &str = "1.0";

/// O rótulo do contexto: o código do ambiente, ou «pessoal».
#[must_use]
pub fn context_label(response: &ExecResponse) -> String {
    response
        .context
        .code
        .clone()
        .unwrap_or_else(|| t("terminal.context.personal").to_owned())
}

/// A resposta do Core, pronta para o `oc-terminal.js` (HANDOFF D008 §T-06).
///
/// `echo` é a linha redigida (`ocsh::redact`), nunca a escrita.
#[must_use]
pub fn localize(response: &ExecResponse, echo: &str) -> Value {
    json!({
        "exit": response.exit,
        "ms": response.ms,
        "capability": response.capability,
        "echo": echo,
        "context": {
            "id": response.context.workspace_id,
            "label": context_label(response),
            "title": response.context.title,
        },
        "blocks": response.blocks.iter().filter_map(block_json).collect::<Vec<_>>(),
    })
}

/// A mesma resposta como entrada tipada do ecrã, para o percurso sem
/// JavaScript (o formulário do prompt).
#[must_use]
pub fn entry(response: &ExecResponse, echo: String) -> TermEntryVm {
    TermEntryVm {
        echo,
        context_label: context_label(response),
        exit: Some(response.exit),
        ms: Some(response.ms),
        capability: response.capability.clone(),
        blocks: response.blocks.iter().filter_map(block_vm).collect(),
    }
}

/// Uma falha a caminho do Core, dita como um bloco do Terminal.
///
/// A linha não chegou a ser um comando: não há exit do Core a mostrar, e o
/// Terminal não finge que houve.
#[must_use]
pub fn transport_failure(key: &str, exit: u8, echo: &str) -> Value {
    let n = note(Tone::Err, key, &[], &[]);
    json!({
        "exit": exit,
        "ms": 0,
        "capability": Value::Null,
        "echo": echo,
        "context": Value::Null,
        "blocks": [n.json()],
    })
}

/// A pergunta da ponte explícita, quando o Core leu `nye ask …` / `? …`.
#[must_use]
pub fn question(response: &ExecResponse) -> Option<&str> {
    response.blocks.iter().find_map(|b| match b {
        Block::Ask { question } => Some(question.as_str()),
        _ => None,
    })
}

/// O que a Nye respondeu pelo caminho canónico (`POST /api/v1/ai/prompt`):
/// o código de saída e o bloco. Uma conclusão degradada é «indisponível» (69)
/// com o motivo tipado do Core — nunca uma resposta inventada.
#[must_use]
pub fn nye_reply(reply: &Value) -> (u8, TermBlockVm) {
    let degraded = reply.get("status").and_then(Value::as_str) == Some("DEGRADED");
    if degraded {
        let code = reply
            .get("reason_code")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let reason = crate::controllers::nye::reason_of(code);
        return (
            69,
            TermBlockVm::Note {
                tone: TermTone::Warn,
                title: t(reason.key()).to_owned(),
                body: Some(t("term.nye.note").to_owned()),
                suggestions: vec![],
            },
        );
    }
    let content = reply
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let paragraphs = content
        .split("\n\n")
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(str::to_owned)
        .collect();
    (
        0,
        TermBlockVm::Nye {
            paragraphs,
            sources: vec![],
        },
    )
}

/// Um bloco já localizado como JSON do `oc-terminal.js`.
#[must_use]
pub fn vm_json(b: &TermBlockVm) -> Value {
    match b {
        TermBlockVm::Note {
            tone,
            title,
            body,
            suggestions,
        } => json!({
            "kind": "note",
            "tone": tone.id(),
            "text": title,
            "detail": body,
            "suggestions": suggestions,
        }),
        TermBlockVm::Nye {
            paragraphs,
            sources,
        } => json!({ "kind": "nye", "paragraphs": paragraphs, "sources": sources }),
        // Os outros blocos vêm sempre do Core, por `block_json`.
        _ => Value::Null,
    }
}

/// O grupo da ajuda de um comando: é a forma da ajuda, não uma permissão.
#[must_use]
pub fn group_of(c: &CommandSpec) -> TermGroup {
    match (c.binding, c.audience, c.family) {
        (Binding::Local, ..) => TermGroup::Shell,
        (Binding::Nye, ..) => TermGroup::Ai,
        (_, Audience::Operator, _) => TermGroup::System,
        (_, Audience::Admin, _) => TermGroup::Admin,
        (_, _, "tasks") => TermGroup::Work,
        _ => TermGroup::Workspace,
    }
}

/// A descoberta desta pessoa (ajuda imediata e autocompletar): os comandos que
/// a ajuda **do Core** lhe mostra, pela ordem do registo. Não é autorização.
#[must_use]
pub fn registry_of(help: &ExecResponse) -> Vec<TermRegistryEntryVm> {
    let visible: Vec<&str> = help
        .blocks
        .iter()
        .flat_map(|b| match b {
            Block::Help { entries, .. } => entries.iter().map(|(_, k)| k.as_str()).collect(),
            _ => vec![],
        })
        .collect();
    COMMANDS
        .iter()
        .filter(|c| visible.contains(&c.help_key))
        .map(|c| TermRegistryEntryVm {
            usage: usage(c),
            completion: if c.sub.is_empty() {
                c.family.to_owned()
            } else {
                format!("{} {}", c.family, c.sub)
            },
            help_key: c.help_key,
            group: group_of(c),
        })
        .collect()
}

fn usage(c: &CommandSpec) -> String {
    let mut out = c.family.to_owned();
    if !c.sub.is_empty() {
        out.push(' ');
        out.push_str(c.sub);
    }
    for a in c.args {
        out.push_str(&if a.required {
            format!(" <{}>", a.name)
        } else {
            format!(" [{}]", a.name)
        });
    }
    out
}

/// Uma nota localizada: título, corpo e sugestões.
struct Note {
    tone: Tone,
    title: String,
    body: Option<String>,
    suggestions: Vec<String>,
}

impl Note {
    fn json(&self) -> Value {
        json!({
            "kind": "note",
            "tone": self.tone,
            "text": self.title,
            "detail": self.body,
            "suggestions": self.suggestions,
        })
    }

    fn vm(self) -> TermBlockVm {
        TermBlockVm::Note {
            tone: tone_vm(self.tone),
            title: self.title,
            body: self.body,
            suggestions: self.suggestions,
        }
    }
}

const fn tone_vm(tone: Tone) -> TermTone {
    match tone {
        Tone::Ok => TermTone::Ok,
        Tone::Info => TermTone::Info,
        Tone::Warn => TermTone::Warn,
        Tone::Err => TermTone::Err,
        Tone::Deny => TermTone::Deny,
    }
}

fn note(tone: Tone, key: &str, params: &[(String, String)], suggestions: &[String]) -> Note {
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
    let not_found = key == "ocsh.err.not_found";
    // Uma palavra POSIX (`ls`, `rm`…) não é um erro de dedo: é outro modelo.
    let posix = not_found
        && params
            .iter()
            .any(|(k, v)| k == "cmd" && POSIX_WORDS.contains(&v.as_str()));
    let mut body: Vec<String> = Vec::new();
    if not_found {
        // «Quis dizer» só com o que o Core sugeriu — um comando que existe e
        // que esta pessoa vê (T-08).
        if let Some(cmd) = suggestions.first() {
            body.push(tf("ocsh.err.did_you_mean", &[("cmd", cmd)]));
        }
        body.push(
            t(if posix {
                "ocsh.err.posix"
            } else {
                "term.not_found.body"
            })
            .to_owned(),
        );
    } else if has(&detail_key) {
        body.push(tf(&detail_key, &args));
    }
    if has(&hint_key) {
        body.push(t(&hint_key).to_owned());
    }
    Note {
        tone,
        title,
        body: (!body.is_empty()).then(|| body.join(" ")),
        suggestions: suggestions.to_vec(),
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

/// A ajuda por grupos, pela ordem do registo; `usage` é sintaxe, nunca traduzida.
fn help_groups(entries: &[(String, String)]) -> Vec<(TermGroup, Vec<(String, String)>)> {
    let mut groups: Vec<(TermGroup, Vec<(String, String)>)> = Vec::new();
    for (usage, key) in entries {
        let group = COMMANDS
            .iter()
            .find(|c| c.help_key == key)
            .map_or(TermGroup::Shell, group_of);
        let item = (usage.clone(), t(key).to_owned());
        match groups.iter_mut().find(|(g, _)| *g == group) {
            Some((_, items)) => items.push(item),
            None => groups.push((group, vec![item])),
        }
    }
    groups
}

fn block_json(block: &Block) -> Option<Value> {
    Some(match block {
        Block::Note {
            tone,
            key,
            params,
            suggestions,
        } => note(*tone, key, params, suggestions).json(),
        Block::Table {
            columns,
            rows,
            pipeline,
        } => json!({
            "kind": "table",
            "columns": columns
                .iter()
                .map(|c| json!({ "id": c, "label": column(c) }))
                .collect::<Vec<_>>(),
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
        Block::Json { value } => json!({ "kind": "json", "value": value }),
        Block::Help { entries, .. } => {
            let mut out = Vec::new();
            for (group, items) in help_groups(entries) {
                out.push(json!({ "group": t(group.key()) }));
                for (usage, text) in items {
                    out.push(json!({ "usage": usage, "text": text }));
                }
            }
            json!({ "kind": "help", "entries": out, "footer": t("ocsh.help.footer") })
        }
        Block::Client { action } => json!({ "kind": "client", "action": action }),
        // A pergunta não se desenha: a resposta da Nye é que substitui o bloco.
        Block::Ask { .. } => return None,
    })
}

fn block_vm(block: &Block) -> Option<TermBlockVm> {
    Some(match block {
        Block::Note {
            tone,
            key,
            params,
            suggestions,
        } => note(*tone, key, params, suggestions).vm(),
        Block::Table {
            columns,
            rows,
            pipeline,
        } => TermBlockVm::Table {
            columns: columns.iter().map(|c| column(c)).collect(),
            rows: rows.clone(),
            pipeline: pipeline.clone(),
            empty: rows.is_empty().then(|| t("ocsh.no_results").to_owned()),
        },
        Block::Facts { rows } => TermBlockVm::Facts {
            rows: rows.iter().map(|(id, v)| (column(id), v.clone())).collect(),
        },
        // Sem JavaScript, o JSON mostra-se como os dados que são.
        Block::Json { value } => TermBlockVm::Facts {
            rows: vec![(
                "json".to_owned(),
                serde_json::to_string_pretty(value).unwrap_or_default(),
            )],
        },
        Block::Help { entries, .. } => TermBlockVm::Help {
            groups: help_groups(entries)
                .into_iter()
                .map(|(g, items)| (t(g.key()).to_owned(), items))
                .collect(),
            footer: Some(t("ocsh.help.footer").to_owned()),
        },
        // `clear`, `history` e `exit` são da sessão do cliente; sem JavaScript
        // não há sessão a limpar nem janela a fechar.
        Block::Client { .. } | Block::Ask { .. } => return None,
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
        let r = localize(
            &resposta(vec![Block::Note {
                tone: Tone::Err,
                key: "ocsh.err.not_found".into(),
                params: vec![("cmd".into(), "prjects".into())],
                suggestions: vec!["context".into()],
            }]),
            "prjects",
        );
        let nota = &r["blocks"][0];
        let titulo = nota["text"].as_str().unwrap();
        assert!(titulo.contains("prjects"), "{titulo}");
        assert!(!titulo.contains("ocsh."), "chave crua: {titulo}");
        let corpo = nota["detail"].as_str().unwrap();
        assert!(corpo.contains("context"), "{corpo}");
        assert!(corpo.contains(t("term.not_found.body")), "{corpo}");
        assert_eq!(r["echo"], "prjects");
    }

    #[test]
    fn palavras_posix_explicam_o_modelo_do_ocsh() {
        let r = localize(
            &resposta(vec![Block::Note {
                tone: Tone::Err,
                key: "ocsh.err.not_found".into(),
                params: vec![("cmd".into(), "ls".into())],
                suggestions: vec![],
            }]),
            "ls",
        );
        assert_eq!(r["blocks"][0]["detail"], t("ocsh.err.posix"));
        assert!(
            !t("ocsh.err.posix").contains("files"),
            "o texto não pode apontar para uma família que não existe"
        );
    }

    #[test]
    fn host_e_sudo_trazem_o_porque() {
        for key in ["ocsh.host.title", "ocsh.sudo.title", "ocsh.denied"] {
            let r = localize(
                &resposta(vec![Block::Note {
                    tone: Tone::Deny,
                    key: key.into(),
                    params: vec![],
                    suggestions: vec![],
                }]),
                "x",
            );
            assert!(r["blocks"][0]["detail"].is_string(), "{key} sem detalhe");
        }
    }

    #[test]
    fn as_colunas_sao_rotuladas_e_as_celulas_ficam_como_vieram() {
        let r = localize(
            &resposta(vec![Block::Table {
                columns: vec!["title".into()],
                rows: vec![vec!["<b>x</b>".into()]],
                pipeline: None,
            }]),
            "tasks list",
        );
        assert_eq!(r["blocks"][0]["columns"][0]["id"], "title");
        assert_eq!(r["blocks"][0]["columns"][0]["label"], t("ocsh.col.title"));
        assert_eq!(r["blocks"][0]["rows"][0][0], "<b>x</b>");
    }

    /// T-05: a ajuda chega por grupos, na forma que o `oc-terminal.js` lê.
    #[test]
    fn a_ajuda_chega_por_grupos() {
        let r = localize(
            &resposta(vec![Block::Help {
                topic: String::new(),
                entries: vec![
                    ("help [topic]".into(), "ocsh.cmd.help".into()),
                    ("whoami".into(), "ocsh.cmd.whoami".into()),
                ],
            }]),
            "help",
        );
        let e = r["blocks"][0]["entries"].as_array().unwrap();
        assert_eq!(e[0]["group"], t("ocsh.group.shell"));
        assert_eq!(e[1]["usage"], "help [topic]");
        assert_eq!(e[2]["group"], t("ocsh.group.workspace"));
        assert_eq!(e[3]["text"], t("ocsh.cmd.whoami"));
    }

    /// A pergunta nunca se desenha como bloco: só a resposta da Nye.
    #[test]
    fn a_pergunta_nao_e_um_bloco() {
        let r = resposta(vec![Block::Ask {
            question: "o que há hoje".into(),
        }]);
        assert_eq!(question(&r), Some("o que há hoje"));
        assert!(localize(&r, "nye ask")
            .get("blocks")
            .unwrap()
            .as_array()
            .unwrap()
            .is_empty());
    }

    #[test]
    fn a_nye_degradada_e_69_com_o_motivo() {
        let (exit, b) = nye_reply(&json!({
            "status": "DEGRADED",
            "reason_code": "AI_NO_PROVIDER_AVAILABLE",
            "content": "texto do Core"
        }));
        assert_eq!(exit, 69);
        let v = vm_json(&b);
        assert_eq!(v["text"], t("nye.reason.no_inference"));
        let (exit, b) = nye_reply(&json!({ "status": "COMPLETED", "content": "a\n\nb" }));
        assert_eq!(exit, 0);
        assert_eq!(vm_json(&b)["paragraphs"], json!(["a", "b"]));
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
        for c in COMMANDS {
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
