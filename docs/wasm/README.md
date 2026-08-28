# WebAssembly no Ocinye OS

**WebAssembly complementa Rust.** É usado estrategicamente para interface,
isolamento, portabilidade e extensibilidade — nunca como obrigação para todos os
componentes (`CLAUDE.md` §16-A).

## Onde é usado hoje

| Uso | Estado | Porquê aqui |
|---|---|---|
| **Capability Runtime** | `CURRENT` | Isolamento, permissões explícitas, limites de recursos, portabilidade futura entre nós. O WASM resolve um problema real. |

## Onde não é usado, e porquê

| Candidato | Decisão |
|---|---|
| **Workspace no browser** | SSR por agora ([ADR-0600](../adrs/0600-leptos-workspace-runtime.md)). Introduzir uma cadeia de build WASM antes de existir interactividade que a justifique seria usar WASM porque podemos. Hidratação é `PLANNED`. |
| **Workloads científicos** | Não. O software científico maduro corre nativamente. Reescrevê-lo para WASM contraria o princípio de não reinventar ferramentas. |
| **Lógica do Core** | Não. Não ganha nada com o sandbox e perderia acesso directo à base de dados. |

## A pergunta que decide

> Estamos a usar WASM porque resolve um problema, ou porque podemos?

Se for a segunda, não se usa.

## Alvo

`wasm32-wasip1` (WASI preview 1), com Wasmtime como host. O Component Model ainda
evolui; a fronteira actual é deliberadamente simples e revisitável por ADR.
