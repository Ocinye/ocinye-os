//! Outbound HTML sanitisation — the trust boundary for member-composed rich text.
//!
//! # A different boundary from inbound
//!
//! [`super::sanitize`] cleans HTML that **arrives** — external, unauthenticated,
//! possibly hostile — for display, and keeps a wide set (images, tables, `cid:`)
//! because the goal is to show received mail faithfully without executing it.
//!
//! This cleans HTML that **leaves**: composed by an authenticated member, to go
//! out in the institution's name (ADR-0415). The goal is to produce a sober
//! message, so the set is narrower — semantic formatting only, no inline styles,
//! no images, no tables, no `cid`. Reusing the inbound sanitiser here would carry
//! the wrong trust assumptions and let through what an outgoing message must not
//! contain.
//!
//! The server is the authority: whatever the client sanitised, this runs again,
//! and its output is what becomes MIME (briefing §13, §49).

use ammonia::Builder;
use std::collections::{HashMap, HashSet};

/// The tags a composed message may keep.
///
/// Semantic formatting and nothing that positions, loads or executes. Absent by
/// design: `script`, `style`, `iframe`, `object`, `embed`, `svg`, `math`,
/// `form`, `input`, `img`, `table` and the rest — an outgoing institutional
/// message needs none of them, and each is a surface.
const OUTBOUND_TAGS: &[&str] = &[
    "p",
    "br",
    "b",
    "strong",
    "i",
    "em",
    "u",
    "s",
    "a",
    "ul",
    "ol",
    "li",
    "blockquote",
    "h1",
    "h2",
    "h3",
    "code",
    "pre",
];

/// URL schemes a composed link may use. No `javascript:`, `data:`, `cid:`.
const OUTBOUND_SCHEMES: &[&str] = &["http", "https", "mailto"];

/// Sanitise member-composed HTML for sending.
///
/// Rebuilds the tree from the allow-list: unknown tags are unwrapped, every
/// attribute except `href` on a link is dropped (so `style`, `class`, `id`,
/// `on*` cannot survive), and only safe URL schemes remain. Formatting is carried
/// by semantic tags, which is what email clients honour anyway.
#[must_use]
pub fn sanitize_outbound(raw: &str) -> String {
    let tags: HashSet<&str> = OUTBOUND_TAGS.iter().copied().collect();

    let mut attributes: HashMap<&str, HashSet<&str>> = HashMap::new();
    attributes.insert("a", ["href"].into_iter().collect());

    let schemes: HashSet<&str> = OUTBOUND_SCHEMES.iter().copied().collect();

    Builder::default()
        .tags(tags)
        .tag_attributes(attributes)
        .generic_attributes(HashSet::new())
        .url_schemes(schemes)
        .url_relative(ammonia::UrlRelative::Deny)
        .clean(raw)
        .to_string()
}

/// Whether sanitised outbound HTML carries any actual content.
///
/// After sanitisation an "empty" body can still be `<p></p>` or whitespace. This
/// says whether there is anything to send, so the send path can fall back to the
/// plain-text route when the member typed nothing but formatting shell.
#[must_use]
pub fn has_visible_content(html: &str) -> bool {
    // Strip tag spans and see whether any text survives. A stricter parse is
    // unnecessary — the caller only needs "did the member write something".
    let mut text = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => text.push(c),
            _ => {}
        }
    }
    !text.trim().is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_scripts_and_event_handlers() {
        let dirty = r#"<p onclick="steal()">olá</p><script>alert(1)</script>"#;
        let clean = sanitize_outbound(dirty);
        assert!(clean.contains("olá"));
        assert!(!clean.contains("script"), "script survived: {clean}");
        assert!(!clean.contains("onclick"), "handler survived: {clean}");
    }

    #[test]
    fn strips_inline_style_but_keeps_semantic_formatting() {
        let dirty = r#"<p style="color:red"><strong>forte</strong> e <em>ênfase</em></p>"#;
        let clean = sanitize_outbound(dirty);
        assert!(clean.contains("<strong>forte</strong>"), "{clean}");
        assert!(clean.contains("<em>ênfase</em>"), "{clean}");
        assert!(!clean.contains("style"), "inline style survived: {clean}");
    }

    #[test]
    fn keeps_safe_links_drops_javascript() {
        let ok = sanitize_outbound(r#"<a href="https://ocinye.com">sítio</a>"#);
        assert!(ok.contains(r#"href="https://ocinye.com""#), "{ok}");

        let bad = sanitize_outbound(r#"<a href="javascript:alert(1)">x</a>"#);
        assert!(!bad.contains("javascript:"), "js scheme survived: {bad}");
    }

    #[test]
    fn drops_images_and_tables() {
        let clean = sanitize_outbound(
            r#"<img src="https://tracker.example/x.png"><table><tr><td>a</td></tr></table><p>corpo</p>"#,
        );
        assert!(!clean.contains("<img"), "image survived: {clean}");
        assert!(!clean.contains("<table"), "table survived: {clean}");
        assert!(clean.contains("corpo"));
    }

    #[test]
    fn keeps_lists_and_blockquote() {
        let clean =
            sanitize_outbound("<ul><li>um</li><li>dois</li></ul><blockquote>cit</blockquote>");
        assert!(
            clean.contains("<ul>") && clean.contains("<li>um</li>"),
            "{clean}"
        );
        assert!(clean.contains("<blockquote>cit</blockquote>"), "{clean}");
    }

    #[test]
    fn recognises_empty_shells_as_without_content() {
        assert!(!has_visible_content("<p></p>"));
        assert!(!has_visible_content("<p>  </p><br>"));
        assert!(has_visible_content("<p>algo</p>"));
    }
}
