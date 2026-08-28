#!/usr/bin/env bash
# Imprime exactamente a prova que o corredor exige, e falha.
#
# É a armadilha que ocorreu de verdade: uma linha convincente no stdout sobre um
# processo que terminou em erro.
#
# A prova tem de ser a verdadeira, e não uma aproximada. Com uma marca errada, a
# fixture seria recusada pela exigência de prova antes de chegar à propriedade
# que existe para isolar — e o teste passaria sem ter testado o estado de saída.
echo "Equivalência de valores renderizados:"
echo "  tokens introduzidos: 18"
exit 42
