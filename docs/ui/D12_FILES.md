# D12 · Ficheiros (`files.rs`, 89)

Completa o D8. Os marcadores listados mantêm-se; os que o pedido abrevia com «…» e existem no ficheiro mantêm-se também, no elemento com o mesmo papel.
| Painel | HTML |
|---|---|
| `files` | `<section class="ods-app" data-oc="fs">` · `.ods-app__toolbar` · `.ods-app__split` (lateral: `escolher_ambiente` / `seletor_de_ambiente`, pastas) |
| `seletor_de_ambiente` | `.ods-seg` Pessoal / Institucional (`aria-selected`); indisponível se o membro não tiver âmbito institucional |
| `all_files` | `<div class="ods-files__grid" data-oc="fs-grelha" data-view="grid">`; alternador `data-oc="fs-vista"` (`.ods-seg`) |
| `ficha_de_pasta` | `<a class="ods-file ods-file--folder" data-oc="fs-item" data-kind="folder">` com `.ods-file__thumb` (ícone `ods-files`), `.ods-file__name`, `.ods-file__meta` |
| `ficha_de_ficheiro` | `<div class="ods-file" data-oc="fs-item" aria-selected="false">` · caixa `<input type="checkbox" class="ods-check ods-file__sel" data-oc="fs-sel">` · `<button class="ods-file__open" data-oc="fs-abrir">` · `<span class="ods-file__thumb" data-oc="fs-thumb">` (img de `/me/files/{v}/thumbnail` ou ícone por `tipo_legivel`) · meta = `tamanho` · `number` |
| `painel_de_pasta` | cabeçalho `.ods-d12-folder-head` (migalhas `.ods-crumbs`, `quota_texto` + `.ods-progress`) |
| `destino_de_carregamento` | `<form data-oc="fs-carregar-form" method="post" action="(actual)">` + `<input type="file" name="file" multiple class="ods-sr-only" data-oc="fs-carregar">` + `<label class="ods-btn ods-btn--primary" for="…">`; zona de largar `.ods-dropzone[data-over]` |
| `barra_de_seleccao` | `<div class="ods-files__batch ods-glass" data-oc="fs-lote" hidden>` · `<span data-oc="fs-lote-conta">` · forms com `<input type="hidden" name="file_ids" data-oc="fs-lote-ids">` (mover, apagar) · `<button data-oc="fs-lote-limpar">` |
| `vista_do_lixo` | mesma grelha; cada ficha com «Restaurar» (`/me/files/restore`) e «Eliminar» (`/me/files/purge`, `ods-btn--danger-soft`); topo com «Esvaziar lixo» (`/files/trash/empty`) e aviso de 30 dias |
| Quick Look | D8 (`fs-quicklook`, `fs-ql-*`) |
**Estados** (todos os painéis com dados): a carregar `ods-skeleton` + `aria-busy="true"` · vazio `ods-empty` · erro `ods-state--error` (texto do Core) · recusado `ods-state--denied` · indisponível `ods-state--unavailable`. Falha do Core nunca aparece como 0 ou lista vazia.
Quota ≥ 90 %: `ods-notice--warning` no `painel_de_pasta`.
