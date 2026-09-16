#!/usr/bin/env bash
# Repõe um release anterior em produção, depressa e com autoridade mínima.
#
# # Porque isto existe
#
# Um deploy pode falhar de uma forma que só aparece no servidor — uma imagem
# construída do stage errado, um serviço que não fica saudável. Quando isso
# acontece, corrigir-para-a-frente é reconstruir tudo (dezenas de minutos), e é
# tempo com produção em baixo. Um release anterior já está no servidor, com as
# imagens já construídas: repô-lo é trocar um symlink e levantar os contentores —
# segundos, não minutos.
#
# # O que este script **não** é
#
# Não é acesso arbitrário à máquina. Toca em três coisas e mais nada: o symlink
# `current`, o `release.env`, e `docker compose up` sobre um release e imagens que
# **já existem** no servidor. Não reconstrói, não faz `git`, não corre comandos
# fora deste conjunto. É a operação estreita de reverter, não uma consola remota.
#
# # Uso
#
#   scripts/rollback-production.sh --list        # (só leitura) que releases há
#   scripts/rollback-production.sh               # repõe o release anterior
#   scripts/rollback-production.sh <sha|curto>   # repõe um release específico
set -euo pipefail

SERVIDOR="${OCINYE_DEPLOY_HOST:-ocinye@195.20.246.118}"
RAIZ="${OCINYE_DEPLOY_ROOT:-/srv/ocinye}"
CHAVE="${OCINYE_DEPLOY_KEY:-$HOME/.ssh/id_ed25519_fm65}"

ssh_() { ssh -i "$CHAVE" -o IdentitiesOnly=yes -o BatchMode=yes "$SERVIDOR" "$@"; }
fatal() { printf '\n  RECUSADO — %s\n\n' "$1" >&2; exit 1; }
passo() { printf '\n== %s ==\n' "$1"; }

# O basename do release corrente, e a lista de releases por data (o mais recente
# primeiro). Uma só ida ao servidor, só de leitura.
ler_estado() {
  ssh_ "
    corrente=\$(basename \"\$(readlink '$RAIZ/current' 2>/dev/null || echo -)\")
    echo \"CORRENTE=\$corrente\"
    echo RELEASES=
    ls -1t '$RAIZ/releases' 2>/dev/null | grep -v '\.tar\.gz\$' || true
  "
}

if [ "${1:-}" = "--list" ]; then
  passo "Releases no servidor"
  ler_estado
  exit 0
fi

ALVO="${1:-}"

passo "Estado actual"
ESTADO="$(ler_estado)"
CORRENTE="$(printf '%s\n' "$ESTADO" | sed -n 's/^CORRENTE=//p')"
RELEASES="$(printf '%s\n' "$ESTADO" | sed -n '/^RELEASES=/,$p' | tail -n +2)"
echo "  corrente  $CORRENTE"

[ -n "$CORRENTE" ] && [ "$CORRENTE" != "-" ] || fatal "não há release corrente para reverter."

# Sem argumento: o alvo é o release anterior — o mais recente que não é o corrente.
if [ -z "$ALVO" ]; then
  ALVO="$(printf '%s\n' "$RELEASES" | grep -v "^$CORRENTE$" | head -n 1)"
  [ -n "$ALVO" ] || fatal "não há release anterior ao corrente para onde reverter."
  echo "  anterior  $ALVO"
fi

# Normaliza um SHA curto/longo para o basename exacto de um release existente.
ALVO_DIR="$(printf '%s\n' "$RELEASES" | grep -m1 "^$ALVO" || true)"
[ -n "$ALVO_DIR" ] || fatal "não há release «$ALVO» no servidor. Corre --list para ver os que há."
[ "$ALVO_DIR" != "$CORRENTE" ] || fatal "«$ALVO_DIR» já é o release corrente."

passo "Verificar o alvo"
COMPOSE="$RAIZ/releases/$ALVO_DIR/infra/compose/docker-compose.production.yml"
# O release tem de ter o seu compose e as imagens já construídas — não se
# reconstrói nada aqui.
ssh_ "
  test -f '$COMPOSE' || { echo 'SEM-COMPOSE'; exit 3; }
  for s in core-server workspace worker; do
    docker image inspect \"ocinye/ocinye-\$s:$ALVO_DIR\" >/dev/null 2>&1 \
      || { echo \"SEM-IMAGEM ocinye/ocinye-\$s:$ALVO_DIR\"; exit 4; }
  done
  echo OK
" | tail -1 | grep -q '^OK$' || fatal "o release «$ALVO_DIR» não está pronto a levantar (falta o compose ou uma imagem)."
echo "  alvo      $ALVO_DIR  (compose e imagens presentes)"

passo "Reverter"
INICIO=$(date +%s)
ssh_ "
  set -e
  ln -sfn '$RAIZ/releases/$ALVO_DIR' '$RAIZ/current'
  ( echo 'OCINYE_RELEASE_SHA=$ALVO_DIR' > /etc/ocinye/release.env ) 2>/dev/null \
    || sudo sh -c \"echo OCINYE_RELEASE_SHA=$ALVO_DIR > /etc/ocinye/release.env\"
  cd '$RAIZ/current'
  OCINYE_RELEASE_SHA='$ALVO_DIR' docker compose -f '$COMPOSE' up -d --remove-orphans
"

passo "Esperar pela saúde do Core"
# O Core é a dependência de que a workspace espera; se ele fica saudável, a
# reversão pegou. Espera-se com prazo, para o RTO ser medido e não infinito.
SAUDAVEL=nao
for _ in $(seq 1 40); do
  estado="$(ssh_ "docker inspect --format '{{.State.Health.Status}}' ocinye-core-1 2>/dev/null || echo -")"
  if [ "$estado" = "healthy" ]; then SAUDAVEL=sim; break; fi
  sleep 3
done
FIM=$(date +%s)
RTO=$((FIM - INICIO))

if [ "$SAUDAVEL" = "sim" ]; then
  printf '\n  Revertido para %s. Core saudável. RTO observado: %ss.\n\n' "$ALVO_DIR" "$RTO"
else
  fatal "revertido para $ALVO_DIR, mas o Core não ficou saudável em ${RTO}s. Investiga com «docker compose ps» no servidor."
fi
