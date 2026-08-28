# ADR-0101 — Permissões nomeadas, âmbitos e grants explícitos

- **Estado:** Accepted
- **Domínio:** Security
- **Impacto:** HIGH
- **Data:** 2026-08-22
- **Complementa:** [ADR-0100](0100-authorization-model.md)

## Context

O [ADR-0100](0100-authorization-model.md) estabeleceu RBAC com regras
contextuais e *fail closed*, e essa fundação mantém-se. O que existia era um
conjunto de onze verbos (`Read`, `Create`, `Update`, `Administer`…) avaliados
contra classificação e membership.

Onze verbos chegam para decidir se alguém pode ler um documento. Não chegam para
distinguir:

- ver um dataset de **descarregar** os seus bytes;
- criar um agente pessoal de criar um agente **institucional**;
- submeter um job de **administrar um nó**;
- gerir membros de **ler investigação classificada**.

Sem essa distinção, o modelo empurra para `if role == admin` espalhado pelo
código — exactamente o que o `CLAUDE.md` §34 proíbe — e torna impossível
responder à pergunta administrativa «porque é que esta pessoa consegue aceder a
isto?».

## Decision

### Permissões nomeadas

Introduz-se `Permission`: 53 capacidades nomeadas, em
`crates/ocinye-contracts/src/access.rs`, cada uma com representação estável
(`documents.download`, `agents.create.institutional`, `compute.manage_nodes`).

Toda a pergunta de autorização passa a ser feita nesses termos:

```rust
can(principal, Permission::DocumentsDownload, &ctx, Some(document_id))
```

### Duas portas independentes

Uma permissão responde *pode este actor fazer esta espécie de operação aqui*.
A classificação responde *pode este actor ver este material em concreto*. **As
duas têm de permitir.**

Mantê-las separadas é o que permite ao `PlatformAdmin` administrar a plataforma
— criar contas, gerir nós, ler audit — **sem** com isso ganhar acesso a ciência
`RESTRICTED`. É a concretização do briefing §49 e do `CLAUDE.md` §34.

### Papéis de sistema, definidos em código

O mapa papel → permissões é um `match` em
`crates/ocinye-domain/src/policy/permissions.rs`, não uma tabela.

Deliberado: um conjunto de permissões editável em tempo de execução é um conjunto
que nenhum teste consegue fixar, e esta é a camada onde um teste exaustivo vale
mais. `PermissionsManage` existe no catálogo mas nenhum papel de sistema o
concede; papéis personalizados ficam **`PLANNED`**.

As permissões efectivas de um actor num contexto são a **união** de:

1. papéis técnicos de âmbito institucional;
2. o papel que detém na unidade desse contexto;
3. o papel que detém no research workspace desse contexto;
4. grants explícitos vivos que se apliquem ali.

A união é correcta *nesta* pergunta — que operações estão abertas. Não é como se
decide classificação: essa porta nunca alarga por se acumularem papéis.

### Âmbitos

`Scope` — `Institution`, `Unit`, `ResearchWorkspace`, `Resource` — é explícito,
nunca implícito. Um grant com âmbito **tem** de nomear o alvo; um grant
institucional **não pode** nomeá-lo. Ambas as regras são constraints da base de
dados, não convenções da aplicação.

### Grants explícitos

`explicit_access_grants` é o **único** caminho para material `RESTRICTED` sem
membership. Por construção, usá-lo custa mais do que pertencer:

- nomeia a permissão exacta — um grant de `documents.view` nunca vira
  `documents.download`;
- nomeia o âmbito e o alvo;
- exige uma **razão escrita** (mínimo 8 caracteres, imposto por constraint);
- regista quem concedeu e quando;
- pode expirar;
- é revogado por timestamp, nunca apagado.

Ninguém pode conceder o que não detém: a rota verifica o `can` do próprio
concedente antes de escrever a linha. Sem isso, quem tivesse `PermissionsManage`
conceder-se-ia tudo.

A liveness — não revogado, não expirado — é decidida em SQL, no repositório. A
camada de policy mantém-se pura e nunca pergunta que horas são.

### Acesso explicável

`explain(principal, permission, ctx, resource_id)` devolve a **origem** concreta
do acesso: papel técnico, membership de unidade, membership de workspace, ou
grant explícito. É o que permite a `GET /administration/members/{id}/access`
responder «porquê», e não apenas «sim».

## Alternatives

| Alternativa | Porque foi rejeitada |
|---|---|
| **Manter só os onze verbos** | Não distingue ver de descarregar, nem administrar de ler. Empurra a decisão para o chamador, que é onde ela se perde. |
| **Permissões em tabela, editáveis na interface** | Um editor de políticas é uma superfície de escalada de privilégios e um conjunto que nenhum teste fixa. Prematuro (`CLAUDE.md` §71); fica `PLANNED`. |
| **Permissões como strings livres** | Um erro de escrita torna-se uma negação silenciosa ou, pior, uma permissão que ninguém tem mas que parece existir. O enum torna-o um erro de compilação. |
| **ABAC completo com motor de políticas** | Poderoso e ilegível. A pergunta «porque é que esta pessoa tem acesso» deixaria de ter resposta curta. |
| **Grants sem expiração** | Um acesso excepcional que não caduca deixa de ser excepcional. A expiração é opcional, mas existe. |

## Consequences

**Positivas** — `if role == admin` não aparece em lado nenhum; acrescentar um
papel muda uma tabela em `ocinye-domain` e nada mais; a interface do Workspace
renderiza-se a partir das capacidades que o Core calculou, em vez de adivinhar
por nome de papel; o acesso é explicável a um revisor meses depois.

**Negativas, aceites** — 53 permissões são muitas para memorizar, e a tabela
papel → permissões cresce. Mitigado por `GET /administration/roles`, que a expõe
tal como o código a define. Alterar o que um papel concede exige um release, o
que é lento de propósito.

**Verificação** — `crates/ocinye-domain/src/policy/permission_tests.rs` enumera
em vez de amostrar: prova que nenhum papel sozinho abre `RESTRICTED`, que um
`UnitManager` de A não administra B, que um `ResearchLead` de um workspace não
gere outro, que um `ExternalCollaborator` sem membership não detém uma única das
53 permissões, que um viewer lê mas não descarrega, que um grant confere apenas a
permissão que nomeia e apenas no âmbito que nomeia, e que tudo o que `can`
permite, `explain` sabe justificar.
