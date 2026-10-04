#!/usr/bin/env bash
# VMs descartáveis para provar o Ocinye OS Installer (D011) — localmente, com
# o Lima, uma de cada vez.
#
#   scripts/installer-vm.sh create  NOME   cria e arranca (Ubuntu 24.04 minimal)
#   scripts/installer-vm.sh ip      NOME   o endereço que o Installer usa
#   scripts/installer-vm.sh baseline NOME  prova que a VM está limpa
#   scripts/installer-vm.sh measure NOME   disco, memória e serviços, agora
#   scripts/installer-vm.sh destroy NOME   apaga a VM e as credenciais de teste
#
# # Só VMs deste script
#
# O NOME tem de ser `ocinye-d011-…`, e `destroy` só apaga uma VM que este
# script criou (a marca fica em ~/.cache/ocinye-installer-test/NOME). Um nome
# parecido não basta: nunca se apaga uma máquina por adivinhar que é de testes.
#
# # Credenciais de teste
#
# Cada VM tem um utilizador `operador` com uma chave SSH e uma palavra-passe de
# sudo **geradas aqui, para esta VM**, guardadas fora do repositório, em
# ~/.cache/ocinye-installer-test/NOME (0700), e apagadas com a VM. (A
# palavra-passe fica também na configuração local do Lima desta VM, que o
# `destroy` apaga.)
set -euo pipefail

TEMPLATE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/infra/installer-test/lima-ubuntu-24.04-minimal.yaml"
CACHE="${HOME}/.cache/ocinye-installer-test"

fatal() { printf '\n  RECUSADO — %s\n\n' "$1" >&2; exit 1; }

nome_valido() {
    [[ "$1" =~ ^ocinye-d011-[a-z0-9-]{1,40}$ ]] || fatal "o nome tem de ser ocinye-d011-…: $1"
}

dono() {  # a VM foi criada por este script?
    [ -f "$CACHE/$1/created-by-installer-vm" ] || fatal "$1 não foi criada por este script: não lhe toco"
}

vm() { limactl shell --workdir / "$1" -- "${@:2}"; }

ip_de() {
    vm "$1" ip -4 -o addr show lima0 | awk '{print $4}' | cut -d/ -f1
}

case "${1:-}" in
    create)
        nome="${2:?nome}"; nome_valido "$nome"
        limactl list -q 2>/dev/null | grep -qx "$nome" && fatal "$nome já existe"
        d="$CACHE/$nome"; rm -rf "$d"; install -d -m 700 "$d"
        ssh-keygen -q -t ed25519 -N '' -C "ocinye-installer-test $nome" -f "$d/id_ed25519"
        head -c 18 /dev/urandom | base64 | tr -d '/+=\n' > "$d/sudo-password"
        chmod 600 "$d/sudo-password"
        : > "$d/created-by-installer-vm"
        limactl create --tty=false --name "$nome" \
            --set ".param.OPERATOR_PUBKEY=\"$(cat "$d/id_ed25519.pub")\"" \
            --set ".param.OPERATOR_PASSWORD=\"$(cat "$d/sudo-password")\"" \
            "$TEMPLATE"
        limactl start --tty=false "$nome"
        ip_de "$nome" > "$d/ip"
        printf '  %s · %s · operador · chave %s\n' "$nome" "$(cat "$d/ip")" "$d/id_ed25519"
        ;;
    ip)
        nome="${2:?nome}"; nome_valido "$nome"; dono "$nome"
        ip_de "$nome"
        ;;
    baseline)
        nome="${2:?nome}"; nome_valido "$nome"; dono "$nome"
        vm "$nome" sh -c '. /etc/os-release && echo "os $ID $VERSION_ID $PRETTY_NAME"'
        vm "$nome" uname -m | sed 's/^/arch /'
        if vm "$nome" sh -c 'command -v docker || command -v containerd || command -v podman'; then
            fatal "a VM já tem um runtime de contentores"
        fi
        echo "docker ausente"
        vm "$nome" sh -c 'test ! -e /srv/ocinye && test ! -e /etc/ocinye && test ! -e /var/lib/ocinye-installer' \
            || fatal "a VM já tem restos do Ocinye"
        echo "sem /srv/ocinye, /etc/ocinye nem /var/lib/ocinye-installer"
        if vm "$nome" sh -c 'ls /usr/share/xsessions /usr/share/wayland-sessions 2>/dev/null | grep -q .'; then
            fatal "a VM tem um ambiente gráfico"
        fi
        echo "sem sessões gráficas nem gestor de ecrã"
        ;;
    measure)
        nome="${2:?nome}"; nome_valido "$nome"; dono "$nome"
        vm "$nome" sh -c 'echo "disco_usado_bytes $(df -B1 --output=used / | tail -1 | tr -d " ")"
            echo "memoria_usada_bytes $(free -b | awk "/^Mem:/ {print \$3}")"
            echo "memoria_disponivel_bytes $(free -b | awk "/^Mem:/ {print \$7}")"
            echo "swap_bytes $(free -b | awk "/^Swap:/ {print \$2}")"
            echo "servicos_a_correr $(systemctl list-units --type=service --state=running --no-legend | wc -l)"'
        if vm "$nome" sh -c 'command -v docker >/dev/null'; then
            vm "$nome" sudo docker stats --no-stream --format '{{.Name}} {{.MemUsage}}' | sed 's/^/contentor /'
            vm "$nome" sudo docker system df --format '{{.Type}} {{.Size}}' | sed 's/^/docker_df /'
        fi
        ;;
    destroy)
        nome="${2:?nome}"; nome_valido "$nome"; dono "$nome"
        limactl delete -f "$nome"
        rm -rf "${CACHE:?}/$nome"
        echo "  $nome apagada"
        ;;
    *)
        sed -n '2,24p' "$0"; exit 2 ;;
esac
