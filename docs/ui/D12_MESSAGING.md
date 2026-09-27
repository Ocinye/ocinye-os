# D12 · Mensagens (`messaging.rs`, 101)

Os marcadores listados mantêm-se; os que o pedido abrevia com «…» e existem no ficheiro mantêm-se também, no elemento com o mesmo papel.
| Painel | HTML |
|---|---|
| `messaging` | `<div class="ods-app">` · `.ods-app__split` (lista de conversas · conversa) |
| `linha_da_conversa` | `<a class="ods-d12-conv" data-oc="conversa" aria-current>`: `ods-avatar--sm` (`iniciais`) + `presenca` (`ods-dot--success`) · nome · última mensagem · `quando_curto` · contagem `ods-count` estático |
| `nova_conversa` | `ods-modal` com `data-oc="campo-nome"`, pessoas (`data-oc="acrescentar-membro"`) e `data-oc="criar-conversa"` |
| `sem_conversa` | `ods-empty` |
| `aberta_view` | `<section class="ods-d12-thread" data-oc="conversa-aberta">` · cabeçalho (nome, `data-oc="detalhes"`) · `.ods-d12-thread__scroll` · `separador_do_dia` = `<div class="ods-d12-thread__day">` · mensagem `.ods-d12-msg` (`--mine`), `hora`/`instante` em `.ods-d12-msg__time` · citação `data-oc="citada"` · acções `data-oc="accoes-da-mensagem"` (`abrir-reaccoes`, `copiar`, responder) |
| composer | `<form class="ods-d12-chat__composer" data-oc="composer">` · `data-oc="a-responder"` (faixa com `cancelar-resposta`) · `abrir-emoji`, `abrir-assist` (menu `assist-menu`, painel `assist`) · indicador `data-oc="a-escrever"` (`ods-field__hint`) |
| `aviso` | `ods-notice` |
**Estados** (todos os painéis com dados): a carregar `ods-skeleton` + `aria-busy="true"` · vazio `ods-empty` · erro `ods-state--error` (texto do Core) · recusado `ods-state--denied` · indisponível `ods-state--unavailable`. Falha do Core nunca aparece como 0 ou lista vazia.
