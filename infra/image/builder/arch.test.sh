#!/usr/bin/env bash
# Deterministic, network-free tests for the builder/target architecture model
# (D013). Proves the three architectures stay independent:
#   * the ocinye-image-builder triple follows the BUILDER VM arch,
#   * the target binaries' triple follows the TARGET arch,
#   * syft follows the BUILDER VM arch,
# and that every selection fails closed on an unknown architecture. Exits
# non-zero on any failure.
set -uo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
arch_sh="$here/arch.sh"
install_syft="$here/install-syft.sh"
sums="$here/syft.sha256"
fails=0

check() { # label  expected  actual
  if [ "$2" = "$3" ]; then echo "ok   $1"; else echo "FAIL $1: expected '$2', got '$3'"; fails=$((fails + 1)); fi
}
check_fail() { # label  cmd...
  local label="$1"; shift
  if "$@" >/dev/null 2>&1; then echo "FAIL $label: expected fail-closed, succeeded"; fails=$((fails + 1)); else echo "ok   $label"; fi
}

# --- Canonical normalisation (shared primitive) ---
check "canonical x86_64 -> amd64"  amd64 "$("$arch_sh" canonical x86_64)"
check "canonical amd64  -> amd64"  amd64 "$("$arch_sh" canonical amd64)"
check "canonical aarch64 -> arm64" arm64 "$("$arch_sh" canonical aarch64)"
check "canonical arm64   -> arm64" arm64 "$("$arch_sh" canonical arm64)"
check_fail "canonical riscv64 fails closed" "$arch_sh" canonical riscv64
check_fail "canonical ppc64le fails closed" "$arch_sh" canonical ppc64le

# --- Builder binary triple follows the BUILDER VM arch (A, B, C, D) ---
check "A builder x86_64  -> x86_64-unknown-linux-musl"  x86_64-unknown-linux-musl  "$("$arch_sh" triple x86_64)"
check "B builder aarch64 -> aarch64-unknown-linux-musl" aarch64-unknown-linux-musl "$("$arch_sh" triple aarch64)"
check "C builder arm64   -> aarch64-unknown-linux-musl" aarch64-unknown-linux-musl "$("$arch_sh" triple arm64)"
check_fail "D builder riscv64 fails closed" "$arch_sh" triple riscv64

# --- Target binaries' triple follows the TARGET arch (F, G) ---
check "F target amd64 -> x86_64-unknown-linux-musl"  x86_64-unknown-linux-musl  "$("$arch_sh" triple amd64)"
check "G target arm64 -> aarch64-unknown-linux-musl" aarch64-unknown-linux-musl "$("$arch_sh" triple arm64)"

# --- E: TARGET arch does not force the BUILDER binary arch ---
# The builder triple is a function of the builder arch alone; a target env of
# amd64 cannot change the arm64 builder's triple.
got="$(OCINYE_IMAGE_TARGET_ARCH=amd64 "$arch_sh" triple aarch64)"
check "E builder arm64 stays arm64 regardless of target=amd64" aarch64-unknown-linux-musl "$got"

# --- H: syft selection is builder-architecture based ---
# install-syft.sh derives its archive from the builder arch via arch.sh canonical.
syft_archive() { # builder uname -> syft archive name
  local m="$1" c
  c="$(OCINYE_ARCH="$m" "$arch_sh" canonical)" || return 2
  echo "syft_1.54.0_linux_${c}.tar.gz"
}
check "H syft(x86_64)  archive amd64" syft_1.54.0_linux_amd64.tar.gz "$(syft_archive x86_64)"
check "H syft(aarch64) archive arm64" syft_1.54.0_linux_arm64.tar.gz "$(syft_archive aarch64)"
# And every chosen syft archive is pinned in the single source of truth.
for m in x86_64 aarch64; do
  a="$(syft_archive "$m")"
  if awk -v a="$a" '$2 == a { f = 1 } END { exit f ? 0 : 1 }' "$sums"; then
    echo "ok   $a pinned in syft.sha256"
  else
    echo "FAIL $a missing from syft.sha256"; fails=$((fails + 1))
  fi
done
# install-syft.sh itself fails closed on an unknown builder arch.
check_fail "install-syft.sh riscv64 fails closed" env OCINYE_BUILDER_UNAME_M=riscv64 bash "$install_syft"

if [ "$fails" -ne 0 ]; then
  echo "arch.test.sh: $fails failure(s)" >&2
  exit 1
fi
echo "arch.test.sh: all passed"
