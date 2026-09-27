# D12 · Administração (`administration.rs`, 41 classes legadas)

Moldura: `.ods-settings` (D10), com a navegação da administração. Os marcadores listados mantêm-se; os que o pedido abrevia com «…» e existem no ficheiro mantêm-se também, no elemento com o mesmo papel.

| Painel | HTML e classes |
|---|---|
| `overview_tab` | `<section class="ods-settings__section"><h2 class="ods-settings__section-title">` + `.ods-d12-metrics` (cartões `.ods-d12-metric`: `__label` mono, `__value`, `__hint`) |
| `new_member` | `<form method="post" action="/admin/members/new">` com `.ods-field` por campo (mesmos `name`); rodapé `.ods-settings__actions` com `ods-btn--primary` |
| `issued_credential` | `.ods-notice.ods-notice--warning` + `<div class="ods-secret" data-oc="secret" data-oc-value="…">••••</div>` + `<button class="ods-iconbtn" data-oc="secret-toggle" aria-pressed="false">` + `<button class="ods-iconbtn" data-oc="secret-copy">`. Mostra-se uma vez; texto «Não voltará a ser mostrada». |
| `member_detail` | cabeçalho `.ods-d12-person` (`ods-avatar--lg`, nome, email, `ods-badge` de estado) + `.ods-tabs` com `overview_tab` · `access_tab` · `security_tab` |
| `access_tab`, `roles_admin`, `grants_admin` | `<table class="ods-table">` (papel, âmbito, `source_label` como `ods-badge`, desde `day`) + form POST por linha (`ods-btn--sm ods-btn--danger-soft` para revogar) |
| `security_tab`, `account_transitions` | `.ods-settings__row` por facto (MFA, última sessão, estado) + botões das transições existentes (suspender/reactivar/eliminar → `ods-btn--danger` com confirmação `ods-modal`) |
| `units_admin`, `workspaces_admin` | `.ods-admin-list` com `.ods-admin-list__opt` (nome, `position_label`, acção) |
| instância | `<div data-oc="instance-admin">` · `<li class="ods-admin-list__opt" data-oc="instance-app" data-app-id="…">` com `ods-switch` · `<button class="ods-btn ods-btn--primary" data-oc="instance-save">` |

**Estados** (todos os painéis com dados): a carregar `ods-skeleton` + `aria-busy="true"` · vazio `ods-empty` · erro `ods-state--error` (texto do Core) · recusado `ods-state--denied` · indisponível `ods-state--unavailable`. Falha do Core nunca aparece como 0 ou lista vazia.
Recusado aparece quando o Core nega a permissão de administração: `ods-state--denied` ocupa o conteúdo; a navegação da administração não aparece.
