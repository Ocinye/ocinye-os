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

> **Emenda 2026-09-10 (§2).** A primeira versão deste ADR fixou o corpo canónico
> como **HTML higienizado**. Corrigiu-se, no mesmo dia e **antes de qualquer
> implementação depender disso** (só a migração inicial existia), para um
> **documento estruturado e versionado** como fonte de verdade, com o HTML e o
> texto simples derivados dele. A razão é a robustez do que vem a seguir —
> checklists, tabelas, imagens, referências a `FileVersion`, código com
> metadados, conflitos e IA futura ganham uma fundação semântica, e uma revisão
> histórica deixa de depender da interpretação de HTML arbitrário. A decisão
> antiga fica registada aqui, e não se apaga; a §2 abaixo é a que vale.

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

### 2. O corpo canónico é um documento estruturado e versionado; o resto deriva-se

A fonte de verdade de uma nota é um **documento estruturado** — um modelo de
blocos em JSON, com um `schema_version` gravado em cada revisão. Um bloco tem
tipo (parágrafo, título, lista, lista ordenada, *checklist*, bloco de código e,
reservados para as fatias seguintes sem redesenho, imagem, anexo e tabela), e o
texto em linha é uma sequência de trechos com marcas (negrito, itálico, código)
e ligações. As referências a imagens e anexos são **explícitas** — apontam a
`FileVersion` exacta, e não bytes embutidos.

Dele **derivam-se**, e nunca o contrário:

- o **HTML** renderizado, higienizado na fronteira de render (com a lista de
  permissões irmã da do correio, ADR-0402) — para a leitura, incluindo sem
  JavaScript;
- o **texto simples**, para o excerto;
- o **texto de pesquisa** que vai ao índice lexical.

O HTML é higienizado também na **fronteira de colar/importar**: o que entra do
editor ou de uma colagem é convertido para o modelo estruturado e **validado
contra o esquema permitido no servidor** — um bloco ou marca que o esquema não
conhece não sobrevive, e nenhum `script`, `iframe`, `svg`, `style`, `form`, nem
esquema `javascript:`/`data:` em ligação passa. O cliente nunca é a autoridade
sobre o que se guarda.

Preferiu-se o documento estruturado ao HTML higienizado como canónico porque os
blocos ricos têm semântica que o HTML dilui: uma *checklist* continua uma
*checklist*, uma tabela continua estruturada, um bloco de código mantém os seus
metadados, e uma referência a uma versão de ficheiro é explícita e não uma tag
`&lt;img&gt;` a interpretar. As revisões ficam deterministas, o controlo de conflitos
mais limpo, e — o que importa a prazo — uma **revisão histórica nunca depende da
interpretação de HTML arbitrário**, e a IA futura endereça uma revisão
estruturada exacta em vez de texto solto. Uma mudança na forma de renderizar não
reescreve a memória institucional.

O `schema_version` em cada revisão permite evoluir o esquema de forma
controlada: um documento antigo lê-se pela versão com que foi escrito, e
migra-se de forma determinista quando se decidir — nunca reinterpretado em
silêncio.

Não se constrói um motor de editor de raiz nem se introduz um segundo
*framework*: escolhe-se uma biblioteca madura, de licença permissiva, cujo modelo
de documento é estruturado (classe ProseMirror), **vendorizada e servida
same-origin** sob `script-src 'self'` (sem CDN, sem `eval`). O modelo canónico do
Ocinye é o que se persiste; o da biblioteca traduz-se para ele na fronteira.

### 3. O editor é vendorizado, same-origin, e melhora o progressivo

A CSP do Workspace é `script-src 'self'` sem `unsafe-inline` nem `unsafe-eval`
(ADR-0600, ADR-0019). O editor é uma biblioteca **vendorizada em `static/`** e
servida same-origin, inicializada a partir do `app.js` — nunca de um CDN, nunca
de `&lt;script&gt;` embutido, nunca de um segundo *framework* (sem React/Vue). Produz
o **documento estruturado canónico** (§2), que o **Core valida contra o esquema
permitido no save** — o cliente nunca é a autoridade sobre o que se guarda. Sem
JavaScript, a nota **lê-se** (o HTML derivado e higienizado, renderizado pelo
servidor) e edita-se por uma área de texto simples que grava um parágrafo:
degradado, mas funcional, como manda a doutrina de melhoria progressiva.

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

Colar uma imagem envia os bytes **pelo Core** → Object Storage → `File`+`FileVersion`,
e o corpo referencia a `FileVersion` exacta, servida de volta pela pré-visualização
same-origin do Core. Nunca `base64` permanente no corpo, nunca uma chave de
object-store na Experience, nunca uma URL pública. Abrir a imagem reautoriza.

**Emenda 2026-09-10 (fatia B).** A decisão original dizia `files::create`, que
exige um `workspace_id` — e uma nota pessoal não tem ambiente. Resolve-se sem
duplicar a primitiva: os `files` passam a servir dois donos, tal como as `notes`
já fazem (migração `0032`). Um ficheiro é de um ambiente **ou** de uma pessoa,
nunca de ninguém, e um `CHECK` impõe-o. Uma imagem de nota é um ficheiro
`owner_id`-scoped, `INTERNAL` como a nota, invisível aos ecrãs institucionais de
Ficheiros (que filtram por ambiente). O caminho pessoal é **aditivo** —
`files::create_personal` e `files::preview_personal_version` ao lado das funções
por ambiente, sem lhes tocar —, e a autoridade vem do dono: só o dono lê, e uma
`FileVersion` de outra pessoa responde «não encontrado» antes de tocar nos bytes.
Guardar uma nota resolve cada `FileVersion` que ela cita e recusa a que não for do
dono. A imagem serve-se por `/me/files/{version_id}/preview` (o Workspace faz
proxy same-origin para o Core), e só formatos que se mostram inline — PNG, JPEG,
WebP — entram; um SVG é um documento com script.

Anexos genéricos (PDF, documento, …) seguem o mesmo subsistema de Files, e ficam
para uma fatia posterior: o esquema do documento já conhece o bloco de anexo para
que entre sem redesenho. Esta emenda toca a decisão de ficheiros de
[ADR-0204](0204-institutional-files-and-folders.md), que passa a admitir um
ficheiro com dono e sem ambiente.

**Emenda 2026-09-10 (pesquisa, fatia C).** A fatia A não indexou as notas
pessoais porque o índice de pesquisa não tinha como dizer «esta linha é de uma
pessoa»: uma nota `INTERNAL`, indexada como estava, seria encontrada por toda a
organização. O `VisibilityFilter` (o modelo de leitura, transversal a toda a
pesquisa) ganha uma dimensão de **dono**: uma linha que pertence a uma pessoa só
é visível a essa pessoa, e as cláusulas de classificação e de filiação nunca lhe
tocam. O `search_documents` ganha `owner_id` (migração `0033`), e o renderizador
de SQL adere por tabela — as tabelas institucionais renderizam byte a byte como
antes; só o índice de pesquisa activa o dono, guardando as cláusulas com
`owner_id IS NULL`. Guardar uma nota pessoal indexa-a com o dono e a projecção de
texto do corpo.

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

- **A** — domínio (dono, documento estruturado canónico + projecções, revisão com autor, GET nota +
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
