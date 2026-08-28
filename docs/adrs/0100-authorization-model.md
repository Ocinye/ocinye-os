# ADR-0100 — RBAC + ABAC contextual, fail closed

- **Estado:** Accepted
- **Domínio:** Security
- **Impacto:** FOUNDATIONAL
- **Data:** 2026-08-22

## Context

O `CLAUDE.md` §34 exige separar título institucional de papel técnico e proíbe o
par `admin`/`user`. O briefing §38 é explícito: ser fundador não implica acesso a
recursos RESTRICTED.

A autorização tem duas faces que têm de concordar: a decisão sobre **um** recurso
já carregado, e o filtro que decide **que linhas** uma listagem devolve. Se
divergirem, uma listagem revela o que a política nega.

## Decision

**RBAC com regras contextuais (ABAC)**, avaliado inteiramente do lado do servidor.

### Duas dimensões separadas

- **Institutional position** (`Founder`, `Director`, `Researcher`, …) — facto
  organizacional. Concede **zero** permissões. Não entra na função de decisão.
- **Technical role** (`platform_admin`, `organisation_admin`, `unit_manager`,
  `research_member`, `collaborator`, `auditor`) — concede capacidade.

A isto acrescem **memberships contextuais**: papel na unidade (`manager`,
`member`) e no research workspace (`lead`, `member`, `viewer`).

### Regra de leitura por classificação

| Classificação | Quem pode ler |
|---|---|
| `PUBLIC`, `INTERNAL` | Qualquer membro activo da organização |
| `CONFIDENTIAL` | Membro da unidade, membro do workspace, ou admin da organização |
| `RESTRICTED` | **Apenas** membro explícito do workspace, ou gestor da unidade |

`RESTRICTED` **ignora deliberadamente papéis administrativos**. É esta linha que
impede "Fundador" de significar "lê tudo".

### Fail closed

Cada caminho de decisão termina numa autorização explícita. Tudo o que a política
não consegue justificar positivamente é negado. Não existe ramo `_ => allow`.

### Negação de leitura devolve 404

Uma leitura negada devolve `not_found`, não `permission_denied`, para que a
existência do recurso não seja revelada. Uma escrita negada sobre um recurso
legível devolve `permission_denied` — seguro, porque a legibilidade já está
estabelecida.

### Política pura e espelho SQL

A política vive em `ocinye-domain::policy`: uma função pura, sem I/O, logo
exaustivamente testável. O filtro SQL correspondente
(`ocinye-domain::policy::visibility`) é o seu espelho para listagens e pesquisa.

**As duas implementações têm de concordar.** Um teste percorre exaustivamente
todas as combinações de classificação, papel e membership e falha se divergirem.

## Alternatives

| Alternativa | Porque foi rejeitada |
|---|---|
| **RBAC puro** | Não exprime "membro deste workspace" nem "gestor desta unidade", que é onde vive quase toda a autorização real do sistema. |
| **ABAC/policy engine externo (OPA, Cedar)** | Expressivo e auditável, mas acrescenta um componente e uma linguagem para uma política que cabe numa função Rust testável. Reavaliável quando as regras deixarem de caber numa leitura. |
| **ACL por recurso** | Máxima granularidade, mas explosão de estado e de UI sem necessidade demonstrada. As memberships cobrem os casos reais. |
| **Autorização no cliente** | Proibido: o browser nunca é autoridade (briefing §17). |

## Consequences

**Positivas** — a política é uma função pura, testável sem base de dados;
listagens e leituras individuais não podem divergir; títulos institucionais não
podem ser confundidos com poder técnico.

**Negativas, aceites** — manter duas implementações (decisão e filtro SQL)
sincronizadas é um custo real, pago com um teste de equivalência exaustivo que
falha assim que uma delas mudar sozinha.

## Referências

`CLAUDE.md` §31, §34, §36 · briefing §38, §39, §40 · ADR-0102
