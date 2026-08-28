# Continuidade, backups e portabilidade

> **Um servidor pode desaparecer. A instituição continua.**

A decisão está em
[ADR-0700](../adrs/0700-institutional-continuity-and-portability.md). O
procedimento está em
[migrate-to-another-server](../runbooks/migrate-to-another-server.md). Esta
página diz o **estado**.

## Estado actual

Os três estados obrigatórios de `CLAUDE.md` §63, e um quarto que faltava:

| Estado | Situação |
|---|---|
| **Classificado** — sabe-se o que tem de viajar | **Sim**, em código, com teste que cobre o esquema |
| **Configurado** — existe procedimento e destino configurável | **Sim**, `scripts/institutional-backup.sh`. **Sem agendamento.** |
| **Executado** — correu e produziu artefacto verificável | **Sim**, cifrado, com somas reconferidas |
| **Restore validado** — foi restaurado e verificado | **Sim, para a base e para os bytes**, a 2026-08-29 |

**Continua a não existir backup operacional.** Um ensaio executado uma vez à
mão, numa máquina de desenvolvimento, prova que o procedimento funciona. Não
prova que existe uma cópia da instituição em qualquer momento dado — porque não
existe.

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
- **Nenhum ensaio periódico.** O de 2026-08-29 foi executado uma vez. Um
  procedimento que se prova uma vez e nunca mais é um procedimento que se
  descobre partido no dia em que é preciso.
- **3-2-1 não existe.** Três cópias, dois meios, uma fora do local.
  **Não declarar antes de existir.**
