//! Sanitisation of received email.
//!
//! # This is a trust boundary
//!
//! Email HTML is written by whoever sent the message. It arrives with scripts,
//! event handlers, tracking pixels, `javascript:` links and SVG that executes.
//! Rendering it as received would put arbitrary attacker-controlled markup
//! inside a session that holds institutional research (briefing §12).
//!
//! # What this does
//!
//! Allow-list sanitisation through `ammonia`, which parses with the same HTML5
//! parser browsers use and rebuilds the tree from what is permitted. An
//! allow-list is the only defensible shape here: a deny-list is a promise to
//! have thought of every attack, and nobody has.
//!
//! On top of that:
//!
//! - **remote content is rewritten, not merely blocked** — the URL is moved to
//!   a `data-oc-remote` attribute so the interface can offer to load it, and
//!   `src` is removed so nothing is fetched meanwhile;
//! - **links are forced through `rel="noopener noreferrer nofollow"`** and open
//!   in a new context;
//! - **the external domain is attached** to every link, so the interface can
//!   show where it really goes (briefing §69).
//!
//! # What this does not do
//!
//! Anti-phishing analysis, or anti-malware. Those are separate problems and
//! pretending otherwise would be worse than not claiming them.

use std::collections::{HashMap, HashSet};

use ammonia::Builder;

/// The result of cleaning one message body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SanitizedBody {
    /// HTML safe to render.
    pub html: String,
    /// How many remote references were neutralised.
    ///
    /// Shown to the member so the offer to load them is honest about what it
    /// would fetch.
    pub blocked_remote_count: usize,
    /// The external domains this message links to, de-duplicated and sorted.
    ///
    /// Lets the interface say where a message points without the member having
    /// to hover every link.
    pub linked_domains: Vec<String>,
    /// How many images live inside the message itself.
    ///
    /// Counted apart from remote ones: an inline image fetches nothing, and
    /// offering to "load remote content" for it would be wrong.
    pub inline_image_count: usize,
}

/// Tags permitted in a message body.
///
/// Deliberately narrow: the formatting an email actually needs, and nothing
/// that positions, loads or executes.
///
/// Notably absent: `script`, `iframe`, `object`, `embed`, `svg`, `math`,
/// `form`, `input`, `button`, `style`, `link`, `meta`, `base`.
const ALLOWED_TAGS: &[&str] = &[
    "p",
    "br",
    "hr",
    "div",
    "span",
    "blockquote",
    "pre",
    "code",
    "b",
    "strong",
    "i",
    "em",
    "u",
    "s",
    "sub",
    "sup",
    "small",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "ul",
    "ol",
    "li",
    "dl",
    "dt",
    "dd",
    "table",
    "thead",
    "tbody",
    "tfoot",
    "tr",
    "td",
    "th",
    "caption",
    "a",
    "img",
];

/// URL schemes permitted on a link.
///
/// No `javascript:`, no `data:`, no `vbscript:`, no `file:`.
/// `cid` is included so an inline reference survives the parser. It fetches
/// nothing from the network — it names a part of this same message — and is
/// rewritten below so the interface can show a placeholder until the part is
/// served.
const ALLOWED_SCHEMES: &[&str] = &["http", "https", "mailto", "tel", "cid"];

/// Sanitise a message body.
///
/// `allow_remote` decides whether images keep their `src`. The default
/// everywhere is `false`, and callers pass `true` only after a member has
/// explicitly asked for this message.
#[must_use]
pub fn sanitize_html(raw: &str, allow_remote: bool) -> SanitizedBody {
    // First pass: ammonia rebuilds the tree from the allow-list. Everything
    // that follows operates on markup that is already structurally safe.
    let mut tags: HashSet<&str> = ALLOWED_TAGS.iter().copied().collect();
    if !allow_remote {
        // With remote content blocked the `img` survives so the placeholder can
        // be shown; its `src` is stripped below.
        tags.insert("img");
    }

    let mut attributes: HashMap<&str, HashSet<&str>> = HashMap::new();
    attributes.insert("a", ["href", "title"].into_iter().collect());
    attributes.insert(
        "img",
        ["src", "alt", "title", "width", "height"]
            .into_iter()
            .collect(),
    );
    attributes.insert("td", ["colspan", "rowspan"].into_iter().collect());
    attributes.insert("th", ["colspan", "rowspan"].into_iter().collect());

    let cleaned = Builder::default()
        .tags(tags)
        .generic_attributes(HashSet::new())
        .tag_attributes(attributes)
        .url_schemes(ALLOWED_SCHEMES.iter().copied().collect())
        // Every link is opened in a new context, with the referrer withheld and
        // the opener severed.
        .link_rel(Some("noopener noreferrer nofollow"))
        .clean(raw)
        .to_string();

    // Second pass: neutralise remote references and collect link domains.
    // Inline references are rewritten in both cases: allowing remote content is
    // a decision about the network, not about parts of this message that the
    // Ocinye does not yet serve.
    let (html, blocked_remote_count, inline_image_count) = if allow_remote {
        let (html, _, inline) = block_remote_selectively(&cleaned, true);
        (html, 0, inline)
    } else {
        block_remote(&cleaned)
    };

    SanitizedBody {
        linked_domains: link_domains(&html),
        html,
        blocked_remote_count,
        inline_image_count,
    }
}

/// Rewrite inline references while leaving remote ones in place.
fn block_remote_selectively(html: &str, keep_remote: bool) -> (String, usize, usize) {
    if !keep_remote {
        return block_remote(html);
    }

    let mut out = String::with_capacity(html.len());
    let mut inline = 0;
    let mut rest = html;

    while let Some(position) = rest.find(" src=\"cid:") {
        let (before, after) = rest.split_at(position);
        out.push_str(before);

        let value_start = &after[" src=\"".len()..];
        let Some(end) = value_start.find('"') else {
            break;
        };

        out.push_str(" data-oc-inline=\"");
        out.push_str(&value_start[..end]);
        out.push('"');
        inline += 1;

        rest = &value_start[end + 1..];
    }

    out.push_str(rest);
    (out, 0, inline)
}

/// Move every `src` to `data-oc-remote` so nothing is fetched.
///
/// Rewriting rather than deleting means the interface can offer to load the
/// images afterwards without going back to the provider.
fn block_remote(html: &str) -> (String, usize, usize) {
    let mut out = String::with_capacity(html.len());
    let mut blocked = 0;
    let mut inline = 0;
    let mut rest = html;

    while let Some(position) = rest.find(" src=\"") {
        let (before, after) = rest.split_at(position);
        out.push_str(before);

        let value_start = &after[" src=\"".len()..];
        let Some(end) = value_start.find('"') else {
            // Malformed beyond the parser's repair: drop the remainder rather
            // than emit half an attribute.
            break;
        };

        let url = &value_start[..end];
        // A `cid:` reference points inside the message itself and fetches
        // nothing from the network. It is still rewritten: the part is not
        // served yet, and leaving the `src` in place would render a broken
        // image where the interface can instead say what it is.
        if url.starts_with("cid:") {
            out.push_str(" data-oc-inline=\"");
            out.push_str(url);
            out.push('"');
            inline += 1;
        } else {
            out.push_str(" data-oc-remote=\"");
            out.push_str(url);
            out.push('"');
            blocked += 1;
        }

        rest = &value_start[end + 1..];
    }

    out.push_str(rest);
    (out, blocked, inline)
}

/// The external domains a body links to.
fn link_domains(html: &str) -> Vec<String> {
    let mut domains: Vec<String> = Vec::new();
    let mut rest = html;

    while let Some(position) = rest.find("href=\"") {
        let after = &rest[position + "href=\"".len()..];
        let Some(end) = after.find('"') else { break };
        let url = &after[..end];

        if let Some(domain) = domain_of(url) {
            if !domains.contains(&domain) {
                domains.push(domain);
            }
        }
        rest = &after[end..];
    }

    domains.sort_unstable();
    domains
}

/// The host of an `http`/`https` URL.
fn domain_of(url: &str) -> Option<String> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let host = rest
        .split(['/', '?', '#'])
        .next()?
        .rsplit('@')
        .next()?
        .split(':')
        .next()?;
    (!host.is_empty()).then(|| host.to_lowercase())
}

/// Turn a plain-text body into safe HTML.
///
/// Escaped and wrapped, so a message that is plain text renders in the same
/// component as one that is HTML — without ever being parsed as markup.
#[must_use]
pub fn text_to_html(text: &str) -> String {
    let escaped = text
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;");

    format!("<pre class=\"oc-mail__plain\">{escaped}</pre>")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clean(raw: &str) -> String {
        sanitize_html(raw, false).html
    }

    #[test]
    fn scripts_do_not_survive_in_any_form() {
        for attack in [
            r#"<script>alert(1)</script>"#,
            r#"<SCRIPT>alert(1)</SCRIPT>"#,
            r#"<scr<script>ipt>alert(1)</script>"#,
            r#"<img src=x onerror="alert(1)">"#,
            r#"<div onclick="alert(1)">texto</div>"#,
            r#"<body onload="alert(1)">"#,
            r#"<svg/onload=alert(1)>"#,
            r#"<iframe src="https://mau.example"></iframe>"#,
            r#"<object data="x"></object>"#,
            r#"<embed src="x">"#,
            r#"<form action="https://mau.example"><input name="p"></form>"#,
        ] {
            let html = clean(attack);
            let lowered = html.to_lowercase();

            // O que importa é que nada **executa**: nenhum elemento de script,
            // nenhum handler de evento, nenhum elemento que carregue algo.
            //
            // Texto inerte que por acaso se leia «alert(1)» é inofensivo, e
            // proibi-lo seria testar a aparência em vez da segurança — foi o
            // que este teste fazia antes, e recusava `ipt&gt;alert(1)`, que é
            // texto escapado.
            for banned in [
                "<script",
                "onerror=",
                "onclick=",
                "onload=",
                "<iframe",
                "<object",
                "<embed",
                "<form",
                "<input",
                "javascript:",
            ] {
                assert!(
                    !lowered.contains(banned),
                    "«{banned}» sobreviveu a {attack:?} → {html}"
                );
            }
        }
    }

    #[test]
    fn dangerous_url_schemes_are_removed() {
        for attack in [
            r#"<a href="javascript:alert(1)">clique</a>"#,
            r#"<a href="JaVaScRiPt:alert(1)">clique</a>"#,
            r#"<a href="data:text/html;base64,PHNjcmlwdD4=">clique</a>"#,
            r#"<a href="vbscript:msgbox(1)">clique</a>"#,
            r#"<a href="file:///etc/passwd">clique</a>"#,
        ] {
            let html = clean(attack).to_lowercase();
            for banned in ["javascript:", "data:text/html", "vbscript:", "file://"] {
                assert!(!html.contains(banned), "«{banned}» sobreviveu: {html}");
            }
            // O texto do link permanece: sanear não é apagar a mensagem.
            assert!(html.contains("clique"));
        }
    }

    #[test]
    fn style_and_css_abuse_do_not_survive() {
        for attack in [
            r#"<style>body{background:url(https://rastreio.example/p.gif)}</style>"#,
            r#"<div style="background:url(https://rastreio.example/p.gif)">x</div>"#,
            r#"<link rel="stylesheet" href="https://mau.example/a.css">"#,
        ] {
            let html = clean(attack).to_lowercase();
            assert!(!html.contains("<style"), "{html}");
            assert!(!html.contains("<link"), "{html}");
            assert!(
                !html.contains("style="),
                "atributo style sobreviveu: {html}"
            );
        }
    }

    #[test]
    fn a_tracking_pixel_is_neutralised_and_counted() {
        let result = sanitize_html(
            r#"<p>Olá</p><img src="https://rastreio.example/p.gif?u=123" width="1" height="1">"#,
            false,
        );

        assert_eq!(result.blocked_remote_count, 1);
        assert!(!result.html.contains("src=\"https://rastreio.example"));
        assert!(result
            .html
            .contains("data-oc-remote=\"https://rastreio.example"));
        assert!(result.html.contains("Olá"));
    }

    #[test]
    fn inline_content_is_not_treated_as_remote() {
        // `cid:` aponta para dentro da própria mensagem e não busca nada.
        let result = sanitize_html(r#"<img src="cid:parte1@exemplo">"#, false);

        // Não conta como remoto, porque nada vai à rede.
        assert_eq!(result.blocked_remote_count, 0);
        assert_eq!(result.inline_image_count, 1);
        // Reescrito: a parte ainda não é servida, e um `src` intacto renderia
        // uma imagem partida onde a interface pode dizer o que é.
        assert!(result
            .html
            .contains("data-oc-inline=\"cid:parte1@exemplo\""));
        assert!(!result.html.contains("src=\"cid:"));
    }

    #[test]
    fn remote_content_loads_only_when_asked() {
        let raw = r#"<img src="https://exemplo.com/foto.png">"#;

        let blocked = sanitize_html(raw, false);
        assert_eq!(blocked.blocked_remote_count, 1);
        assert!(!blocked.html.contains("src=\"https://exemplo.com"));

        let allowed = sanitize_html(raw, true);
        assert_eq!(allowed.blocked_remote_count, 0);
        assert!(allowed
            .html
            .contains("src=\"https://exemplo.com/foto.png\""));
    }

    #[test]
    fn links_carry_noopener_noreferrer_and_nofollow() {
        let html = clean(r#"<a href="https://exemplo.com/a">ligação</a>"#);
        assert!(html.contains("noopener"), "{html}");
        assert!(html.contains("noreferrer"), "{html}");
        assert!(html.contains("nofollow"), "{html}");
    }

    #[test]
    fn the_domains_a_message_links_to_are_collected() {
        let result = sanitize_html(
            r#"<a href="https://Exemplo.COM/a">a</a>
               <a href="https://exemplo.com/b">b</a>
               <a href="http://outro.example:8080/c?x=1">c</a>
               <a href="mailto:x@y.z">d</a>"#,
            false,
        );

        // Minúsculas, deduplicado, ordenado; `mailto:` não é um domínio de
        // navegação e não entra.
        assert_eq!(result.linked_domains, vec!["exemplo.com", "outro.example"]);
    }

    #[test]
    fn a_userinfo_url_does_not_disguise_its_host() {
        // `https://banco.pt@atacante.example/` vai para `atacante.example`.
        let result = sanitize_html(
            r#"<a href="https://banco.pt@atacante.example/login">entrar</a>"#,
            false,
        );
        assert_eq!(result.linked_domains, vec!["atacante.example"]);
    }

    #[test]
    fn ordinary_formatting_survives_intact() {
        let html = clean(
            r#"<p>Caro <strong>Carlos</strong>,</p>
               <p>Segue a <em>análise</em>:</p>
               <ul><li>Primeiro</li><li>Segundo</li></ul>
               <blockquote>Citação anterior</blockquote>
               <table><tr><th>A</th><td>1</td></tr></table>"#,
        );

        for kept in [
            "<strong>",
            "<em>",
            "<ul>",
            "<li>",
            "<blockquote>",
            "<table>",
            "Carlos",
        ] {
            assert!(html.contains(kept), "«{kept}» perdeu-se: {html}");
        }
    }

    #[test]
    fn malformed_html_does_not_panic_and_produces_something_safe() {
        for attack in [
            "<p><p><p><div><span>",
            "<<<<>>>>",
            "<a href=\"",
            "<img src=\"",
            &"<div>".repeat(500),
            "\u{0}\u{1}\u{2}",
        ] {
            let result = sanitize_html(attack, false);
            assert!(!result.html.to_lowercase().contains("<script"));
        }
    }

    #[test]
    fn plain_text_is_escaped_and_never_parsed_as_markup() {
        let html = text_to_html("<script>alert(1)</script> & \"aspas\"");
        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;script&gt;"));
        assert!(html.contains("&amp;"));
        assert!(html.contains("&quot;"));
    }

    #[test]
    fn an_empty_body_is_handled() {
        let result = sanitize_html("", false);
        assert_eq!(result.blocked_remote_count, 0);
        assert!(result.linked_domains.is_empty());
    }
}
