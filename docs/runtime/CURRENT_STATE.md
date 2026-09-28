# Runtimes — estado actual (R0)

> Factos verificados na árvore em 2026-09-27, no ramo `ui/claude-design`
> (D0–D13 aplicados). O Terminal (ocsh) vive em `feat/ocsh-terminal` e ainda
> não está neste ramo. Nada aqui é intenção: a arquitectura-alvo está em
> [`TARGET_ARCHITECTURE.md`](TARGET_ARCHITECTURE.md).

## Resumo

O Ocinye OS tem hoje **um** runtime: a Web. O Workspace é Leptos SSR com
melhoria progressiva (`app.js`), atrás de uma sessão BFF; não há PWA, casca
nativa, `ocinye://`, Gestor de Janelas no cliente, nem navegador integrado.

| Área | Estado | Evidência |
|---|---|---|
| Renderização | SSR (Leptos 0.8, `ssr`), sem WASM nem hidratação | `apps/workspace/src/ui/mod.rs:41-96` |
| Sessão | BFF em memória; o token do Core nunca sai do processo | `apps/workspace/src/session.rs:30,65-79` |
| Cookie de sessão | `ocinye_session`: `HttpOnly; SameSite=Lax; Secure` (forçado em produção) | `session.rs:189-197`, `config.rs:167-171` |
| CSRF | `same_origin_only` + `origin_is_ours` (Origin = URL pública) | `routes.rs:835,903` |
| CSP do Workspace | `default-src 'none'; script-src 'self'; style-src 'self'; font-src 'self'; img-src 'self' data:; connect-src 'self'; frame-src 'self'; form-action 'self'; base-uri 'none'; frame-ancestors 'none'` | `routes.rs:761-810` |
| Outros cabeçalhos | `X-Frame-Options: DENY`, `Referrer-Policy: same-origin`, `COOP: same-origin`, `Permissions-Policy: geolocation=(), microphone=(), camera=()`, `Cache-Control: no-store` em tudo, HSTS em produção; sem COEP | `routes.rs:761-810`; testes em `apps/workspace/tests/security_headers.rs` |
| CORS | só no Core, vazio por omissão (`OCINYE_CORS_ALLOWED_ORIGINS`); o Workspace não tem | `services/core-server/src/routes/mod.rs:161-178` |
| Estáticos | `/static/*` por `ServeDir`, **sem versão no URL** e com `no-store` | `routes.rs:611,799` |
| PWA | **ausente**: sem manifesto, service worker, `theme-color`; só `<link rel="icon">` | `ui/mod.rs:61` |
| Tempo real | Core serve `GET /api/v1/realtime` com *bearer*; o browser abre `/realtime` no host do Workspace, que **não tem essa rota** — ver «Achados» | `core-server/src/routes/realtime.rs:43`; `app.js:4368,5105` |
| SSE | ausente | — |
| Notificações | Core + sino (`/notifications/recent` ao abrir); sem Web Notifications nem Push | `app.js:161` |
| Ligações profundas | rotas HTTPS canónicas (`ROUTES`, `routes.rs:36-224`); `ocinye://` **ausente** | — |
| Registo de aplicações | `ApplicationManifest` sem campo de runtime | `crates/ocinye-contracts/src/application.rs:438-465` |
| Gestor de Janelas (D5) | **ausente** no código; `docs/ui/D5_WINDOWS.md` é proposta (G-05) | — |
| Desktop (D4) | layout fixo servido em `GET /` | `ui/screens/home.rs` |
| Nye | `/ask`, planos do Core agentic | `routes.rs:597`, `ui/screens/ask.rs` |
| Terminal / ocsh | em `feat/ocsh-terminal` (`283cacc`): `/terminal`, `/terminal/exec`, Core `POST /api/v1/commands/exec` | ADR-0312 |
| Ficheiros — carregar | sessões por partes (40 MiB por parte), preflight de quota, envio único até 640 MiB | `routes.rs:472-525,10932,11292` |
| Ficheiros — descarregar | same-origin, em stream, `Content-Disposition`; nunca URLs assinadas ao browser | ADR-0608; `modules/files/service.rs:1248` |
| Armazenamento | `ObjectStore` S3 (Garage) | `crates/ocinye-core/src/storage.rs` |
| Autenticação | Core: login, MFA obrigatório para privilegiados, sessões opacas *bearer* guardadas pela BFF | `core-server/src/routes/auth.rs:32-53` |
| Core público? | na produção da Ocinye, sim (`api.ocinye.com`); numa Instância instalada, **não** — só o Workspace | `infra/nginx/instance/30-instance.conf.template`; `docker-compose.production.yml:124-133` |
| Identidade pública | Core: `/health`, `/ready`, `/api/v1/instance/branding`, `/api/v1/instance/logo`; Workspace: só `/health` (`ok`) | `unauthenticated_sweep_http.rs:167-188` |
| Casca nativa | **ausente** (sem tauri/wry/electron no `Cargo.lock`) | — |
| Workspace Rust | 5 crates, 4 serviços, `apps/workspace`; `wasm/capabilities` fora | `Cargo.toml:3-14` |
| CI | um workflow: estático, rustsec, postura, testes (Chrome) | `.github/workflows/ci.yml` |
| Release e instalação | `scripts/release-bundle.sh` (não assinado) + `install/ocinye` (install/upgrade/rollback/backup) | ADR-0701 |
| Compatibilidade de browsers | E2E só em Chromium (`chromiumoxide`) | `apps/workspace/tests/browser.rs` |

## Achados

1. **O socket de tempo real do browser não tem destino.** `app.js` abre
   `ws(s)://<host do Workspace>/realtime`; o Workspace não tem a rota nem proxy
   de WebSocket, e o nginx da Instância só encaminha o Workspace. O tempo real
   das Mensagens e o aviso «nota alterada noutro sítio» não chegam a ligar.
   Defeito anterior a este programa; registado como tarefa própria.
2. **A identidade de uma Instância auto-instalada só se vê pelo Workspace.** O
   Core não é público nessas instalações; a casca Desktop tem de validar a
   Instância pelo host do Workspace (ADR-0704).
3. **Estáticos sem versão.** Com `no-store` não há cache antiga a servir, mas
   também não há forma de saber que o release mudou; «há uma nova versão» (D15
   G-17) precisa de um identificador de build.
4. `app.js` e a ADR-0413 citavam uma «ADR-0019» que não existe; corrigido para a ADR-0602 (SSR com melhoria progressiva) na R0.

## O que isto quer dizer para o programa

- A Web já cumpre quase tudo da garantia de acesso (ADR-0018); faltam a prova
  constitucional como viagem única, o manifesto PWA e o identificador de build.
- O Browser na Web só pode usar `iframe` com *sandbox* e recurso honesto; a CSP
  já tem `frame-src 'self'` e precisa de `https:` só na página do Browser.
- O Gestor de Janelas (D5) não existe: o Browser nasce como aplicação em página,
  como o Terminal, até G-05.
