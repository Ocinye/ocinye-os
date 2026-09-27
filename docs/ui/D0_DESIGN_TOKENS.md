# D0 · Tokens e fundação — especificação

> Fonte visual: os protótipos aprovados `Ocinye OS.dc.html` e `Ocinye OS Apps.dc.html`
> (projecto Claude Design «Ocinye OS UI mockups», 2026-09-27). Ficheiro:
> `apps/workspace/static/ocinye-ds.css`.

## Como entra

1. Copiar `handoff/static/ocinye-ds.css` para `apps/workspace/static/`.
2. Ligar no `<head>` com `<link rel="stylesheet" href="/static/ocinye-ds.css">` — é same-origin, cabe na CSP.
3. Pôr `class="ods-root"` e `data-theme="light|dark|system"` no `<html>`. O tema vem da preferência do membro; `system` segue o sistema.
4. Enquanto o CSS legado existir, os dois convivem: `ods-` e `oc-` não se tocam.

## Regras

- Uma vista nunca escreve uma cor, um raio ou uma sombra: usa um token.
- Claro e escuro são **os mesmos nomes**; só `:root[data-theme="dark"]` muda valores.
- Texto de corpo em dourado usa `--ods-gold-text` (#8A6110), nunca `--ods-gold-500`: o dourado puro não tem contraste para texto pequeno.
- Estado nunca só por cor: cada ponto de estado vem com texto («OPERACIONAL», «SEM NÓ ACTIVO»).
- Movimento é curto (120–240 ms) e desliga-se todo com `prefers-reduced-motion`.

## Tokens por papel

| Papel | Token | Valor claro | Uso no desenho |
|---|---|---|---|
| Fundo do Desktop | `--ods-surface-desktop` | gradiente navy | Desktop, ecrã de bloqueio |
| Widget | `--ods-surface-widget` + `--ods-shadow-widget` + `--ods-radius-xl` | branco 97 %, 18px | todos os widgets e os 4 indicadores |
| Popover de vidro | `.ods-glass` | branco 72 %, blur 28px, borda branca 60 % | menu do utilizador, relógio, perfil, estado, Criar, menu de contexto |
| Vidro escuro | `.ods-glass--dark` | navy 55 %, blur 26px | barra vertical de apps |
| Tooltip | `--ods-surface-tooltip` | navy 86 % | nomes na barra de apps |
| Acção principal | `--ods-action-primary` | dourado | «+ Criar», enviar, botões de confirmação |
| Distintivo dourado | `.ods-gold-badge` | gradiente dourado metálico | ícones de perfil Re/Bu/Pe/Ed |
| Seleccionado | `--ods-surface-selected` | #E8F0F9 | linha seleccionada no Monitor |
| Foco | `--ods-focus-ring` | anel branco + dourado | todos os controlos |

## Medidas fixas do desenho

| Elemento | Medida |
|---|---|
| Barra de topo | 44px |
| Logótipo e ícone de perfil | 26 × 26, raio 7 |
| Grelha do Desktop | linhas de 16,333px, gap 14px → 1 unidade = 168px, 2 = 350px, minimizado = 2 linhas ≈ 47px |
| Colunas do Desktop | 1 (<768) · 2 (<1100) · 3 (<1500) · 4 |
| Lançador de aplicações | 860 × 640, centrado, altura fixa |
| Botão das apps / Nye | núcleo 52px e 64px; área 96px; mesma linha, 16px das margens |
| Barra vertical de apps | botões 48px, ícones 20px, 84px acima do fundo |
| Popup circular do Nye | 480px (encolhe até 100vw − 32px) |
| Repouso dos flutuantes | 5 s |

## Ícones (sprite novo, `static/ods-icons.svg`)

Traço 1,4–1,6, `currentColor`, cantos redondos. Símbolos exigidos pelo desenho:
`home grid apps-brand nye mic search close check lock user settings help logout
calendar mail notes files tasks project idea units data storage activity gauge
agent audit eye arrow-r bell trash`. O `nye` oficial (cabeça hexagonal com antena
interior, orelhas hexagonais, viseira com dois olhos dourados) e o `apps-brand`
(cinco hexágonos, o do centro dourado) estão no protótipo e copiam-se tal como estão.
