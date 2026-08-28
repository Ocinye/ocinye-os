# Changelog

Registo factual do que mudou. **Roadmap não entra aqui como concluído**
(`CLAUDE.md` §69).

Formato: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Não lançado]

### Repositório institucional estabelecido — 2026-08-23

O Ocinye OS passou a estar versionado. Não é uma release, não é uma versão 1.0,
e não representa nenhum deployment: é o momento em que a instituição começou a
guardar a história deste projecto.

**[`Ocinye/ocinye-os`](https://github.com/Ocinye/ocinye-os)** — privado, na
organização `Ocinye`, com o primeiro commit em identidade humana. Sem tag, sem
release, sem licença: o licenciamento é decisão institucional própria e o
repositório é privado até que deixe de o ser por decisão, não por omissão.

O primeiro commit representa o estado verificado de Foundation v1 e do
Research + Knowledge Agentic Retrofit v1. **Não foi fabricada história
anterior**: 344 ficheiros num commit de raiz, porque foi isso que aconteceu.

#### Antes de publicar

Uma varredura de publicação, e não outra auditoria de produto: segredos,
caminhos locais da máquina, artefactos de compilação, ficheiros grandes.

- **Zero segredos.** `gitleaks` (150 regras) mais a varredura do projecto. Os
  únicos achados do gitleaks estavam em `target/` — cabeçalhos PEM nos metadados
  de compilação das crates `pkcs8` e `pem_rfc7468`, que não são segredos e não
  são versionados.
- **Zero caminhos da máquina.** O único aparente é `file:///etc/passwd` dentro de
  uma fixture do higienizador de HTML: entrada hostil de teste, e tem de ficar.
- **Zero binários grandes.** Dois PNG do logótipo, deduplicados pelo Git.
- Credenciais de infraestrutura: todas `ocinye_dev_only` ou `CHANGE_ME`.

#### CI

A CI existia e nunca tinha corrido. Publicá-la obrigou a corrigi-la:

- **`permissions: contents: read`** no topo. Não havia bloco nenhum, e o
  omissão herda o que a organização decidir.
- **`gitleaks` como binário oficial**, verificado por checksum, em vez da action
  — que exige licença comprada em repositórios de organização, e teria falhado.
- **`cargo audit` real** em vez de `rustsec/audit-check`: as excepções vivem em
  `.cargo/audit.toml`, com a razão escrita, e é o binário que as lê.
- **Actions fixadas ao SHA**, não à tag. Uma tag move-se.
- **O guarda do `FixtureProvider`** e a verificação da biblioteca de ADRs, que
  o sweep local corria e a CI não.
- **`concurrency`**, para cancelar runs obsoletos da mesma branch.
- **Um passo que conta os testes que correram.** Foi ele que apanhou o primeiro
  erro real: comparava o total do projecto (638) com o da invocação
  `cargo test --workspace` (632) — os outros 6 correm noutra workspace. O gate
  estava certo e a contagem errada.

Também `.gitleaks.toml`, para que o mesmo comando passe localmente e na CI: sem
ele, `gitleaks dir .` numa máquina de desenvolvimento falha sempre por causa de
`target/`, e um gate que só é verde onde ninguém olha deixa de ser um gate.

#### Segurança do repositório

| | Estado |
|---|---|
| Visibilidade | **Privada**, verificada por releitura da API |
| Forks privados | **Desactivados** |
| Wiki · Projects · Discussions | Desactivados; a documentação vive em `docs/` |
| `GITHUB_TOKEN` por omissão | `read`, e o workflow declara `contents: read` |
| Dependabot alerts | **Activo**, verificado por releitura |
| Dependabot security updates | **Activo**, verificado por releitura |
| Secret scanning · push protection | **Indisponível** no plano Free para repositórios privados |
| Rulesets · branch protection | **Indisponível** no plano Free para repositórios privados |

#### A lacuna que fica declarada

**`main` não está protegida por regra do GitHub.** A API responde «Upgrade to
GitHub Pro or make this repository public» tanto a *rulesets* como a *branch
protection*. Não é omissão: é o plano.

Hoje, o que separa `main` de um push directo é disciplina escrita
(`CLAUDE.md` §73) e não o servidor. Fechar a lacuna exige uma decisão
institucional — plano **Team**, ou repositório público — e nenhuma das duas é
uma decisão de engenharia.

#### `CLAUDE.md`

A §73 dizia «não inicializes o repositório Git». Passou a ser a disciplina Git
do projecto: o que é permitido dentro de uma tarefa, o que exige autorização, e
o que é proibido sempre. **Claude Code continua a nunca ser autor, committer ou
co-autor** (§72, inalterada).

### Research + Knowledge Agentic Retrofit v1 — as costuras — 2026-08-23

Research e Knowledge já eram agent-addressable desde 2026-08-22: 25
capabilities, `ResourceRef` resolvido pelo Core, Context Engine, contexto
mínimo autorizado. **A arquitectura estava lá.** O que esta milestone
encontrou foi que duas das suas operações mais óbvias nunca tinham funcionado, e
que uma terceira aceitava mais do que devia.

#### Encontrado, e corrigido

- **`research.idea.create` e `collaboration.task.create` eram inalcançáveis**
  *(defeito, falha fechada)*. Ambas declaram âmbito de unidade ou de workspace e
  recebiam esse identificador pelo **`input`**. O executor autoriza um passo que
  não nomeia recurso nenhum contra o contexto do *pedido* — a organização, sem
  unidade e sem ambiente — e `ideas.create` e `tasks.create` são permissões que
  vêm de pertença, que aí não é consultada.

  É exactamente o defeito que o [ADR-0306](docs/adrs/0306-resource-resolution-as-authorization-boundary.md)
  foi escrito para fechar, deixado em duas capabilities. A milestone de Agosto
  chegou a medi-lo e a nomear `research.idea.create` — e corrigiu o executor sem
  corrigir a capability. Continuou a falhar fechada, e por isso em silêncio.

  Corrigido endereçando o recurso por `resources`. A unidade passou a ser um
  `ResourceKind` endereçável, resolvido por `organisation::get_unit`, porque uma
  Ideia nasce dentro de uma unidade e é a unidade que dá o contexto.

  **`research.idea.create` corre com sucesso pela primeira vez**, ponta a ponta:
  linguagem natural, fornecedor determinístico, planner, resolver, executor,
  serviço de domínio, e uma Ideia que existe na base de dados.

- **`assignee_id` não era verificado** *(segurança)*. Viajava do pedido para a
  coluna, e a única guarda era a chave estrangeira — que prova que o
  identificador nomeia *uma* pessoa, e nada mais. Uma tarefa numa organização
  podia nomear alguém de outra como responsável, o que atravessa a fronteira de
  inquilino que todas as outras decisões do Core respeitam; e porque um
  identificador real era aceite onde um inventado falhava, respondia também a
  «este UUID é uma pessoa aqui?».

  A regra que fecha isto não é política nova: **só se atribui trabalho a quem o
  poderia ler**, que é `evaluate` com `Action::Read` contra o contexto da própria
  tarefa. Verificado por reversão.

#### Acrescentado

Quatro capabilities, 25 → **29**, cada uma ligada a um pedido concreto:

| Identificador | O quê |
|---|---|
| `research.idea.revise` | Rever os campos descritivos de uma Ideia |
| `knowledge.note.revise` | Rever uma Nota; a versão anterior fica no histórico |
| `collaboration.task.transition` | Mover uma tarefa, com o workflow a decidir |
| `collaboration.task.assign` | Atribuir, ou retirar a atribuição |

**Rever não é transitar, e transitar não é reclassificar.** Os schemas nomeiam
os campos que podem mudar e nenhum outro: `state`, `workspace_id`,
`promoted_project_id` e a classificação continuam fora de alcance mesmo quando
um modelo os escreve à mesma — o que um teste confirma escrevendo-os.

#### Testes

**620 → 638.** Entre eles:

- `an_artefact_stricter_than_its_workspace_is_closed_on_every_agentic_path` —
  **F-01 pelas cinco vias agentic**: resolução de `ResourceRef`, leitura por
  capability, mutação por capability, Context Engine e pesquisa. Esta milestone
  abriu vias novas para os domínios que F-01 tocou, e nenhuma delas pode passar
  ao lado da correcção. Verificado por reversão.
- `every_membership_scoped_capability_is_reachable_by_a_member` — percorre o
  registry e mede a propriedade que faltava, em vez de confiar em que cada
  handler novo se lembrou.
- `a_task_cannot_be_assigned_to_somebody_who_could_not_read_it` e
  `assignment_respects_the_tasks_own_classification`.
- `revising_an_idea_cannot_reach_the_fields_the_domain_owns`.
- `a_task_transition_the_workflow_forbids_is_refused`.
- `creating_an_idea_end_to_end_actually_creates_one`, e a sua negativa.

#### Fixture

Uma referência sem identificador na frase passou a emitir um UUID nulo em vez de
uma string vazia. Ambos são respostas erradas, e a diferença importa: uma string
vazia não é um `ResourceRef`, logo a *proposta* não desserializa e não há plano
para examinar; um identificador inventado é o que um modelo que alucinou um
recurso produz — um plano bem formado que resolve para nada, que é o caso que o
Core existe para recusar.

#### Sem migration

O modelo já suportava tudo. Nada foi acrescentado ao esquema.

### Agentic Plan Lifecycle — a proposta torna-se durável — 2026-08-23

A única lacuna funcional que a Security Baseline v1 deixou declarada e por
fechar. O mecanismo de aprovação existia, estava correcto e era testado abaixo
da API — e nunca tinha sido exercitado através dela, porque
`agentic::repository::create_plan` não era chamado por lado nenhum.

#### O que estava partido

O Agent Runtime construía um `ActionPlan`, devolvia-o e largava-o. Nada o
escrevia. `GET /agentic/plans` devolvia sempre vazio, e aprovar ou executar um
plano por identificador respondia «não encontrado» — para um plano que tinha
acabado de ser produzido.

Falhava fechado, e por isso era invisível: nada corria indevidamente, apenas
nada corria. Com a inferência em `NO_RESOURCE`, nem sequer era observável.

#### Acrescentado

- **Persistência ligada ao Runtime.** Uma proposta é escrita **depois** de
  sobreviver ao planner — capability que existe, referência que resolve, número
  de passos limitado, entrada dentro do tamanho. Uma proposta recusada não deixa
  linha nenhuma. Nasce em `awaiting_approval` quando precisa de consentimento, e
  em `proposed` quando não.
- **`GET /api/v1/agentic/plans/{id}`**, e a listagem paginada com ordem total.
- **`agentic::lifecycle`**, um serviço de aplicação. As rotas passaram a ser
  quatro chamadas finas: uma decisão que vive num handler HTTP é uma decisão que
  só um cliente HTTP alcança — e foi assim que o portão de aprovação chegou a
  existir sem nunca ser exercitado.
- **Transições atómicas.** Aprovar, rejeitar e reclamar para execução são um
  `UPDATE … WHERE state = ANY(abertos) AND requested_by = $2`. Ler-decidir-escrever
  tem um intervalo entre a aplicação e a base de dados, e é por esse intervalo
  que o mesmo plano se executa duas vezes.
- **Protecção contra repetição.** Uma segunda execução — sequencial ou
  concorrente — é recusada com o estado real, em vez de repetir o efeito. A
  garantia está em PostgreSQL, não num lock em memória que uma segunda instância
  do Core não partilharia.
- **Risco lido do registry no momento da execução.** Uma capability
  reclassificada para cima desde que o plano foi construído passa a exigir a
  confirmação que hoje exige. Nunca ao contrário.
- **Imutabilidade material verificada.** O digest é recalculado a partir dos
  passos guardados e comparado com o guardado. Um plano cujo conteúdo material
  mudou é recusado — e, se já tinha sido reclamado, é resolvido como `failed`
  em vez de ficar preso em `executing`.
- **Auditoria do ciclo:** `plan_created`, `plan_approved`, `plan_rejected`,
  `plan_executed`. Sem o pedido do membro, sem o material recuperado, sem as
  palavras do modelo — nada disso é guardado, por isso nada disso pode ser
  servido.

#### Os controlos do Workspace deixaram de ser mortos

«Confirmar e executar» e «Rejeitar» já existiam em `/ask` e apontavam para estes
endpoints. Nenhuma interface nova foi construída: o que faltava era o outro lado.

#### Testes

**611 → 626.** Quinze testes novos contra PostgreSQL real, em
`crates/ocinye-core/tests/agentic_lifecycle.rs`, incluindo o caminho inteiro —
linguagem natural, fornecedor determinístico, plano validado, persistência,
consentimento, reautorização, Capability Executor, e uma linha que passa a
existir na base de dados. Nada mockado abaixo do Runtime, nenhum `INSERT` no
domínio a fingir efeito.

Quatro protecções foram verificadas por reversão — a regressão corrida contra o
código anterior, e a falhar:

| Removido | O que falhou |
|---|---|
| A persistência | 13 dos 15 testes — o estado exacto antes desta milestone |
| A reclamação atómica | Duas execuções concorrentes produziram dois efeitos |
| A reautorização | Um plano correu com acesso que o actor já não tinha |
| A expiração da confirmação | Uma confirmação caducada executou |

#### Continua `PLANNED`

**Action / Plan History como produto.** A persistência criada aqui é fundação
operacional, exigida pela segurança do ciclo. Pesquisa, cronologia, diff,
exportação e analytics sobre planos são milestone própria.

#### Correcções editoriais da Security Baseline v1

- A contagem dizia «4 `LOW`» e a tabela listava seis (F-07 a F-12). O total
  correcto é **12 findings: 1 `HIGH`, 5 `MEDIUM`, 6 `LOW`**. Corrigido em todos
  os sítios que o repetiam.
- O risco residual estava num só saco. Passou a distinguir: **1 risco residual
  aceite** (`lru`, compilado, sem correcção), **1 advisory sem aplicação ao
  artefacto de release** (`rsa`, via `sqlx-mysql`, que não é compilado), **3 na
  watchlist de manutenção**, e **1 limitação de segurança conhecida** (MFA).
- A secção que descrevia a lacuna de persistência **não foi reescrita**. Levou
  uma nota de follow-up datada. Um registo de auditoria que se corrige à medida
  que a realidade muda deixa de ser um registo.

### Ocinye OS Security Baseline v1 — auditoria adversarial com remediação — 2026-08-23

Auditoria de segurança de ponta a ponta do repositório, com correcção em linha,
regressão por finding e reverificação. Nada de arquitectura mudou: a forma
`Deterministic Core + Agentic Control Plane` está intacta, e nenhuma autoridade
migrou para agentes, modelo, fornecedor ou interface.

Registo completo, com reprodução, causa raiz e teste por cada finding:
[`docs/security/2026-08-23-security-baseline-v1.md`](docs/security/2026-08-23-security-baseline-v1.md).

**Resultado: 12 findings — 1 `HIGH`, 5 `MEDIUM`, 6 `LOW`. Todos corrigidos.**
Nenhum `CRITICAL`. Um risco residual aceite e escrito.

#### Corrigido

- **A classificação do artefacto passou a governar a leitura directa, e não a do
  seu Research Workspace** *(`HIGH`)*. Um artefacto pode ser mais restrito do
  que o ambiente que o guarda — `effective_classification` toma a mais
  restritiva das duas, e reclassificar um workspace para baixo não toca no
  material que ele já contém. A listagem sempre soube disso, porque
  `VisibilityFilter` filtra pela classificação da própria linha; a leitura
  directa não sabia. Um dataset `RESTRICTED` num workspace `INTERNAL` estava
  escondido de `GET /datasets` e era devolvido por
  `GET /datasets/{id}/versions` a qualquer membro da instituição. O mesmo para
  notas, fontes, documentos e tarefas, pelo resolver do plano agentic.
  Corrigido numa função — `research::readable_artefact_workspace` — por onde
  passam agora os cinco caminhos, e não em cinco `if`.

- **O corpo aceite por omissão deixou de ser 640 MiB** *(`MEDIUM`)*. O limite
  era único e valia para toda a API, incluindo `POST /auth/login`, que corre
  antes de existir sessão para recusar. Passou a 1 MiB, com o limite grande
  aplicado por rota nas três que carregam ficheiro. A relação entre os dois é
  agora uma asserção constante: desfazê-la não compila.

- **A equalização de temporização no início de sessão passou a acompanhar os
  parâmetros configurados** *(`MEDIUM`)*. O verificador que iguala «não existe
  esta conta» a «palavra-passe errada» era uma constante com os parâmetros
  Argon2 por omissão. O Argon2 lê o custo da string que verifica: um operador
  que seguisse `docs/security/` e subisse `OCINYE_ARGON2_MEMORY_KIB` reabria o
  oráculo de enumeração em silêncio. Medido com `m=64 MiB, t=3`: 240 ms contra
  1,25 s.

- **Escritas entre origens passaram a ser recusadas, no Core e no Workspace**
  *(`MEDIUM`)*. A protecção assentava apenas em `SameSite`, que compara o
  domínio registável e não a origem: uma página em `ocinye.com` — reservado
  para o futuro website público — é *same-site* com `workspace.ocinye.com`, e o
  browser envia-lhe o cookie da sessão.

- **`bootstrap-admin` passou a correr uma única vez, mesmo consigo próprio**
  *(`MEDIUM`)*. A verificação dentro da transacção não bloqueava nada: duas
  execuções concorrentes criavam **dois** administradores de plataforma, à
  primeira tentativa. Fechado com `pg_advisory_xact_lock`.

- **A stack TLS/HTTP legada deixou de ser ligada** *(`MEDIUM`)*. `aws-sdk-s3`
  trazia no conjunto `default` a feature `rustls`, que é a stack **legada** do
  SDK, ao lado da moderna: `hyper 0.14`, `h2 0.3.27`, `rustls 0.21.12` e
  `rustls-webpki 0.101.7` eram compilados sem nunca serem usados, com quatro
  avisos RustSec abertos. `aws-config` estava declarado e **nunca era usado**
  por linha nenhuma. De 5 vulnerabilidades para 1.

- **A trilha de auditoria deixou de ser esvaziável por `TRUNCATE`** *(`LOW`)*.
  Os triggers de 0001 são `FOR EACH ROW`; `TRUNCATE` não percorre linhas, e
  executava sem objecção. Migration `0012`.

- **Uma invocação WASM deixou de interromper a que corre ao lado** *(`LOW`)*.
  O relógio de época pertence ao `Engine`: um fio por invocação incrementava-o
  ao seu prazo e matava todas as outras. Medido: uma invocação de 2 s morria aos
  218 ms, a dizer que tinha excedido o seu limite.

- **A pré-visualização de contexto deixou de mostrar o que nunca iria a um
  modelo** *(`LOW`)*. Faltava-lhe `may_process_with_ai`, que o Context Engine
  agentic sempre aplicou. Ler não é processar.

- **A credencial do Node Agent passou a ser criada já protegida** *(`LOW`)*.
  Era escrita e só depois posta a `0600`; entre as duas chamadas ficava sob a
  umask do processo.

- **A paginação do correio deixou de transbordar** *(`LOW`)*. Era a única
  colecção que não usava `PageRequest`.

- **Um botão de correio que a CSP tornava inerte foi removido** *(`LOW`)*.
  «Carregar mesmo assim» recarregava a página, o aviso desaparecia, e as imagens
  continuavam bloqueadas por `img-src 'self' data:`.

#### Hardening

- PostgreSQL, Redis e MinIO passam a publicar em `127.0.0.1` no compose, em vez
  de em todas as interfaces com as credenciais que estão no `.env.example`.
- `cargo audit` passou a correr também no `./scripts/verify.sh`, com as
  excepções em `.cargo/audit.toml` — cada uma com a razão escrita.

#### Declarado, não corrigido

- **Planos agentic não são persistidos.** `create_plan` não é chamado, pelo que
  `GET /agentic/plans` devolve vazio e aprovar ou executar um plano por
  identificador responde «não encontrado». Falha fechada, e hoje nem é
  observável, porque a inferência é `NO_RESOURCE`. Mas o portão de aprovação,
  que é testado ao nível do executor, **nunca foi exercitado através de HTTP**.
  Ligar a persistência é alteração funcional, não correcção de segurança.

#### Testes

**588 → 611**, todos verdes, nenhuma suite saltada, com PostgreSQL real. Cada correcção de
`HIGH` e `MEDIUM` foi verificada por reversão: a regressão foi corrida contra o
código anterior e falhou.

### Research + Knowledge como módulos nativos agent-addressable — 2026-08-22

O primeiro grande domínio científico da Ocinye integrado no Agentic Control
Plane, através da arquitectura congelada na Architecture Baseline v1 — sem a
redesenhar, e testando-a.

#### O que o audit encontrou

Duas propriedades do executor eram insuficientes. Nenhuma permitiu alguma vez um
acesso indevido: ambas falhavam fechado. As duas juntas tornavam o núcleo
científico inalcançável por esta via.

**Os `ResourceRef` não eram verificados.** `ExecutionContext.resources` estava
documentado como «já resolvido e verificado», e não estava — as referências que
um modelo escrevia atravessavam o planner e o executor sem que nada as
procurasse. Não era explorável, porque nenhum handler as lia; era uma garantia
por acidente, e a documentação convidava o próximo handler a quebrá-la.

**O contexto de autorização era o do pedido.** Cada passo era autorizado contra
a organização: sem unidade, sem ambiente, `INTERNAL`. Para «pode pesquisar no
acervo» é a pergunta certa. Para «pode ler *esta* Nota» está errada de duas
maneiras ao mesmo tempo — recusa quem tem acesso por pertença, e não tem
unidade nenhuma de que uma referência estrangeira possa estar fora.

Medido antes da correcção: **nenhuma** capability de âmbito workspace era
alcançável por um membro cujo acesso viesse de pertença. Incluindo
`research.idea.create`, que existia desde a milestone anterior e nunca tinha sido
exercitada por um caminho bem-sucedido — os testes provavam recusas que passavam
pela razão errada.

#### A correcção

[ADR-0306](docs/adrs/0306-resource-resolution-as-authorization-boundary.md): a
resolução de recursos é uma fronteira de autorização, e o contexto vem do
recurso.

```text
resolve capability  →  um nome inventado resolve para nada
resolve resources   →  cada um, pelo serviço de domínio que o detém
authorise           →  contra o contexto de cada recurso, ou o do pedido
validate input      →  contra o schema publicado
approval gate       →  efeito externo e privilégio exigem sempre uma pessoa
execute             →  o serviço de domínio, que detém o invariante
audit               →  o que foi pedido, por quem, através de que agente
```

Ausência e recusa dão a mesma resposta. O título vem do Core, não do modelo.
Endereçar é através de `resources`, nunca de um identificador no `input`.

#### Capabilities

De 11 para **25**. Catorze novas, cada uma ligada a um pedido concreto:

| Domínio | Novas |
|---|---|
| Knowledge | `note.read` · `source.read` · `document.read` · `links.list` · `note.create` · `source.create` · `link.create` |
| Research | `workspace.overview` · `idea.read` · `project.read` · `idea.transition` · `project.transition` · `idea.promote` |
| Collaboration | `task.list` |

**Deliberadamente não expostas:** membership, mudança de classificação, qualquer
eliminação definitiva, upload de documentos, registo de base legal para conteúdo
integral, edição de Notas existentes, e pesquisa por tipo de entidade — esta
última porque `knowledge.search` já a cobre e duplicá-la não acrescentaria nada.

**Sem Domain Agents.** O Agent Registry suporta-os, mas o valor real que
trariam — expor menos capabilities conforme o contexto — já é entregue por
`context::domains_for`. Um segundo conceito a fazer o que o primeiro faz, sem
poder nenhum novo, seria arquitectura por simetria.

#### Uma permissão nova

`links.create`. Uma relação tipada é um objecto de investigação de primeira
classe (`CLAUDE.md` §13); pedir emprestada a permissão das notas seria dizer, no
ecrã de administração, que relacionar exige escrever notas. Concedida onde
`notes.create` já é concedida. O catálogo passa de 63 para **64**.

#### Uma permissão corrigida

`research.idea.promote` declarava `projects.create`, que só gestores de unidade
detêm — mas `promote_idea` autoriza quem pode transicionar naquele ambiente, o
que inclui o líder do Research Workspace. O descriptor estava a impor uma
segunda política, mais restritiva que a da instituição e não declarada em lado
nenhum. Passou a `projects.manage`, que é o que o domínio realmente exige.

#### Selecção

O Context Engine passa a distinguir **o que a pesquisa encontrou** do **que o
membro apontou**. A selecção vai primeiro no envelope, passa pelo mesmo resolver,
e uma selecção inalcançável **pára o pedido** em vez de ser descartada em
silêncio — responder sobre material diferente daquele para que a pessoa apontou é
pior do que não responder.

Ambos os caminhos passam pelo tecto de processamento com IA, mais baixo do que o
de leitura. Nesta instalação, sem nó local, nada acima de `INTERNAL` chega a um
modelo.

#### Superfícies contextuais

Um painel discreto na Ideia, no Projecto e no acervo de conhecimento. Não é uma
janela de conversa: uma linha de input, sugestões do próprio domínio, e o
contexto no endereço. **Prompt everywhere, not chat everywhere.**

Com zero fornecedores, declara-se indisponível com a razão do Core e lembra o
que continua a funcionar. Quem não tem `ai.use` não vê o painel de todo — o Core
recusaria na mesma, e mostrar o campo seria convidar a uma recusa.

Três variantes entraram no catálogo de ecrãs que os guardas percorrem: sem
inferência, sem permissão, e com inferência disponível.

#### Testes

**588**, todos verdes, mais 25 novos numa suite própria contra PostgreSQL real.
Entre eles: referência para outra unidade, para outra organização, de tipo
errado, e inventada — todas com a mesma resposta; a `label` do modelo descartada;
relação com um extremo inalcançável a não deixar aresta; conversão repetida a
produzir um Projecto e um conflito; selecção inalcançável a parar o pedido; e
conteúdo hostil — instruções de sistema, nomes de capabilities e pseudo-invocações
de ferramentas dentro de Notas e títulos de fontes reais — a não alterar nada.

Quatro E2E completos com o `FixtureProvider`, **sem GPU**: uma Nota criada de
ponta a ponta, a pesquisa a responder com zero fornecedores, `Perguntar` e
`Executar` a declararem-se indisponíveis sem mutar nada, e um modelo
completamente subvertido a não tocar em Research.

#### Documentação

`docs/domain/`, `docs/knowledge/` e `docs/search/` deixam de ser dívida
reconhecida: 76 → 173, 61 → 145 e 88 → 121 linhas, com os ciclos de vida
verificados contra `crates/ocinye-domain/src/workflow/`, as três dimensões de
autorização, a distinção entre fonte, entrada bibliográfica e documento, o que
está e não está indexado, e a diferença entre pesquisar e perguntar.

Threat model: catorze ameaças novas no plano agentic, cada uma com o seu teste.

#### Estado que não mudou

Nenhum fornecedor de inferência foi provisionado. **AI providers = 0. AI nodes =
0. Compute nodes = 0. L40S = `PLANNED`.** Nenhuma migration nova: o schema já
tinha tudo o que este trabalho precisava, incluindo `origin_idea_id` e
`promoted_project_id` nos dois sentidos.

### Architecture Baseline v1 e ADR Namespace Rebaseline v1 — 2026-08-22

Duas linhas de base, ambas documentais. **Não são versões de produto:** não há
release, não há tag, e o sistema não se chama 1.0.

#### ADR Namespace Rebaseline v1

As ADRs estavam numeradas pela ordem em que os problemas apareceram. Lidas em
conjunto, pareciam um diário de implementação em vez de uma arquitectura
deliberada — e uma decisão fundacional como o modelo de autorização vivia em
`0008`, entre a escolha de base de dados e a de object storage.

Reorganizadas por **domínio**, com posição dentro de cada faixa a seguir
dependência e não data:

| Faixa | Família | ADRs |
|---|---|---|
| `0001–0099` | Foundations | 11 |
| `0100–0199` | Identidade, segurança, autorização | 5 |
| `0200–0299` | Conhecimento, dados, memória institucional | 3 |
| `0300–0399` | IA, controlo agentic, inferência | 6 |
| `0400–0499` | Módulos institucionais nativos | 9 |
| `0500–0599` | Computação, nós, Capability Runtime | 2 |
| `0600–0699` | Workspace e Experience Plane | 3 |

`0700–0799`, `0800–0899` e `0900–0999` ficam **vazias**. Nenhuma ADR foi criada
para preencher espaço.

Feito agora, antes de qualquer divulgação externa, porque é a última
oportunidade deliberada de o fazer. **Nenhuma mudança funcional** decorreu da
renumeração.

#### Duas ADRs novas

A decisão mais fundamental do projecto não tinha registo próprio — estava
distribuída pelo `CLAUDE.md`, pelo README e implícita em dezenas de outras.

- **ADR-0001 — O Ocinye OS como sistema operacional institucional AI-native.**
  A porta de entrada da biblioteca: o que o sistema é, e o que deliberadamente
  não é. Nomeia as quatro propriedades que o definem e aponta para as decisões
  que as concretizam.
- **ADR-0003 — Módulos nativos, não aplicações desligadas.** O princípio geral
  existia apenas instanciado no correio. Generalizado, com o contrato que
  qualquer módulo novo cumpre — e com a distinção entre *nativo* e *não usar
  recursos externos*, que não são a mesma coisa.

#### Número, domínio e impacto passam a ser coisas separadas

Toda a ADR declara agora `Domínio` e `Impacto`. Cinco são `FOUNDATIONAL`.

> **Os números definem um namespace estável. A importância é metadata, não
> renumeração.**

E, a partir desta baseline: **um identificador de ADR aceite é permanente.** A
importância não renumera, o estado não renumera, e uma decisão que muda é
**substituída, não reescrita**. `CLAUDE.md` §68 passou a dizê-lo em termos
normativos.

#### Índice reescrito

`docs/adrs/README.md` deixou de ser uma lista numérica. Tem «Start here» com
três caminhos de leitura, as decisões fundacionais em destaque, um **grafo de
dependências** em Mermaid, navegação por domínio, catálogo completo com
domínio/impacto/estado, e as regras de escrita.

#### Architecture Baseline v1

Uma correcção no README: **o estilo de uma aresta deixou de significar estado**.
Havia relações `CURRENT` desenhadas a tracejado — o outbox que alimenta o
Worker, o Agent Registry a informar o Main Agent — ao lado de relações
`PLANNED` com o mesmo traço. Agora o traço indica natureza (auxiliar ou
assíncrona) e o **estado está sempre escrito** no rótulo.

Congelado nesta baseline: arquitectura AI-native, autoridade do Core, Agentic
Control Plane, fronteira canónica de fornecedor, contrato de módulo nativo,
fundação de conhecimento e dados, abstracção de computação, e as fronteiras de
segurança.

**Baseline não significa arquitectura imutável.** Significa que mudanças futuras
são explícitas e historicamente rastreáveis.

#### Referências

467 referências a ADRs em 143 ficheiros, actualizadas atomicamente. Zero
identificadores antigos activos. Zero ligações mortas.

#### Dois achados durante a operação

**As migrations aplicadas mudaram de checksum.** Sete migrations contêm
referências a ADRs em comentários e em `COMMENT ON`, e a renumeração alterou-as
— o que o guarda de checksum do SQLx apanhou imediatamente, como deve.

Deixá-las intactas teria sido pior: a antiga `ADR-0007` era o Identity Provider,
e o número `0007` passou a ser «Fronteiras de domínio». Dezoito comentários
passariam a apontar para decisões erradas.

Mantidas, portanto, e as bases de dados locais recriadas. **Isto só foi possível
porque nada está deployado**, e o `CLAUDE.md` §58 continua a valer: a partir do
primeiro deployment, migrations aplicadas não se editam, e uma rebaseline como
esta deixa de ter esta saída.

**Um defeito introduzido e corrigido dentro da milestone.** O script de
renumeração usou `@@` como marcador temporário e removeu-o no fim — removendo
também o operador de full-text `@@` do PostgreSQL em três consultas, de pesquisa
institucional e de correio. Apanhado pela suite de testes contra base de dados
real, e restaurado. Nada foi divulgado com o defeito.

### README raiz: Architecture Baseline v1 — 2026-08-22

O `README.md` raiz passa a ser a representação canónica da arquitectura do
Ocinye OS. **Não é uma versão de produto**: não há release, não há tag, e o
sistema não se chama 1.0. É uma referência documental — as milestones seguintes
encaixam nesta arquitectura em vez de a redesenharem.

Reescrito por inteiro, e depois revisto cirurgicamente. Seis correcções
conceptuais que a revisão expôs:

- **A autorização acontece duas vezes, e o README dizia-o mal.** O protocolo
  listava `Authorize` uma vez, e o diagrama punha a autorização depois do plano
  e da aprovação. A implementação sempre teve duas avaliações da mesma
  fronteira: **filtragem de exposição** antes de o modelo ver capabilities ou
  contexto, e **autorização em tempo de execução** imediatamente antes do
  efeito. Existem porque o tempo passa entre elas — e uma confirmação humana
  demora, por definição, tempo humano.
- **Confirmação não é autorização.** Depois de confirmado, o Core reavalia e
  pode recusar. Material `RESTRICTED` para fora da instituição continua recusado
  depois da confirmação.
- **«O Ocinye OS é um sistema completo» dizia mais do que se pretendia.**
  Lia-se como *feature-complete*. Substituído por: as capacidades
  determinísticas e a interface tradicional permanecem operacionais, e o que
  depende de inferência degrada honestamente.
- **A L40S estava descrita como um adapter.** É **hardware**. A cadeia correcta
  separa quatro coisas: GPU → servidor de inferência → modelo → **Ocinye
  Provider Adapter**, que é quem implementa o contrato. Corrigido também em
  `docs/agentic/`.
- **«Toda esta topologia é PLANNED. Nada dela existe.»** contradizia a secção de
  estado: Workspace, Core, Worker, PostgreSQL e Redis existem e correm
  localmente. O que não existe é o **deployment** segundo aquela topologia.
- **«Mail não é uma integração»** era tecnicamente falso — o Mail usa um
  fornecedor por IMAP/SMTP. O que se nega é integração *superficial*, e a
  fronteira própria está agora explícita.

E o invariante do correio ficou arquitecturalmente preciso:

> **A IA prepara. A pessoa autoriza. O Core envia.**

Quem executa o efeito externo é o Core, não a pessoa que carregou no botão.

#### Corrigido — inconsistência encontrada na auditoria

**O README afirmava «object storage — NOT CONFIGURED» e listava MinIO na
infraestrutura local.** `.env.example` configura o MinIO, e nessa configuração o
Core reporta armazenamento disponível. O que não existe é armazenamento
**institucional**, com residência `UNDECLARED`. Corrigido no README e em
`docs/feature-status/`.

#### Estrutura

24 secções, quatro diagramas Mermaid — arquitectura do sistema, execução de
acção assistida por IA, integração de módulo nativo, e topologia alvo. Os quatro
renderizam, verificado com `@mermaid-js/mermaid-cli`.

Contadores frágeis fora do corpo: números de testes, de ícones, de ecrãs e de
ADRs mudam a cada milestone e envelhecem no documento que menos deve envelhecer.

### Endurecimento da fundação agentic: contrato, conformidade, intenção — 2026-08-22

[ADR-0305](docs/adrs/0305-provider-conformance.md), e expansão do
[ADR-0304](docs/adrs/0304-canonical-inference-contract.md).

Milestone curta e sem funcionalidades novas de produto: consolidar a fronteira
entre o Agent Runtime e qualquer fornecedor futuro, antes de a documentar como
baseline.

#### Adicionado — o contrato endurecido

- **`ContractVersion`.** Uma variante, `V1`, e uma versão desconhecida é
  recusada. Mudanças incompatíveis passam a ser explícitas em vez de
  descobertas. **Sem `QwenV1` nem `DeepSeekV1`** — versionar por modelo seria o
  contrato pertencer aos modelos.
- **Prazo no pedido**, e `infer_within_deadline` a aplicá-lo **do lado do
  Core**. Um provider que fica pendurado não prende o pedido.
- **Limite de tamanho** da resposta, e do input de cada passo do plano.
- **`ModelIdentity::normalised`.** É texto controlado pelo fornecedor que
  aterra em logs: newlines num nome de modelo forjam linhas de log, um megabyte
  enche um disco.
- **Três variantes de erro novas**: `Timeout`, `ResponseTooLarge`,
  `UnsupportedContractVersion`. `is_transient()` diz que repetir não é fútil —
  **não** que é seguro.
- **Observabilidade da chamada**: adapter, capacidade, duração, classe de
  desfecho. Nunca o prompt, o material recuperado ou o texto de erro do modelo.

#### Adicionado — a Provider Conformance Suite

`intelligence::conformance::certify` — **10 verificações**, sem GPU, sem rede,
sem base de dados, a correr em **2,3 segundos**.

> **Um fornecedor não é suportado enquanto não a passar.**

A suite tem duas metades, e a divisão é deliberada. Esta certifica um **adapter
em isolamento**. O resto — «um provider hostil não escala», «um `ResourceRef`
alucinado não resolve», «o risco não pode ser baixado» — não é propriedade do
provider mas da **reacção do Core** a um, e vive nos testes de integração.
Nenhum adapter pode certificar o Core.

**Um provider hostil passa a suite**, e isso é o ponto: conformidade é sobre a
fronteira, não sobre as intenções do modelo. Passar não torna um provider
confiável; torna-o utilizável.

#### Adicionado — comportamentos de fixture

De 4 para **8**: `Timeout`, `Partial`, `WrongVersion` e `Oversized`, além dos
existentes.

#### Corrigido

- **«Encontra o último relatório» estava a ser lido como `Act`.** É uma
  instrução na forma e uma pesquisa na substância, e encaminhá-la para `Act`
  fá-la exigir um modelo que esta instalação não tem — um pedido perfeitamente
  respondível voltaria indisponível. Verbos de leitura (`encontra`, `procura`,
  `mostra`, `lista`, `pesquisa`) passam a `Search`.
- **A suite demorava 315 segundos**, porque a sonda usava o prazo de produção
  de 45s contra um fixture que nunca responde. O prazo do pedido agentic passou
  a configurável — que era o que faltava de qualquer forma — e a suite corre em
  2,3s.
- **O guarda de isolamento do fixture não disparava.** O padrão procurava as
  linhas de aresta do `cargo tree` em vez da linha do pacote. Corrigido, e
  **verificado a introduzir a fuga de propósito** para confirmar que deteta.

#### Alterado

- `verify.sh` passa a verificar **estruturalmente** que o binário do servidor
  não resolve `test-fixtures`, em vez de depender de um `strings` que falharia
  em silêncio se o optimizador mantivesse o código sem os literais. O `strings`
  fica como complemento empírico.
- `Intent::detect` ganhou `abre`, `remove`, `partilha`, `promove`, `cancela` e
  outros, com regressões para as frases nominalizadas do briefing §73–§77.
- A política de saída estruturada está escrita: campo desconhecido é ruído;
  em falta, `null`, tipo errado e enum desconhecido são recusados.

#### Testes

De **534** para **563**. Novos: 10 de conformidade, 9 da reacção do Core a cada
comportamento de provider, e 10 de detecção de intenção — incluindo que uma
frase inglesa cai para `Search`, porque **não há suporte a inglês** e não deve
ser declarado sem alguém o implementar.

#### Estado factual, inalterado

Fornecedores de IA: **0**. Nós de IA: **0**. Nós de computação: **0**.
L40S: `PLANNED`, não provisionada. `FixtureProvider`: só em testes.

### Contrato canónico de inferência e retrofit agentic do Ocinye Mail — 2026-08-22

[ADR-0304](docs/adrs/0304-canonical-inference-contract.md).

Duas correcções de direcção, e o retrofit que elas tornaram possível.

#### Corrigido — o E2E não dependia de um modelo real

Ao fechar a milestone anterior afirmei que o E2E agentic tinha de esperar por um
modelo, porque um fixture teria de imitar o formato de um. **Estava errado**, e
estava errado por uma lacuna: o AI Gateway não tinha contrato de resposta
próprio, pelo que a resposta implícita era «o que quer que o modelo devolva» —
exactamente o acoplamento que o ADR-0300 e o ADR-0301 proíbem.

- **`InferenceProvider`**: o contrato canónico. `system`, `data` e `instruction`
  são campos distintos — um contrato que aceitasse uma string opaca teria já
  fundido política de sistema com conteúdo recuperado antes do adapter, e é
  nessa separação que a defesa contra injecção assenta.
- **Saída estruturada faz parte do contrato.** O Runtime precisa de um plano.
  Arrancar forma a um modelo concreto é trabalho de adapter, e o Core valida na
  mesma: um provider a afirmar conformidade não é conformidade.
- **`InferenceError` é fechado e mudo.** O texto de erro de um modelo pode citar
  o prompt de volta, e o prompt pode conter correspondência de um membro.
- **`FixtureProvider`**: determinístico, quatro comportamentos — cooperativo,
  **hostil**, malformado, ausente. Atrás de `#[cfg(feature = "test-fixtures")]`;
  **verificado com `strings` que um binário de release não o contém**.

#### Corrigido — a barra lê a frase

`Search · Ask · Act` como três botões era a implementação, não o destino.
`Intent::detect` lê a frase: primeira palavra imperativa → `Act`, interrogativa
ou `?` → `Ask`, o resto → `Search`.

**Determinística**, por duas razões: encaminhar um pedido para um modelo para
decidir se o encaminha para um modelo é circular; e quem escreve a mesma frase
duas vezes tem de obter o mesmo comportamento duas vezes.

**Ambiguidade cai sempre para pesquisar.** «criação de tarefas no Ocinye» é uma
pesquisa que contém um verbo, não uma ordem. Uma leitura errada para `Act`
executa o que ninguém pediu; para `Search` apenas mostra resultados. Os três
modos ficam visíveis como controlo e reserva.

#### Adicionado — o Ocinye Mail no plano agentic

Quatro capabilities novas, total do Mail: **sete**. Registry: 7 → **11**.

| Nova | Risco |
|---|---|
| `mail.search` | Consulta — dentro da caixa, pelo filtro de pertença |
| `mail.read` | Consulta — já higienizado, sem conteúdo remoto |
| `mail.draft_transform` | Alteração menor — sem IA, recusa e **deixa o rascunho intacto** |
| `mail.evaluate_send` | Consulta — responde à pergunta que o envio levantaria, **antes** do envio |

`institutional_domains` passou a derivar das caixas que a instituição tem, para
que a política de classificação funcione na camada de capabilities sem lhe
enfiar `CoreConfig` pelo meio.

#### Adicionado — o E2E que prova a arquitectura

O caminho inteiro, sem GPU:

```
linguagem natural → Main Agent → ActionPlan → Capability → aprovação → Core → resultado
```

E o fluxo do correio: procurar → ler → preparar rascunho → **parar**;
transformar; e só então enviar — risco 3, confirmação, autorização, verificação,
auditoria. Cada seta é código real.

Mais: um administrador com todos os papéis administrativos que existem recebe
`ResourceNotFound` ao tentar `mail.read` e `mail.draft_reply` na caixa de outra
pessoa — pelo plano agentic, como já acontecia pela interface.

#### Testes

De **513** para **534**. Os 12 novos da suite agentic incluem o modelo
completamente subvertido a produzir *nada*, uma falha de modelo a não deixar
rasto de execução, e o fluxo de correio de ponta a ponta.

#### Limitações declaradas

- **Ainda não há inferência.** O que existe é o caminho, escrito e testado
  contra o contrato. Falta um adapter que sirva `GENERAL`.
- **`mail.draft_transform` recusa** sem nó de IA, e o rascunho fica como estava.
- **`mail.read` devolve os metadados e o excerto**, não o corpo: buscar o corpo
  precisa do provider, e nesta instalação não há nenhum configurado.
- **`mail.send` agentic continua a devolver indisponível**: o envio pertence ao
  pipeline que `POST /mail/send` detém.

### AI-native architecture e Agentic Control Plane — 2026-08-22

O Ocinye OS passa a ter um plano agentic. Quatro ADRs:
[0032](docs/adrs/0002-deterministic-core-and-agentic-control-plane.md) a
[0035](docs/adrs/0303-capability-registry-and-executor.md).

**Não há inferência nesta instalação, e nada aqui finge que há.** O que foi
construído é tudo excepto o modelo — e `Pesquisar` funciona sem ele.

#### Adicionado — contratos e política

- `ocinye-contracts::agentic`: `CapabilityId`, `ResourceRef`, `RiskLevel` (5
  níveis), `AutonomyLevel` (6, com tecto em `Workflow`), `ActionPlan`,
  `ApprovalRequirement`, `ExecutionStatus`, `Reversibility`, `Intent`.
- `ocinye-domain::policy::agentic`: a invariante
  **Actor ∩ Agent Scope ∩ Resource Policy**, pura e exaustivamente testada. O
  actor é a primeira porta; cada uma a seguir só estreita.
- `is_delegable_to_agents`: gestão de permissões, papéis, membros, plataforma,
  IA, computação e correio nunca vão para um agente — e **o registry não
  arranca** se alguma capability as exigir.

#### Adicionado — o plano agentic

- **Capability Registry**: 7 capabilities em 5 domínios, conjunto fechado
  definido em código. Não é uma tabela: um conjunto editável em tempo de
  execução é um conjunto que nenhum teste consegue fixar.
- **Capability Executor**: resolver → autorizar → validar → aprovação →
  executar → auditar.
- **Context Engine**: contexto mínimo e autorizado, com **dois** tectos — o de
  leitura do actor e o de processamento por IA, que é mais baixo. Reporta
  quantos resultados reteve, porque «não posso enviar isto a um modelo» é
  diferente de «não encontrei nada».
- **Action Planner**: onde a saída do modelo deixa de ser confiável. Recusa
  capabilities inexistentes, planos com mais de 8 passos, e propostas vazias.
  O digest liga uma aprovação ao **efeito** do plano.
- **Aprovações**: pessoa + digest + 15 minutos. As três.
- **Main Agent**: a lista de capabilities mais larga que existe, e nenhum
  privilégio.
- **Universal Command Surface**: `Search · Ask · Act` na barra do Workspace, com
  as três intenções como escolha explícita — adivinhar «executar» a partir de
  uma frase ambígua é como uma pergunta se torna uma acção que ninguém pediu.
- Migration `0011`: `action_plans` e `action_approvals`. Sem prompts, sem
  raciocínio do modelo, sem contexto recuperado.

#### Corrigido — encontrado a construir

- **Autorizar passou a acontecer antes de validar.** Um erro de validação
  descreve a forma da entrada de uma capability, e devolvê-lo a quem não a pode
  usar entrega o mapa de uma interface que essa pessoa não tem que ver. **Um
  teste que assumia a ordem inversa expôs isto.**
- **Um campo obrigatório a `null` era tratado como presente.** `null` é ausência
  por extenso, e aceitá-la deixava um modelo satisfazer um campo com nada.
- **`Capability` significava duas coisas.** Uma respondia «o correio está
  configurado?» e a outra ia responder «criar uma pasta». O existente passou a
  **`SystemCapability`**, que é o que sempre foi — 70 utilizações em 7 ficheiros.
- **Faltavam permissões nomeadas para tarefas.** `TasksView`, `TasksCreate` e
  `TasksEdit` entraram no catálogo (60 → **63**) e nos papéis de workspace: sem
  elas, `collaboration.task.create` teria de declarar uma permissão que o serviço
  não verifica.
- **`knowledge.search` exigia `BibliographyView`**, que nenhum papel detém
  institucionalmente — a pesquisa ficaria inalcançável pelo plano agentic para
  toda a gente. Passou a `OrganisationView`, que é o que a precondição
  realmente é: a autorização da pesquisa está dentro da query.

#### Testes

De **421** para **513**, todos verdes. Os 16 novos de segurança são todos
ataques: capability inventada, escalada exaustiva sobre as 63 permissões,
injecção indirecta, reutilização de aprovação, plano descontrolado, fuga pela
auditoria, e a verificação de que **nenhuma capability alcança shell, SQL,
ficheiros, rede ou segredos**.

#### Limitações declaradas

- **Inferência: `NO_RESOURCE`.** Zero nós de IA. `Perguntar` e `Executar`
  devolvem indisponível com a razão e com o que ainda funciona.
- **Domain Agents com prompt próprio: `PLANNED`.** O domínio é hoje fronteira no
  registry e no Context Engine.
- **`mail.send` agentic devolve indisponível**: o envio pertence ao pipeline que
  `POST /mail/send` detém, e duplicá-lo duplicaria a política de classificação.
- **`AutonomyLevel::Autonomous` é inalcançável.** Existe no tipo para que as
  comparações sejam totais e para que activá-lo seja uma alteração deliberada.
- **Sem jobs agentic em segundo plano, sem trabalho agendado, sem automação por
  evento.** `PLANNED`, em Observe → Suggest.

### Ocinye Mail — transporte IMAP real — 2026-08-22

Fecha o que [ADR-0401](docs/adrs/0401-mail-provider-abstraction.md) deixou como
`PLANNED`. Decisões em [ADR-0408](docs/adrs/0408-imap-transport.md).

**A instalação continua sem correio configurado.** O que mudou é que passa a
haver um caminho real para o configurar, e uma forma de o provar.

#### Adicionado — transporte

- **Leitura IMAP completa**: ligação TLS, autenticação, listagem paginada por
  UID, corpo, anexos, flags `\Seen` e `\Flagged`, e mover entre pastas.
- **Descoberta de pastas.** `Sent`, `Sent Items`, `INBOX.Sent` e `Enviados` são
  todos reais em servidores reais. O adaptador faz `LIST` e resolve por nome
  exacto, depois por segmento final, e só então cai no convencional. Fixar
  `Sent` no código funciona até encontrar um servidor que discorde — e nessa
  altura o envio parece funcionar enquanto nada é arquivado.
- **`OCINYE_MAIL_IMAP_TLS` e `OCINYE_MAIL_SMTP_TLS`**, com `tls` (implícito) e
  `starttls`. **Não existe valor que desligue a cifra**: `false`, `none`, `off`
  e `plain` fazem o Core recusar arrancar, com a razão escrita.
- **`ocinye-core-server mail-check`.** Prova credenciais sem arrancar o Core.
  Imprime anfitriões, portos, segurança, nomes de pasta e contagens — nunca a
  password, a resposta de autenticação, assuntos, remetentes ou corpos. E **não
  envia nada**: diagnosticar um caminho de saída enviando correio a alguém é
  como mensagens de teste chegam a pessoas reais.
- **Sincronização** (`POST /mail/mailboxes/{id}/sync`) e o botão «Actualizar»,
  que enche o índice a partir do serviço. Uma falha fica registada em
  `last_sync_error` e aparece ao lado da caixa: «nada de novo» e «não consegui
  perguntar» têm de ser distinguíveis.

#### Corrigido

- **O `rustls` entrava em pânico ao primeiro pedido de correio.** A árvore tem
  dois provedores criptográficos — `ring` via `lettre`, `aws-lc-rs` via o SDK da
  AWS — e o `rustls` recusa-se a adivinhar. O `ClientConfig` passa a nomear
  `ring` explicitamente, o que evita também o global de uma só chamada
  `install_default()`. **Encontrado por correr o `mail-check`, não por o ler**;
  sem ele o primeiro sintoma teria sido um pânico em produção.
- **Um UID de IMAP só é único dentro de uma pasta.** `fetch_message`,
  `fetch_attachment`, `set_read`, `set_starred` e `move_message` passam a
  receber a `MailFolder`, que o índice já registava. Assumir `INBOX` devolveria
  a mensagem errada — ou nenhuma — ao abrir algo em «Enviados».
- **Marcar como lida deixou de ser efeito secundário de ler.** O `FETCH` usa
  `BODY.PEEK`; alterar o estado é uma operação própria, tomada deliberadamente.
- **As flags passam pelo serviço antes do índice.** Antes, o índice era escrito
  sem o serviço saber, e a interface mostrava um estado que o servidor de
  correio não tinha.
- **O dossier de design e o sprite tinham divergido.** Os cinco ícones de
  correio estavam no sprite e não em `ICONS.md`. Registados, e o teste de
  fidelidade passa a verificar **nos dois sentidos** — uma só direcção deixa
  passar exactamente o caso que aconteceu.

#### Alterado

- `mail.sync` deixa de ser `PLANNED` e passa a **`DEGRADED`**, não a
  `AVAILABLE`: um membro actualiza uma pasta, e nada o faz por ele. Correio novo
  não aparece sozinho, e a capacidade tem de dizer isso.
- `MailConfig` e `ImapSmtpConfig` incluem a segurança de transporte no `Debug`
  redigido, ao lado dos anfitriões.

#### Dependências

`tokio-rustls` (`ring`, `tls12`) e `webpki-roots`. Raiz da Mozilla compilada e
não a do sistema: o conjunto de raízes em que o Ocinye OS confia passa a ser o
mesmo num portátil, na CI e num servidor. Sem opção para ignorar a verificação
do certificado — uma que exista acaba ligada em produção.

#### Limitações declaradas

- **Não existe worker de ingestão.** A sincronização é pedida por quem lê.
- **STARTTLS para IMAP não está implementado.** Devolve um erro claro em vez de
  cair para uma sessão sem cifra.
- **A descarga de anexos** é lida pelo adaptador e ainda não tem rota nem ecrã.
- **Uma credencial por instalação.** A versão multiutilizador precisa de
  credenciais por caixa, cifradas em repouso — `PLANNED`, com ADR próprio.

#### Testes

De **414** para **421**, todos verdes. Os novos cobrem a resolução de nomes de
pasta contra hierarquias e localizações reais, os bytes de anexo pelo mesmo
índice que a interface mostra, e a recusa de qualquer valor que desligue a
cifra.

### Ocinye Mail — 2026-08-22

Correio institucional como módulo do Ocinye Core. Oito ADRs:
[0023](docs/adrs/0400-mail-as-institutional-surface.md) a
[0030](docs/adrs/0407-mail-index-not-archive.md).

**Nesta instalação o correio não está configurado.** O módulo existe, está
testado, e a interface declara o estado em vez de mostrar uma caixa vazia.

#### Adicionado — domínio

- `ocinye-contracts::mail`: pastas, papéis de caixa partilhada, acções de
  composição, política de conteúdo remoto, origem de rascunho, e `MailAddress`
  com decisão interno/externo por correspondência **exacta** de domínio.
- Sete permissões: `mail.use`, `mail.send`, `mail.ai_use`, `mail.shared.view`,
  `mail.shared.send`, `mail.shared.manage`, `mail.administer` — total do
  catálogo passa de 53 para **60**.
- Quatro capacidades: `mail`, `mail.send`, `mail.sync`, `mail.ai_assist`.
  Reportadas em separado porque falham em separado.
- Migration `0010`: oito tabelas. Invariantes verificados contra PostgreSQL
  real, incluindo `ck_mailboxes_ownership_agrees`, que é o que segura a
  fronteira de privacidade.

#### Adicionado — segurança

- **Higienização por lista de permissões** (`ammonia`) de todo o HTML recebido.
  Doze testes. Um único `inner_html` em todo o Workspace, documentado no
  ficheiro onde vive.
- **Conteúdo remoto bloqueado por omissão**, contado e mostrado. Carregá-lo é
  acto explícito por mensagem, e fica no audit trail.
- **Política de envio por classificação**: `RESTRICTED` não sai para
  destinatário externo, e confirmar **não desfaz a recusa** — existe um teste
  com esse nome. A classificação mais alta governa a mensagem inteira.
- **Fronteira de privacidade em SQL.** Nenhuma consulta de correio consulta um
  papel; nenhum caminho aceita um `person_id` diferente do actor. Uma caixa
  alheia lê-se como inexistente.
- `safe_filename` e `ck_mail_attachment_filename_is_safe` contra path traversal
  por nome de anexo.
- **Credenciais fora da base de dados.** `mail_provider_settings` não tem
  colunas de credenciais; `Debug` redigido em `MailConfig` e `ImapSmtpConfig`,
  com teste — `CoreConfig` deriva `Debug` e é registado no arranque.

#### Adicionado — assistência de escrita

- Conjunto **fechado** de dez acções; blocos de dados delimitados
  (`<<<EMAIL_RECEBIDO`, `<<<RASCUNHO`, `<<<PEDIDO_DO_MEMBRO`); e a garantia que
  não depende do modelo: **a assistência não tem nenhuma acção com efeito ao seu
  alcance**.
- **Gerar não é enviar.** Um formulário, dois `formaction`: `/mail/assist`
  devolve texto, `/mail/send` é a única rota que fala com o serviço. `assist`
  não chama `send` — não é verificação, é a ausência de uma chamada.
- Sem nó de IA, **todo o resto do correio funciona** e o painel diz porquê.

#### Adicionado — interface

- Seis ecrãs: caixa, leitura, composer, composer sem IA, definições, e o estado
  «não configurado». Todos no catálogo que testa UI sem comportamento.
- Cinco ícones novos no sprite (`mail`, `star`, `reply`, `archive`, `trash`) —
  catálogo passa de 37 para **42**, com verificação cruzada bidireccional.
- `select_labelled`, `field_with_value` e `textarea_with_value`: um composer
  re-renderizado com uma sugestão não pode devolver os campos em branco.

#### Alterado

- `IndexedMessage` passa a carregar `mailbox_id` e `folder`, para que abrir uma
  mensagem por identificador a mostre na sua própria caixa.
- `Screen::Mail` na navegação e na command palette, filtrada por `mail.use`.
- O Core **recusa arrancar** com correio parcialmente configurado, ou com
  correio configurado e lista de domínios institucionais vazia.

#### Limitações declaradas

- **Ingestão IMAP: `PLANNED`.** `mail.sync` reporta `planned`. O adaptador
  autentica e envia; não sincroniza correio recebido.
- **Anexos: `PLANNED`.** Descritos na leitura, com a descarga declarada
  indisponível. Depende de object storage, que não está configurado.
- **Administração de caixas partilhadas: `PLANNED`.** Modelo e consultas
  existem; o ecrã não.
- **Agentes que actuam sobre correio: `NOT IMPLEMENTED`.** Exige ADR próprio.

#### Corrigido — durante esta implementação

- **As sete permissões de correio não eram concedidas a papel nenhum.** Estavam
  no catálogo, verificadas em cada rota e cada consulta, documentadas em ADR — e
  inalcançáveis. Tudo compilava, o clippy estava limpo, e os testes unitários
  passavam, porque cada um constrói o seu próprio principal. **Um teste de
  integração apanhou-o.**
- Acrescentado `nenhuma_permissao_fica_sem_papel_que_a_conceda`, que apanha a
  classe. Uma permissão sem papel entra numa lista de excepções **com a razão
  escrita**, ou o teste falha.
- Esse guarda apanhou imediatamente um segundo órfão:
  **`agents.create.institutional` não era concedida a ninguém.** Passou para o
  `PlatformAdmin`, que já detinha `AgentsManage` — administrar todos os agentes
  e não poder criar um institucional era uma lacuna, não uma restrição. O
  `ResearchLead` **não** a recebeu: liderar um workspace não é autoridade sobre
  a instituição, e o teste que já guardava essa distinção manteve-se intacto.

#### Testes

De **349** para **414**, todos verdes. `cargo clippy --workspace --all-targets
-- -D warnings` limpo.

### Auditoria transversal de integridade funcional — 2026-08-22

Auditoria de ponta a ponta do Ocinye OS contra a invariante:

> **Se um membro vê uma opção no Ocinye Workspace, essa opção tem comportamento
> definido.**

#### Corrigido — dead UI

- **Os Agentes eram inteiramente interface.** Não existia tabela, nem endpoint,
  nem persistência. A lista lia `/api/v1/ai/models` e renderizava **modelos** sob
  cabeçalhos de agente; o formulário submetia para uma rota que só aceitava
  `GET`, devolvendo **405**. Agora existem `ai_agents`, `GET`/`POST
  /api/v1/ai/agents`, e o fluxo funciona de ponta a ponta **com zero nós de IA**.
- **`POST /ai/prompt` não existia**: submeter o Prompt devolvia 405. O endpoint
  foi criado e recusa com **503 e razão institucional** quando não há capacidade,
  que o Workspace mostra como estado nativo do ecrã.
- **O formulário de agentes não persistia nada.** Os nomes dos campos não
  correspondiam ao Core (`description` em vez de `purpose`, `k-bib` em vez de
  `uses_bibliography`), e o âmbito era um grupo de `<button>` sem `name` — tinha
  aparência de escolha e **nunca era submetido**. Substituído por radios reais.
- **O campo «Modelo base» foi retirado**: acoplava a UX a nomes de modelo, contra
  o §41 do `CLAUDE.md`. O agente pede **capacidade**; o Gateway escolhe o modelo.
- **~36 separadores de filtro nas listas** eram `<button role="tab">` sem
  handler. Declarados indisponíveis, com a razão.
- **O campo de pesquisa de cada lista** estava fora de qualquer formulário e sem
  handler. Passou a filtrar as linhas visíveis, e o rodapé acompanha a contagem.
- **Botão «Filtrar»** e **controlos de paginação** retirados: sem handler, e o
  «página seguinte» aparecia activo.
- **Sino de notificações** retirado, com o seu ponto de «não lidas» falso. O
  Ocinye Core não tem conceito de notificação.
- **Chips do Prompt** (Anexar, Dataset, Documento, Ferramentas) e a afirmação
  **«⏎ enviar»**: os primeiros passaram a declarados indisponíveis, a segunda foi
  retirada — sem JavaScript, Enter numa textarea insere uma linha.
- **Sugestões do Prompt** passaram a submeter o pedido que enunciam.
- **O selector de agente do Prompt** era um `aria-haspopup="listbox"` que não
  abria; passou a ligação para a lista de agentes.
- **`/admin/members/{id}` existia e nada lhe ligava.** As linhas da lista de
  membros passaram a levar ao detalhe.

#### Corrigido — verdade dos estados

- **Uma recusa do Core aparecia como lista vazia.** `optional()` engole erros, o
  que serve um painel secundário e é errado para o conteúdo principal: `/audit`
  devolvia «0 eventos» a quem não tem `audit.view`. As listas passaram a usar
  `required()`, e uma recusa aparece como recusa.
- **Os cartões da Home diziam `0`** quando o Core recusava a contagem —
  apresentando «sem acesso» como «não existe». Um cartão sem contagem real deixou
  de ser mostrado.
- **A acção primária das listas ignorava permissões.** «Novo Agente» aparecia a
  um `PlatformAdmin`, que não detém `agents.create.personal`. Cada lista declara
  agora a permissão que a sua acção exige.
- **Um `403` do Core caía no ecrã de «erro inesperado».** Passou a ter tratamento
  próprio, com saída para o trabalho do membro.
- **Uma recusa renderizava o ecrã de login**, pelo que um membro com sessão
  válida via um formulário de autenticação e concluía que a sessão terminara.

#### Adicionado — modelo de capacidades

- **`Capability` e `CapabilityState`** em `ocinye-contracts`: dez capacidades,
  seis estados (`available`, `no_resource`, `not_configured`, `unavailable`,
  `degraded`, `planned`). Nunca `OFFLINE` para tudo.
- **`GET /api/v1/system/capabilities`**: uma resposta, apurada de linhas reais,
  com a razão de cada estado e a dependência que aguarda. Sem isto, `if no_gpu`
  espalhava-se por vinte componentes.
- **Permissão e disponibilidade separadas.** «Não pode utilizar IA» e «pode, mas
  não há nó» são situações diferentes e passam a ter mensagens diferentes.
- **Estado de agente derivado** da disponibilidade real: `configured` sem
  capacidade, `ready` com ela. Um agente nunca diz `active` sem poder executar.

#### Adicionado — superfícies em falta

- **Ecrã de pesquisa** (`/search`). O Core servia `/api/v1/search` e não havia
  por onde lá chegar: a caixa da barra superior abria a command palette, que
  filtra navegação e não procura em nada. Cada resultado mostra a sua
  classificação; a pesquisa semântica é declarada indisponível com a razão.
- **Ecrãs de 404, recusa e falha**, coerentes com o Ocinye OS. Antes, um caminho
  desconhecido devolvia o 404 vazio do Axum. O Core ganhou o mesmo fallback, com
  o envelope de erro habitual.
- **Ecrã de detalhe de membro** ligado a partir da lista, com separadores de
  Acesso e Segurança.

#### Adicionado — guardas contra regressão

Três testes percorrem **todos** os ecrãs renderizados e falham se a invariante
quebrar:

| Teste | O que impede |
|---|---|
| `nenhum_botao_existe_sem_comportamento` | Um `<button>` que não submete, não tem handler e não se declara indisponível. |
| `nenhum_campo_existe_sem_destino` | Um `<input>` fora de formulário e sem handler. |
| `nenhuma_ancora_leva_a_lado_nenhum` | `href="#"` ou `href=""`. |

#### Verificado

- **349 testes**, todos verdes. `./scripts/verify.sh` com `EXIT=0`.
- **Navegação completa por HTTP** com Core e Workspace reais: 19 rotas a 200 e o
  caminho desconhecido a 404.
- **Fluxo de agentes de ponta a ponta com zero nós**: criação, persistência,
  estado `configured`, nome duplicado recusado, tecto de classificação imposto.
- **Fixture de nó** (removida em seguida): registar **um** nó e **um** modelo fez
  `compute` e `ai.general` passarem a `available` e os agentes a `ready`, **sem
  alterar código, migration ou interface**. É a invariante que garante que o
  CAM-01 será integrado pela arquitectura existente.

#### Documentação

- **[docs/feature-status/](docs/feature-status/README.md)** — matriz factual do
  que existe, do que está indisponível e porquê.
- **[docs/ui-core-contract/](docs/ui-core-contract/README.md)** — os dois
  princípios e como são impostos por teste.
- `docs/ai/`, `docs/compute/` e `docs/search/` actualizados.


### Identidade, autenticação e autorização — 2026-08-22

A camada de Identity & Access Management do Ocinye OS. **Componente
security-critical**, e uma reversão arquitectural registada em ADR.

#### Alterado — decisão arquitectural

- **A autenticação passa a ser feita no Ocinye Core**
  ([ADR-0103](docs/adrs/0103-core-owned-authentication.md)), que **substitui** o
  [ADR-0102](docs/adrs/0102-identity-provider.md) (Keycloak por OIDC). Nome de
  utilizador e palavra-passe, como factor único.
- **`MFA = NOT IMPLEMENTED`, e não exigido nesta fase.** É uma redução real de
  segurança face ao ADR-0102, assumida e documentada — não escondida. Treze
  ficheiros que afirmavam MFA obrigatório foram corrigidos.
- **`CLAUDE.md` §33 reescrito.** A norma anterior proibia autenticação no Core;
  o ADR-0103 explica porquê mudou e o que se perdeu. As proibições que continuam
  sem excepção — nunca criptografia própria, nunca esquema de sessão inventado,
  nunca uma palavra-passe armazenada — ficaram explícitas.
- **Keycloak removido** da infraestrutura, por ter deixado de ser usado. O
  módulo `oidc` do Workspace foi eliminado pela mesma razão (`CLAUDE.md` §53).

#### Adicionado — autenticação

- **Verificadores Argon2id** em formato PHC, com salt por hash e parâmetros
  configuráveis, validados **no arranque em todos os ambientes**: um Core que
  não consegue calcular verificadores em condições recusa-se a arrancar.
- **Rehash transparente**: subir o custo não invalida palavras-passe; um
  verificador antigo é substituído em silêncio no início de sessão seguinte.
- **Política de palavras-passe** ([ADR-0104](docs/adrs/0104-password-policy-and-hashing.md)):
  mínimo de 15 caracteres, máximo de 256, sem regras de composição, passphrases e
  Unicode aceites, **sem rotação periódica**. Normalização NFC e mais nada — sem
  `trim`, sem mudança de caixa, sem truncagem.
- **Blocklist local e versionada**, com canonicalização que dobra substituições
  *leet* e corta dígitos finais, mais detecção por código de padrões repetidos,
  percursos de teclado e o nome da instituição. Nenhuma palavra-passe sai da
  instituição, em nenhuma forma.
- **Credenciais temporárias** geradas por CSPRNG: ~139 bits de entropia, alfabeto
  sem caracteres ambíguos, validade de 24 horas configurável, apresentadas **uma
  única vez** e consumidas ao serem usadas.
- **Sessões server-side** com identificador opaco de 256 bits; só o digest
  SHA-256 é persistido. Cookie `HttpOnly` · `SameSite=Strict` · `Secure`.
  **Não existe promoção de sessão**: uma sessão restrita é revogada e
  substituída, nunca elevada no lugar.
- **Throttling** por conta e por origem de rede, em janela. Deliberadamente
  **não** é bloqueio de conta: bloquear ao fim de N falhas entrega uma negação
  de serviço a quem souber um nome de utilizador.
- **Mensagem única de recusa** para utilizador inexistente, palavra-passe
  errada, credencial expirada e conta suspensa — com verificação contra um
  verificador dummy quando não há conta, para igualar o tempo de resposta.
- **Bootstrap do primeiro administrador**: `ocinye-core-server bootstrap-admin`,
  que corre uma única vez (verificado antes e dentro da transacção), sem flag de
  override, e que também começa com credencial temporária. **Não existe
  credencial por omissão em parte alguma do código ou da configuração.**

#### Adicionado — autorização

- **53 permissões nomeadas**, quatro âmbitos e grants explícitos
  ([ADR-0101](docs/adrs/0101-permissions-scopes-and-grants.md)). Toda a pergunta
  de autorização passa por `can(actor, permission, contexto)`; **`if role ==
  admin` não existe em lado nenhum.**
- **Duas portas independentes**: a permissão decide *que operação*, a
  classificação decide *que material*. É isso que permite ao `PlatformAdmin`
  administrar a plataforma sem ganhar acesso a ciência `RESTRICTED`.
- **`ResearchLead` e `ExternalCollaborator`** juntam-se aos papéis técnicos. O
  segundo detém **zero** permissões institucionais, por desenho.
- **Grants explícitos** com razão escrita obrigatória, autor, âmbito nomeado e
  expiração opcional. Ninguém pode conceder o que não detém.
- **Acesso explicável**: `explain()` devolve a origem concreta de cada permissão
  — papel, membership de unidade, membership de workspace, ou grant.
- **Extractor tipado `Authorised<T>`**: a permissão é verificada **antes** de o
  corpo do pedido ser deserializado. Sem isto, um chamador não autorizado
  enviava `{}` e recebia um 422 que lhe descrevia o schema da operação.

#### Adicionado — interface

- **Ecrã de início de sessão** funcional: submete ao Core, com `Mostrar
  palavra-passe`, gestores de palavras-passe e colar a funcionar, e sem
  `maxlength` que trunque em silêncio.
- **Ecrã de primeiro acesso**, com a linguagem visual do OS e não de um
  formulário de recuperação. Declara a regra, mostra a recusa do Core tal como
  veio, e oferece sempre uma saída.
- **Administração de membros**: criar utilizador, apresentação da credencial
  **uma única vez** (coberta por omissão, com `Mostrar` e `Copiar`), e detalhe
  com separadores de Acesso e Segurança.
- **Navegação permission-aware**: barra lateral, menu `+ Criar` e command
  palette renderizam apenas o que o membro pode alcançar. Um grupo sem itens
  visíveis desaparece com o seu título.

#### Corrigido

- **O Workspace lia o envelope de erro errado** (`error.message` em vez de
  `message`), pelo que a mensagem do Core nunca chegava ao membro: uma recusa de
  palavra-passe apareceria como «the operation could not be completed». Agora
  preso por um teste construído a partir do próprio tipo do Core.
- **Ordem de canonicalização na blocklist**: dobrar substituições antes de cortar
  dígitos finais transformava o `123` de `Password123` em letras, e a entrada
  deixava de casar. Apanhado pelos próprios testes.
- **Texto ilegível no ecrã de primeiro acesso**: `.oc-first__lead` e
  `.oc-first__rules` não tinham estilo e herdavam tinta escura sobre navy — a
  regra que o membro precisa de ler era a que não se lia.
- **`ck_people_status`**: `departed` substituído por `disabled`, que conflatia
  «saiu da instituição» com «não pode entrar».

#### Verificado

- **312 testes**, todos verdes. 306 na workspace mais 6 na capacidade WASM.
- **15 testes E2E de identidade** contra PostgreSQL real, provando entre outros
  que uma credencial temporária nunca abre sessão normal, que expira, que é
  consumida ao ser usada, e que **nenhuma palavra-passe aparece em
  `credentials`, `audit_events` ou `authentication_attempts`**.
- **Testes de permissão exaustivos**: enumeram todas as combinações de papel,
  permissão e classificação em vez de amostrar. Cada fronteira tem ALLOW e DENY.
- **Fluxo completo exercitado por HTTP** contra Core e PostgreSQL reais:
  bootstrap → login restrito → toda a API institucional recusada (403) →
  política de palavra-passe → mudança → rotação de sessão → criação de membro →
  reset → suspensão. Zero plaintext em base de dados, auditoria e log.
- **8 constraints de identidade verificadas contra a base**: temporária sem
  expiração, permanente com expiração, verificador não-Argon2id, nome de
  utilizador com espaço, grant institucional com alvo, grant com âmbito sem
  alvo, grant sem razão, e duas credenciais permanentes activas.


### Passagem visual e verdade do repositório — 2026-08-22

Revisão ecrã a ecrã dos 20 ecrãs renderizados em Chrome headless, e alinhamento
da documentação normativa com o estado real.

#### Corrigido

- **Cartão de IA do painel**: o título saía num `<div>` sem classe, e o CSS só
  estilizava `.oc-ai-panel h2` — texto escuro sobre fundo navy, ilegível, e
  sobreposto ao parágrafo seguinte. Passou a `<h2>`, como nos restantes cartões.
- **«Continuar trabalho» vazio**: o cartão é montado à mão e omitia o
  `oc-card__body`, pelo que a frase de estado vazio encostava à moldura. Os
  tiles do ramo cheio já traziam o seu; o ramo vazio passou a trazer também.
- **Controlos desactivados pareciam activos**: `.oc-prompt__send` e
  `.oc-suggestion` só mudavam o cursor. Passam a usar `opacity: .4`, a mesma
  convenção de `.oc-page-btn[disabled]`.
- **Campos desactivados sem estilo**: `.oc-input`, `.oc-textarea` e `.oc-select`
  não tinham regra `:disabled`. Ganharam fundo apagado e texto `meta` — não
  opacidade, porque nestes ecrãs o conteúdo do campo é muitas vezes a
  explicação de por que está vazio.
- **Selector de modelo em branco**: sem modelos, a única `<option>` saía
  `disabled`, e um browser não mostra uma opção desactivada — o campo aparecia
  vazio em vez de dizer porquê. O `disabled` passou para o `<select>`, e uma
  opção indisponível deixou de submeter a sua frase como valor.
- **Hub de IA repetia-se**: título e corpo traziam a mesma frase à letra. O
  título passou a nomear o estado; o corpo mantém a frase do Core.
- **Lista de agentes contradizia o próprio botão**: dizia que «um agente precisa
  de um modelo» ao lado de um «Novo Agente» activo. Por §41 um agente define-se
  por capacidade, não por modelo; a frase passou a dizer o que falta — o nó.
- **Lista de membros contradizia a contagem**: «Ainda não há membros para além
  de si» aparecia ao lado de «0 membros», e o estado vazio só surge com zero
  linhas — a frase nunca era verdade quando era vista.
- **Estado vazio de ideias** repetia o subtítulo do ecrã à letra.
- **Pré-visualização do Research Workspace** fixava `Screen::Projects` para uma
  fixture que é uma Ideia, mostrando um trilho que o router nunca produz.

#### Alterado

- **`CLAUDE.md` §1, §19, §66 e §67** descreviam o repositório vazio de antes da
  implementação. Reescritas com os factos verificáveis por `./scripts/verify.sh`
  (§69, §83).
- **Perfis `dev` e `test`** passam a `debug = "line-tables-only"`: a árvore de
  dependências gerava ~9,5 GB em `target/debug`, e um backtrace continua a
  apontar ficheiro e linha. Passou a ~1,5 GB.

#### Verificado

- `./scripts/verify.sh` verde de ponta a ponta: fmt, clippy `-D warnings`,
  capacidades WASM, 183 testes, builds de release, varrimento de segredos e
  validação do Compose.
- **Os 8 testes de autorização correram pela primeira vez contra PostgreSQL
  real** (18.1, pgvector 0.8.1) em vez de se saltarem. As 7 migrations aplicam
  de base vazia e produzem 30 tabelas.


### Ocinye Workspace — implementação do dossier de design — 2026-08-22

Os 20 ecrãs do handoff de design, implementados em Leptos SSR.

#### Adicionado

- **Dossier de design** em [`design/`](design/README-implementacao.md): 20 ecrãs,
  tokens, 37 ícones, logótipo e protótipo navegável, trazidos para o repositório
  como fonte de verdade visual.
- **Sistema visual** em `static/ocinye.css`: todos os tokens de
  `DESIGN_TOKENS.md`, reset, focus ring dourado global, duas animações,
  `prefers-reduced-motion`.
- **Componentes partilhados**: `DataTable` (usado pelos oito ecrãs de lista),
  `Badge` (sete tons, sempre ponto e texto), `Button` (quatro variantes),
  `Tabs` (pill e contextual), `Field`/`Textarea`/`Select`/`Toggle`/`Checkbox`,
  `EmptyState`, `ProgressBar`, `ProgressDonut`, `Card`, `Kpi`.
- **Shell autenticada**: sidebar colapsável de cinco grupos, topbar com
  breadcrumb, pesquisa global, menu `+ Criar`, notificações, estado do Core e
  avatar.
- **Command palette** `⌘K` com grupos `NAVEGAR` e `ACÇÕES`, filtro local,
  navegação por setas e devolução do foco ao fechar.
- **Os 20 ecrãs**: login, painel, O Meu Trabalho, unidades e detalhe, ideias,
  projectos, Research Workspace, conhecimento, bibliografia, dados, Ocinye AI,
  agentes, criar agente, Prompt Ocinye, computação, actividade, administração,
  audit log e criação de ideia.
- **Camada de interacção** (`static/app.js`): palette, sidebar, menu de criação,
  tabs locais e densidade das tabelas. Sem dados, sem autorização, sem tokens.
- **Testes de fidelidade ao design**: lêem o dossier e comparam token a token,
  ícone a ícone.
- **Guarda contra ligações mortas**: um teste renderiza todos os ecrãs e falha
  se alguma ligação apontar para um caminho sem rota.

#### Alterado

- A interface anterior do Workspace foi substituída pela do dossier. As camadas
  de sessão, OIDC, cliente do Core e cabeçalhos de segurança mantiveram-se.
- [ADR-0600](docs/adrs/0600-leptos-workspace-runtime.md) refinado por
  [ADR-0602](docs/adrs/0602-workspace-ssr-progressive-enhancement.md): a condição que
  adiava a hidratação foi reavaliada agora que existe interactividade real.

#### Corrigido

- As sondas ao fornecedor de identidade e ao Core não tinham limite próprio. Com
  o Docker parado a reter a porta, o ecrã de início de sessão ficava pendurado
  até ao timeout geral do cliente em vez de dizer o estado. Ambas passaram a ter
  limite curto.

#### Desvios ao dossier, declarados

- `/ideas/{id}` e `/projects/{id}` encaminham para `/workspaces/{id}`: promover
  uma ideia mantém o mesmo Research Workspace, e um URL canónico é mais
  verdadeiro do que dois para o mesmo objecto.
- As acções cujo ecrã o dossier não especifica ficam visíveis e declaradas como
  indisponíveis, em vez de levarem a um 404.
- Leptos SSR em vez de React, por Rust-first ser princípio institucional.

#### Por fazer, declarado

- Paginação, ordenação e filtros: os endpoints do Core aceitam os parâmetros; os
  controlos da UI ainda não os submetem.
- Comparação visual lado a lado com o protótipo num browser.

---

### Fundação do Ocinye OS — 2026-08-22

Primeira implementação da camada digital do Primeiro Núcleo Computacional.

#### Adicionado

**Princípio tecnológico**

- **Rust-first** inscrito como princípio arquitectural oficial da Ocinye
  (`CLAUDE.md` §16-A, [ADR-0004](docs/adrs/0004-rust-first.md)).

**Crates**

- `ocinye-contracts` — tipos canónicos institucionais. 13 testes.
- `ocinye-domain` — workflows e política de autorização, puros. 33 testes,
  incluindo a equivalência exaustiva entre a política e o filtro SQL.
- `ocinye-observability` — logging estruturado e correlação. 6 testes.
- `ocinye-core` — 10 módulos de domínio, persistência e serviços de aplicação.
- `ocinye-capabilities` — Capability Runtime WASM/WASI com limites verificados.

**Serviços**

- `core-server` — API `/api/v1`, cabeçalhos de segurança, health e readiness
  honestos.
- `worker` — drena o outbox transaccional; mantém a disponibilidade de modelos
  honesta face à liveness dos nós.
- `node-agent` — **esqueleto**: enrola, faz heartbeat, reporta recursos. Não
  executa jobs.

**Workspace**

- Interface Leptos SSR com BFF OIDC: os tokens ficam no servidor, nunca no
  browser.
- Design system próprio; classificação visível e nunca só por cor.
- Painel, unidades, Research Workspace, criação de ideia, pesquisa, actividade,
  estado de IA e de computação.

**Dados**

- 7 migrations, verificadas contra PostgreSQL 17 + pgvector.
- Auditoria append-only imposta por trigger.
- 9 invariantes impostos pela própria base de dados.

**Capacidades**

- `bibtex-import` — capacidade de exemplo compilada para `wasm32-wasip1`.

**Documentação**

- 16 ADRs.
- README raiz como mapa; README por componente.
- Arquitectura, segurança, modelo de ameaças, modelo de dados, autorização,
  domínio, IA, computação, protocolo de nó, capacidades, WASM, identidade,
  conhecimento, storage, pesquisa, desenvolvimento, testes, operação, backups,
  runbooks, deployment.

**Infraestrutura**

- Docker Compose: PostgreSQL + pgvector, Redis, MinIO (bucket privado), Keycloak.
- Realm Keycloak com MFA obrigatória, PKCE e mapeamento de audiência.
- CI: formatação, clippy, scan de segredos, auditoria de dependências, testes com
  base de dados, validação de migrations, builds de release, validação da stack.

#### Corrigido

Três defeitos encontrados por fazer os testes correr de verdade:

- **Pesquisa completamente não funcional.** `websearch_to_tsquery` e
  `to_tsvector` recebiam a configuração de texto como `text` em vez de
  `regconfig`, o que fazia toda a pesquisa falhar em tempo de execução.
  Apanhado pelo teste de fuga por pesquisa — exactamente o risco que
  [ADR-0009](docs/adrs/0009-postgresql-sqlx.md) declarou ao escolher SQL
  verificado em tempo de execução.

- **Limites do Capability Runtime mal classificados.** O esgotamento de fuel era
  classificado por correspondência de texto na mensagem de erro, e reportado como
  falha ordinária. Passou a ser classificado pelo tipo do trap
  (`Trap::OutOfFuel`, `Trap::Interrupt`), pelo que um operador consegue
  distinguir "esta capacidade precisa de mais fuel" de "esta capacidade está
  avariada".

- **Testes do sandbox WASM a saltar em silêncio.** O script de build produzia o
  componente no directório errado. Como o cargo esconde o output dos testes que
  passam, os quatro testes saltavam e reportavam sucesso — e os dois defeitos
  acima sobreviveram atrás desse verde. O script passou a construir para o
  `target` partilhado, e os testes passaram a **falhar** com instruções em vez de
  saltar.

#### Alterado

- A suite de autorização passou a distinguir "não configurado" de "avariado":
  salta se `OCINYE_TEST_DATABASE_URL` não estiver definida, e **falha** se
  estiver definida e a base for inalcançável. A CI define-a sempre, pelo que não
  pode perder esta cobertura em silêncio.

#### Estado real

- **Nós computacionais: 0.** `CAM-01` não existe.
- **Fornecedores de IA: 0.** Nenhum fornecedor externo é usado em substituição.
- **Pesquisa semântica: indisponível.** Sem embeddings, porque sem nó de IA.
- **Backups: não configurados.** Nenhum restore testado.
- **Deploy: nenhum.** Nada está em produção.
- **Fluxo OIDC ponta a ponta: não verificado** contra um IdP a correr.
