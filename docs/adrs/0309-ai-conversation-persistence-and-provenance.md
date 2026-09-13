# ADR-0309 — Persistência e proveniência de conversas de IA

- **Estado:** Accepted
- **Domínio:** AI
- **Impacto:** MEDIUM
- **Data:** 2026-09-13
- **Complementa:** [ADR-0308](0308-typed-ai-interaction-envelope.md) · [ADR-0300](0300-ai-gateway.md)

## Context

O [ADR-0308](0308-typed-ai-interaction-envelope.md) deu à resposta de uma
interacção um envelope tipado — origem, estado, código de razão, proveniência de
modelo. Mas a resposta vivia só o tempo do pedido: o Workspace renderizava o
turno e ele desaparecia. Faltava a **verdade histórica**.

A verdade que importa preservar é a distinção, no tempo:

```
MEMBRO   "Resume este documento."
SISTEMA  DEGRADED · AI_NO_PROVIDER_AVAILABLE · model=null

…mais tarde, quando existir um nó…

MEMBRO   "Resume este documento."
MODELO   Qwen Coder · ocinye-node-01 · …
```

Sem persistência, a arquitectura conhece `SYSTEM | MODEL | TOOL | AGENT` durante
a execução e perde essa verdade depois. Uma resposta que o **sistema** deu porque
não havia inferência não pode, mais tarde, passar por resposta de **modelo**.

O `ai_jobs` — o ledger operacional — não serve para isto: **deliberadamente não
guarda o prompt nem a resposta** (é evidência de procura, não de conteúdo). Uma
conversa é outra coisa: é a história do próprio membro, e os seus turnos *são* o
conteúdo.

## Decision

Uma **conversa** é persistida (migração 0044): `ai_conversations` (dona, título
derivado do primeiro prompt) e `ai_conversation_turns` (a sequência de turnos).
Ao contrário do `ai_jobs`, os turnos **guardam o conteúdo** — o que o membro
pediu, e o que respondeu.

- **Privada ao dono.** A conversa é do membro, como uma nota. A leitura é
  owner-scoped; uma conversa que não é do requerente é indistinguível de
  inexistente (`404`), pelo que nunca se revela que a conversa de outra pessoa
  existe ([ADR-0100](0100-authorization-model.md)). Morre com o dono
  (`ON DELETE CASCADE`).
- **Proveniência tipada no turno de resposta.** Cada turno tem um `role` —
  `member` para o pedido, ou a origem tipada da resposta
  (`system`/`model`/`tool`/`agent`) — e, quando um modelo respondeu, o modelo, o
  fornecedor e o nó. Os identificadores de modelo/nó são **proveniência**
  (strings/ids simples), pela mesma razão do ledger de uso: sobrevivem à remoção
  do que referenciam.
- **A história não se reescreve.** Um turno de sistema fica um turno de sistema.
  Quando existir inferência, um pedido novo produz um turno de modelo; os turnos
  antigos não são retroactivamente convertidos.
- Cada interacção grava os dois turnos numa transacção — inteira ou nenhuma —, na
  mesma transacção que regista o uso e o trabalho concluído, para que história,
  uso e trabalho aterrem juntos.

O `/ai/prompt` persiste toda a interacção, degradada ou concluída.
`GET /ai/conversations` lista as conversas do membro, e
`GET /ai/conversations/{id}` devolve os seus turnos — ambos owner-scoped.

## Alternatives

- **Não persistir, e reconstruir a história do `ai_jobs`.** Rejeitado: o
  `ai_jobs` não guarda conteúdo por decisão, e não deve passar a guardar — são
  registos diferentes, com fins diferentes.
- **Guardar o conteúdo no `ai_jobs`.** Rejeitado: misturaria evidência
  operacional (procura, razão, sem conteúdo) com história pessoal (conteúdo,
  privada ao dono), e apagaria a fronteira que o [ADR-0108](0108-resource-governance-and-compute-control-plane.md)
  desenhou.
- **Uma conversa por membro, sempre.** Rejeitado: cada interacção do Prompt de
  hoje é independente; uma conversa por interacção é a granularidade honesta, e a
  continuação explícita (`conversation_id`) fica disponível para quando a
  interface a suportar.

## Consequences

- O membro tem uma história de IA legível e privada, com a origem de cada
  resposta explícita — a base de a interface a mostrar (fatia K).
- `ai_conversations` e `ai_conversation_turns` são estado institucional e viajam
  na continuidade, comparados por identidade (ADR-0700).
- O conteúdo persistido é **privado ao dono** e herda a disciplina de
  classificação do resto do sistema; não é indexado para pesquisa nem alcançável
  por outro membro.
- Falta ainda: a continuação de conversa a partir da interface, e a superfície de
  histórico no Workspace (fatia K).
