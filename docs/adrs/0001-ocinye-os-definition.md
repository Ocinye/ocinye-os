# ADR-0001 — O Ocinye OS como sistema operacional institucional AI-native

- **Estado:** Accepted
- **Domínio:** Foundation
- **Impacto:** FOUNDATIONAL
- **Data:** 2026-08-22
- **Decisões derivadas:** [ADR-0002](0002-deterministic-core-and-agentic-control-plane.md) · [ADR-0003](0003-native-modules.md) · [ADR-0004](0004-rust-first.md)

## Context

A Ocinye é uma instituição angolana de investigação aplicada, engenharia e
infraestruturas digitais. Precisa de infraestrutura digital própria.

A resposta por omissão a essa necessidade é construir aplicações: um sítio com
área privada, um gestor documental, mais tarde um painel administrativo, mais
tarde uma integração de IA. Cada uma resolve o seu problema, e ao fim de alguns
anos a instituição tem sete sistemas que não se conhecem, sete modelos de
identidade e nenhuma memória institucional.

Esta ADR existe para recusar esse caminho **antes** de ele começar, e para
declarar o que estamos a construir em vez dele.

Nenhuma ADR anterior formalizava isto. Estava distribuído pelo `CLAUDE.md`, pelo
README e implícito em dezenas de decisões — o que é dizer que a decisão mais
fundamental do projecto não tinha registo próprio.

## Decision

> **O Ocinye OS é o sistema operacional institucional da Ocinye.**

Não uma aplicação, nem um conjunto delas. A infraestrutura digital central
através da qual a instituição organiza pessoas, formula ideias, conduz
investigação, preserva conhecimento, comunica e opera recursos computacionais.

Quatro propriedades definem-no, e cada uma tem a sua própria ADR:

**AI-native, não AI-dependent.** A inteligência artificial é uma interface
primária de operação, transversal ao sistema — e o sistema opera por inteiro sem
nenhum modelo disponível ([ADR-0002](0002-deterministic-core-and-agentic-control-plane.md)).

**Governado pelo Core.** A autoridade institucional — identidade, autorização,
política, invariantes, persistência, verificação, auditoria — pertence a uma
camada determinística, e nada a partilha
([ADR-0002](0002-deterministic-core-and-agentic-control-plane.md),
[ADR-0100](0100-authorization-model.md)).

**Módulos nativos, não aplicações desligadas.** Uma aplicação institucional —
correio, investigação, um futuro calendário — entra no Ocinye OS como módulo,
com a mesma identidade, autorização e auditoria que o resto
([ADR-0003](0003-native-modules.md)).

**Orientado a capabilities.** O que qualquer coisa pode fazer ao sistema é um
conjunto de operações tipadas e autorizadas, não acesso à infraestrutura
([ADR-0303](0303-capability-registry-and-executor.md)).

### O que isto exclui

Um website com área privada. Um CMS. Um painel administrativo. Um dashboard
SaaS. Um chatbot com login. Uma intranet genérica. Um gestor de ficheiros.

A distinção não é semântica: cada um desses desenhos leva a um sistema que a
instituição sobrevive, e o que se pretende é o inverso.

### Horizonte

O sistema deve sobreviver à evolução da própria instituição. Não é desenhado em
torno das necessidades dos primeiros quatro utilizadores, e deve funcionar com
dez unidades, cem investigadores, múltiplos nós computacionais — e sem nenhum
deles.

## Alternatives

**Um conjunto de aplicações integradas.** O caminho comum, e o que produz sete
modelos de identidade. Recusado em [ADR-0003](0003-native-modules.md).

**Adoptar uma plataforma existente.** Nenhuma trata autorização institucional,
classificação, proveniência e um plano agentic como fronteiras de primeira
ordem. Adaptar uma custaria mais do que construir, e a arquitectura ficaria
refém das decisões de produto de outra organização.

**Adiar a definição.** Tentador — o sistema podia começar por um módulo e
descobrir-se pelo caminho. Recusado porque as decisões que este documento
enquadra são precisamente as que não se tomam a meio.

## Consequences

- Toda a decisão arquitectural subsequente é avaliada contra esta: *pertence a
  um sistema operacional institucional de investigação, ou é mais uma página de
  um website?*
- A rastreabilidade não é acrescentada anos depois. Nasce com cada entidade que
  a justifique.
- Estado real e roadmap são coisas distintas, marcadas como tal, sempre
  (`CLAUDE.md` §69).
- Este documento é a porta de entrada da biblioteca de ADRs. Não repete o
  README: nomeia as decisões e aponta para elas.
