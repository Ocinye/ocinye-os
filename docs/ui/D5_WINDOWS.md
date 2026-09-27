# D5 · Gestor de janelas

CSS: `static/ods-d5-windows.css`. **Depende de G-05** — nenhum ficheiro deste pacote entra antes de haver decisão.

## Proposta de contrato (para o Claude Code decidir)
- Estado das janelas no cliente (`app.js`), **por sessão de página**, sem `localStorage` enquanto G-02 não guardar janelas no Core.
- Cada janela mostra a **rota real** num `<iframe class="ods-window__frame" src="/files" title="Ficheiros">` (`frame-src 'self'` já está na CSP). A URL da janela focada vai para `history.replaceState`: recarregar abre essa rota, e as ligações directas continuam a funcionar (§21).
- Posição e tamanho aplicados por CSSOM (`el.style.setProperty('left', x+'px')`), nunca por `style=""` no HTML.

## Estrutura
```html
<section class="ods-window ods-window-surface" data-oc="window" data-app="files" data-state="normal|minimized|maximized" data-focused role="dialog" aria-label="Ficheiros">
  <header class="ods-window__bar" data-oc="window-drag">
    <span class="ods-window__title"><svg class="ods-icon">…</svg>Ficheiros</span>
    <div class="ods-window__ctl">
      <button class="ods-iconbtn" data-oc="window-minimize" aria-label="Minimizar">…</button>
      <button class="ods-iconbtn" data-oc="window-maximize" aria-label="Maximizar" aria-pressed="false">…</button>
      <button class="ods-iconbtn" data-oc="window-close" aria-label="Fechar">…</button>
    </div>
  </header>
  <iframe class="ods-window__frame" src="/files" title="Ficheiros"></iframe>
  <div class="ods-window__resize" data-oc="window-resize" aria-hidden="true"></div>
</section>
```
Teclado: Ctrl/⌘+Tab alterna, Ctrl+↑ ou F3 abre «Todas as janelas» (`.ods-overview`, `data-oc="overview"`), Esc fecha a visão geral. Alternativa acessível ao arrastar: botões de encaixe (esquerda/direita/maximizar) no menu da barra.
Telemóvel: uma janela de cada vez, em ecrã inteiro, sem arrastar.

## Até G-05
As aplicações abrem como hoje (navegação de página). O botão «Todas as janelas» da barra de apps mostra `aria-disabled="true"`.
