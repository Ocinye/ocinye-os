# ADR-0009 — PostgreSQL com SQLx e SQL explícito

- **Estado:** Accepted
- **Domínio:** Foundation
- **Impacto:** HIGH
- **Data:** 2026-08-22

## Context

O `CLAUDE.md` §25 fixa PostgreSQL como fonte canónica dos metadados
institucionais. Falta decidir como o Rust lhe acede, e com que grau de abstracção.

Duas exigências entram em tensão: o briefing §69 pede SQL explícito e recusa
esconder operações críticas atrás de abstracções opacas; ao mesmo tempo, a
verificação de queries em tempo de compilação do SQLx (`query!`) exige uma base
de dados acessível durante a compilação, ou ficheiros `.sqlx` gerados.

## Decision

**PostgreSQL** com **SQLx**, usando SQL escrito à mão.

Nesta fase, `sqlx::query_as` com structs `FromRow` — **verificação em tempo de
execução**, não as macros `query!`. Consequência aceite: um erro de SQL só
aparece quando a query corre, pelo que **cada repositório tem de ter testes de
integração contra uma base real** — o que já é exigido pelo `CLAUDE.md` §59.

Razão: builds herméticos. `cargo build` não deve exigir uma base de dados
disponível, nem em CI, nem na máquina de um novo programador. Adoptar
`cargo sqlx prepare` com os ficheiros `.sqlx` commitados é a evolução natural e
está registada como trabalho futuro, não como decisão adiada indefinidamente.

Regras adicionais:

- Toda a alteração de schema é uma **migration versionada** em `migrations/`,
  aplicada por `sqlx::migrate!`. Nunca alteração manual em produção.
- Nenhuma query concatena input do utilizador. Todos os valores são parâmetros
  vinculados.
- O filtro de autorização faz parte da query (`WHERE`), nunca uma filtragem em
  memória depois de ler tudo — inclui `COUNT`, facetas e sugestões.
- Ficheiros grandes não vão para o PostgreSQL (ADR-0200).

## Alternatives

| Alternativa | Porque foi rejeitada |
|---|---|
| **Diesel** | ORM maduro e com verificação em tempo de compilação, mas o DSL afasta-se do SQL, precisamente o que o briefing §69 pede para evitar em operações críticas. |
| **SeaORM** | Ergonomia superior para CRUD; o Ocinye OS não é uma colecção de CRUDs (briefing §14) e as queries mais importantes são as de autorização, onde o SQL explícito é mais auditável. |
| **`tokio-postgres` puro** | Perderia migrations, pool e mapeamento de tipos, que o SQLx já resolve bem. |
| **SQLx com macros `query!`** | Verificação em tempo de compilação é valiosa, mas exige base de dados no build ou ficheiros `.sqlx` mantidos. Adiado deliberadamente até a estabilização do schema — decisão registada, não esquecida. |

## Consequences

**Positivas** — SQL auditável e legível numa revisão de segurança; migrations
versionadas e reproduzíveis; builds herméticos.

**Negativas, aceites** — erros de SQL só aparecem em tempo de execução, o que
transfere a responsabilidade para os testes de integração. É uma dívida
consciente com mitigação declarada.

## Referências

`CLAUDE.md` §25, §58, §59 · briefing §69, §70 · ADR-0200
