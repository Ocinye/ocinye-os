# ADR-0617 — PWA: identidade instalável, sem cache de estado autenticado

- **Estado:** Proposed
- **Domínio:** Workspace
- **Impacto:** MEDIUM
- **Depende de:** [ADR-0018](0018-universal-web-access-and-runtime-classes.md) · [ADR-0601](0601-workspace-bff-session.md)
- **Data:** 2026-09-27

## Context

Instalar a Web como aplicação (PWA) dá janela própria e ícone, sem instalador.
Um *service worker* agressivo, porém, serve páginas antigas com permissões
antigas, prende clientes em versões incompatíveis e guarda conteúdo sensível no
disco.

## Decision

- **Manifesto** servido pelo Workspace: `name` da Instância, `start_url: /`,
  `scope: /`, `display: standalone`, `theme_color` do D15, ícones do sprite da
  marca.
- **Nenhum service worker, nesta fundação (R2).** Os navegadores actuais
  instalam a Web sem ele, e sem ele não há cache nenhuma que possa prender um
  cliente. Se um dia for preciso (ecrã «sem ligação»), vale a regra seguinte.
- **Se existir, service worker mínimo:** se existir, só serve os estáticos
  versionados (`/static/*` do release corrente) e um ecrã «sem ligação»; **nunca**
  guarda HTML autenticado, respostas do BFF ou da API, ficheiros, pré-visualizações
  nem nada do Core. Actualiza-se a cada release (`skipWaiting` controlado) e
  apaga as caches de releases anteriores; nunca prende um cliente numa versão
  incompatível.
- A PWA é conveniência: o mesmo modelo de dados, a mesma sessão, o mesmo Core.
- A instalação nunca é pedida à força: uma sugestão, uma vez, dispensável (D15).

## Alternatives

- **Offline completo com cache do Workspace.** Recusado: estado autorizado
  antigo em disco, e mentiras sobre o que está guardado.

## Consequences

- Critério de instalabilidade provado por E2E onde a automação o permite;
  a acessibilidade Web nunca depende de instalar.
