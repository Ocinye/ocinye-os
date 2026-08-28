# Feature & Capability Status

Estado factual das funcionalidades do Ocinye OS, apurado na auditoria transversal
de 2026-08-22 e revisto pela
[Security Baseline v1](../security/2026-08-23-security-baseline-v1.md) de
2026-08-23. **Nada aqui é aspiracional.** Cada linha foi verificada contra o
código, a base de dados e o comportamento HTTP real.

Princípio que este documento serve:

> **Se um membro vê uma opção no Ocinye Workspace, essa opção tem comportamento
> definido.**

Ver também: [UI ↔ Core contract](../ui-core-contract/README.md).

## Vocabulário

| Estado | Significado |
|---|---|
| `AVAILABLE` | Funciona agora, de ponta a ponta. |
| `NO_RESOURCE` | Nada foi registado que a sirva. **Não é erro** — é o estado normal antes da infraestrutura existir. |
| `NOT_CONFIGURED` | Existe quem a sirva, mas esta instalação não foi configurada para a usar. |
| `UNAVAILABLE` | Configurada e registada, mas não responde. |
| `DEGRADED` | Responde, mas não por completo. |
| `PLANNED` | Decidida e desenhada, não construída. |

Os quatro primeiros são apurados em tempo real pelo Core e servidos em
`GET /api/v1/system/capabilities`. `PLANNED` é uma afirmação deste documento.

## Matriz

| Capacidade | Core | Workspace | Infraestrutura | Estado |
|---|---|---|---|---|
| **Identidade e sessões** | implementado | implementado | n/a | `AVAILABLE` |
| **Autorização, permissões, grants** | implementado | permission-aware | n/a | `AVAILABLE` |
| **Unidades** | implementado | implementado | n/a | `AVAILABLE` |
| **Ideias** | implementado | lista, criação, workspace | n/a | `AVAILABLE` |
| **Projectos** | implementado | lista, workspace | n/a | `AVAILABLE` |
| **Research Workspaces** | implementado | 13 tabs, 2 funcionais | n/a | `AVAILABLE` (parcial — ver abaixo) |
| **Ideia → Projecto** | implementado, idempotente | botão declarado indisponível | falta ecrã de conversão | `AVAILABLE` pela API e pelo plano agentic; `PLANNED` no ecrã |
| **Relações tipadas entre artefactos** | implementado | listagem | n/a | `AVAILABLE` |
| **Bibliografia** | implementado | lista | n/a | `AVAILABLE` |
| **Notas, documentos, datasets** | implementado | leitura | MinIO local; nenhum armazenamento institucional | `AVAILABLE` em desenvolvimento |
| **Pesquisa textual** | implementado | implementado | PostgreSQL FTS | `AVAILABLE` |
| **Pesquisa semântica** | preparado | declarada indisponível | sem embeddings | `NO_RESOURCE` |
| **Agentes de IA** | implementado | lista, criação | não requer nó | `AVAILABLE` |
| **Execução de agentes** | — | estado derivado | sem nó de IA | `NO_RESOURCE` |
| **Prompt Ocinye** | endpoint implementado | implementado | sem nó de IA | `NO_RESOURCE` |
| **Inferência (IA geral/coding/reasoning)** | Gateway preparado | integrado | sem nó de IA | `NO_RESOURCE` |
| **Embeddings** | preparado | — | sem nó de IA | `NO_RESOURCE` |
| **Correio — modelo, permissões, política** | implementado | n/a | n/a | `AVAILABLE` |
| **Correio — leitura e envio** | implementado | 7 ecrãs | serviço ausente | `NOT_CONFIGURED` |
| **Correio — transporte IMAP** | implementado | n/a | serviço ausente | `NOT_CONFIGURED` |
| **Correio — sincronização (`mail.sync`)** | manual, implementada | botão «Actualizar» | sem worker de ingestão | `DEGRADED` |
| **Correio — assistência de escrita** | implementado | implementado | sem nó de IA | `NO_RESOURCE` |
| **Correio — anexos no envio** | modelo definido | — | depende de armazenamento institucional | `PLANNED` |
| **Correio — descarga de anexos** | adaptador lê-os | declarada indisponível | falta rota e ecrã | `PLANNED` |
| **Correio — administração de caixas partilhadas** | consultas implementadas | sem ecrã | n/a | `PLANNED` |
| **Capability Registry** | conjunto fechado ([matriz](../agentic/operation-capability-matrix.md)) | administração | n/a | `AVAILABLE` |
| **Arranque institucional e prontidão** | `/ready` com projecção pública fechada, compatibilidade e criticalidade | Splash, portão de entrada, entrega de sessão, destino profundo | n/a | `AVAILABLE` |
| **Calendário e Centro Temporal** | eventos, prazos projectados, quatro vistas | `/calendar` e o relógio da barra superior | n/a | `AVAILABLE` |
| **Lembretes** | entregues pelo worker institucional | Centro Temporal e formulários | n/a | `AVAILABLE` |
| **Notificações** | entrega in-app; correio fica para depois | sino da barra superior | n/a | `AVAILABLE` |
| **Capability Executor** | implementado | n/a | n/a | `AVAILABLE` |
| **Context Engine** | implementado | n/a | n/a | `AVAILABLE` |
| **Context Engine — selecção de recursos** | implementado | superfícies contextuais | n/a | `AVAILABLE` |
| **Resolução autorizada de `ResourceRef`** | implementado, 8 tipos | n/a | n/a | `AVAILABLE` |
| **Action Planner** | implementado | plano visível | n/a | `AVAILABLE` |
| **Aprovações e risco** | portão no executor, e o lifecycle persistido que o exercita | confirmar / rejeitar | n/a | `AVAILABLE` |
| **Lifecycle de planos agentic** | `create_plan` ligado ao Runtime; `list · get · approve · reject · execute`; reclamação atómica; reautorização em execução | ligado aos controlos existentes de `/ask` | requer um plano, logo requer inferência para o produzir a partir de linguagem natural | `AVAILABLE` (o ciclo); `NO_RESOURCE` (produzir o plano sem nó de IA) |
| **Action / Plan History como produto** | a persistência existe; a experiência não | — | — | `PLANNED` — pesquisa, cronologia, diff, exportação e analytics são milestone própria |
| **Main Agent / Agent Runtime** | implementado | Universal Command Surface | sem nó de IA | `NO_RESOURCE` |
| **Universal Command Surface — Pesquisar** | implementado | implementado | não requer IA | `AVAILABLE` |
| **Universal Command Surface — Perguntar / Executar** | implementado | declarada indisponível | sem nó de IA | `NO_RESOURCE` |
| **Research agent-addressable** | 8 capabilities | superfície contextual em Ideia e Projecto | sem nó de IA para `Perguntar`/`Executar` | `AVAILABLE` (leitura e escrita pelo Core); `NO_RESOURCE` (síntese) |
| **Knowledge agent-addressable** | 9 capabilities | superfície contextual no acervo | idem | `AVAILABLE`; `NO_RESOURCE` (síntese) |
| **Collaboration agent-addressable** | 4 capabilities: criar, listar, transitar, atribuir | tarefas no Research Workspace | idem | `AVAILABLE` |
| **Unidade endereçável por `ResourceRef`** | resolvida por `organisation::get_unit` | n/a | n/a | `AVAILABLE` — é o âmbito em que uma Ideia nasce |
| **Superfícies contextuais de assistência** | n/a | Ideia · Projecto · Conhecimento | sem nó de IA | `NO_RESOURCE` — visível, com a razão |
| **Domain Agents com prompt próprio** | domínio é fronteira no registry | — | — | `PLANNED` |
| **Contrato canónico de inferência** | implementado, versionado | n/a | n/a | `AVAILABLE` |
| **Provider Conformance Suite** | 10 verificações | n/a | não requer GPU | `AVAILABLE` |
| **Detecção de intenção** | determinística | superfície de comando | não requer IA | `AVAILABLE` |
| **L40S / nó de IA** | Gateway e contrato preparados | estado real | **não provisionado** | `NO_RESOURCE` |
| **IA proactiva** | — | — | — | `PLANNED` |
| **Workflows autónomos** | tecto é `Workflow` | — | — | `NOT IMPLEMENTED` |
| **Jobs agentic em segundo plano** | — | — | — | `PLANNED` |
| **Compute Registry** | implementado | implementado | 0 nós | `NO_RESOURCE` |
| **Submissão de jobs** | permissão definida | — | 0 nós | `PLANNED` |
| **Node Agent** | protocolo implementado | n/a | nenhum nó | `NO_RESOURCE` |
| **Capability Runtime (WASM)** | implementado; invocado por `knowledge::review_bibliography` | n/a | local | `AVAILABLE` |
| **Ferramentas bibliográficas** | `knowledge::review_bibliography`, executada no isolamento WASM/WASI | ecrã em Bibliografia; capability agentic `knowledge.bibliography.review` | n/a | `AVAILABLE` |
| **Object Storage** | implementado | leitura | não configurado | `NOT_CONFIGURED` |
| **Auditoria** | implementado | implementado | n/a | `AVAILABLE` |
| **Actividade** | implementado | implementado | n/a | `AVAILABLE` |
| **Administração de membros** | implementado | criar, detalhe, acesso, segurança | n/a | `AVAILABLE` |
| **Notificações** | **não existe** | removido | n/a | `PLANNED` |
| **Definições / tema / idioma** | não existe | declarado indisponível | n/a | `PLANNED` |

## Ocinye AI — estado detalhado

**Nenhum nó de IA Ocinye está registado.** Isto é um estado operacional, não uma
lacuna arquitectural.

| Peça | Estado | Nota |
|---|---|---|
| AI Gateway | implementado | Pede **capacidades**, nunca modelos (ADR-0300). |
| Model Registry | implementado | Tabela `ai_models`; vazia. |
| Capacidades | `GENERAL` · `CODING` · `REASONING` · `EMBEDDING` | Todas `NO_RESOURCE`. |
| Agentes | `AVAILABLE` | Definíveis e persistidos **sem nó**. |
| Estado de agente | derivado | `configured` sem capacidade, `ready` com ela. |
| Prompt Ocinye | `NO_RESOURCE` | O endpoint existe e recusa com 503 e razão institucional. |
| Contexto RAG | implementado | Permission-aware; nunca executado por falta de modelo. |
| Anexos ao prompt | `PLANNED` | Chips visíveis e declarados indisponíveis. |
| Fornecedor externo | **nunca** | Não é ligado para disfarçar a ausência (ADR-0300). |

### O que acontece quando um nó for registado

Verificado com uma fixture durante a auditoria: inserir **um** nó e **um** modelo
com `capabilities: ["GENERAL","CODING"]` fez, sem alterar código nem migration:

- `ai.general` e `ai.coding` passarem de `no_resource` a `available`;
- `compute` passar a `available` — «1 de 1 nós activos»;
- os agentes existentes passarem de `configured` a `ready`;
- `ai.reasoning` e `ai.embedding` passarem a `not_configured`, que é diferente de
  `no_resource` e diz a verdade: há nó, não há modelo para essa capacidade.

**Não é preciso reconstruir o Workspace.** É a invariante que a arquitectura de
capacidades existe para garantir.

## Ocinye Compute — estado detalhado

| Peça | Estado |
|---|---|
| Compute Registry | implementado, `0` nós |
| Enrollment | implementado |
| Heartbeat | implementado |
| Identidade de máquina | implementada |
| Reporte de recursos e capacidades | implementado |
| Modelos instalados | implementado |
| Health e `last_seen` | implementado |
| Submissão de jobs | `PLANNED` — permissão definida, sem execução |

**Nenhuma lógica depende de `CAM-01`.** O registo aceita `0..N` nós, e o nome
`CAM-01` não aparece em nenhum caminho de código.

## Research Workspace — tabs

Treze tabs por dossier. Duas navegam, as restantes estão **declaradas
indisponíveis** e não são clicáveis:

| Tab | Estado |
|---|---|
| Visão geral | `AVAILABLE` |
| IA | `AVAILABLE` — leva ao Prompt vinculado |
| Bibliografia · Fontes · Notas · Documentos · Datasets · Código · Experiências · Resultados · Tarefas · Actividade · Histórico | `PLANNED` |

## Listas — recortes

Os separadores «Minhas», «Da Unidade», «Seguidas», «Arquivadas» das listas estão
**declarados indisponíveis**: o Core ainda não os expõe como parâmetros de
consulta. O separador activo é o único que funciona.

O campo de pesquisa de cada lista **filtra localmente** as linhas visíveis. É
deliberado e documentado no código: o Workspace pede uma página, e o filtro ajuda
a encontrar dentro dela.

## Paginação

Retirada das listas. O Core pagina, mas o Workspace pede uma página só; mostrar
controlos de página seria afirmar que há mais páginas.

## O que foi removido nesta auditoria

| Elemento | Porquê |
|---|---|
| Sino de notificações | Botão sem handler, com ponto de «não lidas» falso. O Core não tem conceito de notificação. |
| Controlos de paginação | Sem handler; o «seguinte» aparecia activo. |
| Botão «Filtrar» das listas | Sem handler e sem painel. |
| Campo «Modelo base» do construtor de agentes | Acoplava a UX a nomes de modelo, contra o §41 do `CLAUDE.md`. |
| Controlo segmentado de âmbito | `<button>` sem `name`: o âmbito nunca era submetido. |
| Afirmação «⏎ enviar» no Prompt | Sem JavaScript, Enter numa textarea insere uma linha. |
