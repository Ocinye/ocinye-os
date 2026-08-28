# ADR-0002 — Deterministic Core + Agentic Control Plane

- **Estado:** Accepted
- **Domínio:** Foundation
- **Impacto:** FOUNDATIONAL
- **Data:** 2026-08-22
- **Depende de:** [ADR-0001](0001-ocinye-os-definition.md)
- **Relaciona-se com:** [ADR-0300](0300-ai-gateway.md) · [ADR-0006](0006-modular-monolith.md)
- **Decisões derivadas:** [ADR-0301](0301-agentic-control-plane.md) · [ADR-0302](0302-agent-access-intersection.md) · [ADR-0303](0303-capability-registry-and-executor.md)

## Context

O [ADR-0001](0001-ocinye-os-definition.md) declarou que o Ocinye OS é AI-native
e governado pelo Core. Esta ADR decide **como** — que responsabilidades ficam de
que lado, e o que torna a não-dependência verificável em vez de declarada.

A IA no Ocinye OS existia como um módulo: um ecrã «Ocinye AI», agentes que se
definiam, um Gateway que reportava indisponível. Útil, e estruturalmente errado
para o que o `CLAUDE.md` §8 declara — que a IA é uma **capacidade transversal do
sistema operacional institucional**, não um departamento.

Ao mesmo tempo, a instituição não tem nenhum nó de IA, e pode não ter durante
meses. Uma arquitectura que assuma inferência disponível produz um sistema que
não funciona.

As duas coisas têm de ser verdade ao mesmo tempo.

## Decision

Duas frases, e a tensão entre elas é a arquitectura:

> **Ocinye OS is AI-native, not AI-dependent.**

> **Ocinye OS is operated with AI, governed by the Core.**

Concretizadas como **Deterministic Core + Agentic Control Plane**.

### O que cada lado detém

| | Detém |
|---|---|
| **Agentic Control Plane** | Compreender, planear, orquestrar, explicar |
| **Ocinye Core** | Identidade, autorização, política, invariantes, persistência, estado, auditoria |

Nenhuma responsabilidade da segunda coluna passa para a primeira. O plano
agentic é uma **interface de operação**, não uma autoridade.

### Não-dependência é verificável, não uma intenção

A distinção entre *native* e *dependent* não é retórica se for testável. É:

- `Intent::Search` não precisa de inferência, e a Universal Command Surface
  responde-lhe com zero nós de IA;
- todas as acções tradicionais do Workspace continuam;
- `Ask` e `Act` devolvem `AgenticOutcome::Unavailable` **com a razão e com o que
  ainda funciona**, e a interface renderiza isso como estado, não como erro;
- existe um teste, `search_works_with_zero_ai_nodes`, que corre exactamente no
  estado desta instalação.

### Não é um chatbot com um sistema por trás

O `CLAUDE.md` §2 lista o que não estamos a construir. «Um chatbot que possui
algumas ferramentas» pertence a essa lista tão bem como um CMS.

Consequências no desenho:

- **Prompt Everywhere, not Chat Everywhere.** Não há histórico de mensagens como
  superfície principal. O que aparece são resultados, planos e confirmações.
- A navegação, os menus e os formulários continuam a ser o caminho normal. A
  superfície de comando é *outro* caminho, não o substituto.
- A resposta de um agente é UI nativa — recursos ligáveis, um plano, uma
  confirmação — e não um parágrafo.

## Alternatives

**Manter a IA como módulo.** Simples, e contraria o `CLAUDE.md` §8: acrescentar
IA a cada módulo separadamente produz seis integrações incompatíveis e nenhuma
fronteira comum onde impor política.

**Construir sobre um framework de agentes existente.** Nenhum dos maduros trata
autorização institucional, classificação e auditoria como fronteiras de primeira
ordem, e o `CLAUDE.md` §16-A diz Rust-first para componentes institucionais.

**Esperar por um nó de IA.** Deixaria a arquitectura por decidir e o primeiro nó
chegaria a um sistema sem sítio onde o ligar. O que se construiu agora é a
fronteira; a inferência liga-se a ela.

## Consequences

- **A inferência continua `PLANNED`.** O que existe é tudo excepto o modelo:
  registry, executor, contexto, plano, aprovações, auditoria, superfície. Todas
  testadas sem modelo, porque nenhuma precisa de um.
- Quando existir um nó, o caminho que hoje devolve `Unavailable` monta o
  envelope, pede `GENERAL` ao Gateway e valida a proposta. Nenhuma outra parte
  muda.
- Todo o módulo nativo novo deve avaliar que capabilities expõe ao plano
  agentic ([ADR-0303](0303-capability-registry-and-executor.md)).
