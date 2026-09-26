# ADR-0017 — Configuração e marca da Instância

- **Estado:** Accepted
- **Domínio:** Foundation
- **Impacto:** MEDIUM
- **Depende de:** [ADR-0013](0013-general-purpose-os-instance-and-node.md) · [ADR-0014](0014-instance-profiles-and-application-activation.md)
- **Data:** 2026-09-26

## Context

Duas organizações usam o mesmo release. O nome, o perfil e as aplicações activas
já eram da Instância ([ADR-0013](0013-general-purpose-os-instance-and-node.md),
[ADR-0014](0014-instance-profiles-and-application-activation.md)). Faltava o
resto do que as distingue sem fork: a língua de quem ainda não escolheu, o fuso
horário, as aplicações fixadas por omissão, e a marca que se vê na porta.

## Decision

**1. `instance_settings`** (migração 0058), uma linha por Instância: língua por
omissão (`pt`, `en`, `fr`), fuso IANA, aplicações fixadas por omissão e o
logótipo. Sem linha, os valores do produto. Gere-se em
`GET/PUT /api/v1/instance/settings` e `PUT /api/v1/instance/logo`, pela
administração da plataforma, com auditoria. Cada valor valida-se contra o que o
produto conhece: os três locales, a base IANA, os manifestos de aplicação.

**2. As fixações por omissão entram onde o membro ainda não escolheu.**
`GET /me/apps/pins` devolve a escolha do membro (`source: member`), senão a da
Instância (`instance`), senão `null` (`product`). A escolha de um membro, mesmo
vazia, nunca é sobreposta.

**3. O logótipo é um objecto governado.** PNG, JPEG ou WebP **pelos bytes** — um
SVG é um documento com script, e fica de fora —, até 512 KiB, guardado no
armazenamento da Instância e servido pelo Core com `nosniff`. Nunca um URL
remoto nem CSS de terceiros.

**4. A marca pública é mínima.** `GET /instance/branding` e `GET /instance/logo`
são as únicas rotas públicas que dizem algo da Instância: o nome, a língua, o
logótipo, e o produto — `Ocinye OS` continua identificável como a plataforma
enquanto não houver uma decisão explícita de white-label. A varredura de rotas
sem sessão lista-as, por método, com a razão.

## Alternatives

- **Configuração por variáveis de ambiente.** Exige reiniciar para mudar uma
  língua, e não é auditada. Rejeitada.
- **Logótipo por URL.** Um pedido a um anfitrião arbitrário a cada página, e um
  vector de rastreio. Rejeitada.

## Consequences

- O mesmo binário serve duas instituições com nomes, perfis, aplicações, línguas,
  fusos, fixações e marcas diferentes — e a mesma segurança, provada em
  `services/core-server/tests/instance_branding_http.rs` com duas bases novas.
- A apresentação final da marca no Workspace aguarda o Claude Design; o contrato
  está pronto.
