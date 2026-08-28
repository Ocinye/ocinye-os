#!/usr/bin/env bash
# Altera um ficheiro versionado, restaura-o, e termina bem com a prova certa.
#
# Corre numa worktree descartável, nunca na árvore de trabalho.
#
# Foi assim que a tokenização foi revertida: o ficheiro voltou ao sítio, mas
# entre uma coisa e outra houve um commit. Emite a prova esperada de propósito,
# para que o que a recuse seja a integridade da árvore e mais nada.
set -e
cd "$(git rev-parse --show-toplevel)"
alvo="apps/workspace/static/ocinye.css"
guardado=$(mktemp)
cp "$alvo" "$guardado"
printf '\n/* passei por aqui */\n' >> "$alvo"
cp "$guardado" "$alvo"
rm -f "$guardado"
echo "Equivalência de valores renderizados:"
exit 0
