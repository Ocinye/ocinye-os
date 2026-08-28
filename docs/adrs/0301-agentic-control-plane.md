# ADR-0301 — O Agentic Control Plane: Main Agent, Runtime, Registry

- **Estado:** Accepted
- **Domínio:** Agentic
- **Impacto:** HIGH
- **Data:** 2026-08-22
- **Complementa:** [ADR-0002](0002-deterministic-core-and-agentic-control-plane.md)

## Context

O [ADR-0002](0002-deterministic-core-and-agentic-control-plane.md) decidiu que existe um plano
agentic. Este regista o que está lá dentro e porquê está separado assim.

## Decision

### Os componentes

| Componente | Responsabilidade | Onde |
|---|---|---|
| **Main Agent / Orchestrator** | Interpretar, decompor, delegar, explicar | `agentic::runtime` |
| **Agent Runtime** | Ciclo de vida, invocação, coordenação, verificação | `agentic::runtime` |
| **Agent Registry** | Definições de agentes: âmbito, tecto, autonomia | `intelligence::agents` |
| **Capability Registry** | O que o Core publica aos agentes | `agentic::registry` |
| **Context Engine** | Contexto mínimo e autorizado | `agentic::context` |
| **Action Planner** | Validar a saída do modelo; produzir planos | `agentic::planner` |
| **Capability Executor** | Autorizar, validar, executar, auditar | `agentic::executor` |

### O Main Agent não é root

Tem a **lista de capabilities mais larga** que existe — tem de alcançar todos os
domínios para orquestrar — e **nenhum privilégio**. Não recebe `PlatformAdmin`,
não contorna políticas, não acede a segredos.

O que consegue fazer é decidido a cada pedido, contra a pessoa que o está a
usar. Existe um teste, `an_agent_never_widens_the_person_using_it`, que percorre
o catálogo inteiro de permissões com o Main Agent e um principal sem papéis, e
verifica que cada uma é recusada.

### Agent Runtime ≠ AI Gateway

O Runtime pergunta «preciso de `GENERAL`». O Gateway decide que modelo e que nó
o servem. **Nenhum nome de modelo aparece no Runtime**, e por isso a chegada da
L40S, e mais tarde de CAM-01, é uma linha no Model Registry e zero linhas aqui
([ADR-0300](0300-ai-gateway.md)).

### Agent Runtime ≠ Capability Runtime

Nomes parecidos, coisas diferentes, e vale a pena a distinção estar escrita:

- **Agent Runtime** orquestra: contexto, plano, execução, verificação.
- **Capability Runtime** ([ADR-0501](0501-capability-runtime-wasm.md)) é o
  sandbox WASM/WASI onde correm capacidades científicas isoladas.

No futuro uma capability tipada pode executar dentro de WASM. Hoje não executa,
e as duas camadas não se tocam.

### Domain Agents, não um mega-agente

`Research`, `Knowledge`, `Data`, `Mail`, `Compute`, `Search`, `Administration`.
Um agente único com instruções infinitas degrada-se com cada domínio que se lhe
acrescenta.

**O membro não escolhe.** Fala com o Main Agent; a delegação é detalhe
arquitectural. Escolher explicitamente continua possível onde faça sentido.

Nesta fase os Domain Agents existem como **fronteira de domínio no Capability
Registry** — cada capability declara o seu domínio, e o Context Engine filtra
por ele — e não como agentes com prompts próprios. Essa separação chega quando
houver inferência para a exercitar.

## Consequences

- O `AgentScope` e o tecto de classificação que já existiam em `ai_agents`
  passam a ter significado operacional: são lidos por `AgentBoundary`.
- `AutonomyLevel::Autonomous` existe no tipo e é **inalcançável**:
  `AutonomyLevel::ceiling()` é `Workflow`. Um agente que inicia trabalho que
  ninguém pediu precisa de política, dono e forma de o parar, e nada disso está
  construído (briefing §70, §145).
- Trabalho proactivo fica em **Observe → Suggest**, e `PLANNED`.
