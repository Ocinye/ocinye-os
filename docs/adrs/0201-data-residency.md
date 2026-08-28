# ADR-0201 — Residência de dados explícita

- **Estado:** Accepted
- **Domínio:** Data
- **Impacto:** MEDIUM
- **Data:** 2026-08-22

## Context

O `CLAUDE.md` §27 distingue **controlo institucional** de **residência física**.
É uma distinção fácil de perder em comunicação e fácil de tornar falsa em
documentação. Hoje a Ocinye não possui storage físico próprio.

## Decision

A residência é um **atributo declarado e persistido**, nunca uma inferência.

Cada `StorageBackend` transporta:

| Campo | Significado |
|---|---|
| `location_label` | Rótulo humano da localização física |
| `residency` | `UNDECLARED` · `THIRD_PARTY_CLOUD` · `OCINYE_CAMAMA` · `OCINYE_COLOCATION` |
| `migration_state` | `stable` · `migration_planned` · `migrating` |

- **`UNDECLARED` é o valor por omissão.** O sistema nunca afirma residência que
  não foi declarada.
- `Residency::is_ocinye_owned()` é o único ponto que decide se a instituição pode
  dizer que os dados residem em infraestrutura sua. Hoje devolve `false` em todos
  os backends existentes.
- Documentação e interface **nunca** afirmam que os dados residem num datacenter
  Ocinye enquanto nenhum backend declarar `OCINYE_CAMAMA` ou
  `OCINYE_COLOCATION`.

Migrar para infraestrutura própria é: registar um novo backend, marcar
`migration_planned`, copiar objectos com verificação de checksum, repontar as
referências, marcar `stable`. Sem alterações no domínio.

## Alternatives

| Alternativa | Porque foi rejeitada |
|---|---|
| **Residência implícita na configuração do endpoint** | Não é consultável nem auditável, e perde-se assim que existir mais de um backend. |
| **Assumir residência única global** | Impede coexistir storage de terceiros com storage próprio durante uma migração. |
| **Não modelar residência** | Tornaria impossível responder "onde residiam estes dados?", uma das perguntas que o system of record tem de responder (briefing §28). |

## Consequences

**Positivas** — a pergunta "onde residem estes dados?" tem resposta por objecto;
migração futura sem remodelação; impossível afirmar residência Ocinye por
descuido.

**Negativas, aceites** — mais um campo a manter correcto em cada backend; a
verdade da declaração depende de quem regista o backend, pelo que o registo é uma
operação administrativa auditada.

## Referências

`CLAUDE.md` §27 · briefing §34 · ADR-0200
