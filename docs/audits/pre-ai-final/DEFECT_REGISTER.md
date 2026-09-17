# Defect Register — Certificação final pré-IA (registo vivo)

A certificação final trata o Ocinye como **um produto ligado**: intenção do
membro → controlo → navegação → pedido → autorização → persistência → estado →
recarregar → consistência entre módulos. Os defeitos aqui saem de percorrer
jornadas reais no browser, não de testar rotas isoladas.

Severidade: **P0** perda de dados/segurança/outage/bypass · **P1** workflow
central impossível · **P2** defeito funcional/UX material · **P3** menor.

Estado: **OPEN** · **FIXED** (com prova) · **DEFERRED** (adiado com razão) ·
**WONTFIX** (por desenho).

> Regra: a ausência de GPU/modelo só explica a ausência de **inferência real de
> modelo**. Não explica funcionalidade determinista incompleta.

## Contagem — encontrados vs. abertos

| Sev | Encontrados | **Abertos** |
|---|---|---|
| P0 | 0 | **0** |
| P1 | 1 (F-10 Ideia→Projecto) | **0** |
| P2 | 4 (F-11 tabs; F-13 Dataset; F-14 Tarefas; F-16 abrir projecto) | **1** (F-11, F-14, F-16 corrigidos) |
| P3 | 2 (F-12 «Partilhar» morto; F-15 detalhe de Agente) | **2** |

## Registo

| ID | Sev | Domínio | Descrição | Estado |
|---|---|---|---|---|
| F-10 | P1 | Ideias/Projectos | **Ideia → Projecto impossível pelo produto.** Uma ideia nasce em `discovery` e só é promovível em `project_candidate`, mas o Workspace não oferecia **nenhum** controlo para a mover pelos estados (`Discovery→Exploration→Concept→Review→ProjectCandidate`). O Core tinha `transition_idea` (`POST /ideas/{id}/transitions`), nunca chamado pelo Workspace. O botão «Promover a Projecto» aparecia em qualquer ideia e levava a um formulário que o Core recusava. | **FIXED** — o Core expõe `available_transitions`+`promotable` na ideia e `may_transition` no ambiente; o Workspace tem um strip de ciclo de vida (avançar/fechar com razão/promover) e a rota `POST /ideas/{id}/transition`. E2E `idea_to_project_e2e`. |
| F-16 | P2 | Projectos | **Abrir um projecto por URL estava partido.** `GET /api/v1/projects/{id}` (`ProjectView`) não devolvia `workspace_id`, e o handler `project_workspace` redirige por ele — logo abrir `/projects/{id}` (da lista de Projectos, ou o recarregar) caía sem destino. Descoberto pela E2E `idea_to_project_e2e` ao recarregar o projecto. | **FIXED** — `workspace_id` acrescentado ao `ProjectView`. |
| F-11 | P2 | Research Workspace | **Separadores inertes** no ambiente de ideia/projecto — visíveis mas sem navegar (Bibliografia, Fontes, Notas, Documentos, Datasets, Tarefas, Actividade, Histórico). Controlos mortos. | **FIXED** — o conteúdo já se rendia como secções; os separadores passam a âncoras (`#ws-…`) com scroll nativo e realce do activo (`data-oc-section-nav`); os sem ecrã (Código/Planeamento/Financiamento) saem da barra em vez de ficarem inertes. |
| F-12 | P3 | Research Workspace | Botão **«Partilhar»** no detalhe de ideia/projecto é `not_yet_available()` — morto. | **OPEN** |
| F-13 | P2 | Datasets | **Sem página de detalhe de Dataset**: as linhas não ligam a lado nenhum, não há `/datasets/{id}`. Um dataset é uma entidade de domínio (§14), não só um upload. | **OPEN** |
| F-14 | P2 | Tarefas | **Sem lista nem detalhe de Tarefa**, e sem UI para mudar estado/atribuir. Só `/tasks/new`; as tarefas aparecem embutidas e só de leitura. | **FIXED** — as linhas de tarefa do ambiente ligam a `/tasks/{id}`; página de detalhe com transições de estado (a partir de `available_transitions` que o Core deriva de `task_targets_from`) e atribuição de responsável, ambas gated por `may_create`. Novas rotas no Core `GET /tasks/{id}` e `POST /tasks/{id}/assignee`. E2E `task_lifecycle_e2e` (criar→abrir→mudar estado→atribuir→prova em PostgreSQL→recarregar). |
| F-15 | P3 | Agentes | **Sem página de detalhe de Agente** (a lista não liga a cada agente). | **OPEN** |

## Notas

- **F-10** é o defeito que motivou esta pass (relatado manualmente). Corrigido
  primeiro, com E2E dedicada, tratado como P1.
- **F-11/F-14** entregues (separadores vivos; detalhe de Tarefa). **F-13**
  (detalhe de Dataset) é a próxima fatia. Cada uma é uma PR coerente.
- Separadores inertes noutros ecrãs (Compute, IA, Conhecimento, «O Meu Trabalho»)
  e `Adicionar Nó` (Compute) estão a avaliar: alguns são estado **planeado** real
  (o nó inscreve-se sozinho pelo Node Agent, não por formulário), outros degradam
  por ausência de inferência — o que é honesto e permitido. Serão classificados
  um a um antes do portão final.

## Limites honestos desta pass

- **Matriz de browser (§35):** o harness actual (`chromiumoxide`) é **só
  Chromium**. WebKit e Firefox exigiriam um driver diferente (infraestrutura), e
  ficam declarados como lacuna do harness, não do produto.
- **Aceitação autenticada em produção:** é do humano (sem credenciais).
