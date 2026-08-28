# O contrato entre o Workspace e o Core

Dois princípios governam tudo o que é visível no Ocinye OS.

## 1. Toda a acção visível tem contrato

> **Se um membro vê uma opção, essa opção tem comportamento definido.**

Para cada elemento interactivo, exactamente uma destas é verdadeira:

| | Condição | O que a interface faz |
|---|---|---|
| **A** | Funciona | A acção executa de ponta a ponta. |
| **B** | Sem permissão | **Não é mostrada.** O Core recusaria na mesma. |
| **C** | Indisponível por dependência real | Fica visível, declarada indisponível, **com a razão**. |
| **D** | Não se aplica ali | Não é mostrada. |

Nada mais é aceitável. Em particular, não existe «o botão está lá mas ainda não
faz nada».

### Como isto é imposto

Três testes em `apps/workspace/src/ui/mod.rs` percorrem **todos** os ecrãs
renderizados e falham se a invariante quebrar:

| Teste | O que garante |
|---|---|
| `nenhum_botao_existe_sem_comportamento` | Todo o `<button>` submete, tem `data-oc`, ou declara-se indisponível. |
| `nenhum_campo_existe_sem_destino` | Todo o `<input>` está num formulário ou tem handler. |
| `nenhuma_ancora_leva_a_lado_nenhum` | Nenhum `href="#"` nem `href=""`. |
| `nenhuma_ligacao_aponta_para_um_ecra_inexistente` | Todo o destino corresponde a uma rota registada. |

Os quatro percorrem um catálogo de **33 ecrãs**, que inclui os seis do Ocinye
Mail — entre eles o estado «correio não configurado», precisamente para que o
caso em que nada funciona seja tão coberto como o caso em que tudo funciona.

Um botão novo sem contrato **não passa na CI**.

## 2. Permissão e disponibilidade são coisas diferentes

Uma acção é executável quando:

```
actor_pode_fazer   E   sistema_consegue_fazer
```

As duas metades vivem em sítios distintos e nunca se confundem:

| | Pergunta | Onde vive | Contrato |
|---|---|---|---|
| **Permissão** | *Este membro pode?* | `ocinye-domain::policy` | `GET /api/v1/me` → `capabilities` |
| **Disponibilidade** | *O sistema consegue?* | `ocinye-core::modules::platform` | `GET /api/v1/system/capabilities` |

O correio mostra bem porque as duas não colapsam numa só. Um membro sem
`mail.ai_use` e um Ocinye OS sem nó de IA produzem o **mesmo** painel apagado, e
frases diferentes: «Não possui autorização para usar a assistência de escrita»
não é a mesma informação que «depende de uma capacidade de IA, que não está
actualmente disponível». Na primeira, pedir acesso resolve; na segunda, não.

### Porque importa

Estas duas frases descrevem situações completamente diferentes, e um sistema que
as confunde mente em ambas:

> **«Não pode utilizar IA.»** — não tem permissão.

> **«Pode utilizar IA, mas não existe nenhum nó disponível.»** — a capacidade não
> está instalada.

Um membro que recebe a primeira quando devia receber a segunda vai pedir acesso a
quem não lho pode dar. Um que recebe a segunda quando devia receber a primeira
vai esperar por hardware que não resolveria nada.

### Consequências concretas

- A barra lateral, o menu `+ Criar` e a command palette renderizam **apenas** o
  que o membro pode alcançar. Não se envia ao cliente uma lista de coisas que ele
  não devia saber que existem.
- A acção primária de cada lista desaparece sem a permissão que exige. Um
  `PlatformAdmin` não vê «Novo Agente», porque não detém `agents.create.personal`
  — administração técnica não é acesso científico.
- Uma recusa do Core **nunca** é renderizada como lista vazia. `optional()`
  engole erros e serve painéis secundários; o conteúdo principal usa
  `required()`, e uma recusa aparece como recusa.
- Um `403` mostra «Não possui acesso a este recurso» com uma saída; um `404`
  mostra «Página não encontrada»; um erro inesperado mostra uma referência de
  correlação e nada mais.

## 3. Esconder não é autorizar

A filtragem da interface é **cortesia**, nunca segurança.

Quem escrever um endereço à mão bate exactamente na mesma recusa do Core. Todos
os endpoints decidem por si, e o extractor `Authorised<T>` verifica a permissão
**antes** de o corpo do pedido ser deserializado — para que um chamador não
autorizado não aprenda sequer o schema da operação.

## 4. Linguagem de estados

Nunca `OFFLINE` para tudo. Seis estados, com significados distintos:

| Estado | Quando | Exemplo de texto |
|---|---|---|
| `available` | Funciona | — |
| `no_resource` | Nada registado que sirva | «Nenhum nó de IA Ocinye está actualmente disponível.» |
| `not_configured` | Existe, não foi configurado | «Nenhum modelo registado serve esta capacidade.» |
| `unavailable` | Registado, não responde | «Existem nós registados, mas nenhum respondeu.» |
| `degraded` | Responde em parte | — |
| `planned` | Decidido, não construído | «Esta capacidade ainda não foi activada nesta instalação.» |

O registo é institucional. Não existe «Oops», «Coming soon» nem «Under
construction» em nenhum ecrã, e um teste em `ocinye-contracts` garante-o.

## 5. A prontidão institucional é uma decisão do Core, não uma leitura da interface

O Workspace nunca conclui, por si, que o sistema está bem. Pergunta a `/ready`,
lê `overall`, e apresenta o que vier. Não conta componentes verdes para chegar a
uma conclusão própria — contar no browser seria uma segunda política de arranque,
e duas políticas acabam por discordar.

| O que a barra diz | O que aconteceu |
|---|---|
| `CORE OK` | O Core respondeu `ready` ou `degraded` |
| `CORE INDISPONÍVEL` | O Core respondeu `blocked` — **ele decidiu** que não pode servir |
| `CORE SEM RESPOSTA` | Não houve decisão nenhuma: sem resposta, ilegível, fora de tempo, ou contrato que não coincide |

`degraded` aparece como `CORE OK` de propósito. O distintivo diz **CORE**, e
`degraded` é uma afirmação sobre a *instalação*: o Core devolve `blocked` antes
de chegar a `degraded`, portanto `degraded` significa que todos os componentes
críticos estão disponíveis e que algum opcional não está. Um Core inteiro não
fica limitado por não haver SMTP configurado nem nenhum nó de computação
registado. A prontidão da instalação continua a dizer-se por inteiro em
`/ready`, que nomeia cada componente e a sua razão.

As duas últimas linhas são a distinção que interessa e a que é mais fácil de
perder. Um sistema que decidiu não servir disse alguma coisa; um sistema que não
respondeu não disse nada. Fundi-las faria a interface afirmar uma decisão que
ninguém tomou.

Isto já esteve errado, e da maneira mais discreta possível: a barra dizia
`CORE OK` porque uma consulta de organização tinha respondido —
`let core_ok = !organisation.is_null()`. Um pedido de domínio responde por razões
suas, e uma delas não é a prontidão institucional. Hoje há um teste estrutural
que recusa o padrão pelo nome.

Detalhe: [ADR-0603](../adrs/0603-boot-and-institutional-readiness.md) e
[docs/architecture](../architecture/README.md#o-arranque).

## 6. Números vêm de dados

Nenhum contador, badge ou estatística é escrito à mão. Se o Core não devolveu
uma contagem — porque recusou, ou porque não respondeu — o cartão **não aparece**.
Mostrar `0` diria «não existe nenhum» a quem apenas não pode ver.

## 7. Um selector não é segurança

A criação científica trouxe selectores que oferecem recursos: a versão de
metodologia que um estudo segue, a versão de dataset que entra numa execução, a
execução que sustenta uma reprodução. Estes selectores fazem duas coisas
diferentes, e nenhuma delas é autorizar.

**Seguem o contrato em vez de esperarem pela recusa.** A matriz de proveniência
aceita `Study → MethodologyVersion` e recusa `Study → Methodology`; o selector
oferece versões. Oferecer a metodologia mutável poria no ecrã uma escolha que o
Core recusa, e deixaria o `422` ensinar a regra a quem já tinha preenchido o
resto.

**E oferecem apenas o que quem escolhe já alcança**, porque a lista vem do Core
com o principal de quem pergunta.

Mas o Core revalida **tudo**, sempre: o tipo, a existência do recurso, a
autorização, a semântica da versão, a compatibilidade da relação e a autoridade
corrente. Uma escolha que não passe pelo selector — escrita à mão, ou vinda de
outro lado — bate na mesma recusa.

> **O selector é conveniência e honestidade. A decisão é do Core.**

## 8. O que a interface não pergunta

Alguns campos não existem no formulário de propósito, e a ausência é a
propriedade.

**A origem da proveniência.** Um resultado é registado **de dentro** da execução
que o produziu, e a operação escreve a aresta na mesma transacção. Não há um passo
a seguir a pedir «agora indique de onde veio», e não há um selector de `origin`:
`operation` significa que o Core **observou** a relação, e uma pessoa a marcá-lo
estaria a afirmar que o sistema viu o que não viu.

**O que o caminho já diz.** Se o formulário abriu dentro de uma execução, o
identificador dessa execução não é um campo — é o contexto.

## 9. O vocabulário do domínio é lido no Core

Estados como `succeeded`, `published` ou `draft` são vocabulário do domínio, em
inglês, e a interface é em português. A leitura — «Correu bem», «Em vigor»,
«Rascunho» — vem do Core, ao lado do vocabulário que traduz.

Uma tabela de tradução no cliente seria um segundo vocabulário, a divergir do
primeiro no dia em que alguém acrescentasse um estado. Pela mesma razão, um campo
de estado é um conjunto fechado no ecrã: uma cadeia de caracteres livre chegaria
a um `CHECK` da base, e um erro de quem preenche voltaria como avaria.
