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
| **Configurado** — existe agendamento e destino | **Não** |
| **Executado** — correu e produziu artefacto verificável | **Sim, uma vez, à mão**, a 2026-08-28 |
| **Restore validado** — foi restaurado e verificado | **Sim, uma vez**, a 2026-08-28 |

**Continua a não existir backup operacional.** Um ensaio executado uma vez à
mão, numa máquina de desenvolvimento, prova que o procedimento funciona. Não
prova que existe uma cópia da instituição em qualquer momento dado — porque não
existe.

## O ensaio de 2026-08-28

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

- **Nenhuma cópia fora do servidor.** Um dump que só existe na máquina que ardeu
  não é um backup.
- **Nenhum agendamento.** O RPO real é *desde o último dump que alguém correu à
  mão*.
- **Nenhuma política de retenção.**
- **Nenhuma cifra dos artefactos de backup.** Um dump não cifrado é uma cópia de
  tudo o que a instituição classificou.
- **Nenhum procedimento de rotação da chave de selagem.**
- **3-2-1 não existe.** Três cópias, dois meios, uma fora do local.
  **Não declarar antes de existir.**
