# Agentic Control Plane

Decisões: [ADR-0002](../adrs/0002-deterministic-core-and-agentic-control-plane.md) ·
[ADR-0301](../adrs/0301-agentic-control-plane.md) ·
[ADR-0302](../adrs/0302-agent-access-intersection.md) ·
[ADR-0303](../adrs/0303-capability-registry-and-executor.md) ·
[ADR-0304](../adrs/0304-canonical-inference-contract.md)

Segurança: [security.md](security.md).
Contrato de inferência: [provider-contract.md](provider-contract.md).
Acrescentar uma capability: [capability-authoring.md](capability-authoring.md).

## Uma capacidade agentic não é o Capability Runtime

A colisão de vocabulário custa a quem lê pela primeira vez, e vale a pena
desfazê-la antes de tudo o resto.

**Uma capacidade agentic** é uma operação tipada que o Agentic Control Plane
pode solicitar: tem identificador, permissão, âmbito, risco e esquema de
entrada, e converge numa Core Operation.

**O Capability Runtime** é a infraestrutura WebAssembly/WASI que o Core pode
usar para executar computação isolada.

São conceitos diferentes, e o caminho entre eles é sempre o mesmo:

```text
Agentic Capability  →  Core Operation  →  Capability Runtime
```

Um agente nunca alcança o Runtime. `knowledge.bibliography.review` é a primeira
capacidade agentic cuja Core Operation atravessa o isolamento — e o que o agente
pede é a revisão da bibliografia, não a execução de um componente.

## As duas frases

> **Ocinye OS is AI-native, not AI-dependent.**

> **Ocinye OS is operated with AI, governed by the Core.**

## Estado

| Componente | Estado | Nota |
|---|---|---|
| Contratos agentic | **`CURRENT`** | `CapabilityId`, `ResourceRef`, `RiskLevel`, `ActionPlan`, `AutonomyLevel` |
| Política de acesso agentic | **`CURRENT`** | Pura, em `ocinye-domain`, exaustivamente testada |
| Capability Registry | **`CURRENT`** | conjunto fechado, definido em código; contagem na [matriz](operation-capability-matrix.md) |
| Capability Executor | **`CURRENT`** | Resolver capability → resolver recursos → autorizar → validar → aprovação → executar → auditar |
| Context Engine | **`CURRENT`** | Dois tectos: leitura e processamento por IA |
| Action Planner | **`CURRENT`** | Valida a saída do modelo; digest liga a aprovação |
| Aprovações | **`CURRENT`** | Pessoa + digest + 15 minutos |
| Lifecycle de planos | **`CURRENT`** | Proposta validada → persistida → consentida → reautorizada → executada uma só vez |
| Agent Runtime | **`CURRENT`** | Degrada declaradamente sem nó de IA |
| Main Agent | **`CURRENT`** | Lista mais larga, zero privilégio |
| Universal Command Surface | **`CURRENT`** | Search · Ask · Act, com Search a funcionar sem IA |
| Contrato canónico de inferência | **`CURRENT`** | `InferenceProvider`, versionado. Nenhum formato de fornecedor no Runtime |
| Guarda de prazo, versão e tamanho | **`CURRENT`** | Aplicado pelo Core, do lado do Core |
| Provider Conformance Suite | **`CURRENT`** | 10 verificações; corre em segundos, sem GPU |
| Fornecedor determinístico de teste | **`CURRENT`** | Atrás de feature; ausente de binários de release |
| Detecção de intenção | **`CURRENT`** | Determinística; ambiguidade cai sempre para pesquisar |
| **Inferência** | **`NO_RESOURCE`** | Zero nós. `Ask` e `Act` declaram-se indisponíveis |
| Domain Agents com prompt próprio | **`PLANNED`** | O domínio já é fronteira no registry |
| Trabalho proactivo | **`PLANNED`** | Observe → Suggest, nunca Observe → Execute |
| Autonomia `Autonomous` | **`NOT IMPLEMENTED`** | Existe no tipo; o tecto é `Workflow` |

## Anatomia

```
crates/ocinye-contracts/src/agentic.rs      vocabulário: identificadores, risco, planos
crates/ocinye-domain/src/policy/agentic.rs  a intersecção de acesso. Pura.
crates/ocinye-core/src/modules/agentic/
  ├── registry.rs       o que o Core publica
  ├── executor.rs       onde a acção se torna estado
  ├── context.rs        contexto mínimo e autorizado
  ├── planner.rs        onde a saída do modelo deixa de ser confiável
  ├── runtime.rs        Main Agent e orquestração
  ├── repository.rs     planos e aprovações, com transições atómicas
  ├── lifecycle.rs      list · get · approve · reject · execute
  └── capabilities/     um ficheiro por domínio
services/core-server/src/routes/agentic.rs  a API
apps/workspace/src/ui/screens/ask.rs        a Universal Command Surface
crates/ocinye-core/tests/agentic_lifecycle.rs  o ciclo, contra PostgreSQL
migrations/0011_agentic.sql                 2 tabelas
```

## O conjunto das capabilities

A lista não se escreve aqui. Vive na
[matriz de operações](operation-capability-matrix.md), emitida pelo catálogo
tipado em `crates/ocinye-core/src/operations.rs` e verificada em CI por
`./scripts/operation-matrix.sh --check`.

Esta secção já teve a tabela escrita à mão, com a contagem no título. Foi assim
que três contagens diferentes chegaram a circular ao mesmo tempo — este
documento, um relatório e o código —, cada uma correcta no dia em que foi
escrita. A matriz mostra, por operação e numa linha cada, o identificador, o
módulo, a exposição, a capability quando existe, e a classe de fronteira quando
não existe.

**Um subconjunto fechado, e não tudo o que a API sabe fazer.** O Core expõe muito
mais pela sua API HTTP; uma capability existe onde há uma operação institucional
coerente, segura de ser *proposta* por um modelo, e que justifica a cobertura de
testes que cada entrada carrega. Desde
[ADR-0307](../adrs/0307-dual-entry-single-authority.md), a ausência de capability
deixou de poder ser omissão: toda a operação significativa está classificada, e
uma que não seja endereçável diz que fronteira de confiança atravessa.

**Rever não é transitar, e transitar não é reclassificar.** `idea.revise` e
`note.revise` tocam texto que um membro escreveu e nada mais: o estado move-se
por `idea.transition`, e a classificação não se muda por nenhuma capability. Um
schema que aceitasse os três seria uma governação escondida dentro de uma
edição (briefing §12).

**Atribuir trabalho é dá-lo a quem o consegue ver.** `task.assign` verifica que
a pessoa nomeada poderia ler a tarefa — a mesma decisão que a deixaria abri-la.
Um identificador ser real não é evidência de nada.

**O registry cresce à medida que cada domínio é auditado.** Transformar cada
endpoint automaticamente numa ferramenta produziria cem portas por testar e
nenhuma fronteira que valha o nome. Ver
[capability-authoring.md](capability-authoring.md).

### Endereçamento: o recurso vem por `resources`

Uma capability de âmbito de unidade ou de workspace **nomeia o seu recurso por
`resources`**, nunca por um identificador no `input`.

Isto não é estilo. O executor autoriza um passo que não nomeia recurso nenhum
contra o contexto do *pedido* — a organização, sem unidade e sem ambiente — e
uma permissão que vem de pertença não existe aí. Uma capability que receba o
identificador pelo `input` é portanto autorizada contra a organização e fica
**inalcançável por exactamente as pessoas que têm a permissão**. Falha fechada,
e em silêncio.

Foi o que aconteceu a `research.idea.create` e a `collaboration.task.create`,
que sobreviveram assim a duas milestones. `every_membership_scoped_capability_is_reachable_by_a_member`
percorre agora o registry e mede a propriedade, em vez de confiar em que cada
handler novo se lembrou.

### O que foi deliberadamente não exposto

Tão importante como a lista acima:

| Operação | Porquê não |
|---|---|
| **Membership de ambiente ou unidade** | Altera quem pode ver o quê. É uma operação de segurança, não uma edição de projecto (`CLAUDE.md` §34). |
| **Mudança de classificação** | Baixar uma classificação é uma decisão institucional sobre exposição. Fica manual. |
| **Qualquer eliminação definitiva** | Existe arquivo, existe desligar uma relação, existe versionamento. Não é preciso apagar. |
| **Upload de documentos** | Os bytes chegam por multipart e dependem de object storage. Não tem forma de operação agentic. |
| **Registar base legal de conteúdo integral** | Decisão jurídica de uma pessoa. Ver a posição da instituição em [`docs/knowledge/`](../knowledge/README.md). |
| **Edição de Notas existentes** | Criar acrescenta; editar a prosa de outra pessoa através de um agente é uma pergunta diferente, ainda não respondida. |
| **Pesquisa por tipo de entidade** | `knowledge.search` já cobre os seis tipos indexados, com a política aplicada dentro da query. Uma capability por tipo seria duplicação. |

## Endereçar recursos

Uma capability que actua sobre um recurso específico **não** recebe o
identificador no `input`. Recebe-o como `ResourceRef`, e o executor resolve-o
antes de decidir seja o que for.

```
ResourceRef{kind, id}  →  serviço de domínio que o detém  →  ResolvedResource
                                                              ├── contexto real
                                                              ├── classificação real
                                                              └── título do Core
```

Três consequências que valem a pena reter:

1. **A autorização acontece contra o contexto do recurso**, não contra o do
   pedido — que para um passo de plano é a instituição, e não nomeia unidade
   nenhuma de que uma referência estrangeira possa estar fora.
2. **Uma referência inventada e uma para outra unidade dão a mesma resposta.**
3. **A `label` que o modelo escreveu é substituída pela do Core.** Um plano que
   descreve um recurso com palavras que ninguém verificou é um plano confirmado
   sob uma descrição errada.

Decisão: [ADR-0306](../adrs/0306-resource-resolution-as-authorization-boundary.md).

## Selecção e contexto

O Context Engine recebe material por dois caminhos:

| | **Recuperação** | **Selecção** |
|---|---|---|
| Como chega | A pesquisa encontra | O membro aponta |
| Ordem no envelope | Depois | **Primeiro** |
| Passa pelo resolver | Sim, indirectamente | **Sim, explicitamente** |
| Se não for alcançável | Não aparece | **O pedido pára** |

A selecção não é um atalho a nada: muda a relevância, não a autoridade. E uma
selecção inalcançável pára o pedido em vez de ser silenciosamente descartada,
porque responder sobre material diferente daquele para que a pessoa apontou é
pior do que não responder.

Ambos os caminhos passam pelo tecto de processamento com IA, que é mais baixo do
que o de leitura. Material retido é **contado e declarado**.

## O fluxo que prova a arquitectura

O cenário do correio, com cada seta a ser código real e testada em
`crates/ocinye-core/tests/agentic.rs`:

> «Encontra o último email do Carlos sobre o Project BESS e prepara uma resposta
> dizendo que enviaremos a versão revista sexta-feira.»

```
Main Agent → mail.search → mail.read → Context Engine → mail.draft_reply → PÁRA
```

O rascunho existe. **Nada saiu da instituição.**

> «Torna mais curto e mais formal.»

```
mail.draft_transform  (sem nó de IA: recusa com razão, e o rascunho fica intacto)
```

> «Enviar.»

```
Risco 3 → aprovação humana → autorização do Core → provider → verificação → auditoria
```

Sem confirmação, `mail.send` devolve `ApprovalRequired` e **não oferece Undo a
algo que nem correu**.

## O que funciona hoje, sem nenhum nó de IA

- **Pesquisar** na Universal Command Surface — determinístico;
- o caminho agentic inteiro, contra o fornecedor determinístico de teste;
- o Capability Registry responde;
- o Capability Executor autoriza, valida, executa e audita;
- as aprovações ligam-se a planos e caducam;
- toda a interface tradicional do Workspace.

## O que não funciona, e diz porquê

**Perguntar** e **executar** a partir de linguagem natural. Devolvem
`AgenticOutcome::Unavailable` com a razão que o Core deu e com o que continua a
funcionar. A interface renderiza isso como estado do ecrã, não como erro.

O caminho está **escrito e testado**: monta o envelope, pede `GENERAL` ao
Gateway, valida a proposta com o planner. O que falta é um adapter que sirva
`GENERAL`.

Que isto seja verdade e não uma promessa é o que o
[contrato canónico](../adrs/0304-canonical-inference-contract.md) e o fornecedor
determinístico de teste demonstram: o mesmo código corre hoje, com quatro
comportamentos de modelo — cooperativo, hostil, malformado e ausente.

Quando a L40S for provisionada, o serviço de inferência que correr nesse nó é
integrado por um **Ocinye Provider Adapter** que implementa o contrato, passa a
Conformance Suite e entra no Model Registry. A GPU é hardware; o adapter é
software; **nada acima do adapter muda**
([provider-contract.md](provider-contract.md)).

## API

| Método | Caminho | O que faz |
|---|---|---|
| `POST` | `/api/v1/agentic/invoke` | A superfície de comando. Search sem modelo |
| `GET` | `/api/v1/agentic/capabilities` | O que o Ocinye OS publica, com o que este membro pode usar |
| `GET` | `/api/v1/agentic/plans` | «O que é que o Ocinye fez por mim?» Os próprios, paginados |
| `GET` | `/api/v1/agentic/plans/{id}` | Um plano, se for o do requerente |
| `POST` | `/api/v1/agentic/plans/{id}/approve` | Confirmar. Liga-se ao digest. **Não executa** |
| `POST` | `/api/v1/agentic/plans/{id}/reject` | Recusar. Terminal |
| `POST` | `/api/v1/agentic/plans/{id}/execute` | Executar. Reclama o plano, e reautoriza |

> **«Reautoriza» quer dizer o que diz desde
> [ADR-0411](../adrs/0411-execution-time-principal-freshness.md).** Antes disso, o
> executor autorizava contra o `Principal` que lhe entregassem, e a frescura vinha
> de a rota HTTP o carregar a cada pedido. Hoje a autoridade volta a
> estabelecer-se dentro do executor, à fonte canónica, a partir da identidade
> guardada com o plano.

**Não existe** uma rota que corra uma capability por identificador. A execução
passa por um plano, validado, que a pessoa detém e — quando altera algo material
— confirmou.

### O ciclo de um plano

```
proposta validada  →  action_plans          (proposed | awaiting_approval)
       ↓
   GET /plans/{id}                          o requerente, e mais ninguém
       ↓
   POST /approve   →  action_approvals      pessoa + digest + 15 minutos
       ↓                                    e nada é executado
   POST /execute
       ↓
   UPDATE … WHERE state = ANY(abertos)      reclamação atómica: um só vence
       ↓
   aprovação exigida hoje? — pelo registry, não pelo risco guardado
       ↓
   Capability Executor, passo a passo       reautoriza contra o actor de agora
       ↓
   settle                                   completed | partially_completed | failed
```

Quatro propriedades, cada uma com teste em
[`agentic_lifecycle.rs`](../../crates/ocinye-core/tests/agentic_lifecycle.rs):

- **Um plano é do requerente.** Conhecer o identificador não é permissão: ler,
  aprovar, rejeitar e executar respondem todos «não encontrado» a outra pessoa.
- **Aprovar não executa.** São dois actos, e continuam a ser dois pedidos.
- **Aprovação não é autorização.** Revogar um acesso depois de confirmar impede
  a execução; a confirmação continua registada, e continua a não ser autoridade.
- **Um efeito acontece no máximo uma vez.** A reclamação é um `UPDATE`
  condicional em PostgreSQL, e não um lock em memória — que uma segunda
  instância do Core não partilharia.

**O que isto não promete:** *exactly-once* contra sistemas externos.
`Core → SMTP` não é uma transacção ACID, e nenhuma quantidade de bloqueio local
a torna uma. O que fica impedido é o Ocinye repetir um plano já executado.

## Limitações declaradas

- **Inferência: `NO_RESOURCE`.** Zero nós de IA.
- **Domain Agents com prompt próprio: `PLANNED`.** O domínio é hoje fronteira no
  registry e no Context Engine, não um agente com instruções próprias.
- **`mail.send` agentic devolve indisponível**: o envio pertence ao pipeline que
  `POST /mail/send` detém, e duplicá-lo seria duplicar a política de
  classificação.
- **Sem jobs agentic em segundo plano.** Um pedido longo não se torna job.
- **Action / Plan History como produto: `PLANNED`.** A persistência do lifecycle
  existe porque a segurança a exige; pesquisa, cronologia, diff, exportação e
  analytics sobre planos são milestone própria.
- **Um plano só nasce de inferência.** Com zero nós de IA, o ciclo está
  implementado e testado, e não há por onde produzir a proposta que o inicia a
  partir de linguagem natural.
- **Sem trabalho agendado nem automação por evento.** `PLANNED`.
