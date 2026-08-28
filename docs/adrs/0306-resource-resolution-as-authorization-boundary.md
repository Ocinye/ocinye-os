# ADR-0306 — Resolução de recursos como fronteira de autorização

- **Estado:** Accepted
- **Domínio:** Agentic
- **Impacto:** HIGH
- **Data:** 2026-08-22
- **Depende de:** [ADR-0303](0303-capability-registry-and-executor.md) · [ADR-0302](0302-agent-access-intersection.md)
- **Refina:** [ADR-0303](0303-capability-registry-and-executor.md)

## Context

O [ADR-0303](0303-capability-registry-and-executor.md) estabeleceu o Capability
Executor e a ordem dos seus gates. Um passo de plano chega com três coisas: uma
capability, um input, e uma lista de `ResourceRef` — o modo como o passo diz
*sobre o quê*.

Ao integrar Research e Knowledge, duas propriedades desse desenho revelaram-se
insuficientes. Nenhuma delas permitiu alguma vez um acesso indevido: ambas
falhavam fechado. Mas as duas juntas tornavam o núcleo científico da instituição
inalcançável por esta via.

### O `ResourceRef` não era verificado

`ExecutionContext.resources` estava documentado como «já resolvido e verificado».
Não estava. Os `ResourceRef` que um modelo escrevia atravessavam o planner e o
executor sem que nada os procurasse na base de dados.

Isto não era explorável, porque nenhum handler os lia: cada um recebia o
identificador do seu alvo no `input` e chamava um serviço de domínio, que
autorizava. A defesa em profundidade estava a fazer o seu trabalho. Mas era uma
garantia por acidente — o primeiro handler a confiar em `ctx.resources` teria
aberto o buraco, e a documentação convidava-o a fazê-lo.

### O contexto de autorização era o do pedido, não o do recurso

Cada passo era autorizado contra `ResourceContext::organisation(...)`: a
organização, sem unidade, sem workspace, `INTERNAL`.

Para «esta pessoa pode pesquisar no acervo» isso é exactamente a pergunta certa.
Para «esta pessoa pode ler *esta* Nota» é a pergunta errada, de duas maneiras ao
mesmo tempo:

- **Recusa demais.** Quem tem acesso por pertencer a um workspace não detém a
  permissão ao nível da instituição. Um líder de Research Workspace era recusado
  a operar dentro do seu próprio ambiente.
- **Não consegue recusar o que importa.** Um contexto que não nomeia unidade
  nenhuma não tem unidade de que uma referência estrangeira possa estar fora.

O efeito medido, antes desta decisão: das capabilities de âmbito workspace,
**nenhuma** era alcançável por um membro cujo acesso viesse de pertença. Isto
inclui `research.idea.create`, que existia desde a milestone agentic anterior e
nunca tinha sido exercitada por um caminho bem-sucedido — os testes existentes
provavam recusas, que passavam pela razão errada.

## Decision

**Um `ResourceRef` é uma afirmação até ser resolvido. A resolução é onde deixa de
o ser, e o contexto que produz é o contexto que autoriza.**

Concretamente:

1. **Resolver antes de decidir.** O executor resolve todos os `ResourceRef` do
   pedido antes de qualquer gate de autorização e antes da validação do schema.

2. **A resolução passa pelo serviço de domínio que detém o recurso.** O resolver
   não consulta a base de dados e não decide nada: chama `research::get_idea`,
   `knowledge::get_note`, `collaboration::get_task`. A política de leitura
   continua a viver onde já vivia.

3. **Ausência e recusa são a mesma resposta.** Um identificador que não existe,
   um que pertence a outra unidade e um de um tipo que este plano não endereça
   devolvem todos `ResourceNotFound`, com a mesma mensagem. Distingui-los faria
   do plano agentic um oráculo para enumerar a instituição por identificador.

4. **O contexto vem do recurso.** Quando um passo nomeia recursos, a decisão é
   tomada contra o contexto de cada um — unidade real, workspace real,
   classificação real. Um passo que não nomeia nenhum mantém o contexto do
   pedido.

5. **A classificação mais estrita governa.** Um artefacto pode estar classificado
   acima do seu ambiente. O contexto leva a mais alta das duas, ou o tecto do
   agente não teria o que recusar.

6. **O título é do Core.** A `label` que o modelo escreveu é substituída pela do
   Core na resolução. Um plano que descreve um recurso com palavras que ninguém
   verificou é um plano confirmado sob uma descrição errada.

7. **Endereçar é através de `resources`, não do `input`.** As capabilities que
   actuam sobre um recurso específico leem-no de `ctx.resources`. Ler um
   identificador do `input` contornaria o gate, e ter dois canais de
   endereçamento significaria que um deles não é verificado.

A ordem do executor passa a ser:

```text
resolve capability  →  um nome inventado resolve para nada
resolve resources   →  cada um, pelo serviço que o detém
authorise           →  contra o contexto de cada recurso, ou o do pedido
validate input      →  contra o schema publicado
approval gate       →  efeito externo e privilégio exigem sempre uma pessoa
execute             →  o serviço de domínio, que detém o invariante
audit               →  o que foi pedido, por quem, através de que agente
```

## Alternatives

| Alternativa | Porque não |
|---|---|
| **Manter o contexto institucional e conceder as permissões ao nível técnico** | Resolveria a recusa a mais destruindo a razão de existirem âmbitos: `NotesCreate` institucional significa poder escrever em qualquer ambiente da instituição. Trocaria um problema de alcance por um problema de acesso. |
| **Deixar os handlers autorizarem** | É o que já acontece, e é a razão de nada ter estado exposto. Mas põe a decisão em vinte e cinco sítios em vez de um, e a próxima capability escrita por alguém com pressa é a que se esquece. |
| **Resolver depois de autorizar** | Foi a primeira tentativa desta milestone. Não funciona: a autorização precisa do contexto que só a resolução produz, pelo que autorizar primeiro é autorizar contra o contexto errado. |
| **Aceitar identificadores no `input` e resolvê-los também** | Dois canais de endereçamento, um dos quais o executor teria de adivinhar como interpretar por capability. O `ResourceRef` existe precisamente para não ser preciso adivinhar. |
| **Resolver parcialmente, executando sobre o que se alcança** | Um passo que nomeia quatro recursos e alcança três não é um passo sobre três. É um passo diferente do que a pessoa confirmou. |

## Consequences

**O que melhora.**

- Research e Knowledge tornam-se alcançáveis por membros cujo acesso vem de
  pertença — que é como o acesso científico realmente funciona.
- Uma referência para outra unidade passa a ser recusada por nome, e não por
  acidente de um contexto genérico.
- `ctx.resources` passa a valer o que a sua documentação dizia.
- Existe um sítio, e um só, para acrescentar um gate que se aplique a tudo.

**O que custa.**

- Uma consulta por recurso nomeado, antes de decidir. Aceitável: os passos
  nomeiam poucos recursos, e o limite de selecção é doze.
- As capabilities que endereçam um recurso deixam de o aceitar no `input`. É uma
  quebra de contrato do schema publicado, absorvida agora porque nada está
  deployado (`CLAUDE.md` §1).
- O plan schema passa a incluir `resources`, com um conjunto fechado de tipos.
  Um modelo que nomeie um tipo fora dele não nomeou nada.

**O que continua a não estar resolvido.**

- O resolver endereça sete tipos. Datasets, pessoas, unidades e os tipos de
  correio resolvem para nada — deliberadamente, até que cada um seja auditado.
- Uma relação entre recursos de workspaces diferentes é recusada. É a resposta
  correcta hoje, porque uma aresta desse tipo não teria um contexto único de
  autorização; se o domínio vier a precisar dela, precisa primeiro de decidir a
  quem pertence.
