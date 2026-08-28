#!/usr/bin/env bash
#
# Podemos confiar nos outros portões?
#
# Os quatro portões arquitecturais verificam o Ocinye. Este verifica-os a eles.
#
# # Porque é que existe
#
# Em 2026-08-25, quatro mecanismos diferentes — escritos todos para observar —
# transformaram «não observei» em «passou»:
#
#   · treze viagens de browser reportaram `ok` sem terem arrancado;
#   · um harness de reversão leu uma falha de compilação como defesa a funcionar;
#   · um contrato de enumeração não apanhou o defeito que o motivou;
#   · um `verify.sh` falhado devolveu `exit 0` através de um pipeline.
#
# A classe tem nome: **falha do observador confundida com sucesso do observado**.
# Fechá-la exige testar os observadores com a mesma desconfiança com que eles
# testam o código.
#
# # O que se prova aqui
#
# Cada propriedade tem uma fixture que a viola de propósito. Se o corredor de
# portões aceitar qualquer uma delas, este script falha — e diz qual.

set -uo pipefail

cd "$(dirname "$0")/.."

CORREDOR="scripts/architecture-gates.sh"
FIXTURES="scripts/harness-fixtures"

falhas=0
provas=0

# Corre o corredor de portões com um portão substituído por uma fixture, e diz
# que veredicto ele deu.
#
# Substituir a tabela por injecção mantém o corredor real em teste: o que se
# exercita é o mecanismo que corre na CI, e não uma cópia sua.
com_fixture() {
    local fixture="$1"
    local temporario
    temporario=$(mktemp)
    sed "s#^Rendered-Value Equivalence|.*#Rendered-Value Equivalence|$FIXTURES/$fixture|Equivalência de valores renderizados:#" \
        "$CORREDOR" > "$temporario"
    chmod +x "$temporario"
    bash "$temporario" 2>&1
    local estado=$?
    rm -f "$temporario"
    return $estado
}

exige() {
    local descricao="$1" fixture="$2"
    provas=$((provas + 1))

    local saida estado
    saida=$(com_fixture "$fixture")
    estado=$?

    if [ "$estado" -eq 0 ]; then
        printf '  %-52s ACEITE ✗\n' "$descricao" >&2
        echo "$saida" | tail -6 | sed 's/^/      /' >&2
        falhas=$((falhas + 1))
    else
        printf '  %-52s recusado\n' "$descricao"
    fi
}

echo "Integridade do sistema de verificação:"
echo

# Um processo que falha não pode passar por dizer a palavra certa.
exige "processo falha mas imprime «PASS»" "diz-pass-e-falha.sh"

# Um processo que muta a árvore versionada falha, mesmo restaurando.
#
# Esta corre numa worktree descartável, e não na árvore de trabalho.
#
# A primeira versão mutava o ficheiro a sério e restaurava-o. Provava a
# detecção, e ao mesmo tempo tocava no ficheiro — o que fazia o guarda de pureza
# do `verify.sh` acusar a própria verificação de ter alterado código versionado.
# Tinha razão: a data mudava.
#
# Uma ferramenta que prova «não se muta o que se observa» não pode fazê-lo
# mutando o que observa. A worktree dá um sítio onde a mutação é real e as
# consequências não são.
provas=$((provas + 1))
worktree=$(mktemp -d)
rm -rf "$worktree"
if git worktree add --detach --quiet "$worktree" HEAD 2>/dev/null; then
    copia=$(mktemp)
    sed "s#^Rendered-Value Equivalence|.*#Rendered-Value Equivalence|$FIXTURES/muta-a-arvore.sh|Equivalência de valores renderizados:#" \
        "$CORREDOR" > "$copia"
    chmod +x "$copia"
    if (cd "$worktree" && bash "$copia" >/dev/null 2>&1); then
        printf '  %-52s ACEITE ✗\n' "verificador altera código versionado" >&2
        falhas=$((falhas + 1))
    else
        printf '  %-52s recusado\n' "verificador altera código versionado"
    fi
    rm -f "$copia"
    git worktree remove --force "$worktree" 2>/dev/null
else
    printf '  %-52s INVALID (worktree)\n' "verificador altera código versionado" >&2
    falhas=$((falhas + 1))
fi

# Terminar bem sem produzir prova nenhuma é INVALID, e não PASS.
exige "processo passa mas não produz prova" "nao-diz-nada-e-passa.sh"

# O alvo a observar não existe, e o processo termina bem.
exige "alvo esperado não existe" "alvo-nao-existe.sh"

# O alvo existe e nenhuma propriedade foi observada.
exige "zero propriedades observadas" "zero-observacoes.sh"

# Um portão que não corre não é um portão que passou.
provas=$((provas + 1))
temporario=$(mktemp)
grep -v '^Rendered-Value Equivalence|' "$CORREDOR" > "$temporario"
chmod +x "$temporario"
saida=$(bash "$temporario" 2>&1)
estado=$?
rm -f "$temporario"
if [ "$estado" -eq 0 ]; then
    printf '  %-52s ACEITE ✗\n' "portão em falta conta como passado" >&2
    falhas=$((falhas + 1))
else
    printf '  %-52s recusado\n' "portão em falta conta como passado"
fi

# E o controlo positivo: com os portões verdadeiros, o corredor passa. Sem isto,
# tudo acima podia estar a recusar por o corredor recusar sempre.
provas=$((provas + 1))
if bash "$CORREDOR" >/dev/null 2>&1; then
    printf '  %-52s aceite\n' "portões verdadeiros, todos verdes"
else
    printf '  %-52s RECUSADO ✗\n' "portões verdadeiros, todos verdes" >&2
    falhas=$((falhas + 1))
fi

echo
printf '  %d propriedades exercitadas\n' "$provas"

if [ "$falhas" -gt 0 ]; then
    echo >&2
    echo "$falhas propriedade(s) do sistema de verificação não se sustentam." >&2
    echo "Enquanto isto for verdade, os outros portões não são evidência." >&2
    exit 1
fi
