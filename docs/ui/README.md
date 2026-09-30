# Interface do Workspace

**Estado: Claude Design D001 a D007.1 em `main`** (a D006 no PR #189, a D007
com a D007.1 no PR #190; nenhuma deployada, produção continua em `4f8d048`).
A D003 traz o Nye ([ADR-0619](../adrs/0619-nye-universal-surface.md)); a D004
Ficheiros, Notas, Calendário e Correio
([ADR-0620](../adrs/0620-productivity-apps-in-managed-windows.md)); a D005
Projectos, O Meu Trabalho, Ideias, Dados e Conhecimento
([ADR-0621](../adrs/0621-research-apps-typed-relations-and-deep-links.md)); a
D006 Unidades e Administração
([ADR-0111](../adrs/0111-organisational-invariants-and-typed-refusals.md)); a
D007 as nove da conclusão e a D007.1 Monitor de Actividade, Resultados e Lixo
([ADR-0622](../adrs/0622-completion-apps-history-evidence-and-agent-authority.md)).
**A D008 — o Ocinye Terminal (ocsh) e o Ocinye Browser — está no ramo
`feat/design-d008`, por fazer merge**
([ADR-0623](../adrs/0623-terminal-and-browser-separate-boundaries.md)): o
registo passa a 28 e **nenhuma aplicação fica `app_pending`**; o Browser é
real no runtime Web e `DESKTOP_RUNTIME_REQUIRED` no Desktop. A secção está no
topo do [`CODE_FEEDBACK.md`](CODE_FEEDBACK.md), e a cobertura em
[`application-design-coverage.md`](application-design-coverage.md). Equipas não
existem: sem domínio, sem desenho, sem aplicação.
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
  falhado. Da D005 (no ramo): os cinco ecrãs de investigação e trabalho como
  corpos de janela, com relações lidas pela linhagem (as duas pontas
  autorizadas), um endereço canónico por recurso, transições só do Core e
  referências tipadas para o Nye. Da D006 (no ramo): Unidades e Administração
  como corpos de janela, a confirmação partilhada das acções privilegiadas (só
  para acções que o Core oferece, com o foco a voltar à acção) e a credencial
  temporária só na resposta que a emite.
- **Registo da integração:** [`design-integration.json`](design-integration.json).
- **O que o Design precisa de saber:** [`CODE_FEEDBACK.md`](CODE_FEEDBACK.md).
- **Referência visual (só referência):** `design/claude-design/reference/`.

As aplicações ainda sem ecrã do Design abrem numa janela gerida com o estado
`app_pending`; chegam nas revisões seguintes. O apagamento anterior da UI está
descrito em [`UI_WIPE_REPORT.md`](UI_WIPE_REPORT.md).
