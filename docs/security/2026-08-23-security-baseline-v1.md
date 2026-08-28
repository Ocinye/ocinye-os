# Ocinye OS — Security Baseline v1

**Auditoria adversarial de segurança com remediação em linha.**

| | |
|---|---|
| **Data** | 2026-08-23 |
| **Âmbito** | Todo o repositório, em `HEAD` local. Nenhum ambiente deployado existe. |
| **Método** | Leitura adversarial, reprodução em teste, correcção, regressão, reverificação. |
| **Resultado** | **12 findings** — 1 `HIGH`, 5 `MEDIUM`, 6 `LOW`. **Todos corrigidos.** 1 risco residual de dependência aceite; ver [§7](#7-risco-residual-e-o-que-não-é-risco-residual) para a taxonomia completa. |
| **Gate** | `./scripts/verify.sh` verde, com PostgreSQL real. 611 testes. |

> **O que esta baseline significa, e o que não significa.**
>
> Significa: as principais superfícies e fronteiras de segurança actualmente
> implementadas foram examinadas adversarialmente, os problemas confirmados
> foram corrigidos, as invariantes críticas passaram a ter teste, e as
> limitações restantes estão escritas.
>
> Não significa que o sistema seja impossível de comprometer. Nenhuma auditoria
> estabelece isso, e afirmá-lo seria a primeira falha desta.

---

## 1. Âmbito auditado

**Auditado por leitura, e exercitado por teste:**

`ocinye-domain` (política de autorização, política agentic, filtro de
visibilidade) · `ocinye-core` (identidade, credenciais, sessões, palavras-passe,
autorização, pesquisa, correio, conhecimento, dados, investigação, colaboração,
governação, plano agentic, inteligência, storage, outbox, auditoria) ·
`ocinye-contracts` (paginação, erros, desserialização) · `ocinye-capabilities`
(sandbox WASM) · `core-server` (extractores, middleware, 62 caminhos de API,
bootstrap, `mail-check`) · `worker` · `node-agent` · `apps/workspace` (sessão BFF, rotas,
render) · migrations · `docker-compose.yml` · CI · `.env.example` · árvore de
dependências.

**Fronteiras de confiança percorridas:**

browser → Workspace · Workspace → Core · cliente de API → Core ·
Core → PostgreSQL · Core → object storage · Core → serviço de correio (IMAP/SMTP)
· Core → fornecedor de inferência · agente → Capability Executor · modelo →
Action Planner · conteúdo recuperado → modelo · nó de computação → Core ·
capacidade WASM → host · outbox → worker.

**Fora de âmbito, por não existir:** qualquer ambiente deployado, infraestrutura
física, nó de computação, fornecedor de inferência real, serviço de correio
configurado, backup institucional. Nada foi executado contra sistemas remotos.

---

## 2. Findings confirmados

| ID | Severidade | Componente | Finding | Estado |
|---|---|---|---|---|
| F-01 | **HIGH** | `ocinye-core` — knowledge, data, collaboration | Um artefacto mais restrito do que o seu Research Workspace era legível por identificador | **Corrigido** |
| F-02 | MEDIUM | `core-server` — router | Limite de corpo de 640 MiB em toda a API, incluindo `POST /auth/login` não autenticado | **Corrigido** |
| F-03 | MEDIUM | `ocinye-core` — autenticação | Equalização de temporização presa aos parâmetros Argon2 por omissão | **Corrigido** |
| F-04 | MEDIUM | `core-server`/`workspace` — CSRF | Protecção assente apenas em `SameSite`, que não isola subdomínios irmãos | **Corrigido** |
| F-05 | MEDIUM | `ocinye-core` — bootstrap | Corrida entre dois `bootstrap-admin` concorrentes criava dois administradores | **Corrigido** |
| F-06 | MEDIUM | dependências | Stack TLS/HTTP legada ligada em release, com quatro avisos RustSec abertos | **Corrigido** |
| F-07 | LOW | migrations | `TRUNCATE` esvaziava a trilha de auditoria apesar dos triggers append-only | **Corrigido** |
| F-08 | LOW | `ocinye-capabilities` | Uma invocação WASM interrompia outra a correr ao lado | **Corrigido** |
| F-09 | LOW | `ocinye-core` — inteligência | A pré-visualização de contexto mostrava material que nunca chegaria a um modelo | **Corrigido** |
| F-10 | LOW | `node-agent` | Credencial de máquina escrita antes de ser protegida | **Corrigido** |
| F-11 | LOW | `core-server` — correio | Parâmetro de paginação sem limite transbordava a multiplicação | **Corrigido** |
| F-12 | LOW | `workspace` — correio | Botão «Carregar mesmo assim» que a CSP tornava inerte | **Corrigido** |

Além destes, uma lacuna funcional confirmada e **não corrigida por opção**, em
[§6](#6-lacuna-confirmada-e-não-corrigida).

Nenhum `CRITICAL` foi encontrado.

---

## 3. Remediação, finding a finding

### F-01 · `HIGH` · Um artefacto mais restrito do que o seu workspace era alcançável por identificador

**O problema.** Um artefacto pode ser mais restrito do que o Research Workspace
que o guarda, por desenho: `effective_classification` toma a mais restritiva
entre a pedida e a do workspace, e `reclassify_workspace` desce a classificação
do workspace sem tocar no material que ele já contém. Um dataset `RESTRICTED`
dentro de um workspace `INTERNAL` é um estado normal e previsto.

O lado da **listagem** sempre soube disto: `VisibilityFilter` filtra pela
classificação do próprio artefacto, em SQL. O lado da **leitura directa** não:
`get_dataset`, `get_note`, `get_source`, `get_document` e `get_task` autorizavam
apenas contra o workspace. As duas metades da mesma regra davam respostas
diferentes.

**Reprodução.** Um membro activo da instituição, sem pertença à unidade nem ao
workspace, contra um workspace `INTERNAL` com material `RESTRICTED`:

```
data::list_datasets   → o dataset não aparece          (correcto)
data::get_dataset     → o dataset é devolvido          (a falha)
data::list_versions   → versões e ficheiros devolvidos
```

Alcançável por HTTP em `GET /api/v1/datasets/{id}/versions`, e pelo resolver do
plano agentic para notas, fontes, documentos e tarefas.

**Causa raiz.** `workspace_context(&workspace, kind)` carrega a classificação do
*workspace*. É a resposta certa para «pode esta pessoa trabalhar aqui» e a
errada para «pode ver *isto*». Os caminhos de escrita de datasets já
compensavam com `.with_classification(dataset.classification())`; os de leitura
não, e a inconsistência era invisível porque cada módulo a repetia por sua
conta.

**Correcção.** Uma função, não cinco `if`
([`research::readable_artefact_workspace`](../../crates/ocinye-core/src/modules/research/service.rs)),
que carrega o workspace, constrói o contexto com a **mais restritiva** das duas
classificações e autoriza a leitura. Todos os cinco caminhos passam por ela.
`update_note` e `transition_task` passaram a usar `artefact_context` pela mesma
razão — não eram exploráveis, porque o portão de escrita exige pertença de
qualquer modo, mas registavam a classificação errada na auditoria.

**Regressão.**
`an_artefact_stricter_than_its_workspace_is_not_reachable_by_identifier` e
`a_workspace_member_still_reaches_the_stricter_artefacts`, em
`tests/authorization.rs`, contra PostgreSQL real. O primeiro falha sem a
correcção; o segundo garante que a correcção não fechou o trabalho de quem
pertence.

---

### F-02 · `MEDIUM` · Corpo de 640 MiB aceite em toda a API

**O problema.** `DefaultBodyLimit::max(640 MiB)` estava aplicado ao router
inteiro, para acomodar as três rotas de upload. Todo o extractor `Json` guarda
o corpo por inteiro antes de o handler existir, e os de `POST /auth/login`
correm **antes** de haver sessão para recusar. Um cliente não autenticado podia
obrigar o Core a reter centenas de megabytes por ligação, sem custo para si.

**Correcção.** Um limite pequeno por omissão (1 MiB — muito acima do maior
pedido que a API aceita, que é um passo de plano, limitado a 16 KiB pelo
planner) e o limite grande aplicado **por rota**, nas três que carregam
ficheiro.

**Regressão.** Duas asserções constantes em `routes/mod.rs`: a relação entre os
dois limites deixa de compilar se for desfeita.

---

### F-03 · `MEDIUM` · A equalização de temporização deixava de equalizar quando os parâmetros subiam

**O problema.** `burn_equivalent_work` existe para que «não existe esta conta» e
«palavra-passe errada» custem o mesmo. Verificava contra uma string PHC
constante com `m=19456,t=2,p=1`. O Argon2 lê o custo da string que verifica, não
do hasher — e `docs/security/` manda benchmarkar e subir
`OCINYE_ARGON2_MEMORY_KIB`. Um operador que seguisse a instrução reabria o
oráculo de enumeração em silêncio.

**Medido**, com `m=64 MiB, t=3`: verificação falsa a 240 ms, verificação real a
1,25 s. Um factor de cinco, mensurável através da rede.

**Correcção.** O verificador de equalização passa a ser construído **com os
parâmetros configurados**, uma vez, na construção do `Authenticator`.

**Regressão.** `a_equalizacao_acompanha_os_parametros_configurados` compara os
dois caminhos com parâmetros acima dos que estavam fixados. Falha com a
constante, passa com a correcção.

---

### F-04 · `MEDIUM` · CSRF assente apenas em `SameSite`

**O problema.** A sessão do Workspace é um cookie `SameSite=Lax`; a do Core,
`SameSite=Strict`. Ambos bloqueiam escritas *cross-site*. «Site» é o domínio
registável, não a origem: uma página em `ocinye.com` — que o `CLAUDE.md` §5
reserva para o futuro website público — é *same-site* com
`workspace.ocinye.com`, e o browser envia o cookie com os seus pedidos. Um XSS
em qualquer subdomínio irmão tornar-se-ia uma escrita autenticada aqui. Um
subdomínio não é uma fronteira de confiança (`CLAUDE.md` §16).

**Correcção.** Um guarda de mesma origem em métodos que alteram estado, nos dois
serviços.

- **Workspace:** o `Origin` tem de existir e tem de ser esta origem. Os browsers
  enviam-no em todos os `POST`, por isso exigi-lo não parte nada. Fora de
  produção aceita-se também um `Origin` `http://` cujo host coincida com o
  `Host` do pedido, para que `localhost` e `127.0.0.1` continuem a servir o
  mesmo processo — e **só** fora de produção, porque comparar apenas o host
  aceitaria uma despromoção de esquema.
- **Core:** um `Origin` presente e não reconhecido é recusado; um `Origin`
  ausente passa, porque não veio do caminho cross-origin de um browser — veio de
  uma CLI, de um notebook, de um agente ou do servidor do Workspace, nenhum dos
  quais uma página hostil consegue conduzir (`CLAUDE.md` §3).

**Regressão.** Cinco testes no Workspace e três no Core, incluindo o caso do
subdomínio irmão que o `SameSite` deixa passar.

---

### F-05 · `MEDIUM` · Dois `bootstrap-admin` concorrentes criavam dois administradores

**O problema.** A garantia de execução única era verificada antes da transacção
e outra vez dentro dela. A segunda verificação não valia nada: um `SELECT` em
`READ COMMITTED` não bloqueia ninguém. Duas execuções concorrentes liam ambas
«não há administrador», inseriam pessoas com nomes de utilizador diferentes, e
ambas commitavam. Nada no esquema proíbe um segundo `platform_admin`. O
comentário no código afirmava que só uma podia passar; era falso.

**Reprodução.** Duas invocações em paralelo contra a mesma organização: **dois
administradores**, à primeira ronda.

**Correcção.** `pg_advisory_xact_lock` dentro da transacção, com um namespace
fixo e o identificador da organização. A segunda tentativa espera, lê um
administrador committed, e recusa. Estrutural, na base de dados, como
`CLAUDE.md` §31 exige.

**Regressão.** `two_concurrent_bootstraps_produce_one_administrator` corre o
cenário cinco vezes e verifica também a contagem de papéis na base.

---

### F-06 · `MEDIUM` · Stack TLS/HTTP legada ligada no binário de release

**O problema.** `cargo audit` reportava cinco vulnerabilidades. Quatro vinham de
uma segunda cópia inteira da stack HTTP — `hyper 0.14`, `h2 0.3.27`,
`rustls 0.21.12`, `rustls-webpki 0.101.7` — resolvida **e compilada** ao lado da
moderna:

| Advisory | Crate | O quê |
|---|---|---|
| RUSTSEC-2026-0258 | `h2 0.3.27` | DATA frames vazios sem limite |
| RUSTSEC-2026-0104 | `rustls-webpki 0.101.7` | Pânico alcançável ao interpretar CRLs |
| RUSTSEC-2026-0098 | `rustls-webpki 0.101.7` | Name constraints aceites indevidamente para URIs |
| RUSTSEC-2026-0099 | `rustls-webpki 0.101.7` | Name constraints aceites para wildcards |

A origem: `aws-sdk-s3` traz no seu conjunto `default` a feature `rustls`, que é
a stack TLS **legada** do SDK, ao lado de `default-https-client`, que é a
moderna. Ambas ficavam activas.

Encontrou-se também `aws-config` declarado e **nunca usado** por nenhuma linha
de código — o Core constrói a configuração do S3 à mão, com credenciais
explícitas.

**Correcção.** `aws-sdk-s3` com `default-features = false` e todas as features
do conjunto por omissão **excepto `rustls`**. `aws-config` removido. Código que
não está ligado não precisa de ter a sua alcançabilidade discutida.

**Verificado.** De 5 vulnerabilidades para 1. A árvore deixou de resolver
`hyper 0.14`, `h2 0.3`, `rustls 0.21` e `rustls-webpki 0.101` em qualquer alvo.

**Regressão.** `cargo audit` passou a correr também no `./scripts/verify.sh`,
não só na CI, com as excepções em `.cargo/audit.toml`, cada uma com a razão
escrita.

---

### F-07 · `LOW` · `TRUNCATE` esvaziava a trilha de auditoria

**O problema.** A migration 0001 instalou triggers `BEFORE UPDATE` e
`BEFORE DELETE` `FOR EACH ROW` sobre `audit_events`, e a documentação passou a
chamar-lhe append-only. `TRUNCATE` não percorre linhas: nenhum trigger de linha
corria. **Verificado empiricamente** contra PostgreSQL: `TRUNCATE audit_events`
executava sem objecção. Quem pudesse escrever na base podia apagar a prova de o
ter feito — a manipulação de auditoria que o modelo de ameaças enumera.

**Correcção.** Migration `0012_audit_truncate_guard.sql`, com um trigger
`BEFORE TRUNCATE ... FOR EACH STATEMENT`. Como os outros dois, é uma barreira
contra a aplicação e não contra um superutilizador da base.

**Regressão.** `the_audit_trail_cannot_be_rewritten_by_the_application` tenta
`UPDATE`, `DELETE` e `TRUNCATE`, cada um numa transacção revertida.

---

### F-08 · `LOW` · Uma invocação WASM interrompia outra

**O problema.** O relógio de época pertence ao `Engine`, não ao `Store`. O host
armava um fio por invocação que incrementava a época **uma vez** ao seu próprio
prazo, o que interrompia todas as invocações a correr nesse instante. Cada fio
dormia também o prazo inteiro mesmo quando a capacidade devolvia logo.

**Medido:** uma invocação com limite de 2 s morria aos 218 ms porque uma de
200 ms tinha acabado ao lado — e a razão devolvida dizia «excedeu o limite de
tempo», que era falso.

**Correcção.** Um ticker por `Engine`, e cada `Store` a exprimir o seu limite em
número de ticks. Sem fio por invocação, e cada limite é o seu.

**Regressão.** `one_invocation_does_not_cut_another_short` e
`a_capability_that_never_yields_is_stopped_at_its_wall_time`.

> Este componente não é hoje alcançado por nenhum pedido: `ocinye-capabilities`
> não é dependência de nenhum serviço. O defeito era latente, e é corrigido
> agora porque o custo de o corrigir depois de ligado é outro.

---

### F-09 · `LOW` · A pré-visualização de contexto mostrava mais do que iria a um modelo

**O problema.** `GET /ai/context-preview` existe para tornar a fronteira de
recuperação inspeccionável. Aplicava a política de leitura do actor e o tecto
declarado do modelo, e **não** `may_process_with_ai` — o tecto institucional de
processamento por IA, que o Context Engine agentic sempre aplicou. Ler não é
processar (`CLAUDE.md` §36, §42). Com zero nós Ocinye o tecto é `INTERNAL`, e a
pré-visualização mostrava `CONFIDENTIAL` e `RESTRICTED` que nunca seriam
enviados.

Não é uma fuga: o material era do próprio actor. É uma pré-visualização que
descrevia outro sistema.

**Correcção.** O tecto efectivo passa a ser o mais restritivo entre o do modelo
e o da instituição, no mesmo sítio onde o Context Engine já o fazia.

**Regressão.** `the_context_preview_never_shows_more_than_inference_would_receive`,
com a contraprova de que a pesquisa continua a devolver o material ao seu dono.

---

### F-10 · `LOW` · Credencial do Node Agent exposta entre a escrita e o `chmod`

**O problema.** `store_agent_token` escrevia o ficheiro e só depois lhe apertava
as permissões para `0600`. Entre as duas chamadas a credencial ficava em disco
sob a umask do processo — tipicamente `0644`. Um nó de computação é uma máquina
que o modelo de ameaças já trata como potencialmente hostil, e o ficheiro é
pequeno e o caminho é conhecido.

**Correcção.** `OpenOptions::mode(0o600)`, que aplica as permissões na criação.
O `set_permissions` fica, para o caso de reenrolar por cima de um ficheiro
preexistente demasiado aberto.

**Regressão.** `the_agent_credential_is_never_readable_by_anyone_else`, que
verifica o modo depois de criar e depois de reescrever sobre um ficheiro `0644`.

---

### F-11 · `LOW` · Paginação de correio sem limite

**O problema.** `GET /mail/mailboxes/{id}/messages?page=…` tomava um `i64` do
cliente e calculava `(page - 1) * 50`. Com `page=9223372036854775807`: pânico em
depuração, `OFFSET` negativo em release. Todo o resto da API pagina por
`PageRequest`, que é `u32` e limitado; o correio era a excepção.

**Correcção.** `clamp(1, MAX_PAGE)` e `saturating_mul`.

---

### F-12 · `LOW` · Um botão de correio que a CSP tornava inerte

**O problema.** O aviso de conteúdo remoto oferecia «Carregar mesmo assim». O
link ia a `?remote=1`, o Core devolvia o corpo com os `src` originais, e a CSP
do Workspace (`img-src 'self' data:`) recusava cada um. A página recarregava, o
aviso desaparecia — porque já nada estava por carregar — e as imagens
continuavam ausentes. O membro ficava com a impressão de que o pedido tinha sido
atendido.

**Correcção.** O botão sai; o estado fica, dito por inteiro. Alargar a CSP a
origens de terceiros seria desmontar a última barreira contra o rastreio por
email para repor um botão; servir o conteúdo através do Ocinye é funcionalidade
por construir, não correcção.

---

## 4. Hardening sem finding associado

| Mudança | Razão |
|---|---|
| PostgreSQL, Redis e MinIO passam a publicar em `127.0.0.1` no compose | Publicavam em `0.0.0.0` com as credenciais que estão no `.env.example`. Numa rede de que não se é dono, isso é uma base de dados com password conhecida à escuta. |
| `cargo audit` no `./scripts/verify.sh` | A CI já corria `rustsec/audit-check`; um programador nunca via o resultado. O sweep local passou a mostrá-lo, quando a ferramenta existe. |
| `.cargo/audit.toml` | Cada excepção com a razão escrita, e sai da lista quando deixar de ser verdade. |

---

## 5. Controlos verificados

Examinados adversarialmente e **considerados correctos nos cenários testados**.
Não é uma prova de ausência de defeito.

**Autenticação e sessões.** Argon2id em formato PHC com rehash transparente, sem
criptografia própria. Palavra-passe nunca armazenada, nunca em log, nunca em
auditoria, nunca em mensagem de erro; o tipo `Secret` não implementa `Display`
nem `Serialize` e redige o `Debug`. Credencial temporária de CSPRNG, expira, uso
único, força a mudança de palavra-passe ao primeiro acesso — imposto no
**extractor**, antes de qualquer handler. Mesma mensagem para as quatro formas
de falhar. Sessões: 256 bits de entropia do CSPRNG, digest SHA-256 na base,
revogação em massa na mudança de palavra-passe, rotação verificada por teste.
Não existe função para ler a palavra-passe de outra pessoa.

**Autorização.** Política pura, testada exaustivamente sobre a combinação de
classificação, papel e pertença. Nenhum ramo `_ => allow`. Papéis
administrativos não abrem `RESTRICTED`, e existe teste que o percorre. Cada
permissão do catálogo tem papel que a conceda ou uma razão escrita para não ter.
Filtro de visibilidade renderizado em SQL, com equivalência provada contra a
política; contagens usam o mesmo predicado que as listagens. Acesso
cross-organisation recusado. IDOR: a leitura negada é indistinguível da
ausência.

**Plano agentic.** O executor autoriza **antes** de validar, para que um erro de
validação não descreva a forma de uma capability a quem não a pode usar.
Recursos resolvem-se antes da decisão, contra o serviço que os detém, e a
decisão é tomada no contexto do recurso. Uma capability inventada resolve para
nada. O risco vem do registry, nunca da proposta — a proposta não tem sequer
campo de risco. A aprovação está ligada à pessoa, ao digest e a quinze minutos,
as três. Um plano alterado invalida a confirmação. Nenhuma capability alcança
shell, SQL, ficheiros, rede ou segredos, e existe teste que percorre o registry.
Fornecedor hostil: risco despromovido, aprovação forjada, resultado fabricado,
identidade de modelo hostil e resposta sobredimensionada — todos recusados na
fronteira.

**Injecção de prompt.** Instruções em conteúdo recuperado não se tornam plano:
o planner só reconhece capabilities do registry. `system`, `data` e `instruction`
são campos distintos do contrato de inferência, e nenhum adapter recebe uma
string opaca em que já tenham sido misturados.

**Correio.** A fronteira de privacidade está nas cláusulas `WHERE`, contra o
próprio actor, e nenhum papel administrativo aparece nesse ficheiro. Envio:
`Permission::MailSend`, mais `may_send()` na caixa, mais a identidade do
remetente conferida contra a caixa resolvida, mais a política de classificação —
e uma confirmação nunca transforma uma recusa em envio. HTML recebido
higienizado por lista de permissões com o parser HTML5, sem `script`, `iframe`,
`svg`, `form`, `style` nem atributos genéricos; `javascript:` e `data:` fora dos
esquemas permitidos. Conteúdo remoto bloqueado por omissão. TLS obrigatório em
IMAP e SMTP, com verificação de certificado e de hostname, e sem via para
desligar; STARTTLS em IMAP falha fechado em vez de cair para texto em claro.
Credenciais do fornecedor nunca em UI, log, erro, auditoria ou `mail-check`.

**Pesquisa.** Permission-aware em SQL. Títulos, excertos, contagens e totais
usam o mesmo predicado. Um membro do workspace vê; um estranho não vê e o total
não o denuncia.

**Storage.** Chaves de objecto geradas pelo sistema e opacas; o nome de ficheiro
do utilizador é apenas metadata, normalizado contra travessia e caracteres de
controlo. Tipos de conteúdo por lista de permissões. Download só depois de
autorização, por URL assinado de cinco minutos, com
`Content-Disposition: attachment` — o que neutraliza SVG activo servido a partir
da origem do storage.

**HTTP.** CSP do Workspace sem `unsafe-inline` e sem `unsafe-eval`, com
`form-action 'self'`, `base-uri 'none'` e `frame-ancestors 'none'`. CSP do Core
`default-src 'none'`. `nosniff`, `no-referrer`/`same-origin`, `no-store` em
respostas por membro. CORS fechado por omissão; nenhuma reflexão de `Origin`.
Nenhum redirect aberto: todos os destinos são fixos ou construídos a partir de
`Uuid` tipados.

**Desserialização.** Todos os `parse(...).unwrap_or(...)` caem para o valor
**mais restrito**: `Revoked`, `PasswordChangeRequired`, `Restricted`, `Shared`,
`Failed`. Um campo obrigatório a `null` conta como ausente, e não como presente.

**Erros.** Falhas de base de dados e internas nunca levam detalhe para fora. A
razão de uma recusa de autorização fica na auditoria, não na resposta.

**Isolamento do fornecedor de teste.** `FixtureProvider` atrás de
`#[cfg(feature = "test-fixtures")]`, activado apenas pelo harness. O sweep
verifica estruturalmente que o binário do servidor resolve `ocinye-core` sem
essa feature.

**Segredos.** Varredura alargada do repositório: nenhuma chave privada, token de
fornecedor ou credencial. `.env.example` sem valores sensíveis. Nenhum log
contém palavra-passe, token, cookie, cabeçalho de autorização, corpo de correio
ou contexto recuperado.

**`unsafe`.** Nenhum. `#![forbid(unsafe_code)]` nos serviços e nos crates.

---

## 6. Lacuna confirmada e não corrigida

**Planos agentic não são persistidos.**
[`agentic::repository::create_plan`](../../crates/ocinye-core/src/modules/agentic/repository.rs)
não é chamado por lado nenhum. `runtime::invoke` constrói um `ActionPlan` e
devolve-o sem o gravar, pelo que `GET /agentic/plans` devolve sempre vazio e
`POST /agentic/plans/{id}/approve|reject|execute` devolvem sempre «Plano não
encontrado».

**Não é uma vulnerabilidade:** falha fechada — nenhum plano pode ser executado.
Hoje nem é observável, porque a inferência é `NO_RESOURCE` e nenhum plano chega
a ser proposto.

**É uma lacuna funcional**, e tem duas consequências que importam declarar:

1. O portão de aprovação existe e é testado ao nível do executor, mas **nunca
   foi exercitado através da superfície HTTP**.
2. A Secção 1 do `CLAUDE.md` descreve «aprovações ligadas a plano e a pessoa»
   como implementadas. É verdade do mecanismo e não do caminho.

Ligar a persistência é uma alteração funcional, não uma correcção de segurança,
e fica fora do que uma auditoria deve decidir sozinha (`CLAUDE.md` §81). É o
próximo passo recomendado antes de qualquer trabalho que dependa do plano
agentic.

> **Follow-up — 2026-08-23.** Esta lacuna foi fechada pela milestone
> **Agentic Plan Lifecycle**: a persistência está ligada ao Agent Runtime, o
> ciclo `list · get · approve · reject · execute` funciona por HTTP, a execução
> reclama o plano atomicamente e reautoriza cada passo, e quinze testes contra
> PostgreSQL cobrem-no — incluindo o que demonstra que revogar um acesso depois
> de confirmar impede a execução.
>
> **O texto acima não foi reescrito.** Descreve o estado observado durante a
> Security Baseline v1, que era verdadeiro quando a auditoria terminou. Um
> registo de auditoria que se corrige a si próprio à medida que a realidade
> muda deixa de ser um registo (`CLAUDE.md` §69).

---

## 7. Risco residual, e o que não é risco residual

Quatro coisas diferentes acabavam na mesma tabela. Separá-las importa: chamar
«risco aceite» a um advisory que não entra no artefacto é inflacionar, e chamar
«aviso» a algo que está compilado é o contrário.

### 7.1 Risco residual aceite — 1

Está no artefacto de release, não tem correcção disponível, e fica.

| Item | Análise |
|---|---|
| `RUSTSEC-2026-0253` — `lru 0.16.4`, unsound em `LruCache::pop()` | **Compilado** (`target/release/.fingerprint/lru-*` existe), via `aws-sdk-s3`, que o usa como cache de identidade. Exige um pânico dentro do `pop` para se manifestar. Nenhuma versão da árvore resolve para uma correcção. Sai desta lista quando `aws-sdk-s3` subir a dependência. |

### 7.2 Advisory sem aplicação ao artefacto de release — 1

Está no `Cargo.lock`, e **não é compilado**. Não é risco aceite: não é risco.

| Item | Evidência |
|---|---|
| `RUSTSEC-2023-0071` — Marvin Attack em `rsa 0.9.10` | Entra apenas por `sqlx-mysql`. O Ocinye usa `sqlx` com `default-features = false` e só o driver `postgres` (ADR-0009). O Cargo resolve as dependências opcionais dos três drivers para o lockfile, e nenhuma delas é construída: **não existe artefacto de `rsa` nem de `sqlx-mysql` em `target/release`**. Sem correcção a montante. Silenciado em `.cargo/audit.toml`, com esta razão escrita lá. |

### 7.3 Watchlist de manutenção — 3

Crates sem manutenção activa, alcançados por dependências que o Ocinye escolheu
deliberadamente. Nenhum tem defeito conhecido; nenhum é alcançado por entrada
não confiável através de código do Ocinye. São sinal para vigiar.

| Item | Via |
|---|---|
| `RUSTSEC-2025-0057` — `fxhash` | `wasmtime` → `fxprof-processed-profile` |
| `RUSTSEC-2024-0436` — `paste` | `leptos` → `either_of` |
| `RUSTSEC-2026-0173` — `proc-macro-error2` | `leptos` → `leptos_macro` |

### 7.4 Limitação de segurança conhecida — 1

Não é um finding desta auditoria, e não é um risco de dependência. É uma decisão
institucional registada, com a sua consequência assumida.

| Item | Consequência |
|---|---|
| **`MFA = NOT IMPLEMENTED`** | Assumido pelo [ADR-0103](../adrs/0103-core-owned-authentication.md). Uma palavra-passe comprometida continua a ser acesso comprometido. Nada nas mitigações desta auditoria muda isso — apenas tornam a obtenção mais cara. |

---

## 8. Limitações desta auditoria

- **Nada foi testado em produção, porque não existe produção.** Nenhum ambiente
  está deployado. As conclusões são sobre o repositório, não sobre uma
  instalação a correr.
- **O correio não está configurado.** O adaptador IMAP/SMTP foi auditado por
  leitura e pelos seus testes; não foi exercitado contra um servidor real.
- **Nenhum fornecedor de inferência real existe.** O caminho agentic foi
  exercitado ponta a ponta contra o fornecedor determinístico, incluindo os
  cenários hostis. Um adapter real trará superfície que não existe hoje.
- **Nenhum nó de computação existe.** O protocolo de nó foi auditado por
  leitura.
- **Nenhum backup foi executado nem restaurado.** Nada mudou aqui.
- **Não foi feita análise criptográfica de temporização para além dos caminhos
  óbvios** — autenticação e comparação de segredos.

---

## 9. Evidência de verificação

```
$ OCINYE_TEST_DATABASE_URL=postgres://…/ocinye_test ./scripts/verify.sh

== Formatação ==                 cargo fmt --all -- --check          ok
== Clippy ==                     --workspace --all-targets -D warnings  ok
== Capacidades WASM ==           componentes construídos             ok
== Testes ==                     605 passed; 0 failed; 1 ignored     ok
== Testes das capacidades ==     6 passed; 0 failed                  ok
== Builds de release ==          --release --workspace               ok
== Isolamento do fornecedor ==   o servidor não activa test-fixtures ok
== Biblioteca de ADRs ==         estrutura e referências             ok
== Segredos ==                   nenhum segredo encontrado           ok
== Dependências ==               cargo audit                         ok
== Docker Compose ==             compose válido                      ok

Sweep concluído.                                          EXIT=0
```

**611 testes**, 0 falhados, com PostgreSQL real. Eram **588** antes desta
auditoria.

Nenhuma suite se salta por falta de infraestrutura: as 96 que precisam de
PostgreSQL correram. O único `ignored` da linha acima é
`despeja_os_ecras_para_inspeccao`, que escreve ficheiros para uma passagem
visual e não verifica nada — marcado assim desde antes desta auditoria.

Cada correcção de F-01, F-03, F-05, F-07, F-08 e F-09 foi verificada por
**reversão**: o teste de regressão foi corrido contra o código anterior e falhou.

Além da suite, verificações adversariais contra o **binário de release** a
correr, com PostgreSQL real:

| O que | Resultado |
|---|---|
| `POST /auth/login` com `Origin: https://ocinye.com` | `403` — «Este pedido não veio de uma origem reconhecida» |
| `POST /auth/login` com `Origin: null` | `403` |
| `POST /auth/login` sem `Origin` (uma CLI) | passa o guarda, `401` na autenticação |
| `GET` com `Origin` hostil | passa — não altera estado |
| `POST /auth/login` com 2 MiB de corpo, não autenticado | `413` |
| `bootstrap-admin` duas vezes em simultâneo | **um** administrador; a segunda recusa |
| `GET /workspaces/{ws}` (INTERNAL, sem pertença) | `200` — o controlo |
| `GET /datasets` (mesmo actor) | 0 itens — a listagem esconde |
| `GET /datasets/{id}/versions` do dataset `RESTRICTED` | `404` — o acesso directo concorda com a listagem |

---

## 10. Avaliação final de arquitectura

| Pergunta | Resposta |
|---|---|
| A arquitectura continua consistente com **Deterministic Core + Agentic Control Plane**? | Sim. Nada nesta auditoria moveu uma decisão para fora do Core. |
| Alguma autoridade migrou para agentes, modelo, fornecedor ou UI? | Não. |
| Existe caminho de execução que não passe pelo Capability Executor? | Não. |
| Uma aprovação consegue substituir uma autorização? | Não. O executor autoriza cada passo outra vez, imediatamente antes do efeito. |
| Alguma capability alcança shell, SQL, ficheiros, rede ou segredos? | Não, e existe teste que percorre o registry. |
| Existe fuga cross-user ou cross-scope conhecida? | Não, nos cenários testados. F-01 era uma e está fechada. |
| Fica algum `CRITICAL` ou `HIGH` conhecido por corrigir? | Não. |
| `./scripts/verify.sh` está verde? | Sim. |

> **No known Critical or High issue remains under the scenarios and attack
> surfaces tested.**

Esta baseline fecha aqui. O próximo reforço deve nascer de um finding concreto
ou de uma superfície nova introduzida por um milestone — não de outro ciclo
genérico de hardening.

---

## 11. Follow-ups posteriores à baseline

Esta secção regista findings **descobertos depois** de esta auditoria fechar.

Os doze findings acima — F-01 a F-12 — são o registo do que foi encontrado no
âmbito e no momento desta auditoria. **Nada aqui os renumera nem altera esse
total.** Reescrever o cabeçalho para «13 findings» diria que a auditoria original
encontrou algo que não encontrou, e um registo histórico que se ajusta ao
presente deixa de servir para o que existe.

---

### SB1-FU-01 · `MEDIUM` · A listagem institucional de datasets revelava artefactos de um workspace inacessível

| | |
|---|---|
| **Descoberto em** | 2026-08-23 |
| **Descoberto durante** | `Workspace Navigation & Screen Integrity v1` — não nesta auditoria |
| **Classe** | Information disclosure · desalinhamento de âmbito de autorização |
| **Superfície** | `GET /api/v1/datasets` sem `workspace_id`, e o ecrã `Dados` |
| **Estado** | **Corrigido** |

**Como apareceu.** A auditoria da barra lateral obrigou a perguntar, ecrã a
ecrã, de onde vinham os dados. Três ecrãs — Conhecimento, Bibliografia e Dados —
apresentam-se ao nível da instituição sobre recursos que pertencem a Research
Workspaces. Ao construir a leitura agregada da Bibliografia percebeu-se que a
condição correcta tem duas metades; ao aplicá-la aos datasets, descobriu-se que
essa listagem **já existia** e só cumpria uma.

**Cenário mínimo.** Um membro da mesma instituição, com direito a ler
`INTERNAL`. Um dataset `INTERNAL` dentro de um Research Workspace a que esse
membro **não** pertence. A listagem institucional incluía-o.

**Causa raiz.** A consulta aplicava o `VisibilityFilter` ao artefacto e não
exigia que o workspace que o contém fosse visível. `INTERNAL` é legível, e
ninguém perguntava *onde* o artefacto estava.

**Impacto, medido contra o payload real.** A linha devolvida traz `code`,
`title`, `description`, `keywords`, `licence`, `usage_restrictions`,
`classification`, `state` e o `workspace_id` do ambiente inacessível. Não traz
ficheiros, URLs nem conteúdo, e **não existe rota de detalhe nem de download de
dataset** — a única outra leitura é `/workspaces/{id}/datasets`, que exige
alcançar o ambiente. Não havia, portanto, caminho de escalada da listagem para o
conteúdo.

É por isso `MEDIUM` e não `HIGH`: expõe existência e metadados descritivos de
investigação alheia, criando um oráculo cross-workspace, mas não dá leitura do
material protegido. A distinção face ao F-01 é essa — ali o acesso directo ao
artefacto e às suas versões estava aberto.

**O que se sabe, e o que não se sabe.** O defeito era alcançável no código.
Nesta instalação não existem dados reais que permitam demonstrar impacto
operacional passado — o que **não** é o mesmo que afirmar que nada foi exposto.
Sem telemetria histórica dessa consulta, essa afirmação não é demonstrável, e
não é feita.

**Correcção.** `visibility::contained_in_visible_workspace`, que reutiliza o
`VisibilityFilter` existente e exige as duas condições. Não foi escrita uma
segunda política de autorização em SQL.

**Regressão.**
`um_dataset_de_um_workspace_alheio_nao_aparece_na_listagem_institucional`, contra
PostgreSQL real. Foi escrito **antes** da correcção e falhou contra o
comportamento então em vigor, com `a listagem institucional revelou um dataset de
um workspace que o membro não alcança`.

**Reversão.** Neutralizar a condição de workspace no auxiliar partilhado
reproduz a fuga — e derruba ao mesmo tempo os testes equivalentes da Bibliografia
e dos Documentos, o que prova que os três domínios passam pela mesma política e
não por três cópias dela.

**Generalização.** Bibliografia, Documentos e Datasets usam agora a mesma
invariante central, formalizada em `CLAUDE.md` §34.1:

> Para um artefacto workspace-scoped exposto por uma vista institucional
> agregada, **tanto o artefacto como o workspace que o contém** têm de ser
> visíveis ao actor. Nenhuma das condições dispensa a outra.

As duas metades protegem coisas diferentes, e há um teste para cada:

| Metade | O que impede |
|---|---|
| visibilidade do **artefacto** | a classe do F-01 — artefacto mais restrito do que o seu ambiente |
| visibilidade do **workspace** | o oráculo de existência de ambientes alheios |

Há ainda um terceiro teste, do sentido inverso: quem **é** membro do workspace
continua a alcançar o artefacto restrito, porque a filiação concede-o por desenho
(ADR-0100). Sem ele, uma correcção exagerada passaria despercebida.

---

### SB1-FU-02 · `MEDIUM` · Listagens de recursos contidos ignoravam a visibilidade do ambiente

| | |
|---|---|
| **Descoberto em** | 2026-08-23 |
| **Descoberto durante** | `Workspace Navigation & Screen Integrity v1`, ao reforçar uma propriedade de coerência entre superfícies |
| **Classe** | Information disclosure · autorização de contentor |
| **Superfícies** | `GET /api/v1/tasks` (com e sem `workspace_id`) · `GET /api/v1/datasets?workspace_id` |
| **Estado** | **Corrigido** |

**Como apareceu.** Um teste que exige que o mesmo recurso tenha o mesmo
veredicto em qualquer superfície falhou — e não pelo lado esperado. A listagem
institucional escondia um dataset que a listagem com âmbito mostrava.

A primeira versão desse teste **não apanhava isto**: o cenário escolhido usava um
artefacto `RESTRICTED`, já escondido pela sua própria classificação, e as
superfícies concordavam por outra razão. Só ao acrescentar um artefacto
**legível** dentro de um ambiente **inalcançável** — onde a condição do ambiente
é a decisiva — é que a divergência apareceu.

**As duas manifestações.** São propriedades diferentes, e `tasks` tinha as duas
abertas:

```text
Datasets
  listagem institucional : visibilidade do artefacto + do ambiente   ✓ (SB1-FU-01)
  listagem com âmbito    : workspace_id do pedido, sem autorização   ✗

Tasks
  listagem institucional : visibilidade do artefacto apenas          ✗
  listagem com âmbito    : workspace_id do pedido, sem autorização   ✗
```

**Agravante.** Para as tarefas, **não era preciso conhecer o identificador do
ambiente alvo**. A listagem sem âmbito revelava ela própria o `workspace_id` do
ambiente inacessível, que podia depois ser fornecido à listagem com âmbito. A
exploração não exigia enumeração de UUIDs.

**Impacto, medido contra o payload real.** Os metadados de tarefa expostos
incluíam título e descrição em texto livre, estado, prioridade, prazo,
identificador do responsável e identificador do ambiente. O identificador do
responsável **é resolúvel** a uma pessoa: `get_person` autoriza ao nível da
organização, pelo que qualquer membro activo o converte num nome.

É por isso um `MEDIUM` **de impacto elevado**: revela quem trabalha em quê, com
que prioridade e para quando, dentro de ambientes a que a pessoa não tem acesso.
Não dá, ainda assim, leitura do conteúdo científico protegido — corpos de
documentos, notas ou ficheiros de dataset —, não atravessa organizações e não
permite alteração de estado. É a mesma fronteira que separa esta família do
`F-01`, que era acesso directo ao artefacto.

**O que se sabe, e o que não se sabe.** O defeito era alcançável no código. Não
existe evidência histórica que permita afirmar exposição operacional passada, e
essa afirmação não é feita.

**Correcção.** Duas, porque são duas propriedades:

- `contained_in_visible_workspace` na listagem institucional de tarefas;
- `research::get_workspace` a resolver e autorizar o âmbito pedido, em tarefas e
  em datasets — a forma que `knowledge::list_sources` já usava.

**Regressões.** `as_tarefas_respeitam_o_ambiente_nas_duas_superficies` e
`os_datasets_respeitam_o_ambiente_nas_duas_superficies`, contra PostgreSQL real.

**Reversão.** Cada metade foi neutralizada isoladamente, e cada uma falha com a
sua própria mensagem — «a listagem revelou uma tarefa de um ambiente
inalcançável» e «um identificador escrito à mão conferiu autoridade sobre um
ambiente alheio». Nenhuma das duas correcções cobre a outra.

---

#### Auditadas e não afectadas

A varredura foi dirigida a superfícies onde um identificador de âmbito vindo do
pedido pode restringir uma consulta. Estas foram testadas e **não** reproduzem a
falha:

| Superfície | Controlo positivo | Sonda adversarial |
|---|---|---|
| `GET /search?workspace_id` | 1 | 0 |
| `GET /activity?workspace_id` | 1 | 0 |
| `GET /workspaces?unit_id` | — | os workspaces devolvidos carregam a sua própria visibilidade |
| `/workspaces/{id}/sources` · `/notes` · `/documents` | — | âmbito resolvido e autorizado por `get_workspace` |

> **Nota metodológica.** A primeira sonda inseriu recursos por SQL directo e
> devolveu `0` na pesquisa e na actividade. Esse zero era ininterpretável: as
> projecções que essas superfícies leem — o índice de pesquisa e o feed — são
> escritas pelas operações de domínio, e um `INSERT` não as alimenta.
>
> A segunda sonda criou pela operação real e provou primeiro que cada superfície
> **encontra** o recurso para quem tem acesso. Só então o `0` adversarial passou
> a significar alguma coisa.
>
> **Um resultado negativo de segurança só tem significado quando um controlo
> positivo prova que a fixture e o caminho de observação estão a funcionar.**
> Sem isso, duas superfícies teriam sido declaradas seguras sem nunca terem sido
> testadas.
