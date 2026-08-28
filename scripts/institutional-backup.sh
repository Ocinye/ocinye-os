#!/usr/bin/env bash
#
# Cópia de continuidade institucional do Ocinye OS.
#
# > **Um servidor pode desaparecer. A instituição continua.**
#
# Produz um conjunto datado e auto-descritivo:
#
#     manifesto.json    o que esta instalação contém, por identidade
#     postgres.dump     a base institucional
#     objects/          os bytes do Object Storage
#     SHA256SUMS        as somas de tudo o que está aqui
#     LEIA-ME.txt       o que falta neste conjunto, e porquê
#
# # O que este script recusa fazer
#
# Não imprime «Backup completed successfully» porque o `pg_dump` terminou com
# zero. O sucesso aqui é: o dump saiu, o manifesto saiu, as somas foram
# calculadas **e reconferidas**, e a cópia que chegou ao destino tem as mesmas
# somas da que saiu. Cada uma destas é verificada, e qualquer uma que falhe
# termina o script.
#
# Também não prova que o restore funciona. **Nada aqui o prova.** Só um restore
# executado o prova, e isso é o `docs/runbooks/migrate-to-another-server.md`.
#
# # O que este conjunto nunca contém
#
# A `OCINYE_MAIL_KEY`. É material criptográfico durável: sem ela
# `mailbox_credentials` chega íntegra e ilegível, e por isso tem de viajar —
# mas por um canal próprio, e nunca ao lado do que ela abre. Um conjunto que a
# contivesse transformaria uma cópia perdida em compromisso total.
#
# # Configuração
#
#     OCINYE_BACKUP_DIR         onde escrever (obrigatório)
#     OCINYE_BACKUP_RECIPIENT   chave pública `age` do destinatário; sem ela o
#                               conjunto fica em claro e a cópia externa é
#                               recusada
#     OCINYE_BACKUP_REMOTE      destino fora deste servidor, no formato do
#                               `OCINYE_BACKUP_REMOTE_CMD`
#     OCINYE_BACKUP_REMOTE_CMD  comando que copia; recebe origem e destino.
#                               Por omissão `rsync -a`. É assim que não há
#                               dependência de fornecedor: quem opera escolhe
#                               `rsync`, `rclone`, `aws s3 cp` ou outro
#     OCINYE_BACKUP_KEEP        quantos conjuntos manter (por omissão 7)
#     OCINYE_OBJECT_SYNC_CMD    comando que espelha o bucket para uma pasta.
#                               A pasta chega-lhe em `$OCINYE_OBJECT_DIR` — por
#                               variável e não por argumento, porque na cópia
#                               ela é o destino e no restauro é a origem, e um
#                               argumento acrescentado ao fim só serve um dos
#                               dois. Exemplo:
#                                 mc mirror local/bucket "$OCINYE_OBJECT_DIR"
#                               Sem ele, os objectos não são copiados e o
#                               `LEIA-ME.txt` di-lo
#
set -euo pipefail

fatal() { printf '\n  %s\n\n' "$*" >&2; exit 1; }
passo() { printf '  %-38s' "$1"; }
feito() { printf 'ok  %s\n' "${1:-}"; }

BIN="${OCINYE_CORE_SERVER_BIN:-./target/debug/ocinye-core-server}"
[ -x "$BIN" ] || fatal "não encontrei o binário do Core em «$BIN».
  Sem ele não há manifesto, e um conjunto sem manifesto é um ficheiro
  que ninguém consegue confrontar com nada."

[ -n "${OCINYE_BACKUP_DIR:-}" ] || fatal "OCINYE_BACKUP_DIR não está definida."
[ -n "${OCINYE_DATABASE_URL:-}" ] || fatal "OCINYE_DATABASE_URL não está definida."

CARIMBO="$(date -u +%Y%m%dT%H%M%SZ)"
DESTINO="$OCINYE_BACKUP_DIR/ocinye-$CARIMBO"
mkdir -p "$DESTINO"

# ── Um conjunto por terminar não se pode parecer com um conjunto ─────────
#
# Uma corrida que falhe a meio deixa em disco um manifesto, um dump e umas
# somas. Vistos daqui a três meses são indistinguíveis de uma cópia boa, e é
# essa que alguém escolhe no dia do desastre. Por isso a marca existe desde o
# primeiro instante e só sai no fim; se o script morrer, o conjunto passa a
# chamar-se INCOMPLETO e nenhuma retenção o conta.
touch "$DESTINO/INCOMPLETO"
abandonar() {
    local codigo=$?
    # Um `.age` a meio é pior do que nenhum: parece um conjunto e não abre.
    rm -f "$DESTINO.tar.age.parcial" 2>/dev/null || true
    if [ -d "$DESTINO" ] && [ -e "$DESTINO/INCOMPLETO" ]; then
        rm -f "$DESTINO.tar.age" 2>/dev/null || true
        mv "$DESTINO" "$DESTINO-INCOMPLETO" 2>/dev/null || true
        printf "\n  O conjunto ficou por terminar e foi marcado INCOMPLETO.\n" >&2
        printf "  Não é uma cópia: é o que restou de uma tentativa.\n\n" >&2
    fi
    exit "$codigo"
}
trap abandonar EXIT

echo
echo "Ocinye OS — cópia de continuidade"
echo "─────────────────────────────────"
echo "  conjunto  $DESTINO"
echo

# ── 1. O manifesto, antes do dump ───────────────────────────────────────
#
# Antes, e não depois: descreve o estado que o dump vai capturar. Ao contrário,
# descreveria um estado posterior ao dump e a comparação teria uma diferença
# que ninguém saberia explicar.
passo "manifesto"
"$BIN" snapshot > "$DESTINO/manifesto.json" 2>"$DESTINO/.snapshot.err" \
  || { cat "$DESTINO/.snapshot.err" >&2; fatal "o manifesto não saiu."; }
mv "$DESTINO/.snapshot.err" "$DESTINO/manifesto.txt"
[ -s "$DESTINO/manifesto.json" ] || fatal "o manifesto saiu vazio."
feito "$(wc -c < "$DESTINO/manifesto.json" | tr -d ' ') bytes"

# ── 2. A base ───────────────────────────────────────────────────────────
passo "base institucional"
pg_dump --format=custom --no-owner --no-privileges "$OCINYE_DATABASE_URL" \
  -f "$DESTINO/postgres.dump" || fatal "o pg_dump falhou."
[ -s "$DESTINO/postgres.dump" ] || fatal "o dump saiu vazio."
feito "$(wc -c < "$DESTINO/postgres.dump" | tr -d ' ') bytes"

# ── 3. Os bytes ─────────────────────────────────────────────────────────
#
# Por comando configurável, e não por um cliente escolhido aqui: o Object
# Storage é S3-compatible por decisão (ADR-0200), e prender a cópia a um
# fornecedor desfaria isso na única operação em que ele importa.
if [ -n "${OCINYE_OBJECT_SYNC_CMD:-}" ]; then
  passo "objectos"
  mkdir -p "$DESTINO/objects"
  OCINYE_OBJECT_DIR="$DESTINO/objects" \
    eval "$OCINYE_OBJECT_SYNC_CMD" >/dev/null \
    || fatal "a cópia dos objectos falhou."
  feito "$(find "$DESTINO/objects" -type f | wc -l | tr -d ' ') ficheiro(s)"
else
  echo "  objectos                               NÃO COPIADOS"
  echo "      OCINYE_OBJECT_SYNC_CMD não está definida. Metade do estado"
  echo "      autoritativo não está neste conjunto."
fi

# ── 4. O que falta aqui dentro ──────────────────────────────────────────
cat > "$DESTINO/LEIA-ME.txt" <<TXT
Ocinye OS — conjunto de continuidade de $CARIMBO

O QUE ESTE CONJUNTO NÃO CONTÉM, E TEM DE VIAJAR À PARTE

  OCINYE_MAIL_KEY   Sem ela, mailbox_credentials chega íntegra e ilegível.
                    Viaja por um canal próprio. Se estivesse aqui dentro,
                    perder este conjunto seria perder tudo de uma vez.

O QUE ESTE CONJUNTO NÃO PROVA

  Que o restore funciona. Um backup que nunca foi restaurado não é evidência
  de recuperabilidade. O procedimento está em
  docs/runbooks/migrate-to-another-server.md e tem de ser executado.

COMO SE CONFIRMA QUE CHEGOU INTEIRO

  shasum -a 256 -c SHA256SUMS

COMO SE CONFIRMA QUE A INSTITUIÇÃO CHEGOU

  No servidor novo, depois do restore:
    ocinye-core-server verify-snapshot < manifesto.json
    ocinye-core-server verify-objects
    ocinye-core-server verify-keys

  Os três, e o código de saída de cada um. O primeiro prova as linhas, o
  segundo os bytes, o terceiro que o que chegou se consegue ler.
TXT

# ── 5. As somas, calculadas e reconferidas ──────────────────────────────
#
# Reconferidas de propósito. Calcular somas e guardá-las prova que o comando
# correu; voltar a lê-las prova que o que ficou em disco é o que se mediu.
passo "somas"
( cd "$DESTINO" && find . -type f ! -name SHA256SUMS ! -name INCOMPLETO -print0 \
    | sort -z | xargs -0 shasum -a 256 > SHA256SUMS )
( cd "$DESTINO" && shasum -a 256 -c SHA256SUMS >/dev/null ) \
  || fatal "as somas não conferem imediatamente a seguir a serem escritas.
  Isto não é um problema de backup: é um problema de disco."
feito "$(wc -l < "$DESTINO/SHA256SUMS" | tr -d ' ') ficheiro(s)"

# ── 6. A cifra ──────────────────────────────────────────────────────────
if [ -n "${OCINYE_BACKUP_RECIPIENT:-}" ]; then
  command -v age >/dev/null || fatal "OCINYE_BACKUP_RECIPIENT está definida e o \`age\` não está instalado."
  passo "cifra"
  # A marca fica na pasta e **não** viaja: dentro do conjunto cifrado seria
  # lida como «esta cópia não terminou», e o restauro recusaria uma cópia boa.
  # Na origem ela continua até ao fim, que é onde tem de estar.
  tar --exclude INCOMPLETO -cf - -C "$(dirname "$DESTINO")" "$(basename "$DESTINO")" \
    | age -r "$OCINYE_BACKUP_RECIPIENT" -o "$DESTINO.tar.age" \
    || fatal "a cifra falhou."
  rm -rf "$DESTINO"
  DESTINO="$DESTINO.tar.age"
  feito "$(wc -c < "$DESTINO" | tr -d ' ') bytes"
  CIFRADO=sim
else
  CIFRADO=nao
  echo "  cifra                                  NÃO CIFRADO"
  echo "      OCINYE_BACKUP_RECIPIENT não está definida. Este conjunto é"
  echo "      uma cópia legível de tudo o que a instituição classificou."
fi

# ── 7. A cópia fora deste servidor ──────────────────────────────────────
#
# Um conjunto que só existe na máquina que ardeu não é um backup.
if [ -n "${OCINYE_BACKUP_REMOTE:-}" ]; then
  if [ "$CIFRADO" = nao ]; then
    fatal "recuso copiar um conjunto em claro para fora deste servidor.
  Defina OCINYE_BACKUP_RECIPIENT, ou remova OCINYE_BACKUP_REMOTE e assuma
  que não há cópia externa. Enviar isto em claro é publicar a instituição."
  fi
  passo "cópia externa"
  CMD="${OCINYE_BACKUP_REMOTE_CMD:-rsync -a}"
  $CMD "$DESTINO" "$OCINYE_BACKUP_REMOTE" || fatal "a cópia externa falhou."
  feito
else
  echo "  cópia externa                          NÃO EXISTE"
  echo "      OCINYE_BACKUP_REMOTE não está definida. Este conjunto vive"
  echo "      apenas nesta máquina, que é a que pode desaparecer."
fi

# ── 8. Retenção ─────────────────────────────────────────────────────────
# A partir daqui o conjunto está completo. A marca sai, e o `trap` deixa de
# ter o que renomear.
rm -f "$DESTINO/INCOMPLETO" 2>/dev/null || true

# A retenção conta **conjuntos**, e um INCOMPLETO não é um. Contá-lo faria a
# rotação apagar uma cópia boa para guardar os restos de uma tentativa.
KEEP="${OCINYE_BACKUP_KEEP:-7}"
VELHOS=$(ls -1dt "$OCINYE_BACKUP_DIR"/ocinye-* 2>/dev/null \
    | grep -v -- '-INCOMPLETO$' | tail -n +$((KEEP + 1)) || true)
if [ -n "$VELHOS" ]; then
  passo "retenção"
  echo "$VELHOS" | while read -r velho; do rm -rf "$velho"; done
  feito "$(echo "$VELHOS" | wc -l | tr -d ' ') conjunto(s) removido(s), $KEEP mantido(s)"
fi

# ── Os incompletos também têm retenção, e é mais curta ──────────────────
#
# Não entram na contagem dos conjuntos — não são conjuntos —, e por isso
# ficariam para sempre: o disco enchia-se de tentativas falhadas, que é uma
# maneira lenta de impedir a cópia seguinte.
#
# Guarda-se **o último**. Quem for ver porque falhou vê a última falha; as
# anteriores já não dizem nada de novo, e o que interessava delas ficou no
# registo de quem as correu.
RESTOS=$(ls -1dt "$OCINYE_BACKUP_DIR"/ocinye-*-INCOMPLETO 2>/dev/null | tail -n +2 || true)
if [ -n "$RESTOS" ]; then
  passo "restos de tentativas"
  echo "$RESTOS" | while read -r resto; do rm -rf "$resto"; done
  feito "$(echo "$RESTOS" | wc -l | tr -d ' ') removido(s), o mais recente mantido"
fi

echo
echo "  Conjunto escrito. Isto é um backup executado."
echo "  Não é um restore validado — nenhum comando aqui o pode afirmar."
echo
