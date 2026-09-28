# Interface do Workspace

**Estado: Claude Design D001.1 integrada** (2026-09-29, ramo
`feat/design-d001-1`, sobre a D001). O código de interface é do Design e está fechado
([`DESIGN_LOCK.md`](DESIGN_LOCK.md)); o contrato detalhado de rotas e
ViewModels está em [`HANDOFF.md`](HANDOFF.md).

- **Integrado:** arranque, login em dois passos, fim de sessão, recuperar
  (indisponível, G-26), primeiro acesso, MFA (configurar, códigos, desafio), a
  casca (barra de cima, menu da conta, barra de aplicações, lançador, paleta),
  o Desktop com os 14 widgets e a personalização, a janela `app_pending`, as
  páginas de erro 404/403/502 e a falha de identidade (503).
- **Registo da integração:** [`design-integration.json`](design-integration.json).
- **O que o Design precisa de saber:** [`CODE_FEEDBACK.md`](CODE_FEEDBACK.md).
- **Referência visual (só referência):** `design/claude-design/reference/`.

As aplicações ainda sem ecrã do Design respondem com a janela `app_pending`
dentro da casca; chegam nas revisões D002+. O apagamento anterior da UI está
descrito em [`UI_WIPE_REPORT.md`](UI_WIPE_REPORT.md).
