# ADR-0600 — Leptos para o Workspace Runtime

- **Estado:** Accepted
- **Domínio:** Workspace
- **Impacto:** MEDIUM
- **Refinado por:** [ADR-0602](0602-workspace-ssr-progressive-enhancement.md)
- **Data:** 2026-08-22

## Context

O Ocinye Workspace é a principal interface humana do Ocinye OS. Sob Rust-first
(ADR-0004), a preferência é um framework Rust com WebAssembly no browser quando
apropriado.

Há uma exigência de segurança que condiciona a arquitectura mais do que a
escolha do framework: o browser **nunca é autoridade** (briefing §17). O token
de acesso não deve viver no browser, e nenhuma decisão de autorização pode ser
tomada no cliente.

## Decision

**Leptos** como camada de vista do Workspace, servido por um servidor Axum
próprio (`apps/workspace`), distinto do Core.

O Workspace é um **Backend-for-Frontend**:

1. Executa o fluxo OIDC Authorization Code + PKCE contra o IdP.
2. Guarda os tokens **do lado do servidor**, numa sessão referenciada por um
   cookie `HttpOnly`, `Secure`, `SameSite=Lax`. O browser nunca vê o token.
3. Chama o Core com o bearer, propagando o `X-Correlation-ID`.

Nesta fase, o Leptos é usado em **SSR puro** (`render_to_string`), sem
hidratação nem WASM no browser. Estado: `CURRENT` = SSR; hidratação e WASM no
cliente = `PLANNED`.

Razão da faseagem: SSR entrega já uma interface acessível, com HTML semântico e
navegação por teclado, sem introduzir uma cadeia de build WASM antes de existir
interactividade que a justifique.

> **Actualização.** O dossier de design trouxe interactividade concreta
> (command palette, sidebar colapsável, menu de criação). A condição desta
> faseagem foi reavaliada em [ADR-0602](0602-workspace-ssr-progressive-enhancement.md),
> que mantém SSR e acrescenta uma camada delimitada de progressive enhancement,
> com a hidratação a continuar como destino declarado. Isto é a aplicação directa do princípio "usar
WASM quando resolve um problema, não porque podemos" (briefing §137). O WASM
entra já onde ganha o seu lugar: no Capability Runtime (ADR-0501).

## Alternatives

| Alternativa | Porque foi rejeitada |
|---|---|
| **Leptos com SSR + hydration desde já** | É o destino. Exige `cargo-leptos` e uma cadeia de build WASM na fundação, antes de haver interactividade que o justifique. Adiado, não abandonado. |
| **Dioxus** | Forte, sobretudo multiplataforma. Menos alinhado com um Workspace primariamente web e com SSR como ponto de partida. |
| **Yew** | Maduro mas essencialmente CSR; um Workspace só-CSR atrasa o primeiro render e complica a sessão server-side. |
| **Sycamore** | Comunidade menor para uma base institucional de longa duração. |
| **Next.js / React** | Ecossistema mais rico, mas quebraria o princípio Rust-first e obrigaria a duplicar em TypeScript os tipos canónicos que `ocinye-contracts` já define. |
| **Tokens no browser (SPA a chamar o Core)** | Rejeitado por segurança: exporia tokens a XSS e tornaria o browser detentor de credenciais institucionais. |

## Consequences

**Positivas** — tokens nunca no browser; tipos partilhados via
`ocinye-contracts` sem duplicação; o Workspace é uma trust boundary explícita;
HTML semântico e acessibilidade por omissão.

**Negativas, aceites** — interactividade rica exige a passagem futura a
hidratação; cada interacção implica um round-trip ao servidor enquanto isso não
acontecer. Declarado como `PLANNED`, não como implementado.

## Referências

`CLAUDE.md` §16-A, §44 · briefing §17, §18, §137 · ADR-0004 · ADR-0102 · ADR-0501
