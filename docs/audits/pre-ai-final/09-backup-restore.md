# 09 — Backup / Restore (Pré-IA)

`backup criado ≠ backup provado` (§63). Esta auditoria não se contentou com um
`pg_dump` a sair zero: montou um ambiente isolado e correu o mecanismo real de
continuidade de ponta a ponta. E a prova apanhou um defeito.

## O que se encontrou — P1, corrigido

O `snapshot` de continuidade — a espinha do `institutional-backup.sh` e do
`verify-snapshot` — **falhava no esquema actual** com `column "id" does not
exist`. Produção não conseguia produzir uma cópia verificável. A causa:
`file_favourites` (migração 0047) nasceu com chave composta `(person_id,
file_id)` e sem `id`, mas está declarada como tabela que viaja
(`Comparacao::Identidades`), e o manifesto enumera cada tabela que viaja por
`SELECT id FROM {tabela}`. Foi a única a quebrar a convenção do esquema, e entrou
**depois** do ensaio de 2026-08-29 — por isso nada o via.

**Corrigido** (PR #115, `main`): migração **0048** dá à `file_favourites` um `id`
UUID único, e um guarda de regressão (`crates/ocinye-core/tests/continuity.rs`)
corre o manifesto de facto contra o esquema real e exige `Ok` — o portão de
decisões via que cada tabela tinha uma decisão, mas não que a decisão
`Identidades` se conseguia **executar**. Ver [findings.md](findings.md) (F-08).

## O ensaio — provado

Ambiente isolado: um Postgres descartável (Docker), separado da produção, sem
tocar em dados reais. Objectos desligados (`OCINYE_BACKUP_BACKEND=none`, sem
`OCINYE_STORAGE_BUCKET`) — a base é o núcleo da continuidade; a metade dos
objectos exercita-se com o mesmo `mc mirror` e fica para o ensaio com armazenamento.

| Passo | Resultado |
|---|---|
| Estado conhecido | `bootstrap-admin` semeou a organização e o administrador (48 migrações aplicadas; `file_favourites.id` presente) |
| **Backup** (`institutional-backup.sh`) | manifesto (6007 B), `pg_dump` (≈349 KB), somas reconferidas, **cifrado com `age`** (≈379 KB). «Isto é um backup executado.» |
| Destruir/alterar | a base de destino, vazia (esquema ausente) |
| **Restauro** (`institutional-restore.sh`) | decifra com a chave `age`, `pg_restore` para a base vazia — «Base restaurada.» |
| **Integridade** | o estado reaparece **idêntico**: `organisations|people|credentials = 1|2|1`, as mesmas identidades (`inst@`, `admin@proof.test`) |
| **Controlo negativo** | restauro com a **chave errada** → `age: no identity matched any of the recipients` → «não foi possível abrir o conjunto». Sem restauro, sem fuga. |
| Duração | backup e restauro na ordem de **segundos** (base de ensaio pequena; a medição escala com o tamanho real) |

A chave de selagem **nunca** viaja no conjunto (o `institutional-backup` recusa-a
por desenho); no ensaio, o `age` foi a cifra do transporte, com a chave privada
fora do conjunto.

## O que fica

- **Objectos + cópia externa + retenção** — exercitados no ensaio de 2026-08-29 e
  no `mc mirror`; re-exercitar a metade dos objectos num ambiente com armazenamento
  fecha o critério 10 do [gate](gate.md).
- **Backup periódico** continua a **não existir** em produção (§1): o mecanismo
  está provado, o agendador não corre em lado nenhum. O RPO é «desde o último
  conjunto que alguém produziu». Não é uma lacuna deste mecanismo — é a activação
  operacional, que precede a primeira máquina de destino.
