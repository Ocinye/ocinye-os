#!/usr/bin/env bash
# The update repositories the update scenario of the image E2E serves to an
# installed machine (ADR-0032). Development keys only; nothing here is a
# production release, and every release it adds is marked synthetic.
#
#   scripts/update-e2e-repos.sh PACOTE_N1 amd64|arm64
#
# The image build already published release N (the image's own) to
# ~/.cache/ocinye-update/repo. From there, under ~/.cache/ocinye-update/e2e:
#
#   unhealthy/  N, then BAD   = PACOTE_N1 -r1 whose session service exits at
#                               once: it passes every signature and fails the
#                               health gate, so the machine rolls back
#   good/       unhealthy, then GOOD  = PACOTE_N1 -r2: the real N+1
#   offline/    good, then GOOD2 = PACOTE_N1 -r3, which asks for a restart,
#                               and offline.ocu, its offline bundle
#   foreign/    GOOD2 in a repository whose root is signed by other keys: the
#                               installed machine must refuse it
#   versions.env               N, BAD, GOOD, GOOD2
#
# Each step copies the previous repository, so metadata versions only grow,
# as on a real server. PACOTE_N1 is a release bundle in
# ~/.cache/ocinye-installer-test/bundles built from a commit after N's.
set -euo pipefail
RAIZ="$(cd "$(dirname "$0")/.." && pwd)"
pacote="${1:?PACOTE_N1}"; arch="${2:?amd64|arm64}"
BUNDLES="$HOME/.cache/ocinye-installer-test/bundles"
UPDATE_CACHE="$HOME/.cache/ocinye-update"
OUT="$UPDATE_CACHE/e2e"
[ -d "$BUNDLES/$pacote" ] || { echo "no bundle $BUNDLES/$pacote" >&2; exit 2; }
[ -d "$UPDATE_CACHE/repo/development" ] || { echo "no development repository: build an image first" >&2; exit 2; }
case "$arch" in
  amd64) alvo=x86_64-unknown-linux-musl ;;
  arm64) alvo=aarch64-unknown-linux-musl ;;
  *) echo "amd64 or arm64" >&2; exit 2 ;;
esac
bins="$RAIZ/target/$alvo/release"
cargo build --release --locked -p ocinye-release-tool --bin ocinye-release --manifest-path "$RAIZ/Cargo.toml" >&2
rel="$RAIZ/target/release/ocinye-release"
keys="$UPDATE_CACHE/dev-keys"
root1="$UPDATE_CACHE/repo/development/metadata/1.root.json"
id="$(tr -d '[:space:]' < "$BUNDLES/$pacote/RELEASE")"

# `<version>  number <n>  <bytes> bytes`, one line per release.
listing() { "$rel" verify --repo "$1" --channel development --root "$root1"; }
latest() { listing "$1" | sort -k3 -n | tail -1 | awk '{print $3}'; }
current_n="$(listing "$UPDATE_CACHE/repo" | sort -k3 -n | tail -1 | awk '{print $1}')"
[ -n "$current_n" ] || { echo "release N not found in the repository" >&2; exit 3; }

work="$(mktemp -d)"; trap 'rm -rf "$work"' EXIT
publish() { # repo bin_dir revision what synthetic number [keys]
  local repo="$1" bin="$2" rv="$3" what="$4" synthetic="$5" num="$6" v
  v="$id-r$rv"
  "$RAIZ/scripts/update-native.sh" "$bin" "$v" "$num" development "$arch" "$work/native-$rv.tar" >&2
  rm -rf "$work/staged-$rv"
  "$rel" stage --bundle "$BUNDLES/$pacote" --native "$work/native-$rv.tar" --out "$work/staged-$rv" \
    --channel development --number "$num" --minimum 0 --revision "$rv" --migrations expand \
    --notes-pt "Release de teste $what (sintético)." --notes-en "Test release $what (synthetic)." \
    --notes-fr "Version de test $what (synthétique)." --synthetic "$synthetic" ${REBOOT:+--reboot} >&2
  "$rel" publish --repo "$repo" --channel development --keys "${7:-$keys}" --staged "$work/staged-$rv" >&2
  echo "$v"
}

rm -rf "$OUT"; mkdir -p "$OUT"
# BAD: the same binaries, but a session service that exits at once.
bad_bins="$work/bad-bins"; mkdir -p "$bad_bins"
cp -p "$bins"/ocinye-* "$bad_bins/"
printf '#!/bin/sh\n# Synthetic: an unhealthy release for the rollback test.\nexit 1\n' > "$bad_bins/ocinye-session-service"
chmod 0755 "$bad_bins/ocinye-session-service"
cp -a "$UPDATE_CACHE/repo" "$OUT/unhealthy"
bad="$(publish "$OUT/unhealthy" "$bad_bins" 1 "unhealthy" "rollback: the session service fails the health gate" $(( $(latest "$OUT/unhealthy") + 1 )))"
cp -a "$OUT/unhealthy" "$OUT/good"
good="$(publish "$OUT/good" "$bins" 2 "N+1" "update: N to N+1 with an expand migration" $(( $(latest "$OUT/good") + 1 )))"
cp -a "$OUT/good" "$OUT/offline"
good2="$(REBOOT=1 publish "$OUT/offline" "$bins" 3 "offline" "offline bundle and power loss during install" $(( $(latest "$OUT/offline") + 1 )))"
"$rel" bundle --repo "$OUT/offline" --channel development --version "$good2" --out "$OUT/offline.ocu" >&2
# FOREIGN: a whole repository under keys the machine has never trusted.
"$rel" keys init "$work/foreign-keys" >&2
"$rel" root create --keys "$work/foreign-keys" --out "$work/foreign-root.json" --days 30 >&2
"$rel" channel init --repo "$OUT/foreign" --channel development --root "$work/foreign-root.json" >&2
publish "$OUT/foreign" "$bins" 4 "foreign" "bad signature: a root the machine does not trust" $(( $(latest "$OUT/offline") + 1 )) "$work/foreign-keys" >/dev/null
printf 'N=%s\nBAD=%s\nGOOD=%s\nGOOD2=%s\n' "$current_n" "$bad" "$good" "$good2" > "$OUT/versions.env"
cat "$OUT/versions.env"
