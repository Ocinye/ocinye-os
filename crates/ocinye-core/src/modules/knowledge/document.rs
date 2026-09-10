//! O documento estruturado de uma nota — a fonte de verdade.
//!
//! # Porque estruturado, e não HTML
//!
//! Uma nota rica guarda-se como um modelo de blocos tipado, versionado por
//! `schema_version`, e não como HTML higienizado (ADR-0413 §2). Os blocos têm
//! semântica — uma *checklist* é uma *checklist*, um bloco de código mantém a
//! sua linguagem, uma imagem aponta a uma `FileVersion` exacta — e uma revisão
//! histórica nunca depende da interpretação de HTML arbitrário.
//!
//! # Isto é a fronteira de confiança
//!
//! O documento chega do cliente, e o cliente não é a autoridade sobre o que se
//! guarda. [`NoteDocument::from_value`] só aceita o que o esquema conhece: tipos
//! desconhecidos, campos a mais, níveis de título fora do intervalo, ligações
//! com esquemas perigosos (`javascript:`, `data:`) e documentos grandes de mais
//! são recusados. O que passa é seguro por construção, e é dele que o HTML e o
//! texto simples se **derivam** — nunca o contrário.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{CoreError, CoreResult};

/// A versão do esquema com que este build escreve.
pub const SCHEMA_VERSION: u32 = 1;

/// Limites que impedem um documento hostil ou absurdo de entrar.
const MAX_BLOCKS: usize = 5_000;
const MAX_TEXT_BYTES: usize = 1_000_000;
const MAX_HEADING_LEVEL: u8 = 3;
const ALLOWED_LINK_SCHEMES: &[&str] = &["http://", "https://", "mailto:", "tel:"];

/// O documento canónico de uma nota.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NoteDocument {
    /// A versão do esquema com que foi escrito.
    pub schema_version: u32,
    /// Os blocos, pela ordem em que aparecem.
    #[serde(default)]
    pub blocks: Vec<Block>,
}

/// Um bloco do documento.
///
/// `image`, `attachment` e `table` são reservados para as fatias seguintes: o
/// esquema conhece-os para que entrem sem redesenho, e o editor da fatia A não
/// os produz.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Block {
    /// Um parágrafo.
    Paragraph {
        /// O texto em linha.
        #[serde(default)]
        content: Vec<Inline>,
    },
    /// Um título de nível 1 a 3.
    Heading {
        /// O nível.
        level: u8,
        /// O texto em linha.
        #[serde(default)]
        content: Vec<Inline>,
    },
    /// Uma lista com marcadores.
    BulletList {
        /// Cada item é uma linha de texto.
        #[serde(default)]
        items: Vec<Vec<Inline>>,
    },
    /// Uma lista numerada.
    OrderedList {
        /// Cada item é uma linha de texto.
        #[serde(default)]
        items: Vec<Vec<Inline>>,
    },
    /// Uma lista de tarefas.
    Checklist {
        /// Os itens, cada um com o seu estado.
        #[serde(default)]
        items: Vec<ChecklistItem>,
    },
    /// Um bloco de código, com a linguagem opcional.
    CodeBlock {
        /// A linguagem, quando declarada.
        #[serde(default)]
        language: Option<String>,
        /// O código, como texto cru.
        #[serde(default)]
        text: String,
    },
    /// Uma imagem, pela versão de ficheiro exacta. (Fatia B.)
    Image {
        /// A `FileVersion` que guarda os bytes.
        file_version_id: Uuid,
        /// O texto alternativo.
        #[serde(default)]
        alt: String,
    },
    /// Um anexo, pela versão de ficheiro exacta. (Fatia B.)
    Attachment {
        /// A `FileVersion` que guarda os bytes.
        file_version_id: Uuid,
        /// O nome a mostrar.
        #[serde(default)]
        name: String,
    },
}

/// Um item de uma *checklist*.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChecklistItem {
    /// Se está concluído.
    #[serde(default)]
    pub checked: bool,
    /// O texto do item.
    #[serde(default)]
    pub content: Vec<Inline>,
}

/// Um trecho de texto em linha.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Inline {
    /// Texto simples, com marcas.
    Text {
        /// O texto.
        text: String,
        /// As marcas aplicadas.
        #[serde(default)]
        marks: Vec<Mark>,
    },
    /// Uma ligação.
    Link {
        /// O destino.
        href: String,
        /// O texto mostrado.
        text: String,
        /// As marcas aplicadas.
        #[serde(default)]
        marks: Vec<Mark>,
    },
}

/// Uma marca de formatação de texto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mark {
    /// Negrito.
    Bold,
    /// Itálico.
    Italic,
    /// Código em linha.
    Code,
}

impl NoteDocument {
    /// Um documento vazio, com um parágrafo — o estado de uma nota nova.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            blocks: vec![Block::Paragraph {
                content: Vec::new(),
            }],
        }
    }

    /// Lê e **valida** um documento vindo do cliente.
    ///
    /// É a fronteira de confiança: o que não couber no esquema não passa.
    ///
    /// # Errors
    ///
    /// [`CoreError::Validation`] quando a versão do esquema não é a deste build,
    /// quando o documento excede os limites, quando um título tem um nível fora
    /// do intervalo, ou quando uma ligação usa um esquema não permitido. A
    /// desserialização recusa por si tipos e campos que o esquema não conhece.
    pub fn from_value(value: serde_json::Value) -> CoreResult<Self> {
        let doc: Self = serde_json::from_value(value).map_err(|erro| {
            CoreError::Validation(format!("O documento da nota não é válido: {erro}."))
        })?;

        if doc.schema_version != SCHEMA_VERSION {
            return Err(CoreError::Validation(format!(
                "A nota foi escrita com a versão de esquema {}, e este servidor usa a {SCHEMA_VERSION}.",
                doc.schema_version
            )));
        }
        if doc.blocks.len() > MAX_BLOCKS {
            return Err(CoreError::Validation(
                "A nota tem blocos a mais.".to_owned(),
            ));
        }

        let mut bytes = 0_usize;
        for bloco in &doc.blocks {
            bloco.validate(&mut bytes)?;
        }
        if bytes > MAX_TEXT_BYTES {
            return Err(CoreError::Validation("A nota é grande de mais.".to_owned()));
        }

        Ok(doc)
    }

    /// A projecção de texto simples — para o excerto e para o índice de pesquisa.
    #[must_use]
    pub fn plain_text(&self) -> String {
        let mut saida = String::new();
        for bloco in &self.blocks {
            bloco.plain_text(&mut saida);
        }
        saida.trim().to_owned()
    }

    /// O HTML derivado, seguro por construção.
    ///
    /// Todo o texto é escapado, e só se emitem elementos conhecidos. Uma ligação
    /// já foi validada na entrada; aqui herda `rel="noopener noreferrer nofollow"`.
    #[must_use]
    pub fn to_html(&self) -> String {
        let mut saida = String::new();
        for bloco in &self.blocks {
            bloco.to_html(&mut saida);
        }
        saida
    }
}

impl Block {
    fn validate(&self, bytes: &mut usize) -> CoreResult<()> {
        match self {
            Self::Paragraph { content } => validate_inlines(content, bytes)?,
            Self::Heading { level, content } => {
                if *level < 1 || *level > MAX_HEADING_LEVEL {
                    return Err(CoreError::Validation(format!(
                        "Um título tem de ser de nível 1 a {MAX_HEADING_LEVEL}."
                    )));
                }
                validate_inlines(content, bytes)?;
            }
            Self::BulletList { items } | Self::OrderedList { items } => {
                for item in items {
                    validate_inlines(item, bytes)?;
                }
            }
            Self::Checklist { items } => {
                for item in items {
                    validate_inlines(&item.content, bytes)?;
                }
            }
            Self::CodeBlock { text, .. } => *bytes += text.len(),
            Self::Image { alt, .. } => *bytes += alt.len(),
            Self::Attachment { name, .. } => *bytes += name.len(),
        }
        Ok(())
    }

    fn plain_text(&self, saida: &mut String) {
        match self {
            Self::Paragraph { content } | Self::Heading { content, .. } => {
                push_inlines_text(content, saida);
                saida.push('\n');
            }
            Self::BulletList { items } | Self::OrderedList { items } => {
                for item in items {
                    push_inlines_text(item, saida);
                    saida.push('\n');
                }
            }
            Self::Checklist { items } => {
                for item in items {
                    push_inlines_text(&item.content, saida);
                    saida.push('\n');
                }
            }
            Self::CodeBlock { text, .. } => {
                saida.push_str(text);
                saida.push('\n');
            }
            Self::Image { alt, .. } => {
                saida.push_str(alt);
                saida.push('\n');
            }
            Self::Attachment { name, .. } => {
                saida.push_str(name);
                saida.push('\n');
            }
        }
    }

    fn to_html(&self, saida: &mut String) {
        match self {
            Self::Paragraph { content } => {
                saida.push_str("<p>");
                inlines_html(content, saida);
                saida.push_str("</p>");
            }
            Self::Heading { level, content } => {
                let n = (*level).clamp(1, MAX_HEADING_LEVEL);
                saida.push_str(&format!("<h{n}>"));
                inlines_html(content, saida);
                saida.push_str(&format!("</h{n}>"));
            }
            Self::BulletList { items } | Self::OrderedList { items } => {
                let tag = if matches!(self, Self::OrderedList { .. }) {
                    "ol"
                } else {
                    "ul"
                };
                saida.push_str(&format!("<{tag}>"));
                for item in items {
                    saida.push_str("<li>");
                    inlines_html(item, saida);
                    saida.push_str("</li>");
                }
                saida.push_str(&format!("</{tag}>"));
            }
            Self::Checklist { items } => {
                saida.push_str("<ul class=\"oc-checklist\">");
                for item in items {
                    let marca = if item.checked { "☑" } else { "☐" };
                    saida.push_str("<li>");
                    saida.push_str(marca);
                    saida.push(' ');
                    inlines_html(&item.content, saida);
                    saida.push_str("</li>");
                }
                saida.push_str("</ul>");
            }
            Self::CodeBlock { language, text } => {
                saida.push_str("<pre><code");
                if let Some(lang) = language {
                    saida.push_str(" data-language=\"");
                    escape_into(lang, saida);
                    saida.push('"');
                }
                saida.push('>');
                escape_into(text, saida);
                saida.push_str("</code></pre>");
            }
            // As imagens e os anexos renderizam-se pelas rotas do Core na fatia B;
            // até lá, o texto alternativo/nome derivado dá conteúdo legível.
            Self::Image { alt, .. } => {
                saida.push_str("<p>");
                escape_into(alt, saida);
                saida.push_str("</p>");
            }
            Self::Attachment { name, .. } => {
                saida.push_str("<p>");
                escape_into(name, saida);
                saida.push_str("</p>");
            }
        }
    }
}

fn validate_inlines(inlines: &[Inline], bytes: &mut usize) -> CoreResult<()> {
    for inline in inlines {
        match inline {
            Inline::Text { text, .. } => *bytes += text.len(),
            Inline::Link { href, text, .. } => {
                *bytes += text.len();
                let baixo = href.to_ascii_lowercase();
                if !ALLOWED_LINK_SCHEMES.iter().any(|s| baixo.starts_with(s)) {
                    return Err(CoreError::Validation(
                        "Uma ligação usa um esquema que não é permitido.".to_owned(),
                    ));
                }
            }
        }
    }
    Ok(())
}

fn push_inlines_text(inlines: &[Inline], saida: &mut String) {
    for inline in inlines {
        match inline {
            Inline::Text { text, .. } | Inline::Link { text, .. } => saida.push_str(text),
        }
    }
}

fn inlines_html(inlines: &[Inline], saida: &mut String) {
    for inline in inlines {
        match inline {
            Inline::Text { text, marks } => {
                let (abre, fecha) = marks_tags(marks);
                saida.push_str(&abre);
                escape_into(text, saida);
                saida.push_str(&fecha);
            }
            Inline::Link { href, text, marks } => {
                // O esquema já foi validado na entrada; aqui escapa-se para o
                // atributo e força-se o `rel` seguro.
                saida.push_str("<a href=\"");
                escape_into(href, saida);
                saida.push_str("\" rel=\"noopener noreferrer nofollow\">");
                let (abre, fecha) = marks_tags(marks);
                saida.push_str(&abre);
                escape_into(text, saida);
                saida.push_str(&fecha);
                saida.push_str("</a>");
            }
        }
    }
}

fn marks_tags(marks: &[Mark]) -> (String, String) {
    let mut abre = String::new();
    let mut fecha = String::new();
    for mark in marks {
        let (a, f) = match mark {
            Mark::Bold => ("<strong>", "</strong>"),
            Mark::Italic => ("<em>", "</em>"),
            Mark::Code => ("<code>", "</code>"),
        };
        abre.push_str(a);
        // Fecha pela ordem inversa.
        fecha.insert_str(0, f);
    }
    (abre, fecha)
}

/// Escapa texto para dentro de HTML — a única defesa de que precisa, porque o
/// modelo é fechado e nunca emite marcação vinda do texto.
fn escape_into(texto: &str, saida: &mut String) {
    for c in texto.chars() {
        match c {
            '&' => saida.push_str("&amp;"),
            '<' => saida.push_str("&lt;"),
            '>' => saida.push_str("&gt;"),
            '"' => saida.push_str("&quot;"),
            '\'' => saida.push_str("&#39;"),
            _ => saida.push(c),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn paragrafo(texto: &str) -> serde_json::Value {
        json!({ "type": "paragraph", "content": [{ "type": "text", "text": texto }] })
    }

    /// Ida e volta: um documento serializa-se, lê-se, e é o mesmo documento.
    #[test]
    fn o_documento_faz_ida_e_volta_sem_perder_semantica() {
        let doc = NoteDocument {
            schema_version: SCHEMA_VERSION,
            blocks: vec![
                Block::Heading {
                    level: 1,
                    content: vec![Inline::Text {
                        text: "Reunião".to_owned(),
                        marks: vec![Mark::Bold],
                    }],
                },
                Block::Checklist {
                    items: vec![
                        ChecklistItem {
                            checked: true,
                            content: vec![Inline::Text {
                                text: "feito".to_owned(),
                                marks: vec![],
                            }],
                        },
                        ChecklistItem {
                            checked: false,
                            content: vec![Inline::Text {
                                text: "por fazer".to_owned(),
                                marks: vec![],
                            }],
                        },
                    ],
                },
            ],
        };
        let valor = serde_json::to_value(&doc).expect("serializa");
        let de_volta = NoteDocument::from_value(valor).expect("lê-se");
        assert_eq!(doc, de_volta, "a ida e volta perdeu semântica");
    }

    /// A projecção de texto simples é o que vai à pesquisa.
    #[test]
    fn o_texto_simples_deriva_do_documento() {
        let doc = NoteDocument::from_value(json!({
            "schema_version": 1,
            "blocks": [paragrafo("Rede de sensores"), paragrafo("consumo em nós remotos")]
        }))
        .expect("válido");
        assert_eq!(doc.plain_text(), "Rede de sensores\nconsumo em nós remotos");
    }

    /// Um bloco que o esquema não conhece não passa a fronteira.
    #[test]
    fn um_bloco_desconhecido_e_recusado() {
        let erro = NoteDocument::from_value(json!({
            "schema_version": 1,
            "blocks": [{ "type": "iframe", "src": "https://mau.example" }]
        }))
        .expect_err("um tipo desconhecido não devia passar");
        assert!(matches!(erro, CoreError::Validation(_)), "veio {erro:?}");
    }

    /// HTML hostil colado, traduzido para o modelo, não sobrevive: o texto
    /// escapa-se, e não há elemento `<script>` nenhum no HTML derivado.
    #[test]
    fn html_hostil_no_texto_escapa_se_e_nao_executa() {
        let doc = NoteDocument::from_value(json!({
            "schema_version": 1,
            "blocks": [paragrafo("</p><script>alert(1)</script>")]
        }))
        .expect("o texto é texto, por mais hostil que pareça");
        let html = doc.to_html();
        assert!(!html.contains("<script>"), "o texto hostil virou marcação: {html}");
        assert!(html.contains("&lt;script&gt;"), "o texto hostil devia aparecer escapado");
    }

    /// Uma ligação `javascript:` é recusada na fronteira.
    #[test]
    fn uma_ligacao_com_esquema_perigoso_e_recusada() {
        let erro = NoteDocument::from_value(json!({
            "schema_version": 1,
            "blocks": [{
                "type": "paragraph",
                "content": [{ "type": "link", "href": "javascript:alert(1)", "text": "carrega" }]
            }]
        }))
        .expect_err("javascript: não devia passar");
        assert!(matches!(erro, CoreError::Validation(_)), "veio {erro:?}");
    }

    /// Um título fora do intervalo é recusado.
    #[test]
    fn um_titulo_de_nivel_absurdo_e_recusado() {
        let erro = NoteDocument::from_value(json!({
            "schema_version": 1,
            "blocks": [{ "type": "heading", "level": 9, "content": [] }]
        }))
        .expect_err("nível 9 não existe");
        assert!(matches!(erro, CoreError::Validation(_)), "veio {erro:?}");
    }

    /// Uma versão de esquema que este build não conhece é recusada, em vez de
    /// ser reinterpretada.
    #[test]
    fn uma_versao_de_esquema_estranha_e_recusada() {
        let erro = NoteDocument::from_value(json!({ "schema_version": 999, "blocks": [] }))
            .expect_err("versão desconhecida");
        assert!(matches!(erro, CoreError::Validation(_)), "veio {erro:?}");
    }
}
