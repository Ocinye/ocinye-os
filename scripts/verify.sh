#!/usr/bin/env bash
# Sweep de verificação do Ocinye OS.
#
# Corre o que a CI corre, na mesma ordem. Um sweep verde não significa que o
# sistema esteja completo — significa que aquilo que existe está consistente.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

step() { printf '\n\033[1m== %s ==\033[0m\n' "$1"; }

# ── Pureza da árvore ────────────────────────────────────────────────────
#
# A verificação observa o candidato; não o altera. Se algum passo tocar em
# código versionado — mesmo restaurando-o a seguir — este sweep falha no fim e
# diz o quê.
#
# Existe por causa de um incidente concreto: um harness de comparação visual
# trocava a folha de estilos versionada, uma execução deixou-a no estado
# anterior, e um commit seguinte capturou a reversão.
raiz_do_repositorio=$(git rev-parse --show-toplevel)
impressao_da_arvore() {
    python3 - <<'FIM' | sort
import os, subprocess
saida = subprocess.run(["git", "ls-files"], capture_output=True, text=True, check=True)
for nome in saida.stdout.splitlines():
    try:
        info = os.stat(nome)
    except OSError:
        print("%s AUSENTE" % nome)
        continue
    print("%s %d %d" % (nome, info.st_size, int(info.st_mtime)))
FIM
}

arvore_antes=$(impressao_da_arvore)
cabeca_antes=$(git -C "$raiz_do_repositorio" rev-parse HEAD)

step "Integridade do sistema de verificação"
# Primeiro de todos, e de propósito.
#
# Os portões seguintes verificam o Ocinye. Este verifica-os a eles: prova que um
# processo falhado não pode passar por dizer a palavra certa, que sair bem sem
# produzir prova é INVALID, que um portão em falta não conta como passado, e que
# um verificador que altere código versionado falha mesmo restaurando.
#
# Corre antes de qualquer compilação porque não faz sentido gastar minutos a
# construir um candidato cuja verificação não é de confiança.
./scripts/harness-integrity.sh

step "Fronteiras arquitecturais"
# O Core detém a verdade institucional; a Experience detém a sua apresentação;
# o Design System detém a consistência, nunca o domínio. Quatro portões com
# nome, e o mesmo significado aqui e na CI — ver docs/architecture/README.md.
./scripts/architecture-gates.sh

step "Consumidores das dependências"
# Uma dependência de produção que ninguém chama é lixo, uma promoção silenciosa
# de teste para produção, ou uma intenção por cumprir. As três custam, e a
# terceira custa em segurança: `subtle` esteve declarado no Core sem uma única
# comparação em tempo constante escrita.
python3 ./scripts/dependency_consumers.py

step "Ligações da documentação"
# A documentação é parte do contrato do sistema. Uma ligação que não abre ensina
# que a documentação não é de confiança, e a partir daí ninguém a lê.
python3 ./scripts/documentation_links.py

step "Consumidores do esquema"
# Uma tabela sem leitor é uma funcionalidade que saiu e deixou o esquema, ou um
# esquema que promete o que não existe. As migrations são história e não se
# apagam; o estado de cada uma tem de estar declarado.
python3 ./scripts/schema_consumers.py

step "Autoridade de escrita institucional"
# Nenhum crate fora do Core escreve no estado institucional. Dois sítios a
# escrever são dois sítios a decidir o que é verdade, e dois acabam por
# discordar. As duas excepções — registo de armazenamento no arranque, drenagem
# do outbox — estão declaradas no script com a razão.
python3 ./scripts/mutation_authority.py

step "Superfície de configuração"
# Uma variável lida e não documentada só existe para quem leu o código. Uma
# documentada e não lida é pior: promete um controlo que não existe.
python3 ./scripts/configuration_surface.py

step "Formatação"
cargo fmt --all -- --check

step "Clippy"
cargo clippy --workspace --all-targets -- -D warnings

step "Capacidades WASM"
./scripts/build-capabilities.sh >/dev/null
echo "componentes construídos"

step "Testes"
if [[ -n "${OCINYE_TEST_DATABASE_URL:-}" ]]; then
    echo "com base de dados: ${OCINYE_TEST_DATABASE_URL%%\?*}"
else
    echo "AVISO: OCINYE_TEST_DATABASE_URL não definida."
    echo "       Os testes de autorização vão saltar-se. A CI define-a sempre."
fi
cargo test --workspace --all-targets

step "Testes das capacidades"
cargo test --manifest-path wasm/capabilities/bibtex-import/Cargo.toml

step "Builds de release"
cargo build --release --workspace

step "Isolamento do fornecedor de teste"
# O `FixtureProvider` é infraestrutura de testes e não pode entrar no binário
# do servidor.
#
# A verificação é **estrutural**: pergunta ao Cargo com que features o binário
# do servidor resolve o `ocinye-core`. Um `strings` sobre o binário
# confirmaria o mesmo empiricamente, e falharia em silêncio se o optimizador
# mantivesse o código sem os literais.
#
# O padrão procura a linha do *pacote* — `ocinye-core v0.1.0 (…) [features]` —
# e não as linhas `ocinye-core feature "…"`, que descrevem arestas do grafo.
resolved="$(cargo tree -p ocinye-core-server -e features --format '{p} [{f}]' 2>/dev/null \
    | grep -E 'ocinye-core v[0-9]' || true)"

if [[ -z "$resolved" ]]; then
    echo "ERRO: não foi possível resolver as features de ocinye-core" >&2
    exit 1
fi

if printf '%s' "$resolved" | grep -q 'test-fixtures'; then
    echo "ENCONTRADO: o binário do servidor resolve ocinye-core com test-fixtures" >&2
    printf '  %s\n' "$resolved" >&2
    exit 1
fi
echo "o servidor não activa test-fixtures"

step "Catálogo de operações"
# Três guardas sobre a mesma afirmação: «duas entradas, uma operação».
#
#   1. `is_delegable_to_agents` ↔ catálogo — a guarda de arranque recusa
#      delegar `PermissionsManage`, `RolesManage` e `MembersManage`. Se o
#      catálogo classificasse como `Addressable` uma operação que essa guarda
#      recusa, o repositório teria duas respostas para a mesma pergunta.
#   2. O ContextEnvelope leva identificadores de capability e mais nada. É a
#      fronteira do modelo: o registry interno nunca atravessa para a inferência.
#   3. Toda a operação não-delegável declara a fronteira de confiança que
#      atravessa, tipada — não inferida de texto livre.
cargo test -p ocinye-core --lib operations -- --nocapture

step "Paridade das duas entradas"
# Conduz o router HTTP real e o executor de capabilities real, e compara o rasto
# que cada um deixou. Uma rota que ganhasse lógica própria — auditoria paralela,
# regra de domínio reimplementada — deixa de convergir e é apanhada aqui.
cargo test -p ocinye-core-server --test parity -- --nocapture

step "Matriz de operações"
./scripts/operation-matrix.sh --check

step "Biblioteca de ADRs"
# Estrutura, não arquitectura: nomes, identificadores únicos, metadata
# obrigatória, valores conhecidos, e que toda a dependência e substituição
# resolva. Se um domínio está certo é pergunta para uma pessoa.
python3 scripts/check-adrs.py

step "Contrato de autoria"
# Os commits pertencem exclusivamente aos autores humanos (`CLAUDE.md` §72).
#
# A regra existia desde o primeiro dia e nunca teve portão: duas identidades
# entraram na mesma — um endereço de outra organização e um bot de dependências
# — e tornaram-se permanentes assim que foram publicadas, porque uma referência
# de Pull Request guarda o commit para sempre. Retirá-las obrigou a recriar o
# repositório.
python3 scripts/authorship-contract.py

step "Factos da documentação"
# Propriedades textuais que envelhecem em silêncio: a definição canónica existe
# uma vez, e uma afirmação que deixou de ser verdade não volta.
#
# Só texto. Propriedades estruturais medem-se onde a estrutura existe — no
# catálogo tipado, no registry, na matriz. Inferir arquitectura por substring já
# custou uma regressão.
python3 scripts/documentation-facts.py

step "Contrato da Secção 1"
# Os números do CLAUDE.md §1 são os da árvore.
#
# Derivá-los não chega: um número derivado que ninguém confronta com o
# documento é um número que o documento pode contradizer à vontade — e foi
# assim que quatro contagens envelheceram em silêncio.
python3 scripts/section-one-contract.py

step 'Contrato da protecção de `main`'
# Os nomes dos *required checks* correspondem a jobs que existem.
#
# Renomear um job não falha nada: o check exigido deixa de ser reportado e o
# Pull Request fica à espera de um resultado que ninguém produz. Um merge
# bloqueado indefinidamente, com o aspecto de uma CI lenta.
python3 scripts/branch_protection_contract.py

step "Segredos"
# Duas varreduras, e não uma. O grep abaixo é a regra do próprio projecto; o
# gitleaks traz cerca de cento e cinquenta regras que ninguém aqui escreveria à
# mão. A CI corre ambas sempre; localmente, o gitleaks corre quando existir.
if command -v gitleaks >/dev/null 2>&1; then
    gitleaks dir . --no-banner --redact --exit-code 1
else
    echo "AVISO: gitleaks não está instalado; só a varredura do projecto correu."
    echo "       A CI corre ambas. Instalar: brew install gitleaks"
fi

if grep -rniE "(password|secret|api[_-]?key|token)[[:space:]]*[:=][[:space:]]*['\"][A-Za-z0-9+/=_-]{16,}" \
     --include='*.rs' --include='*.toml' --include='*.yml' --include='*.json' \
     . 2>/dev/null | grep -v '^./target' | grep -viE 'CHANGE_ME|_dev_only|_ci_only|example|placeholder'; then
    echo "ENCONTRADO: possível segredo no repositório" >&2
    exit 1
fi
echo "nenhum segredo encontrado"

step "Contrato de enumeração"
# Verde não chega. Uma suite só é prova se os testes que se esperava dela foram
# descobertos e correram — ver CLAUDE.md §59.
./scripts/test-enumeration.sh

step "Versões vulneráveis conhecidas"
# Não é um scanner. É a lista curta das vulnerabilidades que este repositório já
# viveu, e a pergunta é só uma: alguma coisa nos trouxe de volta a uma versão
# que já nos mordeu? Corre offline e em milissegundos, e por isso vem primeiro.
./scripts/known-vulnerable-versions.sh

step "Política dos portões de segurança"
# Os avaliadores de política, contra fixtures. Sem rede: o que aqui se testa é a
# decisão, não o que as bases de advisories por acaso contêm hoje.
python3 scripts/test_supply_chain.py

step "Advisories RustSec"
# A CI corre `cargo audit` sempre. Localmente, a ferramenta pode não
# estar instalada — e obrigar a instalá-la para correr o sweep seria atrito sem
# proveito. Corre quando existe, e diz o que falta quando não.
#
# As excepções estão em `.cargo/audit.toml`, cada uma com a razão escrita.
#
# Isto cobre a base RustSec, e só essa. A base do GitHub é outra, e tem o seu
# passo a seguir — ver ADR-0105.
if command -v cargo-audit >/dev/null 2>&1; then
    cargo audit
else
    echo "AVISO: cargo-audit não está instalado; a auditoria de dependências"
    echo "       não correu aqui. A CI corre-a sempre."
    echo "       Instalar: cargo install cargo-audit --locked"
fi

step "Advisories do GitHub"
# A outra base. Precisa de rede e do `gh` autenticado; quando não os há, diz-se
# que não correu, e nunca que não encontrou nada — não é a mesma coisa.
if command -v gh >/dev/null 2>&1 && gh auth status >/dev/null 2>&1; then
    python3 scripts/advisory_gate.py --lock Cargo.lock
else
    echo "AVISO: o GitHub não pôde ser consultado aqui; a base de advisories do"
    echo "       GitHub NÃO foi verificada. Isto não é o mesmo que zero"
    echo "       vulnerabilidades. A CI verifica-a sempre."
fi

step "Docker Compose"
docker compose -f infra/compose/docker-compose.yml config --quiet
echo "compose válido"

step "Pureza da árvore"
arvore_depois=$(impressao_da_arvore)
cabeca_depois=$(git -C "$raiz_do_repositorio" rev-parse HEAD)

if [ "$cabeca_antes" != "$cabeca_depois" ]; then
    echo "A VERIFICAÇÃO MUDOU O HEAD: $cabeca_antes → $cabeca_depois" >&2
    exit 1
fi
if [ "$arvore_antes" != "$arvore_depois" ]; then
    echo "A VERIFICAÇÃO ALTEROU CÓDIGO VERSIONADO" >&2
    diff <(echo "$arvore_antes") <(echo "$arvore_depois") | grep '^[<>]' | head -10 >&2
    echo >&2
    echo "Uma ferramenta de observação não precisa de alterar o que observa." >&2
    exit 1
fi
echo "nenhum ficheiro versionado foi alterado pela verificação"

printf '\n\033[1mSweep concluído.\033[0m\n'
