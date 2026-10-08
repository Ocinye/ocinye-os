#!/usr/bin/env bash
# Canonical architecture mapping for the Ocinye OS image builder (D013).
#
# THREE independent architectures — never conflate them:
#   * host arch    : the developer machine running scripts/image-build.sh
#   * builder arch : where ocinye-image-builder and syft RUN — the Lima VM
#   * target arch  : the Ocinye image being produced (--arch: amd64 | arm64)
#
# This helper is the single source of the `uname -m` / token normalisation and
# of the Rust target triple, shared by install-syft.sh and image-build.sh so the
# mapping is not duplicated. It fails closed on an unknown architecture; it never
# silently defaults.
#
#   arch.sh canonical [ARCH]   # x86_64|amd64 -> amd64 ; aarch64|arm64 -> arm64
#   arch.sh triple    [ARCH]   # -> x86_64-unknown-linux-musl | aarch64-unknown-linux-musl
#
# ARCH defaults to $OCINYE_ARCH, else `uname -m`.
set -uo pipefail

_canonical() {
  case "$1" in
    x86_64 | amd64) echo amd64 ;;
    aarch64 | arm64) echo arm64 ;;
    *)
      echo "UNSUPPORTED_ARCH: $1 (expected x86_64/amd64 or aarch64/arm64)" >&2
      return 2
      ;;
  esac
}

_triple() {
  case "$(_canonical "$1")" in
    amd64) echo x86_64-unknown-linux-musl ;;
    arm64) echo aarch64-unknown-linux-musl ;;
    *) return 2 ;;
  esac
}

cmd="${1:-}"
arg="${2:-${OCINYE_ARCH:-$(uname -m)}}"
case "$cmd" in
  canonical) _canonical "$arg" ;;
  triple) _triple "$arg" ;;
  *)
    echo "usage: arch.sh canonical|triple [ARCH]" >&2
    exit 2
    ;;
esac
