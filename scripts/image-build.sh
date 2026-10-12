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
ARCH_SH="$RAIZ/infra/image/builder/arch.sh"
mkdir -p "$CACHE/dev-signing" "$CACHE/dist"
chmod 700 "$CACHE/dev-signing"

# The builder VM's architecture — the Lima `arch:` field, the single source of
# truth for where ocinye-image-builder and syft run. It decides the BUILDER
# binary triple; the Ocinye target arch (--arch) never does.
builder_vm_arch() { awk '/^arch:/ { print $2; exit }' "$RAIZ/infra/image/builder/lima-image-builder.yaml"; }
# The ocinye-image-builder executable runs inside the builder VM, so its triple
# follows the builder VM arch (fail-closed on unknown via arch.sh).
btriple="$("$ARCH_SH" triple "$(builder_vm_arch)")"

musl() { # alvo
  local t="$1" up
  up="$(echo "$t" | tr 'a-z-' 'A-Z_')"
  env "CARGO_TARGET_${up}_LINKER=rust-lld" cargo build --release --locked --target "$t" \
    -p ocinye-firstboot -p ocinye-oie -p ocinye-image-builder \
    -p ocinye-system-service -p ocinye-session-service --manifest-path "$RAIZ/Cargo.toml" >&2
}

# O compositor e a interface do sistema ligam a pilha gráfica do sistema de
# base (WebKitGTK, Mesa, libseat): constroem-se num contentor dessa base, para
# a arquitectura do ALVO, e ficam ao lado dos binários estáticos. O Docker já é
# pré-requisito do pacote de release; nada mais é pedido a esta máquina.
seat() { # arch alvo
  local arch="$1" alvo="$2" img="ocinye-system-ui-build:$1"
  docker build --platform "linux/$arch" -t "$img" "$RAIZ/infra/system-ui/build" >&2
  mkdir -p "$RAIZ/target/$alvo/release"
  docker run --rm --platform "linux/$arch" \
    -v "$RAIZ":/src -v "ocinye-cargo-registry:/opt/cargo/registry" \
    -v "ocinye-seat-target-$arch:/target" -v "$RAIZ/target/$alvo/release":/out \
    "$img" sh -euc '
      cd /src/apps/compositor && CARGO_TARGET_DIR=/target/compositor cargo build --release --locked
      cd /src/apps/system-ui && CARGO_TARGET_DIR=/target/system-ui cargo build --release --locked --features tauri/custom-protocol
      # The Update Engine links aws-lc (through tough): built on this base too.
      cd /src && CARGO_TARGET_DIR=/target/engine cargo build --release --locked -p ocinye-update-engine
      install -m 0755 /target/compositor/release/ocinye-compositor /target/system-ui/release/ocinye-system-ui \
        /target/engine/release/ocinye-update-engine /out/
    ' >&2
}

vm_existe() { limactl list -q 2>/dev/null | grep -qx "$VM"; }

# The image's release as an update (ADR-0032). The development channel's keys
# live in ~/.cache/ocinye-update/dev-keys (0700), generated once, never in git
# nor in an image; the local repository the images' releases are published to
# is ~/.cache/ocinye-update/repo. Every image build publishes its release under
# the next number; the image carries the channel's first root (public keys).
UPDATE_CACHE="$HOME/.cache/ocinye-update"
publish_update() { # pacote arch alvo out_dir
  local pacote="$1" arch="$2" alvo="$3" out="$4" rel id rev num v
  cargo build --release --locked -p ocinye-release-tool --bin ocinye-release --manifest-path "$RAIZ/Cargo.toml" >&2
  rel="$RAIZ/target/release/ocinye-release"
  if [ ! -d "$UPDATE_CACHE/dev-keys" ]; then
    "$rel" keys init "$UPDATE_CACHE/dev-keys" >&2
    "$rel" root create --keys "$UPDATE_CACHE/dev-keys" --out "$UPDATE_CACHE/root-development.json" --days 730
    "$rel" channel init --repo "$UPDATE_CACHE/repo" --channel development --root "$UPDATE_CACHE/root-development.json"
  fi
  id="$(tr -d '[:space:]' < "$BUNDLES/$pacote/RELEASE")"
  # Next number on the channel, next revision of this release id.
  local listing; listing="$("$rel" verify --repo "$UPDATE_CACHE/repo" --channel development \
    --root "$UPDATE_CACHE/repo/development/metadata/1.root.json" 2>/dev/null || true)"
  num=$(( $(printf '%s\n' "$listing" | awk '/number/ {print $3}' | sort -n | tail -1 | sed 's/^$/0/') + 1 ))
  rev=$(( $(printf '%s\n' "$listing" | grep -c "^$id-r" || true) + 1 ))
  v="$id-r$rev"
  mkdir -p "$out"
  "$RAIZ/scripts/update-native.sh" "$RAIZ/target/$alvo/release" "$v" "$num" development "$arch" "$out/native.tar" >&2
  rm -rf "$UPDATE_CACHE/staged/$v"
  "$rel" stage --bundle "$BUNDLES/$pacote" --native "$out/native.tar" --out "$UPDATE_CACHE/staged/$v" \
    --channel development --number "$num" --minimum 0 --revision "$rev" \
    --migrations "${OCINYE_UPDATE_MIGRATIONS:-expand}" \
    --notes-pt "${OCINYE_UPDATE_NOTES_PT:-Compilação de desenvolvimento $v.}" \
    --notes-en "${OCINYE_UPDATE_NOTES_EN:-Development build $v.}" \
    --notes-fr "${OCINYE_UPDATE_NOTES_FR:-Compilation de développement $v.}" >&2
  "$rel" publish --repo "$UPDATE_CACHE/repo" --channel development --keys "$UPDATE_CACHE/dev-keys" \
    --staged "$UPDATE_CACHE/staged/$v" >&2
  rm -rf "$UPDATE_CACHE/staged/$v"
  cp "$UPDATE_CACHE/repo/development/metadata/1.root.json" "$out/root.json"
  printf 'UPDATE_VERSION=%s\nUPDATE_CHANNEL=development\n' "$v" > "$out/update.env"
  echo "$v"
}

vm_create() {
  vm_existe && return 0
  limactl create --name "$VM" --tty=false \
    --set ".mounts[0].location = \"$RAIZ\"" \
    "$RAIZ/infra/image/builder/lima-image-builder.yaml"
  limactl start "$VM"
}

# syft, installed for the BUILDER VM's own architecture (not the Ocinye target).
# Idempotent; reads the pinned soma from the mounted repo. Run after the VM is up.
ensure_syft() {
  limactl shell "$VM" sudo -n bash "$RAIZ/infra/image/builder/install-syft.sh"
}

case "${1:-}" in
  build)
    pacote="${2:?PACOTE}"; arch="${3:?amd64|arm64}"; formatos="${4:-qcow2,raw,iso}"
    # Target binaries (ocinye-firstboot, ocinye-oie) follow the TARGET arch.
    alvo="$("$ARCH_SH" triple "$arch")"
    [ -d "$BUNDLES/$pacote" ] || { echo "pacote não encontrado: $BUNDLES/$pacote" >&2; exit 2; }
    [ -z "$(git -C "$RAIZ" status --porcelain)" ] || { echo "DIRTY_SOURCE_TREE: a árvore tem alterações" >&2; exit 3; }
    commit="$(git -C "$RAIZ" rev-parse HEAD)"
    # A build needs room for the artifacts here and for the VM's scratch:
    # refuse before starting rather than fill the disk (ENOSPC is INVALID).
    livre=$(df -g "$HOME" | tail -1 | awk '{print $4}')
    [ "$livre" -ge "${OCINYE_IMAGE_BUILD_MIN_FREE_GB:-30}" ] || { echo "ESPAÇO INSUFICIENTE: ${livre} GB livres" >&2; exit 4; }
    # Target-arch binaries for the image, then the builder binary for the VM arch.
    musl "$alvo"; musl "$btriple"
    seat "$arch" "$alvo"
    nome="$pacote-$arch-$(date -u +%Y%m%dT%H%M%SZ)"
    update_dir="$CACHE/update-input/$nome"
    versao="$(publish_update "$pacote" "$arch" "$alvo" "$update_dir")"
    echo "release $versao published on the development channel" >&2
    vm_create
    limactl start "$VM" >/dev/null 2>&1 || true
    ensure_syft
    limactl shell "$VM" sudo -n "$RAIZ/target/$btriple/release/ocinye-image-builder" build \
      --bundle "$BUNDLES/$pacote" --arch "$arch" \
      --out "$CACHE/dist/$nome" --work "/var/lib/ocinye-image-build/work-$arch" \
      --cache "$CACHE" --image-dir "$RAIZ/infra/image" \
      --bin-dir "$RAIZ/target/$alvo/release" --update-dir "$update_dir" --builder-commit "$commit" \
      --signing-dir "$CACHE/dev-signing" --formats "$formatos" \
      --memory "${OCINYE_IMAGE_BUILD_MEMORY:-2560}" --cpus "${OCINYE_IMAGE_BUILD_CPUS:-3}" ${OCINYE_IMAGE_BUILD_REUSE_COMMON:+--reuse-common}
    echo "$CACHE/dist/$nome"
    ;;
  verify)
    nome="${2:?NOME}"; arch="${3:?arch}"
    limactl shell "$VM" sudo -n "$RAIZ/target/$btriple/release/ocinye-image-builder" verify \
      --out "$CACHE/dist/$nome" --signing-dir "$CACHE/dev-signing" --arch "$arch"
    ;;
  vm-create) vm_create; limactl start "$VM" >/dev/null 2>&1 || true; ensure_syft ;;
  vm-destroy) vm_existe && limactl delete -f "$VM" ;;
  vm-shell) limactl shell "$VM" ;;
  *) sed -n '2,15p' "$0"; exit 2 ;;
esac
