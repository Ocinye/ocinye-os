#!/usr/bin/env bash
# A prova do Ocinye OS Installer (D011) numa VM descartável, com o controlador
# real (ocinye-installer-cli), o executor real (ocinye-bootstrap) e um pacote
# de release real.
#
#   scripts/installer-e2e.sh config  VM PACOTE [self-signed|provided]   escreve a configuração
#   scripts/installer-e2e.sh trust   VM          mostra a chave do servidor e fixa-a, explicitamente
#   scripts/installer-e2e.sh resolve VM          os nomes .test passam a resolver para a VM (DNS controlado)
#   scripts/installer-e2e.sh argv-watch VM start|stop   amostra os argumentos de todos os processos (prova de segredos)
#   scripts/installer-e2e.sh audit   VM          procura cada segredo em tudo o que fica ou se imprime (só contagens)
#   scripts/installer-e2e.sh run     VM [args…]   corre o ocinye-installer-cli com a configuração
#
# A VM tem de ser uma das de scripts/installer-vm.sh (ocinye-d011-…, criada
# por ele): nunca um servidor de produção, nunca um anfitrião adivinhado.
# Tudo o que é de teste — configuração, chaves, certificados sintéticos,
# recibos — fica em ~/.cache/ocinye-installer-test/VM, fora do repositório.
set -euo pipefail

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CACHE="${HOME}/.cache/ocinye-installer-test"
CLI="$RAIZ/target/debug/ocinye-installer-cli"

fatal() { printf '\n  RECUSADO — %s\n\n' "$1" >&2; exit 1; }

vm_valida() {
    [[ "$1" =~ ^ocinye-d011-[a-z0-9-]{1,40}$ ]] || fatal "a VM tem de ser ocinye-d011-…: $1"
    [ -f "$CACHE/$1/created-by-installer-vm" ] || fatal "$1 não foi criada por scripts/installer-vm.sh"
}

case "${1:-}" in
    config)
        vm="${2:?VM}"; pacote="${3:?pacote}"; tls="${4:-self-signed}"; vm_valida "$vm"
        d="$CACHE/$vm"; ip="$(cat "$d/ip")"
        [ -r "$pacote/MANIFEST.json" ] || fatal "não é um pacote com MANIFEST.json: $pacote"
        if [ "$tls" = provided ]; then
            [ -r "$d/tls/cert.pem" ] || fatal "faltam os certificados sintéticos em $d/tls (cert.pem, key.pem, chain.pem)"
            tls_json="{\"cert\": \"$d/tls/cert.pem\", \"key\": \"$d/tls/key.pem\", \"chain\": \"$d/tls/chain.pem\"}"
        else
            tls_json='"self-signed"'
        fi
        # Sudo sem palavra-passe: não há ficheiro, e a configuração não o nomeia.
        sudo_linha=""
        [ -f "$d/sudo-password" ] && sudo_linha="  \"sudo_password_file\": \"$d/sudo-password\",
"
        cat > "$d/installer.json" <<EOF
{
  "bundle": "$pacote",
  "state_dir": "$d/state",
  "host": "$ip",
  "user": "operador",
  "key": "$d/id_ed25519",
$sudo_linha  "instance_name": "Instância de Prova D011",
  "distributions": ["business", "research"],
  "canonical": "os.d011.test",
  "bound": [
    {"host": "business.d011.test", "distribution": "business"},
    {"host": "research.d011.test", "distribution": "research"}
  ],
  "tls": $tls_json,
  "credential_file": "first-admin-credential",
  "admin": {
    "person": {"name": "Pessoa de Prova", "email": "pessoa@d011.test"},
    "privileged": {"name": "Pessoa de Prova (Admin)", "email": "admin@d011.test"}
  }
}
EOF
        chmod 600 "$d/installer.json"
        echo "  $d/installer.json"
        ;;
    trust)
        vm="${2:?VM}"; vm_valida "$vm"; d="$CACHE/$vm"
        out="$("$CLI" fingerprint "$d/installer.json")" || true
        fp="$(printf '%s\n' "$out" | python3 -c 'import json,sys
for l in sys.stdin:
    v=json.loads(l)
    if v["kind"]=="host_key_unknown": print(v["value"]["fingerprint"])')"
        [ -n "$fp" ] || { printf '%s\n' "$out"; fatal "o servidor não apresentou uma chave nova (já fixada?)"; }
        # A chave que a VM diz ter, lida pelo canal de gestão do Lima — e não
        # pela mesma ligação que se está a verificar.
        local_fp="$(limactl shell --workdir / "$vm" -- ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub | awk '{print $2}')"
        [ "$fp" = "$local_fp" ] || fatal "a chave apresentada ($fp) não é a da VM ($local_fp)"
        python3 - "$d/installer.json" "$fp" <<'PY'
import json,sys
p,fp=sys.argv[1],sys.argv[2]
c=json.load(open(p)); c["trust_fingerprint"]=fp
json.dump(c,open(p,"w"),indent=2)
PY
        echo "  confiança explícita: $fp"
        ;;
    resolve)
        vm="${2:?VM}"; vm_valida "$vm"; d="$CACHE/$vm"
        python3 - "$d/installer.json" "$(cat "$d/ip")" <<'PY'
import json,sys
p,ip=sys.argv[1],sys.argv[2]
c=json.load(open(p))
hosts=[c["canonical"]]+[b["host"] for b in c.get("bound",[])]
c["resolve"]={h:[ip] for h in hosts}
json.dump(c,open(p,"w"),indent=2)
PY
        echo "  DNS controlado: $(cat "$d/ip")"
        ;;
    argv-watch)
        vm="${2:?VM}"; vm_valida "$vm"
        case "${3:?start|stop}" in
            start) limactl shell --workdir / "$vm" -- sudo systemd-run --unit ocinye-argv-watch --quiet \
                       sh -c 'while :; do ps -eo args >> /root/ocinye-argv-samples; sleep 0.2; done'
                   echo "  a amostrar argumentos" ;;
            stop)  limactl shell --workdir / "$vm" -- sudo systemctl stop ocinye-argv-watch
                   limactl shell --workdir / "$vm" -- sudo sh -c 'echo "  amostras: $(grep -c "^COMMAND" /root/ocinye-argv-samples)"' ;;
        esac
        ;;
    audit)
        vm="${2:?VM}"; vm_valida "$vm"; d="$CACHE/$vm"
        t="$(mktemp -d)"; mkdir -p "$t/secrets"
        tar -C "$d" -cf "$t/operator.tar" --exclude id_ed25519 --exclude sudo-password \
            --exclude first-admin-credential --exclude 'state/first-admin-credential' .
        [ -f "$d/sudo-password" ] && cp "$d/sudo-password" "$t/secrets/sudo-password"
        [ -f "$d/state/first-admin-credential" ] && cp "$d/state/first-admin-credential" "$t/secrets/first-admin-credential"
        cp "$RAIZ/infra/installer-test/secret-audit.py" "$t/"
        w="/tmp/ocinye-audit-$RANDOM$RANDOM"
        limactl copy -r "$t" "$vm:$w" >/dev/null
        rm -rf "$t"
        set +e
        limactl shell --workdir / "$vm" -- sudo sh -c "chown -R root:root $w && chmod -R go-rwx $w && python3 $w/secret-audit.py $w; r=\$?; rm -rf $w; exit \$r"
        r=$?; set -e
        exit $r
        ;;
    run)
        vm="${2:?VM}"; vm_valida "$vm"; shift 2
        cmd="${1:?comando}"; shift
        "$CLI" "$cmd" "$CACHE/$vm/installer.json" "$@"
        ;;
    *)
        sed -n '2,17p' "$0"; exit 2 ;;
esac
