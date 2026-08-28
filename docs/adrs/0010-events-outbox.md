# ADR-0010 — Eventos de domínio com transactional outbox

- **Estado:** Accepted
- **Domínio:** Foundation
- **Impacto:** HIGH
- **Data:** 2026-08-22

## Context

Alterações de estado institucional precisam de desencadear trabalho assíncrono:
indexação, checksums, notificações, futuramente embeddings e jobs de computação.

Publicar num broker depois de fazer commit na base de dados cria a falha
clássica: o commit passa, a publicação falha, e o efeito perde-se sem sinal.

## Decision

**Transactional outbox.** Os eventos de domínio são escritos numa tabela
`outbox_events` **na mesma transacção** que a mudança de estado que os produziu.
O Worker drena a tabela.

- Ou a mudança de estado e o evento fazem commit juntos, ou nenhum deles.
- Nomes de evento são um contrato: versionados, documentados, nunca renomeados
  em silêncio.
- O payload transporta **identificadores e transições de estado**, nunca conteúdo
  de documentos, conteúdo de datasets ou dados pessoais.
- O consumo é **idempotente**: um evento reentregue não pode duplicar efeitos.
- **Sem Kafka nesta fase** (`CLAUDE.md` §21). A drenagem usa PostgreSQL com
  `FOR UPDATE SKIP LOCKED`, que suporta múltiplos workers em concorrência.

## Alternatives

| Alternativa | Porque foi rejeitada |
|---|---|
| **Publicação directa num broker** | Perde a atomicidade entre estado e evento — exactamente o problema que este ADR existe para evitar. |
| **Kafka / NATS** | Proibido nesta fase; a durabilidade que trazem já é dada pelo PostgreSQL, com muito menos operação. |
| **`LISTEN`/`NOTIFY` do PostgreSQL** | Notificações não são duráveis: um worker em baixo perde-as. Útil como sinal de despertar por cima do outbox, não como substituto. |
| **Sem eventos (chamadas directas)** | Acopla módulos e coloca trabalho pesado no caminho do pedido síncrono. |

## Consequences

**Positivas** — nenhum efeito se perde silenciosamente; múltiplos workers sem
coordenação adicional; o outbox é ele próprio um registo auditável do que o
sistema decidiu propagar.

**Negativas, aceites** — a drenagem faz polling, com latência da ordem do
intervalo configurado; a tabela precisa de política de retenção; a idempotência
é responsabilidade de cada handler, verificada em testes.

## Referências

`CLAUDE.md` §21, §22 · briefing §46, §74 · ADR-0006 · ADR-0011
