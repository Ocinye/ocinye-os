# D12 · Notas (`notes.rs`, 66)

Completa o D8.
| Painel | HTML |
|---|---|
| `notes_list` | `.ods-app__split--3`: pastas · lista `note_card` · editor |
| `note_card` | `<a class="ods-notes__list-item" aria-current>` (`__title`, `__excerpt`, meta mono com data e ícone de partilha) |
| `shared_notes_list` | igual, com o autor em `ods-avatar--sm` |
| `notes_trash` | lista com «Restaurar» (`/notes/{id}/restaurar`) e «Eliminar» (`/notes/{id}/eliminar`, confirmação `ods-modal`) |
| `note_editor` | `.ods-editor` + `.ods-editor__title` + `.ods-editor__state[data-dirty]` (ProseMirror existente) · barra lateral `.ods-drawer` com separadores Actividade / Histórico / Partilha |
| `activity_panel` | `.ods-timeline` |
| `history_panel` | lista de revisões (`.ods-menu__item` com data) |
| `revision_preview` | `.ods-editor` só leitura + `ods-btn--primary` «Repor esta versão» |
| `shared_note_reader` | `.ods-editor` só leitura + `ods-badge` «Partilhada por …» |
| `share_panel` | form `/notes/{id}/partilhar`: `.ods-input` pessoa + `.ods-seg` ler/editar + lista actual |
**Estados** (todos os painéis com dados): a carregar `ods-skeleton` + `aria-busy="true"` · vazio `ods-empty` · erro `ods-state--error` (texto do Core) · recusado `ods-state--denied` · indisponível `ods-state--unavailable`. Falha do Core nunca aparece como 0 ou lista vazia.
