# ocsh — modelo de comandos

> Proposta M0. A gramática e os códigos de saída seguem o D13_OCSH do Claude
> Design; onde o repositório já decide (risco, aprovação, autoridade), o
> repositório prevalece.

## Gramática (v1)

```ebnf
linha      = comando { "|" operacao } ;
comando    = familia [ sub ] { argumento | opcao } ;
familia    = ident ;                     (* files, projects, … *)
sub        = ident ;                     (* list, mkdir, … — só se a família o declarar *)
argumento  = palavra | citada ;
opcao      = "--" ident [ "=" valor | valor ] | "-" letra ;
operacao   = ident { argumento | opcao } ; (* filter, sort, head, count, export *)
palavra    = { caracter sem espaço, sem aspas, sem "|" } ;
citada     = '"' { qualquer | '\"' } '"' | "'" { qualquer } "'" ;
```

Não existe, e é recusado **antes** de qualquer execução:
`$( )`, `` ` ` ``, `${…}`/`$VAR`, `;`, `&&`, `||`, `>`, `<`, `>>`, `&`, globs
(`*`, `?` fora de aspas não têm significado especial — são texto), `~user`.
`?` no início da linha é o atalho explícito para `nye ask`.

A linha tem limite de tamanho (4 KiB) e de símbolos; caracteres de controlo
(excepto espaço e tabulação) são recusados.

## AST

```text
Invocation { family, sub?, args: [Value], options: {name → Value}, flags: {name} }
Pipeline   { head: Invocation, stages: [Stage] }
Stage      { op: filter|sort|head|count|export, args }
Value      = Text | Integer | Bool   (tipado pela definição, não pelo parser)
```

O parser só conhece a sintaxe. A validação contra o registo (família existe,
sub existe, argumentos obrigatórios, tipos, opções conhecidas) é o segundo passo,
e produz `Typed { definition, args, options }` ou um erro de uso.

## Definição de comando

```text
CommandDefinition {
  family            "files"
  sub               "mkdir"              (ou nenhum)
  aliases           []                    (v1: nenhum; sem aliases POSIX)
  args              [ArgSpec{name, kind: Text|Path|Id|Integer, required, sensitive}]
  options           [OptSpec{name, kind, sensitive}]
  binding           Local                 (help, clear, history, exit)
                  | Capability(id)        (sempre que há efeito ou leitura do Core)
  output            Table{columns} | Lines | Scalar | Note
  json              bool                  (aceita --json)
  help_key          "ocsh.cmd.files.mkdir"
  complete          None | Apps | Folders | Workspaces | Contexts | Projects
  min_role          Member | Admin | Operator   (só descoberta; a decisão é do Core)
}
```

Um só registo alimenta parse, validação, ajuda, autocompletar, documentação e
execução. O risco **não** está na definição do comando: é o da capability
(`RiskLevel` + `ApprovalRequirement`), lido do registo agentic no momento da
execução. Um comando não pode declarar-se mais seguro do que a acção que faz.

### Risco → interacção

| Capability | Terminal |
|---|---|
| `ReadOnly`, `LowImpact` com `Approval::Never` | executa |
| aprovação `Once`/`Always`, ou `MaterialMutation` | pré-visualização do Core + `[s/N]` (plano) |
| `ExternalEffect`, `Privileged`, irreversível | pré-visualização + palavra escrita + recibo (plano) |
| capability não-delegável | não é comando do ocsh; a UI é o caminho |

## Códigos de saída

| Código | Significado |
|---|---|
| 0 | sucesso |
| 1 | falha de execução, recurso não encontrado, confirmação recusada |
| 2 | uso inválido (sintaxe, sub, opção, argumento) |
| 69 | capacidade indisponível (ex.: IA sem recurso) |
| 77 | permissão negada, política bloqueou, requer elevação |
| 126 | bloqueado por desenho (shell do anfitrião, `sudo`) |
| 127 | comando não encontrado |
| 130 | cancelado |

Estáveis e documentados; não espelham o HTTP.

## Contexto

Cada sessão de terminal (um separador) tem um envelope:
`{actor, instance, context: personal | workspace(id), locale, timezone, session}`.
O actor e a instância vêm da sessão do Workspace; o contexto viaja em cada
pedido e o Core **reautoriza** que o actor lhe chega, sempre. `context use`
muda o contexto do separador só depois de o Core o aceitar.

## Espaço de nomes

`~` é o contexto activo. `~/files` é o armazenamento do Ocinye nesse contexto
(pessoal: «Meus ficheiros»; ambiente: os ficheiros do ambiente). Caminhos
resolvem para IDs de pasta pela API de Ficheiros. `..`, codificações, NUL e
caminhos absolutos do anfitrião (`/etc`, `/home`) não saem do espaço: ou
resolvem dentro de `~/files`, ou são erro «caminho fora do espaço Ocinye».

## Segredos

Argumentos e opções marcados `sensitive` na definição (e, por nome, `--password`,
`--token`, `--secret`, `--key`) são substituídos por `••••` antes de histórico,
logs e descrição de auditoria. Nenhum comando v1 pede um segredo pela linha; os
que vierem a pedir usam uma entrada mascarada separada.

## Inventário v1 (proposto)

| Comando | Ligação | Estado da capability |
|---|---|---|
| `help`, `clear`, `history`, `exit` | Local | — |
| `whoami` | Capability `identity.self.read` | **a criar** (leitura) |
| `context` / `context list` / `context use` | Capability `research.workspace.list` + reautorização | **a criar** (leitura) |
| `apps list` | `identity.apps.list` | **a criar** (leitura do registo + activação + fixações) |
| `apps pin` / `apps unpin` | `identity.apps.pin` / `.unpin` | **a criar** (`LowImpact`) |
| `open <app>` | Local de navegação + verificação `identity.apps.list` | — |
| `projects list` / `show` | `research.workspace.list` / `research.project.read` | a criar / existe |
| `tasks list` | `collaboration.task.list` | existe |
| `storage` | `resource.self.read` | **a criar** (leitura) |
| `system status` / `version` | `platform.status.read` | **a criar** (leitura, sem segredos) |
| `ai status` | `intelligence.status.read` | **a criar** (leitura) |
| `nodes list` | `compute.node.list` | existe |
| `files pwd/ls/cd/mkdir/rename/mv/trash/restore/info` | `files.personal.*` | **a criar** (leitura e `LowImpact`) |
| `files open` / `files download` | navegação same-origin | — |
| `notes list` | `knowledge.note.list` | a criar (leitura) |
| `nye ask` / `? …` | invocação explícita da Superfície Universal | existe (`/agentic/invoke`) |

Fora da v1, por falta de contrato: `window *`, `desktop *` (G-05), `session
elevate` (G-14), `backup *` (sem API), `members *` mutáveis (não-delegáveis),
`system restart/shutdown/update` (sem capability governada), `jobs *`, Ctrl+C
sobre trabalhos (sem cancelamento no Core), histórico no servidor (G-15),
`.ocsh` (M8).

Cada comando que entrar tem de funcionar, ter ajuda nas três línguas, estar
autorizado pelo Core e ter testes. Nenhum comando morto.
