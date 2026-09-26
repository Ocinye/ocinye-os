#!/usr/bin/env bash
# A prova de actualização (Parte 10): instalar N, criar dados, actualizar para
# N+1, provar os dados intactos, tentar um release que falha e ver a Instância
# voltar sozinha, e reverter à mão — num anfitrião Linux limpo e descartável.
#
# Três pacotes:
#   A  o release N, instalado de raiz
#   B  o release N+1, que se instala por cima de A
#   C  um release que falha de propósito: uma migração que aplica e outra que
#      rebenta, para que a falha chegue **depois** de o esquema ter mudado — o
#      caso em que reverter exige repor a base, e não só trocar o release
#
# Uso: scripts/upgrade-e2e.sh PACOTE_A PACOTE_B PACOTE_C
set -euo pipefail

A="$(cd "${1:?pacote A}" && pwd)"; B="$(cd "${2:?pacote B}" && pwd)"; C="$(cd "${3:?pacote C}" && pwd)"
PORTO="${OCINYE_INSTALL_E2E_PORT:-18443}"
DOMINIO="os.instalacao.test"
NOME="ocinye-upgrade-e2e"

passo() { printf '\n== %s ==\n' "$1"; }
no_anfitriao() { docker exec "$NOME" "$@"; }
sql() { no_anfitriao docker exec ocinye-postgres-1 psql -U ocinye -d ocinye -tAc "$1" | tr -d '[:space:]'; }
release() { no_anfitriao sed -n 's/^OCINYE_RELEASE_SHA=//p' /etc/ocinye/release.env; }
falha() { printf '\n  FALHOU — %s\n\n' "$1" >&2; exit 1; }

passo "Anfitrião limpo"
docker rm -f "$NOME" >/dev/null 2>&1 || true
docker run -d --privileged --name "$NOME" -p "127.0.0.1:$PORTO:443" \
    -e DOCKER_TLS_CERTDIR= docker:27-dind >/dev/null
trap 'docker rm -f "$NOME" >/dev/null 2>&1 || true' EXIT
for _ in $(seq 1 60); do no_anfitriao docker info >/dev/null 2>&1 && break; sleep 2; done
no_anfitriao sh -c 'apk add --no-cache bash curl coreutils >/dev/null'
for p in A B C; do docker cp "${!p}" "$NOME:/root/$p"; done

passo "Instalar N ($(cat "$A/RELEASE"))"
no_anfitriao /root/A/install/ocinye install --domain "$DOMINIO" --public-url "https://$DOMINIO:$PORTO" \
    --instance-name "Instância de actualização" --profile research \
    --name "Pessoa de Prova" --email pessoa@instalacao.test \
    --admin-name "Pessoa de Prova (Admin)" --admin-email admin@instalacao.test \
    --tls self-signed --credential-file /root/credencial
CRED="$(mktemp)"; docker cp "$NOME:/root/credencial" "$CRED"

passo "Dados reais, criados por um browser"
OCINYE_INSTALLED_URL="https://$DOMINIO:$PORTO" OCINYE_INSTALLED_EMAIL=admin@instalacao.test \
OCINYE_INSTALLED_CREDENTIAL_FILE="$CRED" OCINYE_INSTALLED_PROFILE=research \
OCINYE_INSTALLED_RESOLVE="$DOMINIO 127.0.0.1" \
    cargo test -q -p ocinye-workspace --test installed_instance -- --ignored
rm -f "$CRED"
NOTAS="$(sql "SELECT count(*) FROM notes WHERE title = 'Primeira nota da Instância'")"
PESSOAS="$(sql "SELECT count(*) FROM people")"
ESQUEMA_N="$(sql "SELECT max(version) FROM _sqlx_migrations")"
[ "$NOTAS" = 1 ] || falha "a nota do browser não está na base ($NOTAS)"
echo "  nota: $NOTAS · pessoas: $PESSOAS · esquema: $ESQUEMA_N"

passo "Actualizar N → N+1 ($(cat "$B/RELEASE"))"
no_anfitriao /root/B/install/ocinye upgrade
[ "$(release)" = "$(cat "$B/RELEASE")" ] || falha "o release corrente não é N+1"
[ "$(sql "SELECT count(*) FROM notes WHERE title = 'Primeira nota da Instância'")" = 1 ] || falha "a nota perdeu-se na actualização"
[ "$(sql "SELECT count(*) FROM people")" = "$PESSOAS" ] || falha "as pessoas mudaram na actualização"
ESQUEMA_N1="$(sql "SELECT max(version) FROM _sqlx_migrations")"
echo "  release $(release) · dados intactos · esquema $ESQUEMA_N → $ESQUEMA_N1"

passo "Um release que falha ($(cat "$C/RELEASE"))"
set +e
no_anfitriao /root/C/install/ocinye upgrade
SAIDA=$?
set -e
[ "$SAIDA" = 3 ] || falha "a actualização falhada devia terminar com 3 (recusada e revertida); terminou com $SAIDA"
[ "$(release)" = "$(cat "$B/RELEASE")" ] || falha "a Instância não voltou a N+1"
[ "$(sql "SELECT max(version) FROM _sqlx_migrations")" = "$ESQUEMA_N1" ] || falha "o esquema não voltou a $ESQUEMA_N1"
[ "$(sql "SELECT to_regclass('public.upgrade_failure_probe') IS NULL")" = t ] \
    || falha "a tabela da migração que aplicou ficou na base: o checkpoint não foi reposto"
[ "$(sql "SELECT count(*) FROM notes WHERE title = 'Primeira nota da Instância'")" = 1 ] || falha "a nota perdeu-se na reversão"
echo "  recusada, revertida para $(release), esquema $ESQUEMA_N1, sem restos, dados intactos"

passo "Reversão manual N+1 → N"
no_anfitriao /root/B/install/ocinye rollback --confirm
[ "$(release)" = "$(cat "$A/RELEASE")" ] || falha "a reversão não voltou a N"
[ "$(sql "SELECT count(*) FROM notes WHERE title = 'Primeira nota da Instância'")" = 1 ] || falha "a nota perdeu-se na reversão manual"
echo "  release $(release) · esquema $(sql "SELECT max(version) FROM _sqlx_migrations") · dados intactos"

passo "Destruir"
no_anfitriao /root/A/install/ocinye uninstall --purge
printf '\n  Actualização, falha revertida e reversão manual provadas de raiz.\n\n'
