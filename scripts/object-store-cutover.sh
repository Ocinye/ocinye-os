#!/usr/bin/env bash
# Passa os bytes de uma Instância do MinIO arquivado para o Garage (ADR-0208).
#
# Corre **no anfitrião**, contra o release novo já extraído, **antes** de o
# release corrente mudar. Chamam-no o `install/ocinye upgrade` (Instâncias
# instaladas) e o `scripts/deploy-production.sh` (a produção da Ocinye): um só
# procedimento, ensaiado pelo primeiro antes de correr no segundo.
#
#   PRE-CHECK  serviços de pé; inventário do MinIO (objectos, bytes) e da base
#   MIGRATE    Core e Worker parados (sem escritas); Garage levantado e
#              preparado; cópia MinIO → Garage com o rclone
#   VERIFY     rclone check; contagem e bytes iguais; `verify-objects` — o Core
#              recalcula a soma de CADA objecto registado na base, a ler do Garage
#   CUTOVER    a configuração do Core passa a apontar para o Garage
#   ROLLBACK   se qualquer passo falhar: a configuração fica como estava, o MinIO
#              continua intacto, e o release anterior volta a arrancar
#
# O MinIO **nunca** é apagado aqui: os dados ficam no volume `object-data`, e a
# configuração anterior em `core.env.pre-garage`, para o rollback manual. Sair
# do MinIO de vez é um passo separado, depois de verificado.
#
# Idempotente: com `/etc/ocinye/object-store-cutover.done`, não faz nada.
#
# Uso: object-store-cutover.sh PASTA_DO_RELEASE_NOVO RELEASE_SHA
set -euo pipefail

NOVO="${1:?pasta do release novo}"
SHA="${2:?sha do release novo}"
CONFIG="${OCINYE_CONFIG_DIR:-/etc/ocinye}"
MARCA="$CONFIG/object-store-cutover.done"
COMPOSE="infra/compose/docker-compose.production.yml"

fatal() { printf '\n  CUTOVER RECUSADO — %s\n\n' "$1" >&2; exit 1; }
passo() { printf '\n== Armazenamento: %s ==\n' "$1"; }
nota()  { printf '  %s\n' "$1"; }

[ -e "$MARCA" ] && { nota "o armazenamento já é o Garage ($(cat "$MARCA"))"; exit 0; }
grep -q '^GARAGE_RPC_SECRET=' "$CONFIG/object-store.env" 2>/dev/null \
    && ! grep -q '^MINIO_ROOT_USER=' "$CONFIG/object-store.env" \
    && { nota "instalação nova, já nasceu no Garage"; exit 0; }
grep -q '^MINIO_ROOT_USER=' "$CONFIG/object-store.env" \
    || fatal "nem MinIO nem Garage configurados em $CONFIG/object-store.env"

novo() { (cd "$NOVO" && OCINYE_RELEASE_SHA="$SHA" docker compose -f "$COMPOSE" "$@"); }
valor() { sed -n "s/^$1=//p" "$2" | tail -1; }

# ── PRE-CHECK ────────────────────────────────────────────────────────────
passo "PRE-CHECK"
docker inspect -f '{{.State.Health.Status}}' ocinye-object-store-1 2>/dev/null | grep -q healthy \
    || fatal "o MinIO corrente não está saudável"
docker inspect -f '{{.State.Health.Status}}' ocinye-postgres-1 2>/dev/null | grep -q healthy \
    || fatal "a base não está saudável"
BUCKET="$(valor OCINYE_STORAGE_BUCKET "$CONFIG/core.env")"
[ -n "$BUCKET" ] || fatal "OCINYE_STORAGE_BUCKET não está em core.env"
BASE_OBJECTOS="$(docker exec ocinye-postgres-1 psql -U ocinye -d ocinye -tAc \
    "SELECT count(*) || ' ' || coalesce(sum(size_bytes),0) FROM storage_objects WHERE status = 'stored'")"
nota "base: $BASE_OBJECTOS (objectos, bytes) registados como guardados"

# Cada ficheiro reescrito fica com o dono, o grupo e o modo do original. Este
# script corre como root; o Compose da produção corre como o utilizador de
# serviço, que lê a configuração pelo grupo. Um `install -m 600` deixou-a legível
# só pelo root — e a produção não arrancou depois de uma passagem verificada.
DONO_CORE="$(stat -c '%u %g %a' "$CONFIG/core.env")"
DONO_OS="$(stat -c '%u %g %a' "$CONFIG/object-store.env")"
copiar() {  # origem destino "uid gid modo"
    set -- "$1" "$2" $3
    install -o "$3" -g "$4" -m "$5" "$1" "$2"
}

# As credenciais do MinIO, guardadas para a fonte da cópia e para o rollback.
copiar "$CONFIG/object-store.env" "$CONFIG/object-store-legacy.env" "$DONO_OS"
copiar "$CONFIG/core.env" "$CONFIG/core.env.pre-garage" "$DONO_CORE"

# ── Os segredos do Garage, gerados aqui ──────────────────────────────────
hexa() { head -c "$1" /dev/urandom | od -An -tx1 | tr -d ' \n'; }
CHAVE="GK$(hexa 12)"; SEGREDO="$(hexa 32)"
umask 077
{
    grep -v '^MINIO_ROOT_' "$CONFIG/object-store.env" || true
    printf 'GARAGE_RPC_SECRET=%s\nGARAGE_ADMIN_TOKEN=%s\n' "$(hexa 32)" "$(hexa 24)"
} > "$CONFIG/object-store.env.garage"
sed -e "s#^OCINYE_STORAGE_ENDPOINT_URL=.*#OCINYE_STORAGE_ENDPOINT_URL=http://object-store:3900#" \
    -e "s#^OCINYE_STORAGE_ACCESS_KEY=.*#OCINYE_STORAGE_ACCESS_KEY=$CHAVE#" \
    -e "s#^OCINYE_STORAGE_SECRET_KEY=.*#OCINYE_STORAGE_SECRET_KEY=$SEGREDO#" \
    "$CONFIG/core.env" > "$CONFIG/core.env.garage"
grep -q '^OCINYE_STORAGE_REGION=' "$CONFIG/core.env.garage" \
    || echo 'OCINYE_STORAGE_REGION=us-east-1' >> "$CONFIG/core.env.garage"
umask 022

rollback() {
    printf '\n== Armazenamento: ROLLBACK ==\n' >&2
    copiar "$CONFIG/core.env.pre-garage" "$CONFIG/core.env" "$DONO_CORE"
    copiar "$CONFIG/object-store-legacy.env" "$CONFIG/object-store.env" "$DONO_OS"
    rm -f "$CONFIG/object-store.env.garage" "$CONFIG/core.env.garage"
    novo stop object-store object-store-legacy >/dev/null 2>&1 || true
    # O release corrente não mudou: voltar a levantá-lo devolve o MinIO, com os
    # dados que nunca deixaram o volume.
    (cd "$(readlink -f "${OCINYE_ROOT:-/srv/ocinye}/current")" \
        && set -a && . "$CONFIG/release.env" && set +a \
        && docker compose -f "$COMPOSE" up -d --remove-orphans >/dev/null 2>&1) || true
    fatal "$1 — a Instância continua no MinIO, intacta"
}

# ── MIGRATE ──────────────────────────────────────────────────────────────
passo "MIGRATE"
# Sem escritas durante a cópia: o Core e o Worker param. É a janela de
# manutenção desta transição, e só dura a cópia e a verificação.
docker stop ocinye-core-1 ocinye-worker-1 ocinye-workspace-1 >/dev/null 2>&1 || true
docker stop ocinye-object-store-1 >/dev/null 2>&1 || true
docker rm ocinye-object-store-1 >/dev/null 2>&1 || true
copiar "$CONFIG/object-store.env.garage" "$CONFIG/object-store.env" "$DONO_OS"
novo --profile legacy-object-store up -d object-store-legacy >/dev/null \
    || rollback "o MinIO não voltou a levantar como fonte"
novo up -d object-store >/dev/null || rollback "o Garage não arrancou"
for _ in $(seq 1 60); do
    [ "$(docker inspect -f '{{.State.Health.Status}}' ocinye-object-store-1 2>/dev/null)" = healthy ] \
        && [ "$(docker inspect -f '{{.State.Health.Status}}' ocinye-object-store-legacy-1 2>/dev/null)" = healthy ] \
        && break
    sleep 3
done
# O Core ainda lê as credenciais do MinIO em core.env: as do Garage vão por `-e`.
GARAGE_ENV=(-e OCINYE_STORAGE_ENDPOINT_URL=http://object-store:3900
            -e OCINYE_STORAGE_REGION=us-east-1
            -e OCINYE_STORAGE_ACCESS_KEY="$CHAVE" -e OCINYE_STORAGE_SECRET_KEY="$SEGREDO")
novo run --rm -T --no-deps "${GARAGE_ENV[@]}" object-store-init >/dev/null \
    || rollback "a preparação do Garage falhou"

LEGADO_CHAVE="$(valor OCINYE_STORAGE_ACCESS_KEY "$CONFIG/core.env.pre-garage")"
LEGADO_SEGREDO="$(valor OCINYE_STORAGE_SECRET_KEY "$CONFIG/core.env.pre-garage")"
rclone_() {
    novo --profile backup run --rm -T --no-deps --entrypoint rclone \
        -e RCLONE_CONFIG=/dev/null \
        -e RCLONE_CONFIG_ORIGEM_TYPE=s3 -e RCLONE_CONFIG_ORIGEM_PROVIDER=Minio \
        -e RCLONE_CONFIG_ORIGEM_ENDPOINT=http://object-store-legacy:9000 \
        -e RCLONE_CONFIG_ORIGEM_ACCESS_KEY_ID="$LEGADO_CHAVE" \
        -e RCLONE_CONFIG_ORIGEM_SECRET_ACCESS_KEY="$LEGADO_SEGREDO" \
        -e RCLONE_CONFIG_ORIGEM_FORCE_PATH_STYLE=true \
        -e RCLONE_CONFIG_DESTINO_TYPE=s3 -e RCLONE_CONFIG_DESTINO_PROVIDER=Other \
        -e RCLONE_CONFIG_DESTINO_ENDPOINT=http://object-store:3900 \
        -e RCLONE_CONFIG_DESTINO_REGION=us-east-1 \
        -e RCLONE_CONFIG_DESTINO_ACCESS_KEY_ID="$CHAVE" \
        -e RCLONE_CONFIG_DESTINO_SECRET_ACCESS_KEY="$SEGREDO" \
        -e RCLONE_CONFIG_DESTINO_FORCE_PATH_STYLE=true \
        backup "$@"
}
novo --profile backup build backup >/dev/null || rollback "a imagem com o rclone não se construiu"
rclone_ copy --quiet "origem:$BUCKET" "destino:$BUCKET" || rollback "a cópia falhou"

# ── VERIFY ───────────────────────────────────────────────────────────────
passo "VERIFY"
rclone_ check --quiet --one-way "origem:$BUCKET" "destino:$BUCKET" \
    || rollback "o rclone check encontrou diferenças"
ORIGEM="$(rclone_ size --json "origem:$BUCKET" | tr -d ' \n')"
DESTINO="$(rclone_ size --json "destino:$BUCKET" | tr -d ' \n')"
nota "MinIO: $ORIGEM"
nota "Garage: $DESTINO"
contagem() { printf '%s' "$1" | grep -o '"count":[0-9]*' | cut -d: -f2; }
bytes() { printf '%s' "$1" | grep -o '"bytes":[0-9]*' | cut -d: -f2; }
[ "$(contagem "$ORIGEM")" = "$(contagem "$DESTINO")" ] && [ "$(bytes "$ORIGEM")" = "$(bytes "$DESTINO")" ] \
    || rollback "a contagem ou os bytes não conferem"
# A prova forte: o Core lê cada objecto registado na base, do Garage, e
# recalcula-lhe a soma.
novo run --rm -T --no-deps "${GARAGE_ENV[@]}" core verify-objects \
    > /tmp/ocinye-cutover-verify.log 2>&1 \
    || { cat /tmp/ocinye-cutover-verify.log >&2; rollback "o verify-objects não passou contra o Garage"; }
tail -3 /tmp/ocinye-cutover-verify.log | sed 's/^/  /'

# ── CUTOVER ──────────────────────────────────────────────────────────────
passo "CUTOVER"
copiar "$CONFIG/core.env.garage" "$CONFIG/core.env" "$DONO_CORE"
rm -f "$CONFIG/core.env.garage" "$CONFIG/object-store.env.garage"
novo --profile legacy-object-store stop object-store-legacy >/dev/null 2>&1 || true
printf '%s %s objectos=%s bytes=%s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$SHA" \
    "$(contagem "$DESTINO")" "$(bytes "$DESTINO")" > "$MARCA"
nota "o Core aponta para o Garage; o MinIO fica parado e intacto (volume object-data)"
nota "rollback manual: repor core.env.pre-garage e object-store-legacy.env, e o release anterior"
