#!/usr/bin/env bash
#
# Uma suite só é prova se os testes que se esperava dela foram descobertos e
# correram.
#
# # Porque é que isto existe
#
# `cargo test` devolve zero quando nada falhou. Não devolve nada sobre o que
# **não correu**. Um teste que retorna cedo — por uma pré-condição em falta, por
# um `.ok()?` num arranque, por um `else { return }` — é contado como passado e
# nunca aparece como salto.
#
# No Ocinye isto escondeu treze das catorze viagens de browser durante uma
# milestone inteira. Todas partilhavam o directório de perfil do Chrome, só a
# primeira arrancava, e as outras saíam em silêncio a dizer `ok`. A CI ficou
# verde e a ADR-0410 foi aceite com uma linha de prova que dizia catorze.
#
# O Calendar não estava errado — quando as catorze correram de facto, passaram
# todas. O que estava errado era a **evidência** com que a cobertura foi
# declarada. É essa a classe que isto fecha.
#
# # O contrato
#
#     esperados == passados        descobertos == passados + ignorados        saltados == 0
#
# `esperados` vem da tabela abaixo, que é deliberada: o número muda quando
# alguém decide mudá-lo, e não quando uma suite encolhe sozinha. É o número de
# testes que têm de **passar**, e não o número que existe.
#
# `ignorados` conta à parte de propósito. Um `#[ignore]` é descoberto e não é
# prova, e um teste que passe a ignorado sem ninguém reparar tira cobertura
# exactamente como um que se salta — a diferença é que este foi uma decisão.
# Fica visível para que continue a ser uma.
#
# `descobertos` vem de `--list`, que enumera sem correr. É o que separa «a suite
# encolheu» de «a suite falhou»: se um ficheiro de testes deixar de compilar
# para dentro do alvo, ou um `#[cfg]` os apagar, a descoberta baixa e o exit
# code não muda.
#
# `passados` vem da execução — e não chega.
#
# Isto foi verificado por reversão, e a primeira versão deste contrato **não
# apanhava** o defeito que o motivou: com o salto silencioso de volta, os treze
# testes saíam cedo, imprimiam `... ok`, e não emitiam marca de salto nenhuma.
# Catorze descobertos, catorze «passados», zero saltados, e treze por correr.
#
# A ausência de uma marca de salto não é prova de execução. Por isso uma suite
# pode declarar uma **marca positiva**, emitida no ponto em que já não é
# possível sair sem correr, e o contrato exige que apareça uma vez por teste:
#
#     marcas == esperados
#
# As viagens de browser emitem `VIAGEM LEVANTADA` quando o harness levanta o
# Core, o Workspace e o browser. Um teste que não chegue lá não a imprime.
#
# O número de marcas não tem de ser igual ao de testes, e aqui não é: a suite
# tem quinze testes e emite quinze marcas, e as duas contagens baterem é
# coincidência, não identidade:
#
#     13 testes levantam um browser cada                  13 marcas
#      1 controlo visual levanta dois — um por estado      2 marcas
#      1 teste estrutural lê o ficheiro do ecrã            0 marcas
#     ──                                                  ──
#     15 testes                                           15 marcas
#
# Exigir uma marca por teste seria exigir um browser ao teste estrutural, que
# nunca precisou de um, e perdoar ao controlo visual metade do trabalho que
# faz. A marca conta execuções, não testes.

set -euo pipefail

cd "$(dirname "$0")/.."

falhas=0

# ── A fonte de expectativa ──────────────────────────────────────────────────
#
# Cada linha: <nome> <esperados> <invocação...>
#
# Só suites críticas. Não é o inventário de tudo o que se testa: é a lista das
# suites cuja contagem, se cair sem ninguém reparar, faz uma afirmação de
# cobertura passar a ser falsa.
#
# Mudar um número aqui é uma decisão. Um número que precisa de descer merece a
# pergunta de porquê.
suites() {
    cat <<'TABELA'
# 75 viagens e 74 marcas, e os números **não** têm de coincidir.
#
# A marca conta levantamentos de harness, não viagens. Duas das 75 são análise
# estática sobre a árvore e não abrem browser nenhum; e
# `a_consolidacao_nao_mudou_o_que_a_pessoa_ve` levanta **dois** harnesses — um
# com os estáticos actuais, outro com o estado anterior — porque a propriedade
# que mede é a comparação entre os dois. 73 viagens × 1 + 1 extra = 74.
#
# 61 desde 2026-08-29: entraram três viagens de Ficheiros — navegar e organizar,
# a recusa a quem tem o identificador e não o acesso, e o largar de bytes a
# sério até ao PostgreSQL. A terceira **só prova alguma coisa com
# `OCINYE_TEST_STORAGE_ENDPOINT` definida**; sem ela levanta o harness, diz em
# voz alta que se saltou, e a marca continua a contar — pelo que este portão
# mede que ela correu, não que os bytes atravessaram.
#
# 62 desde 2026-08-29: entrou a viagem que confirma que uma imagem institucional
# carrega com `img-src 'self'` — mede `naturalWidth` no Chrome, porque um `<img>`
# recusado pela CSP continua a existir no HTML.
#
# 75 desde 2026-08-30: entraram o bootstrap de unidade com gestão de pertenças
# pelo produto, a recusa dos mesmos controlos a quem não gere (incluindo por HTTP
# directo), e a vista agregada de ficheiros a atravessar ambientes sem vazar o
# alheio.
#
# 72 desde 2026-08-30: entraram as duas viagens da relevância de módulo — uma
# conta de investigação sem pertenças vê os quatro módulos de CONHECIMENTO, e um
# colaborador externo não os ganha.
#
# 70 desde 2026-08-30: entraram as duas viagens da navegação de CONHECIMENTO —
# quem pertence a um ambiente vê as entradas como navegação, e ver a entrada não
# dá acesso a ficheiro nenhum.
#
# 68 desde 2026-08-29: entrou a viagem que fecha as citações — carregar, citar,
# abrir a versão citada, carregar uma versão nova, e voltar a abrir a mesma
# citação para exigir que continue a mostrar os bytes que citou.
#
# 67 desde 2026-08-29: entraram a paráfrase que só a recuperação semântica
# encontra — com controlo lexical a zero, para não haver dúvida sobre qual das
# duas metades trabalhou — e a que exige que a semântica indisponível seja
# declarada e nunca descrita como avaria.
#
# 65 desde 2026-08-29: entrou também a viagem que exige que a pré-visualização
# mostre exactamente o texto que a pesquisa encontra — um caminho só, e não dois
# que podem divergir.
#
# 64 desde 2026-08-29: entraram as duas viagens de extracção de conteúdo — a
# frase que só existe no corpo de um PDF, e o formato que se guarda mas não se
# lê. Ambas exigem armazenamento; sem ele, falham na CI em vez de se saltarem.
#
# 77 desde 2026-08-30: entrou a frescura da pertença nos dois sentidos, na mesma
# sessão viva. O marcador é a própria pessoa na lista de membros: o nome da
# unidade estaria sempre lá — o detalhe é legível a quem investiga, tenha ou não
# pertença — e a barra de topo escreve o nome de quem está autenticado.
#
# 76 desde 2026-08-30: entrou a viagem da conta suspensa a meio da sessão. A
# pertença sobrevive à suspensão e a autoridade não — e verificou-se por
# reversão que só desligando **quatro** camadas independentes é que a unidade
# volta a ser legível a quem foi suspenso.
#
# Auditado em 2026-08-29, em série, marca a marca. O número continua fixo: uma
# viagem que deixe de levantar faz a contagem cair e o portão fecha.
viagens-de-browser|77|-p ocinye-workspace --test browser|VIAGEM LEVANTADA|76
paridade|7|-p ocinye-core-server --test parity
verificador-de-tokens|31|-p ocinye-core --test authn
autorizacao|12|-p ocinye-core --test authorization
catalogo-de-operacoes|13|-p ocinye-core --lib operations
estado-do-correio|3|-p ocinye-core-server --test mail_status_http
isolamento-de-caixas|10|-p ocinye-core --test mailbox_isolation
validacao-cientifica|6|-p ocinye-core --test scientific_validation
linhagem-cientifica|3|-p ocinye-core --test scientific_lineage
TABELA
}

verifica() {
    local nome="$1" esperados="$2" invocacao="$3" marca="${4:-}" marcas_esperadas="${5:-0}"
    local saida descobertos passados ignorados saltados marcas

    # shellcheck disable=SC2086
    descobertos=$(cargo test $invocacao -- --list 2>/dev/null | grep -c ': test$' || true)

    saida=$(mktemp)
    # shellcheck disable=SC2086
    if ! cargo test $invocacao -- --nocapture >"$saida" 2>&1; then
        printf '  %-24s a suite falhou\n' "$nome" >&2

        # O pânico primeiro, e só depois o fim.
        #
        # Com `--nocapture` a mensagem de pânico sai **no momento em que
        # acontece**, e não no bloco de resumo. Numa suite de cinquenta e seis
        # testes isso deixa-a a meio do ficheiro, e um `tail -20` mostrava só
        # a lista de nomes que falharam — «a suite falhou», sem dizer porquê.
        # Uma viagem de browser intermitente ficava por classificar entre
        # defeito e infraestrutura, que são coisas diferentes.
        if grep -q "panicked at" "$saida"; then
            echo "      ── onde falhou ──" >&2
            grep -n -A 4 "panicked at" "$saida" | sed 's/^/      /' | head -40 >&2
            echo "      ── fim da corrida ──" >&2
        fi
        sed 's/^/      /' "$saida" | tail -20 >&2
        rm -f "$saida"
        falhas=$((falhas + 1))
        return
    fi

    passados=$(grep -cE '^test .+ \.\.\. ok$' "$saida" || true)
    ignorados=$(grep -cE '^test .+ \.\.\. ignored' "$saida" || true)
    # Dois marcadores, porque há duas línguas a dizer a mesma coisa.
    #
    # O portão procurava só `skipping:` — o marcador em inglês do arranque do
    # browser. As viagens que dependem de object storage escrevem `SALTADO:`,
    # e por isso sete delas passavam por verdes sem que o portão as visse.
    # Um teste saltado reporta ok; se o portão não conhece o marcador, o
    # portão também reporta ok.
    saltados=$(grep -cE '^(skipping:|SALTADO:)' "$saida" || true)
    marcas=0
    [ -n "$marca" ] && marcas=$(grep -cF "$marca" "$saida" || true)
    rm -f "$saida"

    if [ "$((descobertos - ignorados))" -ne "$esperados" ]; then
        printf '  %-24s descobertos %s, ignorados %s, esperados %s\n' \
            "$nome" "$descobertos" "$ignorados" "$esperados" >&2
        echo "      A suite encolheu ou cresceu sem que a expectativa mudasse." >&2
        echo "      Se foi de propósito, actualize a tabela em $0." >&2
        falhas=$((falhas + 1))
        return
    fi

    if [ "$saltados" -gt 0 ]; then
        printf '  %-24s %s teste(s) saltaram-se\n' "$nome" "$saltados" >&2
        echo "      Um teste saltado reporta ok e conta como cobertura que não existe." >&2
        falhas=$((falhas + 1))
        return
    fi

    if [ "$passados" -ne "$esperados" ]; then
        printf '  %-24s passaram %s, esperados %s\n' "$nome" "$passados" "$esperados" >&2
        falhas=$((falhas + 1))
        return
    fi

    if [ -n "$marca" ] && [ "$marcas" -ne "$marcas_esperadas" ]; then
        printf '  %-24s correram %s, passaram %s, esperadas %s execuções\n' \
            "$nome" "$marcas" "$passados" "$marcas_esperadas" >&2
        echo "      A suite passou sem que todos os testes tivessem corrido." >&2
        echo "      A marca é emitida no ponto em que já não é possível sair" >&2
        echo "      sem correr, e apareceu $marcas vez(es) em $marcas_esperadas." >&2
        falhas=$((falhas + 1))
        return
    fi

    local nota=""
    [ "$ignorados" -gt 0 ] && nota="$nota · $ignorados ignorado(s)"
    [ -n "$marca" ] && nota="$nota · $marcas levantamentos de harness"
    printf '  %-24s %s esperados · %s descobertos · %s passados · 0 saltados%s\n' \
        "$nome" "$esperados" "$descobertos" "$passados" "$nota"
}

echo "Contrato de enumeração das suites críticas:"

while IFS='|' read -r nome esperados invocacao marca marcas_esperadas; do
    # Linhas em branco e comentários não são suites.
    #
    # Sem a segunda metade, um comentário dentro da tabela virava uma suite com
    # invocação vazia — e o contrato reportava dez falhas onde havia zero,
    # enterrando a única verdadeira no meio delas.
    [ -z "$nome" ] && continue
    case "$nome" in \#*) continue ;; esac
    verifica "$nome" "$esperados" "$invocacao" "${marca:-}" "${marcas_esperadas:-0}"
done < <(suites)

if [ "$falhas" -gt 0 ]; then
    echo >&2
    echo "$falhas suite(s) não provaram o que se espera delas." >&2
    echo >&2
    echo "Verde não chega. Uma suite só é prova se os testes que se esperava" >&2
    echo "dela foram descobertos e correram." >&2
    exit 1
fi
