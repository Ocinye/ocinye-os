# ADR-0413 — Notas: conhecimento institucional editável, versionado e partilhável

- **Estado:** Accepted
- **Domínio:** Knowledge
- **Impacto:** HIGH
- **Data:** 2026-09-10
- **Relaciona-se com:** [ADR-0006](0006-modular-monolith.md) ·
  [ADR-0100](0100-authorization-model.md) ·
  [ADR-0101](0101-permissions-scopes-and-grants.md) ·
  [ADR-0200](0200-object-storage.md) ·
  [ADR-0202](0202-search-fts-pgvector.md) ·
  [ADR-0204](0204-institutional-files-and-folders.md) ·
  [ADR-0402](0402-mail-html-sanitisation.md) ·
  [ADR-0411](0411-execution-time-principal-freshness.md) ·
  [ADR-0600](0600-leptos-workspace-runtime.md)

## Context

O Ocinye OS vai ter um módulo nativo de **Notas** — escrita rápida e
conhecimento colaborativo — antes de ligar qualquer nó de IA. A Experience tem
de parecer simples (abrir → escrever → autosave → organizar → encontrar →
partilhar), mas por baixo cada nota é um objecto institucional com identidade,
autoria, versões, permissões e classificação. A simplicidade vive na Experience;
autoridade, proveniência, segurança e memória vivem no Core.

O repositório **já tem** metade disto. Uma auditoria a 2026-09-10 confirmou:
existem as tabelas `notes` e `note_revisions` (migração `0003`), o modelo `Note`
e as operações `create_note`/`update_note`/`list_notes` no módulo `knowledge`,
com *snapshot* imutável a cada edição; a autorização por classificação e o
`VisibilityFilter`; a indexação em `search_documents` com `entity_type = "note"`;
o higienizador de HTML por lista de permissões (`ammonia`, ADR-0402); os
`files`/`file_versions`/`storage_objects` com pré-visualização servida pelo Core,
same-origin, sob a CSP actual; os eventos de outbox `note.created`/`note.updated`;
a auditoria e a actividade.

O que **não** existe: um editor, um corpo rico (o `body` é `TEXT` simples), a
leitura de uma nota só e o seu histórico por HTTP, a partilha por membro, o
controlo de conflitos, as pastas, o lixo, e a dimensão **pessoal** — hoje uma
nota é sempre um artefacto de um Research Workspace, e o módulo pedido é do foro
**PESSOAL** (dono = o membro).

Este ADR decide o domínio e as fronteiras. Não decide a biblioteca de editor
concreta — isso é da primeira fatia, sob os critérios abaixo.

## Decision

### 1. Uma nota é conhecimento pessoal do membro, e a `notes` é a primitiva única

Não se cria uma segunda tabela de notas. A `notes` passa a servir os dois casos
sem os confundir: ganha um **dono** (`owner_id`) e o `workspace_id`/`unit_id`
tornam-se **opcionais**. Uma nota pessoal tem dono e nenhum workspace; uma nota
de investigação continua ligada ao seu ambiente. A autoridade unifica-se numa
regra só:

> **Acesso efectivo a uma nota = actor ∩ classificação ∩ (é o dono ∨ tem uma
> partilha viva ∨ — quando a nota é de um workspace — pertence ao workspace).**

Nunca uma união que alargue: a classificação é sempre um tecto, e uma partilha
não a fura (§4). É a mesma forma da intersecção do plano agentic (`CLAUDE.md`
§8).

### 2. O corpo canónico é HTML higienizado; o texto simples deriva-se

O `body` passa a guardar **HTML higienizado** pela lista de permissões da
Ocinye — um perfil de Notas irmão do do correio (ADR-0402): os mesmos elementos
seguros (parágrafos, títulos, listas, *checklists*, tabelas, citações, código,
ligações, imagens), **sem** `script`, `iframe`, `svg`, `style`, `form`, nem
esquemas `javascript:`/`data:` em ligações. O texto simples para pesquisa e para
o excerto **deriva-se** do HTML, e é ele que vai ao índice — nunca marcação.

Preferiu-se HTML higienizado a um modelo estruturado (AST/JSON de editor) porque
não existe nenhum modelo estruturado no sistema, introduzi-lo traria uma
dependência e um esquema que a stack SSR não precisa, e o higienizador maduro já
existe e é a defesa. É a decisão que a §19 do pedido pede que se documente. Se um
dia um modelo estruturado se justificar (edição concorrente fina, âncoras
estáveis para IA), é um ADR novo que substitui esta parte — não uma reescrita
silenciosa.

### 3. O editor é vendorizado, same-origin, e melhora o progressivo

A CSP do Workspace é `script-src 'self'` sem `unsafe-inline` nem `unsafe-eval`
(ADR-0600, ADR-0019). O editor é uma biblioteca **vendorizada em `static/`** e
servida same-origin, inicializada a partir do `app.js` — nunca de um CDN, nunca
de `&lt;script&gt;` embutido, nunca de um segundo *framework* (sem React/Vue). Sobre
`contenteditable`, produz HTML que o **Core higieniza no save** — o cliente
nunca é a autoridade sobre o que se guarda. Sem JavaScript, a nota **lê-se**
(HTML higienizado, renderizado pelo servidor) e edita-se por uma área de texto
simples que grava texto: degradado, mas funcional, como manda a doutrina de
melhoria progressiva.

Critérios para a biblioteca (escolhida na fatia A, e registada quando o for):
mantida, licença permissiva, superfície pequena e controlada, sem dependência de
nuvem nem telemetria, sem `eval`/`new Function` (não passaria a CSP), modelo de
documento e acessibilidade de teclado, e integrável com SSR sem tomar conta da
página.

### 4. Partilha é uma tabela própria de papéis por membro, ligada à política

Cria-se `note_shares (note_id, subject_id, role, granted_by_id, granted_at,
revoked_at)`, com `role ∈ {viewer, editor}`. Não se sobrecarrega o
`explicit_access_grants` — que é de permissão única e **ainda não é consultado**
pela política — porque a partilha de uma nota é por pessoa e por papel, e tem de
entrar na decisão de leitura e de escrita. A política de notas passa a consultar
`note_shares`: um *viewer* lê, um *editor* lê e escreve, sempre **∩
classificação**. Revogar uma partilha tem efeito na operação seguinte, resolvida
à fonte (ADR-0411) — mesmo que o colaborador tenha a nota aberta.

Não há partilha pública nem «qualquer pessoa com a ligação» na v1. Só membros
autenticados da Ocinye, e colaboradores externos só dentro das políticas que já
existem.

### 5. O save é uma troca condicionada pela revisão base (sem clobber silencioso)

`update_note` deixa de ser uma escrita cega. Passa a receber a **revisão base**
que o editor tinha, e a escrita é condicional: `UPDATE … WHERE id = $1 AND
revision = $base`; zero linhas afectadas → `CoreError::Conflict`, e a Experience
mostra que a nota mudou noutro sítio e oferece resolver **sem perder nenhum dos
lados**. É o idioma de reivindicação condicional que o plano agentic já usa,
aplicado ao conteúdo. Não é CRDT — e é deliberado: partilha, edição por vários,
revisões, notificação e conflito honesto dão um editor colaborativo robusto sem
cursores múltiplos, que ficam para quando houver necessidade real de várias
pessoas a digitar na mesma nota ao mesmo tempo.

### 6. As revisões são imutáveis e sabem quem as escreveu; restaurar cria uma nova

`note_revisions` passa a preencher `authored_by_id` (hoje fica nulo). Uma revisão
antiga nunca se altera. Restaurar uma versão **cria uma revisão nova** com o
conteúdo da antiga — não apaga as posteriores. O histórico é a memória de quem
mudou o quê, e daqui a anos ainda o diz.

### 7. Organização: pastas do dono, etiquetas, e lixo

`note_folders` (do dono) organiza; `notes.folder_id` é opcional; a pasta **não**
concede autoridade por si. As etiquetas (`tags`, que já existem) filtram. O
apagar é **soft**: `notes.deleted_at` leva ao Lixo, de onde se restaura ou se
elimina definitivamente — e a eliminação definitiva respeita política, auditoria
e as referências (uma imagem partilhada por outro recurso não desaparece só
porque a nota desapareceu).

### 8. Imagens e anexos são Files; o corpo referencia a versão exacta

Colar uma imagem envia os bytes **pelo Core** (`files::create`) → Object Storage
→ `File`+`FileVersion`, e o corpo referencia a `FileVersion` exacta, servida de
volta pela pré-visualização same-origin do Core (`/file-versions/{id}/preview`).
Nunca `base64` permanente no corpo, nunca uma chave de object-store na Experience,
nunca uma URL pública. Abrir a imagem reautoriza. Anexos (PDF, documento, …)
seguem o mesmo subsistema de Files, com o cartão a mostrar nome, tipo, tamanho e
versão, e a abertura governada pelo Core.

### 9. Notificação em tempo real é planeada, não inventada agora

Avisar «alguém acabou de actualizar esta nota» exige um canal e um evento novos
(o `realtime` de hoje só conhece conversas e pessoas). Fica para a fatia de
histórico/actividade, e até lá a Experience não finge tempo real. CRDT não entra.

## Alternatives

- **Uma tabela `personal_notes` separada.** Duplicaria a primitiva de notas e as
  suas revisões, contra o princípio de não duplicar. Recusada.
- **Modelo de documento estruturado (ProseMirror/portable-text) como canónico.**
  Mais poder para edição fina e para a IA ancorar, mas uma dependência e um
  esquema novos que a stack não precisa hoje, e sem o higienizador maduro que já
  temos. Adiada para quando houver necessidade real; substituirá a §2 por ADR.
- **Partilha por `explicit_access_grants`.** Existe e tem âmbito `resource`, mas
  é permissão única, não está ligada à decisão da política, e não exprime
  *viewer*/*editor* por pessoa. `note_shares` é mais claro e é o que a leitura e a
  escrita precisam de consultar.
- **`last-write-wins`.** Simples e errado: perde trabalho em silêncio. A troca
  condicionada pela revisão custa uma coluna que já existe.
- **CRDT / cursores múltiplos na v1.** Fora de âmbito por decisão de produto.

## Consequences

- **Migração nova** relaxa `notes.workspace_id`/`unit_id` para opcionais e
  acrescenta `owner_id`, `folder_id`, `deleted_at`; cria `note_folders` e
  `note_shares`; preenche `authored_by_id` nas revisões novas. Migração aditiva,
  reproduzível de base vazia, com decisão de continuidade para cada tabela nova
  (o portão de `continuity` fecha sem ela).
- **A autorização de notas ganha a partilha** como fonte adicional, sempre ∩
  classificação, e a política passa a ser testada com *viewer*/*editor*, com o
  `PlatformAdmin` a **não** ganhar leitura de notas privadas, e a identidade
  privilegiada a **não** herdar as notas da identidade humana ligada.
- **A pesquisa** indexa a projecção de texto simples da revisão corrente, filtrada
  por autorização; revogar uma partilha tira a nota da pesquisa do ex-colaborador.
- **O backup institucional** já cobre tudo isto: metadados, revisões, partilhas,
  pastas e etiquetas viajam no PostgreSQL; imagens e anexos no Object Storage. Não
  se cria um segundo mecanismo; prova-se uma nota a sobreviver a um restauro.
- **Sem IA**, o módulo funciona por inteiro. Quando a IA chegar, trabalha sobre
  uma `NoteRevision` exacta, autorizada e auditável, e qualquer mutação sua passa
  por compreender → autorizar → planear → confirmar → autorizar à execução →
  executar → verificar → auditar. A partilha continua uma operação do Core: um
  agente nunca alarga acesso só porque o texto diz «partilha com todos».

## Fatias (cada uma deixa o produto coerente)

- **A** — domínio (dono, corpo HigienizadoHTML, revisão com autor, GET nota +
  histórico, save condicionado por revisão) + editor básico + autosave + entrada
  «Notas» na navegação PESSOAL.
- **B** — conteúdo rico + colar imagens + anexos (via Files/FileVersion).
- **C** — pastas + etiquetas + pesquisa (lexical, filtrada, revisão corrente).
- **D** — partilha + papéis *viewer*/*editor* + controlo de conflitos (vertical:
  nunca meia-partilha).
- **E** — histórico/restauro + lixo/restauro + actividade + notificação realtime.
- **F** — viagens de browser E2E + reversões de segurança (XSS/IDOR/classificação/
  PlatformAdmin/autoridade stale/identidade ligada) + acessibilidade.
- **G** — integração de produção + prova de backup/restauro + entrada na
  certificação do sistema inteiro.

Só se declara `OCINYE_NOTES_READY` quando toda a Definition of Done (#130 do
pedido) se verifica, e `OCINYE_STABLE_PRE_AI_READY` continua retido até Notas e
todos os outros módulos estarem certificados.
