# ADR-0021 — O instalador consome contratos tipados; as invariantes ficam no Core

- **Estado:** Proposed
- **Domínio:** Foundation
- **Impacto:** MEDIUM
- **Depende de:** [ADR-0019](0019-multi-distribution-instance.md) · [ADR-0020](0020-access-endpoints.md) · [ADR-0701](0701-release-bundle-and-host-installer.md)
- **Data:** 2026-09-30

## Decision

1. As invariantes de produto (≥ 1 Distribuição activada, um canónico activo, pontos fixos só em
   Distribuições activadas, acesso do primeiro administrador) vivem no **Core**.
2. O instalador remoto (D011) só chama operações tipadas e auditadas (`ocinye …`, API do Core) listadas em
   `docs/architecture/d011-installer-boundary.md`.
3. O instalador **não** escreve na base de dados, não edita tabelas de Distribuições, acessos, pontos de
   acesso nem estado de membros.
4. A arquitectura completa da D011 fica para a D011.
