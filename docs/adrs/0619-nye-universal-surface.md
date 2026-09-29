# ADR-0619 — O Nye: a superfície universal apresenta, o Core decide e executa

- **Estado:** Accepted
- **Domínio:** Workspace
- **Impacto:** HIGH
- **Depende de:** [ADR-0301](0301-agentic-control-plane.md) · [ADR-0303](0303-capability-registry-and-executor.md) · [ADR-0308](0308-typed-ai-interaction-envelope.md) · [ADR-0309](0309-ai-conversation-persistence-and-provenance.md) · [ADR-0601](0601-workspace-bff-session.md) · [ADR-0618](0618-window-manager.md)
- **Data:** 2026-09-29

## Context

O Claude Design entregou na D003 a apresentação do Nye: a superfície universal
aberta pela paleta (`Ctrl K`) em todas as páginas — pesquisar, perguntar, agir
—, a proposta de acção com o seu risco e a confirmação, e a aplicação Nye com
conversas, compositor e voz (`ui/nye/mod.rs`, `oc-nye.css`, `oc-nye.js`,
`i18n/ui_nye.rs`). A aplicação é a que o registo chamava «Prompt Ocinye».

O Core já tinha tudo o que decide: `POST /agentic/invoke` (pesquisa
determinística; perguntar e agir só com inferência), planos persistidos e
imutáveis com digest, aprovação e execução por HTTP, e conversas owner-scoped
(ADR-0309). Não havia inferência em produção, nem streaming, nem voz.

## Decision

**Papéis.** O Design apresenta; o Workspace traduz o que o Core devolve para os
ViewModels do Design (`controllers/nye.rs`); o Core autoriza, planeia, decide o
risco e a confirmação, executa e audita. Nenhum JavaScript é autoridade: sem
ele, cada acção é um formulário.

**Disponibilidade honesta.** Pesquisar está sempre disponível com o Core
saudável. Perguntar e agir dizem porquê não estão: sem `ai.use`
(`permission_denied`), sem fornecedor, sem modelo compatível, ou inferência
indisponível — lidos do inventário do Core, nunca inventados. Voz e anexos
declaram-se indisponíveis. Um pedido de perguntar ou agir que o Core recusa
continua a pesquisar, para que o membro nunca fique sem resposta útil.

**Pesquisa.** Os resultados vêm de `invoke` com a autorização do Core; o
Workspace só agrupa por tipo e liga os tipos com rota (nota, ficheiro,
projecto, ideia, dataset, unidade). Tipos sem rota não aparecem como ligações
partidas.

**Risco.** O Core tem cinco níveis e continua a ter cinco. «Navegação» e
«destrutivo» do Design são apresentação: nenhum plano do Core é mostrado como
tal, e um nível desconhecido é mostrado como o mais alto.

**Confirmação ligada ao que se viu.** A proposta mostra o digest do plano do
Core; o formulário de confirmação devolve-o. `POST /ask/plans/{id}/execute`
relê o plano e recusa com `409`, sem aprovar, quando o digest falta ou não é o
do plano. O digest liga-se ao efeito (capacidades, entradas, recursos), não ao
texto. A aprovação e a execução continuam a ser do Core, que reautoriza cada
passo (ADR-0411). O plano de outra pessoa lê-se como ausente.

**Conversas.** A aplicação Nye lista e abre só as conversas do membro; um
pedido continua uma conversa só se for do próprio (`conversation_id` no
contrato, ignorado quando é de outra pessoa). Um turno de sistema nunca é
mostrado como resposta de modelo, e o texto degradado do Core não é desenhado
como mensagem.

**Conteúdo do modelo.** Desenhado em blocos de texto (parágrafos e listas),
nunca como HTML. Nenhum raciocínio interno é mostrado.

**Aplicação.** O rótulo passa a «Nye» em pt/en/fr; o `id` (`prompt`), a rota
(`/ai/prompt`) e o manifesto ficam. Uma só janela (política do registo,
ADR-0618).

## Alternatives

- **Um sexto nível de risco para navegação.** Recusado: navegar não é uma
  capacidade do Core, e um nível que só o Workspace conhece seria uma segunda
  política.
- **Confirmar por identificador de plano, sem digest.** Recusado: um
  formulário antigo confirmaria outro efeito.
- **Simular streaming ou voz.** Recusado: não existem; mostrá-los seria
  evidência inventada.
- **Esconder perguntar/agir sem IA.** Recusado: o membro deve ver o que existe e
  porque não está disponível.

## Consequences

- Perguntar e agir só respondem quando houver um fornecedor que sirva `GENERAL`;
  até lá dizem porquê e pesquisam.
- O contexto (o que o membro tem aberto) ainda não viaja com o pedido: agir
  sobre um recurso exige nomeá-lo.
- Renomear e arquivar conversas, streaming, anexos e voz ficam por fazer no
  Core.
- As viagens `apps/workspace/tests/d003_journeys.rs` provam a superfície sem
  inferência; `d003_act_journeys.rs` prova proposta, confirmação, recusa e
  execução pelo caminho de produção, com o fornecedor determinístico de testes
  do Core.
