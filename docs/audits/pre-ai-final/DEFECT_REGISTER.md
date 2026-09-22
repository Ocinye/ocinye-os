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
| P1 | 3 (F-10 Ideia→Projecto; F-17 abrir da lista; F-18 «Actividade» dava 500/502) | **0** |
| P2 | 4 (F-11 tabs; F-13 Dataset; F-14 Tarefas; F-16 abrir projecto) | **0** (F-11, F-13, F-14, F-16 corrigidos) |
| P3 | 2 (F-12 «Partilhar» morto; F-15 detalhe de Agente) | **0** (F-15 corrigido; F-12 já não se reproduz) |

## Registo

| ID | Sev | Domínio | Descrição | Estado |
|---|---|---|---|---|
| F-10 | P1 | Ideias/Projectos | **Ideia → Projecto impossível pelo produto.** Uma ideia nasce em `discovery` e só é promovível em `project_candidate`, mas o Workspace não oferecia **nenhum** controlo para a mover pelos estados (`Discovery→Exploration→Concept→Review→ProjectCandidate`). O Core tinha `transition_idea` (`POST /ideas/{id}/transitions`), nunca chamado pelo Workspace. O botão «Promover a Projecto» aparecia em qualquer ideia e levava a um formulário que o Core recusava. | **FIXED** — o Core expõe `available_transitions`+`promotable` na ideia e `may_transition` no ambiente; o Workspace tem um strip de ciclo de vida (avançar/fechar com razão/promover) e a rota `POST /ideas/{id}/transition`. E2E `idea_to_project_e2e`. |
| F-16 | P2 | Projectos | **Abrir um projecto por URL estava partido.** `GET /api/v1/projects/{id}` (`ProjectView`) não devolvia `workspace_id`, e o handler `project_workspace` redirige por ele — logo abrir `/projects/{id}` (da lista de Projectos, ou o recarregar) caía sem destino. Descoberto pela E2E `idea_to_project_e2e` ao recarregar o projecto. | **FIXED** — `workspace_id` acrescentado ao `ProjectView`. |
| F-11 | P2 | Research Workspace | **Separadores inertes** no ambiente de ideia/projecto — visíveis mas sem navegar (Bibliografia, Fontes, Notas, Documentos, Datasets, Tarefas, Actividade, Histórico). Controlos mortos. | **FIXED** — o conteúdo já se rendia como secções; os separadores passam a âncoras (`#ws-…`) com scroll nativo e realce do activo (`data-oc-section-nav`); os sem ecrã (Código/Planeamento/Financiamento) saem da barra em vez de ficarem inertes. |
| F-12 | P3 | Research Workspace | Botão **«Partilhar»** no detalhe de ideia/projecto é `not_yet_available()` — morto. | **RESOLVIDO (não reproduz)** — o detalhe de ideia/projecto foi reestruturado na F-11 e já não renderiza nenhum botão «Partilhar» nem qualquer controlo `not_yet_available()`. Provado pelo teste de ecrã `nenhum_separador_do_ambiente_e_morto`, que afirma que o ambiente não contém a marca «Ainda não disponível». Partilha de ideia/projecto não existe no Core e não se inventa por causa de um botão que já não existe. |
| F-13 | P2 | Datasets | **Sem página de detalhe de Dataset**: as linhas não ligam a lado nenhum, não há `/datasets/{id}`. Um dataset é uma entidade de domínio (§14), não só um upload. | **FIXED** — as linhas da lista de Datasets ligam a `/datasets/{id}`; página de detalhe com a governança (código, classificação, estado, origem, licença, restrições de uso, palavras-chave) e as versões (rótulo, estado, ficheiros, tamanho, proveniência). Nova rota no Core `GET /datasets/{id}` (`data::get_dataset` reautoriza pela posse e pela classificação do próprio dataset). E2E `dataset_detail_e2e`. |
| F-14 | P2 | Tarefas | **Sem lista nem detalhe de Tarefa**, e sem UI para mudar estado/atribuir. Só `/tasks/new`; as tarefas aparecem embutidas e só de leitura. | **FIXED** — as linhas de tarefa do ambiente ligam a `/tasks/{id}`; página de detalhe com transições de estado (a partir de `available_transitions` que o Core deriva de `task_targets_from`) e atribuição de responsável, ambas gated por `may_create`. Novas rotas no Core `GET /tasks/{id}` e `POST /tasks/{id}/assignee`. E2E `task_lifecycle_e2e` (criar→abrir→mudar estado→atribuir→prova em PostgreSQL→recarregar). |
| F-15 | P3 | Agentes | **Sem página de detalhe de Agente** (a lista não liga a cada agente). | **FIXED** — as linhas da lista de Agentes ligam a `/ai/agents/{id}`; página de detalhe com a definição (capacidade, âmbito, tecto de classificação, fontes, instruções, autor) e o **estado real derivado** — configurado, com a explicação de que corre quando existir capacidade. Nova rota no Core `GET /ai/agents/{id}` (`intelligence::agents::get` decide a visibilidade em SQL, como a lista). E2E `agent_detail_e2e`. |
| F-18 | P1 | Actividade | **«Actividade» dava 502 (o Core devolvia 500).** A `activity_entries` ganhou linhas **owner-scoped** (nota pessoal: `workspace_id IS NULL`, migração 0031/0038), mas o feed institucional lia `workspace_id` para um `Uuid` **não-opcional** — e descodificar `NULL`→`Uuid` falha (erro de base em 3 ms). Só se manifestava com dados reais e visibilidade larga (o `PlatformAdmin` alcança essas linhas; `page_size=100` puxa-as); a base fresca do CI nunca tinha uma. Diagnosticado pelos logs de produção (`GET /api/v1/activity` → 500 «database error»). | **FIXED** — `ActivityEntry.workspace_id`/`ActivityView.workspace_id` passam a `Option<Uuid>` (a verdade do esquema), e o feed institucional filtra `WHERE workspace_id IS NOT NULL` — a actividade owner-scoped é privada ao dono, tem o seu próprio painel, e não entra no feed partilhado. Teste `o_feed_de_actividade_ignora_actividade_owner_scoped_e_nao_rebenta` (com prova por reversão). |
| F-20 | P2 | Ficheiros | **Carregar não mostrava progresso e era um ficheiro de cada vez.** Escolher um ficheiro submetia a página inteira, sem qualquer sinal do que subia, e o campo pessoal não aceitava múltiplos. Encontrado na aceitação autenticada em produção. | **FIXED** — janela de progresso ao estilo Google Drive (`static/app.js` + `.oc-up` em `ocinye.css`): cada ficheiro sobe no seu `XMLHttpRequest` para `/files/upload` (que passa a responder em JSON via `Accept`, `aceita_json`), com barra, percentagem, estado e cancelamento; o campo pessoal ganha `multiple` e sobe todos em paralelo. Melhoria progressiva — sem JS o formulário submete-se por inteiro. Teste `so_pede_json_quem_o_aceita`; a janela verifica-se na aceitação. |
| F-21 | P3 | Recursos | **«Disponível» lia-se igual ao limite.** Com ~35 MiB usados num limite de 10 GiB, o disponível (9,9655 GiB) arredondava a uma casa decimal para «10,0 GiB» — e então «em uso 35 MiB» convivia com «disponível 10 GiB», que se lê como contradição. A conta no Core estava certa (`disponível = limite − uso`); enganava o arredondamento. | **FIXED** — o disponível arredonda para baixo (9,9 GiB) e o uso para cima, para nunca colapsarem no valor do limite (`resources.rs`, `human_bytes_com`). Teste `o_disponivel_nunca_arredonda_para_o_limite_inteiro`, com prova por reversão. |
| F-19 | P2 | Correio | **O destinatário era confirmado antes de estar completo.** No compositor, o texto por resolver do campo «Para» virava ficha assim que o campo perdia o foco: um fragmento de pesquisa (`fidel`) tornava-se um destinatário `fidel`, que não é um endereço entregável. Encontrado na aceitação autenticada em produção. | **FIXED** — um destinatário passa a ser só uma sugestão escolhida ou um endereço escrito por inteiro (`static/app.js`): perder o foco ou submeter aceita apenas o que parece uma morada; um fragmento fica no campo. Enter/vírgula continuam a completar pela sugestão em foco. Camada de melhoria progressiva sem base a testar — verificado na aceitação em produção. |
| F-17 | P1 | Ideias/Projectos | **Clicar numa Ideia (ou Projecto) na lista dava «Página não encontrada».** As listas de Ideias e Projectos são servidas por `/api/v1/workspaces?kind=…`, pelo que o `id` de cada linha é o do **Research Workspace**, não o da ideia/projecto. A linha ligava a `/ideas/{id}` (e `/projects/{id}`), dando esse id de ambiente a uma rota que procura uma ideia/projecto com esse id — que não existe — e caía em 404. A Home já ligava bem (`/workspaces/{id}`); só as listas estavam erradas. Encontrado na aceitação autenticada em produção. O `idea_to_project_e2e` não o apanhava porque nunca abria pela lista (criava e era logo redirigido para o ambiente). | **FIXED** — as linhas de Ideia e Projecto ligam a `/workspaces/{id}`, como a Home. Guarda de ecrã `a_linha_de_ideia_ou_projecto_liga_ao_ambiente` e E2E `clicar_numa_ideia_na_lista_leva_ao_ambiente`. |

## Notas

- **F-10** é o defeito que motivou esta pass (relatado manualmente). Corrigido
  primeiro, com E2E dedicada, tratado como P1.
- **Todos os defeitos desta pass estão fechados.** F-10 (P1), F-11/F-13/F-14/F-16
  (P2) e F-15 (P3) corrigidos, cada um com prova; F-12 (P3) já não se reproduz.
  Cada fatia é uma PR coerente (F-10/F-11/F-16 em #120; F-14 em #121; F-13 em
  #122; F-15 na sua própria PR).
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
