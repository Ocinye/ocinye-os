#!/usr/bin/env bash
# Capturas do Workspace, para revisão visual humana.
#
# # O que isto é, e o que não é
#
# É a ferramenta do portão de qualidade: levanta a stack a sério, prepara um
# cenário determinado, navega com um browser verdadeiro e grava PNGs para
# alguém olhar.
#
# Não é um teste. Não compara píxeis, não guarda imagens de referência e não
# entra no `verify.sh`. Congelar píxeis transformaria cada ajuste deliberado de
# desenho numa falha, e a suite passaria a defender o passado em vez de
# defender uma propriedade.
#
#     as viagens de browser provam que funciona
#     estas capturas mostram se está premium
#
# # Onde ficam
#
# Fora da árvore versionada, por omissão em `/tmp/ocinye-capturas`. A
# verificação recusa-se a aprovar um repositório que ela própria alterou, e uma
# ferramenta que despeja imagens dentro dele estaria a trabalhar contra isso.
set -euo pipefail

cd "$(dirname "$0")/.."

: "${OCINYE_TEST_CAPTURAS_DIR:=/tmp/ocinye-capturas}"
export OCINYE_TEST_CAPTURAS_DIR

if [[ -z "${OCINYE_TEST_DATABASE_URL:-}" ]]; then
    echo "ERRO: OCINYE_TEST_DATABASE_URL não está definida." >&2
    echo "      As capturas levantam o sistema a sério, e isso precisa de base." >&2
    exit 1
fi

alvo="${1:-capturas_}"

rm -rf "${OCINYE_TEST_CAPTURAS_DIR:?}"
mkdir -p "$OCINYE_TEST_CAPTURAS_DIR"

echo "A capturar para $OCINYE_TEST_CAPTURAS_DIR"
cargo test -q -p ocinye-workspace --test browser "$alvo" -- --ignored --nocapture

produzidas=$(find "$OCINYE_TEST_CAPTURAS_DIR" -name '*.png' | wc -l | tr -d ' ')

# Uma execução que não produz nada é uma execução falhada, mesmo que o `cargo`
# devolva zero: um teste que se salta reporta `ok` e não grava ficheiro nenhum.
if [[ "$produzidas" -eq 0 ]]; then
    echo "ERRO: nenhuma captura foi produzida." >&2
    echo "      Um alvo que se salta reporta ok — isto não é sucesso." >&2
    exit 1
fi

echo
echo "$produzidas captura(s):"
find "$OCINYE_TEST_CAPTURAS_DIR" -name '*.png' | sort | while read -r f; do
    printf '  %s  (%s)\n' "$(basename "$f")" "$(du -h "$f" | cut -f1)"
done

# Perfis do Chrome que o harness cria. O `Drop` fecha-os, mas uma execução
# interrompida não chega lá — e dois mil perfis abandonados já custaram 442 MB.
find "${TMPDIR:-/tmp}" -maxdepth 1 -name 'ocinye-e2e-*' -type d -mmin +5 -exec rm -rf {} + 2>/dev/null || true
