#!/usr/bin/env bash
# Constrói imagens de DESENVOLVIMENTO do Ocinye OS (D013, fase A) a partir de um
# pacote de release D011 — numa VM Linux descartável, nunca neste Mac.
#
#   scripts/image-build.sh build  PACOTE amd64|arm64 [qcow2,raw,iso]
#   scripts/image-build.sh verify NOME amd64|arm64      re-verifica SHA256SUMS e a assinatura
#   scripts/image-build.sh vm-create | vm-destroy | vm-shell
#
# PACOTE é um directório de ~/.cache/ocinye-installer-test/bundles. A saída fica
# em ~/.cache/ocinye-image-builder/dist/<nome>/; a chave de desenvolvimento em
# ~/.cache/ocinye-image-builder/dev-signing/ (gerada uma vez, nunca no
# repositório, nunca numa imagem). Canal: só `development` (o estável falha
# fechado: `ocinye-image-builder stable-gate`).
#
# A árvore tem de estar limpa: o commit do construtor fica no manifesto.
set -euo pipefail
RAIZ="$(cd "$(dirname "$0")/.." && pwd)"
VM=ocinye-image-builder
CACHE="$HOME/.cache/ocinye-image-builder"
BUNDLES="$HOME/.cache/ocinye-installer-test/bundles"
mkdir -p "$CACHE/dev-signing" "$CACHE/dist"
chmod 700 "$CACHE/dev-signing"

musl() { # alvo
  local t="$1" up
  up="$(echo "$t" | tr 'a-z-' 'A-Z_')"
  env "CARGO_TARGET_${up}_LINKER=rust-lld" cargo build --release --locked --target "$t" \
    -p ocinye-firstboot -p ocinye-oie -p ocinye-image-builder --manifest-path "$RAIZ/Cargo.toml" >&2
}

vm_existe() { limactl list -q 2>/dev/null | grep -qx "$VM"; }

vm_create() {
  vm_existe && return 0
  limactl create --name "$VM" --tty=false \
    --set ".mounts[0].location = \"$RAIZ\"" \
    "$RAIZ/infra/image/builder/lima-image-builder.yaml"
  limactl start "$VM"
}

case "${1:-}" in
  build)
    pacote="${2:?PACOTE}"; arch="${3:?amd64|arm64}"; formatos="${4:-qcow2,raw,iso}"
    case "$arch" in amd64) alvo=x86_64-unknown-linux-musl ;; arm64) alvo=aarch64-unknown-linux-musl ;; *) exit 2 ;; esac
    [ -d "$BUNDLES/$pacote" ] || { echo "pacote não encontrado: $BUNDLES/$pacote" >&2; exit 2; }
    [ -z "$(git -C "$RAIZ" status --porcelain)" ] || { echo "DIRTY_SOURCE_TREE: a árvore tem alterações" >&2; exit 3; }
    commit="$(git -C "$RAIZ" rev-parse HEAD)"
    musl "$alvo"; musl aarch64-unknown-linux-musl
    vm_create
    limactl start "$VM" >/dev/null 2>&1 || true
    nome="$pacote-$arch-$(date -u +%Y%m%dT%H%M%SZ)"
    limactl shell "$VM" sudo -n "$RAIZ/target/aarch64-unknown-linux-musl/release/ocinye-image-builder" build \
      --bundle "$BUNDLES/$pacote" --arch "$arch" \
      --out "$CACHE/dist/$nome" --work "/var/lib/ocinye-image-build/work-$arch" \
      --cache "$CACHE" --image-dir "$RAIZ/infra/image" \
      --bin-dir "$RAIZ/target/$alvo/release" --builder-commit "$commit" \
      --signing-dir "$CACHE/dev-signing" --formats "$formatos" \
      --memory "${OCINYE_IMAGE_BUILD_MEMORY:-2560}" --cpus "${OCINYE_IMAGE_BUILD_CPUS:-3}"
    echo "$CACHE/dist/$nome"
    ;;
  verify)
    nome="${2:?NOME}"; arch="${3:?arch}"
    limactl shell "$VM" sudo -n "$RAIZ/target/aarch64-unknown-linux-musl/release/ocinye-image-builder" verify \
      --out "$CACHE/dist/$nome" --signing-dir "$CACHE/dev-signing" --arch "$arch"
    ;;
  vm-create) vm_create ;;
  vm-destroy) vm_existe && limactl delete -f "$VM" ;;
  vm-shell) limactl shell "$VM" ;;
  *) sed -n '2,15p' "$0"; exit 2 ;;
esac
