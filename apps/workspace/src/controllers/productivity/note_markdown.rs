//! O texto das Notas no editor do Design (D004): Markdown restrito ⇄
//! `NoteDocument` do Core.
//!
//! O Core guarda uma nota como documento estruturado (ADR-0413): parágrafos,
//! títulos 1–3, listas planas, listas de tarefas, código, imagens e anexos por
//! versão de ficheiro; marcas negrito, itálico e código; ligações `http`,
//! `https`, `mailto` e `tel`. O editor do Design é um `textarea`. Esta
//! tradução é a fronteira entre os dois, e é **sem perdas**: tudo o que o
//! documento tem escreve-se como texto, e o texto volta ao mesmo documento.
//!
//! Só se lêem as estruturas que o documento tem. Tudo o resto é texto: uma
//! linha `> citação` (o botão «citação» do Design; o Core não tem citação) fica
//! um parágrafo com esse texto, tal como foi escrito. Ao escrever, uma linha
//! de texto que pareceria estrutura leva `\` à frente, para nunca mudar de
//! natureza.
//!
//! Nada daqui é HTML nem autoridade: o resultado vai ao Core, que o valida
//! (`NoteDocument::from_value`) antes de o guardar.

use serde_json::{json, Value};

/// Os esquemas de ligação que o Core aceita.
const LINK_SCHEMES: &[&str] = &["http://", "https://", "mailto:", "tel:"];
/// Imagens e anexos da nota, pela versão exacta do ficheiro.
const IMAGE_PREFIX: &str = "ocinye:file/";
const ATTACHMENT_PREFIX: &str = "ocinye:attachment/";

// ── Documento → texto ────────────────────────────────────────────────────

/// O texto do editor para este documento. `None` se o documento tiver algo
/// que esta tradução não conhece (o editor fica só de leitura, nunca perde).
#[must_use]
pub fn to_markdown(doc: &Value) -> Option<String> {
    let blocks = doc.get("blocks")?.as_array()?;
    let mut out: Vec<String> = Vec::new();
    for b in blocks {
        let piece = match b.get("type")?.as_str()? {
            "paragraph" => paragraph_text(b.get("content"))?,
            "heading" => {
                let level = b.get("level")?.as_u64()?;
                if !(1..=3).contains(&level) {
                    return None;
                }
                format!(
                    "{} {}",
                    "#".repeat(usize::try_from(level).ok()?),
                    inlines(b.get("content"))?
                )
            }
            "bullet_list" => list(b.get("items")?, |_| "- ".to_owned())?,
            "ordered_list" => list(b.get("items")?, |i| format!("{}. ", i + 1))?,
            "checklist" => {
                let mut lines = Vec::new();
                for it in b.get("items")?.as_array()? {
                    let mark = if it.get("checked").and_then(Value::as_bool).unwrap_or(false) {
                        "x"
                    } else {
                        " "
                    };
                    lines.push(format!("- [{mark}] {}", inlines(it.get("content"))?));
                }
                lines.join("\n")
            }
            "code_block" => {
                let lang = b.get("language").and_then(Value::as_str).unwrap_or("");
                let text = b.get("text").and_then(Value::as_str).unwrap_or("");
                if text.lines().any(|l| l.trim_start().starts_with("```")) {
                    return None;
                }
                format!("```{lang}\n{text}\n```")
            }
            "image" => format!(
                "![{}]({IMAGE_PREFIX}{})",
                escape(b.get("alt").and_then(Value::as_str).unwrap_or("")),
                b.get("file_version_id")?.as_str()?
            ),
            "attachment" => format!(
                "[{}]({ATTACHMENT_PREFIX}{})",
                escape(b.get("name").and_then(Value::as_str).unwrap_or("")),
                b.get("file_version_id")?.as_str()?
            ),
            _ => return None,
        };
        out.push(piece);
    }
    Some(out.join("\n\n"))
}

fn list(items: &Value, marker: impl Fn(usize) -> String) -> Option<String> {
    let mut lines = Vec::new();
    for (i, it) in items.as_array()?.iter().enumerate() {
        lines.push(format!("{}{}", marker(i), inlines(Some(it))?));
    }
    Some(lines.join("\n"))
}

/// Um parágrafo: cada linha que pareceria estrutura leva `\`.
fn paragraph_text(content: Option<&Value>) -> Option<String> {
    let text = inlines(content)?;
    Some(
        text.split('\n')
            .map(|line| {
                if looks_structural(line) {
                    format!("\\{line}")
                } else {
                    line.to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

fn inlines(content: Option<&Value>) -> Option<String> {
    let Some(content) = content else {
        return Some(String::new());
    };
    let mut out = String::new();
    for i in content.as_array()? {
        let marks: Vec<&str> = i
            .get("marks")
            .and_then(Value::as_array)
            .map(|m| m.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        let text = i.get("text")?.as_str()?;
        let mut body = if marks.contains(&"code") {
            if text.contains('`') || text.is_empty() {
                return None;
            }
            format!("`{text}`")
        } else {
            escape(text)
        };
        if marks.contains(&"italic") {
            body = format!("_{body}_");
        }
        if marks.contains(&"bold") {
            body = format!("**{body}**");
        }
        match i.get("type")?.as_str()? {
            "text" => out.push_str(&body),
            "link" => {
                let href = i.get("href")?.as_str()?;
                if href.contains(')') || href.contains(char::is_whitespace) {
                    return None;
                }
                out.push_str(&format!("[{body}]({href})"));
            }
            _ => return None,
        }
    }
    Some(out)
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if matches!(c, '\\' | '*' | '_' | '`' | '[' | ']') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Uma linha que a leitura tomaria por estrutura.
fn looks_structural(line: &str) -> bool {
    line.starts_with("# ")
        || line.starts_with("## ")
        || line.starts_with("### ")
        || line.starts_with("- ")
        || line.starts_with("* ")
        || line.starts_with("```")
        || line.starts_with('\\')
        || ordered_marker(line).is_some()
        || media_line(line).is_some()
}

// ── Texto → documento ────────────────────────────────────────────────────

/// O documento para este texto. Sempre um documento válido na forma; o Core
/// valida o resto (limites, esquemas das ligações).
#[must_use]
pub fn from_markdown(text: &str) -> Value {
    let text = text.replace("\r\n", "\n");
    let lines: Vec<&str> = text.split('\n').collect();
    let mut blocks: Vec<Value> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if line.trim().is_empty() {
            i += 1;
            continue;
        }
        if let Some(lang) = line.strip_prefix("```") {
            let mut body = Vec::new();
            let mut j = i + 1;
            while j < lines.len() && lines[j].trim_end() != "```" {
                body.push(lines[j]);
                j += 1;
            }
            if j < lines.len() {
                let lang = lang.trim();
                blocks.push(json!({
                    "type": "code_block",
                    "language": if lang.is_empty() { Value::Null } else { Value::String(lang.to_owned()) },
                    "text": body.join("\n"),
                }));
                i = j + 1;
                continue;
            }
        }
        if let Some((level, rest)) = heading(line) {
            blocks
                .push(json!({ "type": "heading", "level": level, "content": parse_inlines(rest) }));
            i += 1;
            continue;
        }
        if let Some(block) = media_line(line) {
            blocks.push(block);
            i += 1;
            continue;
        }
        if checklist_item(line).is_some() {
            let mut items = Vec::new();
            while i < lines.len() {
                let Some((checked, rest)) = checklist_item(lines[i]) else {
                    break;
                };
                items.push(json!({ "checked": checked, "content": parse_inlines(rest) }));
                i += 1;
            }
            blocks.push(json!({ "type": "checklist", "items": items }));
            continue;
        }
        if bullet(line).is_some() {
            let mut items = Vec::new();
            while i < lines.len() && checklist_item(lines[i]).is_none() {
                let Some(rest) = bullet(lines[i]) else { break };
                items.push(parse_inlines(rest));
                i += 1;
            }
            blocks.push(json!({ "type": "bullet_list", "items": items }));
            continue;
        }
        if ordered_marker(line).is_some() {
            let mut items = Vec::new();
            while i < lines.len() {
                let Some(rest) = ordered_marker(lines[i]) else {
                    break;
                };
                items.push(parse_inlines(rest));
                i += 1;
            }
            blocks.push(json!({ "type": "ordered_list", "items": items }));
            continue;
        }
        // Um parágrafo: linhas seguidas até uma linha vazia ou uma estrutura.
        let mut para = Vec::new();
        while i < lines.len() && !lines[i].trim().is_empty() {
            let l = lines[i];
            if !para.is_empty() && looks_structural(l) && !l.starts_with('\\') {
                break;
            }
            para.push(
                l.strip_prefix('\\')
                    .filter(|r| looks_structural(r))
                    .unwrap_or(l),
            );
            i += 1;
        }
        blocks.push(json!({ "type": "paragraph", "content": parse_inlines(&para.join("\n")) }));
    }
    if blocks.is_empty() {
        blocks.push(json!({ "type": "paragraph", "content": [] }));
    }
    json!({ "schema_version": 1, "blocks": blocks })
}

fn heading(line: &str) -> Option<(u8, &str)> {
    for (level, prefix) in [(3, "### "), (2, "## "), (1, "# ")] {
        if let Some(rest) = line.strip_prefix(prefix) {
            return Some((level, rest));
        }
    }
    None
}

fn bullet(line: &str) -> Option<&str> {
    line.strip_prefix("- ").or_else(|| line.strip_prefix("* "))
}

fn checklist_item(line: &str) -> Option<(bool, &str)> {
    let rest = line.strip_prefix("- [")?;
    let (mark, rest) = rest.split_at_checked(1)?;
    let rest = rest
        .strip_prefix("] ")
        .or_else(|| (rest == "]").then_some(""))?;
    match mark {
        " " => Some((false, rest)),
        "x" | "X" => Some((true, rest)),
        _ => None,
    }
}

fn ordered_marker(line: &str) -> Option<&str> {
    let digits = line.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 || digits > 4 {
        return None;
    }
    line[digits..].strip_prefix(". ")
}

/// Uma linha que é só uma imagem ou um anexo da nota.
fn media_line(line: &str) -> Option<Value> {
    let (is_image, rest) = match line.strip_prefix("![") {
        Some(r) => (true, r),
        None => (false, line.strip_prefix('[')?),
    };
    let close = find_unescaped(rest, ']')?;
    let label = unescape(&rest[..close]);
    let target = rest[close + 1..].strip_prefix('(')?.strip_suffix(')')?;
    let prefix = if is_image {
        IMAGE_PREFIX
    } else {
        ATTACHMENT_PREFIX
    };
    let id = target.strip_prefix(prefix)?;
    uuid::Uuid::parse_str(id).ok()?;
    Some(if is_image {
        json!({ "type": "image", "file_version_id": id, "alt": label })
    } else {
        json!({ "type": "attachment", "file_version_id": id, "name": label })
    })
}

fn find_unescaped(s: &str, target: char) -> Option<usize> {
    let mut escaped = false;
    for (i, c) in s.char_indices() {
        if escaped {
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == target {
            return Some(i);
        }
    }
    None
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(n) = chars.next() {
                out.push(n);
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// As marcas e ligações de uma linha de texto.
fn parse_inlines(text: &str) -> Value {
    let mut out: Vec<Value> = Vec::new();
    parse_into(text, &[], &mut out);
    Value::Array(merge(out))
}

fn push_text(out: &mut Vec<Value>, text: String, marks: &[&str]) {
    if !text.is_empty() {
        out.push(json!({ "type": "text", "text": text, "marks": marks }));
    }
}

fn parse_into(text: &str, marks: &[&str], out: &mut Vec<Value>) {
    let chars: Vec<char> = text.chars().collect();
    let mut buf = String::new();
    let mut i = 0;
    // Linear, e não quadrático (A001-M008): uma linha de um milhão de
    // caracteres não pode prender um trabalhador. A partir de onde já se sabe
    // que um delimitador não fecha, não se volta a procurar; um `[` cuja
    // ligação falhou num `]` diz o mesmo de todos os `[` antes dele.
    let mut no_close: [usize; 3] = [usize::MAX; 3]; // `**`, `*`, `_`
    let mut closing = |chars: &[char], from: usize, delim: &str, slot: usize| {
        if from >= no_close[slot] {
            return None;
        }
        let found = find_closing(chars, from, delim);
        if found.is_none() {
            no_close[slot] = no_close[slot].min(from);
        }
        found
    };
    let mut no_link = false;
    let mut link_skip_until = 0usize;
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' && i + 1 < chars.len() {
            buf.push(chars[i + 1]);
            i += 2;
            continue;
        }
        if c == '`' {
            if let Some(end) = chars[i + 1..].iter().position(|&x| x == '`') {
                if end > 0 {
                    push_text(out, std::mem::take(&mut buf), marks);
                    let code: String = chars[i + 1..i + 1 + end].iter().collect();
                    let mut m = marks.to_vec();
                    m.push("code");
                    out.push(json!({ "type": "text", "text": code, "marks": m }));
                    i += end + 2;
                    continue;
                }
            }
        }
        if c == '*' && chars.get(i + 1) == Some(&'*') {
            if let Some(end) = closing(&chars, i + 2, "**", 0) {
                push_text(out, std::mem::take(&mut buf), marks);
                let inner: String = chars[i + 2..end].iter().collect();
                let mut m = marks.to_vec();
                m.push("bold");
                parse_into(&inner, &m, out);
                i = end + 2;
                continue;
            }
        }
        if c == '_' || c == '*' {
            let delim = c.to_string();
            let slot = if c == '*' { 1 } else { 2 };
            if let Some(end) = closing(&chars, i + 1, &delim, slot) {
                if end > i + 1 {
                    push_text(out, std::mem::take(&mut buf), marks);
                    let inner: String = chars[i + 1..end].iter().collect();
                    let mut m = marks.to_vec();
                    m.push("italic");
                    parse_into(&inner, &m, out);
                    i = end + 1;
                    continue;
                }
            }
        }
        if c == '[' && !no_link && i >= link_skip_until {
            let attempt = link_at(&chars[i..]);
            match attempt {
                Err(None) => no_link = true,
                Err(Some(close)) => link_skip_until = i + close,
                Ok(_) => {}
            }
            if let Ok((label, href, used)) = attempt {
                push_text(out, std::mem::take(&mut buf), marks);
                let mut inner = Vec::new();
                parse_into(&label, marks, &mut inner);
                let mut first_marks: Vec<Value> = Vec::new();
                let mut label_text = String::new();
                for part in inner {
                    if first_marks.is_empty() {
                        first_marks = part
                            .get("marks")
                            .and_then(Value::as_array)
                            .cloned()
                            .unwrap_or_default();
                    }
                    label_text.push_str(part.get("text").and_then(Value::as_str).unwrap_or(""));
                }
                out.push(json!({ "type": "link", "href": href, "text": label_text, "marks": first_marks }));
                i += used;
                continue;
            }
        }
        buf.push(c);
        i += 1;
    }
    push_text(out, buf, marks);
}

/// O fecho de `delim` a partir de `from`, saltando os escapados.
fn find_closing(chars: &[char], from: usize, delim: &str) -> Option<usize> {
    let d: Vec<char> = delim.chars().collect();
    let mut i = from;
    while i + d.len() <= chars.len() {
        if chars[i] == '\\' {
            i += 2;
            continue;
        }
        if chars[i..i + d.len()] == d[..] {
            // `**` não fecha um `*` simples.
            if d.len() == 1 && i + 1 < chars.len() && chars[i + 1] == d[0] && d[0] == '*' {
                i += 2;
                continue;
            }
            return Some(i);
        }
        i += 1;
    }
    None
}

/// `[texto](destino)` com um esquema aceite. Lê os caracteres onde estão,
/// sem copiar o resto da linha. A recusa diz onde estava o `]`
/// (`Err(Some(posição))`) ou que nenhuma ligação desta linha pode fechar a partir
/// daqui — sem `]`, ou sem `)` depois dele (`Err(None)`), para quem chama
/// não repetir a mesma procura a partir de cada `[` seguinte.
fn link_at(chars: &[char]) -> Result<(String, String, usize), Option<usize>> {
    let mut escaped = false;
    let mut close = None;
    for (j, &c) in chars.iter().enumerate().skip(1) {
        if escaped {
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == ']' {
            close = Some(j);
            break;
        }
    }
    let close = close.ok_or(None)?;
    if chars.get(close + 1) != Some(&'(') {
        return Err(Some(close));
    }
    // Sem nenhum `)` depois daqui, nenhuma ligação seguinte nesta linha fecha:
    // `Err(None)` diz isso a quem chama.
    let end = chars[close + 2..]
        .iter()
        .position(|&c| c == ')')
        .map(|p| close + 2 + p)
        .ok_or(None)?;
    let href: String = chars[close + 2..end].iter().collect();
    if href.is_empty()
        || href.contains(char::is_whitespace)
        || !LINK_SCHEMES.iter().any(|p| href.starts_with(p))
    {
        return Err(Some(close));
    }
    let label: String = chars[1..close].iter().collect();
    Ok((label, href, end + 1))
}

/// Junta trechos seguidos com as mesmas marcas.
fn merge(parts: Vec<Value>) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();
    for p in parts {
        if let (Some(last), true) = (out.last_mut(), p["type"] == "text") {
            if last["type"] == "text" && last["marks"] == p["marks"] {
                let joined = format!(
                    "{}{}",
                    last["text"].as_str().unwrap_or(""),
                    p["text"].as_str().unwrap_or("")
                );
                last["text"] = Value::String(joined);
                continue;
            }
        }
        out.push(p);
    }
    out
}

/// O número de palavras do texto (para nada que decida; só informação).
#[must_use]
pub fn words(text: &str) -> usize {
    text.split_whitespace().count()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Uma linha hostil não prende o trabalhador (A001-M008): marcas e `[`
    /// sem fecho, aos cem mil, lêem-se em tempo linear e ficam texto.
    #[test]
    fn uma_linha_hostil_le_se_em_tempo_linear() {
        for hostile in [
            "*a".repeat(100_000),
            "_".repeat(200_000),
            "**x".repeat(60_000),
            "[".repeat(200_000),
            "[a](x".repeat(40_000),
            format!("{}]", "[".repeat(100_000)),
        ] {
            let started = std::time::Instant::now();
            let doc = from_markdown(&hostile);
            assert!(
                started.elapsed() < std::time::Duration::from_secs(3),
                "{} caracteres demoraram {:?}",
                hostile.len(),
                started.elapsed()
            );
            assert!(doc.is_object());
        }
    }

    /// E o que era formatação continua a ser.
    #[test]
    fn as_marcas_e_as_ligacoes_continuam_iguais() {
        let doc = from_markdown(
            "**negrito** _itálico_ `código` [sítio](https://exemplo.test) [x](javascript:alert(1))",
        );
        let s = doc.to_string();
        assert!(s.contains("\"bold\"") && s.contains("\"italic\"") && s.contains("\"code\""));
        assert!(s.contains("https://exemplo.test"));
        assert!(!s.contains("\"href\":\"javascript:"), "{s}");
    }

    fn roundtrip(doc: &Value) {
        let md = to_markdown(doc).expect("representável");
        let back = from_markdown(&md);
        assert_eq!(&back, doc, "documento → texto → documento mudou:\n{md}");
        assert_eq!(
            to_markdown(&back).as_deref(),
            Some(md.as_str()),
            "texto instável"
        );
    }

    fn text(t: &str, marks: &[&str]) -> Value {
        json!({ "type": "text", "text": t, "marks": marks })
    }

    #[test]
    fn todos_os_blocos_do_core_voltam_iguais() {
        let doc = json!({ "schema_version": 1, "blocks": [
            { "type": "heading", "level": 1, "content": [text("Torre 2", &[])] },
            { "type": "heading", "level": 3, "content": [text("Notas", &["italic"])] },
            { "type": "paragraph", "content": [
                text("Vento ", &[]), text("forte", &["bold"]), text(" e ", &[]),
                text("x = 1", &["code"]), text(" ver ", &[]),
                { "type": "link", "href": "https://ocinye.com/a_b", "text": "site", "marks": [] }
            ]},
            { "type": "bullet_list", "items": [[text("um", &[])], [text("dois", &["bold", "italic"])]] },
            { "type": "ordered_list", "items": [[text("primeiro", &[])], [text("segundo", &[])]] },
            { "type": "checklist", "items": [
                { "checked": true, "content": [text("feito", &[])] },
                { "checked": false, "content": [text("por fazer", &[])] }
            ]},
            { "type": "code_block", "language": "rust", "text": "fn main() {\n    let a = *b;\n}" },
            { "type": "image", "file_version_id": "0f0e0d0c-0b0a-4908-8706-050403020100", "alt": "Anemómetro [A]" },
            { "type": "attachment", "file_version_id": "1f0e0d0c-0b0a-4908-8706-050403020100", "name": "dados.csv" }
        ]});
        roundtrip(&doc);
    }

    #[test]
    fn texto_que_parece_estrutura_continua_texto() {
        let doc = json!({ "schema_version": 1, "blocks": [
            { "type": "paragraph", "content": [text("# não é título\n- nem lista\n1. nem esta\n> citação\n*asteriscos* e _traços_ \\ [x]", &[])] }
        ]});
        roundtrip(&doc);
    }

    #[test]
    fn a_citacao_do_design_fica_um_paragrafo_com_o_que_foi_escrito() {
        let doc = from_markdown("> uma citação");
        assert_eq!(doc["blocks"][0]["type"], "paragraph");
        assert_eq!(doc["blocks"][0]["content"][0]["text"], "> uma citação");
    }

    #[test]
    fn ligacoes_so_com_esquemas_aceites() {
        let doc = from_markdown("[x](javascript:alert(1)) [y](https://a.b)");
        let content = doc["blocks"][0]["content"].as_array().unwrap();
        assert!(content.iter().all(|c| c["href"] != "javascript:alert(1)"));
        assert!(content
            .iter()
            .any(|c| c["type"] == "link" && c["href"] == "https://a.b"));
    }

    #[test]
    fn html_e_so_texto() {
        let doc = from_markdown("<script>alert(1)</script> <img onerror=x>");
        assert_eq!(doc["blocks"][0]["type"], "paragraph");
        assert_eq!(
            doc["blocks"][0]["content"][0]["text"],
            "<script>alert(1)</script> <img onerror=x>"
        );
    }

    #[test]
    fn o_texto_vazio_e_a_nota_nova() {
        assert_eq!(
            from_markdown(""),
            json!({ "schema_version": 1, "blocks": [{ "type": "paragraph", "content": [] }] })
        );
        assert_eq!(to_markdown(&from_markdown("")).as_deref(), Some(""));
    }

    #[test]
    fn o_que_nao_conhece_nao_se_edita() {
        let doc = json!({ "schema_version": 1, "blocks": [{ "type": "table", "rows": [] }] });
        assert!(to_markdown(&doc).is_none());
    }
}
