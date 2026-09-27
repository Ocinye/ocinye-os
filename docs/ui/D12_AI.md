# D12 · Ocinye AI, agentes, Pesquisar/Perguntar e Prompt

## `ai.rs` (31) — `/ai`, `/ai/agents*`
| Painel | HTML |
|---|---|
| `hub` | `.ods-page` · `.ods-page__head` · `.ods-d12-metrics` com `counter`/`metric` (`.ods-d12-metric`) · `.ods-cards` com `items` |
| `items` | `<a class="ods-card">` (`__code` = modelo/fornecedor, `__title`, `__body`, `__foot` com `ods-dot` + estado em texto) |
| `new_agent` | `<form method="post" action="/ai/agents/new">` · `campo` = `.ods-field` · `fonte` = `<fieldset class="ods-d12-sources">` com `ods-check` por fonte |
| `agent_detail` | `.ods-detail` (principal: descrição, fontes; lateral: `metric` em `.ods-kv`) |
Sem fornecedor: `ods-state--unavailable` com `shell.status.ai.none.desc`.

## `ask.rs` (28) — `/ask`, planos execute/reject
| Painel | HTML |
|---|---|
| `ask` / `command_form` | `<form class="ods-search ods-d12-ask__form" method="get" action="/ask" role="search">` `name="q"` |
| `results` / `result` | `<ol class="ods-d12-results">` · `<li class="ods-d12-result" data-oc="ask-result">` (ícone da entidade, título, `entity_label` em `ods-badge`, destino) |
| `planned` | `.ods-d12-plan` (`ods-glass` não; superfície `ods-window-surface`): passos em `<ol class="ods-d12-plan__steps">` + forms `POST /ask/plans/{id}/execute` (`ods-btn--primary`) e `/reject` (`ods-btn`) |
| `executed` | `.ods-d12-plan[data-done]` com `ods-check` visual (ícone `ods-check` verde) por passo |
| `unavailable` | `ods-state--unavailable` com o motivo devolvido |

## `prompt.rs` (37) — `/ai/prompt`
```html
<div class="ods-d12-chat">
  <div class="ods-d12-chat__scroll" data-oc="prompt-scroll" data-ods-scroll>
    <div class="ods-d12-turn ods-d12-turn--member">…member_turn…</div>
    <div class="ods-d12-turn ods-d12-turn--ocinye"><div class="ods-d12-turn__body" data-oc="resposta">…</div>
      <div class="ods-d12-turn__actions"><button class="ods-btn ods-btn--sm ods-btn--ghost" data-oc="copiar-resposta">Copiar</button> …action_chip = <a class="ods-chip">…</a></div></div>
  </div>
  <form class="ods-d12-chat__composer" method="post" action="/ai/prompt" data-oc="prompt-form">
    <textarea class="ods-d12-chat__input" name="…(actual)" data-oc="prompt-textarea" rows="1"></textarea>
    <button class="ods-nye-orb__send" type="submit" data-oc="prompt-send" aria-label="Enviar">…</button>
  </form>
</div>
```
**Estados** (todos os painéis com dados): a carregar `ods-skeleton` + `aria-busy="true"` · vazio `ods-empty` · erro `ods-state--error` (texto do Core) · recusado `ods-state--denied` · indisponível `ods-state--unavailable`. Falha do Core nunca aparece como 0 ou lista vazia.
