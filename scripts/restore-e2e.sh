#!/usr/bin/env bash
# A prova de backup e restauro (Parte 11): uma Instância com dados reais, um
# conjunto de continuidade, a origem destruída, e a mesma instituição de volta
# noutro anfitrião limpo — com as mesmas pessoas a entrar com as mesmas
# credenciais e o mesmo segundo factor.
#
# Dois anfitriões `docker:dind`, criados para esta corrida e apagados no fim:
#   origem   instala, uma pessoa trabalha por um browser, faz backup
#   destino  instala **a partir do conjunto**, com a raiz de selagem da origem
#
# Entre os dois só viajam o conjunto e a raiz de selagem — e viajam por
# caminhos separados, como o ADR-0700 exige.
#
# Uso: scripts/restore-e2e.sh PASTA_DO_PACOTE
set -euo pipefail

PACOTE="$(cd "${1:?indique a pasta do pacote}" && pwd)"
PORTO="${OCINYE_INSTALL_E2E_PORT:-18443}"
DOMINIO="os.instalacao.test"
ORIGEM="ocinye-restore-e2e-origem"
DESTINO="ocinye-restore-e2e-destino"
TRABALHO="$(mktemp -d)"

passo() { printf '\n== %s ==\n' "$1"; }
falha() { printf '\n  FALHOU — %s\n\n' "$1" >&2; exit 1; }
limpar() { docker rm -f -v "$ORIGEM" "$DESTINO" >/dev/null 2>&1 || true; rm -rf "$TRABALHO"; }
trap limpar EXIT

anfitriao() {  # nome
    docker rm -f -v "$1" >/dev/null 2>&1 || true
    docker run -d --privileged --name "$1" -p "127.0.0.1:$PORTO:443" \
        -e DOCKER_TLS_CERTDIR= docker:27-dind >/dev/null
    for _ in $(seq 1 60); do docker exec "$1" docker info >/dev/null 2>&1 && break; sleep 2; done
    docker exec "$1" sh -c 'apk add --no-cache bash curl coreutils >/dev/null'
    docker exec "$1" sh -c 'test ! -e /etc/ocinye && test ! -e /srv/ocinye' || falha "$1 não está limpo"
    docker cp "$PACOTE" "$1:/root/pacote"
}

browser() {  # teste
    OCINYE_TEST_INSTALLED_URL="https://$DOMINIO:$PORTO" \
    OCINYE_TEST_INSTALLED_EMAIL=admin@instalacao.test \
    OCINYE_TEST_INSTALLED_CREDENTIAL_FILE="$TRABALHO/credencial" \
    OCINYE_TEST_INSTALLED_STATE_FILE="$TRABALHO/estado" \
    OCINYE_TEST_INSTALLED_PROFILE=research \
    OCINYE_TEST_INSTALLED_RESOLVE="$DOMINIO 127.0.0.1" \
        cargo test -q -p ocinye-workspace --test installed_instance -- --ignored --exact "$1"
}

passo "Origem: instalar"
anfitriao "$ORIGEM"
docker exec "$ORIGEM" /root/pacote/install/ocinye install --domain "$DOMINIO" \
    --public-url "https://$DOMINIO:$PORTO" --instance-name "Instância de origem" --profile research \
    --name "Pessoa de Prova" --email pessoa@instalacao.test \
    --admin-name "Pessoa de Prova (Admin)" --admin-email admin@instalacao.test \
    --tls self-signed --credential-file /root/credencial
docker cp "$ORIGEM:/root/credencial" "$TRABALHO/credencial"

passo "Origem: uma pessoa trabalha"
browser uma_instancia_instalada_abre_entra_e_trabalha
[ -s "$TRABALHO/estado" ] || falha "o browser não guardou a palavra-passe e o seed"

passo "Origem: backup"
docker exec "$ORIGEM" /root/pacote/install/ocinye backup
CONJUNTO="$(docker exec "$ORIGEM" sh -c 'ls -1dt /srv/ocinye/backups/ocinye-* | grep -v INCOMPLETO | head -1')"
docker cp "$ORIGEM:$CONJUNTO" "$TRABALHO/conjunto"
# A raiz de selagem viaja à parte — nunca dentro do conjunto.
docker exec "$ORIGEM" sed -n 's/^OCINYE_SEALING_KEY=//p' /etc/ocinye/core.env > "$TRABALHO/selagem"
grep -rqF "$(cat "$TRABALHO/selagem")" "$TRABALHO/conjunto" && falha "a raiz de selagem está dentro do conjunto"
ORIGEM_NOTAS="$(docker exec "$ORIGEM" docker exec ocinye-postgres-1 psql -U ocinye -d ocinye -tAc 'SELECT count(*) FROM notes' | tr -d '[:space:]')"
echo "  conjunto $(basename "$CONJUNTO") · $(find "$TRABALHO/conjunto/objects" -type f | wc -l | tr -d ' ') objecto(s) · $ORIGEM_NOTAS nota(s)"

passo "Origem: destruída"
docker rm -f -v "$ORIGEM" >/dev/null

passo "Destino: instalar a partir do conjunto"
anfitriao "$DESTINO"
docker cp "$TRABALHO/conjunto" "$DESTINO:/root/conjunto"
docker cp "$TRABALHO/selagem" "$DESTINO:/root/selagem"
docker exec "$DESTINO" /root/pacote/install/ocinye install --domain "$DOMINIO" \
    --public-url "https://$DOMINIO:$PORTO" --tls self-signed \
    --restore /root/conjunto --sealing-key-file /root/selagem

passo "Destino: a mesma pessoa volta a entrar"
browser uma_instancia_restaurada_reconhece_quem_la_estava

passo "Negativo: sem a raiz de selagem"
docker exec "$DESTINO" /root/pacote/install/ocinye uninstall --purge
set +e
docker exec "$DESTINO" /root/pacote/install/ocinye install --domain "$DOMINIO" --tls self-signed \
    --restore /root/conjunto 2>&1 | tail -3
SAIDA=${PIPESTATUS[0]}
set -e
[ "$SAIDA" != 0 ] || falha "restaurar sem a raiz de selagem devia ser recusado"
echo "  recusado"

passo "Negativo: com uma raiz de selagem errada"
docker exec "$DESTINO" /root/pacote/install/ocinye uninstall --purge >/dev/null 2>&1 || true
head -c 32 /dev/urandom | base64 > "$TRABALHO/errada"
docker cp "$TRABALHO/errada" "$DESTINO:/root/errada"
set +e
docker exec "$DESTINO" /root/pacote/install/ocinye install --domain "$DOMINIO" --tls self-signed \
    --restore /root/conjunto --sealing-key-file /root/errada > "$TRABALHO/errada.log" 2>&1
SAIDA=$?
set -e
tail -4 "$TRABALHO/errada.log"
[ "$SAIDA" != 0 ] || falha "restaurar com a raiz errada abriu a Instância"
grep -q "verificação de continuidade não passou" "$TRABALHO/errada.log" \
    || falha "a recusa não veio da verificação de continuidade"
docker exec "$DESTINO" sh -c 'test -z "$(docker ps -q --filter name=ocinye-workspace-1)"' \
    || falha "o Workspace arrancou sobre um restauro que não abre"
echo "  recusado pela verificação de continuidade; o Workspace nunca arrancou"

printf '\n  Backup e restauro provados: outra máquina, as mesmas pessoas, os mesmos dados.\n\n'
