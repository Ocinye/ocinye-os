# ADR-0603 — Arranque do Ocinye OS e prontidão institucional

- **Estado:** Accepted
- **Domínio:** Workspace
- **Impacto:** HIGH
- **Data:** 2026-08-24 · aceite a 2026-08-25
- **Relaciona-se com:** [ADR-0601](0601-workspace-bff-session.md) ·
  [ADR-0602](0602-workspace-ssr-progressive-enhancement.md) ·
  [ADR-0411](0411-execution-time-principal-freshness.md)

## Context

Abrir o Ocinye OS é abrir uma URL e esperar que esteja tudo bem. Se o núcleo
estiver em baixo, a pessoa descobre-o depois de escrever a palavra-passe. Se o
correio não estiver configurado, descobre-o ao abrir o correio. Não há momento
em que o sistema diga o que é de si próprio.

A auditoria encontrou as peças espalhadas, e uma contradição:

- `/ready` já existe, já responde sem sessão, e já é consultado pela página de
  entrada com um limite de três segundos;
- `SystemCapabilityState` já distingue `Available`, `NoResource`,
  `NotConfigured`, `Unavailable`, `Degraded` e `Planned` — e já lê um valor
  desconhecido como `Unavailable`, nunca como utilizável;
- `platform::system_capabilities` já produz razões em linguagem institucional,
  documentadas como seguras para mostrar a um membro;
- **e o `CORE OK` da barra superior não vem de nada disto.** Vem de
  `!organisation.is_null()`: o Core respondeu a um pedido de domínio, logo
  assume-se que está bem. É uma inferência, não uma medição, e pode discordar de
  `/ready` sem que nada acuse.

## Decision

### As frases

> **The Workspace is not presented as ready until the deterministic Core has
> established institutional readiness.**

> **Boot readiness and user authorization are separate concerns.**

> **Module degradation may limit capabilities without preventing institutional
> operation.**

> **A boot animation never creates readiness; it only represents facts
> established by the system.**

### 1 · `/ready` é a única fonte pública de prontidão

Não `/boot`, não `/status`, não uma segunda rota. `/ready` já existe com esta
semântica e já é chamado sem sessão; estende-se, e mantém-se público, barato,
sem escrita, com limite de tempo e `Cache-Control: no-store`.

`/health` continua a ser liveness. Fundir os dois destruiria a única pergunta
que se pode fazer a um processo que ainda não sabe se consegue servir.

### 2 · `/system/capabilities` continua autenticado

Não se transforma no endereço de arranque. É o catálogo para quem já entrou.

### 3 · Seguro para um membro não é seguro antes de autenticar

> **Public readiness is an installation-health projection, not the
> authenticated system-capability catalogue.**

Uma razão sem `stack trace` nem `hostname` continua segura para quem já entrou e
pode, ainda assim, dizer a um desconhecido quantos nós existem ou que adaptador
está configurado.

Por isso a projecção pública **não** é o catálogo com campos removidos: é um
conjunto fechado. `ReadinessComponentId` tem nove variantes e não tem campo
livre; as razões saem de `readiness::reasons`, treze frases fixas. Acrescentar
algo ao que é público exige mexer nesse ficheiro — não acontece por alguém ter
acrescentado uma capacidade noutro sítio.

Fora, deliberadamente: identificadores de capabilities, operações, âmbitos,
papéis, permissões, contagens de recursos, nomes de fornecedores ou de nós,
adaptadores, topologia.

### 4 · Nenhum componente infere prontidão de um pedido de domínio

> **No UI component may infer Core readiness from the success of an unrelated
> domain request.**

O `CORE OK` deixa de derivar de `!organisation.is_null()`. Arranque e barra
superior passam a partilhar a mesma fundação factual, com ciclos de vida
diferentes: o arranque é um retrato num instante; a barra superior é uma sonda
que continua a olhar. Prontidão no arranque não garante saúde permanente depois
dele.

### 5 · A criticalidade é ortogonal ao estado, e vem do Core

`Unavailable` não diz se é grave: a persistência indisponível bloqueia, o correio
indisponível não. São duas perguntas, e juntá-las obrigaria a inventar variantes
como `UnavailableButFine`.

`overall` é decidido pelo Core. O Workspace não conta componentes verdes no
browser: seria uma segunda política de arranque, e duas políticas acabam por
discordar.

### 6 · Incompatibilidade de contrato é falha crítica

`CONTRACT_VERSION` é comparado no arranque. Um Workspace e um Core instalados
separadamente podem ficar de gerações diferentes; quando ficam, o arranque
di-lo — em vez de rebentar mais tarde num erro de desserialização que ninguém
consegue ler.

Não é o SHA do Git, que é privado e muda a cada commit. Não é o `API_VERSION`,
que é o `v1` do caminho e muda raramente por desenho.

### 7 · O marcador de arranque não tem autoridade nenhuma

> **Boot-complete is presentation state only. Forging or deleting it cannot
> grant or remove authority.**

Responde a uma única pergunta: «já mostrei a experiência inicial desta versão
nesta sessão de browser?». Não é sessão, não é `Principal`, não é prontidão, não
é autorização. Forjá-lo à mão continua a levar ao Login.

### O desconhecido nunca deixa arrancar

`ReadinessOverall::parse` lê qualquer valor que não conheça como `Blocked`, e
`Criticality::parse` lê qualquer valor que não conheça como `Critical`. Uma
versão futura do Core não pode introduzir um estado que um Workspace antigo
desconheça e ele conclua, por não perceber, que está tudo bem.

### A máquina de estados

```text
UNINITIALIZED
     ↓
CHECKING
     ├── núcleo inalcançável ──────────→ BLOCKED
     ├── falha crítica ────────────────→ BLOCKED
     ├── incompatibilidade ────────────→ BLOCKED
     ├── degradação opcional ──────────→ DEGRADED
     └── tudo o que é crítico pronto ──→ READY
                                            │
                                  resolver sessão
                                            │
              ┌─────────────────────────────┼──────────────────────┐
              ▼                             ▼                      ▼
            LOGIN                    FLUXO OBRIGATÓRIO         WORKSPACE
```

A resolução de sessão fica **fora** de `overall`. Não ter sessão não é o sistema
estar degradado — é o sistema estar pronto e a pessoa ainda não ter entrado.

E a distinção que salva o diagnóstico: `401` não é `500`. «Não autenticado» leva
ao Login; «o serviço de identidade não respondeu» é falha de prontidão, e nunca
se apresenta como «faça login novamente».

## Alternatives

**Servir o catálogo autenticado em `/ready`.** Uma linha de código, e a
instalação passa a contar a um desconhecido quantos nós tem.

**Deixar o Workspace decidir `overall`.** Convidaria a que o browser
reclassificasse criticalidade, e duas políticas de arranque acabam por discordar
num sítio onde ninguém está a olhar.

**Chamar-lhe *Secure Boot*.** Não há cadeia criptográfica nenhuma aqui. O nome
prometeria uma garantia que não existe.

## Consequences

O `CORE OK` muda de fonte, e é uma alteração de comportamento: passa a poder
dizer que o Core está mal em situações em que antes dizia que estava bem, porque
antes só sabia que um pedido tinha respondido.

O arranque acrescenta uma chamada a `/ready` na abertura — uma, por sessão de
browser, e não por navegação. É o custo de deixar de descobrir que o núcleo está
em baixo depois de escrever a palavra-passe.

## O que ficou construído

Esta secção descreve o que existe, e não o que se pretendia. Foi escrita depois
de o ciclo estar provado ponta a ponta.

### O portão

Um pedido de documento que chegue sem ter visto o arranque nesta janela é
encaminhado para `/boot`, com o destino preservado. Corre **antes** de qualquer
página ser construída.

A alternativa era cada página decidir por si se já houve arranque — e uma página
nova que se esquecesse disso passaria a ser a porta de trás. Um portão vale para
tudo o que passa, incluindo o que ainda não foi escrito.

Não encaminha o que não é uma pessoa a abrir o sistema: estáticos, avatares, a
sonda de saúde, pedidos que não pedem HTML, nem submissões de formulário —
encaminhar um `POST` perderia o que alguém escreveu.

### A máquina de estados

```text
Uninitialized → Checking → Ready
                         → Degraded
                         → Blocked

                           Unreachable
```

`Unreachable` vive do lado da Experience de propósito. Não é um
`ReadinessOverall`: é a **ausência** de um. `Blocked` é o Core a dizer que não
está pronto; `Unreachable` é não termos obtido decisão nenhuma. Ambos impedem o
arranque e são diagnósticos opostos — um sabe-se, o outro não.

### O marcador

Um cookie de sessão do browser, sem `Max-Age`: morre com a janela.

O que ele pode fazer é dispensar o Splash. Nada mais. A sonda de prontidão corre
na mesma, portanto um marcador inventado não faz um Core bloqueado parecer
pronto; e a sessão é resolvida na mesma, portanto não autentica ninguém.

> **O arranque-concluído pode ser guardado como estado de apresentação. A
> autoridade sobre prontidão não pode.**

Só é gravado quando houve por onde seguir. Gravá-lo num arranque bloqueado faria
a tentativa seguinte saltar a apresentação de um problema que continua lá.

### O destino de regresso

É a única entrada de terceiros que atravessa o arranque, e por isso é validado
contra o **catálogo de rotas do próprio Workspace**, por lista de permitidos.

Uma segunda lista escrita à mão divergiria da primeira ao fim de duas
funcionalidades, e a que decide segurança seria justamente a desactualizada.

### A entrega

Uma actualização de meta, e não JavaScript. O arranque é o momento em que menos
se pode assumir que há scripts a correr, e exactamente aquele em que tem de
funcionar.

Não há percentagens a subir nem etapas a acender: quando a página chega ao
browser, a decisão já foi tomada no servidor. Se a prontidão vier num instante,
o arranque dura um instante — encenar espera seria mentir sobre o sistema para o
fazer parecer mais sério.

### O que a topbar passou a dizer

Três estados, porque são três situações:

| | |
|---|---|
| `CORE OK` | pronto, inclusive com capacidades opcionais indisponíveis |
| `CORE INDISPONÍVEL` | o Core respondeu que não está em condições |
| `CORE SEM RESPOSTA` | não houve resposta |

Um booleano obrigava a escolher entre as duas últimas, que são as piores de
confundir.

Houve um quarto, `CORE LIMITADO`, para `degraded`. Foi retirado porque dizia a
coisa errada sobre a coisa certa: o distintivo fala do **Core**, e `degraded`
fala da *instalação*. Uma instalação sem correio, sem inferência e sem
computação continua a ter um Core inteiro — `decide()` devolve `blocked` antes
de chegar a `degraded` — e apresentá-lo a amarelo dizia a quem entra que o
sistema estava avariado quando o que faltava era configuração opcional. O
`/ready` não mudou e continua a nomear cada componente em falta.

## O que está guardado, e onde

| Propriedade | Onde é provada |
|---|---|
| A semântica pública de `/ready` | `services/core-server/tests/readiness_http.rs` |
| Estado ou criticalidade desconhecidos falham fechado | `ocinye-contracts::readiness` |
| Bloqueado e sem resposta são distintos | `ocinye-workspace::boot` |
| O marcador morre com a janela e não autoriza | idem, e viagens de browser |
| Nenhum destino de regresso sai do Ocinye OS | idem, e ao nível do HTTP |
| A prontidão não é inferida de um pedido de domínio | `tests/experience_boundary.rs` |
| Uma abertura a frio encontra o arranque primeiro | viagens de browser |
| Um crítico em baixo recusa ao nível do HTTP | `readiness_http.rs` |
| Uma sessão a meio não é libertada pelo arranque | viagens de browser |
| Retroceder não prende a pessoa no arranque | idem |
| A topbar acompanha o Core ao longo da sessão | idem |
| Um Core que recupera deixa passar quem estava preso | idem |
| Uma razão hostil do Core não se torna marcação | `ui::screens::boot` |

A última linha é a que dá sentido a todas as outras: existe uma pessoa, num
Chrome a sério, que abre o Ocinye OS e encontra a prontidão institucional antes
do Login e antes do Workspace.

## O que este arranque não faz

Não executa inferência, não planeia, não chama modelos. O sistema arranca com
zero fornecedores de IA e zero nós de computação, e isso aparece como estado
factual de capacidade — não como falha.

Não é um monitor contínuo. Depois de entrar, a observação é da topbar; o
arranque não reaparece a cada falha, porque reaparecer transformaria uma
cortesia numa interrupção.

## O que se aprendeu a construir isto

Três coisas que não estavam no desenho e que ficam escritas porque voltarão a
aparecer.

**A prova de bloqueio estava no sítio errado.** A regra de criticidade tinha um
teste ao nível do HTTP, e esse teste começava por `if critico_em_baixo`. Num
ambiente saudável o ramo nunca corria: passava sem ter observado nada. Só
apareceu por reversão — fazer um crítico em falta degradar em vez de bloquear
deixou as doze viagens de HTTP verdes. A prova nova põe a base em baixo por
baixo de um sistema que estava de pé.

**A entrega do arranque não pode ficar no histórico.** É comportamento do
browser, não do nosso código: o Chrome substitui a entrada quando o atraso da
actualização de meta é inferior a um segundo, e acrescenta-a quando é maior. O
atraso de `0.6s` está escolhido também por isto, e há uma viagem que o prova em
vez de o assumir.

**O arranque não consulta a sessão, e é isso que o mantém seguro.** Entrega ao
destino; é o destino que decide quem entra. Um arranque que resolvesse a sessão
sozinho teria de reimplementar essa decisão, e passaria a haver dois sítios a
dizer quem pode trabalhar — que é como se cria uma passagem por baixo de um
fluxo obrigatório.
