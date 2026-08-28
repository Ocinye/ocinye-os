# ADR-0005 — Monorepo com Cargo workspace

- **Estado:** Accepted
- **Domínio:** Foundation
- **Impacto:** MEDIUM
- **Data:** 2026-08-22

## Context

O Ocinye OS é composto por componentes que partilham tipos institucionais e
evoluem em conjunto: Core, Workspace, Worker, Node Agent, Capability Runtime.
Separá-los em repositórios distintos obrigaria a publicar e versionar crates
internas antes de existir qualquer consumidor externo.

O `CLAUDE.md` §19 já estabelecia a preferência por monorepo. Falta decidir a
granularidade: quantas crates, e segundo que critério.

## Decision

Monorepo único com um **Cargo workspace** na raiz.

Granularidade **proporcional**: uma crate por fronteira arquitectural real, não
uma crate por domínio conceptual. Dez crates, não trinta:

```
crates/ocinye-contracts      tipos canónicos + DTOs; sem I/O; compilável para wasm32
crates/ocinye-domain         invariantes puros: workflows, política de autorização
crates/ocinye-observability  logging estruturado, correlação
crates/ocinye-core           persistência + serviços de aplicação (módulos por domínio)
crates/ocinye-capabilities   manifesto de capacidades + host runtime WASM
services/core-server         binário Axum (Core Runtime)
services/worker              binário do Worker Runtime
services/node-agent          binário do Node Runtime
apps/workspace               servidor do Workspace Runtime (Leptos SSR)
wasm/capabilities/*          capacidades convidadas, alvo wasm32-wasip1
```

Os domínios institucionais (identity, research, knowledge, data, collaboration,
governance, search, ai, compute, storage) são **módulos dentro de
`ocinye-core`**, com fronteiras explícitas — ver ADR-0006.

`wasm/capabilities/*` fica **excluído** do workspace: compila para
`wasm32-wasip1` e incluí-lo forçaria a sua compilação em cada build nativo.

## Alternatives

| Alternativa | Porque foi rejeitada |
|---|---|
| **Polyrepo** | Obriga a publicar crates internas e a coordenar versões entre repositórios antes de haver qualquer benefício. Torna atómica uma mudança de contrato impossível. |
| **Uma crate por domínio (~20 crates)** | Tempos de compilação e ruído de manifestos desproporcionados para a dimensão actual. Fronteiras de domínio são melhor servidas por módulos com API explícita nesta fase (ADR-0006). |
| **Crate única** | Impede separar `contracts` (que tem de compilar para wasm32 e ser consumida pelo Workspace) do código de persistência, que nunca deve chegar ao browser. |

## Consequences

**Positivas**

- Uma alteração de contrato e os seus consumidores mudam no mesmo commit.
- `cargo test --workspace` cobre tudo; CI simples.
- `ocinye-contracts` é a única crate que o Workspace precisa de partilhar com o
  Core, o que torna explícito o que pode e o que não pode chegar ao cliente.

**Negativas**

- O workspace inteiro recompila com mais frequência do que crates isoladas.
- Exige disciplina para que `ocinye-core` não se torne um saco sem fronteiras;
  mitigado pelo ADR-0006 e por READMEs por módulo.

## Referências

`CLAUDE.md` §19, §20 · ADR-0004 · ADR-0006 · ADR-0501
