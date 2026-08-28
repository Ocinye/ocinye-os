# ADR-0501 — Capability Runtime em WebAssembly/WASI

- **Estado:** Accepted
- **Domínio:** Compute
- **Impacto:** HIGH
- **Data:** 2026-08-22

## Context

O Ocinye OS precisa de executar transformações sobre artefactos institucionais:
importar BibTeX, extrair metadados, validar datasets, processar resultados. Estas
capacidades irão crescer, virão de vários autores e correrão sobre dados
classificados.

Executá-las no processo do Core dá-lhes, por omissão, a base de dados, o
filesystem, a rede e os secrets. É exactamente o que não se quer.

## Decision

**Ocinye Capability Runtime**: capacidades como componentes WebAssembly
executados em **Wasmtime** com **WASI**, sob capability-based security.

### Onde o WASM ganha o seu lugar

Este é o sítio onde o WASM resolve um problema real — isolamento, portabilidade,
permissões explícitas, distribuição futura entre nós — e não "porque podemos"
(briefing §64, §137). É por isso que o WASM entra aqui e **não** no Workspace
nesta fase (ADR-0600).

### Manifesto

Cada capacidade declara: identificador, nome, versão, inputs, outputs,
permissões requeridas, política de rede, política de filesystem, limites de
recursos, runtime suportado e compatibilidade. Assinatura e checksum estão
previstos e ainda **não** implementados — declarado como tal.

### Deny by default

O host concede apenas o que o manifesto pede e a política institucional aprova:

- **sem acesso à rede** salvo declaração explícita e aprovação;
- **sem filesystem do host**; apenas os inputs que o host injecta;
- **sem acesso ao PostgreSQL, a secrets ou a outros workspaces**;
- limites de combustível (fuel), memória e tempo aplicados pelo host.

### WASM não é segurança mágica

O sandbox é uma camada, não a política. Continuam a aplicar-se: validação de
input, limites de recursos, autorização antes da invocação, proveniência do
resultado e trust boundaries explícitas.

### Âmbito nesta fase

Manifesto, modelo de permissões, abstracção de runtime, host Wasmtime com limites
e **uma** capacidade de exemplo (importador de BibTeX). Não é um marketplace nem
um ecossistema de plugins.

## Alternatives

| Alternativa | Porque foi rejeitada |
|---|---|
| **Executar in-process no Core** | Dá acesso implícito a base de dados, secrets e filesystem — o problema que este ADR existe para resolver. |
| **Subprocessos com utilizador dedicado** | Isolamento dependente do SO, sem portabilidade e sem limites de recursos uniformes entre plataformas. |
| **Containers por capacidade** | Isolamento sólido, mas arranque pesado para transformações de segundos e exige um container runtime em cada nó. |
| **Wasmer / WasmEdge** | Alternativas legítimas. Wasmtime preferido por governação Bytecode Alliance, maturidade do Component Model e alinhamento com o ecossistema Rust. |
| **Adiar completamente** | Deixaria as capacidades a nascer dentro do Core, tornando a extracção futura um rewrite. |

## Consequences

**Positivas** — capacidades não confiáveis correm sob permissões explícitas;
portáveis entre Core, Worker e futuros nós sem recompilar; o modelo de permissões
existe antes do primeiro plugin externo.

**Negativas, aceites** — o Wasmtime é uma dependência substancial; a passagem de
dados através da fronteira WASM tem custo; o Component Model ainda evolui, pelo
que a fronteira usa WASI preview 1 com `wasm32-wasip1`, revisitável por ADR.

## O que a primeira integração operacional fixou

*Acrescentado a 2026-08-26, quando `knowledge::review_bibliography` passou a ser
o primeiro consumidor. Não altera a decisão acima; regista o que a
implementação teve de decidir para a cumprir.*

**O Core escolhe o componente; quem chama pede uma operação de domínio.** Não
existe caminho — de uma pessoa, de um agente ou da API — por onde um pedido
nomeie o que se executa. `Component` é uma enumeração fechada em código, sem
construtor a partir de texto, e é essa ausência que impede o endpoint de se
tornar um executor de código arbitrário com outro nome. Por isso o caminho é
`/workspaces/{id}/bibliography/review`, e não `/runtime/run`.

**O manifesto viaja com o Core, e não em disco.** É o manifesto que declara o
combustível, o tempo, a memória e que o componente não pede rede nem sistema de
ficheiros — ou seja, **é a política**. Em disco, é uma política que se edita num
servidor; embebido no binário, não se alarga sem recompilar. O componente
continua a vir de fora, porque é grande e é construído à parte.

**Um componente por construir é uma capacidade indisponível, não uma instalação
partida.** O Core arranca; a operação que precisar dele recusa com uma razão que
se lê. É a mesma escolha que o correio e a inferência fazem.

**A execução acontece fora da thread que serve pedidos.** O motor é síncrono, e
uma capacidade pode gastar o seu tempo todo: o combustível limita o que corre, e
`spawn_blocking` limita a quem custa.

**Em macOS, sinais em vez de portas de excepção Mach.** Um *exception port* do
Mach é do processo e não do motor: quem o instala arbitra as excepções de tudo o
que corre ali dentro. Num processo que também conduz browsers, isso aborta o
processo. Ver `crates/ocinye-capabilities/src/runtime.rs`.

## Referências

`CLAUDE.md` §16-A · briefing §8, §60–§66 · ADR-0004 · ADR-0600
