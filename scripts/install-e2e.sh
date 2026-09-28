#!/usr/bin/env bash
# A prova de instalação (Parte 9): instalar o pacote de um release num anfitrião
# Linux **limpo e descartável**, entrar por um browser, trabalhar, destruir — e
# repetir do zero com outro perfil.
#
# O anfitrião é um contentor `docker:dind`: um Linux com o seu próprio Docker, sem
# nenhum ficheiro do Ocinye, que se cria para cada corrida e se apaga no fim. Só
# se lhe acrescentam as ferramentas que um anfitrião Linux normal já traz (bash,
# curl, coreutils) — nada do Ocinye OS.
#
# Uso: scripts/install-e2e.sh PASTA_DO_PACOTE
set -euo pipefail

# NOT_RUN enquanto não houver interface. A prova conduz a Instância instalada
# por um browser (`apps/workspace/tests/installed_instance.rs`), e esse teste saiu
# com o apagamento da UI (2026-09-28). Correr o resto e dizer verde seria provar
# menos do que o nome promete; volta quando o código do Claude Design chegar.
if [ ! -f "$(git rev-parse --show-toplevel)/apps/workspace/tests/installed_instance.rs" ]; then
    echo "NOT_RUN: a viagem de browser da Instância instalada não existe (UI apagada; ver docs/ui/UI_WIPE_REPORT.md)" >&2
    exit 2
fi

PACOTE="${1:?indique a pasta do pacote (scripts/release-bundle.sh)}"
[ -r "$PACOTE/RELEASE" ] || { echo "não é um pacote: $PACOTE" >&2; exit 2; }
PACOTE="$(cd "$PACOTE" && pwd)"
PORTO="${OCINYE_INSTALL_E2E_PORT:-18443}"
DOMINIO="os.instalacao.test"
DIND_IMAGEM="docker:27-dind"

passo() { printf '\n== %s ==\n' "$1"; }

uma_corrida() {
    local perfil="$1" nome="ocinye-install-e2e-$1" credencial
    credencial="$(mktemp)"

    passo "Anfitrião limpo ($perfil)"
    docker rm -f -v "$nome" >/dev/null 2>&1 || true
    docker run -d --privileged --name "$nome" \
        -p "127.0.0.1:$PORTO:443" \
        -e DOCKER_TLS_CERTDIR= "$DIND_IMAGEM" >/dev/null
    trap 'docker rm -f -v "'"$nome"'" >/dev/null 2>&1 || true' RETURN
    for _ in $(seq 1 60); do
        docker exec "$nome" docker info >/dev/null 2>&1 && break
        sleep 2
    done
    docker exec "$nome" docker info >/dev/null
    docker exec "$nome" sh -c 'apk add --no-cache bash curl coreutils >/dev/null'
    docker exec "$nome" sh -c 'test ! -e /etc/ocinye && test ! -e /srv/ocinye' \
        || { echo "o anfitrião não está limpo" >&2; return 1; }
    echo "  $DIND_IMAGEM · sem /etc/ocinye nem /srv/ocinye"

    passo "Instalar ($perfil)"
    docker cp "$PACOTE" "$nome:/root/pacote"
    docker exec "$nome" /root/pacote/install/ocinye install \
        --domain "$DOMINIO" \
        --public-url "https://$DOMINIO:$PORTO" \
        --instance-name "Instância de prova $perfil" \
        --profile "$perfil" \
        --name "Pessoa de Prova" --email "pessoa@instalacao.test" \
        --admin-name "Pessoa de Prova (Admin)" --admin-email "admin@instalacao.test" \
        --tls self-signed \
        --credential-file /root/credencial
    docker cp "$nome:/root/credencial" "$credencial"

    passo "Browser ($perfil)"
    OCINYE_TEST_INSTALLED_URL="https://$DOMINIO:$PORTO" \
    OCINYE_TEST_INSTALLED_EMAIL="admin@instalacao.test" \
    OCINYE_TEST_INSTALLED_CREDENTIAL_FILE="$credencial" \
    OCINYE_TEST_INSTALLED_PROFILE="$perfil" \
    OCINYE_TEST_INSTALLED_RESOLVE="$DOMINIO 127.0.0.1" \
        cargo test -p ocinye-workspace --test installed_instance -- --ignored --exact uma_instancia_instalada_abre_entra_e_trabalha --nocapture
    rm -f "$credencial"

    passo "Estado e desinstalação ($perfil)"
    docker exec "$nome" /root/pacote/install/ocinye status
    docker exec "$nome" /root/pacote/install/ocinye uninstall --purge
    docker exec "$nome" sh -c 'test ! -e /etc/ocinye && test ! -e /srv/ocinye \
        && test -z "$(docker ps -aq)" && test -z "$(docker volume ls -q)"'
    echo "  anfitrião sem Instância, sem contentores, sem volumes"
}

for perfil in ${OCINYE_INSTALL_E2E_PROFILES:-research business personal education}; do
    uma_corrida "$perfil"
done

printf '\n  Instalação provada de raiz, uma vez por perfil.\n\n'
