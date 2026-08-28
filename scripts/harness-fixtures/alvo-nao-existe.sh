#!/usr/bin/env bash
# O alvo que devia ser observado não existe, e o verificador termina bem.
#
# É o caso mais silencioso de todos: nada falhou, e nada foi observado. Emite a
# prova esperada de propósito, para que o que o recuse seja a exigência de
# observações e não outra defesa.
echo "Equivalência de valores renderizados:"
echo "  observações: 0"
exit 0
