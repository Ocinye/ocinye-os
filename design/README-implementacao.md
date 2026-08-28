# Dossier de design — como é usado neste repositório

Esta pasta é o **handoff de design do Ocinye Workspace**, tal como foi entregue.
É a fonte de verdade visual.

## Ficheiros

| Ficheiro | O que é |
|---|---|
| [`README.md`](README.md) | Especificação dos 20 ecrãs, componentes, estados e comportamentos |
| [`DESIGN_TOKENS.md`](DESIGN_TOKENS.md) | Cores, tipografia, espaçamento, radius, sombras, animações |
| [`IMPLEMENTATION_PROMPT.md`](IMPLEMENTATION_PROMPT.md) | O prompt de implementação original |
| [`icons/`](icons/ICONS.md) | Os 37 ícones e o mapa de utilização |
| [`assets/ocinye_logo.png`](assets/ocinye_logo.png) | Logótipo oficial |
| [`prototype/`](prototype/) | Protótipo navegável — **referência, não código de produção** |

## Onde está implementado

[`apps/workspace`](../apps/workspace/README.md), em Leptos SSR.

O dossier sugere React + TypeScript + Vite *quando o projecto não tem frontend*.
Este tem, e é **Rust-first por princípio institucional**
([ADR-0004](../docs/adrs/0004-rust-first.md), `CLAUDE.md` §16-A). A decisão de
implementação está registada em
[ADR-0602](../docs/adrs/0602-workspace-ssr-progressive-enhancement.md).

## O dossier é verificado, não copiado

[`apps/workspace/tests/design_fidelity.rs`](../apps/workspace/tests/design_fidelity.rs)
lê estes ficheiros directamente e compara com o que está implementado:

- **todos** os tokens de `DESIGN_TOKENS.md` existem no CSS com o mesmo valor;
- os 37 ícones de `ICONS.md` existem no sprite, e o sprite não tem símbolos por
  declarar;
- o focus ring, as dimensões da shell, as duas animações e a tipografia são as
  do dossier.

Uma alteração ao design que não chegue ao código falha nesses testes, em vez de
divergir em silêncio.

## Desvios deliberados

Estão declarados, não escondidos:

| Desvio | Porquê |
|---|---|
| `/ideas/{id}` e `/projects/{id}` encaminham para `/workspaces/{id}` | Promover uma ideia mantém o *mesmo* Research Workspace; um URL canónico é mais verdadeiro do que dois para o mesmo objecto |
| Acções sem ecrã ficam visíveis e declaradas indisponíveis | O dossier especifica-as, mas não especifica os seus ecrãs; ligá-las produziria 404s |
| Leptos SSR em vez de React | Rust-first ([ADR-0004](../docs/adrs/0004-rust-first.md)) |

## Actualizar o design

1. Substituir os ficheiros nesta pasta.
2. Correr `cargo test -p ocinye-workspace`.
3. Os testes de fidelidade dizem exactamente o que divergiu.
