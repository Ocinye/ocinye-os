//! Renderização de Markdown para as respostas do Prompt Ocinye.
//!
//! # Porque existe
//!
//! Uma resposta do Prompt é **conteúdo**: prosa, títulos, listas, código,
//! tabelas, ligações. Para o membro a ler como um documento — e não como texto
//! de `textarea` — é preciso estrutura. O modelo (quando existir) produz
//! Markdown; a resposta de sistema de hoje é prosa simples, que o Markdown
//! renderiza como um parágrafo.
//!
//! # A fronteira de confiança
//!
//! O conteúdo vem de fora do Ocinye Workspace — do Core, e um dia de um modelo.
//! **Nunca** é marcação de confiança. Por isso não se usa `push_html` do
//! `pulldown-cmark`, que deixaria passar HTML em bruto: percorre-se o fluxo de
//! eventos do parser e emite-se apenas a árvore que este módulo autoriza. Todo o
//! texto é escapado; HTML em bruto (`<script>`, `<img onerror=…>`) é tratado
//! como texto e escapado também; ligações só sobrevivem com esquema
//! `http`, `https` ou `mailto`; imagens remotas não são carregadas — mostra-se o
//! texto alternativo. É o único sítio da Experience que produz marcação a partir
//! de conteúdo alheio, e fá-lo por construção fechada (ver o portão
//! `conteudo_do_dominio_nunca_vira_marcacao`).

use pulldown_cmark::{Alignment, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

/// Escapa um texto para poder viver dentro de um nó de texto ou de um atributo.
fn escape(input: &str, out: &mut String) {
    for ch in input.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
}

/// Uma ligação só sobrevive com um esquema que não executa nada.
///
/// `javascript:`, `data:` e `vbscript:` são vectores de execução; um endereço
/// relativo (sem esquema) é do próprio Workspace e é seguro. Tudo o resto é
/// recusado, e a ligação torna-se texto.
fn safe_href(url: &str) -> Option<String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return None;
    }
    // Um esquema é `algo:` antes de qualquer `/`, `?` ou `#`. Sem esquema, é
    // relativo — do Workspace — e passa.
    let scheme_end = trimmed.find([':', '/', '?', '#']);
    if let Some(idx) = scheme_end {
        if trimmed.as_bytes()[idx] == b':' {
            let scheme = trimmed[..idx].to_ascii_lowercase();
            if !matches!(scheme.as_str(), "http" | "https" | "mailto") {
                return None;
            }
        }
    }
    let mut out = String::new();
    escape(trimmed, &mut out);
    Some(out)
}

/// O nível de título emitido, sempre um abaixo do que o Markdown pede.
///
/// A página já tem o seu `h1`; um `#` do modelo torna-se `h2`, e a hierarquia da
/// resposta vive por baixo da da página — nunca a disputa.
fn heading_tag(level: HeadingLevel) -> &'static str {
    match level {
        HeadingLevel::H1 => "h2",
        HeadingLevel::H2 => "h3",
        HeadingLevel::H3 => "h4",
        _ => "h5",
    }
}

/// Renderiza Markdown como HTML seguro, pronto para `inner_html`.
///
/// O resultado é um conjunto fechado de etiquetas com todo o texto escapado.
#[must_use]
pub fn render(source: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);

    let parser = Parser::new_ext(source, options);
    let mut out = String::new();

    // Estado do bloco de código: acumula-se o texto e a linguagem para o emitir
    // com barra (rótulo + copiar) no fim.
    let mut code_buf = String::new();
    let mut code_lang = String::new();
    let mut in_code = false;

    // Estado da tabela: alinhamentos por coluna, e se se está no cabeçalho.
    let mut aligns: Vec<Alignment> = Vec::new();
    let mut col = 0usize;
    let mut in_head = false;

    // Que etiqueta fecha cada ligação aberta: `</a>` quando o esquema é seguro,
    // `</span>` quando a ligação foi recusada e ficou texto neutro.
    let mut link_close: Vec<&'static str> = Vec::new();

    for event in parser {
        match event {
            Event::Start(tag) => match tag {
                Tag::Paragraph => out.push_str("<p>"),
                Tag::Heading { level, .. } => {
                    out.push('<');
                    out.push_str(heading_tag(level));
                    out.push('>');
                }
                Tag::BlockQuote(_) => out.push_str("<blockquote>"),
                Tag::CodeBlock(kind) => {
                    in_code = true;
                    code_buf.clear();
                    code_lang.clear();
                    if let CodeBlockKind::Fenced(info) = kind {
                        // A linguagem é a primeira palavra da linha de abertura.
                        let lang = info.split_whitespace().next().unwrap_or("");
                        code_lang.push_str(lang);
                    }
                }
                Tag::List(Some(start)) => {
                    if start == 1 {
                        out.push_str("<ol>");
                    } else {
                        out.push_str("<ol start=\"");
                        out.push_str(&start.to_string());
                        out.push_str("\">");
                    }
                }
                Tag::List(None) => out.push_str("<ul>"),
                Tag::Item => out.push_str("<li>"),
                Tag::Emphasis => out.push_str("<em>"),
                Tag::Strong => out.push_str("<strong>"),
                Tag::Strikethrough => out.push_str("<del>"),
                Tag::Link {
                    dest_url, title, ..
                } => {
                    if let Some(href) = safe_href(&dest_url) {
                        out.push_str("<a href=\"");
                        out.push_str(&href);
                        out.push_str("\" rel=\"noopener noreferrer nofollow\" target=\"_blank\"");
                        if !title.is_empty() {
                            out.push_str(" title=\"");
                            escape(&title, &mut out);
                            out.push('"');
                        }
                        out.push('>');
                        link_close.push("</a>");
                    } else {
                        // Uma ligação sem esquema seguro não é ligação: fica o
                        // texto, dentro de um `span` que a marca como neutra.
                        out.push_str("<span class=\"oc-md-deadlink\">");
                        link_close.push("</span>");
                    }
                }
                Tag::Image { .. } => {
                    // Conteúdo remoto não se carrega a partir de uma resposta: o
                    // texto alternativo entra, a imagem não (mesma postura do
                    // correio). Os eventos de texto seguintes são o `alt`.
                    out.push_str("<span class=\"oc-md-img\">[imagem: ");
                }
                Tag::Table(alignments) => {
                    aligns = alignments;
                    out.push_str("<div class=\"oc-md-table\"><table>");
                }
                Tag::TableHead => {
                    in_head = true;
                    col = 0;
                    out.push_str("<thead><tr>");
                }
                Tag::TableRow => {
                    col = 0;
                    out.push_str("<tr>");
                }
                Tag::TableCell => {
                    let cell = if in_head { "th" } else { "td" };
                    let class = match aligns.get(col) {
                        Some(Alignment::Center) => " class=\"oc-md-c\"",
                        Some(Alignment::Right) => " class=\"oc-md-r\"",
                        _ => "",
                    };
                    out.push('<');
                    out.push_str(cell);
                    out.push_str(class);
                    out.push('>');
                }
                // Notas de rodapé e listas de tarefas não fazem parte do
                // vocabulário de uma resposta; ignoram-se em silêncio.
                _ => {}
            },
            Event::End(tag) => match tag {
                TagEnd::Paragraph => out.push_str("</p>"),
                TagEnd::Heading(level) => {
                    out.push_str("</");
                    out.push_str(heading_tag(level));
                    out.push('>');
                }
                TagEnd::BlockQuote(_) => out.push_str("</blockquote>"),
                TagEnd::CodeBlock => {
                    in_code = false;
                    out.push_str("<div class=\"oc-md-code\"><div class=\"oc-md-code__bar\">");
                    out.push_str("<span class=\"oc-md-code__lang\">");
                    if code_lang.is_empty() {
                        out.push_str("texto");
                    } else {
                        escape(&code_lang, &mut out);
                    }
                    out.push_str(
                        "</span><button type=\"button\" class=\"oc-md-code__copy\" \
                                  data-oc=\"copiar-codigo\">Copiar</button></div><pre><code>",
                    );
                    escape(&code_buf, &mut out);
                    out.push_str("</code></pre></div>");
                }
                TagEnd::List(true) => out.push_str("</ol>"),
                TagEnd::List(false) => out.push_str("</ul>"),
                TagEnd::Item => out.push_str("</li>"),
                TagEnd::Emphasis => out.push_str("</em>"),
                TagEnd::Strong => out.push_str("</strong>"),
                TagEnd::Strikethrough => out.push_str("</del>"),
                TagEnd::Link => out.push_str(link_close.pop().unwrap_or("</a>")),
                TagEnd::Image => out.push_str("]</span>"),
                TagEnd::Table => out.push_str("</table></div>"),
                TagEnd::TableHead => {
                    in_head = false;
                    out.push_str("</tr></thead><tbody>");
                }
                TagEnd::TableRow => out.push_str("</tr>"),
                TagEnd::TableCell => {
                    out.push_str(if in_head { "</th>" } else { "</td>" });
                    col += 1;
                }
                _ => {}
            },
            Event::Text(text) => {
                if in_code {
                    code_buf.push_str(&text);
                } else {
                    escape(&text, &mut out);
                }
            }
            Event::Code(code) => {
                out.push_str("<code class=\"oc-md-inline\">");
                escape(&code, &mut out);
                out.push_str("</code>");
            }
            // HTML em bruto nunca é injectado: é texto, e escapa-se como texto.
            Event::Html(html) | Event::InlineHtml(html) => escape(&html, &mut out),
            Event::SoftBreak => out.push(' '),
            Event::HardBreak => out.push_str("<br>"),
            Event::Rule => out.push_str("<hr>"),
            _ => {}
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_prosa_simples_vira_um_paragrafo() {
        let html = render("Nenhuma capacidade de inferência está disponível.");
        assert_eq!(
            html,
            "<p>Nenhuma capacidade de inferência está disponível.</p>"
        );
    }

    #[test]
    fn html_em_bruto_e_escapado_e_nunca_injectado() {
        let html = render("Olá <script>alert('x')</script> <img src=x onerror=y>");
        assert!(!html.contains("<script>"));
        assert!(!html.contains("<img"));
        assert!(html.contains("&lt;script&gt;"));
    }

    #[test]
    fn uma_ligacao_javascript_nao_sobrevive() {
        let html = render("[carrega](javascript:alert(1))");
        assert!(!html.contains("javascript:"));
        assert!(!html.contains("<a "));
        assert!(html.contains("oc-md-deadlink"));
    }

    #[test]
    fn uma_ligacao_https_sobrevive_com_atributos_seguros() {
        let html = render("[Ocinye](https://ocinye.com)");
        assert!(html.contains("href=\"https://ocinye.com\""));
        assert!(html.contains("rel=\"noopener noreferrer nofollow\""));
        assert!(html.contains("target=\"_blank\""));
    }

    #[test]
    fn um_bloco_de_codigo_traz_lingua_e_copiar() {
        let html = render("```rust\nfn main() {}\n```");
        assert!(html.contains("oc-md-code__lang"));
        assert!(html.contains(">rust<"));
        assert!(html.contains("data-oc=\"copiar-codigo\""));
        assert!(html.contains("fn main() {}"));
    }

    #[test]
    fn titulos_descem_um_nivel_para_nao_disputar_o_h1_da_pagina() {
        let html = render("# Título");
        assert!(html.contains("<h2>Título</h2>"));
        assert!(!html.contains("<h1>"));
    }

    #[test]
    fn as_listas_e_a_enfase_sao_estruturais() {
        let html = render("- **um**\n- *dois*");
        assert!(html.contains("<ul><li><strong>um</strong></li><li><em>dois</em></li></ul>"));
    }

    #[test]
    fn uma_tabela_e_envolvida_para_deslizar() {
        let html = render("| a | b |\n|---|---|\n| 1 | 2 |");
        assert!(html.contains("oc-md-table"));
        assert!(html.contains("<th>a</th>"));
        assert!(html.contains("<td>1</td>"));
    }

    #[test]
    fn codigo_em_linha_e_marcado() {
        let html = render("usa `cargo test` agora");
        assert!(html.contains("<code class=\"oc-md-inline\">cargo test</code>"));
    }
}
