#!/usr/bin/env bash
# Emite — ou verifica — a matriz de operações e exposição agentic.
#
# A matriz não é escrita à mão. É emitida pelo catálogo tipado em
# `crates/ocinye-core/src/operations.rs`, que é a única fonte das contagens.
# Já aconteceu neste projecto ficarem três contagens diferentes em circulação
# — documento, relatório e código — porque cada uma era mantida à parte. Este
# script existe para que isso deixe de ser possível.
#
#   ./scripts/operation-matrix.sh           reescreve o ficheiro versionado
#   ./scripts/operation-matrix.sh --check    falha se o versionado estiver velho
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

destino="docs/agentic/operation-capability-matrix.md"

gerar() {
    cargo test --quiet -p ocinye-core --lib despeja_a_matriz -- --ignored --nocapture 2>/dev/null \
        | awk '/^# Matriz de operações/ {dentro = 1}
               /^test operations::/    {dentro = 0}
               /^test result:/         {dentro = 0}
               dentro'
}

emitido="$(gerar)"

if [[ -z "$emitido" ]]; then
    echo "ERRO: o catálogo não emitiu matriz nenhuma." >&2
    echo "      Correr à mão para ver porquê:" >&2
    echo "      cargo test -p ocinye-core --lib despeja_a_matriz -- --ignored --nocapture" >&2
    exit 1
fi

if [[ "${1:-}" == "--check" ]]; then
    if ! diff -u "$destino" <(printf '%s\n' "$emitido"); then
        echo >&2
        echo "ERRO: $destino não corresponde ao catálogo tipado." >&2
        echo "      O catálogo mudou e a matriz ficou para trás." >&2
        echo "      Regenerar com: ./scripts/operation-matrix.sh" >&2
        exit 1
    fi
    echo "a matriz corresponde ao catálogo"
    exit 0
fi

printf '%s\n' "$emitido" > "$destino"
echo "escrito: $destino"
