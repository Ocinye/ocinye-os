# D14 · Contratos novos

Continua a numeração de `CLAUDE_DESIGN_FUNCTIONAL_GAPS.md` (G-01…G-09).

| ID | Contrato | Proposta | Até existir |
|---|---|---|---|
| **G-10** | Registo de comandos | `GET /api/commands?lang=pt` → famílias, subcomandos, argumentos, opções, tipo de recurso, risco, grupo, descrições localizadas, **já filtrado pela função do membro**. Mesma fonte da Superfície Universal e do Nye | registo estático no cliente (tabela de `D13_OCSH.md`), só descoberta |
| **G-11** | Execução | `POST /api/commands/exec` `{line, context, session_id}` → `{exit, blocks:[{kind, …}], cap, policy, audit_id, ms}`. O parse final é no Core. Pipelines resolvidos no Core sobre dados tipados | só comandos locais (`help`, `clear`, `history`, `whoami`, `exit`) e ponte do Gestor de Janelas |
| **G-12** | Streaming e cancelamento | `GET /api/commands/{run_id}/events` (SSE same-origin) com `progress`, `line`, `log`, `done`; `POST /api/commands/{run_id}/cancel` → `{cancelled, partial: "…texto do que ficou feito…"}` | upload via sessões de carregamento existentes (0027/0050) sem cancelamento |
| **G-13** | Pré-visualização de risco | `exec` devolve `{needs_confirmation: {level: "med"|"high", preview:[[k,v]…], word, token}}`; `POST /api/commands/confirm {token, typed}` → recibo | comandos médios/altos não disponíveis no Terminal |
| **G-14** | Elevação de sessão | `POST /api/session/elevate {mfa_code, terminal_session_id}` → `{expires_at, caps}`; `DELETE /api/session/elevate`. Reusa MFA TOTP (0029). Escopo: um separador | `session elevate` → `ods.state.pending_contract`; admin usa a UI |
| **G-15** | Preferências e histórico | `GET/PUT /api/me/terminal-preferences`; `GET/POST/DELETE /api/me/terminal-history` (redacção de segredos no servidor) | preferências em memória; histórico só da sessão |

Enquanto G-11 não existir, o Terminal mostra no primeiro comando não-local: nota `warn` `ocsh.state.pending_exec` («Este comando ainda não está ligado ao Core nesta versão.»). Nunca simular resultados.

## CSP / verify.sh
- Sem `eval`, sem `new Function`, sem `innerHTML` com saída do Core: blocos renderizados por `textContent`/nós.
- SSE e fetch só same-origin. Copiar usa `navigator.clipboard.writeText` (permitido pela CSP actual).
- Todas as strings visíveis passam pelo catálogo (`terminal.*`, `ocsh.*`); descrições de comandos vêm localizadas de G-10.
