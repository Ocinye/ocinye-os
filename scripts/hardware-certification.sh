#!/usr/bin/env bash
# Certificação de hardware mínimo (Parte 15): medir, e não inventar, o que é
# «a máquina mais pequena» para o Ocinye OS **sem GPU**.
#
# Cada classe candidata é um anfitrião descartável com limites de CPU e memória
# (`docker run --cpus --memory`), onde se instala o pacote, se mede, e se corre a
# viagem de browser inteira. Uma classe só passa se instala, se a viagem passa, e
# se cumpre os limiares abaixo. Os números saem da corrida — e escrevem-se em
# `docs/install/hardware-results.md`, com a data, o release e a máquina que mediu.
#
# As classes são candidatas, não promessas: se uma falha, o mínimo sobe, e a
# página di-lo.
#
# Uso: scripts/hardware-certification.sh PASTA_DO_PACOTE [CLASSE...]
#      CLASSE = "cpus:memória", por exemplo "2:4g" "4:8g" (por omissão as duas)
set -euo pipefail

PACOTE="$(cd "${1:?indique a pasta do pacote}" && pwd)"; shift
CLASSES=("$@"); [ ${#CLASSES[@]} -gt 0 ] || CLASSES=("2:4g" "4:8g")
PORTO="${OCINYE_INSTALL_E2E_PORT:-18443}"
DOMINIO="os.instalacao.test"
RESULTADOS="docs/install/hardware-results.md"
# Limiares de aceitação: o que uma pessoa aceita esperar, em milissegundos, e o
# tempo máximo até a Instância estar saudável.
MAX_INSTALACAO_S=900
MAX_P95_MS=800
MAX_PASSO_MS=5000

passo() { printf '\n== %s ==\n' "$1"; }

p95() { sort -n | awk '{a[NR]=$1} END {i=int(NR*0.95); if (i<1) i=1; print a[i]}'; }
p50() { sort -n | awk '{a[NR]=$1} END {i=int(NR*0.5); if (i<1) i=1; print a[i]}'; }

linhas=()
for classe in "${CLASSES[@]}"; do
    cpus="${classe%%:*}"; memoria="${classe##*:}"
    nome="ocinye-hw-${cpus}c${memoria}"
    trabalho="$(mktemp -d)"
    passo "Classe $cpus vCPU / $memoria"
    docker rm -f "$nome" >/dev/null 2>&1 || true
    docker run -d --privileged --name "$nome" --cpus "$cpus" --memory "$memoria" \
        -p "127.0.0.1:$PORTO:443" -e DOCKER_TLS_CERTDIR= docker:27-dind >/dev/null
    for _ in $(seq 1 60); do docker exec "$nome" docker info >/dev/null 2>&1 && break; sleep 2; done
    docker exec "$nome" sh -c 'apk add --no-cache bash curl coreutils >/dev/null'
    docker cp "$PACOTE" "$nome:/root/pacote"

    estado="PASS"; razao=""
    inicio=$(date +%s)
    if ! docker exec "$nome" /root/pacote/install/ocinye install --domain "$DOMINIO" \
            --public-url "https://$DOMINIO:$PORTO" --instance-name "Certificação" --profile research \
            --name "Pessoa" --email pessoa@instalacao.test \
            --admin-name "Pessoa (Admin)" --admin-email admin@instalacao.test \
            --tls self-signed --credential-file /root/credencial > "$trabalho/instalacao.log" 2>&1; then
        estado="FAIL"; razao="a instalação falhou ($(tail -2 "$trabalho/instalacao.log" | tr '\n' ' '))"
    fi
    instalacao=$(( $(date +%s) - inicio ))

    memoria_repouso="—"; p50_ms="—"; p95_ms="—"; tempos="—"
    if [ "$estado" = PASS ]; then
        sleep 60   # repouso: a medição de memória não pode apanhar o arranque
        memoria_repouso="$(docker exec "$nome" docker stats --no-stream --format '{{.MemUsage}}' \
            | awk '{v=$1; u=v; gsub(/[0-9.]/,"",u); gsub(/[A-Za-z]/,"",v);
                    m=(u=="GiB")?v*1024:(u=="KiB")?v/1024:v; t+=m} END {printf "%.0f MiB", t}')"
        for _ in $(seq 1 200); do
            curl -ks -o /dev/null -w '%{time_total}\n' --resolve "$DOMINIO:$PORTO:127.0.0.1" \
                "https://$DOMINIO:$PORTO/login"
        done | awk '{printf "%d\n", $1*1000}' > "$trabalho/latencias"
        p50_ms="$(p50 < "$trabalho/latencias")"; p95_ms="$(p95 < "$trabalho/latencias")"
        docker cp "$nome:/root/credencial" "$trabalho/credencial"
        if OCINYE_TEST_INSTALLED_URL="https://$DOMINIO:$PORTO" OCINYE_TEST_INSTALLED_EMAIL=admin@instalacao.test \
           OCINYE_TEST_INSTALLED_CREDENTIAL_FILE="$trabalho/credencial" OCINYE_TEST_INSTALLED_PROFILE=research \
           OCINYE_TEST_INSTALLED_RESOLVE="$DOMINIO 127.0.0.1" OCINYE_TEST_INSTALLED_TIMINGS_FILE="$trabalho/tempos" \
             cargo test -q -p ocinye-workspace --test installed_instance -- --ignored --exact \
               uma_instancia_instalada_abre_entra_e_trabalha > "$trabalho/viagem.log" 2>&1; then
            tempos="$(tr '\n' ' ' < "$trabalho/tempos")"
            lento="$(awk -v m="$MAX_PASSO_MS" '$2 > m {print $1}' "$trabalho/tempos" | tr '\n' ' ')"
            [ -z "$lento" ] || { estado="FAIL"; razao="passos acima de ${MAX_PASSO_MS} ms: $lento"; }
        else
            estado="FAIL"; razao="a viagem de browser falhou"
        fi
        [ "$instalacao" -le "$MAX_INSTALACAO_S" ] || { estado="FAIL"; razao="instalação em ${instalacao}s"; }
        [ "$p95_ms" -le "$MAX_P95_MS" ] || { estado="FAIL"; razao="${razao:+$razao; }p95 ${p95_ms} ms"; }
    fi
    docker rm -f "$nome" >/dev/null 2>&1 || true
    rm -rf "$trabalho"
    echo "  $estado ${razao:+— $razao}"
    linhas+=("| $cpus vCPU · $memoria | $estado | ${instalacao}s | $memoria_repouso | $p50_ms / $p95_ms | $tempos | ${razao:-—} |")
done

{
    echo "# Resultados da certificação de hardware"
    echo
    echo "Gerado por \`scripts/hardware-certification.sh\` a $(date -u +%Y-%m-%d), release"
    echo "\`$(cat "$PACOTE/RELEASE")\` ($(cat "$PACOTE/BUILD" 2>/dev/null || echo release)),"
    echo "num anfitrião $(uname -m) com $(sysctl -n hw.ncpu 2>/dev/null || nproc) núcleos."
    echo "Sem GPU, sem fornecedor de IA. Limiares: instalação ≤ ${MAX_INSTALACAO_S}s,"
    echo "p95 da página de entrada ≤ ${MAX_P95_MS} ms, cada passo da viagem ≤ ${MAX_PASSO_MS} ms."
    echo
    echo "| Classe | Resultado | Instalação | Memória em repouso | Entrada p50 / p95 (ms) | Passos da viagem (ms) | Razão |"
    echo "|---|---|---|---|---|---|---|"
    printf '%s\n' "${linhas[@]}"
} > "$RESULTADOS"
printf '\n  Resultados em %s\n\n' "$RESULTADOS"
