# D1 · Primitivas

CSS: `static/ods-d1-primitives.css` (depende de `ocinye-ds.css`). Ícones: `static/ods-icons.svg` (40 símbolos, prefixo `ods-`).
Cada primitiva é uma função Leptos em `ui/ods/` (pasta nova; nada em `ui/components/`). As assinaturas são indicativas.

## Regras comuns
- Nenhum `style=""`. Variantes por modificador (`--primary`), estado por atributo ARIA (`aria-selected`, `aria-expanded`, `aria-invalid`, `aria-disabled`, `hidden`).
- Comportamento (abrir/fechar, teclado) em `static/app.js`, ligado por `data-oc`. O CSS nunca depende de uma classe posta pelo JS: o JS alterna atributos.
- Texto sempre por chave i18n.

| Primitiva | HTML | Classes / estados | Teclado e ARIA |
|---|---|---|---|
| Button | `<button type="button\|submit" class="ods-btn ods-btn--primary">` | `--primary --navy --danger --danger-soft --ghost --sm --block`; desactivado = `disabled` | nativo |
| IconButton | `<button class="ods-iconbtn" aria-label="…"><svg class="ods-icon"><use href="/static/ods-icons.svg#ods-bell"/></svg></button>` | `aria-expanded` quando abre popover | `aria-label` obrigatório |
| Input | `<label class="ods-field"><span class="ods-field__label">…</span><input class="ods-input" name="…"></label>` | erro: `aria-invalid="true"` + `<span class="ods-field__error" id="…">` com `aria-describedby` | label sempre presente |
| SearchInput | `<form class="ods-search" role="search" method="get" action="…"><svg class="ods-icon"/><input class="ods-search__input" name="q"><kbd class="ods-kbd">⌘K</kbd></form>` | foco = anel dourado | Enter envia |
| Checkbox / Radio / Switch | `<input type="checkbox" class="ods-check\|ods-switch">` | nativos | `role="switch"` no switch |
| Tabs | `<div class="ods-tabs" role="tablist"><button role="tab" class="ods-tabs__tab" aria-selected="true">… <span class="ods-tabs__count">27</span></button></div>` | seleccionado = navy | ←/→ entre separadores; mantém `aria-selected` (contrato) |
| SegmentedControl | `<div class="ods-seg" role="tablist"><button class="ods-seg__opt" aria-selected="true">` | | idem |
| Badge / Status / Dot | `<span class="ods-badge ods-badge--success">OPERACIONAL</span>` · `<span class="ods-dot ods-dot--warning" aria-hidden="true">` | estado nunca só por cor: o ponto vem sempre com texto | |
| Count | `<span class="ods-count" aria-hidden="true">4</span>` dentro de um botão `position:relative` | | o número repete-se no `aria-label` do botão |
| Tooltip | `<div class="ods-tooltip" role="tooltip" id="…">` | posicionado por `app.js` via CSSOM (`el.style.setProperty('left', …)`), permitido pela CSP | `aria-describedby` no alvo; aparece também com foco |
| Popover / Menu | `<div class="ods-popover ods-popover--right ods-glass" data-oc="…-menu" role="menu" hidden><div class="ods-menu">…<button role="menuitem" class="ods-menu__item">` | aberto = sem `hidden` | Esc fecha e devolve foco; ↑/↓ entre itens; clique fora fecha |
| ContextMenu | igual ao Menu, posição por CSSOM | | Shift+F10 abre |
| Modal | `<div class="ods-modal" role="dialog" aria-modal="true" aria-labelledby="…" hidden><div class="ods-scrim"></div><div class="ods-modal__panel ods-window-surface">` | | foco preso; Esc fecha |
| Drawer | `<aside class="ods-drawer ods-window-surface" role="dialog">` | | idem |
| Toast | `<div class="ods-toast" role="status" aria-live="polite">…<button class="ods-toast__undo">Anular</button></div>` | some após 8 s (anular) ou 4 s | |
| Banner / Notice | `<div class="ods-banner">` · `<div class="ods-notice ods-notice--warning" role="status">` | | |
| Table | `<table class="ods-table">`, `<th aria-sort="descending">`, `<tr aria-selected="true">`, números `<td class="ods-num">` | | cabeçalho ordenável é `<button>` dentro do `th` |
| EmptyState | `<div class="ods-empty"><span class="ods-empty__icon">…</span><p class="ods-empty__title">…</p><p class="ods-empty__body">…</p></div>` | | |
| Estados honestos | `<div class="ods-state ods-state--denied\|--unavailable\|--error" role="status">` + ícone `ods-lock` / `ods-status` | **vazio ≠ a carregar ≠ indisponível ≠ recusado ≠ erro**; falha do Core nunca aparece como 0 | |
| Progress | `<div class="ods-progress" role="progressbar" aria-valuenow="62" data-ods-value="62"><div class="ods-progress__bar"></div></div>` | largura posta por `app.js` a partir de `data-ods-value` | |
| Skeleton | `<div class="ods-skeleton" aria-hidden="true">` | só enquanto carrega | `aria-busy="true"` no contentor |
| Breadcrumbs | `<nav class="ods-crumbs" aria-label="…"><a>UENR-001</a><span aria-hidden="true">/</span><span aria-current="page">Home</span></nav>` | | `aria-current` (contrato) |
| Avatar | `<span class="ods-avatar">FM</span>` ou `<img>` de `/avatar/me/{version}` | `--sm --lg` | `alt` = nome |
