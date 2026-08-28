#!/usr/bin/env bash
#
# Capacidade de disco do runner, medida e exigida em vez de suposta.
#
# A CI já morreu duas vezes com `No space left on device` a meio de um `rustc`,
# vinte minutos depois do início. Lê-se como avaria do código e não é: é o runner
# a ficar sem espaço. O erro aparece longe da causa, no ficheiro errado, e manda
# quem investiga para o sítio errado.
#
# Três verbos, e a ordem entre eles é a tese:
#
#   medir     diz quanto há, com nome, para que qualquer alteração futura ao
#             consumo seja visível no log em vez de ser descoberta por avaria;
#   libertar  remove apenas o que a imagem do GitHub traz e este projecto não
#             usa, tolerando ausências porque a imagem muda sem aviso;
#   exigir    falha em quinze segundos se o que resta não chega, em vez de falhar
#             em vinte minutos dentro do compilador.
#
# O limiar não é um número escolhido por gosto: vem do consumo medido, e a razão
# está registada onde ele é invocado.

set -euo pipefail

livre_gb() {
    # `OCINYE_CI_DISK_FREE_GB` existe para que o limiar possa ser exercitado sem
    # encher um disco de verdade. Fora dos testes ninguém a define.
    if [ -n "${OCINYE_CI_DISK_FREE_GB:-}" ]; then
        echo "${OCINYE_CI_DISK_FREE_GB}"
        return
    fi
    df -k / | awk 'NR==2 { printf "%.1f", $4 / 1048576 }'
}

medir() {
    printf '%-28s %s GB livres\n' "${1:-disco}" "$(livre_gb)"
}

libertar() {
    echo "Libertar espaço no runner"
    medir "  antes"

    # Ferramentas que a imagem do GitHub traz por omissão e que o Ocinye OS não
    # usa. Nenhuma delas participa em qualquer build, teste ou serviço deste
    # repositório; o Docker, o Rust, o clang, o PostgreSQL e o Chrome ficam.
    local alvo
    for alvo in /usr/share/dotnet \
                /opt/ghc \
                /usr/local/lib/android \
                /usr/local/share/boost \
                /usr/local/share/powershell \
                /usr/share/swift \
                /opt/hostedtoolcache/CodeQL; do
        if [ -e "$alvo" ]; then
            sudo rm -rf "$alvo" || echo "  (não foi possível remover $alvo)"
        fi
    done

    # Imagens pré-carregadas, não o motor: o job do Compose precisa do Docker.
    sudo docker image prune --all --force >/dev/null 2>&1 || true

    medir "  depois"
}

exigir() {
    local minimo="$1" livre
    livre=$(livre_gb)

    if awk -v l="$livre" -v m="$minimo" 'BEGIN { exit !(l < m) }'; then
        echo >&2
        echo "Disco insuficiente para a verificação do Ocinye." >&2
        echo "  livres:    ${livre} GB" >&2
        echo "  necessário: ${minimo} GB" >&2
        echo >&2
        echo "Falha agora, e não daqui a vinte minutos dentro do compilador." >&2
        exit 1
    fi

    printf '%-28s %s GB livres, mínimo %s GB\n' "capacidade suficiente" "$livre" "$minimo"
}

case "${1:-}" in
    medir)    medir "${2:-disco}" ;;
    libertar) libertar ;;
    exigir)   exigir "${2:?indique o mínimo em GB}" ;;
    *)
        echo "uso: $0 {medir [etiqueta]|libertar|exigir <GB>}" >&2
        exit 64
        ;;
esac
