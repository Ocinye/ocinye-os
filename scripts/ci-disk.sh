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
    # O volume que segura o `target/`, e não `/`: num runner com montagens
    # separadas são coisas diferentes, e o que interessa é onde a compilação
    # escreve. Hoje, num Mac, dão o mesmo — e é por isso que se escreve agora,
    # enquanto a diferença não custa nada.
    local raiz
    raiz="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
    df -k "$raiz" | awk 'NR==2 { printf "%.1f", $4 / 1048576 }'
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
        echo >&2
        echo "Isto não é um defeito do código: é evidência que não pode ser" >&2
        echo "produzida. Uma sweep que fica sem espaço a meio falha em sítios que" >&2
        echo "nada têm a ver com a causa — na milestone de ficheiros, o sintoma" >&2
        echo "foi o armazenamento a recusar escritas e quatro provas a darem" >&2
        echo "StorageUnavailable." >&2
        echo >&2
        echo "Para libertar caches de compilação, deliberadamente:" >&2
        echo "  ./scripts/ci-disk.sh caches" >&2
        exit 1
    fi

    printf '%-28s %s GB livres, mínimo %s GB\n' "capacidade suficiente" "$livre" "$minimo"
}

# Caches de compilação reconstruíveis, e nada mais.
#
# Explícito e a pedido de alguém: nunca corre sozinho, e **nunca** durante uma
# verificação — mudar o ambiente enquanto se produz evidência estraga a
# evidência. Diz o que remove antes de remover.
#
# Não toca em: base de dados, object storage, fixtures institucionais, árvore
# Git, nem em nada que não se reconstrua com um `cargo build`.
# Dois níveis, porque têm preços diferentes e quem decide tem de os ver.
#
#   caches            incremental e release. Barato: a próxima compilação de
#                     debug reaproveita quase tudo.
#   caches profundas  também o `target/debug` inteiro. Liberta dezenas de GB e
#                     custa uma recompilação completa da árvore.
#
# Nenhum dos dois corre sozinho, e nenhum corre durante uma verificação.
caches() {
    local raiz alvo profundo="${1:-}"
    raiz="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

    echo "Caches de compilação reconstruíveis em $raiz/target"
    medir "  antes"
    echo

    local alvos="target/debug/incremental target/release/incremental target/release"
    if [ "$profundo" = "profundas" ]; then
        alvos="$alvos target/debug"
        echo "  (profundas: a próxima compilação reconstrói a árvore inteira)"
        echo
    fi

    for alvo in $alvos; do
        if [ -e "$raiz/$alvo" ]; then
            printf '  %-34s %s\n' "$alvo" "$(du -sh "$raiz/$alvo" 2>/dev/null | cut -f1)"
        fi
    done
    echo

    for alvo in $alvos; do
        [ -e "$raiz/$alvo" ] && rm -rf "${raiz:?}/$alvo"
    done

    medir "  depois"
    echo
    echo "Nada mais foi tocado: nem base de dados, nem object storage, nem"
    echo "fixtures, nem a árvore versionada."
}

case "${1:-}" in
    medir)    medir "${2:-disco}" ;;
    libertar) libertar ;;
    caches)   caches "${2:-}" ;;
    exigir)   exigir "${2:?indique o mínimo em GB}" ;;
    *)
        echo "uso: $0 {medir [etiqueta]|libertar|caches [profundas]|exigir <GB>}" >&2
        exit 64
        ;;
esac
