# Continuidade, backups e portabilidade

> **Um servidor pode desaparecer. A instituição continua.**

A decisão está em
[ADR-0700](../adrs/0700-institutional-continuity-and-portability.md). O
procedimento está em
[migrate-to-another-server](../runbooks/migrate-to-another-server.md). Esta
página diz o **estado**.

## Estado

> **IMPLEMENTATION COMPLETE — OPERATIONAL ACTIVATION PENDING FIRST SERVER**

O software está feito e provado. O que falta não se resolve aqui: falta um
servidor onde o agendador dispare.

**Não instalar um agendador numa máquina de desenvolvimento para poder escrever
que existe backup periódico.** Seria fabricar exactamente a evidência que este
trabalho existe para recusar.

## O portão de activação

Quando existir o primeiro servidor da Ocinye, isto fecha-se com uma execução
real. É pequeno e é factual:

1. instalar a unidade correspondente de [`infra/scheduling/`](../../infra/scheduling/);
2. confirmar que está `enabled`/`loaded`;
3. **esperar por uma execução iniciada pelo agendador**;
4. confirmar que produziu o conjunto cifrado;
5. confirmar a cópia **por leitura de volta no destino externo**;
6. confirmar a retenção, local e remota;
7. executar um restore drill **a partir desse conjunto**;
8. medir, e só então fixar RPO e RTO.

E a distinção que dá sentido ao passo 3:

| | |
|---|---|
| cópia corrida à mão | **não satisfaz** |
| comando corrido à mão a imitar o agendador | **não satisfaz** |
| o agendador disparou o trabalho | **satisfaz** |

A terceira linha é a única que prova a propriedade que interessa: que a
instituição é copiada **quando ninguém se lembra de a copiar**.

## Estado dos mecanismos

Os três estados obrigatórios de `CLAUDE.md` §63, e um quarto que faltava:

| Estado | Situação |
|---|---|
| **Classificado** — sabe-se o que tem de viajar | **Sim**, em código, com teste que cobre o esquema |
| **Configurado** — existe procedimento e destino configurável | **Sim**, `scripts/institutional-backup.sh`, com cifra, destino externo e retenção nas duas pontas |
| **Executado** — correu e produziu artefacto verificável | **Sim**, sem terminal, cifrado, confirmado por leitura de volta |
| **Restore validado** — foi restaurado e verificado | **Sim**, com as três verificações a observar e a passar |

E o quinto, que é o que falta:

| | |
|---|---|
| **Em operação** — corre sem que alguém se lembre | **Não.** Não há servidor onde instalar o agendador. |

**Portabilidade não é política de backup.** Os ensaios provam que a instituição
sabe mudar de sítio, e que o processo que a copia existe e funciona. Não provam
que existe uma cópia da instituição em qualquer momento dado — porque, sem
agendador a correr num servidor, não existe.

> **Portability is not yet an operational backup policy. A recoverable
> institution requires scheduled, protected, off-host copies and rehearsed
> restoration.**

E a lição que o incidente da cópia externa deixou:

> **Successful local execution is not evidence of successful off-host
> preservation. A remote copy is confirmed only by an observation made against
> the destination.**

## O trio

```bash
./scripts/institutional-backup.sh                 # produz um conjunto
./scripts/institutional-restore.sh <conjunto>     # repõe numa base vazia
./scripts/institutional-verify.sh <conjunto>      # as três perguntas
```

Nenhum deles imprime «backup completed successfully» porque o `pg_dump`
terminou com zero. O `backup` recusa enviar um conjunto em claro para fora do
servidor; o `restore` recusa escrever por cima de uma base com estado; o
`verify` sai **2** — nem 0 nem 1 — quando nada falhou e alguma coisa não chegou
a ser observada.

A chave de selagem **nunca** entra no conjunto. Viaja por canal próprio, e é
por isso que perder um conjunto não é perder tudo.

Não há dependência de fornecedor: o comando que move os objectos é
configuração (`OCINYE_OBJECT_SYNC_CMD`), e recebe a pasta em
`$OCINYE_OBJECT_DIR`. Serve `mc`, `rclone`, `aws s3` ou o que a instalação
tiver.

## O ensaio de 2026-08-29 — A → B → C

Uma instituição construída de propósito, pela API, e migrada duas vezes: uma
unidade, uma ideia com o seu ambiente, dois documentos com bytes reais, a
cadeia científica completa — hipótese, metodologia, versão publicada, estudo,
execução, resultado `RESTRICTED`, validação — e dois membros.

### Os bytes, oito controlos

| | Resultado |
|---|---|
| destino vazio | **FAIL**, com os dois objectos nomeados |
| endpoint inacessível | **INVALID** — «nada foi verificado», e não «em falta» |
| storage não configurado | **INVALID** — «metade do estado não foi observada» |
| transporte real dos bytes | 2 objectos, 4 616 bytes |
| restore completo | **PASS**, somas recalculadas |
| um objecto corrompido | **FAIL** por soma: esperava `436d6db0b98c…`, leu `2535afb9654c…` |
| um objecto apagado | **FAIL** por ausência |
| um objecto a mais | **passa, e é dito**: órfão nomeado, nada apagado |

O terceiro e o segundo são a razão de este comando existir. A primeira versão
dele escreveu «303 objectos em falta» contra um MinIO desligado.

### O servidor C, inteiramente limpo

Base vazia, bucket vazio, Redis vazio, sem fornecedor de IA, sem nó de
computação, e **credenciais de armazenamento novas** — as antigas deixam de
funcionar, que é o que «substituível» quer dizer.

Conjunto cifrado com `age` → restaurado → verificado:

```
  as linhas          PASS     29 recursos, 2 objectos, 4 arestas
  os bytes           PASS
  a legibilidade     NOT_RUN  não havia estado selado para abrir
  EXIT=2
```

**Saiu 2, e está certo.** Nada falhou e uma das três não observou nada.

### A retenção, e os restos

Cinco conjuntos com `OCINYE_BACKUP_KEEP=3`, e corridas falhadas pelo meio:

```
  ocinye-…232404Z                 ← mantidos: 3
  ocinye-…232402Z
  ocinye-…232400Z
  ocinye-…232401Z-INCOMPLETO      ← não conta como conjunto, e não é apagado
```

Um `INCOMPLETO` **não entra** na contagem: se entrasse, a rotação apagaria uma
cópia boa para guardar os restos de uma tentativa. Mas também não fica para
sempre — guarda-se **o último**, porque o disco a encher-se de tentativas
falhadas é uma maneira lenta de impedir a cópia seguinte.

### Quando a verificação tem de falhar

Base restaurada, bucket vazio — o pior caso realista, porque é o que uma
verificação ingénua declara bom:

```
  as linhas          PASS     29 recursos, 2 objectos, 4 arestas
  os bytes           FAIL     2 objecto(s) em falta
  a legibilidade     NOT_RUN
  EXIT=1
```

O `verify-snapshot` passa: as linhas chegaram todas. É por isso que ele não
chega.

### Pelo produto, e não pelos comandos

No Workspace do servidor B, com sessão nova:

| | |
|---|---|
| o membro autentica-se | sessão nova; as antigas não viajam |
| o ambiente abre | com os dois documentos |
| a cadeia científica | hipótese, metodologia, estudo e resultado no ecrã |
| o resultado | mesmo identificador, `produzido por`, `segue`, «Validação confirmou» |
| a linhagem | montante e jusante percorrem-se, com `origem: operation` |
| o documento | descarrega e a soma bate: `436d6db0b98c…` nos três sítios |
| `RESTRICTED` | um membro sem acesso recebe `not_found` — nem a existência |
| a auditoria | mesmo primeiro evento; a actividade nova fica por cima |
| sem IA | `ai_general: no_resource`, e o conhecimento e a pesquisa abrem à mesma |

### A chave

A base de desenvolvimento tem 318 credenciais seladas reais. Dump, restauro
para base limpa, e `verify-keys` dos dois lados:

```
  origem:  235 de 318 não abriram
  destino: 235 de 318 não abriram
```

**Idêntico.** As 83 que abrem são as seladas com a chave configurada, e abrem
nos dois lados: o restauro preservou a interpretabilidade exactamente. As 235
são fixtures acumuladas por testes, cada uma selada com uma chave efémera —
resíduo do ambiente de desenvolvimento, não defeito.

E sem a chave:

```
  Error: há 318 credencial(is) selada(s) nesta base e nenhuma
  `OCINYE_MAIL_KEY` configurada. O estado chegou íntegro e ilegível.
```

É o único estado que os outros dois verificadores deixam passar os dois.

**O que não foi produzido:** um `verify-keys` inteiramente verde sobre a
instituição construída de propósito. Ligar uma caixa exige um servidor IMAP a
responder — o Core recusa guardar credenciais que não conseguiu provar — e o
ensaio não tinha nenhum.

## O ensaio anterior, de 2026-08-28

Executado contra a base institucional local: 19 migrations, 166 232 recursos,
303 objectos registados, 91 arestas de proveniência, 39 165 eventos de
auditoria.

| Passo | Resultado |
|---|---|
| `snapshot` da base de origem | manifesto de 8 MB, saída zero |
| `pg_dump --format=custom` | 19 MB em 1,4 s |
| base nova criada e **vazia** | 0 tabelas |
| **controlo negativo**: `sqlx migrate run` numa base vazia, depois `verify-snapshot` | **saída 1**, enumerou as ausências por família — 29 732 pessoas, 7 817 credenciais, 28 105 papéis |
| `pg_restore` para uma base limpa | 1,9 s |
| `verify-snapshot` contra o manifesto | **saída 0**, «chegaram com as mesmas identidades» |

O controlo negativo é a parte que importa. Uma base criada de novo com as
mesmas migrations tem as mesmas 62 tabelas, e nada em comum com a instituição.
Sem esse passo, o verde do passo seguinte não provaria nada:

> **Restaurar não é criar o domínio outra vez.**

**O que o ensaio não cobriu:** o transporte do Object Storage. O MinIO local
não estava a correr, e `verify-objects` recusou-se a concluir seja o que for —
correctamente. Metade do estado autoritativo não foi exercitada, e essa metade
continua por provar.

## O que viaja, e o que não viaja

Não está aqui numa tabela escrita à mão. Está em código, e responde:

```bash
ocinye-core-server continuity-inventory
```

Três activos viajam. O terceiro é o que costuma ser esquecido:

| | Porquê |
|---|---|
| **PostgreSQL** | fonte canónica. Não existe noutro sítio |
| **Object Storage** | os bytes a que a base aponta |
| **`OCINYE_MAIL_KEY`** | sem ela, `mailbox_credentials` chega íntegra e **ilegível** |

Um `pg_dump` salva a base. Não salva os bytes a que ela aponta, nem a chave sem
a qual parte das linhas é ruído. Um backup assim é uma cópia perfeitamente
íntegra e completamente inútil, e isso só se descobre no dia do desastre.

As **credenciais de fornecedor** — S3, correio, IA — não viajam. Rodam-se no
servidor novo.

## As duas verificações, e o que cada uma prova

| Comando | Prova | Não prova |
|---|---|---|
| `verify-snapshot` | que cada identidade, chave e aresta de proveniência chegou | que existe um único byte no bucket |
| `verify-objects` | que os bytes lidos do bucket batem com as somas registadas | nada, se o bucket não responder — e di-lo |

A segunda linha da segunda coluna é a razão de o probe de saúde existir. A
primeira versão de `verify-objects` escreveu o relatório mais alarmante que
sabe escrever — 303 objectos em falta — contra um MinIO que estava
simplesmente desligado.

> **`INVALID` não é `FAIL`.** Um verificador que não conseguiu observar não
> descobriu um problema: não correu.

## O ensaio operacional de 2026-08-29 — A → cofre → B

O anterior provou **portabilidade**. Este prova o **processo**: cópia
automática, cifrada, para um endpoint S3 fora do servidor, restaurada num
servidor inteiramente limpo, com as **três** verificações a observar.

A instituição de origem tinha as três coisas ao mesmo tempo: 165 641 recursos,
dois objectos com bytes reais, e **83 credenciais seladas que abrem com a chave
da instalação**.

### A cadeia

```
servidor A ── cópia automática, sem terminal ──▶ pacote cifrado com `age`
                                                        │
                                          cofre S3 fora do servidor
                                                        │
servidor B (base vazia, bucket vazio, Redis vazio, credenciais novas)
                                                        │
                                                    restauro
                                                        │
                                              verificação institucional
```

| | |
|---|---|
| cópia automática, sem terminal e sem `stdin` | `EXIT=0` |
| pacote cifrado | 28 279 736 bytes, com a soma ao lado |
| cópia para o cofre | **confirmada por leitura de volta** |
| soma do pacote recebido em B | confere |
| restauro para base vazia | 2 objectos repostos |
| **as linhas** | `PASS` — 165 641 recursos, 2 objectos, 91 arestas |
| **os bytes** | `PASS` |
| **a legibilidade** | `PASS` — 83 credenciais abriram |
| | **`EXIT=0`** |

**É a primeira verificação institucional inteiramente verde**: as três
perguntas observaram, e as três passaram.

### O controlo negativo que o par exige

Mesmo restauro, mesma base, mesmos bytes — **sem a chave durável**:

```
  as linhas          PASS
  os bytes           PASS
  a legibilidade     FAIL
      O estado chegou íntegro e ilegível.
  EXIT=1
```

Bytes correctos e indecifráveis também são conhecimento perdido, e o
verificador diz que sim.

### Pelo produto, no servidor B

Entrada no Workspace, o ambiente abre com os dois documentos, e o documento
descarrega com a soma `436d6db0b98c…` — a mesma da origem.

### Três defeitos meus, encontrados a correr isto

**A cópia «fora do servidor» ficou dentro do repositório.** O ficheiro de
ambiente tinha `OCINYE_BACKUP_REMOTE_CMD=mc cp --quiet` **sem aspas**. Em `sh`,
`VAR=valor comando` corre o comando com a variável apenas no ambiente dele — a
variável nunca foi definida, o script caiu no `rsync -a` por omissão, e o
`rsync` copiou alegremente para uma pasta local chamada `OFF/`, dentro da
árvore de trabalho. E saiu zero. **A cópia externa foi declarada feita sem
nunca ter acontecido.**

A correcção não é ensinar aspas a quem escreve o ficheiro: é **ler de volta**.
`OCINYE_BACKUP_VERIFY_CMD` devolve a soma do que está no destino, e o script
compara. Sem esse comando, a cópia é declarada **«ENVIADA, NÃO CONFIRMADA»** —
nunca «ok».

> **«O comando de transporte saiu zero» não é «a cópia chegou».**

**A recusa saía muda.** Com o transporte a fingir, o script recusava — e sem
dizer porquê: o `set -e` matava-o na substituição antes de a mensagem existir,
e o `trap` calava-se porque depois da cifra já não há pasta para marcar. Uma
recusa sem razão obriga quem a lê a adivinhar, e quem adivinha às três da manhã
adivinha mal.

**O cofre crescia sem limite.** A retenção governava só esta máquina. Quatro
corridas com `KEEP=2` deixavam dois conjuntos aqui e cinco pacotes lá. Um cofre
cheio impede a cópia seguinte tão bem como um disco cheio.
`OCINYE_BACKUP_REMOTE_PRUNE_CMD` aplica retenção no destino; sem ele, cada
corrida diz **«NÃO APLICADA»**.

## Os nove factos operacionais

Medidos a 2026-08-29, sem inferência. `verify.sh` verde demonstra o contrato
que existe; **não cria política operacional que ainda não foi construída.**

| | |
|---|---|
| **Onde ficam fisicamente os backups** | Em lado nenhum. `OCINYE_BACKUP_DIR` não está definida em nenhuma instalação; os conjuntos do ensaio viveram num directório temporário e foram apagados. |
| **Existe cópia fora da máquina** | **O mecanismo existe e foi exercitado** contra um endpoint S3 separado, com confirmação por leitura de volta e retenção no destino. **Nenhuma instalação o tem configurado**, porque nenhuma instalação existe. |
| **Automático ou manual** | **Executável sem terminal e sem `stdin`, com estado de saída inequívoco** — provado. As unidades de agendamento estão escritas em [`infra/scheduling/`](../../infra/scheduling/) e **não estão instaladas em lado nenhum**: agendar é configuração de um servidor que não existe. Até lá, o RPO é *desde o último conjunto que alguém produziu*. |
| **Os artefactos estão cifrados** | **Quando se pede.** A cifra `age` é opcional e **obrigatória para destino externo**: o script recusa enviar em claro. O pacote leva a soma ao lado, guardada fora dele, para quem transporta poder conferir sem ter a chave. `OCINYE_BACKUP_RECIPIENT` não está definida em nenhuma instalação. |
| **Que material interpreta estado durável** | Uma peça: `OCINYE_MAIL_KEY`, que interpreta `mailbox_credentials`. É a única coluna de criptograma do esquema, há dois portões que o mantêm verdadeiro, e **está provado nas duas direcções**: com a chave, 83 credenciais abrem no servidor novo; sem ela, a verificação recusa. |
| **Retenção implementada** | `OCINYE_BACKUP_KEEP` conjuntos completos (7 por omissão) mais **um** resto de tentativa falhada, **e** retenção no destino por comando configurável. Exercitada nas duas pontas: quatro corridas com `KEEP=2` deixaram dois aqui e dois no cofre. Nenhuma instalação a define, porque nenhuma produz conjuntos. |
| **Redis vazio** | O servidor B arrancou com um Redis de 0 chaves e nenhuma verificação falhou. Está classificado `EPHEMERAL`, e essa é a prova de que não é fonte de verdade. |
| **Sem IA, correio e computação** | `ai_*` e `compute` reportam `no_resource`; `lexical_search` fica `available`. O conhecimento, a pesquisa e a cadeia científica abriram à mesma no servidor migrado. |
| **Migração completa já executada** | **Sim, duas vezes.** A → B → C provou a portabilidade e terminou em `EXIT=2` (a legibilidade não teve o que observar). A → cofre → B provou o processo e terminou em **`EXIT=0`, com as três a observar** — incluindo 83 credenciais seladas que abriram do outro lado. |

## Artefactos de modelo — a terceira forma da memória

Decisão em [ADR-0203](../adrs/0203-institutional-model-artifacts.md).

Até aqui a memória institucional era **explícita**: documentos, datasets,
resultados, proveniência. No dia em que a Ocinye afinar um modelo, parte da
capacidade passa a existir **nos pesos**, e não se reconstrói a olhar para o
PostgreSQL.

A classificação de continuidade já distingue os dois casos, e o
`continuity-inventory` responde:

| | Viaja | |
|---|---|---|
| `DURABLE_MODEL_ARTIFACT` | **sim** | o que a Ocinye treinou. Ninguém mais o tem. |
| `EXTERNAL_REACQUIRABLE` | não | o modelo base publicado — **se** a versão exacta, a soma e a licença o permitirem. |

### O que a auditoria de 2026-08-29 encontrou

**`ai_models` não é um registo de artefactos.** É um inventário reportado pelo
nó: `replace_reported_models` apaga e volta a inserir a cada relatório, pelo
que os identificadores são novos de cada vez, e `ON DELETE CASCADE` faz a linha
desaparecer com o nó. **Hoje é o nó que detém o modelo** — o inverso do que
`ADR-0203` decide.

Isto expôs um defeito no próprio manifesto: `ai_models` estava a ser comparada
por identidade. Passa hoje porque há zero nós, e **partia-se no dia em que o
primeiro ligasse**. Corrigido, com a razão escrita e um teste que lê o código
que a justifica.

**Um artefacto de modelo não tem por onde entrar.** A lista de tipos aceites
recusa `application/octet-stream` — correctamente, é uma lista de permissões
para artefactos documentais. Alargá-la seria transformar a fronteira de uploads
num canal para binários arbitrários. Um modelo precisa do seu próprio caminho
tipado.

### O portão de entrada

> **No first institutional model without continuity.**

As perguntas abaixo não são roadmap: são **precondição**, e vivem em código
(`continuity::models`). No dia em que uma migration criar `model_artifacts`,
`model_versions`, `training_runs`, `evaluation_runs` ou `model_checkpoints`, o
portão fecha e nomeia as que continuam abertas.

> **An Ocinye-trained model must not be promoted to durable institutional
> status until its artifact, exact base-model dependency, training lineage,
> required runtime components, classification, evaluation evidence and restore
> path are governed by the continuity system.**

Assim não é preciso fabricar um sistema de treino agora, e a dívida também não
espera pelo dia em que um `.safetensors` importante está perdido num SSD de
GPU.

### As onze perguntas

Dez estão por responder, porque não há modelo nenhum para as responder. Uma
está respondida, e é a fundadora:

| | |
|---|---|
| os artefactos sobrevivem à perda do nó de treino | por responder |
| os pesos vivem fora do nó de computação | por responder |
| cada versão liga ao modelo base **exacto**, com soma e licença | por responder |
| as versões de dataset de treino são preservadas | por responder |
| a receita de treino é preservada | por responder |
| tokenizer, configuração e adaptadores acompanham os pesos | por responder |
| os checkpoints seguem a política de retenção | por responder |
| o modelo restaurado mantém as somas idênticas | por responder |
| a linhagem inspecciona-se **sem** o servidor GPU original | por responder |
| o conhecimento continua acessível com o modelo em baixo | **sim** — é a arquitectura actual |
| um modelo treinado sobre dados sensíveis não vira contorno de autorização | por responder |

### O que não foi construído, e porquê

Não existe `Model`, `ModelVersion`, `ModelArtifact`, `TrainingRun` nem
`EvaluationRun`. Não existe promoção, retenção aplicada nem registo de licença.

Não por esquecimento: não há nó de computação, não há treino, e não há um único
artefacto para preservar. A forma correcta destas tabelas depende do que só se
sabe ao afinar o primeiro modelo — que técnica, que ficheiros acompanham, que
avaliação sustenta a promoção. Construí-la agora seria desenhar contra
imaginação.

## O que continua a não existir

- **Nenhum agendamento.** O procedimento existe e corre-se à mão. Enquanto não
  houver `cron`, `systemd timer` ou equivalente configurado numa instalação
  real, **o RPO é *desde o último conjunto que alguém produziu*.**
- **Nenhum destino externo configurado.** O script suporta um
  (`OCINYE_BACKUP_REMOTE`) e recusa-se a usá-lo sem cifra. Nenhuma instalação o
  tem definido, porque nenhuma instalação existe fora de desenvolvimento.
- **Nenhum procedimento de rotação da chave de selagem.** A `OCINYE_MAIL_KEY`
  viaja como está. Trocá-la exige reselar `mailbox_credentials`, e isso não
  está escrito nem implementado.
- **Nenhum ensaio periódico.** Os de 2026-08-29 correram uma vez cada. Um
  procedimento que se prova uma vez e nunca mais é um procedimento que se
  descobre partido no dia em que é preciso.
- **Dívida conhecida na interface do transporte.** `OCINYE_BACKUP_REMOTE_CMD` é
  uma linha de shell inteira dentro de uma variável, e foi por aí que a cópia
  externa se perdeu: uma linha sem aspas num ficheiro de ambiente não define
  variável nenhuma, e o valor por omissão escreveu para o sítio errado em
  silêncio. A confirmação por leitura de volta fecha a consequência grave — já
  não é possível declarar uma cópia que não chegou —, mas a interface continua
  frágil. Separar executável e argumentos, ou escolher um *driver* nomeado,
  fica registado como dívida e **não se faz agora**: mudaria a configuração de
  um processo que ainda não corre em lado nenhum.
- **3-2-1 não existe.** Três cópias, dois meios, uma fora do local.
  **Não declarar antes de existir.**
