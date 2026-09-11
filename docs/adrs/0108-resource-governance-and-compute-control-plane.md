# ADR-0108 — Governança de recursos e o control-plane de computação

- **Estado:** Accepted
- **Domínio:** Identity, segurança, autorização e governação
- **Impacto:** FOUNDATIONAL
- **Data:** 2026-09-12
- **Complementa:** [ADR-0100](0100-authorization-model.md) · [ADR-0101](0101-permissions-scopes-and-grants.md) · [ADR-0500](0500-compute-registry-node-agent.md) · [ADR-0300](0300-ai-gateway.md) · [ADR-0304](0304-canonical-inference-contract.md)

## Context

O Ocinye OS sabe **o que** cada actor pode aceder ou fazer — RBAC + ABAC, fail
closed ([ADR-0100](0100-authorization-model.md), [ADR-0101](0101-permissions-scopes-and-grants.md)).
Não sabe **quanto** de capacidade institucional cada actor pode consumir. Hoje
não existe contabilidade de recursos nenhuma: o armazenamento pessoal cresce sem
limite servidor, não há orçamento de computação, e não há linguagem para uma
alocação, uma reserva ou um pedido.

Essa lacuna tem de ser fechada **antes** de existir o primeiro nó GPU, não
depois. O fornecedor de GPU real está por resolver, e é precisamente por isso
que a governança de recursos tem de existir primeiro: o sistema tem de saber
governar capacidade independentemente de qualquer cloud, fabricante, modelo ou
nó — para que ligar o primeiro L40S seja registar um recurso, não redesenhar o
sistema operativo à volta do fornecedor que calhar aparecer.

A ausência de inferência real é um **estado de produção suportado**:
`compute_nodes = 0`, `inference_providers = 0`, IA indisponível. Isto não pode
impedir perfis, quotas, alocações, pedidos, ledger de uso, admissão nem a
experiência de recursos de existirem.

## Decision

**Governança de recursos é um eixo separado da autoridade de acesso.** Uma
alocação de recurso não concede acesso a dados, unidades, projectos, ambientes
nem administração; e uma posição institucional não concede capacidade — um
Fundador não é automaticamente ilimitado, nem um administrador.

Quatro conceitos ficam **distintos** e nunca colapsam num contador mutável:

- **Capacidade** — o que a instituição física ou logicamente tem.
- **Entitlement** — o que um âmbito pode consumir (uma alocação).
- **Reserva** — capacidade comprometida a uma operação.
- **Uso** — o que foi de facto consumido.

Consequências directas desta separação:

1. **Vocabulário tipado.** Um recurso é um `ResourceType` com uma `ResourceUnit`
   explícita. `value = 10` sem unidade é um defeito. Um âmbito é um par
   `(ResourceScopeType, id)` — organização, membro, unidade, ambiente, projecto
   — para que um âmbito novo não exija reescrever o esquema.

2. **Entitlement por perfil + overrides.** Todo o membro activo recebe um perfil
   de alocação configurável. O entitlement efectivo deriva do perfil, de um
   override explícito, e de concessões temporárias que somam por cima e expiram.
   Os valores são **configuração institucional**, editáveis por quem administra,
   nunca congelados em lógica de negócio. O storage por omissão nasce em 10 GiB
   — conservador para uma instituição sem armazenamento físico declarado — e é
   editável; os valores de GPU ficam configuráveis até a calibração com hardware
   real (M5).

3. **Ledger de uso canónico e append-only.** O uso é registado como facto
   consumado, com um âmbito de cobrança canónico, e a base de dados recusa
   alterá-lo ou apagá-lo — como a auditoria. Contadores materializados podem
   existir por desempenho, mas reconciliam contra o ledger. O storage
   instantâneo mede-se directamente dos `storage_objects`, não do ledger.

4. **Admissão fail-closed, no chokepoint que já existe.** A admissão de recursos
   é um portão no caminho central de execução agentic (`executor::execute`) e no
   ponto de admissão de bytes do storage. Um agente opera **dentro** do envelope
   de execução e não o alarga; um fornecedor de inferência não se auto-autoriza
   capacidade; um nó de computação **executa** política e não a **autora**.

5. **Capacidade zero é um estado válido.** Com zero nós, uma reserva de
   computação termina com `CAPACITY_UNAVAILABLE` — nunca sucesso, nunca um nó
   falso. Entitlement e disponibilidade são factos diferentes: um membro pode
   estar autorizado a `10h/mês` de GPU enquanto a instituição não tem GPU.
   Ligar um nó mais tarde **não** exige migrar utilizadores nem perfis.

6. **Governança de recursos não é IA disponível.** `OCINYE_RESOURCE_GOVERNANCE_READY`
   é um portão distinto de `OCINYE_AI_READY`. Pode declarar-se com `GPU = 0`; não
   torna a IA disponível. Nenhum recurso de produção é fingido para completar a
   experiência.

## Alternatives

- **Modelar utilizadores normais como donos de fatias fixas de VRAM** (Fidel =
  12 GB). Rejeitado: a inferência partilhada trata os pesos do modelo como
  infra-estrutura partilhada; o consumo governa-se por política lógica
  (concorrência, orçamento de tempo, contexto, classes de modelo), não por posse
  de VRAM.
- **Fazer da facturação do fornecedor cloud o modelo canónico.** Rejeitado:
  quota, capacidade, uso e entitlement institucionais não podem depender da
  implementação de facturação de um fornecedor. O custo pode ser ingerido mais
  tarde, à parte.
- **Esperar pelo GPU real.** Rejeitado explicitamente: a incerteza sobre o
  hardware é a razão para governar recursos já.
- **Reutilizar RBAC para capacidade** (um Fundador é ilimitado). Rejeitado: é a
  confusão que este ADR existe para evitar — acesso e capacidade são sistemas
  diferentes.

## Consequences

- Novas permissões de governança (`resources.view`, `resources.allocate`,
  `resources.profiles.manage`, `resources.requests.review`), concedidas a quem
  administra e a mais ninguém. As de computação (`compute.*`) já existiam.
- Um domínio novo do Core (`modules/resource`) com o esquema em
  `migrations/0040_resource_governance.sql`: perfis, regras, alocações e o ledger
  de uso. As tabelas de capacidade, reserva e pedido chegam nas suas fatias.
- A entrega é em fatias revíveis: domínio+esquema, perfis+ciclo de vida, storage
  com enforcement, ledger, «Meus Recursos», Administração, pedidos, capacidade,
  reserva+admissão, scheduler, control-plane de computação, entitlements de IA,
  segurança, e certificação E2E.
- O estado de produção após o marco, e antes do primeiro GPU, é sucesso:
  governança pronta e imposta, IA indisponível.
