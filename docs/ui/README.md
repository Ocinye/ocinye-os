# Interface do Workspace

**Estado: Claude Design D002 integrada no ramo `feat/design-d002`**, sobre a
D001.2.1 em `main` (PR #181, merge `7c20f8d`; nenhuma das duas deployada,
produção continua em `4f8d048`). A D001 está fechada; a D002 está ligada ao
motor e funcionalmente certificada, com a paridade visual por fechar (dois
defeitos devolvidos ao Design). O código de interface é do Design e está fechado
([`DESIGN_LOCK.md`](DESIGN_LOCK.md)); o contrato detalhado de rotas e
ViewModels está em [`HANDOFF.md`](HANDOFF.md).

- **Integrado:** arranque, login em dois passos, fim de sessão, recuperar
  (indisponível, G-26), primeiro acesso, MFA (configurar, códigos, desafio), a
  casca (barra de cima, menu da conta, barra de aplicações, lançador, paleta),
  o Desktop com os 14 widgets e a personalização, a janela `app_pending`, as
  páginas de erro 404/403/502 e a falha de identidade (503). Da D002: as
  janelas geridas (janela, prateleira, alternador, escolha entre janelas,
  encaixe, fechar com trabalho por guardar), o menu de contexto do Desktop e os
  painéis de estado, notificações e relógio. O motor é de Code
  ([ADR-0618](../adrs/0618-window-manager.md)).
- **Registo da integração:** [`design-integration.json`](design-integration.json).
- **O que o Design precisa de saber:** [`CODE_FEEDBACK.md`](CODE_FEEDBACK.md).
- **Referência visual (só referência):** `design/claude-design/reference/`.

As aplicações ainda sem ecrã do Design abrem numa janela gerida com o estado
`app_pending`; chegam nas revisões seguintes. O apagamento anterior da UI está
descrito em [`UI_WIPE_REPORT.md`](UI_WIPE_REPORT.md).
