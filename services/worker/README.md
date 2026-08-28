# `worker` — Worker Runtime

Drena o outbox transaccional e executa trabalho que não deve bloquear um pedido.

## Finalidade

Propagar eventos de domínio, actualizar estado derivado e manter honesta a visão
de disponibilidade do Intelligence Plane.

## Porque polling, e porque chega

Os eventos são duráveis no PostgreSQL
([ADR-0010](../../docs/adrs/0010-events-outbox.md)), pelo que o custo do polling
é latência, não perda. `FOR UPDATE SKIP LOCKED` permite vários workers em
paralelo sem coordenação além da base de dados.

## Idempotência

Todo o handler tem de ser seguro a correr duas vezes. Um evento pode ser
reentregue após uma falha entre o handler ter sucesso e a linha ser marcada como
publicada — "correu duas vezes" é um caso normal, não uma excepção.

## Retentativas

Backoff exponencial limitado, com o erro registado na linha. Após 10 tentativas
o evento **deixa de ser tentado mas não é descartado**: um evento encravado é um
sinal, e apagá-lo esconderia um problema real.

## Estado derivado

De 30 em 30 segundos marca como indisponíveis os modelos de nós que deixaram de
reportar. Sem esta varredura, um nó que morresse deixaria os seus modelos
anunciados como disponíveis — exactamente o tipo de afirmação que a plataforma
não pode fazer.

## O que este worker ainda não faz

A indexação de pesquisa acontece **dentro da transacção que a originou**, não
aqui, para que o índice nunca descreva um artefacto que foi revertido.

O handler é por isso deliberadamente fino hoje: existe para que trabalho diferido
— checksums, previews, embeddings, notificações — tenha um sítio durável e
idempotente para onde ir.

## Execução

```bash
set -a && source .env && set +a
cargo run --bin ocinye-worker
```

## Segurança relevante

Os eventos são registados apenas pelas **chaves** do payload, nunca pelo payload
inteiro: a garantia de que os payloads não transportam conteúdo passa a estar
nesta linha, não em cada emissor futuro.
