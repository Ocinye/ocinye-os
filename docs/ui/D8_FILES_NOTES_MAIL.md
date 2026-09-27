# D8 · Ficheiros, Notas e Correio

CSS: `static/ods-d8-apps-core.css`. Os três têm backend completo: **só muda a apresentação**. Upload segmentado, estado por guardar, autosave, arrastar e diálogos vivem no `app.js` e ficam (inventário).

## Moldura comum
`<div class="ods-app"><div class="ods-app__toolbar">…</div><div class="ods-app__split[ --3]"><nav class="ods-app__side">…</nav><div class="ods-app__main" data-ods-scroll>…</div></div></div>`
Navegação lateral: `aria-current="page"` no item activo (fundo navy, texto branco).

## Ficheiros — `/files`, `/me/files/*`, `/me/folders*`, `/files/uploads`, `/files/personal-upload`, `/files/upload-preflight`
| Elemento | Classe | Marcador (mantém) |
|---|---|---|
| raiz | `.ods-app` | `data-oc="fs"` |
| vista grelha/lista | `.ods-seg` | `data-oc="fs-vista"` |
| nova pasta | `.ods-btn` | `data-oc="nova-pasta"` (form `action="/me/folders"` ou `/files/folder`) |
| carregar | form + `<input type="file" name="file" multiple class="ods-sr-only">` | `data-oc="fs-carregar-form"`, `data-oc="fs-carregar"` |
| grelha | `.ods-files__grid` `data-view="grid\|list"` | `data-oc="fs-grelha"` |
| item | `.ods-file` `aria-selected` | `data-oc="fs-item"`, `fs-sel`, `fs-abrir`, `fs-thumb` |
| lote | `.ods-files__batch .ods-glass` | `fs-lote`, `fs-lote-conta`, `fs-lote-ids` (`name="file_ids"`), `fs-lote-limpar` |
| Quick Look | `.ods-quicklook` | `fs-quicklook`, `fs-ql-fechar`, `fs-ql-nome`, `fs-ql-descarregar`, `fs-ql-corpo` |
Estados: pasta vazia (`ods-empty` + «Carregar ficheiros»), quota cheia (`ods-notice--warning`), erro de upload por ficheiro (linha com `ods-state--error` e «Tentar de novo»), lixo (`/me/files/restore`, `/me/files/purge`, `/files/trash/empty`).

## Notas — `/notes`, `/notes/new`, `/notes/{id}`, `/notes/{id}/gravar`, `…/mover`, `…/partilhar`, `…/apagar`, `…/restaurar`, `…/eliminar`, `/notes/partilhadas`, `/notes/lixo`, `/notes/folders`
- Lista (`.ods-notes__list-item`) + editor (`.ods-editor`). O editor é o ProseMirror existente (`editor/`, `static/notes-editor.js`, HEADLESS): só se restiliza o contentor.
- Estado de gravação: `.ods-editor__state` com `data-dirty` enquanto houver alterações; fechar com alterações abre o diálogo existente (guardar / descartar / cancelar).
- O formato guardado **não muda**.

## Correio — `/mail`, `/mail/compose`, `/mail/send`, `/mail/drafts`, `/mail/message/{id}`, `…/flags`, `/mail/{mailbox}`, `…/sync`, `…/connect`, `…/disconnect`
| Elemento | Marcador (mantém) |
|---|---|
| raiz | `data-oc="mail"` |
| separadores | `data-oc="separador"` com `aria-selected` |
| disposição | `disposicao`, `alternar-pastas`, `focar-leitura`, `repor-disposicao`, `disposicao-reposta` |
| compositor | `compositor`, `compositor-pega`, `compositor-expandir`, `compositor-fechar`, `compositor-draft-id`, `linha-cc`, `linha-bcc`, `mostrar-cc`, `mostrar-bcc`, `anexos-lista`, `anexo` (+`data-oc-id`), `tirar-anexo`, `compositor-editor`, `compositor-corpo`, `compositor-html`, `assinatura`, `compositor-estado`, `compositor-enviar`, `compositor-descartar`, `descartar-guardar`, `descartar-descartar`, `descartar-cancelar`, `compositor-puxador`, `compositor-ferramentas`, `ferramenta`, `compositor-ficheiro`, `destinatarios` (+`data-oc-campo`), `fichas`, `destino-entrada`, `sugestoes`, `assistencia` |
| caixas | `ligar-caixa-nova`, `ligacao-caixa`, `ligar-caixa`, `desligar-caixa` |
Linha: `.ods-mail__row` com `data-unread`; compositor `.ods-composer .ods-window-surface` com `data-expanded`. Nenhum estado funcional (rascunho, spam, lixo, arquivo) fica escondido.
