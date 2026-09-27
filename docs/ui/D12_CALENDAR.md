# D12 · Calendário (`calendar.rs`, 86)

Os marcadores listados mantêm-se; os que o pedido abrevia com «…» e existem no ficheiro mantêm-se também, no elemento com o mesmo papel.
| Painel | HTML |
|---|---|
| `toolbar` | `.ods-app__toolbar`: `step` (← hoje →, `ods-iconbtn`) · `period_text` (`<h2 class="ods-d12-cal__period">`) · `.ods-seg` dia/semana/mês (`aria-selected`) · `ods-btn--primary` «Novo evento» |
| `calendar` (semana/dia) | `<div class="ods-d12-cal" data-oc="linha-do-tempo">` · `eixo_das_horas` = `.ods-d12-cal__axis` · `linhas_das_horas` = `.ods-d12-cal__lines` · `coluna_do_dia` = `.ods-d12-cal__col` (+`aria-current="date"` hoje) · `faixa_de_dia_inteiro` = `.ods-d12-cal__allday` · `agora_no_dia` = `.ods-d12-cal__now` · evento = `<a class="ods-d12-cal__event" data-tone="navy\|gold\|green">` posicionado por `dispor` via CSSOM (`top`, `height`, `left`, `width`) |
| `month_grid_start` (mês) | `.ods-cal__grid` ampliado: `.ods-d12-cal__month` com `.ods-d12-cal__cell` e até 3 `.ods-d12-cal__pill` + «+n» |
| editor de evento | `<form … data-oc="editor">` · `data-oc="inicio"`, `"fim"` (`.ods-input` datetime) · `data-oc="all-day"` (`ods-switch`) · `data-oc="allday-fields"` / `"timed-fields"` (alternam `hidden`) · `data-oc="timezone"` + `"timezone-label"` · `data-oc="scope"` (`.ods-seg`) · participantes: `data-oc="participantes"`, `"procura-pessoa"` (`.ods-input`), `"lista-pessoas"` (`.ods-menu`), `"pessoa"` (`.ods-menu__item`), `"escolhidos"` (`.ods-chips` com `.ods-token`), `"sem-pessoas"` (`ods-empty` compacto) · `data-oc="submeter"` (`ods-btn--primary`) · `data-oc="temporal-centre"` no contentor que centra a hora actual |

**Estados** (todos os painéis com dados): a carregar `ods-skeleton` + `aria-busy="true"` · vazio `ods-empty` · erro `ods-state--error` (texto do Core) · recusado `ods-state--denied` · indisponível `ods-state--unavailable`. Falha do Core nunca aparece como 0 ou lista vazia.
