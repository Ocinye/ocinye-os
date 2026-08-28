# Ocinye Workspace — Design Tokens

## CSS custom properties

```css
:root {
  /* Marca */
  --oc-navy:            #0B2D4A;  /* Deep Navy — estrutura, sidebar, acções institucionais */
  --oc-navy-deep:       #071E33;  /* fundo do login */
  --oc-navy-hover:      #123C60;  /* hover de botão navy */
  --oc-navy-mid:        #1C4B74;  /* avatares, gradiente do login */
  --oc-gold:            #E0A731;  /* Sunrise Gold — acento, CTA, selected, envio */

  /* Superfícies */
  --oc-canvas:          #F6F8FA;  /* fundo da aplicação */
  --oc-surface:         #FFFFFF;  /* cartões, topbar, tabelas */
  --oc-surface-subtle:  #FAFCFD;  /* header de coluna, painéis informativos */
  --oc-surface-muted:   #F3F6F9;  /* campos, pills neutras */
  --oc-surface-tint:    #F1F4F8;  /* pills de tipo, hover em navy claro */
  --oc-surface-hover:   #F8FAFC;  /* hover de linha de tabela */

  /* Linhas */
  --oc-border:          #E4E9F0;  /* borda padrão */
  --oc-border-soft:     #EEF2F6;  /* divisores internos de cartão */
  --oc-border-faint:    #F3F6F9;  /* divisores de linha */
  --oc-border-strong:   #C3CDD8;  /* hover de borda, checkbox */

  /* Texto */
  --oc-text:            #0F1A24;  /* títulos e primeira célula */
  --oc-text-body:       #28394A;  /* corpo de actividade */
  --oc-text-secondary:  #42546A;  /* parágrafos, labels de campo */
  --oc-text-muted:      #5F7183;  /* subtítulos, células secundárias */
  --oc-text-meta:       #7C8B9A;  /* labels mono de coluna */
  --oc-text-faint:      #8A98A6;  /* placeholders, contagens */
  --oc-text-ghost:      #A9B5C1;  /* metadados mono, indisponível */
  --oc-placeholder:     #98A6B4;

  /* Funcionais */
  --oc-success:         #3E8F66;  --oc-success-text: #2E6B4C;
  --oc-success-bg:      #F0F7F3;  --oc-success-border: #D8EBE0;
  --oc-info:            #2B6CB0;  --oc-info-text: #20537F;
  --oc-info-bg:         #EFF5FB;  --oc-info-border: #D5E4F0;
  --oc-warning:         #C87A22;  --oc-warning-text: #8A4B10;
  --oc-warning-bg:      #FDF2E7;  --oc-warning-border: #F2DBBE;
  --oc-error:           #B3261E;  --oc-error-text: #8C2019;
  --oc-error-bg:        #FCF0EF;  --oc-error-border: #F1D6D3;
  --oc-gold-text:       #8A6110;  --oc-gold-bg: #FDF6E7; --oc-gold-border: #F2E3BE;

  /* Sobre navy (login e sidebar) */
  --oc-on-navy:         #FFFFFF;
  --oc-on-navy-70:      rgba(255,255,255,.68);
  --oc-on-navy-50:      rgba(255,255,255,.5);
  --oc-on-navy-42:      rgba(255,255,255,.42);
  --oc-on-navy-32:      rgba(255,255,255,.32);
  --oc-on-navy-hover:   rgba(255,255,255,.07);
  --oc-on-navy-active:  rgba(255,255,255,.10);
  --oc-on-navy-line:    rgba(255,255,255,.08);

  /* Tipografia */
  --oc-font-sans: 'IBM Plex Sans', system-ui, sans-serif;
  --oc-font-mono: 'IBM Plex Mono', ui-monospace, monospace;

  /* Radius */
  --oc-r-xs: 4px;   /* badges, pills mono */
  --oc-r-sm: 7px;   /* pills de tab, botões pequenos */
  --oc-r-md: 8px;   /* botões, campos, itens de nav */
  --oc-r-lg: 11px;  /* cartões */
  --oc-r-xl: 14px;  /* prompt input, command palette */
  --oc-r-2xl: 16px; /* cartão de login */
  --oc-r-tile: 20px;/* tile de logótipo e empty state grande */

  /* Sombras */
  --oc-shadow-card:    0 2px 10px rgba(11,45,74,.06);
  --oc-shadow-input:   0 4px 18px rgba(11,45,74,.06);
  --oc-shadow-menu:    0 16px 40px rgba(11,45,74,.14);
  --oc-shadow-overlay: 0 30px 80px rgba(7,30,51,.32);
  --oc-shadow-logo:    0 18px 50px rgba(0,0,0,.35), 0 0 0 1px rgba(255,255,255,.14);

  /* Painéis pousados — ver «Painéis pousados» */
  --oc-pop-surface: color-mix(in srgb, var(--oc-surface) 88%, transparent);
  --oc-pop-blur:    blur(18px) saturate(140%);
  --oc-pop-line:    color-mix(in srgb, var(--oc-border) 70%, transparent);
  --oc-pop-radius:  13px;
  --oc-pop-hover:   color-mix(in srgb, var(--oc-navy) 6%, transparent);
  --oc-pop-pad:     6px;
  --oc-pop-shadow:
    0 16px 40px rgba(11,26,45,.20),
    0 2px 8px rgba(11,26,45,.10),
    inset 0 1px 0 rgba(255,255,255,.60);

  /* Escala de apresentação */
  --oc-interface-scale: 1.15;

  /* Layout */
  --oc-sidebar-w: 224px;
  --oc-sidebar-w-collapsed: 58px;
  --oc-topbar-h: 52px;
  --oc-row-h: 38px;
  --oc-row-h-dense: 30px;
  --oc-page-pad: 22px 24px 40px;
}
```

## Escala de apresentação

`--oc-interface-scale` é aplicada uma vez, na raiz, por `zoom`. **Não** é um
multiplicador a somar às medidas abaixo: todas as medidas deste dossier
continuam a ser as medidas do desenho, e é contra elas que
`design_fidelity.rs` compara o CSS.

A escala diz apenas a que tamanho a instituição apresenta esse desenho. Nenhuma
proporção muda com ela, e um componente que precise de crescer sozinho continua
a ser uma alteração ao protótipo — não um ajuste deste número.

| | |
| --- | --- |
| Valor actual | `1.15` |
| Referência (o protótipo) | `1.0` |
| Onde se muda | `:root` em `ocinye.css`, um sítio só |

Os pontos de quebra passam a ser avaliados contra o viewport dividido pela
escala, pelo que acontecem numa janela proporcionalmente maior. É o esperado: o
conteúdo é maior e precisa do espaço mais cedo.

## Escala de espaçamento

Múltiplos usados no protótipo: `2, 4, 5, 6, 7, 8, 9, 10, 11, 12, 14, 15, 16, 17, 18, 20, 22, 24, 26,
34, 40, 48, 56`px. Regras práticas:

| Uso | Valor |
| --- | --- |
| gap entre ícone e label de nav | 10px |
| gap entre cartões / colunas de grelha | 12px |
| padding interno de cartão | 14–17px |
| padding de cabeçalho de secção | 13px 15px |
| padding de célula de tabela | 0 16px |
| padding de página | 22px 24px 40px |
| gap entre grupos de sidebar | 12px acima do label |
| empty state (padding vertical) | 56px |

## Escala tipográfica

| Papel | Fonte | Tamanho / peso | Extra |
| --- | --- | --- | --- |
| Título de ecrã | Sans | 19px / 600 | `letter-spacing:-.01em` |
| Título de Home | Sans | 21px / 600 | `letter-spacing:-.01em` |
| Título de empty state | Sans | 15–15.5px / 600 | — |
| Título de prompt | Sans | 18px / 600 | — |
| Cabeçalho de secção | Sans | 13px / 600 | — |
| Valor de KPI | Sans | 25–27px / 600 | `line-height:1` |
| Corpo | Sans | 12.5px / 400 | `line-height:1.7` em parágrafos |
| Célula principal de tabela | Sans | 12.5px / 500 | — |
| Célula secundária | Sans | 12px / 400 | `#5F7183` |
| Item de nav | Sans | 12.5px / 500 | — |
| Botão | Sans | 12px / 500–600 | — |
| Label de campo | Sans | 11.5px / 500 | `#42546A` |
| Legenda | Sans | 11–11.5px / 400 | `#8A98A6` |
| Metadados / códigos / datas | Mono | 11–11.5px / 400 | `#7C8B9A` – `#A9B5C1` |
| Label de coluna | Mono | 10px / 500 | `letter-spacing:.1em` |
| Badge | Mono | 10px / 500 | `letter-spacing:.04em` |
| Label de grupo (sidebar) | Mono | 9.5px / 500 | `letter-spacing:.16em` |
| Wordmark `OCINYE OS` | Sans | 11.5–25px / 600 | `letter-spacing:.14em` |
| Subtítulo `OCINYE WORKSPACE` | Mono | 12px / 400 | `letter-spacing:.22em`, `#E0A731` |

Mínimo de corpo legível: 12px. Mono a 9.5–10px apenas para labels e badges em maiúsculas.

## Animações

```css
@keyframes ocFade  { from { opacity:0; transform:translateY(4px) } to { opacity:1; transform:none } }
@keyframes ocPulse { 0%,100% { opacity:.35 } 50% { opacity:1 } }
```
- Menus e overlays: `ocFade .12s–.14s ease`.
- Ponto de estado do Core: `ocPulse 2.6s infinite`.
- Sidebar: `transition: width .18s ease`.
- Hover de cartão/borda: `transition: border-color .15s, box-shadow .15s`.

## Camadas de sobreposição

Semânticas, e nunca numéricas. Um módulo não precisa de saber que `modal` vale
200; precisa de saber que um popover não lhe passa por cima.

| Token | Valor | Quem a usa |
| --- | --- | --- |
| `--oc-z-base` | 1 | contexto de empilhamento do ecrã de entrada |
| `--oc-z-sticky` | 20 | reservado |
| `--oc-z-dropdown` | 40 | menu de conta |
| `--oc-z-popover` | 60 | menu de criação |
| `--oc-z-skip` | 100 | ligação de salto, que tem de aparecer ao receber foco |
| `--oc-z-modal` | 200 | paleta de comandos |
| `--oc-z-toast` | 300 | reservado |
| `--oc-z-critical` | 400 | reservado — o ecrã de arranque terá aqui o seu lugar |

Três não têm consumidor. Ficam declaradas porque a hierarquia se decide uma vez,
e não se negoceia a cada superfície que aparece.

## Painéis pousados

Um painel que abre por cima da interface — o menu da conta, o sino das
notificações, o calendário da barra — não é um cartão. É uma **superfície
pousada**: lê-se como algo por cima do que está por baixo, e não como um
rectângulo colado.

| Token | Valor |
| --- | --- |
| `--oc-pop-surface` | `color-mix(in srgb, var(--oc-surface) 88%, transparent)` |
| `--oc-pop-blur` | `blur(18px) saturate(140%)` |
| `--oc-pop-line` | `color-mix(in srgb, var(--oc-border) 70%, transparent)` |
| `--oc-pop-radius` | 13px |
| `--oc-pop-shadow` | três camadas — ver abaixo |
| `--oc-pop-hover` | `color-mix(in srgb, var(--oc-navy) 6%, transparent)` |
| `--oc-pop-pad` | 6px |

A translucidez é ligeira **de propósito**. A 88% o painel deixa adivinhar o que
está por baixo sem que o texto dependa disso: onde o browser não souber
desfocar, o fundo continua quase opaco e não se perde nada.

A sombra tem três camadas, e cada uma faz uma coisa:

```
0 16px 40px rgba(11,26,45,.20)      o assentamento largo — o painel está longe do fundo
0 2px 8px rgba(11,26,45,.10)        o contacto curto — a aresta encosta a algo
inset 0 1px 0 rgba(255,255,255,.60) o fio de luz em cima — dá a espessura
```

### Como se escreve um painel novo

Não se escolhe nada. A classe traz a superfície inteira:

```html
<div class="oc-pop meu-painel" hidden>
  <div class="oc-pop__head">
    <span class="oc-pop__title">Título</span>
    <span class="oc-pop__meta">3 POR LER</span>
  </div>
  <a class="oc-pop__item" href="…">
    <svg …/>
    <span><b>O assunto</b><em>o que isto é</em></span>
  </a>
  <div class="oc-pop__foot">…</div>
</div>
```

A regra própria do painel declara **só o que o distingue** — onde abre e que
largura tem:

```css
.meu-painel { top: calc(100% + 8px); right: 0; width: 320px; }
```

O ritmo das linhas faz parte do acabamento tanto como a superfície: ícone à
esquerda, assunto a negrito, e por baixo uma linha a dizer o que aquilo é. Um
painel com a superfície certa e linhas de texto seco lê-se como menos acabado
do que os outros, e foi assim que o sino ficou durante um dia.

### O que está fechado

Os valores vivem nos tokens e em mais lado nenhum. Reescrevê-los numa regra
nova falha o portão `a_superficie_de_um_painel_nao_se_reescreve`, em
`apps/workspace/tests/design_fidelity.rs`.

O portão não procura a classe — procura o **valor repetido**. Um portão que
exigisse `.oc-pop` seria satisfeito por quem acrescentasse a classe e depois a
sobrepusesse; o que falha é a cópia, e é a cópia que se mede.

Isto não é zelo abstracto. Três painéis chegaram a este acabamento nesta
interface, e as três vezes por cópia. Das três, uma foi parar dentro de uma
`@media` e não se aplicava em ecrã normal, outra reescreveu a referência em vez
de a seguir, e a terceira ficou com a superfície certa e o conteúdo por acabar.
Nenhuma delas apareceu num portão; as três apareceram numa captura.

**O menu da conta é a referência e não participa.** Continua a escrever as
propriedades à mão — pelos tokens, com os mesmos valores — porque uma referência
que participa deixa de ser referência: passa a poder mudar com o grupo, e no dia
em que mudar não há contra o quê comparar.

## Foco

Um só tratamento, atravessando tudo.

| Token | Valor |
| --- | --- |
| `--oc-focus-color` | `var(--oc-gold)` |
| `--oc-focus-width` | 2px |
| `--oc-focus-offset` | 1px |
| `--oc-focus-offset-wrap` | 2px |

O `wrap` existe para quando o anel envolve um **contentor** cujo filho é que tem
o foco, e precisa de livrar a borda do contentor. São dois casos, e não dois
acidentes — que era o que estava antes de o token existir.

## Movimento

| Token | Valor |
| --- | --- |
| `--oc-duration-fast` | .12s |
| `--oc-duration-normal` | .15s |
| `--oc-duration-slow` | .18s |
| `--oc-ease-standard` · `--oc-ease-enter` · `--oc-ease-exit` | `ease` |

As três curvas resolvem hoje todas para `ease`, que é a única em uso. Existem
separadas para dar vocabulário a quem entra — para que o arranque, o Centro
Temporal e um módulo futuro não escolham curvas diferentes — e podem divergir
mais tarde por decisão.

## Política das categorias

| Categoria | Política |
| --- | --- |
| Cor | tokens semânticos |
| Espaçamento | escala canónica para o espaçamento recorrente |
| Tipografia | papéis semânticos |
| Movimento | durações e curvas |
| Camadas | papéis semânticos de sobreposição |
| Foco | tratamento canónico único |
| Geometria excepcional | literais permitidos, com razão local escrita |

A última linha é deliberada, e é o que impede este dossier de se tornar uma
religião de variáveis. Os valores ímpares medidos no protótipo — 5, 7, 9, 11,
13, 15px — **não precisam de virar tokens**: são geometria de um componente
concreto, não a linguagem do sistema. O objectivo nunca foi zero números no CSS;
foi que um módulo novo tenha nomes para alcançar em vez de inventar os seus.

Três durações ficam literais pela mesma razão, cada uma com a sua:

- `ocPulse 2.6s` — o ritmo do ponto de estado do Core;
- `ocFade .14s` — dez milissegundos fora da escala; aproximá-los seria uma
  alteração visual, e a tokenização muda a fonte de verdade e não os pixels;
- `transform .1s` — composição de um elemento.

## Regras de uso da cor

1. Deep Navy é a cor de estrutura: sidebar, botões institucionais, valores de progresso, títulos.
2. Sunrise Gold é acento: CTA principal, estado selecionado, botão de envio, pontos de destaque,
   focus ring. Nunca preencher áreas grandes com dourado.
3. Fundos de superfície apenas em branco/off-white/cinza-azulado; no máximo dois fundos por ecrã.
4. Cores funcionais só em badges, pontos de estado e prazos — nunca como decoração.
5. Sem gradientes fortes, sem glassmorphism (excepto o cartão do login), sem sombras pesadas.
