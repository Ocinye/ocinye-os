# DESIGN_LOCK · D008

D001–D007.1 mantêm os seus locks. A D008 fecha:

**Terminal (D008-A):** cromado (título, contexto, estado do Core, Procurar, Limpar ecrã, Ajuda) · linha de comandos e contexto no prompt · entradas (eco redigido + estado com ícone, texto e código + duração + capability) · blocos (nota, tabela, factos, ajuda, ligações, Nye, recibo) · ajuda/autocompletar a partir do registo · confirmação por plano no diálogo partilhado D006 · colagem de várias linhas em espera · procurar no ecrã · modelo móvel (toolbar de ícones com nome, prompt fixo acima da doca, 44 px).

**Browser (D008-B):** faixa de abas (id interno; compactação >8 + «Todas as abas») · barra (recuar/avançar só onde o runtime dá histórico; recarregar/parar; origem com estado de ligação; «Endereço pedido» na Web; runtime; Nye; transferências; menu) · linhas de aviso do cromado (Web, permissões, pop-ups, IDN, ecrã inteiro) · **faixa «Conteúdo externo · origem»** e moldura da vista · estados internos (nova aba, não é endereço, esquema bloqueado, falha, certificado, a carregar, aba falhou, pode recusar) · painéis laterais (Nye com pré-visualização do envio; transferências; ligação) · modelo móvel (chip de origem, contador de abas, lista de abas, barra inferior 48 px).

Não se reinterpretam superfícies de outras aplicações. Os estados DESKTOP/DEDICATED estão fechados em forma; o comportamento depende do adaptador nativo.


---

# Histórico (D007.1 e anteriores)

# DESIGN_LOCK — D007.1

Locked: `ui/apps/monitor.rs`, `ui/apps/results.rs`, `ui/apps/trash.rs`, their VMs (D007.1 block), `.oc-mon-*`, `.oc-rsl-*`, `.oc-trash-*` and their responsive rules, the `org.act.stop_service` confirmation copy, `ui_reg` strings.
Rules: Monitor never shows a value the runtime did not report, never shows zero for unreported, never shows a chart without a series, never offers Stop without a typed inventory and `may_stop`. Resultados has no creation in the toolbar. Lixo never promises a deadline and never submits permanent deletion until FG-D4.1-02 is resolved.
Preserved: D001–D007 unchanged except the additive `StopService` variant and the `nav.audit` label. Not locked: Terminal, Browser.

---

# DESIGN_LOCK — Design revision D007

Locked (additive; D001–D006 locks unchanged): `ui/apps/messages.rs`, `fabric.rs` (IA, Agentes, Computação), `ledger.rs` (Meus Recursos, Actividade, Auditoria), `member.rs` (Definições, Ajuda), `ops.rs`; the D007 block of `oc-apps.css` (.oc-msg-*, .oc-ops-*); `ops()` in `oc-apps.js`; `i18n/ui_ops.rs`. Rules: IA is not a chat; agents show the authority boundary; compute shows only reported numbers; activity redacts; audit is read-only with allow-listed metadata; settings edit only the member layer.

---

# DESIGN_LOCK — Design revision D006

Locked (D006-owned, additive; D001–D005 locks unchanged):
- `ui/apps/org.rs` — account status tag, role tag (single tone, no hierarchy), unit role/status tags, avatar, refusal, notice, action link, **shared privileged confirmation** (`confirm`), **once-only credential** (`credential_once`).
- `ui/apps/units.rs` — Unidades: list · detail · members · governed add-member picker · create/edit (code immutable) · archive via confirmation.
- `ui/apps/admin.rs` — Administração: Membros roster and member detail (account, units, environments, roles, explicit grants, permission sources, security, sessions) · Novo membro · credential screen · Papéis catalogue · Instância.
- CSS: the `D006` block of `oc-apps.css` (`.oc-org-*`). JS: `credential` and `confirmDlg` in `oc-apps.js`, bound by `OcApps.init(root)`.
- Copy: `i18n/ui_org.rs` (pt canonical, en, fr).

Rules locked with them: no action without a Core flag; no free-form role; candidates only from the Core; the secret only in the POST response that issued it; focus starts on Cancelar; no permission matrix; no Teams UI until a Team domain exists; no Distribution change in D006.

---

# DESIGN_LOCK — Design revision D005

New locks (Design-owned): `ui/apps/{res,projects,work,ideas,datasets,knowledge}.rs`, D005 block of `view_models.rs`, D005 block of `oc-apps.css`, `reslist` in `oc-apps.js`, `i18n/ui_research.rs`. Locked patterns: list | detail two panes (1180 px of window), title column never yields, `data-prio` column order, transitions only from the Core list, closing an idea requires a reason, promotion keeps the idea, dataset versions ≠ file versions, no dataset content preview, source text labelled as data, http(s)-only external links, resource link = canonical deep link.
Code-owned: routes, adapters, VM filling, label mapping of transitions, timezone/locale formatting, search, persistence, dirty template. Core-owned: authorisation, membership, transitions, promotion, classification, provenance, link resolution. Code must not: add a Kanban/board, fake a completion checkbox, enumerate members outside the environment, render source HTML, show storage identifiers, add sort headers without a Core sort, or add a per-app assistant. All D001–D004.1 locks stay.

---

# DESIGN_LOCK — Design revision D004.1

Re-locked: `static/oc-apps.css` (`files` container + column priority; ≤ 640 target block; sticky Notes save), `static/oc-apps.js` (`init(root)`), `ui/apps/{files,mail,mod}.rs`, `AppError` in `view_models.rs` (+NotConnected), `i18n/ui_apps.rs`. Code must not: give secondary Files columns a width that squeezes Nome below 200 px, re-enable a purge submit without a governed confirmation, map `connected = false` to `Unavailable`, shrink ≤ 640 hit areas below 44 px. All D001–D004 locks stay.

---

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
