# Modelo de ameaças do Ocinye OS

Documento vivo. Actualiza-o sempre que a arquitectura mudar
(`CLAUDE.md` §32).

**Última revisão: 2026-08-22**, correspondente à fundação descrita no
[README](../../README.md).

## O que está em causa

O Ocinye OS pretende ser o *system of record* da instituição. O que a comprometer
não é uma indisponibilidade: é a perda de integridade da memória institucional,
ou a exposição de investigação e de dados sob restrições contratuais.

## Activos

| Activo | Porque importa |
|---|---|
| Bibliografia, notas, documentos, datasets | O trabalho científico em si. |
| Proveniência e linhagem ideia → projecto | Sem ela, os resultados não são reprodutíveis nem auditáveis. |
| Registo de auditoria | A evidência de quem fez o quê. |
| Identidades e memberships | Determinam todo o acesso. |
| Credenciais de máquina | Um nó comprometido é um ponto de apoio. |
| Classificação | Se puder ser alterada sem registo, tudo o resto cede. |

> **Revisto em 2026-08-23** pela
> [Ocinye OS Security Baseline v1](../security/2026-08-23-security-baseline-v1.md).
> As linhas marcadas «Corrigido pela Baseline v1» descrevem uma mitigação que
> não existia, ou que existia e não cobria o que dizia cobrir.

## Ameaças e mitigações

### Identidade e acesso

> **Actualizado em 2026-08-22** pelo [ADR-0103](../adrs/0103-core-owned-authentication.md).
> A autenticação passou a ser feita no Core, com palavra-passe como factor único.
> As ameaças abaixo mudaram em consequência.

| Ameaça | Mitigação | Estado |
|---|---|---|
| **Account takeover** | Mínimo de 15 caracteres; blocklist; Argon2id; throttling por conta e por origem; sessões de 12 h. **Sem MFA** — ver nota abaixo. | Implementado. Redução assumida face ao ADR-0102. |
| **Credential stuffing** | Blocklist de palavras-passe conhecidas de fugas; throttling por conta impede testar listas em larga escala. | Implementado, testado. |
| **Password spraying** | O throttling conta por **conta** e não só por origem: uma tentativa em cada conta a partir de mil origens continua a bater no limite por conta. | Implementado, testado. |
| **Força bruta** | Recusa por janela, não bloqueio permanente: bloquear ao fim de N falhas entrega uma negação de serviço a quem souber um nome de utilizador. | Implementado, testado. |
| **Palavra-passe fraca** | Política aplicada no Core, não no browser; blocklist com canonicalização; padrões repetitivos e percursos de teclado recusados por código. | Implementado, testado. |
| **Ataque offline a um dump da base de dados** | Argon2id, m=19 MiB, t=2, salt único por hash. **Sem pepper** ([ADR-0104](../adrs/0104-password-policy-and-hashing.md)). | Implementado. Pepper `PLANNED`. |
| **Intercepção da credencial temporária** | Validade de 24 h; consumida ao primeiro uso; apresentada uma única vez; runbooks proíbem email, SMS e chat. | Implementado; a entrega é procedimento humano. |
| **Reutilização de credencial temporária** | Consumida ao definir a palavra-passe; não pode tornar-se a palavra-passe definitiva; constraint garante no máximo uma activa. | Implementado, testado contra DB real. |
| **Account enumeration** | Mensagem única para as quatro causas de falha; verificação contra um verificador dummy quando não há conta, para igualar o tempo de resposta. O dummy é construído **com os parâmetros Argon2 configurados**: uma constante deixava de equalizar assim que um operador subisse o custo, que é o que a documentação lhe manda fazer. | Implementado, testado por medição. Corrigido pela Baseline v1 (F-03). |
| **Session fixation** | Sessão revogada e **substituída** em cada início de sessão e mudança de palavra-passe. Não existe promoção de sessão. | Implementado, testado. |
| **Session theft** | Cookie `HttpOnly`/`Secure`/`SameSite=Strict`; identificador de 256 bits; só o digest é persistido; token nunca chega ao browser no Workspace. | Implementado. |
| **Acesso com credencial de administrador** | Uma credencial criada por um administrador só abre sessão restrita; nenhuma API normal responde nela. | Implementado, testado contra DB real. |
| **Autorização stale** | Papéis, memberships e grants lidos da base de dados **a cada pedido**. Suspensão, desactivação e reset revogam sessões de imediato. | Implementado, testado. |
| **Privilege escalation** | Papéis técnicos separados da posição institucional; `RESTRICTED` ignora papéis administrativos; ninguém pode conceder um grant que não detém; criar um `platform_admin` exige sê-lo. | Implementado, testado exaustivamente. |
| **Abuso administrativo** | Nenhum administrador consegue ler a palavra-passe de outro membro — não existe tal função. Reset revoga sessões e fica na auditoria com autor. Um administrador não pode desactivar-se a si próprio. | Implementado, testado. |
| **Abuso de reset** | Reset exige `members.manage`, é auditado com autor e razão, e os runbooks exigem confirmação de identidade por canal distinto. | Implementado; a confirmação é procedimento humano. |
| **Broken access control** | Política pura, fail closed, com testes que enumeram todas as combinações de papel, permissão e classificação. | Implementado. |
| **IDOR** | Autorização por recurso independente do identificador; leitura negada devolve `not_found`. A decisão usa a classificação **do artefacto**, que pode estar acima da do seu Research Workspace — usar a do workspace tornava legível por identificador aquilo que a listagem já escondia. | Implementado, testado contra DB real. Corrigido pela Baseline v1 (F-01). |
| **Oráculo de existência de um Research Workspace** | Uma vista institucional agregada — Bibliografia, Documentos, Dados — só mostra um artefacto se **o artefacto e o workspace que o contém** forem ambos visíveis ao actor. Só a primeira condição deixava passar um artefacto legível dentro de um ambiente inacessível, e o título de uma referência ou o código de um dataset dizem o que se investiga, e onde. As duas metades saem do mesmo `VisibilityFilter`, num auxiliar partilhado, para que uma vista nova não possa esquecer metade. | Implementado, testado contra DB real. Corrigido em 2026-08-23 (SB1-FU-01). |
| **Âmbito pedido pelo cliente como autoridade** | Um `workspace_id` vindo do pedido resolve e autoriza o ambiente antes de poder restringir a consulta — `get_workspace` primeiro, filtro depois. Sem isso, escrever o identificador de um Research Workspace alheio devolvia as tarefas e datasets legíveis lá dentro, com título, descrição, prazo e o responsável. A listagem sem âmbito revelava ela própria esse identificador, pelo que a exploração não exigia adivinhar UUIDs. | Implementado, testado contra DB real. Corrigido em 2026-08-23 (SB1-FU-02). |
| **CSRF** | `SameSite=Strict` no cookie do Core e `SameSite=Lax` no do Workspace, **mais** um guarda de mesma origem em métodos que alteram estado nos dois serviços. `SameSite` compara o domínio registável e não a origem: sozinho, deixa passar uma escrita vinda de qualquer `*.ocinye.com`, e o `CLAUDE.md` §5 reserva `ocinye.com` para um website público. Terminar sessão exige `POST`. | Implementado, testado. Corrigido pela Baseline v1 (F-04). |
| **Repetição de um efeito já cometido** | A execução de um plano reclama-o com um `UPDATE` condicional; um segundo pedido, sequencial ou concorrente, é recusado com o estado real em vez de repetir o efeito. A garantia vive em PostgreSQL, não num lock em memória que uma segunda instância do Core não partilharia. **Não se promete *exactly-once* contra sistemas externos**: `Core → SMTP` não é uma transacção ACID. | Implementado, testado com execuções concorrentes. |
| **Consentimento a congelar autorização obsoleta** | Uma confirmação é consentimento, e a autorização é decidida outra vez imediatamente antes do efeito. Revogar um acesso depois de confirmar impede a execução; a confirmação fica registada e continua a não ser autoridade. | Implementado, testado contra DB real. |
| **Trabalho atribuído a quem não o alcança** | Uma tarefa só é atribuível a quem poderia lê-la — `evaluate` com `Action::Read` contra o contexto da própria tarefa. A chave estrangeira prova que o identificador nomeia *uma* pessoa, e nada mais: sem esta regra, uma tarefa numa organização podia nomear alguém de outra, e a diferença entre «existe» e «não existe» era um oráculo. | Implementado, testado contra DB real. Corrigido em 2026-08-23. |
| **Capability inalcançável por endereçar o recurso pelo `input`** | Uma capability de âmbito de unidade ou workspace nomeia o recurso por `resources`. Recebê-lo pelo `input` faz o executor autorizá-la contra a organização, onde uma permissão que vem de pertença não existe — falha fechada, e em silêncio. Um teste percorre o registry e mede a propriedade. | Implementado, testado. Corrigido em 2026-08-23. |
| **Plano de outra pessoa alcançado por identificador** | Propriedade na cláusula `WHERE` de cada operação do ciclo. Ler, aprovar, rejeitar e executar respondem todos como ausência. | Implementado, testado. |
| **Adulteração da auditoria** | `audit_events` tem triggers que recusam `UPDATE`, `DELETE` **e `TRUNCATE`**. Os dois primeiros são `FOR EACH ROW` e não viam o `TRUNCATE`, que não percorre linhas: a tabela esvaziava-se sem objecção. | Implementado, testado contra DB real. Corrigido pela Baseline v1 (F-07). |

> **A ameaça que não está mitigada: ausência de segundo factor.** Quem obtiver
> uma palavra-passe válida obtém a sessão. Nada nesta tabela o impede — apenas
> tornam a obtenção mais cara. É a consequência assumida do ADR-0103, e a razão
> pela qual MFA é o primeiro item de `PLANNED` em segurança.

### Aplicação

| Ameaça | Mitigação | Estado |
|---|---|---|
| **XSS** | SSR com um único script próprio; CSP sem `unsafe-inline` e sem `unsafe-eval`, com `form-action 'self'`, `base-uri 'none'` e `frame-ancestors 'none'`; Leptos escapa; o único `inner_html` recebe HTML já higienizado por lista de permissões no Core. | Implementado, testado. |
| **SQL injection** | Todos os valores são parâmetros vinculados. O único fragmento interpolado é o filtro de visibilidade, e interpola apenas `Uuid`, cujo `Display` só produz hexadecimal e hífenes — coberto por teste. | Implementado. |
| **SSRF** | Nenhum URL vindo do cliente é seguido; redirecções desactivadas no cliente HTTP do Core e do agente. | Implementado. |
| **Log injection** | Identificadores vindos do cliente só são adoptados se parecerem identificadores emitidos por nós; caso contrário são substituídos. | Implementado, testado. |
| **Denial of service por parâmetro** | Tamanho de página limitado em vez de rejeitado, em **todas** as colecções; limite de corpo pequeno por omissão (1 MiB) e grande apenas nas três rotas de upload, porque `POST /auth/login` corre antes de existir sessão para recusar; palavra-passe recusada acima de 4096 bytes **antes** de qualquer hashing, para que um chamador não autenticado não escolha quanto trabalho o servidor faz. | Implementado. Corrigido pela Baseline v1 (F-02, F-11). Rate limiting existe no início de sessão; nos restantes endpoints é `PLANNED`. |

### Dados e conteúdo

| Ameaça | Mitigação | Estado |
|---|---|---|
| **Search leakage** | Predicado de autorização na query; `COUNT` sobre o mesmo predicado. | Implementado, testado. |
| **Object storage exposure** | Bucket privado; chave opaca gerada pelo sistema; download autorizado e auditado; URL assinada de 5 min. | Implementado. |
| **Malicious upload** | Allow-list de tipos; `Content-Type` do cliente não é confiável; nome normalizado contra traversal; checksum. | Implementado, testado. |
| **Malware** | Hook previsto; `scanned_at` `NULL` significa "não analisado". | **Não implementado**, declarado. |
| **Data exfiltration** | Exportar `RESTRICTED` é mais estreito do que ler; downloads auditados. | Implementado. |
| **Audit tampering** | Trigger na base de dados rejeita `UPDATE` e `DELETE`. | Implementado, verificado. |
| **Insider misuse** | Auditoria com actor, momento, alvo, classificação e correlação; mudanças de classificação exigem motivo. | Implementado. |

### Proveniência e linhagem científica

A linhagem é uma superfície nova: percorre relações entre recursos, e a **forma
do grafo é ela própria informação**. Saber que um resultado depende de mais três
coisas já diz que há três coisas, e a que unidade pertencem costuma deduzir-se do
resto.

| Ameaça | Mitigação | Estado |
|---|---|---|
| **Travessia por trás de uma fronteira** | Cada nó é resolvido pelo serviço que o detém, com a política de quem percorre; um nó recusado termina a travessia e as suas arestas não são seguidas. | Implementado, testado. |
| **Canal lateral pela topologia** | Um nó recusado não devolve nada: nem identificador, nem tipo, nem título, nem ambiente, nem contagem. Uma fronteira escondida é indistinguível de uma folha visível. | Implementado, testado por comparação de respostas. |
| **`truncated` como sinal de existência** | `truncated` significa apenas que a consulta atingiu o limite de profundidade **entre os recursos visíveis**; um nó recusado nunca o activa. | Implementado, testado. |
| **Aresta para um recurso inalcançável** | Ambas as pontas são resolvidas com a política de quem escreve, antes de a relação existir. Um identificador nomeia âmbito; não o concede. | Implementado, testado. |
| **Relação semanticamente impossível** | Matriz de compatibilidade sobre a tripla tipo + verbo + tipo, fail closed. | Implementado, testado. |
| **Travessia sem fim** | Conjunto de visitados e tecto de profundidade: um ciclo científico legítimo não prende a consulta. | Implementado, testado. |
| **`workspace_id NULL` lido como acesso global** | `NULL` significa que a relação não está confinada a um ambiente; a visibilidade continua a decidir-se pelas duas pontas. | Implementado. |

### Afirmação institucional indevida

Um actor indevidamente capaz de validar um resultado não está a alterar
metadados. Está a produzir uma **afirmação institucional**: o registo passa a
dizer que a Ocinye tem aquele resultado por confirmado, e diz em nome de alguém.

| Ameaça | Mitigação | Estado |
|---|---|---|
| **Validar sem autoridade** | `results.validate` é uma permissão própria, que não decorre de poder escrever no ambiente; a operação exige-a. | Implementado, testado. |
| **Delegação a um agente** | `science::record_validation` é `non_delegable` atrás da `INSTITUTIONAL_CLAIM_BOUNDARY`; nenhuma capability a executa, e uma aprovação não a abre. | Implementado, testado percorrendo o registry. |
| **Rótulo sem evidência** | Uma reprodução exige a execução que a sustenta. | Implementado, testado. |
| **Proveniência inventada por inferência** | `origin = operation` só é escrito por operações que observaram a relação; o caminho agentic escreve `declared`. | Implementado. |

### Inteligência

| Ameaça | Mitigação | Estado |
|---|---|---|
| **RAG leakage** | Recuperação passa pela política do próprio requerente, depois pelo tecto do modelo. | Implementado; inspeccionável via `/ai/context-preview`. |
| **Prompt injection** | Conteúdo recuperado é dados, nunca instrução; quatro camadas estruturalmente distintas. | Arquitectura definida; sem modelo para exercitar. |
| **Poisoned knowledge** | Proveniência preservada; `raw_metadata` guarda o registo importado tal como chegou. | Implementado. |
| **Fuga para fornecedor externo** | Nenhum fornecedor externo é contactado. Exige flag, registo explícito e ADR próprio. | Implementado. |

### Correio institucional

O correio é a superfície mais exposta do Ocinye OS: é a única entrada que
**qualquer pessoa no mundo** pode usar, sem conta, com HTML e anexos
arbitrários, e cujo conteúdo será renderizado a um membro autenticado.

| Ameaça | Mitigação | Estado |
|---|---|---|
| **XSS pelo corpo da mensagem** | Higienização por lista de permissões (`ammonia`) antes de qualquer renderização; `<script>`, `<iframe>`, `<form>`, `on*` e esquemas fora de `http`/`https`/`mailto`/`tel`/`cid` removidos. Um único `inner_html` em todo o Workspace, documentado. | Implementado, 12 testes ([ADR-0402](../adrs/0402-mail-html-sanitisation.md)). |
| **Rastreio por conteúdo remoto** | Bloqueado. O `src` é reescrito e contado; o Core sabe servir o corpo com ele a pedido explícito por mensagem, e regista-o no audit trail. O Workspace **não o carrega**: a sua CSP declara `img-src 'self' data:`, e a interface diz o estado em vez de oferecer um botão que a política tornaria inerte. Preferência corrompida lê-se como bloquear. | Implementado. |
| **Prompt injection por email recebido** | Conjunto **fechado** de dez acções; blocos de dados delimitados; e — a garantia que não depende do modelo — a assistência **não tem nenhuma acção com efeito ao seu alcance**: devolve texto. | Implementado, testado ([ADR-0405](../adrs/0405-mail-prompt-injection.md)). |
| **Envio automático por IA** | Separação estrutural: `assist` e `send` são rotas distintas, e `assist` não chama `send`. Não é verificação — é a ausência de uma chamada. | Implementado ([ADR-0406](../adrs/0406-ai-generated-is-not-sent.md)). |
| **Leitura de correspondência alheia por privilégio** | A pertença está na cláusula `WHERE` de cada consulta; nenhuma consulta de correio consulta um papel. Caixa alheia lê-se como **inexistente**. | Implementado ([ADR-0404](../adrs/0404-mail-privacy-boundary.md)). |
| **Exfiltração de material classificado por email** | `RESTRICTED` não sai para destinatário externo, e confirmar não desfaz a recusa. A classificação mais alta governa a mensagem inteira. | Implementado, 11 testes ([ADR-0403](../adrs/0403-mail-send-policy.md)). |
| **Domínio semelhante (`ocinye.com.atacante.net`)** | Correspondência **exacta** de domínio; lista vazia torna tudo externo (falha fechada). | Implementado, testado. |
| **Path traversal por nome de anexo** | `safe_filename` retira tudo o que se pareça com caminho; constraint `ck_mail_attachment_filename_is_safe` reforça na base. | Implementado, testado. |
| **Fuga de credenciais de correio** | `mail_provider_settings` **não tem colunas de credenciais**; a password vive só no ambiente; `Debug` redigido em `MailConfig` e `ImapSmtpConfig`; `ProviderHealth` não as contém estruturalmente. | Implementado, testado. |
| **Correio como arquivo secundário** | `mail_messages` guarda metadados e excerto; corpos e anexos não são persistidos. | Implementado ([ADR-0407](../adrs/0407-mail-index-not-archive.md)). |
| **Malware em anexo recebido** | — | **Não implementado.** A descarga de anexos está declarada indisponível na interface, o que hoje remove a via. Reabrir quando os anexos forem ligados. |

### Plano agentic

A afirmação que esta camada faz não é «o modelo resiste a instruções
maliciosas» — isso não é testável. É **um modelo completamente subvertido não
consegue causar nada**, e é essa que os testes verificam.

| Ameaça | Mitigação | Estado |
|---|---|---|
| **Prompt injection** | Intenção vem da pessoa, capabilities do registry, autoridade do Core. Conteúdo recuperado é dado. | Implementado |
| **Injecção indirecta** (email, documento, metadata de dataset) | Idem. Nenhum conteúdo redefine permissões, política ou instruções de sistema. | Implementado |
| **Tool injection** — capability inventada | O registry é um conjunto fechado; um nome bem formado que não existe não resolve. | Implementado, testado com 5 nomes |
| **Capability escalation** | `may_invoke` verifica o actor primeiro. Teste exaustivo sobre as 64 permissões com o Main Agent e um principal sem papéis. | Implementado, testado |
| **Acesso a shell / SQL / rede / segredos** | Não existe capability que o faça. Teste percorre o registry por 13 marcadores de infraestrutura. | Implementado, testado |
| **Definição de agente maliciosa** | Configuração é entrada não confiável: cada campo é tecto, nenhum é concessão. | Implementado, testado |
| **Autoridade não delegável** | Permissões, papéis, membros, plataforma, IA, computação e correio nunca vão para um agente. **O registry não arranca** se violar isto. | Implementado, `assert!` no arranque |
| **Approval bypass** | Confirmação ligada a pessoa + digest + 15 minutos. As três. | Implementado, testado |
| **Approval reuse após alteração** | O digest cobre o efeito; mudar o destinatário invalida a confirmação. | Implementado, testado |
| **Modelo a rotular-se seguro** | A proposta não tem campo de risco. Vem do descriptor. | Implementado, testado |
| **Runaway plan** | Máximo de 8 passos; uma proposta de 200 é recusada. | Implementado, testado |
| **`ResourceRef` alucinado** | Cada referência é resolvida pelo serviço de domínio que a detém, antes de qualquer decisão. Um identificador inventado resolve para nada. | Implementado, testado contra DB real |
| **`ResourceRef` para outra unidade** | A autorização corre contra o contexto do **recurso** — unidade, ambiente e classificação reais —, não contra o do pedido ([ADR-0306](../adrs/0306-resource-resolution-as-authorization-boundary.md)). | Implementado, testado contra DB real |
| **`ResourceRef` de tipo errado** | O identificador de uma Nota apresentado como Projecto não resolve: o tipo faz parte da procura. | Implementado, testado |
| **`ResourceRef` para outra organização** | O serviço de domínio limita por organização; a referência não resolve. | Implementado, testado |
| **Enumeração por identificador** | «Não existe», «não é seu» e «tipo não endereçável» devolvem a mesma mensagem. | Implementado, testado |
| **`label` enganadora num plano** | O título vem do Core na resolução; o que o modelo escreveu é descartado. Uma confirmação nunca descreve o recurso com palavras não verificadas. | Implementado, testado |
| **Relação como canal lateral** | Criar uma relação exige alcançar **os dois** extremos; falhar num recusa o passo inteiro e não deixa aresta. | Implementado, testado contra DB real |
| **Relação entre ambientes diferentes** | Recusada: uma aresta assim não teria contexto único de autorização. | Implementado, testado |
| **Relação inventada** | Conjunto fechado de sete relações; `grants_admin_to` não é uma delas. | Implementado, testado |
| **Conversão Ideia → Projecto duplicada** | `promote_idea` recusa uma Ideia que já tem `promoted_project_id`. Repetir um plano confirmado produz um Projecto e um conflito. | Implementado, testado contra DB real |
| **Transição de estado inventada** | Os alvos vêm de `workflow::*_targets_from`. Um modelo que proponha `discovery → promoted` é recusado pelo domínio. | Implementado, testado |
| **Selecção como atalho à autorização** | O que o membro selecciona passa pelo mesmo resolver. Uma selecção inalcançável **pára o pedido**. | Implementado, testado |
| **Conteúdo hostil em Notas, fontes e títulos** | Testado com instruções de sistema, nomes de capabilities e pseudo-invocações de ferramentas dentro de artefactos reais. Nada altera o principal, e nada é executado. | Implementado, testado contra DB real |
| **RAG leakage** | Dois tectos: leitura do actor, e processamento por IA, mais baixo. O que é retido é contado e mostrado. | Implementado, testado |
| **Schema leakage** | Autorizar **antes** de validar. | Implementado |
| **Fuga pela mensagem de recusa** | Todas as recusas leem `PermissionDenied`; nenhuma diz qual porta travou. | Implementado, testado |
| **Fuga pela auditoria** | Guarda quem, qual capability, que risco, que desfecho. Nunca o prompt nem a entrada. | Implementado, testado |
| **Autonomia não supervisionada** | `Autonomous` existe no tipo e é inalcançável: o tecto é `Workflow`. | Implementado, testado |
| **Poisoned memory** | — | **Sem memória agentic persistente.** Nada a envenenar. |
| **Model compromise** | — | **Sem modelo.** Reabrir quando existir um nó. |
| **Node compromise (IA)** | — | **Sem nó.** Ver Computação abaixo para o modelo de nós. |

### Computação e cadeia de fornecimento

| Ameaça | Mitigação | Estado |
|---|---|---|
| **Node compromise** | Tudo o que um nó reporta é entrada não confiável; nunca influencia autorização; números limitados ao domínio da coluna. | Implementado. |
| **Node Agent compromise** | Identidade de máquina própria, revogável; nunca credenciais humanas; ligação só para fora. | Implementado. |
| **Lateral movement** | Nó não aceita tráfego de aplicação; futura ligação por WireGuard. | Arquitectura definida; sem nó. |
| **Capability maliciosa** | WASM sem rede, sem filesystem, sem ambiente; fuel, memória e tempo limitados, e o limite de tempo de uma invocação é o seu — não o da que corre ao lado. Pedir rede é **recusado**. | Implementado, testado contra componente real. Isolamento temporal corrigido pela Baseline v1 (F-08). |
| **Supply chain** | Toolchain fixado; `cargo audit` na CI **e** no sweep local, com excepções escritas; features de dependências limitadas ao necessário, para que código não usado não seja sequer ligado; capacidade de exemplo deliberadamente sem dependências pesadas. | Parcial — **sem verificação de assinatura de capacidades**. Stack TLS legada removida pela Baseline v1 (F-06). |
| **Backup compromise** | Nenhuma. O procedimento de migração produz um `pg_dump` **não cifrado** e um manifesto que enumera todas as identidades institucionais. Quem obtiver o dump obtém tudo o que a instituição classificou; quem obtiver **também** a `OCINYE_MAIL_KEY` obtém as credenciais de caixa dos membros. | **Não mitigado.** A chave viaja por canal próprio, o que separa os dois compromissos e não impede nenhum. Cifrar os artefactos e definir retenção é trabalho por fazer ([ADR-0700](../adrs/0700-institutional-continuity-and-portability.md)). |

## Suposições

- O Identity Provider é operado com competência e não está comprometido.
- Quem tem `platform_admin` é de confiança: pode conceder papéis. A auditoria
  regista-o, mas não o impede.
- O host que corre o Core é de confiança. Uma capacidade WASM não o é.
- A base de dados não é acessível directamente por membros.

## Lacunas conhecidas

Nenhuma destas está escondida; todas estão listadas no
[README de segurança](../security/README.md):

1. Sem rate limiting fora do início de sessão, que **tem** throttling.
2. Sem análise antimalware de uploads.
3. Sem verificação de assinatura de componentes WASM.
4. Sem backups configurados. O **procedimento** de restore existe e foi
   exercitado uma vez à mão a 2026-08-28, incluindo o controlo negativo que
   distingue restaurar de recriar; o que não existe é agendamento, cópia fora
   do servidor, retenção ou cifra dos artefactos. O RPO real é *desde o último
   dump que alguém correu à mão*.
5. Sessões do Workspace em memória: um reinício termina-as.
6. **O fluxo OIDC não foi verificado ponta a ponta** contra um IdP a correr.
7. ~~Planos agentic não são persistidos.~~ **Fechado em 2026-08-23** pela
   milestone Agentic Plan Lifecycle: o ciclo funciona por HTTP, a execução
   reclama o plano atomicamente e reautoriza cada passo, e quinze testes contra
   PostgreSQL cobrem-no.
