# Pedido D13 ao Claude Design

> Depois do D12, todos os ecrãs estão no Claude Design **menos o Correio** (caixa
> de entrada e compositor), que espera por duas respostas. O resto são decisões
> que o Claude Code tomou por não haver desenho, e que ficam à espera de
> confirmação: estão em `docs/ui/CLAUDE_DESIGN_QUESTIONS.md` (Q-19 a Q-35), cada
> uma com a proposta aplicada.

Ramo: `ui/claude-design`. Formato: como o D12 (`docs/ui/D13_*.md`, CSS em
`static/ods-d13-*.css`, textos em `i18n/`).

## 1. Bloqueia o Correio — Q-23 e Q-24

O `app.js` já implementa o comportamento; falta o desenho que o torna visível.
O contrato que o código cumpre hoje, e que o desenho tem de servir:

### Grelha de três colunas (Q-23)

| O quê | Como o `app.js` o diz |
|---|---|
| Largura das pastas e da lista | `--oc-mail-pastas` e `--oc-mail-lista` (px), no elemento `[data-oc="mail"]`, por CSSOM |
| Mínimos | pastas 168px, lista 240px, leitura 380px |
| Pegas de redimensionar | dois `<div role="separator" data-oc="separador" data-oc-separador="pastas\|lista" tabindex="0">` entre as colunas; arrasto e setas (Shift = passo grande) |
| Pastas recolhidas | `data-oc-pastas="recolhido"` em `[data-oc="mail"]`; botão `data-oc="alternar-pastas"` com `aria-pressed` |
| Modo de leitura | botão `data-oc="focar-leitura"` com `aria-pressed`: recolhe as pastas e põe a lista no mínimo |
| Repor | `data-oc="repor-disposicao"` nas definições do Correio |
| Scroll | a lista e a leitura rolam **dentro** da coluna; a página não rola |

O D12 dá `.ods-app__split--3` com `220px · minmax(280px, 380px) · 1fr` fixos, e
chama «separador» a uma fila de separadores de pasta — que no código não existe
(as pastas vivem na lateral). Pedido: a regra que liga a grelha às duas
variáveis, ao estado recolhido e às duas pegas (onde ficam na grelha, como se
desenham, largura sensível), e a confirmação de que as pastas continuam na
lateral.

Três viagens de browser medem isto e estão vermelhas até lá:
`a_pessoa_arruma_o_correio_e_nao_o_parte`, `o_correio_rola_por_dentro_e_nao_por_fora`
e `o_compositor_obedece_e_guarda_o_que_se_escreveu`.

### Compositor (Q-24)

| O quê | Como o `app.js` o diz |
|---|---|
| Tamanho livre | `--oc-comp-largura`, `--oc-comp-altura` (px) por CSSOM, a partir da pega `data-oc="compositor-puxador"`; mínimo 380×320, limitado ao ecrã |
| Posição | `--oc-comp-fundo`, `--oc-comp-direita` |
| Expandir | `data-oc="compositor-expandir"` |

O D8 dá dois tamanhos (normal e `data-expanded`). Pedido: se o tamanho livre
fica, a regra que usa as variáveis; se não fica, dizê-lo, e o puxador sai.

## 2. Regras provisórias a confirmar

Em `static/ods-integration.css`, cada uma com a razão escrita ao lado:

- **Q-25**: fora do Desktop, `.ods-shell__main` termina 84px acima do fundo, para
  o botão das aplicações não tapar o conteúdo.
- **Q-35**: `[hidden]` ganha a `display` próprio; `fieldset` sem moldura; cores
  das variantes de `.ods-btn` quando o botão é um `<a>`.

Adoptar (movendo-as para uma folha do desenho), substituir ou retirar.

## 3. O resto

Q-19 a Q-22 e Q-27 a Q-34 (a Q-26 resolveu-se no código), em `CLAUDE_DESIGN_QUESTIONS.md`. Nenhuma bloqueia;
todas têm a proposta aplicada e visível no ramo.
