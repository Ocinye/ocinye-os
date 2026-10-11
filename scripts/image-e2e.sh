#!/usr/bin/env bash
# Certificação local das imagens do Ocinye OS (D013, fase A), dentro da VM do
# construtor (scripts/image-build.sh vm-create): QCOW2 (dois clones), RAW e ISO
# (instalação offline num disco em branco, com discos a proteger ao lado).
#
#   scripts/image-e2e.sh virt|raw|iso|all NOME amd64|arm64
#
# NOME é a pasta em ~/.cache/ocinye-image-builder/dist. Os resultados ficam em
# ~/.cache/ocinye-image-builder/e2e/NOME/result-*.json, uma linha RESULT por
# cenário: PASS, FAIL ou INVALID (falha do banco de ensaio, nunca um PASS).
set -euo pipefail
RAIZ="$(cd "$(dirname "$0")/.." && pwd)"
VM=ocinye-image-builder
CACHE="$HOME/.cache/ocinye-image-builder"
cenario="${1:?virt|raw|iso|all}"; nome="${2:?NOME}"; arch="${3:?amd64|arm64}"
dist="$CACHE/dist/$nome"
[ -f "$dist/IMAGE_MANIFEST.json" ] || { echo "sem IMAGE_MANIFEST.json em $dist" >&2; exit 2; }
stem="$(python3 -c "import json,sys;m=json.load(open(sys.argv[1]));i=m['image'];print(f\"ocinye-os-{i['release_id']}-r{i['revision']}{'-dev' if i['build_kind']=='development' else ''}-{m['architecture']}\")" "$dist/IMAGE_MANIFEST.json")"
raw_sha="$(python3 -c "import json,sys;m=json.load(open(sys.argv[1]));print(next(a['uncompressed_sha256'] for a in m['artifacts'] if a['format']=='raw'))" "$dist/IMAGE_MANIFEST.json" 2>/dev/null || true)"
work="/var/lib/ocinye-image-build/e2e/$nome"
corre() {
  limactl shell "$VM" sudo -n python3 "$RAIZ/infra/image/e2e/image_e2e.py" "$@" --arch "$arch" --work "$work/$1"
}
rc=0
quer() { [ "$cenario" = "$1" ] || [ "$cenario" = all ]; }
if quer virt; then corre virt --qcow2 "$dist/$stem.qcow2" || rc=1; fi
if quer raw; then corre raw --raw-zst "$dist/$stem.raw.zst" --raw-sha256 "$raw_sha" || rc=1; fi
if quer iso; then corre iso --iso "$dist/$stem.iso" || rc=1; fi
mkdir -p "$CACHE/e2e/$nome"
# Copy the verdicts and serial logs out, and — for a failed install — the
# OFFLINE diagnostics the harness gathered (curtin's saved log/config, the ESP
# tree, fstab, the boot trees). The preserved target.qcow2 is large and stays in
# the builder VM's work directory ($work/*/failure/target.qcow2); the diagnostics
# are what must survive the VM being destroyed, so they always come out.
limactl shell "$VM" sudo -n sh -c "
  cp $work/*/result-*.json $work/*/*-serial.log '$CACHE/e2e/$nome/' 2>/dev/null
  for f in $work/*/failure $work/*/failure-*; do
    [ -d \"\$f\" ] || continue
    s=\$(basename \"\$(dirname \"\$f\")\")-\$(basename \"\$f\")
    mkdir -p '$CACHE/e2e/$nome/'\"\$s\"
    cp -a \"\$f\"/diagnostics '$CACHE/e2e/$nome/'\"\$s\"/ 2>/dev/null
  done
  true
"
exit $rc
