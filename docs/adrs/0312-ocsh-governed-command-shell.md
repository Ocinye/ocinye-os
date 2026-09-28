# ADR-0312 — ocsh: uma shell de comandos governada sobre as capabilities do Core

- **Estado:** Proposed
- **Domínio:** Agentic
- **Impacto:** HIGH
- **Depende de:** [ADR-0303](0303-capability-registry-and-executor.md) · [ADR-0307](0307-dual-entry-single-authority.md) · [ADR-0411](0411-execution-time-principal-freshness.md)
- **Data:** 2026-09-27

## Context

O Ocinye OS tem duas entradas para as mesmas operações: a interface e o Nye
(ADR-0307). Falta a terceira que um sistema operativo tem — uma linha de
comandos, para quem trabalha depressa, repete, e quer compor.

O caminho fácil seria o errado: um terminal no browser ligado a uma shell do
servidor. Isso daria a qualquer membro com acesso ao Terminal o anfitrião
inteiro — base de dados, segredos, objectos —, por cima de toda a autorização
que o Core existe para aplicar. Um «terminal» assim não é uma funcionalidade; é
a remoção da fronteira de segurança.

O segundo caminho errado é mais subtil: um terminal que aceite qualquer texto e
o entregue a um modelo para decidir o que fazer. A linha de comandos passaria a
ser uma superfície de *prompt injection* com efeitos.

## Decision

### A frase

> **ocsh is the canonical Ocinye command shell. It parses deterministically,
> executes only typed Core capabilities, and never reaches the host.**

### 1. Terminal ≠ ocsh ≠ Host Shell

O **Ocinye Terminal** é a aplicação. O **ocsh** é a linguagem e a execução. A
**Host Shell** — uma consola do anfitrião para operadores — é um conceito
separado, privilegiado, futuro, com a sua própria ADR, e nada do ocsh a
alcança. Não há `sh -c`, `exec`, `std::process::Command` com texto do pedido,
globbing, substituição de comandos, redirecção, nem variáveis do processo.
`sudo`, `bash`, `ssh` e semelhantes são reconhecidos para dizer isso (código
126), nunca executados.

### 2. Parse determinístico, no Core

A gramática é pequena: `família [sub] [argumentos] [--opções] [| operação …]`.
O lexer recusa, antes de haver comando, a sintaxe de anfitrião fora de aspas
(`;`, `&`, `&&`, `||`, `>`, `<`, `` ` ``, `$(`, `${`, `$NOME`). O parser valida
contra o registo e devolve uma invocação tipada ou um erro de uso. Vive em
`ocinye-contracts` (puro, sem IO) para servir Core, Workspace e um futuro CLI;
**o parse que decide é o do Core**.

### 3. Comando desconhecido não vai para o Nye

Só `nye …` (e o atalho `? …`) entra em linguagem natural, e o Nye **propõe**.
Um comando desconhecido é código 127, com «quis dizer» por distância de edição.

### 4. Só capabilities

Um comando não-local invoca uma capability do registo agentic, pelo executor
existente — autoridade restabelecida à fonte (ADR-0411), política, esquema,
risco, aprovação, auditoria. Onde falta a capability, cria-se, e ela passa a
servir também o Nye. Não há executor do Terminal, nem chamada directa a
serviços, nem atalho privilegiado. O risco é o da capability: um comando não se
declara mais seguro do que a acção que faz.

### 5. Confirmações são planos

Risco que exige confirmação cria um `ActionPlan` de um passo. A confirmação é a
aprovação existente — ligada à pessoa, ao digest canónico do plano e a quinze
minutos — e a execução é o `execute` existente, de uso único. A palavra escrita
dos riscos altos é verificada pelo Core.

### 6. Um registo

Uma só lista de comandos alimenta parse, validação, ajuda, autocompletar,
documentação e execução. Cada comando ligado a uma capability tem de existir no
registo agentic; um teste falha se não existir. Nenhum comando morto.

### 7. Saída tipada, códigos estáveis

O Core devolve blocos tipados; o cliente nunca interpreta HTML vindo do Core.
Códigos de saída estáveis: 0, 1, 2, 69, 77, 126, 127, 130
(`docs/terminal/COMMAND_MODEL.md`).

### 8. Segredos

Opções e argumentos sensíveis são redigidos antes de histórico, logs e
auditoria. Nenhum comando pede um segredo pela linha.

## Alternatives

- **Shell do anfitrião com restrições** (lista de comandos permitidos, contentor).
  Recusado: a fronteira passaria a ser a lista, e o Core deixaria de ser a
  autoridade. Uma consola de operador, se vier, é outra superfície.
- **Linguagem natural por omissão.** Recusado: indeterminismo e injecção.
- **Gramática POSIX completa.** Recusado: complexidade sem uso, e cada operador
  a mais é uma pergunta de segurança a mais.
- **Um executor próprio do Terminal.** Recusado: seria uma segunda
  implementação de autorização, a divergir da primeira (ADR-0307).

## Consequences

- Terminal, Nye e interface partilham a semântica de autoridade.
- Comandos novos exigem capabilities novas: a superfície agentic cresce com o
  Terminal, e cada capability nova é também do Nye.
- A shell não compõe processos; compõe dados tipados (pipelines).
- Streaming, cancelamento e elevação de sessão ficam por decidir em ADRs
  próprias, sobre infra que ainda não existe.
