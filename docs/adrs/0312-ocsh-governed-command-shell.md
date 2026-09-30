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

## Emenda — D008 (2026-09-30)

Decidida pelo utilizador na integração da D008. A história acima fica como
estava; o que muda diz-se aqui.

1. **§5, a palavra escrita, fica substituída.** O ocsh **não** pede para
   escrever `SIM`, `CONFIRMAR`, `REVOGAR` nem equivalente. As operações de alto
   impacto seguem o modelo de confirmação governada comum do Ocinye OS:

   ```text
   comando lido → capability tipada → autorização e risco no Core
   → plano imutável proposto → confirmação privilegiada partilhada
   → execução exacta → recibo e auditoria
   ```

   A interface de confirmação é a partilhada com o resto do Ocinye OS (D006);
   o Terminal não inventa uma segunda autoridade de confirmação. Uma futura
   Consola do Anfitrião (break-glass) para operadores pode definir uma
   cerimónia mais forte, e **não é o ocsh**. Hoje nenhum comando do registo v1
   exige confirmação; um que a exigisse é recusado (77) até o plano chegar ao
   Terminal (TERMINAL-11).
2. **`|` fica, e diz-se o que é.** Onde se lia «sem pipes», lê-se: **sem pipes
   da shell; a composição tipada do ocsh é permitida.** Cada etapa resolve-se
   num registo fechado de passos tipados (`filter`, `sort`, `head`, `count`,
   `export json`), é validada estruturalmente e opera sobre os dados que o Core
   já autorizou. Nunca passa stdout a um processo, nunca resolve executáveis,
   nunca invoca `/bin/sh`, bash, zsh, PowerShell ou cmd, nem transforma texto
   em comando. Quando uma etapa desencadeia uma capability governada, a
   autoridade do Core aplica-se.
3. **«Quis dizer» só com comandos que existem.** As sugestões POSIX
   (`POSIX_HINTS`) só apontam para comandos registados (`cls → clear`,
   `man → help`), com um teste; as outras palavras POSIX (`ls`, `cd`, `rm`…)
   explicam o modelo e não sugerem nada. O Core só sugere uma família que a
   pessoa vê.
4. **Uma sessão por janela.** O Terminal é `SingleInstance`: sem separadores,
   painéis divididos nem «sessão de administração»; `exit` fecha a janela. O
   texto do catálogo que falava de separadores, painéis, elevação, inspector e
   preferências saiu.
5. **A ponte da Nye.** `nye ask` / `? …` devolve do Core um bloco tipado
   `Ask { question }`; o Workspace leva a pergunta ao caminho canónico
   (`POST /api/v1/ai/prompt`), onde o Core decide de novo (permissão de IA,
   política, disponibilidade). Um comando desconhecido continua `127` e nunca
   chega à Nye.
6. **O histórico** vive só na memória da janela e guarda só a linha redigida
   (`echo`, de `ocsh::redact`, devolvido pelo Workspace).

A separação entre o Terminal e o Browser é da
[ADR-0623](0623-terminal-and-browser-separate-boundaries.md).
