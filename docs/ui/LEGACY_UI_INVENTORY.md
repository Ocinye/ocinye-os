# Inventário da UI legada (UI Reset, fatia 0)

> O que existe **antes** de se retirar a camada visual legada do Workspace, e o
> que prende o comportamento e os testes a essa camada. Os números saem de
> `scripts/ui-legacy-inventory.sh`, que só lê; não se escrevem à mão.

## Ponto de recuperação

| | |
|---|---|
| `main` no início do reset | `4f8d048` |
| Produção | `4f8d0489f54e` — continua na UI legada até à migração final; **este reset não faz deploy** |
| Ramo do reset | `chore/ui-reset-foundation` |

Voltar à UI legada é voltar a `4f8d048`. Não se cria tag: a §73 exige instrução
explícita para isso, e o SHA na `main` já é recuperável.

## Medição a 2026-09-27

| | |
|---|---|
| ficheiros Rust de UI | 45 |
| linhas Rust de UI | 35 510 |
| linhas de CSS legado (`static/ocinye.css`) | 7 191 |
| linhas de `static/app.js` | 5 031 |
| símbolos no sprite de ícones | 50 |
| atributos `class=` na UI Rust | 2 164 |
| classes `oc-` distintas na UI Rust | 825 |
| marcadores `data-oc` na UI Rust | 353 |
| selectores `.oc-` no `app.js` | 38 |
| classes `oc-`/`is-` alternadas no `app.js` | 59 |
| selectores `.oc-` nos testes de browser | 421 |

## O que é o quê

| Camada | Onde | Classificação |
|---|---|---|
| Folha de estilo, tokens, tipografia, raios, sombras | `static/ocinye.css` | `VISUAL_ONLY` — sai |
| Sprite de ícones e o seu mapeamento | `static/icons.svg`, `ui/icon.rs` | `VISUAL_ONLY` — sai |
| Primitivas visuais (cartão, distintivo, botão, tabela, separadores…) | `ui/components/` | `VISUAL_ONLY` na forma; os contratos de acessibilidade dos separadores (`aria-current`/`aria-selected`) ficam |
| Casca: barra lateral, barra de topo, lançador, paleta | `ui/shell.rs` | `MIXED_UI_AND_LOGIC` — a filtragem por permissões, a fixação e o registo de aplicações ficam; a apresentação sai |
| Ecrãs | `ui/screens/` | `MIXED_UI_AND_LOGIC` — formulários, rotas de POST, estados de erro/vazio e marcadores `data-oc` ficam; a composição visual sai |
| Comportamento no browser | `static/app.js` | `MIXED_UI_AND_LOGIC` — upload segmentado, estado por guardar, arrastar, autosave, diálogos ficam; os 38 selectores e as 59 classes visuais passam a `data-*` antes de o CSS sair |
| Editor de notas | `editor/`, `static/notes-editor.js` | `HEADLESS` — ProseMirror, fica |
| Registo de aplicações, papéis, tempo, markdown | `ui/apps.rs`, `ui/roles.rs`, `ui/tempo.rs`, `ui/markdown.rs` | `HEADLESS` — fica |
| i18n | `src/i18n/` | fica, com o portão de chaves em falta |
| Rotas, sessão BFF, cliente do Core | `routes.rs`, `session.rs`, `api.rs` | fica |

## O que prende os testes à apresentação

As viagens de browser seleccionam 421 classes `.oc-`. Antes de a folha de
estilo sair, cada uma passa a um marcador de comportamento (`data-oc`,
`data-state`) — ou fica registada na matriz de contratos como à espera da UI
nova. Nenhuma se perde em silêncio.
