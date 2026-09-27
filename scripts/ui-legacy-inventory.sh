#!/usr/bin/env bash
# Inventário da camada visual legada do Workspace (UI Reset, fatia 0).
#
# Só lê. Os números do inventário e do relatório final saem daqui, e não se
# escrevem à mão: o que mede é o que falta retirar, e o que prende o
# comportamento e os testes à apresentação.
set -euo pipefail
cd "$(dirname "$0")/.."

UI=apps/workspace/src/ui
STATIC=apps/workspace/static
TESTS=apps/workspace/tests

linhas() { cat "$@" 2>/dev/null | wc -l | tr -d ' '; }
conta() { grep -rhoE "$1" "${@:2}" 2>/dev/null | wc -l | tr -d ' '; }
distintos() { grep -rhoE "$1" "${@:2}" 2>/dev/null | sort -u | wc -l | tr -d ' '; }

RS=$(find "$UI" -name '*.rs')
printf '%-44s %s\n' "ficheiros Rust de UI" "$(echo "$RS" | wc -l | tr -d ' ')"
printf '%-44s %s\n' "linhas Rust de UI" "$(linhas $RS)"
printf '%-44s %s\n' "linhas de CSS legado" "$(linhas "$STATIC"/*.css)"
printf '%-44s %s\n' "linhas de app.js" "$(linhas "$STATIC/app.js")"
printf '%-44s %s\n' "símbolos no sprite de ícones" "$(conta '<symbol ' "$STATIC/icons.svg")"
printf '%-44s %s\n' "atributos class= na UI Rust" "$(conta 'class=' $RS)"
printf '%-44s %s\n' "classes oc- distintas na UI Rust" "$(distintos '"oc-[a-z0-9_-]+' $RS)"
printf '%-44s %s\n' "marcadores data-oc na UI Rust" "$(conta 'data-oc' $RS)"
printf '%-44s %s\n' "selectores .oc- no app.js" "$(distintos '\.oc-[a-z0-9_-]+' "$STATIC/app.js")"
printf '%-44s %s\n' "classes oc-/is- alternadas no app.js" "$(distintos "'(oc|is)-[a-z0-9_-]+'" "$STATIC/app.js")"
printf '%-44s %s\n' "selectores .oc- nos testes de browser" "$(distintos '\.oc-[a-z0-9_-]+' "$TESTS")"
