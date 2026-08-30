#!/usr/bin/env bash
# Leva um commit exacto da `main` canónica até ao servidor de produção.
#
# # O que viaja
#
# Só conteúdo versionado, produzido por `git archive` a partir de um commit.
# Um `scp -r` da pasta de trabalho levaria consigo `target/`, o `.env`, o
# scratchpad, caches e artefactos locais — e a instituição passaria a correr
# aquilo que estava por acaso no disco de quem fez o deploy.
#
# # O que identifica um release
#
# O SHA do commit. É o nome da pasta no servidor, a etiqueta das imagens, e o
# valor de `OCINYE_RELEASE_SHA` na configuração. `docker images` passa a
# responder à pergunta «o que está em produção?» sem depender de memória.
#
# # O que este script **não** faz
#
# Não faz deploy de uma branch. Produção corre `main`, e uma árvore suja ou um
# commit que não está em `origin/main` são recusados aqui — não descobertos
# depois, no servidor.
set -euo pipefail

SERVIDOR="${OCINYE_DEPLOY_HOST:-ocinye@195.20.246.118}"
RAIZ="${OCINYE_DEPLOY_ROOT:-/srv/ocinye}"
CHAVE="${OCINYE_DEPLOY_KEY:-$HOME/.ssh/id_ed25519_fm65}"
COMPOSE="infra/compose/docker-compose.production.yml"

ssh_() { ssh -i "$CHAVE" -o IdentitiesOnly=yes -o BatchMode=yes "$SERVIDOR" "$@"; }

fatal() { printf '\n  RECUSADO — %s\n\n' "$1" >&2; exit 1; }
passo()  { printf '\n== %s ==\n' "$1"; }

# ── 1. O candidato ──────────────────────────────────────────────────────
passo "O commit que vai para produção"

[ -z "$(git status --porcelain)" ] \
  || fatal "a árvore tem alterações por comprometer.
  Um release construído a partir de uma árvore suja não é reproduzível: o que
  ficaria em produção não corresponde a commit nenhum."

git fetch origin main --quiet
SHA="$(git rev-parse origin/main)"
CURTO="${SHA:0:12}"

git merge-base --is-ancestor "$SHA" HEAD 2>/dev/null \
  || git rev-parse HEAD | grep -q "$SHA" \
  || echo "  aviso: a árvore local não está em origin/main; o release sai de origin/main."

echo "  origin/main    $SHA"
echo "  release        $CURTO"

# ── 2. O pacote ─────────────────────────────────────────────────────────
passo "O pacote"
PACOTE="$(mktemp -d)/ocinye-$CURTO.tar.gz"
git archive --format=tar.gz --output="$PACOTE" "$SHA"
SOMA="$(shasum -a 256 "$PACOTE" | awk '{print $1}')"
echo "  $(basename "$PACOTE")  $(du -h "$PACOTE" | cut -f1)"
echo "  sha256  $SOMA"

# ── 3. A viagem ─────────────────────────────────────────────────────────
passo "A transferência"
ssh_ "install -d -m 755 '$RAIZ/releases/$CURTO'"
scp -q -i "$CHAVE" -o IdentitiesOnly=yes "$PACOTE" "$SERVIDOR:$RAIZ/releases/$CURTO.tar.gz"

# A soma confere-se **do lado de lá**. Uma transferência que corre sem erro não
# é uma transferência que chegou inteira, e é do destino que isso se sabe.
LA="$(ssh_ "sha256sum '$RAIZ/releases/$CURTO.tar.gz' | awk '{print \$1}'")"
[ "$LA" = "$SOMA" ] || fatal "o que chegou ao servidor não é o que saiu.
  aqui:     $SOMA
  servidor: $LA"
echo "  soma confirmada no destino"

ssh_ "tar -xzf '$RAIZ/releases/$CURTO.tar.gz' -C '$RAIZ/releases/$CURTO' \
      && rm -f '$RAIZ/releases/$CURTO.tar.gz'"

# ── 4. Construir ────────────────────────────────────────────────────────
passo "Construir as imagens"
ssh_ "cd '$RAIZ/releases/$CURTO' \
      && OCINYE_RELEASE_SHA='$CURTO' docker compose -f '$COMPOSE' build"

# ── 5. O apontador ──────────────────────────────────────────────────────
#
# `current` só muda depois de as imagens existirem. Apontar primeiro e construir
# depois deixaria uma janela em que `current` promete uma coisa que ainda não
# está pronta — e é nessa janela que um reboot acontece.
passo "Trocar o release corrente"
ANTERIOR="$(ssh_ "readlink '$RAIZ/current' 2>/dev/null || true")"
ssh_ "ln -sfn '$RAIZ/releases/$CURTO' '$RAIZ/current' \
      && echo 'OCINYE_RELEASE_SHA=$CURTO' > /etc/ocinye/release.env" 2>/dev/null \
  || ssh_ "ln -sfn '$RAIZ/releases/$CURTO' '$RAIZ/current' \
      && sudo sh -c 'echo OCINYE_RELEASE_SHA=$CURTO > /etc/ocinye/release.env'"
[ -n "$ANTERIOR" ] && echo "  anterior  $ANTERIOR"
echo "  corrente  $RAIZ/releases/$CURTO"

# ── 6. Levantar ─────────────────────────────────────────────────────────
passo "Levantar"
ssh_ "cd '$RAIZ/current' \
      && OCINYE_RELEASE_SHA='$CURTO' docker compose -f '$COMPOSE' up -d --remove-orphans"

passo "Saúde"
ssh_ "cd '$RAIZ/current' && OCINYE_RELEASE_SHA='$CURTO' docker compose -f '$COMPOSE' ps"

printf '\n  Release %s em produção.\n\n' "$CURTO"
