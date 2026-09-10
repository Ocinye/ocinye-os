// The seam between the editor's document and the Core's NoteDocument.
//
// These two functions are the whole contract. `docToNote` produces exactly the
// JSON that crates/ocinye-core/src/modules/knowledge/document.rs accepts, and
// `noteToDoc` reads exactly what it produces. A link, which the Core models as
// its own inline, is a mark in ProseMirror: it is flattened on the way out and
// rebuilt on the way in, so the round trip loses nothing.

import { isSafeHref, noteSchema } from "./schema.js";

const SCHEMA_VERSION = 1;

/// A ProseMirror document → the canonical NoteDocument JSON.
export function docToNote(doc) {
  const blocks = [];
  doc.forEach((node) => {
    const block = blockToNote(node);
    if (block) blocks.push(block);
  });
  // The Core accepts an empty document, but a note always has at least a
  // paragraph so the cursor has somewhere to land.
  if (blocks.length === 0) blocks.push({ type: "paragraph", content: [] });
  return { schema_version: SCHEMA_VERSION, blocks };
}

function blockToNote(node) {
  const n = noteSchema.nodes;
  switch (node.type) {
    case n.paragraph:
      return { type: "paragraph", content: inlinesToNote(node) };
    case n.heading:
      return { type: "heading", level: node.attrs.level, content: inlinesToNote(node) };
    case n.bullet_list:
      return { type: "bullet_list", items: listItemsToNote(node) };
    case n.ordered_list:
      return { type: "ordered_list", items: listItemsToNote(node) };
    case n.checklist:
      return { type: "checklist", items: checkItemsToNote(node) };
    case n.code_block:
      return { type: "code_block", language: node.attrs.language || null, text: node.textContent };
    default:
      return null;
  }
}

function listItemsToNote(list) {
  const items = [];
  list.forEach((li) => {
    const paragraph = li.firstChild;
    items.push(paragraph ? inlinesToNote(paragraph) : []);
  });
  return items;
}

function checkItemsToNote(list) {
  const items = [];
  list.forEach((item) => {
    const paragraph = item.firstChild;
    items.push({
      checked: Boolean(item.attrs.checked),
      content: paragraph ? inlinesToNote(paragraph) : [],
    });
  });
  return items;
}

function inlinesToNote(node) {
  const out = [];
  node.forEach((child) => {
    if (!child.isText || !child.text) return;
    let href = null;
    const marks = [];
    child.marks.forEach((mark) => {
      switch (mark.type) {
        case noteSchema.marks.link:
          href = mark.attrs.href;
          break;
        case noteSchema.marks.strong:
          marks.push("bold");
          break;
        case noteSchema.marks.em:
          marks.push("italic");
          break;
        case noteSchema.marks.code:
          marks.push("code");
          break;
        default:
          break;
      }
    });
    if (href) out.push({ type: "link", href, text: child.text, marks });
    else out.push({ type: "text", text: child.text, marks });
  });
  return out;
}

/// The canonical NoteDocument JSON → a ProseMirror document.
export function noteToDoc(note) {
  const n = noteSchema.nodes;
  const blocks = note && Array.isArray(note.blocks) ? note.blocks : [];
  const nodes = [];
  for (const block of blocks) {
    const node = noteBlockToPm(block);
    if (node) nodes.push(node);
  }
  if (nodes.length === 0) nodes.push(n.paragraph.create());
  return n.doc.create(null, nodes);
}

function noteBlockToPm(block) {
  if (!block || typeof block.type !== "string") return null;
  const n = noteSchema.nodes;
  switch (block.type) {
    case "paragraph":
      return n.paragraph.create(null, inlineNodes(block.content));
    case "heading": {
      const level = Math.min(3, Math.max(1, block.level || 1));
      return n.heading.create({ level }, inlineNodes(block.content));
    }
    case "bullet_list":
      return n.bullet_list.create(null, listItemNodes(block.items));
    case "ordered_list":
      return n.ordered_list.create(null, listItemNodes(block.items));
    case "checklist":
      return n.checklist.create(null, checkItemNodes(block.items));
    case "code_block": {
      const text = typeof block.text === "string" ? block.text : "";
      return n.code_block.create(
        { language: block.language || null },
        text ? noteSchema.text(text) : null,
      );
    }
    // image and attachment are reserved for Slice B: the editor does not yet
    // produce them, and it renders nothing for them rather than guessing.
    default:
      return null;
  }
}

function listItemNodes(items) {
  const n = noteSchema.nodes;
  return (Array.isArray(items) ? items : []).map((content) =>
    n.list_item.create(null, n.paragraph.create(null, inlineNodes(content))),
  );
}

function checkItemNodes(items) {
  const n = noteSchema.nodes;
  return (Array.isArray(items) ? items : []).map((item) =>
    n.check_item.create(
      { checked: Boolean(item && item.checked) },
      n.paragraph.create(null, inlineNodes(item && item.content)),
    ),
  );
}

function inlineNodes(content) {
  const arr = Array.isArray(content) ? content : [];
  const out = [];
  for (const inline of arr) {
    if (!inline || typeof inline.text !== "string" || !inline.text) continue;
    const marks = [];
    (Array.isArray(inline.marks) ? inline.marks : []).forEach((mark) => {
      if (mark === "bold") marks.push(noteSchema.marks.strong.create());
      else if (mark === "italic") marks.push(noteSchema.marks.em.create());
      else if (mark === "code") marks.push(noteSchema.marks.code.create());
    });
    if (inline.type === "link" && isSafeHref(inline.href || "")) {
      marks.push(noteSchema.marks.link.create({ href: inline.href }));
    }
    out.push(noteSchema.text(inline.text, marks));
  }
  return out.length ? out : null;
}
