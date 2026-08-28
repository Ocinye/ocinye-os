# ADR-0307 — Dual Entry, Single Authority: operabilidade agentic universal por capabilities tipadas

- **Estado:** Accepted
- **Domínio:** Agentic
- **Impacto:** HIGH
- **Data:** 2026-08-23
- **Complementa:** [ADR-0002](0002-deterministic-core-and-agentic-control-plane.md),
  [ADR-0301](0301-agentic-control-plane.md),
  [ADR-0303](0303-capability-registry-and-executor.md)

## Context

O [ADR-0002](0002-deterministic-core-and-agentic-control-plane.md) decidiu que o
Ocinye OS é **operado com IA e governado pelo Core**. O
[ADR-0303](0303-capability-registry-and-executor.md) fechou o conjunto do que um
agente pode causar. Nenhum dos dois responde à pergunta que a auditoria desta
milestone tornou concreta:

> Quando alguém escreve «cria uma unidade chamada Engenharia Computacional»,
> porque é que o sistema não sabe fazê-lo, se o botão ao lado sabe?

À data desta decisão o registry tinha vinte e nove capabilities em cinco
domínios, e nenhuma em **Dados**, **Organisation**, **Administração** ou
**Identidade** — módulos nativos completos, com operações determinísticas a
funcionar, que o plano agentic simplesmente não alcançava.

(As contagens actuais não se escrevem aqui. Vivem na
[matriz](../agentic/operation-capability-matrix.md), emitida pelo catálogo
tipado. Um ADR que repetisse números seria mais uma cópia a envelhecer.)

Isso não aconteceu por decisão. Aconteceu por omissão: nada no repositório
obrigava alguém a **decidir** se uma operação nova é delegável. Uma capability
nascia quando alguém se lembrava, e a ausência de capability não era distinguível
de uma recusa deliberada.

E há uma segunda ausência, mais silenciosa. O `CapabilityDescriptor` tem doze
campos e nenhum diz **qual operação do Core executa**. A ligação existe dentro do
handler, em código, e não há forma tipada de perguntar «esta capability e aquele
formulário terminam no mesmo sítio?». Duas implementações da mesma regra podiam
divergir durante meses sem nada acusar.

## Decision

### A frase

> **A traditional Workspace action and its agentic equivalent converge on the
> same deterministic Core operation. The interface chooses how the intent enters
> the system; it does not change who owns authority or domain state.**

E a obrigação que dela decorre:

> **Every meaningful Ocinye OS operation is agent-addressable by design, or
> explicitly classified as non-delegable.**

### Duas entradas, uma operação

```text
Workspace UI ──────────────┐
                           ▼
                   Core Operation
                           ▲
                           │
Capability Executor ◄──── ActionPlan ◄──── Main Agent ◄──── intenção
```

Nunca:

```text
UI    → implementação A
Agent → implementação B
```

A convergência é **na operação do Core**, e não na camada agentic. A UI não chama
o Capability Executor — se chamasse, o Workspace deixava de funcionar sem plano
agentic, e a arquitectura passava de *AI-native* a *AI-dependent* por dentro,
sem ninguém decidir isso.

### A unidade é a operação, não a rota

Rejeita-se explicitamente:

```text
HTTP endpoint → exposto automaticamente ao modelo
```

O Ocinye OS não é «API mais tool calling». Uma operação pode ter rota HTTP,
formulário, capability e consumidor de worker — **a regra de domínio vive uma
vez**.

### Classificação obrigatória

Cada operação institucional significativa tem exactamente uma disposição:

| | |
|---|---|
| `Addressable(CapabilityId)` | descobrível e invocável pelo plano agentic |
| `NonDelegable(reason)` | existe e funciona pela interface determinística, e não pode ser delegada |
| `NotImplemented(reason)` | a operação ainda não existe no Core |

Não existe uma quarta. Em particular não existe *«Addressable, capability por
implementar»*: se a operação existe e é delegável, a capability faz parte da
mesma passagem. E `NotImplemented` **não** serve para adiar trabalho sobre uma
operação que já existe.

> `Unclassified = 0` não significa que tudo está exposto. Significa que tudo foi
> decidido.

### O critério de não-delegabilidade

Este ADR rejeita «administração é perigosa, logo não se delega». O critério é
outro, e é arquitectural:

> **An operation whose secure execution requires disclosing a secret to the
> Agentic Control Plane is non-delegable by architecture.**

E a sua consequência, que importa tanto quanto:

> **High risk does not automatically mean non-delegable.**

Uma operação privilegiada continua agent-addressable se tiver entrada tipada,
autorização forte, confirmação material obrigatória e reautorização no momento da
execução. Mudar uma palavra-passe é não-delegável não por ser perigoso, mas
porque o fluxo seguro exige que alguém escreva a palavra-passe actual — e essa
palavra nunca pode entrar no contexto de um modelo.

> **Agentic operability does not imply secret operability.**

O agente pode abrir Definições → Segurança e explicar o que se segue. Não recebe
`current_password`.

### A segunda classe: mutação da fronteira de autoridade

Há um segundo critério, e não é sobre risco:

> **An operation whose primary effect is to change the authorization boundary or
> another person's ability to access the system is non-delegable by
> architecture.**

A diferença está no **depois**. Enviar um email externo é de alto impacto, e o
seu efeito acaba nele: continua endereçável. Conceder um papel muda quem poderá
exercer autoridade a partir dali — e o efeito seguinte já não é o da operação, é
o de tudo o que a pessoa passa a poder fazer.

Nesta classe entram, medidas e não presumidas:

| operação | porquê |
|---|---|
| `identity::grant_role` | muda o que uma pessoa pode exercer |
| `identity::revoke_role` | muda o que uma pessoa pode exercer |
| `identity::set_account_status` | muda se uma pessoa pode sequer entrar |
| `governance::create_grant` | cria autoridade explícita |
| `governance::revoke_grant` | retira autoridade explícita |
| `organisation::add_unit_member` | **medido**: filiação numa unidade expande o acesso efectivo |

Uma operação por linha, e não `grant_role / revoke_role` numa só. Agrupar por
semelhança de nome é como uma contagem de treze passou por oito neste mesmo
trabalho: a tabela fica mais curta e quem a lê deixa de conseguir contar.

Esta lista é fixada pelo teste `a_fronteira_de_autoridade_e_a_que_foi_decidida`.
Acrescentar ou retirar uma operação desta fronteira faz a CI falhar até que este
documento e a [matriz](../agentic/operation-capability-matrix.md) acompanhem.

O último caso é o que mostra porque a regra precisa de ser medida e não
declarada. Filiação numa unidade parece metadado organizacional. Não é: o teste
`pertencer_a_uma_unidade_expande_o_acesso_efectivo` mostra que a mesma pessoa,
sem lhe tocar em papel técnico nenhum, passa a poder criar ideias e ver datasets
só por ser acrescentada.

#### O ataque que isto elimina

Conteúdo recuperado — um documento, um email, um dataset — é `UNTRUSTED DATA` e
não consegue autorizar nada: o Core impede. Mas consegue **induzir propostas**:
texto que leva o plano a sugerir, uma e outra vez, uma escalada plausível, até
alguém confirmar uma delas por cansaço.

Contra essa classe, a confirmação humana é a última barreira — e uma barreira que
depende de alguém estar atento ao fim de um dia longo. Não publicar a capability
elimina o vector inteiro.

É por isso que a guarda `is_delegable_to_agents`, que recusa no arranque qualquer
capability sobre `PermissionsManage`, `RolesManage` ou `MembersManage`, **continua
onde estava**. Este ADR não a levanta nem a estreita: incorpora-a, e explica-a.

> A confirmação humana e a reautorização em tempo de execução continuam
> obrigatórias para as operações endereçáveis de alto risco. Não são usadas como
> **substituto** de retirar a superfície de proposta onde a própria operação muda
> a autoridade do sistema.

O plano agentic continua a poder ajudar: abrir a Administração, explicar a
operação, resolver o contexto para mostrar. O que não faz é emitir um plano
executável para estas.

### As quatro classes de fronteira

> **Revisto com a `Scientific Infrastructure v1`**, que acrescentou a quarta
> classe. Até aí eram três. A prosa desta secção também estava partida ao meio
> da tabela — a explicação de `messaging::remove_member` ficara entre duas
> linhas dela, e a última linha deixara de aparecer como linha. Corrigido aqui.

Um critério em texto livre obriga quem lê a inferir, e inferir é onde as
classificações se confundem umas com as outras. As razões deixam de ser prosa e
passam a ser um tipo, `TrustBoundary`:

> **Non-delegability is determined by the nature of the trust boundary crossed,
> not by risk level alone.**

| classe | o que a operação atravessa |
|---|---|
| `SECRET_BOUNDARY` | a execução segura exigiria revelar um segredo ao plano agentic |
| `AUTHORITY_BOUNDARY` | o efeito primário é mudar a fronteira de autorização, ou a capacidade de outra pessoa aceder ao sistema |
| `USER_MEDIATED_BINARY_BOUNDARY` | o conteúdo entra por um acto material da pessoa — um ficheiro que ela escolhe — e não por texto que um modelo componha |
| `INSTITUTIONAL_CLAIM_BOUNDARY` | o efeito é uma afirmação institucional cujo peso vem de quem a assina |

**Sobre `AUTHORITY_BOUNDARY`.** `messaging::remove_member` entrou nela com as
Mensagens v1. Retirar alguém de uma conversa — ou sair dela — muda quem lê o
que lá se diz a partir desse momento, que é a mesma natureza de revogar uma
concessão. O que fecha a delegação não é o impacto: é o vector. Conteúdo de uma
conversa é lido pelo plano agentic, e uma frase colocada lá por alguém não pode
acabar a excluir uma pessoa dessa conversa.

**Sobre `USER_MEDIATED_BINARY_BOUNDARY`.** É a mais fácil de perder de vista.
`data::add_version_file` e `identity::set_photograph` não são perigosas nem
tocam em autoridade: são simplesmente operações cujo conteúdo **não existe no
espaço em que um modelo opera**. Um agente pode preparar tudo à volta — os
metadados, a versão, a descrição — e a escolha do binário continua a ser da
pessoa.

**Sobre `INSTITUTIONAL_CLAIM_BOUNDARY`.** As outras três fecham-se por causa do
que a operação **alcança**: um segredo, a autoridade de alguém, bytes que só a
pessoa tem. Esta fecha-se por causa de quem a **assina**.

`science::record_validation` é a única, e o exemplo explica a classe. Validar um
resultado científico — ou dá-lo por reproduzido — não muda o acesso de ninguém,
não revela nada, e nem sequer é difícil de desfazer: uma validação errada
corrige-se com outra, e o domínio guarda as duas. O que não se desfaz é a
atribuição. O registo diz que **alguém afirmou aquilo**, e é essa pessoa que lhe
dá valor; um agente a produzi-la deixaria a instituição com uma afirmação sem
ninguém por trás.

E é por isso que não se resolve com aprovação, que é a resposta habitual a «isto
é sério demais para um agente fazer sozinho». Uma confirmação humana continuaria
a deixar a afirmação escrita como se tivesse sido *feita*, e não *assumida* — e
a diferença entre as duas é toda a razão de a validação existir. É a mesma linha
que a `Scientific Infrastructure v1` traça para a proveniência:

> **AI may suggest provenance. AI may not invent institutional provenance.**

O critério é estreito de propósito, e o teste
`a_fronteira_de_afirmacao_e_a_que_foi_decidida` fixa a lista para que não deixe
de o ser. **Irreversível não é afirmação institucional**:
`science::record_execution` é irreversível — uma corrida aconteceu, e apagar o
registo não a desfaz — e é endereçável. O que a distingue é que registar uma
execução descreve o que se passou; validar afirma o que a instituição sabe.

Quem lê a [matriz](../agentic/operation-capability-matrix.md) vê a classe ao lado
de cada operação não-delegável, e não tem de a deduzir de uma frase.

E a lição que a filiação em unidades ensinou, escrita como regra e não como nota
de rodapé:

> **Organisational membership is authorization-relevant state whenever policy
> derives effective access from it; it must not be treated as mere profile
> metadata.**

### A ligação tipada, nos dois sentidos

O `CapabilityDescriptor` passa a conhecer a `OperationId` que executa, e o
catálogo de operações passa a conhecer a disposição de cada uma. Isso torna
verificáveis quatro propriedades que até aqui eram intenção:

```text
operação significativa   → exactamente uma disposição
disposição Addressable   → uma CapabilityId que existe
CapabilityId             → uma OperationId que existe
NonDelegable             → nenhuma capability reclama essa operação
NonDelegable             → uma TrustBoundary declarada
```

E uma quinta, que nenhuma delas alcança porque não vive no catálogo: que a
entrada do Workspace e a entrada agentic **acabam mesmo no mesmo sítio**. O
Workspace não chama o Core directamente — submete um formulário que vira `POST`
numa rota HTTP. A suite `services/core-server/tests/parity.rs` conduz as duas
entradas a sério, com o router real e o executor real, e compara o rasto de
auditoria que cada uma deixou. A auditoria é escrita **dentro** da operação do
Core: se as duas entradas a escrevem de forma indistinguível, passaram as duas
pelo mesmo código.

### `OperationId` é metadata interna

O modelo continua a receber **apenas identificadores de capability**. Não recebe
`OperationId`, nem permissão, nem risco, nem esquemas internos, nem o descritor.

Hoje isto é verdade por construção — o `ContextEnvelope` leva
`available_capabilities: Vec<String>`. Passa a ser verdade por invariante:

> **The inference context receives capability identifiers, never the internal
> capability registry representation.**

A guarda existe para o dia em que alguém quiser mandar mais «para o modelo
planear melhor». Risco, aprovação, disponibilidade e a ligação à operação são
factos do Core, e não informação de planeamento.

## Alternatives

| Alternativa | Porque foi rejeitada |
|---|---|
| **Expor cada endpoint HTTP como tool** | A unidade de autorização passaria a ser a rota, e não a operação. Cada rota nova tornar-se-ia poder novo sem revisão. |
| **O modelo chamar os módulos directamente** | Dissolve a fronteira de execução: deixa de haver um sítio onde a autorização em tempo de execução acontece. |
| **Uma implementação de IA por módulo** | Duas implementações da mesma regra divergem. É exactamente o que este ADR existe para impedir. |
| **Uma capability genérica `execute_anything`** | Destrói a auditabilidade: o registry deixa de descrever o que pode acontecer. |
| **Manter a IA como assistente de conversa** | Deixa a operabilidade por linguagem natural fora da arquitectura, e torna-a impossível de acrescentar depois sem reescrever os módulos. |
| **A UI chamar o Capability Executor «para unificar»** | Inverte a dependência: o Workspace passaria a precisar do plano agentic para funcionar. *AI-native* tornar-se-ia *AI-dependent*. |
| **Levantar a guarda de delegabilidade para obter paridade** | Trocaria uma defesa estrutural existente por atenção humana, contra um modelo de ameaça em que conteúdo hostil influencia propostas. Paridade não é razão para remover uma defesa. |
| **Só documentar a matriz de cobertura** | Documentação sem guarda envelhece em silêncio. Foi assim que as lacunas de Dados e Organisation existiram sem ninguém as ter decidido. |

## Consequences

**Positivas** — a linguagem natural passa a ser uma interface de operação
universal, e não uma superfície separada; os módulos continuam determinísticos e
funcionam sem provider de inferência; a cobertura agentic passa a ser mensurável
em vez de anedótica; um módulo novo integra-se de forma previsível, porque a
lista de perguntas que tem de responder está escrita.

**Custos, e são reais** — cada operação nova passa a exigir uma decisão de
exposição, e isso é trabalho que antes ninguém fazia; o registry cresce
deliberadamente e cada entrada é superfície a manter e a defender; há mais testes;
a resolução de referências e a desambiguação são complexidade nova; os planos
compostos multiplicam os modos de falha parcial; e as `CapabilityId` passam a ser
contrato institucional — renomeá-las deixa de ser refactor e passa a ser
migração.

**Sobre o estado deste ADR** — fica `Proposed` enquanto o repositório não o
cumprir. Passa a `Accepted` quando `Unclassified = 0`, nenhuma capability for
órfã, nenhuma operação `Addressable` estiver por implementar, e as guardas o
demonstrarem. Assim `Accepted` significa **o repositório já cumpre isto**, e não
**concordámos com isto**.
