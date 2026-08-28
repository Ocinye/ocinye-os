# ADR-0007 — Fronteiras de domínio como módulos, e a API separada do núcleo

- **Estado:** Accepted
- **Domínio:** Foundation
- **Impacto:** HIGH
- **Data:** 2026-08-22
- **Refina:** [ADR-0006](0006-modular-monolith.md)

## Context

O ADR-0006 fixou o modular monolith e descreveu a forma de um módulo como
`mod.rs`, `model.rs`, `repository.rs`, `service.rs`, `api.rs`.

Ao implementar, o `api.rs` revelou-se mal colocado. O `CLAUDE.md` §3 exige que o
Core seja consumível por clientes que não falam HTTP — CLI, notebooks, agentes,
Node Agents. Um módulo que contenha as suas próprias rotas HTTP torna o
transporte parte do núcleo.

## Decision

Os módulos de domínio em `ocinye-core` contêm `mod.rs`, `model.rs`,
`repository.rs` e `service.rs`. **O transporte vive em
`services/core-server/src/routes/`**, organizado pelos mesmos nomes de domínio.

A superfície HTTP passa a ser um adaptador substituível: acrescentar uma CLI ou
um servidor gRPC é acrescentar outro adaptador sobre os mesmos serviços, não
reescrever os módulos.

Duas regras mantêm a fronteira honesta:

1. **Um handler HTTP nunca chama um repositório.** Chama sempre um serviço. É
   isto que impede uma rota de esquecer a autorização.
2. **Nenhum serviço conhece tipos HTTP.** Não recebe `Request`, não devolve
   `Response`, não conhece códigos de estado. Traduz-se `CoreError` no adaptador.

### Nota sobre a agregação de rotas

`routes/collaboration.rs` serve também os endpoints de datasets. A superfície
HTTP do Data Plane é pequena e igualmente scoped ao workspace, e separá-la
produziria um ficheiro de quinze linhas. **A separação de domínio mantém-se
intacta** em `ocinye-core::modules::data`; o que é agrupado é o adaptador.

Isto está registado por ser exactamente o tipo de conveniência que, se não for
declarada, se lê depois como fronteira esbatida.

## Alternatives

| Alternativa | Porque foi rejeitada |
|---|---|
| **`api.rs` dentro de cada módulo**, como no ADR-0006 | Torna o transporte parte do núcleo e contraria `CLAUDE.md` §3. Um segundo cliente obrigaria a mover código. |
| **Uma crate por módulo, com fronteiras impostas pelo compilador** | Atraente. Desproporcionado agora (ADR-0005); reavaliável quando um módulo for candidato real a extracção. |
| **Serviços a devolver tipos HTTP** | Acoplamento na direcção errada, e impossibilita usar o Core de um notebook. |

## Consequences

**Positivas** — o Core é utilizável sem HTTP; a superfície HTTP é substituível;
a regra "handlers não chamam repositórios" é verificável numa revisão.

**Negativas, aceites** — a fronteira é mantida por convenção e revisão, não pelo
compilador. Cada `README.md` de módulo declara o que não pertence ali, e os
repositórios são `pub` apenas dentro do módulo.

## Referências

`CLAUDE.md` §3, §17, §20 · ADR-0005 · ADR-0006
