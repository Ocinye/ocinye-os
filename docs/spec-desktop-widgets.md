# Desktop — contrato dos widgets (Continuar trabalho, Estado do sistema)

Tipos em `apps/workspace/src/ui/view_models.rs`; vista em `ui/screens/home/mod.rs`; chaves em `i18n/ui_shell.rs`. Tabelas dos campos: `docs/ui/HANDOFF.md` · «11a».

## 1. Continuar trabalho (`WidgetContent::Continue`)

### Itens
- Fonte: últimos objectos **abertos ou editados pelo próprio membro** (eventos `open` e `write` do registo de actividade). A actividade de outros membros não conta.
- Tipos (`ContinueKind`): `idea`, `project`, `file`, `note`, `dataset`. Excluídos: tarefas (têm widget próprio), correio, mensagens, eventos.
- Âmbito: tudo o que o membro alcança (todas as suas unidades e o Espaço pessoal). O Desktop não tem espaço activo (CLAUDE.md §34.3). No Espaço pessoal só há `file` e `note`.
- Um item por objecto, ordenado por último toque, do mais recente para o mais antigo.
- Excluir: apagados, na reciclagem, sem permissão actual, toques com mais de 30 dias.
- Quantidade: o BFF devolve até **7**; o widget mostra 3 (2×1) ou 7 (2×2).

### Meta (composta na vista)
`{TIPO} · {tempo}`; projectos: `{TIPO} · {progresso}% · {tempo}`. Ex.: `IDEIA · 2 h`, `PROJECTO · 65% · 5 h`, `FICHEIRO · 22/09`.

| chave | pt | en | fr |
|---|---|---|---|
| `desk.cont.type.idea` | IDEIA | IDEA | IDÉE |
| `desk.cont.type.project` | PROJECTO | PROJECT | PROJET |
| `desk.cont.type.file` | FICHEIRO | FILE | FICHIER |
| `desk.cont.type.note` | NOTA | NOTE | NOTE |
| `desk.cont.type.dataset` | DATASET | DATASET | JEU DE DONNÉES |
| `desk.cont.empty` | Ainda não há trabalho recente. | No recent work yet. | Pas encore de travail récent. |
| `desk.cont.progress_sr` (leitor de ecrã) | {pct} % das tarefas do projecto concluídas | {pct}% of the project’s tasks done | {pct} % des tâches du projet terminées |

Tempo curto (`Ago`, desde o último toque do membro):

| `Ago` | chave | pt | en | fr |
|---|---|---|---|---|
| `Now` (< 1 min) | `time.short.now` | agora | now | à l’instant |
| `Minutes(n)` | `time.short.min` | {n} min | {n} min | {n} min |
| `Hours(n)` | `time.short.hour` | {n} h | {n} h | {n} h |
| `Days(n)` (< 7 d) | `time.short.day` | {n} d | {n} d | {n} j |
| `Date(s)` (≥ 7 d) | — | dd/mm | dd/mm | dd/mm |

Progresso (só `project`): `round(100 × tarefas concluídas / tarefas do projecto)`, sem contar as canceladas. Sem tarefas: `None`, e a meta fica `PROJECTO · 5 h`.

### Destino (`href`)
| tipo | abre |
|---|---|
| idea | Investigação › Ideias, ficha da ideia |
| project | Projectos, ficha do projecto |
| file | Ficheiros, pasta do ficheiro com o ficheiro seleccionado e pré-visualização |
| note | Notas, nota aberta em edição |
| dataset | Datasets, ficha do dataset |

Se a aplicação não estiver no perfil: pré-visualização em janela (G-05, entrega 11d).

### Estados
Carregar (`Loading`, esqueleto), vazio (`desk.cont.empty`), erro (`Failed`, com referência), indisponível (`Unavailable`: registo de actividade desligado), sem permissão (`Denied`).

## 2. Estado do sistema (`WidgetContent::Health`)

Desenho: `{ESTADO}` em grande, com ponto de cor; por baixo `Core · {nós} · {cópia}`.

- Nós: os nós de computação (GPU/CPU), com o estado derivado do batimento. Sem nós registados (`nodes_total = 0`) o segmento não aparece: `Core · cópia 03:00`.
- Horas e datas da cópia: na zona do membro (a do browser), como nos outros widgets.

Estado (`Health`, via `HealthVm::derive_state`): o pior entre nós e cópia.
| `Health` | chave | pt | en | fr | regra |
|---|---|---|---|---|---|
| Operational | `health.state.ok` | OK | OK | OK | Core responde, todos os nós activos (ou nenhum registado) e cópia com êxito há < 24 h |
| Degraded | `health.state.degraded` | DEGRADADO | DEGRADED | DÉGRADÉ | ≥ 1 nó em baixo, ou última cópia falhou / tem > 24 h |
| Unavailable | `health.state.down` | INDISPONÍVEL | DOWN | INDISPONIBLE | Core sem acesso à base de dados |

| chave | pt | en | fr |
|---|---|---|---|
| `health.core` | Core | Core | Core |
| `health.nodes.all.one/.other` | {count} nó / nós | {count} node / nodes | {count} nœud / nœuds |
| `health.nodes.partial` | {up}/{total} nós | {up}/{total} nodes | {up}/{total} nœuds |
| `health.backup.today` | cópia {time} | backup {time} | sauvegarde {time} |
| `health.backup.yesterday` | cópia ontem {time} | backup yesterday {time} | sauvegarde hier {time} |
| `health.backup.date` | cópia {date} | backup {date} | sauvegarde {date} |
| `health.backup.failed` | cópia falhou | backup failed | échec de sauvegarde |
| `health.backup.none` | sem cópia | no backup | aucune sauvegarde |
| `health.backup.unknown` (D001.1: the Core has no record) | cópia sem registo | backup not recorded | sauvegarde non enregistrée |
| `health.open` | Abrir o Monitor | Open Monitor | Ouvrir le Moniteur |

Visível a todos os membros. Administrador: `admin_href` preenchido, e o widget é um link para o Monitor. Membro: `None`, e o widget não é clicável.

## 3. Gestão de avisos (Administração)
Entrega 11b.
