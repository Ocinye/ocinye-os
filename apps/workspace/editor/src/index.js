// Entry point for the vendored note editor bundle.
//
// Progressive enhancement, like static/app.js: on load it finds every
// `[data-oc-notes-editor]` root and mounts an editor on it. Nothing here reads
// credentials or talks to the Core directly — saves go to the same-origin
// Workspace BFF, which holds the session token (ADR-0601).

import { mount } from "./editor.js";
import { docToNote, noteToDoc } from "./serialize.js";
import { noteSchema } from "./schema.js";

function mountAll() {
  const roots = document.querySelectorAll("[data-oc-notes-editor]");
  roots.forEach((root) => {
    if (root.dataset.ocMounted === "true") return;
    root.dataset.ocMounted = "true";
    mount(root);
  });
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", mountAll);
} else {
  mountAll();
}

// A small public handle, mostly for the browser tests to assert the round trip
// without a running Core.
window.OcinyeNotesEditor = { mount, mountAll, docToNote, noteToDoc, schema: noteSchema };
