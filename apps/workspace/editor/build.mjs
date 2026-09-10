// Build the note editor into one same-origin IIFE bundle.
//
// The Workspace CSP is `script-src 'self'` with no nonce and no CDN: the editor
// must be one self-contained file served from /static, with no eval and no
// dynamic import. esbuild's iife format with the ProseMirror deps inlined gives
// exactly that. The output is committed to the repository as a vendored asset;
// rebuild it with `npm ci && npm run build` in this directory after changing
// any source here, and commit the result in the same change.

import { build } from "esbuild";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const outfile = resolve(here, "../static/notes-editor.js");

await build({
  entryPoints: [resolve(here, "src/index.js")],
  outfile,
  bundle: true,
  format: "iife",
  target: ["es2019"],
  minify: true,
  sourcemap: false,
  legalComments: "none",
  // No eval, no dynamic import — the CSP forbids both, and the bundle needs
  // neither. Fail the build loudly if a dependency introduces one.
  supported: { "dynamic-import": false },
  logLevel: "info",
});

console.log(`Built ${outfile}`);
