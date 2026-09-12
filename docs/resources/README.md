# Governança de recursos

Como o Ocinye OS governa **quanto** de capacidade institucional cada âmbito pode
consumir. É um eixo separado da autoridade de acesso (RBAC): uma alocação de
recurso não concede acesso a dados, unidades, projectos, ambientes nem
administração, e uma posição institucional não concede capacidade. Decisão de
arquitectura: [ADR-0108](../adrs/0108-resource-governance-and-compute-control-plane.md).

## Os quatro conceitos

Nunca um contador mutável:

| Conceito | O que é | Onde vive |
|---|---|---|
| **Capacidade** | o que a instituição física ou logicamente tem | (fatia futura) |
| **Entitlement** | o que um âmbito pode consumir | `resource_allocations` + perfis |
| **Reserva** | capacidade comprometida a uma operação | (fatia futura) |
| **Uso** | o que foi de facto consumido | `resource_usage_events` (append-only) |

## Vocabulário tipado

Um recurso é um `ResourceType` com uma `ResourceUnit` explícita — `value = 10`
sem unidade é um defeito. Um âmbito é um par `(ResourceScopeType, id)`:
organização, membro, unidade, ambiente de investigação, projecto. Um âmbito novo
não exige reescrever o esquema.

## Entitlement efectivo

Todo o membro activo resolve um entitlement a partir do seu **perfil de
alocação** (por omissão `MEMBER_STANDARD`), mais **overrides** explícitos e
**concessões temporárias** que somam por cima e expiram. O override substitui a
base do perfil; cada contribuição é registada, para o número poder ser explicado
(«20 GiB = perfil 10 + temporária 10, expira 2026-10-15»).

Os valores são **configuração institucional**, editáveis por quem administra,
nunca congelados em lógica de negócio. O storage por omissão nasce em **10 GiB**
(`OCINYE_STORAGE_QUOTA_BYTES`, quando a fatia de storage o ligar) — conservador
para uma instituição sem armazenamento físico declarado.

## Estado de implementação

Entregue em fatias revíveis.

- **Feito (fatias A–C):** vocabulário tipado
  (`crates/ocinye-contracts/src/resource.rs`), esquema
  (`migrations/0040_resource_governance.sql`: `resource_profiles`,
  `resource_profile_rules`, `resource_allocations`, `resource_usage_events`),
  resolução de entitlement, permissões de governança; perfis atribuídos por
  membro (`people.resource_profile_id`, migração 0041) com perfil por omissão no
  bootstrap e atribuição auditada; e **armazenamento pessoal medido e imposto no
  servidor** — uso por `SUM(size_bytes)` sobre `owner_id`, admissão fail-closed
  com tranca por membro (segura à concorrência), redução de quota segura (nunca
  apaga), estados `Normal`/`Warning`/`Critical`/`OverQuota`
  (`crates/ocinye-core/src/modules/resource/storage.rs`).
- **Feito (fatia E):** «Meus Recursos» — o ecrã do membro
  (`apps/workspace/src/ui/screens/resources.rs`, servido em `/resources`) mostra
  uso, limite, disponível, o estado derivado e a explicação de como o limite se
  compõe (perfil mais concessões temporárias), a partir de
  `GET /api/v1/resources/me` (o Core resolve o dono pela sessão e autoriza).
  Torna visível a imposição da fatia C: uma parede que o membro passa a ver
  antes de bater nela.
- **A seguir:** ledger de uso, Administração → Recursos (ver e alocar a outros
  membros), pedidos, registo de capacidade, reserva + admissão de computação,
  fronteira do scheduler, control-plane de computação, entitlements de IA,
  endurecimento de segurança, e certificação E2E → `OCINYE_RESOURCE_GOVERNANCE_READY`.

## O que a fundação não faz

- **Não impõe** quotas ainda — nenhum caminho de bytes ou de computação consulta
  o entitlement nesta fatia.
- **Não torna a IA disponível.** `OCINYE_RESOURCE_GOVERNANCE_READY` é um portão
  distinto de `OCINYE_AI_READY`. Capacidade zero é um estado válido: um membro
  pode estar autorizado a GPU enquanto a instituição não tem nó nenhum.
- **Não finge capacidade de produção.** Um simulador de capacidade existe só nos
  testes; produção começa em zero real.
