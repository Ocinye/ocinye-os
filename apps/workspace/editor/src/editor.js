// The note editor: a ProseMirror view over the Ocinye note schema, with a
// toolbar, keyboard map, input rules, a clickable checklist, and the autosave
// state machine.
//
// The autosave rule is load-bearing (ADR-0413 §7): the status never reads
// "Guardado" until the Core has answered 200, content is never dropped on a
// failure, and a stale base revision is a refusal to overwrite, not a silent
// last write. The editor holds the document; only the Core makes it true.

import { baseKeymap, setBlockType, toggleMark } from "prosemirror-commands";
import { history, redo, undo } from "prosemirror-history";
import { inputRules, textblockTypeInputRule, wrappingInputRule } from "prosemirror-inputrules";
import { keymap } from "prosemirror-keymap";
import { EditorState, Plugin } from "prosemirror-state";
import { liftListItem, splitListItem, wrapInList } from "prosemirror-schema-list";
import { EditorView } from "prosemirror-view";

import { isSafeHref, noteSchema } from "./schema.js";
import { docToNote, noteToDoc } from "./serialize.js";

const DEBOUNCE_MS = 1200;

// Same-origin endpoint that uploads a note image through the BFF to the Core.
const NOTE_FILE_UPLOAD_URL = "/me/files";

const STATUS_TEXT = {
  clean: "",
  editing: "Por guardar…",
  saving: "A guardar…",
  saved: "Guardado",
  error: "Não foi possível guardar.",
  conflict: "Esta nota foi alterada noutra sessão. Recarregue para ver a versão actual.",
  uploading: "A carregar imagem…",
  image_error: "Não foi possível carregar a imagem.",
};

// The toolbar, in order. Each entry names the command it runs and how to tell
// whether it is active for the current selection, so the button can light up.
function toolbarSpec(schema) {
  const n = schema.nodes;
  const m = schema.marks;
  return [
    { key: "p", label: "P", name: "Parágrafo", run: setBlockType(n.paragraph), active: isBlock(n.paragraph) },
    { key: "h1", label: "H1", name: "Título 1", run: setBlockType(n.heading, { level: 1 }), active: isBlock(n.heading, { level: 1 }) },
    { key: "h2", label: "H2", name: "Título 2", run: setBlockType(n.heading, { level: 2 }), active: isBlock(n.heading, { level: 2 }) },
    { key: "h3", label: "H3", name: "Título 3", run: setBlockType(n.heading, { level: 3 }), active: isBlock(n.heading, { level: 3 }) },
    { key: "sep1", separator: true },
    { key: "bold", label: "N", name: "Negrito", run: toggleMark(m.strong), active: hasMark(m.strong) },
    { key: "italic", label: "I", name: "Itálico", run: toggleMark(m.em), active: hasMark(m.em) },
    { key: "code", label: "‹›", name: "Código em linha", run: toggleMark(m.code), active: hasMark(m.code) },
    { key: "link", label: "⛓", name: "Ligação", run: linkCommand(schema), active: hasMark(m.link) },
    { key: "sep2", separator: true },
    { key: "ul", label: "• —", name: "Lista", run: wrapInList(n.bullet_list), active: inList(n.bullet_list) },
    { key: "ol", label: "1.", name: "Lista numerada", run: wrapInList(n.ordered_list), active: inList(n.ordered_list) },
    { key: "check", label: "☑", name: "Tarefas", run: wrapInList(n.checklist), active: inList(n.checklist) },
    { key: "pre", label: "{ }", name: "Bloco de código", run: setBlockType(n.code_block), active: isBlock(n.code_block) },
    { key: "sep3", separator: true },
    { key: "undo", label: "↶", name: "Anular", run: undo, active: () => false },
    { key: "redo", label: "↷", name: "Refazer", run: redo, active: () => false },
  ];
}

function isBlock(type, attrs) {
  return (state) => {
    const { $from, to } = state.selection;
    const node = $from.node($from.depth);
    if ($from.parent.type !== type && node.type !== type) {
      // Look at the closest textblock.
      const parent = $from.parent;
      if (parent.type !== type) return false;
    }
    if (state.selection.empty && $from.parent.type === type) {
      return attrs ? attrsMatch($from.parent.attrs, attrs) : true;
    }
    let found = false;
    state.doc.nodesBetween($from.pos, to, (n) => {
      if (n.type === type && (!attrs || attrsMatch(n.attrs, attrs))) found = true;
    });
    return found;
  };
}

function attrsMatch(actual, wanted) {
  return Object.keys(wanted).every((k) => actual[k] === wanted[k]);
}

function hasMark(type) {
  return (state) => {
    const { from, $from, to, empty } = state.selection;
    if (empty) return Boolean(type.isInSet(state.storedMarks || $from.marks()));
    return state.doc.rangeHasMark(from, to, type);
  };
}

function inList(type) {
  return (state) => {
    const { $from } = state.selection;
    for (let d = $from.depth; d > 0; d -= 1) {
      if ($from.node(d).type === type) return true;
    }
    return false;
  };
}

// A link toggle that asks for a destination and refuses an unsafe scheme. If
// the selection is already a link, it is removed.
function linkCommand(schema) {
  const link = schema.marks.link;
  return (state, dispatch, view) => {
    const { from, to, empty } = state.selection;
    if (empty && !link.isInSet(state.storedMarks || state.selection.$from.marks())) {
      // Nothing selected and not on a link: nothing sensible to link.
      return false;
    }
    if (hasMark(link)(state)) {
      if (dispatch) toggleMark(link)(state, dispatch);
      return true;
    }
    if (!dispatch) return true;
    const href = (typeof window !== "undefined" ? window.prompt("Endereço da ligação (http, https, mailto, tel):", "https://") : null);
    if (!href) return true;
    if (!isSafeHref(href)) {
      if (typeof window !== "undefined") window.alert("Esse endereço não é permitido.");
      return true;
    }
    toggleMark(link, { href })(state, dispatch);
    if (view) view.focus();
    return true;
  };
}

// The clickable checkbox: a click on the box toggles the item's checked state.
function checklistPlugin(schema) {
  return new Plugin({
    props: {
      handleClickOn(view, _pos, node, nodePos, event) {
        if (
          node.type === schema.nodes.check_item &&
          event.target &&
          event.target.closest &&
          event.target.closest(".oc-check-box")
        ) {
          const tr = view.state.tr.setNodeMarkup(nodePos, null, { checked: !node.attrs.checked });
          view.dispatch(tr);
          return true;
        }
        return false;
      },
    },
  });
}

function buildInputRules(schema) {
  const n = schema.nodes;
  return inputRules({
    rules: [
      textblockTypeInputRule(/^(#{1,3})\s$/, n.heading, (match) => ({ level: match[1].length })),
      wrappingInputRule(/^\s*[-*]\s$/, n.bullet_list),
      wrappingInputRule(/^\s*\d+\.\s$/, n.ordered_list),
      wrappingInputRule(/^\[[ xX]?\]\s$/, n.checklist),
      textblockTypeInputRule(/^```$/, n.code_block),
    ],
  });
}

function buildKeymap(schema) {
  const n = schema.nodes;
  const m = schema.marks;
  return keymap({
    "Mod-z": undo,
    "Shift-Mod-z": redo,
    "Mod-y": redo,
    "Mod-b": toggleMark(m.strong),
    "Mod-i": toggleMark(m.em),
    "Mod-`": toggleMark(m.code),
    // In a list, Enter splits the item; elsewhere it falls through to baseKeymap.
    Enter: chain(splitListItem(n.list_item), splitListItem(n.check_item)),
    // Backspace at the start of an empty item lifts it out of the list.
    Backspace: chain(liftListItem(n.list_item), liftListItem(n.check_item)),
  });
}

// Parse the tags field: comma-separated, trimmed, de-duplicated case-insensitively,
// and capped so the field cannot grow without bound.
function parseTags(value) {
  const seen = new Set();
  const out = [];
  for (const raw of String(value || "").split(",")) {
    const tag = raw.trim();
    if (!tag) continue;
    const key = tag.toLowerCase();
    if (seen.has(key)) continue;
    seen.add(key);
    out.push(tag);
    if (out.length >= 20) break;
  }
  return out;
}

function chain(...cmds) {
  return (state, dispatch, view) => {
    for (const cmd of cmds) {
      if (cmd(state, dispatch, view)) return true;
    }
    return false;
  };
}

function mountToolbar(container, view, schema) {
  const spec = toolbarSpec(schema);
  const buttons = [];
  for (const item of spec) {
    if (item.separator) {
      const sep = document.createElement("span");
      sep.className = "oc-notes-toolbar-sep";
      sep.setAttribute("aria-hidden", "true");
      container.appendChild(sep);
      continue;
    }
    const button = document.createElement("button");
    button.type = "button";
    button.className = "oc-notes-tool";
    button.dataset.command = item.key;
    button.textContent = item.label;
    button.title = item.name;
    button.setAttribute("aria-label", item.name);
    button.setAttribute("aria-pressed", "false");
    button.addEventListener("mousedown", (event) => event.preventDefault()); // keep selection
    button.addEventListener("click", () => {
      item.run(view.state, view.dispatch, view);
      view.focus();
    });
    container.appendChild(button);
    buttons.push({ button, item });
  }
  return function refresh(state) {
    for (const { button, item } of buttons) {
      let on = false;
      try {
        on = item.active(state);
      } catch (_e) {
        on = false;
      }
      button.classList.toggle("is-active", on);
      button.setAttribute("aria-pressed", on ? "true" : "false");
    }
  };
}

/// Mount an editor on a `[data-oc-notes-editor]` root element.
export function mount(root) {
  const surface = root.querySelector("[data-oc-notes-surface]");
  const toolbarEl = root.querySelector("[data-oc-notes-toolbar]");
  const statusEl = root.querySelector("[data-oc-notes-status]");
  const titleInput = root.querySelector("[data-oc-notes-title]");
  const tagsInput = root.querySelector("[data-oc-notes-tags]");
  const saveUrl = root.getAttribute("data-save-url");
  if (!surface || !saveUrl) return;

  // The initial document travels in a data attribute — the browser decodes the
  // attribute value, so no `</script>` breakout is possible and no inline block
  // is needed. A legacy note (plain body, no structured document) arrives as an
  // empty value and opens as an empty document.
  let initialDoc = null;
  const raw = root.getAttribute("data-oc-notes-doc");
  if (raw && raw.trim()) {
    try {
      const parsed = JSON.parse(raw);
      if (parsed && Array.isArray(parsed.blocks)) initialDoc = parsed;
    } catch (_e) {
      initialDoc = null;
    }
  }

  let baseRevision = parseInt(root.getAttribute("data-revision") || "0", 10) || 0;
  let dirty = false;
  let saving = false;
  let timer = null;

  function setStatus(state) {
    if (!statusEl) return;
    statusEl.dataset.state = state;
    statusEl.textContent = "";
    const text = document.createElement("span");
    text.className = "oc-notes-status-text";
    text.textContent = STATUS_TEXT[state] || "";
    statusEl.appendChild(text);
    if (state === "error") {
      const retry = document.createElement("button");
      retry.type = "button";
      retry.className = "oc-notes-retry";
      retry.textContent = "Tentar de novo";
      retry.addEventListener("click", () => runSave());
      statusEl.appendChild(retry);
    }
  }

  function scheduleSave() {
    dirty = true;
    if (statusEl && statusEl.dataset.state !== "conflict") setStatus("editing");
    if (timer) clearTimeout(timer);
    timer = setTimeout(runSave, DEBOUNCE_MS);
  }

  async function runSave() {
    if (saving || !dirty) return;
    if (statusEl && statusEl.dataset.state === "conflict") return; // a reload is required first
    saving = true;
    dirty = false; // capture point; edits during the request set it true again
    setStatus("saving");
    const body = {
      title: titleInput ? titleInput.value : "",
      base_revision: baseRevision,
      document: docToNote(view.state.doc),
      tags: parseTags(tagsInput ? tagsInput.value : ""),
    };
    try {
      const res = await fetch(saveUrl, {
        method: "POST",
        headers: { "Content-Type": "application/json", Accept: "application/json" },
        body: JSON.stringify(body),
      });
      if (res.status === 200) {
        const data = await res.json();
        if (typeof data.revision === "number") baseRevision = data.revision;
        // Only now is it true. If more edits arrived while we saved, keep going.
        if (dirty) {
          setStatus("editing");
          scheduleSave();
        } else {
          setStatus("saved");
        }
      } else if (res.status === 409) {
        dirty = true;
        setStatus("conflict");
      } else {
        dirty = true;
        setStatus("error");
      }
    } catch (_e) {
      dirty = true;
      setStatus("error");
    } finally {
      saving = false;
    }
  }

  let view;

  // Upload an image through the BFF, then insert a block that cites the exact
  // FileVersion the Core returned. The bytes never live in the document — only
  // the reference does — and inserting it is a doc change, so autosave persists
  // the reference the same way it persists any edit.
  function uploadImage(file) {
    if (!file) return;
    const form = new FormData();
    form.append("file", file, file.name || "imagem");
    setStatus("uploading");
    fetch(NOTE_FILE_UPLOAD_URL, { method: "POST", headers: { Accept: "application/json" }, body: form })
      .then((res) => (res.status === 200 ? res.json() : Promise.reject(res)))
      .then((data) => {
        if (!data || !data.file_version_id) {
          setStatus("image_error");
          return;
        }
        const node = noteSchema.nodes.image.create({
          file_version_id: data.file_version_id,
          alt: file.name || "",
        });
        view.dispatch(view.state.tr.replaceSelectionWith(node).scrollIntoView());
      })
      .catch(() => setStatus("image_error"));
  }

  const state = EditorState.create({
    doc: noteToDoc(initialDoc),
    schema: noteSchema,
    plugins: [
      buildInputRules(noteSchema),
      buildKeymap(noteSchema),
      keymap(baseKeymap),
      history(),
      checklistPlugin(noteSchema),
    ],
  });

  view = new EditorView(surface, {
    state,
    dispatchTransaction(tr) {
      const next = view.state.apply(tr);
      view.updateState(next);
      if (refresh) refresh(next);
      if (tr.docChanged) scheduleSave();
    },
    // A pasted (or dropped) image is uploaded, not embedded: a data: URL in the
    // body is refused, and an external image URL is never fetched by us.
    handlePaste(_v, event) {
      return handleImageFiles(event.clipboardData);
    },
    handleDrop(_v, event) {
      return handleImageFiles(event.dataTransfer);
    },
  });

  function handleImageFiles(source) {
    const files = source && source.files;
    if (!files || !files.length) return false;
    const images = Array.from(files).filter((f) => f.type && f.type.startsWith("image/"));
    if (!images.length) return false;
    images.forEach(uploadImage);
    return true;
  }

  const refresh = toolbarEl ? mountToolbar(toolbarEl, view, noteSchema) : null;
  if (refresh) refresh(view.state);

  if (toolbarEl) mountImageButton(toolbarEl, uploadImage);

  if (titleInput) {
    titleInput.addEventListener("input", scheduleSave);
  }
  if (tagsInput) {
    tagsInput.addEventListener("input", scheduleSave);
  }

  setStatus("clean");
  return view;
}

// The image button and its hidden file input. Kept out of the command toolbar
// because inserting an image is not a ProseMirror command — it opens a file
// dialog and uploads before there is anything to insert.
function mountImageButton(container, uploadImage) {
  const sep = document.createElement("span");
  sep.className = "oc-notes-toolbar-sep";
  sep.setAttribute("aria-hidden", "true");
  container.appendChild(sep);

  const input = document.createElement("input");
  input.type = "file";
  input.accept = "image/png,image/jpeg,image/webp";
  input.hidden = true;
  input.addEventListener("change", () => {
    const file = input.files && input.files[0];
    if (file) uploadImage(file);
    input.value = "";
  });

  const button = document.createElement("button");
  button.type = "button";
  button.className = "oc-notes-tool";
  button.textContent = "Imagem";
  button.title = "Inserir imagem";
  button.setAttribute("aria-label", "Inserir imagem");
  button.addEventListener("mousedown", (event) => event.preventDefault());
  button.addEventListener("click", () => input.click());

  container.appendChild(button);
  container.appendChild(input);
}
