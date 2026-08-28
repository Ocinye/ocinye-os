#!/usr/bin/env bash
#
# Os portões da fronteira Core / Experience / Design System.
#
# # Quatro estados, e só um é verde
#
#     PASS      a propriedade foi observada e está satisfeita
#     FAIL      a propriedade foi observada e está violada
#     INVALID   o verificador, o build ou a fixture falhou
#     NOT_RUN   a verificação não chegou a correr
#
# `INVALID` não pode virar `PASS` por ter aparecido uma linha bonita no stdout.
# `NOT_RUN` não pode virar `PASS` por o processo principal ter terminado.
#
# Isto não é zelo abstracto. Em 2026-08-25 uma verificação foi corrida dentro de
# `verify.sh 2>&1 | tee log | grep …`, e num pipeline o estado que volta é o do
# último comando: o `grep` correu bem, e uma execução falhada foi registada como
# sucesso. Um commit seguiu-se, e levou consigo a reversão acidental de metade
# do trabalho que aquela verificação teria apanhado.
#
# # Executar e analisar são operações separadas
#
# O estado de saída do processo que detém a propriedade é a autoridade sobre a
# execução. A saída de texto só é lida **depois**, e nunca pode contradizê-lo.
#
# # Um verificador não muta o que observa
#
# Uma ferramenta de observação não deve precisar de restaurar o objecto
# observado. Se um portão alterar ficheiros versionados — mesmo que os restaure
# a seguir — isto falha, e diz porquê.

set -uo pipefail

# A raiz vem do Git, e não de `dirname $0`.
#
# Parece detalhe e não é: o portão de integridade corre cópias deste script a
# partir de um directório temporário, e com `dirname $0` cada cópia mudava para
# `/var/folders/…`. Todos os portões falhavam por não encontrarem os ficheiros,
# e o portão de integridade lia isso como «recusou a fixture» — passando por
# ter observado uma coisa que nunca chegou a acontecer.
#
# Era a própria classe que ele existe para fechar, dentro dele.
raiz=$(git rev-parse --show-toplevel 2>/dev/null) || {
    echo "INVALID: isto tem de correr dentro do repositório." >&2
    exit 2
}
cd "$raiz"

# nome|comando|prova
#
# A `prova` é uma marca que a saída do portão tem de conter quando ele passa.
# Sem isto, um portão que termine bem sem ter feito nada — porque a suite ficou
# vazia, porque um filtro não encontrou nada, porque o binário arrancou e saiu —
# seria contado como propriedade satisfeita. Saída zero sem evidência é a outra
# metade da mesma classe: silêncio lido como confirmação.
#
# As contagens são deliberadas, e mudam **por decisão**. Uma suite que encolheu
# sozinha tem de dar vermelho; por isso acrescentar um teste obriga a vir aqui,
# e isso é a funcionalidade e não o atrito.
#
# Contagens actualizadas em 2026-08-27:
#
#   Experience Structural Boundary  6 → 7
#     `nenhuma_vista_decide_um_dia_civil_em_greenwich`, do plano de tempo real
#     (24a0b96). O portão esteve INVALID desde esse commit — não FAIL: a
#     propriedade continuou satisfeita, e o que faltou foi a prova bater certo.
#     Ninguém o viu porque `architecture-gates.sh` não voltou a correr.
#
#   Design System Integrity  23 → 29
#     Mais um em 2026-08-27: `nenhum_ecra_pede_um_nome_de_utilizador`, que mede
#     a superfície e não o repositório — `autocomplete="username"` e o claim
#     `preferred_username` continuam legítimos.
#     Cinco portões novos da mesma sessão: as mensagens próprias do outro lado,
#     tudo o que o arranque chama existe, os painéis igualam o painel da conta,
#     as regras partilhadas fora de uma media query, e a superfície de um painel
#     não se reescreve.
portoes() {
    cat <<'TABELA'
Architecture Dependency Boundary|python3 scripts/architecture_boundaries.py|Fronteiras arquitecturais:
Experience Structural Boundary|cargo test -q -p ocinye-workspace --test experience_boundary|test result: ok. 7 passed
Design System Integrity|cargo test -q -p ocinye-workspace --test design_fidelity|test result: ok. 29 passed
Rendered-Value Equivalence|python3 scripts/rendered_value_equivalence.py|Equivalência de valores renderizados:
TABELA
}

ESPERADOS=4

# A impressão digital da árvore versionada: caminho, tamanho e data de
# alteração. Um ficheiro tocado e restaurado muda a data, e por isso aparece
# aqui mesmo quando o conteúdo volta ao que era.
#
# Em Python, e não em `stat`, por portabilidade: no macOS `stat -f` formata o
# ficheiro e no Linux mostra o **sistema de ficheiros**. A CI descobriu-o da
# maneira mais clara possível — a impressão digital vinha `Blocks: Total: …`, que
# muda entre duas leituras porque o disco se enche, e o guarda acusava toda a
# gente. Falhava para o lado seguro, e falhava na mesma.
impressao_da_arvore() {
    python3 - <<'FIM' | sort
import os, subprocess
saida = subprocess.run(["git", "ls-files"], capture_output=True, text=True, check=True)
for nome in saida.stdout.splitlines():
    try:
        info = os.stat(nome)
    except OSError:
        print("%s AUSENTE" % nome)
        continue
    print("%s %d %d" % (nome, info.st_size, int(info.st_mtime)))
FIM
}

antes_da_arvore=$(impressao_da_arvore)

descobertos=0
executados=0
passaram=0
problemas=()

echo "Fronteiras arquitecturais:"
echo

while IFS='|' read -r nome comando prova; do
    [ -z "$nome" ] && continue
    descobertos=$((descobertos + 1))

    # Executar. O estado de saída é guardado antes de qualquer análise de texto,
    # e a análise nunca o pode contradizer — só agravar.
    saida=$(eval "$comando" 2>&1)
    estado=$?
    executados=$((executados + 1))

    case "$estado" in
        0) veredicto="PASS" ;;
        1) veredicto="FAIL" ;;
        *) veredicto="INVALID" ;;
    esac

    # Terminar bem sem produzir a prova esperada é `INVALID`, e não `PASS`.
    if [ "$veredicto" = "PASS" ] && [ -n "${prova:-}" ]; then
        if ! echo "$saida" | grep -qF "$prova"; then
            veredicto="INVALID"
            saida="$saida
      (o portão terminou bem e não produziu a prova esperada: «${prova}»)"
        fi
    fi

    # Zero observações é `INVALID`, e não `PASS`.
    #
    # É o caso mais silencioso: nada falhou, e nada foi olhado. Um filtro que
    # não bateu, uma suite que ficou vazia, um alvo que deixou de existir — o
    # portão termina bem e não observou propriedade nenhuma. Qualquer contagem
    # que o portão imprima tem de ser maior que zero.
    if [ "$veredicto" = "PASS" ]; then
        if echo "$saida" | grep -qE ':[[:space:]]*0[[:space:]]*$|:[[:space:]]*0$|\b0 passed\b'; then
            veredicto="INVALID"
            saida="$saida
      (o portão terminou bem e declarou zero observações)"
        fi
    fi

    if [ "$veredicto" = "PASS" ]; then
        passaram=$((passaram + 1))
        printf '  %-38s PASS\n' "$nome"
    else
        problemas+=("$nome=$veredicto")
        printf '  %-38s %s (saída %s)\n' "$nome" "$veredicto" "$estado"
        echo "$saida" | sed 's/^/      /'
        echo
    fi
done < <(portoes)

depois_da_arvore=$(impressao_da_arvore)
if [ "$antes_da_arvore" != "$depois_da_arvore" ]; then
    echo >&2
    echo "VERIFICADOR ALTEROU CÓDIGO VERSIONADO" >&2
    echo >&2
    diff <(echo "$antes_da_arvore") <(echo "$depois_da_arvore") \
        | grep '^[<>]' | head -10 | sed 's/^/      /' >&2
    echo >&2
    echo "Uma ferramenta de observação não precisa de restaurar o objecto" >&2
    echo "observado. Restaurar depois não desfaz o problema: durante a" >&2
    echo "execução, os outros verificadores viram outra árvore." >&2
    problemas+=("Tree Integrity=FAIL")
fi

echo
printf '  %d esperados\n  %d descobertos\n  %d executados\n  %d passaram\n  %d não correram\n' \
    "$ESPERADOS" "$descobertos" "$executados" "$passaram" \
    "$((ESPERADOS - executados))"

if [ "$descobertos" -ne "$ESPERADOS" ] || [ "$executados" -ne "$ESPERADOS" ]; then
    echo >&2
    echo "NOT_RUN: esperavam-se $ESPERADOS portões e correram $executados." >&2
    echo "Uma verificação que não corre não é uma verificação que passou." >&2
    exit 2
fi

if [ "${#problemas[@]}" -gt 0 ]; then
    echo >&2
    echo "${#problemas[@]} problema(s): ${problemas[*]}" >&2
    exit 1
fi
