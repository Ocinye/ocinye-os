# Registo de aplicações — inventário

> A **fonte** deste inventário é o registo tipado em
> [`apps/workspace/src/ui/apps.rs`](../../apps/workspace/src/ui/apps.rs); esta
> tabela é um retrato dele. Quando divergirem, o código manda — e um teste
> (`cada_aplicacao_tem_rota_registada`, `…_no_catalogo`, `…_unicos`) impede o
> registo de ficar incompleto. A rota, o rótulo, o ícone e o **direito** de cada
> aplicação vêm do `Screen` que ela abre; a categoria, a descrição e a política
> de fixação são do registo.

23 aplicações (todos os ecrãs de primeira classe menos a superfície de comando
`Search`/`Ask`, que é uma superfície distinta — briefing §36).

| id | rota | categoria | direito que a revela | fixável | por omissão |
|---|---|---|---|---|---|
| `notes` | `/notes` | Produtividade | — (qualquer membro) | sim | **sim** |
| `calendar` | `/calendar` | Produtividade | `calendar.view` | sim | — |
| `work` | `/my-work` | Produtividade | — (qualquer membro) | não (estrutural) | — |
| `home` | `/` | Produtividade | — (qualquer membro) | não (estrutural) | — |
| `mail` | `/mail` | Comunicação | `mail.use` | sim | — |
| `messages` | `/messages` | Comunicação | `messaging.use` | sim | — |
| `files` | `/files` | Conhecimento | módulo `files` (relevância) | sim | **sim** |
| `knowledge` | `/knowledge` | Conhecimento | módulo `knowledge` | sim | — |
| `bibliography` | `/bibliography` | Conhecimento | módulo `bibliography` | sim | — |
| `units` | `/units` | Investigação | `units.view` | sim | — |
| `ideas` | `/ideas` | Investigação | `ideas.view` | sim | — |
| `projects` | `/projects` | Investigação | `projects.view` | sim | **sim** |
| `datasets` | `/datasets` | Investigação | módulo `datasets` | sim | — |
| `prompt` | `/ai/prompt` | Investigação | `ai.use` | sim | — |
| `ai` | `/ai` | Investigação | `ai.use` | sim | — |
| `agents` | `/ai/agents` | Investigação | `agents.view` | sim | — |
| `compute` | `/compute` | Investigação | `compute.view` | sim | — |
| `resources` | `/resources` | Administração | — (qualquer membro) | sim | — |
| `activity` | `/activity` | Administração | `organisation.view` | sim | — |
| `administration` | `/admin` | Administração | `members.manage` | sim | — |
| `audit` | `/audit` | Administração | `audit.view` | sim | — |
| `settings` | `/settings` | Administração | — (qualquer membro) | sim | — |
| `help` | `/help` | Administração | — (qualquer membro) | sim | — |

## Disponibilidade

Nesta fatia, a visibilidade é binária e segue a barra lateral: uma aplicação
aparece no lançador quando o membro a pode abrir (direito institucional presente,
ou módulo relevante), e o Core continua a ser a autoridade. O **Prompt** é
visível a quem tem `ai.use` e **lança mesmo sem fornecedor de inferência** — a
disponibilidade da aplicação é um eixo distinto da do fornecedor (briefing §54).

Os estados de disponibilidade mais ricos (`DEGRADED`, badges de contagem real) e
a fixação por membro são fatias seguintes.
