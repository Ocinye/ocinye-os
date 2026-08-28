# Autorização

Decisões: [ADR-0100](../adrs/0100-authorization-model.md) (RBAC contextual,
fail closed) e [ADR-0101](../adrs/0101-permissions-scopes-and-grants.md)
(permissões nomeadas, âmbitos, grants explícitos). Este documento descreve como
funciona.

## Como se faz uma pergunta de autorização

```rust
can(principal, Permission::DocumentsDownload, &ctx, Some(document_id))
```

Toda a pergunta tem esta forma. **`if role == admin` não aparece em lado
nenhum**: acrescentar um papel muda uma tabela em `ocinye-domain`, não cinquenta
sítios de chamada.

### Duas portas independentes

| Porta | Responde a |
|---|---|
| **Permissão** | *Pode este actor fazer esta espécie de operação aqui?* |
| **Classificação** | *Pode ver este material em concreto?* |

**As duas têm de permitir.** É esta separação que permite ao `platform_admin`
administrar a plataforma sem com isso ganhar acesso a ciência `RESTRICTED`.

### As permissões efectivas

União de quatro origens, avaliada no contexto do recurso:

1. papéis técnicos de âmbito institucional;
2. o papel que o actor detém **na unidade desse contexto**;
3. o papel que detém **no research workspace desse contexto**;
4. grants explícitos vivos que se apliquem ali.

A união é correcta para *esta* pergunta — que operações estão abertas. Não é
como se decide classificação: essa porta nunca alarga por se acumularem papéis.

## Duas dimensões, deliberadamente separadas

### Posição institucional

`Founder`, `Director`, `UnitLead`, `PrincipalInvestigator`, `Researcher`,
`Engineer`, `Fellow`, `Student`, `ExternalCollaborator`.

**Concede zero permissões.** Não aparece sequer no tipo `Principal`: a política
não tem nada que a ver com ela.

### Papel técnico

| Papel | O que concede | O que **não** concede |
|---|---|---|
| `platform_admin` | Operar a plataforma: contas, papéis, nós, IA, audit. | Qualquer conteúdo científico. Nem `INTERNAL`. |
| `organisation_admin` | Pessoas, unidades, papéis; ver ideias e projectos. | Conteúdo `RESTRICTED`; administração da plataforma. |
| `unit_manager` | À escala institucional, quase nada. O seu poder é **contextual**: aplica-se dentro das unidades que efectivamente gere. | Outras unidades. |
| `research_lead` | O mesmo que `research_member` à escala institucional; lidera dentro dos workspaces onde é `lead`. | Outros workspaces. |
| `research_member` | Ver a estrutura; usar IA; criar agente pessoal. | Escrever fora dos seus workspaces. |
| `collaborator` | Ver que a instituição existe. | Praticamente tudo o resto. |
| `external_collaborator` | **Nada.** Zero permissões institucionais. | Tudo, excepto o que membership ou grant lhe der. |
| `auditor` | Ler evidência, dentro do âmbito autorizado. | Todo o conteúdo. Ter `audit.view` não é audit global. |

A tabela real é `crates/ocinye-domain/src/policy/permissions.rs`, e
`GET /api/v1/administration/roles` expõe-a tal como o código a define.

### Papéis são de sistema

Definidos em código, não em tabela. Um conjunto de permissões editável em tempo
de execução é um conjunto que nenhum teste consegue fixar, e esta é a camada
onde um teste exaustivo vale mais. **Papéis personalizados são `PLANNED`**
(ADR-0101).

### Memberships contextuais

- **Unidade:** `manager` ou `member`.
- **Research Workspace:** `lead`, `member` ou `viewer`.

É aqui que vive quase toda a autorização real.

## A regra de leitura

| Classificação | Quem lê |
|---|---|
| `PUBLIC`, `INTERNAL` | Qualquer membro activo |
| `CONFIDENTIAL` | Membro da unidade, membro do workspace, ou admin |
| `RESTRICTED` | **Só** membro explícito do workspace, ou gestor da unidade |

`RESTRICTED` ignora papéis administrativos. Um teste percorre **todos** os papéis
técnicos e afirma que nenhum, isolado, abre `RESTRICTED`.

Note a assimetria: no workspace, **qualquer** papel qualifica — incluindo
`viewer`. Na unidade, só **gestão**. Um membro comum de uma unidade não lê o
material `RESTRICTED` dessa unidade sem ser membro do workspace.

## Forma da recusa

| Situação | Resposta |
|---|---|
| Leitura negada | `not_found` — não revela existência |
| Escrita negada sobre recurso legível | `permission_denied` — seguro, a legibilidade já é conhecida |
| Escrita negada sobre recurso ilegível | `not_found` |

O **motivo** da recusa nunca chega ao chamador: é registado na auditoria, onde um
revisor o pode ler, em vez de ser entregue como pista do que teria funcionado.

## Escrita

| Âmbito | Quem escreve |
|---|---|
| Workspace | `lead` ou `member` do workspace; gestor da unidade. **`viewer` nunca.** |
| Unidade | Gestor da unidade, ou admin da organização |
| Organização | Admin da organização |

## Direitos mais estreitos do que ler

**Exportar `RESTRICTED`** exige `lead` do workspace ou gestor da unidade. Tirar
material restrito da instituição é deliberadamente mais estreito do que o
consultar.

**Classificar** e **gerir membros** exigem `lead`, gestor de unidade ou admin.
Mudar uma classificação exige um **motivo**, registado na auditoria.

## Listagens: o espelho SQL

A regra existe duas vezes: como decisão e como `WHERE`. `VisibilityFilter` é a
descrição; `visibility::to_sql` traduz.

O predicado faz parte da query, pelo que `LIMIT`, `OFFSET` e `COUNT` operam
apenas sobre o conjunto autorizado — um total que contasse linhas escondidas
revelaria a sua existência.

**As duas implementações têm de concordar**, e um teste exaustivo falha se uma
mudar sozinha.

## Fail closed

Todo o caminho termina numa autorização explícita. Não existe `_ => allow`. Um
membro inactivo é negado em todas as acções e classificações — incluindo se tiver
`platform_admin` e for `lead` do workspace.

## Âmbitos

`Institution` · `Unit` · `ResearchWorkspace` · `Resource`

Explícito, nunca implícito. Um grant com âmbito **tem** de nomear o alvo; um
grant institucional **não pode** nomeá-lo. Ambas as regras são constraints da
base de dados.

`Resource` usa-se com parcimónia: um grant por documento é gerível a dezenas e
ingerível a milhares. Prefira o workspace.

## Grants explícitos

O **único** caminho para material `RESTRICTED` sem membership. Por construção,
usá-lo custa mais do que pertencer:

| Campo | Obrigatório | Porquê |
|---|---|---|
| `permission` | Sim | Um grant de `documents.view` nunca vira `documents.download`. |
| `scope` + `scope_id` | Sim (excepto institucional) | Sem alvo não há grant. |
| `reason` | Sim, ≥ 8 caracteres | Um acesso que ninguém consegue justificar depois é um acesso que ninguém consegue rever depois. |
| `granted_by` | Sim | Sempre atribuível. |
| `expires_at` | Não | Um acesso excepcional que não caduca deixa de ser excepcional. |

**Ninguém pode conceder o que não detém.** A rota verifica o `can` do próprio
concedente antes de escrever a linha; sem isso, quem tivesse `permissions.manage`
conceder-se-ia tudo.

Revogar é um timestamp, nunca um `DELETE`: que um acesso existiu é parte do
registo institucional.

## Acesso explicável

> Porque é que esta pessoa consegue aceder a isto?

`GET /api/v1/administration/members/{id}/access` responde com a **origem** de
cada permissão, não com um sim/não:

| Origem | Significado |
|---|---|
| `technical_role` | Um papel de âmbito institucional. |
| `unit_membership` | Pertence à unidade que detém o recurso. |
| `workspace_membership` | Pertence ao research workspace. |
| `explicit_grant` | Um grant nomeado, atribuível e revisível. |

## O que a interface faz com isto

`GET /api/v1/me` devolve as permissões institucionais do chamador. O Workspace
usa-as para **não mostrar** o que o membro não pode usar: barra lateral, menu
`+ Criar` e command palette são todos filtrados por elas.

**Isto não é autorização.** Esconder um controlo é cortesia; a decisão está no
Core, e escrever o caminho à mão bate na mesma recusa. O que a filtragem evita é
enviar ao cliente uma lista de coisas que ele não devia sequer saber que existem
(`CLAUDE.md` §4, briefing §65).

## Verificação

```bash
cargo test -p ocinye-domain                      # enumera, não amostra
OCINYE_TEST_DATABASE_URL=… cargo test -p ocinye-core --test authorization
```

Os testes do domínio percorrem **todas** as combinações de papel, permissão e
classificação. Cada fronteira tem um teste ALLOW e um teste DENY, e os DENY são
os que importam.
