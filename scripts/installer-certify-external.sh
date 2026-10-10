#!/usr/bin/env bash
# Certificação final do Ocinye OS Installer (D011) numa VM EXTERNA: um Ubuntu
# Server 24.04 LTS Minimal `amd64` acabado de criar num fornecedor de cloud.
#
#   scripts/installer-certify-external.sh HOST UTILIZADOR CHAVE PACOTE IMPRESSAO
#
#   HOST        endereço IP da VM
#   UTILIZADOR  conta com sudo sem palavra-passe (a que a imagem de cloud cria)
#   CHAVE       chave SSH privada dessa conta
#   PACOTE      pasta do pacote de release `amd64` (scripts/release-bundle.sh)
#   IMPRESSAO   impressão digital SHA256 da chave ed25519 do servidor, LIDA NA
#               CONSOLA DO FORNECEDOR — nunca pela ligação que se vai verificar
#
# Estado: NOT_RUN. Este guião existe para que a certificação seja um comando; não
# correu ainda contra nenhuma VM externa, e o primeiro uso é também a sua prova.
#
# # Só uma VM descartável
#
# O Installer instala um sistema e abre portas. Este guião recusa correr sem
# OCINYE_D011_EXTERNAL_VM_IS_DISPOSABLE=yes, recusa um anfitrião que não seja
# Ubuntu 24.04 `x86_64` e recusa um onde já exista Ocinye ou Docker a correr
# contentores. Nunca um servidor de produção.
#
# Tudo o que é de teste fica em ~/.cache/ocinye-installer-test/external-<host>,
# fora do repositório. O resultado é uma linha:
#   RESULT d011-external-amd64 PASS|FAIL|INVALID
set -euo pipefail

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CLI="$RAIZ/target/debug/ocinye-installer-cli"
host="${1:?HOST}"; user="${2:?UTILIZADOR}"; key="${3:?CHAVE}"; pacote="${4:?PACOTE}"; fp="${5:?IMPRESSAO}"

fim() { printf 'RESULT d011-external-amd64 %s %s\n' "$1" "${2:-}"; [ "$1" = PASS ] && exit 0; [ "$1" = FAIL ] && exit 1; exit 2; }

[ "${OCINYE_D011_EXTERNAL_VM_IS_DISPOSABLE:-}" = yes ] || fim INVALID "defina OCINYE_D011_EXTERNAL_VM_IS_DISPOSABLE=yes: a VM vai ser alterada"
[[ "$host" =~ ^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+$ ]] || fim INVALID "HOST tem de ser um endereço IPv4"
[[ "$fp" =~ ^SHA256:[A-Za-z0-9+/]{43}$ ]] || fim INVALID "IMPRESSAO tem de ser SHA256:… (43 caracteres)"
[ -r "$key" ] || fim INVALID "chave ilegível: $key"
[ -r "$pacote/MANIFEST.json" ] || fim INVALID "não é um pacote com MANIFEST.json: $pacote"
python3 - "$pacote/MANIFEST.json" <<'PY' || fim INVALID "o pacote não é amd64"
import json,sys
sys.exit(0 if json.load(open(sys.argv[1]))["target"]["arch"]=="amd64" else 1)
PY

d="$HOME/.cache/ocinye-installer-test/external-$host"
mkdir -p "$d"; chmod 700 "$d"
exec > >(tee "$d/certification.log") 2>&1
echo "D011 external amd64 certification · $(date -u +%Y-%m-%dT%H:%M:%SZ) · commit $(git -C "$RAIZ" rev-parse HEAD)"

# A chave do servidor: a que ele apresenta tem de ser a que o operador leu na
# consola. Só então fica fixada para as ligações de gestão deste guião.
ssh-keyscan -T 15 -t ed25519 "$host" > "$d/known_hosts" 2>/dev/null || fim INVALID "o servidor não respondeu ao ssh-keyscan"
visto="$(ssh-keygen -lf "$d/known_hosts" | awk '{print $2}')"
[ "$visto" = "$fp" ] || fim FAIL "a chave apresentada ($visto) não é a da consola ($fp)"
rsh() { ssh -i "$key" -o IdentitiesOnly=yes -o BatchMode=yes -o StrictHostKeyChecking=yes \
            -o UserKnownHostsFile="$d/known_hosts" -o ConnectTimeout=20 "$user@$host" "$@"; }

echo "== linha de base da VM"
rsh 'set -e; . /etc/os-release; echo "$ID $VERSION_ID $(uname -m)"; sudo -n true' > "$d/baseline.txt" || fim INVALID "sem acesso SSH com sudo sem palavra-passe"
cat "$d/baseline.txt"
grep -q '^ubuntu 24\.04 x86_64$' "$d/baseline.txt" || fim INVALID "não é Ubuntu 24.04 x86_64"
rsh 'test ! -e /etc/ocinye && test ! -e /srv/ocinye' || fim INVALID "já existe Ocinye neste anfitrião"
rsh 'if command -v docker >/dev/null 2>&1; then test -z "$(sudo -n docker ps -q)"; fi' || fim INVALID "há contentores a correr neste anfitrião"
rsh 'df -BG --output=avail / | tail -1; free -m | sed -n 2p; systemctl list-units --type=service --state=running --no-legend | wc -l; dpkg -l | grep -c "^ii"' > "$d/measure-before.txt"

echo "== configuração (auto-assinado, DNS controlado para nomes .test)"
cat > "$d/installer.json" <<EOF
{
  "bundle": "$pacote",
  "state_dir": "$d/state",
  "host": "$host",
  "user": "$user",
  "key": "$key",
  "trust_fingerprint": "$fp",
  "instance_name": "Instância de Certificação D011",
  "distributions": ["business", "research"],
  "canonical": "os.d011.test",
  "bound": [
    {"host": "business.d011.test", "distribution": "business"},
    {"host": "research.d011.test", "distribution": "research"}
  ],
  "resolve": {"os.d011.test": ["$host"], "business.d011.test": ["$host"], "research.d011.test": ["$host"]},
  "tls": "self-signed",
  "credential_file": "first-admin-credential",
  "admin": {
    "person": {"name": "Pessoa de Prova", "email": "pessoa@d011.test"},
    "privileged": {"name": "Pessoa de Prova (Admin)", "email": "admin@d011.test"}
  }
}
EOF
chmod 600 "$d/installer.json"

( cd "$RAIZ" && cargo build --locked -p ocinye-installer-controller --bin ocinye-installer-cli ) || fim INVALID "o ocinye-installer-cli não compilou"

echo "== amostragem de argumentos (prova de segredos)"
rsh 'sudo -n systemd-run --unit ocinye-argv-watch --quiet sh -c "while :; do ps -eo args >> /root/ocinye-argv-samples; sleep 0.2; done"' || fim INVALID "não foi possível iniciar a amostragem"

echo "== preflight"
"$CLI" preflight "$d/installer.json" > "$d/preflight.jsonl" || { tail -5 "$d/preflight.jsonl"; fim FAIL "preflight recusou ou bloqueou (ver $d/preflight.jsonl)"; }

echo "== instalação"
set +e
"$CLI" install "$d/installer.json" > "$d/install.jsonl"; rc=$?
set -e
rsh 'sudo -n systemctl stop ocinye-argv-watch' || true
estado="$(python3 - "$d/install.jsonl" <<'PY'
import json,sys
s=""
for l in open(sys.argv[1]):
    try: v=json.loads(l)
    except ValueError: continue
    if v.get("kind")=="lifecycle": s=v["value"].get("state","")
print(s)
PY
)"
echo "código de saída do instalador: $rc · estado final: ${estado:-nenhum}"

echo "== depois"
rsh 'df -BG --output=avail / | tail -1; free -m | sed -n 2p; systemctl list-units --type=service --state=running --no-legend | wc -l; dpkg -l | grep -c "^ii"; sudo -n docker ps --format "{{.Names}} {{.Status}}"' > "$d/measure-after.txt" || true
cat "$d/measure-after.txt"

echo "== auditoria de segredos"
t="$(mktemp -d)"; mkdir -p "$t/w/secrets"
tar -C "$d" -cf "$t/w/operator.tar" --exclude first-admin-credential --exclude 'state/first-admin-credential' .
[ -f "$d/state/first-admin-credential" ] && cp "$d/state/first-admin-credential" "$t/w/secrets/first-admin-credential"
cp "$RAIZ/infra/installer-test/secret-audit.py" "$t/w/"
w="/tmp/ocinye-audit-$RANDOM$RANDOM"
scp -q -r -i "$key" -o IdentitiesOnly=yes -o StrictHostKeyChecking=yes -o UserKnownHostsFile="$d/known_hosts" "$t/w" "$user@$host:$w"
rm -r "$t"
set +e
rsh "sudo -n sh -c 'chown -R root:root $w && chmod -R go-rwx $w && python3 $w/secret-audit.py $w; r=\$?; rm -r $w; exit \$r'" > "$d/secret-audit.txt"; audit=$?
set -e
cat "$d/secret-audit.txt"

echo "== veredicto"
[ "$rc" = 0 ] || fim FAIL "o instalador saiu com $rc (ver $d/install.jsonl)"
[ "$estado" = INSTALLED_TEST_MODE ] || fim FAIL "estado final $estado, esperado INSTALLED_TEST_MODE"
[ "$audit" = 0 ] || fim FAIL "a auditoria encontrou segredos (ver $d/secret-audit.txt)"
fim PASS "evidência em $d"
