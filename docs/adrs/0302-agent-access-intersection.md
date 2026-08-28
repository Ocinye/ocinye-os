# ADR-0302 — Effective Agent Access é uma intersecção

- **Estado:** Accepted
- **Domínio:** Agentic
- **Impacto:** HIGH
- **Data:** 2026-08-22
- **Complementa:** [ADR-0100](0100-authorization-model.md) · [ADR-0101](0101-permissions-scopes-and-grants.md)

## Context

Um agente é configurado por um membro. A configuração diz que capabilities
admite, que classificação alcança, que autonomia tem, a que unidade ou workspace
está ligado.

Um sistema onde essa configuração pudesse *conceder* seria um sistema onde
escrever a definição certa dá acesso ao que se quiser. Não é uma possibilidade
teórica: é a forma mais óbvia de atacar esta arquitectura.

## Decision

> **Effective Agent Access = Actor Access ∩ Agent Scope ∩ Resource Policy**

Uma **intersecção**. Nunca uma união.

### A ordem das portas é parte da decisão

`may_invoke` verifica, por esta ordem:

1. **o actor** — `can(principal, permission, ctx, resource)`;
2. **a definição do agente** — a capability está admitida?
3. **a ligação do agente** — a unidade e o workspace a que está preso;
4. **a classificação** — o tecto do agente, depois o da capability;
5. **a autonomia** — o mínimo entre a do agente e a da capability.

O actor é o primeiro, e cada porta seguinte só pode estreitar. Uma implementação
que verificasse a definição do agente primeiro estaria correcta hoje e a uma
refactorização de distância de ser uma escalada de privilégio, porque a forma
deixaria de dizer qual é a autoritativa.

### Configuração de agente é entrada não confiável

Um membro escreve-a. Portanto:

- **cada campo é um tecto**, nenhum é uma concessão;
- um agente que declare âmbito institucional e todas as capabilities continua
  limitado por quem o usa;
- partilhar um agente **não transfere** permissões sobre recursos;
- system prompts privilegiados não são configuráveis pelo utilizador.

### Alguma autoridade não é delegável de todo

`is_delegable_to_agents` recusa: gestão de permissões, gestão de papéis, criação
e gestão de membros, administração da plataforma, infraestrutura de IA,
administração de computação e administração de correio.

Não é uma preferência. **O registry recusa-se a arrancar** se alguma capability
exigir uma destas — um `assert!` na construção, para que não possa entrar sem
alguém reparar.

A razão: mudar quem acede a quê é um acto que uma pessoa pratica
deliberadamente, não um desfecho de uma conversa que uma frase num email pode
influenciar.

## Alternatives

**Deixar a definição de agente conceder dentro de limites.** Convidativo para
agentes institucionais. Recusado: a excepção tornar-se-ia o caminho, e a
intersecção deixaria de ser uma invariante para passar a ser uma convenção.

## Consequences

- `AgentBoundary` vive em `ocinye-domain` e é pura, o que torna estas decisões
  exaustivamente testáveis — e os testes exaurem-nas.
- Uma recusa não diz **qual** porta a travou. Todas leem `PermissionDenied` para
  fora; a razão fica na mensagem e na auditoria. Dizer qual gate parou alguém
  desenha o mapa da fronteira para quem a está a sondar.
- O Main Agent é o caso extremo que prova a regra: a lista mais larga que
  existe, e nenhum privilégio.
