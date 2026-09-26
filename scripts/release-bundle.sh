#!/usr/bin/env bash
# Produz o pacote de instalação de um commit: a árvore, as imagens e as somas.
#
# # O que o pacote é
#
#   ocinye-os-<sha>/
#     RELEASE            o SHA curto
#     SHA256SUMS         a soma de tudo o resto
#     source.tar.gz      `git archive` do commit — nada da pasta de trabalho
#     images/*.tar       as cinco imagens do release, para uma arquitectura
#     install/ocinye     o instalador
#
# O instalador confere as somas antes de escrever uma linha no anfitrião, pelo
# que um pacote alterado no caminho é recusado, e não instalado.
#
# # O que não faz
#
# Não assina. As somas provam que o pacote chegou como saiu; quem o produziu
# prova-se com uma assinatura, que é a Parte 17 (release). Até lá, o pacote viaja
# por um canal em que se confia, e as somas publicam-se à parte.
#
# Uso: scripts/release-bundle.sh [--commit SHA] [--platform linux/amd64|linux/arm64] [--out DIR]
set -euo pipefail

COMMIT="HEAD"
PLATAFORMA=""
SAIDA="dist"
while [ $# -gt 0 ]; do
    case "$1" in
        --commit) COMMIT="$2"; shift 2 ;;
        --platform) PLATAFORMA="$2"; shift 2 ;;
        --out) SAIDA="$2"; shift 2 ;;
        *) echo "opção desconhecida: $1" >&2; exit 2 ;;
    esac
done

fatal() { printf '\n  RECUSADO — %s\n\n' "$1" >&2; exit 1; }

SHA_LONGO="$(git rev-parse --verify "$COMMIT^{commit}")" || fatal "commit desconhecido: $COMMIT"
SHA="${SHA_LONGO:0:12}"
PACOTE="$SAIDA/ocinye-os-$SHA"
TRABALHO="$(mktemp -d)"
trap 'rm -rf "$TRABALHO"' EXIT

rm -rf "$PACOTE"
mkdir -p "$PACOTE/images" "$PACOTE/install"

# A árvore do commit, e só ela.
git archive --format=tar.gz --output="$PACOTE/source.tar.gz" "$SHA_LONGO"
git show "$SHA_LONGO:install/ocinye" > "$PACOTE/install/ocinye"
chmod 755 "$PACOTE/install/ocinye"
printf '%s\n' "$SHA" > "$PACOTE/RELEASE"

# As imagens constroem-se a partir da mesma árvore extraída, e não da pasta de
# trabalho: o que vai no pacote é o commit.
tar -xzf "$PACOTE/source.tar.gz" -C "$TRABALHO"
# Os mesmos stages e argumentos que o Compose de produção nomeia para cada
# serviço — sem o Compose, que exige a configuração de um anfitrião instalado.
construir() {  # imagem  stage  binário
    docker build ${PLATAFORMA:+--platform "$PLATAFORMA"} \
        -f "$TRABALHO/infra/docker/Dockerfile" --target "$2" --build-arg BIN="$3" \
        -t "ocinye/ocinye-$1:$SHA" "$TRABALHO"
}
construir core-server       runtime           ocinye-core-server
construir workspace         runtime           ocinye-workspace
construir worker            runtime           ocinye-worker
construir conversion-runner conversion-runner ocinye-conversion-runner
construir converter         converter         ocinye-convert
for servico in core-server workspace worker conversion-runner converter; do
    docker save -o "$PACOTE/images/ocinye-$servico.tar" "ocinye/ocinye-$servico:$SHA"
done

(cd "$PACOTE" && find . -type f ! -name SHA256SUMS -print0 | sort -z \
    | xargs -0 shasum -a 256 > SHA256SUMS)

printf '\n  Pacote %s\n' "$PACOTE"
du -sh "$PACOTE" | awk '{print "  " $1}'
