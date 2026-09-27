# DESIGN_LOCK

A implementação visual canónica do Ocinye OS, escrita pelo Claude Design no próprio repositório. O que está `LOCKED` não se reestrutura, não se re-estiliza, não se lhe trocam ícones nem se «simplifica» a marcação. A integração funcional (dados, rotas, contratos, estado) faz-se **à volta** destas vistas.

- Base: `feat/runtime-r2` @ `c7988e8`
- Ramo de Design: `feat/design-ui-canonical`
- Revisão de Design: Proposta v2 (`design/claude-design/reference/Ocinye OS Proposta.dc.html`), 28 set 2026
- Fronteira: `apps/workspace/src/ui/screens/*.rs` (vista), `apps/workspace/static/ods-*.css` (estilo). Comportamento visual puro em `static/app.js` nos blocos marcados `DESIGN ·`.

## Fatia 1 · Autenticação

| Componente | Implementação | Referência | Estado | Locked |
|---|---|---|---|---|
| Moldura (barra, identidade, rodapé com idioma) | `ui/screens/login.rs` · `barra()`, `identidade()`, `rodape()` · `static/ods-d3-auth.css` | D7 | ligado | sim |
| Login | `login_na_porta(core_ready, message, &Porta)`; `login(…)` mantém a assinatura antiga | D7, P1–P4, E4 | ligado (`POST /login`) | sim |
| Esqueceu a palavra-passe? | ligação em `login_na_porta` → `/password/recover` | D7 | ligado à vista D10 | sim |
| Chave de acesso · SSO | `indisponivel()` em `login.rs`; SSO só com `InstanceProfile::Business` | D7, P2 | sem ADR · `aria-disabled` | sim |
| Seletor de idioma | `rodape()` · `POST /login/language` | D7 | G-30 · CONNECT | sim |
| Perfil + endereço | `identidade(&Porta)` | D7, P1–P4 | G-31 · CONNECT | sim |
| Core indisponível | `login_na_porta(false, …)` | D14 | ligado (sonda) | sim |
| Escolher espaço | `escolher_espaco(&[EspacoVista], &Porta)` | D9 | ligações `/workspaces/{id}`; «lembrar» sem contrato | sim |
| Recuperar palavra-passe | `recover(enviado, disponivel, &Porta)` | D10 | G-26 · `disponivel=false` até ao POST | sim |
| Sessão expirada · acesso revogado | `fim_de_sessao(FimDeSessao, &Porta, Option<QuemEstava>)` | D12, D13 | G-27 · `/login?reason=…` | sim |
| Segundo factor (seis caixas + um campo) | `ui/screens/mfa.rs` · `.ods-auth__otp` | D8 | ligado (`/mfa/*`) | sim |
| Primeiro acesso | `ui/screens/first_access.rs` | D11 | ligado | sim |
| Arranque | `ui/screens/boot.rs` | moldura D7 | ligado | sim |
| Alternador de palavra-passe | `static/app.js` · bloco `DESIGN · alternador` | D11 | visual puro | sim |

### Para o Claude Code ligar
- `routes.rs` `login`/`login_submit`: construir `Porta { nome, perfil, host }` a partir de `/api/v1/instance/branding` e do cabeçalho `Host`, e chamar `login_na_porta`.
- `GET /login?reason=expired|revoked` → `fim_de_sessao`.
- `GET /password/recover` → `recover(false, false, &porta)` até existir o POST.
- Depois do login, com mais de um espaço → `escolher_espaco`.
- `POST /login/language`: o botão submete `lang` e `return_to`.

### Diferenças conscientes face ao protótipo
- MFA: as seis caixas são desenho e o código entra num só `<input autocomplete="one-time-code">` por cima delas. Só CSS: o passo das caixas é o passo dos dígitos (`1ch + letter-spacing`). O realce da caixa seguinte, se se quiser, é comportamento (`app.js`); por agora, o conjunto fica dourado com foco.

## Fatias seguintes
2. Casca + Desktop + barra de aplicações + lançador — `shell.rs`, `home.rs`
3. Janelas
4. Nye
5. Terminal
6. Browser
