# ADR-0004 — Rust-first como princípio tecnológico da Ocinye

- **Estado:** Accepted
- **Domínio:** Foundation
- **Impacto:** FOUNDATIONAL
- **Data:** 2026-08-22
- **Âmbito:** Todo o Ocinye OS

## Context

O Ocinye OS é o sistema operacional institucional da Ocinye: pretende tornar-se
o *system of record* da instituição, com um horizonte de vida de décadas, e
progressivamente ligar-se a infraestrutura física própria (nós de computação,
GPU, storage, futuramente HPC).

Este perfil impõe requisitos pouco habituais numa aplicação de gestão:

- **Correcção a longo prazo.** Erros de autorização ou de integridade de dados
  são institucionalmente caros e difíceis de detectar tardiamente.
- **Um único conjunto de tipos canónicos** partilhado entre Core, Workspace,
  Worker e Node Agent, sem duplicação divergente (briefing §16).
- **Componentes que correrão em hardware próprio** — agentes de nó, runtimes de
  capacidades — onde ausência de runtime pesado e previsibilidade de recursos
  são vantagens reais.
- **Isolamento de extensões** através de WebAssembly, cujo ecossistema de host
  runtimes e de *guest tooling* é hoje mais maduro em Rust do que em qualquer
  alternativa.

Uma primeira iteração deste repositório foi iniciada em Python/FastAPI, seguindo
a stack de referência então inscrita no `CLAUDE.md` §18. Essa iteração nunca foi
commitada, nunca teve migrations, testes, documentação nem milestone concluída.

## Decision

**Ocinye is Rust-first.**

Rust é a linguagem principal do Ocinye OS e a escolha por defeito para
componentes institucionais, serviços, runtimes, agentes, contratos e ferramentas
operacionais da plataforma, salvo quando outra tecnologia for claramente mais
adequada ao problema.

Isto é um **princípio arquitectural oficial da Ocinye**, parte da sua identidade
tecnológica, e não uma escolha circunstancial da primeira versão do Ocinye OS.

Duas regras eliminam a ambiguidade:

1. **Rust-first não significa Rust-only.** A investigação científica pode usar
   Python, Fortran, C/C++, Julia, OpenFOAM, MPI ou qualquer outra tecnologia
   adequada. Software científico maduro **não** é reescrito em Rust para ser Rust.
2. **WebAssembly complementa Rust.** WASM/WASI é usado estrategicamente para
   interface, isolamento, portabilidade e extensibilidade — nunca como obrigação
   universal.

Regra operacional vinculativa:

> Qualquer novo componente do Ocinye OS deve ser considerado primeiro para
> implementação em Rust. A adopção de outra linguagem para componentes
> institucionais requer uma razão técnica concreta e deve ser documentada quando
> tiver impacto arquitectural.

Corolário: Rust-first **não** autoriza reinventar infraestrutura madura. Não se
constrói base de dados, identity provider, filesystem, TLS, criptografia,
message broker, storage engine, container runtime nem scheduler HPC próprios.

### Supersessão da iteração Python/FastAPI

A stack de referência anterior (FastAPI + Next.js), inscrita no `CLAUDE.md` §18,
fica **superseded por esta decisão**. O código Python correspondente foi removido
do repositório: não estava commitado, não tinha migrations, testes nem
documentação, e mantê-lo criaria exactamente a divergência entre documentação e
implementação que o `CLAUDE.md` §69 proíbe. O `CLAUDE.md` §18 foi actualizado e
passou a referenciar este e os restantes ADRs.

## Alternatives

| Alternativa | Porque foi rejeitada |
|---|---|
| **Python (FastAPI)** | Excelente para investigação e prototipagem, e continuará a ser usado em workloads científicos. Como núcleo institucional de longa duração, oferece garantias de correcção mais fracas em tempo de compilação, e obrigaria a duplicar os tipos canónicos no cliente. Foi a escolha inicial; é agora superseded. |
| **TypeScript full-stack (Node)** | Tipos partilhados entre servidor e browser são um ponto forte real, mas o Node é má base para Node Agent e Capability Runtime, e o isolamento de extensões via WASM é menos maduro do lado do host. |
| **Go** | Operacionalmente sólido e simples. Sistema de tipos menos expressivo para invariantes de domínio (estados, classificação, capacidades), e ecossistema WASM host/guest menos avançado. |
| **Java/Kotlin (JVM)** | Maturidade e ferramentas de topo. Peso de runtime desproporcionado para agentes em nós próprios, e afastado do objectivo de proximidade ao hardware da futura camada física. |
| **Rust-only** | Rejeitado explicitamente: obrigaria a reescrever ou evitar software científico consolidado, contrariando o princípio de não reinventar ferramentas (briefing §102). |

## Consequences

**Positivas**

- Um único conjunto de tipos canónicos (`ocinye-contracts`) partilhado por Core,
  Workspace, Worker e Node Agent; divergência de definições passa a ser um erro
  de compilação, não um bug em produção.
- Invariantes de domínio (workflows, classificação, papéis) expressáveis no
  sistema de tipos, com `match` exaustivo a impedir estados esquecidos.
- Uma única linguagem cobre servidor, worker, agente de nó, runtime de
  capacidades e interface — reduzindo a superfície de conhecimento necessária.
- Base natural para o Capability Runtime WASM/WASI (ADR-0501) e para binários
  sem runtime a correr em nós próprios.

**Negativas, aceites conscientemente**

- Curva de aprendizagem superior e tempos de compilação mais longos do que em
  Python ou TypeScript.
- Menor disponibilidade de programadores Rust do que Python no mercado angolano;
  mitigado por documentação forte e por Rust ser exigido apenas nos componentes
  institucionais, não na investigação.
- Ecossistema mais fino em áreas científicas específicas; mitigado precisamente
  pela regra "Rust-first, não Rust-only".
- Trabalho já iniciado em Python foi descartado.

## Referências

- `CLAUDE.md` §16-A (princípio), §18 (stack), §71 (princípio de evolução)
- ADR-0005 (monorepo), ADR-0008 (Axum/Tokio), ADR-0600 (Leptos), ADR-0501 (WASM)
