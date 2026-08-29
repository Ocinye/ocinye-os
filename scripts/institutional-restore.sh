#!/usr/bin/env bash
#
# Restauro de um conjunto de continuidade do Ocinye OS.
#
# > **Restaurar não é criar o domínio outra vez.**
#
# Uma instalação nova com as mesmas migrations tem as mesmas tabelas e nada em
# comum com a instituição. Este script traz a instituição; o que prova que ela
# chegou é o `institutional-verify.sh`, que corre depois e é uma coisa separada.
#
# # O que este script recusa fazer
#
# Restaurar por cima de uma base que já tem estado institucional. Um restore
# sobre uma instalação viva mistura duas instituições, e a mistura não se
# desfaz. A base tem de estar vazia, e é o script que o confirma — não quem o
# corre.
#
# Também não corre migrations antes. Correr `sqlx migrate run` numa base vazia
# «para preparar» cria o domínio de novo, que é exactamente o que o verificador
# existe para recusar; e o dump traz o seu próprio esquema.
#
# # Configuração
#
#     OCINYE_DATABASE_URL       a base de destino, que tem de estar vazia
#     OCINYE_RESTORE_IDENTITY   chave privada `age`, quando o conjunto vier
#                               cifrado
#     OCINYE_OBJECT_RESTORE_CMD comando que devolve os objectos ao bucket. A
#                               pasta de origem chega-lhe em
#                               `$OCINYE_OBJECT_DIR`. Exemplo:
#                                 mc mirror "$OCINYE_OBJECT_DIR" local/bucket
#                               Sem ele os bytes não são repostos, e o script
#                               di-lo
#
set -euo pipefail

fatal() { printf '\n  %s\n\n' "$*" >&2; exit 1; }
passo() { printf '  %-38s' "$1"; }
feito() { printf 'ok  %s\n' "${1:-}"; }

CONJUNTO="${1:-}"
[ -n "$CONJUNTO" ] || fatal "Uso: institutional-restore.sh <conjunto>"
[ -e "$CONJUNTO" ] || fatal "não encontrei «$CONJUNTO»."
[ -n "${OCINYE_DATABASE_URL:-}" ] || fatal "OCINYE_DATABASE_URL não está definida."

echo
echo "Ocinye OS — restauro de continuidade"
echo "────────────────────────────────────"
echo

# ── 1. Abrir, se vier fechado ───────────────────────────────────────────
TEMP=""
ABERTO=""
if [ -f "$CONJUNTO" ] && case "$CONJUNTO" in *.age) true;; *) false;; esac; then
    [ -n "${OCINYE_RESTORE_IDENTITY:-}" ] \
      || fatal "o conjunto está cifrado e OCINYE_RESTORE_IDENTITY não está definida.
  A chave que o abre viaja por um canal próprio. Sem ela isto é uma cópia
  perfeitamente íntegra e completamente inútil."
    command -v age >/dev/null || fatal "o \`age\` não está instalado."
    passo "abrir"
    # Ao lado do conjunto, e não num temporário aleatório: o manifesto é
    # preciso **depois** deste script, para a verificação, e um caminho que já
    # não existe é uma instrução que não se pode seguir.
    TEMP="$(dirname "$CONJUNTO")/$(basename "$CONJUNTO" .tar.age).aberto"
    rm -rf "$TEMP"; mkdir -p "$TEMP"
    age -d -i "$OCINYE_RESTORE_IDENTITY" "$CONJUNTO" | tar -xf - -C "$TEMP" \
      || fatal "não foi possível abrir o conjunto."
    chmod 700 "$TEMP"
    CONJUNTO="$TEMP/$(ls -1 "$TEMP" | head -1)"
    ABERTO="$TEMP"
    feito
fi
[ -d "$CONJUNTO" ] || fatal "«$CONJUNTO» não é uma pasta de conjunto."
[ -e "$CONJUNTO/INCOMPLETO" ] && fatal "este conjunto está marcado INCOMPLETO.
  É o que restou de uma cópia que não terminou, e não uma cópia."

# ── 2. Confirmar que chegou inteiro, antes de tocar na base ─────────────
passo "somas do conjunto"
[ -f "$CONJUNTO/SHA256SUMS" ] || fatal "o conjunto não traz SHA256SUMS."
( cd "$CONJUNTO" && shasum -a 256 -c SHA256SUMS >/dev/null ) \
  || fatal "as somas não conferem. O conjunto não chegou inteiro, e restaurar
  a partir dele escreveria estado corrompido por cima de uma base vazia."
feito "$(wc -l < "$CONJUNTO/SHA256SUMS" | tr -d ' ') ficheiro(s)"

# ── 3. A base tem de estar vazia ────────────────────────────────────────
passo "base de destino"
TABELAS=$(psql "$OCINYE_DATABASE_URL" -Atc \
  "SELECT count(*) FROM information_schema.tables WHERE table_schema='public'") \
  || fatal "a base de destino não respondeu."
[ "$TABELAS" = "0" ] || fatal "a base de destino tem $TABELAS tabela(s).
  Recuso restaurar por cima de estado existente: misturar duas instituições
  não se desfaz. Crie uma base vazia."
feito "vazia"

# ── 4. A base ───────────────────────────────────────────────────────────
passo "restaurar a base"
pg_restore --no-owner --no-privileges --dbname "$OCINYE_DATABASE_URL" \
  "$CONJUNTO/postgres.dump" || fatal "o pg_restore falhou."
feito

# ── 5. Os bytes ─────────────────────────────────────────────────────────
if [ -n "${OCINYE_OBJECT_RESTORE_CMD:-}" ] && [ -d "$CONJUNTO/objects" ]; then
    passo "repor os objectos"
    OCINYE_OBJECT_DIR="$CONJUNTO/objects" \
      eval "$OCINYE_OBJECT_RESTORE_CMD" >/dev/null \
      || fatal "a reposição dos objectos falhou."
    feito "$(find "$CONJUNTO/objects" -type f | wc -l | tr -d ' ') ficheiro(s)"
else
    echo "  objectos                               NÃO REPOSTOS"
    echo "      Metade do estado autoritativo não foi reposta. O"
    echo "      \`verify-snapshot\` vai passar mesmo assim: ele compara o"
    echo "      registo dos objectos, não os bytes."
fi

echo
echo "  Base restaurada. Isto ainda não é um restore validado."
echo
echo "  Corra agora, e leia o código de saída de cada um:"
echo "    ocinye-core-server verify-snapshot < $CONJUNTO/manifesto.json"
echo "    ocinye-core-server verify-objects"
echo "    ocinye-core-server verify-keys"
echo
echo "  Ou os três de uma vez:  ./scripts/institutional-verify.sh $CONJUNTO"
echo
if [ -n "$ABERTO" ]; then
    echo "  O conjunto ficou aberto em:"
    echo "    $ABERTO"
    echo "  São dados institucionais decifrados. Apague-os depois de verificar:"
    echo "    rm -rf \"$ABERTO\""
    echo
fi
