# Interface do Workspace

**Estado: Claude Design D001 a D009 em `main`** (a D006 no PR #189, a D007
com a D007.1 no PR #190, a D008 no PR #191, a D009 no PR #192; nenhuma
deployada, produção continua em `4f8d048`).
A D003 traz o Nye ([ADR-0619](../adrs/0619-nye-universal-surface.md)); a D004
Ficheiros, Notas, Calendário e Correio
([ADR-0620](../adrs/0620-productivity-apps-in-managed-windows.md)); a D005
Projectos, O Meu Trabalho, Ideias, Dados e Conhecimento
([ADR-0621](../adrs/0621-research-apps-typed-relations-and-deep-links.md)); a
D006 Unidades e Administração
([ADR-0111](../adrs/0111-organisational-invariants-and-typed-refusals.md)); a
D007 as nove da conclusão e a D007.1 Monitor de Actividade, Resultados e Lixo
([ADR-0622](../adrs/0622-completion-apps-history-evidence-and-agent-authority.md));
a D008 o Ocinye Terminal (ocsh) e o Ocinye Browser
([ADR-0623](../adrs/0623-terminal-and-browser-separate-boundaries.md)): o
registo tem 28 aplicações e **nenhuma fica `app_pending`**; o Browser é real
no runtime Web e `DESKTOP_RUNTIME_REQUIRED` no Desktop.
**A D009 (pacote R2) — predefinições por Distribuição e iconografia canónica —
está em `main`**
([ADR-0624](../adrs/0624-distribution-defaults.md),
[`distribution-defaults.md`](distribution-defaults.md),
[`iconography.md`](iconography.md)): um ponto de partida por Distribuição
(widgets, fixações, fundo, primeiros passos), um ícone por aplicação e quatro
de Distribuição; nada disto é autorização. **A D010 (pacote D010B) — acesso,
várias Distribuições e superfície do sistema — foi integrada no ramo
`feat/design-d010`** ([ADR-0019](../adrs/0019-multi-distribution-instance.md),
[ADR-0020](../adrs/0020-access-endpoints.md),
[ADR-0625](../adrs/0625-distribution-context-and-switching.md)): Distribuições
activadas por Instância e acesso por membro, uma activa por sessão no Core,
pontos de acesso que falham fechado, mudança de Distribuição com o diálogo do
D002, contexto da sessão, bloqueio do ecrã e as secções novas da
Administração; a instalação fica para a D011
([`d011-installer-boundary.md`](../architecture/d011-installer-boundary.md)).
A secção está no topo do [`CODE_FEEDBACK.md`](CODE_FEEDBACK.md), e a cobertura em
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
  conversas do próprio membro. Da D004: os quatro ecrãs como corpos
  de janela, o motor de envio de ficheiros (Code), o Markdown restrito das
  Notas, o fuso do membro no Calendário e o rascunho que sobrevive a um envio
  falhado. Da D005: os cinco ecrãs de investigação e trabalho como
  corpos de janela, com relações lidas pela linhagem (as duas pontas
  autorizadas), um endereço canónico por recurso, transições só do Core e
  referências tipadas para o Nye. Da D006: Unidades e Administração
  como corpos de janela, a confirmação partilhada das acções privilegiadas (só
  para acções que o Core oferece, com o foco a voltar à acção) e a credencial
  temporária só na resposta que a emite. Da D009: o Desktop, as fixações e o fundo
  de partida de cada Distribuição (Research, Business, Personal, Education),
  filtrados pela autoridade do membro (um widget que não pode ver esconde-se e
  fica na disposição), os primeiros passos no distintivo, e o mapa único
  aplicação → ícone.
- **Registo da integração:** [`design-integration.json`](design-integration.json).
- **O que o Design precisa de saber:** [`CODE_FEEDBACK.md`](CODE_FEEDBACK.md).
- **Referência visual (só referência):** `design/claude-design/reference/`.

As aplicações ainda sem ecrã do Design abrem numa janela gerida com o estado
`app_pending`; chegam nas revisões seguintes. O apagamento anterior da UI está
descrito em [`UI_WIPE_REPORT.md`](UI_WIPE_REPORT.md).
