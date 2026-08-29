# Runbook — Mudar a Ocinye para outro servidor

> **Um servidor pode desaparecer. A instituição continua.**

Este runbook move uma instalação do Ocinye OS de um servidor para outro, e
prova que o que chegou é o que saiu. Serve tanto para uma migração planeada
como para uma recuperação depois de perder a máquina — a diferença está no
[passo 1](#1-parar-de-escrever), que só é possível no primeiro caso.

**Estado deste procedimento:** executado de ponta a ponta a 2026-08-29,
incluindo o transporte do Object Storage entre dois endpoints S3-compatible e
um restauro para um servidor inteiramente limpo. Resultados em
[`docs/backups/README.md`](../backups/README.md).

**O caminho curto.** Os passos abaixo são o que os três comandos fazem, e
existem para quem precise de os fazer à mão ou de perceber o que correu mal:

```bash
./scripts/institutional-backup.sh                 # no servidor antigo
./scripts/institutional-restore.sh <conjunto>     # no novo, base vazia
./scripts/institutional-verify.sh <conjunto>      # e leia o código de saída
```

## Antes de começar

Pergunte ao sistema o que é preciso levar. A resposta é código, e não uma lista
que alguém manteve:

```bash
ocinye-core-server continuity-inventory
```

Três coisas viajam, e as três são necessárias:

| | Onde está | Sem ela |
|---|---|---|
| **PostgreSQL** | base institucional | não há instituição |
| **Object Storage** | bucket S3-compatible | as referências apontam para o nada |
| **`OCINYE_MAIL_KEY`** | cofre de segredos | `mailbox_credentials` chega ilegível |

A chave viaja **por um canal próprio**, e nunca dentro do dump nem no mesmo
ficheiro. Não é estado institucional: é o que torna parte dele legível.

As credenciais de fornecedor — S3, correio, IA — **não** viajam. Rodam-se no
servidor novo. Copiá-las alarga a exposição sem preservar memória nenhuma.

## 1. Parar de escrever

Só possível numa migração planeada. Numa recuperação, salte para o passo 3 e
leia o [RPO](#rpo-e-rto) para saber o que se perdeu.

```bash
systemctl stop ocinye-core ocinye-worker    # ou o equivalente da instalação
```

Deixe o worker drenar a `outbox_events` antes de parar. A fila não é comparada
pelo manifesto — é entrega, não estado — e um evento por entregar perde-se em
silêncio.

## 2. Descrever o que se leva

```bash
ocinye-core-server snapshot > manifesto.json
```

Escreve o manifesto em `stdout` e o resumo em `stderr`, para que o ficheiro
saia limpo e quem corre o comando continue a ver o que levou.

Guarde `manifesto.json` **fora** do dump. É a única coisa que permite ao
servidor novo dizer que o que recebeu é o que saiu.

## 3. Copiar a base

```bash
pg_dump --format=custom --no-owner --no-privileges "$OCINYE_DATABASE_URL" -f ocinye.dump
```

`--no-owner` e `--no-privileges` porque os papéis do PostgreSQL são do servidor,
e não da instituição.

## 4. Copiar os objectos

O bucket inteiro, com as chaves preservadas. As chaves estão registadas na base
e um objecto que mude de chave deixa de ser encontrável:

```bash
mc mirror --preserve origem/ocinye-artifacts destino/ocinye-artifacts
```

O endereço do serviço é configuração da instalação; a **chave** é institucional.
Mudar de fornecedor muda o endereço e nunca as chaves.

## 5. Preparar o servidor novo

Instale o Ocinye OS, configure `.env`, e **não** corra `bootstrap-admin`: já há
administradores, e vêm no dump.

O Redis arranca vazio. É assim que deve ser: não autoriza e não persiste. Se
alguma coisa deixar de funcionar por causa disso, o defeito é a coisa que passou
a depender dele para persistir.

## 6. Restaurar

```bash
createdb ocinye
pg_restore --no-owner --no-privileges --dbname "$OCINYE_DATABASE_URL" ocinye.dump
```

**Não corra `sqlx migrate run` numa base vazia para «preparar» o restore.** Isso
cria a instituição de novo — as mesmas tabelas, nenhuma da mesma história — e é
exactamente o erro que o passo seguinte foi feito para apanhar.

Se o servidor novo trouxer uma versão mais recente do Ocinye OS: **restaure para
o nível compatível, verifique, e só depois evolua.** Restaurar directamente para
um esquema mais recente confunde uma falha de transporte com uma falha de
evolução.

## 7. Provar que chegou

```bash
ocinye-core-server verify-snapshot < manifesto.json
echo $?
```

Sai **zero** quando cada identidade, cada objecto registado e cada aresta de
proveniência coincidem. Sai **não-zero** e diz onde quando não coincidem.

Leia o código de saída, e não o texto. Um comando que imprime linhas bonitas e
sai não-zero falhou.

Depois, os bytes — que é outra pergunta:

```bash
ocinye-core-server verify-objects
echo $?
```

`verify-snapshot` compara o **registo** dos objectos; `verify-objects` lê cada
um do bucket e recalcula a soma. O primeiro passa num servidor cujo bucket está
vazio. Corra os dois.

Se `verify-objects` disser que o Object Storage não está acessível, **nada foi
verificado** — o que não é a mesma coisa que estar tudo bem.

Um objecto no bucket sem linha na base é um **órfão**: é nomeado e não falha,
porque uma escrita falhada deixa um destes. Mas num servidor acabado de
restaurar quer dizer que o destino não estava vazio.

E a terceira pergunta, que as outras duas deixam passar:

```bash
ocinye-core-server verify-keys
echo $?
```

`mailbox_credentials` viaja no dump íntegra. Sem a chave de selagem chega
ilegível, e os dois comandos anteriores passam. Este recusa.

## 8. Só então, arrancar

```bash
systemctl start ocinye-core ocinye-worker
```

As sessões antigas não valem: quem estava autenticado volta a entrar. Identidade
persiste, autoridade restabelece-se.

Confirme que o correio volta a ligar-se. Se `mailbox_credentials` chegou sem a
chave, as caixas dizem-no — a instituição está intacta e o correio está
ilegível, e é uma avaria da chave, não do restore.

## RPO e RTO

Medido numa máquina local, em duas escalas. São grandezas para orientar, não
compromissos:

| | 166 232 recursos | 29 recursos, 2 objectos |
|---|---|---|
| `pg_dump` | 1,4 s → 19 MB | 0,18 s → 266 KB |
| `pg_restore` numa base limpa | 1,9 s | 0,36 s |
| `snapshot` | ~2 s → 8 MB de manifesto | <1 s → 7,4 KB |
| conjunto cifrado completo | — | 328 KB |

**RTO** — o tempo entre ter as cópias e ter a instituição de pé — é dominado
pelo transporte e pela instalação do servidor novo, não por estes comandos.

**RPO** — o que se perde — é **o tempo desde o último `pg_dump`**. Hoje não há
agendamento nenhum, pelo que o RPO real é *desde o último que alguém correu à
mão*. Isto não é uma estimativa optimista a confirmar: é a situação, e está
escrita para que não passe por resolvida.

## O que este runbook não cobre

- **Agendamento.** Não existe. O `institutional-backup.sh` corre quando alguém
  o corre, e é isso que define o RPO real.
- **Um destino externo configurado.** O script aceita um e **recusa-se a usá-lo
  sem cifra**; nenhuma instalação o tem definido.
- **Rotação da chave de selagem.** `OCINYE_MAIL_KEY` viaja como está. Trocá-la
  exige reselar `mailbox_credentials`, e não há procedimento escrito.
- **3-2-1.** Não existe, e não deve ser declarado.
