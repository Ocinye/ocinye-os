#!/usr/bin/env bash
#
# Guardas de última linha para vulnerabilidades que este repositório já viveu.
#
# Isto não é um scanner e não tenta ser. Um scanner responde "o que há de novo?"
# e vem de fora — `cargo audit` para o RustSec, `advisory_gate.py` para a base do
# GitHub. Esta lista responde a outra pergunta, muito mais estreita:
#
#     alguma coisa nos trouxe de volta a uma versão que já nos mordeu?
#
# Cada entrada corresponde a um incidente real, e por isso a lista cresce devagar
# e por decisão, nunca por varrimento. Uma entrada sai quando a versão vulnerável
# deixar de ser alcançável pela resolução do Cargo — o que exige justificação,
# não conveniência.
#
# Vive aqui, e não só no Dependabot, porque um `Cargo.lock` pode regredir num
# merge antes de qualquer serviço externo ter oportunidade de reparar.

set -euo pipefail

cd "$(dirname "$0")/.."

falhas=0

# Uso: guarda <crate> <versão mínima segura> <referência>
guarda() {
    local crate="$1" minima="$2" referencia="$3"
    local encontradas

    encontradas=$(awk -v c="$crate" '
        /^\[\[package\]\]/ { nome = "" }
        /^name = / { gsub(/^name = "|"$/, ""); nome = $0 }
        /^version = / && nome == c { gsub(/^version = "|"$/, ""); print }
    ' Cargo.lock)

    if [ -z "$encontradas" ]; then
        printf '  %-16s ausente da árvore\n' "$crate"
        return
    fi

    local versao
    for versao in $encontradas; do
        if [ "$(printf '%s\n%s\n' "$minima" "$versao" | sort -V | head -1)" = "$minima" ]; then
            printf '  %-16s %-10s >= %s\n' "$crate" "$versao" "$minima"
        else
            printf '  %-16s %-10s ABAIXO de %s — %s\n' \
                "$crate" "$versao" "$minima" "$referencia" >&2
            falhas=$((falhas + 1))
        fi
    done
}

echo "Versões vulneráveis conhecidas:"

# GHSA-h395-gr6q-cpjc / CVE-2026-25537 — confusão de tipos na validação de
# claims standard. Um `exp` ou `nbf` com tipo JSON errado podia ser tratado como
# ausente, e uma validação activa deixava de ser aplicada. Dependência directa de
# runtime do `ocinye-core`, compilada no `core-server` e no `worker`.
guarda jsonwebtoken 10.3.0 "GHSA-h395-gr6q-cpjc"

if [ "$falhas" -gt 0 ]; then
    echo >&2
    echo "$falhas versão(ões) vulnerável(eis) conhecida(s) voltaram à árvore." >&2
    exit 1
fi
