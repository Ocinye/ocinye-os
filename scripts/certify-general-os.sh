#!/usr/bin/env bash
# A certificação do Ocinye OS de uso geral (Parte 16): as provas todas, no mesmo
# release, por esta ordem, e a primeira que falhar pára tudo.
#
#   1. o pacote de prova do commit corrente
#   2. ./scripts/verify.sh — 125 viagens de browser, testes HTTP, portões
#   3. instalação de raiz, uma por perfil
#   4. actualização N → N+1 e release falhado revertido (precisa de N+1 e de C)
#   5. backup e restauro noutro anfitrião
#   6. certificação de hardware
#
# O mapa passo a passo está em docs/certification/general-os.md.
#
# Uso: scripts/certify-general-os.sh [PACOTE_N PACOTE_N1 PACOTE_FALHA]
#      Sem argumentos constrói só o pacote do commit corrente e salta o passo 4,
#      e di-lo — um passo saltado não é um passo passado.
set -euo pipefail

passo() { printf '\n######## %s ########\n' "$1"; }
SAIDA="${OCINYE_CERTIFY_OUT:-dist}"

passo "1. Pacote de prova"
./scripts/release-bundle.sh --proof --out "$SAIDA"
PACOTE="$SAIDA/ocinye-os-$(git rev-parse --short=12 HEAD)"

passo "2. verify.sh"
./scripts/verify.sh

passo "3. Instalação, por perfil"
./scripts/install-e2e.sh "$PACOTE"

passo "4. Actualização"
if [ $# -eq 3 ]; then
    ./scripts/upgrade-e2e.sh "$1" "$2" "$3"
else
    echo "  NOT_RUN — faltam os pacotes N, N+1 e o release que falha."
    NAO_CORREU=1
fi

passo "5. Backup e restauro"
./scripts/restore-e2e.sh "$PACOTE"

passo "6. Hardware"
./scripts/hardware-certification.sh "$PACOTE"

if [ -n "${NAO_CORREU:-}" ]; then
    printf '\n  Certificação INCOMPLETA: a actualização não correu.\n\n'
    exit 2
fi
printf '\n  Certificação completa para %s.\n\n' "$(git rev-parse --short=12 HEAD)"
