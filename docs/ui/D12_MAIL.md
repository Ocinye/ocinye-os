# D12 · Correio (`mail.rs`, 120)

Completa o D8 (linha e compositor). Os marcadores listados mantêm-se; os que o pedido abrevia com «…» e existem no ficheiro mantêm-se também, no elemento com o mesmo papel.
| Painel | HTML |
|---|---|
| `mail` | `<div class="ods-app" data-oc="mail">` · `.ods-app__toolbar` (`comandos_de_disposicao`, `sync_action`, `compose_action`) · `.ods-app__split--3` |
| `separador` | `.ods-tabs` com `<a role="tab" class="ods-tabs__tab" data-oc="separador" aria-selected>` (Entrada, Enviados, Rascunhos, Arquivo, Spam, Lixo + `__count`) |
| `comandos_de_disposicao` | `<div data-oc="disposicao">` · `ods-iconbtn` `data-oc="alternar-pastas"` e `"focar-leitura"` (`aria-pressed`) · `data-oc="repor-disposicao"` + `data-oc="disposicao-reposta"` (`ods-field__hint`, `hidden`) |
| `rail` | `<nav class="ods-app__side">` com as caixas (`.ods-app__side-item`, `aria-current`) |
| `list` / `row` | `<div class="ods-app__main" data-ods-scroll>` · `<a class="ods-mail__row" data-unread aria-current>` (D8) · `flag_form` = form inline com `ods-iconbtn` (estrela, lido) |
| `reading_placeholder` | `ods-empty` «Seleccione uma mensagem» |
| `reading` | `<article class="ods-d12-mail-read">`: cabeçalho (`addresses` em `.ods-kv`, data), corpo higienizado do Core em `.ods-d12-mail-read__body`, anexos `.ods-chips` |
| `service_notice` | `ods-notice--warning` (sincronização atrasada, caixa desligada) |
| `unavailable_screen` | `ods-state--unavailable` a ocupar a app, com «Ligar caixa» (`data-oc="ligar-caixa-nova"`) |
| caixas | `.ods-admin-list` com `data-oc="ligacao-caixa"`, `"ligar-caixa"`, `"desligar-caixa"` |
| compositor | D8 (`compositor*`, `anexo*`, `assinatura`, `assistencia`, `destinatarios`, `fichas`, `sugestoes`, `descartar-*`) |
**Estados** (todos os painéis com dados): a carregar `ods-skeleton` + `aria-busy="true"` · vazio `ods-empty` · erro `ods-state--error` (texto do Core) · recusado `ods-state--denied` · indisponível `ods-state--unavailable`. Falha do Core nunca aparece como 0 ou lista vazia.
