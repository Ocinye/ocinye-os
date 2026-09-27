#!/usr/bin/env bash
# O transporte da cópia institucional para fora deste servidor.
#
# **Bash, e não `/bin/sh`**, por coerência com os irmãos: a codificação de
# credenciais do tempo do `mc` já não existe, mas o shebang que a protegia fica.
#
# # Porque isto existe
#
# Porque a interface anterior era um **comando de shell numa variável de
# ambiente**:
#
#     OCINYE_BACKUP_REMOTE_CMD=… cp --quiet
#     OCINYE_BACKUP_VERIFY_CMD=… cat cofre/…
#
# Três problemas, e nenhum é estético.
#
# O primeiro é que `$CMD "$origem" "$destino"` sem aspas divide por espaços: o
# comando e os seus argumentos ficam à mercê de como alguém escreveu a linha, e
# um valor com espaços sem aspas não define variável nenhuma — cai no
# `rsync -a` por omissão, que copia alegremente para uma pasta **local** com o
# nome do destino e sai zero. Foi assim que uma cópia «fora do servidor» acabou
# dentro da árvore de trabalho e foi declarada feita.
#
# O segundo é o `eval` da confirmação: qualquer coisa que chegue àquela variável
# corre com os privilégios de quem faz o backup.
#
# O terceiro é que a interface não diz o que precisa. «Um comando que copia» não
# é um contrato: não se pode validar, não se pode sondar antes de tentar, e não
# se pode dizer a quem instala o que está em falta.
#
# Aqui a configuração é **estruturada** e as operações são explícitas. Não há
# `eval` e não há interpolação de shell: cada operação invoca o cliente com
# argumentos que este script escreve.
#
# # Configuração
#
#     OCINYE_BACKUP_BACKEND        `s3` ou `none` (omissão: `none`)
#     OCINYE_BACKUP_S3_ENDPOINT    https://…
#     OCINYE_BACKUP_S3_REGION      omissão: us-east-1
#     OCINYE_BACKUP_S3_BUCKET
#     OCINYE_BACKUP_S3_PREFIX      omissão: conjuntos
#     OCINYE_BACKUP_S3_ACCESS_KEY
#     OCINYE_BACKUP_S3_SECRET_KEY
#
# # Operações
#
#     probe                  o destino existe e responde
#     put FICHEIRO NOME      envia
#     read-back NOME         escreve no stdout o que **está lá**
#     list                   os conjuntos no destino, mais recente primeiro
#     prune QUANTOS          aplica retenção no destino
#
# # O cliente é o rclone
#
# Era o `mc` do MinIO, que deixou de ser servido quando o MinIO foi arquivado
# (ADR-0208). O rclone fala S3 com qualquer implementação compatível, é mantido,
# e configura-se só por ambiente (`RCLONE_CONFIG_COFRE_*`) — as credenciais ficam
# fora da linha de comandos e portanto fora do `ps`, como com o `MC_HOST_*`. A
# escolha é uma dependência declarada, e não um comando arbitrário.
set -eu

fatal() { printf 'backup-remote: %s\n' "$1" >&2; exit 1; }

BACKEND="${OCINYE_BACKUP_BACKEND:-none}"
[ "$BACKEND" = none ] && fatal "não há destino remoto configurado (OCINYE_BACKUP_BACKEND=none)."
[ "$BACKEND" = s3 ] || fatal "backend desconhecido: $BACKEND. O único suportado é 's3'."

for obrigatoria in OCINYE_BACKUP_S3_ENDPOINT OCINYE_BACKUP_S3_BUCKET \
                   OCINYE_BACKUP_S3_ACCESS_KEY OCINYE_BACKUP_S3_SECRET_KEY; do
  eval "valor=\${$obrigatoria:-}"
  [ -n "$valor" ] || fatal "$obrigatoria não está definida."
done
PREFIXO="${OCINYE_BACKUP_S3_PREFIX:-conjuntos}"
command -v rclone >/dev/null 2>&1 || fatal "o rclone não está instalado."

export RCLONE_CONFIG_COFRE_TYPE=s3
export RCLONE_CONFIG_COFRE_PROVIDER=Other
export RCLONE_CONFIG_COFRE_ENDPOINT="$OCINYE_BACKUP_S3_ENDPOINT"
export RCLONE_CONFIG_COFRE_ACCESS_KEY_ID="$OCINYE_BACKUP_S3_ACCESS_KEY"
export RCLONE_CONFIG_COFRE_SECRET_ACCESS_KEY="$OCINYE_BACKUP_S3_SECRET_KEY"
export RCLONE_CONFIG_COFRE_REGION="${OCINYE_BACKUP_S3_REGION:-us-east-1}"
export RCLONE_CONFIG_COFRE_FORCE_PATH_STYLE=true
export RCLONE_CONFIG=/dev/null

ALVO="cofre:$OCINYE_BACKUP_S3_BUCKET/$PREFIXO"

case "${1:-}" in
  probe)
    # Sondar antes de tentar: descobrir que as credenciais estão erradas depois
    # de cifrar meio gigabyte é descobri-lo tarde de mais para servir.
    rclone lsf --max-depth 1 "cofre:$OCINYE_BACKUP_S3_BUCKET" >/dev/null 2>&1 \
      || fatal "o destino não respondeu, ou as credenciais não abrem o bucket."
    printf 'destino alcançável: %s/%s\n' "$OCINYE_BACKUP_S3_BUCKET" "$PREFIXO"
    ;;
  put)
    [ $# -eq 3 ] || fatal "uso: put FICHEIRO NOME"
    [ -f "$2" ] || fatal "não existe: $2"
    rclone copyto --quiet "$2" "$ALVO/$3" \
      || fatal "o envio de $3 falhou."
    ;;
  read-back)
    # Lê o que **está lá**. O `put` sair zero não é prova de que chegou: um
    # cliente mal configurado escreve numa pasta local e sai zero na mesma.
    [ $# -eq 2 ] || fatal "uso: read-back NOME"
    rclone cat "$ALVO/$2" 2>/dev/null || fatal "não foi possível ler $2 do destino."
    ;;
  list)
    rclone lsf "$ALVO/" 2>/dev/null | sed 's#/$##' | grep -v '\.sha256$' | sort -r || true
    ;;
  prune)
    [ $# -eq 2 ] || fatal "uso: prune QUANTOS"
    QUANTOS="$2"
    # Nunca apagar o único conjunto válido: uma retenção que deixa o cofre vazio
    # é uma perda, não uma limpeza.
    [ "$QUANTOS" -ge 1 ] || fatal "a retenção tem de manter pelo menos um conjunto."
    VELHOS="$("$0" list | tail -n +$((QUANTOS + 1)))"
    [ -n "$VELHOS" ] || exit 0
    printf '%s\n' "$VELHOS" | while read -r velho; do
      [ -n "$velho" ] || continue
      rclone purge "$ALVO/$velho" >/dev/null 2>&1 \
        || rclone deletefile "$ALVO/$velho" >/dev/null 2>&1 || true
      rclone deletefile "$ALVO/$velho.sha256" >/dev/null 2>&1 || true
      printf 'removido do destino: %s\n' "$velho"
    done
    ;;
  *)
    fatal "operação desconhecida: ${1:-}. Use probe, put, read-back, list ou prune."
    ;;
esac
