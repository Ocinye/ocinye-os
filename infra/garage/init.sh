#!/bin/sh
# Prepara o Garage de uma Instância: layout, chave do Core, bucket privado.
#
# Idempotente — corre em cada arranque do Compose, e só muda o que falta. Corre
# na imagem do Core (tem `curl`), porque a do Garage não tem shell; fala com a API
# de administração do Garage, pela rede interna, com o token de administração.
#
# O Garage não tem acesso anónimo: um bucket é privado até uma chave o receber, e
# aqui só a chave do Core o recebe. É o equivalente do `mc anonymous set none`
# que o MinIO precisava.
#
# Ambiente: GARAGE_ADMIN_TOKEN, OCINYE_GARAGE_ADMIN_URL (http://object-store:3903),
# OCINYE_STORAGE_ACCESS_KEY (GK + 24 hex), OCINYE_STORAGE_SECRET_KEY (64 hex),
# OCINYE_STORAGE_BUCKET, OCINYE_GARAGE_CAPACITY (bytes; por omissão 1 TB — um peso
# relativo num nó único, não uma quota).
set -euo pipefail

URL="${OCINYE_GARAGE_ADMIN_URL:-http://object-store:3903}/v2"
: "${GARAGE_ADMIN_TOKEN:?falta GARAGE_ADMIN_TOKEN}"
: "${OCINYE_STORAGE_ACCESS_KEY:?falta OCINYE_STORAGE_ACCESS_KEY}"
: "${OCINYE_STORAGE_SECRET_KEY:?falta OCINYE_STORAGE_SECRET_KEY}"
: "${OCINYE_STORAGE_BUCKET:?falta OCINYE_STORAGE_BUCKET}"
CAPACIDADE="${OCINYE_GARAGE_CAPACITY:-1000000000000}"

api() {  # método caminho [corpo]
    curl -sS -X "$1" -H "Authorization: Bearer $GARAGE_ADMIN_TOKEN" \
        -H 'Content-Type: application/json' ${3:+-d "$3"} "$URL/$2"
}
campo() {  # nome — o primeiro valor desse campo num JSON, sem jq
    grep -o "\"$1\": *[^,}]*" | head -1 | sed 's/^[^:]*: *//; s/"//g'
}

for _ in $(seq 1 60); do
    api GET GetClusterStatus >/dev/null 2>&1 && break
    sleep 2
done
estado="$(api GET GetClusterStatus)" || { echo "o Garage não respondeu" >&2; exit 1; }

# ── Layout: o nó recebe um papel, uma vez ────────────────────────────────
if printf '%s' "$estado" | grep -q '"role": *null'; then
    no="$(printf '%s' "$estado" | campo id)"
    versao="$(api GET GetClusterLayout | campo version)"
    api POST UpdateClusterLayout \
        "{\"roles\":[{\"id\":\"$no\",\"zone\":\"local\",\"capacity\":$CAPACIDADE,\"tags\":[]}]}" >/dev/null
    api POST ApplyClusterLayout "{\"version\":$(( versao + 1 ))}" >/dev/null
    echo "layout aplicado (versão $(( versao + 1 )))"
fi

# ── A chave do Core, com o identificador e o segredo da configuração ─────
if ! api GET "GetKeyInfo?id=$OCINYE_STORAGE_ACCESS_KEY" | grep -q '"accessKeyId"'; then
    api POST ImportKey "{\"accessKeyId\":\"$OCINYE_STORAGE_ACCESS_KEY\",\"secretAccessKey\":\"$OCINYE_STORAGE_SECRET_KEY\",\"name\":\"ocinye-core\"}" \
        | grep -q '"accessKeyId"' || { echo "a importação da chave falhou" >&2; exit 1; }
    echo "chave do Core importada"
fi

# ── O bucket, privado, só para essa chave ────────────────────────────────
if ! api GET "GetBucketInfo?globalAlias=$OCINYE_STORAGE_BUCKET" | grep -q '"id"'; then
    api POST CreateBucket "{\"globalAlias\":\"$OCINYE_STORAGE_BUCKET\"}" >/dev/null
    echo "bucket $OCINYE_STORAGE_BUCKET criado"
fi
bucket="$(api GET "GetBucketInfo?globalAlias=$OCINYE_STORAGE_BUCKET" | grep -o '"id": *"[0-9a-f]\{64\}"' | head -1 | grep -o '[0-9a-f]\{64\}')"
[ -n "$bucket" ] || { echo "o bucket não existe depois de criado" >&2; exit 1; }
api POST AllowBucketKey \
    "{\"bucketId\":\"$bucket\",\"accessKeyId\":\"$OCINYE_STORAGE_ACCESS_KEY\",\"permissions\":{\"read\":true,\"write\":true,\"owner\":true}}" \
    | grep -q '"read": *true' || { echo "a chave não recebeu o bucket" >&2; exit 1; }
echo "bucket ready and private"
