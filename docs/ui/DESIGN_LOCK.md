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
| Segundo factor · configurar (QR), códigos, desafio | `ui/screens/mfa.rs` · `.ods-auth__otp` | D8, D8a, D8b | ligado (`/mfa/*`) · **só super admin** (sessão privilegiada, ADR-0107); membros não passam por estes ecrãs | sim |
| Primeiro acesso | `ui/screens/first_access.rs` | D11 | ligado | sim |
| Arranque | `ui/screens/boot.rs` | moldura D7 | ligado | sim |
| Alternador de palavra-passe | `static/app.js` · bloco `DESIGN · alternador` | D11 | visual puro | sim |

### Para o Claude Code ligar
- `Porta` ganhou `core: Option<bool>`: preencher com `boot::probe(...).state.may_hand_off()` em todos os ecrãs da família (se construíres `Porta` com literal, acrescenta o campo ou usa `..Default::default()`).
- MFA e primeiro acesso: `enrollment_na_porta`, `recovery_codes_na_porta`, `challenge_na_porta`, `first_access_na_porta` recebem `&Porta`. As funções antigas mantêm-se e delegam com `Porta::default()`.
- `routes.rs` `login`/`login_submit`: construir `Porta { nome, perfil, host }` a partir de `/api/v1/instance/branding` e do cabeçalho `Host`, e chamar `login_na_porta`.
- `GET /login?reason=expired|revoked` → `fim_de_sessao`.
- `GET /password/recover` → `recover(false, false, &porta)` até existir o POST.
- Depois do login, com mais de um espaço → `escolher_espaco`.
- `POST /login/language`: o botão submete `lang` e `return_to`.

### Diferenças conscientes face ao protótipo
- Rótulo do campo de identidade: «Endereço institucional» (`login.institutional_address`) e não «Endereço de correio ou utilizador». O Core só aceita o endereço (ADR-0106) e o campo é `type="email"`: prometer «ou utilizador» levaria a uma recusa sem explicação.
- MFA: as seis células mostram os dígitos (`[data-part="otp-cell"]`), espelhados pelo bloco `DESIGN · código de seis dígitos` do `app.js`; o campo único `[data-part="otp-input"]` fica por cima, transparente, e é o que se submete. Sem JavaScript, o campo aparece como um campo normal.

## Fatias seguintes
2. Casca + Desktop + barra de aplicações + lançador — `shell.rs`, `home.rs`
3. Janelas
4. Nye
5. Terminal
6. Browser
