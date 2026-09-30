# HANDOFF — Ocinye OS canonical UI · Design revision D006

Cumulative: D001 → D005 unchanged plus D006. Supersedes D005. Observed repository: `feat/design-d005` @ `f94b79370ad2c8f3d60ec5d33d5a4650392a34c2` (D005 integrated by Code; merge to `main` not observed). `implementation/` = that tree's files for everything D006 touches, plus D006. Not reset.

**PROCESS RULE.** Claude Code must not start integrating D006 until D005 is merged into `main`. Then: rebase this package's patch on `main`, fmt/build/clippy/tests, `verify.sh`, push, PR, CI green, merge, **STOP**. Merge is not deployment authorisation; production deployment always needs explicit user authorisation. Nothing in this package deploys.

# D006 · Organização · pertença · administração — Unidades · Administração (Membros · Papéis · Instância)

**Core governs; applications implement.** The UI is never the authority for identity, membership, authorisation, roles, invariants, invitations or audit. It renders Core-authorised state and sends the typed POSTs the BFF already has.

## Repository truth this is built on
| Domain | Core (tables · routes) | What the design shows |
|---|---|---|
| Member (conta) | `people` (full_name, display_name, email, institutional_position, status `invited/active/suspended/disabled`, identity_kind, avatar_kind, created_at, last_seen_at) · `GET /administration/members` (MembersManage) · `/people` (MembersView, shared directory) | Administração › Membros: roster, member detail |
| Member creation | `POST /administration/members` → account `invited` + **temporary credential returned once** (docs/identity); `provision`, `password-reset` return the same once-only credential | «Novo membro» form → credential screen shown once. **No email sent** (none exists) |
| Token invitations | `invitations` (status pending/accepted/revoked/expired, token digest only) · `POST /invitations`, `/invitations/accept` — no list, revoke or read | Not designed (M-22). `invited` is shown as the account status «Convidado» |
| Account status | `POST /administration/members/{id}/status` {status, reason ≥ 4}; suspend/disable revoke all sessions immediately; self-lockout refused; last usable `platform_admin` refused (`ensure_not_sole_platform_admin`) | Suspender · Desactivar · Reactivar through the shared confirmation, reason required |
| Delete | `DELETE /administration/members/{id}` only for a never-activated account (`may_be_deleted`) | «Apagar acesso não usado»; a used account is disabled, never deleted |
| Roles | 8 system `TechnicalRole`s defined in code (`GET /administration/roles`, permissions per role, `system: true`); custom roles PLANNED; grant/revoke `/people/{id}/roles` (platform administration); explicit grants (`/administration/grants`, reason ≥ 8); `GET …/access` explains the source of each permission | Papéis: read-only catalogue (no matrix, no editor). Member: roles held (+ revoke), grant role, explicit grants (+ revoke), «Porque tem este acesso» |
| Position | `InstitutionalPosition` (9), grants nothing (ADR-0100) | shown + changeable when `may_change_position` |
| Security | `GET …/security`: has password, temporary credential expiry/expired, MFA required/enrolled, last sign-in, failures, live sessions (agent, IP prefix); revoke one session | Segurança da conta; never a verifier, hash, token or session id |
| Unit | `units` (code immutable, name, description, research_areas, status active/archived, flat) · `unit_memberships` (manager/member, revoked not deleted) · list/get(+`may_manage_members`)/create(+code suggestion; creator becomes manager)/update/archive · members add(upsert)/revoke (last manager refused) | App Unidades: Activas · Arquivadas, detail, members, create/edit, archive |
| Team | **nothing** — no table, contract or registry entry | **Not designed.** TEAMS-01…10 = CORE_CONTRACT_REQUIRED (T-01) |
| Instance | `organisations` (name, profile), `instance_settings` (default_locale, timezone, default_pins, logo), `instance_applications` (explicit decisions over the profile) | Instância: name, Distribution (read-only), language, timezone, application decisions (`save_instance`) |

App Registry (`experience/apps.rs`): `units` → `/units` (`units.view`), `administration` → `/admin` (`members.manage`, keywords include «membros»). Both SingleInstance. **No change** made. A standalone Members directory would need a registry entry (APP_REGISTRY_CHANGE_REQUIRED, deferred, M-24).

## Shared pieces (`ui/apps/org.rs`) — additive
- `status_tag(OrgAccountStatus)` — text + icon + tone (invited neutral/clock, active done/check, suspended attention/lock, disabled closed/minus).
- `role_tag(OrgTechRole)` — translated label + stable id in mono. **One tone for every role**: the Core does not rank them, the view does not suggest a hierarchy.
- `unit_role_tag`, `unit_status_tag`, `avatar(&OrgAvatarVm)` (initials from the Code's existing initials rule; image only from a Core-served URL, never external).
- `refusal(OrgRefusal)` — self-lockout, last platform admin, last unit manager, cannot grant unheld, not deletable, stale, option unavailable; says what to do next; reveals nothing new.
- `notice(OrgNotice)` — one status line after a successful POST (not a toast).
- `action(&OrgActionVm)` — a link that opens the confirmation (`?confirm=<kind>`, GET). It never executes.
- `confirm(&OrgConfirmVm)` — **SHARED_CONFIRMATION_EXTENSION**. One privileged confirmation for all D006 actions, drawn by the route after the shell like `wm::dirty_close` and `nye::confirm_dialog`, on the same `.oc-overlay/.oc-dialog` primitive. Title names the target («Suspender a conta de Marta Quintas»); facts: target, where, current → proposed; consequence copy is the Core's documented semantics; reason field only when the Core requires (`Required(min)`) or accepts it (`Optional`); note «O Core volta a verificar…». Focus starts on **Cancelar**; the button that removes access is `oc-btn-line--danger` and never focused by default; Tab/Shift+Tab stay inside; Esc = Cancelar; a second submit is blocked (`aria-busy`). Confirmation is not authorisation.
- `credential_once(&OrgCredentialOnceVm)` — the **only** VM carrying a secret. `type="password"` by default; Mostrar/Esconder (`aria-pressed`) and Copiar (`navigator.clipboard`, announced) in `oc-apps.js`; nothing is stored. Facts: nothing was sent; it only lets the person sign in and set their own password; if lost, reset — it is never recoverable. «Já entreguei» returns to the member.

## Applications
- `units::app(&UnitsVm)` — `res::list` (title = name; code and state in the title cell; column «Áreas» leaves first) · `two_pane` · detail (code, name, state, edit/archive/Nye only when given, areas, description, members with role; change role/remove only when `may_manage`; «Acrescentar membro» governed picker) · form (create: code suggestion marked indicative; edit: code read-only, never an input).
- `admin::app(&AdminVm)` — whole-app `error` renders **only** the error (no nav, no data). Membros: roster table (identity column = avatar + name + state + address, never yields; Posição p1, Unidades p2, Registo p3, Última actividade p4 leave by container width) · member detail · «Novo membro» · credential screen. Papéis: system catalogue. Instância: information + application decisions (`apps_action: None` = read-only).

## The governed picker (units add member)
Native controls only: a search field (`GET …?candidate_q=`) and a `fieldset` of radios for the **Core-supplied** candidates (no custom combobox, no static `<select>` of the Instance). States: not searched (`None`), no results, results, unavailable (`unavailable: true` while the candidates contract is missing — U-09 — shown with its reason, not as a dead control). Forging `person_id` must still fail in the Core.

## Code tasks (first gate: compile)
1. **Wire** `mod ui_org;` in `i18n/mod.rs` and `super::ui_org::UI_ORG` in `catalog.rs` GROUPS (`apply.sh` does it). `pub mod admin; pub mod org; pub mod units;` are in `ui/apps/mod.rs`. New tests: `org::tests` ×5, `admin::tests` ×5, `units::tests` ×5. Rust was **not compiled** here.
2. **Routes** (existing paths, rendering only): `/units`, `/units/{id}`, `/units/new`, `/units/{id}/edit` → `units::app`; `/admin`, `/admin/members/{id}`, `/admin/members/new`, `/admin/instance` → `admin::app`. Add `/admin/roles` (A-03) and a POST for unit archive (U-08). Each `?confirm=<kind>` renders the same page plus `org::confirm` after the shell. POSTs stay the existing ones; on Core refusal re-render the confirmation with `refusal`; on success redirect with a notice key.
3. **Once-only credential.** `create_member`, `member_reset_password`, `provision_member` are `interface_pending()` today. Render `credential_once` directly in the POST response (no redirect carrying the secret), `Cache-Control: no-store`, no logging, no storage. Never put the secret in a URL, a cookie, a flash message or any other VM.
4. **Available actions.** Use the Core's flags (`may_manage_account`, `may_be_provisioned`, `may_be_deleted`, `may_change_position`, `may_manage_roles`, `may_manage_grants`, `may_manage_members`) and the account status (M-10 adapter rule). Never offer suspend/disable on `is_self`; never offer revoking one's own `platform_admin`.
5. **Enumeration.** Roster only through `/administration/members` (MembersManage). Unit members only from `/units/{id}/members`. Member links from Unidades only when the viewer holds MembersManage. No endpoint beyond these is enumerated to fill a picker.
6. **Fail closed.** Direct URL without authority → `AdminVm.error = Some(PermissionDenied|NotFound)`; lost mid-session → `Revoked`. Nothing protected is rendered first. Re-read after every POST (no cached authority).
7. **Dates/timezone.** All timestamps in the member/Instance timezone (`/me.timezone`), not the browser's. The Top Bar timezone issue is untouched.
8. **Dirty close.** New member and unit forms are `data-oc="app-doc"`; completed privileged actions never use dirty-close.

## Security tests Code must run (not done here)
member enumeration (roster without MembersManage; `/people` scope) · cross-unit access (manage members of a unit one does not manage) · forged `person_id` / `unit_id` / `role` / `grant_id` / `session_id` · role not returned by the Core (no free-form role) · self-lockout (suspend/disable self, revoke own last platform_admin) · last platform admin and last unit manager (revoke **and** demotion through upsert, U-12) · stale privilege (demoted admin keeps an open window; next request fails closed) · credential secrecy (never in logs, audit, HTML other than the POST response, caches, URLs) · direct `/admin/*` URLs · unauthorised organisational relationships (workspace titles in member detail resolved with the viewer's authority, M-05).

## Real-browser checks Code must run
keyboard (Tab/Shift+Tab, lists ↑/↓/Home/End, picker radios, confirmation trap + Esc, focus return) · mobile targets ≥ 44 at 390×844 · responsive columns at ~720/760/820/900 · frame-loaded windows (`OcApps.init(root)` binds the confirmation and credential inside `root`) · pt/en/fr · screen reader (NOT_RUN here).

## Reference
`reference/d006/fixture.html?state=<id>` (index without `state`); `window.__fx(state, vp, lang)` redraws without reload. Data is fictional; Papéis permission lists in the fixture are illustrative (the product reads `GET /administration/roles`).

## Nye
Units: `AppNyeVm` (`units.nye`) using the existing D003 `unit` reference. Members and Administration: **none**. Nye never assigns roles, creates members, changes status or membership.

## Frozen
D001–D005 visuals and contracts; Window Manager; App Registry; Top Bar timezone; Distribution (shown, not changed).

---

# HANDOFF — Ocinye OS canonical UI · Design revision D005

Cumulative: D001 → D004.1 unchanged plus D005. Supersedes D004.1. Observed repository: `feat/design-d004` @ `6eeade4bc5cff79ac7d1ffc5a98267fc6b2c005e` (D004.1 certified). `implementation/` = that tree + D005. Not reset.

# D005 · Investigação e trabalho — Projectos · O Meu Trabalho · Ideias · Dados · Conhecimento

**Core governs; applications implement.** Five view functions over typed VMs, inside D002 managed windows, on the D004 application system (frame, nav, search, states, pagination, save state, Nye link). New shared pieces live in `ui/apps/res.rs`; nothing in D001–D004.1 was reinterpreted.

## Repository truth this is built on
| Domain | Core (routes · tables) | What the design shows |
|---|---|---|
| Ambiente de investigação | `research_workspaces` (kind idea/project, classification), `workspace_memberships` (lead/member/viewer) | «Ambiente» on every resource; people read-only |
| Projecto | `projects` (code, title, summary, objectives, state draft/active/on_hold/completed/archived, origin_idea_id, responsible, started/completed); `GET /projects/{id}`, `POST …/transitions` | list · detail · lineage · transitions. **No create** (a project is born from idea promotion), **no edit** (no route) |
| Tarefa | `tasks` (title, description, state todo/in_progress/blocked/in_review/done/cancelled, priority low/normal/high/critical, assignee, due_on DATE, closed_at); `/tasks` (mine, open_only, workspace_id), create, transitions, assignee | App **O Meu Trabalho** (`ApplicationId::Work`, `/my-work`). Dense list; complete/reopen are transitions; assign among environment people. No board |
| Ideia | `ideas` (summary, research_question, hypothesis, motivation, keywords, state discovery→exploration→concept→review→project_candidate→promoted · rejected/archived with required outcome_note); create, transitions, promotion | lifecycle strip · closing requires a reason · promotion creates the project in the **same** environment; the idea stays, linked |
| Dataset | `datasets` (code, origin vocabulary, licence, usage_restrictions, keywords, classification, state draft/active/deprecated/archived); `dataset_versions` (label, status draft/published/withdrawn, provenance, derived_from, totals); `dataset_files` (logical path) | App **Dados**. Catalogue; versions are resources (not file versions); files by logical path; **no content preview** |
| Conhecimento | `documents` (a claim about a file; content does not travel), `sources` (bibliographic entry: authors, year, DOI…, `content_right`), `research_links` (closed relation vocabulary, origin declared/operation) | App **Conhecimento**: Documentos · Bibliografia. **Bibliografia** (`/bibliography`) opens the same view in the Sources section |

## Shared pieces (`ui/apps/res.rs`)
- `list(&ResListVm)` — semantic `<table>`; column 1 = title + code + state (+ priority) and never yields; extra columns carry `data-prio` 1–4 and leave on a `reslist` container query (<760 p4, <640 p3, <520 p2, <360 p1). No sort headers (no Core sort contract). Rows are links; ↑/↓/Home/End move focus (`oc-apps.js` `reslist`), focus ≠ open (`aria-current`).
- `two_pane(pane, list, detail)` — lista | detalhe side by side from 1180 px of **window** (`@container app`); below, one at a time by `ResPane`, with «Voltar à lista» (`list_href`, keeps filters).
- `state_tag`, `class_tag`, `priority_tag` — text always; tone from 5 semantic tones (neutral/progress/attention/done/closed); Confidencial/Restrito add a lock.
- `transitions(&ResTransitionsVm)` — one POST form; only `available_transitions` from the Core; one `primary`; targets that require a reason sit behind «Encerrar…» with a required textarea.
- `link_row` / `links_section` — the **resource link**: icon, type, title, short meta, relation label, «registada pela operação». Opens the owning app's canonical resource. `href: None` = exists but has no screen → not a link. A link exists only if the Core resolved **both** ends (ADR-0306).
- `prose`, `keywords`, `kv`, `detail_head`, `select`/`input`/`textarea` (`rcdata`).

## Code tasks (first gate: compile)
1. **Compile and test.** Rust was not compiled here. `mod ui_research;` in `i18n/mod.rs` + `super::ui_research::UI_RESEARCH` in GROUPS. New tests: `res::tests` ×3, `projects::tests` ×3, `work::tests` ×2, `ideas::tests` ×1, `datasets::tests` ×1, `knowledge::tests` ×1.
2. **VM names** avoid clashes with `ocinye_contracts`: `ProjectStatus`, `TaskStatus`, `TaskPriorityLevel`, `IdeaStage`, `DatasetStatus` are view enums; map from the contract enums in the controller.
3. **Routes (BFF)**: `/projects[/{id}]`, `/my-work[/{id}|/new]`, `/ideas[/{id}|/new]`, `/datasets[/{id}[?v=label]|/new]`, `/knowledge/{documents|sources}[/{id}|/new]`, `/bibliography` → `knowledge::app` with `KnowledgeSection::Sources`. Normal navigation stays in the window (all five are `SingleInstance`). Re-resolve and re-authorise on every load; revoked → `AppError::Revoked`, never stale content.
4. **Adapters**: see FUNCTIONAL_GAPS D005 (27 ADAPTER_REQUIRED). Transition labels: target state → key (`work.to.done` «Concluir», done→in_progress `work.to.reopen`, blocked→in_progress `work.to.resume`, cancelled→todo `work.to.todo`; ideas `ideas.to.*`; projects `projects.to.*`). `requires_note` from `workflow::requires_outcome_note`.
5. **Dates**: `due_on` is a date; «vencida» and all timestamps in the member/Instance timezone, not the browser's (D004 T1 stays a separate shell follow-up).
6. **Sizes** in the member's locale (D004 rule: Ko/Mo/Go in fr).
7. **Dirty close**: the four create forms are `data-oc="app-doc"` (`doc_form_id`); fill `DirtyCloseVm.save_label` with the create key. Transitions, assign, promotion are immediate POSTs: no dirty state.
8. **Search**: Knowledge shows its field only when `query: Some` (served by the search index, filtered to sources/documents). Projects/Tasks/Ideas/Datasets have no text-search contract → no field.
9. **External URLs**: only `http(s)` become links (the view enforces it too). Source abstracts and titles are data: label stays.

## Nye
`AppNyeVm` per resource (`projects.nye`, `work.nye`, `ideas.nye`, `data.nye`, `know.nye.source`, `know.nye.document`) → canonical Nye with a typed reference (FG-D5-16). No per-app assistant. Everything works with no inference.

## Frozen
D001–D004.1 visuals and contracts; Window Manager; App Registry (no change: labels «Projectos», «O Meu Trabalho», «Ideias», «Dados», «Conhecimento», «Bibliografia»; all SingleInstance); Notes folders, Top Bar timezone, Mail transport untouched.

---

# HANDOFF — Ocinye OS canonical UI · Design revision D004.1

Cumulative: D001 → D004 unchanged plus D004.1. Supersedes D004. Based on `feat/design-d004` @ `4c4ae32` (Code's D004 integration). Hotfix only; no architecture reopened. D005 not started.

## Apply
`git apply patch/d004-to-d004.1.patch` on 4c4ae32 (7 files), or copy those 7 files from implementation/. Every other implementation file is byte-identical to Code's tree at 4c4ae32 (copied back in, so `apply.sh` no longer reverts Code's fixes). Then `cargo fmt`, clippy, `cargo test` (new: `files::tests::d004_1_eliminar_definitivamente_nunca_submete`, `mail::tests::caixa_nao_ligada_nao_promete_tentar_de_novo`; `apps::tests` now iterates 11 errors).

## Code tasks
1. **MAILBOX_NOT_CONNECTED.** When the selected mailbox has `connected = false` (`controllers/productivity/mail.rs` already reads it): message open → `message_error = Some(AppError::NotConnected)` (the list stays: stored data is shown); send → `compose.error = Some(AppError::NotConnected)`, draft kept; fill `connect_href` with the Mail settings route if one exists, otherwise leave `None` (the text alone says where to go). Do not map it to `Unavailable`. `AppError` is `Copy`; any exhaustive `match` on it in Code gains one arm.
2. **Permanent delete.** Nothing to wire. The button never submits; keep refusing `op=purge` on the server until a governed flow exists (FG-D4.1-02).
3. **OcApps.init(root).** The `?frame=1` workaround in `wm-engine.js` can call `OcApps.init(frameRoot)` directly; calling `init()` again is harmless.

## Files column priority (D4-V1)
Measured on the table's own container (`.oc-files-drop`), so the inspector and sidebar are already subtracted:
| Table width | Columns |
|---|---|
| ≥ 900 | ☐ · Nome · Tipo 120 · Alterado 128 · Tamanho 86 · Dono 170 |
| 680–899 | ☐ · Nome · Tipo 104 · Alterado 116 · Tamanho 80 |
| 560–679 | ☐ · Nome · Tipo · Alterado |
| < 560 | ☐ · Nome · Alterado |
Priority: Nome > Alterado > Tipo > Tamanho > Dono. Alterado ranks above Tipo because the D004 mobile model already kept it (the icon already shows the kind). Grid view unchanged.

## Notes toolbar
«Guardar» (sticky, `right: touch + 2px`) and «Mais acções» (sticky, `right: 0`) stay in view; formatting scrolls under them. No overflow menu, no markup change. The save-state text may pass under «Guardar» while scrolled; the live region still announces it.

## Not changed (frozen)
Files/Notes/Calendar/Mail architecture, WM contracts, dirty close (Notes «Guardar», Mail «Guardar rascunho» via the global D002 dialog), upload protocol (multipart, real part progress, no fixed cap), Notes Markdown model, Mail transport, Calendar timezone model and mobile geometry, App Registry, Core/storage/Nye contracts.

## Recorded, not fixed here
- **Top Bar clock timezone (D4-T1):** cross-shell follow-up (FG-D4.1-04). Calendar stays authoritative to the member/Instance zone.
- **Notes folders:** functional gap, deferred (FG-D4.1-03).
- **Query-only links (D4-L1):** `?sort=`/`?view=` lose folder/section; needs ready `href`s in the VM (FG-D4.1-05).
- **Reference artefacts:** primary-button icons render dark in captures (gold in the browser), and `d004-cal-mobile-day` shows blocks stacked — both REFERENCE_RENDERING_LIMITATION; production CSS unchanged.

---

# HANDOFF — Ocinye OS canonical UI · Design revision D004

Cumulative: D001 → D003.1 unchanged plus D004.

# D004 · Core productivity applications — Files · Notes · Calendar · Mail

Observed repository: `feat/design-d003` @ `d4fa8443630468914b46acfb465e37619a0c2cd6` (D003/D003.1 integration by Code). D004 files derive from that tree; not reset.

**Core governs; applications implement.** The four applications are view functions over typed VMs. They never authorise, never format dates themselves, never show storage (no bucket, object key, S3/Garage, host path), never render message or note HTML, and every AI entry point is a link to canonical Nye.

## 1 · Shared application system (`ui/apps/mod.rs`, `oc-apps.css`, `oc-apps.js`)
`frame(app, label, toolbar, side?, main, inspector?)` → `.oc-app` (toolbar `role="toolbar"`, optional side nav, main, optional inspector, polite live region). Helpers: `nav`, `search` (scope written in the placeholder: «Pesquisar nesta pasta / em Notas / nesta caixa»; ⌘K stays the global Nye search), `primary` (the app's own creation), `nye` (canonical Nye link), `save_state` (Clean/Dirty/Saving/Saved/Failed, inline, not a toast), `error` (10 typed `AppError`s, one language for all apps), `empty`, `skeleton` (partial, never a full-app spinner), `load_state`, `more` (cursor pagination: never 10 000 rows in the DOM), `doc_form_id`.
Layout by container queries on the window body: < 720 side nav is a drawer and the inspector replaces the main pane; ≥ 720 side fixed (Mail: two panes); ≥ 1100 inspector beside the list (Mail: three panes). 1440 = D002 normal window, 924 = D002 maximised, 390 = D002 full screen. No engine of its own.
Focus / selected / open are distinct: gold focus ring; selected = light-blue fill + checkbox; open = navy ring/inset.
Touch ≤ 640: `--oc-app-touch: 44px` + transparent `::after` hit areas (D003.1 pattern) on icon buttons, buttons, segmented options, checkboxes; 44 px rows.

## 2 · Files (`ui/apps/files.rs`)
Sections from the Core: My files, Recent, Starred, Shared with me (gap), Projects and units, Trash. Crumbs (never host paths). List (sortable `aria-sort` columns Name/Type/Modified/Size/Owner·context, fixed layout) or Grid (thumbnails). Multi-selection (checkbox, Space, Shift+↑/↓) with a selection bar (Download, Move, Star, Trash; in Trash: Restore, Delete permanently). Inspector: safe read-only preview (image; PDF in a sandboxed iframe; text/code as text, truncated; unsupported state), download, Ask Nye, details, rename, versions (download/make current; «no version history» when `None`). Upload: button (file picker) and drop zone emit the same `oc:files-upload {files, folder}`; Code runs multipart + incremental hash + capacity preflight — **no fixed 512 MB limit**; progress is `<progress max=parts value=done>` only when measured, indeterminate while checking; tray states queued/checking/sending/done/conflict (keep both / new version)/failed (retry)/cancelled (retry). Dragging an Ocinye item onto a folder emits `oc:files-move {id, target}`; Move is always also a form. Trash note states that permanent deletion cannot be undone (Code must add the governed confirmation, FG-D4-18).

## 3 · Notes (`ui/apps/notes.rs`)
`NOTES_EDITOR_MODEL` = restricted Markdown text in a `textarea` (the Core stores note text with revisions; no block model exists, so no Notion clone). Toolbar inserts syntax (heading, bold, italic, list, link, code, quote). White sheet on the grey app ground. Explicit Save with `base_revision`; Conflict is a typed failure that keeps the text. **No autosave** (the Core has none; FG-D4-26). List with active note (surface card), shared marker, scoped search, empty/no-match states. Narrow: list is the screen without an open note; with a note, the sheet (list in the drawer).

## 4 · Calendar (`ui/apps/calendar.rs`)
Month (7-column grid, today, outside days, selected, up to 3 chips + «+N»), Week and Day (time scale, all-day row, overlapping lanes, current-time marker), Agenda (default on mobile). Positions (start minute, duration, lane/lanes) are computed by Code **in the member's time zone** and written as `data-*`; `oc-apps.js` turns them into CSS variables (CSP: nothing inline). Without JS, events list in the day. Time zone chip in the toolbar; the event inspector shows the original time zone when it differs. Colours only by Core `EventScope` (personal/unit/workspace/institution). Create/edit in the inspector (no popup); cancelled events stay, struck through. No drag-reschedule. No external calendars implied.

## 5 · Mail (`ui/apps/mail.rs`)
Mailbox/folders (Core `MailFolder`: Inbox, Starred, Drafts, Sent, Archive, Spam, Trash; counts only from the Core), list (unread dot + weight, sender, subject, snippet, time, attachment/star/external flags, selection), reading pane (headers, remote content blocked by default, **body as plain-text paragraphs inside a frame labelled «Conteúdo da mensagem»** — untrusted, never HTML, never able to confirm or authorise anything), attachments (download, save to Files). Composer: To, Cc/Bcc, Subject, Markdown-restricted body, formatting, attachments from Files, external-recipient notice, Save draft, Send (`formaction` → governed external-communication capability; Core confirmation when required), draft/send states from `OutboxState`; failed send keeps the draft. No mailbox connected → state with «Connect a mailbox». Narrow: list → message/compose with back.

## 6 · Dirty close (D002 reuse) — D002_COMPONENT_EXTENSION (minimal)
`DirtyCloseVm` += `save_label: Option<&'static str>` («Guardar rascunho» for Mail) and `save_form: Option<String>` (the app form «Guardar» submits with `then=close`, so text that only exists in the editor is saved). `oc-wm.js`: `dirty()` accepts an element and is exported as `OcWm.bindDirty`. Flow: `oc-apps.js` marks the window `data-dirty` on input; clicking the window's close on a dirty window mounts the server-rendered `<template data-part="app-dirty" data-win>` (containing `wm::dirty_close`) after `.oc-shell` (the usual global slot) and binds it. Cancel removes it; Don't save posts `/wm/{id}/close decision=discard`. Existing `DirtyCloseVm` literals need the two fields (`None`).

## 7 · Registry / policy
Unchanged: Notes and Files `MultiWindow`, Calendar and Mail `SingleInstance` (Code's registry). App names stay Code's catalogue (`nav.*`). APP_REGISTRY_CHANGE_REQUIRED = FALSE.

## 8 · Contracts: FILES-01…10, NOTES-01…08, CAL-01…08, MAIL-01…10 — see FUNCTIONAL_GAPS § D004 (53 rows).

---

# HANDOFF — Ocinye OS canonical UI · Design revision D003.1

# D003.1 · Nye universal surface — parity & accessibility hotfix

Based on D003 as integrated by Code: `feat/design-d003` @ `8ad860936ee84d4b5756d10842eedb7daed6b0eb`. The package's `ui/nye/mod.rs` is Code's integrated file (the 4 compile fixes, `cargo fmt`) plus D003.1; Code-owned fixes (search without ai.use, composer field `q`, journey assertions) are not touched.

1. **Initial query filter.** One filter: the `oc-shell.js` palette filter (on `input`). `oc-nye.js` dispatches that same `input` event when the surface opens with a value (server `/ask?q=` or reopen), never a second implementation. «Sem resultados» (`data-part="nye-empty"`, server) hides once an app matches or the field changes; the app group hides when empty. Server result groups are never touched.
2. **Real modality.** `aria-modal="true"` kept. While open: focus starts in the field; Tab/Shift+Tab wrap inside the form over visible, enabled, non-inert controls (SVG elements excluded); every other shell subtree up to `body` gets `inert` (blocking dialogs excluded) and is released on close; a `focusin` guard pulls stray focus back.
3. **Esc in two steps.** Capture-phase handler: focus on any control other than the field → back to the field, surface stays; focus in the field → the existing `oc-shell.js` close. A blocking dialog (dirty close, Nye confirmation) keeps priority: the handler stands aside. Launcher/switcher unchanged.
4. **Focus restoration.** The opener is captured on open; on close without navigation focus returns to it if still visible, else to the first focusable control of `.oc-nyebar` (the canonical invocation). Never BODY.
5. **Touch targets ≤ 640.** `--oc-nye-touch: 44px`: transparent centred `::after` hit areas on mode pills, send/stop, icon buttons (mic, attach, panels, drawer), `.oc-nye-btn`, tabs, attachment remove, context chip; min-height 44 on «Continuar na Nye», suggestions, voice language select, details/activity summaries, conversation rows. Visual geometry changes only where a block grew (listed in REFERENCE_CHANGES). D002 app-bar and window controls untouched.

Contracts: VIEW_MODEL / CORE / AI_FABRIC / WORKSPACE / WINDOW / APP_REGISTRY / WIDGET_REGISTRY changes = NONE. Markup: one attribute (`data-part="nye-empty"`).

### Validation (D003.1)
| # | Check (fixture `reference/d003/fixture.html`, production `oc-shell.js` + `oc-nye.js` + CSS, synthetic DOM events) | Viewport | Result |
|---|---|---|---|
| A1 | Opened by the server with `q=nota`: only «Notas» visible | 924×540 | PASS |
| A2 | «Sem resultados» hidden when an app matches | 924×540 | PASS |
| A3 | Clearing the field restores all 8 apps and hides «Sem resultados» | 924×540 | PASS |
| A4 | Typing a no-match hides the app group | 924×540 | PASS |
| A5 | Opened with `q=xyzzy`: app group hidden, «Sem resultados» shown | 924×540 | PASS |
| A6 | Server-provided deterministic groups (3) stay visible | 1440×900 | PASS |
| B1 | Initial focus in the field | 924×540 | PASS |
| B2 | `.oc-top` and `.oc-desk` are `inert` while open | 924×540 | PASS |
| B3 / B4 | Tab from last → first; Shift+Tab from first → last | 924×540 | PASS |
| B5 | No SVG element in the focus order | 924×540 | PASS |
| B6 | A control behind (app bar) cannot take focus | 924×540 | PASS |
| B7 | `inert` removed after close | 924×540 | PASS |
| B8 | Trap wraps at desktop | 1440×900 | PASS |
| C1 | Esc on a result → field, surface stays open | 924×540 | PASS |
| C2 | Esc in the field closes | 924×540 | PASS |
| D1 | Server-opened then closed: focus goes to the top-bar Nye field (never BODY) | 924×540 | PASS |
| D2 | ⌘K from an app-bar button opens with focus in the field | 924×540 | PASS |
| D3 | Esc restores focus to that app-bar button | 924×540 | PASS |
| D4 | Ctrl K from the top-bar Nye field; Esc restores focus to it | 924×540 | PASS |
| E1 | Mode pills: visual 28 px, hit 90×44 | 390×844 | PASS |
| E2 | «Continuar na Nye» 44 px; submit 44×44; result rows 48 px | 390×844 | PASS |
| E3 | No horizontal overflow in the surface | 390×844 | PASS |

Keys were dispatched as synthetic `KeyboardEvent`s: the trap's own wrap logic was exercised; native Tab movement between middle elements and a real screen reader were not. Rust not compiled here: `nye::tests` +2 (`d003_1_…`) are Code's first gate.

---

### D003 · Prateleira removida (decisão do membro, 29 set)
A prateleira de janelas em baixo do Desktop (D002 `wm::shelf`, `.oc-shelf`) foi retirada. As janelas abertas vivem só na barra de aplicações lateral: fixadas com o ponto de execução (1/2); aplicações em execução não fixadas (ex.: Nye) aparecem depois de um separador (`data-part="dock-running"`, `data-running`) e saem quando a última janela fecha. Várias janelas da mesma aplicação: a escolha (`.oc-chooser`) do clique na barra; todas as janelas: o alternador (Alt+W / ícone na barra da janela). `--shelf-h` fica 0. Classe: D002_COMPONENT_EXTENSION (WM presentation). Ficheiros: `ui/wm/mod.rs`, `ui/shell/mod.rs` (dock), `static/oc-wm.css`. Chaves `wm.shelf`/`wm.shelf.all` ficam no catálogo sem uso.

**THIS PACKAGE IS THE COMPLETE CANONICAL OCINYE UI HANDOFF.
DO NOT RECREATE THE DESIGN.
APPLY THE PROVIDED VISUAL IMPLEMENTATION AND CONNECT REAL LOGIC.**

Cumulative: D003 contains D001 → D002.1 unchanged plus Nye. Nothing outside this package is needed.

---

# D003 · Nye — Search · Ask · Act

Base observed: branch `fix/wasmtime-rustsec-2026-0314` @ `55c3e22faf0054dc1d66e84764dbcfd3b0630571` (read from the local checkout). It is `origin/main` @ `b3cbc8e5b9939542f2f517b71b6202b50d36be98` plus one dependency commit (wasmtime 48.0.3, RUSTSEC-2026-0314/0315/0316). The reflog shows the checkout moved from `feat/design-d002` @ `56fb765` to `b3cbc8e`, and the working tree already contains `ui/wm`, `controllers/windows.rs` and the D002.1 switcher placement, so the D002.1 integration is on main. The D003 files were derived from this tree (the Rust files differ from the D002.1 package only by `cargo fmt`). Not reset to `56fb765`.

**One assistant, many agents, one authority: the Core.** Nye is the human-facing surface. It owns no authority, decides no permission, never shows model reasoning and never names a model or provider in the normal UI. Everything it shows is supplied by the Core (or the AI Fabric through the Core) through typed view models.

## 1 · Decisions

| Decision | Value | Why |
|---|---|---|
| `COMMAND_PALETTE_DECISION` | `EXTEND_INTO_NYE_UNIVERSAL_SURFACE` | One global command surface. The D001 palette becomes the Nye universal surface; with `ShellVm.nye = None` it is byte-for-byte the D001 palette. |
| `D001_COMPONENT_EXTENSION` | Command Palette → Nye Universal Surface | see §3 |
| `D002_COMPONENT_EXTENSION` | window shelf removed; open windows only in the side app bar (running unpinned apps after a separator). No overlay z-index change. Top bar: the Nye bar microphone becomes a link to `/ai/prompt?voice=1` only when `voice_input` is `Available`; otherwise the D001 disabled control. | minimal |
| Full application | the existing registry entry `ApplicationId::Prompt` (`/ai/prompt`, `SingleInstance`), presented as **Nye**, as the body of a D002 managed window (`WindowContent::Ready`). Conversations live inside the app. | registry already single-instance; Design does not invent policy |
| `DESKTOP_WIDGET_DECISION` | **A · deferred.** The widget registry (`screens/home/registry.rs`, `ocinye_contracts::desktop`) has no Nye kind. `WIDGET_REGISTRY_CHANGE_REQUIRED = FALSE`. | registry rules are Core-bound |
| Shortcut | ⌘K / Ctrl K stays (already defined by `oc-shell.js`). The label in the surface comes from `NyeSurfaceVm.shortcut` (runtime-provided; `None` hides it). Alt + W stays the switcher. | no conflict |
| Voice | push-to-talk only. No wake word, no passive listening. | brief §43–45 |
| Risk vocabulary | Core `RiskLevel` is canonical. `NyeRisk` maps it: `read_only`→ReadOnly, `low_impact`→ReversibleWrite, `material_mutation`→InstitutionalChange, `external_effect`→ExternalCommunication, `privileged`→Privileged. `Navigation` = opening by deep link (no plan). `Destructive` has **no Core source today** (no definitive deletion is a capability): the presentation exists, the capability does not. | do not invent Core policy |
| Confirmation | from the Core only (`requires_approval` → `NyeAuth::ConfirmationRequired`). The risk class chooses the *presentation*: card button for ReadOnly…InstitutionalChange; the global confirmation dialog for External/Privileged/Destructive. | brief §15–18 |
| Theme | Light, on the existing Ocinye tokens (`oc-base.css`). No new colours outside the state ramps already in D001/D002. | brief §104 |

Note on the bound Industry design system: D001/D002 lock the Ocinye tokens (IBM Plex, navy/gold, capsule controls). D003 follows the lock; nothing from Industry enters production code.

## 2 · Surfaces

**A · Universal surface** (`nye::surface`, drawn by `shell::palette` when `vm.nye` is `Some`). Field, four modes (Automático / Pesquisar / Perguntar / Executar, a native radio group sent as `intent`, empty = Core `Intent::detect`), context chip when it matters, availability line, «Lido como: …» after a request, deterministic results grouped by type (the D001 app list stays as the «Aplicações» group, filtered client-side), an optional compact answer or proposal, and «Continuar na Nye». Submits `GET /ask?q=…&intent=…`; the route answers with `NyeSurfaceVm.open = true`. Ask/Act radios are `disabled` with the typed reason when `ask`/`act` are unavailable; Search never is.

**B · Compact overlay** = A. It holds short answers (`message(…, compact)`) and single proposals. Anything longer offers «Continuar na Nye» (`continue_href`), which opens the app with the same request.

**C · Nye application** (`nye::app`). Conversation list (rail), title, context chip, compact status chip (Pesquisa · Respostas · Voz — not the D002 status panel), Sources/Activity toggles, availability banner, the log, the composer, and the side panel with two tabs: **Fontes** («what supports this») and **Actividade** («what Nye did»). Layout by **container queries on the window body** — no responsive engine: < 860 px the rail is a drawer; ≥ 860 px the rail is fixed; ≥ 1180 px the side panel is a column, below it an overlay drawer; ≤ 560 px compact bar. Desktop 1440: a normal managed window. Tablet < 1100: D002 maximizes it. Mobile ≤ 640: D002's one full-screen surface.

**D · Voice** (`nye::voice`, shown in the app when `NyeAppVm.voice` is `Some`). Recording indicator (dot + text, red only while `Listening`), a 112 px push-to-talk button with `aria-pressed`, hold-to-talk or tap-to-toggle, stop speaking, replay (only with `voice_output`), language pt/en/fr (backend may report `lang_detected`), «Voltar ao texto», and the privacy line «O áudio não é guardado…». `oc-nye.js` emits `oc:nye` intents (`voice-start`, `voice-stop`, `voice-stop-speaking`, `voice-replay`, `voice-lang`) and exposes `OcNye.voiceState(state, text)`; recording, STT and TTS are Code/runtime. Leaving voice mode or `pagehide` always emits `voice-stop`.

**E · Desktop widget**: deferred (§1).

## 3 · D001 component extension report — Command Palette → Nye Universal Surface

Preserved: `#oc-palette`, `data-oc="palette"`, `.oc-overlay`/`.oc-overlay__scrim`, `data-part="palette-q"`, `data-part="palette-item"` + `data-search` (client filter by `oc-shell.js`, unchanged), ⌘K/Ctrl K open, Esc close, `:target` no-JS open, the scrim link. New: `data-nye`, `data-open` when the server opens it, form `action="/ask"` (was `/search`) with `intent`, modes, availability, context, server results, compact answer/proposal, ↓/↑ navigation between results (`oc-nye.js`), footer «Continuar na Nye». Files: `ui/shell/mod.rs` (palette dispatch + `nyebar_mic`), new `ui/nye/mod.rs`, `static/oc-nye.css`, `static/oc-nye.js`. Visual change only while open; closed, the shell is D002.1 (regression test `a_nye_alarga_a_paleta_sem_mudar_as_camadas`, reference `d003-closed-regression`). Contract change: `ShellVm.nye: Option<NyeSurfaceVm>` (default `None`).

## 4 · Overlay priority (stacking model, no new z-index)

All global overlays are `.oc-overlay` (z 200) outside `.oc-desk` (`isolation: isolate`); later in the tree wins:

    .oc-desk (isolated: Desktop → windows → chooser)
      ↓ .oc-top (z 50)
      ↓ launcher        (shell_with_window)
      ↓ Nye surface     (shell_with_window · palette slot)
      ↓ switcher        (shell_with_window)
      ↓ blocking dialog (routes.rs · shell_page, after .oc-shell): dirty close OR Nye confirmation

Rules: at most one blocking dialog per response (the route renders `wm::dirty_close` or `nye::confirm_dialog`, never both; a pending dirty close wins and the Nye confirmation is re-offered after). While a blocking dialog is open, ⌘K/⌘J are swallowed (`oc-nye.js`, capture phase). Opening the switcher closes the Nye surface. The launcher and the Nye surface are mutually exclusive (existing `oc-shell.js`). A confirmation can never be under Nye because it is drawn after the shell.

## 5 · Act: proposal → confirmation → execution

`nye::proposal` shows kicker, risk, state, title, target, scope, parameters (`NyeField`, long values as a block), affected items (`NyeLine`, with per-item result after execution), consequences, the Core decision line, external-content note, superseded note, execution summary/error, and «Detalhes» (capability id, audit reference, time, confirmation expiry). Actions only when `auth == ConfirmationRequired && state == AwaitingConfirmation && !superseded`:
- low/medium impact: `POST /ask/plans/{id}/execute` with hidden `digest` (existing route: approve then execute);
- high impact: «Rever e confirmar» → `?confirm={id}` → the route renders `nye::confirm_dialog` after the shell. The dialog: risk, «Confirma esta acção?», title, target, scope, all fields (full message body), consequences, irreversible note for Destructive, immutability note + expiry; Cancel = `formaction …/reject`, Confirm = `…/execute` with `digest`. Initial focus on Cancel, focus trapped, Esc = Cancel, focus restored.
- «Alterar» (`edit_href`) creates a new proposal; the old one renders `superseded` and cannot be confirmed.
- Retry appears only when `NyeExecutionVm.retry_allowed` and a `retry_action` exist. Progress (`meter`) only when the executor supplies `(done, total)`.

Content never confirms: the only confirmation path is a member-submitted form. `cites_external` adds the explicit note.

## 6 · Ask, sources, activity

Messages render `NyeBlock` (paragraph with citations, heading, list, steps, code, quote, table). **No model HTML**: Code converts the answer into blocks. Streaming: `data-oc="nye-stream"` + `data-src` (same-origin SSE; events `delta {text}`, `done`, `error {code}`); text enters by `textContent`, on `done` the page reloads the server version. Live region: one `role="status"` per app announcing «A responder…» / «Resposta concluída.», never tokens; the streaming body is `aria-busy`. Stop and «Responder de novo» are POST forms (inference only; never an action). Grounding: `Ungrounded` shows «Sem fontes do Ocinye…», `Revoked` shows the removal note, and revoked sources render as «Fonte já não disponível» with no title, context or link (`a_fonte_revogada_nao_mostra_conteudo`). External sources carry «Conteúdo externo · não verificado» and the trust note. Activity is steps (domain, label, state, factual detail): searches, capabilities, results — never prompts, reasoning or system messages. Domain agents appear only as a domain label on a step («A trabalhar com Projectos…»); there is one avatar, one name.

Processing location (`NyeProcessing`) is shown only when supplied: Local / External chips; Blocked by policy note; ApprovalRequired offers «Só nesta Instância» / «Aprovar o envio» (POST `egress_action`).

## 7 · Availability, no inference, connectivity

`NyeAvailability { search, ask, act, voice_input, voice_output, attachments, link }` — never «AI on/off». No inference → «Respostas da IA indisponíveis. A Nye continua a pesquisar, abrir e executar comandos determinísticos.» with the typed reason; Search keeps working (`d003-no-inference`, `d003-search-no-ai`). The word «offline» is never used for missing AI (test). `link`: `Reconnecting` (warn) and `CoreUnavailable` (alert) are distinct from «no inference» (`d003-core-down`).

## 8 · View models (`ui/view_models.rs`, appended)

`NyeIntent`, `NyeReason` (19 typed reasons → `nye.reason.*`), `NyeAvail`, `NyeLink`, `NyeAvailability`, `NyeContextKind`/`State`/`Vm`, `NyeKind`, `NyeHitVm`, `NyeHitGroupVm`, `NyeTrust`, `NyeSourceVm`, `NyeStepState`, `NyeStepVm`, `NyeRisk`, `NyeAuth`, `NyeExecState`, `NyeField`, `NyeLine`, `NyeExecutionVm`, `NyeProposalVm`, `NyeRole`, `NyeBlock`, `NyeMsgState`, `NyeGrounding`, `NyeProcessing`, `NyeAttachmentVm`, `NyeMessageVm`, `NyeConvItemVm`, `NyeConversationVm`, `NyeComposerState`, `NyeComposerVm`, `NyeVoiceState`, `NyeLang`, `NyeVoiceVm`, `NyePanel`, `NyeAppVm`, `NyeSurfaceVm`; `ShellVm.nye`. No executable authority in any of them; `plan_id` + `digest` identify, the Core authorises.

## 9 · Semantic contracts (existing first)

| Group | Existing | Needed |
|---|---|---|
| NYE-01 availability | `GET /ai/status`, `GET /search/semantic-availability`, `AgenticOutcome::Unavailable{reason_code}` | map to `NyeAvailability`; voice/attachment flags |
| NYE-02 conversations | `GET /ai/conversations`, `GET /ai/conversations/{id}` (migration 0044, owner-scoped) | create/rename/archive/search |
| NYE-03 send / stream | `POST /ai/prompt` (non-streaming), `POST /agentic/invoke` | same-origin SSE stream |
| NYE-04 search | `GET /search`, `invoke` → `Results{sources}`, `Intent::detect` | app/setting/command hits (Workspace registry) |
| NYE-05 sources | `ContextSource` (entity, title, locator, classification) | deep-link resolution; re-authorisation on every render |
| NYE-06 proposal | `invoke` → `Planned{plan, requires_approval}`, `ActionPlan{steps, digest}` | structured, labelled parameters per capability |
| NYE-07 confirmation | `/agentic/plans/{id}/approve`, Workspace `/ask/plans/{id}/execute` | digest check in the Workspace route; expiry exposed |
| NYE-08 execution | `/execute`, `Executed{plan, summary}`, `GET /agentic/plans/{id}`, `PlanState` | `retry_allowed`, audit reference per plan |
| NYE-09 context | `invoke{module, workspace_id, resource_*}` | per-conversation context envelope (no global active unit, CLAUDE.md §34.3) |
| NYE-10 attachments | — (`PLANNED` in docs/ai) | attach Ocinye files by reference |
| NYE-11 STT · NYE-12 TTS | — | AI Fabric capabilities + RuntimeCapabilities (microphone) |
| NYE-13 cancel | plan `reject` | stop a running answer; cancel a running plan |
| NYE-14 activity | plan steps and results | retrieval/assembly steps for Ask; domain label |

Full classification: `FUNCTIONAL_GAPS.md` § D003.

## 10 · What Code wires (summary; order in APPLY_PLAN)

1. `/ask`: build `NyeSurfaceVm` from `POST /agentic/invoke` (or `GET /search` when `intent=search`) and render the shell with `nye.open = true`; `?confirm={id}` renders `nye::confirm_dialog` after the shell (same slot as `dirty_close`).
2. Every shell page: `ShellVm.nye = Some(NyeSurfaceVm { open: false, … })` with availability (cheap, cached per request).
3. `/ai/prompt`: the Nye window (`WindowContent::Ready(nye::app(&vm))`), SingleInstance through the D002 engine; `?panel=`, `?voice=`, `?new=`, `?cq=`, `?q=` are presentation parameters.
4. Execute route: check `digest` equals the plan's before approving.
5. Rename the app label: `nav.prompt` «Prompt Ocinye» → «Nye» (and `apps.desc.prompt`), in `catalog.rs` (Code-owned file). `nav.ai` «Ocinye AI» stays the infrastructure app (AI Hub), not Nye.

---


# Previous revisions (kept)

**THIS PACKAGE IS THE COMPLETE CANONICAL OCINYE UI HANDOFF.
DO NOT RECREATE THE DESIGN.
APPLY THE PROVIDED VISUAL IMPLEMENTATION AND CONNECT REAL LOGIC.**

This package (D003) replaces D002.1, D002, D001.2.1 and every earlier Design delivery. It is cumulative, and nothing outside this ZIP is needed. D001.1 is a corrective revision: visual parity and missing presentation states. It adds no product scope; everything listed as D002 stays deferred.

---

## D002.1 · windowing parity and accessibility hotfix

Based on D002 as integrated on `feat/design-d002` (Code's compile fixes and `cargo fmt` included; read from the local checkout). Source of the defects: `docs/ui/CODE_FEEDBACK.md` and `docs/ui/design-integration.json` (D002 record). Three defects, three fixes, nothing else.

**Contract freeze.** No change to the window-manager engine, lifecycle, launch policy, App Registry, persistence, Core contracts, Workspace WM routes, view models or the responsive model. `wm::switcher` becomes `pub` (a presentation function, same markup, same ids and `data-oc` hooks); `wm::layer` no longer draws it.

### 1 · Switcher layering (D002_VISUAL_PARITY_DEFECT 1)
Cause: `layer()` drew `#oc-switcher` inside `.oc-desk__work`. `.oc-desk` (D001) has `isolation: isolate`, so everything inside it (Desktop, windows, shelf, chooser, and the switcher's z 200) composites as one plane under `.oc-top` (z 50). The top bar stayed above the scrim and clickable.

Fix (tree order, not z-index): `shell_with_window` draws `{vm.wm.as_ref().map(wm::switcher)}` after `{palette(vm)}`, a sibling of the launcher and palette, outside `.oc-desk`. The existing `.oc-overlay` z 200 and `.oc-overlay__scrim` (inset 0) then apply in the shell's context:

    .oc-desk (isolated: Desktop → windows → shelf/chooser)
      ↓ .oc-top (z 50)
      ↓ switcher scrim (.oc-overlay__scrim, full viewport, dims and takes pointer)
      ↓ switcher card (.oc-switcher, z 1 inside the overlay)

No new z-index. With `wm: None` nothing is drawn (D001 unchanged). `oc-wm.js` already finds the switcher at document level; the `#oc-switcher` anchors and the engine's `Alt + W` are unaffected. The fixture that produced `d002-switcher` already drew it there, so the reference stands.

### 2 · Closed top-bar controls (D002_VISUAL_PARITY_DEFECT 2)
Cause: in D001 the status pill, bell and clock are flex items of `.oc-top`, so they are blockified (`inline-flex` → `flex`). Inside `<details>` the `<summary>` is a block, and an `inline-flex` summary (pill) or an `inline-flex` child (clock) sits on a text line aligned by baseline: pill +0.25px, clock +1px.

Fix: `.oc-panel-menu > summary { display: flex; align-items: center; }`. No text line, the clock face is blockified as in D001. `details/summary` stays the control (keyboard, Enter/Space, expanded state). At ≤ 640px the D001 rule hides `.oc-status`; the panel wrapper follows it (`.oc-panel-menu:has(> .oc-status) { display: none; }`), so no empty flex item adds a gap.

### 3 · Dirty-close focus trap (D002_A11Y_DEFECT)
Cause: focusables were `button:not([aria-disabled="true"]), [href]`; `[href]` matched the icon's `<use href>`.

Fix in `oc-wm.js · dirty()`: `a[href], button:not([disabled]), input:not([disabled]):not([type="hidden"]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])`, then filtered: not `aria-disabled="true"`, not inside `[hidden]`/`[inert]`, rendered. Tab from the last wraps to the first, Shift+Tab from the first wraps to the last (also when focus is outside the list). Unchanged: initial focus on «Guardar» (first enabled button when saving is not possible), Esc = «Cancelar». New: when the dialog closes without a navigation (removed, `hidden`, or `data-open` dropped by the engine), focus returns to the control that had it before, or to the window's close button. A form submission navigates, as in D002.

### 4 · Dirty-close dialog: already a global overlay (verified, no code change)
The dirty-close dialog is not inside `.oc-desk` in the integrated tree. `routes.rs · shell_page` renders `{body}{dialog.map(ui::wm::dirty_close)}`, and `body` is the complete shell (`home` / `app_pending` → `shell_with_window`). The dialog is therefore a sibling after `.oc-shell`, at `<body>` level, outside every shell stacking context. `.oc-shell` sets no z-index, so the `.oc-overlay` z 200 of the dialog sits above `.oc-top` (z 50) and the isolated `.oc-desk`. An earlier D002.1 note said the dialog was inside `.oc-desk`; that was wrong.

Locked placement: `wm::dirty_close` is rendered after the shell (or as the shell's last child), never inside `.oc-desk` or `.oc-wm`. Order: `.oc-desk` → `.oc-top` → launcher/palette → switcher → dirty close. Moving the call into `shell_with_window` would change the signatures of `shell_with_window`, `app_pending` and `home` and the Code call sites in `routes.rs` without changing the result, so D002.1 leaves it where it is.

Verified in `reference/d002.1/validate.html` (dialog placed as `shell_page` places it): a click on the top bar or on a window hits the dialog backdrop; a click on «Guardar» reaches the button; Tab and Shift+Tab stay inside; Esc = Cancelar; after closing without navigation, focus returns.

### Not changed
White app glyph: verified white in the real browser; the dark glyph in the captures is **REFERENCE_RENDERING_LIMITATION** (annotated in `reference/screenshots/INDEX.json`). Production CSS unchanged. Tablet and mobile compositions unchanged.

---

## D002 · Windowing and shell interactions

The baseline is **D001.2.1, certified and frozen**. It was read from the local checkout: HEAD = `feat/design-d001-2-1` @ `e579d3c99b4dd34bd88127cbe90b8d2cd4e363df`. `main` @ `7c20f8d` could **not** be seen locally: `packed-refs` shows `main` 4f8d048 and `origin/main` 8a62438, both older. D002 is built on the files integrated in `e579d3c`. The only differences between those files and the D001.2.1 package are `cargo fmt` formatting.

### Model
- Applications open in **Ocinye-managed windows**: `<section class="oc-win">` inside `.oc-wm`, over the work area. There is no `window.open`, no popup and no generic modal. The model works in Web, Desktop and Dedicated alike: it has no host API or host chrome, and full-workspace mode stays possible because the layer only occupies `.oc-desk__work`.
- **The window manager owns presentation; the Core owns authority.** Design renders; Code decides focus, order, geometry, snapping, persistence, launch policy (single or multi-instance) and RBAC.
- **Pinned ≠ running ≠ active ≠ minimised.**
  - The app bar holds pinned apps. A running app gets one white dot; several windows get two dots. The active app keeps the D001 blue background, and its dots turn gold.
  - The **Window Shelf** (bottom centre, glass) lists every open window: active in blue with white text, visible in neutral, minimised with a dashed outline and ink-3 name. Unsaved work shows as a gold dot.
  - The app bar is not the taskbar.
- **Zones**
  - The window owns minimise, maximise/restore and close.
  - The shelf owns switching between windows and "All windows".
  - The app bar owns launching, focusing and choosing among an app's windows.
  - The top bar owns the status, notification and clock panels.
  - The Desktop owns its context menu.
  - Application content owns its own resources.
  - No control is duplicated across zones.

### Components (`apps/workspace/src/ui/wm/mod.rs`, `static/oc-wm.css`, `static/oc-wm.js`)
| Component | Rust | Typed inputs |
|---|---|---|
| AppWindow + TitleBar + Controls + Content | `wm::window(&WindowVm, Option<AnyView>)` | `WindowVm { id, app_id, app_href, href, title, subtitle, state: WindowState, active, z, geometry: WindowGeometry, dirty, content: WindowContent }` |
| Window layer (windows, snap preview, shelf, choosers, switcher, live region) | `wm::layer(&WmVm, active_body)` | `WmVm { windows, switcher_hint: Option<String>, multi_window_apps }` |
| RunningAppIndicator | `wm::dock_run`, `wm::dock_label` | computed from `WmVm` by `app_id` (`AppTile` is unchanged) |
| MultiWindowChooser | inside `layer` (apps with ≥ 2 windows) | «Nova janela» only if `multi_window_apps` contains the app |
| WindowSwitcher + WindowPreview | inside `layer` (`#oc-switcher`) | the preview is a representative drawing (app icon and lines), never a capture |
| SnapPreview | inside `layer` | `OcWm.snap('left'\|'right'\|'max'\|null)` |
| DirtyCloseDialog | `wm::dirty_close(&DirtyCloseVm)` | `{ window_id, title, can_save }` |
| DesktopContextMenu | `wm::desktop_menu(can_customise, has_default)`, placed in `home()` | reuses the D001 controls (library, background, customise, restore) |
| SystemStatusPanel | `wm::status_panel(&StatusPanelVm)` | `{ overall, capabilities: [CapabilityVm { kind: Core\|Compute\|Backup\|Ai, required, state: Option<Health>, detail }], storage, detail_href }` |
| NotificationsPanel | `wm::notifications_panel(&NotificationsPanelVm)` | `{ items: Load<Vec<NotificationItem { id, title, body, when: Ago, read, href }>> }` |
| ClockPanel | `wm::clock_panel(&ClockPanelVm)` | `{ today: (y, m, d), first_weekday, days_in_month, agenda: Load<Vec<WidgetItem>> }` |
| Shell with windows | `shell::shell_with_window(&ShellVm, desk, active_body)` | `ShellVm.wm: Option<WmVm>`, `ShellVm.panels: TopPanels` |

### Window states (`data-*`, set by the engine)
- `data-state`: `normal`, `maximized` (fills the Ocinye work area, 8px margin, above the shelf; never the host screen), `minimized` (hidden, stays on the shelf), `snap-left` or `snap-right` (halves).
- `data-active`: active or inactive. Active has a white bar, ink title, full icon and the long shadow with a blue hairline. Inactive has a `#F3F6F9` bar, ink-3 title, 72% icon and a short shadow. Inactive windows stay clickable and are never shown as disabled.
- `data-dragging`: solid and lifted, no scale, `grabbing` cursor. This keeps it distinct from a widget being edited, which is dashed and shrinks.
- `data-resizing`: blue outline plus a «W × H» badge (`OcWm.size`). `data-at-min` turns the outline amber at 360 × 240.
- `data-loading`: progress strip under the title plus a skeleton.
- `data-pulse`: gold ring once. Use it when a single-instance app is launched again: focus the existing window and announce «Esta aplicação já estava aberta.» (`OcWm.pulse(id)`).
- `z` (`data-z`): stacking. Front and back are distinguished by the bar (colour and contrast) as well as the shadow, not by shadow alone.
- Content: `Ready`, `Loading`, `Pending` (the D001 app_pending language, now inside the window), `Failed(ref)`, `Denied`, `Unavailable`.
- Snap: dragging near the left, right or top edge of the work area (16px) shows the preview (left half, right half, maximise). Quarter snaps were left out on purpose.

### Deep links, refresh, restore
- `GET /files/abc` renders the shell with the window layer. The deep-linked window has `content: Ready` and its body; the other open windows have `Loading` and are fetched by the engine with `?frame=1`.
- The Desktop sits underneath: on `/` it is the D001 Desktop; on an app route it is the wallpaper only.
- Visual state lives in `data-*` set by the server, so a refresh or a restored session renders the same thing. Nothing depends on an animation.
- Without JS, only the active window is shown, maximised. Controls `POST /wm/{id}`; the switcher opens by anchor and every window is a link.

### Responsive (the D001 shell widths: 1100 and 640)
- **≥ 1100px**: free windows, drag, resize, snap.
- **641–1099px (tablet, which includes the 924×540 baseline)**: windows are always maximised. No drag, resize or snap, and the maximise control is hidden, since it would be a dead control. Switching is through the shelf or the switcher.
- **≤ 640px (mobile)**: one app surface at a time, full screen. A 48px bar with «Voltar ao Desktop» (minimise), the name and resource, the switcher button (44px) and close. The shelf is hidden; the D001 app bar stays at the bottom.

### Keyboard and accessibility
- Everything is a real button, link or form.
- Every control has an aria-label with the app name (`wm.minimize` and similar take `{name}`).
- The switcher is `role=dialog`: arrows and Tab move, Enter opens, Esc leaves.
- The chooser: Esc closes it, and focus goes to the first item.
- The Desktop context menu is `role=menu`: right-click, the Menu key or Shift+F10; arrows; Esc returns focus to the Desktop.
- The dirty-close dialog is `role=alertdialog` with the focus trapped inside it: Esc = Cancel, and focus starts on «Guardar».
- The live region announces snap targets and the single-instance focus.
- `prefers-reduced-motion` removes every animation. `forced-colors` is supported.
- **Shortcuts:** Design fixes none. `WmVm.switcher_hint` carries the platform text, and Code maps the keys and calls `OcWm.openSwitcher()`.

### Motion
- Open: 200ms, fade plus 6px rise. Mobile: 12px slide.
- Switcher: fade plus scale .98. Shelf item arriving: 8px rise.
- Snap preview: 150ms on its edges. Single-instance focus: 600ms gold ring.
- Minimise and restore are instant (`display`), so they never delay work.
- There is no blur on windows. Blur is used only on the shelf, switcher, chooser, context menu and panels, so it stays light with 8–12 windows open.

### Dirty close
- Clean → close immediately. Dirty → `dirty_close` with «Guardar» (gold, main action), «Não guardar» (danger outline) and «Cancelar».
- The form is `POST /wm/{id}/close` with `decision=save|discard|cancel`.
- If `can_save = false`, «Guardar» is drawn unavailable with its reason.
- This dialog is for unsaved application work only. It is never for deleting records or other destructive operations.

### Top-bar panels (D001_COMPONENT_EXTENSION)
- With `ShellVm.panels.{status, notifications, clock} = None`, each control stays the D001 link, byte for byte. With `Some`, the control opens its `<details>` panel, using the D001 menu behaviour and glass.
- **Status panel.** The overall state counts **required capabilities only**. Required capabilities are listed first, then optional ones. When AI is not Operational the panel says «Sem IA, o Ocinye OS continua a funcionar…». A missing record shows as «Sem registo», which claims neither success nor failure. Storage is shown as a `<meter>`. «Estado detalhado» appears only when `detail_href` is set (administration).
- **Notifications panel.** Up to 6 items, with a dot for unread. «Marcar todas como lidas» is `POST /notifications/read-all`. «Ver todas» goes to `/notifications`.
- **Clock panel.** The month (no navigation) and up to 3 of today's events, then «Abrir o Calendário». It is not a second Calendar.
- The account menu is D001 and unchanged. The distribution badge and «+ Criar» are also unchanged.

### D001_COMPONENT_EXTENSION (additive, and D001 output is unchanged when `wm = None` and `panels = default`)
| Component | Change | Why |
|---|---|---|
| `shell::shell` | Delegates to `shell_with_window`. With `wm`, `main` goes into `.oc-desk__work` next to the layer, and `.oc-desk` gets `data-wm`. | The windows need a layer over the work area that doesn't scroll with the Desktop. |
| `shell::dock` | Pinned apps get `data-oc="dock-app"`, `data-windows` and `.oc-dock__run`, plus an aria-label with the window count, **only when they have windows** | Running must be distinguishable from pinned. |
| `shell::top_bar` | CORE·IA, the bell and the clock open panels when `panels.*` is set | These are the missing top-bar panels (FG-004/005/006). |
| `shell::app_pending` | With `wm`, returns only the shell; the window arrives with `WindowContent::Pending`. Returns `AnyView`. | app_pending now lives inside the managed window. |
| `home::home` | Adds `wm::desktop_menu` at the end | The Desktop context menu (FG-009) |
| `document.rs` | Shell surface loads `oc-wm.css` and `oc-wm.js` | New assets |
| `icons.svg` | Adds `win-min`, `win-max`, `win-restore` and `windows` | Window controls |

**D001_REGRESSION_CHANGES = NONE expected.** With `wm: None` and `panels: TopPanels::default()`, the shell, Desktop, auth and error pages render the D001.2.1 markup (a test in `shell` pins this). No D001 screenshot changes. The new stylesheet only targets new classes, apart from `.oc-dock__btn { position: relative }`, which is neutral in D001.

### VIEW_MODEL_CHANGES (additive)
- `ShellVm.wm: Option<WmVm>` and `ShellVm.panels: TopPanels` are new fields. `ShellVm` derives `Default`, but **controllers that build `ShellVm { … }` without `..Default::default()` must add `wm: None, panels: TopPanels::default()`**.
- New types: `WindowState`, `WindowContent`, `WindowGeometry`, `WINDOW_MIN`, `WindowVm`, `WmVm`, `DirtyCloseVm`, `Capability`, `CapabilityVm`, `StatusPanelVm`, `NotificationItem`, `NotificationsPanelVm`, `ClockPanelVm`, `TopPanels`.
- No change to the widget registry, widget identifiers, desktop persistence, auth or error pages.
- CORE_CONTRACT_CHANGES = NONE. The BFF contracts that are needed are in FUNCTIONAL_GAPS FG-010 and FG-026 to FG-031.

### CSP note
`oc-wm.js` writes geometry with `element.style.setProperty('--x', …)` (CSSOM). CSP `style-src 'self'` blocks `style=` attributes and inline `<style>` blocks, but **not** CSSOM. The server HTML still carries no `style=` (the tests check this).

## 000. D001.2.1: Indicators hotfix (CSS only)
- `.oc-kpis` changes from `repeat(auto-fit, minmax(150px, 1fr))` to `repeat(4, minmax(0, 1fr))`. Only 4 or 2 columns exist, so 3 + 1 is impossible by construction.
- The 2 + 2 threshold moves from 639px to 643.98px of work area. 4 tiles need 4 × 150 + 3 × 14 = 642px inside the widget, and the widget's transparent border takes 1px on each side, so the work area needs ≥ 644px.
- Work area 639, 640, 641, 642 or 643px → 2 + 2. Work area ≥ 644px → 4 in a row, each tile ≥ 150px. No clipping, no overflow.
- Only `oc-desk.css` changes (delta: `patch/d001.2-to-d001.2.1.patch`). No Rust, view models, registry, defaults, i18n or contracts change.
- The reveal tile at the bottom left is still FG-007 (reference only).

## 00. D001.2: closing D001 visual parity

The input was `docs/ui/CODE_FEEDBACK.md` and `docs/ui/design-integration.json` (D001.1, `feat/design-d001-1`). D001.2 changes CSS only, plus two reference corrections. No Rust, no JS, no i18n.

### Correction to the D001.1 premise: the reference is a 2-column composition
The prototype that produced every Desktop reference picks its columns like this: `cols = W < 1100 ? 2 : W < 1500 ? 3 : 4` (see `reference/Ocinye OS.dc.html`). The 924×540 capture is therefore a **2-column grid**. The Indicators span the full width (4 in a row, 107px). Calendar and Tasks are **1×2**, each half the width.

We both read the capture as "4 columns, with Calendar and Tasks at 2×2" in D001 and D001.1. That was wrong, and it is the root of defects 3 and 4 and of the Calendar/Tasks ambiguity.

D001's real defect was never "2 columns" as such. It was Indicators growing to 259px and the page growing. D001.1 fixed the page growth, which stays fixed.

### RESEARCH_DEFAULT_LAYOUT_DECISION = REGISTRY_IS_CANONICAL
- `registry::system_default(Research)` is unchanged: Calendar 1×2, Tasks 1×2.
- The reference shows exactly that default, in the 2-column state of an 854px work area. **No reference file needs correcting** for this. The Desktop screenshots stay as they are (`REFERENCE_CORRECTION` = none for the layout).
- Code: delete the layout you saved by hand through `PUT /me/desktop` (Calendar and Tasks at 2×2), or press «Repor predefinição», and compare with the default.
- WIDGET_DEFAULT_CONTRACT_CHANGE = FALSE.

### LOGIN_VERTICAL_REFERENCE_DECISION = IMPLEMENTATION_CANONICAL_REFERENCE_CORRECTED
- The three `auth-login-*` captures came from a prototype document taller than 540px: they have a scrollbar and a 28px offset. They move to `archive/screenshots-superseded/` as DO_NOT_USE_FOR_PARITY. Their horizontal and size information is still right.
- Canonical auth at 924×540 is the implementation as integrated: logo 52px at x=436, card 360px at x=282, centred vertically with no overflow, status bar at y=0.
- The production login does **not** move.

### Fixes
| # | Defect | Fix | File |
|---|---|---|---|
| 1 | The CORE·IA chip was transparent | Base pill `#F3F6F9` (the reference value). Hover and open `#EAEFF4`, pressed `#E1E8F0`, gold focus ring. Status semantics are unchanged. | `oc-shell.css` |
| 2 | Left inset 18px with the app bar hidden | `.oc-desk[data-dock="hidden"] .oc-desk__main { padding-left: 30px }`. With the bar hidden the work area gets the Desktop's own margin, and the grid starts at 6 + 30 = x=36. With the bar visible the 12px joint to the bar stays. This is not tied to one width. | `oc-shell.css` |
| 3 | 1-column titles were cut | (a) Columns: the grid has 4 columns only when the work area is ≥ 960px (1-column widgets ≥ 225px). From 400 to 959px it has 2 columns (≥ 193px), and below 400px, 1. At 924px, titles sit in about 420px (hidden bar) or about 390px (visible bar). (b) Titles are never truncated. They break onto a second line at a word boundary (`line-clamp: 2`, `text-wrap: balance`, 1.2 line height), and the subtitle wraps too. The header gap goes from 9 to 8px. Checked for every widget title and subtitle, the longest being «Continuar trabalho», «Armazenamento», «Estado do sistema» and «ATRIBUÍDAS A MIM»: they fit on one line at ≥ 225px and wrap cleanly below that. | `oc-desk.css` |
| 4 | Indicators 3 + 1 at 639px | Indicators stay 4 in a row while the work area is ≥ 640px (107px). Below that they go 2 + 2 across two rows (the widget takes 9 rows, 259px). 3 + 1 no longer exists. | `oc-desk.css` |

### Responsive table (Desktop, app bar visible / hidden; work area ≈ viewport − 130 / − 70)
| Viewport | Columns | Indicators |
|---|---|---|
| 1440 | 4 / 4 | 4 in a row |
| 1100 | 4 / 4 | 4 in a row |
| 924 | 2 / 2 | 4 in a row, 107px (the reference) |
| 899 | 2 / 2 | 4 in a row |
| 760 | 2 / 2 | 4 in a row (visible bar: 2 + 2 once the area drops below 640) |
| 640 | 2 / 2 | 2 + 2 (4 in a row only from a 644px work area) |
| 639 (mobile shell: bar at the bottom, 14px padding) | 2 | 2 + 2 |
| 520 | 2 | 2 + 2 |
| 375 | 1 | 2 + 2 |

### D001.1 checklist item "4 columns at 924×540"
Superseded by the correction above: **2 columns at 924×540 is the reference.** Everything D001.1 actually fixed stays fixed:
- Indicators at 107px, no page growth, the app bar in view;
- collapse chrome and tooltip, MFA key, recovery codes, login focus;
- account-menu glass, weekday, restore copy;
- `Backup::Unknown`, identity failure, 404, 403 and 502.

### Contract report
`VIEW_MODEL_CHANGES = NONE` · `WIDGET_REGISTRY_CHANGES = NONE` (`registry.rs` is byte-identical to D001) · `WIDGET_DEFAULT_CHANGES = NONE` · `CORE_CONTRACT_CHANGES = NONE`. Status semantics are unchanged (OPERACIONAL; CORE_STATUS_CONTRACT_FOLLOWUP stays with Code).

---

## 0. D001.1: what changed and what you must do (still valid, except the "4 columns at 924" premise, which §00 corrects)

The input was `docs/ui/CODE_FEEDBACK.md` and `docs/ui/design-integration.json`, read from the repository.

### CONTRACT_CHANGES_FROM_D001
View-model additions only. There is no change to the widget registry, to widget identifiers, sizes or mandatory flags, to the Desktop persistence or versioning, to auth or shell contracts, or to any Core/BFF API.

| Change | Kind | Why it was unavoidable | What Code does |
|---|---|---|---|
| `DesktopDefault.source: DefaultSource { System, Instance }` (new field; `Default` = `System`) | view model | The restore sheet must tell a built-in Distribution default apart from an administrator publication without inventing a version or a date. No existing field could carry that honestly. | Set `System` for `registry::system_default`, with `published` left empty. The sheet shows «PREDEFINIÇÃO DO SISTEMA · Disposição Research do Ocinye OS · Incluída no Ocinye OS…». `Instance` is only for FG-014 publications. With `System`, the «nova predefinição» notice never shows, whatever `base_version` is, so the `base_version` pin workaround can go. |
| `Backup::Unknown` (new variant) | view model | The Core has no backup record. `Never` and `Failed` would both be false. | Use `Backup::Unknown` and pass `backup_fresh = true` to `derive_state`, so a missing record doesn't degrade the state. The widget can be `Ready` instead of `Unavailable`. |
| `ErrorKind`, `ErrorVm { kind, reference, retry_href }` (new) | view model | New error pages | Render `screens::error::in_shell(&ShellVm, &ErrorVm)` when there is a member, and `screens::error::at_door(&DoorVm, &ErrorVm)` when there isn't, with HTTP 404, 403 or 502. `retry_href` is used only for 502. |
| `IdentityFailVm { door, reference, retry_href }` (new) | view model | The fail-closed identity page | When `/me` fails technically, render `screens::auth::identity::identity_unavailable(&vm)` with 503, replacing the bare `core_error`. |

Widget registry: CONTRACT_CHANGE_REQUIRED = **NONE**. The Core test that pins `ocinye_contracts::desktop` to `registry::KINDS` stays green.

### Fixes
| # | Defect | Fix | File |
|---|---|---|---|
| 1 | 2 columns below 1100 px (the reference shows 4 at 924×540) | The grid answers to the **width of the work area** (a container query on `.oc-desk__main`): 4 columns down to a 640 px area, 2 down to 400 px, 1 below that. At 924 px the area is about 800 px, so 4 columns. At 1440 px, still 4. Indicators keep 107 px. | `oc-desk.css`, `oc-shell.css` |
| 2 | The page grew and the app bar fell below the fold | `.oc-shell` has height `100dvh` (fallback `100vh`) and `overflow: hidden`. `.oc-desk__main` scrolls internally (`min-height: 0`). | `oc-shell.css` |
| 3 | The collapse button had native chrome | `button.oc-dw__all` resets `appearance`, border, background and font, and uses the gold focus ring | `oc-desk.css` |
| 4 | The tooltip showed «Recolher {name}» | `title` uses `tf` with the name in pt, en and fr. A test forbids a raw `{name}`. | `home/mod.rs` |
| 5 | The 32-character MFA key overflowed | The columns get `min-width: 0`. The key wraps at any character, is never clipped, keeps 12 px mono, is `user-select: all`, and copies the raw key. | `oc-auth.css` |
| 6 | 17-character recovery codes wrapped | `repeat(auto-fill, minmax(18.5ch, 1fr))` + `nowrap` + tabular figures: one code per line, and the column count adapts | `oc-auth.css` |
| 7 | Focusing step 2 scrolled the page | `.oc-auth` has height `100dvh`, and the stage scrolls internally. `focus({ preventScroll: true })`. The focus itself is kept. | `oc-auth.css`, `oc-auth.js` |
| 8 | The account menu was opaque | The generic `.oc-menu__pop` rule came later and overwrote the glass. Glass (72 % white, blur 28 px, radius 20 px) now lives in the base rule. This is a real mismatch with the reference. | `oc-shell.css` |
| 9 | «Segunda» | The weekday is always 3 letters and the month is the Intl short form without a dot: pt «Seg 28 set», en «Mon 28 Sep», fr «Lun 28 sept». Checked against Intl. | `oc-base.js` |
| 10 | The built-in default looked published | `DefaultSource`: an origin label plus honest copy, with no version or date for `System` | `home/mod.rs`, `oc-desk.css`, `ui_shell.rs` |

### New presentation states
- **Backup with no record**: «cópia sem registo» / «backup not recorded» / «sauvegarde non enregistrée». Neutral text, and the state dot follows only the Core and the nodes.
- **Identity could not be confirmed**: the auth frame with no shell. Red shield. «Não foi possível confirmar a sua identidade» and an explanation that the workspace is hidden for security. «Tentar de novo» (`retry_href`), «Terminar sessão» (`POST /logout`), and the reference only.
- **404 / 403 / 502**: a glass card in the work area, or the auth frame when there is no member. It carries the code, an icon (search, lock, warning), a title, one sentence, «Voltar ao Desktop» (or «Iniciar sessão» at the door), «Tentar de novo» for 502 only, and the reference. Never an endpoint name, trace or host. Keys: `error.*`.

### PRODUCT_DECISION · door status
The door shows **OPERACIONAL whenever every mandatory capability is healthy.** Optional capabilities, AI above all, never change the door: Ocinye must work without AI. Their state belongs to the CORE·IA chip and to «Estado do sistema».
- **DEGRADADA** is reserved for a *mandatory* capability working with limits. **INDISPONÍVEL** is shown when a mandatory capability fails, or when `/ready` doesn't answer or reports `blocked`.
- The current integration (`degraded` → OPERACIONAL) is correct for today.
- **CORE_STATUS_CONTRACT_FOLLOWUP**: `/ready` should report mandatory and optional components separately, so that «DEGRADADA» can appear when it is true. Until then the door never shows it.

### Accepted as is (from CODE_FEEDBACK)
- End of session: the D001 code wins over the reference card; no unit is shown (§34.3).
- G-27: always `expired` until the Core gives the reason.
- Recovery with `available = true`: FG-002, D002.

### Deferred
`DESKTOP_DARK_DESIGN`, `DESKTOP_SYSTEM_DESIGN`, and everything listed as D002 (§1, §10).

---

## 1. Read this first: what is implemented and what is reference only

The Design exists in two forms. Know which one you are looking at.

| Form | Where | What it is | Your action |
|---|---|---|---|
| **Visual implementation** (Rust + Leptos SSR, CSS, static JS) | `implementation/` | Production code for the repository's real stack. Covers: auth, the shell, the Desktop, the app window frame. | APPLY, then WIRE to real data |
| **Golden visual reference** (interactive HTML prototype) | `reference/Ocinye OS.dc.html` (+ `Ocinye OS Apps.dc.html`) | The whole designed product: every app, window manager, Nye, lock screen, admin, setup. | REFERENCE_ONLY. Not code to copy. |

**Visual implementation is complete for** boot, login, recovery, end of session, first access, MFA, the shell (top bar, app bar, launcher, palette, menus), the Desktop with all 14 widgets and full customisation, and the app window frame.

**Visual implementation is NOT yet written for** the 29 applications, Nye, the lock screen, the setup wizard and the window preview `/preview/{kind}/{id}`. Since D002 the window manager, the top-bar panels (status, notifications, clock) and the Desktop context menu are implemented. They exist only in the reference. See `EXPORT_LIMITATIONS` (§10) and `FUNCTIONAL_GAPS.md`. Do not build these by hand from screenshots. They will arrive as further `implementation/` files in later Design revisions (D002+), each cumulative like this one.

"VISUAL IMPLEMENTATION COMPLETE" and "FUNCTIONAL IMPLEMENTATION COMPLETE" are tracked separately per component in `COMPONENT_INVENTORY.md`.

## 2. Repository baseline (observed)

- D001 was integrated on `feat/design-d001`, based on `c99cbda`. D001.1 applies on top of it: its files replace the D001 files one for one.

- Repository: `ocinye-os` (local checkout). Remote name: UNKNOWN.
- Observed commit: `c99cbdac2402c6fed16fbcd0d69690fa51e57358` (detached HEAD; branch name UNKNOWN, earlier referred to as `chore/ui-wipe`).
- The baseline contains **no** `apps/workspace/src/ui/` directory. This package creates it.
- Crate: `apps/workspace` (`ocinye-workspace`, lib `ocinye_workspace`). Axum BFF + Leptos 0.8 SSR (`to_html`), no hydration. Static assets are served from `/static/`.
- Package manager: cargo. There is no JS toolchain. Static JS is hand-written and has no build step.
- i18n: `crate::catalogo!` entries (`pt`/`en`/`fr`) in `src/i18n/*.rs`, collected in `catalog.rs::GROUPS`. Resolved with `t`, `tf` and `tp`. The locale comes from a task-local.
- Root `Cargo.toml` already declares `leptos 0.8 (ssr)` and `qrcode 0.14 (svg)` in `[workspace.dependencies]`.

## 3. Architecture of the Design code

```
apps/workspace/src/ui/
  mod.rs              module root
  document.rs         <html>/<head>: CSS/JS per Surface (Auth | Shell), runtime.js first
  view_models.rs      ALL view models. The only contract between routes and views
  components/mod.rs   icon(), app_icon(), core_error(), pending()
  testing.rs          assert_contracts(): CSP and a11y checks used by every view test
  shell/mod.rs        shell(), top_bar, dock, launcher, palette, app_window(), app_pending()
  screens/auth/*      boot, login (+recover, session_end), first_access, mfa
  screens/home/*      Desktop (home()) + registry (widget kinds, sizes, system defaults, diff)
apps/workspace/src/i18n/ui_auth.rs · ui_base.rs · ui_shell.rs   (all Design copy, pt/en/fr)
apps/workspace/static/oc-*.css · oc-*.js · icons.svg · ocinye-logo.png
```

Rules the code already follows. Keep them when you wire:
- **CSP `style-src 'self'`**: no inline styles and no `style=` attributes. `assert_contracts` fails a view test if one appears.
- **Behaviour** comes only from static JS bound by `data-oc="…"` / `data-part="…"`. JS never decides authorisation and never stores member data (the one exception is the `sessionStorage` dock-hidden preference).
- **Everything works without JS.** JS only enhances: the launcher and palette open by `:target` anchors, and restoring the Desktop is a real `POST`.
- **Views only read view models.** `Option::None` means "Core did not answer". The view then shows an honest state and never an invented `0`. `Load<T>` = `Ready | Loading | Empty | Failed(ref) | Denied | Unavailable | Inactive`, and every view draws every branch.

## 4. Ownership

- **DESIGN owns** everything under `implementation/`: markup, CSS, static JS, i18n copy, the widget registry and view-model shapes. See `DESIGN_LOCK.md`.
- **CLAUDE CODE owns** routes, controllers, Core/BFF calls, filling the view models, sessions, RBAC, persistence, tests beyond the view tests, and deletion of obsolete UI.
- Changing a view model's **shape** is a Design decision. If the Core cannot supply a field, map it in the route and record the gap. Do not reshape the view.

## 5. How a route uses the UI

```rust
use ocinye_workspace::ui::{document, screens, view_models::*};
let vm: DesktopVm = /* filled from the Core */;
let html = document::render(
    &DocumentVm { title: t("desk.title").into(), surface: Surface::Shell, theme: Theme::Light },
    screens::home::home(&vm),
);
```
Full route → view → view-model tables: `implementation/docs/ui/HANDOFF.md` (sections "Rota → vista → ViewModel" for P1, P2 and P2.3, plus "11a · O que o BFF preenche"). That document is the detailed contract log and ships in this ZIP. §7 below summarises it.

**Two server behaviours are required** for auth to work:
1. A refused `POST /login` must render `login(&LoginVm { error, email, .. })`, not the plain text `invalid_credentials`.
2. `POST /mfa/confirm` must render `codes(&MfaCodesVm)`.

## 6. Canonical terminology (binding)

- **Ocinye** = the company. **Ocinye OS** = the product. **Ocinye Instance** = one concrete installation. **Ocinye Distribution** = Research | Business | Personal | Education.
- The Distribution belongs to the Instance. A Profile belongs to the Member. The Personal Space is a Context. Runtime = Web | Desktop | Dedicated.
- Never "Instance Profile". The Core still calls the distribution `profile` (`InstanceProfile`); convert it in the route (`view_models::Distribution`). This is marked `BACKEND_TERMINOLOGY_MIGRATION_REQUIRED`.
- Login copy (in `ui_auth.rs`, verified):
  - `auth.instance.operational` = «INSTÂNCIA OCINYE OS · OPERACIONAL» / «OCINYE OS INSTANCE · OPERATIONAL» / «INSTANCE OCINYE OS · OPÉRATIONNELLE»
  - `auth.secure_access` = «Acesso seguro à sua Instância Ocinye OS» / «Secure access to your Ocinye OS Instance» / «Accès sécurisé à votre instance Ocinye OS»
- Distribution badges show only `Research`, `Business`, `Personal`, `Education` (keys `dist.*`).
- «Instância» is capitalised in pt and en.

## 7. Decisions that previously existed only in conversation (now binding)

Login and access
- Login has two steps in the browser only, and is always a single `POST /login`. There is no request between the steps (no account oracle).
- The door shows **no** passkey ("Usar chave de acesso"), no SSO button and no "choose workspace" step. The Apps prototype still carries unused `passkey`/`sso`/`ws` strings: DO_NOT_IMPLEMENT.
- The door shows the logo, «OCINYE OS» and the distribution (code + name). It shows **no** Instance name or address.
- MFA setup (D8a) and recovery codes (D8b) use two columns. The QR is generated server-side (`qrcode`). The manual key appears only with `/mfa?show_key=1`.
- Password recovery (D10) is unavailable (G-26). The neutral confirmation is designed.

Context and the Desktop's global state
- **No context selector and no global active unit** (CLAUDE.md §34.3). This supersedes the reference wherever it still shows a unit, for example window titles «Ficheiros · UENR-001» or the terminal banner «contexto UENR-001»: do not render the unit there.

Desktop
- The grid has 4 columns and 16.333px rows with a 14px gap. A 1-row widget is 168px, a 2-row widget 350px, Indicators 107px. Maximum width 1480px.
- Every Desktop widget is glass (`.oc-dw`, `--dw-*` variables). On light wallpapers (mist, sand) the text turns dark automatically.
- There is no greeting. There is an `<h1>` for screen readers only. «Personalizar» is a round pencil in the corner.
- Nye is not a widget. It lives in the top bar and the floating panel.
- Widget subtitles are fixed registry text, never the unit.
- **Continue working**: shows everything the member can reach, since the Desktop has no active space. The member's own open/write events on idea/project/file/note/dataset. Up to 7 items, deduplicated, at most 30 days old. Meta is `TYPE · time`, projects `TYPE · 65% · time` (65% = done / (total − cancelled) tasks). Full rules: `implementation/docs/spec-desktop-widgets.md`.
- **System status**: visible to every member. The state is the worst of Core, compute nodes and backup (`HealthVm::derive_state`). Compute nodes are GPU/CPU nodes by heartbeat; with 0 nodes the segment is hidden. Backup times use the member's time zone. Only administrators get `admin_href` (the Monitor link).
- The system default is per Distribution (`registry::system_default`). There is no difference by account role, apart from admin-only widgets. At present there are none, since System status is for everyone.
- Restoring the default changes only the layout, never widget data. A 409 on `PUT /me/desktop` means another session saved first; the Desktop shows the conflict state.

Visual language
- Pill controls (radius = half the height), round icon buttons, cards 20px, the auth card 24px.
- One active navigation state: Ocinye blue background with white text, and no gold underline.
- One gold per screen: the main action.
- Top-bar logo: 32px circle (image 44px).

## 8. Integration order

Follow `APPLY_PLAN.md`. It is deterministic and needs no decisions from you.

## 9. Parity verification

1. `cargo clippy -p ocinye-workspace --all-targets -- -D warnings`, then `cargo test -p ocinye-workspace`. The view tests include CSP and a11y contracts and every-state rendering.
2. Open `reference/Ocinye OS.dc.html` in a browser (it runs as is, with `support.js` beside it). Compare against the running Workspace at the same viewport.
   - Research distribution, pt, light theme.
   - Use `?scene=1` for a clean Desktop. Add `&launcher=1`, `&user=1`, `&nye=1`, `&lock=1`, `&open=<app>`, `&open=<app>&max=1` or `&overview=1` for other states.
3. Screenshots in `reference/screenshots/` (index: `reference/screenshots/INDEX.json`, at 924×540) show the targets. Where code exists, it must match them. Where it does not, they are what later revisions deliver.

## 10. EXPORT_LIMITATIONS

| What is missing | Why | Exists in Design? | What Claude Code needs |
|---|---|---|---|
| Leptos implementation of the 29 applications (Work, Notes, Files, Calendar, Tasks, History, Resources, Mail, Messages, Units, Ideas, Projects, Datasets, Results, Knowledge, Bibliography, Nye, Ocinye AI, Agents, Compute, Activity, Monitor, Administration, Audit log, Settings, Help, Trash, Terminal, Browser) | Not yet authored as production code. They exist only as the interactive prototype. | Yes: `reference/Ocinye OS Apps.dc.html`, screenshots `app-*` | Nothing now. Serve `shell::app_pending` on those routes until D002+ delivers them. |
| Window preview `/preview/{kind}/{id}` (FG-011) | Not in D002 scope | Yes: reference | Later revision |
| Nye panel, floating bubble, voice | Not yet authored as production code | Yes: screenshot `desktop-nye-panel` | FG-020 |
| Lock screen (G-01), dock auto-hide (FG-007), launcher categories/favourites/recents (FG-008) | Not in D002 scope | Yes (reference) | Later revisions |
| Setup wizard (setup → distribution → apps → AI → initialise → ready) | Not yet authored as production code. The prototype enters it through an internal flow that can't be reached through a URL, so no screenshot exists. | Yes: `reference/Ocinye OS Apps.dc.html` (`screen` prop: `setup`, `setupProfile`, `setupApps`, `setupAi`, `setupInit`, `setupReady`) | Open the Apps prototype and set the `screen` prop, or wait for D002+ |
| Dark theme for the shell, Desktop and apps | Designed only inside the app prototype (`theme` prop). Not in production CSS apart from auth and a few shell rules. | Partly | MISSING_DESIGN_STATE for Desktop dark. The Desktop is wallpaper-driven glass. |
| Theme "system" | Never designed | No | MISSING_DESIGN_STATE. Map it to the OS preference only once designed. |
| Screenshots of Business, Personal and Education, EN/FR beyond login, dark, tablet and mobile | The capture tool renders one viewport (924×540). Other distributions need a prop change and weren't captured. | Yes, via the prototype props | Change the `distribution` prop in the reference and compare manually |
| Photo wallpaper | No upload contract exists. The sheet says it is not yet available. | Designed as unavailable | FG-003 |
| Font binaries | Not needed: IBM Plex already ships in the repository (`apps/workspace/static/fonts/`, with LICENSE) | n/a | none |
