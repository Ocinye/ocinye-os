# ADR-0109 — Admissão de pedidos de IA e o ledger de uso imutável

- **Estado:** Accepted
- **Domínio:** Identity
- **Impacto:** HIGH
- **Data:** 2026-09-13
- **Complementa:** [ADR-0108](0108-resource-governance-and-compute-control-plane.md) · [ADR-0304](0304-canonical-inference-contract.md)

## Context

O [ADR-0108](0108-resource-governance-and-compute-control-plane.md) construiu a
fundação da governança de recursos — capacidade, entitlement, reserva e uso como
conceitos distintos — e impôs **só** o armazenamento pessoal. O caminho de
execução de IA (o Prompt, `POST /ai/prompt`) roteava por um modelo e executava
sem passar por nenhuma admissão: quando a primeira GPU entrasse, o roteamento
estaria certo mas faltaria o mecanismo que decide **se aquele membro pode
consumir aquele modelo**. Ligar a governança ao caminho de IA antes de existir
inferência real é a pré-condição de a certificar (M5 §H).

Ao escrever o primeiro consumidor do ledger de uso, surgiu um defeito latente
que ninguém tinha exercido: o `resource_usage_events` é **append-only**, imposto
por gatilhos que recusam qualquer `UPDATE` ou `DELETE`, mas as suas colunas de
contexto (`model_id`, `compute_node_id`, `actor_person_id`, …) tinham chaves
estrangeiras `ON DELETE SET NULL`. Apagar uma linha referenciada dispara um
`UPDATE` no ledger para anular a referência, e o gatilho recusa-o — pelo que a
linha referenciada deixa de poder ser apagada de todo. Em produção isto morde de
imediato: o `ai_models` é apagado e reinserido a cada relatório de nó, e o
primeiro uso registado encravaria o relatório seguinte.

## Decision

### A admissão de IA

Um pedido de IA é admitido no servidor, **antes de qualquer inferência**, contra
o entitlement efectivo de `model_access` do membro, com a mesma disciplina do
armazenamento pessoal (`admit_personal_bytes`): um *advisory lock* por membro
serializa pedidos concorrentes, o uso é medido do ledger e o limite resolvido na
mesma transacção, e `usado + pedido ≤ limite` decide — **fail-closed**. Um
limite de zero (nenhum resolvido) admite tudo, exactamente como o armazenamento:
uma instalação que não escolheu medir a IA não a bloqueia por acidente. Medir é
uma decisão de **dados** — uma regra de perfil ou uma alocação de `model_access`
—, não de código.

Para um pedido síncrono, **a transacção é a reserva**. A admissão toma o lock; se
um modelo responder, o uso é gravado no ledger na mesma transacção (a reserva
confirma); se não, nada é gravado (a reserva liberta-se com a transacção). Não há
janela em que uma cobrança sobreviva a uma falha, nem uma falha guarde uma
cobrança. Uma tabela de reservas persistida — para trabalhos de computação
**assíncronos** — é uma fatia posterior, como o [ADR-0108](0108-resource-governance-and-compute-control-plane.md)
já previa.

### O ledger é proveniência imutável

As colunas de contexto do `resource_usage_events` deixam de ter chaves
estrangeiras com `ON DELETE SET NULL` e passam a ser **UUIDs de proveniência**
(migração 0043). Um evento de uso regista que um acesso usou o modelo X no nó Y;
esse facto histórico tem de **sobreviver** à remoção de X e de Y, não ser mutado
quando eles saem. O `organisation_id` mantém o seu `ON DELETE CASCADE`: apagar
uma organização é um desmantelamento completo, não uma operação de rotina.

O ledger nunca guarda o prompt nem a resposta — apenas *que* o acesso aconteceu,
com o modelo, o nó e o trabalho.

## Alternatives

- **Manter os FKs `SET NULL` e ensinar o gatilho a distinguir a anulação do
  sistema.** Rejeitado: frágil, e o ledger não *deve* ser mutado de todo — a
  anulação em cascata é a operação errada, não uma que valha a pena permitir.
- **`ON DELETE CASCADE` no ledger.** Rejeitado: apagaria história, o oposto de um
  registo append-only.
- **`ON DELETE NO ACTION`.** Rejeitado: bloquearia a remoção do modelo/nó, que é
  uma operação de rotina.
- **Admitir contra tokens/tempo de GPU já agora.** Rejeitado: sem inferência real
  não há consumo a medir; `model_access` (uma contagem de acessos) é a unidade
  honesta e imposta hoje. As unidades de GPU/VRAM/tempo já existem no vocabulário
  e entram quando houver hardware que as consuma.

## Consequences

- `POST /ai/prompt` admite antes de executar; um pedido acima da quota conclui
  como resposta de sistema degradada, `reason_code=AI_RESOURCE_QUOTA_EXCEEDED`,
  sem chamar o modelo.
- O `resource_usage_events` ganha o seu **primeiro escritor** (o consumo de IA) e
  o seu primeiro leitor (a medição do uso), e torna-se imutável de facto: apagar
  um modelo ou um nó já não colide com a sua append-only-ness.
- O perfil por omissão continua **sem** quota de IA — a produção não muda (0 nós,
  0 modelos; a admissão nem sequer é alcançada com o `NoProvider`). O mecanismo
  está ligado e provado, pronto para o primeiro nó.
- Falta ainda: reservas persistidas para trabalhos assíncronos, e a admissão de
  `gpu`/`vram`/`gpu_time`/`compute_time` quando existir hardware.
