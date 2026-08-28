//! Example Ocinye capability: BibTeX import.
//!
//! Reads BibTeX on stdin and writes institutional source records as JSON on
//! stdout. It requests no network and no filesystem, which is what lets the
//! host run somebody else's parser over an uploaded file safely.
//!
//! This is an example of the contract, not a complete BibTeX implementation:
//! it handles the entry shapes the institution actually receives and reports
//! what it could not parse instead of guessing.

use std::collections::BTreeMap;
use std::io::{self, Read, Write};

use serde::Serialize;

/// One parsed bibliographic entry.
#[derive(Debug, Serialize)]
struct SourceRecord {
    /// BibTeX entry type, for example `article`.
    entry_type: String,
    /// Citation key.
    citation_key: String,
    /// Title.
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    /// Authors, split on ` and `.
    authors: Vec<String>,
    /// Year.
    #[serde(skip_serializing_if = "Option::is_none")]
    year: Option<i32>,
    /// Journal, proceedings or book title.
    #[serde(skip_serializing_if = "Option::is_none")]
    container_title: Option<String>,
    /// DOI.
    #[serde(skip_serializing_if = "Option::is_none")]
    doi: Option<String>,
    /// Every field as it appeared, kept for provenance.
    raw_fields: BTreeMap<String, String>,
}

/// What the capability returns.
#[derive(Debug, Serialize)]
struct Output {
    /// Successfully parsed entries.
    sources: Vec<SourceRecord>,
    /// Entries that could not be parsed, reported rather than dropped.
    skipped: Vec<String>,
    /// The parsed entries written back in a canonical shape.
    ///
    /// Only what parsed: an entry this component could not read is reported in
    /// `skipped` and never guessed at. Normalising and inventing are different
    /// things, and only the first belongs here.
    normalized: String,
}

fn main() {
    let mut input = String::new();
    if io::stdin().read_to_string(&mut input).is_err() {
        eprintln!("input is not valid UTF-8");
        std::process::exit(1);
    }

    let output = parse(&input);

    match serde_json::to_vec(&output) {
        Ok(bytes) => {
            if io::stdout().write_all(&bytes).is_err() {
                std::process::exit(1);
            }
        }
        Err(error) => {
            eprintln!("could not serialise output: {error}");
            std::process::exit(1);
        }
    }
}

/// Parse a BibTeX document.
fn parse(input: &str) -> Output {
    let mut sources = Vec::new();
    let mut skipped = Vec::new();

    for block in split_entries(input) {
        match parse_entry(&block) {
            Some(record) => sources.push(record),
            None => {
                // A short excerpt, not the whole block: diagnostics must not
                // become a second copy of the input.
                skipped.push(block.chars().take(60).collect::<String>().trim().to_owned());
            }
        }
    }

    let normalized = render(&sources);
    Output {
        sources,
        skipped,
        normalized,
    }
}

/// Write parsed entries back in a canonical shape.
///
/// # O que «normalizar» significa aqui, e o que não significa
///
/// Significa uma forma: tipo de entrada em minúsculas, um campo por linha,
/// chaves ordenadas alfabeticamente, valores entre chavetas, indentação
/// constante. Duas bibliografias com o mesmo conteúdo e formatações diferentes
/// saem daqui iguais.
///
/// Não significa corrigir. Um ano que não é um número, um DOI que não existe ou
/// um autor escrito ao contrário saem como entraram: este componente não tem
/// como saber o que estava certo, e adivinhar seria pior do que não mexer.
///
/// A ordem dos campos vem do `BTreeMap`, e a das entradas é a do documento. Não
/// há aqui relógio, aleatoriedade nem localização: a mesma entrada dá sempre a
/// mesma saída.
fn render(sources: &[SourceRecord]) -> String {
    let mut saida = String::new();
    for (indice, record) in sources.iter().enumerate() {
        if indice > 0 {
            saida.push('\n');
        }
        saida.push('@');
        saida.push_str(&record.entry_type);
        saida.push('{');
        saida.push_str(&record.citation_key);
        saida.push_str(",\n");

        let largura = record
            .raw_fields
            .keys()
            .map(String::len)
            .max()
            .unwrap_or(0);

        for (campo, valor) in &record.raw_fields {
            saida.push_str("  ");
            saida.push_str(campo);
            for _ in campo.len()..largura {
                saida.push(' ');
            }
            saida.push_str(" = {");
            saida.push_str(valor);
            saida.push_str("},\n");
        }
        saida.push_str("}\n");
    }
    saida
}

/// Split a document into entry blocks by brace depth.
///
/// Depth tracking rather than a naive split on `@`, because titles and
/// abstracts routinely contain `@`.
fn split_entries(input: &str) -> Vec<String> {
    let mut entries = Vec::new();
    let mut current = String::new();
    let mut depth = 0_i32;
    let mut in_entry = false;

    for character in input.chars() {
        if !in_entry {
            if character == '@' {
                in_entry = true;
                current.push(character);
            }
            continue;
        }

        current.push(character);
        match character {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth <= 0 {
                    entries.push(std::mem::take(&mut current));
                    in_entry = false;
                    depth = 0;
                }
            }
            _ => {}
        }
    }

    if !current.trim().is_empty() {
        entries.push(current);
    }
    entries
}

/// Parse one entry block.
fn parse_entry(block: &str) -> Option<SourceRecord> {
    let block = block.trim();
    let open = block.find('{')?;
    let entry_type = block[1..open].trim().to_ascii_lowercase();
    if entry_type.is_empty() {
        return None;
    }

    let body = block[open + 1..].trim_end().trim_end_matches('}');
    let (citation_key, rest) = body.split_once(',')?;
    let citation_key = citation_key.trim().to_owned();
    if citation_key.is_empty() {
        return None;
    }

    let raw_fields = parse_fields(rest);

    let authors = raw_fields
        .get("author")
        .map(|value| {
            value
                .split(" and ")
                .map(|author| author.trim().to_owned())
                .filter(|author| !author.is_empty())
                .collect()
        })
        .unwrap_or_default();

    Some(SourceRecord {
        title: raw_fields.get("title").cloned(),
        year: raw_fields.get("year").and_then(|value| value.trim().parse().ok()),
        container_title: raw_fields
            .get("journal")
            .or_else(|| raw_fields.get("booktitle"))
            .cloned(),
        doi: raw_fields.get("doi").cloned(),
        entry_type,
        citation_key,
        authors,
        raw_fields,
    })
}

/// Parse `key = {value}` pairs, respecting nested braces and quotes.
fn parse_fields(body: &str) -> BTreeMap<String, String> {
    let mut fields = BTreeMap::new();
    let mut chars = body.chars().peekable();

    loop {
        let mut key = String::new();
        while let Some(&character) = chars.peek() {
            chars.next();
            if character == '=' {
                break;
            }
            key.push(character);
        }

        let key = key.trim().to_ascii_lowercase();
        if key.is_empty() {
            break;
        }

        // Skip whitespace before the value.
        while chars.peek().is_some_and(|c| c.is_whitespace()) {
            chars.next();
        }

        let value = match chars.peek() {
            Some('{') => {
                chars.next();
                read_braced(&mut chars)
            }
            Some('"') => {
                chars.next();
                read_quoted(&mut chars)
            }
            // A bare value, such as a number or a string macro.
            Some(_) => {
                let mut value = String::new();
                while let Some(&character) = chars.peek() {
                    if character == ',' {
                        break;
                    }
                    value.push(character);
                    chars.next();
                }
                value.trim().to_owned()
            }
            None => break,
        };

        fields.insert(key, normalise(&value));

        // Consume the separating comma.
        while let Some(&character) = chars.peek() {
            chars.next();
            if character == ',' {
                break;
            }
        }

        if chars.peek().is_none() {
            break;
        }
    }

    fields
}

fn read_braced(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    let mut value = String::new();
    let mut depth = 1_i32;

    for character in chars.by_ref() {
        match character {
            '{' => {
                depth += 1;
                value.push(character);
            }
            '}' => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
                value.push(character);
            }
            _ => value.push(character),
        }
    }
    value
}

fn read_quoted(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    let mut value = String::new();
    for character in chars.by_ref() {
        if character == '"' {
            break;
        }
        value.push(character);
    }
    value
}

/// Collapse whitespace and strip the braces BibTeX uses to protect casing.
fn normalise(value: &str) -> String {
    value
        .replace(['{', '}'], "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
@article{mucai2024,
  title   = {Wind resource assessment for {Angola}: a coastal study},
  author  = {Mucai, Ana and Silva, João P.},
  journal = {Renewable Energy},
  year    = {2024},
  doi     = {10.1016/j.renene.2024.01.001}
}

@inproceedings{lueji2023,
  title     = "Distributed solar microgrids",
  author    = "Lueji, Maria",
  booktitle = "Proceedings of the Angolan Energy Conference",
  year      = 2023
}
"#;

    #[test]
    fn parses_both_brace_and_quote_delimited_entries() {
        let output = parse(SAMPLE);
        assert_eq!(output.sources.len(), 2);
        assert!(output.skipped.is_empty());
    }

    #[test]
    fn preserves_authors_titles_and_identifiers() {
        let output = parse(SAMPLE);
        let article = &output.sources[0];

        assert_eq!(article.entry_type, "article");
        assert_eq!(article.citation_key, "mucai2024");
        assert_eq!(
            article.title.as_deref(),
            Some("Wind resource assessment for Angola: a coastal study")
        );
        assert_eq!(article.authors, vec!["Mucai, Ana", "Silva, João P."]);
        assert_eq!(article.year, Some(2024));
        assert_eq!(article.doi.as_deref(), Some("10.1016/j.renene.2024.01.001"));
        assert_eq!(article.container_title.as_deref(), Some("Renewable Energy"));
    }

    #[test]
    fn handles_bare_values_and_booktitle() {
        let output = parse(SAMPLE);
        let paper = &output.sources[1];
        assert_eq!(paper.year, Some(2023));
        assert_eq!(
            paper.container_title.as_deref(),
            Some("Proceedings of the Angolan Energy Conference")
        );
    }

    #[test]
    fn an_at_sign_inside_a_field_does_not_split_the_entry() {
        let input = r#"@misc{k, title = {Contact us at a@b.org}, year = {2020}}"#;
        let output = parse(input);
        assert_eq!(output.sources.len(), 1);
        assert_eq!(output.sources[0].title.as_deref(), Some("Contact us at a@b.org"));
    }

    #[test]
    fn unparseable_entries_are_reported_not_dropped() {
        let output = parse("@{no key here}");
        assert!(output.sources.is_empty());
        assert_eq!(output.skipped.len(), 1);
    }

    #[test]
    fn raw_fields_are_kept_for_provenance() {
        let output = parse(SAMPLE);
        assert!(output.sources[0].raw_fields.contains_key("journal"));
    }

    /// A normalização é uma forma, e não uma correcção.
    #[test]
    fn a_normalizacao_da_a_mesma_forma_a_bibliografias_diferentes() {
        let desalinhado = "@ARTICLE{k,TITLE={T},author={A},YEAR={2024}}";
        let arrumado = "@article{k,\n  author = {A},\n  title = {T},\n  year = {2024}\n}";

        let um = parse(desalinhado).normalized;
        let outro = parse(arrumado).normalized;

        assert_eq!(um, outro, "a mesma bibliografia deu duas formas");
        assert!(um.starts_with("@article{k,"), "{um}");
        assert!(um.contains("author = {A}"), "{um}");
    }

    /// Entradas que não se leram não aparecem normalizadas.
    ///
    /// Normalizar é dar forma ao que se leu. Inventar uma forma para o que não
    /// se conseguiu ler seria apresentar como arrumado aquilo que ninguém
    /// entendeu.
    #[test]
    fn o_que_nao_se_leu_nao_entra_na_normalizacao() {
        let saida = parse("@article{bom, title = {T}}\n@misc{partido");
        assert_eq!(saida.sources.len(), 1);
        assert_eq!(saida.skipped.len(), 1);
        assert!(saida.normalized.contains("bom"));
        assert!(
            !saida.normalized.contains("partido"),
            "uma entrada por ler apareceu normalizada: {}",
            saida.normalized
        );
    }

    /// A mesma entrada dá sempre a mesma saída.
    #[test]
    fn a_normalizacao_e_determinista() {
        let primeira = parse(SAMPLE).normalized;
        for _ in 0..5 {
            assert_eq!(parse(SAMPLE).normalized, primeira);
        }
    }
}
