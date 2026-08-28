# ADR-0003 — Módulos nativos, não aplicações desligadas

- **Estado:** Accepted
- **Domínio:** Foundation
- **Impacto:** FOUNDATIONAL
- **Data:** 2026-08-22
- **Depende de:** [ADR-0001](0001-ocinye-os-definition.md) · [ADR-0006](0006-modular-monolith.md)
- **Primeira aplicação:** [ADR-0400](0400-mail-as-institutional-surface.md)

## Context

A instituição vai precisar de correio, de calendário, de contactos, de gestão de
laboratório, de coisas que ainda não sabemos nomear. A pergunta que se coloca de
cada vez é a mesma:

> Integramos uma aplicação existente, ou construímos isto dentro do Ocinye OS?

Sem uma resposta decidida, a resposta é tomada caso a caso — e cada caso escolhe
a integração, porque é sempre mais rápida. Ao fim de três decisões dessas, a
instituição tem três sistemas com identidades próprias, e a classificação, a
auditoria e a memória institucional param nas fronteiras entre eles.

O correio tornou este trade-off concreto: existiam webmails maduros que se
podiam embeber num iframe em dias.

## Decision

> **Aplicações institucionais pertencem ao Ocinye OS como módulos nativos, não
> como aplicações desligadas.**

Um módulo nativo é um módulo do Ocinye Core com a sua própria superfície no
Ocinye Workspace. Partilha, sem excepção:

| | |
|---|---|
| **Identidade** | As mesmas pessoas, as mesmas sessões |
| **Autorização** | O mesmo catálogo de permissões, o mesmo `can()` |
| **Classificação** | `PUBLIC` · `INTERNAL` · `CONFIDENTIAL` · `RESTRICTED` |
| **Pesquisa** | O mesmo índice, com a mesma consciência de permissões |
| **Eventos** | O mesmo outbox transaccional |
| **Auditoria** | O mesmo registo append-only |
| **Disponibilidade** | O mesmo modelo de `SystemCapability` |
| **Plano agentic** | Expõe as operações que forem seguras e úteis |

### Recursos externos entram por uma fronteira própria

Nativo **não** significa que a Ocinye escreva um servidor de correio. Significa
que o módulo é do Ocinye OS e usa o recurso externo através de um adaptador que
o módulo detém:

```
Módulo nativo  →  domínio do Core  →  Adaptador  →  protocolo  →  serviço externo
```

O domínio nunca vê o protocolo. É a mesma forma que o object storage
([ADR-0200](0200-object-storage.md)), a inferência
([ADR-0304](0304-canonical-inference-contract.md)) e o correio
([ADR-0401](0401-mail-provider-abstraction.md)) usam.

### Todo o módulo novo declara o seu contrato

Domínio, recursos, permissões, âmbitos, capabilities, eventos, integração com a
pesquisa, regras de contexto para IA, comportamento de classificação, eventos de
auditoria, disponibilidade e superfície contextual na interface.

## Alternatives

**Integrar aplicações maduras.** Mais rápido em cada caso individual. O custo
não aparece no primeiro módulo — aparece no terceiro, quando a instituição tem
três modelos de identidade e nenhuma resposta à pergunta «quem teve acesso a
isto».

E o `CLAUDE.md` §16 obriga a tratar a fronteira com um sistema externo como não
confiável: pouco se pode afirmar sobre o que lá acontece.

**Serviços separados por módulo.** Contraria [ADR-0006](0006-modular-monolith.md)
sem necessidade real. Os módulos partilham pessoas, permissões e auditoria;
extrair um cedo cria três chamadas de rede para responder «esta pessoa pode ler
isto».

**Decidir caso a caso.** É o estado sem esta ADR, e converge sempre para a
integração.

## Consequences

- Cada módulo nativo custa mais a construir do que uma integração custaria a
  ligar. É o preço de a instituição ter uma só identidade e uma só memória.
- Um módulo com fronteira externa herda a hostilidade dessa fronteira. O correio
  é o caso extremo: qualquer pessoa no mundo pode enviar-lhe conteúdo
  ([ADR-0402](0402-mail-html-sanitisation.md)).
- **Nem tudo é um módulo.** Software científico consolidado — OpenFOAM, um
  solver, um notebook — não se reimplementa: liga-se, através do Compute Plane
  ou do Capability Runtime ([ADR-0004](0004-rust-first.md)).
- O contrato acrescenta trabalho a cada módulo novo, e é isso que impede o
  décimo módulo de ser um sistema à parte com o logótipo certo.
