# Intelligence Plane — Ocinye AI Gateway

**Estado: 0 fornecedores. Nenhum nó de IA da Ocinye existe.**

Esta secção existe na plataforma e reporta esse estado. Não o esconde, e não
recorre a um fornecedor externo para o disfarçar.

## O princípio

A IA é uma **capacidade transversal** da Ocinye, não um módulo, um departamento
ou um chatbot. A arquitectura é **AI-native mas não AI-dependent**: a plataforma
funciona integralmente sem qualquer modelo disponível.

## Capacidades, nunca modelos

A aplicação pede `GENERAL`, `CODING`, `REASONING` ou `EMBEDDING`. O mapeamento
para um modelo concreto é **configuração**:

```
OCINYE_AI_CAPABILITY_MAP=GENERAL=qwen2.5,CODING=qwen2.5-coder,REASONING=deepseek-r1
```

Vazio por omissão. **Nenhum nome de modelo aparece no código.** Ligar um nó com
modelos diferentes muda o comportamento sem alterar código.

## Indisponível é uma resposta correcta

Sem nó enrolado, `resolve_capability` devolve `capability_unavailable` e a
interface diz que nenhum nó de IA da Ocinye está disponível.

Isto não é uma avaria e nunca quebra a plataforma: as funcionalidades que
quereriam IA degradam de forma explícita e informada.

## Nenhum fornecedor externo automático

O Core **não** contacta OpenAI, Anthropic, Google ou qualquer outro. O tipo
`ProviderKind::External` existe para representar uma decisão institucional
futura, mas nunca é seleccionado implicitamente: exige
`OCINYE_AI_ALLOW_EXTERNAL_PROVIDERS`, registo explícito do fornecedor e um ADR
próprio que analise confidencialidade e residência de dados.

## RAG permission-aware por construção

A montagem de contexto aplica a política de leitura **do próprio requerente**,
usando o mesmo caminho de pesquisa que qualquer outra query. Um modelo nunca pode
receber um artefacto que quem pergunta não conseguiria abrir.

Depois disso aplica-se o **tecto do modelo**: um modelo aprovado apenas para
`INTERNAL` nunca recebe `CONFIDENTIAL`, mesmo para quem o possa ler.

`GET /api/v1/ai/context-preview` mostra exactamente o que uma recuperação
colocaria no contexto. Existe para que a fronteira seja inspeccionável **antes de
existir qualquer modelo** que a consuma.

## Prompt injection

Conteúdo recuperado é **dados**, potencialmente hostis — nunca instrução. As
quatro camadas são estruturalmente distintas: system policy, application policy,
user input, retrieved content.

Conteúdo recuperado não pode alterar permissões, escalar privilégios nem
desencadear acções com efeitos.

## Rastreabilidade

Cada `AiJob` regista capacidade, modelo, versão, âmbito, momento, requerente e as
**referências** dos artefactos recuperados. Prompts e respostas **não** são
persistidos: a proveniência de uma resposta é que artefactos a informaram, não
uma segunda cópia deles.

## Quando `CAM-01` existir

1. Registar o nó (`POST /compute/nodes`).
2. Instalar o agente com o token de enrolamento.
3. O agente reporta os modelos que tem.
4. Definir `OCINYE_AI_CAPABILITY_MAP`.
5. As capacidades passam a disponíveis.

**Sem alterações de código.** É essa a diferença entre acrescentar um nó e
reescrever a aplicação.

## Não implementado

- Inferência propriamente dita — não há nada para onde a encaminhar.
- Geração de embeddings, e portanto pesquisa semântica.
- Agentes.
- Adaptadores para fornecedores externos.

## Agentes

Um agente é uma **definição**: nome, propósito, instruções, a capacidade que
pede, o âmbito onde vive e o conhecimento a que pode recorrer.

**Definir um agente não precisa de modelo**, e por isso `ai_agents` é utilizável
com zero nós registados. O que um modelo em falta impede é a **execução**, e esse
estado é **derivado** em cada leitura — nunca guardado:

| Estado | Quando |
|---|---|
| `ready` | Uma capacidade compatível pode servi-lo agora. |
| `configured` | Definido e completo, sem capacidade que o sirva. **O estado normal antes do primeiro nó.** |
| `disabled` | O seu dono desactivou-o. |
| `archived` | Arquivado. |

Um agente **nunca** diz `active` quando nada o consegue executar.

### O agente nunca alarga quem o usa

```
acesso efectivo = intersecção(acesso do actor, acesso do agente, política do recurso)
```

Nunca a união. `max_classification` é um tecto, verificado na criação contra o
que o criador consegue ler, e verificado outra vez na recuperação de contexto.

### Âmbitos e permissões

| Âmbito | Permissão exigida |
|---|---|
| `personal` | `agents.create.personal` |
| `workspace` | `agents.create.project` |
| `unit` | `agents.create.unit` |
| `institutional` | `agents.create.institutional` |

Um `PlatformAdmin` **não** detém nenhuma destas: administração técnica não é
acesso científico (`CLAUDE.md` §34).

## Prompt Ocinye sem nó

`POST /api/v1/ai/prompt` existe e responde. Sem capacidade disponível devolve
**503 `capability_unavailable`** com a razão institucional, que o Workspace
renderiza como estado nativo do ecrã — nunca um alerta do browser.

O pedido recusado é registado como job rejeitado: é a evidência de procura por
uma capacidade que ainda não existe, e é o que justifica adquirir um nó.

**Anexar contexto** — ficheiros, datasets, documentos — está `PLANNED`. Os chips
continuam visíveis no dock, declarados indisponíveis com a razão.

## Assistência no correio

O caso mais exposto de *prompt injection* em todo o Ocinye OS, porque qualquer
pessoa no mundo pode enviar texto para um endereço da Ocinye.

Três defesas, em camadas:

1. **Conjunto fechado de dez acções.** O modelo nunca recebe um verbo que um
   email tenha proposto.
2. **Blocos de dados delimitados** (`<<<EMAIL_RECEBIDO`, `<<<RASCUNHO`,
   `<<<PEDIDO_DO_MEMBRO`), com o sistema a declarar que o primeiro é material a
   processar e não instruções a seguir.
3. **A assistência não tem poderes.** Devolve uma `String`. Não envia, não move
   mensagens, não lê outras caixas, não altera permissões. Uma injecção bem
   sucedida escreve texto estranho num campo que o membro lê antes de enviar.

E a separação que torna o resto verificável: **texto gerado não é mensagem
enviada**. `POST /mail/assist` e `POST /mail/send` são rotas distintas, e a
primeira não chama a segunda ([ADR-0406](../adrs/0406-ai-generated-is-not-sent.md)).

`SendPolicy::may_use_as_ai_context` permite `PUBLIC` e `INTERNAL`. `human_read`
não implica `ai_processing_allowed`.

Detalhe: [docs/mail/ai.md](../mail/ai.md).

## O contrato pertence ao Ocinye

Nenhum modelo, fornecedor, GPU ou servidor de inferência define o contrato
interno do Ocinye OS. Qwen adapta-se ao contrato; DeepSeek adapta-se ao
contrato; uma futura L40S adapta-se ao contrato.

```
Agent Runtime → contrato canónico → InferenceProvider → adapter → modelo
```

Formatos específicos de fornecedor **terminam no adapter**.

Um fornecedor não é suportado enquanto não passar a
[Provider Conformance Suite](../adrs/0305-provider-conformance.md). Detalhe e
passos: [docs/agentic/provider-contract.md](../agentic/provider-contract.md).

## Estado das capacidades

`GET /api/v1/system/capabilities` reporta o estado real de cada capacidade,
apurado a partir de linhas reais. Ver
[docs/feature-status/](../feature-status/README.md).
