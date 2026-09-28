# ADR-0612 — O Browser Manager e o recurso honesto da Web

- **Estado:** Proposed
- **Domínio:** Workspace
- **Impacto:** HIGH
- **Depende de:** [ADR-0611](0611-runtime-capability-boundary.md) · [ADR-0703](0703-desktop-trust-boundary-and-native-bridge.md) · [ADR-0016](0016-application-manifest-contract.md)
- **Data:** 2026-09-27

## Context

O Browser é uma aplicação do Ocinye (Lançador, Gestor de Janelas, Nye, ocsh).
Precisa de um dono para abas, navegação, histórico, permissões de sites e
transferências — e esse dono não pode ser o Gestor de Janelas, que governa
janelas do Ocinye, nem o Core, que não navega a Internet por ninguém.

## Decision

### 1. Browser Manager, no cliente

Um módulo de cliente, único dono da navegação externa, com a API
`open(url)`, `window.new({private})`, `tab.new/close/focus`, `navigate`,
`reload`, `back`, `forward`, `tabs.list`, `current.read_metadata`, e — no
Desktop — `current.read_text` (extracção controlada, ADR-0616). UI, ocsh e Nye
usam **esta** API (evento `ocinye-browser`, D15 G-18); nenhum manipula um
webview ou um `iframe` directamente.

### 2. Janela ≠ aba

```text
Gestor de Janelas ── janela «Browser» (uma janela Ocinye)
                      └ Browser Manager ── BrowserWindow { id, window_id, partition, tabs[], active }
                                            └ BrowserTab { id, url, title, loading, can_back, can_forward, origin, security }
```

Uma aba nunca é uma janela do Ocinye; `window list` mostra «Browser · 3 abas».

### 3. Por runtime

| | Web | Desktop / Dedicated |
|---|---|---|
| Superfície | `iframe` `sandbox="allow-scripts allow-forms allow-popups allow-popups-to-escape-sandbox"` sem `allow-same-origin` nem `allow-top-navigation` | webview externo na partição do Browser (ADR-0703) |
| Site recusa incorporar | **recurso honesto**: «Abrir num novo separador do navegador» (`noopener,noreferrer`) | abre dentro |
| Histórico, título | só o que a aba sabe (URL pedida); título do site indisponível por origem cruzada | do webview |
| Texto da página para o Nye | **não** (a caixa de areia do navegador impede); só o que a pessoa colar | extracção controlada |

A aba do Ocinye nunca navega para fora. O Workspace ganha
`frame-src https:` na CSP **só** na página do Browser; `script-src` e o resto
ficam iguais.

### 4. Nunca um proxy

O Core **não** vai buscar páginas por ninguém para contornar `X-Frame-Options`
ou CORS. Um *probe* opcional (`GET /browser/probe?url=`, D15 G-18) que leia só
cabeçalhos de incorporação fica fora desta fase: mesmo sem corpo, é o Core a
fazer pedidos para URLs que um membro escolhe (SSRF). Até haver decisão
própria, a Web detecta por tempo limite e diz a verdade.

### 5. Registo

`ApplicationId::Browser`, categoria `system`, rota `/browser`, no manifesto
(ADR-0016), activo em todos os perfis. O manifesto ganha a declaração de
capacidade de runtime preferida (`integrated_webview`), que **adapta** a
aplicação na Web — nunca a esconde.

## Alternatives

- **Aba = janela do Gestor de Janelas.** Recusado: dez abas seriam dez janelas.
- **Browser Manager no Core.** Recusado: o Core não guarda estado de navegação
  de ninguém, e não navega.
- **Proxy de páginas pelo Core.** Recusado: privacidade, cookies, CSP,
  credenciais, direitos e desempenho (secção 28 do programa).

## Consequences

- Na Web, «resumir esta página» não é prometido; a matriz diz `limitado`.
- O comportamento do Browser na Web é testado com uma página de teste que
  recusa incorporação (viagem obrigatória).
