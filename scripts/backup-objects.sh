#!/bin/sh
# Traz para o conjunto de continuidade os bytes institucionais.
#
# # Porque não um comando no ambiente
#
# `OCINYE_OBJECT_SYNC_CMD` tinha exactamente o problema do transporte remoto:
# um comando de shell numa variável, corrido por `eval`. E tinha um problema a
# mais — obrigava quem instala a escrever **outra vez** onde vivem os bytes,
# quando a aplicação já o sabe.
#
# Aqui a origem é a mesma configuração que o Core usa. Duas descrições do mesmo
# armazenamento é um sítio onde discordar, e o dia em que discordassem o backup
# copiaria um bucket que já não é o da instituição.
#
# A arquitectura continua S3-compatible (ADR-0200). O cliente é uma dependência
# declarada; o endpoint é o que a instalação tem.
#
# # Configuração
#
#     OCINYE_STORAGE_ENDPOINT_URL   a mesma que o Core lê
#     OCINYE_STORAGE_ACCESS_KEY
#     OCINYE_STORAGE_SECRET_KEY
#     OCINYE_STORAGE_BUCKET
#
# # Uso
#
#     backup-objects.sh mirror DESTINO
set -eu

fatal() { printf 'backup-objects: %s\n' "$1" >&2; exit 1; }

for obrigatoria in OCINYE_STORAGE_ENDPOINT_URL OCINYE_STORAGE_ACCESS_KEY \
                   OCINYE_STORAGE_SECRET_KEY OCINYE_STORAGE_BUCKET; do
  eval "valor=\${$obrigatoria:-}"
  [ -n "$valor" ] || fatal "$obrigatoria não está definida."
done

command -v mc >/dev/null 2>&1 || fatal "o cliente 'mc' não está instalado."

codificar() {
  printf '%s' "$1" | od -An -tx1 -v | tr ' ' '\n' | grep -v '^$' | while read -r byte; do
    case "$byte" in
      2d|2e|5f|7e|3[0-9]|4[1-9a-f]|5[0-9a]|6[1-9a-f]|7[0-9a]) printf '%b' "\\x$byte" ;;
      *) printf '%%%s' "$(printf '%s' "$byte" | tr '[:lower:]' '[:upper:]')" ;;
    esac
  done
}

ESQUEMA="$(printf '%s' "$OCINYE_STORAGE_ENDPOINT_URL" | sed 's#://.*##')"
ANFITRIAO="$(printf '%s' "$OCINYE_STORAGE_ENDPOINT_URL" | sed 's#^[a-z]*://##')"
MC_HOST_ocinyeorigem="$ESQUEMA://$(codificar "$OCINYE_STORAGE_ACCESS_KEY"):$(codificar "$OCINYE_STORAGE_SECRET_KEY")@$ANFITRIAO"
export MC_HOST_ocinyeorigem

case "${1:-}" in
  mirror)
    [ $# -eq 2 ] || fatal "uso: mirror DESTINO"
    mkdir -p "$2"
    mc mirror --quiet --overwrite \
      "ocinyeorigem/$OCINYE_STORAGE_BUCKET" "$2" >/dev/null \
      || fatal "a cópia dos objectos falhou."
    ;;
  *)
    fatal "operação desconhecida: ${1:-}. Use mirror."
    ;;
esac
