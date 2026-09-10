// The editor schema — a faithful mirror of the Core's NoteDocument model.
//
// Every node and mark here has a counterpart in
// crates/ocinye-core/src/modules/knowledge/document.rs. The editor is not the
// authority on what may be stored: the Core re-validates every document at its
// boundary. But the schema is the first gate, and it is deliberately closed —
// a pasted element the schema does not know is dropped, not smuggled through.

import { Schema } from "prosemirror-model";

// The link schemes the Core allows (document.rs ALLOWED_LINK_SCHEMES). A link
// with any other scheme is refused here, so it never reaches the Core, and the
// text survives without becoming a live link.
const SAFE_LINK_SCHEMES = ["http://", "https://", "mailto:", "tel:"];

export function isSafeHref(href) {
  const low = (href || "").trim().toLowerCase();
  return SAFE_LINK_SCHEMES.some((s) => low.startsWith(s));
}

const nodes = {
  // The document is a flat sequence of blocks, like the Core model.
  doc: { content: "block+" },

  paragraph: {
    group: "block",
    content: "inline*",
    parseDOM: [{ tag: "p" }],
    toDOM() {
      return ["p", 0];
    },
  },

  // Headings 1..3 — the Core refuses anything outside that range.
  heading: {
    group: "block",
    content: "inline*",
    defining: true,
    attrs: { level: { default: 1 } },
    parseDOM: [
      { tag: "h1", attrs: { level: 1 } },
      { tag: "h2", attrs: { level: 2 } },
      { tag: "h3", attrs: { level: 3 } },
    ],
    toDOM(node) {
      return ["h" + node.attrs.level, 0];
    },
  },

  // A code block holds raw text and carries no marks.
  code_block: {
    group: "block",
    content: "text*",
    marks: "",
    code: true,
    defining: true,
    attrs: { language: { default: null } },
    parseDOM: [
      {
        tag: "pre",
        preserveWhitespace: "full",
        getAttrs(dom) {
          return { language: dom.getAttribute("data-language") || null };
        },
      },
    ],
    toDOM(node) {
      const attrs = node.attrs.language ? { "data-language": node.attrs.language } : {};
      return ["pre", ["code", attrs, 0]];
    },
  },

  // Lists are flat: one line of inline text per item, no nesting — the Core
  // model stores each item as a single Vec<Inline>.
  bullet_list: {
    group: "block",
    content: "list_item+",
    parseDOM: [{ tag: "ul", getAttrs: (dom) => (dom.hasAttribute("data-checklist") ? false : null) }],
    toDOM() {
      return ["ul", 0];
    },
  },

  ordered_list: {
    group: "block",
    content: "list_item+",
    parseDOM: [{ tag: "ol" }],
    toDOM() {
      return ["ol", 0];
    },
  },

  list_item: {
    content: "paragraph",
    defining: true,
    parseDOM: [{ tag: "li" }],
    toDOM() {
      return ["li", 0];
    },
  },

  // A checklist is its own node so that a task is a task in the model, not an
  // inferred "☐ " prefix in some text.
  checklist: {
    group: "block",
    content: "check_item+",
    parseDOM: [{ tag: "ul[data-checklist]" }],
    toDOM() {
      return ["ul", { "data-checklist": "true", class: "oc-checklist" }, 0];
    },
  },

  check_item: {
    content: "paragraph",
    defining: true,
    attrs: { checked: { default: false } },
    parseDOM: [
      {
        tag: "li[data-checked]",
        getAttrs(dom) {
          return { checked: dom.getAttribute("data-checked") === "true" };
        },
      },
    ],
    toDOM(node) {
      const cls = "oc-check-item" + (node.attrs.checked ? " is-checked" : "");
      return [
        "li",
        { "data-checked": String(node.attrs.checked), class: cls },
        ["span", { class: "oc-check-box", contenteditable: "false" }],
        ["div", { class: "oc-check-body" }, 0],
      ];
    },
  },

  text: { group: "inline" },
};

const marks = {
  // Order here fixes the toDOM nesting; the Core model keeps marks as a set,
  // so the order carries no meaning beyond rendering.
  strong: {
    parseDOM: [
      { tag: "strong" },
      { tag: "b", getAttrs: (n) => n.style.fontWeight !== "normal" && null },
      { style: "font-weight=bold" },
      { style: "font-weight=700" },
    ],
    toDOM() {
      return ["strong", 0];
    },
  },
  em: {
    parseDOM: [{ tag: "em" }, { tag: "i" }, { style: "font-style=italic" }],
    toDOM() {
      return ["em", 0];
    },
  },
  code: {
    parseDOM: [{ tag: "code" }],
    toDOM() {
      return ["code", 0];
    },
  },
  link: {
    attrs: { href: {} },
    inclusive: false,
    parseDOM: [
      {
        tag: "a[href]",
        getAttrs(dom) {
          const href = dom.getAttribute("href") || "";
          // A dangerous scheme is dropped: the mark is refused and the text
          // stays as plain text. This is the paste boundary (ADR-0413 §6).
          if (!isSafeHref(href)) return false;
          return { href };
        },
      },
    ],
    toDOM(node) {
      return ["a", { href: node.attrs.href, rel: "noopener noreferrer nofollow" }, 0];
    },
  },
};

export const noteSchema = new Schema({ nodes, marks });
