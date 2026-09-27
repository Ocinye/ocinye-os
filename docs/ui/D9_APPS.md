# D9 · Restantes aplicações

CSS: `static/ods-d9-apps.css` + primitivas D1. Cada ecrã usa `.ods-page` (listas e detalhes) e mantém as rotas, formulários e marcadores actuais do seu ficheiro em `ui/screens/`.

| App | Rotas | Padrão | Notas |
|---|---|---|---|
| O Meu Trabalho | `/my-work` | lista `.ods-table` | |
| Unidades | `/units`, `/units/new`, `/units/{id}`, membros POST | `.ods-cards` + `.ods-detail` | só Research/Education (`InstanceProfile::activates`) |
| Ideias | `/ideas`, `/ideas/new`, `/ideas/{id}`, `…/transition` | cartões por estado | transições = forms POST existentes |
| Projectos | `/projects`, `/projects/new`, `/projects/{id}` | cartões + detalhe | |
| Cadeia científica | `/workspaces/{id}/science`, `/methodologies/*`, `/studies/*`, `/executions/*`, `/results/*` | `.ods-timeline` | |
| Datasets | `/datasets`, `/datasets/new`, `/datasets/{id}` | tabela | |
| Tarefas | `/tasks/new`, `/tasks/{id}`, `…/transition`, `…/assignee` | detalhe | |
| Calendário | `/calendar`, `/calendar/events/{id}` | grelha mensal (`.ods-cal__*` do D2 ampliado) | |
| Mensagens | `/messages*` | `.ods-app__split--3` | manter marcadores de `messaging.rs` |
| Conhecimento / Bibliografia | `/knowledge`, `/bibliography` | cartões / tabela | |
| Meus Recursos | `/resources` | `.ods-progress` por quota | |
| Ocinye AI / Agentes / Computação | `/ai`, `/ai/agents*`, `/compute` | cartões + detalhe | estado do fornecedor: `0056`/`0057` |
| Actividade | `/activity` | `.ods-timeline` | |
| Pesquisa | `/search` | lista agrupada | |
| Ajuda | `/help` | cartões | |
| **Monitor de Actividade** | — | tabela ordenável + resumo + gráfico | **G-08**: a app aparece no lançador com `ods-state--unavailable` e o texto `monitor.unavailable`; sem dados simulados |

Todos os ecrãs têm os cinco estados (D1). Um Core em falha mostra `ods-state--error`, nunca zero.
