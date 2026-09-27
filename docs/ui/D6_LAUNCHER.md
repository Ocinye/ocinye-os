# D6 · Lançador e barra de aplicações

CSS: `static/ods-d6-launcher.css`. Dados: **o registo** `ui/apps.rs` (categorias `productivity research knowledge communication administration`, rótulos por chave `apps.category.*`) e as fixações (`PUT /apps/pins`, tabela `0051_member_app_pins`). Não há segunda lista no frontend.

## Lançador (⌘J)
```html
<div class="ods-launcher" data-oc="launcher" role="dialog" aria-modal="true" aria-labelledby="ods-launcher-title" hidden>
  <div class="ods-scrim" data-oc="launcher-fechar"></div>
  <div class="ods-launcher__panel ods-window-surface" data-oc="launcher-painel">
    <div class="ods-launcher__head">
      <div class="ods-launcher__title-row"><svg class="ods-icon ods-icon--lg">…apps-brand…</svg><h2 class="ods-launcher__title" id="ods-launcher-title">Aplicações</h2><span class="ods-badge">27 aplicações</span><span class="ods-kbd">ESC</span><button class="ods-iconbtn" data-oc="launcher-fechar" aria-label="Fechar">…</button></div>
      <label class="ods-search ods-launcher__search"><svg class="ods-icon">…search…</svg><input class="ods-search__input" data-oc="launcher-input" placeholder="Pesquisar aplicações do Ocinye…"><kbd class="ods-kbd">⌘J</kbd></label>
      <div class="ods-tabs ods-launcher__cats" role="tablist"><button class="ods-tabs__tab" role="tab" aria-selected="true" data-oc="launcher-chip" data-category="all">Todas <span class="ods-tabs__count">27</span></button>…</div>
    </div>
    <div class="ods-launcher__body" data-ods-scroll>
      <div class="ods-launcher__sections">favoritos (= fixadas) · recentes (G-06)</div>
      <div class="ods-launcher__grid" data-oc="launcher-grelha">
        <div class="ods-launcher__cell" data-oc="launcher-cell">
          <a class="ods-launcher__item" href="/files" data-oc="launcher-item" data-app-id="files"><span class="ods-app-tile" data-oc="launcher-icone">…</span><span><span class="ods-launcher__name" data-oc="launcher-nome">Ficheiros</span><span class="ods-launcher__desc">…</span></span></a>
          <button class="ods-iconbtn ods-launcher__pin" data-oc="launcher-pin" data-app-id="files" aria-pressed="true" aria-label="Desafixar Ficheiros">…star…</button>
        </div>
      </div>
      <div class="ods-empty" data-oc="launcher-vazio" hidden>…</div>
    </div>
    <div class="ods-launcher__foot">↑↓ navegar · ↵ abrir · esc fechar</div>
  </div>
</div>
```
Marcadores mantidos: `launcher launcher-open launcher-fechar launcher-painel launcher-input launcher-chip launcher-grelha launcher-cell launcher-item launcher-icone launcher-nome launcher-pin launcher-vazio`. O `home` não tem `launcher-pin` (teste existente).
Tamanho fixo 860×640; o conteúdo rola com a barra inteligente (`data-ods-scroll`).
**Recentes (G-06):** a secção mostra `ods-state--unavailable` até `recent_apps` existir.

## Barra vertical
Abre pelo botão `.ods-float-apps` (D4). Ordem de cima para baixo: Mostrar Desktop, Todas as janelas (`aria-disabled` até G-05), Aplicações, separador, **fixadas** (do registo), separador, Lixo (`/files` com filtro de lixo existente).
```html
<nav class="ods-shelf ods-glass--dark" data-oc="side-pinned" aria-label="Aplicações fixadas" hidden>
  <a class="ods-shelf__btn" href="/" aria-current="page" data-tip="Mostrar Desktop" aria-label="Mostrar Desktop">…home…</a>
  …
  <a class="ods-shelf__btn" href="/files" data-oc="fixada" data-app-id="files" data-tip="Ficheiros" aria-label="Ficheiros">…<span class="ods-shelf__running" aria-hidden="true"></span></a>
</nav>
```
O nome aparece num `.ods-tooltip` à direita, posto pelo `app.js` (hover e foco). Botão e barra somem após `--ods-idle-delay` sem rato (`data-ods-float`, `data-idle` na raiz), excepto com foco ou rato perto do canto.
