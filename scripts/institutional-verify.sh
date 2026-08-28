#!/usr/bin/env bash
#
# As três perguntas da continuidade institucional, feitas juntas.
#
# | | prova | não prova |
# |---|---|---|
# | `verify-snapshot` | que cada identidade chegou | que existe um byte no bucket |
# | `verify-objects`  | que os bytes batem com as somas | nada, se o bucket não responder |
# | `verify-keys`     | que o que chegou se consegue **ler** | nada, se não houver estado selado |
#
# # Porque os três, e não um
#
# Porque cada um deixa passar exactamente aquilo que o seguinte apanha. Um
# restore com a base intacta e o bucket vazio passa no primeiro. Um restore com
# a base e os bytes e sem a chave de selagem passa nos dois primeiros — e
# entrega à instituição correio que ninguém consegue abrir.
#
# # O protocolo dos quatro estados
#
# Este script distingue-os, e nunca promove um a `PASS`:
#
#     PASS      a propriedade foi observada e está satisfeita
#     FAIL      a propriedade foi observada e está violada
#     INVALID   o verificador não conseguiu observar
#     NOT_RUN   a verificação não chegou a correr
#
# «Nada para verificar» **não é** `PASS`. Um verificador que não encontrou o que
# devia observar não teve sucesso: observou zero.
#
set -uo pipefail

MANIFESTO="${1:-}"
BIN="${OCINYE_CORE_SERVER_BIN:-./target/debug/ocinye-core-server}"
[ -d "$MANIFESTO" ] && MANIFESTO="$MANIFESTO/manifesto.json"

echo
echo "Ocinye OS — verificação de continuidade institucional"
echo "─────────────────────────────────────────────────────"
echo

falhas=0
nao_observado=0

# Executar e analisar são operações separadas: o código de saída do processo
# que detém a propriedade é a autoridade, e o texto só se lê depois. Um
# `comando | grep` devolveria o estado do `grep`.
correr() {
    local nome="$1"; shift
    local saida estado
    saida="$("$@" 2>&1)"; estado=$?
    if [ "$estado" -eq 0 ]; then
        if printf '%s' "$saida" | grep -q "Nada foi verificado\|nenhum estado selado"; then
            printf '  %-18s %s\n' "$nome" "NOT_RUN"
            printf '%s\n' "$saida" | sed -n '$p' | sed 's/^/      /'
            nao_observado=$((nao_observado + 1))
        else
            printf '  %-18s %s\n' "$nome" "PASS"
        fi
    elif printf '%s' "$saida" | grep -q "não está configurado\|respondeu «unreachable»\|respondeu «unresponsive»"; then
        printf '  %-18s %s\n' "$nome" "INVALID"
        printf '%s\n' "$saida" | head -2 | sed 's/^/      /'
        nao_observado=$((nao_observado + 1))
    else
        printf '  %-18s %s\n' "$nome" "FAIL"
        printf '%s\n' "$saida" | tail -3 | sed 's/^/      /'
        falhas=$((falhas + 1))
    fi
}

if [ -z "$MANIFESTO" ] || [ ! -f "$MANIFESTO" ]; then
    printf '  %-18s %s\n' "as linhas" "NOT_RUN"
    echo "      Sem manifesto não há comparação: há uma leitura."
    echo "      Uso: institutional-verify.sh <conjunto|manifesto.json>"
    nao_observado=$((nao_observado + 1))
else
    saida="$("$BIN" verify-snapshot < "$MANIFESTO" 2>&1)"; estado=$?
    if [ "$estado" -eq 0 ]; then
        printf '  %-18s %s\n' "as linhas" "PASS"
        printf '%s\n' "$saida" | grep -E "recursos institucionais" | sed 's/^ */      /'
    else
        printf '  %-18s %s\n' "as linhas" "FAIL"
        printf '%s\n' "$saida" | tail -3 | sed 's/^/      /'
        falhas=$((falhas + 1))
    fi
fi

correr "os bytes"      "$BIN" verify-objects
correr "a legibilidade" "$BIN" verify-keys

echo
if [ "$falhas" -gt 0 ]; then
    echo "  $falhas verificação(ões) falharam. O que chegou não é o que saiu."
    exit 1
fi
if [ "$nao_observado" -gt 0 ]; then
    echo "  Nenhuma verificação falhou, e $nao_observado não chegou a observar nada."
    echo "  Isto NÃO é um restore validado: é um restore parcialmente verificado."
    echo "  Um verificador que não observou não teve sucesso — observou zero."
    exit 2
fi
echo "  As três observaram, e as três passaram."
echo "  A instituição chegou íntegra, completa e legível."
