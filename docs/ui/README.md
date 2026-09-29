# Interface do Workspace

**Estado: Claude Design D003.1 (Nye) em `main`** (PR #185, merge `ae8bf1d`;
nenhuma deployada, produção continua em `4f8d048`). A D001, a D002 e a D003
estão fechadas. A D003 traz o Nye: a superfície
universal em todas as páginas e a aplicação Nye, ligadas ao Core
([ADR-0619](../adrs/0619-nye-universal-surface.md)); a pesquisa funciona sem
IA, perguntar e agir dizem porquê não estão, e a voz está indisponível.
**A D004 — Ficheiros, Notas, Calendário e Correio —, com o fecho responsivo e
de acessibilidade da D004.1, está no ramo `feat/design-d004`, por fazer merge**
([ADR-0620](../adrs/0620-productivity-apps-in-managed-windows.md)); os defeitos
e as faltas do Design estão no topo do [`CODE_FEEDBACK.md`](CODE_FEEDBACK.md).
O código de interface é do Design e está fechado
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
  ([ADR-0618](../adrs/0618-window-manager.md)). Da D003: o Nye — pesquisar,
  perguntar, agir, propostas com confirmação ligada ao digest do plano,
  conversas do próprio membro. Da D004 (no ramo): os quatro ecrãs como corpos
  de janela, o motor de envio de ficheiros (Code), o Markdown restrito das
  Notas, o fuso do membro no Calendário e o rascunho que sobrevive a um envio
  falhado.
- **Registo da integração:** [`design-integration.json`](design-integration.json).
- **O que o Design precisa de saber:** [`CODE_FEEDBACK.md`](CODE_FEEDBACK.md).
- **Referência visual (só referência):** `design/claude-design/reference/`.

As aplicações ainda sem ecrã do Design abrem numa janela gerida com o estado
`app_pending`; chegam nas revisões seguintes. O apagamento anterior da UI está
descrito em [`UI_WIPE_REPORT.md`](UI_WIPE_REPORT.md).
