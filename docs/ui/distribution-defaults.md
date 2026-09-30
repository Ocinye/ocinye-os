# Predefinições de Distribuição · D009

> **A Distribuição define o ponto de partida. O Core decide o que o membro pode fazer.**
> `DEFAULT ≠ AUTHORITY` · `NOT DEFAULT ≠ FORBIDDEN` · `VISIBLE DEFAULT ≠ AUTHORIZED`

Fonte de verdade: `apps/workspace/src/experience/distribution.rs` (tipada, sem JSON, sem caminhos). Repositório observado: `main @ ece29e98d688f366006833b3fe7267e7e1663cc0`.

## Hierarquia (DIST-11/12/13)

```
disposição do membro      member_desktop_layouts (linha = personalizou) · member_app_pins (lista, mesmo vazia = escolha)
  ?? predefinição da Instância   FG-014 — NÃO EXISTE no repositório; nunca é inventada
  ?? predefinição da Distribuição DEFAULTS[d] · DISTRIBUTION_DEFAULTS_VERSION = 2 (D001–D008: 1)
  ?? predefinição do sistema      SystemFallback — sem widgets, fundo ocinye, fixações do manifesto (notes, files, projects)
```

- **Repor** apaga a linha do membro (`POST /me/desktop/restore`, sem mudança): o membro volta a seguir a predefinição efectiva — a da Instância quando existir, senão a da Distribuição, senão a do sistema (`distribution::restore_target`). Pré-visualização (diferenças) + confirmação + desfazer (toast D001) mantêm-se.
- **Actualização do produto:** membro **com** linha: intacto. Membro **sem** linha: passa a ver a versão nova. Instância nova: recebe a versão corrente. Uma predefinição publicada pela Instância (quando existir) ganha sempre.
- **Distribuição desconhecida** (Core silencioso ou valor fora do enum): predefinição do sistema, **nunca Research** (antes: `.unwrap_or(Distribution::Research)`).
- **Marcador de personalização:** hoje só a presença da linha + `version` optimista. `source_template_id`/versão de origem **não** são gravados (G9-04, STORAGE_REQUIRED).

## Estados de aplicação (§25)

| Estado | Onde vive | D009 |
|---|---|---|
| REGISTERED | `ApplicationId::ALL` (28) | sem mudança |
| INSTANCE_ACTIVE | `instance_applications` ?? `InstanceProfile::activates` | sem mudança |
| DISTRIBUTION_DEFAULT_ACTIVE | `InstanceProfile::activates` | sem mudança (ADR-0014 §4) |
| AUTHORIZED | Core (`/me` capabilities + módulos) | sem mudança |
| RECOMMENDED | `DistributionDefaults::recommended` | **novo**, só apresentação |
| PINNED | `member_app_pins` ?? `DistributionDefaults::pins` | **pins por Distribuição** |
| CURRENTLY_OPEN | Gestor de Janelas (D002) | sem mudança |

Uma fixação por omissão passa pelo mesmo `visible_to` da barra: sem autorização ou inactiva **cai**, e a seguinte ocupa o lugar. Terminal, Browser e Nye nunca são fixados por omissão (a Nye é a superfície universal da barra de cima, D003).

## Matriz

### Research (`research`)

- **Ícone:** `dist-research` · **Fundo:** `field (Campo)` · escurecimento 20 %
- **Descrição (canónica, D001):** pt «Predefinições para centros de investigação, laboratórios e equipas de I&D.» · en «Defaults for research centres, laboratories and R&D teams.» · fr «Paramètres par défaut pour les centres de recherche, laboratoires et équipes de R&D.»
- **DEFAULT_ACTIVE_APPS** (`InstanceProfile::activates`, sem alteração): `home`, `work`, `notes`, `calendar`, `trash`, `mail`, `messages`, `files`, `knowledge`, `bibliography`, `units`, `ideas`, `projects`, `datasets`, `results`, `prompt`, `ai`, `agents`, `compute`, `resources`, `activity`, `administration`, `audit`, `monitor`, `settings`, `help`, `terminal`, `browser` (28)
- **DEFAULT_PINNED_APPS = DEFAULT_APP_ORDER** (8): 1. O Meu Trabalho · 2. Projectos · 3. Ideias · 4. Dados · 5. Resultados · 6. Conhecimento · 7. Ficheiros · 8. Notas
- **RECOMMENDED_APPS** (só no painel da Distribuição; não fixadas nem activadas): Bibliografia, Calendário, Mensagens, Computação
- **PRIVILEGED_CONDITIONAL_APPS** (nunca fixadas; no Gestor só com autoridade): Administração, Registo de auditoria, Monitor de Actividade
- **Widgets:** kpis 4×1 · projects 2×1 — linha 1: Indicadores (4 col.) · linha 2: Projectos (2 col.)
- **Primeiros passos:** «Começar com Research» — Esta Instância começa preparada para investigação. As aplicações fixadas para começar estão na barra de aplicações.

### Business (`business`)

- **Ícone:** `dist-business` · **Fundo:** `module (Módulo)` · escurecimento 20 %
- **Descrição (canónica, D001):** pt «Predefinições para empresas e organizações operacionais.» · en «Defaults for companies and operational organisations.» · fr «Paramètres par défaut pour les entreprises et organisations opérationnelles.»
- **DEFAULT_ACTIVE_APPS** (`InstanceProfile::activates`, sem alteração): `home`, `work`, `notes`, `calendar`, `trash`, `mail`, `messages`, `files`, `units`, `projects`, `prompt`, `resources`, `activity`, `administration`, `audit`, `monitor`, `settings`, `help`, `terminal`, `browser` (20)
- **DEFAULT_PINNED_APPS = DEFAULT_APP_ORDER** (7): 1. O Meu Trabalho · 2. Calendário · 3. Correio · 4. Mensagens · 5. Projectos · 6. Ficheiros · 7. Notas
- **RECOMMENDED_APPS** (só no painel da Distribuição; não fixadas nem activadas): Unidades, Actividade, Browser
- **PRIVILEGED_CONDITIONAL_APPS** (nunca fixadas; no Gestor só com autoridade): Administração, Registo de auditoria, Monitor de Actividade
- **Widgets:** tasks 1×2 · calendar 1×2 · projects 2×1 · files 2×1 — col. 1–2: Tarefas, Calendário (2 linhas) · col. 3–4: Projectos / Ficheiros
- **Primeiros passos:** «Começar com Business» — Esta Instância começa preparada para o trabalho de uma organização: tarefas, agenda e comunicação.

### Personal (`personal`)

- **Ícone:** `dist-personal` · **Fundo:** `calm (Calma)` · escurecimento 20 %
- **Descrição (canónica, D001):** pt «Predefinições para ambientes Ocinye individuais.» · en «Defaults for individual Ocinye environments.» · fr «Paramètres par défaut pour les environnements Ocinye individuels.»
- **DEFAULT_ACTIVE_APPS** (`InstanceProfile::activates`, sem alteração): `home`, `work`, `notes`, `calendar`, `trash`, `mail`, `files`, `prompt`, `resources`, `administration`, `monitor`, `settings`, `help`, `terminal`, `browser` (15)
- **DEFAULT_PINNED_APPS = DEFAULT_APP_ORDER** (6): 1. Ficheiros · 2. Notas · 3. Calendário · 4. O Meu Trabalho · 5. Meus Recursos · 6. Lixo
- **RECOMMENDED_APPS** (só no painel da Distribuição; não fixadas nem activadas): Correio, Browser, Ajuda
- **PRIVILEGED_CONDITIONAL_APPS** (nunca fixadas; no Gestor só com autoridade): Administração, Monitor de Actividade
- **Widgets:** notes 2×1 · files 2×1 · calendar 2×1 · storage 1×1 — linha 1: Notas | Ficheiros · linha 2: Calendário | Armazenamento
- **Primeiros passos:** «Começar com Personal» — Esta Instância começa simples: os seus ficheiros, notas e agenda. O que guarda fica no seu espaço pessoal.

### Education (`education`)

- **Ícone:** `dist-education` · **Fundo:** `lattice (Trama)` · escurecimento 20 %
- **Descrição (canónica, D001):** pt «Predefinições para escolas, universidades e ambientes de aprendizagem.» · en «Defaults for schools, universities and learning environments.» · fr «Paramètres par défaut pour les écoles, universités et environnements d’apprentissage.»
- **DEFAULT_ACTIVE_APPS** (`InstanceProfile::activates`, sem alteração): `home`, `work`, `notes`, `calendar`, `trash`, `mail`, `messages`, `files`, `knowledge`, `bibliography`, `units`, `projects`, `prompt`, `resources`, `activity`, `administration`, `audit`, `monitor`, `settings`, `help`, `terminal`, `browser` (22)
- **DEFAULT_PINNED_APPS = DEFAULT_APP_ORDER** (8): 1. O Meu Trabalho · 2. Unidades · 3. Projectos · 4. Conhecimento · 5. Bibliografia · 6. Calendário · 7. Ficheiros · 8. Notas
- **RECOMMENDED_APPS** (só no painel da Distribuição; não fixadas nem activadas): Mensagens, Correio, Actividade
- **PRIVILEGED_CONDITIONAL_APPS** (nunca fixadas; no Gestor só com autoridade): Administração, Registo de auditoria, Monitor de Actividade
- **Widgets:** calendar 1×2 · tasks 1×2 · projects 2×1 · notes 2×1 — col. 1–2: Calendário, Tarefas (2 linhas) · col. 3–4: Projectos / Notas
- **Primeiros passos:** «Começar com Education» — Esta Instância começa preparada para ensino e aprendizagem, com o conhecimento e a bibliografia à mão.


## Widgets (14, registo `KINDS` = `ocinye_contracts::desktop::WIDGET_KINDS`)

| id | dados (Core) | autorização | tamanhos | obrigatório | em predefinições D009 |
|---|---|---|---|---|---|
| kpis | units, workspaces idea/project, datasets (totais) | units.view/ideas.view/projects.view/datasets.view | 4×1 | não | Research |
| notice | — (FG-013: não existe no Core) | — | 2×1 | **não (D009; era sim)** | nenhuma |
| continue | /me/notes + /me/files recentes | do próprio | 2×1, 2×2 | não | nenhuma |
| tasks | /tasks?mine | do próprio + projectos | 1×2, 1×1, 2×2 | não | Business, Education |
| calendar | /calendar/agenda (hoje) | calendar.view | 1×2, 2×1, 2×2 | não | Business, Personal, Education |
| notes | /me/notes | do próprio | 1×1, 2×1 | não | Personal, Education |
| files | /me/files recentes | do próprio | 2×1, 2×2 | não | Business, Personal |
| mail | /mail/status + mailboxes | mail.use + caixa ligada | 1×2, 2×1 | não | nenhuma (depende de caixa ligada) |
| activity | /activity | organisation.view | 1×2, 2×2 | não | nenhuma |
| projects | /workspaces?kind=project&mine | projects.view | 2×1, 1×1 | não | Research, Business, Education |
| ideas | /workspaces?kind=idea (total) | ideas.view | 1×1, 2×1 | não | nenhuma |
| datasets | /datasets (total) | datasets.view | 1×1 | não | nenhuma |
| storage | /me/files (quota) | do próprio | 1×1 | não | Personal |
| health | /compute/status | qualquer membro; ligação ao Monitor só com platform.administer | 1×1, 2×1 | não | nenhuma |

Nenhuma predefinição usa `notice` (sem fonte de dados), `continue`, `mail` (depende de caixa ligada) nem `health`. Um widget cujo pedido o Core recusa (`Denied`) ou cuja aplicação está inactiva (`Inactive`) fica **escondido** (`hidden`, `data-withheld`) e continua na disposição — gravar não o apaga.

## Registo de aplicações (28, reconciliado programaticamente)

| id | rota | pt | en | fr | categoria | ícone actual (app_icon) | fonte | gate | janela | classe | activa R/B/P/E | ícone D009 | auditoria |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `home` | `/` | Home | Home | Accueil | Productivity | `home` | `static/icons.svg` | — | Single | Essential | ✓✓✓✓ | `home` | KEEP |
| `work` | `/my-work` | O Meu Trabalho | My Work | Mon travail | Productivity | `work` | `static/icons.svg` | — | Single | Essential | ✓✓✓✓ | `work` | KEEP |
| `notes` | `/notes` | Notas | Notes | Notes | Productivity | `notes` | `static/icons.svg` | — | Multi | Optional | ✓✓✓✓ | `notes` | KEEP |
| `calendar` | `/calendar` | Calendário | Calendar | Calendrier | Productivity | `calendar` | `static/icons.svg` | calendar.view | Single | Optional | ✓✓✓✓ | `calendar` | KEEP |
| `trash` | `/trash` | Lixo | Trash | Corbeille | Productivity | `trash` | `static/icons.svg` | — | Single | Essential | ✓✓✓✓ | `trash` | KEEP |
| `mail` | `/mail` | Correio | Mail | Courrier | Communication | `mail` | `static/icons.svg` | mail.use | Single | Optional | ✓✓✓✓ | `mail` | KEEP |
| `messages` | `/messages` | Mensagens | Messages | Messages | Communication | `messages` | `static/icons.svg` | messaging.use | Single | Optional | ✓✓·✓ | `messages` | KEEP |
| `files` | `/files` | Ficheiros | Files | Fichiers | Knowledge | `files` | `static/icons.svg` | módulo files | Multi | Essential | ✓✓✓✓ | `files-app` | REFINE · DUPLICATE_SEMANTICS (= folha de Notas) |
| `knowledge` | `/knowledge` | Conhecimento | Knowledge | Connaissance | Knowledge | `knowledge` | `static/icons.svg` | módulo knowledge + bibliography.view | Single | Optional | ✓··✓ | `knowledge` | KEEP |
| `bibliography` | `/bibliography` | Bibliografia | Bibliography | Bibliographie | Knowledge | `bibliography` | `static/icons.svg` | módulo bibliography + bibliography.view | Single | Optional | ✓··✓ | `bibliography` | KEEP |
| `units` | `/units` | Unidades | Units | Unités | Research | `units` | `static/icons.svg` | units.view | Single | Optional | ✓✓·✓ | `org-tree` | REPLACE · DUPLICATE_SEMANTICS (= grelha Indicadores) |
| `ideas` | `/ideas` | Ideias | Ideas | Idées | Research | `idea` | `static/icons.svg` | ideas.view | Single | Optional | ✓··· | `idea` | KEEP |
| `projects` | `/projects` | Projectos | Projects | Projets | Research | `project` | `static/icons.svg` | projects.view | Single | Optional | ✓✓·✓ | `project` | KEEP |
| `datasets` | `/datasets` | Dados | Data | Données | Research | `data` | `static/icons.svg` | módulo datasets + datasets.view | Single | Optional | ✓··· | `data` | KEEP |
| `results` | `/results` | Resultados | Results | Résultats | Research | `results` | `static/icons.svg` | projects.view | Single | Optional | ✓··· | `results` | KEEP |
| `prompt` | `/ai/prompt` | Nye | Nye | Nye | Research | `nye` | `static/icons.svg` | ai.use | Single | Optional | ✓✓✓✓ | `nye` | KEEP (excepção de cor da marca) |
| `ai` | `/ai` | Ocinye AI | Ocinye AI | Ocinye AI | Research | `ai` | `static/icons.svg` | ai.use | Single | Optional | ✓··· | `ai-fabric` | REPLACE · DUPLICATE_SEMANTICS (hexágono = Nye) |
| `agents` | `/ai/agents` | Agentes | Agents | Agents | Research | `agent` | `static/icons.svg` | agents.view | Single | Optional | ✓··· | `agents` | REPLACE · WRONG_SEMANTICS (robô = chatbot) |
| `compute` | `/compute` | Computação | Compute | Calcul | Research | `compute` | `static/icons.svg` | compute.view | Single | Optional | ✓··· | `compute` | KEEP |
| `resources` | `/resources` | Meus Recursos | My Resources | Mes ressources | Administration | `workspace` | `static/icons.svg` | — | Single | Essential | ✓✓✓✓ | `resources` | REPLACE · WRONG_SEMANTICS (camadas ≠ quota) |
| `activity` | `/activity` | Actividade | Activity | Activité | Administration | `activity` | `static/icons.svg` | organisation.view | Single | Optional | ✓✓·✓ | `activity-feed` | REPLACE · WRONG_SEMANTICS (pulso = telemetria) |
| `administration` | `/admin` | Administração | Administration | Administration | Administration | `admin` | `static/icons.svg` | members.manage | Single | Essential | ✓✓✓✓ | `administration` | REPLACE · DUPLICATE_SEMANTICS (raios = sol/definições) |
| `audit` | `/audit` | Registo de auditoria | Audit log | Journal d’audit | Administration | `shield` | `static/icons.svg` | audit.view | Single | Optional | ✓✓·✓ | `audit-record` | REPLACE · WRONG_SEMANTICS (escudo = segurança) |
| `monitor` | `/admin/monitor` | Monitor de Actividade | Activity Monitor | Moniteur d’activité | Administration | `gauge` | `static/icons.svg` | platform.administer | Single | Essential | ✓✓✓✓ | `gauge` | KEEP |
| `settings` | `/settings` | Definições | Settings | Paramètres | Administration | `settings` | `static/icons.svg` | — | Single | Essential | ✓✓✓✓ | `gear` | REFINE · INCONSISTENT_WEIGHT (dois círculos) |
| `help` | `/help` | Ajuda | Help | Aide | Administration | `help` | `static/icons.svg` | — | Single | Essential | ✓✓✓✓ | `help` | KEEP |
| `terminal` | `/terminal` | Terminal | Terminal | Terminal | System | `terminal` | `static/icons.svg` | — (cada comando pelo Core) | Single | Optional | ✓✓✓✓ | `terminal` | KEEP |
| `browser` | `/browser` | Browser | Browser | Browser | System | `browser` | `static/icons.svg` | — | Single | Optional | ✓✓✓✓ | `browser-window` | REFINE · DUPLICATE_SEMANTICS (globo = Idioma) |

## Primeiros passos (FIRST-09)

Sem assistente e sem estado gravado (não há contrato de persistência). O **painel da Distribuição** (distintivo na barra de cima) mostra: descrição, primeiros passos, as fixações por omissão **visíveis a este membro**, recomendadas, «Abrir aplicações» e a frase «A Distribuição define o ponto de partida…». Nada abre sozinho. Um Desktop vazio não mostra cartão nenhum (decisão do Fidel, 30 set 2026): o lápis no canto superior direito basta.
