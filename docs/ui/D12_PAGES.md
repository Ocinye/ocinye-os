# D12 · Páginas de lista, detalhe e consulta

Moldura `.ods-page` (D9). Métricas sempre em `.ods-d12-metrics` › `.ods-d12-metric` (`__label` mono maiúsculas, `__value` 22px tabular, `__hint`).

## `lists.rs` (27) — listas de unidades, ideias, projectos, datasets, bibliografia, auditoria
| Painel | HTML |
|---|---|
| `list_screen` | `.ods-page__head` (título, contagem, acção primária) + filtros + `<table class="ods-table">` + `footer` |
| `unit_selector` | `<form method="get">` com `<select class="ods-input" name="(actual)">` — filtro por consulta, não unidade activa global (Q-02) |
| `ideas_tabs`, `projects_tabs` | `.ods-tabs` (`aria-selected`) |
| `items`, `unit_cell`, `string_list`, `number`, `truncated` | células `<td>`; números `ods-num`; texto longo com `title` e reticências |
| `actor_da_auditoria` | `ods-avatar--sm` + nome; `data-oc="normalizado"` e `data-oc="revisao"` mantêm-se nas células actuais |
| `pager`, `footer_paginado` | `<nav class="ods-d12-pager" aria-label="Paginação">` (anterior/seguinte `ods-btn--sm`, «1–50 de 230» mono) |
| `sem_autorizacao` | `ods-state--denied` a ocupar a lista |

## `workspaces.rs` (49) — `/workspaces/{id}`, `/ideas/{id}`, `/projects/{id}`
| Painel | HTML |
|---|---|
| `research_workspace` | `.ods-page` · cabeçalho (`.ods-card__code` + título + `ods-badge` de estado = `idea_state_label`) · `tabs` (`.ods-tabs`, destinos de `tab_destination`) · `.ods-detail` |
| `idea_overview`, `project_overview` | principal: descrição, `metric`/`metric_text` em `.ods-d12-metrics`; lateral `.ods-kv` |
| `idea_lifecycle_actions` | forms POST `…/transition` com o verbo de `idea_transition_verb` (`ods-btn--navy`; recusar `ods-btn`) |
| `activity_list` | `.ods-timeline` |
| `task_list` | `<table class="ods-table">` com `task_state_label` em `ods-badge` |

## `science.rs` (19) — cadeia científica
| Painel | HTML |
|---|---|
| `scientific_chain` | `<ol class="ods-d12-chain">` · `etapa` = `<li class="ods-d12-chain__step" data-state="done\|current\|todo">` com `rotulo_do_recurso` e ligação |
| `cabecalho`, `contexto_do_ambiente` | `.ods-page__head` + `.ods-crumbs` |
| `result_detail`, `passos`, `classificacoes` | `.ods-detail`; passos em `.ods-timeline`; classificações `.ods-chips` |
| `validate_result`, `nova_hipotese`, `accoes` | forms POST existentes em `.ods-settings__actions` |
| `recusa` | `ods-state--denied` com o motivo |

## `my_work.rs` (15), `knowledge.rs` (15), `resources.rs` (12), `compute.rs` (13), `search.rs` (19), `help.rs` (11)
| Ecrã · painel | HTML |
|---|---|
| `my_work` · `items` | grupos por prazo (`.ods-label` + `.ods-table`) |
| `knowledge` · `counter` | `.ods-d12-metrics`; `counter_not_implemented` = `.ods-d12-metric[data-unavailable]` com «—» e `ods.state.pending_contract` (nunca 0) |
| `resources` · `metric`, `state_badge`, `source_label` | `.ods-d12-metric` com `.ods-progress` (`human_bytes`/`human_bytes_com`); estado `ods-badge--success\|warning\|error` |
| `compute` · `columns`, `items`, `metric` | `.ods-d12-metrics` + `.ods-table` de nós (`bytes` em `ods-num`) |
| `search` · `resultados_do_corpo`, `entity_label`, `destination` | `.ods-d12-results` agrupado por `entity_label` (`.ods-label` por grupo) |
| `help` · `seccao`, `p`, `estado` | `.ods-d12-help`: índice lateral + artigos; `estado` = `ods-badge` |
**Estados** (todos os painéis com dados): a carregar `ods-skeleton` + `aria-busy="true"` · vazio `ods-empty` · erro `ods-state--error` (texto do Core) · recusado `ods-state--denied` · indisponível `ods-state--unavailable`. Falha do Core nunca aparece como 0 ou lista vazia.

## `notice.rs` (5) — ecrãs de aviso
Estrutura única: `<main class="ods-d12-notice" data-kind="…">` · `.ods-empty__icon` · título · texto · acções.
| Painel | `data-kind` | Ícone | Acção |
|---|---|---|---|
| `not_found` | `not-found` | `ods-search` | «Voltar ao Desktop» `/` |
| `failure` | `failure` | `ods-status` | «Tentar de novo» (recarregar) |
| `access_denied` | `denied` | `ods-lock` | «Voltar» |
| `application_inactive` | `inactive`, com `data-oc="app-inactive"` | `apps-brand` | texto «Não activa nesta instância» |
| `unavailable` | `unavailable` | `ods-status` | — |
| `rejected` | `rejected` | `ods-shield` | «Voltar» |
| `conflict` | `conflict` | `ods-edit` | «Recarregar» |

## `settings.rs` (40)
Moldura D10. `seccoes_das_definicoes` = `.ods-settings__nav` (`aria-current`). `facto` = `.ods-settings__row` (rótulo, valor, `__row-hint`). `account`: `imagem_de_perfil` = `ods-avatar--lg` + `.ods-avatar-pick`. `language`: `.ods-seg` pt · en · fr (form actual). `apps`: `.ods-admin-list` com `ods-switch` de fixação. `security`: palavra-passe (`.ods-field`), sessões. `mfa_recovery`: `<pre class="ods-auth__codes" data-oc="recovery-codes">` + `data-oc="recovery-copy"`, `"recovery-download"`.
