# D13 · Correio (Q-23, Q-24)

CSS: `static/ods-d13-mail.css`. Substitui, **só no Correio**, `.ods-app__split--3` por `.ods-mail`. As restantes regras de linha/leitura do D8/D12 (`.ods-mail__row`, `.ods-d12-mail-read`) mantêm-se.

## Decisões
1. **As pastas vivem na lateral.** O «separador» do D12 descrito como `.ods-tabs` foi um erro de leitura: no Correio, `separador` é a **pega de redimensionar**. Não há fila de separadores de pasta.
2. **As larguras são da pessoa.** A grelha lê `--oc-mail-pastas` e `--oc-mail-lista` que o `app.js` já escreve. Os valores iniciais (220px / 340px) são só omissão.
3. **O compositor fica com tamanho livre.** O `compositor-puxador` mantém-se; `data-expanded` continua como atalho para o tamanho grande.

## Estrutura (marcadores do `app.js` intactos)
```html
<div class="ods-mail" data-oc="mail" data-oc-pastas="aberto|recolhido">
  <nav class="ods-mail__pastas" data-ocs aria-label="Pastas">
    <div class="ods-mail__pastas-head">
      <button class="ods-iconbtn" data-oc="alternar-pastas" aria-pressed="false" aria-label="Recolher pastas"><svg class="ods-icon"><use href="/static/ods-icons.svg#ods-panel-left"/></svg></button>
    </div>
    <a class="ods-mail__pasta" aria-current="page" href="/mail?folder=inbox"><svg class="ods-icon">…inbox…</svg><span class="ods-mail__pasta-nome">Entrada</span><span class="ods-mail__pasta-n">12</span></a>
    …
  </nav>
  <div role="separator" class="ods-mail__sep" data-oc="separador" data-oc-separador="pastas" aria-orientation="vertical" aria-label="Largura das pastas" aria-valuemin="168" aria-valuenow="220" tabindex="0"></div>
  <section class="ods-mail__lista" data-ocs aria-label="Mensagens">
    <div class="ods-mail__lista-bar">… pesquisa · filtros · <button class="ods-iconbtn" data-oc="focar-leitura" aria-pressed="false" aria-label="Modo de leitura">…focus…</button></div>
    <a class="ods-mail__row" …>…</a>
  </section>
  <div role="separator" class="ods-mail__sep" data-oc="separador" data-oc-separador="lista" aria-orientation="vertical" aria-label="Largura da lista" aria-valuemin="240" aria-valuenow="340" tabindex="0"></div>
  <article class="ods-mail__leitura" data-ocs aria-label="Leitura">
    <div class="ods-d12-mail-read">…</div>
  </article>
</div>
```
- `aria-valuenow` actualizado pelo `app.js` com a largura em px (já escreve as variáveis; basta espelhar).
- Com pastas recolhidas, a coluna das pastas fica um trilho de 56px: só ícones e contagem em ponto; `.ods-mail__pasta-nome` e `.ods-mail__pasta-n` escondem-se, o nome passa a `title`/`aria-label`. A pega das pastas desaparece (não se redimensiona um trilho).
- **Modo de leitura** (`focar-leitura`): o `app.js` já recolhe as pastas e põe a lista no mínimo. O desenho não acrescenta estado; o botão mostra `aria-pressed="true"` com o fundo `--ods-surface-selected`.

## Pegas
- Pista de 6px na grelha; a linha visível tem 1px (`--ods-border-soft`) ao centro.
- Área sensível de 14px (`::before` com −4px de cada lado), `cursor: col-resize`.
- Hover, arrasto (`[data-dragging]`, se o `app.js` o puser) e foco: linha de 2px `--ods-gold-500`. Foco com o anel `--ods-focus-ring` por fora.
- Teclado: o que o `app.js` já faz (setas; Shift = passo grande). Home/End opcionais.
- Duplo clique numa pega = repor a largura de omissão (proposta; se o `app.js` não o fizer, fica `repor-disposicao` nas definições).

## Scroll
- `.ods-mail` ocupa a altura do contentor e **não rola**. Cada coluna rola por dentro (`overflow-y: auto; min-height: 0`).
- A página não rola no Correio: `.ods-shell__main:has(> [data-oc="mail"])` fica coluna flex sem scroll próprio (ver CSS). A regra Q-25 (faixa de 84px) **não** se aplica ao Correio: o flutuante das aplicações passa por cima da coluna de leitura, que tem `padding-bottom` para lhe dar folga.

## Larguras pequenas
| Largura | Comportamento |
|---|---|
| ≥ 1280px | três colunas + duas pegas |
| 1024–1279px | pastas recolhidas por omissão (CSS força o trilho de 56px se `data-oc-pastas` não estiver definido pela pessoa) |
| 768–1023px | trilho + lista + leitura; a pega da lista mantém-se |
| < 768px | uma coluna: lista. A leitura é a rota `/mail/{id}` (já existe). Pegas escondidas |

## Compositor (Q-24)
```html
<section class="ods-composer" data-oc="compositor" role="dialog" aria-label="Nova mensagem">
  <button class="ods-composer__grip" data-oc="compositor-puxador" aria-label="Redimensionar compositor"><svg class="ods-icon">…grip…</svg></button>
  <header class="ods-composer__bar">… título · <button data-oc="compositor-expandir" aria-pressed="false">…</button> · minimizar · fechar</header>
  …linhas, corpo, rodapé do D8…
</section>
```
- Posição e tamanho vêm de `--oc-comp-direita`, `--oc-comp-fundo`, `--oc-comp-largura`, `--oc-comp-altura` (omissões 24px / 0 / 560px / 620px). Mínimo 380×320; máximo ao ecrã menos 16px e a barra de topo.
- O compositor está ancorado em baixo à direita: a pega fica no **canto superior esquerdo** (cresce para cima e para a esquerda). 18×18, ícone `ods-grip`, `cursor: nwse-resize`, visível sempre (não só no hover).
- `data-expanded`: ignora as variáveis e ocupa `inset: 5% 10%` (como no D8). A pega esconde-se enquanto expandido.
- < 640px: ecrã inteiro, sem pega, sem arrasto.
- O rascunho guarda-se como hoje; nada do tamanho vai para o Core (é preferência local).
