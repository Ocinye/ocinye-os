# ADR-0011 — Redis para filas e coordenação

- **Estado:** Accepted
- **Domínio:** Foundation
- **Impacto:** MEDIUM
- **Data:** 2026-08-22

## Context

O `CLAUDE.md` §6 prevê Redis para cache e filas "quando houver necessidade
concreta". Convém declarar qual é essa necessidade e, sobretudo, o que Redis
**não** é autorizado a fazer.

## Decision

Redis é adoptado para **coordenação efémera**, não como fonte de verdade.

Usos autorizados:

- sinal de despertar do worker (para reduzir latência do polling do outbox);
- locks de curta duração para trabalho não reentrante;
- rate limiting;
- cache de dados derivados e recalculáveis.

Usos **proibidos**:

- guardar estado institucional — a verdade está no PostgreSQL (`CLAUDE.md` §25);
- guardar sessões de forma que a perda de Redis destrua sessões sem recuperação;
- substituir o outbox: a durabilidade dos eventos é do PostgreSQL (ADR-0010).

O sistema tem de funcionar, mais lentamente, se o Redis estiver indisponível.
Uma falha de Redis degrada; não corrompe.

## Alternatives

| Alternativa | Porque foi rejeitada |
|---|---|
| **Só PostgreSQL** | Possível e considerado, dado que o outbox já vive lá. Redis acrescenta rate limiting e locks com muito menos carga na base canónica. |
| **RabbitMQ** | Filas melhores, mais um sistema com estado a operar; a durabilidade já é dada pelo outbox. |
| **Valkey** | Fork legítimo do Redis, tecnicamente próximo. Redis mantido por familiaridade operacional; a troca seria de baixo custo e não exigiria mudar código. |

## Consequences

**Positivas** — worker reactivo; rate limiting simples; a fonte de verdade
mantém-se única.

**Negativas, aceites** — mais um serviço em desenvolvimento e produção; exige
disciplina para que ninguém comece a tratar Redis como fonte de verdade — o que
a revisão de código verifica.

## Referências

`CLAUDE.md` §6, §25 · ADR-0010
