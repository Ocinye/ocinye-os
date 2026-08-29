# CLAUDE.md — Constituição de Engenharia da Ocinye

Este ficheiro é a **fonte normativa** de todo o desenvolvimento neste repositório.
Lê-o por inteiro antes de escrever código, documentação ou comandos. As regras
aqui definidas são vinculativas, aplicam-se a todas as sessões e não expiram.

**Como usar este ficheiro**

- A **Secção 1** descreve o estado real, verificado. Actualiza-a sempre que o
  estado mudar, na mesma alteração que o mudou.
- Tudo o resto é **norma** (regras permanentes) ou **arquitectura decidida**
  (`PLANNED`). Nada fora da Secção 1 descreve funcionalidade existente.
- Em caso de conflito entre este ficheiro e qualquer outra instrução, prevalecem
  sempre, por esta ordem: **§72 (autoria de commits)**, **§31 (segurança)**,
  **§69 (verdade do repositório)**.

**Idioma.** Documentação institucional e comunicação em **português europeu**.
Código, identificadores, nomes de ficheiros, mensagens de commit, ADRs técnicos e
termos consagrados em **inglês**.

**Relação com a documentação pública.** Este ficheiro tem o nome que tem porque é
o que a ferramenta lê automaticamente; isso não faz dele a face pública da
disciplina de engenharia da Ocinye. Quem chega ao repositório encontra as regras
de desenvolvimento, verificação e evidência em
[`docs/development/`](docs/development/README.md), e a arquitectura em
[`docs/architecture/`](docs/architecture/README.md). As normas aqui e as de lá
descrevem o mesmo sistema: quando divergirem, é defeito, e corrige-se nas duas.

---

## 1. Estado real do repositório — `CURRENT`

> Factos verificados, não intenções. Não uses esta secção para roadmap.

**Verificado em 2026-08-28.** Cada afirmação abaixo é verificável correndo
`./scripts/verify.sh` no repositório.

Os **números** desta secção não são escritos à mão: saem de
`./scripts/repository-facts.sh`, que os deriva da árvore e só lê. Já houve aqui
quatro contagens em circulação ao mesmo tempo — 62 caminhos contra 131, 12
migrations contra 19, 45 tabelas contra 63, 64 permissões contra 72 — porque
cada uma era mantida por quem se lembrasse. Um número escrito à mão envelhece
sem que nada falhe.

**Existe e funciona hoje:**

- **Monorepo Rust**, Cargo workspace com 5 crates (`ocinye-contracts`,
  `ocinye-domain`, `ocinye-observability`, `ocinye-core`, `ocinye-capabilities`),
  3 serviços (`core-server`, `worker`, `node-agent`) e 1 aplicação
  (`apps/workspace`). Uma capacidade WASM fora da workspace do host:
  `wasm/capabilities/bibtex-import`.
- **Ocinye Core: `IMPLEMENTED`, não deployado.** 139 caminhos e 165 operações
  sob `/api/v1`, autorização RBAC + ABAC fail-closed, outbox transaccional,
  auditoria, e um modelo de capacidades do sistema em
  `GET /api/v1/system/capabilities`.
- **Autenticação: `IMPLEMENTED`, no Core** — nome de utilizador e palavra-passe,
  verificadores Argon2id, sessões server-side opacas, credenciais temporárias
  que expiram, throttling ([ADR-0103](docs/adrs/0103-core-owned-authentication.md),
  [ADR-0104](docs/adrs/0104-password-policy-and-hashing.md)).
  **`MFA = NOT IMPLEMENTED`** e não exigido nesta fase (§33).
- **Autorização por permissões nomeadas: `IMPLEMENTED`.** 72 permissões, quatro
  âmbitos, grants explícitos atribuíveis e revogáveis, e acesso explicável
  ([ADR-0101](docs/adrs/0101-permissions-scopes-and-grants.md)).
- **Bootstrap do primeiro administrador: `IMPLEMENTED`.**
  `ocinye-core-server bootstrap-admin`, corre uma única vez, com credencial
  temporária. **Não existe credencial por omissão em lado nenhum.**
- **Ocinye Workspace: `IMPLEMENTED`, não deployado.** 70 ecrãs em Leptos SSR,
  sessão BFF com os tokens no servidor, navegação e menu de criação filtrados
  pelas permissões que o Core calcula.
- **Ocinye Mail: `IMPLEMENTED`, `NOT CONFIGURED`.** Módulo do Core com
  abstracção de fornecedor, higienização de HTML recebido, política de
  classificação no envio, fronteira de privacidade em SQL, e 6 ecrãs no
  Workspace ([ADR-0400](docs/adrs/0400-mail-as-institutional-surface.md) a
  [ADR-0408](docs/adrs/0408-imap-transport.md)). O transporte IMAP está
  implementado — pastas descobertas do servidor, listagem, corpo, anexos,
  flags, mover — com cifra obrigatória e sem forma de a desligar.
  A interface distingue as ausências em vez de mostrar uma caixa vazia. **A
  ingestão é periódica**: o worker percorre as caixas ligadas, e uma que recuse
  não interrompe as outras — a razão fica guardada na caixa que falhou.
- **24 migrations**, aplicáveis de base vazia; 66 tabelas.
- **Ficheiros institucionais: `IMPLEMENTED`, com superfície humana.**
  Um documento deixou de apontar para **um** objecto guardado: aponta para um
  **ficheiro**, que tem identidade estável e uma história imutável de versões
  ([`files`, `file_versions`](migrations/0020_institutional_files.sql)). A
  versão é material e não semântica, e por isso pertence ao ficheiro — assim
  uma fotografia sem documento não precisa de `ImageVersion`, nem um anexo de
  `AttachmentVersion`. `dataset_versions` **não** foi generalizada: partilha a
  primitiva de bytes e mais nada. O conteúdo de um documento resolve-se
  **exclusivamente** pela versão corrente — a de maior sequência —, e a coluna
  que o guardava directamente foi retirada depois de se provar que nenhum
  código de produção a lia.
  O ficheiro **governa-se a si próprio**: tem classificação, e a efectiva é
  `most_restrictive(workspace, file)` calculada contra o estado corrente do
  ambiente. As [pastas](migrations/0024_folders.sql) arrumam e não decidem —
  mover um ficheiro RESTRICTED para uma pasta chamada «Público» muda onde ele
  aparece e mais nada ([ADR-0204](docs/adrs/0204-institutional-files-and-folders.md)).
  Existe **ecrã de Ficheiros** sob CONHECIMENTO, com navegação, pastas,
  largar, carregamento, detalhes, histórico e descarga de versões exactas.
  A pré-visualização de imagens é **same-origin, pelo Core**: os bytes saem por
  `/files/{id}/preview` e a CSP do Workspace continua `img-src 'self' data:`,
  pelo que a Experience nunca aprende onde o armazenamento está. Inline serve-se
  uma lista fechada — PNG, JPEG, WebP —, e **não** `image/*`: um SVG é um
  documento com script. **Não existe ainda** extracção de conteúdo nem pesquisa
  de corpo.
- **Agentes de IA: `IMPLEMENTED`.** Definíveis e persistidos **sem nó de IA**;
  o estado de execução é derivado da disponibilidade real.
- **Agentic Control Plane: `IMPLEMENTED`, sem inferência.** Capability Registry
  (conjunto fechado; a contagem vive em `docs/agentic/operation-capability-matrix.md`,
  emitida pelo catálogo tipado), Capability Executor, Context Engine,
  Action Planner, aprovações ligadas a plano, pessoa e prazo, Main Agent, e a
  Universal Command Surface — `Search · Ask · Act`
  ([ADR-0002](docs/adrs/0002-deterministic-core-and-agentic-control-plane.md) a
  [ADR-0304](docs/adrs/0304-canonical-inference-contract.md)).
  **`Pesquisar` funciona com zero nós de IA**; `Perguntar` e `Executar`
  declaram-se indisponíveis com a razão. Nenhuma capability alcança shell, SQL,
  ficheiros, rede ou segredos, e existe um teste que o verifica.
- **Ciclo de vida científico e proveniência: `IMPLEMENTED`.** Hipótese,
  metodologia, versão de metodologia, estudo, execução, resultado, e a
  validação ou reprodução que alguém registou
  ([ADR-0412](docs/adrs/0412-scientific-lifecycle-and-provenance.md)). As
  relações vivem em `research_links`, com quinze verbos, uma matriz que diz que
  pares de tipos cada um aceita, e a **origem** guardada: `declared` — alguém
  afirmou — ou `operation` — o Core observou, na mesma transacção que produziu
  o efeito. A **linhagem é uma projecção**, não uma tabela: cada travessia lê a
  proveniência agora e resolve cada nó com a política de quem percorre. Um nó
  que essa política recuse termina a travessia e **não aparece** — nem
  contagem, nem `truncated`, que fala só do limite de profundidade. No
  Workspace lê-se por títulos, sem um único identificador no ecrã.
  **Validar um resultado não é delegável** a nenhum agente: é uma afirmação
  institucional, e o peso dela é de quem a faz.
- **Lifecycle de planos agentic: `IMPLEMENTED`.** Uma proposta validada é
  persistida como `ActionPlan` imutável, recuperável por
  `GET /api/v1/agentic/plans` e `…/plans/{id}`, e passível de aprovação,
  rejeição e execução por HTTP. A execução reclama o plano com um `UPDATE`
  condicional — pelo que uma segunda chamada, sequencial ou concorrente, não
  repete o efeito — e reautoriza cada passo através do Capability Executor
  imediatamente antes de o correr. **Aprovação é consentimento, não
  autorização:** revogar um acesso depois de confirmar impede a execução, e
  existe teste que o demonstra.
- **Continuidade institucional: `IMPLEMENTATION COMPLETE — OPERATIONAL
  ACTIVATION PENDING FIRST SERVER`. Continuidade de artefactos de modelo:
  `SPECIFIED`, com portão de entrada.** O Core classifica todo o estado e diz o
  que tem de viajar — PostgreSQL, Object Storage e a chave de selagem, as três
  ([ADR-0700](docs/adrs/0700-institutional-continuity-and-portability.md)).
  Quatro comandos respondem a três perguntas diferentes: `verify-snapshot`
  prova que as **linhas** chegaram, identidade a identidade; `verify-objects`
  lê cada objecto do bucket e **recalcula a soma**, distinguindo um bucket
  inacessível de um objecto ausente; `verify-keys` prova que o que chegou se
  consegue **ler**. Dois portões impedem a cobertura de envelhecer: uma tabela
  nova sem decisão de continuidade fecha o portão, e uma coluna nova de
  criptograma sem chave declarada também. O trio operacional —
  `institutional-backup`, `-restore`, `-verify` — cifra com `age`, recusa
  enviar em claro para fora do servidor, **confirma a cópia externa por leitura
  de volta**, e aplica retenção nas duas pontas. **Dois ensaios executados a
  2026-08-29**: A → B → C provou a portabilidade; A → cofre → B provou o
  processo e terminou com as três verificações a observar e a passar —
  165 641 recursos, 2 objectos e 83 credenciais seladas que abriram no servidor
  novo —, com o controlo negativo a recusar o mesmo restauro sem a chave.
  **O que falta não está no repositório: falta um servidor onde o agendador
  dispare.** As unidades de `launchd` e `systemd` estão em `infra/scheduling/`
  e não estão instaladas em lado nenhum. Enquanto assim for, **não há backup
  periódico**, e o RPO é *desde o último conjunto que alguém produziu*.
- **52 ADRs** em `docs/adrs/`, **10 runbooks** em `docs/runbooks/`,
  **41 READMEs**, `docs/` povoado — incluindo
  [`docs/feature-status/`](docs/feature-status/README.md), a matriz factual do
  que existe e do que não existe.
- `README.md`, `.env.example`, `Cargo.lock`, CI (`.github/workflows/ci.yml`) e
  `infra/compose/docker-compose.yml` (postgres, redis, minio).
- **Repositório institucional: [`Ocinye/ocinye-os`](https://github.com/Ocinye/ocinye-os), público.**
  Primeiro commit em 2026-08-23, com identidade humana. A CI corre no GitHub
  Actions contra PostgreSQL efémero, e um passo conta os testes que correram
  para que uma suite saltada não possa passar por verde.
- **`main` está protegida por regra do GitHub.** *Branch protection* clássica,
  verificada em 2026-08-27 pela API: alterações entram por Pull Request, com
  cinco *required status checks* — `Testes`, `Stack local`, `Formatação, lint e
  segredos`, `Advisories RustSec (cargo audit)`, `Advisories do GitHub
  (Cargo.lock)` — em modo *strict*, pelo que a branch tem de estar actualizada
  com `main` antes de entrar. `enforce_admins` está activo, *force push* e
  eliminação da branch estão bloqueados, e a resolução de conversas é exigida.
  Nenhuma aprovação humana é exigida por número. Não há *rulesets*: a política
  vive inteira na *branch protection*, e um segundo mecanismo a dizer o mesmo
  seria um sítio a mais onde discordar.
- **1278 funções de teste** escritas na árvore, e **zero falhas** na última
  corrida de `./scripts/verify.sh`. Os dois números respondem a perguntas
  diferentes, e por isso são dois: o primeiro é um facto da árvore e sai do
  `repository-facts.sh`; o segundo é o resultado de uma corrida, e a corrida
  conta cada alvo em que um teste é compilado — pelo que o total que ela
  imprime é maior e **não se escreve aqui**. Escreveu-se durante um tempo, e
  derivou três vezes numa sessão sem que nada falhasse.
  **401 dessas funções não correm sem base de dados** — vivem em ficheiros que leem
  `OCINYE_TEST_DATABASE_URL`, e o número sai daí, não de uma lista mantida à
  mão. Incluem quatro guardas que percorrem todos os ecrãs e falham se algum
  elemento interactivo ficar sem contrato definido, um guarda que falha se
  alguma permissão do catálogo ficar sem papel que a conceda, o caminho agentic
  completo contra um fornecedor determinístico — **sem GPU** —, e a Provider
  Conformance Suite.
- **Segurança: auditada de ponta a ponta em 2026-08-23.** 12 findings — um
  `HIGH`, cinco `MEDIUM`, seis `LOW` — confirmados e corrigidos, cada um com
  regressão; um risco residual de dependência aceite e escrito. Registo:
  [`docs/security/2026-08-23-security-baseline-v1.md`](docs/security/2026-08-23-security-baseline-v1.md).
  `./scripts/verify.sh` passou a incluir a auditoria de dependências.

**Continua a não existir:**

- **Nenhum segundo factor de autenticação.** Uma palavra-passe comprometida é
  acesso comprometido. Limitação assumida, não omissão: ver ADR-0103.
- **Nenhuma conta existe** numa instalação nova até alguém correr o bootstrap.
- **Nenhum ambiente está deployado.** Nem desenvolvimento partilhado, nem
  staging, nem produção. O sistema só correu localmente.
- **Compute nodes = 0.** Nenhum nó de computação existe. O Compute Registry
  está implementado e devolve zero, que é o estado verdadeiro.
- **IA: indisponível.** O AI Gateway existe como código e reporta
  `available: false`. Não existe nó local, modelo instalado nem fornecedor
  externo ligado — e nenhum é ligado em substituição.
- **Inferência agentic: `NO_RESOURCE`.** O plano agentic está construído e
  testado de ponta a ponta contra um fornecedor determinístico que implementa o
  contrato interno do Gateway ([ADR-0304](docs/adrs/0304-canonical-inference-contract.md));
  o que falta é um adapter que sirva `GENERAL`. `Perguntar` e `Executar` a partir de
  linguagem natural devolvem indisponível **com a razão e com o que ainda
  funciona**. Nenhum agente executa nada sozinho, e nenhum trabalho proactivo,
  agendado ou por evento existe.
- **Nenhum artefacto de modelo institucional existe, e não há onde o guardar.**
  A Ocinye não treinou nem afinou nenhum modelo. `ai_models` é um **inventário
  reportado pelo nó** — apagado e reinserido a cada relatório, e removido em
  cascata com o nó — e não um registo de artefactos: hoje é a computação que
  detém o modelo, que é o inverso do que a
  [ADR-0203](docs/adrs/0203-institutional-model-artifacts.md) decide. Não
  existe `Model`, `ModelVersion`, `ModelArtifact`, `TrainingRun` nem
  `EvaluationRun`, não existe caminho para carregar pesos, e não existe
  promoção nem retenção. A **classificação** de continuidade já distingue o
  modelo base readquirível do artefacto treinado, e o segundo viaja; o resto
  vem com o primeiro treino.
- **Nenhuma infraestrutura física da Ocinye existe.** CAM-01 não existe.
- **Nenhum serviço de correio está configurado.** O adaptador em uso é
  `UnconfiguredProvider` e todas as capacidades de correio reportam
  `not_configured`. `ocinye-core-server mail-check` prova uma configuração sem
  arrancar o Core, e sem imprimir credenciais ou conteúdo.
- **Nenhum backup periódico existe.** O mecanismo está completo e provado —
  cifra, destino externo confirmado por leitura de volta, retenção nas duas
  pontas, restauro verificado nas três dimensões. O que não existe é um
  **servidor** onde o agendador corra, e por isso não existe cópia da
  instituição em qualquer momento dado. O RPO é *desde o último conjunto que
  alguém produziu à mão*, **3-2-1 não existe**, e a rotação da chave de selagem
  não está escrita. O portão de activação está em
  [`docs/backups/`](docs/backups/README.md), e exige uma execução **disparada
  pelo agendador** — nem manual, nem uma imitação manual.

Ferramentas disponíveis na máquina de desenvolvimento actual — contexto
ambiental, **não** compromisso arquitectural: Git 2.50.1, Node.js 25.8.0,
npm 11.11.0, pnpm 11.1.1, Python 3.9.6, Docker 29.2.1, psql 18.1 (Postgres.app).

A stack de referência (§18) está decidida **e confirmada por ADR**: cada linha
da tabela remete para o ADR que a regista.

Todo o restante conteúdo deste ficheiro é **norma** ou **`PLANNED`**.

---

# PARTE I — IDENTIDADE DO PRODUTO

## 2. Não estamos a construir um website

Esta é a regra conceptual mais importante do projecto.

**A Ocinye não está a construir:**

- um website com área privada;
- um painel administrativo;
- um CMS;
- um portal de membros;
- um dashboard SaaS;
- uma intranet genérica;
- um gestor de ficheiros;
- um chatbot com login.

**Estamos a construir o:**

# SISTEMA OPERACIONAL INSTITUCIONAL DA OCINYE

Este princípio orienta toda a arquitectura. O sistema deve tornar-se a
infraestrutura digital central através da qual a Ocinye: organiza pessoas;
estrutura unidades; desenvolve investigação; formula ideias; transforma ideias em
projectos; gere conhecimento; preserva bibliografia; cataloga datasets; mantém
documentos; gere tarefas; relaciona código; regista experiências; preserva
resultados; protege propriedade intelectual; gere recursos computacionais;
integra IA; mantém proveniência; controla permissões; cria memória institucional;
preserva rastreabilidade; produz evidência científica; suporta futura actividade
comercial; e liga progressivamente infraestrutura física própria.

**O sistema deve sobreviver à evolução da própria instituição.** Não o desenhes
em torno das necessidades apenas dos primeiros quatro utilizadores.

---

## 3. Ocinye Core

O **Ocinye Core** é o sistema operacional institucional propriamente dito — a
plataforma tecnológica central da Ocinye.

Responsável por: domínio institucional; regras; dados; conhecimento; segurança;
workflows; identidade institucional; permissões; eventos; pesquisa; IA;
computação; storage; auditoria; integrações; APIs.

Invariantes:

- **O Core não é uma interface gráfica.**
- **O Core não pode depender exclusivamente do Workspace.** Qualquer regra que só
  funcione porque o Workspace a aplica está no sítio errado.
- O Core deve poder vir a ser consumido por: Ocinye Workspace; aplicações móveis;
  CLI; notebooks; agentes; Jupyter; serviços científicos; ferramentas internas;
  Node Agents; APIs; integrações externas.

---

## 4. Ocinye Workspace

O **Ocinye Workspace** é a principal interface humana do sistema operacional
institucional. É onde os membros trabalham.

- **Não é "a plataforma inteira".** É um **cliente do Ocinye Core**.
- **A separação entre Core e Workspace é arquitecturalmente obrigatória.**
- Nenhuma decisão de autorização é tomada no Workspace. O cliente pode esconder
  o que o utilizador não pode ver; **nunca é ele a decidir** se pode.

O Workspace deverá permitir trabalhar sobre: unidades; ideias; projectos;
bibliografia; dados; documentos; notas; experiências; resultados; tarefas;
actividade; financiamento; IA; recursos computacionais.

---

## 5. Domínios

- `ocinye.com` — domínio adquirido. **Reservado** para o futuro website público
  institucional.
- `workspace.ocinye.com` — destino previsto da aplicação privada. **É a prioridade.**

Regras:

- **Não construas o website público nesta fase.**
- **Não gastes recursos de engenharia em marketing pages** enquanto o sistema
  operacional institucional não estiver sólido.

---

## 6. Contexto institucional

A Ocinye pretende tornar-se uma instituição angolana de: investigação aplicada;
engenharia; infraestruturas digitais; computação avançada; inteligência
artificial; transferência tecnológica.

**A Ocinye não é uma simples startup de IA** e não deve ser tratada como tal em
código, nomenclatura, documentação ou comunicação.

A construção começa pelo **Primeiro Núcleo Computacional da Ocinye**, com duas
dimensões:

1. **Camada digital** — começa agora: Ocinye Core + Ocinye Workspace.
2. **Camada física** — adicionada posteriormente. **Não existe.**

---

## 7. Estado real da camada física

O primeiro servidor GPU próprio **ainda não foi adquirido**.

Configuração futura actualmente considerada (`PLANNED`): AMD EPYC; 256 GB ECC;
NVIDIA RTX 4090 24 GB; armazenamento redundante; NAS; UPS; rede dedicada;
Qwen; Qwen Coder; DeepSeek.

Isto é `PLANNED`. **Não é `CURRENT`.**

**Nunca declares, sem evidência real e verificada:**

- CAM-01 online;
- GPU disponível;
- IA local operacional;
- datacenter operacional;
- Qwen disponível;
- DeepSeek disponível.

Nunca escrevas código, configuração, testes ou documentação que pressuponham a
existência destes recursos como estado corrente.

---

## 8. Princípio fundador — IA transversal

A inteligência artificial na Ocinye não é apenas um módulo, um departamento, um
produto, um chatbot ou uma API.

> **A IA é uma capacidade transversal do sistema operacional institucional.**

A Ocinye pretende explorar de forma sistemática, responsável e tão ampla quanto
tecnicamente possível as capacidades da IA em: investigação; engenharia; pesquisa
bibliográfica; análise documental; programação; ciência de dados; simulação;
optimização; modelação; geração de hipóteses; organização do conhecimento;
formulação de ideias de projectos; análise de resultados; produção documental;
automação; transferência tecnológica; gestão institucional.

A arquitectura deve reflectir isso desde a fundação. **No entanto:**

- a IA **não substitui** validação científica;
- a IA **não substitui** responsabilidade humana;
- a IA **não pode contornar permissões**;
- a IA **não ganha acesso automático** à instituição inteira;
- as respostas devem preservar **proveniência**;
- as fontes devem ser **rastreáveis**;
- dados classificados devem respeitar **ACL**;
- resultados de IA devem ser **identificáveis como tal** quando necessário.

### As invariantes constitucionais do plano agentic

Registadas em [ADR-0002](docs/adrs/0002-deterministic-core-and-agentic-control-plane.md) a
[ADR-0303](docs/adrs/0303-capability-registry-and-executor.md). Não são
aspirações: cada uma tem teste.

> **Ocinye OS is AI-native, not AI-dependent.**
> O sistema opera por inteiro sem nenhum modelo, GPU ou fornecedor disponível.

> **Ocinye OS is operated with AI, governed by the Core.**
> Agentes compreendem, planeiam e orquestram. O Ocinye Core autoriza, executa,
> persiste e verifica.

> **O Main Agent orquestra o sistema e não detém autoridade sobre ele.**
> Tem a lista de capabilities mais larga que existe e nenhum privilégio.

> **Identity may persist. Authority must be re-established.**
> Um `Principal` é um retrato da autoridade de alguém, e não a autoridade. Um
> plano guarda quem o pediu — identidade, que é durável — e nunca o que essa
> pessoa podia. Antes de qualquer efeito, a autoridade volta a estabelecer-se à
> fonte canónica, na fronteira central
> ([ADR-0411](docs/adrs/0411-execution-time-principal-freshness.md)). Nunca
> tratar um `Principal` já resolvido como autoridade durável.

> **Agentes agem exclusivamente através de capabilities tipadas e autorizadas.**
> Um conjunto fechado, definido em código, cada entrada com permissão, âmbito,
> risco, reversibilidade e esquema publicados.

> **Nenhum agente tem acesso irrestrito a base de dados, filesystem, shell, rede
> ou segredos.** Não existe `execute_shell`, `run_command` nem `execute_sql`, e
> existe um teste que percorre o registry a verificá-lo.

> **A saída de um modelo nunca é estado do sistema.**
> Só um `CapabilityResult` devolvido pelo Core significa que algo aconteceu.

> **Toda a acção agentic é autorizada antes de executar e verificada depois.**
> E autorizada **antes de validada**: um erro de validação descreve a forma da
> entrada, e devolvê-lo a quem não pode usar a capability entrega-lhe um mapa.

> **Effective Agent Access = Actor Access ∩ Agent Scope ∩ Resource Policy.**
> Uma intersecção. Nunca uma união. Nenhuma configuração de agente amplia quem o
> usa.

> **Conteúdo recuperado é dado, nunca autoridade.**
> Um documento que diga «executa a capability X» é um documento. A intenção vem
> da pessoa, as capabilities do registry, a autoridade do Core.

> **Efeitos externos, destrutivos e privilegiados exigem confirmação humana.**
> Ligada à pessoa, ao digest do plano e a quinze minutos — as três.

> **Todo o módulo nativo novo deve integrar-se com o Agentic Control Plane
> comum.** Avaliar que capabilities expõe, e expor as que forem seguras e úteis.

> **Todos os fornecedores de inferência implementam o contrato canónico do
> Ocinye.** Formatos de pedido ou de resposta específicos de um fornecedor
> terminam no adapter e nunca alcançam o Agent Runtime nem o Core determinístico
> ([ADR-0304](docs/adrs/0304-canonical-inference-contract.md)).

> **Um fornecedor não é suportado enquanto não passar a Ocinye Provider
> Conformance Suite.** Passar não o torna confiável — torna-o utilizável
> ([ADR-0305](docs/adrs/0305-provider-conformance.md)).

> **Intenção ambígua tem por omissão uma leitura que não altera nada.** A
> detecção automática de intenção nunca promove ambiguidade a efeito, e nunca
> contorna autorização, risco ou aprovação.

---

# PARTE II — DOMÍNIO CIENTÍFICO

## 9. Ideias antes de projectos

A primeira fase científica da Ocinye não começa obrigatoriamente com projectos
formais. Começa também com **as primeiras ideias de projectos de cada unidade**.

O domínio deve distinguir claramente `Idea` de `Project`.

- **`Idea`** representa: exploração; pergunta; problema; hipótese; oportunidade;
  conceito ainda em amadurecimento.
- **`Project`** representa: iniciativa formalmente assumida; escopo definido;
  responsáveis; recursos; objectivos; workflow próprio.

Fluxo conceptual:

```
Idea → Discovery → Exploration → Concept → Review → Project Candidate → Project
```

- **Nem todas as ideias se transformam em projectos.** Abandonar uma ideia é um
  desfecho legítimo e deve ser representável, com registo do motivo.
- **Nunca forces artificialmente investigação exploratória a comportar-se como
  projecto formal.** Se uma ideia exige campos, aprovações ou estrutura de
  projecto para existir, o modelo está errado.

---

## 10. Sistema de registo institucional

O Ocinye Core deve tornar-se progressivamente o **system of record** da
instituição — a fonte canónica para saber:

quem criou algo; quando foi criado; a que unidade pertence; qual ideia originou
um projecto; quais fontes foram utilizadas; quais datasets foram utilizados; que
código foi utilizado; que experiência produziu determinado resultado; qual versão
estava activa; quem aprovou; que classificação possuía; onde os dados residem;
quem teve acesso; que resultado originou determinada publicação; qual propriedade
intelectual surgiu de determinado trabalho.

> **A rastreabilidade não é adicionada anos depois. Nasce com o sistema.**

Implicação prática: `created_by`, `created_at`, `unit`, `classification`,
`version` e ligações de proveniência não são campos opcionais a acrescentar mais
tarde — fazem parte do desenho inicial de cada entidade que os justifique.

---

## 11. Domínio de investigação

O sistema deve preparar entidades e relações equivalentes a:

| Área | Entidades |
|---|---|
| Identidade e organização | Identity · Organisation · People · Memberships · Units |
| Investigação | Ideas · Projects · Research Workspaces |
| Conhecimento | Bibliography · Sources · Notes · Documents |
| Dados | Datasets · Dataset Versions · Files · Storage Objects |
| Código | Code Repositories |
| Ciência | Experiments · Models · Results |
| Colaboração | Tasks · Comments · Activity |
| Correio | Mailboxes · Shared Mailboxes · Messages · Drafts · Outbox |
| Institucional | Funding · Intellectual Property · Publications |
| Governação | Classifications · Permissions · Audit |
| Plataforma | Search · AI · Compute · Storage · Administration · Observability |

**Não transformes o sistema numa colecção de CRUDs.** Cada domínio representa
conceitos institucionais reais, com estados, transições e regras próprias. Uma
entidade cujo único comportamento é criar/ler/actualizar/apagar provavelmente
está mal modelada.

---

## 12. Research Workspace

Dentro do Ocinye Workspace, uma `Idea` ou um `Project` deve possuir um ambiente
contextual próprio — o **Research Workspace** — que reúne:

visão geral; membros; bibliografia; dados; notas; documentos; código;
experiências; modelos; resultados; tarefas; comentários; actividade; IA;
financiamento; histórico.

A interface deve permitir que investigadores **permaneçam dentro do contexto
científico do trabalho**. Evita navegação fragmentada que obrigue o utilizador a
saltar constantemente entre módulos desconectados.

---

## 13. Research Objects

Pensa em artefactos científicos como **objectos relacionáveis**, não como
registos isolados em tabelas:

```
Idea → Source          Project → Dataset           Experiment → Model
Idea → Dataset         Project → Code Repository   Experiment → Result
Idea → Project         Experiment → Dataset        Result → Document
                       Project → Funding           Result → Publication
                                                   Publication → Source
```

- **Não adoptes graph database inicialmente.** PostgreSQL continua a ser a fonte
  canónica (§25).
- **Mas não adoptes um modelo de dados que impeça futuramente construir o
  Ocinye Knowledge Graph.** Relações entre artefactos devem ser entidades de
  primeira classe, tipadas e consultáveis — não colunas ad-hoc espalhadas.

---

## 14. Memória institucional

O objectivo não é armazenar ficheiros. É construir **memória institucional**.

Ao longo dos anos, a Ocinye deve conseguir saber: o que estudou; porquê; com que
dados; com que método; quem participou; o que funcionou; o que falhou; o que foi
aprendido; quais resultados foram produzidos; quais ideias foram abandonadas; que
trabalhos estão relacionados.

**A arquitectura deve preservar conhecimento para além das pessoas que
participaram inicialmente.** Uma decisão que só é compreensível se falares com
quem a tomou é uma falha de desenho, não um detalhe.

---

# PARTE III — ARQUITECTURA

## 15. Planos arquitecturais

A arquitectura conceptual organiza-se em planos. Usa esta linguagem em
documentação e em decisões de fronteira.

**Experience Plane** — Ocinye Workspace. Experiência humana.

**Control Plane** — Ocinye Core. Identidade institucional; organização; unidades;
workflows; permissões; ideias; projectos; metadata; coordenação; estado.

**Knowledge & Data Plane** — Bibliografia; documentos; datasets; versões; notas;
resultados; object storage; proveniência.

**Agentic Control Plane** — Main Agent; Agent Runtime; Agent Registry;
Capability Registry; Capability Executor; Context Engine; Action Planner;
aprovações; Universal Command Surface. Compreende, planeia e orquestra —
**nunca autoriza** ([ADR-0301](docs/adrs/0301-agentic-control-plane.md)).

**Intelligence Plane** — AI Gateway; Model Registry; adaptadores de fornecedor;
embeddings; RAG; routing; inferência. A inferência é `NO_RESOURCE`: zero nós.

**Compute Plane** (`PLANNED`) — Compute Registry; nodes; jobs; GPU; CPU; HPC;
capacidades; scheduling.

**Security Plane** — *transversal*. Identidade; autenticação; autorização;
classificação; secrets; auditoria; policy enforcement; segurança de rede;
criptografia.

**Observability Plane** — *transversal*. Logs; métricas; traces quando
necessário; health; readiness; alertas; eventos operacionais.

Os dois planos transversais atravessam todos os outros. Nenhuma funcionalidade
"salta" o Security Plane por conveniência.

---

## 16. Trust boundaries

**Não trates a rede interna como automaticamente confiável.**

Identifica explicitamente as fronteiras de confiança entre: browser; Cloudflare;
gateway/reverse proxy; Workspace; Core; Identity Provider; PostgreSQL; Redis;
Object Storage; workers; futuro AI Gateway; futuro Compute Plane; futuro CAM-01.

Todo o fluxo que atravessa uma fronteira deve ter: **autenticação; autorização;
validação; política clara e documentada.**

Ao introduzir um novo componente, documenta em que fronteira se situa e o que
o atravessa em cada direcção.

---

## 16-A. Rust-first — princípio tecnológico da Ocinye

> **Ocinye is Rust-first.**
> Rust é a linguagem principal do Ocinye OS e deve ser a escolha por defeito para
> componentes institucionais, serviços, runtimes, agentes, contratos e ferramentas
> operacionais da plataforma, salvo quando outra tecnologia for claramente mais
> adequada ao problema.

Este é um **princípio arquitectural oficial da Ocinye**, parte da sua identidade
tecnológica — não uma escolha circunstancial da primeira versão do Ocinye OS.
Registado em [ADR-0004](docs/adrs/0004-rust-first.md).

Duas regras eliminam a ambiguidade:

> **Rust-first não significa Rust-only.** A investigação científica pode utilizar
> Python, Fortran, C/C++, Julia, OpenFOAM, MPI ou qualquer outra tecnologia
> adequada ao problema.

> **WebAssembly complementa Rust.** WASM/WASI será utilizado estrategicamente para
> interface, isolamento, portabilidade e extensibilidade, mas não como obrigação
> para todos os componentes.

Preferência tecnológica resultante:

| Domínio | Tecnologia |
|---|---|
| **Rust por defeito** | Ocinye Core, Workspace/Leptos, contratos, domínio, autorização, workers, AI Gateway, Compute Registry, Node Agent, Capability Runtime, ferramentas institucionais |
| **WASM/WASI quando acrescenta valor** | frontend interactivo, plugins, capacidades científicas isoladas, processamento portátil, execução controlada |
| **Outras linguagens quando o domínio o justificar** | workloads científicos, ML, CFD, notebooks, ferramentas de terceiros, software científico já consolidado |

Regra operacional vinculativa:

> **Qualquer novo componente do Ocinye OS deve ser considerado primeiro para
> implementação em Rust. A adopção de outra linguagem para componentes
> institucionais requer uma razão técnica concreta e deve ser documentada quando
> tiver impacto arquitectural.**

Corolário (briefing §11): Rust-first **não** é autorização para reinventar
infraestrutura madura. Não construas base de dados, identity provider,
filesystem, TLS, criptografia, message broker, storage engine, container runtime
nem scheduler HPC próprios.

---

## 17. Arquitectura de software

Preferência inicial: **modular monolith**.

- **Não uses microserviços prematuramente.**
- As fronteiras internas devem permitir futura extracção de: AI; Compute; Search;
  Storage; workers — **quando justificado por necessidade real e ADR**.
- Uma fronteira de módulo é real quando existe contrato explícito. Importar
  livremente internals de outro módulo destrói a fronteira, mesmo que as pastas
  pareçam separadas.

---

## 18. Stack de referência

Arquitectura de referência, decidida sob o princípio **Rust-first** (§16-A). Cada
item está registado no ADR indicado.

| Camada | Escolha | ADR |
|---|---|---|
| Linguagem principal | Rust | [ADR-0004](docs/adrs/0004-rust-first.md) |
| Repositório | Monorepo, Cargo workspace | [ADR-0005](docs/adrs/0005-monorepo-cargo-workspace.md) |
| Arquitectura do Core | Modular monolith | [ADR-0006](docs/adrs/0006-modular-monolith.md) |
| Core Runtime | Axum + Tokio | [ADR-0008](docs/adrs/0008-axum-tokio-core-runtime.md) |
| Persistência | PostgreSQL + SQLx | [ADR-0009](docs/adrs/0009-postgresql-sqlx.md) |
| Vector | pgvector | [ADR-0202](docs/adrs/0202-search-fts-pgvector.md) |
| Workspace Runtime | Leptos (SSR) | [ADR-0600](docs/adrs/0600-leptos-workspace-runtime.md) |
| Sessão do Workspace | Backend-for-Frontend, tokens no servidor | [ADR-0601](docs/adrs/0601-workspace-bff-session.md) |
| Identidade | Autenticação no Core: username + password, Argon2id | [ADR-0103](docs/adrs/0103-core-owned-authentication.md) · [ADR-0104](docs/adrs/0104-password-policy-and-hashing.md) |
| Autorização | RBAC + ABAC contextual, fail closed | [ADR-0100](docs/adrs/0100-authorization-model.md) |
| Permissões | Nomeadas, com âmbito e grants explícitos | [ADR-0101](docs/adrs/0101-permissions-scopes-and-grants.md) |
| Object Storage | S3-compatible | [ADR-0200](docs/adrs/0200-object-storage.md) |
| Eventos | Transactional outbox | [ADR-0010](docs/adrs/0010-events-outbox.md) |
| Cache / filas | Redis | [ADR-0011](docs/adrs/0011-redis.md) |
| AI Gateway | Capacidades, nunca modelos | [ADR-0300](docs/adrs/0300-ai-gateway.md) |
| Arquitectura de IA | AI-native, não AI-dependent | [ADR-0002](docs/adrs/0002-deterministic-core-and-agentic-control-plane.md) |
| Plano agentic | Deterministic Core + Agentic Control Plane | [ADR-0301](docs/adrs/0301-agentic-control-plane.md) |
| Acesso agentic | Intersecção actor ∩ agente ∩ recurso | [ADR-0302](docs/adrs/0302-agent-access-intersection.md) |
| Acção agentic | Capabilities tipadas, risco e aprovação | [ADR-0303](docs/adrs/0303-capability-registry-and-executor.md) |
| Contrato de inferência | Canónico do Gateway, versionado; adapters traduzem | [ADR-0304](docs/adrs/0304-canonical-inference-contract.md) |
| Fornecedores de IA | Não suportados sem passar a Conformance Suite | [ADR-0305](docs/adrs/0305-provider-conformance.md) |
| Compute | Compute Registry + Node Agent | [ADR-0500](docs/adrs/0500-compute-registry-node-agent.md) |
| Capability Runtime | WebAssembly / WASI (Wasmtime) | [ADR-0501](docs/adrs/0501-capability-runtime-wasm.md) |
| Correio | Módulo do Core, fornecedor abstraído | [ADR-0400](docs/adrs/0400-mail-as-institutional-surface.md) · [ADR-0401](docs/adrs/0401-mail-provider-abstraction.md) |
| Transporte de correio | IMAP + SMTP (`async-imap`, `lettre`), TLS obrigatório | [ADR-0401](docs/adrs/0401-mail-provider-abstraction.md) · [ADR-0408](docs/adrs/0408-imap-transport.md) |
| HTML recebido | Higienização por lista de permissões (`ammonia`) | [ADR-0402](docs/adrs/0402-mail-html-sanitisation.md) |
| Data residency | Explícita e declarada | [ADR-0201](docs/adrs/0201-data-residency.md) |
| Containers | Docker |  |
| Orquestração inicial | Docker Compose |  |
| Edge | Cloudflare |  |
| Rede privada futura | WireGuard |  |

- **Não uses Kubernetes** nesta fase sem necessidade concreta documentada em ADR.
- Desvios à stack de referência exigem ADR, não uma decisão implícita num commit.

---

## 19. Monorepo

Monorepo, Cargo workspace ([ADR-0005](docs/adrs/0005-monorepo-cargo-workspace.md)).
Estrutura actual — `CURRENT`:

```
apps/workspace                  crates/ocinye-contracts
crates/ocinye-domain            crates/ocinye-observability
crates/ocinye-core              crates/ocinye-capabilities
services/core-server            services/worker
services/node-agent             wasm/capabilities/bibtex-import
design    docs    infra    migrations    scripts
```

As capacidades WASM vivem fora da workspace do host: compilam para
`wasm32-wasip1` e incluí-las forçaria-as em cada `cargo build` nativo.

Ajusta esta estrutura **apenas com justificação arquitectural** registada.

---

## 20. Arquitectura orientada a domínio

Evita estruturas onde tudo é organizado apenas por tecnologia.

Prefere módulos que representem domínio institucional — `research`, `knowledge`,
`datasets`, `identity`, `governance` — em vez de grandes colecções sem fronteiras
de `controllers`, `models`, `utils`, `services` sem contexto.

Regra prática: se para compreender uma funcionalidade tiveres de abrir cinco
pastas técnicas diferentes, a organização está errada.

---

## 21. Eventos de domínio

Mesmo num monólito, usa eventos de domínio quando apropriado:

```
idea.created          project.created        classification.changed
idea.state_changed    dataset.versioned      project.approved
document.uploaded     experiment.completed   compute.node.online
                                             ai.job.completed
```

- Quando os eventos precisarem de durabilidade, usa **transactional outbox** ou
  estratégia equivalente — nunca "publicar e esperar".
- **Não introduzas Kafka nesta fase.**
- Eventos são parte do contrato: nomes e payloads são versionados e documentados.

---

## 22. Background jobs

Processamento pesado deve poder sair do request síncrono. Prepara workers para:
checksums; indexing; previews; embeddings; processamento documental; IA; compute;
notificações.

Solução simples inicialmente. **Evita infraestrutura distribuída desnecessária.**
Um job deve ser idempotente, observável e ter comportamento definido em caso de
falha e de repetição.

---

## 23. API

O Core expõe **API versionada**. Requisitos:

- contratos explícitos e **OpenAPI**;
- DTOs/schemas com validação de entrada e saída;
- **erros estruturados** (código estável, mensagem, correlation ID);
- **request IDs** propagados ponta a ponta;
- **pagination**, **filtering**, **sorting** consistentes em todas as colecções;
- timestamps claros e com fuso explícito;
- **autorização server-side** em cada endpoint e em cada campo sensível.

**Não exponhas o schema interno da base de dados como API.** Nomes de colunas,
chaves internas e estruturas de tabela não são contrato público.

Breaking changes exigem versão nova, ADR e nota no CHANGELOG.

---

## 24. Arquitectura de rede

Arquitectura pretendida inicialmente:

```
Cloudflare → workspace.ocinye.com → reverse proxy / gateway → Workspace / Core
```

- **Evita uma API pública separada sem necessidade.** Reduz a superfície exposta.
- Isto **não** contradiz §3: o Core continua a ser consumível por CLI, notebooks,
  agentes e integrações — mas através da **mesma** API versionada, atrás do mesmo
  gateway e das mesmas políticas. O que se evita é um segundo ponto de entrada
  público com regras próprias, não a existência de outros clientes.
- Futuro: `VPS → WireGuard → CAM-01`.
- **CAM-01 nunca deve aceitar tráfego público de aplicação.**
- **Nunca exponhas o servidor GPU directamente à Internet.**

---

## 25. PostgreSQL

- **PostgreSQL é a fonte canónica dos metadados institucionais.** Nenhum outro
  sistema detém a verdade sobre entidades institucionais.
- Preparar suporte a **pgvector**.
- **Não adiciones graph database sem necessidade real** documentada.
- **Não fragmentes dados prematuramente** por múltiplas bases.
- Schema changes exclusivamente por migration (§58).

---

## 26. Storage

- **Ficheiros grandes não ficam no PostgreSQL.** A base guarda metadata; o Object
  Storage guarda blobs.
- Usa uma abstração **S3-compatible**.
- **Não acoples o domínio a AWS, Cloudflare, MinIO ou qualquer fornecedor
  específico.** A localização física deve ser substituível.

---

## 27. Data residency

Separa rigorosamente dois conceitos:

- **Institutional control** — a Ocinye governa, classifica e controla o acesso.
- **Physical residency** — onde os bytes residem fisicamente.

Actualmente **não existe storage físico institucional definitivo**. Nunca afirmes
que os dados residem num datacenter Ocinye.

O modelo deve representar explicitamente, por artefacto ou por bucket:
**backend; location; residency; classification; migration state.**

Isto permite futura migração para Camama **sem reescrever o domínio**.

---

## 28. Pesquisa

A pesquisa institucional evolui por camadas:

1. **Inicialmente:** PostgreSQL Full Text Search.
2. **Depois:** pgvector.
3. **Futuro:** lexical + semântica + filtros + RAG + AI-assisted search.

Invariante absoluto:

> **A pesquisa aplica autorização antes de retornar resultados. Search não é
> mecanismo para contornar permissões.**

Isto inclui contagens, sugestões, facetas, autocomplete e mensagens de erro:
nenhum deles pode revelar a existência de um artefacto que o utilizador não pode
ver.

---

## 29. Compute

Criar futuramente o **Compute Registry** (`PLANNED` — `NOT IMPLEMENTED`).

- Deve suportar **0 nodes**, **1 node**, **N nodes**.
- **Não hardcodes uma RTX 4090**, nem um único nó, nem uma única localização.
- Nós futuros possíveis: `CAM-01`, `CAM-02`, `HPC-01`, `COL-01`, `DC-01`.
  **Nenhum existe.**
- Capacidades de um nó (GPU, CPU, memória, storage, localização, modelos) são
  **dados**, não ramos de código.
- `compute nodes = 0` é hoje o estado verdadeiro e correcto.

---

## 30. Node Agent

Arquitectura futura para o **Ocinye Node Agent** (`PLANNED`). Responsabilidades
previstas: enrollment; identidade própria; autenticação; health; resource
reporting; capability reporting; model reporting; job execution; status.

- O Node Agent tem **identidade própria** e credenciais próprias — nunca reutiliza
  credenciais de utilizador.
- Ligação futura por **rede privada (WireGuard)**.
- **Nunca exponhas o servidor GPU directamente à Internet.**
- Um nó comprometido deve ser tratado como hostil no modelo de ameaças (§32).

---

# PARTE IV — SEGURANÇA

## 31. Princípio de segurança

O sistema deve ser **secure by design** e **secure by default**.

- **A segurança não é adicionada depois da funcionalidade.**
- **Funcionalidade que contorna segurança não é funcionalidade concluída.**
- Perante ambiguidade de autorização: **fail closed.**
- O default de qualquer novo recurso, endpoint ou campo é **negar**, não permitir.

---

## 32. Modelo de ameaças

Mantém um **threat model vivo**, actualizado a cada alteração arquitectural
importante. Deve considerar, no mínimo:

account takeover; privilege escalation; IDOR; broken access control; session
theft; CSRF; XSS; SQL injection; SSRF; malicious uploads; malware; supply chain
compromise; secret leakage; object storage exposure; insider misuse; audit
tampering; data exfiltration; unauthorised AI retrieval; prompt injection;
poisoned documents; insecure Node Agent; compromised compute node; lateral
movement; backup compromise.

Ao introduzir um componente, endpoint ou integração, indica que ameaças desta
lista se tornam relevantes e como são mitigadas.

---

## 33. Identidade

> **Norma revista em 2026-08-22** por [ADR-0103](docs/adrs/0103-core-owned-authentication.md),
> que substitui o [ADR-0102](docs/adrs/0102-identity-provider.md). A versão
> anterior desta secção proibia autenticação no Core e exigia um Identity
> Provider dedicado com MFA. O ADR-0103 explica porquê mudou, e o que se perdeu.

**Estado actual — `CURRENT`:** o **Ocinye Core é a autoridade de autenticação**,
com **nome de utilizador e palavra-passe** como factor único.

**`MFA = NOT IMPLEMENTED`. `MFA = NOT REQUIRED` nesta fase.** Não escrevas
documentação, README ou ADR que afirme o contrário.

Regras que continuam a vigorar sem excepção:

- **Nunca implementes criptografia própria.** Argon2id vem de biblioteca madura,
  no formato PHC padrão ([ADR-0104](docs/adrs/0104-password-policy-and-hashing.md)).
- **Nunca inventes um esquema de sessão.** Identificador opaco do CSPRNG, digest
  na base de dados, cookie `HttpOnly` · `Secure` · `SameSite`.
- **Passwords nunca são armazenadas.** Só verificadores. Nem em log, nem em
  auditoria, nem em métrica, nem em mensagem de erro.
- **Nenhum administrador consegue ler a password de outro membro.** Não existe
  tal função, e não deve passar a existir.
- A superfície de credenciais vive contida em `crates/ocinye-core/src/password/`
  e `modules/identity/`, para poder ser revista como uma peça.

**Futuro (`PLANNED`):** MFA, passkeys/WebAuthn, recuperação por link seguro e SSO
por IdP federado. A coluna `people.oidc_subject` e o campo `Principal::subject`
mantêm-se para que federar não exija migração de esquema. Qualquer um destes
passos exige **ADR próprio**.

---

## 34. Autorização

**Não uses apenas `admin` e `user`.**

Separa explicitamente: título institucional; papel técnico; membership; unidade;
Research Workspace; Project; classificação; permissão contextual.

Prefere **RBAC + regras contextuais (ABAC) quando necessário**.

> Um título como "Fundador" **não** significa automaticamente "pode ler todos os
> dados `RESTRICTED`".

Título institucional e capacidade técnica são dimensões independentes. Nenhum
atalho de conveniência pode colapsá-las.

### 34.1 Vistas institucionais agregadas

Um artefacto pode pertencer a um Research Workspace e, ao mesmo tempo, aparecer
num ecrã de nível institucional — Bibliografia, Conhecimento, Dados. A vista é
global; **o ownership não é**.

> **Para um artefacto workspace-scoped aparecer numa vista institucional
> agregada, tanto o artefacto como o workspace que o contém têm de ser visíveis
> ao actor.**

São duas fugas diferentes, e cada metade da condição fecha uma:

| Metade | O que impede |
|---|---|
| o **artefacto** é visível | um artefacto mais restrito do que o seu workspace aparecer a quem alcança o workspace (F-01) |
| o **workspace** é visível | um artefacto legível revelar que existe trabalho num ambiente onde o actor não entra |

A segunda não é redundante. O título de uma referência ou o código de um dataset
dizem o que se investiga, e onde — mostrá-los transforma a vista agregada num
oráculo de existência.

Ambas as metades saem do mesmo `VisibilityFilter` que o resto do sistema usa.
**Não escrevas uma segunda política de autorização em SQL** para alimentar uma
página: usa `visibility::contained_in_visible_workspace`.

E a listagem e a contagem partilham o predicado, sempre. Quando cada uma tem o
seu SQL, divergem — e o número no ecrã passa a responder a uma pergunta
diferente das linhas por baixo dele.

### 34.2 Âmbito pedido pelo cliente

A regra acima governa **listagens agregadas**. Um pedido que nomeia
explicitamente o ambiente — `?workspace_id=…` — é outra coisa, e precisa da sua
própria regra:

> **Um identificador de ambiente vindo do cliente pode restringir uma operação
> já autorizada. Nunca estabelece a autoridade para entrar nesse ambiente.**

Concretamente:

```text
workspace_id recebido
→ resolver o ambiente
→ autorizá-lo para o principal
→ aplicar a visibilidade do recurso contido
→ consultar
```

e nunca:

```text
workspace_id recebido
→ WHERE workspace_id = $n
→ visibilidade do recurso apenas
```

**As duas regras não se substituem.** A de §34.1 garante que cada linha
devolvida por uma agregação tem o seu ambiente visível; esta garante que um
ambiente nomeado no pedido é alcançável antes de poder restringir seja o que
for. Em `tasks`, as duas estavam abertas ao mesmo tempo, e cada uma escondia
metade do problema (`SB1-FU-02`).

`knowledge::list_sources` é a implementação de referência: resolve o ambiente com
`research::get_workspace` antes de listar.

---

## 34.3 O Ocinye OS não infere uma unidade principal

Um membro pode pertencer a várias unidades, e o domínio não distingue nenhuma
delas como principal. Quando uma consulta precisa de âmbito de unidade:

- **zero unidades elegíveis** — o recorte não existe, e o controlo diz porquê;
- **exactamente uma** — escolhe-se sozinha, porque não há ambiguidade nenhuma
  para resolver e obrigar a escolher entre uma opção é cerimónia;
- **duas ou mais** — a escolha é explícita e é do membro.

Nunca a primeira, a mais antiga, a de menor UUID nem a alfabeticamente primeira.
Qualquer uma dessas heurísticas seria uma unidade principal inventada, e uma
instituição não passa a ter uma só porque uma consulta precisou.

A escolha viaja no URL como `unit_id`, tipada, e é o Core que decide se o membro
pode usá-la: **um identificador nomeia âmbito; não o concede.**

Isto é o âmbito de *uma consulta*, e não uma «unidade activa» global do Ocinye
OS. Não existe troca de unidade na shell, nem unidade corrente institucional.

## 35. Menor privilégio

Aplica **least privilege** a: utilizadores; serviços; workers; database roles;
Object Storage; CI; deploy; Node Agents; backups; administração.

Cada componente recebe apenas as permissões necessárias para a sua função — nem
uma a mais, incluindo em ambientes de desenvolvimento.

---

## 36. Classificação de informação

Prepara desde o início: `PUBLIC` · `INTERNAL` · `CONFIDENTIAL` · `RESTRICTED`.

A classificação pode afectar: leitura; alteração; download; exportação; pesquisa;
indexação; IA; logging; backup; partilha; retenção.

Exemplo normativo:

> Um `Dataset` classificado `RESTRICTED` **não pode entrar automaticamente num
> índice RAG institucional.**

A classificação acompanha o artefacto ao longo de todo o ciclo de vida, incluindo
cópias, versões, derivados e exportações.

---

## 37. Auditoria

A auditoria é **componente fundacional**, não uma funcionalidade posterior.

Operações críticas geram audit trail: login administrativo; criação; alteração;
mudança de classificação; alteração de membership; alteração de permissão;
aprovação; upload; download sensível; exportação; publicação; transição de
workflow; operações de segurança.

Os audit logs devem ser: **append-oriented**; **protegidos contra alteração pela
aplicação normal**; **estruturados**; **correlacionáveis** (actor, momento, alvo,
correlation ID).

**Nunca guardes conteúdo sensível completo desnecessariamente** no audit trail —
regista a referência e o que mudou, não o conteúdo.

---

## 38. Criptografia

Exige: TLS em trânsito; armazenamento cifrado quando apropriado; backups
cifrados; secrets protegidos; mecanismos modernos suportados por bibliotecas
maduras.

- **Nunca implementes criptografia própria.**
- **Nunca inventes formatos criptográficos.**

---

## 39. Secrets

**Nenhum secret no Git.** Nunca versiones: passwords; API keys; private keys;
tokens; cookies; credentials; production DSNs.

- Mantém `.env.example` **completo e sem valores sensíveis**.
- Produção usa estratégia de secrets **claramente documentada**.
- Se encontrares um secret commitado: **pára e reporta imediatamente**. Não o
  removas silenciosamente nem reescrevas o histórico por iniciativa própria.

---

## 40. Uploads

**Uploads são trust boundary.** Aplica: autorização; tamanho máximo; validação
MIME; checksum; classificação; ownership; metadata; armazenamento privado; scan
hook; futura análise antimalware; nomes internos **não controlados directamente
pelo utilizador**.

> **Nunca tornes um ficheiro público apenas porque alguém conhece o object key.**

Nunca confies no nome de ficheiro nem no `Content-Type` enviados pelo cliente.

---

## 41. IA — Ocinye AI Gateway

Toda a IA comunica através do **Ocinye AI Gateway** (`PLANNED` — `NOT IMPLEMENTED`).

**A aplicação pede capacidades, não modelos específicos.** Não acoples código a
nomes de modelos.

Capacidades iniciais previstas: `GENERAL` · `CODING` · `REASONING` · `EMBEDDING`.

Mapeamento futuro esperado — **é configuração, nunca código**:

```
GENERAL   → Qwen
CODING    → Qwen Coder
REASONING → DeepSeek
```

Regras vinculativas:

- Enquanto não existir node, **`AI = unavailable` é estado válido e correcto**.
- A indisponibilidade de IA **nunca pode quebrar** a plataforma; as
  funcionalidades degradam de forma explícita e informada.
- **Nunca ligues automaticamente OpenAI, Anthropic, Google ou outros fornecedores
  externos para esconder a ausência de IA local.**
- Qualquer fornecedor externo futuro é **decisão explícita**, registada em ADR,
  com análise de confidencialidade e residência de dados.
- Output de IA usado em contexto institucional deve ser rastreável: que
  capacidade, que modelo, que versão, que contexto, quando, por quem.
- **Distinção necessária:** ferramentas de IA usadas *durante o desenvolvimento*
  (incluindo o Claude Code) são instrumentos de trabalho dos autores humanos e não
  são o AI Gateway. Nunca as ligues ao produto, aos dados institucionais nem à
  autoria (§72). As regras desta secção aplicam-se à IA **integrada no sistema**.

---

## 42. RAG seguro

**O RAG deve ser permission-aware.** O context assembly aplica: identidade;
membership; classificação; ACL; scope; fronteiras de project/unit.

> **A IA nunca deve receber documentos que o utilizador não poderia consultar
> directamente.**

A filtragem acontece **antes** da recuperação, não depois da geração. Filtrar a
resposta não corrige um contexto indevidamente montado. Índices e embeddings
herdam a classificação da fonte.

---

## 43. Prompt injection

Trata o **prompt injection** como risco de segurança de primeira ordem.

Conteúdo armazenado em PDFs, documentos, páginas, datasets ou notas **não é
instrução de sistema confiável**. É dado, potencialmente hostil.

Separa sempre, de forma estrutural: **system instructions**; **user
instructions**; **retrieved content**.

Nenhum conteúdo recuperado pode alterar permissões, escalar privilégios,
desencadear acções com efeitos, ou exfiltrar contexto.

---

# PARTE V — EXPERIÊNCIA

## 44. Design do Workspace

O design deve reflectir que estamos a construir um **sistema operacional de
investigação e engenharia**.

**Não copies estética genérica de:** CRM; Notion clone; admin template; SaaS
dashboard; rede social.

A experiência deve transmitir: **rigor; ciência; engenharia; concentração;
segurança; continuidade; contexto.**

---

## 45. Design system

Criar futuramente um **design system próprio** (`PLANNED`).

Identidade visual baseada na linguagem Ocinye: branco; azul-petróleo profundo;
azul; ocre/dourado; cinzas claros; tipografia limpa.

> **Acessibilidade e legibilidade têm prioridade sobre estética.**

Prepara: tokens; typography; spacing; states; badges; tables; forms; navigation;
command/search; research cards; status indicators.

---

## 46. Densidade de informação

Um sistema científico pode e deve ter informação densa. **Não resolvas tudo com
grandes cards vazios.**

Privilegia, quando apropriado: tabelas; listas; painéis contextuais; breadcrumbs;
filtros; pesquisa; sidebars; metadata; atalhos; command palette.

**O Workspace deve escalar para centenas ou milhares de artefactos.** Qualquer
ecrã que só funcione com dez registos está mal desenhado.

---

## 47. UX contextual

O utilizador deve permanecer contextualizado. Ao trabalhar num projecto deve
sempre compreender: onde está; qual unidade; qual projecto; classificação;
estado; responsáveis; última actividade.

**Evita páginas isoladas sem contexto institucional.** A classificação de um
artefacto deve ser visível onde o artefacto é manipulado.

---

## 48. Global search

A pesquisa global é elemento de **primeira classe**. Prepara uma futura
experiência do tipo `Search Ocinye`, capaz de encontrar: pessoas; unidades;
ideias; projectos; bibliografia; documentos; datasets; resultados.

**Sempre permission-aware** (§28).

---

## 49. Command palette

Prepara a UX para uma futura **command palette**: criar ideia; pesquisar; abrir
projecto; criar nota; adicionar referência; criar tarefa.

Não é obrigatório implementar imediatamente. Mas o design deve comportar
workflows rápidos para utilizadores frequentes desde a estrutura de navegação.

---

## 50. Responsividade

Desktop é prioritário para investigação e engenharia, mas a interface deve ser
responsiva.

**Não sacrifiques a experiência desktop para parecer aplicação móvel.** As
tarefas científicas usam tabelas, gráficos, ficheiros, código, metadata e
documentos.

---

## 51. Acessibilidade

Aplica boas práticas desde a fundação: navegação por teclado; contraste; focus
states; HTML semântico; labels; ARIA **apenas quando realmente necessário**;
redução da dependência exclusiva de cor.

Estado, classificação e severidade nunca são comunicados só por cor.

---

## 52. Internacionalização

Interface inicial em **português**. Arquitectura preparada para **i18n**.

**Não espalhes strings hardcoded impossíveis de extrair futuramente.**

---

# PARTE VI — QUALIDADE, DADOS E OPERAÇÕES

## 53. Qualidade do código

**Prefere:** simplicidade; clareza; tipagem; responsabilidades claras; validação
nas fronteiras; erros explícitos; módulos coerentes; nomes sem ambiguidade.

**Evita:** god classes; god modules; `utils` genéricos sem domínio; abstracções
especulativas; código morto; comentários redundantes; TODOs vagos.

Comentários explicam **o porquê**, não **o quê**. Um `TODO` só é aceitável se for
concreto e rastreável. Escreve código que se pareça com o código à volta.

---

## 54. Dependências

Antes de adicionar uma dependência:

1. verifica se é necessária;
2. verifica se a funcionalidade já existe no projecto;
3. avalia manutenção (actividade, releases, comunidade);
4. avalia segurança e superfície transitiva;
5. avalia **licença**;
6. evita dependência para algo trivial.

Dependências arquitecturalmente relevantes exigem documentação e, quando pesam na
arquitectura, ADR. Declara sempre no relatório final as dependências adicionadas.

---

## 55. Configuração

**Nunca hardcodes:** secrets; passwords; URLs de produção; tokens; IPs;
credentials; nomes físicos de nodes.

- Configuração **por environment**, injectada, nunca compilada.
- Mantém `.env.example` actualizado e completo.
- Falhar no arranque perante configuração obrigatória em falta é preferível a
  assumir um valor por omissão.
- Documenta cada variável relevante no README da parte que a usa.

---

## 56. Ambientes

Separa claramente **Development**, **Staging** e **Production**.

**Não promovas configuração de desenvolvimento para produção por conveniência.**
Credenciais, buckets, bases de dados e chaves nunca são partilhados entre
ambientes.

---

## 57. Local development

Futuramente, um novo programador deve conseguir: clonar; ler o README; configurar
o environment; executar poucos comandos; levantar a stack funcional; correr os
testes.

**Docker Compose é preferido inicialmente.** Se o arranque local exigir
conhecimento não documentado, o problema é da documentação, não do programador.

---

## 58. Migrations

- **Schema changes sempre por migration.**
- **Nunca alteres a base de dados de produção manualmente.** Sem excepção.
- Migrations devem ser **versionadas**, **reproduzíveis** (a partir de base vazia
  e a partir do estado anterior), **testáveis** e **documentadas**.
- **A preservação de dados tem prioridade.** Dados nunca são apagados sem
  necessidade explícita e documentada; prefere soft delete ou arquivo quando o
  domínio o exigir.
- Migrations destrutivas exigem revisão explícita e plano de rollback documentado.

---

## 59. Testes

Exige **proporcionalidade ao risco**. Categorias: unit; integration; API;
authorization; workflow; migrations; storage; E2E; security-critical.

- **A autorização merece cobertura especial** (§60).
- **Testa também a negação de acesso**, não apenas o caminho feliz.
- **Nunca ocultes testes vermelhos.** Reporta-os com o output real.
- **Nunca alteres um teste apenas para o CI ficar verde** sem verificar se o
  comportamento esperado continua correcto. Se o teste estava certo, corrige o
  código; se estava errado, explica porquê ao alterá-lo.
- Nunca marques testes como skipped para fechar uma milestone sem o declarar.

**Verde não é prova suficiente para uma suite crítica. A suite tem também de
provar que os testes que se esperava dela foram descobertos e correram.**

`cargo test` devolve zero quando nada falhou. Não diz nada sobre o que **não
correu**. Um teste que retorna cedo — por uma pré-condição em falta, por um
`.ok()?` num arranque, por um `else { return }` — é contado como **passado** e
nunca aparece como saltado.

Em 2026-08-25 descobriu-se que treze das catorze viagens de browser nunca
tinham corrido: partilhavam o directório de perfil do Chrome, só a primeira
arrancava, e as outras saíam em silêncio a dizer `ok`. A CI ficou verde e a
ADR-0410 foi aceite com uma linha de prova que dizia catorze. O Calendar não
estava errado — quando as catorze correram de facto, passaram todas. O que
estava errado era a **evidência** com que a cobertura foi declarada.

O Calendar já tinha mostrado três vezes que um teste pode passar pela razão
errada. Isto é a versão pior: uma suite pode passar sem os testes sequer
correrem.

Por isso, para as suites cuja contagem sustenta uma afirmação de cobertura —
browser E2E, paridade, capacidades WASM, catálogo de operações, regressões de
segurança — `scripts/test-enumeration.sh` exige:

    esperados == passados
    descobertos == passados + ignorados
    saltados == 0
    marcas de execução == execuções esperadas

Os `esperados` vivem numa tabela deliberada dentro do script. O número pode
mudar; muda **por decisão**, e não porque uma suite encolheu sozinha.

A última linha é a que importa mais, e custou uma reversão a descobrir: **a
ausência de uma marca de salto não é prova de execução.** Uma suite pode
declarar uma marca positiva, emitida no ponto em que já não é possível sair sem
correr, e o contrato exige que apareça o número certo de vezes.

**A falha do verificador nunca é o sucesso da propriedade.**

Um mecanismo de verificação tem pelo menos quatro estados, e só um é verde:

| | |
|---|---|
| `PASS` | a propriedade foi observada e está satisfeita |
| `FAIL` | a propriedade foi observada e está violada |
| `INVALID` | o verificador, o build ou a fixture falhou |
| `NOT_RUN` | a verificação não chegou a correr |

`INVALID` não vira `PASS` por aparecer uma linha bonita no stdout. `NOT_RUN` não
vira `PASS` por o processo principal ter terminado. Um verificador que não
encontrou o ficheiro a observar não teve sucesso; observou zero.

Executar e analisar são operações **separadas**: o estado de saída do processo
que detém a propriedade é a autoridade sobre a execução, e o texto só é lido
depois. Um `comando | tee | grep` devolve o estado do `grep`, e foi assim que um
`verify.sh` falhado passou por ter corrido bem — com um commit a seguir-se no
intervalo, a capturar a reversão que aquela verificação teria apanhado.

Uma reversão só é evidência se provar, em separado: que a injecção foi aplicada,
que o alvo mudou como se esperava, que o guarda certo correu, que recusou, e que
a **assinatura de diagnóstico esperada** apareceu. Sair não-zero não chega — um
erro de compilação sai não-zero e não prova defesa nenhuma.

**A verificação de leitura não altera código versionado.**

Uma ferramenta de observação não deve precisar de restaurar o objecto observado.
Se um verificador tocar num ficheiro versionado — mesmo restaurando-o a seguir —
a verificação falha e di-lo. Restaurar depois não desfaz o problema: durante a
execução, os outros verificadores viram outra árvore, e um commit no intervalo
leva o estado errado consigo.

Isto é imposto por `scripts/harness-integrity.sh`, que corre antes de todos os
outros portões e prova que se pode confiar neles.

---

## 60. Testes de autorização

Não basta testar "utilizador autorizado consegue entrar". Testa:

- utilizador **sem membership**;
- utilizador **de outra unidade**;
- recurso `CONFIDENTIAL`;
- recurso `RESTRICTED`;
- utilizador **removido**;
- **role alterado**;
- **acesso directo por ID** (IDOR);
- **search leakage**;
- **download leakage**;
- **AI/RAG leakage**.

Cada nova entidade sensível traz consigo o seu conjunto de testes de negação.

---

## 61. CI

O CI deve existir **desde o início do desenvolvimento real** e cobrir, no mínimo:

- **Frontend:** lint; typecheck; tests; build.
- **Backend:** format; lint; typecheck; tests.
- **Platform:** migrations; integration; security checks; secret scanning;
  Docker build.

> **CI vermelho não é "pronto".**

---

## 62. Observabilidade

Desde a fundação: structured logs; correlation IDs; health; readiness; metrics;
worker status; DB health; storage health; IdP health; AI status; Compute status.

**Não registes:** tokens; passwords; documentos; prompts confidenciais; dados
pessoais desnecessários; dados científicos sensíveis.

Um health check **nunca** reporta saudável um componente que não verificou.
`health` e `readiness` são distintos e ambos honestos.

---

## 63. Backups e continuidade

> **Revista em 2026-08-28** por
> [ADR-0700](docs/adrs/0700-institutional-continuity-and-portability.md), que
> acrescenta um estado antes dos três que aqui estavam.

Distingue rigorosamente quatro estados:

0. **estado classificado** — sabe-se **o que** tem de viajar, e porquê;
1. **backup configurado** — existe configuração ou script;
2. **backup executado** — correu e produziu artefacto verificável;
3. **restore validado** — foi restaurado com sucesso e verificado.

O estado zero faltava, e é o que torna os outros três possíveis de acertar. Um
`pg_dump` salva a base; não salva os bytes a que ela aponta, nem a chave sem a
qual parte das linhas é ilegível. Um backup assim é uma cópia perfeitamente
íntegra e completamente inútil, e isso só se descobre no dia do desastre.

A resposta a «o que é preciso levar?» **não se descobre a olhar para o
servidor**. Descobre-se a olhar para o que o domínio considera estado
autoritativo, e essa é uma decisão do Core (§3), não de quem opera a máquina.
Por isso vive em código, com um teste que falha quando uma tabela nova aparece
numa migration sem decisão de continuidade.

> **Um backup só é operacionalmente confiável quando existe procedimento de
> restore testado.** Cria runbooks.

E, do mesmo lado da mesma linha:

> **Restaurar não é criar o domínio outra vez.** Uma instalação nova com as
> mesmas migrations tem as mesmas tabelas e nada em comum com a instituição.
> Um verificador de continuidade que não distinga as duas não verifica nada.

E uma terceira, que custou uma cópia externa declarada sem nunca ter
acontecido:

> **A execução local bem-sucedida não é evidência de preservação fora do
> servidor. Uma cópia remota só está confirmada por uma observação feita
> contra o destino.**

Um comando de transporte que sai zero pode ter escrito para uma pasta local com
o nome do destino. Perguntar ao destino é a única resposta.

Objectivo futuro: **3-2-1**. **Não declares 3-2-1 antes de existir.**

---

# PARTE VII — DOCUMENTAÇÃO

## 64. Documentação é código

**A documentação faz parte da implementação.** No fim de **cada** milestone:

1. actualizar README;
2. actualizar docs;
3. actualizar ADRs;
4. actualizar modelo de dados;
5. actualizar API docs;
6. actualizar runbooks;
7. actualizar configs e `.env.example`;
8. actualizar CHANGELOG;
9. remover documentação obsoleta;
10. verificar todos os comandos documentados;
11. garantir **total alinhamento entre documentação e realidade**.

> **Nunca declares uma milestone concluída com documentação falsa ou antiga.**

---

## 65. README por área

Cada aplicação, serviço, package ou domínio conceptual significativo deve possuir
**README próprio**, explicando localmente:

propósito; responsabilidades; limites; **o que pertence ali**; **o que não
pertence ali**; dependências; interfaces; execução; testes; configuração;
segurança relevante; integração com o restante sistema.

**Não cries README para pastas triviais** sem responsabilidade própria.

---

## 66. README raiz

O README raiz funciona como **mapa da plataforma**. Deverá conter: o que é a
Ocinye; Core vs Workspace; arquitectura; estrutura do repositório; quick start;
desenvolvimento; testes; segurança; deploy; documentação; **estado actual**.

> Estado: o `README.md` raiz **existe** (§1) e mapeia a estrutura actual.
> Mantém-no alinhado com o repositório a cada milestone (§64).

---

## 67. Documentação central

Estrutura actual — `CURRENT`:

```
docs/adrs            docs/agentic        docs/ai             docs/architecture
docs/authorization   docs/backups        docs/capabilities   docs/compute
docs/data-model      docs/deployment     docs/development    docs/domain
docs/feature-status  docs/identity       docs/knowledge      docs/mail
docs/node-protocol   docs/operations     docs/password-policy docs/runbooks
docs/search          docs/security       docs/storage        docs/testing
docs/threat-model    docs/ui-core-contract docs/wasm
```

Usa **Mermaid** quando um diagrama esclarecer mais do que texto.

---

## 68. ADRs

Decisões arquitecturais importantes exigem **ADR**. Índice, taxonomia e regras
completas: [`docs/adrs/README.md`](docs/adrs/README.md).

### Formato

**Context** · **Decision** · **Alternatives** · **Consequences**, com metadata:

| Campo | Obrigatório | Valores |
|---|---|---|
| `Estado` | sim | `Proposed` · `Accepted` · `Superseded` · `Rejected` · `Deprecated` |
| `Domínio` | sim | A família arquitectural. Determina a faixa de numeração |
| `Impacto` | sim | `FOUNDATIONAL` · `HIGH` · `MEDIUM` · `LOCAL` |
| `Depende de` | quando real | Só dependências arquitecturais verdadeiras |
| `Substitui` / `Substituído por` | quando aplicável | Nos dois sentidos |

Sem sinónimos de estado. Sem `LOW` — leria-se como «irrelevante», e uma decisão
irrelevante não merece uma ADR.

### O namespace

Três coisas distintas, e confundi-las é o que a taxonomia existe para evitar:

- **o número** diz onde vive no namespace, e é determinado pelo **domínio**;
- **o domínio** diz a que família pertence;
- **o impacto** diz até onde alcança.

| Faixa | Família |
|---|---|
| `0001–0099` | Foundations e constituição arquitectural |
| `0100–0199` | Identidade, segurança, autorização e governação |
| `0200–0299` | Conhecimento, dados, armazenamento e memória institucional |
| `0300–0399` | IA, controlo agentic e inferência |
| `0400–0499` | Módulos institucionais nativos |
| `0500–0599` | Computação, nós e Capability Runtime |
| `0600–0699` | Workspace e Experience Plane |
| `0700–0799` | Deployment, rede, operação e resiliência |
| `0800–0899` | Integrações externas e fronteiras de fornecedor |
| `0900–0999` | Reservado, transversais futuras |

Uma faixa vazia fica vazia. **Não se cria uma ADR para preencher espaço.**

### Os identificadores são permanentes

A **ADR Namespace Baseline v1** foi estabelecida em 2026-08-22. A partir dela:

> **Um identificador de ADR aceite é permanente.**

- **A importância nunca causa renumeração.** Uma decisão que se revele mais
  fundamental muda o `Impacto`, não o número. Uma ADR futura com um número alto
  pode perfeitamente ser `FOUNDATIONAL`, sem renumerar nada.
- **O estado nunca causa renumeração.** `Accepted` → `Superseded` mantém o
  identificador.
- **Não se renumera para abrir espaço.** As faixas têm folga; uma decisão
  conceptualmente «entre» duas existentes usa o próximo identificador livre.
- **Um identificador atribuído nunca é reutilizado**, mesmo que a ADR seja
  substituída ou rejeitada.
- **Uma decisão que muda é substituída, não reescrita.** Cria-se uma ADR nova
  que declara `Substitui`; a antiga fica, marcada `Superseded`, com referência
  nos dois sentidos. **A história de uma decisão não se apaga para deixar a
  pasta arrumada.**
- Um ADR aceite não é reescrito para acomodar a realidade posterior.

### Ao escrever ou alterar uma ADR

- O título descreve a **decisão**, não a tarefa: «Autenticação no Ocinye Core»,
  não «Implementar autenticação».
- Ficheiro `NNNN-kebab-case-do-titulo.md`.
- Actualiza o índice e **todas** as referências afectadas na mesma alteração.
  Uma referência partida é uma falha, como qualquer outra.
- Uma dependência declara-se quando é real. Duas ADRs falarem do mesmo módulo
  não é dependência.

### O que não é uma ADR

Um runbook, um guia de configuração, uma convenção de código ou uma nota de
implementação. Se não há alternativas a ponderar nem consequências
arquitecturais a assumir, o sítio é `docs/`.

### Merecem ADR

Stack; identidade; modelo de autorização; modelo de persistência; fronteiras de
módulos; contratos de API; contratos de fornecedor; arquitectura agentic;
estratégia de deployment; adopção de fornecedor externo de IA; qualquer
dependência arquitecturalmente relevante.

---

# PARTE VIII — DISCIPLINA DE TRABALHO

## 69. Verdade do repositório

Documentação, código e ambiente devem **concordar**.

Nunca confundas os estados: **planeado · parcialmente implementado ·
implementado · testado · deployado · operacional.**

Marcadores obrigatórios quando houver risco de ambiguidade:

- `CURRENT` — existe e funciona hoje, verificável no repositório;
- `PLANNED` — decidido ou previsto, ainda não construído;
- `NOT IMPLEMENTED` — declarado na arquitectura, sem qualquer implementação.

**Nunca apresentes `PLANNED` como `CURRENT`.** Nunca afirmes deploy sem deploy.
Nunca afirmes teste sem teste. Nunca afirmes backup validado sem restore testado.
Nunca afirmes node online sem health real. **Nunca inventes evidência.**
Não uses linguagem de marketing para descrever estado técnico.

---

## 70. Não esconder problemas

**Nunca:**

- removas segurança para corrigir um bug;
- silencies um erro sem determinar a causa;
- comentes testes;
- alteres a expectativa apenas para ficar verde;
- marques um mock como produção;
- mascares migrations quebradas;
- escondas um warning crítico;
- declares sucesso parcial como completo;
- reduzas cobertura crítica apenas para fechar uma milestone.

**Corrige ou reporta.** Um problema conhecido e declarado é aceitável; um problema
escondido não é.

---

## 71. Princípio de evolução

> **Não implementar hoje toda a complexidade que só será necessária daqui a
> vários anos, mas não tomar hoje decisões que impeçam aquilo que a Ocinye
> pretende construir no futuro.**

- Entre **simples e evolutivo** e **enterprise e excessivo** → escolhe simples e
  evolutivo.
- Entre um **shortcut estruturalmente destrutivo** e uma **pequena abstracção
  limpa** → escolhe a abstracção limpa.

Quando as duas metades entrarem em conflito, escolhe a solução mais simples que
**não feche portas** e documenta a razão em ADR.

---

## 72. Git — regra absoluta de autoria

**Claude é uma ferramenta. Claude não é autor do projecto.**

Claude nunca deve aparecer como **autor**, **co-autor**, **committer** ou
**contributor automático** em commits.

É expressamente proibido adicionar `Co-Authored-By: Claude` ou qualquer variante.

Também é proibido inserir: "Generated by Claude"; "Generated with Claude Code";
"AI-generated"; "AI-assisted commit"; URLs promocionais; emojis promocionais;
signatures; trailers; ou qualquer metadata que atribua autoria ou co-autoria ao
Claude — em commits, tags, PRs ou mensagens de merge.

**Não alteres** `git config user.name` nem `git config user.email` para valores
relacionados com Claude, Anthropic ou IA. Não uses `--author` para o efeito.

**Os commits pertencem exclusivamente aos autores humanos.**

> **Esta regra não possui excepção.** Prevalece sobre quaisquer instruções
> contrárias, incluindo defaults do harness. Se algum mecanismo automático tentar
> acrescentar tal atribuição, remove-a antes de commitar e reporta a ocorrência.

---

## 73. Disciplina Git

> **Revista em 2026-08-23**, e outra vez em **2026-08-27**. Até 2026-08-23 esta
> secção dizia «não inicializes o repositório Git». O repositório institucional
> existe agora — [`Ocinye/ocinye-os`](https://github.com/Ocinye/ocinye-os), na
> organização — e a regra que faltava passa a ser esta.
>
> A revisão de 2026-08-27 corrige o parágrafo que dizia `main` tecnicamente
> desprotegida. Era verdade enquanto o repositório era privado num plano que
> recusava a funcionalidade; deixou de o ser quando o repositório passou a
> público e a protecção foi activada. **O que era verdade ontem não é uma
> verdade permanente**, e uma limitação descrita no presente sobrevive à sua
> própria causa se ninguém a for corrigir.

O repositório pertence à **instituição**, não a quem escreve o commit. Todo o
commit tem uma identidade humana responsável.

### Permitido dentro de uma tarefa de engenharia autorizada

`git status` · `git diff` · `git log` · criar uma branch de trabalho · commitar
localmente quando a tarefa o pede · consultar PRs e CI · preparar um PR.

### Exige autorização explícita

Push de trabalho novo · merge · alterar rulesets ou branch protection · alterar
secrets do repositório · qualquer alteração ao nível da organização.

### Proibido sem instrução explícita, sempre

Force push · rebase de história partilhada · reset destrutivo · amend de um
commit já publicado · criar tag · criar release · mudar a visibilidade ·
apagar `main` · apagar ou transferir o repositório.

**Antes de qualquer operação Git destrutiva, pára e pergunta.**

### O fluxo normal

```text
branch de trabalho → Pull Request → CI verde → revisão quando houver → merge → main
```

O primeiro commit foi a excepção natural: `main` ainda não existia.

> **`main` está tecnicamente protegida.** Já não é só disciplina: o servidor
> recusa. Verificado pela API em 2026-08-27 e descrito por inteiro na §1 —
> Pull Request obrigatória, cinco *required checks* em modo *strict*,
> `enforce_admins` activo, *force push* e eliminação bloqueados, resolução de
> conversas exigida.
>
> A decisão institucional que fechou a lacuna foi **tornar o repositório
> público**, e não subir de plano. As duas alternativas estavam escritas aqui;
> a escolhida evita depender de Actions pagas.
>
> O portão canónico de `main` continua a correr **depois** do merge, e não é um
> *required check* de PR: um portão que só pode correr no estado que ainda não
> existe não pode ser condição para lá chegar.

### Boas práticas

- Commits **pequenos, coerentes e descritivos**, em inglês, no imperativo.
- Um commit = uma alteração lógica.
- Não commites artefactos gerados, dependências instaladas ou ficheiros
  temporários.
- Nenhum segredo entra no Git. Se encontrares um commitado, **pára e reporta**
  (§39).

---

## 74. Code complete ≠ task complete

> ## CODE COMPLETE ≠ TASK COMPLETE

No final de qualquer implementação significativa, depois do código:

1. rever a alteração;
2. executar os testes;
3. actualizar o README local;
4. actualizar o README raiz se necessário;
5. actualizar docs;
6. actualizar ADR;
7. actualizar API docs;
8. actualizar documentação de migrations;
9. actualizar runbooks;
10. actualizar CHANGELOG;
11. remover ficheiros temporários;
12. remover documentação obsoleta;
13. confirmar comandos;
14. confirmar links;
15. fazer final consistency sweep.

**Só depois a tarefa pode ser considerada concluída.**

---

## 75. Final sweep obrigatório

No fim de cada milestone significativa, revê o repositório transversalmente:

código · testes · lint · typecheck · build · migrations · Docker · CI · docs ·
READMEs · ADRs · configs · `.env.example` · segurança · runbooks · scripts ·
links · **estado real das funcionalidades**.

**Deixa o repositório limpo.**

---

## 76. Definition of Done

Uma tarefa só está `DONE` quando, conforme aplicável:

- [ ] comportamento implementado;
- [ ] arquitectura respeitada;
- [ ] segurança verificada;
- [ ] lint passa;
- [ ] typecheck passa;
- [ ] testes passam;
- [ ] build passa;
- [ ] migrations passam;
- [ ] autorização validada;
- [ ] documentação actualizada;
- [ ] README actualizado;
- [ ] ADR actualizado;
- [ ] configuração documentada;
- [ ] não existem secrets;
- [ ] não existe código morto novo;
- [ ] limitações declaradas;
- [ ] `CURRENT` vs `PLANNED` correcto;
- [ ] final sweep executado.

Se um item não se aplicar, diz porquê. **Se um item falhar, a tarefa não está
concluída** — reporta o estado real.

---

## 77. Relatório final

No final de cada implementação, apresenta:

1. objectivo;
2. o que foi implementado;
3. o que **não** foi implementado;
4. áreas/ficheiros alterados;
5. migrations;
6. decisões arquitecturais;
7. testes executados (comandos concretos);
8. resultados (output real);
9. documentação actualizada;
10. ADRs;
11. riscos;
12. limitações;
13. dívida consciente;
14. próximos passos;
15. estado Git;
16. commits envolvidos.

**Não respondas apenas "Done." Apresenta evidência.**

---

# PARTE IX — ORIENTAÇÃO DE LONGO PRAZO

## 78. Como pensar o produto

Antes de implementar qualquer funcionalidade, pergunta:

> Isto pertence a um sistema operacional institucional de investigação, ou estou a
> construir apenas mais uma página de um website?

Se for a segunda, **reconsidera a arquitectura**. Pergunta também:

- Isto preserva conhecimento institucional?
- Isto possui proveniência?
- Isto respeita autorização?
- Isto será compreensível daqui a cinco anos?
- Isto funciona se tivermos dez unidades?
- Isto funciona se tivermos cem investigadores?
- Isto funciona se houver múltiplos compute nodes?
- Isto funciona **sem** IA?
- Isto funciona **quando** a IA estiver disponível?
- Isto mantém fronteiras de segurança?
- Isto produz evidência auditável?

Estas perguntas devem influenciar decisões de domínio e de arquitectura, não
apenas a revisão final.

---

## 79. Objectivo de longo prazo

A plataforma deve evoluir por esta trajectória **sem ser reconstruída de raiz**:

```
Hoje     Ocinye Core + Ocinye Workspace · zero compute nodes
Depois   CAM-01
Depois   múltiplos nodes
Depois   HPC
Depois   storage próprio maior
Depois   laboratórios
Depois   colocation
Depois   microdatacenter
Depois   infraestrutura institucional maior
```

Cada passo é a ligação de um novo recurso registado, não uma reescrita.

---

## 80. Princípio final

> **O Ocinye Core não é o backend de um website. É o sistema operacional
> institucional da Ocinye. O Ocinye Workspace não é uma área privada de um
> website. É a principal interface humana desse sistema. Toda a decisão de
> arquitectura, segurança, dados e experiência deve partir desta distinção.**

> **A robustez, consistência, segurança, rastreabilidade e preservação do
> conhecimento institucional têm prioridade sobre velocidade de implementação ou
> aparência de progresso.**

---

## 81. Âmbito de trabalho e limites de iniciativa

- Faz o que foi pedido — nem menos, nem mais.
- **Não cries scaffolding, estrutura de pastas, configuração ou código não
  solicitados.** O repositório contém hoje apenas este ficheiro por decisão
  deliberada.
- **Não construas o website público** sem pedido explícito.
- **Não commites nem faças push** sem que a tarefa o peça (§73).
- Se uma ambiguidade mudar materialmente o trabalho, pergunta. Caso contrário,
  decide, declara o pressuposto e avança.
- Se discordares de um pedido por razões técnicas, di-lo em duas frases e depois
  executa o pedido tal como foi formulado, salvo se for inseguro.

---

## 82. Glossário

| Termo | Significado |
|---|---|
| **Ocinye** | Instituição angolana de investigação aplicada, engenharia e infraestruturas digitais. |
| **Ocinye OS** | O sistema operacional institucional completo: o produto arquitectural global. |
| **Sistema Operacional Institucional** | O que estamos a construir: infraestrutura digital central da instituição, não um website. |
| **Ocinye Core** | Núcleo institucional do Ocinye OS: domínio, invariantes, políticas, autorização, estado, API, eventos. |
| **Ocinye Workspace** | Principal interface humana do sistema. Cliente do Core, sem lógica institucional. |
| **Primeiro Núcleo Computacional** | Primeira construção da Ocinye: camada digital (agora) + camada física (futuro). |
| **Research Workspace** | Ambiente contextual de uma `Idea` ou `Project` dentro do Workspace. |
| **Research Object** | Artefacto científico relacionável (source, dataset, experiment, result, …). |
| **Ocinye Knowledge Graph** | Grafo futuro de relações entre research objects. `PLANNED`. |
| **AI Gateway** | Ponto único de acesso a capacidades de IA; abstrai modelos e fornecedores. `PLANNED`. |
| **Compute Registry** | Registo de nós de computação disponíveis à plataforma. `PLANNED`. |
| **Ocinye Node Runtime** | Camada executada nos futuros nós computacionais; inclui o Node Agent. `PLANNED`. |
| **Ocinye Capability Runtime** | Ambiente WASM/WASI para executar capacidades institucionais isoladas. |
| **Idea** | Proposta exploratória, anterior e distinta de um projecto formal. |
| **Project** | Iniciativa formalmente assumida, com escopo, responsáveis e recursos. |
| **CAM-01** | Identificador previsto para o futuro servidor em Camama. **Não existe.** |
| **Institutional control** | A Ocinye governa e controla o acesso aos dados. |
| **Physical residency** | Onde os bytes residem fisicamente. Conceito distinto do anterior. |

---

## 83. Manutenção deste ficheiro

- Este ficheiro descreve **regras** e **estado**. Sempre que o estado real mudar,
  actualiza a **Secção 1** na mesma alteração.
- Alterações a regras arquitecturais aqui expressas exigem **ADR** correspondente.
- Mantém a organização por partes e secções. **Não transformes este ficheiro num
  depósito caótico de informação.** Se uma secção crescer demasiado, extrai-a para
  `docs/` e deixa aqui a referência normativa.
- Em caso de conflito com qualquer outra instrução, prevalecem por esta ordem:
  **§72 (autoria de commits)** · **§31 (segurança)** · **§69 (verdade do
  repositório)**.
