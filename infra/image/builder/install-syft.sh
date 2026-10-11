#!/usr/bin/env bash
# Installs the pinned syft into the DISPOSABLE BUILDER VM.
#
# Architecture rule (D013): syft is a BUILD TOOL. Its architecture follows the
# architecture of the machine where it RUNS — the builder VM — and NEVER the
# architecture of the Ocinye target image being produced. Syft scans a mounted
# target root filesystem as data (`syft scan dir:<rootfs>`); it does not execute
# any binary from that rootfs, so an arm64 syft on an arm64 builder scans an
# amd64 rootfs correctly. Do not install a target-architecture syft here, and do
# not install syft inside the Ocinye image.
#
# Version and per-archive SHA-256 are pinned in the co-located `syft.sha256`,
# the single source of truth. HTTPS only, checksum verified before extraction.
set -euo pipefail

SYFT_VERSION=1.54.0
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
sums="$here/syft.sha256"

# The builder VM's own architecture, canonicalised by the shared helper (which
# fails closed on an unknown arch). `OCINYE_ARCH` overrides `uname -m` for tests.
machine="${OCINYE_BUILDER_UNAME_M:-$(uname -m)}"
syft_arch="$(OCINYE_ARCH="$machine" "$here/arch.sh" canonical)"

archive="syft_${SYFT_VERSION}_linux_${syft_arch}.tar.gz"

sum="$(awk -v a="$archive" '$2 == a { print $1 }' "$sums")"
if [ -z "$sum" ]; then
  echo "NO_PINNED_CHECKSUM: $archive not in $sums" >&2
  exit 3
fi

# Idempotent: a matching syft is already in place after the first provision.
if command -v syft >/dev/null 2>&1 && syft version 2>/dev/null | grep -q "Version:[[:space:]]*${SYFT_VERSION}\b"; then
  exit 0
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"
curl -fsSLo syft.tgz "https://github.com/anchore/syft/releases/download/v${SYFT_VERSION}/${archive}"
echo "${sum}  syft.tgz" | sha256sum -c -
tar -xzf syft.tgz syft
install -m 0755 syft /usr/local/bin/syft
