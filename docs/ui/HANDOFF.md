# D003 · Nye — Search · Ask · Act

Base observed: branch `fix/wasmtime-rustsec-2026-0314` @ `55c3e22faf0054dc1d66e84764dbcfd3b0630571` (read from the local checkout). It is `origin/main` @ `b3cbc8e5b9939542f2f517b71b6202b50d36be98` plus one dependency commit (wasmtime 48.0.3, RUSTSEC-2026-0314/0315/0316). The reflog shows the checkout moved from `feat/design-d002` @ `56fb765` to `b3cbc8e`, and the working tree already contains `ui/wm`, `controllers/windows.rs` and the D002.1 switcher placement, so the D002.1 integration is on main. The D003 files were derived from this tree (the Rust files differ from the D002.1 package only by `cargo fmt`). Not reset to `56fb765`.

**One assistant, many agents, one authority: the Core.** Nye is the human-facing surface. It owns no authority, decides no permission, never shows model reasoning and never names a model or provider in the normal UI. Everything it shows is supplied by the Core (or the AI Fabric through the Core) through typed view models.

## 1 · Decisions

| Decision | Value | Why |
|---|---|---|
| `COMMAND_PALETTE_DECISION` | `EXTEND_INTO_NYE_UNIVERSAL_SURFACE` | One global command surface. The D001 palette becomes the Nye universal surface; with `ShellVm.nye = None` it is byte-for-byte the D001 palette. |
| `D001_COMPONENT_EXTENSION` | Command Palette → Nye Universal Surface | see §3 |
| `D002_COMPONENT_EXTENSION` | none to the window manager, top-bar layout or overlay z-indexes. Top bar: the Nye bar microphone becomes a link to `/ai/prompt?voice=1` only when `voice_input` is `Available`; otherwise the D001 disabled control. | minimal |
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

    .oc-desk (isolated: Desktop → windows → shelf/chooser)
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


# HANDOFF · UI completa do Ocinye OS (Claude Design)

Base: `chore/ui-wipe` @ `c99cbda`. Entrega por partes; o `apply.sh` é cumulativo e a última parte deixa a UI inteira.

## Partes
| Parte | Conteúdo | Estado |
|---|---|---|
| P0 | Fundação: documento, ViewModels comuns, componentes, `oc-base.css/js`, `icons.svg`, `i18n/ui_base.rs` | entregue · correcções do P0 incluídas (ver abaixo) |
| P1 | `screens/auth`: arranque, login, recuperar, fim de sessão, primeiro acesso, MFA (configurar, códigos, desafio) | entregue · compila, clippy limpo; 2 testes corrigidos |
| **P2** | `shell/` (barra de cima, barra de aplicações, lançador, paleta, «+ Criar», notificações, Nye, estado Core·IA, janela de aplicação) + `screens/home` | **esta entrega** |
| **P2.3** | Desktop do protótipo: widgets móveis e redimensionáveis, biblioteca, fundo, escurecimento, repor predefinição com diferenças e «Anular» (G-02/03/04) | **esta entrega** |
| P3 | settings, help, search, notifications | |
| P4 | files, notes | |
| P5 | mail, messages, calendar | |
| P6 | research, science, knowledge | |
| P7 | ai, terminal | |
| P8 | admin | |

### Correcções do P0 (pedidas)
1. `document.rs`: `r#as="font"` nas duas `<link rel="preload">`.
2. `ui/testing.rs::assert_contracts` passa a ser usado (testes do documento e de todas as vistas de P1).
3. `qrcode` só entra no `Cargo.toml` quando existe `ui/screens/auth/mfa.rs` (P1, que o usa).

## Ficheiros (P0 + P1)
- `src/ui/mod.rs`, `document.rs`, `view_models.rs`, `testing.rs`, `components/mod.rs`
- `src/ui/screens/mod.rs`, `screens/auth/{mod,login,first_access,mfa,boot}.rs`
- `src/i18n/ui_base.rs` (`UI_BASE`), `src/i18n/ui_auth.rs` (`UI_AUTH`)
- `static/oc-base.css`, `oc-base.js`, `oc-auth.css`, `oc-auth.js`, `icons.svg`, `ocinye-logo.png`

## Como uma rota usa a interface
```rust
let doc = DocumentVm { title: t("auth.login.title").into(), surface: Surface::Auth, theme: Theme::Dark };
Html(ui::document::render(&doc, ui::screens::auth::login::login(&vm)))
```

## Páginas da P1: rota → vista → ViewModel
| Rota | Vista | ViewModel | Formulários |
|---|---|---|---|
| `GET /boot` | `auth::boot::boot` | `BootVm` | — (ligações `/boot`, `/login`) |
| `GET /login` · `POST /login` recusado | `auth::login::login` | `LoginVm` | `POST /login {email, password}` · `POST /login/language {lang, return_to=/login}` |
| `GET /login?reason=expired\|revoked` | `auth::login::session_end` | `SessionEndVm` | `POST /login/language` |
| `GET /password/recover` | `auth::login::recover` | `RecoverVm` | `POST /password/recover {email}` (só com `available`) · `POST /login/language {return_to=/password/recover}` |
| `GET /first-access` · `POST` recusado | `auth::first_access::first_access` | `FirstAccessVm` | `POST /first-access {password, confirmation}` · `POST /logout` |
| `GET /mfa` (por configurar) | `auth::mfa::setup` | `MfaSetupVm` | `POST /mfa/confirm {code}` · `POST /logout` |
| resposta a `POST /mfa/confirm` | `auth::mfa::codes` | `MfaCodesVm` | `POST /mfa/acknowledge {acknowledged=1}` · `POST /logout` |
| `GET /mfa` (configurado) | `auth::mfa::challenge` | `MfaChallengeVm` | `POST /mfa/challenge {code}` · `POST /mfa/recovery {code}` · `POST /logout` |

Título do documento: `auth.login.title`, `auth.recover.title`, `auth.first.title`, `auth.mfa.setup_title`, `auth.mfa.codes_title`, `auth.mfa.challenge_title`, `auth.boot.*_title`, `auth.end.*_title`. `Surface::Auth`, `Theme::Dark`.

## ViewModels da P1
| Tipo | Campo | Tipo | Significado |
|---|---|---|---|
| `DoorVm` | `distribution` | `Option<Distribution>` | de `GET /api/v1/instance/branding` (`profile` no Core; converter na rota) |
| | `core` | `Option<Health>` | estado sondado neste pedido; `None` não mostra nada |
| `LoginVm` | `door` · `error` · `email` | `DoorVm` · `Option<String>` · `String` | a mensagem do Core (igual para todas as falhas) e o endereço escrito, para o preservar |
| `RecoverVm` | `door` · `available` · `sent` | `DoorVm` · `bool` · `bool` | `available=false` enquanto G-26; `sent=true` mostra a confirmação neutra |
| `SessionEndVm` | `door` · `reason` | `DoorVm` · `SessionEndReason` | `Expired` (cookie desconhecido) ou `Revoked` (G-27) |
| `FirstAccessVm` | `door` · `display_name` · `email` · `min_length` · `error` | … · `u32` · `Option<String>` | `min_length` do Core |
| `MfaSetupVm` | `door` · `otpauth_uri` · `manual_key` · `error` | … | o QR é gerado no servidor (`qrcode`); nunca vai para a URL |
| `MfaCodesVm` | `door` · `codes` | `Vec<String>` | só na resposta a `/mfa/confirm` |
| `MfaChallengeVm` | `door` · `error` · `recovery_open` | … · `bool` | `recovery_open` abre a secção de código de recuperação (após erro nela) |
| `BootVm` | `door` · `state` · `components` · `reference` | … · `BootState` · `Vec<BootComponent>` · `Option<String>` | `BootComponent { name, health, note }`, já traduzidos |

## Comportamento JS
| `data-oc` / `data-part` | Ficheiro | O que faz |
|---|---|---|
| `clock` | `oc-base.js` | «SEGUNDA-FEIRA, 28/09 · 12:20» no idioma do `<html lang>` |
| `copy` + `data-copy-target` | `oc-base.js` | copia o texto; marca `data-copied` 1,6 s |
| `login-steps`, `login-next`, `login-change`, `step-id`, `step-pw`, `login-email`, `login-initial` | `oc-auth.js` | endereço → palavra-passe no browser; um único `POST /login`. Se a palavra-passe chegar preenchida (gestor) ou houver erro com endereço, abre no passo 2 |
| `otp`, `otp-input`, `otp-cell` | `oc-auth.js` | espelha o campo único `name="code"` nas seis células e realça a seguinte |
| `reveal` + `data-oc-target`, `data-label-show/hide`, `reveal-label` | `oc-auth.js` | mostra/oculta a palavra-passe nova (só no primeiro acesso) |
| `save-codes` + `data-codes`, `data-filename` | `oc-auth.js` | descarrega os códigos num `.txt` local (nada sai do browser) |

## Estados desenhados (P1)
| Página | Estados |
|---|---|
| login | normal · erro do Core (mensagem + endereço preservado) · Instância em baixo (faixa, entrar desactivado) |
| recuperar | indisponível (G-26: campo e botão desactivados, razão por `aria-describedby`) · confirmação neutra (`sent`) |
| fim de sessão | expirada · revogada (sem formulário de entrada) |
| primeiro acesso | normal · erro de validação do Core |
| MFA | configurar (QR ou falha do QR → chave manual) · erro de código · códigos (confirmação obrigatória) · desafio · recuperação |
| arranque | a arrancar · pronta · parada (componente + referência) |

## Lacunas
- **G-26** recuperar palavra-passe: `RecoverVm.available=false`; a confirmação neutra está desenhada.
- **G-27** motivo da revogação: `SessionEndReason::Revoked` mostra texto genérico, sem motivo.
- Chave de acesso e SSO: fora do login (decisão do Fidel).

## P2 · casca e Home
### Ficheiros
`src/ui/shell/mod.rs` (`shell`, `app_window`), `src/ui/screens/home/mod.rs` (`home`), `src/i18n/ui_shell.rs` (`UI_SHELL`), `static/oc-shell.css`, `static/oc-shell.js`, `components::app_icon(href)`.

### Rota → vista → ViewModel
| Rota | Vista | ViewModel |
|---|---|---|
| `GET /` | `screens::home::home` | `HomeVm { shell, greeting, widgets }` |
| todas as páginas autenticadas (P3–P8) | `shell::shell(&ShellVm, main)` + `shell::app_window(title, href, body)` | `ShellVm` |

Documento: `Surface::Shell`, `Theme` do membro.

### ViewModels
| Tipo | Campo | Tipo | Significado |
|---|---|---|---|
| `ShellVm` | `display_name`, `email` | `String` | do membro |
| | `contexts` | `Vec<ContextItem{name, kind, href, current}>` | selector de espaço; vazio = sem selector. Nunca contém distribuições |
| | `apps` | `Vec<AppTile{href, label, description, pinned, active}>` | `experience::apps::visible_to` + `pinned_visible`; `label`/`description` de `name_key`/`description_key`; o ícone sai de `app_icon(href)` |
| | `unread` | `Option<u32>` | `None` = Core não respondeu (sem número) |
| | `core`, `ai` | `Option<Health>` | G-09; `ai = Unavailable` = sem nó |
| | `privileged` | `bool` | faixa «Sessão privilegiada» |
| | `query` | `String` | preserva a pesquisa |
| `HomeVm` | `greeting` | `String` | «Boa tarde, Fidel», já traduzida |
| | `widgets` | `Vec<Widget{title, href, items: Load<Vec<WidgetItem{title, meta, href}>>}>` | pela ordem da distribuição |

### Formulários e ligações
`GET /search?q=` (barra e paleta) · `POST /logout` · `/settings`, `/settings/language`, `/help`, `/notifications`, `/activity` (estado), `/ask` (Nye) · «+ Criar»: `/tasks/new`, `/ideas/new`, `/projects/new`, `/calendar/events/new`, `/mail/compose`, `/bibliography/new`, `/datasets/new` · contextos: `/workspaces/{id}`.

### Comportamento JS (oc-shell.js)
| `data-oc` / `data-part` | O que faz |
|---|---|
| `menu` | um `<details>` aberto de cada vez; fecha fora e com Esc |
| `launcher`, `launcher-open`, `launcher-q`, `launcher-item`, `launcher-empty` | abre (⌘J ou botão), filtra por `data-search` (pt/en/fr já no texto), Esc fecha. Sem JS: `#oc-launcher` + `:target` |
| `palette`, `palette-q`, `palette-item` | ⌘K; filtra; Enter pesquisa em `/search`. Sem JS: `#oc-palette` |
| `dock`, `dock-toggle` | esconder/mostrar a barra (duplo clique esconde); preferência visual em `sessionStorage` |

### Estados
Casca: contador só com `unread: Some(n>0)`; estado CORE·IA só com sonda; faixa privilegiada. Home: cada widget desenha `Ready` · `Empty` · `Failed` (referência) · `Denied` · `Unavailable` · `Inactive`.

### Lacunas (P2)
- **G-01** bloquear ecrã: item do menu com `aria-disabled` e a razão.
- **G-02/03/04** Desktop e widgets persistentes: disposição fixa; «Personalizar» desactivado com a razão.
- **G-05** janelas: as aplicações abrem como página inteira dentro da área de trabalho (`app_window`).
- **G-06** recentes: não mostrados.
- **G-07** voz do Nye e **G-08** monitor de actividade: não aparecem na casca.
- **G-09** estado CORE·IA: mostrado só se a rota sondar.
- Fixar/desafixar: **não é lacuna** — `PUT /apps/pins` (JS, `data-oc="pin"`); sem JS, ligação para `/settings/apps`.

### Correcções da P2.1
1. `shell/mod.rs`: `r##"href="#oc-launcher""##`.
2. Login: `data-oc="login-change"` tem `data-part="step-pw"`, fica `hidden` até ao passo 2 (o JS só o mostra em `data-step="pw"`) e `.oc-auth__who[hidden]{display:none}`.
3. MFA (ADR-0107): sem `manual_key`, «Mostrar chave manual» é uma ligação para `/mfa?show_key=1`; sem `<details>` nem `<code>` no documento. Com `manual_key`, a chave e «Copiar».

### Fixar aplicações
`AppTile` ganhou `id: &'static str` (`ApplicationId`) e `pinnable: bool` (`Application::can_pin`). Cada tile fixável tem `<button data-oc="pin" data-app=id aria-pressed>`. O JS envia `PUT /apps/pins` com `{"pinned": [ids fixados]}` e recarrega; em falha repõe o estado. Sem JS: «Gerir aplicações fixadas» → `/settings/apps`.

### Selector de espaço
Navegação pura: `ContextItem.current` só é `true` quando a página é `/workspaces/{id}` desse contexto; a vista não guarda nem sugere um espaço activo global (CLAUDE.md §34.3). Nas outras páginas o botão mostra só o ícone e «Espaço de trabalho».

### Saudação
`home::defaults::greeting_key(hora_local)`: 05–11 `home.greeting.morning` («Bom dia, {name}»), 12–19 `.afternoon` («Boa tarde, {name}»), 20–04 `.evening` («Boa noite, {name}»). Hora da Instância; `{name}` = primeiro nome.

### Widgets
`home::defaults::widgets_for(Distribution) -> &[WidgetSpec{kind, title_key, href}]`.

| Distribuição | Widgets, por ordem |
|---|---|
| Research | Tasks · Projects · Ideas · Notes · Files · Datasets · Activity |
| Business | Tasks · Mail · Calendar · Files · Projects · Messages · Activity |
| Education | Calendar · Tasks · Notes · Files · Messages |
| Personal | Notes · Tasks · Files · Calendar · Mail |

| Widget | Título / «ver tudo» | Dados (até 5) | `title` | `meta` | `href` |
|---|---|---|---|---|---|
| Tasks | `home.w.tasks` · `/my-work` | tarefas atribuídas ao membro, abertas, por prazo | título | prazo `dd/mm` (ou sem prazo) | `/tasks/{id}` |
| Projects | `home.w.projects` · `/projects` | projectos em que participa, por actualização | nome | código · estado | `/projects/{id}` |
| Ideas | `home.w.ideas` · `/ideas` | ideias do contexto, mais recentes | título | código · estado | `/ideas/{id}` |
| Notes | `home.w.notes` · `/notes` | notas do membro, por edição | título | `hh:mm` hoje ou `dd/mm` | `/notes/{id}` |
| Files | `home.w.files` · `/files` | ficheiros alterados pelo membro | nome | tipo · tamanho | `/files/{id}` |
| Datasets | `home.w.datasets` · `/datasets` | datasets do contexto, recentes | nome | formato · tamanho | `/datasets/{id}` |
| Mail | `home.w.mail` · `/mail` | mensagens por ler (Inbox) | assunto | remetente · `hh:mm` | `/mail/message/{id}` |
| Calendar | `home.w.calendar` · `/calendar` | eventos de hoje, por hora | título | `hh:mm` · local | `/calendar/events/{id}` |
| Messages | `home.w.messages` · `/messages` | conversas com actividade recente | nome da conversa | `hh:mm` da última | `/messages/{conversation}` |
| Activity | `home.w.activity` · `/activity` | acontecimentos recentes do contexto | descrição | «há X» | ligação ao objecto |

Correio sem conta configurada → `Load::Unavailable`; aplicação inactiva na Instância → `Load::Inactive`; lista vazia → `Load::Empty`; o Core não respondeu → `Load::Failed`.

### Tempo relativo (Actividade e outros «há X»)
Chaves em `ui_base`, via `tp`: `time.now` (< 1 min) · `time.minutes` (< 60 min) · `time.hours` (hoje, < 24 h) · `time.yesterday` · `time.days` (2–6 dias) · a partir de 7 dias, `dd/mm`.

### Metas ajustados à ligação (aceites pelo Design)
Projectos/Ideias: meta = código, ligação `/workspaces/{id}`. Datasets: código · estado, por código. Calendário: só a hora. Ficheiros pessoais: ligação `/files`. Actividade: tempo relativo acima.

## P2.3 · Desktop (G-02/03/04)
### Ficheiros
`ui/screens/home/mod.rs` (`home(&DesktopVm)`), `ui/screens/home/registry.rs` (registo de widgets, predefinições do sistema, `diff`, `greeting_key`; substitui `defaults.rs`), `static/oc-desk.css`, `static/oc-desk.js` (carregados na superfície `Shell`), chaves `desk.*` em `ui_shell.rs`. A casca ganhou `ShellVm.wallpaper` e `ShellVm.dim` (o fundo vale em todas as páginas).

### O contrato proposto: o que muda
O protótipo **não posiciona por x,y**: é uma grelha de 4 colunas em que a ordem é a posição e cada widget tem um tamanho de entre os permitidos do seu tipo. Também **não tem densidade nem widgets minimizados**; tem **escurecimento** do fundo (0–60 %). Proposta:

```
GET /me/desktop → 200
{
  "version": 7,                       // concorrência optimista
  "wallpaper": "org",                 // ocinye|dusk|org|mist|slate|sand
  "fit": "fill",                      // reservado para «photo» (ainda sem contrato)
  "dim": 20,                          // 0–60, passos de 5
  "widgets": [ { "id": "tasks", "kind": "tasks", "w": 1, "h": 2 }, … ],   // ordem = posição
  "base_version": 4,                  // versão da predefinição de onde veio (null = sistema)
  "can_customise": true,              // política da Instância
  "default": {                        // a predefinição publicada para a distribuição (ou null)
    "name": "Research Desktop Default", "version": 5, "published_at": "2026-09-27",
    "wallpaper": "org", "dim": 20, "widgets": [ … ]
  }
}

PUT /me/desktop {version, wallpaper, fit, dim, widgets} → 200 {mesma forma que o GET}
  409 se version ≠ actual · 422 se inválido (tipo desconhecido, tamanho não permitido,
  obrigatório em falta, admin_only sem papel, id repetido, dim fora de 0–60)
POST /me/desktop/restore → copia a predefinição para o membro; 200 {GET} com
  Accept: application/json, 303 → / sem JS. base_version passa a default.version.
```
- Retirar do contrato: `x`, `y`, `minimized`, `density`. Acrescentar: `dim`, `base_version`, `can_customise`, `default`.
- Validação no Core com a mesma tabela de `registry::KINDS` (tamanhos, `mandatory`, `admin_only`); se preferir, mova-a para `ocinye-contracts` e eu importo-a na vista.
- «Anular» depois de repor não precisa de rota: o JS guarda a disposição anterior e faz `PUT`.
- Os dados de cada widget não fazem parte deste contrato: a rota de `/` pede-os ao Core por tipo (tabela «Widgets» acima; `kpis`, `notice`, `storage`, `health`, `continue` são novos).

### Rota → vista → ViewModel
`GET /` → `screens::home::home` → `DesktopVm`. **Substitui** `HomeVm` e `Widget` da P2.

| Campo | Tipo | Significado |
|---|---|---|
| `shell` | `ShellVm` | inclui `wallpaper: Wallpaper` e `dim: u8` |
| `greeting` | `String` | `tf(registry::greeting_key(hora), {name})` |
| `version` | `u32` | de `GET /me/desktop` |
| `widgets` | `Vec<DeskWidget{placed: PlacedWidget{id, kind, w, h}, content: WidgetContent}>` | pela ordem do contrato |
| `default` | `Option<DesktopDefault{name, version, published, wallpaper, dim, widgets}>` | `published` já formatada |
| `base_version` | `Option<u32>` | aviso «nova predefinição» quando `default.version > base_version` |
| `is_admin` | `bool` | mostra `health` na biblioteca |
| `can_customise` | `bool` | `false`: «Personalizar» desactivado com a razão; sem folhas |

`WidgetContent`: `List(Load<Vec<WidgetItem>>)` (tasks, calendar, notes, files, mail, activity, projects, ideas, datasets, notice, continue) · `Metrics(Load<Vec<Metric{label, value, href}>>)` (kpis, health) · `Storage(Load<StorageUse{used, total, percent}>)` · `Nye` (sem dados; formulário `GET /ask?q=`).

Predefinição do sistema por distribuição: `registry::system_default(Distribution)` (usar quando o administrador ainda não publicou).

### Comportamento JS (oc-desk.js)
| `data-oc` | O que faz |
|---|---|
| `desk` (+ `data-version`) | raiz; `data-editing` em edição |
| `desk-edit`, `desk-edit-done` | entra/sai de Personalizar (Esc também sai) |
| `dw-left`, `dw-right` | move por teclado; arrastar com o rato faz o mesmo |
| `dw-resize` | percorre `data-sizes` do widget |
| `dw-remove` | retira (não existe nos obrigatórios) |
| `desk-lib-open`, `lib-cat`, `lib-add` | biblioteca: categorias, pesquisa, acrescentar (grava e recarrega para trazer os dados) |
| `desk-bg-open`, `bg-wall`, `bg-dim` | fundo e escurecimento em directo (`data-wall`, `--oc-dim` por CSSOM) |
| `desk-restore-open`, `desk-restore-form` | folha com as diferenças; `POST /me/desktop/restore` |
| `desk-undo` | «Anular» durante 10 s depois de repor |
| `dialog-close` | fecha a folha |
Cada alteração grava com `PUT /me/desktop` (450 ms de espera); estado em `desk-status` (a guardar · guardado · falhou · conflito).

### Estados
Cada widget: `Ready` · `Empty` · `Failed` (referência) · `Denied` · `Unavailable` · `Inactive`. Política fechada: sem Personalizar. Nova predefinição publicada: aviso com «Ver alterações». Repor sem diferenças: botão desactivado e «já está igual». Conflito 409: mensagem para recarregar.

### Lacunas que ficam
- Fundo com fotografia própria: precisa de contrato de carregamento; a folha diz que ainda não está disponível.
- G-05 janelas: entrega seguinte.

## P2.5 · Desktop igual ao protótipo
1. **Nye fora da grelha.** No protótipo a Nye vive na barra de cima (e na janela flutuante), não é um widget. `WidgetKind::Nye`, `WidgetContent::Nye` e a categoria IA saem; `system_default` já não a inclui. Se o Core guardar `kind: "nye"`, trate-o como desconhecido (422) ou retire-o na migração.
2. **Indicadores.** `Metric { icon, label, value, qualifier, href }`. Os quatro, por ordem, estão em `registry::KPIS`: (título, qualificativo, ícone, rota) — Unidades·activas·`/units`, Ideias·em investigação·`/ideas`, Projectos·em execução·`/projects`, Datasets·catalogados·`/datasets`. O qualificativo concorda com o número: `tp(qualifier_key, n)` (`desk.kpi.*_q.one/.other`). Números por estado vêm do Core; sem resposta, `Load::Failed`.
3. **Linhas compactas.** Grelha de `16.333px` com 14px de intervalo: widget de 1 linha = 168px, de 2 = 350px, Indicadores = 107px; largura máxima 1480px.
4. **Barra de cima.** Logótipo (menu da conta) · distintivo da distribuição (código em ouro, com o painel «DISTRIBUIÇÃO DA INSTÂNCIA» e «Definido pela administração da Instância.») · selector de contexto · trilho «/ {crumb}» · barra da Nye ao centro (`GET /ask?q=`) com microfone (G-07: `aria-disabled` e razão) e ⌘K · «Criar» · CORE·IA · notificações · data curta «Seg 28 set» + «15:17» (`data-oc="clock" data-format="short"`). `ShellVm` ganha `distribution: Option<Distribution>` e `crumb: String` (ex.: `t("apps.home")`).
5. **Sem saudação.** `DesktopVm.greeting` sai; há um `<h1>` só para leitores de ecrã. «Personalizar» é um lápis redondo no canto (opacidade .35, 1 com foco/rato).
6. **Listas e contadores** como no protótipo: uma linha por item (título à esquerda, meta em mono à direita, traço fino entre linhas); Tarefas com caixa (decorativa), Actividade/Correio/Projectos com ponto de cor, Calendário com hora à esquerda e faixa de cor, Avisos em caixa. Contadores (`WidgetContent::Count(Load<Count{value, qualifier}>)`): Ideias («12 em investigação»), Datasets («23 catalogados»), Projectos a 1 coluna («6 em execução»), Estado do sistema («OK · Core · 4 nós · cópia 03:00»). Armazenamento: «155 GB de 250 GB» com barra. Projectos a 2 colunas = lista.
- `DeskWidget.subtitle: Option<String>`: a linha mono por baixo do título («UENR-001 · ATRIBUÍDAS A MIM»).

## P2.6 · sem selector de contexto; recolher; o que falta para «igual ao protótipo»
### Feito nesta entrega (protótipo e código)
- **Selector de contexto retirado** da barra de cima, da barra lateral do protótipo, do título das janelas e do painel da Nye (CLAUDE.md §34.3). `ContextItem` e `ShellVm.contexts` saem.
- **Subtítulos sem unidade**: Tarefas «ATRIBUÍDAS A MIM», Avisos «ORGANIZAÇÃO · OBRIGATÓRIO», Estado do sistema «INSTÂNCIA»; os restantes sem subtítulo.
- **Recolher/expandir** em cada widget (`data-oc="dw-min"`, sempre disponível, `aria-expanded`), como no protótipo: recolhido ocupa 2 linhas finas e mostra só o cabeçalho. **Correcção ao contrato do Core**: `minimized: bool` volta a `widgets[]` em `GET/PUT /me/desktop` (eu tinha-o retirado por engano; o protótipo tem-no).
- **Faixa de sessão privilegiada retirada** (não existe no protótipo); `ShellVm.privileged` sai.

### Ainda não igual ao protótipo (entregas seguintes, cumulativas)
1. **Casca, passagem de fidelidade**: barra de aplicações, lançador e paleta ao pormenor do protótipo; menu da conta com «Conta», «Definições», «Aparência do Desktop», «Ajuda», «Bloquear ecrã ⌘L» (G-01), «Terminar sessão»; painéis de notificações, estado CORE·IA e relógio.
2. **Widgets**: Calendário da semana (o protótipo tem a vista da semana com cores); Continuar trabalho com o tipo de cada item; estado «sem permissão» desenhado (o protótipo mostra-o em Datasets).
3. **Predefinições por conta** (superadmin, admin, membro): o protótipo tem uma predefinição por distribuição e só o Estado do sistema é de administração; confirmo a tabela no HANDOFF quando entregar.
4. **Janelas de aplicação (G-05)** e **todas as aplicações do lançador** (P3–P8).

## P2.7
- **Subtítulos**: texto fixo do registo (`KindSpec.subtitle_key`), não dados. Tarefas `desk.sub.tasks` «ATRIBUÍDAS A MIM»; Avisos `desk.sub.notice` «ORGANIZAÇÃO · OBRIGATÓRIO»; Estado do sistema `desk.sub.health` «INSTÂNCIA». Os outros não têm. `DeskWidget.subtitle` sai (a rota não o preenche).
- **Teste do lápis** verifica `oc-desk-pencil` e `data-oc="desk-edit"` em separado.
- **Menu da conta** como no protótipo: iniciais em ouro, nome, endereço, «OCINYE OS · RESEARCH»; Conta · Definições · Aparência do Desktop (abre a folha Fundo; fora do Desktop vai a `/#appearance`) · Ajuda · Bloquear ecrã ⌘L (G-01, desactivado com a razão) · Terminar sessão. Menus em vidro claro (raio 20px, itens em cápsula).
- **Sem permissão** (`Load::Denied`): caixa tracejada com cadeado e «Sem permissão para ver este conteúdo. Peça acesso ao coordenador.» (`desk.denied`).
- **Calendário e Continuar trabalho**: o desenho já é o do protótipo (hora à esquerda e faixa de cor; título e meta). O «tipo» de Continuar trabalho vem no `meta` («IDEIA · 2 h», «65%», «1 d»), como no protótipo.
- **Predefinições por conta**: no protótipo não há diferença entre superadmin, admin e membro na predefinição; a única diferença é que o Estado do sistema só existe na biblioteca para administradores (`DesktopVm.is_admin`). A predefinição é por distribuição (`system_default`).

## Contratos alterados
- `HomeVm` e `Widget` **substituídos** por `DesktopVm`, `DeskWidget`, `PlacedWidget`, `WidgetContent`, `Metric`, `StorageUse`, `DesktopDefault`, `Wallpaper`, `WidgetKind` (P2.3). `home::defaults` passa a `home::registry` (`greeting_key` mantém-se; `widgets_for` → `system_default`).
- `ShellVm`: novos `wallpaper` e `dim`; P2.5: `distribution` e `crumb`.
- P2.7: `DeskWidget.subtitle` sai (subtítulos fixos no registo).
- P2.6: `PlacedWidget.minimized` entra; `ContextItem`, `ShellVm.contexts` e `ShellVm.privileged` saem.
- P2.5: `DesktopVm.greeting` sai; `DeskWidget.subtitle` entra; `Metric` ganha `icon` e `qualifier`; novo `Count` e `WidgetContent::Count`; `WidgetKind::Nye` e `WidgetContent::Nye` saem.
- `AppTile`: **novos campos** `id: &'static str` e `pinnable: bool` (P2.1), para fixar a sério.
**O servidor tem de mudar dois comportamentos para a P1 funcionar:** `POST /login` recusado deve renderizar `login(&LoginVm { error, email, .. })` em vez de responder texto `invalid_credentials`; `POST /mfa/confirm` deve renderizar `codes(&MfaCodesVm)`.

## Por decidir
- Os estados «INSTÂNCIA OCINYE OS · DEGRADADA / INDISPONÍVEL» seguem a fórmula aprovada para «OPERACIONAL»; confirmar.
- `min_length` do primeiro acesso: vem do Core (`minimum_password_length`, hoje 15); a vista não o fixa.


## 11a · Desktop: fecho dos widgets (cumulativo sobre c99cbda)

Regras e chaves: `docs/spec-desktop-widgets.md`. Contratos alterados em `view_models.rs`:

- `Load<T>`: **novo ramo `Loading`** (bloco ainda à espera do Core). Todos os widgets o desenham (esqueleto, `aria-busy`).
- `WidgetContent`: **novos ramos** `Continue(Load<Vec<ContinueItem>>)` e `Health(Load<HealthVm>)`. «Continuar trabalho» e «Estado do sistema» deixam de usar `List`/`Count` com strings: a vista compõe a meta e a linha a partir de dados tipados.
- Novos tipos: `ContinueKind`, `Ago`, `ContinueItem`, `Backup`, `HealthVm`. O estado usa o `Health` já existente (`Operational`/`Degraded`/`Unavailable`).
- Registo: `Health` deixa de ser `admin_only` (todos o vêem) e deixa de ter «Ver tudo»; o link para o Monitor é `HealthVm::admin_href`, só para administradores.

### O que o BFF preenche

**ContinueItem** (até 7, por último toque desc.)
| campo | tipo | origem |
|---|---|---|
| kind | ContinueKind (`idea`/`project`/`file`/`note`/`dataset`) | tipo do objecto |
| title | String | nome do objecto |
| href | String | ficha/editor; sem a app no perfil: `/preview/{kind}/{id}` (G-05, 11d) |
| progress | Option<u8> | só `project`: round(100 × concluídas / (total − canceladas)); `None` sem tarefas |
| when | Ago | `Ago::from_secs(agora − último toque, || dd/mm no idioma)` |

Estados: `Loading`, `Empty` (texto próprio `desk.cont.empty`), `Failed(ref)`, `Unavailable` (registo de actividade desligado).

**HealthVm**
| campo | tipo | origem |
|---|---|---|
| state | Health | `HealthVm::derive_state(quorum, nodes_up, nodes_total, backup_fresh)` |
| nodes_up / nodes_total | u16 | nós do Core |
| backup | Backup | última cópia com êxito: `Today(hh:mm)` / `Yesterday(hh:mm)` / `Date(dd/mm)`; `Failed` se a última tentativa falhou; `Never` |
| admin_href | Option<String> | `Some("/admin/monitor")` só para administradores |

`backup_fresh` = última cópia com êxito há < 24 h e última tentativa não falhou. Core sem quórum mas o BFF responde: `Ready` com `state: Unavailable`. O BFF não chega ao Core: `Failed(ref)`.

### 11a.2 · respostas
- Âmbito de «Continuar trabalho»: tudo o que o membro alcança. Não há espaço activo no Desktop; `?unit_id=`/`?personal=true` ficam para as aplicações.
- Nós: são os de computação (batimento). `nodes_total = 0` esconde o segmento e não degrada o estado. `derive_state(core_ok, …)` (antes `quorum`).
- Hora da cópia: zona do membro.
- Teste do Estado do sistema: verifica a classe e o estado em separado (como em 379b6c5).


## 14 · Inventário do Desktop

Tudo o que o Desktop do protótipo mostra ou faz, contra o código. **Feito** = no código e ligado. **14a** = este pacote. **14b–14e** = fatias seguintes, por esta ordem. **P3–P8** = aplicações.

Fatias: **14b** Administração: avisos e publicar a predefinição do Desktop · **14c** casca ao pormenor (barra de cima, barra de aplicações, lançador, menu de contexto, bloqueio) · **14d** janelas G-05 e pré-visualização · **14e** Nye (painel, bolha, voz).

### Barra de cima
| elemento | estado | dados do Core/BFF |
|---|---|---|
| Logótipo → menu da conta (Conta, Definições, Aparência, Ajuda, Bloquear ⌘L, Terminar sessão) | feito (Bloquear: 14c) | `ShellVm.display_name`, `email`, `distribution` |
| Distintivo da distribuição + painel | feito | `ShellVm.distribution` |
| Trilho «/ {aplicação}» | feito; com janelas, mostra a janela da frente: 14d | `ShellVm.crumb` |
| Barra da Nye (anexar, microfone, enviar, ⌘K) | feito como `GET /ask?q=`; painel no sítio: 14e; microfone G-07 | contrato da Nye (14e) |
| «+ Criar» com atalho por item | feito; atalhos por item: 14c | rotas `/…/new` |
| CORE · IA | feito | `ShellVm.core`, `ai` |
| Painel «Estado do sistema» (Core: versão, Instância, serviços; IA: estado, descrição, meta; «Ver opções de IA», «Estado detalhado») | 14c | **novo** `StatusVm { core_version, instance_code, services_active: Option<u32>, ai: Health, ai_detail: Option<String> }` |
| Notificações: botão e contador | feito | `ShellVm.unread` |
| Painel das notificações (lista, por ler, «Marcar todas como lidas», «Ver todas») | 14c | **novo** `GET /notifications?limit=6` → `[{id, icon, title, body, at, read, href}]`; `POST /notifications/read-all`; `POST /notifications/{id}/read` |
| Relógio «Seg 28 set · 15:17» | feito | — (browser) |
| Painel do relógio (dia por extenso, mês, «Abrir Calendário») | 14c | — (browser) |

### Barra de aplicações
| elemento | estado | dados |
|---|---|---|
| Home / «Mostrar Desktop» | feito como link; minimizar todas: 14d | — |
| Lançador | feito | `ShellVm.apps` |
| «Todas as janelas» | 14d | — (cliente) |
| Aplicações fixadas, activa | feito | `AppTile.pinned`, `active` |
| Ponto «aberta» e janelas abertas não fixadas | 14d | — (cliente) |
| Lixo (vazio/cheio) | 14c botão; aplicação Lixo: P3 | **novo** `ShellVm.trash_count: Option<u32>` |
| Esconder sozinha após 5 s; botão para a trazer com n.º de janelas | esconder à mão: feito; automático e contador: 14c | — |
| Dicas ao passar o rato | 14c | — |

### Lançador
| elemento | estado | dados |
|---|---|---|
| Título, contagem, ESC, pesquisa ⌘J | feito (contagem e chips: 14c) | — |
| Separadores por categoria com contagem | 14c | **novo** `AppTile.category` (do manifesto) |
| Secções, descrições, estado «Aberto · 2 janelas» / «Várias janelas» | descrições feitas; resto 14c/14d | `AppTile.multi_window: bool` (manifesto) |
| Fixar/desafixar | feito (`PUT /apps/pins`) | — |
| Vazio / sem resultados / erro / sem permissão | sem resultados feito; resto 14c | `Load` em `ShellVm.apps` |

### Paleta
No protótipo, ⌘K abre o painel da Nye, que faz de paleta: sugestões, abrir aplicações, acções com confirmação e anular, pesquisa. O código tem uma paleta de aplicações com `GET /search`, que fica até à **14e**, quando o painel da Nye a substitui.

### Área do Desktop
| elemento | estado | dados |
|---|---|---|
| Grelha e os 14 widgets com dados reais | feito | 11a, P2.3–P2.7 |
| Fundo, motivo institucional, escurecimento | feito; fotografia: lacuna (contrato de carregamento) | `wallpaper`, `dim`, `fit` |
| Menu de contexto (botão direito): Adicionar widget, Mudar fundo, Todas as janelas, Organizar widgets, Repor disposição, Definições do Desktop | 14c | — |
| Aviso «nova predefinição» com Ver / Usar / Dispensar | «Ver» feito; «Usar» e «Dispensar»: 14c | **novo** `DesktopVm.dismissed_default: Option<u32>` e `POST /me/desktop/dismiss {version}` |
| Toast com «Anular» | feito | — |
| Bloqueio de ecrã (⌘L; hora, data, iniciais, palavra-passe, erro, Terminar sessão) | 14c (G-01) | **novo** `POST /session/lock` → 204; `POST /session/unlock {password}` → 204 / 401; sessão bloqueada responde `/lock` a qualquer rota |

### Personalizar
| elemento | estado |
|---|---|
| Entrar e sair, barra de edição | feito |
| Arrastar, ← →, tamanho, retirar (obrigatórios com cadeado), recolher | feito |
| Biblioteca: pesquisa, categorias, acrescentar | feito |
| Fundo e escurecimento | feito |
| Repor predefinição com diferenças e «Anular» | feito |
| A guardar / guardado / falhou / conflito 409 | feito |
| Política sem personalização | feito |
| **Publicar a predefinição** (administração) | não existe no protótipo; desenho na **14b** (Administração › Desktop predefinido) |

### Janelas (G-05) · 14d
Abrir (e várias janelas nas aplicações que o permitem), focar, mover, redimensionar, minimizar para a barra, maximizar e restaurar (também com duplo clique no título), ecrã inteiro (saída com Esc e botão que aparece no topo), fechar, encaixar nas bordas (metades, quartos, máximo) com pré-visualização, «Todas as janelas» (⌃↑ / F3), alternar (⌘/Ctrl/Alt+Tab), «Mostrar Desktop», abrir o Terminal com ⌃⇧`. Em tablet e telemóvel (< 1024 px) as janelas ocupam a área toda e não se arrastam. Pré-visualização `/preview/{kind}/{id}`. Contrato proposto: **qualquer rota de aplicação aceita `?frame=1`** e devolve só o corpo da janela (sem casca), com o título em `<template data-part="win-title">`. Sem JS, a mesma rota sem `frame` abre em página inteira (`app_window`, já existe). A disposição das janelas fica no browser (`sessionStorage`), não no Core. Pormenor em 14d.

### Nye · 14e
Painel (sugestões, mensagens por tipo: acção, armazenamento, resposta, política, recusa, confirmação, progresso, pesquisa; «Anular»; «Nova conversa»; abrir completa), bolha flutuante que se arrasta, voz (G-07). Contrato do BFF na 14e.

### Destinos dos widgets
Nenhum ecrã de destino é desta fatia. Até cada ecrã chegar, **a rota responde `shell::app_pending(&shell, título, rota)`** (novo na 14a): a janela com «Este ecrã ainda não está disponível nesta versão do Ocinye OS.» (`shell.app.pending`). Assim nenhuma ligação fica morta.

| widget | «Ver tudo» | itens | ecrã em |
|---|---|---|---|
| Indicadores | — | `/units`, `/ideas`, `/projects`, `/datasets` | P4 (investigação) |
| Avisos | `/notifications` | aviso | P3 |
| Continuar trabalho | `/files` | ficha/editor ou `/preview/{kind}/{id}` | ficheiros e notas P3; pré-visualização 14d; ideias/projectos/datasets P4 |
| Tarefas | `/my-work` | `/tasks/{id}` | P3 |
| Calendário | `/calendar` | `/calendar/events/{id}` | P5 |
| Notas | `/notes` | `/notes/{id}` | P3 |
| Ficheiros | `/files` | `/files/{id}` | P3 |
| Correio | `/mail` | `/mail/{id}` | P5 |
| Actividade | `/activity` | origem do evento | P3 |
| Projectos | `/projects` | `/projects/{id}` | P4 |
| Ideias | `/ideas` | — | P4 |
| Datasets | `/datasets` | — | P4 |
| Armazenamento | `/files` | — | P3 |
| Estado do sistema | — | `/admin/monitor` (só admin) | P8 (Monitor) |
| Menu da conta | `/account`, `/settings`, `/help` | — | P3 |
| Barra de cima | `/search`, `/notifications` | — | P3 |


## D001.1 · correcções de paridade e estados em falta
Contratos: `DesktopDefault.source` (`DefaultSource::System | Instance`), `Backup::Unknown`, `ErrorKind`/`ErrorVm`, `IdentityFailVm`. Novas vistas: `screens::error::{in_shell, at_door}` e `screens::auth::identity::identity_unavailable`. O registo de widgets não muda. Detalhe: HANDOFF.md do pacote D001.1, §0.

## D001.2 · fecho da paridade da D001
Só CSS (`oc-shell.css`, `oc-desk.css`): a pastilha CORE·IA; a margem de 30px sem a barra; 4 colunas só com a área ≥ 960px (a referência a 924 tem 2 colunas); títulos em até duas linhas; Indicadores 4 numa linha ou 2 + 2. Decisões: REGISTRY_IS_CANONICAL e IMPLEMENTATION_CANONICAL_REFERENCE_CORRECTED. Detalhe: HANDOFF.md do pacote, §00.

## D002 · janelas e interacções da casca
Ver HANDOFF.md do pacote D002 (§ D002): componentes, estados, contratos WM-1…WM-5, painéis e responsivo.


## D002.1 · correcção (paridade e acessibilidade)
- Alternador: desenhado por `shell_with_window` depois da paleta, fora do `.oc-desk` (`isolation: isolate`); o véu cobre e bloqueia a barra de cima. `wm::switcher` é público; `wm::layer` já não o desenha.
- Painéis fechados: `.oc-panel-menu > summary { display: flex }`; pastilha, sino e relógio nas posições da D001.2.1.
- Diálogo de alterações: só controlos focáveis reais (sem `<use href>`); Tab e Shift+Tab dão a volta; o foco volta ao controlo anterior quando o diálogo fecha sem navegar.
