# DESIGN_LOCK — Design revision D004

New locks: `src/ui/apps/{mod,files,notes,calendar,mail}.rs`, `static/oc-apps.css`, `static/oc-apps.js`, `src/i18n/ui_apps.rs`, D004 block of `view_models.rs`. Re-locked: `wm/mod.rs` (`dirty_close` save_label/save_form), `oc-wm.js` (`bindDirty`), `document.rs` (+oc-apps assets, last), `ui/mod.rs`. All D001–D003.1 locks stay. Ownership: Design = rendering, layout, states, client mechanics; Code = routes, VM filling, formatting, upload engine, dirty template, persistence; Core = authorisation, storage, revisions, mail transport, calendar data. Code must not: render message/note HTML, expose storage identifiers, add a second chatbot, add a second confirmation dialog, invent collections or counts, or cap uploads at a fixed size.

---

# DESIGN_LOCK — Design revision D003.1

Re-locked: `static/oc-nye.js` (initial filter via the shell's `input` filter; modal trap + `inert`; two-step Esc; focus restoration), `static/oc-nye.css` (`--oc-nye-touch` block ≤ 640), `src/ui/nye/mod.rs` (`data-part="nye-empty"`, 2 tests). Code must not add a second app filter, remove `aria-modal`, or shrink the ≤ 640 hit areas below 44 px. All D003 locks stay.

---

# DESIGN_LOCK — Design revision D003

### D003 · Prateleira removida (decisão do membro, 29 set)
A prateleira de janelas em baixo do Desktop (D002 `wm::shelf`, `.oc-shelf`) foi retirada. As janelas abertas vivem só na barra de aplicações lateral: fixadas com o ponto de execução (1/2); aplicações em execução não fixadas (ex.: Nye) aparecem depois de um separador (`data-part="dock-running"`, `data-running`) e saem quando a última janela fecha. Várias janelas da mesma aplicação: a escolha (`.oc-chooser`) do clique na barra; todas as janelas: o alternador (Alt+W / ícone na barra da janela). `--shelf-h` fica 0. Classe: D002_COMPONENT_EXTENSION (WM presentation). Ficheiros: `ui/wm/mod.rs`, `ui/shell/mod.rs` (dock), `static/oc-wm.css`. Chaves `wm.shelf`/`wm.shelf.all` ficam no catálogo sem uso.

D003 adds and locks the Nye components below and re-locks `shell/mod.rs` (palette dispatch, Nye bar microphone), `document.rs` (loads `oc-nye.css`/`oc-nye.js` last), `ui/mod.rs`, `view_models.rs` (append-only) and `icons.svg` (+ `stop`, `volume`). Every D001/D002/D002.1 lock below stays.

| Component | Package path (`implementation/…`) | Rev | Owner of logic |
|---|---|---|---|
| Nye universal surface (palette extension): field, modes, availability, context chip, results, compact answer/proposal, footer | `apps/workspace/src/ui/nye/mod.rs` (`surface`), `static/oc-nye.css`, `static/oc-nye.js` | D003 | Code (routes, VMs) · Core (search, intent, availability) |
| Messages, safe blocks, sources, activity, processing/egress, grounding | `nye/mod.rs` (`message`, `sources`, `activity`) | D003 | Core / AI Fabric supply data |
| Proposal, execution states, risk/state tags | `nye/mod.rs` (`proposal`) | D003 | Core decides authorisation, confirmation, state |
| Strong confirmation dialog | `nye/mod.rs` (`confirm_dialog`) | D003 | Core; Workspace checks digest |
| Nye application (rail, bar, status chip, log, composer, side panel, drawers, container-query layout) | `nye/mod.rs` (`app`) | D003 | Code (persistence via Core) |
| Voice (push-to-talk) | `nye/mod.rs` (`voice`), `oc-nye.js` (`oc:nye` intents) | D003 | Runtime + AI Fabric (STT/TTS) |
| Nye view models | `src/ui/view_models.rs` (D003 block) | D003 | shapes locked; filled by Code |
| Nye copy pt/en/fr | `src/i18n/ui_nye.rs` | D003 | Design |
| Shell palette dispatch + Nye bar mic | `src/ui/shell/mod.rs` | D003 | — |

Placement lock (D003): the Nye surface is the palette slot of `shell_with_window` (after the launcher, before the switcher), outside `.oc-desk`. `nye::confirm_dialog` is rendered by the route after the shell (the `dirty_close` slot), never inside `.oc-desk`, `.oc-wm`, the Nye surface or the Nye window. At most one blocking dialog per response. No new z-index.

Claude Code must not: show model or provider names in the normal UI; render model HTML; show prompts, system messages or reasoning in activity; infer authorisation or confirmation on the client; persist conversations, confirmations, capabilities or results in localStorage; add wake-word or always-listening voice; render a destructive capability that the Core does not publish.

---

# DESIGN_LOCK — Design revision D002.1

D002.1 re-locks four files (rows below marked D002.1): `static/oc-wm.css`, `static/oc-wm.js`, `src/ui/wm/mod.rs`, `src/ui/shell/mod.rs`. Also locked: the switcher is a global shell overlay drawn after the palette, outside `.oc-desk`; it must not move back into `.oc-desk` or `.oc-wm`. The dirty-close dialog (`wm::dirty_close`) is rendered after the shell (as today by `routes.rs · shell_page`) or as its last child, never inside `.oc-desk` or `.oc-wm`. Order: `.oc-desk` → `.oc-top` → launcher/palette → switcher → dirty close. Closed panel summaries are `display: flex`.

Claude Code must NOT redesign, restyle, simplify, substitute icons, change typography or spacing, replace the design system, resurrect old UI, approximate components, or recreate anything from screenshots. Integrate around the locked code through the view models.

Locked visual properties, for every row below: hierarchy, layout, dimensions, spacing, typography (IBM Plex Sans/Mono), colours and tokens, borders, radius, shadows, glass treatment, icons (`static/icons.svg`), animations and reduced-motion rules, responsive breakpoints, focus styles and a11y attributes.

| Component | Package path (`implementation/…`) | Repository path | Rev |
|---|---|---|---|
| Base document | `apps/workspace/src/ui/document.rs` | same | D001 |
| Tokens, reset, focus, common states | `apps/workspace/static/oc-base.css` | same | D001 |
| Clock, copy | `apps/workspace/static/oc-base.js` | same | D001 |
| Icon sprite | `apps/workspace/static/icons.svg` | same | D001 |
| Logo | `apps/workspace/static/ocinye-logo.png` | same | D001 |
| Shared parts: `icon`, `app_icon`, `core_error`, `pending` | `apps/workspace/src/ui/components/mod.rs` | same | D001 |
| Auth frame: bar, identity, footer, language | `apps/workspace/src/ui/screens/auth/mod.rs`, `static/oc-auth.css` | same | D001 |
| Login (2 steps), Recover (G-26), End of session | `…/screens/auth/login.rs`, `static/oc-auth.js` | same | D001 |
| First access | `…/screens/auth/first_access.rs` | same | D001 |
| MFA: setup (2 columns), codes (2 columns), challenge | `…/screens/auth/mfa.rs` | same | D001 |
| Boot | `…/screens/auth/boot.rs` | same | D001 |
| Shell: top bar (logo 32px, account menu, distribution badge, crumb, Nye bar, «+ Criar», CORE·IA, notifications, clock), app bar, launcher, palette | `…/ui/shell/mod.rs`, `static/oc-shell.css`, `static/oc-shell.js` | same | D001 |
| App window frame (full page, pre-G-05) and `app_pending` | `…/ui/shell/mod.rs` (`app_window`, `app_pending`) | same | D001 |
| Desktop: grid, 14 widgets, glass, customise bar, drag and move, resize, remove, collapse, library, background and dimming sheet, restore with diff, undo toast, save states | `…/screens/home/mod.rs`, `static/oc-desk.css`, `static/oc-desk.js` | same | D001 |
| Widget registry: kinds, sizes, categories, mandatory, subtitles, system defaults per Distribution, diff | `…/screens/home/registry.rs` | same | D001 |
| Error pages 404/403/502 (shell and door) | `apps/workspace/src/ui/screens/error.rs`, `static/oc-shell.css` (`.oc-error`), `static/oc-auth.css` | same | D001.1 |
| Identity could not be confirmed | `apps/workspace/src/ui/screens/auth/identity.rs` | same | D001.1 |
| Window manager: window, controls, content states, layer, shelf, running indicator, chooser, switcher, snap preview, dirty close, Desktop context menu, status/notifications/clock panels | `apps/workspace/src/ui/wm/mod.rs`, `static/oc-wm.css`, `static/oc-wm.js` (presentation part only; the engine is Code's) | same | D002.1 |
| Shell extension (window layer, running dots, panels) | `apps/workspace/src/ui/shell/mod.rs` | same | D002.1 |
| View-model shapes | `…/ui/view_models.rs` | same | D001 |
| All Design copy pt/en/fr | `…/src/i18n/ui_auth.rs`, `ui_base.rs`, `ui_shell.rs` | same | D001 |

Locked by reference, not yet implemented: every screen in `reference/` that has no row above. They are locked in the sense that nobody implements them outside Design (see `HANDOFF.md` §10).

Approved decisions: `HANDOFF.md` §7.
