#!/usr/bin/env bash
# Pack the native layer of a release (ADR-0032): every Ocinye binary of an
# installed machine, its units, and `release.json`, in one tar the Update
# Engine extracts to /usr/lib/ocinye/releases/<version>/.
#
#   scripts/update-native.sh BIN_DIR VERSION NUMBER CHANNEL ARCH OUT.tar
#
# BIN_DIR holds the target-architecture binaries (target/<triple>/release).
# The units are the `ocinye-*` units of infra/image/rootfs. Deterministic:
# sorted, owner 0, fixed mtime.
set -euo pipefail
RAIZ="$(cd "$(dirname "$0")/.." && pwd)"
bin="${1:?BIN_DIR}"; version="${2:?VERSION}"; number="${3:?NUMBER}"; channel="${4:?CHANNEL}"; arch="${5:?ARCH}"; out="${6:?OUT}"
[[ "$version" =~ ^[0-9a-f]{12}-r[1-9][0-9]*$ ]] || { echo "version is <release id>-r<n>" >&2; exit 2; }
[[ "$number" =~ ^[1-9][0-9]*$ ]] || { echo "number is a positive integer" >&2; exit 2; }
case "$channel" in development|candidate|stable) ;; *) echo "unknown channel" >&2; exit 2 ;; esac
case "$arch" in amd64|arm64) ;; *) echo "unknown arch" >&2; exit 2 ;; esac
work="$(mktemp -d)"; trap 'rm -rf "$work"' EXIT
install -d "$work/bin" "$work/units"
for b in ocinye-system-service ocinye-session-service ocinye-system ocinye-firstboot \
         ocinye-compositor ocinye-system-ui ocinye-update-engine; do
  [ -x "$bin/$b" ] || { echo "missing $bin/$b" >&2; exit 3; }
  install -m 0755 "$bin/$b" "$work/bin/$b"
done
for u in "$RAIZ"/infra/image/rootfs/etc/systemd/system/ocinye-*; do
  install -m 0644 "$u" "$work/units/"
done
printf '{"schema":1,"version":"%s","release_number":%s,"channel":"%s","arch":"%s"}\n' \
  "$version" "$number" "$channel" "$arch" > "$work/release.json"
epoch="${SOURCE_DATE_EPOCH:-$(git -C "$RAIZ" log -1 --format=%ct)}"
if tar --version 2>/dev/null | grep -q GNU; then
  tar -C "$work" --sort=name --owner=0 --group=0 --numeric-owner --mtime="@$epoch" -cf "$out" release.json bin units
else
  # bsdtar (macOS): the same, with its own spelling.
  find "$work" -exec touch -h -t "$(date -r "$epoch" +%Y%m%d%H%M.%S)" {} +
  tar -C "$work" --uid 0 --gid 0 --numeric-owner -cf "$out" release.json bin units
fi
echo "$out"
