#!/usr/bin/env bash
# Traz para o conjunto de continuidade os bytes institucionais — e devolve-os
# ao armazenamento num restauro.
#
# # O cliente é o rclone
#
# Era o `mc` do MinIO. O MinIO foi arquivado e o `mc` deixou de ser servido,
# o que partiu o backup nocturno em Setembro de 2026. O rclone é mantido, fala
# S3 com qualquer implementação, e configura-se só por ambiente — sem ficheiro,
# sem credenciais em URL (ADR-0208).
#
# # A origem é a configuração do Core
#
# Não se escreve outra vez onde vivem os bytes: lê-se a mesma configuração que o
# Core usa. Duas descrições do mesmo armazenamento seriam um sítio onde
# discordar, e o backup copiaria um bucket que já não é o da instituição.
#
# # Configuração
#
#     OCINYE_STORAGE_ENDPOINT_URL   a mesma que o Core lê
#     OCINYE_STORAGE_ACCESS_KEY
#     OCINYE_STORAGE_SECRET_KEY
#     OCINYE_STORAGE_BUCKET
#     OCINYE_STORAGE_REGION         por omissão us-east-1
#
# Uso:
#     backup-objects.sh mirror  PASTA   do bucket para a pasta
#     backup-objects.sh restore PASTA   da pasta para o bucket
set -eu

fatal() { printf 'backup-objects: %s\n' "$1" >&2; exit 1; }

for obrigatoria in OCINYE_STORAGE_ENDPOINT_URL OCINYE_STORAGE_ACCESS_KEY \
                   OCINYE_STORAGE_SECRET_KEY OCINYE_STORAGE_BUCKET; do
  eval "valor=\${$obrigatoria:-}"
  [ -n "$valor" ] || fatal "$obrigatoria não está definida."
done
command -v rclone >/dev/null 2>&1 || fatal "o rclone não está instalado."

# O remoto «instituicao», descrito só por ambiente: nada no disco, nada na linha
# de comandos que um `ps` mostrasse.
export RCLONE_CONFIG_INSTITUICAO_TYPE=s3
export RCLONE_CONFIG_INSTITUICAO_PROVIDER=Other
export RCLONE_CONFIG_INSTITUICAO_ENDPOINT="$OCINYE_STORAGE_ENDPOINT_URL"
export RCLONE_CONFIG_INSTITUICAO_ACCESS_KEY_ID="$OCINYE_STORAGE_ACCESS_KEY"
export RCLONE_CONFIG_INSTITUICAO_SECRET_ACCESS_KEY="$OCINYE_STORAGE_SECRET_KEY"
export RCLONE_CONFIG_INSTITUICAO_REGION="${OCINYE_STORAGE_REGION:-us-east-1}"
export RCLONE_CONFIG_INSTITUICAO_FORCE_PATH_STYLE=true
export RCLONE_CONFIG=/dev/null

case "${1:-}" in
  mirror)
    [ $# -eq 2 ] || fatal "uso: mirror DESTINO"
    mkdir -p "$2"
    rclone copy --quiet "instituicao:$OCINYE_STORAGE_BUCKET" "$2" \
      || fatal "a cópia dos objectos falhou."
    # A cópia confere-se contra a origem, ficheiro a ficheiro, e não se presume
    # por o comando ter saído zero.
    rclone check --quiet --one-way "instituicao:$OCINYE_STORAGE_BUCKET" "$2" \
      || fatal "a cópia dos objectos não confere com o bucket."
    ;;
  restore)
    [ $# -eq 2 ] || fatal "uso: restore ORIGEM"
    [ -d "$2" ] || fatal "«$2» não é uma pasta."
    rclone copy --quiet "$2" "instituicao:$OCINYE_STORAGE_BUCKET" \
      || fatal "a reposição dos objectos falhou."
    rclone check --quiet --one-way "$2" "instituicao:$OCINYE_STORAGE_BUCKET" \
      || fatal "os objectos repostos não conferem com o conjunto."
    ;;
  *)
    fatal "operação desconhecida: ${1:-}. Use mirror ou restore."
    ;;
esac
